//! Diagnostic parity with the official `dart analyze` (plan A1/A2).
//!
//! * [`json`]: the JSON v1 of `dart analyze --format=json`, read and written.
//! * [`analysis`]: our side — a batch of files becomes one program, with
//!   diagnostics per file, with codes.
//! * [`bridge`]: codes for the diagnostics that still come out as text only.
//! * [`filters`]: `analysis_options.yaml` and `// ignore:`.
//! * [`oracle`]: running and recording the oracle (SDK 3.6.2 and 3.13.4).
//! * [`corpus`]: the groups of `corpus/diagnostics/`.
//! * [`scoreboard`]: the per-code comparison.
//!
//! Publication rule (plan §2.3): syntax is always published; a semantic code
//! is only published if it is in `crates/analysis/verified.txt` (zero
//! false positives, wrong positions or wrong messages in the corpus and in the
//! real projects; false negatives are allowed). The rest exists internally and
//! only goes to the scoreboard.

pub mod analysis;
pub mod corpus;
pub mod filters;
pub mod json;
pub mod oracle;
pub mod scoreboard;
pub mod bridge;
pub mod projects;
pub mod output;

use analysis::{AnalysisResult, Engine};
use oracle::Record;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub use dartforge_analysis::publication::{VERIFIED, is_published, verified};

/// The analysis diagnostics in JSON v1, with the filters applied (the root
/// `analysis_options.yaml` and `// ignore:`). `only_published` applies the
/// publication rule.
pub fn json_diagnostics(analysis: &AnalysisResult, root: &Path, options: &filters::AnalysisOptions, only_published: bool) -> Vec<json::DiagJson> {
    let mut out = Vec::new();
    // Subfolder options (INFRA §4.5): the nearest `analysis_options.yaml`
    // above the file, below the root, applies to it; its `exclude`s add to
    // the root's.
    let mut by_subfolder: std::collections::HashMap<std::path::PathBuf, filters::AnalysisOptions> = std::collections::HashMap::new();
    for (p, a) in &analysis.files {
        if oracle::relative(p, root).is_some_and(|rel| options.is_excluded(&rel)) {
            continue;
        }
        let own: Option<&filters::AnalysisOptions> = match filters::AnalysisOptions::from_subfolder(p, root) {
            Some(file) => {
                if !by_subfolder.contains_key(&file) {
                    let read = filters::AnalysisOptions::read_file(&file);
                    by_subfolder.insert(file.clone(), read);
                }
                by_subfolder.get(&file)
            }
            None => None,
        };
        if own.is_some_and(|o| o.excludes_file(p)) {
            continue;
        }
        let options = own.unwrap_or(options);
        let lines = json::LineIndex::new(&a.text);
        let path = p.to_string_lossy();
        for (d, syntax) in publishable(a, options, only_published) {
            out.push(json::to_json(&path, &lines, &d, syntax));
        }
        // The lint rules enabled in `linter: rules:` (INFRA §8), over the
        // text's tree, with the `// ignore:` comments and the options'
        // `errors:` (which silences the rule or changes its severity).
        if options.rules.values().any(|enabled| *enabled) {
            let enabled = |rule: &str| options.rules.get(rule).copied().unwrap_or(false);
            let ignored = filters::IgnoreComments::from_text(&a.text);
            // The reports over the program's tree, with semantics; without it
            // (a file outside the program), over the text's tree.
            let mut reports = match &a.lint_reports {
                Some(r) => r.iter().filter(|x| enabled(x.code.name)).cloned().collect::<Vec<_>>(),
                None => {
                    let mut names = dartforge_intern::Interner::new();
                    let parsed = dartforge_frontend::parser::parse(&a.text, &mut names);
                    let unit = dartforge_analysis::Unit { ast: &parsed.ast, unit: &parsed.unit, source: &a.text };
                    dartforge_analysis::lints::run(unit, &names, &enabled)
                }
            };
            // Those of the rules that need the resolved program, already
            // computed by the engine.
            for l in &a.semantic_lints {
                if let Some(code) = dartforge_analysis::lints::codes_g::ALL.iter().copied().find(|k| k.unique == l.unique)
                    && enabled(code.name)
                {
                    reports.push(dartforge_analysis::lints::rules::LintReport { code, span: l.span, args: l.args.clone() });
                }
            }
            reports.sort_by_key(|r| (r.span.start, r.span.end, r.code.unique));
            // The same code at the same place with the same arguments is
            // reported only once (`final (a, b) = …` reports the keyword per
            // variable).
            reports.dedup_by(|x, y| x.span == y.span && x.code.unique == y.code.unique && x.args == y.args);
            for r in reports {
                let severity = match options.errors.get(r.code.name) {
                    Some(None) => continue,
                    Some(Some(s)) => s.name(),
                    None => "INFO",
                };
                // `cannot-ignore` with the lint's name: `// ignore:` does not silence it.
                if options.lint_ignorable(r.code.name, r.code.unique)
                    && ignored.ignores_lint(r.code.name, r.code.unique, lines.position(r.span.start).line)
                {
                    continue;
                }
                out.push(json::DiagJson {
                    code: r.code.name.to_string(),
                    severity: severity.to_string(),
                    error_type: "LINT".to_string(),
                    location: json::Local {
                        file: path.to_string(),
                        range: json::JsonRange { start: lines.position(r.span.start), end: lines.position(r.span.end.max(r.span.start)) },
                    },
                    problem_message: r.message(),
                    correction_message: r.correction(),
                    context_messages: Vec::new(),
                    documentation: r.code.documented.then(|| format!("https://dart.dev/diagnostics/{}", r.code.name)),
                });
            }
        }
    }
    json::sort(&mut out);
    out
}

