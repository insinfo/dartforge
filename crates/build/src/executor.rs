//! The executors behind a single choice (`docs/BUILD-MOTOR.md` §6):
//! **native** (Rust, reads the semantic database), **Dart** (`dfexec/1`: the
//! VM process of [`crate::vm`], or [`Unavailable`] when it was not enabled)
//! and **fallback** (what `build_runner` left on disk, in `engine.rs`). An
//! executor that cannot generate refuses with a reason.
use crate::query::{SemanticDb, Query, Digest, digest_file, digest_bytes};
use crate::graph::{AssetId, Graph};
use crate::packages::PackageGraph;
use crate::value::Map;
use dartforge_elements::model::Program;
use dartforge_intern::Interner;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

// ------------------------------------------------------------------ native

/// An action requested from a native generator.
#[derive(Debug, Clone)]
pub struct NativeAction {
    pub factory: String,
    pub input: AssetId,
    pub natural_input: PathBuf,
    /// Expected outputs and the natural path of each one.
    pub outputs: Vec<(AssetId, PathBuf)>,
    pub options: Map,
}

/// A request: one action (per-file generator) or all the actions of the
/// phases the generator covers in a package (per-package generator — ngdart's
/// stage A).
#[derive(Debug, Clone)]
pub struct NativeRequest {
    pub package: String,
    pub package_root: PathBuf,
    pub actions: Vec<NativeAction>,
    /// The root of each package in `package_config.json` (Sass's `package:`
    /// resolves through them, like `AssetId.resolve` in `build`).
    pub roots: BTreeMap<String, PathBuf>,
    /// The `pubspec.lock` version of each package the generator imitates
    /// ([`crate::descriptor::imitates`]): `sass_builder` picks the dart-sass
    /// version by it.
    pub versions: BTreeMap<String, String>,
}

/// What a native generator returns: content by natural output path, and
/// refusals by natural **input** path (the scoreboard counts the reasons).
#[derive(Debug, Default)]
pub struct NativeOutput {
    pub outputs: BTreeMap<PathBuf, Vec<u8>>,
    pub refusals: BTreeMap<PathBuf, String>,
    /// Files the generator actually regenerated in this call.
    pub generated_unit_count: usize,
    /// The generator confirmed that the previous round's queries are still
    /// valid; `GeneratorCtx` records only the answers that changed.
    pub reuse_queries: bool,
}

/// A Rust generator that imitates an ecosystem builder byte for byte.
pub trait NativeGenerator: Send + Sync {
    /// Builder key (`ngdart:ngdart`).
    fn key(&self) -> &'static str;
    /// Covered factories.
    fn covers(&self, factory: &str) -> bool;
    /// One call per package (with all the actions) instead of one per action.
    fn by_package(&self) -> bool;
    /// Equality gate: `false` = the output is not yet verified byte for byte
    /// against the official one; the engine only uses it to measure
    /// (`--comparar`) and publishes the fallback instead.
    fn verified(&self) -> bool;
    fn generate(
        &self,
        ctx: &mut GeneratorCtx<'_>,
        request: &NativeRequest,
    ) -> Result<NativeOutput, String>;
}

/// The only read channel of a native generator: every read becomes a query
/// (soundness rule — what it did not query cannot invalidate it).
pub struct GeneratorCtx<'a> {
    pub program: Option<(&'a Program, &'a Interner)>,
    /// Events of this update, already normalized by the engine.
    pub changed_paths: &'a HashSet<PathBuf>,
    pub(crate) db: &'a dyn SemanticDb,
    pub(crate) memory: &'a dyn Fn(&Path) -> Option<Arc<[u8]>>,
    pub(crate) queries: Vec<(Query, Option<Digest>)>,
}

