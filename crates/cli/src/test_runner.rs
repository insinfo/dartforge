//! Native execution of ordinary package:test suites through its own Engine.
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs,
    io::{BufRead, BufReader},
    path::{Component, Path, PathBuf},
    process::{Command, ExitCode, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const USAGE: &str = "usage: dartforge test [<file|directory> ...] [--name <regex> | --plain-name <text>] [--reporter expanded|json] [--sdk <lib>] [--packages <package_config.json>] [--process-timeout <seconds>] [--keep-artifacts]\nRuns resolved package:test suites in native DartForge executables, serially.\nRequires resolved test, test_api and test_core dependencies. Library annotations, dart_test.yaml, browser platforms, coverage and watch mode are not supported yet.";

#[derive(Debug)]
struct Options {
    paths: Vec<PathBuf>,
    sdk: Option<PathBuf>,
    packages: Option<PathBuf>,
    filter: String,
    plain: bool,
    json: bool,
    timeout: Duration,
    keep: bool,
}

impl Options {
    fn parse(args: &[OsString]) -> Result<Self, String> {
        let mut options = Self {
            paths: Vec::new(),
            sdk: None,
            packages: None,
            filter: String::new(),
            plain: false,
            json: false,
            timeout: Duration::from_secs(120),
            keep: false,
        };
        let mut filter_seen = false;
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            let text = arg.to_str().ok_or("test: options must be valid UTF-8")?;
            match text {
                "--sdk" => {
                    options.sdk = Some(PathBuf::from(
                        it.next().ok_or("--sdk requires a library directory")?,
                    ))
                }
                "--packages" => {
                    options.packages = Some(PathBuf::from(
                        it.next()
                            .ok_or("--packages requires a package configuration")?,
                    ))
                }
                "--name" | "-n" | "--plain-name" => {
                    if filter_seen {
                        return Err("test: use only one --name or --plain-name filter".into());
                    }
                    filter_seen = true;
                    options.plain = text == "--plain-name";
                    options.filter = it
                        .next()
                        .and_then(|v| v.to_str())
                        .ok_or("test: the name filter requires text")?
                        .to_owned();
                }
                "--reporter" | "-r" => match it.next().and_then(|v| v.to_str()) {
                    Some("expanded") => options.json = false,
                    Some("json") => options.json = true,
                    _ => return Err("test: --reporter must be expanded or json".into()),
                },
                "--process-timeout" => {
                    let seconds = it
                        .next()
                        .and_then(|v| v.to_str())
                        .and_then(|v| v.parse::<u64>().ok())
                        .filter(|v| *v > 0)
                        .ok_or("test: --process-timeout requires positive seconds")?;
                    options.timeout = Duration::from_secs(seconds);
                }
                "--keep-artifacts" => options.keep = true,
                "--" => {
                    options.paths.extend(it.map(PathBuf::from));
                    break;
                }
                _ if text.starts_with('-') => {
                    return Err(format!("test: unsupported option {text}\n{USAGE}"));
                }
                _ => options.paths.push(PathBuf::from(arg)),
            }
        }
        if options.paths.is_empty() {
            options.paths.push(PathBuf::from("test"));
        }
        Ok(options)
    }
}

fn discover(path: &Path, files: &mut BTreeSet<PathBuf>, explicit: bool) -> std::io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_file() {
        if (explicit
            || path
                .file_name()
                .and_then(|v| v.to_str())
                .is_some_and(|v| v.ends_with("_test.dart")))
            && path.extension().is_some_and(|v| v == "dart")
        {
            files.insert(path.canonicalize()?);
        }
    } else if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if matches!(
                entry.file_name().to_str(),
                Some(".dart_tool" | ".git" | "build")
            ) {
                continue;
            }
            // A symlinked directory may point back into the discovery tree.
            if !entry.file_type()?.is_symlink() {
                discover(&entry.path(), files, false)?;
            }
        }
    }
    Ok(())
}

fn package_config(
    suite: &Path,
    explicit: Option<&Path>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(path) = explicit {
        return Ok(path.canonicalize()?);
    }
    for ancestor in suite.parent().into_iter().flat_map(Path::ancestors) {
        let path = ancestor.join(".dart_tool/package_config.json");
        if path.is_file() {
            return Ok(path.canonicalize()?);
        }
    }
    Err(format!(
        "test: no package_config.json for {}; resolve package:test dependencies first",
        suite.display()
    )
    .into())
}

