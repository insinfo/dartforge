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
//! * **One program per package.** The files of the package that an analysis
//!   covers (plus the owning library of each open part) go into a single
//!   `Engine::analyze_with`, with the open texts winning over the disk.
//! * **Incremental.** [`plan`] chooses them from what the previous analyses
//!   left ([`Memo`]): an edit that keeps the file's API signature reanalyzes
//!   only its library; one that changes it, the files that depend on it
//!   through the directives, transitively. The other files to publish get
//!   their previous diagnostics, the same a fresh analysis gives
//!   (`tests/typed_incremental.rs`).
//! * **Version.** Each result carries the version of the analyzed text; the
//!   server only publishes it if it is still the current one. A result of
//!   an old version is discarded, never overwrites that of a new one.
//! * **Fast edits.** A package's request is a set (marking it again does not
//!   enqueue another analysis): a burst of edits becomes one analysis. The
//!   running analysis checks, between the engine's phases, whether the
//!   package was marked again; if so, it stops and restarts with the new
//!   text.
//! * **Memory.** The worker retains only the [`Engine`] (SDK layout and
//!   library names), the current text of each open document (shared with
//!   the jobs, not copied) and the [`Memo`]s (diagnostics and directive
//!   targets). Program, trees and type table never live in the server: with
//!   its executable (what the binary does) the analyses run in child
//!   processes — the small ones in a resident child that keeps the trees
//!   between analyses, within a budget and an idle time ([`ResidentChild`]),
//!   the large ones in a child of their own ([`run_in_child`]). The allocator
//!   of a long-lived process keeps the pages of an analysis that dropped, so
//!   the server would otherwise stay at the peak of the largest package
//!   analyzed (`docs/LSP.md`, "Memória" and "Edição incremental").
//!
//! [`Engine`]: dartforge_parity::analysis::Engine

use crate::{Analyzer, SyntacticAnalyzer};
use dartforge_diagnostics::Diagnostic;
use dartforge_parity::analysis::{Engine, is_part, key};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

/// Result of an analysis for one file: an open document (with the version
/// of the analyzed text) or a file of the roots that is not open (without a
/// version, with the disk text for the ranges).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    /// The API declaration by declaration
    /// ([`dartforge_elements::incremental::api_digest`]): which names an API
    /// change changed ([`api_affected`]).
    #[serde(default)]
    pub api: Option<dartforge_elements::incremental::ApiDigest>,
    /// The words of the text
    /// ([`dartforge_elements::incremental::text_words`]): the names it can
    /// refer to.
    #[serde(default)]
    pub words: Vec<u64>,
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
    /// Packages whose open documents the resident process should load for the
    /// queries ([`crate::remote`]) when no resident runs: after the first
    /// analysis of a package (which runs in a process of its own), or when a
    /// query found none.
    /// A true value also refreshes an existing resident whose current program
    /// no longer includes a query's document after a selective analysis.
    warm: std::collections::BTreeMap<PathBuf, bool>,
    /// Package being analyzed now.
    running: Option<PathBuf>,
    /// Packages of `dirty` that only `.dart` files changed on the disk made
    /// dirty: their running analysis goes on (it stays valid for what it
    /// read, and the next one sees the rest) instead of being cancelled. A
    /// cancelled load over the kept program loses it, and the next analysis
    /// loads the whole program again (a wave whose files come with gaps
    /// started an analysis that the rest of the wave cancelled).
    soft: BTreeSet<PathBuf>,
    /// Test hook: no analysis starts while true.
    paused: bool,
    shutdown: bool,
    /// A request for the resident process's memory measure
    /// ([`Typed::resident_memory`]): the worker answers it between analyses.
    memory_request: Option<mpsc::Sender<Option<ResidentMemory>>>,
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
    /// Analyses started so far (measurement and tests).
    analyses: usize,
    /// When the last file changed on the disk, while no editor change came
    /// after it: a wave of generated files (`build_runner`, `webdev`) arrives
    /// as many notifications, and one analysis follows the wave
    /// ([`DISK_QUIET`]).
    disk_at: Option<Instant>,
}

impl State {
    /// Keeps a required refresh when an ordinary warm request is coalesced.
    fn request_warm(&mut self, root: PathBuf, refresh: bool) {
        self.warm
            .entry(root)
            .and_modify(|required| *required |= refresh)
            .or_insert(refresh);
    }

    /// Uses the worker's existing root spelling. Queries canonicalize file
    /// URIs, while didOpen may retain a symlink or lexical directory alias.
    /// Ambiguous aliases keep the existing local fallback.
    fn warm_root(&self, file: &Path) -> Option<PathBuf> {
        let root = package_root(file, &self.folders);
        if self
            .documents
            .values()
            .any(|document| document.root == root)
        {
            return Some(root);
        }
        let canonical = dartforge_elements::config::canonicalize(&root).ok()?;
        let mut found = None;
        for document in self.documents.values() {
            if dartforge_elements::config::canonicalize(&document.root)
                .is_ok_and(|path| path == canonical)
            {
                if found.as_ref().is_some_and(|path| path != &document.root) {
                    return None;
                }
                found = Some(document.root.clone());
            }
        }
        found
    }
}

/// How long the disk must stay quiet after a change, without an editor change
/// in between, before the analysis of what changed starts: a watcher reports
/// the files of a wave a few milliseconds apart, and a later change cancels an
/// analysis that started too soon. It was 250 ms; on `new_sali` a wave of 40
/// `.g.dart` then took ~460 ms from its last notification against Dart's
/// ~100-370 ms, and with 20 ms (and the cheaper analysis) ~135 ms.
/// `DARTFORGE_LSP_DISK_QUIET_MS` changes it (`0`: no wait, to measure).
const DISK_QUIET: Duration = Duration::from_millis(20);

