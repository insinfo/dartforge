//! The engine: plan + graph + per-action fingerprint + deterministic
//! schedule + executors, publishing into an in-memory `Generation`.
//!
//! * Phases run in series; the actions of a phase run in parallel
//!   (`std::thread::scope`, atomic index, up to `workers`), with results
//!   gathered in action order — the result does not depend on the number of
//!   workers.
//! * An action that already ran only runs again if the digest of some query
//!   it made changed (revalidation), and an output equal to the previous one
//!   invalidates nobody (output cutoff).
//! * Laziness (D-B6): under [`Demand::Loader`] only the outputs the
//!   loader can import (`.dart`) and the `build_to: source` ones are
//!   computed; the rest (a served `.css`, `--compare`) goes through
//!   [`Engine::materialize`] or [`Demand::Everything`]. With the Dart executor,
//!   the hidden phases it runs are included too (a later builder may read
//!   them); only the optional ones stay lazy.
use crate::query::{SemanticDb, Query, Digest, digest_bytes};
use crate::executor::{
    ActionService, GeneratorCtx, Availability, DartExecutor, NativeGenerator, Unavailable,
    NativeAction, ActionRequest, ExtensionsRequest, NativeRequest, BuilderScript, BuildStepService,
    glob_candidates, query_digest,
};
use crate::graph::{AssetId, Graph};
use crate::packages::PackageGraph;
use crate::plan::{Configs, Phase, TargetNode, Plan, phases};
use dartforge_elements::config::PackageConfig;
use dartforge_elements::generated::{GenerationBuilder, Generation, key};
use dartforge_elements::model::Program;
use dartforge_elements::unit_cache::FileStamp;
use dartforge_intern::Interner;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct EngineOptions {
    pub release: bool,
    pub workers: usize,
    /// A stale fallback becomes an error (D-B5, `dartforge build --strict`).
    pub strict: bool,
    /// Also runs the native generators that are not verified yet, only to
    /// measure them (`--compare`).
    pub measure_unchecked: bool,
    /// Reads and writes the state across processes in
    /// `.dart_tool/dartforge/build/estado` (§4.1). Off by default: only when
    /// the user asks for it (`dartforge build --state`,
    /// `DARTFORGE_BUILD_STATE=1`), per governing rule 6 of `PLANO.md`.
    pub persist: bool,
    /// `--config <name>`: `build.<name>.yaml` in place of the root's
    /// `build.yaml` (targets, `global_options` and `triggers`).
    pub config: Option<String>,
    /// `--define <builder>=<option>=<value>`, on top of the `global_options`.
    pub defines: crate::command_line::Defines,
    /// `--build-filter`: only the actions with an output some filter matches
    /// run from the start; the others only if a running step reads their
    /// output (`shouldBuildForDirs` and the official on-demand build).
    pub filters: Vec<crate::command_line::BuildFilter>,
}

impl Default for EngineOptions {
    fn default() -> Self {
        EngineOptions {
            release: false,
            workers: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .min(8),
            strict: false,
            measure_unchecked: false,
            persist: false,
            config: None,
            defines: Default::default(),
            filters: Vec::new(),
        }
    }
}

/// An action's queries with the digest of each answer.
type Queries = Vec<(Query, Option<Digest>)>;

/// Who produced an output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    Native(&'static str),
    Dart,
    Support,
    Pending,
    /// The builder (or the executor, in the middle of the action) failed: no
    /// outputs, and the whole generation fails (DF-BUILD-005).
    Failure,
    /// A generated input that was not written (or failed): the official tool
    /// does not run the step (`_runPostProcessAction` and
    /// `_matchingPrimaryInputs`, `wasOutput`).
    Skipped,
    /// `run_only_if_triggered: true` and no trigger fired
    /// ([`crate::triggers`]): the step does not run, and nothing is written.
    NotTriggered,
}

/// Why an action did not come out of the Dart executor.
#[derive(Debug, Clone)]
enum DartFailure {
    /// There is no executor (disabled, does not start): the fallback policy
    /// applies.
    Unavailable(String),
    /// The builder failed, or the executor crashed in the middle of the
    /// action.
    Builder(String),
}

/// What is known about an action that ran.
#[derive(Debug, Clone)]
pub struct ActionRecord {
    pub fingerprint: Digest,
    pub queries: Vec<(Query, Option<Digest>)>,
    /// Expected outputs and their content (`None` = not written).
    pub outputs: Vec<(AssetId, Option<Arc<[u8]>>)>,
    pub origin: Origin,
    /// Reason it is pending (or why the native generator refused, when the
    /// fallback supplied it).
    pub reason: Option<String>,
    /// Measurement of an unverified native generator (only with
    /// `measure_unchecked`).
    pub measured: Vec<(AssetId, Arc<[u8]>)>,
    /// In a native action, the outputs the native generator does not write
    /// and that came from the fallback (the `.css.dart` the official ngdart
    /// writes next to the `.css.shim.dart`): they count as pending, not as
    /// equal.
    pub from_fallback: Vec<AssetId>,
    /// Post-processor: inputs marked by `deletePrimaryInput`.
    pub deleted_outputs: Vec<AssetId>,
    /// Came from the state saved by another process and has not been checked
    /// in this session yet: the first verification recomputes **all**
    /// queries, not only the ones the events touch (§4.1).
    pub restored: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Demand {
    /// What the loader can import (`.dart`) and the `source` outputs.
    Loader,
    /// Everything (`dartforge build`, `--compare`).
    Everything,
}

/// Report of one update (goes into the `dev` crate's `Report`).
#[derive(Debug, Clone, Default)]
pub struct EngineReport {
    pub checked_actions: usize,
    pub executed_actions: usize,
    pub reevaluated_queries: usize,
    pub changed_outputs: usize,
    pub native_count: usize,
    pub dart: usize,
    /// Units actually regenerated inside a per-package generator.
    pub native_unit_count: usize,
    /// Queries recorded by generators in this update.
    pub generator_queries: usize,
    pub support: usize,
    /// Of the fallback actions, those that produced content read from
    /// `build_runner`'s disk (for the others, the official tool wrote nothing
    /// there).
    pub support_with_output: usize,
    pub pending_by_reason: BTreeMap<String, usize>,
    /// Steps with `run_only_if_triggered` that no trigger fired.
    pub not_triggered: usize,
    /// Actions whose builder failed in this update: the generation failed.
    pub failures: Vec<String>,
    pub elapsed_time: Duration,
    /// Revalidation of the per-package rounds (digests of the affected
    /// queries).
    pub revalidation_time: Duration,
    /// Per-package native generators (ngdart's stage A).
    pub native_time: Duration,
}

impl EngineReport {
    /// Adds a following pass (the on-demand build of `--build-filter`
    /// repeats the update).
    fn add(&mut self, o: EngineReport) {
        self.checked_actions += o.checked_actions;
        self.executed_actions += o.executed_actions;
        self.reevaluated_queries += o.reevaluated_queries;
        self.native_count += o.native_count;
        self.dart += o.dart;
        self.native_unit_count += o.native_unit_count;
        self.generator_queries += o.generator_queries;
        self.support += o.support;
        self.support_with_output += o.support_with_output;
        for (m, n) in o.pending_by_reason {
            *self.pending_by_reason.entry(m).or_default() += n;
        }
        self.not_triggered += o.not_triggered;
        self.failures.extend(o.failures);
        self.elapsed_time += o.elapsed_time;
        self.revalidation_time += o.revalidation_time;
        self.native_time += o.native_time;
    }

    pub fn text(&self) -> String {
        let mut s = format!(
            "engine: {} actions checked, {} run ({} native, {} Dart, {} fallback, {} units regenerated), {} queries reevaluated, {} generator queries, {} outputs changed, {:.1} ms (revalidation {:.1} ms, per-package native {:.1} ms)",
            self.checked_actions,
            self.executed_actions,
            self.native_count,
            self.dart,
            self.support,
            self.native_unit_count,
            self.reevaluated_queries,
            self.generator_queries,
            self.changed_outputs,
            self.elapsed_time.as_secs_f64() * 1000.0,
            self.revalidation_time.as_secs_f64() * 1000.0,
            self.native_time.as_secs_f64() * 1000.0,
        );
        if self.not_triggered > 0 {
            s.push_str(&format!("\n  not triggered: {}", self.not_triggered));
        }
        for (m, n) in &self.pending_by_reason {
            s.push_str(&format!("\n  pending ({n}): {m}"));
        }
        for f in &self.failures {
            s.push_str(&format!("\n  failure: {f}"));
        }
        s
    }
}

pub struct Update {
    pub generation: Arc<Generation>,
    /// Natural paths whose content changed (or disappeared) in this update.
    pub altered_outputs: Vec<PathBuf>,
    pub rel: EngineReport,
    pub warnings: Vec<String>,
}

/// Context of an update: the semantic database and, in a session, the
/// already loaded program (`BuildStep.resolver` without an extra load).
pub struct UpdateContext<'a> {
    pub db: &'a dyn SemanticDb,
    pub program: Option<(&'a Program, &'a Interner)>,
}

/// One run of a per-package generator in a round.
struct PackageRound {
    queries: Vec<(Query, Option<Digest>)>,
    outputs: BTreeMap<PathBuf, Arc<[u8]>>,
    refusals: BTreeMap<PathBuf, String>,
    error: Option<String>,
    generated_units: usize,
}

pub struct Engine {
    pub root: PathBuf,
    pub package_graph: PackageGraph,
    pub configs: Configs,
    pub plan: Plan,
    pub phases: Vec<Phase>,
    pub targets: Vec<TargetNode>,
    pub graph: Graph,
    pub options: EngineOptions,
    natives: Vec<Arc<dyn NativeGenerator>>,
    dart: std::sync::Mutex<Box<dyn DartExecutor>>,
    dart_prepared: AtomicBool,
    /// The run-time extensions were already checked with the Dart executor
    /// ([`Engine::check_extensions`]).
    extensions_checked: bool,
    records: Vec<Option<ActionRecord>>,
    /// Actions per phase (indices into `graph.actions`).
    per_phase: Vec<Vec<usize>>,
    /// Rounds of the per-package generators: (generator, package).
    package_rounds: HashMap<(usize, Arc<str>), PackageRound>,
    memory: HashMap<PathBuf, Arc<[u8]>>,
    /// Output → natural path (the generation key).
    pub natural_paths_map: BTreeMap<AssetId, PathBuf>,
    /// Natural paths of the `.dart` outputs (what the loader may ask for).
    expected: HashSet<PathBuf>,
    /// Natural paths of the listed sources (a new or deleted file rebuilds
    /// the graph, as `graph.dart:343-356` re-evaluates the globs).
    sources: HashSet<PathBuf>,
    /// Directories with sources of the root package: a directory's mark
    /// changes when a file enters or leaves it.
    dirs: Vec<PathBuf>,
    /// Configuration files: when one changes, the plan is rebuilt.
    config_files: Vec<PathBuf>,
    marks: HashMap<PathBuf, Option<FileStamp>>,
    /// What [`Engine::changes`] looks at (recomputed on every update).
    watched: Vec<PathBuf>,
    generation: Arc<Generation>,
    warned: HashSet<PathBuf>,
    labels: HashMap<String, &'static str>,
    /// Outputs that the graph rebuild removed from memory (the action or the
    /// anchor no longer exists): they join the update's changed paths, so
    /// that the generation is republished without them.
    removed: Vec<PathBuf>,
    /// Saved state read at startup, applied on the first update (after the
    /// extensions and the anchors were checked with the Dart executor).
    to_restore: Option<crate::persistence::State>,
    /// The Dart builders' code from the restored state, checked: it allows
    /// re-saving restored Dart actions without the executor having run.
    restored_code: Option<Vec<PathBuf>>,
    /// Some record changed since the state was last written.
    state_dirty: bool,
    /// With `--build-filter`: the requested actions (those with a filtered
    /// output, the chain of their generated inputs, and those that a step
    /// that ran read).
    requested: HashSet<usize>,
}

impl Drop for Engine {
    fn drop(&mut self) {
        if let Ok(mut dart) = self.dart.lock() {
            dart.shutdown();
        }
    }
}

fn natural_path(graph: &PackageGraph, id: &AssetId) -> PathBuf {
    let root = graph
        .node(&id.package)
        .map(|n| n.root.clone())
        .unwrap_or_default();
    key(&root.join(id.path.as_ref()))
}

struct Indices {
    per_phase: Vec<Vec<usize>>,
    natural_paths: BTreeMap<AssetId, PathBuf>,
    expected: HashSet<PathBuf>,
    sources: HashSet<PathBuf>,
    dirs: Vec<PathBuf>,
}

fn build_indices(gp: &PackageGraph, graph: &Graph, phase_count: usize) -> Indices {
    let mut per_phase = vec![Vec::new(); phase_count];
    for (i, a) in graph.actions.iter().enumerate() {
        if a.live() {
            per_phase[a.phase].push(i);
        }
    }
    let mut natural_paths = BTreeMap::new();
    let mut expected = HashSet::new();
    for a in &graph.actions {
        for s in &a.outputs {
            let n = natural_path(gp, s);
            if s.path.ends_with(".dart") {
                expected.insert(n.clone());
            }
            natural_paths.insert(s.clone(), n);
        }
    }
    let mut sources = HashSet::new();
    let mut dirs = BTreeSet::new();
    let root = &gp.nodes[gp.root];
    for (package, paths) in &graph.sources {
        let Some(node) = gp.node(package) else { continue };
        for c in paths {
            if c.contains('$') {
                continue;
            }
            let n = key(&node.root.join(c.as_ref()));
            if node.name == root.name {
                let mut d = n.parent();
                while let Some(x) = d {
                    if !x.starts_with(&root.root) || x == root.root {
                        break;
                    }
                    dirs.insert(x.to_path_buf());
                    d = x.parent();
                }
            }
            sources.insert(n);
        }
    }
    Indices {
        per_phase,
        natural_paths,
        expected,
        sources,
        dirs: dirs.into_iter().collect(),
    }
}

