//! Typed diagnostics in the continuous flow (audit L01).
//!
//! At each `didOpen`/`didChange`/`didClose`, the [`crate::Server`] publishes
//! the syntactic diagnostics right away (parser + local checkers of
//! `crates/analysis`) and asks this worker for the full analysis of the
//! document's package. The analysis is **the same** as `dartforge analyze`'s:
//! [`dartforge_parity::analysis::Engine`] (load, outline, `crates/types`
//! inference and the `crates/analysis` checkers) followed by
//! [`dartforge_parity::publishable`] (the publication rule of
//! `verified.txt`, `analysis_options.yaml` and `// ignore:`). No rule is
//! duplicated here: this only chooses which files to analyze and discards
//! what became stale.
//!
//! * **One program per package.** All open documents of the package (plus
//!   the owning library of each open part) go into a single
//!   `Engine::analyze_with`, with the open texts winning over the disk. So
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
//! * **Memory.** The worker retains only the [`Engine`] (SDK layout and
//!   library names) and the current text of each open document (shared with
//!   the jobs, not copied). Program, trees and type table live for one
//!   analysis — in a child process when the server was given its executable
//!   ([`run_in_child`], what the binary does): the allocator of a long-lived
//!   process keeps the pages of an analysis that dropped, so the server would
//!   otherwise stay at the peak of the largest package analyzed (`docs/LSP.md`,
//!   "Memória").
//!
//! [`Engine`]: dartforge_parity::analysis::Engine

use crate::{Analyzer, SyntacticAnalyzer};
use dartforge_diagnostics::Diagnostic;
use dartforge_parity::analysis::{Engine, is_part, key};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

/// Result of an analysis for one file: an open document (with the version
/// of the analyzed text) or a file of the roots that is not open (without a
/// version, with the disk text for the ranges).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Result {
    pub uri: String,
    /// Version of the analyzed text; `None` for a file that is not open.
    pub version: Option<i32>,
    /// The analyzed text of a file that is not open, to place its diagnostics
    /// (`None` when it has none, and for an open document).
    pub text: Option<String>,
    /// Full list to publish: syntax (parser) + published semantics.
    pub diagnostics: Vec<Diagnostic>,
    /// The diagnostics of a YAML file of the package (`analysis_options.yaml`,
    /// `pubspec.yaml`), already in the analyzer's JSON (§3.2: the non-Dart
    /// files validated when the contexts are created and at each watcher event).
    pub yaml: Vec<dartforge_parity::json::DiagJson>,
    /// What the worker remembers of an analyzed `.dart` file to plan the next
    /// analyses ([`Memo`]); `None` for the YAML files and the documents in
    /// excluded folders.
    #[serde(default)]
    pub facts: Option<Facts>,
}

/// The API signature and the directive targets of an analyzed text.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Facts {
    /// The file, spelled as analyzed.
    pub path: PathBuf,
    /// [`dartforge_elements::incremental::api_signature`] of the text.
    pub signature: Option<u64>,
    /// The [`key`]s of the files its directives point to, each with whether the
    /// directive is a `part` or a `part of`.
    pub targets: Vec<(PathBuf, bool)>,
}

/// What the last completed analyses of a package left, per file [`key`]: the
/// published diagnostics with the [`Facts`] of the analyzed text.
///
/// It is what keeps an edit local (docs/LSP.md, "Edição incremental"): a
/// changed file whose API signature is the same only reanalyzes its library
/// (the file, its parts and its owner); the files that depend on it are
/// republished with what they had, because the declarations they see did not
/// change. A changed signature reanalyzes the dependents through the directives,
/// transitively, as before. Only diagnostics and directive targets are kept,
/// never a program or a tree.
#[derive(Debug, Default)]
struct Memo {
    files: HashMap<PathBuf, Remembered>,
}

#[derive(Debug)]
struct Remembered {
    facts: Facts,
    diagnostics: Vec<Diagnostic>,
}