/// A report on a non-Dart file, in JSON v1.
fn report_to_json(file: &str, lines: &json::LineIndex<'_>, r: &dartforge_analysis::non_dart::Report) -> json::DiagJson {
    json::DiagJson {
        code: r.code.name.to_string(),
        severity: r.code.severity.name().to_string(),
        error_type: r.code.error_type.name().to_string(),
        location: json::Local {
            file: file.to_string(),
            range: json::JsonRange { start: lines.position(r.span.start), end: lines.position(r.span.end.max(r.span.start)) },
        },
        problem_message: r.message(),
        correction_message: r.correction(),
        context_messages: Vec::new(),
        documentation: r.code.documented.then(|| format!("https://dart.dev/diagnostics/{}", r.code.name)),
    }
}

/// The diagnostics of each `AndroidManifest.xml` under `root` (the analyzer's
/// `ManifestValidator`, `dartforge_analysis::non_dart::manifest`), in JSON v1.
/// Only with `analyzer: optional-checks: chrome-os-manifest-checks` in the
/// options; hidden folders are skipped.
pub fn manifest_diagnostics(root: &Path, options: &filters::AnalysisOptions) -> Vec<json::DiagJson> {
    let mut out = Vec::new();
    if !options.chrome_os_manifest_checks {
        return out;
    }
    let mut stack = vec![root.to_path_buf()];
    let mut found: Vec<PathBuf> = Vec::new();
    while let Some(folder) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else { continue };
        for e in entries.flatten() {
            let (p, name) = (e.path(), e.file_name());
            let name = name.to_string_lossy();
            if p.is_dir() {
                if !name.starts_with('.') {
                    stack.push(p);
                }
            } else if name == "AndroidManifest.xml" {
                found.push(p);
            }
        }
    }
    found.sort();
    for file in found {
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        let lines = json::LineIndex::new(&text);
        let path = file.to_string_lossy();
        out.extend(dartforge_analysis::non_dart::manifest::validate(&text).iter().map(|r| report_to_json(&path, &lines, r)));
    }
    out
}

/// The diagnostics of the `analysis_options.yaml` of `root`
/// (`analyzeAnalysisOptions`, `dartforge_analysis::non_dart::options`), in
/// JSON v1. An `include:` is relative to the including file, or `package:`
/// through the `package_config.json` found above the root. The SDK constraint
/// is not passed: a lint rule removed with `since` is not reported.
pub fn options_diagnostics(root: &Path) -> Vec<json::DiagJson> {
    let file = root.join("analysis_options.yaml");
    let Ok(text) = std::fs::read_to_string(&file) else { return Vec::new() };
    let config = dartforge_elements::config::PackageConfig::discover(&file)
        .and_then(|p| dartforge_elements::config::PackageConfig::load(&p).ok());
    let resolver = |from: &Path, uri: &str| -> Option<PathBuf> {
        if uri.starts_with("package:") {
            config.as_ref().and_then(|c| c.resolve_package_uri(uri).ok())
        } else {
            from.parent().map(|p| p.join(uri))
        }
    };
    let root_text = root.to_string_lossy();
    let ctx = dartforge_analysis::non_dart::options::OptionsContext {
        file: &file,
        context_root: root_text.as_ref(),
        resolver: &resolver,
        sdk_allows: None,
    };
    let lines = json::LineIndex::new(&text);
    let path = file.to_string_lossy();
    dartforge_analysis::non_dart::options::analyze(&text, &ctx).iter().map(|r| report_to_json(&path, &lines, r)).collect()
}