/// Canonical form of an event path (the file may have been deleted: then
/// the directory is canonicalized).
fn canonical(p: &Path) -> Option<PathBuf> {
    let c = match std::fs::canonicalize(p) {
        Ok(c) => c,
        Err(_) => std::fs::canonicalize(p.parent()?)
            .ok()?
            .join(p.file_name()?),
    };
    Some(key(&dartforge_elements::config::without_verbatim(c)))
}

/// Could the query have changed with these events?
fn affected(
    c: &Query,
    changed: &HashSet<PathBuf>,
    dart_changed: bool,
    structural: &HashSet<PathBuf>,
) -> bool {
    match c {
        // A listing only changes when a file enters or leaves (a watched
        // directory that changed, or a new/deleted file).
        Query::Glob { dir, .. } => structural.iter().any(|m| m.starts_with(dir)),
        Query::ActiveGlobs {
            dir, candidates, ..
        } => {
            structural.iter().any(|m| m.starts_with(dir))
                || candidates
                    .iter()
                    .any(|(rel, _)| changed.contains(&key(&dir.join(rel))))
        }
        _ => c.path().is_some_and(|p| changed.contains(p)) || (c.semantic() && dart_changed),
    }
}

impl Engine {
    /// Builds the engine of a project: reads the configuration and assembles
    /// plan, phases and graph. Only called when [`crate::detect`] said yes.
    pub fn new(root: &Path, cfg: &PackageConfig, options: EngineOptions) -> Result<Engine, String> {
        crate::count_instance();
        let package_graph = PackageGraph::build(root, cfg, None)?;
        let script_configs = Configs::read(&package_graph, false)?;
        let plan = Plan::from_script(&package_graph, &script_configs)?;
        let configs = Configs::read_with_config(&package_graph, true, options.config.as_deref())?;
        let (phases, targets) = phases(
            &package_graph,
            &configs,
            &plan,
            options.release,
            &options.defines,
        )?;
        let graph = Graph::build(&package_graph, &configs, &phases, &targets)?;
        let Indices {
            per_phase,
            natural_paths,
            expected,
            sources,
            dirs,
        } = build_indices(&package_graph, &graph, phases.len());
        let mut config_files = vec![
            key(&package_graph.root_dir.join("pubspec.lock")),
            key(
                &package_graph
                    .root_dir
                    .join(".dart_tool")
                    .join("package_config.json"),
            ),
        ];
        let r = &package_graph.nodes[package_graph.root].root;
        config_files.push(key(&r.join("pubspec.yaml")));
        // Each package's `build.yaml`, whether it exists or not (creating it
        // changes the plan), and the root directory, whose mark changes when
        // a `<package>.build.yaml` override appears or disappears
        // (DF-BUILD-018).
        for node in &package_graph.nodes {
            config_files.push(key(&node.root.join("build.yaml")));
        }
        config_files.push(key(r));
        if let Some(k) = &options.config {
            config_files.push(key(&r.join(format!("build.{k}.yaml"))));
        }
        if let Ok(ls) = std::fs::read_dir(r) {
            for e in ls.flatten() {
                if e.file_name().to_string_lossy().ends_with(".build.yaml") {
                    config_files.push(key(&e.path()));
                }
            }
        }
        config_files.sort();
        config_files.dedup();
        let action_count = graph.actions.len();
        let mut m = Engine {
            root: root.to_path_buf(),
            package_graph,
            configs,
            plan,
            phases,
            targets,
            graph,
            options,
            natives: Vec::new(),
            dart: std::sync::Mutex::new(Box::new(Unavailable::default())),
            dart_prepared: AtomicBool::new(false),
            extensions_checked: false,
            records: vec![None; action_count],
            per_phase,
            package_rounds: HashMap::new(),
            memory: HashMap::new(),
            natural_paths_map: natural_paths,
            expected,
            sources,
            dirs,
            config_files,
            marks: HashMap::new(),
            watched: Vec::new(),
            generation: Arc::new(Generation::default()),
            warned: HashSet::new(),
            labels: HashMap::new(),
            removed: Vec::new(),
            to_restore: None,
            restored_code: None,
            state_dirty: false,
            requested: HashSet::new(),
        };
        m.request_by_filters();
        for c in m.config_files.iter().chain(&m.dirs) {
            m.marks.insert(c.clone(), FileStamp::read(c));
        }
        m.recompute_watched();
        if m.options.persist {
            m.to_restore = crate::persistence::read(&crate::persistence::directory(
                &m.package_graph.root_dir,
            ));
        }
        #[cfg(feature = "native-generators")]
        {
            for g in crate::natives::all() {
                m.register_native(g);
            }
        }
        Ok(m)
    }

    pub fn register_native(&mut self, g: Arc<dyn NativeGenerator>) {
        self.natives.push(g);
    }

    pub fn set_dart_executor(&mut self, e: Box<dyn DartExecutor>) {
        if let Ok(mut previous) = self.dart.lock() {
            previous.shutdown();
        }
        self.dart = std::sync::Mutex::new(e);
        self.dart_prepared.store(false, Ordering::Release);
    }

    pub fn generation(&self) -> Arc<Generation> {
        self.generation.clone()
    }

    /// Natural paths of the expected `.dart` outputs: the first load must
    /// not abort with "could not read" for one of them.
    pub fn expected_outputs(&self) -> &HashSet<PathBuf> {
        &self.expected
    }

    pub fn record(&self, action: usize) -> Option<&ActionRecord> {
        self.records.get(action).and_then(|r| r.as_ref())
    }

    /// Non-Dart inputs the engine watches (the loader already watches the
    /// `.dart` files): the configuration files and what the actions queried
    /// outside memory.
    pub fn watched(&self) -> Vec<PathBuf> {
        self.watched.clone()
    }

    /// Recomputes the watched list (after each update) and records the mark
    /// of whatever just entered it — the state the actions read.
    fn recompute_watched(&mut self) {
        let v = self.compute_watched();
        for p in &v {
            if !self.marks.contains_key(p) {
                self.marks.insert(p.clone(), FileStamp::read(p));
            }
        }
        self.watched = v;
    }

    fn compute_watched(&self) -> Vec<PathBuf> {
        let mut v: BTreeSet<PathBuf> = self
            .config_files
            .iter()
            .chain(&self.dirs)
            .cloned()
            .collect();
        let queries = self
            .records
            .iter()
            .flatten()
            .flat_map(|r| r.queries.iter())
            .chain(self.package_rounds.values().flat_map(|p| p.queries.iter()));
        for (c, _) in queries {
            if let Query::File(p) = c {
                if !p.to_string_lossy().ends_with(".dart")
                    && !self.memory.contains_key(p)
                    && !self.is_fallback_path(p)
                {
                    v.insert(p.clone());
                }
            }
        }
        v.into_iter().collect()
    }

    fn is_fallback_path(&self, p: &Path) -> bool {
        p.starts_with(self.package_graph.root_dir.join(".dart_tool"))
    }

    /// Watched paths whose mark changed since the last look (mtime and size).
    pub fn changes(&mut self) -> Vec<PathBuf> {
        // One `stat` per watched path, in parallel like `AnalyzedUnitCache` (on
        // Windows each `stat` goes through the antivirus).
        let paths = &self.watched;
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(8);
        let now: Vec<Option<FileStamp>> = if paths.len() < 64 || threads < 2 {
            paths.iter().map(|p| FileStamp::read(p)).collect()
        } else {
            let chunk = paths.len().div_ceil(threads);
            std::thread::scope(|s| {
                let hs: Vec<_> = paths
                    .chunks(chunk)
                    .map(|c| s.spawn(move || c.iter().map(|p| FileStamp::read(p)).collect::<Vec<_>>()))
                    .collect();
                hs.into_iter()
                    .flat_map(|h| h.join().unwrap_or_default())
                    .collect()
            })
        };
        let mut v = Vec::new();
        for (p, m) in self.watched.iter().zip(now) {
            match self.marks.get(p) {
                Some(before) if *before == m => {}
                _ => {
                    v.push(p.clone());
                    self.marks.insert(p.clone(), m);
                }
            }
        }
        v
    }

