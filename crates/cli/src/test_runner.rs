//! Native execution of ordinary package:test suites through package:test's
//! own runner.
//!
//! `dartforge test` keeps package:test's semantics by running package:test's
//! own code natively. A host program (`test_runner/host.dart`), compiled once
//! per package configuration and cached under `.dart_tool/dartforge/`, is
//! `dart test` with one platform replaced: it parses the command line and the
//! `dart_test.yaml` files with `test_core`'s `Configuration`, loads the suites
//! like its `Runner`/`Loader`, and runs them through its `Engine` and
//! reporters (`expanded`, `compact`, `json`, `github`, `failures-only`,
//! `silent`, file reporters, `--fail-fast`). Each suite is compiled into a
//! native executable around test_core's own native bootstrap and talks to the
//! host over test_core's socket protocol, as `dart test -c exe` does. The
//! library annotations that test_core reads with the analyzer come from
//! [`metadata::library_metadata`], which the host requests through
//! `dartforge test --dartforge-library-metadata`.
mod metadata;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    time::{SystemTime, UNIX_EPOCH},
};

const USAGE: &str = "DartForge options of dartforge test:
    --sdk <lib>                  The Dart SDK library directory used to compile the suites.
    --packages <config>          The package_config.json (default: the one of the working directory).
    --process-timeout <seconds>  Kills a suite's native executable after this many seconds.
    --keep-artifacts             Keeps the generated bootstraps and executables.
Suites run on the vm platform as native executables (the exe compiler). Browser and
node platforms, coverage, debugging and line/column selection are refused.";

/// The internal option the host uses to read library annotations.
const METADATA_OPTION: &str = "--dartforge-library-metadata";
/// The internal option the host uses to compile a suite through the cache.
const COMPILE_OPTION: &str = "--dartforge-compile-suite";

#[derive(Debug)]
struct Options {
    /// Arguments for package:test's own command-line parser.
    forwarded: Vec<String>,
    sdk: Option<PathBuf>,
    packages: Option<PathBuf>,
    timeout: Option<u64>,
    keep: bool,
    help: bool,
}

impl Options {
    fn parse(args: &[OsString]) -> Result<Self, String> {
        let mut options = Self {
            forwarded: Vec::new(),
            sdk: None,
            packages: None,
            timeout: None,
            keep: false,
            help: false,
        };
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            let text = arg.to_str().ok_or("test: options must be valid UTF-8")?;
            let mut value = |name: &str| -> Result<String, String> {
                it.next()
                    .and_then(|v| v.to_str())
                    .map(str::to_owned)
                    .ok_or_else(|| format!("test: {name} requires a value"))
            };
            match text {
                "--sdk" => options.sdk = Some(PathBuf::from(value("--sdk")?)),
                "--packages" => options.packages = Some(PathBuf::from(value("--packages")?)),
                "--process-timeout" => {
                    let seconds = value("--process-timeout")?
                        .parse::<u64>()
                        .ok()
                        .filter(|v| *v > 0)
                        .ok_or("test: --process-timeout requires positive seconds")?;
                    options.timeout = Some(seconds);
                }
                "--keep-artifacts" => options.keep = true,
                "--" => {
                    options.forwarded.push(text.to_owned());
                    for rest in it.by_ref() {
                        options.forwarded.push(
                            rest.to_str()
                                .ok_or("test: paths must be valid UTF-8")?
                                .to_owned(),
                        );
                    }
                }
                _ => {
                    if text == "--help" || text == "-h" {
                        options.help = true;
                    }
                    options.forwarded.push(text.to_owned());
                }
            }
        }
        Ok(options)
    }
}

/// `dart test` uses the package configuration of the working directory.
fn package_config(cwd: &Path, explicit: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return dartforge_elements::config::canonicalize(path)
            .map_err(|e| format!("test: {}: {e}", path.display()));
    }
    for ancestor in cwd.ancestors() {
        let path = ancestor.join(".dart_tool/package_config.json");
        if path.is_file() {
            return dartforge_elements::config::canonicalize(&path)
                .map_err(|e| format!("test: {}: {e}", path.display()));
        }
    }
    Err(format!(
        "test: no package_config.json for {}; resolve package:test dependencies first",
        cwd.display()
    ))
}