/// The diagnostics of the `pubspec.yaml` of `root` (the analyzer's
/// `PubspecValidator`, `dartforge_analysis::non_dart::pubspec`, and the pubspec
/// lints enabled in the options, `dartforge_analysis::lints::pubspec`), in
/// JSON v1, without those silenced by `# ignore:`/`# ignore_for_file:` (the
/// `IgnoreInfo.forYaml` at the end of `validatePubspec`). Without the file,
/// nothing. The validator's warnings come out as it gives them; the lints
/// take the options' `errors:`.
pub fn pubspec_diagnostics(root: &Path, options: &filters::AnalysisOptions) -> Vec<json::DiagJson> {
    let file = root.join("pubspec.yaml");
    let Ok(text) = std::fs::read_to_string(&file) else { return Vec::new() };
    let lines = json::LineIndex::new(&text);
    let path = file.to_string_lossy();
    let ignored = dartforge_analysis::lints::pubspec::YamlIgnores::parse(&text);
    let mut out: Vec<json::DiagJson> = dartforge_analysis::non_dart::pubspec::validate(&text, root)
        .into_iter()
        .filter(|r| !ignored.ignores(r.code.name, r.code.name, r.span.start))
        .map(|r| json::DiagJson {
            code: r.code.name.to_string(),
            severity: r.code.severity.name().to_string(),
            error_type: r.code.error_type.name().to_string(),
            location: json::Local {
                file: path.to_string(),
                range: json::JsonRange { start: lines.position(r.span.start), end: lines.position(r.span.end.max(r.span.start)) },
            },
            problem_message: r.message(),
            correction_message: r.correction(),
            context_messages: Vec::new(),
            documentation: r.code.documented.then(|| format!("https://dart.dev/diagnostics/{}", r.code.name)),
        })
        .collect();
    if options.rules.values().any(|enabled| *enabled) {
        let enabled = |rule: &str| options.rules.get(rule).copied().unwrap_or(false);
        for r in dartforge_analysis::lints::pubspec::run(&text, &enabled) {
            if ignored.ignores(r.code.name, r.code.unique, r.span.start) {
                continue;
            }
            let severity = match options.errors.get(r.code.name) {
                Some(None) => continue,
                Some(Some(s)) => s.name(),
                None => "INFO",
            };
            out.push(json::DiagJson {
                code: r.code.name.to_string(),
                severity: severity.to_string(),
                error_type: "LINT".to_string(),
                location: json::Local {
                    file: path.to_string(),
                    range: json::JsonRange { start: lines.position(r.span.start), end: lines.position(r.span.end.max(r.span.start)) },
                },
                problem_message: r.message(),
                correction_message: r.correction(),
                context_messages: Vec::new(),
                documentation: r.code.documented.then(|| format!("https://dart.dev/diagnostics/{}", r.code.name)),
            });
        }
    }
    out
}

/// The diagnostics of a file that reach the user, with the syntax flag: the
/// publication rule (with `only_published`), the `analysis_options.yaml`
/// (`options`) and the text's `// ignore:` comments. It is the filter shared by
/// `dartforge analyze` and the LSP.
pub fn publishable(a: &analysis::FileAnalysis, options: &filters::AnalysisOptions, only_published: bool) -> Vec<(dartforge_diagnostics::Diagnostic, bool)> {
    let lines = json::LineIndex::new(&a.text);
    let ignored = filters::IgnoreComments::from_text(&a.text);
    let mut out = Vec::new();
    for (i, d) in a.diags.iter().enumerate() {
        let syntax = i < a.syntactic_count;
        if only_published && !is_published(d, syntax) {
            continue;
        }
        // The two `inference_failure_on_*` of the `BestPracticesVerifier` only
        // with `strict-inference: true` (INFRA §4.6); the engine always
        // reports them.
        if !options.strict_inference
            && d.code.is_some_and(|c| matches!(c.info().name, "inference_failure_on_untyped_parameter" | "inference_failure_on_function_return_type"))
        {
            continue;
        }
        let Some(d) = options.process(d.clone()) else { continue };
        let line = lines.position(d.span.start).line;
        if options.is_ignorable(&d) && ignored.ignores(&d, line) {
            continue;
        }
        out.push((d, syntax));
    }
    // `IgnoreValidator` (the unit's last phase): the names repeated in the
    // `ignore` comments.
    for d in ignored.duplicates() {
        if only_published && !is_published(&d, false) {
            continue;
        }
        if let Some(d) = options.process(d) {
            out.push((d, false));
        }
    }
    out
}