impl<'a> GeneratorCtx<'a> {
    pub(crate) fn new(
        program: Option<(&'a Program, &'a Interner)>,
        changed: &'a HashSet<PathBuf>,
        database: &'a dyn SemanticDb,
        memory: &'a dyn Fn(&Path) -> Option<Arc<[u8]>>,
    ) -> Self {
        GeneratorCtx {
            program: program,
            changed_paths: changed,
            db: database,
            memory: memory,
            queries: Vec::new(),
        }
    }

    /// Records a query with the digest of its current answer.
    pub fn register(&mut self, c: Query) {
        let d = query_digest(&c, self.db, self.memory);
        self.queries.push((c, d));
    }

    pub fn read(&mut self, p: &Path) -> Option<Arc<[u8]>> {
        let key = dartforge_elements::gerado::chave(p);
        let v = (self.memory)(&key).or_else(|| std::fs::read(p).ok().map(Arc::from));
        let d = v.as_deref().map(digest_bytes);
        self.queries.push((Query::File(key), d));
        v
    }

    pub fn exists(&mut self, p: &Path) -> bool {
        let key = dartforge_elements::gerado::chave(p);
        let yes = (self.memory)(&key).is_some() || p.is_file();
        self.queries
            .push((Query::Exists(key), yes.then(|| digest_bytes(b"1"))));
        yes
    }
}

/// The candidates of a `findAssets` call by action `action`, in order:
/// sources and predicted outputs of the input's package that match the
/// pattern, without the outputs the action cannot see because of its phase
/// (`build_impl.dart:443-463`). The engine recomputes the list after a
/// structural event to know whether the glob's answer actually changed.
pub(crate) fn glob_candidates(
    graph: &Graph,
    action: usize,
    pattern: &crate::glob::Glob,
) -> Vec<AssetId> {
    let Some(a) = graph.actions.get(action) else {
        return Vec::new();
    };
    let (package, phase) = (a.input.package.clone(), a.phase);
    let sources = graph.sources.get(&package).into_iter().flat_map(|cs| {
        cs.iter().map(|c| AssetId {
            package: package.clone(),
            path: c.clone(),
        })
    });
    let generated = graph
        .generated
        .range(AssetId::new(&package, "")..)
        .take_while(|(id, _)| id.package == package)
        .map(|(id, _)| id.clone());
    let mut ids: Vec<_> = sources
        .chain(generated)
        .filter(|id| {
            pattern.matches(&id.path)
                && graph
                    .generated
                    .get(id)
                    .is_none_or(|g| g.phase < phase || (g.phase == phase && g.action == action))
        })
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// Current digest of a query's answer.
pub(crate) fn query_digest(
    c: &Query,
    database: &dyn SemanticDb,
    memory: &dyn Fn(&Path) -> Option<Arc<[u8]>>,
) -> Option<Digest> {
    match c {
        Query::File(p) => memory(p)
            .map(|b| digest_bytes(&b))
            .or_else(|| digest_file(p)),
        Query::Exists(p) => (memory(p).is_some() || p.is_file()).then(|| digest_bytes(b"1")),
        Query::Glob { dir, pattern } => {
            let g = crate::glob::Glob::new(pattern).ok()?;
            let list = crate::graph::list(dir, std::slice::from_ref(&g));
            Some(digest_bytes(
                list.into_iter().collect::<Vec<_>>().join("\n").as_bytes(),
            ))
        }
        Query::ActiveGlobs {
            dir, candidates, ..
        } => {
            let list = candidates
                .iter()
                .filter_map(|(rel, generated)| {
                    let path = dartforge_elements::gerado::chave(&dir.join(rel));
                    (if *generated {
                        memory(&path).is_some()
                    } else {
                        path.is_file()
                    })
                    .then_some(rel.as_str())
                })
                .collect::<Vec<_>>();
            Some(digest_bytes(list.join("\n").as_bytes()))
        }
        _ => database.digest(c),
    }
}

// -------------------------------------------------------------------- Dart

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Available,
    Unavailable(String),
}

#[derive(Debug, Clone)]
pub struct ExecutorError(pub String);

