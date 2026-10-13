//! Generation stages in the `dartforge dev` session: the point where the
//! build engine (`crates/build`) — and, later, the macro host — enter the
//! compilation without reopening `Session::compile`.
//!
//! **Zero cost for those who do not use it** (PLANO.md, governing rule): the
//! session only creates a stage if [`build_stage`] detects builders defined
//! in the resolved packages — once per session.
//! Without a stage, `Session.stages` is empty and the cost per edit is that of
//! iterating over an empty `Vec`.
use dartforge_build::query::{SemanticDb, Query, Digest};
use dartforge_build::{UpdateContext, Demand, Engine, EngineOptions, EngineReport};
use dartforge_elements::generated::{key, Generation};
use dartforge_elements::model::Program;
use dartforge_intern::Interner;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// What the session offers a stage: the program already loaded (it is the
/// `BuildStep.resolver` without an extra load) and the dirty files.
pub struct SessionCtx<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
}

pub struct StageUpdate {
    pub generation: Arc<Generation>,
    /// Natural paths whose generated text changed: only those units are
    /// invalidated.
    pub changed: Vec<PathBuf>,
    pub report: EngineReport,
    pub warnings: Vec<String>,
}

/// Reader of generated outputs by natural path, for `serve` (a `.css`
/// requested by the browser is a demand that materializes the action).
pub type Provider = Arc<dyn Fn(&Path) -> Option<Arc<[u8]>> + Send + Sync>;

pub trait GenerationStage: Send {
    /// Is `p` (a natural path) an expected output? The first load does not
    /// abort with "could not read" for it.
    fn expects(&self, p: &Path) -> bool;
    /// Non-Dart inputs that the stage watches.
    fn watched(&self) -> Vec<PathBuf>;
    /// Watched files that changed since the last look.
    fn changes(&mut self) -> Vec<PathBuf>;
    /// Updates the stage after the edits in `changed`.
    ///
    /// # Errors
    ///
    /// A failure of the engine, with its message.
    fn update(&mut self, ctx: &SessionCtx<'_>, changed: &[PathBuf]) -> Result<StageUpdate, String>;
    fn provider(&self) -> Option<Provider>;
    /// Libraries that enter the program besides the input: the inputs of
    /// the ngdart generator (as `build_runner` resolves each one). Without
    /// them, a component reachable only from its own `.template.dart` (which
    /// does not exist yet) stays out of the program, and the generator
    /// rejects it.
    fn roots(&self) -> Vec<PathBuf> {
        Vec::new()
    }
}

/// The build engine stage.
pub struct BuildStage {
    engine: Arc<Mutex<Engine>>,
    /// Root of the input's package.
    root: PathBuf,
    expected: HashSet<PathBuf>,
    /// Digest of the text of each library (`LibrarySource`), memoized
    /// across compilations and invalidated when one of its units gets dirty.
    sources: Mutex<HashMap<String, Digest>>,
}

/// Detects and builds the build engine stage. `None` = the project uses no
/// builders (nothing is read besides `package_config.json`).
pub fn build_stage(input: &Path, packages: Option<&Path>) -> Option<Result<BuildStage, String>> {
    let path = packages.map(Path::to_path_buf).or_else(|| dartforge_elements::config::PackageConfig::discover(input))?;
    let cfg = dartforge_elements::config::PackageConfig::load(&path).ok()?;
    if !dartforge_build::detect(&cfg) {
        return None;
    }
    let root = dartforge_build::package_root(input)?;
    // The state saved across processes (§4.1 of BUILD-MOTOR.md), only when
    // the user asks for it (`DARTFORGE_BUILD_ESTADO=1`): the session starts
    // from the actions that are still valid.
    let options = EngineOptions { persist: dartforge_build::persistence::requested_in_env(), ..EngineOptions::default() };
    Some(Engine::new(&root, &cfg, options).map(|mut m| {
        // Dart builders on the VM only when asked for (DARTFORGE_BUILD_DART).
        dartforge_build::vm::attach_from_env(&mut m, &path);
        m
    }).map(|m| BuildStage {
        root: root.clone(),
        expected: m.expected_outputs().clone(),
        engine: Arc::new(Mutex::new(m)),
        sources: Mutex::new(HashMap::new()),
    }))
}

impl BuildStage {
    pub fn engine(&self) -> Arc<Mutex<Engine>> {
        self.engine.clone()
    }
}

/// The semantic database over the session's `Program`.
struct SessionDb<'a> {
    program: &'a Program,
    by_uri: HashMap<&'a str, usize>,
    sources: &'a Mutex<HashMap<String, Digest>>,
}