fn check_packages(config: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let configuration: Value = serde_json::from_slice(&fs::read(config)?)?;
    let packages = configuration["packages"]
        .as_array()
        .ok_or("test: invalid package configuration")?;
    for name in ["test", "test_api", "test_core"] {
        if !packages.iter().any(|p| p["name"].as_str() == Some(name)) {
            return Err(format!("test: package configuration must resolve package:{name}").into());
        }
    }
    Ok(())
}

const HOST_SOURCE: &str = include_str!("test_runner/host.dart");

const COMPILE_FLAGS: [&str; 5] = [
    "--optimize",
    "--memory=tracing",
    "--exceptions=check",
    "--roots=shadow",
    "--stack-trace=symbolic",
];

/// The `compile-native` arguments shared by the host and the suites.
fn compile_arguments(config: &Path, sdk: Option<&Path>) -> Vec<String> {
    let mut arguments = vec![
        "--packages".to_owned(),
        config.to_string_lossy().into_owned(),
    ];
    arguments.extend(COMPILE_FLAGS.iter().map(|f| (*f).to_owned()));
    if let Some(sdk) = sdk {
        arguments.push("--sdk".to_owned());
        arguments.push(sdk.to_string_lossy().into_owned());
    }
    arguments
}

fn executable_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_owned()
    }
}