    fn label(&mut self, s: &str) -> &'static str {
        if let Some(r) = self.labels.get(s) {
            return r;
        }
        let r: &'static str = Box::leak(s.to_string().into_boxed_str());
        self.labels.insert(s.to_string(), r);
        r
    }

    pub fn support_path(&self, id: &AssetId, hidden: bool) -> PathBuf {
        if hidden {
            self.package_graph
                .root_dir
                .join(".dart_tool/build/generated")
                .join(id.package.as_ref())
                .join(id.path.as_ref())
        } else {
            self.natural_paths_map.get(id).cloned().unwrap_or_default()
        }
    }

    /// Is the action part of this update? `dart_active`: there is a Dart
    /// executor. A Dart builder may read a `cache` output of an earlier phase
    /// that is not `.dart` (the combining_builder reads the `.g.part` parts by
    /// glob), and the engine does not interrupt an action to compute another
    /// one; so, with the executor, the hidden phases without a native
    /// generator are not lazy — only the optional ones, as in `build_runner`
    /// (`is_optional`).
    fn is_demanded(&self, a: usize, demand: Demand, dart_active: bool) -> bool {
        if self.records[a].is_some() {
            return true;
        }
        if !self.options.filters.is_empty() {
            return self.requested.contains(&a);
        }
        if demand == Demand::Everything {
            return true;
        }
        let fi = self.graph.actions[a].phase;
        let f = &self.phases[fi];
        !f.hidden
            || self.graph.actions[a]
                .outputs
                .iter()
                .any(|s| s.path.ends_with(".dart"))
            || (dart_active && !f.optional && self.native_for_phase(fi).is_none())
    }

    /// The outputs that builder `key` (e.g. `ngdart:ngdart`) does not see
    /// when resolving the program: those of every phase from its first one
    /// on, by natural path ([`dartforge_elements::generated::key`]). In
    /// `build_runner` an action only reads outputs of phases before its own;
    /// a `build_to: source` output already on disk (the `i18n`'s
    /// `messages.i18n.dart`, a phase after ngdart in limitless_ui) does not
    /// exist for ngdart's resolver, and the type that would come from it is
    /// `dynamic`.
    ///
    /// The program is a single one for all packages; the builder's first
    /// phase is what counts, and an output of a phase between the first
    /// package's and another's is hidden from the second as well.
    pub fn outputs_invisible_to(&self, key: &str) -> HashSet<PathBuf> {
        let Some(first) = (0..self.phases.len())
            .find(|&fi| self.plan.applications[self.phases[fi].application].key == key)
        else {
            return HashSet::new();
        };
        self.graph
            .generated
            .iter()
            .filter(|(_, g)| g.phase >= first)
            .map(|(id, _)| {
                dartforge_elements::generated::key(&natural_path(&self.package_graph, id))
            })
            .collect()
    }

    /// The `.dart` inputs of the actions of builder `key` (e.g.
    /// `ngdart:ngdart`), by natural path: the files it generates for —
    /// reachable from `main` or not. `build_runner` resolves each one; the
    /// native generators' program must have all of them (a component nobody
    /// imports also gets a `.template.dart`).
    pub fn inputs_of(&self, key: &str) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = self
            .graph
            .actions
            .iter()
            .filter(|a| a.live())
            .filter(|a| self.plan.applications[self.phases[a.phase].application].key == key)
            .filter(|a| a.input.path.ends_with(".dart"))
            .map(|a| natural_path(&self.package_graph, &a.input))
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// The native generator that covers the phase, if the locked version is
    /// the imitated one and the package is the one published on pub.dev (a
    /// fork with the same version may have another factory: it goes to the
    /// executor, DF-BUILD-022).
    fn native_for_phase(&self, fi: usize) -> Option<usize> {
        let f = &self.phases[fi];
        let key = f
            .equivalent
            .unwrap_or(&self.plan.applications[f.application].key);
        self.natives.iter().position(|g| {
            g.key() == key
                && g.covers(&f.factory)
                && (g.verified() || self.options.measure_unchecked)
                && crate::descriptor::imitates(key).iter().all(|(p, vs)| {
                    self.package_graph
                        .lock
                        .get(*p)
                        .is_some_and(|t| t.is_from_pub_dev() && vs.contains(&t.version.as_str()))
                })
        })
    }

    /// The `pubspec.lock` versions of the packages the native generator `key`
    /// imitates (the ones that pick its mode; see [`crate::descriptor::imitates`]).
    fn imitated_versions(&self, key: &str) -> BTreeMap<String, String> {
        crate::descriptor::imitates(key)
            .iter()
            .filter_map(|(p, _)| {
                let t = self.package_graph.lock.get(*p)?;
                Some(((*p).to_owned(), t.version.clone()))
            })
            .collect()
    }

    /// Did the configuration change (pubspec, lock, build.yaml)? Then the
    /// plan is rebuilt from scratch, as `build_runner` does when the script
    /// changes.
    fn config_changed(&self, changed: &HashSet<PathBuf>) -> bool {
        self.config_files.iter().any(|c| changed.contains(c))
            || changed.iter().any(|m| {
                m.file_name()
                    .is_some_and(|n| n.to_string_lossy().ends_with("build.yaml"))
            })
    }

    /// A file that entered or left (or a watched directory that changed):
    /// the sources, and with them the expected outputs, may have changed.
    fn is_structural(&self, m: &Path) -> bool {
        if self.dirs.iter().any(|d| d == m) {
            return true;
        }
        if self.sources.contains(m) {
            return !m.is_file();
        }
        // A new file in a listed package (outside `.dart_tool` and not a
        // known output written to disk).
        let listed = self.graph.listed.iter().any(|p| {
            self.package_graph.node(p).is_some_and(|n| {
                m.starts_with(&n.root) && !m.starts_with(n.root.join(".dart_tool"))
            })
        });
        listed && m.is_file() && !self.natural_paths_map.values().any(|n| n == m)
    }

    /// Redoes the source listing and the expected outputs with the same
    /// plan; the records of the actions that still exist (same phase and
    /// same input) are kept.
    fn rebuild_graph(&mut self, changed: &mut HashSet<PathBuf>) -> Result<(), String> {
        let graph = Graph::build(&self.package_graph, &self.configs, &self.phases, &self.targets)?;
        let mut old: HashMap<(usize, AssetId), ActionRecord> = HashMap::new();
        for (i, r) in std::mem::take(&mut self.records).into_iter().enumerate() {
            if let Some(r) = r {
                let a = &self.graph.actions[i];
                old.insert((a.phase, a.input.clone()), r);
            }
        }
        let Indices {
            per_phase,
            natural_paths,
            expected,
            sources,
            dirs,
        } = build_indices(&self.package_graph, &graph, self.phases.len());
        self.records = graph
            .actions
            .iter()
            .map(|a| {
                if a.live() {
                    old.remove(&(a.phase, a.input.clone()))
                } else {
                    None
                }
            })
            .collect();
        // Outputs that no longer exist leave memory. A post-processor's
        // outputs are not in the graph: those of the anchors that still exist
        // remain valid.
        self.state_dirty = true;
        let dynamic: Vec<PathBuf> = graph
            .actions
            .iter()
            .zip(&self.records)
            .filter(|(a, _)| a.post_process)
            .filter_map(|(_, r)| r.as_ref())
            .flat_map(|r| {
                r.outputs
                    .iter()
                    .map(|(s, _)| natural_path(&self.package_graph, s))
            })
            .collect();
        let valid: HashSet<&PathBuf> = natural_paths.values().chain(&dynamic).collect();
        let gone: Vec<PathBuf> = self
            .memory
            .keys()
            .filter(|k| !valid.contains(k))
            .cloned()
            .collect();
        for k in gone {
            self.memory.remove(&k);
            self.removed.push(k.clone());
            changed.insert(k);
        }
        for d in &dirs {
            self.marks.entry(d.clone()).or_insert_with(|| FileStamp::read(d));
        }
        self.graph = graph;
        self.per_phase = per_phase;
        self.natural_paths_map = natural_paths;
        self.expected = expected;
        self.sources = sources;
        self.dirs = dirs;
        self.request_by_filters();
        Ok(())
    }

    /// Recomputes whatever is needed and publishes the generation.
    pub fn update(
        &mut self,
        ctx: &UpdateContext<'_>,
        changed: &[PathBuf],
        demand: Demand,
    ) -> Result<Update, String> {
        let mut at = self.update_pass(ctx, changed, demand)?;
        // `--build-filter`: a step that ran read the output of an action that
        // was not requested — the official tool builds it on the spot
        // (`_isReadableNode` → `_runLazyPhaseForInput`). Here it joins the
        // requested ones and the update repeats: the new output wakes up
        // whoever read it.
        while !self.options.filters.is_empty() {
            let new = self.requested_by_reads();
            if new.is_empty() {
                break;
            }
            self.requested.extend(new);
            let more = self.update_pass(ctx, &[], demand)?;
            let mut altered: BTreeSet<PathBuf> = at.altered_outputs.into_iter().collect();
            altered.extend(more.altered_outputs);
            at.altered_outputs = altered.into_iter().collect();
            at.rel.add(more.rel);
            at.rel.changed_outputs = at.altered_outputs.len();
            at.warnings.extend(more.warnings);
            at.generation = more.generation;
        }
        Ok(at)
    }

    /// The actions `--build-filter` requests from the start
    /// (`_matchingPrimaryInputs` with `shouldBuildForDirs`): those of a
    /// non-optional phase with some output that a filter matches and that is
    /// visible in the build (outside the root, only the public assets), and,
    /// for each one, the action that produces its generated input.
    /// Post-processor anchors always run.
    fn request_by_filters(&mut self) {
        self.requested.clear();
        if self.options.filters.is_empty() {
            return;
        }
        let mut stack = Vec::new();
        for (a, action) in self.graph.actions.iter().enumerate() {
            if !action.live() {
                continue;
            }
            let phase = &self.phases[action.phase];
            let requested = action.post_process
                || !phase.optional
                    && action.outputs.iter().any(|s| {
                        self.options.filters.iter().any(|f| f.matches(s)) && self.visible_in_build(s)
                    });
            if requested {
                stack.push(a);
            }
        }
        while let Some(a) = stack.pop() {
            if !self.requested.insert(a) {
                continue;
            }
            if let Some(g) = self.graph.generated.get(&self.graph.actions[a].input) {
                stack.push(g.action);
            }
        }
    }

    /// `isVisibleInBuild`: everything at the root; outside it, the public
    /// assets (`lib/**`, `bin/**`, … and `additional_public_assets`).
    fn visible_in_build(&self, id: &AssetId) -> bool {
        let Some(&p) = self.package_graph.by_name.get(id.package.as_ref()) else {
            return false;
        };
        if p == self.package_graph.root {
            return true;
        }
        crate::graph::VISIBLE_OUTSIDE_ROOT
            .iter()
            .map(|s| s.to_string())
            .chain(
                self.configs.by_package[p]
                    .additional_public
                    .iter()
                    .cloned(),
            )
            .any(|g| crate::glob::Glob::new(&g).is_ok_and(|g| g.matches(&id.path)))
    }

    /// The not yet requested actions whose output a step that ran queried:
    /// by path (`canRead`, a read) or as a glob candidate (`findAssets`),
    /// from a phase before the reader's.
    fn requested_by_reads(&self) -> Vec<usize> {
        let by_natural: HashMap<&Path, &AssetId> = self
            .natural_paths_map
            .iter()
            .map(|(id, n)| (n.as_path(), id))
            .collect();
        let mut new = BTreeSet::new();
        for (a, r) in self.records.iter().enumerate() {
            let Some(r) = r else { continue };
            let phase = self.graph.actions[a].phase;
            let mut request = |id: &AssetId| {
                if let Some(g) = self.graph.generated.get(id)
                    && g.phase < phase
                    && self.records[g.action].is_none()
                    && !self.requested.contains(&g.action)
                {
                    new.insert(g.action);
                }
            };
            for (c, _) in &r.queries {
                match c {
                    Query::ActiveGlobs {
                        dir, candidates, ..
                    } => {
                        for (rel, generated) in candidates {
                            if *generated
                                && let Some(id) = by_natural.get(key(&dir.join(rel)).as_path())
                            {
                                request(id);
                            }
                        }
                    }
                    _ => {
                        if let Some(id) = c.path().and_then(|p| by_natural.get(p)) {
                            request(id);
                        }
                    }
                }
            }
        }
        new.into_iter().collect()
    }

    /// One engine pass over the phases (see [`Engine::update`]).
    fn update_pass(
        &mut self,
        ctx: &UpdateContext<'_>,
        changed: &[PathBuf],
        demand: Demand,
    ) -> Result<Update, String> {
        let t0 = Instant::now();
        // The engine's paths come from `package_config.json` (canonical); an
        // event may arrive under another name for the same file (Windows 8.3
        // short name, link): the canonical form is added as well.
        let mut changed: HashSet<PathBuf> = {
            let mut m = HashSet::with_capacity(changed.len() * 2);
            for p in changed {
                m.insert(key(p));
                if let Some(c) = canonical(p) {
                    m.insert(c);
                }
            }
            m
        };
        if self.config_changed(&changed) {
            let old_memory = std::mem::take(&mut self.memory);
            let options = self.options.clone();
            let natives = std::mem::take(&mut self.natives);
            let dart = std::mem::replace(
                &mut self.dart,
                std::sync::Mutex::new(Box::new(Unavailable::default())),
            );
            // The `package_config.json` reread from disk (a `path`
            // dependency may have moved or changed version): the stored one
            // is from the start of the session. If it is unreadable now, the
            // update fails, as in a new process.
            let file = self
                .package_graph
                .root_dir
                .join(".dart_tool")
                .join("package_config.json");
            let cfg = PackageConfig::load(&file)?;
            *self = Engine::new(&self.root.clone(), &cfg, options)?;
            self.natives = natives;
            self.dart = dart;
            self.dart_prepared.store(false, Ordering::Release);
            // Everything that existed counts as possibly changed.
            for k in old_memory.keys() {
                changed.insert(k.clone());
            }
            self.memory = old_memory;
        }
        if !self.extensions_checked {
            self.check_extensions(&mut changed)?;
        }
        // Events that change listings: a watched directory, a new or deleted
        // file. They also redo the sources and the expected outputs.
        let structural: HashSet<PathBuf> = changed
            .iter()
            .filter(|m| self.is_structural(m))
            .cloned()
            .collect();
        if !structural.is_empty() {
            self.rebuild_graph(&mut changed)?;
        }
        if self.dart_prepared.load(Ordering::Acquire)
            && let Ok(mut d) = self.dart.lock()
        {
            d.new_round();
        }
        let mut rel = EngineReport::default();
        let mut warnings = Vec::new();
        let mut altered: BTreeSet<PathBuf> = self.removed.drain(..).collect();
        let dart_changed = changed
            .iter()
            .any(|p| p.to_string_lossy().ends_with(".dart"));
        let mut packages_done: HashSet<(usize, Arc<str>)> = HashSet::new();
        let mut packages_ran: HashSet<(usize, Arc<str>)> = HashSet::new();
        let dart_reason = self
            .dart
            .lock()
            .map(|d| match d.availability() {
                Availability::Available => None,
                Availability::Unavailable(m) => Some(m),
            })
            .unwrap_or_else(|_| Some("Dart executor poisoned".into()));
        if self.to_restore.is_some() {
            self.apply_restoration(dart_reason.as_deref());
        }

        for fi in 0..self.phases.len() {
            let actions: Vec<usize> = self.per_phase[fi]
                .iter()
                .copied()
                .filter(|&a| self.is_demanded(a, demand, dart_reason.is_none()))
                .collect();
            if actions.is_empty() {
                continue;
            }
            let native = self.native_for_phase(fi);
            // Per-package generator: one round per (generator, package), done
            // (or reused) in the first phase it covers.
            let mut round_ran: Option<bool> = None;
            if let Some(g) = native.filter(|&g| self.natives[g].by_package()) {
                let package: Arc<str> = self.package_graph.nodes[self.phases[fi].package]
                    .name
                    .as_str()
                    .into();
                let k = (g, package.clone());
                if !packages_done.contains(&k) {
                    packages_done.insert(k.clone());
                    if self.run_package(
                        ctx,
                        g,
                        &package,
                        &changed,
                        dart_changed,
                        &structural,
                        &mut rel,
                    )? {
                        packages_ran.insert(k.clone());
                    }
                }
                round_ran = Some(packages_ran.contains(&k));
            }
            // Which actions need to (re)run.
            let mut to_run = Vec::new();
            for &a in &actions {
                rel.checked_actions += 1;
                let dirty = match (&self.records[a], round_ran) {
                    (None, _) => true,
                    // Covered by a package round: redone if the round was.
                    (Some(_), Some(r)) => r,
                    // From the saved state: all queries, and the `source`
                    // outputs on disk.
                    (Some(r), None) if r.restored => {
                        rel.reevaluated_queries += r.queries.len();
                        let (dirty, off_disk) = self.check_restored(a, ctx.db);
                        for n in off_disk {
                            self.memory.remove(&n);
                        }
                        if !dirty && let Some(r) = self.records[a].as_mut() {
                            r.restored = false;
                        }
                        dirty
                    }
                    (Some(r), None) => {
                        // Only the queries the events may have changed.
                        let memory = |p: &Path| self.memory.get(p).cloned();
                        let mut redone = Vec::new();
                        let dirty = r
                            .queries
                            .iter()
                            .enumerate()
                            .filter(|(_, (c, _))| {
                                affected(c, &changed, dart_changed, &structural)
                                    && !self.is_invisible(a, c)
                            })
                            .any(|(i, (c, d))| {
                                rel.reevaluated_queries += 1;
                                if let Query::ActiveGlobs { dir, pattern, .. } = c {
                                    // The graph may have gained (or lost) a
                                    // candidate: the list is redone on the new
                                    // graph and only a different answer
                                    // dirties the action.
                                    if structural.iter().any(|m| m.starts_with(dir)) {
                                        let Ok(g) = crate::glob::Glob::new(pattern) else {
                                            return true;
                                        };
                                        let candidates = glob_candidates(&self.graph, a, &g)
                                            .into_iter()
                                            .map(|id| {
                                                (
                                                    id.path.to_string(),
                                                    self.graph.generated.contains_key(&id),
                                                )
                                            })
                                            .collect();
                                        let redone_query = Query::ActiveGlobs {
                                            dir: dir.clone(),
                                            pattern: pattern.clone(),
                                            candidates,
                                        };
                                        let differs =
                                            query_digest(&redone_query, ctx.db, &memory) != *d;
                                        redone.push((i, redone_query));
                                        return differs;
                                    }
                                }
                                query_digest(c, ctx.db, &memory) != *d
                            });
                        if !dirty && !redone.is_empty() {
                            // The answer is the same, but the candidates are
                            // those of the new graph: a future event on one of
                            // them has to wake the action up.
                            if let Some(r) = self.records[a].as_mut() {
                                for (i, c) in redone {
                                    r.queries[i].0 = c;
                                }
                            }
                        }
                        dirty
                    }
                };
                if dirty {
                    to_run.push(a);
                }
            }
            if to_run.is_empty() {
                continue;
            }
            // Steps that do not run: a generated input that was not written
            // (`wasOutput`) and, with `run_only_if_triggered`, those no
            // trigger fires. The triggered ones keep the trigger's reads (the
            // input and the parts), which decide the next time.
            let mut preset: HashMap<usize, ActionRecord> = HashMap::new();
            let mut from_trigger: HashMap<usize, Queries> = HashMap::new();
            if !self.phases[fi].post_process {
                for &a in &to_run {
                    if let Some(r) = self.unwritten_input(a) {
                        preset.insert(a, r);
                    } else if let Some((fired, queries)) = self.trigger_decision(a) {
                        if fired {
                            from_trigger.insert(a, queries);
                        } else {
                            preset
                                .insert(a, self.without_outputs(a, queries, Origin::NotTriggered));
                        }
                    }
                }
            }
            let run: Vec<usize> = to_run
                .iter()
                .copied()
                .filter(|a| !preset.contains_key(a))
                .collect();
            let ran: Vec<ActionRecord> = match native {
                _ if run.is_empty() => Vec::new(),
                Some(g) if self.natives[g].by_package() => run
                    .iter()
                    .map(|&a| self.package_round_record(g, a, dart_reason.as_deref()))
                    .collect(),
                Some(g) => {
                    let mut v = Vec::with_capacity(run.len());
                    for &a in &run {
                        v.push(self.native_action(ctx, &changed, g, a, dart_reason.as_deref()));
                    }
                    v
                }
                None if dart_reason.is_none() && self.phases[fi].post_process => {
                    self.post_process_serial(&run)
                }
                None if dart_reason.is_none() => {
                    run.iter().map(|&a| self.dart_or_fallback(a)).collect()
                }
                None => self.fallback_in_parallel(&run, dart_reason.as_deref()),
            };
            let mut ran: HashMap<usize, ActionRecord> = run.into_iter().zip(ran).collect();
            let new_records: Vec<ActionRecord> = to_run
                .iter()
                .map(|a| {
                    preset.remove(a).unwrap_or_else(|| {
                        let mut r = ran.remove(a).expect("record of an action that ran");
                        if let Some(c) = from_trigger.remove(a) {
                            r.queries.extend(c);
                        }
                        r
                    })
                })
                .collect();
            for (a, mut r) in to_run.into_iter().zip(new_records) {
                if !matches!(r.origin, Origin::Skipped | Origin::NotTriggered) {
                    rel.executed_actions += 1;
                }
                match r.origin {
                    Origin::Native(_) => rel.native_count += 1,
                    Origin::Dart => rel.dart += 1,
                    Origin::Support => {
                        rel.support += 1;
                        if r.outputs.iter().any(|(_, c)| c.is_some()) {
                            rel.support_with_output += 1;
                        }
                    }
                    Origin::Failure => rel.failures.push(r.reason.clone().unwrap_or_default()),
                    Origin::NotTriggered => rel.not_triggered += 1,
                    Origin::Pending | Origin::Skipped => {}
                }
                if let Some(m) = &r.reason {
                    if r.origin == Origin::Pending {
                        *rel.pending_by_reason.entry(m.clone()).or_default() += 1;
                    }
                }
                // Stale fallback (D-B5).
                if r.origin == Origin::Support {
                    let input = self.input_natural_path(a);
                    for (s, _) in &r.outputs {
                        let loc = self.support_path(s, self.phases[fi].hidden);
                        if is_newer(&input, &loc) && self.warned.insert(loc.clone()) {
                            let msg = format!(
                                "{}: {} may be out of date — {}; run 'dart run build_runner build'",
                                self.plan.applications[self.phases[fi].application].key,
                                s.text(),
                                r.reason
                                    .as_deref()
                                    .unwrap_or("the Dart executor produced no current output")
                            );
                            if self.options.strict {
                                return Err(msg);
                            }
                            warnings.push(msg);
                        }
                    }
                }
                // A post-processor's outputs that the new run did not write
                // disappear (the anchor's `_cleanUpStaleOutputs`).
                if let Some(old) = &self.records[a]
                    && self.graph.actions[a].post_process
                {
                    for (s, _) in &old.outputs {
                        if !r.outputs.iter().any(|(x, _)| x == s) {
                            let n = self.natural_path_of(s);
                            if self.memory.remove(&n).is_some() {
                                altered.insert(n.clone());
                                changed.insert(n);
                            }
                        }
                    }
                }
                // `deletePrimaryInput` (`deletedBy`): the official
                // `FinalizedReader` — `serve` and the merged output directory
                // — stops seeing the input; the builders keep reading it. For
                // a generated input the effect is the same here: it leaves
                // the published generation (`publish`). A deleted source
                // would still be on the disk DartForge serves: a warning, and
                // an error in strict mode.
                let before: Vec<AssetId> = self.records[a]
                    .as_ref()
                    .map(|x| x.deleted_outputs.clone())
                    .unwrap_or_default();
                if before != r.deleted_outputs {
                    for id in before.iter().chain(&r.deleted_outputs) {
                        if self.graph.generated.contains_key(id) {
                            let n = self.natural_path_of(id);
                            altered.insert(n);
                        }
                    }
                }
                for deleted in r
                    .deleted_outputs
                    .iter()
                    .filter(|x| !self.graph.generated.contains_key(*x))
                {
                    let msg = format!(
                        "{}: the source {} was marked by deletePrimaryInput; DartForge has no merged output directory (`build -o`) and does not hide sources in `serve`, so it stays visible",
                        self.plan.applications[self.phases[fi].application].key,
                        deleted.text()
                    );
                    if self.options.strict {
                        return Err(msg);
                    }
                    warnings.push(msg);
                }
                // Output cutoff: only what changed content moves on.
                for (s, c) in &r.outputs {
                    let n = self.natural_path_of(s);
                    let before = self.memory.get(&n).cloned();
                    let equal = match (&before, c) {
                        (Some(x), Some(y)) => x == y,
                        (None, None) => true,
                        _ => false,
                    };
                    if !equal {
                        altered.insert(n.clone());
                        changed.insert(n.clone());
                        match c {
                            Some(c) => {
                                self.memory.insert(n, c.clone());
                            }
                            None => {
                                self.memory.remove(&n);
                            }
                        }
                    }
                }
                let fingerprint = self.fingerprint(a, &r.queries);
                r.fingerprint = fingerprint;
                self.records[a] = Some(r);
                self.state_dirty = true;
            }
        }
        rel.changed_outputs = altered.len();
        if !altered.is_empty() || self.generation.is_empty() {
            self.publish();
        }
        if self.options.persist && self.state_dirty {
            match self.save() {
                Ok(()) => self.state_dirty = false,
                // `crates/build/tests/corpus.rs` matches "not saved".
                Err(e) => warnings.push(format!("engine state not saved: {e}")),
            }
        }
        rel.elapsed_time = t0.elapsed();
        self.recompute_watched();
        Ok(Update {
            generation: self.generation.clone(),
            altered_outputs: altered.into_iter().collect(),
            rel,
            warnings,
        })
    }

    /// The run-time extensions come from the `Builder` object, not from
    /// `build.yaml` (`expected_outputs.dart`). With the Dart executor
    /// available, the engine asks, for each phase that no verified native
    /// generator covers, what they are (`build.extensions`) and, if any of
    /// them differs from the descriptor or from `build.yaml` — one factory of
    /// a builder with several, extensions that depend on the options —,
    /// rebuilds the graph with the `Builder`'s. Once per plan; without an
    /// executor, the predicted ones stay.
    fn check_extensions(&mut self, changed: &mut HashSet<PathBuf>) -> Result<(), String> {
        self.extensions_checked = true;
        let candidates: Vec<usize> = (0..self.phases.len())
            .filter(|&fi| self.phases[fi].extensions.is_some() && self.native_for_phase(fi).is_none())
            .collect();
        let post: Vec<usize> = (0..self.phases.len())
            .filter(|&fi| self.phases[fi].post_process && self.phases[fi].replaced_by.is_none())
            .collect();
        let available = self
            .dart
            .lock()
            .is_ok_and(|d| d.availability() == Availability::Available);
        if (candidates.is_empty() && post.is_empty()) || !available || self.prepare_dart().is_some()
        {
            return Ok(());
        }
        let mut differs = false;
        // Post-processors: the object's `inputExtensions` decide the anchors
        // (`_actionMatches` of a `PostBuildAction`).
        for fi in post {
            let f = &self.phases[fi];
            let key = self.plan.applications[f.application].key.clone();
            let request = ExtensionsRequest {
                key,
                factory: f.factory.clone(),
                options: f.options.clone(),
                root: f.root,
            };
            let answer = self
                .dart
                .lock()
                .map_err(|_| "Dart executor poisoned")?
                .post_inputs(&request);
            let Ok(Some(v)) = answer else { continue };
            if self.phases[fi].post_inputs.as_ref() != Some(&v) {
                self.phases[fi].post_inputs = Some(v);
                differs = true;
            }
        }
        for fi in candidates {
            let f = &self.phases[fi];
            let key = self.plan.applications[f.application].key.clone();
            let request = ExtensionsRequest {
                key: key.clone(),
                factory: f.factory.clone(),
                options: f.options.clone(),
                root: f.root,
            };
            let answer = self
                .dart
                .lock()
                .map_err(|_| "Dart executor poisoned")?
                .extensions(&request);
            // No answer (a factory that fails to instantiate, an executor
            // that does not know): the predicted ones stay, and the action
            // will report the error when it runs.
            let Ok(Some(v)) = answer else { continue };
            if self.phases[fi]
                .extensions
                .as_ref()
                .is_some_and(|e| e.declared != v)
            {
                self.phases[fi].extensions = Some(crate::extensions::Extensions::new_for(&v, &key)?);
                differs = true;
            }
        }
        if differs {
            self.rebuild_graph(changed)?;
        }
        Ok(())
    }

    /// Is the query a file query about an output the action cannot see
    /// because of its phase — of a later phase, or of the same phase and
    /// another action, or its own output (`build_impl.dart:443-463`)? Then
    /// the answer the action got does not depend on the content (it is
    /// "unreadable", or what the action itself wrote) and does not change
    /// while the graph does not change; the digest from disk or memory does
    /// not represent it.
    fn is_invisible(&self, a: usize, c: &Query) -> bool {
        let (Query::File(p) | Query::Exists(p)) = c else {
            return false;
        };
        let phase = self.graph.actions[a].phase;
        let Some(node) = self
            .package_graph
            .nodes
            .iter()
            .filter(|n| !n.root.as_os_str().is_empty() && p.starts_with(&n.root))
            .max_by_key(|n| n.root.as_os_str().len())
        else {
            return false;
        };
        let Ok(rel) = p.strip_prefix(&node.root) else {
            return false;
        };
        let path = rel.to_string_lossy().replace('\\', "/");
        self.graph
            .generated
            .get(&AssetId::new(&node.name, &path))
            .is_some_and(|g| g.phase >= phase)
    }

    /// Global key of the saved state: what no query sees — engine version,
    /// the executable (the native generators' code), the plan, the lock and
    /// the mode.
    fn state_key(&self) -> String {
        let mut h = blake3::Hasher::new();
        h.update(crate::VERSION.as_bytes());
        h.update(crate::persistence::executable_identity().as_bytes());
        h.update(self.plan.canonical_text().as_bytes());
        let mut lock: Vec<_> = self.package_graph.lock.iter().collect();
        lock.sort_by_key(|(name, _)| *name);
        for (name, t) in lock {
            h.update(format!("{name} {} {:?}\n", t.version, t.kind).as_bytes());
        }
        h.update(&[u8::from(self.options.release)]);
        // `--config` and the accumulated `triggers` decide what runs without
        // showing up in any query (the official `buildTriggersDigest`).
        h.update(format!("{:?}", self.options.config).as_bytes());
        h.update(format!("{:?}", self.configs.triggers.by_builder).as_bytes());
        h.finalize().to_hex().to_string()
    }

    /// Key of an action in the saved state: the phase (with the extensions
    /// and the post-processor inputs already checked), the options, the
    /// input and the predicted outputs.
    fn action_key(&self, a: usize) -> String {
        let action = &self.graph.actions[a];
        let f = &self.phases[action.phase];
        let mut h = blake3::Hasher::new();
        h.update(f.identity(&self.plan, &self.package_graph).as_bytes());
        h.update(f.options.canonical_text().as_bytes());
        h.update(&[u8::from(f.root), u8::from(action.post_process)]);
        h.update(action.input.text().as_bytes());
        for s in &action.outputs {
            h.update(b"\n");
            h.update(s.text().as_bytes());
        }
        h.finalize().to_hex().to_string()
    }

    /// The Dart builders' code (files of the bootstrap's depfile) outside the
    /// `hosted` packages, which the lock already pins: file and content
    /// digest. `None` if it is unknown (the executor did not compile the
    /// bootstrap in this session and nothing was restored) or if one of them
    /// disappeared.
    fn dart_code(&self) -> Option<Vec<(PathBuf, Digest)>> {
        let list = self
            .dart
            .lock()
            .ok()
            .and_then(|d| d.code())
            .or_else(|| self.restored_code.clone())?;
        let hosted: Vec<&Path> = self
            .package_graph
            .nodes
            .iter()
            .filter(|n| {
                n.kind == crate::config::DependencyType::Hosted && !n.root.as_os_str().is_empty()
            })
            .map(|n| n.root.as_path())
            .collect();
        let mut v = Vec::new();
        for p in list {
            if hosted.iter().any(|h| p.starts_with(h)) {
                continue;
            }
            v.push((p.clone(), crate::query::digest_file(&p)?));
        }
        v.sort();
        Some(v)
    }

    /// Can the record go into the saved state (§4.1)?
    fn is_persistable(&self, a: usize, r: &ActionRecord) -> bool {
        let fi = self.graph.actions[a].phase;
        !matches!(r.origin, Origin::Pending)
            && r.measured.is_empty()
            && self
                .native_for_phase(fi)
                .is_none_or(|g| !self.natives[g].by_package())
            && r.queries
                .iter()
                .all(|(c, _)| crate::persistence::persistable(c))
    }

    /// Writes the state: the persistable actions and the outputs' content.
    fn save(&mut self) -> Result<(), String> {
        let has_dart = self
            .records
            .iter()
            .flatten()
            .any(|r| r.origin == Origin::Dart);
        let dart_code = if has_dart { self.dart_code() } else { None };
        let mut actions = Vec::new();
        let mut contents = Vec::new();
        for (a, r) in self.records.iter().enumerate() {
            let Some(r) = r else { continue };
            if !self.is_persistable(a, r) {
                continue;
            }
            let origin = match r.origin {
                Origin::Native(n) => crate::persistence::SavedOrigin::Native(n.to_string()),
                Origin::Dart if dart_code.is_some() => crate::persistence::SavedOrigin::Dart,
                Origin::Support => crate::persistence::SavedOrigin::Support,
                Origin::Skipped if self.graph.actions[a].post_process => {
                    crate::persistence::SavedOrigin::Skipped
                }
                Origin::Dart
                | Origin::Pending
                | Origin::Failure
                | Origin::Skipped
                | Origin::NotTriggered => continue,
            };
            let outputs = r
                .outputs
                .iter()
                .map(|(id, c)| {
                    let d = c.as_ref().map(|c| {
                        let d = digest_bytes(c);
                        contents.push((d, c.clone()));
                        d
                    });
                    (id.clone(), d)
                })
                .collect();
            actions.push(crate::persistence::SavedAction {
                key: self.action_key(a),
                origin,
                reason: r.reason.clone(),
                queries: r.queries.clone(),
                outputs,
                from_support: r.from_fallback.clone(),
                deleted_outputs: r.deleted_outputs.clone(),
            });
        }
        let state = crate::persistence::State {
            key: self.state_key(),
            dart_code,
            actions,
        };
        crate::persistence::write_state(
            &crate::persistence::directory(&self.package_graph.root_dir),
            &state,
            &contents,
        )
    }

    /// Applies the state read at startup: each saved action whose key still
    /// exists and whose origin is still the one this engine would choose
    /// comes back as a `restored` record, with its outputs in memory. Its
    /// validity is checked on the first verification of each one
    /// ([`Engine::check_restored`]).
    fn apply_restoration(&mut self, dart_reason: Option<&str>) {
        let Some(state) = self.to_restore.take() else {
            return;
        };
        if state.key != self.state_key() {
            return;
        }
        let dir = crate::persistence::directory(&self.package_graph.root_dir);
        let dart_valid = state.dart_code.as_ref().is_some_and(|l| {
            l.iter()
                .all(|(p, d)| crate::query::digest_file(p).as_ref() == Some(d))
        });
        if dart_valid {
            self.restored_code = state
                .dart_code
                .as_ref()
                .map(|l| l.iter().map(|(p, _)| p.clone()).collect());
        }
        let by_key: HashMap<String, usize> = (0..self.graph.actions.len())
            .filter(|&a| self.graph.actions[a].live() && self.records[a].is_none())
            .map(|a| (self.action_key(a), a))
            .collect();
        for s in state.actions {
            let Some(&a) = by_key.get(&s.key) else {
                continue;
            };
            let fi = self.graph.actions[a].phase;
            let native = self.native_for_phase(fi);
            let origin = match &s.origin {
                crate::persistence::SavedOrigin::Native(n) => match native {
                    Some(g) if self.natives[g].key() == n && !self.natives[g].by_package() => {
                        Origin::Native(self.natives[g].key())
                    }
                    _ => continue,
                },
                crate::persistence::SavedOrigin::Dart if dart_valid && native.is_none() => {
                    Origin::Dart
                }
                crate::persistence::SavedOrigin::Support
                    if native.is_none() && dart_reason.is_some() =>
                {
                    Origin::Support
                }
                crate::persistence::SavedOrigin::Skipped if self.graph.actions[a].post_process => {
                    Origin::Skipped
                }
                _ => continue,
            };
            let mut outputs = Vec::with_capacity(s.outputs.len());
            for (id, d) in &s.outputs {
                match d {
                    None => outputs.push((id.clone(), None)),
                    Some(d) => match crate::persistence::read_blob(&dir, d) {
                        Some(b) => outputs.push((id.clone(), Some(b))),
                        None => break,
                    },
                }
            }
            if outputs.len() != s.outputs.len() {
                continue;
            }
            for (id, c) in &outputs {
                if let Some(c) = c {
                    let n = self.natural_path_of(id);
                    self.memory.insert(n, c.clone());
                }
            }
            let fingerprint = self.fingerprint(a, &s.queries);
            self.records[a] = Some(ActionRecord {
                fingerprint,
                queries: s.queries,
                outputs,
                origin,
                reason: s.reason,
                measured: Vec::new(),
                from_fallback: s.from_support,
                deleted_outputs: s.deleted_outputs,
                restored: true,
            });
        }
    }

    /// Checks a restored record: all queries (a `ActiveGlobs` with the
    /// candidates of the current graph) and, for a `source` output of a
    /// native generator or of the Dart executor, the file on disk, which the
    /// CLI only rewrites when the content changes. Returns whether it is
    /// dirty and the output paths that differ from disk (memory forgets them,
    /// so that the rerun counts them as changed and rewrites them).
    fn check_restored(&self, a: usize, database: &dyn SemanticDb) -> (bool, Vec<PathBuf>) {
        let Some(r) = self.records[a].as_ref() else {
            return (true, Vec::new());
        };
        let memory = |p: &Path| self.memory.get(p).cloned();
        let dirty = r
            .queries
            .iter()
            .filter(|(c, _)| !self.is_invisible(a, c))
            .any(|(c, d)| {
                let current = match c {
                    Query::ActiveGlobs { dir, pattern, .. } => {
                        let Ok(g) = crate::glob::Glob::new(pattern) else {
                            return true;
                        };
                        let candidates = glob_candidates(&self.graph, a, &g)
                            .into_iter()
                            .map(|id| {
                                (id.path.to_string(), self.graph.generated.contains_key(&id))
                            })
                            .collect();
                        query_digest(
                            &Query::ActiveGlobs {
                                dir: dir.clone(),
                                pattern: pattern.clone(),
                                candidates,
                            },
                            database,
                            &memory,
                        )
                    }
                    _ => query_digest(c, database, &memory),
                };
                current != *d
            });
        let mut off_disk = Vec::new();
        let f = &self.phases[self.graph.actions[a].phase];
        if !f.hidden && matches!(r.origin, Origin::Native(_) | Origin::Dart) {
            for (s, c) in &r.outputs {
                let n = self.natural_path_of(s);
                if let Some(c) = c
                    && std::fs::read(&n).ok().as_deref() != Some(&c[..])
                {
                    off_disk.push(n);
                }
            }
        }
        (dirty || !off_disk.is_empty(), off_disk)
    }

    /// Natural path of an output: the one the graph predicted or, for a
    /// post-processor output, the asset's path in the package.
    fn natural_path_of(&self, s: &AssetId) -> PathBuf {
        self.natural_paths_map
            .get(s)
            .cloned()
            .unwrap_or_else(|| natural_path(&self.package_graph, s))
    }

    /// The post-processing anchors of a phase, in series and in graph order:
    /// each one sees the others' outputs as "already existing" (the official
    /// `addAsset` rejects an asset the graph already has).
    fn post_process_serial(&self, to_run: &[usize]) -> Vec<ActionRecord> {
        let mut owners: BTreeMap<AssetId, usize> = BTreeMap::new();
        for (a, (action, r)) in self.graph.actions.iter().zip(&self.records).enumerate() {
            if let (true, Some(r)) = (action.post_process, r) {
                for (s, _) in &r.outputs {
                    owners.insert(s.clone(), a);
                }
            }
        }
        let mut v = Vec::with_capacity(to_run.len());
        for &a in to_run {
            let occupied = owners
                .iter()
                .filter(|(_, d)| **d != a)
                .map(|(s, _)| s.clone())
                .collect();
            let r = self
                .post_process_action(a, occupied)
                .unwrap_or_else(|e| ActionRecord {
                    fingerprint: [0; 32],
                    queries: Vec::new(),
                    outputs: Vec::new(),
                    origin: Origin::Pending,
                    reason: Some(e),
                    measured: Vec::new(),
                    from_fallback: Vec::new(),
                    deleted_outputs: Vec::new(),
                    restored: false,
                });
            owners.retain(|_, d| *d != a);
            for (s, _) in &r.outputs {
                owners.insert(s.clone(), a);
            }
            v.push(r);
        }
        v
    }

    /// `_runPostProcessBuilderForAnchor`: a generated input is only processed
    /// if it was written (`wasOutput`, without failure); then the
    /// post-processor runs on the Dart executor with the
    /// `PostProcessBuildStep` served by the engine.
    ///
    /// The error messages start with "post-processor":
    /// `crates/build/tests/corpus.rs` matches that prefix on the pending
    /// reasons.
    fn post_process_action(
        &self,
        a: usize,
        occupied: BTreeSet<AssetId>,
    ) -> Result<ActionRecord, String> {
        let action = self.graph.actions.get(a).ok_or("nonexistent anchor")?;
        if self.graph.generated.contains_key(&action.input) {
            let n = self.natural_path_of(&action.input);
            if !self.memory.contains_key(&n) {
                // The negative query wakes the anchor up when the input is
                // written.
                return Ok(ActionRecord {
                    fingerprint: [0; 32],
                    queries: vec![(Query::File(n), None)],
                    outputs: Vec::new(),
                    origin: Origin::Skipped,
                    reason: None,
                    measured: Vec::new(),
                    from_fallback: Vec::new(),
                    deleted_outputs: Vec::new(),
                    restored: false,
                });
            }
        }
        if let Some(e) = self.prepare_dart() {
            return Err(e);
        }
        let phase = &self.phases[action.phase];
        let memory = |id: &AssetId| {
            self.natural_paths_map
                .get(id)
                .and_then(|p| self.memory.get(p))
                .cloned()
        };
        let mut service = ActionService::new(&self.graph, &self.package_graph, a, &memory);
        service.occupied = occupied;
        let request = ActionRequest {
            phase: action.phase,
            key: self.plan.applications[phase.application].key.clone(),
            factory: phase.factory.clone(),
            options: phase.options.clone(),
            root: phase.root,
            input: action.input.clone(),
            allowed_outputs: Vec::new(),
        };
        let mut dart = self.dart.lock().map_err(|_| "Dart executor poisoned")?;
        let result = dart
            .post_process_step(&request, &mut service)
            .map_err(|e| e.0)?;
        if result.failed {
            // "severe" and "error" are log levels of the dfexec/1 protocol.
            let details = result
                .logs
                .iter()
                .filter(|(level, _)| level == "severe" || level == "error")
                .map(|(_, message)| message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(if details.is_empty() {
                format!("{}: Dart post-processor failed", request.key)
            } else {
                format!("{}: {details}", request.key)
            });
        }
        for (id, bytes) in result.outputs {
            if service
                .writes
                .get(&id)
                .is_some_and(|b| b.as_ref() != bytes.as_ref())
            {
                return Err(format!(
                    "post-processor (Dart) returned different bytes for {}",
                    id.text()
                ));
            }
            if !service.writes.contains_key(&id) {
                service.write(&id, bytes).map_err(|e| {
                    format!(
                        "post-processor wrote an asset that already exists: {}",
                        e.0.text()
                    )
                })?;
            }
        }
        if let Some(x) = result.deleted_outputs.iter().find(|x| **x != action.input) {
            return Err(format!(
                "post-processor deleted {}, which is not the primary input",
                x.text()
            ));
        }
        let outputs = service
            .writes
            .into_iter()
            .map(|(id, b)| (id, Some(b)))
            .collect();
        // The anchor depends on the digest of the primary input even if the
        // post-processor does not read it (`_postProcessBuildShouldRun`
        // compares the `previousInputsDigest`): `part_cleanup` only deletes
        // it.
        let n = self.natural_path_of(&action.input);
        let d = self
            .memory
            .get(&n)
            .map(|b| digest_bytes(b))
            .or_else(|| crate::query::digest_file(&n));
        let mut queries = vec![(Query::File(n), d)];
        queries.extend(service.queries);
        Ok(ActionRecord {
            fingerprint: [0; 32],
            queries,
            outputs,
            origin: Origin::Dart,
            reason: None,
            measured: Vec::new(),
            from_fallback: Vec::new(),
            deleted_outputs: result.deleted_outputs,
            restored: false,
        })
    }

    /// Record of a step that does not run: no output written, only the
    /// queries that decided so.
    fn without_outputs(
        &self,
        a: usize,
        queries: Vec<(Query, Option<Digest>)>,
        origin: Origin,
    ) -> ActionRecord {
        ActionRecord {
            fingerprint: [0; 32],
            queries,
            outputs: self.graph.actions[a]
                .outputs
                .iter()
                .map(|s| (s.clone(), None))
                .collect(),
            origin,
            reason: None,
            measured: Vec::new(),
            from_fallback: Vec::new(),
            deleted_outputs: Vec::new(),
            restored: false,
        }
    }

    /// The primary input is an output of an earlier phase that was not
    /// written (or failed): the step does not run (`_matchingPrimaryInputs`:
    /// `if (!input.wasOutput) return`; in 2.16.1, `skipMissingPrimaryInput`).
    /// The negative query wakes it up when the input is written.
    fn unwritten_input(&self, a: usize) -> Option<ActionRecord> {
        let input = &self.graph.actions[a].input;
        let g = self.graph.generated.get(input)?;
        // Only when whoever produces the input actually ran: without an
        // executor (the action pending, or read from the fallback) it is
        // unknown what the builder would write, and the step follows the
        // fallback policy.
        let known = self.records[g.action].as_ref().is_some_and(|r| {
            matches!(
                r.origin,
                Origin::Dart
                    | Origin::Native(_)
                    | Origin::Failure
                    | Origin::NotTriggered
                    | Origin::Skipped
            )
        });
        if !known {
            return None;
        }
        let n = self.natural_path_of(input);
        if self.memory.contains_key(&n) {
            return None;
        }
        Some(self.without_outputs(a, vec![(Query::File(n), None)], Origin::Skipped))
    }

    /// `_allowedByTriggers`: `None` if the phase does not have
    /// `run_only_if_triggered: true` (or the profile has no triggers);
    /// otherwise, whether some trigger of the builder fires, with the reads
    /// the decision made (the input and, for an annotation trigger, the parts
    /// readable by the phase — including those generated by earlier phases).
    fn trigger_decision(&self, a: usize) -> Option<(bool, Queries)> {
        let action = &self.graph.actions[a];
        let phase = &self.phases[action.phase];
        if !phase.only_if_triggered {
            return None;
        }
        let key = &self.plan.applications[phase.application].key;
        let short_name = self.package_graph.profile().trigger_by_short_name();
        let Some(triggers) = self.configs.triggers.for_builder(key, short_name) else {
            return Some((false, Vec::new()));
        };
        let memory = |id: &AssetId| {
            self.natural_paths_map
                .get(id)
                .and_then(|p| self.memory.get(p))
                .cloned()
        };
        let mut service = ActionService::new(&self.graph, &self.package_graph, a, &memory);
        let primary = service
            .read(&action.input)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default();
        let fired = crate::triggers::triggered(triggers, &primary, |uri| {
            let id = resolve_uri(uri, &action.input)?;
            if !service.can_read(&id) {
                return None;
            }
            service
                .read(&id)
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        });
        Some((fired, service.queries))
    }

    fn input_natural_path(&self, a: usize) -> PathBuf {
        natural_path(&self.package_graph, &self.graph.actions[a].input)
    }

    fn prepare_dart(&self) -> Option<String> {
        let Ok(mut dart) = self.dart.lock() else {
            return Some("Dart executor poisoned".into());
        };
        match dart.availability() {
            Availability::Unavailable(m) => return Some(m),
            Availability::Available if self.dart_prepared.load(Ordering::Acquire) => {
                return None;
            }
            Availability::Available => {}
        }
        // Only what can become a Dart action (builders and post-processors):
        // the replaced ones never run (§6 of BUILD-MOTOR.md); importing them
        // would only make the script's compilation bigger.
        let applications = self
            .plan
            .applications
            .iter()
            .filter(|a| crate::descriptor::replaced_by(&a.key).is_none())
            .map(|a| (a.key.clone(), a.import.clone(), a.factories.clone()))
            .collect();
        let mut h = blake3::Hasher::new();
        h.update(crate::VERSION.as_bytes());
        h.update(self.plan.canonical_text().as_bytes());
        let mut lock: Vec<_> = self.package_graph.lock.iter().collect();
        lock.sort_by_key(|(name, _)| *name);
        for (name, package) in lock {
            h.update(name.as_bytes());
            h.update(package.version.as_bytes());
            h.update(format!("{:?}", package.kind).as_bytes());
        }
        let script = BuilderScript {
            applications,
            cache_key: h.finalize().to_hex().to_string(),
        };
        match dart.prepare(&script) {
            Ok(()) => {
                self.dart_prepared.store(true, Ordering::Release);
                None
            }
            Err(e) => Some(e.0),
        }
    }

    /// The action through the Dart executor; without an executor, the
    /// fallback. A failure of the builder (exception, severe error,
    /// disallowed output) or of the executor in the middle of the action is a
    /// generation failure, never `build_runner`'s old result (DF-BUILD-005).
    fn dart_or_fallback(&self, a: usize) -> ActionRecord {
        match self.dart_action(a) {
            Ok(r) => r,
            Err(DartFailure::Builder(e)) => self.failure(a, e),
            Err(DartFailure::Unavailable(e)) => self.fallback(a, Some(&e)),
        }
    }

    /// The record of an action that failed: no output (the old one, from
    /// disk or memory, does not count), the failure as the reason.
    fn failure(&self, a: usize, e: String) -> ActionRecord {
        let action = &self.graph.actions[a];
        let input = natural_path(&self.package_graph, &action.input);
        let d = self
            .memory
            .get(&input)
            .map(|b| digest_bytes(b))
            .or_else(|| crate::query::digest_file(&input));
        ActionRecord {
            fingerprint: [0; 32],
            queries: vec![(Query::File(input), d)],
            outputs: action.outputs.iter().map(|s| (s.clone(), None)).collect(),
            origin: Origin::Failure,
            reason: Some(e),
            measured: Vec::new(),
            from_fallback: Vec::new(),
            deleted_outputs: Vec::new(),
            restored: false,
        }
    }

    fn dart_action(&self, a: usize) -> Result<ActionRecord, DartFailure> {
        if let Some(e) = self.prepare_dart() {
            return Err(DartFailure::Unavailable(e));
        }
        let action = self
            .graph
            .actions
            .get(a)
            .ok_or(DartFailure::Unavailable("nonexistent Dart action".into()))?;
        let phase = &self.phases[action.phase];
        let memory = |id: &AssetId| {
            self.natural_paths_map
                .get(id)
                .and_then(|p| self.memory.get(p))
                .cloned()
        };
        let mut service = ActionService::new(&self.graph, &self.package_graph, a, &memory);
        let request = ActionRequest {
            phase: action.phase,
            key: self.plan.applications[phase.application].key.clone(),
            factory: phase.factory.clone(),
            options: phase.options.clone(),
            root: phase.root,
            input: action.input.clone(),
            allowed_outputs: action.outputs.clone(),
        };
        let mut dart = self
            .dart
            .lock()
            .map_err(|_| DartFailure::Unavailable("Dart executor poisoned".into()))?;
        // The executor crashed in the middle of the action: a failure, not a
        // missing executor.
        let result = dart.execute(&request, &mut service).map_err(|e| {
            DartFailure::Builder(format!("{}: executor failure: {}", request.key, e.0))
        })?;
        if result.failed {
            // "severe" and "error" are log levels of the dfexec/1 protocol.
            let details = result
                .logs
                .iter()
                .filter(|(level, _)| level == "severe" || level == "error")
                .map(|(_, message)| message.as_str())
                .chain(
                    service
                        .logs
                        .iter()
                        .filter(|(level, _)| *level == crate::executor::Level::Severe)
                        .map(|(_, message)| message.as_str()),
                )
                .collect::<Vec<_>>()
                .join("; ");
            return Err(DartFailure::Builder(if details.is_empty() {
                format!("{}: Dart builder failed", request.key)
            } else {
                format!("{}: {details}", request.key)
            }));
        }
        for (id, bytes) in result.outputs {
            if let Some(already_written) = service.writes.get(&id) {
                if already_written.as_ref() != bytes.as_ref() {
                    return Err(DartFailure::Builder(format!(
                        "Dart builder returned different bytes for {} after writing through BuildStep",
                        id.text()
                    )));
                }
            }
            service.write(&id, bytes).map_err(|e| {
                DartFailure::Builder(format!(
                    "Dart builder wrote a disallowed output: {}",
                    e.0.text()
                ))
            })?;
        }
        let outputs = action
            .outputs
            .iter()
            .map(|id| (id.clone(), service.writes.get(id).cloned()))
            .collect();
        Ok(ActionRecord {
            fingerprint: [0; 32],
            queries: service.queries,
            outputs,
            origin: Origin::Dart,
            reason: None,
            measured: Vec::new(),
            from_fallback: Vec::new(),
            deleted_outputs: Vec::new(),
            restored: false,
        })
    }

    /// `blake3(version ‖ key ‖ factory ‖ imitated ‖ options ‖ identity ‖
    /// input ‖ queries)`.
    fn fingerprint(&self, a: usize, queries: &[(Query, Option<Digest>)]) -> Digest {
        let action = &self.graph.actions[a];
        let f = &self.phases[action.phase];
        let ap = &self.plan.applications[f.application];
        let mut h = blake3::Hasher::new();
        h.update(crate::VERSION.as_bytes());
        h.update(ap.key.as_bytes());
        h.update(f.factory.as_bytes());
        for (p, v) in self.imitated_versions(&ap.key) {
            h.update(p.as_bytes());
            h.update(v.as_bytes());
        }
        h.update(f.options.canonical_text().as_bytes());
        h.update(&[u8::from(f.root)]);
        h.update(f.identity(&self.plan, &self.package_graph).as_bytes());
        h.update(action.input.text().as_bytes());
        for (c, d) in queries {
            h.update(format!("{c:?}").as_bytes());
            h.update(d.as_ref().map(|d| &d[..]).unwrap_or(b"-"));
        }
        *h.finalize().as_bytes()
    }

    /// Runs (or reuses) the per-package generator `g` over `package`.
    /// Returns whether it actually ran.
    fn run_package(
        &mut self,
        ctx: &UpdateContext<'_>,
        g: usize,
        package: &Arc<str>,
        changed: &HashSet<PathBuf>,
        dart_changed: bool,
        structural: &HashSet<PathBuf>,
        rel: &mut EngineReport,
    ) -> Result<bool, String> {
        let k = (g, package.clone());
        if let Some(r) = self.package_rounds.get(&k) {
            let maybe = r
                .queries
                .iter()
                .any(|(c, _)| affected(c, changed, dart_changed, structural));
            if !maybe {
                return Ok(false);
            }
            // Only the queries the events may have changed are redone.
            let t = Instant::now();
            let memory = |p: &Path| self.memory.get(p).cloned();
            let mut unchanged = true;
            for (c, d) in r
                .queries
                .iter()
                .filter(|(c, _)| affected(c, changed, dart_changed, structural))
            {
                rel.reevaluated_queries += 1;
                if query_digest(c, ctx.db, &memory) != *d {
                    unchanged = false;
                    break;
                }
            }
            rel.revalidation_time += t.elapsed();
            if unchanged {
                return Ok(false);
            }
        }
        let t_generate = Instant::now();
        let generator = self.natives[g].clone();
        // All the actions of the phases the generator covers in the package.
        let mut actions = Vec::new();
        for (fi, f) in self.phases.iter().enumerate() {
            if self.package_graph.nodes[f.package].name != **package
                || self.native_for_phase(fi) != Some(g)
            {
                continue;
            }
            for &a in &self.per_phase[fi] {
                let action = &self.graph.actions[a];
                actions.push(NativeAction {
                    factory: f.factory.clone(),
                    input: action.input.clone(),
                    natural_input: natural_path(&self.package_graph, &action.input),
                    outputs: action
                        .outputs
                        .iter()
                        .map(|s| (s.clone(), self.natural_paths_map[s].clone()))
                        .collect(),
                    options: f.options.clone(),
                });
            }
        }
        let node = self.package_graph.node(package).ok_or("unknown package")?;
        let request = NativeRequest {
            package: package.to_string(),
            package_root: node.root.clone(),
            actions,
            roots: self.package_roots(),
            versions: self.imitated_versions(generator.key()),
        };
        let (result, queries) = {
            let memory = |p: &Path| self.memory.get(p).cloned();
            let mut c = GeneratorCtx::new(ctx.program, changed, ctx.db, &memory);
            let r = generator.generate(&mut c, &request);
            (r, std::mem::take(&mut c.queries))
        };
        rel.generator_queries += queries.len();
        let round = match result {
            Ok(s) => PackageRound {
                queries: if s.reuse_queries {
                    let previous = self
                        .package_rounds
                        .get(&k)
                        .ok_or("ngdart: previous queries missing")?;
                    let mut kept = previous.queries.clone();
                    for (new, digest) in queries {
                        if let Some((_, old)) = kept.iter_mut().find(|(c, _)| *c == new) {
                            *old = digest;
                        } else {
                            kept.push((new, digest));
                        }
                    }
                    kept
                } else {
                    queries
                },
                outputs: s
                    .outputs
                    .into_iter()
                    .map(|(p, b)| (key(&p), Arc::from(b)))
                    .collect(),
                refusals: s.refusals.into_iter().map(|(p, m)| (key(&p), m)).collect(),
                error: None,
                generated_units: s.generated_unit_count,
            },
            Err(e) => PackageRound {
                queries,
                outputs: BTreeMap::new(),
                refusals: BTreeMap::new(),
                error: Some(e),
                generated_units: 0,
            },
        };
        rel.native_unit_count += round.generated_units;
        self.package_rounds.insert(k, round);
        rel.native_time += t_generate.elapsed();
        Ok(true)
    }

    /// Record of an action covered by a per-package generator: the generated
    /// outputs come from the round; whatever it refused goes to the
    /// fallback.
    fn package_round_record(&self, g: usize, a: usize, dart_reason: Option<&str>) -> ActionRecord {
        let action = &self.graph.actions[a];
        let package: Arc<str> = self.package_graph.nodes[self.phases[action.phase].package]
            .name
            .as_str()
            .into();
        let round = self.package_rounds.get(&(g, package));
        let generator = &self.natives[g];
        let outputs: Vec<(AssetId, Option<Arc<[u8]>>)> = action
            .outputs
            .iter()
            .map(|s| {
                (
                    s.clone(),
                    round
                        .and_then(|r| r.outputs.get(&self.natural_paths_map[s]))
                        .cloned(),
                )
            })
            .collect();
        let input = natural_path(&self.package_graph, &action.input);
        // The queries stay in the package round (that is what gets
        // revalidated); copying them into each action cost hundreds of ms per
        // edit.
        let queries = Vec::new();
        // The native generator covers the action when it generated some output
        // of it and did not refuse it.
        let refusal =
            round.and_then(|r| r.error.clone().or_else(|| r.refusals.get(&input).cloned()));
        let generated_any = outputs.iter().any(|(_, c)| c.is_some());
        if generator.verified() && refusal.is_none() && generated_any {
            if outputs.iter().any(|(_, c)| c.is_none()) && dart_reason.is_none() {
                match self.dart_action(a) {
                    Ok(r) => return r,
                    Err(DartFailure::Builder(e)) => return self.failure(a, e),
                    Err(DartFailure::Unavailable(_)) => {}
                }
            }
            // An output the native generator does not write and the official
            // one does (the `.css.dart` next to the `.css.shim.dart`) comes
            // from the fallback and is marked as such.
            let mut outputs = outputs;
            let mut from_fallback = Vec::new();
            let hidden = self.phases[action.phase].hidden;
            for (s, c) in outputs.iter_mut() {
                if c.is_none() {
                    if let Ok(b) = std::fs::read(self.support_path(s, hidden)) {
                        *c = Some(Arc::from(b));
                        from_fallback.push(s.clone());
                    }
                }
            }
            return ActionRecord {
                fingerprint: [0; 32],
                queries,
                outputs,
                origin: Origin::Native(generator.key()),
                reason: None,
                measured: Vec::new(),
                from_fallback,
                deleted_outputs: Vec::new(),
                restored: false,
            };
        }
        // The fallback of an action that already came from it is not reread:
        // `build_runner`'s disk does not change under the session (declared
        // limitation: running it separately requires restarting the
        // session).
        if dart_reason.is_none() {
            match self.dart_action(a) {
                Ok(r) => return r,
                Err(DartFailure::Builder(e)) => return self.failure(a, e),
                Err(DartFailure::Unavailable(_)) => {}
            }
        }
        let mut r = match &self.records[a] {
            Some(prev) if prev.origin == Origin::Support => {
                let mut r = prev.clone();
                r.reason = dart_reason.map(|m| format!("build_runner fallback: {m}"));
                r
            }
            _ => self.fallback(a, dart_reason),
        };
        if !generator.verified() {
            r.measured = outputs
                .into_iter()
                .filter_map(|(s, c)| c.map(|c| (s, c)))
                .collect();
        }
        let native_reason = refusal
            .unwrap_or_else(|| format!("{}: does not generate this output", generator.key()));
        r.reason = Some(match r.reason.take() {
            Some(m) => format!("{native_reason}; {m}"),
            None => native_reason,
        });
        r
    }

    /// Name → root of each package in the graph.
    fn package_roots(&self) -> BTreeMap<String, PathBuf> {
        self.package_graph
            .nodes
            .iter()
            .map(|n| (n.name.clone(), n.root.clone()))
            .collect()
    }

    fn native_action(
        &self,
        ctx: &UpdateContext<'_>,
        changed: &HashSet<PathBuf>,
        g: usize,
        a: usize,
        dart_reason: Option<&str>,
    ) -> ActionRecord {
        let action = &self.graph.actions[a];
        let f = &self.phases[action.phase];
        let generator = self.natives[g].clone();
        let request = NativeRequest {
            package: action.input.package.to_string(),
            package_root: self.package_graph.nodes[f.package].root.clone(),
            actions: vec![NativeAction {
                factory: f.factory.clone(),
                input: action.input.clone(),
                natural_input: natural_path(&self.package_graph, &action.input),
                outputs: action
                    .outputs
                    .iter()
                    .map(|s| (s.clone(), self.natural_paths_map[s].clone()))
                    .collect(),
                options: f.options.clone(),
            }],
            roots: self.package_roots(),
            versions: self.imitated_versions(generator.key()),
        };
        let memory = |p: &Path| self.memory.get(p).cloned();
        let mut c = GeneratorCtx::new(ctx.program, changed, ctx.db, &memory);
        let res = generator.generate(&mut c, &request);
        let queries = std::mem::take(&mut c.queries);
        match res {
            Ok(s)
                if generator.verified()
                    && s.refusals.is_empty()
                    && !s.outputs.is_empty()
                    && (dart_reason.is_some()
                        || action
                            .outputs
                            .iter()
                            .all(|id| s.outputs.contains_key(&self.natural_paths_map[id]))) =>
            {
                ActionRecord {
                    fingerprint: [0; 32],
                    queries,
                    outputs: action
                        .outputs
                        .iter()
                        .map(|id| {
                            (
                                id.clone(),
                                s.outputs
                                    .get(&self.natural_paths_map[id])
                                    .map(|b| Arc::from(b.clone())),
                            )
                        })
                        .collect(),
                    origin: Origin::Native(generator.key()),
                    reason: None,
                    measured: Vec::new(),
                    from_fallback: Vec::new(),
                    deleted_outputs: Vec::new(),
                    restored: false,
                }
            }
            Ok(s) => {
                if dart_reason.is_none() {
                    match self.dart_action(a) {
                        Ok(r) => return r,
                        Err(DartFailure::Builder(e)) => return self.failure(a, e),
                        Err(DartFailure::Unavailable(_)) => {}
                    }
                }
                let mut r = self.fallback(a, dart_reason);
                r.measured = action
                    .outputs
                    .iter()
                    .filter_map(|id| {
                        s.outputs
                            .get(&self.natural_paths_map[id])
                            .map(|b| (id.clone(), Arc::from(b.clone())))
                    })
                    .collect();
                let m = s.refusals.values().next().cloned().unwrap_or_else(|| {
                    format!(
                        "{}: output not yet verified byte for byte",
                        generator.key()
                    )
                });
                r.reason = Some(match r.reason.take() {
                    Some(x) => format!("{m}; {x}"),
                    None => m,
                });
                r
            }
            Err(m) => {
                if dart_reason.is_none() {
                    match self.dart_action(a) {
                        Ok(r) => return r,
                        Err(DartFailure::Builder(e)) => return self.failure(a, e),
                        Err(DartFailure::Unavailable(_)) => {}
                    }
                }
                let mut r = self.fallback(a, dart_reason);
                r.reason = Some(match r.reason.take() {
                    Some(x) => format!("{m}; {x}"),
                    None => m,
                });
                r
            }
        }
    }

    fn fallback_in_parallel(&self, actions: &[usize], dart_reason: Option<&str>) -> Vec<ActionRecord> {
        let n = self.options.workers.max(1).min(actions.len());
        if n <= 1 || actions.len() < 8 {
            return actions
                .iter()
                .map(|&a| self.fallback(a, dart_reason))
                .collect();
        }
        let next = AtomicUsize::new(0);
        let mut all: Vec<(usize, ActionRecord)> = std::thread::scope(|s| {
            let hs: Vec<_> = (0..n)
                .map(|_| {
                    s.spawn(|| {
                        let mut mine = Vec::new();
                        loop {
                            let i = next.fetch_add(1, Ordering::Relaxed);
                            let Some(&a) = actions.get(i) else { break };
                            mine.push((i, self.fallback(a, dart_reason)));
                        }
                        mine
                    })
                })
                .collect();
            hs.into_iter()
                .flat_map(|h| h.join().unwrap_or_default())
                .collect()
        });
        all.sort_by_key(|(i, _)| *i);
        all.into_iter().map(|(_, r)| r).collect()
    }

    /// Fallback executor: what `build_runner` left on disk.
    fn fallback(&self, a: usize, dart_reason: Option<&str>) -> ActionRecord {
        let action = &self.graph.actions[a];
        let f = &self.phases[action.phase];
        let input = natural_path(&self.package_graph, &action.input);
        let memory = |p: &Path| self.memory.get(p).cloned();
        let mut queries = Vec::with_capacity(1 + action.outputs.len());
        let d = memory(&input)
            .map(|b| digest_bytes(&b))
            .or_else(|| crate::query::digest_file(&input));
        queries.push((Query::File(input), d));
        let mut outputs = Vec::with_capacity(action.outputs.len());
        for s in &action.outputs {
            let loc = self.support_path(s, f.hidden);
            let c: Option<Arc<[u8]>> = std::fs::read(&loc).ok().map(Arc::from);
            queries.push((
                Query::File(key(&loc)),
                c.as_deref().map(digest_bytes),
            ));
            outputs.push((s.clone(), c));
        }
        let has_build_dir = self
            .package_graph
            .root_dir
            .join(".dart_tool/build")
            .is_dir();
        let any = outputs.iter().any(|(_, c)| c.is_some());
        if !any && !has_build_dir {
            let m = format!(
                "no fallback: {} (there is no .dart_tool/build; run 'dart run build_runner build')",
                dart_reason.unwrap_or("no executor")
            );
            return ActionRecord {
                fingerprint: [0; 32],
                queries,
                outputs,
                origin: Origin::Pending,
                reason: Some(m),
                measured: Vec::new(),
                from_fallback: Vec::new(),
                deleted_outputs: Vec::new(),
                restored: false,
            };
        }
        ActionRecord {
            fingerprint: [0; 32],
            queries,
            outputs,
            origin: Origin::Support,
            reason: dart_reason.map(|m| format!("build_runner fallback: {m}")),
            measured: Vec::new(),
            from_fallback: Vec::new(),
            deleted_outputs: Vec::new(),
            restored: false,
        }
    }

    /// Publishes the generation: outputs with UTF-8 content that the loader
    /// reads from memory — the `cache` ones, and the `source` ones of native
    /// generators (the fallback ones are already on disk, at the natural
    /// path).
    fn publish(&mut self) {
        // Outputs marked by `deletePrimaryInput`: out of the generation, like
        // the official `FinalizedReader` (`finalized_reader.dart:44`).
        let deleted: HashSet<&AssetId> = self
            .records
            .iter()
            .flatten()
            .flat_map(|r| r.deleted_outputs.iter())
            .collect();
        let deleted: HashSet<AssetId> = deleted.into_iter().cloned().collect();
        let mut c = GenerationBuilder::new();
        let mut h = blake3::Hasher::new();
        let mut items: Vec<(PathBuf, Arc<[u8]>, &'static str, PathBuf)> = Vec::new();
        let mut labels: Vec<(usize, String)> = Vec::new();
        for (a, r) in self.records.iter().enumerate() {
            let Some(r) = r else { continue };
            let action = &self.graph.actions[a];
            let f = &self.phases[action.phase];
            if !f.hidden && !matches!(r.origin, Origin::Native(_) | Origin::Dart) {
                continue;
            }
            for (s, content) in &r.outputs {
                let Some(content) = content else { continue };
                if deleted.contains(s) {
                    continue;
                }
                let label = match r.origin {
                    Origin::Native(_) | Origin::Dart => {
                        self.plan.applications[f.application].key.clone()
                    }
                    _ => "build_runner".to_string(),
                };
                labels.push((a, label));
                items.push((
                    self.natural_path_of(s),
                    content.clone(),
                    "",
                    natural_path(&self.package_graph, &action.input),
                ));
            }
        }
        let labels: Vec<&'static str> = labels.into_iter().map(|(_, k)| self.label(&k)).collect();
        for (i, (n, content, _, input)) in items.into_iter().enumerate() {
            let Ok(text) = std::str::from_utf8(&content) else {
                continue;
            };
            h.update(n.to_string_lossy().as_bytes());
            h.update(&digest_bytes(&content));
            c.put(n, text, labels[i], vec![input]);
        }
        let id = u64::from_le_bytes(h.finalize().as_bytes()[..8].try_into().unwrap_or([0; 8]));
        match c.finish(id) {
            Ok(g) => self.generation = g,
            Err(e) => {
                for m in e {
                    eprintln!("build engine error: {m}");
                }
            }
        }
    }

    /// Content of an output by natural path, computing it if it has not been
    /// computed yet (the demand for a `.css` requested by the browser).
    pub fn materialize(&mut self, ctx: &UpdateContext<'_>, path: &Path) -> Option<Arc<[u8]>> {
        let mut k = key(path);
        if let Some(c) = self.memory.get(&k) {
            return Some(c.clone());
        }
        // `package_config.json` uses the canonical root; the served URL may
        // arrive with the lexical spelling (8.3 alias or link on Windows).
        // The generated file does not exist yet, so only the parent is
        // canonicalized.
        if !self.natural_paths_map.values().any(|n| n == &k) {
            if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
                if let Ok(parent) = std::fs::canonicalize(parent) {
                    k = key(&dartforge_elements::config::without_verbatim(parent).join(name));
                }
            }
        }
        if let Some(c) = self.memory.get(&k) {
            return Some(c.clone());
        }
        let id = self
            .natural_paths_map
            .iter()
            .find(|(_, n)| **n == k)
            .map(|(id, _)| id.clone())?;
        let g = self.graph.generated.get(&id)?.clone();
        if self.records[g.action].as_ref().is_some_and(|r| r.restored) {
            let (dirty, off_disk) = self.check_restored(g.action, ctx.db);
            for n in off_disk {
                self.memory.remove(&n);
            }
            match self.records[g.action].as_mut() {
                Some(r) if !dirty => r.restored = false,
                _ => self.records[g.action] = None,
            }
        }
        if self.records[g.action].is_none() {
            let a = g.action;
            let fi = self.graph.actions[a].phase;
            let dart_reason = self
                .dart
                .lock()
                .map(|d| match d.availability() {
                    Availability::Available => None,
                    Availability::Unavailable(m) => Some(m),
                })
                .unwrap_or_else(|_| Some("Dart executor poisoned".into()));
            let mut r = match self.native_for_phase(fi) {
                Some(n) if !self.natives[n].by_package() => {
                    self.native_action(ctx, &HashSet::new(), n, a, dart_reason.as_deref())
                }
                _ if dart_reason.is_none() => self.dart_or_fallback(a),
                _ => self.fallback(a, dart_reason.as_deref()),
            };
            r.fingerprint = self.fingerprint(a, &r.queries);
            for (s, c) in &r.outputs {
                if let Some(c) = c {
                    self.memory.insert(self.natural_paths_map[s].clone(), c.clone());
                }
            }
            self.records[a] = Some(r);
            self.state_dirty = true;
            self.publish();
            self.recompute_watched();
            if self.options.persist && self.save().is_ok() {
                self.state_dirty = false;
            }
        }
        self.memory.get(&k).cloned()
    }

    /// `build_to: source` outputs of native generators among `altered`, with
    /// their content: they go to disk (D-B2). The fallback ones are already
    /// there.
    pub fn native_source_outputs(&self, altered: &[PathBuf]) -> Vec<(PathBuf, Arc<[u8]>)> {
        if altered.is_empty() {
            return Vec::new();
        }
        let alt: HashSet<&PathBuf> = altered.iter().collect();
        let mut v = Vec::new();
        for (a, r) in self.records.iter().enumerate() {
            let Some(r) = r else { continue };
            if !matches!(r.origin, Origin::Native(_) | Origin::Dart)
                || self.phases[self.graph.actions[a].phase].hidden
            {
                continue;
            }
            for (s, c) in &r.outputs {
                let n = &self.natural_paths_map[s];
                if let (true, Some(c)) = (alt.contains(n), c) {
                    v.push((n.clone(), c.clone()));
                }
            }
        }
        v
    }

    /// Canonical state (actions, origin, reason, digest of each output and
    /// the published generation): what the determinism and "incremental =
    /// from scratch" invariants compare.
    pub fn canonical_state(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        for (i, a) in self.graph.actions.iter().enumerate() {
            if !a.live() {
                continue;
            }
            let f = &self.phases[a.phase];
            let head = format!(
                "{}#{} {}",
                self.plan.applications[f.application].key,
                f.factory,
                a.input.text()
            );
            match &self.records[i] {
                None => lines.push(format!("{head} -")),
                Some(r) => {
                    let outputs: Vec<String> = r
                        .outputs
                        .iter()
                        .map(|(s, c)| {
                            let d = c
                                .as_deref()
                                .map(|b| blake3::hash(b).to_hex().to_string())
                                .unwrap_or_else(|| "∅".into());
                            format!("{}={}", s.path, &d[..d.len().min(16)])
                        })
                        .collect();
                    let deleted: Vec<String> = r.deleted_outputs.iter().map(AssetId::text).collect();
                    lines.push(format!(
                        "{head} {:?} {} [{}]{}",
                        r.origin,
                        r.reason.as_deref().unwrap_or(""),
                        outputs.join(" "),
                        if deleted.is_empty() {
                            String::new()
                        } else {
                            format!(" deleted=[{}]", deleted.join(" "))
                        }
                    ));
                }
            }
        }
        lines.sort();
        let mut g: Vec<String> = self
            .generation
            .iter()
            .map(|(p, f)| {
                let rel = p
                    .strip_prefix(&self.root)
                    .unwrap_or(p)
                    .to_string_lossy()
                    .replace('\\', "/");
                format!(
                    "generated {rel} {}",
                    &blake3::hash(f.content.as_bytes()).to_hex()[..16]
                )
            })
            .collect();
        g.sort();
        lines.extend(g);
        lines.join("\n")
    }

    /// Scoreboard against a reference (the corpus oracle, or the fallback on
    /// disk): per reference output, equal/pending/different.
    ///
    /// The post-processor reasons start with "post-processor":
    /// `crates/build/tests/corpus.rs` matches that prefix on the pending
    /// reasons.
    pub fn scoreboard(&self, reference: &BTreeMap<AssetId, Vec<u8>>) -> Scoreboard {
        let mut p = Scoreboard::default();
        for (id, expected) in reference {
            let Some(g) = self.graph.generated.get(id) else {
                // A post-processor writes outputs the graph does not predict
                // (`PostProcessBuildStep`): they are in the anchor's record.
                let from_anchor = self
                    .graph
                    .actions
                    .iter()
                    .zip(&self.records)
                    .filter(|(a, _)| a.post_process)
                    .filter_map(|(_, r)| r.as_ref())
                    .find_map(|r| {
                        r.outputs
                            .iter()
                            .find(|(s, _)| s == id)
                            .map(|(_, c)| (r, c.clone()))
                    });
                if let Some((r, c)) = from_anchor {
                    match (&r.origin, c) {
                        (Origin::Dart, Some(c)) if c.as_ref() == expected.as_slice() => {
                            p.equal.push(id.clone())
                        }
                        _ => p
                            .different
                            .push((id.clone(), "post-processor: differs from the official one".into())),
                    }
                    continue;
                }
                let has_post = self
                    .phases
                    .iter()
                    .any(|f| f.post_process && self.package_graph.nodes[f.package].name == *id.package);
                if has_post {
                    p.pending.push((
                        id.clone(),
                        "post-processor: output only with the Dart executor".into(),
                    ));
                } else {
                    p.different
                        .push((id.clone(), "output not predicted by the plan".into()));
                }
                continue;
            };
            match self.records[g.action].as_ref() {
                None => p.pending.push((id.clone(), "not computed".into())),
                Some(r) => {
                    let c = r
                        .outputs
                        .iter()
                        .find(|(s, _)| s == id)
                        .and_then(|(_, c)| c.clone());
                    match (&r.origin, c) {
                        (Origin::Native(n), _) if r.from_fallback.contains(id) => {
                            p.pending.push((
                                id.clone(),
                                format!(
                                    "{n}: output not generated by the native generator; build_runner fallback"
                                ),
                            ))
                        }
                        (Origin::Native(_) | Origin::Dart, Some(c))
                            if c.as_ref() == expected.as_slice() =>
                        {
                            p.equal.push(id.clone())
                        }
                        (Origin::Native(n), _) => p
                            .different
                            .push((id.clone(), format!("{n}: differs from the official output"))),
                        (Origin::Dart, _) => p.different.push((
                            id.clone(),
                            "Dart executor: differs from the official output".into(),
                        )),
                        (Origin::NotTriggered, _) => p.different.push((
                            id.clone(),
                            "no trigger fired here; the official tool wrote it".into(),
                        )),
                        (Origin::Skipped, _) => p.different.push((
                            id.clone(),
                            "generated input not written here; the official tool wrote it".into(),
                        )),
                        _ => {
                            if let Some((_, m)) = r.measured.iter().find(|(s, _)| s == id) {
                                if m.as_ref() == expected.as_slice() {
                                    p.measured_equal += 1;
                                } else {
                                    p.measured_different += 1;
                                }
                            }
                            p.pending.push((
                                id.clone(),
                                r.reason.clone().unwrap_or_else(|| "fallback".into()),
                            ))
                        }
                    }
                }
            }
        }
        // An extra native output (one the reference does not have). One from
        // an optional builder (`is_optional`) only exists in the official
        // tool if someone asked for it: without a failure of it on that input
        // (`error_cache`), it is an output the official graph never requested
        // (unreachable) — left out of the count, not pending: it is not
        // expected.
        for (i, r) in self.records.iter().enumerate() {
            let Some(r) = r else { continue };
            let n = match r.origin {
                Origin::Native(n) => n,
                Origin::Dart => "Dart executor",
                _ => continue,
            };
            let action = &self.graph.actions[i];
            let optional = self.phases[action.phase].optional;
            for (s, c) in &r.outputs {
                if c.is_none() || reference.contains_key(s) {
                    continue;
                }
                if optional && !self.failed_in_official(&action.input) {
                    p.unrequested.push((
                        s.clone(),
                        format!("{n}: optional output the official graph does not request"),
                    ));
                } else {
                    p.different.push((s.clone(), format!("{n}: extra output")));
                }
            }
        }
        p
    }

    /// The official `build_runner` recorded an error of some builder on this
    /// input (`.dart_tool/build/<hash>/error_cache/<package>/<phase>/<path>`).
    fn failed_in_official(&self, input: &AssetId) -> bool {
        let base = self.package_graph.root_dir.join(".dart_tool/build");
        let Ok(hashes) = std::fs::read_dir(&base) else {
            return false;
        };
        for h in hashes.flatten() {
            let package = h.path().join("error_cache").join(input.package.as_ref());
            let Ok(phases) = std::fs::read_dir(&package) else {
                continue;
            };
            for f in phases.flatten() {
                if f.path().join(input.path.as_ref()).is_file() {
                    return true;
                }
            }
        }
        false
    }
}

