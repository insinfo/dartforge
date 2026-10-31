//! Native execution of ordinary package:test suites through its own Engine.
//!
//! `dartforge test` keeps package:test's semantics by running package:test's
//! own code natively. A planner program, compiled once per package
//! configuration and cached under `.dart_tool/dartforge/`, parses the command
//! line and the `dart_test.yaml` files with `test_core`'s `Configuration`,
//! finds the suites like its `Runner`/`Loader` and resolves each suite's
//! configuration, merged with the library annotations that
//! [`metadata::library_metadata`] reads. Each suite then runs in its own
//! native worker built around package:test's `Declarer` and `Engine`.
mod metadata;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::{BufRead, BufReader, Write},
    path::{Component, Path, PathBuf},
    process::{Command, ExitCode, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const USAGE: &str = "usage: dartforge test [<file|directory> ...] [dart test options] [--reporter expanded|json] [--sdk <lib>] [--packages <package_config.json>] [--process-timeout <seconds>] [--keep-artifacts]
Runs resolved package:test suites in native DartForge executables on the vm platform.
Command-line options, dart_test.yaml (including presets, tags, on_os, on_platform,
paths, filename, timeout, retry and concurrency) and the library annotations
@TestOn, @Timeout, @Skip, @OnPlatform, @Tags and @Retry follow package:test.
Suites report the vm platform with the exe compiler. Requires resolved test,
test_api and test_core dependencies. Browser and node platforms, coverage,
debugging, --fail-fast, file reporters, line/column selection and watch mode
are not supported and are refused.";

/// Options of `dart test` that the native runner cannot honor; they are
/// refused before anything is compiled. The planner refuses the same
/// settings when they come from `dart_test.yaml`.
const REFUSED_FLAGS: [(&str, &str); 5] = [
    ("--coverage", "coverage collection"),
    ("--pause-after-load", "debugging"),
    ("--debug", "debugging"),
    ("--fail-fast", "--fail-fast"),
    ("--file-reporter", "file reporters"),
];

#[derive(Debug)]
struct Options {
    /// Arguments for package:test's own command-line parser.
    forwarded: Vec<String>,
    sdk: Option<PathBuf>,
    packages: Option<PathBuf>,
    /// `Some(true)` for the JSON reporter, `None` when not given.
    json: Option<bool>,
    timeout: Duration,
    keep: bool,
}

/// Values of `-p`/`--platform` in every spelling package:args accepts.
fn platform_values(arguments: &[String]) -> Vec<String> {
    let mut values = Vec::new();
    let mut iter = arguments.iter();
    while let Some(arg) = iter.next() {
        if arg == "--" {
            break;
        }
        let value = if arg == "-p" || arg == "--platform" {
            iter.next().cloned()
        } else if let Some(value) = arg.strip_prefix("--platform=") {
            Some(value.to_owned())
        } else if let Some(value) = arg.strip_prefix("-p") {
            (!arg.starts_with("--")).then(|| value.to_owned())
        } else {
            None
        };
        values.extend(
            value
                .into_iter()
                .flat_map(|v| v.split(',').map(str::to_owned).collect::<Vec<_>>()),
        );
    }
    values
}

impl Options {
    fn parse(args: &[OsString]) -> Result<Self, String> {
        let mut options = Self {
            forwarded: Vec::new(),
            sdk: None,
            packages: None,
            json: None,
            timeout: Duration::from_secs(120),
            keep: false,
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
                "--reporter" | "-r" => options.json = Some(reporter(&value("--reporter")?)?),
                _ if text.starts_with("--reporter=") => {
                    options.json = Some(reporter(&text["--reporter=".len()..])?)
                }
                "--process-timeout" => {
                    let seconds = value("--process-timeout")?
                        .parse::<u64>()
                        .ok()
                        .filter(|v| *v > 0)
                        .ok_or("test: --process-timeout requires positive seconds")?;
                    options.timeout = Duration::from_secs(seconds);
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
                    let name = text.split('=').next().unwrap_or(text);
                    if let Some((_, what)) = REFUSED_FLAGS.iter().find(|(flag, _)| *flag == name) {
                        return Err(format!(
                            "test: {what} ({name}) is not supported by the native runner"
                        ));
                    }
                    if matches!(text, "--watch" | "-w") {
                        return Err("test: watch mode is not supported by the native runner".into());
                    }
                    options.forwarded.push(text.to_owned());
                }
            }
        }
        let others: Vec<String> = platform_values(&options.forwarded)
            .into_iter()
            .filter(|p| p != "vm")
            .collect();
        if !others.is_empty() {
            return Err(format!(
                "test: the native runner only runs the vm platform; {} is not supported",
                others.join(", ")
            ));
        }
        Ok(options)
    }
}