/// The host only depends on its source, the package configuration, the
/// compiler and the compile options, so its binary is reused across runs.
fn host_binary(config: &Path, sdk: Option<&Path>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let compiler = std::env::current_exe()?;
    let compiler_metadata = fs::metadata(&compiler)?;
    let arguments = compile_arguments(config, sdk);
    let mut hash = Sha256::new();
    for part in [
        HOST_SOURCE.as_bytes(),
        &fs::read(config)?,
        compiler.to_string_lossy().as_bytes(),
        &compiler_metadata.len().to_le_bytes(),
        &compiler_metadata
            .modified()?
            .duration_since(UNIX_EPOCH)?
            .as_nanos()
            .to_le_bytes(),
        arguments.join(" ").as_bytes(),
    ] {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    let key: String = hash
        .finalize()
        .iter()
        .take(12)
        .map(|b| format!("{b:02x}"))
        .collect();
    let tool = config
        .parent()
        .ok_or("test: invalid package configuration path")?;
    let directory = tool.join("dartforge/test-host").join(&key);
    let binary = directory.join(executable_name("host"));
    if binary.is_file() {
        return Ok(binary);
    }
    fs::create_dir_all(&directory)?;
    let source = directory.join("host.dart");
    fs::write(&source, HOST_SOURCE)?;
    // Another run may compile the same key: publish the binary atomically.
    let partial = directory.join(format!(
        "{}.{}",
        executable_name("host"),
        std::process::id()
    ));
    let output = Command::new(&compiler)
        .arg("compile-native")
        .arg(&source)
        .arg("-o")
        .arg(&partial)
        .args(&arguments)
        .output()?;
    fs::write(directory.join("compile.stdout"), &output.stdout)?;
    fs::write(directory.join("compile.stderr"), &output.stderr)?;
    if !output.status.success() {
        let _ = fs::remove_file(&partial);
        return Err(format!(
            "test: failed to compile the test host with DartForge:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    fs::rename(&partial, &binary)?;
    Ok(binary)
}

/// How many suites DartForge compiles at the same time. Native compilation
/// is far heavier than the VM's kernel compilation, so it does not follow
/// `--concurrency`; `DARTFORGE_TEST_COMPILE_JOBS` overrides the default.
fn compile_jobs() -> usize {
    if let Some(jobs) = std::env::var("DARTFORGE_TEST_COMPILE_JOBS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
    {
        return jobs;
    }
    let cpus = std::thread::available_parallelism().map_or(1, |n| n.get());
    (cpus / 4).clamp(1, 4)
}

/// `--dartforge-library-metadata <path>...`: the annotations of each suite.
fn print_library_metadata(paths: &[OsString]) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let mut libraries = serde_json::Map::new();
    for path in paths {
        let path = path.to_str().ok_or("test: paths must be valid UTF-8")?;
        let source = fs::read_to_string(path)?;
        libraries.insert(path.to_owned(), metadata::library_metadata(&source));
    }
    println!("{}", Value::Object(libraries));
    Ok(ExitCode::SUCCESS)
}

/// `--dartforge-compile-suite <bootstrap> -o <output> <compile arguments>`:
/// `compile-native` through a cache of suite executables under
/// `.dart_tool/dartforge/test-suites/`.
///
/// The key covers everything the compilation reads: the bootstrap with the
/// suite's absolute path, this compiler (path, size, modification time), the
/// compile arguments, the package configuration, the selected SDK, the
/// `DARTFORGE_*` environment and the path, size and modification time of
/// every file under every package root of the configuration. A hit copies
/// the executable the same compilation produced before; anything the key
/// cannot read compiles without the cache.
fn compile_suite(args: &[OsString]) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let [bootstrap, flag, output, rest @ ..] = args else {
        return Err("test: invalid suite compilation".into());
    };
    if flag != "-o" {
        return Err("test: invalid suite compilation".into());
    }
    let (bootstrap, output) = (PathBuf::from(bootstrap), PathBuf::from(output));
    let compile = || -> std::io::Result<std::process::ExitStatus> {
        Command::new(std::env::current_exe()?)
            .arg("compile-native")
            .arg(&bootstrap)
            .arg("-o")
            .arg(&output)
            .args(rest)
            .status()
    };
    let key = suite_key(&bootstrap, rest);
    let Some((key, directory)) = key else {
        let status = compile()?;
        return Ok(ExitCode::from(
            status
                .code()
                .and_then(|c| u8::try_from(c).ok())
                .unwrap_or(1),
        ));
    };
    let cached = directory.join(&key).join(executable_name("suite"));
    if cached.is_file() && fs::copy(&cached, &output).is_ok() {
        return Ok(ExitCode::SUCCESS);
    }
    let status = compile()?;
    if status.success() && output.is_file() {
        let target = directory.join(&key);
        if fs::create_dir_all(&target).is_ok() {
            let partial = target.join(format!("suite.{}", std::process::id()));
            if fs::copy(&output, &partial).is_ok() && fs::rename(&partial, &cached).is_err() {
                let _ = fs::remove_file(&partial);
            }
        }
    }
    Ok(ExitCode::from(
        status
            .code()
            .and_then(|c| u8::try_from(c).ok())
            .unwrap_or(1),
    ))
}

/// The cache key of a suite compilation and the cache directory, or `None`
/// when some input cannot be read.
fn suite_key(bootstrap: &Path, arguments: &[OsString]) -> Option<(String, PathBuf)> {
    let text = fs::read_to_string(bootstrap).ok()?;
    // The bootstrap imports the suite relative to a per-run directory.
    let (before, rest) = text.split_once("import \"")?;
    let (relative, after) = rest.split_once('"')?;
    let relative: String = serde_json::from_str(&format!("\"{relative}\"")).ok()?;
    let suite =
        dartforge_elements::config::canonicalize(&bootstrap.parent()?.join(relative)).ok()?;
    let arguments: Vec<String> = arguments
        .iter()
        .map(|a| a.to_str().map(str::to_owned))
        .collect::<Option<_>>()?;
    let config = PathBuf::from(&arguments[arguments.iter().position(|a| a == "--packages")? + 1]);
    let mut hash = Sha256::new();
    let mut part = |bytes: &[u8]| {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    };
    part(before.as_bytes());
    part(suite.to_string_lossy().as_bytes());
    part(after.as_bytes());
    let compiler = std::env::current_exe().ok()?;
    let metadata = fs::metadata(&compiler).ok()?;
    part(compiler.to_string_lossy().as_bytes());
    part(&metadata.len().to_le_bytes());
    part(
        &metadata
            .modified()
            .ok()?
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_nanos()
            .to_le_bytes(),
    );
    part(arguments.join("\0").as_bytes());
    let configuration = fs::read(&config).ok()?;
    part(&configuration);
    let mut environment: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| k.starts_with("DARTFORGE_") && k != "DARTFORGE_TEST_COMPILE_JOBS")
        .collect();
    environment.sort();
    for (name, value) in environment {
        part(name.as_bytes());
        part(value.as_bytes());
    }
    let sdk = arguments
        .iter()
        .position(|a| a == "--sdk")
        .map(|i| PathBuf::from(&arguments[i + 1]))
        .or_else(|| std::env::var_os("DARTFORGE_SDK_LIB").map(PathBuf::from));
    if let Some(sdk) = sdk {
        part(sdk.to_string_lossy().as_bytes());
        stamp_tree(&sdk, &mut part);
    }
    let packages: Value = serde_json::from_slice(&configuration).ok()?;
    let base = url::Url::from_file_path(config.parent()?).ok()?;
    let base = url::Url::parse(&format!("{}/", base.as_str().trim_end_matches('/'))).ok()?;
    for package in packages["packages"].as_array()? {
        let root = base
            .join(package["rootUri"].as_str()?)
            .ok()?
            .to_file_path()
            .ok()?;
        part(root.to_string_lossy().as_bytes());
        stamp_tree(&root, &mut part);
    }
    let key: String = hash
        .finalize()
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect();
    Some((key, config.parent()?.join("dartforge/test-suites")))
}

/// Feeds the path, size and modification time of every file below `root`
/// (sorted), skipping `.dart_tool`, `.git` and `build` directories.
fn stamp_tree(root: &Path, part: &mut impl FnMut(&[u8])) {
    let mut stack = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = stack.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if name != ".dart_tool" && name != ".git" && name != "build" {
                    stack.push(entry.path());
                }
            } else if let Ok(metadata) = entry.metadata() {
                let modified = metadata
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_nanos());
                files.push((entry.path(), metadata.len(), modified));
            }
        }
    }
    files.sort();
    for (path, length, modified) in files {
        part(path.to_string_lossy().as_bytes());
        part(&length.to_le_bytes());
        part(&modified.to_le_bytes());
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

/// Runs package:test suites through the natively compiled test host.
pub(crate) fn run(args: &[OsString]) -> Result<ExitCode, Box<dyn std::error::Error>> {
    if args.first().is_some_and(|a| a == METADATA_OPTION) {
        return print_library_metadata(&args[1..]);
    }
    if args.first().is_some_and(|a| a == COMPILE_OPTION) {
        return compile_suite(&args[1..]);
    }
    let options = match Options::parse(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("error: {message}\n{USAGE}");
            return Ok(ExitCode::from(64));
        }
    };
    if !cfg!(feature = "native") {
        return Err("dartforge test requires a CLI built with --features native".into());
    }
    let cwd = dartforge_elements::config::canonicalize(&std::env::current_dir()?)?;
    let config = package_config(&cwd, options.packages.as_deref())?;
    check_packages(&config)?;
    let host = host_binary(&config, options.sdk.as_deref())?;

    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let directory =
        std::env::temp_dir().join(format!("dartforge-test-{}-{unique}", std::process::id()));
    fs::create_dir(&directory)?;
    let workspace = Workspace {
        path: dartforge_elements::config::canonicalize(&directory)?,
        keep: options.keep,
    };
    if options.keep {
        eprintln!("Test artifacts: {}", workspace.path.display());
    }
    let compiler = std::env::current_exe()?.to_string_lossy().into_owned();
    let settings = json!({
        "compiler": compiler,
        "compileArguments": compile_arguments(&config, options.sdk.as_deref()),
        "compileCommand": ["test", COMPILE_OPTION],
        "compileJobs": compile_jobs(),
        "metadataCommand": [compiler, "test", METADATA_OPTION],
        "packageConfig": config,
        "processTimeout": options.timeout,
        "workDirectory": workspace.path.join("suites"),
    });
    let settings_path = workspace.path.join("settings.json");
    fs::write(&settings_path, serde_json::to_vec(&settings)?)?;
    let mut command = Command::new(&host);
    command
        .current_dir(&cwd)
        .arg(&settings_path)
        .args(&options.forwarded);
    // package:test prints `Platform.executable` in the command that reruns a
    // failed test (`<dart> test <path> -p vm --plain-name ...`); here that
    // command is this executable.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::arg0(&mut command, &compiler);
    let status = command.status()?;
    if options.help {
        println!("{USAGE}");
    }
    Ok(match status.code() {
        Some(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        None => ExitCode::FAILURE,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn dartforge_options_are_kept_and_package_test_options_are_forwarded() {
        let options = Options::parse(&args(&[
            "test/a_test.dart",
            "--plain-name",
            "group async",
            "--reporter",
            "json",
            "--file-reporter",
            "json:out.json",
            "--fail-fast",
            "-P",
            "ci",
            "--sdk",
            "/sdk/lib",
            "--keep-artifacts",
            "--process-timeout",
            "30",
            "-p",
            "vm",
            "--",
            "--sdk",
            "--help",
        ]))
        .unwrap();
        assert_eq!(
            options.forwarded,
            [
                "test/a_test.dart",
                "--plain-name",
                "group async",
                "--reporter",
                "json",
                "--file-reporter",
                "json:out.json",
                "--fail-fast",
                "-P",
                "ci",
                "-p",
                "vm",
                "--",
                "--sdk",
                "--help",
            ]
        );
        assert_eq!(options.sdk, Some(PathBuf::from("/sdk/lib")));
        assert_eq!(options.timeout, Some(30));
        assert!(options.keep);
        assert!(!options.help, "paths after -- are not options");
    }

    #[test]
    fn dartforge_option_values_are_checked() {
        for values in [
            vec!["--process-timeout", "0"],
            vec!["--process-timeout", "x"],
            vec!["--sdk"],
            vec!["--packages"],
        ] {
            assert!(Options::parse(&args(&values)).is_err(), "{values:?}");
        }
        assert!(Options::parse(&args(&["-h"])).unwrap().help);
    }

    #[test]
    fn host_template_runs_package_test_engine_and_reporters() {
        assert!(
            HOST_SOURCE.contains("allReporters[config.reporter]!.factory(config, engine, stdout)")
        );
        assert!(HOST_SOURCE.contains("stopOnFirstFailure: config.stopOnFirstFailure"));
        assert!(HOST_SOURCE.contains("internalBootstrapNativeTest(() => test.main, args)"));
        assert!(HOST_SOURCE.contains("deserializeSuite(path, platform, suiteConfig"));
    }

    #[test]
    fn compile_arguments_carry_the_package_configuration_and_sdk() {
        let arguments =
            compile_arguments(Path::new("/p/.dart_tool/pc.json"), Some(Path::new("/sdk")));
        assert_eq!(arguments[..2], ["--packages", "/p/.dart_tool/pc.json"]);
        assert_eq!(arguments[arguments.len() - 2..], ["--sdk", "/sdk"]);
        assert!(
            compile_arguments(Path::new("/c"), None)
                .iter()
                .all(|a| a != "--sdk")
        );
    }

    #[test]
    fn package_configuration_comes_from_the_working_directory() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dartforge-test-config-{unique}"));
        fs::create_dir_all(path.join(".dart_tool")).unwrap();
        fs::create_dir_all(path.join("test/nested")).unwrap();
        let workspace = Workspace {
            path: path.clone(),
            keep: false,
        };
        let config = path.join(".dart_tool/package_config.json");
        fs::write(
            &config,
            r#"{"packages":[{"name":"test"},{"name":"test_api"}]}"#,
        )
        .unwrap();
        let found = package_config(&path.join("test/nested"), None).unwrap();
        assert_eq!(
            found,
            dartforge_elements::config::canonicalize(&config).unwrap()
        );
        assert!(
            check_packages(&found)
                .unwrap_err()
                .to_string()
                .contains("package:test_core")
        );
        drop(workspace);
    }
}
