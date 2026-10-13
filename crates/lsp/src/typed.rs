//! Typed diagnostics in the continuous flow (audit L01).
//!
//! At each `didOpen`/`didChange`/`didClose`, the [`crate::Server`] publishes
//! the syntactic diagnostics right away (parser + local checkers of
//! `crates/analysis`) and asks this worker for the full analysis of the
//! document's package. The analysis is **the same** as `dartforge analyze`'s:
//! [`dartforge_parity::analysis::Motor`] (load, outline, `crates/types`
//! inference and the `crates/analysis` checkers) followed by
//! [`dartforge_parity::publicaveis`] (the publication rule of
//! `verificados.txt`, `analysis_options.yaml` and `// ignore:`). No rule is
//! duplicated here: this only chooses which files to analyze and discards
//! what became stale.
//!
//! * **One program per package.** All open documents of the package (plus
//!   the owning library of each open part) go into a single
//!   `Motor::analisar_com`, with the open texts winning over the disk. So
//!   editing an imported file reanalyzes whoever imports it, directly or
//!   transitively, without a dependency graph of its own.
//! * **Version.** Each result carries the version of the analyzed text; the
//!   server only publishes it if it is still the current one. A result of
//!   an old version is discarded, never overwrites that of a new one.
//! * **Fast edits.** A package's request is a set (marking it again does not
//!   enqueue another analysis): a burst of edits becomes one analysis. The
//!   running analysis checks, between the engine's phases, whether the
//!   package was marked again; if so, it stops and restarts with the new
//!   text.
//! * **Memory.** The worker retains only the [`Motor`] (SDK layout and
//!   library names) and a copy of the current text of each open document.
//!   Program, trees and type table live for one analysis and drop with it.
//!
//! [`Motor`]: dartforge_parity::analise::Motor

use crate::{Analyzer, SyntacticAnalyzer};
use dartforge_diagnostics::Diagnostic;
use dartforge_parity::analise::{Motor, chave, e_parte};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

/// Result of an analysis for one file: an open document (with the version
/// of the analyzed text) or a file of the roots that is not open (without a
/// version, with the disk text for the ranges).
#[derive(Debug)]
pub(crate) struct Result {
    pub uri: String,
    /// Version of the analyzed text; `None` for a file that is not open.
    pub version: Option<i32>,
    /// The analyzed text of a file that is not open.
    pub text: Option<String>,
    /// Full list to publish: syntax (parser) + published semantics.
    pub diagnostics: Vec<Diagnostic>,
    /// The diagnostics of a YAML file of the package (`analysis_options.yaml`,
    /// `pubspec.yaml`), already in the analyzer's JSON (§3.2: the non-Dart
    /// files validated when the contexts are created and at each watcher event).
    pub yaml: Vec<dartforge_parity::json::DiagJson>,
}

/// Open document as the worker sees it.
struct Document {
    version: i32,
    text: String,
    path: PathBuf,
    root: PathBuf,
}

#[derive(Default)]
struct State {
    documents: HashMap<String, Document>,
    /// Packages (root) with an analysis requested and not started yet.
    dirty: BTreeSet<PathBuf>,
    /// Package being analyzed now.
    running: Option<PathBuf>,
    /// Test hook: no analysis starts while true.
    paused: bool,
    shutdown: bool,
    /// `showTodos`: all TODOs, or those of these kinds (in uppercase).
    all: bool,
    all_types: Vec<String>,
    /// The folders excluded from analysis (`analysisExcludedFolders`).
    excluded: Vec<PathBuf>,
    /// The workspace folders (the included roots, §3.3).
    folders: Vec<PathBuf>,
    /// Per package, the files changed since its last analysis.
    changed: HashMap<PathBuf, BTreeSet<PathBuf>>,
    /// Packages whose next analysis publishes every file (the first one, or
    /// after the configuration changes).
    publish_all: BTreeSet<PathBuf>,
}

type Shared = Arc<(Mutex<State>, Condvar)>;

/// Hook called when there is a result (the `main` loop wakes up with it).
#[derive(Clone)]
pub(crate) struct Wake(pub Arc<dyn Fn() + Send + Sync>);