fn reporter(value: &str) -> Result<bool, String> {
    match value {
        "expanded" => Ok(false),
        "json" => Ok(true),
        _ => Err("test: --reporter must be expanded or json".into()),
    }
}

/// `dart test` uses the package configuration of the working directory.
fn package_config(cwd: &Path, explicit: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return path
            .canonicalize()
            .map_err(|e| format!("test: {}: {e}", path.display()));
    }
    for ancestor in cwd.ancestors() {
        let path = ancestor.join(".dart_tool/package_config.json");
        if path.is_file() {
            return path
                .canonicalize()
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

const SELECTION_SOURCE: &str = include_str!("test_runner/selection.dart");
const PLANNER_SOURCE: &str = include_str!("test_runner/planner.dart");
const WORKER_SOURCE: &str = include_str!("test_runner/worker.dart");
/// Prefix of the planner's protocol lines; ordinary output never starts so.
const PROTOCOL: &str = "\u{0}dartforge-test-plan:";

fn worker_source(directory: &Path, suite: &Path) -> Result<String, String> {
    let uri = dart_string_contents(&relative_import(directory, suite)?);
    let (before, after) = WORKER_SOURCE
        .split_once("__DARTFORGE_SUITE_URI__")
        .ok_or("test: invalid worker template")?;
    Ok(format!("{before}{uri}{after}"))
}

fn planner_source() -> String {
    PLANNER_SOURCE.replace("__DARTFORGE_PROTOCOL__", &dart_string_contents(PROTOCOL))
}

const COMPILE_FLAGS: [&str; 5] = [
    "--optimize",
    "--memory=tracing",
    "--exceptions=check",
    "--roots=shadow",
    "--stack-trace=symbolic",
];

fn compile(
    source: &Path,
    binary: &Path,
    config: &Path,
    sdk: Option<&Path>,
) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("compile-native")
        .arg(source)
        .arg("-o")
        .arg(binary)
        .arg("--packages")
        .arg(config)
        .args(COMPILE_FLAGS);
    if let Some(sdk) = sdk {
        command.arg("--sdk").arg(sdk);
    }
    let output = command.output()?;
    let directory = binary.parent().unwrap_or(Path::new("."));
    fs::write(directory.join("compile.stdout"), &output.stdout)?;
    fs::write(directory.join("compile.stderr"), &output.stderr)?;
    Ok(output)
}

fn executable_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_owned()
    }
}