fn disk_quiet() -> Duration {
    std::env::var("DARTFORGE_LSP_DISK_QUIET_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map_or(DISK_QUIET, Duration::from_millis)
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

/// The reference index records the resident process makes of the edited
/// files after an analysis (docs/LSP.md, "Décima sexta rodada"): the worker
/// asks for them while the server has an index, and the semantic analyzer
/// takes them (waiting for one of the current text) before a search, instead
/// of inferring the edited library again in the server.
///
/// It is also the server's link to the resident for the library queries the
/// resident answers ([`crate::remote`]).
#[doc(hidden)]
#[derive(Default)]
pub struct ResidentRecords {
    state: Mutex<RecordState>,
    changed: Condvar,
    queries: Mutex<QueryState>,
    answered: Condvar,
    /// The worker's state, to ask it for a resident with the open documents.
    worker: std::sync::OnceLock<std::sync::Weak<(Mutex<State>, Condvar)>>,
}

/// The queries sent to the resident process and their answers.
#[derive(Default)]
struct QueryState {
    /// The resident's input while it runs (the worker writes its analyses
    /// to the same pipe).
    input: Option<Arc<Mutex<std::io::BufWriter<std::process::ChildStdin>>>>,
    /// A new number for each resident process started.
    generation: u64,
    next: u64,
    /// The queries sent and not answered yet, and the answers not taken.
    waiting: std::collections::HashSet<u64>,
    answers: HashMap<u64, crate::remote::Answer>,
    /// When the last query was answered: the resident does not end idle
    /// while it answers queries.
    used: Option<Instant>,
}

#[derive(Default)]
pub(crate) struct RecordState {
    /// The project root of the server's reference index; `None`: no records
    /// are wanted.
    pub(crate) wanted: Option<PathBuf>,
    /// The files ([`key`]s) whose records are being made, with the stamp
    /// ([`crate::reference_index::text_stamp`]) of the text they come from
    /// and the id of the resident's analysis that makes them.
    pub(crate) pending: HashMap<PathBuf, (u64, u64)>,
    /// The encoded indexes and their complete open-document context.
    pub(crate) ready: Vec<RecordBatch>,
    pub(crate) contexts: HashMap<u64, RecordContext>,
    epoch: u64,
    next: u64,
}

impl RecordState {
    fn next_id(&mut self) -> u64 {
        self.next = self.next.wrapping_add(1);
        self.next
    }

    fn invalidate(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.pending.clear();
        self.contexts.clear();
        self.ready.clear();
    }
}

/// A record's own text does not identify the API its imports exposed when
/// it was resolved. Keep all open overlays, including other packages.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct RecordContext {
    sdk_lib: PathBuf,
    texts: BTreeMap<PathBuf, u64>,
}

impl RecordContext {
    pub(crate) fn current(sdk_lib: &Path, documents: &crate::DocumentStore) -> Option<Self> {
        let mut texts = BTreeMap::new();
        for uri in documents.uris() {
            let Some(path) = crate::project::uri_file(uri)
                .filter(|p| p.extension().is_some_and(|e| e == "dart"))
            else {
                continue;
            };
            let stamp = crate::reference_index::text_stamp(documents.get(uri)?);
            if texts
                .insert(key(&path), stamp)
                .is_some_and(|old| old != stamp)
            {
                // Two URIs expose conflicting overlays of the same file.
                return None;
            }
        }
        Some(Self {
            sdk_lib: key(sdk_lib),
            texts,
        })
    }
}

pub(crate) struct RecordBatch {
    pub(crate) bytes: Vec<u8>,
    pub(crate) context: RecordContext,
}

impl ResidentRecords {
    pub(crate) fn lock(&self) -> std::sync::MutexGuard<'_, RecordState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Waits until the records change or `limit` passes.
    pub(crate) fn wait<'a>(
        &self,
        state: std::sync::MutexGuard<'a, RecordState>,
        limit: Duration,
    ) -> std::sync::MutexGuard<'a, RecordState> {
        self.changed
            .wait_timeout(state, limit)
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .0
    }

    /// The server's index covers the project at `root` (`None`: it has none).
    pub(crate) fn want(&self, root: Option<PathBuf>) {
        let mut state = self.lock();
        if state.wanted != root {
            state.wanted = root;
            state.invalidate();
            self.changed.notify_all();
        }
    }

    /// A disk or configuration change invalidates the context of records
    /// already queued and jobs selected before the change.
    pub(crate) fn invalidate(&self) {
        self.lock().invalidate();
        self.changed.notify_all();
    }

    /// The edited documents of an analysis of the package at `root` whose
    /// records are wanted: those open, changed since the last analysis and
    /// inside the index's project.
    fn wanted_for(
        &self,
        root: &Path,
        open: &[(String, i32, PathBuf, Arc<str>)],
        changed: &BTreeSet<PathBuf>,
    ) -> (Vec<PathBuf>, u64) {
        let state = self.lock();
        let Some(project) = &state.wanted else {
            return (Vec::new(), state.epoch);
        };
        if !(root.starts_with(project) || project.starts_with(root)) {
            return (Vec::new(), state.epoch);
        }
        let paths = open
            .iter()
            .map(|o| key(&o.2))
            .filter(|k| changed.contains(k) && k.starts_with(project))
            .collect();
        (paths, state.epoch)
    }

    /// The resident's analysis `id` makes the records of `job`.
    fn started(&self, job: &Job) -> u64 {
        let mut state = self.lock();
        // The sequence spans resident restarts: a late old reader must not
        // attach its records to a new job with the same process-local id.
        let id = state.next_id();
        if job.records.is_empty() || job.records_epoch != state.epoch || state.wanted.is_none() {
            return id;
        }
        state.contexts.insert(
            id,
            RecordContext {
                sdk_lib: key(&job.sdk_lib),
                texts: job
                    .texts
                    .iter()
                    .map(|(p, t)| (p.clone(), crate::reference_index::text_stamp(t)))
                    .collect(),
            },
        );
        for (_, _, path, text) in &job.open {
            let k = key(path);
            if job.records.contains(&k) {
                state
                    .pending
                    .insert(k, (crate::reference_index::text_stamp(text), id));
            }
        }
        id
    }

    /// The records of the resident's analysis `id` will not come.
    fn abandoned(&self, id: u64) {
        let mut state = self.lock();
        state.pending.retain(|_, (_, i)| *i != id);
        state.contexts.remove(&id);
        self.changed.notify_all();
    }

    /// The resident sent the records of its analysis `id` (hexadecimal, empty
    /// when none were made).
    fn arrived(&self, id: u64, hex: &str) {
        let mut state = self.lock();
        state.pending.retain(|_, (_, i)| *i != id);
        if let Some(context) = state.contexts.remove(&id)
            && let Some(bytes) = from_hex(hex)
            && !bytes.is_empty()
        {
            // Unconsumed records have not changed the index. It can rebuild
            // any omitted units from their current texts, so keep only one
            // batch instead of accumulating an editing history until a query.
            state.ready.clear();
            state.ready.push(RecordBatch { bytes, context });
        }
        self.changed.notify_all();
    }

    /// The resident process is gone: nothing it was making will come.
    fn gone(&self) {
        let mut state = self.lock();
        state.pending.clear();
        state.contexts.clear();
        self.changed.notify_all();
        drop(state);
        self.detach();
    }

    fn queries(&self) -> std::sync::MutexGuard<'_, QueryState> {
        self.queries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// A resident process started with this input.
    fn attach(&self, input: Arc<Mutex<std::io::BufWriter<std::process::ChildStdin>>>) {
        let mut q = self.queries();
        q.input = Some(input);
        q.generation += 1;
        self.answered.notify_all();
    }

    /// The resident process ended: the queries waiting for it get no answer.
    fn detach(&self) {
        let mut q = self.queries();
        if q.input.take().is_some() {
            q.generation += 1;
        }
        self.answered.notify_all();
    }

    /// The resident answered query `id`.
    fn answer(&self, id: u64, answer: crate::remote::Answer) {
        let mut q = self.queries();
        if q.waiting.contains(&id) {
            q.answers.insert(id, answer);
            q.used = Some(Instant::now());
            self.answered.notify_all();
        }
    }

    /// When the resident last answered a query.
    fn last_query(&self) -> Option<Instant> {
        self.queries().used
    }

    /// Asks the worker for a resident process with the open documents of the
    /// package of `file` (it starts one when none runs); false when none can
    /// come (the analyses run in this process, the resident is off, the worker
    /// is paused or nothing of that package is open).
    fn warm(&self, file: &Path, refresh: bool) -> bool {
        let Some(worker) = self.worker.get().and_then(std::sync::Weak::upgrade) else {
            return false;
        };
        let mut e = worker
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if e.process.is_none() || e.paused || e.shutdown || resident_budget() == 0 {
            return false;
        }
        let Some(root) = e.warm_root(file) else {
            return false;
        };
        e.request_warm(root, refresh);
        worker.1.notify_all();
        true
    }

    /// Sends `request` (a query on `file`) to the resident process and waits
    /// for its answer, up to `limit`; `None` when there is no resident in time
    /// (the worker is asked for one), it ended, or it did not answer in time.
    pub(crate) fn query(
        &self,
        file: &Path,
        request: &crate::remote::QueryRequest,
        limit: Duration,
    ) -> Option<crate::remote::Answer> {
        use std::io::Write;
        let deadline = Instant::now() + limit;
        let mut asked = false;
        let (input, generation, id) = loop {
            let mut q = self.queries();
            if let Some(input) = q.input.clone() {
                q.next += 1;
                let id = q.next;
                q.waiting.insert(id);
                break (input, q.generation, id);
            }
            if !asked {
                asked = true;
                drop(q);
                if !self.warm(file, false) {
                    return None;
                }
                continue;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            drop(
                self.answered
                    .wait_timeout(q, left)
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
        };
        crate::memory::trace("query sent");
        let sent = {
            let mut w = input
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            serde_json::to_writer(&mut *w, &Sent::Query { id, query: request }).is_ok()
                && w.write_all(b"\n").is_ok()
                && w.flush().is_ok()
        };
        let mut q = self.queries();
        if !sent {
            q.waiting.remove(&id);
            return None;
        }
        loop {
            if let Some(a) = q.answers.remove(&id) {
                if matches!(a, crate::remote::Answer::NeedsAnalysis) {
                    // The resident keeps this id deferred. Ask the worker for
                    // all current open inputs without holding the query lock;
                    // its analysis will complete this same query and deadline.
                    drop(q);
                    let requested = self.warm(file, true);
                    q = self.queries();
                    if !requested {
                        q.waiting.remove(&id);
                        return None;
                    }
                    continue;
                }
                q.waiting.remove(&id);
                crate::memory::trace("query answered");
                return Some(a);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if q.generation != generation || left.is_zero() {
                q.waiting.remove(&id);
                return None;
            }
            q = self
                .answered
                .wait_timeout(q, left)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }
}

/// Bytes as lowercase hexadecimal (the records go on the resident's JSON lines).
pub(crate) fn to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(DIGITS[usize::from(b >> 4)] as char);
        s.push(DIGITS[usize::from(b & 15)] as char);
    }
    s
}

/// The bytes of [`to_hex`]'s text; `None` when it is not hexadecimal.
pub(crate) fn from_hex(text: &str) -> Option<Vec<u8>> {
    let digit = |c: u8| -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            _ => None,
        }
    };
    let b = text.as_bytes();
    if !b.len().is_multiple_of(2) {
        return None;
    }
    b.chunks(2)
        .map(|p| Some((digit(p[0])? << 4) | digit(p[1])?))
        .collect()
}