#[derive(Debug, Clone)]
pub struct DisallowedOutput(pub AssetId);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Fine,
    Info,
    Warning,
    Severe,
}

/// The equivalent of `.dart_tool/build/entrypoint/build.dart`: the factory
/// imports and the key → factories map (`build_script_generate.dart`).
#[derive(Debug, Clone)]
pub struct BuilderScript {
    pub applications: Vec<(String, String, Vec<String>)>,
    /// Interim key from the plan, lock versions and engine version; sources
    /// and ABI will be added before enabling the real process cache.
    pub cache_key: String,
}

/// `build.executar` of `dfexec/1` (`docs/BUILD-PROTOCOLO.md` §3).
#[derive(Debug, Clone)]
pub struct ActionRequest {
    pub phase: usize,
    pub key: String,
    pub factory: String,
    pub options: Map,
    pub root: bool,
    pub input: AssetId,
    pub allowed_outputs: Vec<AssetId>,
}

/// Run-time extensions in the order of the `buildExtensions` map: input →
/// outputs.
pub type RuntimeExtensions = Vec<(String, Vec<String>)>;

/// `build.extensoes` of `dfexec/1`: a phase's factory, with its options.
#[derive(Debug, Clone)]
pub struct ExtensionsRequest {
    pub key: String,
    pub factory: String,
    pub options: Map,
    pub root: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ActionResult {
    pub outputs: Vec<(AssetId, Arc<[u8]>)>,
    pub logs: Vec<(String, String)>,
    pub failed: bool,
    /// Post-processor: the inputs marked by `deletePrimaryInput` (the node's
    /// `deletedBy`; only the merged output directory and the official
    /// `serve` stop seeing them).
    pub deleted_outputs: Vec<AssetId>,
}

/// The `Resolver` served by the semantic database (Phase 3 of BUILD-RUST.md).
pub trait ResolverService {}

/// The `BuildStep` served by the engine: every call becomes a `Query`.
pub trait BuildStepService {
    fn can_read(&mut self, id: &AssetId) -> bool;
    fn read(&mut self, id: &AssetId) -> Option<Arc<[u8]>>;
    fn find_assets(&mut self, glob: &str) -> Vec<AssetId>;
    fn digest(&mut self, id: &AssetId) -> Option<[u8; 32]>;
    fn write(&mut self, id: &AssetId, bytes: Arc<[u8]>) -> Result<(), DisallowedOutput>;
    /// `None` ⇒ the analyzer runs inside the executor.
    fn resolver(&mut self) -> Option<&mut dyn ResolverService>;
    fn log(&mut self, level: Level, msg: &str);
}

/// An action's view of the graph. The executor never receives free paths:
/// an `AssetId` must exist in the graph, and outputs of the action's own
/// phase only become readable after the current action writes them
/// (`build_impl.dart:443-463`).
pub struct ActionService<'a> {
    pub graph: &'a Graph,
    pub packages: &'a PackageGraph,
    pub action: usize,
    pub memory: &'a dyn Fn(&AssetId) -> Option<Arc<[u8]>>,
    pub writes: BTreeMap<AssetId, Arc<[u8]>>,
    pub queries: Vec<(Query, Option<Digest>)>,
    pub logs: Vec<(Level, String)>,
    /// In a post-processor anchor: the outputs other anchors already wrote
    /// (the official `addAsset` rejects an asset the graph already has).
    pub occupied: std::collections::BTreeSet<AssetId>,
}

impl<'a> ActionService<'a> {
    pub fn new(
        graph: &'a Graph,
        packages: &'a PackageGraph,
        action: usize,
        memory: &'a dyn Fn(&AssetId) -> Option<Arc<[u8]>>,
    ) -> Self {
        Self {
            graph,
            packages,
            action,
            memory,
            writes: BTreeMap::new(),
            queries: Vec::new(),
            logs: Vec::new(),
            occupied: Default::default(),
        }
    }