/// Open document as the worker sees it.
struct Document {
    version: i32,
    /// Shared with the analysis jobs, which take it without a copy.
    text: Arc<str>,
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
    /// The packages of `folders` must be enumerated (and analyzed with every
    /// file published): the worker walks the folders, never the main loop, so
    /// a large workspace does not block the requests.
    scan: bool,
    /// The worker is walking the folders (without the lock): not idle yet.
    scanning: bool,
    /// The executable that runs each analysis in a child process
    /// ([`run_in_child`]); without it, the worker thread analyzes.
    process: Option<PathBuf>,
    /// Per package, the files changed since its last analysis.
    changed: HashMap<PathBuf, BTreeSet<PathBuf>>,
    /// Packages whose next analysis publishes every file (the first one, or
    /// after the configuration changes).
    publish_all: BTreeSet<PathBuf>,
    /// Packages whose [`Memo`] no longer holds (the configuration or the
    /// package configuration changed): the next analysis starts from scratch.
    stale: BTreeSet<PathBuf>,
    /// Files given to the analyses so far (measurement and tests).
    analyzed_files: usize,
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
    let folder = folders
        .iter()
        .filter(|p| file.starts_with(p))
        .max_by_key(|p| p.components().count());
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
    /// Starts the worker with the SDK at `sdk_lib` (the SDK's `lib/`). With
    /// `process` (the server's own executable), each analysis runs in a child
    /// process (see [`run_in_child`]); without it, in the worker thread.
    pub(crate) fn start(
        sdk_lib: PathBuf,
        wake: Option<Wake>,
        process: Option<PathBuf>,
    ) -> Option<Self> {
        let shared: Shared = Arc::new((
            Mutex::new(State {
                process,
                ..State::default()
            }),
            Condvar::new(),
        ));
        let (tx, rx) = mpsc::channel();
        let p = Arc::clone(&shared);
        // Deep bodies recurse deeply: a dedicated stack, as in `analyze`.
        std::thread::Builder::new()
            .name("dartforge-lsp-typed".into())
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
        e.changed
            .entry(root.clone())
            .or_default()
            .insert(key(&path));
        e.documents.insert(
            uri.to_string(),
            Document {
                version,
                text: Arc::from(text),
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
        let roots: Vec<PathBuf> = e.documents.values().map(|d| d.root.clone()).collect();
        e.publish_all.extend(roots.iter().cloned());
        e.stale.extend(roots.iter().cloned());
        e.dirty.extend(roots);
        e.scan = true;
        self.shared.1.notify_all();
    }

    /// The workspace folders (§3.3): every `.dart` in them is analyzed and
    /// published (§3.2, I7 of §16.10), package by package; the first analysis of
    /// each package publishes every file.
    pub(crate) fn folders(&self, folders: Vec<PathBuf>) {
        let mut e = self.state();
        e.folders = folders;
        e.scan = true;
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
        let inside = e.folders.iter().any(|p| path.starts_with(p))
            || e.documents.values().any(|o| o.root == root);
        if !inside {
            return;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if matches!(
            name,
            "analysis_options.yaml" | "pubspec.yaml" | "package_config.json"
        ) {
            e.publish_all.insert(root.clone());
            e.stale.insert(root.clone());
            e.changed.entry(root.clone()).or_default().insert(key(path));
            e.dirty.insert(root);
            self.shared.1.notify_all();
            return;
        }
        if path.extension().is_none_or(|x| x != "dart") {
            return;
        }
        // An open document counts through the editor.
        if e.documents.values().any(|d| key(&d.path) == key(path)) {
            return;
        }
        e.changed.entry(root.clone()).or_default().insert(key(path));
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
                e.changed
                    .entry(d.root.clone())
                    .or_default()
                    .insert(key(&d.path));
                e.dirty.insert(d.root);
                self.shared.1.notify_all();
            }
        }
    }

    /// Files given to the analyses so far (a cancelled analysis counts too).
    pub(crate) fn analyzed_files(&self) -> usize {
        self.state().analyzed_files
    }

    /// Ready results, without blocking.
    pub(crate) fn receive(&self) -> Vec<Result> {
        self.results.try_iter().collect()
    }

    /// The worker is idle now (nothing requested, nothing running), without
    /// waiting.
    pub(crate) fn idle(&self) -> bool {
        let e = self.state();
        ((e.dirty.is_empty() && !e.scan) || e.paused) && e.running.is_none() && !e.scanning
    }

    /// Waits for the worker to become idle (nothing requested, nothing
    /// running), up to `limit`. True if it did.
    pub(crate) fn wait_idle(&self, limit: Duration) -> bool {
        let end = Instant::now() + limit;
        let mut e = self.state();
        loop {
            if ((e.dirty.is_empty() && !e.scan) || e.paused) && e.running.is_none() && !e.scanning {
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

    /// Runs the next analyses in child processes of `exe` (or, with `None`, in
    /// the worker thread).
    pub(crate) fn set_process(&self, exe: Option<PathBuf>) {
        self.state().process = exe;
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
fn work(p: &Shared, sdk_lib: &Path, tx: &mpsc::Sender<Result>, wake: Option<Wake>) {
    let (lock, sinal) = &**p;
    let block = || {
        lock.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    };
    let mut engine: Option<Engine> = None;
    // What the completed analyses left, per package root (only in this thread).
    let mut memos: HashMap<PathBuf, Memo> = HashMap::new();
    loop {
        let (root, open, texts, config, changed, process) = {
            let mut e = block();
            while !e.shutdown && ((e.dirty.is_empty() && !e.scan) || e.paused) {
                e = sinal
                    .wait(e)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            if e.shutdown {
                return;
            }
            if e.scan {
                // The walk runs without the lock: the server keeps answering.
                e.scan = false;
                e.scanning = true;
                let folders = e.folders.clone();
                drop(e);
                let packages = folder_packages(&folders);
                let mut e = block();
                e.scanning = false;
                if e.folders == folders {
                    e.publish_all.extend(packages.iter().cloned());
                    e.dirty.extend(packages);
                }
                sinal.notify_all();
                continue;
            }
            let root = e.dirty.pop_first().expect("non-empty");
            let open: Vec<(String, i32, PathBuf, Arc<str>)> = e
                .documents
                .iter()
                .filter(|(_, d)| d.root == root)
                .map(|(u, d)| (u.clone(), d.version, d.path.clone(), d.text.clone()))
                .collect();
            // All open documents (of any package) win over the disk.
            let texts: HashMap<PathBuf, Arc<str>> = e
                .documents
                .values()
                .map(|d| (key(&d.path), d.text.clone()))
                .collect();
            e.running = Some(root.clone());
            let config = AnalysisConfiguration {
                all: e.all,
                all_types: e.all_types.clone(),
                excluded: e.excluded.clone(),
            };
            // Files that are not open only come in with the package in a workspace
            // folder.
            let in_workspace = e
                .folders
                .iter()
                .any(|p| root.starts_with(p) || p.starts_with(&root));
            let all = in_workspace && e.publish_all.remove(&root);
            let reset = e.stale.remove(&root);
            let changed_ = e.changed.remove(&root).unwrap_or_default();
            // The memos of the packages that have nothing open and are outside the
            // workspace folders are no longer needed.
            let live: BTreeSet<&PathBuf> = e.documents.values().map(|d| &d.root).collect();
            memos.retain(|r, _| {
                *r == root
                    || live.contains(r)
                    || e.folders
                        .iter()
                        .any(|p| r.starts_with(p) || p.starts_with(r))
            });
            (
                root,
                open,
                texts,
                config,
                (in_workspace, all, reset, changed_),
                e.process.clone(),
            )
        };
        if engine.is_none() {
            match Engine::new(sdk_lib) {
                Ok(m) => engine = Some(m),
                Err(error) => {
                    eprintln!("[dartforge-lsp] typed diagnostics disabled: {error}");
                    let mut e = block();
                    e.running = None;
                    e.dirty.clear();
                    e.scan = false;
                    e.paused = true;
                    sinal.notify_all();
                    continue;
                }
            }
        }
        let engine_ref = engine.as_ref().expect("engine ready");
        crate::memory::log(&format!("before the analysis of {}", root.display()));
        let cancelled = || {
            let e = block();
            e.shutdown || e.dirty.contains(&root)
        };
        let (in_workspace, all, reset, changed) = changed;
        // Without a memo (the first analysis, or after the configuration changed)
        // every file is analyzed.
        if reset || all {
            memos.remove(&root);
        }
        let plan = plan(
            memos.get(&root),
            &root,
            open,
            &texts,
            &config,
            in_workspace,
            &changed,
        );
        let job = Arc::new(Job {
            sdk_lib: sdk_lib.to_path_buf(),
            root: root.clone(),
            open: plan.open,
            others: plan.others,
            texts,
            config,
            in_workspace,
            all,
            changed,
        });
        let fresh = if job.open.is_empty() && job.others.is_empty() {
            // Nothing to analyze: every file to publish keeps what it had.
            Some(Vec::new())
        } else {
            block().analyzed_files += job.open.len() + job.others.len();
            let results = match process
                .as_deref()
                .and_then(|exe| run_in_child(exe, &job, &cancelled))
            {
                Some(results) => results,
                None => run_in_process(engine_ref, &job, &cancelled),
            };
            (!results.is_empty()).then_some(results)
        };
        let Some(fresh) = fresh else {
            let mut e = block();
            if e.dirty.contains(&root) {
                // Cancelled: the next analysis still sees these changes (and the
                // signatures they are compared with are the memo's, untouched).
                e.changed
                    .entry(root.clone())
                    .or_default()
                    .extend(job.changed.iter().cloned());
                if job.all {
                    e.publish_all.insert(root.clone());
                }
                if reset {
                    e.stale.insert(root.clone());
                }
            } else {
                // Failed (a panic, a crashed child): what the memo says may be
                // stale; the next analysis of the package starts from scratch.
                memos.remove(&root);
            }
            e.running = None;
            sinal.notify_all();
            continue;
        };
        let memo = memos.entry(root.clone()).or_default();
        let results = merge(memo, fresh, plan.order, plan.excluded);
        let happened = !results.is_empty();
        for r in results {
            let _ = tx.send(r);
        }
        crate::memory::log("after the analysis");
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
#[derive(serde::Serialize, serde::Deserialize)]
struct AnalysisConfiguration {
    all: bool,
    all_types: Vec<String>,
    excluded: Vec<PathBuf>,
}

/// One analysis: the package, the texts that win over the disk and what to
/// publish. It is also the request a child process receives (JSON on its
/// `stdin`).
#[derive(serde::Serialize, serde::Deserialize)]
struct Job {
    /// The SDK's `lib/`.
    sdk_lib: PathBuf,
    /// The package root.
    root: PathBuf,
    /// The open documents of the package to analyze: URI, version, path and
    /// text.
    open: Vec<(String, i32, PathBuf, Arc<str>)>,
    /// The files of the package that are not open to analyze and publish.
    others: Vec<PathBuf>,
    /// The texts of every open document (of any package), by [`key`].
    texts: HashMap<PathBuf, Arc<str>>,
    config: AnalysisConfiguration,
    /// The package is in a workspace folder (its YAML files are published).
    in_workspace: bool,
    /// The first analysis of the package, or after the configuration changes
    /// (its YAML files are published).
    all: bool,
    /// The files changed since the package's last analysis.
    changed: BTreeSet<PathBuf>,
}

/// The hidden argument that makes the `dartforge-lsp` binary a child analysis
/// process ([`typed_analysis_child`]).
pub const CHILD_ARGUMENT: &str = "--typed-analysis";

/// Runs `job` in the worker thread. Empty when cancelled or when the analysis
/// panicked (the syntactic diagnostics remain).
fn run_in_process(engine: &Engine, job: &Job, cancelled: &dyn Fn() -> bool) -> Vec<Result> {
    // With the memory log on, the cancellation points (between the engine's
    // phases and before each library's bodies) sample the live heap.
    let calls = std::cell::Cell::new(0usize);
    let sampling = || {
        if crate::memory::logging() {
            calls.set(calls.get() + 1);
            if calls.get() <= 3 || calls.get().is_multiple_of(100) {
                crate::memory::log(&format!("cancellation point {}", calls.get()));
            }
        }
        cancelled()
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        analyze_package(engine, job, &sampling)
    })) {
        Ok(r) => r,
        Err(_) => {
            eprintln!(
                "[dartforge-lsp] the typed analysis of {} panicked; the syntactic diagnostics remain",
                job.root.display()
            );
            Vec::new()
        }
    }
}

/// Runs `job` in a child process: `exe` (the server's own binary) with
/// [`CHILD_ARGUMENT`] reads the job from its `stdin`, analyzes and writes the
/// results to its `stdout`, and exits.
///
/// The whole program of an analysis (on `new_sali`'s frontend, 3,316 libraries
/// and ~590 MiB live at the peak) lives and dies with the child: the server
/// process keeps only the documents and the published diagnostics, and its
/// resident memory does not stay at the analysis peak (the system allocator
/// keeps the freed pages of an in-process analysis; `docs/LSP.md`,
/// "Memória"). Cancelling kills the child, and a crash in the analysis (a panic,
/// a stack overflow) ends only the child.
///
/// `None` when the child cannot be used (it does not start, or the job does not
/// serialize): the caller analyzes in the thread. Otherwise the results, empty
/// when cancelled or when the child failed.
fn run_in_child(exe: &Path, job: &Arc<Job>, cancelled: &dyn Fn() -> bool) -> Option<Vec<Result>> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    // A path that is not UTF-8 does not go into JSON: such a job stays in the
    // thread.
    if job.root.to_str().is_none()
        || job
            .texts
            .keys()
            .chain(job.open.iter().map(|o| &o.2))
            .any(|p| p.to_str().is_none())
    {
        return None;
    }
    let mut child = Command::new(exe)
        .arg(CHILD_ARGUMENT)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .ok()?;
    let stdin = child.stdin.take()?;
    let stdout = child.stdout.take()?;
    // Writing and reading in their own threads, streamed (no copy of the job
    // or of the results as one buffer): neither side blocks on a full pipe, and
    // this thread stays free to notice a cancellation.
    let request = Arc::clone(job);
    let writer = std::thread::spawn(move || {
        let mut stdin = std::io::BufWriter::new(stdin);
        let _ = serde_json::to_writer(&mut stdin, &*request);
        let _ = stdin.flush();
    });
    let (sent, output) = mpsc::channel();
    std::thread::spawn(move || {
        let results: Option<Vec<Result>> =
            serde_json::from_reader(std::io::BufReader::new(stdout)).ok();
        let _ = sent.send(results);
    });
    let results = loop {
        match output.recv_timeout(Duration::from_millis(10)) {
            Ok(results) => break Some(results),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if cancelled() {
                    let _ = child.kill();
                    break None;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break None,
        }
    };
    let status = child.wait();
    let _ = writer.join();
    let Some(results) = results else {
        return Some(Vec::new());
    };
    if !status.is_ok_and(|s| s.success()) {
        eprintln!(
            "[dartforge-lsp] the typed analysis of {} failed in its process; the syntactic diagnostics remain",
            job.root.display()
        );
        return Some(Vec::new());
    }
    Some(results.unwrap_or_default())
}

/// The child analysis process: reads a job (JSON) from `stdin`, runs it and
/// writes the results (JSON) to `stdout`. Returns the exit code (0 when the
/// results were written, 2 for an unreadable job or a missing SDK).
///
/// The `dartforge-lsp` binary calls it when started with
/// [`CHILD_ARGUMENT`]; the server starts such children itself after
/// [`crate::Server::analyze_in_child_processes`].
///
/// ```no_run
/// if std::env::args().nth(1).as_deref() == Some(dartforge_lsp::TYPED_ANALYSIS_ARGUMENT) {
///     std::process::exit(dartforge_lsp::typed_analysis_child());
/// }
/// ```
pub fn typed_analysis_child() -> i32 {
    use std::io::{Read, Write};
    let mut input = Vec::new();
    if std::io::stdin().lock().read_to_end(&mut input).is_err() {
        return 2;
    }
    let Ok(job) = serde_json::from_slice::<Job>(&input) else {
        return 2;
    };
    drop(input);
    // Deep bodies recurse deeply: a dedicated stack, as in the worker thread.
    let run = std::thread::Builder::new()
        .name("dartforge-lsp-typed".into())
        .stack_size(1 << 30)
        .spawn(move || {
            let engine = match Engine::new(&job.sdk_lib) {
                Ok(engine) => engine,
                Err(error) => {
                    eprintln!("[dartforge-lsp] typed diagnostics disabled: {error}");
                    return 2;
                }
            };
            let results = analyze_package(&engine, &job, &|| false);
            crate::memory::log("after the analysis, in the child");
            let mut out = std::io::BufWriter::new(std::io::stdout().lock());
            if serde_json::to_writer(&mut out, &results).is_err() || out.flush().is_err() {
                return 1;
            }
            0
        });
    match run.map(std::thread::JoinHandle::join) {
        Ok(Ok(code)) => code,
        _ => 101,
    }
}

/// The packages of the workspace folders: that of each `.dart` in them.
fn folder_packages(folders: &[PathBuf]) -> Vec<PathBuf> {
    let mut v: BTreeSet<PathBuf> = BTreeSet::new();
    for p in folders {
        for a in dartforge_parity::corpus::dart_files(p) {
            v.insert(package_root(&a, folders));
        }
    }
    v.into_iter().collect()
}

/// The files the directives of `text` (the file at `path`) point to, by
/// [`key`], each with whether the directive is a `part` or a `part of`; and
/// the API signature of the text. `package:` URIs resolve through `packages`;
/// other schemes (`dart:`) have no file.
fn facts(path: &Path, text: &str, packages: Option<&dartforge_elements::PackageConfig>) -> Facts {
    let mut names = dartforge_intern::Interner::new();
    let parsed = dartforge_frontend::parser::parse(text, &mut names);
    let signature =
        dartforge_elements::incremental::api_signature_of(text, &parsed.ast, &parsed.unit);
    let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut targets = Vec::new();
    for d in &parsed.unit.directives {
        use dartforge_frontend::ast::DirectiveKind as K;
        let (lit, part) = match &d.kind {
            K::Import { uri, .. } | K::Export { uri, .. } => (Some(uri), false),
            K::Part { uri } => (Some(uri), true),
            K::PartOf { uri: Some(uri), .. } => (Some(uri), true),
            _ => (None, false),
        };
        let Some(uri) = lit.and_then(dartforge_elements::load::string_lit_value) else {
            continue;
        };
        let target = if let Some(rest) = uri.strip_prefix("package:") {
            let Some((package, rel)) = rest.split_once('/') else {
                continue;
            };
            let Some(dir) = packages.and_then(|c| {
                c.package_dirs
                    .iter()
                    .find(|(n, _)| n == package)
                    .map(|(_, d)| d.clone())
            }) else {
                continue;
            };
            dir.join(rel)
        } else if uri.contains(':') {
            continue;
        } else {
            base.join(&*uri)
        };
        targets.push((key(&target), part));
    }
    Facts {
        path: path.to_path_buf(),
        signature,
        targets,
    }
}

/// A `.dart` file to publish after an analysis: an open document (URI and
/// version) or a file of the workspace package read from disk.
#[derive(Debug)]
struct Target {
    key: PathBuf,
    path: PathBuf,
    open: Option<(String, i32)>,
}

/// What an analysis of a package does, decided by [`plan`].
#[derive(Debug)]
struct Plan {
    /// The open documents to analyze.
    open: Vec<(String, i32, PathBuf, Arc<str>)>,
    /// The files not open to analyze.
    others: Vec<PathBuf>,
    /// Every `.dart` file to publish, in order: the analyzed ones get the fresh
    /// result, the others what the [`Memo`] has.
    order: Vec<Target>,
    /// The open documents in excluded folders: published empty.
    excluded: Vec<(String, i32)>,
}

/// Decides what to analyze and what to publish (Dart's driver republishes the
/// changed files and those that depend on them; §3.2, I7).
///
/// To publish: the open documents of the package and, with the package in a
/// workspace folder, every `.dart` that `dart analyze` would see on the first
/// analysis (or without a memo), afterwards the changed files and those that
/// depend on them through the directives (`import`, `export`, `part`,
/// `part of`), transitively.
///
/// To analyze, with a memo: a changed file whose API signature is not the one
/// it had (or that the memo does not know) and everything that depends on it,
/// transitively; a changed file with the same signature, only its library —
/// the file with its parts and its owner (the `part`/`part of` links, both
/// ways), whose diagnostics may depend on a body (`unused_element`); and any
/// file to publish that the memo does not have. The rest is published with what
/// the memo has: a file's diagnostics depend only on its library's text and on
/// the declarations it sees, which a body edit elsewhere does not change.
fn plan(
    memo: Option<&Memo>,
    root: &Path,
    open: Vec<(String, i32, PathBuf, Arc<str>)>,
    texts: &HashMap<PathBuf, Arc<str>>,
    config: &AnalysisConfiguration,
    in_workspace: bool,
    changed: &BTreeSet<PathBuf>,
) -> Plan {
    // Documents in excluded folders are not analyzed: their publication is empty.
    let excluded_ = |c: &Path| config.excluded.iter().any(|e| c.starts_with(e));
    let (excluded, open): (Vec<_>, Vec<_>) = open.into_iter().partition(|o| excluded_(&o.2));
    let excluded = excluded.into_iter().map(|o| (o.0, o.1)).collect();
    // The package's `.dart` files that `dart analyze` would see (outside the
    // exclusions of `analysis_options.yaml` and of `analysisExcludedFolders`).
    let of_package: Vec<PathBuf> = if in_workspace {
        let options = dartforge_parity::filters::AnalysisOptions::read(root);
        dartforge_parity::projects::files(root, &options)
            .into_iter()
            .filter(|a| !excluded_(a))
            .collect()
    } else {
        Vec::new()
    };
    let open_keys: BTreeSet<PathBuf> = open.iter().map(|o| key(&o.2)).collect();
    let mut order: Vec<Target> = open
        .iter()
        .map(|(u, v, c, _)| Target {
            key: key(c),
            path: c.clone(),
            open: Some((u.clone(), *v)),
        })
        .collect();
    let disk: Vec<Target> = of_package
        .iter()
        .map(|a| Target {
            key: key(a),
            path: a.clone(),
            open: None,
        })
        .filter(|t| !open_keys.contains(&t.key))
        .collect();
    let Some(memo) = memo else {
        let others = disk.iter().map(|t| t.path.clone()).collect();
        order.extend(disk);
        return Plan {
            open,
            others,
            order,
            excluded,
        };
    };
    // The changed files as they are now: their signatures and directives.
    let package_config = root.join(".dart_tool").join("package_config.json");
    let packages = package_config
        .is_file()
        .then(|| dartforge_elements::PackageConfig::load(&package_config).ok())
        .flatten();
    let path_of = |k: &PathBuf| -> PathBuf {
        open.iter()
            .map(|o| &o.2)
            .chain(of_package.iter())
            .find(|c| key(c) == *k)
            .cloned()
            .unwrap_or_else(|| k.clone())
    };
    let mut now: HashMap<PathBuf, Option<Facts>> = HashMap::new();
    for k in changed {
        if k.extension().is_none_or(|e| e != "dart") {
            continue;
        }
        let path = path_of(k);
        let text = texts
            .get(k)
            .map(|t| t.to_string())
            .or_else(|| std::fs::read_to_string(&path).ok());
        now.insert(k.clone(), text.map(|t| facts(&path, &t, packages.as_ref())));
    }
    // The directive graph, from the memo with the changed files' links updated:
    // who points at each file, and the `part`/`part of` links both ways.
    let mut pointing: HashMap<&Path, Vec<&Path>> = HashMap::new();
    let mut parts: HashMap<&Path, Vec<&Path>> = HashMap::new();
    let links = memo
        .files
        .iter()
        .filter(|(k, _)| !now.contains_key(*k))
        .map(|(k, r)| (k, &r.facts.targets))
        .chain(
            now.iter()
                .filter_map(|(k, f)| f.as_ref().map(|f| (k, &f.targets))),
        );
    for (k, targets) in links {
        for (t, part) in targets {
            pointing.entry(t).or_default().push(k);
            if *part {
                parts.entry(t).or_default().push(k);
                parts.entry(k).or_default().push(t);
            }
        }
    }
    let closure = |from: Vec<&Path>, graph: &HashMap<&Path, Vec<&Path>>| {
        let mut output: BTreeSet<PathBuf> = from.iter().map(|p| p.to_path_buf()).collect();
        let mut stack = from;
        while let Some(x) = stack.pop() {
            for d in graph.get(x).into_iter().flatten() {
                if output.insert(d.to_path_buf()) {
                    stack.push(d);
                }
            }
        }
        output
    };
    // To publish: the open documents and the files affected by the changes.
    let affected = closure(changed.iter().map(PathBuf::as_path).collect(), &pointing);
    order.extend(disk.into_iter().filter(|t| affected.contains(&t.key)));
    // To analyze.
    let same_api = |k: &PathBuf| {
        let before = memo.files.get(k).and_then(|r| r.facts.signature);
        let after = now
            .get(k)
            .and_then(|f| f.as_ref().and_then(|f| f.signature));
        before.is_some() && before == after
    };
    let (body, api): (Vec<&PathBuf>, Vec<&PathBuf>) = changed
        .iter()
        .filter(|k| k.extension().is_some_and(|e| e == "dart"))
        .partition(|k| same_api(k));
    let mut fresh = closure(api.into_iter().map(PathBuf::as_path).collect(), &pointing);
    fresh.extend(closure(
        body.into_iter().map(PathBuf::as_path).collect(),
        &parts,
    ));
    fresh.extend(
        order
            .iter()
            .filter(|t| !memo.files.contains_key(&t.key))
            .map(|t| t.key.clone()),
    );
    let others = order
        .iter()
        .filter(|t| t.open.is_none() && fresh.contains(&t.key))
        .map(|t| t.path.clone())
        .collect();
    let open = open
        .into_iter()
        .filter(|o| fresh.contains(&key(&o.2)))
        .collect();
    Plan {
        open,
        others,
        order,
        excluded,
    }
}

/// The results to publish after an analysis: those of `fresh`, which also
/// update the `memo`, and, for each file of `order` that was not analyzed, what
/// the memo has (a file not open with diagnostics goes with its disk text, to
/// place them). In `order`, then the YAML files, then the open documents in
/// excluded folders.
fn merge(
    memo: &mut Memo,
    fresh: Vec<Result>,
    order: Vec<Target>,
    excluded: Vec<(String, i32)>,
) -> Vec<Result> {
    let mut analyzed: HashMap<PathBuf, Result> = HashMap::new();
    let mut tail = Vec::new();
    for r in fresh {
        match &r.facts {
            Some(f) => {
                let k = key(&f.path);
                memo.files.insert(
                    k.clone(),
                    Remembered {
                        facts: f.clone(),
                        diagnostics: r.diagnostics.clone(),
                    },
                );
                analyzed.insert(k, r);
            }
            None => tail.push(r),
        }
    }
    let mut output = Vec::with_capacity(order.len() + tail.len());
    for t in order {
        if let Some(r) = analyzed.remove(&t.key) {
            output.push(r);
            continue;
        }
        let Some(m) = memo.files.get(&t.key) else {
            continue;
        };
        let diagnostics = m.diagnostics.clone();
        let r = match t.open {
            Some((uri, version)) => Result {
                uri,
                version: Some(version),
                text: None,
                diagnostics,
                yaml: Vec::new(),
                facts: None,
            },
            None => {
                let Ok(u) = url::Url::from_file_path(&t.path) else {
                    continue;
                };
                let text = if diagnostics.is_empty() {
                    None
                } else {
                    match std::fs::read_to_string(&t.path) {
                        Ok(text) => Some(text),
                        Err(_) => continue,
                    }
                };
                Result {
                    uri: u.to_string(),
                    version: None,
                    text,
                    diagnostics,
                    yaml: Vec::new(),
                    facts: None,
                }
            }
        };
        output.push(r);
    }
    // An analyzed file outside the order (none is expected) still goes out.
    output.extend(analyzed.into_values());
    output.extend(tail);
    output.extend(excluded.into_iter().map(|(uri, version)| Result {
        uri,
        version: Some(version),
        text: None,
        diagnostics: Vec::new(),
        yaml: Vec::new(),
        facts: None,
    }));
    output
}

/// Analyzes the files of `job` (its open documents and the other files of the
/// package that [`plan`] chose) in a single program of package `root`, and
/// returns their publications with their [`Facts`], plus the package's YAML
/// files when they are due. Empty when cancelled.
fn analyze_package(engine: &Engine, job: &Job, cancelled: &dyn Fn() -> bool) -> Vec<Result> {
    let Job {
        root,
        open,
        others,
        texts,
        config,
        in_workspace,
        all,
        changed,
        ..
    } = job;
    let (root, in_workspace, all) = (root.as_path(), *in_workspace, *all);
    // Documents in excluded folders are not analyzed: their publication is empty.
    let excluded = |c: &Path| config.excluded.iter().any(|e| c.starts_with(e));
    let mut excluded_output: Vec<Result> = open
        .iter()
        .filter(|(_, _, c, _)| excluded(c))
        .map(|(u, v, _, _)| Result {
            uri: u.clone(),
            version: Some(*v),
            text: None,
            diagnostics: Vec::new(),
            yaml: Vec::new(),
            facts: None,
        })
        .collect();
    let open: Vec<(String, i32, PathBuf, Arc<str>)> = open
        .iter()
        .filter(|(_, _, c, _)| !excluded(c))
        .cloned()
        .collect();
    let open = &open[..];
    let options = dartforge_parity::filters::AnalysisOptions::read(root);
    if open.is_empty() && others.is_empty() {
        return excluded_output;
    }
    let mut files: Vec<PathBuf> = others.clone();
    for (_, _, path, text) in open {
        // A part comes in through its owning library (the engine does not import parts).
        if is_part(text)
            && let Some(owning) = crate::semantic::owning_library(path, text, None)
        {
            files.push(owning);
        }
        files.push(path.clone());
    }
    files.sort_by_key(|a| key(a));
    files.dedup_by_key(|a| key(a));
    let package_config = root.join(".dart_tool").join("package_config.json");
    let package_config = package_config.is_file().then_some(package_config);
    let packages = package_config
        .as_deref()
        .and_then(|c| dartforge_elements::PackageConfig::load(c).ok());
    let texts: HashMap<PathBuf, String> = texts
        .iter()
        .map(|(k, t)| (k.clone(), t.to_string()))
        .collect();
    let Some(analysis) =
        engine.analyze_with(root, &files, package_config.as_deref(), &texts, cancelled)
    else {
        return Vec::new();
    };
    crate::memory::log(&format!(
        "after the engine ({} files kept)",
        analysis.files.len()
    ));
    let mut syntactic = SyntacticAnalyzer::new();
    let mut output = Vec::new();
    let mut targets: Vec<(String, Option<i32>, PathBuf, &str)> = open
        .iter()
        .map(|(u, v, c, t)| (u.clone(), Some(*v), c.clone(), &**t))
        .collect();
    for a in others {
        let Some(file_) = analysis.files.get(&key(a)) else {
            continue;
        };
        let Ok(u) = url::Url::from_file_path(a) else {
            continue;
        };
        targets.push((u.to_string(), None, a.clone(), file_.text.as_str()));
    }
    for (uri, version, path, text) in &targets {
        let Some(file) = analysis.files.get(&key(path)) else {
            continue;
        };
        if dartforge_parity::oracle::relative(path, root)
            .is_some_and(|rel| options.is_excluded(&rel))
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
        let published: Vec<Diagnostic> = dartforge_parity::publishable(file, &options, true)
            .into_iter()
            .filter(|(_, syntax)| !syntax)
            .map(|(d, _)| d)
            .collect();
        let mut semantic: Vec<Diagnostic> = Vec::new();
        for (d, syntax) in dartforge_parity::publishable(file, &options, false) {
            if syntax {
                continue;
            }
            let published_ = published
                .iter()
                .any(|x| x.span == d.span && x.code == d.code && x.message == d.message);
            let todo = d.code.is_some_and(|c| {
                c.info().error_type == dartforge_diagnostics::ErrorType::Todo
                    && (d.severity != dartforge_diagnostics::Severity::Info
                        || config.all
                        || config
                            .all_types
                            .iter()
                            .any(|t| *t == c.info().name.to_uppercase()))
            });
            if (published_ || todo)
                && !semantic.iter().any(|x: &Diagnostic| {
                    x.span == d.span && x.code == d.code && x.message == d.message
                })
            {
                semantic.push(d);
            }
        }
        // A published one that the unfiltered list does not bring (the
        // `IgnoreValidator` reports separately) goes at the end, as in the analyzer.
        for d in published {
            if !semantic
                .iter()
                .any(|x| x.span == d.span && x.code == d.code && x.message == d.message)
            {
                semantic.push(d);
            }
        }
        // The order is that of the `LibraryAnalyzer` (the phases, §1.2 of INFRA):
        // the Dart server sends the list in insertion order (§3.6).
        diagnostics.extend(semantic);
        output.push(Result {
            uri: uri.clone(),
            version: *version,
            // A file that is not open needs its text only to place its
            // diagnostics: an empty list goes without it.
            text: (version.is_none() && !diagnostics.is_empty()).then(|| text.to_string()),
            diagnostics,
            yaml: Vec::new(),
            facts: Some(facts(path, text, packages.as_ref())),
        });
    }
    // The package's YAML files: on the first analysis (and after the
    // configuration changes) and when one of them changed on disk. The text is
    // the disk's, as Dart's watcher reads it.
    if in_workspace {
        let options_yaml = root.join("analysis_options.yaml");
        let pubspec_yaml = root.join("pubspec.yaml");
        let changed_ = |p: &Path| all || changed.contains(&key(p));
        let mut yaml: Vec<(PathBuf, Vec<dartforge_parity::json::DiagJson>)> = Vec::new();
        if options_yaml.is_file() && changed_(&options_yaml) {
            yaml.push((
                options_yaml.clone(),
                dartforge_parity::options_diagnostics(root),
            ));
        }
        if pubspec_yaml.is_file() && (changed_(&pubspec_yaml) || changed_(&options_yaml)) {
            yaml.push((
                pubspec_yaml.clone(),
                dartforge_parity::pubspec_diagnostics(root, &options),
            ));
        }
        for (path, diagnostics) in yaml {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(u) = url::Url::from_file_path(&path) else {
                continue;
            };
            output.push(Result {
                uri: u.to_string(),
                version: None,
                text: Some(text),
                diagnostics: Vec::new(),
                yaml: diagnostics,
                facts: None,
            });
        }
    }
    output.append(&mut excluded_output);
    output
}

/// The SDK `lib/` of the analyzer, when it diagnoses with types.
pub(crate) fn analyzer_sdk<A: Analyzer>(analyzer: &A) -> Option<PathBuf> {
    if std::env::var("DARTFORGE_LSP_TYPED").is_ok_and(|v| v == "0") {
        return None;
    }
    analyzer.sdk_for_diagnostics()
}