fn check_suite(suite: &Path, config: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let configuration: Value = serde_json::from_slice(&fs::read(config)?)?;
    let packages = configuration["packages"]
        .as_array()
        .ok_or("test: invalid package configuration")?;
    for name in ["test", "test_api", "test_core"] {
        if !packages.iter().any(|p| p["name"].as_str() == Some(name)) {
            return Err(format!("test: package configuration must resolve package:{name}").into());
        }
    }
    let root = suite_root(suite)?;
    if root.join("dart_test.yaml").exists() {
        return Err(
            "test: dart_test.yaml is not supported yet; its settings cannot be silently ignored"
                .into(),
        );
    }
    let source = fs::read_to_string(suite)?;
    let mut interner = dartforge_intern::Interner::new();
    let parsed = dartforge_frontend::parser::parse(&source, &mut interner);
    if parsed
        .unit
        .directives
        .iter()
        .any(|d| !d.metadata.is_empty())
    {
        return Err(format!(
            "test: library annotations in {} are not supported yet",
            suite.display()
        )
        .into());
    }
    Ok(())
}

fn suite_root(suite: &Path) -> std::io::Result<PathBuf> {
    for ancestor in suite.parent().into_iter().flat_map(Path::ancestors) {
        if ancestor.join("pubspec.yaml").is_file() {
            return Ok(ancestor.to_owned());
        }
    }
    std::env::current_dir()
}

/// Relative source imports avoid depending on native loader file-URI support.
fn relative_import(from: &Path, to: &Path) -> Result<String, String> {
    let left: Vec<Component<'_>> = from.components().collect();
    let right: Vec<Component<'_>> = to.components().collect();
    let shared = left.iter().zip(&right).take_while(|(a, b)| a == b).count();
    if shared == 0 {
        return Err("test: temporary worker and suite must be on the same filesystem root".into());
    }
    let mut pieces = vec!["..".to_owned(); left.len() - shared];
    for part in &right[shared..] {
        pieces.push(
            part.as_os_str()
                .to_str()
                .ok_or("test: suite path must be valid UTF-8")?
                .to_owned(),
        );
    }
    Ok(pieces.join("/"))
}

fn dart_string_contents(text: &str) -> String {
    // JSON and Dart share these escapes, except interpolation and single quotes.
    let quoted = serde_json::to_string(text).expect("strings are JSON serializable");
    quoted[1..quoted.len() - 1]
        .replace('$', "\\$")
        .replace('\'', "\\'")
}

fn worker_source(directory: &Path, suite: &Path) -> Result<String, String> {
    let uri = dart_string_contents(&relative_import(directory, suite)?);
    let path = dart_string_contents(
        suite
            .to_str()
            .ok_or("test: suite path must be valid UTF-8")?,
    );
    let (before, rest) = include_str!("test_runner/worker.dart")
        .split_once("__DARTFORGE_SUITE_URI__")
        .ok_or("test: invalid worker template")?;
    let (middle, after) = rest
        .split_once("__DARTFORGE_SUITE_PATH__")
        .ok_or("test: invalid worker template")?;
    Ok(format!("{before}{uri}{middle}{path}{after}"))
}

fn relay(line: &str, json_reporter: bool) {
    let event = serde_json::from_str::<Value>(line)
        .unwrap_or_else(|_| json!({"type":"print", "message":line}));
    if json_reporter {
        println!("{event}");
        return;
    }
    match event["type"].as_str() {
        Some("testStart") => println!("RUN {}", event["name"].as_str().unwrap_or("")),
        Some("testDone") => println!(
            "{} {}",
            event["result"].as_str().unwrap_or(""),
            event["name"].as_str().unwrap_or("")
        ),
        Some("error" | "loadError") => {
            eprintln!("{}", event["error"].as_str().unwrap_or(line));
            if let Some(stack) = event["stackTrace"].as_str() {
                eprintln!("{stack}");
            }
        }
        Some("message" | "print") => println!("{}", event["message"].as_str().unwrap_or(line)),
        _ => {}
    }
}