    /// A source readable by path: from the graph, or from an unlisted
    /// package (a dependency the resolver reads).
    fn is_source(&self, id: &AssetId) -> bool {
        self.graph.has_source(id) || self.graph.external_source(id)
    }

    fn path(&self, id: &AssetId) -> Option<PathBuf> {
        if !self.graph.exists(id) && !self.graph.external_source(id) {
            return None;
        }
        self.query_path(id)
    }

    fn query_path(&self, id: &AssetId) -> Option<PathBuf> {
        let node = self.packages.no(&id.package)?;
        // Negative queries must observe files that have not entered the
        // graph yet; even so, they cannot escape the package.
        if id
            .path
            .split('/')
            .any(|p| p.is_empty() || p == ".." || p == "." || p.contains('\\') || p.contains(':'))
        {
            return None;
        }
        Some(dartforge_elements::gerado::chave(
            &node.root.join(id.path.as_ref()),
        ))
    }

    fn bytes(&self, id: &AssetId) -> Option<Arc<[u8]>> {
        let phase = self.graph.actions.get(self.action)?.phase;
        if let Some(g) = self.graph.generated.get(id) {
            if g.phase > phase || (g.phase == phase && g.action != self.action) {
                return None;
            }
            return if g.phase == phase {
                self.writes.get(id).cloned()
            } else {
                (self.memory)(id)
            };
        }
        if !self.is_source(id) {
            return None;
        }
        std::fs::read(self.path(id)?).ok().map(Arc::from)
    }

    fn is_available(&self, id: &AssetId) -> bool {
        let Some(phase) = self.graph.actions.get(self.action).map(|a| a.phase) else {
            return false;
        };
        if let Some(g) = self.graph.generated.get(id) {
            return if g.phase > phase {
                false
            } else if g.phase == phase {
                g.action == self.action && self.writes.contains_key(id)
            } else {
                (self.memory)(id).is_some()
            };
        }
        self.is_source(id) && self.path(id).is_some_and(|p| p.is_file())
    }

    fn record(&mut self, id: &AssetId, exists: bool, bytes: Option<&[u8]>) {
        if let Some(p) = self.query_path(id) {
            let c = if exists {
                Query::Exists(p)
            } else {
                Query::File(p)
            };
            let d = if exists {
                bytes.map(|_| digest_bytes(b"1"))
            } else {
                bytes.map(digest_bytes)
            };
            self.queries.push((c, d));
        }
    }
}

impl BuildStepService for ActionService<'_> {
    fn can_read(&mut self, id: &AssetId) -> bool {
        let yes = self.is_available(id);
        self.record(id, true, yes.then_some(&[]));
        yes
    }

    fn read(&mut self, id: &AssetId) -> Option<Arc<[u8]>> {
        let b = self.bytes(id);
        self.record(id, false, b.as_deref());
        b
    }

    fn find_assets(&mut self, glob: &str) -> Vec<AssetId> {
        let Ok(pattern) = crate::glob::Glob::new(glob) else {
            return Vec::new();
        };
        let Some(package) = self
            .graph
            .actions
            .get(self.action)
            .map(|a| a.input.package.clone())
        else {
            return Vec::new();
        };
        let mut ids = glob_candidates(self.graph, self.action, &pattern);
        let candidates: Vec<_> = ids
            .iter()
            .map(|id| (id.path.to_string(), self.graph.generated.contains_key(id)))
            .collect();
        ids.retain(|id| self.can_read(id));
        // An empty listing also depends on the generated outputs that do not
        // exist yet. An old fallback file does not count as a current output.
        if let Some(node) = self.packages.no(&package) {
            self.queries.push((
                Query::ActiveGlobs {
                    dir: node.root.clone(),
                    pattern: glob.to_string(),
                    candidates: candidates,
                },
                Some(digest_bytes(
                    ids.iter()
                        .map(|id| id.path.as_ref())
                        .collect::<Vec<_>>()
                        .join("\n")
                        .as_bytes(),
                )),
            ));
        }
        ids
    }

    fn digest(&mut self, id: &AssetId) -> Option<[u8; 32]> {
        self.read(id).as_deref().map(digest_bytes)
    }

    fn write(&mut self, id: &AssetId, bytes: Arc<[u8]>) -> Result<(), DisallowedOutput> {
        let Some(action) = self.graph.actions.get(self.action) else {
            return Err(DisallowedOutput(id.clone()));
        };
        if action.pos {
            // `PostProcessBuildStep.writeAsBytes` → `addAsset`
            // (`build_impl.dart:643-656`): any asset the graph does not have
            // yet — not a source, not a predicted output, not another
            // anchor's output, nor the same asset twice in the action.
            let new = !self.graph.exists(id)
                && !self.graph.external_source(id)
                && !self.occupied.contains(id)
                && !self.writes.contains_key(id)
                && self.packages.no(&id.package).is_some()
                && self.query_path(id).is_some();
            if !new {
                return Err(DisallowedOutput(id.clone()));
            }
            self.writes.insert(id.clone(), bytes);
            return Ok(());
        }
        if self.path(id).is_none() || !action.outputs.contains(id) {
            return Err(DisallowedOutput(id.clone()));
        }
        self.writes.insert(id.clone(), bytes);
        Ok(())
    }

    fn resolver(&mut self) -> Option<&mut dyn ResolverService> {
        None
    }

    fn log(&mut self, level: Level, msg: &str) {
        self.logs.push((level, msg.to_string()));
    }
}