/// The planner only depends on its sources, the package configuration, the
/// compiler and the compile options, so its binary is reused across runs.
fn planner_binary(
    config: &Path,
    sdk: Option<&Path>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let source = planner_source();
    let compiler = std::env::current_exe()?;
    let compiler_metadata = fs::metadata(&compiler)?;
    let mut hash = Sha256::new();
    for part in [
        source.as_bytes(),
        SELECTION_SOURCE.as_bytes(),
        &fs::read(config)?,
        compiler.to_string_lossy().as_bytes(),
        &compiler_metadata.len().to_le_bytes(),
        &compiler_metadata
            .modified()?
            .duration_since(UNIX_EPOCH)?
            .as_nanos()
            .to_le_bytes(),
        sdk.map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
            .as_bytes(),
        COMPILE_FLAGS.join(" ").as_bytes(),
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
    let directory = tool.join("dartforge/test-planner").join(&key);
    let binary = directory.join(executable_name("planner"));
    if binary.is_file() {
        return Ok(binary);
    }
    fs::create_dir_all(&directory)?;
    fs::write(directory.join("planner.dart"), &source)?;
    fs::write(directory.join("selection.dart"), SELECTION_SOURCE)?;
    // Another run may compile the same key: publish the binary atomically.
    let partial = directory.join(format!(
        "{}.{}",
        executable_name("planner"),
        std::process::id()
    ));
    let output = compile(&directory.join("planner.dart"), &partial, config, sdk)?;
    if !output.status.success() {
        let _ = fs::remove_file(&partial);
        return Err(format!(
            "test: failed to compile the test planner with DartForge:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    fs::rename(&partial, &binary)?;
    Ok(binary)
}

enum Planned {
    /// The planner already reported the problem and exited with this code.
    Exit(u8),
    Refused(String),
    Plan {
        plan: Value,
        suites: Vec<Value>,
    },
}

/// Runs the planner, answering its request for library annotations.
fn plan(
    binary: &Path,
    cwd: &Path,
    options: &Options,
) -> Result<Planned, Box<dyn std::error::Error>> {
    let mut child = Command::new(binary)
        .current_dir(cwd)
        .args(&options.forwarded)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let mut stdin = child.stdin.take().ok_or("test: missing planner input")?;
    let stdout = child.stdout.take().ok_or("test: missing planner output")?;
    let mut plan = None;
    let mut suites = None;
    let mut refusal = None;
    for line in BufReader::new(stdout).lines() {
        let line = line?;
        let Some(message) = line.strip_prefix(PROTOCOL) else {
            println!("{line}");
            continue;
        };
        let message: Value = serde_json::from_str(message)?;
        match message["type"].as_str() {
            Some("refusal") => refusal = message["message"].as_str().map(str::to_owned),
            Some("plan") => {
                let mut libraries = serde_json::Map::new();
                for load in message["loads"].as_array().ok_or("test: invalid plan")? {
                    if load["missing"] == true {
                        continue;
                    }
                    let path = load["path"].as_str().ok_or("test: invalid plan")?;
                    if !libraries.contains_key(path) {
                        let source = fs::read_to_string(cwd.join(path))?;
                        libraries.insert(path.to_owned(), metadata::library_metadata(&source));
                    }
                }
                writeln!(stdin, "{}", json!({ "libraries": libraries }))?;
                stdin.flush()?;
                plan = Some(message);
            }
            Some("suites") => suites = Some(message["suites"].clone()),
            _ => return Err("test: unknown planner message".into()),
        }
    }
    drop(stdin);
    let status = child.wait()?;
    if let Some(message) = refusal {
        return Ok(Planned::Refused(message));
    }
    match (plan, suites) {
        (Some(plan), Some(Value::Array(suites))) if status.success() => {
            Ok(Planned::Plan { plan, suites })
        }
        _ => Ok(Planned::Exit(
            status
                .code()
                .and_then(|c| u8::try_from(c).ok())
                .filter(|c| *c != 0)
                .unwrap_or(1),
        )),
    }
}

/// Prints worker events, prefixing test names with the suite when several run.
struct Relay {
    json: bool,
    suite: String,
    prefix: bool,
    /// Text output of a suite running in parallel is kept together.
    buffer: Option<Vec<(bool, String)>>,
}

impl Relay {
    fn out(&mut self, error: bool, text: String) {
        match &mut self.buffer {
            Some(lines) => lines.push((error, text)),
            None if error => eprintln!("{text}"),
            None => println!("{text}"),
        }
    }

    fn name(&self, event: &Value) -> String {
        let name = event["name"].as_str().unwrap_or("");
        if self.prefix {
            format!("{}: {name}", self.suite)
        } else {
            name.to_owned()
        }
    }

    fn event(&mut self, mut event: Value) {
        if self.json {
            event["suite"] = json!(self.suite);
            println!("{event}");
            return;
        }
        match event["type"].as_str() {
            Some("testStart") => {
                let text = format!("RUN {}", self.name(&event));
                self.out(false, text);
            }
            Some("testDone") => {
                let text = format!(
                    "{} {}",
                    event["result"].as_str().unwrap_or(""),
                    self.name(&event)
                );
                self.out(false, text);
            }
            Some("error" | "loadError") => {
                let mut text = event["error"].as_str().unwrap_or("").to_owned();
                if let Some(stack) = event["stackTrace"].as_str() {
                    text = format!("{text}\n{stack}");
                }
                self.out(true, text);
            }
            Some("message" | "print") => {
                let text = event["message"].as_str().unwrap_or("").to_owned();
                self.out(false, text);
            }
            _ => {}
        }
    }

    fn line(&mut self, line: &str) {
        let event = serde_json::from_str::<Value>(line)
            .unwrap_or_else(|_| json!({"type":"print", "message":line}));
        self.event(event);
    }

    fn flush(&mut self) {
        for (error, text) in self.buffer.take().unwrap_or_default() {
            if error {
                eprintln!("{text}");
            } else {
                println!("{text}");
            }
        }
    }
}

#[derive(Default)]
struct Tally {
    passed: u64,
    failed: u64,
    skipped: u64,
    /// A suite failed to compile, load or report.
    broken: bool,
}

struct SuiteRun {
    binary: PathBuf,
    spec: PathBuf,
    report: PathBuf,
    token: String,
    relay: Relay,
}

/// Runs one compiled worker under the process timeout.
fn run_worker(job: SuiteRun, cwd: &Path, timeout: Duration) -> std::io::Result<Tally> {
    let SuiteRun {
        binary,
        spec,
        report,
        token,
        relay,
    } = job;
    let suite = relay.suite.clone();
    let relay = Arc::new(Mutex::new(relay));
    let mut child = Command::new(&binary)
        .current_dir(cwd)
        .arg(&report)
        .arg(&token)
        .arg(&spec)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let stdout = child.stdout.take().expect("piped worker output");
    let reader_relay = Arc::clone(&relay);
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(line) => reader_relay.lock().expect("relay").line(&line),
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
        if started.elapsed() >= timeout {
            child.kill()?;
            let status = child.wait()?;
            eprintln!("Test process timed out: {suite}");
            break status;
        }
        thread::sleep(Duration::from_millis(20));
    };
    let _ = reader.join();
    relay.lock().expect("relay").flush();
    let summary = fs::read(&report)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    let Some(summary) =
        summary.filter(|s| s["schemaVersion"] == 2 && s["token"].as_str() == Some(&token))
    else {
        eprintln!("Test process produced no valid completion report: {suite} ({status})");
        return Ok(Tally {
            broken: true,
            ..Tally::default()
        });
    };
    let count = |key: &str| summary[key].as_u64().unwrap_or(u64::MAX);
    let (total, passed, failed, skipped) = (
        count("total"),
        count("passed"),
        count("failed"),
        count("skipped"),
    );
    let consistent = [total, passed, failed, skipped]
        .iter()
        .all(|v| *v != u64::MAX);
    let broken = !consistent
        || if total == 0 {
            status.code() != Some(79)
        } else {
            let succeeded = status.success() && summary["success"] == true && failed == 0;
            // A suite that claims success must account for every selected test.
            (succeeded && passed.checked_add(skipped) != Some(total)) || (!succeeded && failed == 0)
        };
    Ok(Tally {
        passed,
        failed,
        skipped,
        broken,
    })
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

/// The reporter: an explicit `--reporter` wins; otherwise dart_test.yaml may
/// select one, compared with package:test's own default.
fn select_reporter(options: &Options, plan: &Value) -> Result<bool, String> {
    if let Some(json) = options.json {
        return Ok(json);
    }
    let chosen = plan["reporter"].as_str().unwrap_or("");
    if chosen == plan["defaultReporter"].as_str().unwrap_or("") {
        return Ok(false);
    }
    match chosen {
        "json" => Ok(true),
        "expanded" | "compact" => Ok(false),
        other => Err(format!(
            "test: the {other} reporter is not supported; use expanded or json"
        )),
    }
}

/// Execute suites in separate DartForge AOT programs, without an SDK test runner.
pub(crate) fn run(args: &[OsString]) -> Result<ExitCode, Box<dyn std::error::Error>> {
    if args.iter().any(|v| v == "--help" || v == "-h") {
        println!("{USAGE}");
        return Ok(ExitCode::SUCCESS);
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
    let cwd = std::env::current_dir()?.canonicalize()?;
    let config = package_config(&cwd, options.packages.as_deref())?;
    check_packages(&config)?;
    let planner = planner_binary(&config, options.sdk.as_deref())?;
    let (plan, suites) = match self::plan(&planner, &cwd, &options)? {
        Planned::Exit(code) => return Ok(ExitCode::from(code)),
        Planned::Refused(message) => {
            eprintln!("error: test: {message}");
            return Ok(ExitCode::from(64));
        }
        Planned::Plan { plan, suites } => (plan, suites),
    };
    let json_reporter = match select_reporter(&options, &plan) {
        Ok(json) => json,
        Err(message) => {
            eprintln!("error: {message}");
            return Ok(ExitCode::from(64));
        }
    };
    let concurrency = plan["concurrency"].as_u64().unwrap_or(1).max(1) as usize;

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
    fs::write(workspace.path.join("selection.dart"), SELECTION_SOURCE)?;

    let runnable = suites.iter().filter(|s| s["action"] == "run").count();
    let mut tally = Tally::default();
    let mut binaries: BTreeMap<String, Option<PathBuf>> = BTreeMap::new();
    let parallel = concurrency > 1 && runnable > 1;
    let mut running: Vec<thread::JoinHandle<std::io::Result<Tally>>> = Vec::new();
    let collect =
        |handle: thread::JoinHandle<std::io::Result<Tally>>, tally: &mut Tally| match handle.join()
        {
            Ok(Ok(result)) => {
                tally.passed += result.passed;
                tally.failed += result.failed;
                tally.skipped += result.skipped;
                tally.broken |= result.broken;
            }
            Ok(Err(error)) => {
                eprintln!("test worker: {error}");
                tally.broken = true;
            }
            Err(_) => tally.broken = true,
        };
    for (index, suite) in suites.iter().enumerate() {
        let path = suite["path"].as_str().ok_or("test: invalid suite plan")?;
        let relay = |buffer: bool| Relay {
            json: json_reporter,
            suite: path.to_owned(),
            prefix: suites.len() > 1,
            buffer: buffer.then(Vec::new),
        };
        match suite["action"].as_str() {
            Some("none") => {}
            Some("error") => {
                let message = suite["message"].as_str().unwrap_or("");
                relay(false).event(json!({"type": "loadError", "error": message}));
                tally.failed += 1;
            }
            Some("skip") => {
                let mut relay = relay(false);
                if let Some(warning) = suite["warning"].as_str() {
                    relay.event(json!({"type": "print", "message": warning}));
                }
                relay.event(json!({"type": "testStart", "name": "(suite)"}));
                if let Some(reason) = suite["reason"].as_str() {
                    relay.event(json!({"type": "message", "name": "(suite)", "message": format!("Skip: {reason}")}));
                }
                relay.event(json!({"type": "testDone", "name": "(suite)", "result": "skipped", "skipped": true}));
                tally.skipped += 1;
            }
            Some("run") => {
                let binary = match binaries.get(path) {
                    Some(binary) => binary.clone(),
                    None => {
                        let suite_path = cwd.join(path).canonicalize()?;
                        let output = workspace.path.join(binaries.len().to_string());
                        fs::create_dir(&output)?;
                        let worker = output.join("worker.dart");
                        fs::write(&worker, worker_source(&output, &suite_path)?)?;
                        fs::write(output.join("selection.dart"), SELECTION_SOURCE)?;
                        let binary = output.join(executable_name("worker"));
                        let compilation =
                            compile(&worker, &binary, &config, options.sdk.as_deref())?;
                        let binary = if compilation.status.success() {
                            Some(binary)
                        } else {
                            eprintln!(
                                "Failed to compile {path} with DartForge:\n{}",
                                String::from_utf8_lossy(&compilation.stderr)
                            );
                            None
                        };
                        binaries.insert(path.to_owned(), binary.clone());
                        binary
                    }
                };
                let Some(binary) = binary else {
                    tally.failed += 1;
                    tally.broken = true;
                    continue;
                };
                let spec = workspace.path.join(format!("suite-{index}.json"));
                fs::write(&spec, serde_json::to_vec(&suite["worker"])?)?;
                let job = SuiteRun {
                    binary,
                    report: workspace.path.join(format!("result-{index}.json")),
                    spec,
                    token: format!("{unique}-{index}"),
                    relay: relay(parallel && !json_reporter),
                };
                while running.len() >= concurrency {
                    let handle = running.remove(0);
                    collect(handle, &mut tally);
                }
                let cwd = cwd.clone();
                let timeout = options.timeout;
                running.push(thread::spawn(move || run_worker(job, &cwd, timeout)));
            }
            _ => return Err("test: invalid suite plan".into()),
        }
    }
    for handle in running {
        collect(handle, &mut tally);
    }

    let ran = tally.passed + tally.failed + tally.skipped;
    let code = if tally.broken || tally.failed > 0 {
        1
    } else if ran == 0 {
        eprintln!(
            "{}",
            plan["noTestsMessage"]
                .as_str()
                .unwrap_or("No tests were found.")
        );
        79
    } else {
        0
    };
    if json_reporter {
        println!(
            "{}",
            json!({"type":"done", "success":code == 0, "passed": tally.passed, "failed": tally.failed, "skipped": tally.skipped})
        );
    } else {
        println!(
            "{} passed, {} failed, {} skipped; {}",
            tally.passed,
            tally.failed,
            tally.skipped,
            match code {
                0 => "all passed or skipped",
                79 => "no tests ran",
                _ => "test run failed",
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
    fn dartforge_options_are_kept_and_package_test_options_are_forwarded() {
        let options = Options::parse(&args(&[
            "test/a_test.dart",
            "--plain-name",
            "group async",
            "--reporter",
            "json",
            "-P",
            "ci",
            "--sdk",
            "/sdk/lib",
            "--keep-artifacts",
            "-p",
            "vm",
            "--",
            "--odd-name_test.dart",
        ]))
        .unwrap();
        assert_eq!(
            options.forwarded,
            [
                "test/a_test.dart",
                "--plain-name",
                "group async",
                "-P",
                "ci",
                "-p",
                "vm",
                "--",
                "--odd-name_test.dart"
            ]
        );
        assert_eq!(options.json, Some(true));
        assert_eq!(options.sdk, Some(PathBuf::from("/sdk/lib")));
        assert!(options.keep);
    }

    #[test]
    fn unsupported_platforms_reporters_and_runner_modes_are_refused() {
        for values in [
            vec!["--platform", "chrome"],
            vec!["-p", "vm,node"],
            vec!["-pchrome"],
            vec!["--platform=firefox"],
            vec!["--reporter", "compact"],
            vec!["--reporter=github"],
            vec!["--process-timeout", "0"],
            vec!["--coverage", "out"],
            vec!["--coverage=out"],
            vec!["--pause-after-load"],
            vec!["--debug"],
            vec!["--fail-fast"],
            vec!["--file-reporter", "json:out.json"],
            vec!["--watch"],
            vec!["--sdk"],
        ] {
            assert!(Options::parse(&args(&values)).is_err(), "{values:?}");
        }
        // Paths after `--` are never read as options.
        assert!(Options::parse(&args(&["--", "-pchrome"])).is_ok());
    }

    #[test]
    fn reporter_from_configuration_is_compared_with_the_package_default() {
        let options = Options::parse(&args(&[])).unwrap();
        let plan = |reporter: &str| json!({"reporter": reporter, "defaultReporter": "compact"});
        assert_eq!(select_reporter(&options, &plan("compact")), Ok(false));
        assert_eq!(select_reporter(&options, &plan("json")), Ok(true));
        assert_eq!(select_reporter(&options, &plan("expanded")), Ok(false));
        assert!(select_reporter(&options, &plan("github")).is_err());
        let explicit = Options::parse(&args(&["-r", "json"])).unwrap();
        assert_eq!(select_reporter(&explicit, &plan("github")), Ok(true));
    }

    #[test]
    fn relative_import_and_dart_escaping_preserve_literal_path() {
        let base = std::env::temp_dir().canonicalize().unwrap();
        let from = base.join("workers/0");
        let suite = base.join("project/test/quote'$__DARTFORGE_SUITE_URI___test.dart");
        let relative = relative_import(&from, &suite).unwrap();
        assert_eq!(
            relative,
            "../../project/test/quote'$__DARTFORGE_SUITE_URI___test.dart"
        );
        let source = worker_source(&from, &suite).unwrap();
        assert!(source.contains("quote\\'\\$__DARTFORGE_SUITE_URI___test.dart"));
        assert!(source.contains("Invoker.guard<Future<bool?>>"));
        assert!(source.contains("await declarer.declare<FutureOr<void>>(suite_source.main)"));
        assert!(planner_source().contains("const _protocol = '\\u0000dartforge-test-plan:';"));
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
        assert_eq!(found, config.canonicalize().unwrap());
        assert!(
            check_packages(&found)
                .unwrap_err()
                .to_string()
                .contains("package:test_core")
        );
        drop(workspace);
    }
}