struct Workspace {
    path: PathBuf,
    keep: bool,
}
impl Drop for Workspace {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

/// Execute suites in separate DartForge AOT programs, without an SDK test runner.
pub(crate) fn run(args: &[OsString]) -> Result<ExitCode, Box<dyn std::error::Error>> {
    if args.iter().any(|v| v == "--help" || v == "-h") {
        println!("{USAGE}");
        return Ok(ExitCode::SUCCESS);
    }
    let options = Options::parse(args)?;
    if !cfg!(feature = "native") {
        return Err("dartforge test requires a CLI built with --features native".into());
    }
    let mut files = BTreeSet::new();
    for path in &options.paths {
        discover(path, &mut files, true)?;
    }
    if files.is_empty() {
        eprintln!("No test files found.");
        return Ok(ExitCode::from(79));
    }
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let directory =
        std::env::temp_dir().join(format!("dartforge-test-{}-{unique}", std::process::id()));
    fs::create_dir(&directory)?;
    let workspace = Workspace {
        path: directory.canonicalize()?,
        keep: options.keep,
    };
    if options.keep {
        eprintln!("Test artifacts: {}", workspace.path.display());
    }
    let executable = std::env::current_exe()?;
    let mut failed = false;
    let mut total = 0u64;
    for (index, suite) in files.iter().enumerate() {
        let config = package_config(suite, options.packages.as_deref())?;
        check_suite(suite, &config)?;
        let output = workspace.path.join(index.to_string());
        fs::create_dir(&output)?;
        let worker = output.join("worker.dart");
        fs::write(&worker, worker_source(&output, suite)?)?;
        let binary = output.join(if cfg!(windows) {
            "worker.exe"
        } else {
            "worker"
        });
        let report = output.join("result.json");
        let token = format!("{unique}-{index}");
        let mut compile = Command::new(&executable);
        compile
            .arg("compile-native")
            .arg(&worker)
            .arg("-o")
            .arg(&binary)
            .arg("--packages")
            .arg(&config)
            .args([
                "--optimize",
                "--memory=tracing",
                "--exceptions=check",
                "--roots=shadow",
                "--stack-trace=symbolic",
            ]);
        if let Some(sdk) = &options.sdk {
            compile.arg("--sdk").arg(sdk);
        }
        let compilation = compile.output()?;
        fs::write(output.join("compile.stdout"), &compilation.stdout)?;
        fs::write(output.join("compile.stderr"), &compilation.stderr)?;
        if !compilation.status.success() {
            eprintln!(
                "Failed to compile {} with DartForge:\n{}",
                suite.display(),
                String::from_utf8_lossy(&compilation.stderr)
            );
            failed = true;
            continue;
        }
        let cwd = suite_root(suite)?;
        let mut child = Command::new(&binary)
            .current_dir(cwd)
            .arg(&report)
            .arg(&token)
            .arg(&options.filter)
            .arg(if options.plain { "plain" } else { "regex" })
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let stdout = child.stdout.take().ok_or("test: missing worker output")?;
        let json_reporter = options.json;
        let reader = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(line) => relay(&line, json_reporter),
                    Err(error) => {
                        eprintln!("test worker output: {error}");
                        break;
                    }
                }
            }
        });
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if started.elapsed() >= options.timeout {
                child.kill()?;
                let status = child.wait()?;
                eprintln!("Test process timed out: {}", suite.display());
                break status;
            }
            thread::sleep(Duration::from_millis(20));
        };
        reader.join().map_err(|_| "test: output reader failed")?;
        let summary = fs::read(&report)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
        let Some(summary) =
            summary.filter(|s| s["schemaVersion"] == 1 && s["token"].as_str() == Some(&token))
        else {
            eprintln!(
                "Test process produced no valid completion report: {} ({status})",
                suite.display()
            );
            failed = true;
            continue;
        };
        let selected = summary["total"]
            .as_u64()
            .ok_or("test: invalid test count")?;
        total += selected;
        let passed = summary["passed"]
            .as_u64()
            .ok_or("test: invalid passed count")?;
        let skipped = summary["skipped"]
            .as_u64()
            .ok_or("test: invalid skipped count")?;
        let errors = summary["failed"]
            .as_u64()
            .ok_or("test: invalid failure count")?;
        if selected > 0
            && (!status.success()
                || summary["success"] != true
                || errors != 0
                || passed.checked_add(skipped) != Some(selected))
        {
            failed = true;
        }
        if selected == 0 && status.code() != Some(79) {
            failed = true;
        }
    }
    let code = if failed {
        1
    } else if total == 0 {
        79
    } else {
        0
    };
    if options.json {
        println!(
            "{}",
            json!({"type":"done", "success":code == 0, "total":total})
        );
    } else {
        println!(
            "{} tests selected; {}",
            total,
            if code == 0 {
                "all passed or skipped"
            } else if code == 79 {
                "no tests matched"
            } else {
                "test run failed"
            }
        );
    }
    Ok(ExitCode::from(code))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn ordinary_test_filters_and_unsupported_flags_are_explicit() {
        let options = Options::parse(&args(&[
            "test/a_test.dart",
            "--plain-name",
            "group async",
            "--reporter",
            "json",
        ]))
        .unwrap();
        assert_eq!(options.paths, [PathBuf::from("test/a_test.dart")]);
        assert_eq!(options.filter, "group async");
        assert!(options.plain && options.json);
        for values in [
            vec!["--name"],
            vec!["--name", "a", "--plain-name", "b"],
            vec!["--platform", "chrome"],
            vec!["--reporter", "compact"],
            vec!["--process-timeout", "0"],
        ] {
            assert!(Options::parse(&args(&values)).is_err(), "{values:?}");
        }
    }

    #[test]
    fn relative_import_and_dart_escaping_preserve_literal_path() {
        let base = std::env::temp_dir().canonicalize().unwrap();
        let from = base.join("workers/0");
        let suite = base.join("project/test/quote'$__DARTFORGE_SUITE_PATH___test.dart");
        let relative = relative_import(&from, &suite).unwrap();
        assert_eq!(
            relative,
            "../../project/test/quote'$__DARTFORGE_SUITE_PATH___test.dart"
        );
        let source = worker_source(&from, &suite).unwrap();
        assert!(source.contains("quote\\'\\$__DARTFORGE_SUITE_PATH___test.dart"));
        assert!(source.contains("Invoker.guard<Future<bool?>>"));
        assert!(source.contains("await declarer.declare<FutureOr<void>>(suite_source.main)"));
    }

    #[test]
    fn discovery_keeps_sorted_unique_suites_and_skips_helper_files() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dartforge-test-discovery-{unique}"));
        fs::create_dir_all(path.join("nested")).unwrap();
        let workspace = Workspace {
            path: path.clone(),
            keep: false,
        };
        for name in ["b_test.dart", "nested/a_test.dart", "helper.dart"] {
            fs::write(path.join(name), "void main() {}").unwrap();
        }
        let mut files = BTreeSet::new();
        discover(&path, &mut files, true).unwrap();
        discover(&path.join("b_test.dart"), &mut files, true).unwrap();
        assert_eq!(files.len(), 2);
        assert!(!files.iter().any(|p| p.ends_with("helper.dart")));
        drop(workspace);
    }
}