/// The Dart builder executor (Phase 4): a persistent process that speaks
/// `dfexec/1`. It will come from the self-hosted native executor, shared
/// with macros.
pub trait DartExecutor: Send {
    fn availability(&self) -> Availability;
    fn prepare(&mut self, script: &BuilderScript) -> Result<(), ExecutorError>;
    /// Starts an engine update: the interval between two `build_runner`
    /// builds, in which it resets the per-build state of the resolvers
    /// (`build_impl.dart:94-96`). Only called after a successful `prepare`.
    fn new_round(&mut self) {}
    /// The run-time extensions of the `Builder` the factory returns with
    /// these options (`builder.buildExtensions`, in map order): what
    /// `build_runner` uses to predict the outputs. `None` when the executor
    /// cannot answer; the engine keeps the ones from the descriptor or from
    /// `build.yaml`.
    fn extensions(
        &mut self,
        _request: &ExtensionsRequest,
    ) -> Result<Option<RuntimeExtensions>, ExecutorError> {
        Ok(None)
    }
    fn execute(
        &mut self,
        request: &ActionRequest,
        service: &mut dyn BuildStepService,
    ) -> Result<ActionResult, ExecutorError>;
    /// The `inputExtensions` of the `PostProcessBuilder` the factory returns
    /// with these options. `None` when the executor cannot answer: the
    /// post-processing phase gets no anchors and the scoreboard counts it as
    /// pending.
    fn post_inputs(
        &mut self,
        _request: &ExtensionsRequest,
    ) -> Result<Option<Vec<String>>, ExecutorError> {
        Ok(None)
    }
    /// `build.posprocessar`: `package:build`'s `runPostProcessBuilder` on the
    /// anchor's input. `allowed_outputs` comes empty; writes go through
    /// [`BuildStepService::write`], which applies the `addAsset` rule.
    /// The files the builders' code depends on (the bootstrap's depfile), so
    /// that the state saved across processes knows when a Dart action became
    /// stale without any of its queries changing. `None` when the executor
    /// does not know: Dart actions are not saved.
    fn code(&self) -> Option<Vec<PathBuf>> {
        None
    }
    fn post_process_step(
        &mut self,
        _request: &ActionRequest,
        _service: &mut dyn BuildStepService,
    ) -> Result<ActionResult, ExecutorError> {
        Err(ExecutorError(
            "this executor does not run post-processors".into(),
        ))
    }
    fn shutdown(&mut self);
}