/// Like [`json_diagnostics`], as records relative to `root`, with what
/// `dart analyze` (the oracle) prints: a `TODO` only comes out when `errors:`
/// promoted it (`analyze.dart:191-192`, the same filter as our CLI).
pub fn records(analysis: &AnalysisResult, root: &Path, options: &filters::AnalysisOptions, only_published: bool) -> Vec<Record> {
    let mut out: Vec<Record> = json_diagnostics(analysis, root, options, only_published)
        .iter()
        .filter(|j| !(j.error_type == "TODO" && j.severity == "INFO"))
        .filter_map(|j| Record::from_json(j, root))
        .collect();
    out.sort();
    out
}

/// Our side's result for a set of files.
#[derive(Debug, Default)]
pub struct Run {
    pub records: Vec<Record>,
    pub files: usize,
    pub batches: usize,
    /// Files whose analysis panicked or exceeded the memory cap or the time
    /// limit (isolated one by one).
    pub panics: Vec<String>,
    pub ambiguous: usize,
}

/// How the batches run.
#[derive(Debug, Clone)]
pub struct Execution {
    pub workers: usize,
    pub batch_size: usize,
    pub progress: bool,
    /// `Some(exe)`: each batch in a child process (`exe _batch ...`), so that
    /// a stack overflow, out-of-memory or infinite loop in one analysis does
    /// not take the scoreboard down. `None`: on the same thread (only panics
    /// are contained).
    pub isolate: Option<PathBuf>,
    /// Maximum time of an isolated batch.
    pub max_time: std::time::Duration,
}

/// Analyzes `files` of `root` in batches. The batches are defined by the
/// order of the files, not by the workers: the result is the same with 1, 4
/// or 8.
pub fn run_ours(
    engine: &Engine,
    root: &Path,
    files: &[PathBuf],
    packages: Option<&Path>,
    options: &filters::AnalysisOptions,
    ex: &Execution,
) -> Run {
    let batches: Vec<&[PathBuf]> = files.chunks(ex.batch_size.max(1)).collect();
    type Partial = (Vec<Record>, Vec<String>, usize);
    let results: Mutex<Vec<Option<Partial>>> = Mutex::new((0..batches.len()).map(|_| None).collect());
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let work = || {
        loop {
            let i = next.fetch_add(1, Ordering::SeqCst);
            if i >= batches.len() {
                break;
            }
            let r = match &ex.isolate {
                Some(exe) => isolated_batch(exe, root, batches[i], packages, ex.max_time),
                None => analyze_batch(engine, root, batches[i], packages),
            };
            results.lock().expect("results")[i] = Some(r);
            let n = done.fetch_add(1, Ordering::SeqCst) + 1;
            if ex.progress && (n % 20 == 0 || n == batches.len()) {
                eprintln!("  {n}/{} batches", batches.len());
            }
        }
    };
    std::thread::scope(|s| {
        let mut hs = Vec::new();
        for _ in 0..ex.workers.max(1) {
            // Deep bodies recurse deeply: own stack, like `compile-js`.
            hs.push(std::thread::Builder::new().stack_size(1 << 30).spawn_scoped(s, work).expect("thread"));
        }
        for h in hs {
            let _ = h.join();
        }
    });
    let mut run = Run { files: files.len(), batches: batches.len(), ..Run::default() };
    for r in results.into_inner().expect("results").into_iter().flatten() {
        run.records.extend(r.0);
        run.panics.extend(r.1);
        run.ambiguous += r.2;
    }
    // `analyzer: exclude/errors` of `analysis_options.yaml`, applied here
    // (the ignore comments were already applied by each batch).
    run.records = run
        .records
        .into_iter()
        .filter(|r| !options.is_excluded(&r.file))
        .filter_map(|mut r| match options.errors.get(&r.code) {
            None => Some(r),
            Some(None) => None,
            Some(Some(s)) => {
                r.severity = s.name().to_string();
                Some(r)
            }
        })
        .collect();
    run.records.sort();
    run.panics.sort();
    run
}