/// `AssetId.resolve(Uri.parse(uri), from: from)`: `package:p/x` is `p|lib/x`,
/// `asset:p/x` is `p|x`, a relative path is relative to `from`'s directory.
/// `None` for another scheme or a path that leaves the package.
fn resolve_uri(uri: &str, from: &AssetId) -> Option<AssetId> {
    if let Some(rest) = uri.strip_prefix("package:") {
        let (p, c) = rest.split_once('/')?;
        return Some(AssetId::new(p, &format!("lib/{c}")));
    }
    if let Some(rest) = uri.strip_prefix("asset:") {
        let (p, c) = rest.split_once('/')?;
        return Some(AssetId::new(p, c));
    }
    if uri.contains(':') {
        return None;
    }
    let mut parts: Vec<&str> = if uri.starts_with('/') {
        Vec::new()
    } else {
        let mut v: Vec<&str> = from.path.split('/').collect();
        v.pop();
        v
    };
    for s in uri.split('/') {
        match s {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            x => parts.push(x),
        }
    }
    Some(AssetId::new(&from.package, &parts.join("/")))
}

fn is_newer(input: &Path, fallback: &Path) -> bool {
    let (Some(e), Some(a)) = (FileStamp::read(input), FileStamp::read(fallback)) else {
        return false;
    };
    e.mtime_ns > a.mtime_ns
}