/// The executor when there is no Dart process: every Dart action is refused
/// with the reason (the engine falls back). The VM executor is in
/// [`crate::vm`].
pub struct Unavailable(pub String);

impl Default for Unavailable {
    fn default() -> Self {
        Unavailable(
            "Dart builder executor disabled (the self-hosted one does not exist yet, BUILD-RUST.md Phase 4; through the VM: `dartforge build --dart <exe>` or DARTFORGE_BUILD_DART)"
                .into(),
        )
    }
}

impl DartExecutor for Unavailable {
    fn availability(&self) -> Availability {
        Availability::Unavailable(self.0.clone())
    }
    fn prepare(&mut self, _script: &BuilderScript) -> Result<(), ExecutorError> {
        Err(ExecutorError(self.0.clone()))
    }
    fn execute(
        &mut self,
        _p: &ActionRequest,
        _s: &mut dyn BuildStepService,
    ) -> Result<ActionResult, ExecutorError> {
        Err(ExecutorError(self.0.clone()))
    }
    fn shutdown(&mut self) {}
}

#[cfg(test)]
mod service_tests {
    use super::*;
    use crate::config::DependencyType;
    use crate::graph::{Action, GeneratedNode};
    use crate::packages::No;

    #[test]
    fn phase_visibility_and_the_actions_own_output() {
        let dir = tempfile::tempdir().unwrap();
        let packages = PackageGraph {
            nos: vec![
                No {
                    name: "p".into(),
                    root: dir.path().join("p"),
                    kind: DependencyType::Path,
                    is_root: true,
                    deps: vec![],
                },
                No {
                    name: "q".into(),
                    root: dir.path().join("q"),
                    kind: DependencyType::Path,
                    is_root: false,
                    deps: vec![],
                },
            ],
            root: 0,
            by_name: [("p".into(), 0), ("q".into(), 1)].into(),
            lock: Default::default(),
            root_dir: dir.path().to_path_buf(),
        };
        let source = AssetId::new("p", "lib/a.dart");
        let first = AssetId::new("p", "lib/a.g.dart");
        let foreign = AssetId::new("q", "lib/other.dart");
        let second = AssetId::new("p", "lib/a.h.dart");
        let future = AssetId::new("p", "lib/a.i.dart");
        let escape = AssetId::new("p", "lib/../escape.dart");
        std::fs::create_dir_all(dir.path().join("p/lib")).unwrap();
        std::fs::create_dir_all(dir.path().join("q/lib")).unwrap();
        std::fs::write(dir.path().join("p/lib/a.dart"), b"source").unwrap();
        std::fs::write(dir.path().join("q/lib/other.dart"), b"foreign").unwrap();
        let mut graph = Graph::default();
        graph
            .sources
            .entry("p".into())
            .or_default()
            .insert("lib/a.dart".into());
        graph
            .sources
            .entry("q".into())
            .or_default()
            .insert("lib/other.dart".into());
        graph.actions = vec![
            Action {
                phase: 0,
                input: source.clone(),
                outputs: vec![first.clone()],
                pos: false,
            },
            Action {
                phase: 1,
                input: first.clone(),
                outputs: vec![second.clone(), escape.clone()],
                pos: false,
            },
            Action {
                phase: 2,
                input: second.clone(),
                outputs: vec![future.clone()],
                pos: false,
            },
        ];
        for (id, action, phase) in [(&first, 0, 0), (&second, 1, 1), (&future, 2, 2)] {
            graph.generated.insert(
                id.clone(),
                GeneratedNode {
                    action: action,
                    phase: phase,
                    hidden: false,
                },
            );
        }
        graph.generated.insert(
            escape.clone(),
            GeneratedNode {
                action: 1,
                phase: 1,
                hidden: false,
            },
        );
        let memory: BTreeMap<AssetId, Arc<[u8]>> = [
            (first.clone(), Arc::from(&b"prior"[..])),
            (future.clone(), Arc::from(&b"future"[..])),
        ]
        .into();
        let read_memory = |id: &AssetId| memory.get(id).cloned();
        let mut s = ActionService::new(&graph, &packages, 1, &read_memory);
        assert_eq!(s.read(&source).as_deref(), Some(&b"source"[..]));
        assert_eq!(s.read(&first).as_deref(), Some(&b"prior"[..]));
        assert!(!s.can_read(&second));
        assert!(!s.can_read(&future));
        assert!(s.write(&future, Arc::from(&b"bad"[..])).is_err());
        assert!(s.write(&escape, Arc::from(&b"bad"[..])).is_err());
        let new = AssetId::new("p", "lib/new.dart");
        assert!(!s.can_read(&new));
        assert!(
            s.queries.iter().any(
                |(c, d)| matches!(c, Query::Exists(p) if p.ends_with("new.dart")) && d.is_none()
            ),
            "a negative canRead must invalidate when the source appears"
        );
        s.write(&second, Arc::from(&b"own"[..])).unwrap();
        assert_eq!(s.read(&second).as_deref(), Some(&b"own"[..]));
        assert_eq!(s.find_assets("lib/**"), vec![source, first, second]);
        assert!(
            !s.find_assets("lib/**").contains(&foreign),
            "findAssets stays in the input's package, like build 2.4.2"
        );
        assert!(
            s.queries
                .iter()
                .any(|(c, d)| matches!(c, Query::Exists(_)) && d.is_none())
        );
    }