impl SemanticDb for SessionDb<'_> {
    fn digest(&self, c: &Query) -> Option<Digest> {
        match c {
            Query::LibrarySource(uri) => {
                if let Some(d) = self.sources.lock().ok()?.get(uri) {
                    return Some(*d);
                }
                let &i = self.by_uri.get(uri.as_str())?;
                let l = &self.program.libraries[i];
                let mut h = blake3::Hasher::new();
                for &u in &l.units {
                    h.update(self.program.unit(u).source.as_bytes());
                    h.update(&[0]);
                }
                let d = *h.finalize().as_bytes();
                self.sources.lock().ok()?.insert(uri.clone(), d);
                Some(d)
            }
            // Public API (the structural hash of `hashes.rs`, which ignores
            // bodies, comments and whitespace) **plus** the annotation and
            // directive lines of each unit: the API hash does not visit
            // metadata, and for a generator the annotation is everything (a
            // new `@Component` in a file that used to be trivial must wake
            // ngdart up).
            Query::LibraryApi(uri) => {
                let key = format!("api {uri}");
                if let Some(d) = self.sources.lock().ok()?.get(&key) {
                    return Some(*d);
                }
                let &i = self.by_uri.get(uri.as_str())?;
                let lib = dartforge_elements::model::LibraryId(i as u32);
                let mut h = blake3::Hasher::new();
                h.update(&crate::hashes::library_hashes(self.program, lib).api.to_le_bytes());
                for &u in &self.program.libraries[i].units {
                    for line in self.program.unit(u).source.lines() {
                        let t = line.trim_start();
                        if t.starts_with('@')
                            || t.starts_with("import ")
                            || t.starts_with("export ")
                            || t.starts_with("part ")
                            || t.starts_with("library")
                        {
                            h.update(t.as_bytes());
                            h.update(b"\n");
                        }
                    }
                }
                let d = *h.finalize().as_bytes();
                self.sources.lock().ok()?.insert(key, d);
                Some(d)
            }
            // `Declaration` and `Index` come with stage B of ngdart (fine
            // queries); until then nobody registers them.
            _ => None,
        }
    }
}

impl GenerationStage for BuildStage {
    fn roots(&self) -> Vec<PathBuf> {
        let Ok(m) = self.engine.lock() else { return Vec::new() };
        // The plan's ngdart builder: the one of `ngdart` 8 or that of the
        // `ngx_dart` 9 fork.
        let ng_key =
            if m.inputs_of("ngx_dart:ngx_dart").is_empty() { "ngdart:ngdart" } else { "ngx_dart:ngx_dart" };
        let mut v = m.inputs_of(ng_key);
        let hidden = m.outputs_invisible_to(ng_key);
        // Only those in the input package's `lib/` and `web/` (`test/` and the
        // test packages are not part of the application; the dependencies
        // arrive through the libraries the application imports), and only
        // libraries: a part (`part of`) enters through its owner.
        let (lib, web) = (self.root.join("lib"), self.root.join("web"));
        v.retain(|p| {
            (p.starts_with(&lib) || p.starts_with(&web))
                && !hidden.contains(p)
                && std::fs::read_to_string(p)
                    .is_ok_and(|t| !t.lines().any(|l| l.trim_start().starts_with("part of")))
        });
        v
    }

    fn expects(&self, p: &Path) -> bool {
        self.expected.contains(&key(p))
    }

    fn watched(&self) -> Vec<PathBuf> {
        self.engine.lock().map(|m| m.watched()).unwrap_or_default()
    }

    fn changes(&mut self) -> Vec<PathBuf> {
        self.engine.lock().map(|mut m| m.changes()).unwrap_or_default()
    }

    fn update(&mut self, ctx: &SessionCtx<'_>, changed: &[PathBuf]) -> Result<StageUpdate, String> {
        // The `LibrarySource` memo loses the libraries that changed.
        if !changed.is_empty() {
            let dirty: HashSet<PathBuf> = changed.iter().map(|p| key(p)).collect();
            let mut f = self.sources.lock().map_err(|_| "database memo poisoned")?;
            for l in &ctx.program.libraries {
                let is_dirty = l
                    .units
                    .iter()
                    .any(|&u| ctx.program.unit(u).path.as_ref().is_some_and(|p| dirty.contains(&key(p))));
                if is_dirty {
                    f.remove(&l.uri);
                    f.remove(&format!("api {}", l.uri));
                }
            }
        }
        let db = SessionDb {
            program: ctx.program,
            by_uri: ctx.program.libraries.iter().enumerate().map(|(i, l)| (l.uri.as_str(), i)).collect(),
            sources: &self.sources,
        };
        let mut m = self.engine.lock().map_err(|_| "engine poisoned")?;
        let up = m.update(
            &UpdateContext { db: &db, program: Some((ctx.program, ctx.interner)) },
            changed,
            Demand::Loader,
        )?;
        self.expected = m.expected_outputs().clone();
        // D-B2: `build_to: source` outputs of a native generator go to disk
        // when the text changes (like `build_runner watch`).
        for (p, c) in m.native_source_outputs(&up.altered_outputs) {
            if std::fs::read(&p).ok().as_deref() != Some(&c[..]) {
                std::fs::write(&p, &c[..]).map_err(|e| format!("{}: {e}", p.display()))?;
            }
        }
        Ok(StageUpdate { generation: up.generation, changed: up.altered_outputs, report: up.rel, warnings: up.warnings })
    }

    fn provider(&self) -> Option<Provider> {
        let engine = self.engine.clone();
        Some(Arc::new(move |p: &Path| {
            let mut m = engine.lock().ok()?;
            m.materialize(&UpdateContext { db: &dartforge_build::query::NoDb, program: None }, p)
        }))
    }
}