impl std::fmt::Debug for Wake {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Despertar")
    }
}

/// The server side: requests come in here, results go out through `receive`.
pub(crate) struct Typed {
    shared: Shared,
    results: mpsc::Receiver<Result>,
}

impl std::fmt::Debug for Typed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tipado").finish_non_exhaustive()
    }
}

/// The package root: the closest directory, walking up, with a
/// `pubspec.yaml` (the same rule as `dartforge analyze`); without a
/// `pubspec.yaml` up to the workspace folder that contains the file, that
/// folder (the context of the included root).
fn package_root(file: &Path, folders: &[PathBuf]) -> PathBuf {
    let mut current = file.parent();
    let folder = folders.iter().filter(|p| file.starts_with(p)).max_by_key(|p| p.components().count());
    while let Some(dir) = current {
        if dir.join("pubspec.yaml").is_file() {
            return dir.to_path_buf();
        }
        if folder.is_some_and(|p| dir == p.as_path()) {
            return dir.to_path_buf();
        }
        current = dir.parent();
    }
    crate::project::project_root(file)
}

impl Typed {
    /// Starts the worker with the SDK at `sdk_lib` (the SDK's `lib/`).
    pub(crate) fn start(sdk_lib: PathBuf, wake: Option<Wake>) -> Option<Self> {
        let shared: Shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let (tx, rx) = mpsc::channel();
        let p = Arc::clone(&shared);
        // Deep bodies recurse deeply: a dedicated stack, as in `analyze`.
        std::thread::Builder::new()
            .name("dartforge-lsp-tipado".into())
            .stack_size(1 << 30)
            .spawn(move || work(&p, &sdk_lib, &tx, wake))
            .ok()?;
        Some(Self {
            shared,
            results: rx,
        })
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Document `uri` is at version `version` with `text`: requests the
    /// analysis of its package (and cancels the one running for the same
    /// package).
    pub(crate) fn document(&self, uri: &str, version: i32, text: &str) {
        let Some(path) = url::Url::parse(uri)
            .ok()
            .filter(|u| u.scheme() == "file")
            .and_then(|u| u.to_file_path().ok())
        else {
            return;
        };
        if path.extension().is_none_or(|e| e != "dart") {
            return;
        }
        let mut e = self.state();
        let root = package_root(&path, &e.folders);
        e.dirty.insert(root.clone());
        e.changed.entry(root.clone()).or_default().insert(chave(&path));
        e.documents.insert(
            uri.to_string(),
            Document {
                version,
                text: text.to_string(),
                path,
                root,
            },
        );
        self.shared.1.notify_all();
    }

    /// The analysis configuration (`showTodos`, `analysisExcludedFolders`); the
    /// packages of the open documents are analyzed again.
    pub(crate) fn configure(&self, all: bool, all_types: Vec<String>, excluded: Vec<PathBuf>) {
        let mut e = self.state();
        e.all = all;
        e.all_types = all_types;
        e.excluded = excluded;
        let roots: Vec<PathBuf> = e.documents.values().map(|d| d.root.clone()).chain(folder_packages(&e.folders)).collect();
        e.publish_all.extend(roots.iter().cloned());
        e.dirty.extend(roots);
        self.shared.1.notify_all();
    }

    /// The workspace folders (§3.3): every `.dart` in them is analyzed and
    /// published (§3.2, I7 of §16.10), package by package; the first analysis of
    /// each package publishes every file.
    pub(crate) fn folders(&self, folders: Vec<PathBuf>) {
        let mut e = self.state();
        e.folders = folders;
        let packages = folder_packages(&e.folders);
        e.publish_all.extend(packages.iter().cloned());
        e.dirty.extend(packages);
        self.shared.1.notify_all();
    }

    /// A file changed on disk (`workspace/didChangeWatchedFiles`, Dart's
    /// watcher, §3.3): a `.dart` that is not open now counts from disk, and
    /// those that depend on it are reanalyzed; `analysis_options.yaml`,
    /// `pubspec.yaml` and `package_config.json` rebuild the contexts (the whole
    /// package is analyzed and published again, the YAML files too).
    pub(crate) fn file_on_disk(&self, path: &Path) {
        let mut e = self.state();
        let root = package_root(path, &e.folders);
        let inside = e.folders.iter().any(|p| path.starts_with(p)) || e.documents.values().any(|o| o.root == root);
        if !inside {
            return;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if matches!(name, "analysis_options.yaml" | "pubspec.yaml" | "package_config.json") {
            e.publish_all.insert(root.clone());
            e.changed.entry(root.clone()).or_default().insert(chave(path));
            e.dirty.insert(root);
            self.shared.1.notify_all();
            return;
        }
        if path.extension().is_none_or(|x| x != "dart") {
            return;
        }
        // An open document counts through the editor.
        if e.documents.values().any(|d| chave(&d.path) == chave(path)) {
            return;
        }
        e.changed.entry(root.clone()).or_default().insert(chave(path));
        e.dirty.insert(root);
        self.shared.1.notify_all();
    }

    /// The document closed: its text leaves the copy and those that depended on
    /// it are reanalyzed with the disk.
    pub(crate) fn closed(&self, uri: &str) {
        let mut e = self.state();
        if let Some(d) = e.documents.remove(uri) {
            // Without the editor's text, the disk's counts: the file changed for the
            // analysis (Dart's `removeOverlay`), and so did those that depend on it.
            let inside = e.folders.iter().any(|p| d.path.starts_with(p));
            if inside || e.documents.values().any(|o| o.root == d.root) {
                e.changed.entry(d.root.clone()).or_default().insert(chave(&d.path));
                e.dirty.insert(d.root);
                self.shared.1.notify_all();
            }
        }
    }

    /// Ready results, without blocking.
    pub(crate) fn receive(&self) -> Vec<Result> {
        self.results.try_iter().collect()
    }

    /// The worker is idle now (nothing requested, nothing running), without
    /// waiting.
    pub(crate) fn idle(&self) -> bool {
        let e = self.state();
        (e.dirty.is_empty() || e.paused) && e.running.is_none()
    }

    /// Waits for the worker to become idle (nothing requested, nothing
    /// running), up to `limit`. True if it did.
    pub(crate) fn wait_idle(&self, limit: Duration) -> bool {
        let end = Instant::now() + limit;
        let mut e = self.state();
        loop {
            if (e.dirty.is_empty() || e.paused) && e.running.is_none() {
                return true;
            }
            let now = Instant::now();
            if now >= end {
                return false;
            }
            e = self
                .shared
                .1
                .wait_timeout(e, end - now)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }

    pub(crate) fn pause(&self, pause: bool) {
        self.state().paused = pause;
        self.shared.1.notify_all();
    }
}

impl Drop for Typed {
    fn drop(&mut self) {
        // Does not wait for the running analysis: it sees `shutdown` at the next
        // cancellation point and the thread ends on its own.
        self.state().shutdown = true;
        self.shared.1.notify_all();
    }
}

/// Worker loop: one package at a time, the oldest marked first.
fn work(
    p: &Shared,
    sdk_lib: &Path,
    tx: &mpsc::Sender<Result>,
    wake: Option<Wake>,
) {
    let (lock, sinal) = &**p;
    let block = || {
        lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    };
    let mut engine: Option<Motor> = None;
    loop {
        let (root, open, texts, config, changed) = {
            let mut e = block();
            while !e.shutdown && (e.dirty.is_empty() || e.paused) {
                e = sinal
                    .wait(e)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            if e.shutdown {
                return;
            }
            let root = e.dirty.pop_first().expect("non-empty");
            let open: Vec<(String, i32, PathBuf, String)> = e
                .documents
                .iter()
                .filter(|(_, d)| d.root == root)
                .map(|(u, d)| (u.clone(), d.version, d.path.clone(), d.text.clone()))
                .collect();
            // All open documents (of any package) win over the disk.
            let texts: HashMap<PathBuf, String> = e
                .documents
                .values()
                .map(|d| (chave(&d.path), d.text.clone()))
                .collect();
            e.running = Some(root.clone());
            let config = AnalysisConfiguration { all: e.all, all_types: e.all_types.clone(), excluded: e.excluded.clone() };
            // Files that are not open only come in with the package in a workspace
            // folder.
            let in_workspace = e.folders.iter().any(|p| root.starts_with(p) || p.starts_with(&root));
            let all = in_workspace && e.publish_all.remove(&root);
            let changed_ = e.changed.remove(&root).unwrap_or_default();
            (root, open, texts, config, (in_workspace, all, changed_))
        };
        if engine.is_none() {
            match Motor::novo(sdk_lib) {
                Ok(m) => engine = Some(m),
                Err(error) => {
                    eprintln!("[dartforge-lsp] typed diagnostics disabled: {error}");
                    let mut e = block();
                    e.running = None;
                    e.dirty.clear();
                    e.paused = true;
                    sinal.notify_all();
                    continue;
                }
            }
        }
        let engine_ref = engine.as_ref().expect("engine ready");
        let cancelled = || {
            let e = block();
            e.shutdown || e.dirty.contains(&root)
        };
        let results = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            analyze_package(engine_ref, &root, &open, &texts, &cancelled, &config, &changed)
        }));
        let results = match results {
            Ok(r) => r,
            Err(_) => {
                eprintln!(
                    "[dartforge-lsp] the typed analysis of {} panicked; the syntactic diagnostics remain",
                    root.display()
                );
                Vec::new()
            }
        };
        if results.is_empty() && changed.0 {
            let mut e = block();
            if e.dirty.contains(&root) {
                e.changed.entry(root.clone()).or_default().extend(changed.2.iter().cloned());
                if changed.1 {
                    e.publish_all.insert(root.clone());
                }
            }
        }
        let happened = !results.is_empty();
        for r in results {
            let _ = tx.send(r);
        }
        {
            let mut e = block();
            e.running = None;
            sinal.notify_all();
        }
        if happened && let Some(d) = &wake {
            (d.0)();
        }
    }
}

/// What in the client configuration changes the analysis.
struct AnalysisConfiguration {
    all: bool,
    all_types: Vec<String>,
    excluded: Vec<PathBuf>,
}

/// The packages of the workspace folders: that of each `.dart` in them.
fn folder_packages(folders: &[PathBuf]) -> Vec<PathBuf> {
    let mut v: BTreeSet<PathBuf> = BTreeSet::new();
    for p in folders {
        for a in dartforge_parity::corpus::arquivos_dart(p) {
            v.insert(package_root(&a, folders));
        }
    }
    v.into_iter().collect()
}

/// The files `affected` by a change in `changed`: they and, through the
/// directives (`import`, `export`, `part`, `part of`), those that depend on
/// them, transitively (those that Dart's driver reanalyzes and republishes).
fn affected(files: &std::collections::BTreeMap<PathBuf, dartforge_parity::analise::Arquivo>, changed: &BTreeSet<PathBuf>, packages: Option<&dartforge_elements::PackageConfig>) -> BTreeSet<PathBuf> {
    let mut dependents: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    for (path, a) in files {
        let mut names = dartforge_intern::Interner::new();
        let analyzed = dartforge_frontend::parser::parse(&a.texto, &mut names);
        let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
        for d in &analyzed.unit.directives {
            use dartforge_frontend::ast::DirectiveKind as K;
            let lit = match &d.kind {
                K::Import { uri, .. } | K::Export { uri, .. } | K::Part { uri } => Some(uri),
                K::PartOf { uri: Some(uri), .. } => Some(uri),
                _ => None,
            };
            let Some(text) = lit.and_then(dartforge_elements::load::string_lit_value) else { continue };
            let target = if let Some(rest) = text.strip_prefix("package:") {
                let Some((package, rel)) = rest.split_once('/') else { continue };
                let Some(dir) = packages.and_then(|c| c.package_dirs.iter().find(|(n, _)| n == package).map(|(_, d)| d.clone())) else { continue };
                dir.join(rel)
            } else if text.contains(':') {
                continue;
            } else {
                base.join(&*text)
            };
            dependents.entry(chave(&target)).or_default().push(path.clone());
        }
    }
    let mut output: BTreeSet<PathBuf> = changed.iter().cloned().collect();
    let mut stack: Vec<PathBuf> = changed.iter().cloned().collect();
    while let Some(x) = stack.pop() {
        for d in dependents.get(&x).into_iter().flatten() {
            if output.insert(d.clone()) {
                stack.push(d.clone());
            }
        }
    }
    output
}

/// Analyzes package `root` in a single program: its open documents and, with
/// the package in a workspace folder, every `.dart` that `dart analyze`
/// would see (§3.2, I7). The open ones always come out; the ones not open,
/// all of them on the package's first analysis (or after the configuration
/// changes), and afterwards those affected by the changed files. Empty when
/// cancelled.
#[allow(clippy::type_complexity)]
fn analyze_package(
    engine: &Motor,
    root: &Path,
    open: &[(String, i32, PathBuf, String)],
    texts: &HashMap<PathBuf, String>,
    cancelled: &dyn Fn() -> bool,
    config: &AnalysisConfiguration,
    changed: &(bool, bool, BTreeSet<PathBuf>),
) -> Vec<Result> {
    let (in_workspace, all, changed) = (changed.0, changed.1, &changed.2);
    // Documents in excluded folders are not analyzed: their publication is empty.
    let excluded = |c: &Path| config.excluded.iter().any(|e| c.starts_with(e));
    let mut excluded_output: Vec<Result> = open
        .iter()
        .filter(|(_, _, c, _)| excluded(c))
        .map(|(u, v, _, _)| Result { uri: u.clone(), version: Some(*v), text: None, diagnostics: Vec::new(), yaml: Vec::new() })
        .collect();
    let open: Vec<(String, i32, PathBuf, String)> = open.iter().filter(|(_, _, c, _)| !excluded(c)).cloned().collect();
    let open = &open[..];
    let options = dartforge_parity::filtros::Opcoes::ler(root);
    // The package's `.dart` files that `dart analyze` would see (outside the
    // exclusions of `analysis_options.yaml` and of `analysisExcludedFolders`).
    let of_package: Vec<PathBuf> = if in_workspace {
        dartforge_parity::projetos::arquivos(root, &options).into_iter().filter(|a| !excluded(a)).collect()
    } else {
        Vec::new()
    };
    if open.is_empty() && of_package.is_empty() {
        return excluded_output;
    }
    let mut files: Vec<PathBuf> = of_package.clone();
    for (_, _, path, text) in open {
        // A part comes in through its owning library (the engine does not import parts).
        if e_parte(text)
            && let Some(owning) = crate::semantic::owning_library(path, text, None)
        {
            files.push(owning);
        }
        files.push(path.clone());
    }
    files.sort_by_key(|a| chave(a));
    files.dedup_by_key(|a| chave(a));
    let package_config = root.join(".dart_tool").join("package_config.json");
    let package_config = package_config.is_file().then_some(package_config);
    let packages = package_config.as_deref().and_then(|c| dartforge_elements::PackageConfig::load(c).ok());
    let Some(analysis) = engine.analisar_com(root, &files, package_config.as_deref(), texts, cancelled)
    else {
        return Vec::new();
    };
    let mut syntactic = SyntacticAnalyzer::new();
    let mut output = Vec::new();
    // The files not open to publish.
    let open_keys: BTreeSet<PathBuf> = open.iter().map(|(_, _, c, _)| chave(c)).collect();
    let publish: BTreeSet<PathBuf> = if all {
        of_package.iter().map(|a| chave(a)).collect()
    } else {
        affected(&analysis.arquivos, changed, packages.as_ref())
    };
    let mut targets: Vec<(String, Option<i32>, PathBuf, String)> = open.iter().map(|(u, v, c, t)| (u.clone(), Some(*v), c.clone(), t.clone())).collect();
    for a in &of_package {
        let k = chave(a);
        if open_keys.contains(&k) || !publish.contains(&k) {
            continue;
        }
        let Some(file_) = analysis.arquivos.get(&k) else { continue };
        let Ok(u) = url::Url::from_file_path(a) else { continue };
        targets.push((u.to_string(), None, a.clone(), file_.texto.clone()));
    }
    for (uri, version, path, text) in &targets {
        let Some(file) = analysis.arquivos.get(&chave(path)) else {
            continue;
        };
        if dartforge_parity::oraculo::relativo(path, root)
            .is_some_and(|rel| options.excluido(&rel))
        {
            continue;
        }
        // Syntax through the same parser as the immediate flow (no flicker); the
        // semantics, all from the engine, through the common publication rule.
        let mut diagnostics = syntactic.syntax(uri, text);
        // The published ones and, with `showTodos` (`_shouldSendError`), the TODOs
        // (all, or the requested kinds; always when the severity was promoted above
        // INFO by `analysis_options.yaml`), each at the position where the analyzer
        // reported it.
        let published: Vec<Diagnostic> =
            dartforge_parity::publicaveis(file, &options, true)
                .into_iter()
                .filter(|(_, syntax)| !syntax)
                .map(|(d, _)| d)
                .collect();
        let mut semantic: Vec<Diagnostic> = Vec::new();
        for (d, syntax) in dartforge_parity::publicaveis(file, &options, false) {
            if syntax {
                continue;
            }
            let published_ = published.iter().any(|x| x.span == d.span && x.code == d.code && x.message == d.message);
            let todo = d.code.is_some_and(|c| {
                c.info().tipo == dartforge_diagnostics::TipoErro::Todo
                    && (d.severity != dartforge_diagnostics::Severidade::Info
                        || config.all
                        || config.all_types.iter().any(|t| *t == c.info().nome.to_uppercase()))
            });
            if (published_ || todo) && !semantic.iter().any(|x: &Diagnostic| x.span == d.span && x.code == d.code && x.message == d.message) {
                semantic.push(d);
            }
        }
        // A published one that the unfiltered list does not bring (the
        // `IgnoreValidator` reports separately) goes at the end, as in the analyzer.
        for d in published {
            if !semantic.iter().any(|x| x.span == d.span && x.code == d.code && x.message == d.message) {
                semantic.push(d);
            }
        }
        // The order is that of the `LibraryAnalyzer` (the phases, §1.2 of INFRA):
        // the Dart server sends the list in insertion order (§3.6).
        diagnostics.extend(semantic);
        output.push(Result {
            uri: uri.clone(),
            version: *version,
            text: version.is_none().then(|| text.clone()),
            diagnostics,
            yaml: Vec::new(),
        });
    }
    // The package's YAML files: on the first analysis (and after the
    // configuration changes) and when one of them changed on disk. The text is
    // the disk's, as Dart's watcher reads it.
    if in_workspace {
        let options_yaml = root.join("analysis_options.yaml");
        let pubspec_yaml = root.join("pubspec.yaml");
        let changed_ = |p: &Path| all || changed.contains(&chave(p));
        let mut yaml: Vec<(PathBuf, Vec<dartforge_parity::json::DiagJson>)> = Vec::new();
        if options_yaml.is_file() && changed_(&options_yaml) {
            yaml.push((options_yaml.clone(), dartforge_parity::diagnosticos_das_opcoes(root)));
        }
        if pubspec_yaml.is_file() && (changed_(&pubspec_yaml) || changed_(&options_yaml)) {
            yaml.push((pubspec_yaml.clone(), dartforge_parity::diagnosticos_do_pubspec(root, &options)));
        }
        for (path, diagnostics) in yaml {
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let Ok(u) = url::Url::from_file_path(&path) else { continue };
            output.push(Result { uri: u.to_string(), version: None, text: Some(text), diagnostics: Vec::new(), yaml: diagnostics });
        }
    }
    output.append(&mut excluded_output);
    output
}

/// The SDK `lib/` of the analyzer, when it diagnoses with types.
pub(crate) fn analyzer_sdk<A: Analyzer>(analyzer: &A) -> Option<PathBuf> {
    if std::env::var("DARTFORGE_LSP_TIPADO").is_ok_and(|v| v == "0") {
        return None;
    }
    analyzer.sdk_for_diagnostics()
}