    #[test]
    fn post_processor_anchor_writes_only_new_assets() {
        let dir = tempfile::tempdir().unwrap();
        let packages = PackageGraph {
            nos: vec![No {
                name: "p".into(),
                root: dir.path().join("p"),
                kind: DependencyType::Path,
                is_root: true,
                deps: vec![],
            }],
            root: 0,
            by_name: [("p".into(), 0)].into(),
            lock: Default::default(),
            root_dir: dir.path().to_path_buf(),
        };
        std::fs::create_dir_all(dir.path().join("p/lib")).unwrap();
        std::fs::write(dir.path().join("p/lib/a.draft"), b"x").unwrap();
        let input = AssetId::new("p", "lib/a.draft");
        let predicted = AssetId::new("p", "lib/a.g.dart");
        let mut graph = Graph::default();
        graph
            .sources
            .entry("p".into())
            .or_default()
            .insert("lib/a.draft".into());
        graph.actions = vec![
            Action {
                phase: 0,
                input: input.clone(),
                outputs: vec![predicted.clone()],
                pos: false,
            },
            Action {
                phase: 1,
                input: input.clone(),
                outputs: vec![],
                pos: true,
            },
        ];
        graph.generated.insert(
            predicted.clone(),
            GeneratedNode {
                action: 0,
                phase: 0,
                hidden: true,
            },
        );
        let memory = |_: &AssetId| None;
        let mut s = ActionService::new(&graph, &packages, 1, &memory);
        s.occupied.insert(AssetId::new("p", "lib/from_other.txt"));
        let b = || Arc::from(&b"r"[..]);
        assert!(
            s.write(&input, b()).is_err(),
            "the source already exists in the graph"
        );
        assert!(
            s.write(&predicted, b()).is_err(),
            "predicted output of another phase"
        );
        assert!(
            s.write(&AssetId::new("p", "lib/from_other.txt"), b())
                .is_err(),
            "output of another anchor"
        );
        assert!(
            s.write(&AssetId::new("p", "lib/../outside.txt"), b())
                .is_err()
        );
        assert!(
            s.write(&AssetId::new("q", "lib/x.txt"), b()).is_err(),
            "unknown package"
        );
        let new = AssetId::new("p", "lib/a.draft.summary");
        s.write(&new, b()).unwrap();
        assert!(s.write(&new, b()).is_err(), "the same asset twice");
        assert_eq!(s.writes.keys().collect::<Vec<_>>(), vec![&new]);
    }