#[cfg(test)]
mod metadata_tests {
    use super::*;

    #[test]
    fn library_annotations_and_project_configuration_are_refused_without_text_guessing() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dartforge-test-metadata-{unique}"));
        fs::create_dir_all(path.join("test")).unwrap();
        let workspace = Workspace {
            path: path.clone(),
            keep: false,
        };
        fs::write(path.join("pubspec.yaml"), "name: fixture").unwrap();
        let config = path.join("custom-packages.json");
        fs::write(
            &config,
            r#"{"packages":[{"name":"test"},{"name":"test_api"},{"name":"test_core"}]}"#,
        )
        .unwrap();
        let suite = path.join("test/basic_test.dart");
        fs::write(
            &suite,
            "// @TestOn('browser') is just a comment.\nvoid main() { print('@Skip'); }",
        )
        .unwrap();
        check_suite(&suite, &config).unwrap();
        fs::write(&suite, "@TestOn('browser')\nlibrary;\nvoid main() {}").unwrap();
        assert!(
            check_suite(&suite, &config)
                .unwrap_err()
                .to_string()
                .contains("library annotations")
        );
        fs::write(&suite, "void main() {}").unwrap();
        fs::write(path.join("dart_test.yaml"), "platforms: [chrome]\n").unwrap();
        assert!(
            check_suite(&suite, &config)
                .unwrap_err()
                .to_string()
                .contains("dart_test.yaml")
        );
        drop(workspace);
    }
}