/// A batch on the same thread; on a panic, each file on its own.
fn analyze_batch(engine: &Engine, root: &Path, batch: &[PathBuf], packages: Option<&Path>) -> (Vec<Record>, Vec<String>, usize) {
    let attempt = |files: &[PathBuf]| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let a = engine.analyze(root, files, packages);
            (records(&a, root, &filters::AnalysisOptions::default(), false), a.ambiguous)
        }))
    };
    if let Ok((r, amb)) = attempt(batch) {
        return (r, Vec::new(), amb);
    }
    let mut regs = Vec::new();
    let mut panics = Vec::new();
    let mut amb = 0;
    for a in batch {
        match attempt(std::slice::from_ref(a)) {
            Ok((r, x)) => {
                regs.extend(r);
                amb += x;
            }
            Err(_) => panics.push(oracle::relative(a, root).unwrap_or_else(|| a.display().to_string())),
        }
    }
    (regs, panics, amb)
}

/// A batch in a child process; if it dies, each file in its own child.
fn isolated_batch(
    exe: &Path,
    root: &Path,
    batch: &[PathBuf],
    packages: Option<&Path>,
    max_time: std::time::Duration,
) -> (Vec<Record>, Vec<String>, usize) {
    if let Some((r, amb)) = child(exe, root, batch, packages, max_time) {
        return (r, Vec::new(), amb);
    }
    let mut regs = Vec::new();
    let mut panics = Vec::new();
    let mut amb = 0;
    for a in batch {
        match child(exe, root, std::slice::from_ref(a), packages, max_time) {
            Some((r, x)) => {
                regs.extend(r);
                amb += x;
            }
            None => panics.push(oracle::relative(a, root).unwrap_or_else(|| a.display().to_string())),
        }
    }
    (regs, panics, amb)
}

/// The executable with the `_batch` subcommand, the files on standard input.
/// `None` if the child does not finish successfully within the time limit.
fn child(
    exe: &Path,
    root: &Path,
    batch: &[PathBuf],
    packages: Option<&Path>,
    max_time: std::time::Duration,
) -> Option<(Vec<Record>, usize)> {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    let pk = packages.map(|p| p.as_os_str().to_owned()).unwrap_or_else(|| "-".into());
    let mut c = Command::new(exe)
        .arg("_batch")
        .arg(root)
        .arg(pk)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    {
        let mut input = c.stdin.take()?;
        let list: String = batch.iter().map(|a| format!("{}\n", a.display())).collect();
        input.write_all(list.as_bytes()).ok()?;
    }
    // The output is read on a thread so the child does not block on a full pipe.
    let mut output = c.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = output.read_to_string(&mut s);
        s
    });
    let start = std::time::Instant::now();
    let status = loop {
        match c.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if start.elapsed() > max_time => {
                let _ = c.kill();
                let _ = c.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
            Err(_) => break None,
        }
    };
    let text = reader.join().ok()?;
    if !status?.success() {
        return None;
    }
    let mut regs = Vec::new();
    let mut amb = None;
    for l in text.lines() {
        if let Some(n) = l.strip_prefix("#ambiguous ") {
            amb = n.trim().parse().ok();
        } else if !l.is_empty() {
            regs.push(serde_json::from_str(l).ok()?);
        }
    }
    Some((regs, amb?))
}

/// The child side of `child`: analyzes and writes the records (with the
/// ignore comments; the parent applies the options) and `#ambiguous N`. A
/// watchdog ends the process if live memory (`live`) exceeds `cap` bytes.
pub fn run_child_batch(
    root: &Path,
    packages: Option<&Path>,
    files: &[PathBuf],
    live: fn() -> usize,
    cap: usize,
) -> i32 {
    std::thread::spawn(move || {
        loop {
            if live() > cap {
                std::process::exit(98);
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    });
    let engine = match Engine::discover() {
        Ok(m) => m,
        Err(_) => return 2,
    };
    let (r2, p2, a2) = (root.to_path_buf(), packages.map(Path::to_path_buf), files.to_vec());
    let finished = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let a = engine.analyze(&r2, &a2, p2.as_deref());
            (records(&a, &r2, &filters::AnalysisOptions::default(), false), a.ambiguous)
        })
        .expect("thread")
        .join();
    match finished {
        Ok((regs, amb)) => {
            let mut s = String::new();
            for r in regs {
                s.push_str(&serde_json::to_string(&r).expect("JSON"));
                s.push('\n');
            }
            s.push_str(&format!("#ambiguous {amb}\n"));
            use std::io::Write;
            let _ = std::io::stdout().write_all(s.as_bytes());
            0
        }
        Err(_) => 3,
    }
}

/// Silences the default panic message (panics are counted in the report).
pub fn silence_panics() {
    std::panic::set_hook(Box::new(|_| {}));
}