    #[test]
    fn unlisted_dependency_is_readable_through_the_package_filter() {
        let dir = tempfile::tempdir().unwrap();
        let packages = PackageGraph {
            nos: vec![
                No {
                    name: "p".into(),
                    root: dir.path().join("p"),
                    kind: DependencyType::Path,
                    is_root: true,
                    deps: vec![],
                },
                No {
                    name: "q".into(),
                    root: dir.path().join("q"),
                    kind: DependencyType::Hosted,
                    is_root: false,
                    deps: vec![],
                },
            ],
            root: 0,
            by_name: [("p".into(), 0), ("q".into(), 1)].into(),
            lock: Default::default(),
            root_dir: dir.path().to_path_buf(),
        };
        std::fs::create_dir_all(dir.path().join("p/lib")).unwrap();
        std::fs::create_dir_all(dir.path().join("q/lib")).unwrap();
        std::fs::create_dir_all(dir.path().join("q/tool")).unwrap();
        std::fs::write(
            dir.path().join("p/lib/a.dart"),
            b"import 'package:q/q.dart';",
        )
        .unwrap();
        std::fs::write(dir.path().join("q/lib/q.dart"), b"class Q {}").unwrap();
        std::fs::write(dir.path().join("q/tool/t.dart"), b"secret").unwrap();
        let mut graph = Graph::default();
        graph
            .sources
            .entry("p".into())
            .or_default()
            .insert("lib/a.dart".into());
        let lib = || vec![crate::glob::Glob::new("lib/**").unwrap()];
        graph.externals.insert(
            "q".into(),
            crate::graph::SourceFilter {
                targets: vec![crate::plan::InputMatcher {
                    include: Some(lib()),
                    exclude: None,
                }],
                visible: Some(lib()),
            },
        );
        let input = AssetId::new("p", "lib/a.dart");
        graph.actions = vec![Action {
            phase: 0,
            input: input.clone(),
            outputs: vec![AssetId::new("p", "lib/a.g.dart")],
            pos: false,
        }];
        let memory = |_: &AssetId| None;
        let mut s = ActionService::new(&graph, &packages, 0, &memory);
        assert!(s.can_read(&AssetId::new("q", "lib/q.dart")));
        assert_eq!(
            s.read(&AssetId::new("q", "lib/q.dart")).as_deref(),
            Some(&b"class Q {}"[..])
        );
        assert!(
            !s.can_read(&AssetId::new("q", "tool/t.dart")),
            "outside a dependency's visible set"
        );
        assert!(s.read(&AssetId::new("q", "lib/missing.dart")).is_none());
        assert!(
            s.queries.iter().any(
                |(c, d)| matches!(c, Query::File(p) if p.ends_with("q.dart")) && d.is_some()
            ),
            "reading the dependency is a query of the action"
        );
    }

    #[test]
    fn buildstep_glob_revalidates_generated_output_in_memory() {
        let dir = tempfile::tempdir().unwrap();
        let query = Query::ActiveGlobs {
            dir: dir.path().to_path_buf(),
            pattern: "lib/**".into(),
            candidates: vec![("lib/a.g.dart".into(), true)],
        };
        let empty = |_: &Path| None;
        let full =
            |p: &Path| (p == dir.path().join("lib/a.g.dart")).then(|| Arc::from(&b"generated"[..]));
        assert_ne!(
            query_digest(&query, &crate::query::NoDb, &empty),
            query_digest(&query, &crate::query::NoDb, &full)
        );
        // An old fallback file on disk does not make the current round's
        // output present when the generator did not publish it in memory.
        std::fs::create_dir(dir.path().join("lib")).unwrap();
        std::fs::write(dir.path().join("lib/a.g.dart"), b"old").unwrap();
        assert_eq!(
            query_digest(&query, &crate::query::NoDb, &empty),
            Some(digest_bytes(b""))
        );
    }
}