#[derive(Debug, Default, Clone)]
pub struct Scoreboard {
    pub equal: Vec<AssetId>,
    pub pending: Vec<(AssetId, String)>,
    pub different: Vec<(AssetId, String)>,
    /// Outputs the builder could produce but the official graph never asks
    /// for (an optional builder with nobody reading the output): not counted
    /// — they are neither expected, nor pending, nor different.
    pub unrequested: Vec<(AssetId, String)>,
    /// Unverified native generators, measured against the reference.
    pub measured_equal: usize,
    pub measured_different: usize,
}

impl Scoreboard {
    pub fn summary(&self) -> String {
        let mut s = format!(
            "{} equal / {} pending / {} different",
            self.equal.len(),
            self.pending.len(),
            self.different.len()
        );
        if !self.unrequested.is_empty() {
            s.push_str(&format!(
                " ({} not requested, not counted)",
                self.unrequested.len()
            ));
        }
        if self.measured_equal + self.measured_different > 0 {
            s.push_str(&format!(
                " (unverified native generators, measured: {} equal, {} different)",
                self.measured_equal, self.measured_different
            ));
        }
        s
    }

    pub fn reasons(&self) -> BTreeMap<String, usize> {
        let mut m = BTreeMap::new();
        for (_, x) in self.pending.iter().chain(&self.different) {
            *m.entry(x.clone()).or_default() += 1;
        }
        m
    }
}