/// The server side: requests come in here, results go out through `receive`.
pub(crate) struct Typed {
    shared: Shared,
    results: mpsc::Receiver<Result>,
    records: Arc<ResidentRecords>,
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
        let records = Arc::new(ResidentRecords::default());
        let _ = records.worker.set(Arc::downgrade(&shared));
        let r = Arc::clone(&records);
        // Deep bodies recurse deeply: a dedicated stack, as in `analyze`.
        std::thread::Builder::new()
            .name("dartforge-lsp-typed".into())
            .stack_size(1 << 30)
            .spawn(move || work(&p, &sdk_lib, &tx, wake, &r))
            .ok()?;
        Some(Self {
            shared,
            results: rx,
            records,
        })
    }

    /// Where the resident's index records of the edited files arrive.
    pub(crate) fn records(&self) -> Arc<ResidentRecords> {
        Arc::clone(&self.records)
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
        e.disk_at = None;
        let root = package_root(&path, &e.folders);
        e.soft.remove(&root);
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
        for r in &roots {
            e.soft.remove(r);
        }
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
    /// package is analyzed and published again, the YAML files too). Whether
    /// an analysis follows (the server then reports itself analyzing, as
    /// Dart's does for what its watcher sees).
    pub(crate) fn file_on_disk(&self, path: &Path) -> bool {
        let mut e = self.state();
        let root = package_root(path, &e.folders);
        let inside = e.folders.iter().any(|p| path.starts_with(p))
            || e.documents.values().any(|o| o.root == root);
        if !inside {
            return false;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if matches!(
            name,
            "analysis_options.yaml" | "pubspec.yaml" | "package_config.json"
        ) {
            e.publish_all.insert(root.clone());
            e.stale.insert(root.clone());
            e.changed.entry(root.clone()).or_default().insert(key(path));
            e.soft.remove(&root);
            e.dirty.insert(root);
            e.disk_at = Some(Instant::now());
            self.shared.1.notify_all();
            return true;
        }
        if path.extension().is_none_or(|x| x != "dart") {
            return false;
        }
        // An open document counts through the editor.
        if e.documents.values().any(|d| key(&d.path) == key(path)) {
            return false;
        }
        e.changed.entry(root.clone()).or_default().insert(key(path));
        // Only the disk: the running analysis of the package is not cancelled.
        if !e.dirty.contains(&root) {
            e.soft.insert(root.clone());
        }
        e.dirty.insert(root);
        e.disk_at = Some(Instant::now());
        self.shared.1.notify_all();
        true
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
                e.soft.remove(&d.root);
                e.dirty.insert(d.root);
                self.shared.1.notify_all();
            }
        }
    }

    /// What the resident analysis process keeps, structure by structure
    /// ([`dartforge_parity::analysis::Resident::measure_memory`]), measured
    /// after the running analysis ends; `None` without a resident process, or
    /// when the answer does not come within `limit`. Measuring empties the
    /// resident: the next analysis starts from the files.
    pub(crate) fn resident_memory(&self, limit: Duration) -> Option<ResidentMemory> {
        let (tx, rx) = mpsc::channel();
        self.state().memory_request = Some(tx);
        self.shared.1.notify_all();
        rx.recv_timeout(limit).ok().flatten()
    }

    /// Files given to the analyses so far (a cancelled analysis counts too).
    pub(crate) fn analyses(&self) -> usize {
        self.state().analyses
    }

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
fn work(
    p: &Shared,
    sdk_lib: &Path,
    tx: &mpsc::Sender<Result>,
    wake: Option<Wake>,
    records: &Arc<ResidentRecords>,
) {
    let (lock, sinal) = &**p;
    let block = || {
        lock.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    };
    let mut engine: Option<Engine> = None;
    // What the completed analyses left, per package root (only in this thread).
    let mut memos: HashMap<PathBuf, Memo> = HashMap::new();
    // Per package root, its `.dart` files as `dart analyze` sees them, and when
    // they were listed ([`package_files`]).
    let mut listed: HashMap<PathBuf, (Instant, Vec<PathBuf>)> = HashMap::new();
    // The resident analysis process, while there is work for it.
    let mut resident: Option<ResidentChild> = None;
    let budget = resident_budget();
    let idle = resident_idle();
    let batch_limit = resident_batch();
    loop {
        let (root, open, texts, config, changed, process) = {
            let mut e = block();
            while !e.shutdown
                && e.memory_request.is_none()
                && ((e.dirty.is_empty() && !e.scan && e.warm.is_empty()) || e.paused)
            {
                let Some(r) = &resident else {
                    e = sinal
                        .wait(e)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    continue;
                };
                // The queries it answers count as use.
                let used = records.last_query().map_or(r.used, |q| q.max(r.used));
                let left = idle.saturating_sub(used.elapsed());
                if left.is_zero() {
                    // Idle for long: its memory goes back to the system.
                    resident = None;
                    continue;
                }
                e = sinal
                    .wait_timeout(e, left)
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .0;
            }
            if e.shutdown {
                return;
            }
            if let Some(answer) = e.memory_request.take() {
                drop(e);
                let _ = answer.send(resident.as_mut().and_then(ResidentChild::measure));
                continue;
            }
            // A wave of files changing on the disk: one analysis after it.
            if let Some(at) = e.disk_at {
                let left = disk_quiet().saturating_sub(at.elapsed());
                if !left.is_zero() {
                    let _ = sinal
                        .wait_timeout(e, left)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    continue;
                }
                e.disk_at = None;
            }
            if e.dirty.is_empty()
                && !e.scan
                && let Some((root, refresh)) = e.warm.pop_first()
            {
                // A resident with the open documents of `root`, for the queries.
                let open: Vec<(String, i32, PathBuf, Arc<str>)> = e
                    .documents
                    .iter()
                    .filter(|(_, d)| d.root == root)
                    .map(|(u, d)| (u.clone(), d.version, d.path.clone(), d.text.clone()))
                    .collect();
                let process = e.process.clone();
                if open.is_empty()
                    || (resident.is_some() && !refresh)
                    || process.is_none()
                    || budget == 0
                {
                    continue;
                }
                let texts: HashMap<PathBuf, Arc<str>> = e
                    .documents
                    .values()
                    .map(|d| (key(&d.path), d.text.clone()))
                    .collect();
                let config = AnalysisConfiguration {
                    all: e.all,
                    all_types: e.all_types.clone(),
                    excluded: e.excluded.clone(),
                };
                let in_workspace = e
                    .folders
                    .iter()
                    .any(|p| root.starts_with(p) || p.starts_with(&root));
                // Not an analysis the client sees: the server stays idle.
                drop(e);
                let package = if in_workspace {
                    package_files(&mut listed, &root, &BTreeSet::new())
                } else {
                    Vec::new()
                };
                let excluded = |c: &Path| config.excluded.iter().any(|x| c.starts_with(x));
                let job = Arc::new(Job {
                    sdk_lib: sdk_lib.to_path_buf(),
                    root: root.clone(),
                    open: open.into_iter().filter(|o| !excluded(&o.2)).collect(),
                    others: Vec::new(),
                    texts,
                    in_workspace,
                    all: false,
                    records: Vec::new(),
                    records_epoch: 0,
                    changed: BTreeSet::new(),
                    context: package.into_iter().filter(|a| !excluded(a)).collect(),
                    config,
                });
                let cancelled = || {
                    let e = block();
                    e.shutdown || (e.dirty.contains(&root) && !e.soft.contains(&root))
                };
                crate::memory::trace("worker warm");
                if let Some(exe) = process.as_deref() {
                    // What it publishes is what the last analysis published.
                    drop(run_in_resident(
                        &mut resident,
                        exe,
                        &job,
                        &cancelled,
                        records,
                    ));
                }
                continue;
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
                    for p in &packages {
                        e.soft.remove(p);
                    }
                    e.dirty.extend(packages);
                }
                sinal.notify_all();
                continue;
            }
            let root = e.dirty.pop_first().expect("non-empty");
            e.soft.remove(&root);
            crate::memory::trace("worker start");
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
            match lsp_engine(sdk_lib) {
                Ok(m) => engine = Some(m),
                Err(error) => {
                    eprintln!("[dartforge-lsp] typed diagnostics disabled: {error}");
                    let mut e = block();
                    e.running = None;
                    e.dirty.clear();
                    e.soft.clear();
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
            e.shutdown || (e.dirty.contains(&root) && !e.soft.contains(&root))
        };
        let (in_workspace, all, reset, changed) = changed;
        // Without a memo (the first analysis, or after the configuration changed)
        // every file is analyzed.
        if reset || all {
            memos.remove(&root);
            listed.remove(&root);
            // A full/configuration analysis may run in a disposable process.
            // Its old resident must not keep the previous package resolution
            // or trusted filesystem cache for a later query refresh.
            resident = None;
        }
        let of_package = if in_workspace {
            package_files(&mut listed, &root, &changed)
        } else {
            Vec::new()
        };
        let plan = plan(
            memos.get(&root),
            &root,
            open,
            &texts,
            &config,
            &of_package,
            &changed,
        );
        // The first analysis of a package runs in a process of its own: no
        // records come from it.
        let wanted = if all {
            (Vec::new(), 0)
        } else {
            records.wanted_for(&root, &plan.open, &changed)
        };
        let job = Arc::new(Job {
            sdk_lib: sdk_lib.to_path_buf(),
            root: root.clone(),
            open: plan.open,
            others: plan.others,
            texts,
            config,
            in_workspace,
            all,
            records: wanted.0,
            records_epoch: wanted.1,
            changed,
            context: plan.package,
        });
        let mut run = |job: &Arc<Job>| -> Vec<Result> {
            match process.as_deref().and_then(|exe| {
                // A large batch (the first analysis of a package, an API edit that
                // many files see) runs in a process of its own: its peak does not
                // stay with the resident process.
                let small = !job.all && job.open.len() + job.others.len() <= batch_limit;
                if budget > 0 && small {
                    run_in_resident(&mut resident, exe, job, &cancelled, records)
                } else {
                    None
                }
                .or_else(|| run_in_child(exe, job, &cancelled))
            }) {
                Some(results) => results,
                None => run_in_process(engine_ref, job, &cancelled),
            }
        };
        // The publications already sent by the first phase, by URI.
        let mut sent: HashMap<String, Option<i32>> = HashMap::new();
        let fresh = if job.open.is_empty() && job.others.is_empty() {
            // Nothing to analyze: every file to publish keeps what it had.
            Some(Vec::new())
        } else {
            {
                let mut e = block();
                e.analyzed_files += job.open.len() + job.others.len();
                e.analyses += 1;
            }
            // The edited documents first (Dart's priority files): when the batch
            // also has their dependents, their own library is analyzed and
            // published before the rest.
            let (first, rest) = match split(&job, &plan.priority) {
                Some((first, rest)) => (Some(Arc::new(first)), Arc::new(rest)),
                None => (None, Arc::clone(&job)),
            };
            let early = match first {
                Some(first) => {
                    crate::memory::trace("worker send first");
                    let results = run(&first);
                    crate::memory::trace("worker back first");
                    if !results.is_empty() {
                        for r in &results {
                            sent.insert(r.uri.clone(), r.version);
                            let _ = tx.send(r.clone());
                        }
                        if let Some(d) = &wake {
                            (d.0)();
                        }
                    }
                    Some(results)
                }
                None => None,
            };
            match early {
                // Cancelled or failed: so is the whole analysis.
                Some(early) if early.is_empty() => None,
                early => {
                    crate::memory::trace("worker send rest");
                    let results = run(&rest);
                    crate::memory::trace("worker back rest");
                    (!results.is_empty()).then(|| {
                        let mut all = early.unwrap_or_default();
                        all.extend(results);
                        all
                    })
                }
            }
        };
        let Some(fresh) = fresh else {
            let mut e = block();
            if e.dirty.contains(&root) && !e.soft.contains(&root) {
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
            drop(e);
            if let Some(d) = &wake {
                (d.0)();
            }
            continue;
        };
        let memo = memos.entry(root.clone()).or_default();
        let mut results = merge(memo, fresh, plan.order, plan.excluded);
        // What the first phase published is not sent again.
        results.retain(|r| sent.get(&r.uri) != Some(&r.version));
        for r in results {
            let _ = tx.send(r);
        }
        crate::memory::trace("worker done");
        crate::memory::log("after the analysis");
        {
            let mut e = block();
            e.running = None;
            // The first analysis of a package ran in a process of its own: the
            // resident loads the open documents for the queries it answers.
            if job.all && resident.is_none() && crate::remote::enabled() {
                e.request_warm(root.clone(), false);
            }
            sinal.notify_all();
        }
        // Even without results: the server reports the end of the analysis.
        if let Some(d) = &wake {
            (d.0)();
        }
    }
}

/// What in the client configuration changes the analysis.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
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
    /// The package's `.dart` files when it is in a workspace folder (empty
    /// otherwise): the resident process loads them all with every batch, so
    /// that the batches of the package share its kept program
    /// (`Resident::set_context`).
    #[serde(default)]
    context: Vec<PathBuf>,
    /// The edited files ([`key`]s) whose reference index records the resident
    /// process makes after answering ([`ResidentRecords`]); empty when the
    /// server has no index.
    #[serde(default)]
    records: Vec<PathBuf>,
    /// The parent-side record invalidation epoch when this job was selected.
    #[serde(default)]
    records_epoch: u64,
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
        analyze_package(engine, job, &sampling, None)
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
    if std::env::args().any(|a| a == RESIDENT_ARGUMENT) {
        return resident_child();
    }
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
            let engine = match lsp_engine(&job.sdk_lib) {
                Ok(engine) => engine,
                Err(error) => {
                    eprintln!("[dartforge-lsp] typed diagnostics disabled: {error}");
                    return 2;
                }
            };
            let results = analyze_package(&engine, &job, &|| false, None);
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

/// With [`CHILD_ARGUMENT`], makes the child analysis process resident
/// ([`resident_child`]).
const RESIDENT_ARGUMENT: &str = "--resident";

/// Default budget of the resident analysis process, in MiB of kept source
/// (`DARTFORGE_LSP_RESIDENT_MIB`; `0` turns it off and every analysis runs in
/// a new process). `new_sali`'s frontend keeps ~37 MiB.
const DEFAULT_RESIDENT_MIB: usize = 64;

/// Default idle time after which the resident process ends and gives its
/// memory back (`DARTFORGE_LSP_RESIDENT_IDLE_SECS`).
const DEFAULT_RESIDENT_IDLE_SECS: u64 = 300;

/// The default of the largest batch (files analyzed and published) the
/// resident process analyzes; a larger one runs in a process of its own
/// ([`run_in_child`]). `DARTFORGE_LSP_RESIDENT_BATCH` changes it. It was 32,
/// to keep the peak of a large batch out of the resident process; measured on
/// `new_sali/frontend` (an export of the barrel, 200 dependents), the resident
/// peak barely moves (446 → 465 MiB) and the total peak drops (791 → 472
/// MiB, no second process), while the dependents settle in half the time (the
/// cold process read and parsed the whole program again).
const RESIDENT_BATCH: usize = 1024;

/// The largest batch the resident process analyzes ([`RESIDENT_BATCH`]).
fn resident_batch() -> usize {
    std::env::var("DARTFORGE_LSP_RESIDENT_BATCH")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(RESIDENT_BATCH)
}

/// A message to the resident process: one JSON value per line.
#[derive(serde::Deserialize)]
enum Request {
    /// Analyze the job; the answer carries the same id.
    Analyze { id: u64, job: Box<Job> },
    /// Stop the analysis with this id at its next cancellation point.
    Cancel { id: u64 },
    /// Measure what the process keeps, structure by structure, emptying it;
    /// the answer carries the same id and [`Response::memory`].
    Measure { id: u64 },
    /// Answer a library query over the kept program ([`crate::remote`]); the
    /// answer carries the same id and [`Response::answer`].
    Query {
        id: u64,
        query: Box<crate::remote::QueryRequest>,
    },
}

/// [`Request`] as the worker writes it (the job borrowed, not copied).
#[derive(serde::Serialize)]
enum Sent<'a> {
    Analyze {
        id: u64,
        job: &'a Job,
    },
    Cancel {
        id: u64,
    },
    Measure {
        id: u64,
    },
    Query {
        id: u64,
        query: &'a crate::remote::QueryRequest,
    },
}

/// The answer of the resident process to [`Request::Analyze`]: the results, or
/// `None` when the analysis was cancelled; to [`Request::Measure`], the
/// measure.
#[derive(serde::Serialize, serde::Deserialize)]
struct Response {
    id: u64,
    results: Option<Vec<Result>>,
    #[serde(default)]
    memory: Option<ResidentMemory>,
    /// After the answer to an analysis whose job asked for records: the
    /// encoded index of the records made, in hexadecimal (empty when none
    /// could be made). It goes to the [`ResidentRecords`], not to the waiting
    /// analysis.
    #[serde(default)]
    records: Option<String>,
    /// The answer to a [`Request::Query`] (its id is the query's).
    #[serde(default)]
    answer: Option<crate::remote::Answer>,
    /// A records query's raw payload follows the JSON line, with this exact
    /// byte length. Its answer contains an empty records marker in the header.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    record_bytes: Option<usize>,
}

/// Writes one response and its optional binary records payload as one frame.
fn write_response(
    out: &mut impl std::io::Write,
    response: &Response,
    payload: Option<&[u8]>,
) -> bool {
    if response.record_bytes != payload.map(<[u8]>::len) {
        return false;
    }
    serde_json::to_writer(&mut *out, response).is_ok()
        && out.write_all(b"\n").is_ok()
        && payload.is_none_or(|bytes| out.write_all(bytes).is_ok())
        && out.flush().is_ok()
}

/// Reads a whole frame before the response is delivered to its waiting query.
/// Binary records can contain newlines and are never parsed as JSON text.
fn read_response(input: &mut impl std::io::BufRead) -> Option<Response> {
    let mut line = String::new();
    if input.read_line(&mut line).ok()? == 0 {
        return None;
    }
    let mut response: Response = serde_json::from_str(&line).ok()?;
    if let Some(length) = response.record_bytes {
        let Some(crate::remote::Answer::Records(bytes)) = response.answer.as_mut() else {
            return None;
        };
        if !bytes.is_empty()
            || response.records.is_some()
            || response.results.is_some()
            || response.memory.is_some()
        {
            return None;
        }
        bytes.try_reserve_exact(length).ok()?;
        bytes.resize(length, 0);
        input.read_exact(bytes).ok()?;
    }
    Some(response)
}

/// What the resident process keeps
/// ([`dartforge_parity::analysis::Resident::measure_memory`]), with the live
/// heap of the process before and after the measure (what is left after is
/// the engine with the SDK and the rest of the process).
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub(crate) struct ResidentMemory {
    pub(crate) heap_before: usize,
    pub(crate) heap_after: usize,
    pub(crate) parts: Vec<dartforge_parity::analysis::MemoryPart>,
}

/// The budget of the resident process, in bytes of kept source (`0`: off).
fn resident_budget() -> usize {
    std::env::var("DARTFORGE_LSP_RESIDENT_MIB")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(DEFAULT_RESIDENT_MIB)
        .saturating_mul(1024 * 1024)
}

/// How long the resident process may stay idle before it ends.
fn resident_idle() -> Duration {
    Duration::from_secs(
        std::env::var("DARTFORGE_LSP_RESIDENT_IDLE_SECS")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or(DEFAULT_RESIDENT_IDLE_SECS),
    )
}

/// The resident analysis process, seen from the worker: a child of the
/// server's own binary that keeps the analyzed units between analyses
/// ([`dartforge_parity::analysis::Resident`]), so that an edit only reparses
/// what changed instead of the whole program (on `new_sali`'s frontend, ~0.7 s
/// of lexing and parsing per analysis).
///
/// The memory stays out of the server, as with [`run_in_child`], and is
/// bounded: the process drops what it keeps above the budget
/// ([`resident_budget`]), and ends after [`resident_idle`] without analyses or
/// when the server ends. A crash only ends the child: the next analysis starts
/// another one.
struct ResidentChild {
    child: std::process::Child,
    /// Shared with the queries ([`ResidentRecords::query`]).
    input: Arc<Mutex<std::io::BufWriter<std::process::ChildStdin>>>,
    records: Arc<ResidentRecords>,
    output: mpsc::Receiver<Response>,
    /// When the last analysis ended.
    used: Instant,
}

impl ResidentChild {
    fn start(exe: &Path, records: &Arc<ResidentRecords>) -> Option<Self> {
        use std::process::{Command, Stdio};
        let mut child = Command::new(exe)
            .arg(CHILD_ARGUMENT)
            .arg(RESIDENT_ARGUMENT)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .ok()?;
        let input = Arc::new(Mutex::new(std::io::BufWriter::new(child.stdin.take()?)));
        records.attach(Arc::clone(&input));
        let stdout = child.stdout.take()?;
        let (sent, output) = mpsc::channel();
        let owner = Arc::clone(records);
        let records = Arc::clone(records);
        std::thread::spawn(move || {
            let mut input = std::io::BufReader::new(stdout);
            while let Some(response) = read_response(&mut input) {
                if let Some(hex) = &response.records {
                    records.arrived(response.id, hex);
                    continue;
                }
                if let Some(answer) = response.answer {
                    records.answer(response.id, answer);
                    continue;
                }
                if sent.send(response).is_err() {
                    break;
                }
            }
            records.gone();
        });
        Some(Self {
            child,
            input,
            records: owner,
            output,
            used: Instant::now(),
        })
    }

    fn send(&mut self, request: &Sent<'_>) -> bool {
        use std::io::Write;
        let mut input = self
            .input
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        serde_json::to_writer(&mut *input, request).is_ok()
            && input.write_all(b"\n").is_ok()
            && input.flush().is_ok()
    }

    /// Runs `job`. `None` when the process is no longer usable (it died, or did
    /// not stop a cancelled analysis in time and was ended): the caller drops
    /// it. Otherwise the results, empty when cancelled.
    fn run(
        &mut self,
        job: &Job,
        cancelled: &dyn Fn() -> bool,
        records: &ResidentRecords,
    ) -> Option<Vec<Result>> {
        let id = records.started(job);
        if !self.send(&Sent::Analyze { id, job }) {
            records.abandoned(id);
            return None;
        }
        let mut cancel_sent: Option<Instant> = None;
        let answer = loop {
            match self.output.recv_timeout(Duration::from_millis(10)) {
                Ok(r) if r.id == id => {
                    if r.results.is_none() {
                        // Cancelled: the resident makes no records.
                        records.abandoned(id);
                    }
                    break Some(r.results.unwrap_or_default());
                }
                Ok(_) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => match cancel_sent {
                    None if cancelled() => {
                        if !self.send(&Sent::Cancel { id }) {
                            break None;
                        }
                        cancel_sent = Some(Instant::now());
                    }
                    // A phase that does not stop soon (the load of a cold program):
                    // the new text matters more than what the process keeps.
                    Some(t) if t.elapsed() > Duration::from_millis(500) => break None,
                    _ => {}
                },
                Err(mpsc::RecvTimeoutError::Disconnected) => break None,
            }
        };
        if answer.is_none() {
            records.abandoned(id);
        }
        self.used = Instant::now();
        answer
    }

    /// The process's memory measure ([`Request::Measure`]); `None` when it
    /// does not answer within a few minutes.
    fn measure(&mut self) -> Option<ResidentMemory> {
        let id = self.records.lock().next_id();
        if !self.send(&Sent::Measure { id }) {
            return None;
        }
        let end = Instant::now() + Duration::from_secs(300);
        loop {
            let left = end.saturating_duration_since(Instant::now());
            match self.output.recv_timeout(left) {
                Ok(r) if r.id == id => return r.memory,
                Ok(_) => {}
                Err(_) => return None,
            }
        }
    }
}

impl Drop for ResidentChild {
    fn drop(&mut self) {
        self.records.detach();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Runs `job` in the resident process (started on demand), or `None` when it
/// cannot be used for this job (the caller falls back to [`run_in_child`]).
fn run_in_resident(
    resident: &mut Option<ResidentChild>,
    exe: &Path,
    job: &Arc<Job>,
    cancelled: &dyn Fn() -> bool,
    records: &Arc<ResidentRecords>,
) -> Option<Vec<Result>> {
    // A path that is not UTF-8 does not go into JSON.
    if job.root.to_str().is_none()
        || job
            .texts
            .keys()
            .chain(job.open.iter().map(|o| &o.2))
            .chain(job.others.iter())
            .any(|p| p.to_str().is_none())
    {
        return None;
    }
    if resident.is_none() {
        *resident = ResidentChild::start(exe, records);
    }
    let child = resident.as_mut()?;
    match child.run(job, cancelled, records) {
        Some(results) => Some(results),
        None => {
            // Dead or ended: the next analysis starts another process.
            *resident = None;
            Some(Vec::new())
        }
    }
}

/// The resident child analysis process ([`ResidentChild`]): reads
/// [`Request`]s from `stdin`, one per line, analyzes each job keeping the
/// analyzed units for the next ones, and writes one [`Response`] per job to
/// `stdout`. A job of another SDK starts over; what is kept above the budget
/// is dropped after the analysis. Ends when `stdin` closes.
fn resident_child() -> i32 {
    use dartforge_parity::analysis::Resident;
    use std::sync::atomic::{AtomicU64, Ordering};
    // The resident knows when the disk changes (the watcher's reports, its own
    // scan every 30 s): what the file system said stays between its loads
    // (`DARTFORGE_LSP_FS_CACHE=0` asks the disk every time).
    dartforge_elements::fs_cache::trust(
        std::env::var("DARTFORGE_LSP_FS_CACHE").map_or(true, |v| v.trim() != "0"),
    );
    let cancelled = Arc::new(AtomicU64::new(0));
    let (jobs, queue) = mpsc::channel::<(u64, Work)>();
    let reader = {
        let cancelled = Arc::clone(&cancelled);
        std::thread::spawn(move || {
            use std::io::BufRead;
            for line in std::io::stdin().lock().lines() {
                let Ok(line) = line else { break };
                let work = match serde_json::from_str::<Request>(&line) {
                    Ok(Request::Analyze { id, job }) => {
                        crate::memory::trace("resident read");
                        (id, Work::Analyze(job))
                    }
                    Ok(Request::Cancel { id }) => {
                        cancelled.store(id, Ordering::SeqCst);
                        continue;
                    }
                    Ok(Request::Measure { id }) => (id, Work::Measure),
                    Ok(Request::Query { id, query }) => {
                        crate::memory::trace("resident query read");
                        (id, Work::Query(query))
                    }
                    Err(_) => break,
                };
                if jobs.send(work).is_err() {
                    break;
                }
            }
        })
    };
    // Deep bodies recurse deeply: a dedicated stack, as in the worker thread.
    let run = std::thread::Builder::new()
        .name("dartforge-lsp-typed".into())
        .stack_size(1 << 30)
        .spawn(move || {
            let mut resident = ResidentLoop {
                engine: None,
                kept: Resident::new(),
                index_cache: crate::reference_index::ResidentCache::default(),
                patch_keys: None,
                served: crate::remote::Served::default(),
                out: std::io::BufWriter::new(std::io::stdout()),
                budget: resident_budget(),
                buffer: std::collections::VecDeque::new(),
                deferred: Vec::new(),
                ready: false,
            };
            loop {
                let (id, work) = match resident.buffer.pop_front() {
                    Some(w) => w,
                    None => match resident.deferred.iter().map(|d| d.2).min() {
                        None => match queue.recv() {
                            Ok(w) => w,
                            Err(_) => break,
                        },
                        Some(soonest) => {
                            let left = soonest.saturating_duration_since(Instant::now());
                            match queue.recv_timeout(left) {
                                Ok(w) => w,
                                Err(mpsc::RecvTimeoutError::Timeout) => {
                                    if !resident.expire_deferred() {
                                        return 1;
                                    }
                                    continue;
                                }
                                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            }
                        }
                    },
                };
                let ok = match work {
                    Work::Query(query) => resident.query(id, query),
                    Work::Measure => resident.measure(id),
                    Work::Analyze(job) => match resident.analyze(id, &job, &cancelled) {
                        Some(completed) => {
                            // The queries that came during the analysis, and those
                            // that waited for it, before the records: the records
                            // are not what the editor waits for.
                            while let Ok(w) = queue.try_recv() {
                                resident.buffer.push_back(w);
                            }
                            resident.serve_waiting() && resident.records(id, &job, completed)
                        }
                        None => false,
                    },
                };
                if !ok {
                    return 1;
                }
            }
            0
        });
    let code = match run.map(std::thread::JoinHandle::join) {
        Ok(Ok(code)) => code,
        _ => 101,
    };
    drop(reader);
    code
}

/// What the resident process reads: an analysis, the memory measure or a
/// query.
enum Work {
    Analyze(Box<Job>),
    Measure,
    Query(Box<crate::remote::QueryRequest>),
}

/// How long a query whose texts the resident's program does not have yet
/// waits for the analysis that brings them before it is answered
/// [`crate::remote::Answer::Unavailable`] (the server stops waiting earlier,
/// [`crate::remote::wait_limit`]).
const QUERY_DEFER: Duration = Duration::from_secs(20);

/// The state of the resident process's analysis thread.
struct ResidentLoop {
    engine: Option<(PathBuf, Engine)>,
    kept: dartforge_parity::analysis::Resident,
    index_cache: crate::reference_index::ResidentCache,
    patch_keys: Option<bool>,
    served: crate::remote::Served,
    out: std::io::BufWriter<std::io::Stdout>,
    budget: usize,
    /// What was read and not handled yet (the queries handled ahead of an
    /// analysis's records leave the rest here, in order).
    buffer: std::collections::VecDeque<(u64, Work)>,
    /// The queries waiting for an analysis, with their deadline.
    deferred: Vec<(u64, Box<crate::remote::QueryRequest>, Instant)>,
    /// The last analysis completed: the kept program is whole (a cancelled one
    /// may leave a unit swapped without its tables).
    ready: bool,
}

impl ResidentLoop {
    fn write(&mut self, response: &Response) -> bool {
        write_response(&mut self.out, response, None)
    }

    fn respond(&mut self, id: u64, answer: crate::remote::Answer) -> bool {
        crate::memory::trace("resident query answered");
        let (answer, bytes) = match answer {
            crate::remote::Answer::Records(bytes) => {
                (crate::remote::Answer::Records(Vec::new()), Some(bytes))
            }
            answer => (answer, None),
        };
        write_response(
            &mut self.out,
            &Response {
                id,
                results: None,
                memory: None,
                records: None,
                answer: Some(answer),
                record_bytes: bytes.as_ref().map(Vec::len),
            },
            bytes.as_deref(),
        )
    }

    /// Answers the query now, or defers it until an analysis brings its texts.
    fn query(&mut self, id: u64, query: Box<crate::remote::QueryRequest>) -> bool {
        let answer = if self.ready {
            crate::remote::serve(
                &mut self.kept,
                &mut self.served,
                &mut self.index_cache,
                &query,
            )
        } else {
            None
        };
        match answer {
            Some(answer) => self.respond(id, answer),
            None => {
                self.deferred
                    .push((id, query, Instant::now() + QUERY_DEFER));
                // A completed selective analysis may retain only the edited
                // library. No later analysis is otherwise scheduled to bring
                // the queried consumer back. Wake the parent worker now.
                self.respond(id, crate::remote::Answer::NeedsAnalysis)
            }
        }
    }

    /// Answers the deferred queries whose texts the program now has, and the
    /// buffered ones before the next analysis.
    fn serve_waiting(&mut self) -> bool {
        let mut waiting = std::mem::take(&mut self.deferred);
        while matches!(self.buffer.front(), Some((_, Work::Query(_)))) {
            if let Some((id, Work::Query(q))) = self.buffer.pop_front() {
                waiting.push((id, q, Instant::now() + QUERY_DEFER));
            }
        }
        for (id, q, deadline) in waiting {
            let answer = if self.ready {
                crate::remote::serve(&mut self.kept, &mut self.served, &mut self.index_cache, &q)
            } else {
                None
            };
            match answer {
                Some(answer) => {
                    if !self.respond(id, answer) {
                        return false;
                    }
                }
                None => self.deferred.push((id, q, deadline)),
            }
        }
        self.expire_deferred()
    }

    /// Answers the deferred queries past their deadline as unavailable.
    fn expire_deferred(&mut self) -> bool {
        let now = Instant::now();
        let (expired, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.deferred)
            .into_iter()
            .partition(|d| d.2 <= now);
        self.deferred = kept;
        expired
            .into_iter()
            .all(|(id, _, _)| self.respond(id, crate::remote::Answer::Unavailable))
    }

    fn measure(&mut self, id: u64) -> bool {
        let heap = || crate::memory::heap_in_use().unwrap_or(0);
        let heap_before = heap();
        let parts = self.kept.measure_memory(&heap);
        let memory = ResidentMemory {
            heap_before,
            heap_after: heap(),
            parts,
        };
        self.write(&Response {
            id,
            results: None,
            memory: Some(memory),
            records: None,
            answer: None,
            record_bytes: None,
        })
    }

    /// Analyzes `job` and writes its results; `None` when the process must
    /// end (no engine, or the output closed), otherwise whether the analysis
    /// completed.
    fn analyze(
        &mut self,
        id: u64,
        job: &Job,
        cancelled: &Arc<std::sync::atomic::AtomicU64>,
    ) -> Option<bool> {
        use std::sync::atomic::Ordering;
        crate::memory::trace("resident start");
        if self
            .engine
            .as_ref()
            .is_none_or(|(sdk, _)| *sdk != job.sdk_lib)
        {
            match lsp_engine(&job.sdk_lib) {
                Ok(e) => self.engine = Some((job.sdk_lib.clone(), e)),
                Err(error) => {
                    eprintln!("[dartforge-lsp] typed diagnostics disabled: {error}");
                    return None;
                }
            }
            self.kept.clear();
        }
        // A changed configuration may change how every unit loads.
        if job.all {
            self.kept.clear();
        }
        let (_, engine_ref) = self.engine.as_ref().expect("engine ready");
        let queries = crate::remote::enabled();
        self.kept.keep_bodies(queries || !job.records.is_empty());
        let stop = || cancelled.load(Ordering::SeqCst) == id;
        let kept = &mut self.kept;
        let results = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            analyze_package(engine_ref, job, &stop, Some(kept))
        }));
        let results = match results {
            Ok(r) if stop() => {
                drop(r);
                None
            }
            Ok(r) => Some(r),
            Err(_) => {
                eprintln!(
                    "[dartforge-lsp] the typed analysis of {} panicked; the syntactic diagnostics remain",
                    job.root.display()
                );
                self.kept.clear();
                Some(Vec::new())
            }
        };
        self.ready = false;
        self.index_cache.analyzed();
        if self.kept.source_bytes() > self.budget {
            self.kept.clear();
        }
        // The queries read the open documents' libraries: the bodies of the rest
        // of a large batch drop.
        let open: std::collections::HashSet<PathBuf> = job.texts.keys().cloned().collect();
        self.kept.retain_bodies(&|program, lib| {
            program.library(lib).units.iter().any(|u| {
                program.unit(*u).path.as_deref().is_some_and(|p| {
                    open.contains(dartforge_elements::generated::key_of(p).as_ref())
                })
            })
        });
        crate::memory::log(&format!(
            "after the analysis, in the resident child ({} units kept)",
            self.kept.units()
        ));
        if crate::memory::logging() {
            let census: Vec<String> = self
                .kept
                .census()
                .iter()
                .map(|(name, n)| format!("{name} {n}"))
                .collect();
            eprintln!("[dartforge-lsp] resident census: {}", census.join(", "));
        }
        crate::memory::trace("resident end");
        if std::env::var_os("DARTFORGE_LSP_DEBUG").is_some() {
            let s = dartforge_elements::fs_cache::stats();
            eprintln!(
                "[dartforge-lsp] resident file system cache: {} hits, {} misses, {} clears",
                s.hits, s.misses, s.clears
            );
        }
        let completed = results.is_some();
        self.ready = completed && self.kept.world_root().is_some();
        let written = self.write(&Response {
            id,
            results,
            memory: None,
            records: None,
            answer: None,
            record_bytes: None,
        });
        written.then_some(completed)
    }

    /// The edited files' index records, after the diagnostics are out: the
    /// bodies the analysis inferred, recorded where they are. Then the free
    /// pages the analysis left go back to the system.
    fn records(&mut self, id: u64, job: &Job, completed: bool) -> bool {
        if !job.records.is_empty() {
            let started = Instant::now();
            let sdk_lib = job.sdk_lib.clone();
            let patch_keys = *self
                .patch_keys
                .get_or_insert_with(|| crate::reference_index::analyzer_view_has_patches(&sdk_lib));
            let cache = &mut self.index_cache;
            let made = if completed {
                self.kept
                    .lend_analysis(|lent| {
                        crate::reference_index::records_of_analysis(
                            lent,
                            &job.records,
                            cache,
                            patch_keys,
                        )
                    })
                    .flatten()
            } else {
                None
            };
            if std::env::var_os("DARTFORGE_LSP_DEBUG").is_some() {
                eprintln!(
                    "[dartforge-lsp] resident: records of {} files in {} us ({} bytes)",
                    job.records.len(),
                    started.elapsed().as_micros(),
                    made.as_ref().map_or(0, Vec::len)
                );
            }
            let response = Response {
                id,
                results: None,
                memory: None,
                records: Some(made.map(|b| to_hex(&b)).unwrap_or_default()),
                answer: None,
                record_bytes: None,
            };
            if !self.write(&response) {
                return false;
            }
        }
        if crate::memory::relief() {
            let given = crate::memory::release_free_pages();
            if crate::memory::logging() {
                eprintln!(
                    "[dartforge-lsp] the resident gave {:.1} MiB back",
                    given as f64 / (1024.0 * 1024.0)
                );
            }
        }
        true
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
    let (signature, api) =
        dartforge_elements::incremental::api_signature_and_digest(text, &parsed.ast, &parsed.unit);
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
        api,
        words: dartforge_elements::incremental::text_words(text),
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
    /// The library of each changed open document (the file with its parts and
    /// its owner): analyzed and published first when the batch has more
    /// ([`split`]).
    priority: BTreeSet<PathBuf>,
    /// The package's `.dart` files `dart analyze` would see, in a workspace
    /// folder (the job's context).
    package: Vec<PathBuf>,
}

/// The files to analyze again after the API change of the `api` files (their
/// API signature changed): their libraries, and the dependents (through the
/// directives, transitively) that can see a changed name; `None` when that
/// cannot be told, and every dependent is analyzed.
///
/// A file's diagnostics depend on its library's text and on the declarations
/// it sees ([`plan`]); a dependent that names none of the changed
/// declarations, directly or through a declaration that changes with them,
/// sees the same ones. The changed names come from comparing the old and the
/// new [`ApiDigest`](dartforge_elements::incremental::ApiDigest) of each
/// changed file, declaration by declaration:
/// - a changed member also changes its owner's name;
/// - a changed class-like declaration also changes the classes of its library
///   that share a supertype of the library with it (a sealed family: the
///   exhaustiveness of a `switch` that names only a sibling);
/// - a declaration of the library or of an affected dependent whose signature
///   names a changed name changes too (an inferred type, a supertype), to a
///   fixpoint.
///
/// A dependent is affected when its text has a changed name (as a word,
/// comments included: an over-approximation), or when it points to an
/// affected file and has an `export` (a barrel). It cannot be told, and every
/// dependent is analyzed, when a file's directives changed, when a changed
/// declaration can be used without its name (an operator, an unnamed
/// extension, `call`, `iterator`, `toString`…), when a file has no digest,
/// or with `DARTFORGE_LSP_API_BY_NAME=0`.
fn api_affected(
    api: &[&PathBuf],
    memo: &Memo,
    now: &HashMap<PathBuf, Option<Facts>>,
    pointing: &HashMap<&Path, Vec<&Path>>,
    parts: &HashMap<&Path, Vec<&Path>>,
) -> Option<BTreeSet<PathBuf>> {
    use dartforge_elements::incremental::{ApiDeclaration, word_hash};
    if std::env::var("DARTFORGE_LSP_API_BY_NAME").is_ok_and(|v| v == "0") {
        return None;
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
    // The facts of a file as they are now: the changed ones', else the memo's.
    let facts_of = |k: &Path| -> Option<&Facts> {
        match now.get(k) {
            Some(f) => f.as_ref(),
            None => memo.files.get(k).map(|r| &r.facts),
        }
    };
    fn add(names: &mut BTreeSet<String>, d: &ApiDeclaration) {
        names.insert(d.name.clone());
        if let Some(o) = &d.owner {
            names.insert(o.clone());
        }
    }
    /// The digests of each name (with its owner) of a digest.
    fn digests(
        a: &dartforge_elements::incremental::ApiDigest,
    ) -> std::collections::BTreeMap<(Option<&str>, &str), Vec<u64>> {
        let mut m: std::collections::BTreeMap<(Option<&str>, &str), Vec<u64>> =
            std::collections::BTreeMap::new();
        for d in &a.declarations {
            m.entry((d.owner.as_deref(), d.name.as_str()))
                .or_default()
                .push(d.digest);
        }
        m
    }
    let mut names: BTreeSet<String> = BTreeSet::new();
    let mut changed_classes: Vec<&ApiDeclaration> = Vec::new();
    for k in api {
        let old = memo.files.get(*k)?.facts.api.as_ref()?;
        let new = now.get(*k)?.as_ref()?.api.as_ref()?;
        if old.directives != new.directives {
            return None;
        }
        let (before, after) = (digests(old), digests(new));
        for d in old.declarations.iter().chain(&new.declarations) {
            let key = (d.owner.as_deref(), d.name.as_str());
            if before.get(&key) == after.get(&key) {
                continue;
            }
            if d.implicit {
                return None;
            }
            add(&mut names, d);
            if d.class_like {
                changed_classes.push(d);
            }
        }
    }
    let library_files = closure(api.iter().map(|p| p.as_path()).collect(), parts);
    let mut library: Vec<&ApiDeclaration> = Vec::new();
    for f in &library_files {
        library.extend(&facts_of(f)?.api.as_ref()?.declarations);
    }
    // The sealed families: the library's classes that share a supertype of
    // the library with a changed class.
    let class_words: Vec<(u64, &ApiDeclaration)> = library
        .iter()
        .filter(|d| d.class_like)
        .map(|d| (word_hash(&d.name), *d))
        .collect();
    for c in &changed_classes {
        for (h, s) in &class_words {
            if *h != word_hash(&c.name) && c.words.binary_search(h).is_ok() {
                add(&mut names, s);
                for (_, sibling) in &class_words {
                    if sibling.words.binary_search(h).is_ok() {
                        add(&mut names, sibling);
                    }
                }
            }
        }
    }
    // The declarations whose signature names a changed name change too.
    let names_one = |d: &ApiDeclaration, hashes: &[u64]| {
        hashes.iter().any(|h| d.words.binary_search(h).is_ok())
    };
    let grow = |names: &mut BTreeSet<String>, declarations: &[&ApiDeclaration]| loop {
        let hashes: Vec<u64> = names.iter().map(|n| word_hash(n)).collect();
        let before = names.len();
        for d in declarations {
            if names_one(d, &hashes) {
                add(names, d);
            }
        }
        if names.len() == before {
            break;
        }
    };
    grow(&mut names, &library);
    // The dependents, to a fixpoint: an affected one may change the names it
    // declares.
    let candidates = closure(api.iter().map(|p| p.as_path()).collect(), pointing);
    let mut affected = library_files.clone();
    let export = word_hash("export");
    loop {
        let hashes: Vec<u64> = names.iter().map(|n| word_hash(n)).collect();
        let mut more: Vec<PathBuf> = Vec::new();
        for d in candidates.iter().filter(|d| !affected.contains(*d)) {
            let Some(f) = facts_of(d) else {
                more.push(d.clone());
                continue;
            };
            let named =
                f.words.is_empty() || hashes.iter().any(|h| f.words.binary_search(h).is_ok());
            let barrel = f.words.binary_search(&export).is_ok()
                && f.targets.iter().any(|(t, _)| affected.contains(t));
            if named || barrel {
                more.push(d.clone());
            }
        }
        if more.is_empty() {
            break;
        }
        for d in more {
            for f in closure(vec![d.as_path()], parts) {
                if affected.insert(f.clone()) {
                    let declarations: Vec<&ApiDeclaration> = facts_of(&f)
                        .and_then(|x| x.api.as_ref())
                        .map(|a| a.declarations.iter().collect())
                        .unwrap_or_default();
                    grow(&mut names, &declarations);
                }
            }
        }
    }
    Some(affected)
}

/// The `.dart` files of the package at `root` that `dart analyze` would see
/// (outside the exclusions of its `analysis_options.yaml`), listed again only
/// when a changed file was created or deleted (it is, or is not, in the list
/// against the disk), when the options file changed, or after
/// [`LISTING_AGE`]: walking the package's tree cost ~12 ms per analysis on
/// `new_sali`'s frontend.
fn package_files(
    listed: &mut HashMap<PathBuf, (Instant, Vec<PathBuf>)>,
    root: &Path,
    changed: &BTreeSet<PathBuf>,
) -> Vec<PathBuf> {
    if let Some((at, files)) = listed.get(root) {
        let known: BTreeSet<PathBuf> = files.iter().map(|f| key(f)).collect();
        // A file the walk would list: in the package, outside hidden folders
        // and `build/` (the generated files of `.dart_tool` do not count).
        let listable = |k: &Path| {
            k.strip_prefix(root).is_ok_and(|rel| {
                rel.components()
                    .all(|c| !c.as_os_str().to_string_lossy().starts_with('.'))
                    && rel
                        .components()
                        .next()
                        .is_none_or(|c| c.as_os_str() != "build")
            })
        };
        let stale = at.elapsed() > LISTING_AGE
            || changed.iter().any(|k| {
                k.file_name().is_some_and(|n| n == "analysis_options.yaml")
                    || (k.extension().is_some_and(|e| e == "dart")
                        && listable(k)
                        && known.contains(k) != k.is_file())
            });
        if !stale {
            return files.clone();
        }
    }
    let options = dartforge_parity::filters::AnalysisOptions::read(root);
    let files = dartforge_parity::projects::files(root, &options);
    listed.insert(root.to_path_buf(), (Instant::now(), files.clone()));
    files
}

/// How long a package's file list ([`package_files`]) serves without a
/// change that says it is stale.
const LISTING_AGE: Duration = Duration::from_secs(30);

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
    package: &[PathBuf],
    changed: &BTreeSet<PathBuf>,
) -> Plan {
    // Documents in excluded folders are not analyzed: their publication is empty.
    let excluded_ = |c: &Path| config.excluded.iter().any(|e| c.starts_with(e));
    let (excluded, open): (Vec<_>, Vec<_>) = open.into_iter().partition(|o| excluded_(&o.2));
    let excluded = excluded.into_iter().map(|o| (o.0, o.1)).collect();
    // The package's `.dart` files that `dart analyze` would see (outside the
    // exclusions of `analysis_options.yaml` and of `analysisExcludedFolders`).
    let of_package: Vec<PathBuf> = package.iter().filter(|a| !excluded_(a)).cloned().collect();
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
            priority: BTreeSet::new(),
            package: of_package,
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
    // An API change: the dependents that can see a changed name
    // ([`api_affected`]), else every dependent.
    let mut fresh = match api_affected(&api, memo, &now, &pointing, &parts) {
        Some(affected) => affected,
        None => closure(api.into_iter().map(PathBuf::as_path).collect(), &pointing),
    };
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
    let priority = closure(
        changed
            .iter()
            .filter(|k| open_keys.contains(*k))
            .map(PathBuf::as_path)
            .collect(),
        &parts,
    );
    let open = open
        .into_iter()
        .filter(|o| fresh.contains(&key(&o.2)))
        .collect();
    Plan {
        open,
        others,
        order,
        excluded,
        priority,
        package: of_package,
    }
}

/// Splits `job` into the analysis of the `priority` files and that of the
/// rest, when it has both and is not a first analysis (which also publishes
/// the YAML files). A file's diagnostics depend only on its library's text and
/// on the declarations it sees ([`plan`]), so the two halves publish what the
/// whole batch would.
fn split(job: &Job, priority: &BTreeSet<PathBuf>) -> Option<(Job, Job)> {
    if job.all || priority.is_empty() {
        return None;
    }
    let (open_first, open_rest): (Vec<_>, Vec<_>) = job
        .open
        .iter()
        .cloned()
        .partition(|o| priority.contains(&key(&o.2)));
    let (others_first, others_rest): (Vec<_>, Vec<_>) = job
        .others
        .iter()
        .cloned()
        .partition(|a| priority.contains(&key(a)));
    if open_first.is_empty() || open_rest.len() + others_rest.len() == 0 {
        return None;
    }
    let half = |open, others| Job {
        sdk_lib: job.sdk_lib.clone(),
        root: job.root.clone(),
        open,
        others,
        texts: job.texts.clone(),
        config: job.config.clone(),
        in_workspace: job.in_workspace,
        all: false,
        changed: job.changed.clone(),
        context: job.context.clone(),
        records: Vec::new(),
        records_epoch: job.records_epoch,
    };
    let (mut first, mut rest) = (half(open_first, others_first), half(open_rest, others_rest));
    let in_first = |r: &PathBuf| first.open.iter().any(|o| key(&o.2) == *r);
    let (to_first, to_rest): (Vec<PathBuf>, Vec<PathBuf>) =
        job.records.iter().cloned().partition(in_first);
    first.records = to_first;
    rest.records = to_rest;
    Some((first, rest))
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

/// The engine of the typed analysis: it runs only the tree lint rules that
/// the files' `analysis_options.yaml` enables, the only ones published.
pub(crate) fn lsp_engine(sdk_lib: &Path) -> std::result::Result<Engine, String> {
    let mut engine = Engine::new(sdk_lib)?;
    engine.lint_rules_from_options(true);
    Ok(engine)
}

/// Analyzes the files of `job` (its open documents and the other files of the
/// package that [`plan`] chose) in a single program of package `root`, and
/// returns their publications with their [`Facts`], plus the package's YAML
/// files when they are due. Empty when cancelled.
fn analyze_package(
    engine: &Engine,
    job: &Job,
    cancelled: &dyn Fn() -> bool,
    resident: Option<&mut dartforge_parity::analysis::Resident>,
) -> Vec<Result> {
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
    let packages_path = package_config.as_deref();
    let analysis = match resident {
        Some(r) => {
            r.set_context(job.context.clone());
            // Only the editor's texts changed (no watcher notification, no
            // document closed): the disk is as the last analysis read it.
            // Otherwise, what changed on it is what the watcher reported (and
            // the documents closed): the resident checks those, not every
            // file it read.
            if !all && changed.iter().all(|k| job.texts.contains_key(k)) {
                r.disk_unchanged();
            } else if !all {
                r.disk_changed_only(
                    changed
                        .iter()
                        .filter(|k| !job.texts.contains_key(*k))
                        .cloned()
                        .collect(),
                );
            }
            engine.analyze_resident(r, root, &files, packages_path, &texts, cancelled)
        }
        None => engine.analyze_with(root, &files, packages_path, &texts, cancelled),
    };
    let Some(analysis) = analysis else {
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

#[cfg(test)]
mod transport_tests {
    use super::{Response, read_response, write_response};
    use crate::remote::Answer;
    use std::io::{BufReader, Cursor};

    fn response(id: u64, answer: Answer, record_bytes: Option<usize>) -> Response {
        Response {
            id,
            results: None,
            memory: None,
            records: None,
            answer: Some(answer),
            record_bytes,
        }
    }

    #[test]
    fn binary_records_preserve_message_boundaries() {
        for bytes in [b"".as_slice(), b"\0\xff\n{\"id\":99}\n"] {
            let mut wire = Vec::new();
            assert!(write_response(
                &mut wire,
                &response(7, Answer::Records(Vec::new()), Some(bytes.len())),
                Some(bytes),
            ));
            assert!(write_response(
                &mut wire,
                &response(8, Answer::Unavailable, None),
                None,
            ));
            // Split both the JSON headers and raw bytes across small reads.
            let mut input = BufReader::with_capacity(3, Cursor::new(wire));
            let first = read_response(&mut input).expect("records frame");
            assert_eq!(first.id, 7);
            assert!(matches!(first.answer, Some(Answer::Records(ref actual)) if actual == bytes));
            let next = read_response(&mut input).expect("next JSON frame");
            assert_eq!(next.id, 8);
            assert!(matches!(next.answer, Some(Answer::Unavailable)));
            assert!(read_response(&mut input).is_none());
        }
    }

    #[test]
    fn truncated_binary_records_are_not_delivered() {
        let mut wire = Vec::new();
        assert!(write_response(
            &mut wire,
            &response(7, Answer::Records(Vec::new()), Some(4)),
            Some(b"data"),
        ));
        wire.pop();
        assert!(read_response(&mut Cursor::new(wire)).is_none());
    }

    #[test]
    fn binary_header_requires_an_empty_records_marker() {
        for answer in [Answer::Unavailable, Answer::Records(vec![1])] {
            let header = response(7, answer, Some(1));
            let mut wire = Vec::new();
            assert!(write_response(&mut wire, &header, Some(b"x")));
            assert!(read_response(&mut Cursor::new(wire)).is_none());
        }
        assert!(!write_response(
            &mut Vec::new(),
            &response(7, Answer::Records(Vec::new()), Some(4)),
            Some(b"x"),
        ));
    }
}

#[cfg(test)]
mod warm_query_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "dartforge-resident-warm-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&root).unwrap();
            std::fs::create_dir(root.join("nested")).unwrap();
            std::fs::write(root.join("pubspec.yaml"), "name: warm_query_test\n").unwrap();
            Self(root)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn open(state: &mut State, root: &Path, name: &str) {
        let path = root.join(name);
        state.documents.insert(
            name.to_string(),
            Document {
                version: 1,
                text: Arc::from("void main() {}"),
                path,
                root: root.to_path_buf(),
            },
        );
    }

    #[test]
    fn refresh_is_not_downgraded_by_an_ordinary_warm_request() {
        let mut state = State::default();
        let root = PathBuf::from("project");
        state.request_warm(root.clone(), true);
        state.request_warm(root.clone(), false);
        assert_eq!(state.warm.pop_first(), Some((root.clone(), true)));
        state.request_warm(root.clone(), false);
        assert_eq!(state.warm.pop_first(), Some((root, false)));
    }

    #[test]
    fn canonical_query_warms_the_existing_lexical_alias_for_an_unsaved_file() {
        let fixture = Fixture::new();
        let canonical = dartforge_elements::config::canonicalize(&fixture.0).unwrap();
        let alias = fixture.0.join("nested/..");
        let mut state = State::default();
        open(&mut state, &alias, "unsaved.dart");
        assert!(!canonical.join("unsaved.dart").exists());
        assert_eq!(
            state.warm_root(&canonical.join("unsaved.dart")),
            Some(alias)
        );
    }

    #[test]
    fn a_query_cannot_warm_an_unrelated_package() {
        let first = Fixture::new();
        let other = Fixture::new();
        let mut state = State::default();
        open(&mut state, &first.0, "main.dart");
        assert_eq!(state.warm_root(&other.0.join("main.dart")), None);
    }

    #[cfg(unix)]
    #[test]
    fn canonical_query_warms_the_existing_symbolic_alias() {
        let fixture = Fixture::new();
        let canonical = dartforge_elements::config::canonicalize(&fixture.0).unwrap();
        let alias = fixture.0.join("nested/alias");
        std::os::unix::fs::symlink(&canonical, &alias).unwrap();
        let mut state = State::default();
        open(&mut state, &alias, "main.dart");
        assert_eq!(state.warm_root(&canonical.join("main.dart")), Some(alias));
    }

    #[cfg(unix)]
    #[test]
    fn distinct_open_aliases_do_not_choose_an_arbitrary_package_context() {
        let fixture = Fixture::new();
        let canonical = dartforge_elements::config::canonicalize(&fixture.0).unwrap();
        let first = fixture.0.join("nested/first");
        let second = fixture.0.join("nested/second");
        std::os::unix::fs::symlink(&canonical, &first).unwrap();
        std::os::unix::fs::symlink(&canonical, &second).unwrap();
        let mut state = State::default();
        open(&mut state, &first, "first.dart");
        open(&mut state, &second, "second.dart");
        assert_eq!(state.warm_root(&canonical.join("unsaved.dart")), None);
    }
}

#[cfg(test)]
mod record_context_tests {
    use super::*;

    fn job(records: &ResidentRecords) -> Job {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let path = root.join("record_context.dart");
        let text: Arc<str> = Arc::from("int value = 0;");
        let uri = url::Url::from_file_path(&path).unwrap().to_string();
        let open = vec![(uri, 1, path.clone(), text.clone())];
        records.want(Some(root.clone()));
        let (wanted, epoch) = records.wanted_for(&root, &open, &BTreeSet::from([key(&path)]));
        assert_eq!(wanted.len(), 1);
        Job {
            sdk_lib: root.join("sdk/lib"),
            root,
            open,
            others: Vec::new(),
            texts: HashMap::from([(key(&path), text)]),
            config: AnalysisConfiguration {
                all: false,
                all_types: Vec::new(),
                excluded: Vec::new(),
            },
            in_workspace: true,
            all: false,
            changed: BTreeSet::from([key(&path)]),
            context: vec![path],
            records: wanted,
            records_epoch: epoch,
        }
    }

    #[test]
    fn invalidation_rejects_running_and_previously_selected_jobs() {
        let records = ResidentRecords::default();
        let old = job(&records);
        let running = records.started(&old);
        assert_eq!(records.lock().contexts.len(), 1);
        records.invalidate();
        records.arrived(running, &to_hex(b"old records"));
        let selected = records.started(&old);
        records.arrived(selected, &to_hex(b"selected before invalidation"));
        assert!(records.lock().ready.is_empty());
        assert!(records.lock().pending.is_empty());
        let fresh = job(&records);
        let id = records.started(&fresh);
        records.arrived(id, &to_hex(b"current records"));
        assert_eq!(records.lock().ready.len(), 1);
        for _ in 0..64 {
            let id = records.started(&fresh);
            records.arrived(id, &to_hex(b"latest records"));
        }
        assert_eq!(records.lock().ready.len(), 1);
        assert_eq!(records.lock().ready[0].bytes, b"latest records");
        records.want(None);
        assert!(records.lock().ready.is_empty());
        let id = records.started(&fresh);
        records.arrived(id, &to_hex(b"old project"));
        assert!(records.lock().ready.is_empty());
    }

    #[test]
    fn a_late_old_resident_cannot_supply_a_new_job() {
        let records = ResidentRecords::default();
        let job = job(&records);
        let old = records.started(&job);
        records.gone();
        let new = records.started(&job);
        assert_ne!(old, new);
        records.arrived(old, &to_hex(b"old resident"));
        assert!(records.lock().ready.is_empty());
        records.arrived(new, &to_hex(b"new resident"));
        assert_eq!(records.lock().ready[0].bytes, b"new resident");
    }

    #[test]
    fn records_require_every_open_dart_overlay_and_the_same_sdk() {
        let records = ResidentRecords::default();
        let mut job = job(&records);
        let foreign = job.root.join("other_package/foreign.dart");
        let foreign_uri = url::Url::from_file_path(&foreign).unwrap().to_string();
        job.texts
            .insert(key(&foreign), Arc::from("int target = 1;"));
        let mut documents = crate::DocumentStore::new();
        documents.open(job.open[0].0.clone(), 1, job.open[0].3.to_string());
        documents.open(foreign_uri.clone(), 1, "int target = 1;".to_string());
        let id = records.started(&job);
        records.arrived(id, &to_hex(b"records"));
        let batch = records.lock().ready.pop().unwrap();
        let matches = |documents: &crate::DocumentStore, sdk: &Path| {
            RecordContext::current(sdk, documents).as_ref() == Some(&batch.context)
        };
        assert!(matches(&documents, &job.sdk_lib));
        assert!(!matches(&documents, &job.sdk_lib.join("other")));
        documents.update(&foreign_uri, 2, "int target = 2;".to_string());
        assert!(!matches(&documents, &job.sdk_lib));
        documents.close(&foreign_uri);
        assert!(!matches(&documents, &job.sdk_lib));
        documents.open(foreign_uri.clone(), 3, "int target = 1;".to_string());
        let yaml = url::Url::from_file_path(job.root.join("pubspec.yaml"))
            .unwrap()
            .to_string();
        documents.open(yaml, 1, "name: example".to_string());
        assert!(matches(&documents, &job.sdk_lib));
        let alias = foreign_uri.replace("foreign.dart", "%66oreign.dart");
        documents.open(alias, 1, "int target = 99;".to_string());
        assert!(RecordContext::current(&job.sdk_lib, &documents).is_none());
    }
}
