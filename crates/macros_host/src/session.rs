//! The macro session of a compilation: the three phases, in CFE 3.6.2
//! order, with the program reloaded between them (docs/MACROS-PROTOCOLO.md
//! §4).
//!
//! * phase 1 (types): all applications; the partial text enters the program
//!   before phase 2;
//! * phase 2 (declarations): each non-empty result enters the program before
//!   the next application (the CFE creates one augmentation library per
//!   result: the next macro sees what the previous one declared);
//! * phase 3 (definitions): all of them; the final text enters the program.
//!
//! The text of each library is **a single one**, assembled from all results
//! in execution order (Rules 1–4), and lives in memory as
//! `<library>.macro.dart` (`elements/src/gerado.rs`), attached by the loader
//! after the parts. It is the same text that materialization writes
//! (docs/MACROS-COMPATIBILIDADE.md).
//!
//! With a [`MacroCache`] ([`apply_incremental`]), each `(application,
//! phase)` is revalidated before going to the executor, and the executor is
//! only started if some application actually needs to run (`crate::cache`).
//! With [`LoadByDiff`], each reload reuses the units already analyzed and
//! only analyzes again the augmentation that changed.
use crate::applications::{Application, Target, detect, order};
use crate::assembly::{ExecutionResult, Header, assemble};
use crate::cache::{
    ApplicationIdentity, Digest, InstanceKey, MacroCache, Recorder, Revalidation, implementation_identity, prune, record,
    request_digest, revalidate,
};
use crate::executor::{ExecutionRequest, MacroExecutor, Phase, QueryError, QueryService};
use crate::model::{Key, Table, View};
use crate::queries::{ProgramResolver, QueryHandler};
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::gerado::{Construtor, Geracao};
use dartforge_elements::model::Program;
use dartforge_elements::unidades::CacheUnidades;
use dartforge_intern::Interner;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static SESSIONS: AtomicUsize = AtomicUsize::new(0);

/// How many macro sessions this process has opened — the zero-cost gate
/// requires 0 for a program without macro applications (like
/// `dartforge_build::instance_count`).
pub fn sessions() -> usize {
    SESSIONS.load(Ordering::Relaxed)
}

/// Reloads the program with the given generated sources.
pub type Load<'c> = dyn FnMut(&mut Interner, Option<Arc<Geracao>>) -> (Program, Vec<Diagnostic>) + 'c;

/// Reloads the program with the given generated sources, reusing the units
/// kept in the [`CacheUnidades`] (the one from `dartforge_elements`, the
/// same as in the resident `dartforge dev` session): the macro session
/// stores there the units of the previous program, takes out the macro
/// augmentation (which changed) and the loader only analyzes again what is
/// not stored. The `Interner` is always the session's own, a condition of
/// the unit cache.
pub type LoadByDiff<'c> =
    dyn FnMut(&mut Interner, Option<Arc<Geracao>>, &mut CacheUnidades) -> (Program, Vec<Diagnostic>) + 'c;

/// The assembled text of a library.
#[derive(Debug, Clone)]
pub struct GeneratedText {
    pub library: String,
    /// `<library>.macro.dart`, next to it.
    pub path: PathBuf,
    pub text: String,
}

/// What a macro session produced (`crates/cli` and `crates/emit_js` read
/// `texts`, `executed_macros` and `warnings`).
pub struct Output {
    pub program: Program,
    pub generation: Option<Arc<Geracao>>,
    /// The texts of the augmented libraries.
    pub texts: Vec<GeneratedText>,
    /// Executions actually sent to the executor (those reused from the cache
    /// do not count).
    pub executed_macros: usize,
    /// Warnings and infos reported by the macros.
    pub warnings: Vec<Diagnostic>,
    /// The expansions, in the order they entered the program — executed or
    /// reused. An incremental compilation must repeat the order of a clean
    /// one.
    pub expansions: Vec<Expansion>,
    /// Where the session spent its time (docs/MACROS-PROTOCOLO.md §8).
    pub measurement: Measurement,
}

/// A macro application in a phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expansion {
    pub application: ApplicationIdentity,
    pub phase: Phase,
    /// Came from the cache, without talking to the executor.
    pub reused: bool,
}

/// The time of a macro session, split by stage, and the counters that
/// explain each share. It measures performance, not correctness: no result
/// depends on these numbers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Measurement {
    /// Handshake and `macro.instanciar`.
    pub startup: Duration,
    /// The model of the `macro.executar` request (target and member
    /// prefetch).
    pub model: Duration,
    /// `macro.executar` up to `macro.resultado`, **without** the time spent
    /// serving queries: the executor running the macro, plus the transport.
    pub execution: Duration,
    /// Serving `macro.consulta` in the host.
    pub queries: Duration,
    /// Revalidating the cache: the request fingerprint and the recorded
    /// queries replayed against the current program.
    pub revalidation: Duration,
    /// Assembly of the augmentation texts (every time).
    pub assembly: Duration,
    /// Reload of the program with the augmentation (every time).
    pub reload: Duration,
    pub executions: usize,
    pub query_count: usize,
    /// Expansions that came from the cache.
    pub reused: usize,
    /// Queries replayed during revalidation.
    pub revalidated_queries: usize,
    /// Expansions that ran again and gave the same result as before (the
    /// following ones still hold: the program they see does not change).
    pub rerun_unchanged: usize,
    /// The executor was started in this session.
    pub executor_started: bool,
    /// How many times the assembly ran and how many results it went through
    /// in total: the accumulated reassembly of phase 2 makes this number grow
    /// with the square of the applications.
    pub assemblies: usize,
    pub assembled_results: usize,
    pub reloads: usize,
    /// Units that the reloads reused instead of analyzing again.
    pub reused_units: usize,
}

impl Measurement {
    /// A readable line, for reports and for the measurement test.
    pub fn summary(&self) -> String {
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        format!(
            "startup {:.1} ms{}; model {:.1} ms; execution {:.1} ms ({} executions); queries {:.1} ms ({}); \
             revalidation {:.1} ms ({} reused, {} queries replayed, {} rerun unchanged); \
             assembly {:.1} ms ({} assemblies, {} results); reload {:.1} ms ({} reloads, {} units reused)",
            ms(self.startup),
            if self.executor_started { "" } else { " (executor not started)" },
            ms(self.model),
            ms(self.execution),
            self.executions,
            ms(self.queries),
            self.query_count,
            ms(self.revalidation),
            self.reused,
            self.revalidated_queries,
            self.rerun_unchanged,
            ms(self.assembly),
            self.assemblies,
            self.assembled_results,
            ms(self.reload),
            self.reloads,
            self.reused_units
        )
    }
}

/// Measures the time spent serving queries around another service.
struct Timed<'s> {
    inner: &'s mut dyn QueryService,
    elapsed: Duration,
    count: usize,
}

impl QueryService for Timed<'_> {
    fn query(&mut self, kind: &str, args: &Value) -> Result<Value, QueryError> {
        let t = Instant::now();
        let r = self.inner.query(kind, args);
        self.elapsed += t.elapsed();
        self.count += 1;
        r
    }
}

/// Does the macro implement the phase interface for the target's kind? (the
/// reply to `macro.instanciar` carries the interface names).
fn applies_in_phase(interfaces: &[String], target: &Target, phase: Phase) -> bool {
    let kind = match target {
        Target::Library(_) => "Library",
        Target::Declaration(Key::Type { .. }) => "Class",
        Target::Declaration(Key::Method { .. }) => "Method",
        Target::Declaration(Key::Constructor { .. }) => "Constructor",
        Target::Declaration(Key::Field { .. }) => "Field",
        Target::Declaration(Key::TopLevelFunction { .. }) => "Function",
        Target::Declaration(Key::TopLevelVariable { .. }) => "Variable",
        _ => return false,
    };
    let suffix = match phase {
        Phase::Types => "TypesMacro",
        Phase::Declarations => "DeclarationsMacro",
        Phase::Definitions => "DefinitionMacro",
    };
    // Mixins and enums are `Key::Type` too; the executor picks the method by
    // the target's real kind.
    let alternatives: &[&str] = match kind {
        "Class" => &["Mixin", "Enum"],
        _ => &[],
    };
    interfaces
        .iter()
        .any(|i| *i == format!("{kind}{suffix}") || alternatives.iter().any(|a| *i == format!("{a}{suffix}")))
}

fn is_non_empty(r: &ExecutionResult) -> bool {
    !(r.enum_values.is_empty()
        && r.extends.is_empty()
        && r.interfaces.is_empty()
        && r.library.is_empty()
        && r.mixins.is_empty()
        && r.types.is_empty())
}

fn diagnostic(app: &Application, msg: &str) -> Diagnostic {
    let class = app.macro_.rsplit('#').next().unwrap_or("");
    Diagnostic::new(format!("{}: application of @{class}: {msg}", app.unit), app.span)
}

/// The texts assembled from `results` (grouped by library, in execution
/// order) and the generation that contains them, plus the sources of
/// `base`.
fn generate(
    results: &[(String, ExecutionResult)],
    table: &mut Table,
    program: &Program,
    interner: &Interner,
    base: Option<&Arc<Geracao>>,
) -> Result<(Vec<GeneratedText>, Arc<Geracao>), String> {
    let mut by_library: Vec<(String, Vec<ExecutionResult>)> = Vec::new();
    for (lib, r) in results {
        match by_library.iter_mut().find(|(l, _)| l == lib) {
            Some((_, v)) => v.push(r.clone()),
            None => by_library.push((lib.clone(), vec![r.clone()])),
        }
    }
    let view = View { program, interner };
    let cell = RefCell::new(table);
    let resolver = ProgramResolver { view: &view, table: &cell };
    let mut texts = Vec::new();
    for (lib, rs) in by_library {
        let text = assemble(&rs, &resolver, Header::AugmentationLibrary(&lib))?;
        let lib_id = view.library_by_uri(&lib).ok_or_else(|| format!("library {lib} disappeared from the program"))?;
        let main = program.library(lib_id).units.first().and_then(|u| program.unit(*u).path.clone());
        let main = main.ok_or_else(|| format!("library {lib} has no file"))?;
        let path = dartforge_elements::load::caminho_da_augmentation_de_macro(&main);
        texts.push(GeneratedText { library: lib, path, text });
    }
    let mut c = Construtor::nova();
    if let Some(b) = base {
        for (p, f) in b.iter() {
            c.por(p.clone(), f.conteudo.clone(), f.gerador, f.entradas.to_vec());
        }
    }
    for t in &texts {
        c.por(t.path.clone(), t.text.as_str(), "macros", vec![]);
    }
    let id = base.map_or(0, |b| b.id) + 1;
    let g = c.concluir(id).map_err(|e| e.join("; "))?;
    Ok((texts, g))
}

/// Applications that still need to run; libraries with a materialized
/// augmentation already have their members in the loaded program.
pub fn pending_applications(program: &Program, interner: &Interner) -> Vec<Application> {
    detect(&View { program, interner })
        .into_iter()
        .filter(|a| !already_materialized(program, &a.library))
        .collect()
}

/// Applies the macros of `program` with `executor`. Without applications, it
/// returns the program as it came — without opening a session (zero cost).
/// On error (executor unavailable, an error diagnostic from a macro, text
/// that does not compile), it returns the diagnostics, each one pointing at
/// the annotation.
///
/// Without a cache and with a full reload; [`apply_incremental`] is the
/// form with the expansion cache and the diff-based reload.
pub fn apply(
    program: Program,
    interner: &mut Interner,
    base: Option<Arc<Geracao>>,
    load: &mut Load<'_>,
    executor: &mut dyn MacroExecutor,
) -> Result<Output, Vec<Diagnostic>> {
    let mut cache = MacroCache::disabled();
    run_session(program, interner, base, Reload::Full(load), executor, &mut cache)
}

/// Like [`apply`], with the diff-based reload ([`LoadByDiff`]) and, if
/// given, the expansion cache: what the previous compilation executed and
/// still holds is reused without talking to the executor, which is only
/// started if some application needs to run. The result — texts, order of
/// the expansions and diagnostics — is that of a clean compilation.
///
/// # Errors
///
/// The same as [`apply`].
pub fn apply_incremental(
    program: Program,
    interner: &mut Interner,
    base: Option<Arc<Geracao>>,
    load: &mut LoadByDiff<'_>,
    executor: &mut dyn MacroExecutor,
    cache: Option<&mut MacroCache>,
) -> Result<Output, Vec<Diagnostic>> {
    let mut disabled;
    let cache = match cache {
        Some(c) => c,
        None => {
            disabled = MacroCache::disabled();
            &mut disabled
        }
    };
    run_session(program, interner, base, Reload::ByDiff(load, CacheUnidades::nova()), executor, cache)
}

/// How the session reloads the program.
enum Reload<'r, 'c> {
    /// A whole load every time.
    Full(&'r mut Load<'c>),
    /// Reusing the units of the previous program.
    ByDiff(&'r mut LoadByDiff<'c>, CacheUnidades),
}

impl Reload<'_, '_> {
    /// Replaces `program` with the program with generation `g`; `texts` are
    /// the macro augmentations, which changed and cannot be reused.
    fn reload(
        &mut self,
        program: &mut Program,
        interner: &mut Interner,
        g: Arc<Geracao>,
        texts: &[GeneratedText],
        measurement: &mut Measurement,
    ) -> Vec<Diagnostic> {
        let (p, d) = match self {
            Reload::Full(load) => load(interner, Some(g)),
            Reload::ByDiff(load, units) => {
                units.recolher(std::mem::take(program));
                for t in texts {
                    units.invalidar(&t.path);
                }
                let r = load(interner, Some(g), units);
                measurement.reused_units += units.reaproveitadas;
                r
            }
        };
        *program = p;
        d
    }
}

/// The executor, started only when some application needs it.
struct LazyExecutor<'e> {
    executor: &'e mut dyn MacroExecutor,
    started: bool,
    /// One instance per (macro, constructor, arguments), like the CFE.
    instances: HashMap<InstanceKey, (u64, Vec<String>)>,
}

impl LazyExecutor<'_> {
    /// Handshake and one instance per distinct key, in the order of `apps`
    /// (the same message sequence as always in a clean compilation).
    fn ensure_started(&mut self, apps: &[Application], keys: &[InstanceKey], measurement: &mut Measurement) -> Result<(), Vec<Diagnostic>> {
        if self.started {
            return Ok(());
        }
        let t = Instant::now();
        self.started = true;
        measurement.executor_started = true;
        if let Err(e) = self.executor.start() {
            self.shut_down();
            return Err(apps.iter().map(|a| diagnostic(a, &e)).collect());
        }
        for (a, k) in apps.iter().zip(keys) {
            if self.instances.contains_key(k) {
                continue;
            }
            match self.executor.instantiate(&a.macro_, &a.constructor, &a.arguments) {
                Ok(x) => {
                    self.instances.insert(k.clone(), x);
                }
                Err(e) => {
                    self.shut_down();
                    return Err(vec![diagnostic(a, &e)]);
                }
            }
        }
        measurement.startup += t.elapsed();
        Ok(())
    }

    fn shut_down(&mut self) {
        if self.started {
            self.executor.shut_down();
            self.started = false;
        }
    }
}

/// The session: the three phases over `program`, with `cache` (disabled in
/// a compilation without a cache).
fn run_session(
    program: Program,
    interner: &mut Interner,
    base: Option<Arc<Geracao>>,
    mut reload: Reload<'_, '_>,
    executor: &mut dyn MacroExecutor,
    cache: &mut MacroCache,
) -> Result<Output, Vec<Diagnostic>> {
    // A library that already includes the materialized augmentation
    // (`import augment 'x.macro.dart'` or `part`, written by `dartforge macros
    // --materializar`) does not run the macros again: like a `.g.dart` on disk.
    let apps = pending_applications(&program, interner);
    if apps.is_empty() {
        return Ok(Output {
            program,
            generation: base,
            texts: Vec::new(),
            executed_macros: 0,
            warnings: Vec::new(),
            expansions: Vec::new(),
            measurement: Measurement::default(),
        });
    }
    SESSIONS.fetch_add(1, Ordering::Relaxed);
    let mut measurement = Measurement::default();
    let MacroCache { enabled, executor_identity, table, statics, interfaces: recorded_interfaces, records } = cache;
    let enabled = *enabled;

    // The stable identity of each application and that of its instance; the
    // implementation enters through the source of the closure of the macro's
    // library, read from the program still without any macro augmentation.
    let identities = ApplicationIdentity::of_all(&apps);
    let mut implementations: HashMap<&str, Digest> = HashMap::new();
    let keys: Vec<InstanceKey> = apps
        .iter()
        .map(|a| {
            let uri = a.macro_.split_once('#').map_or(a.macro_.as_str(), |(u, _)| u);
            let implementation = if enabled {
                *implementations.entry(uri).or_insert_with(|| implementation_identity(&program, uri))
            } else {
                [0; 32]
            };
            InstanceKey {
                implementation,
                macro_: a.macro_.clone(),
                constructor: a.constructor.clone(),
                arguments: a.arguments.to_string(),
            }
        })
        .collect();

    let mut executor = LazyExecutor { executor, started: false, instances: HashMap::new() };
    let mut results: Vec<(String, ExecutionResult)> = Vec::new();
    let mut errors: Vec<Diagnostic> = Vec::new();
    let mut warnings: Vec<Diagnostic> = Vec::new();
    let mut expansions: Vec<Expansion> = Vec::new();
    let mut used: HashSet<(ApplicationIdentity, Phase)> = HashSet::new();
    let mut executed = 0usize;
    let mut program = program;
    let mut generation = base.clone();

    let mut reload_with = |program: &mut Program,
                           generation: &mut Option<Arc<Geracao>>,
                           interner: &mut Interner,
                           table: &mut Table,
                           results: &[(String, ExecutionResult)],
                           measurement: &mut Measurement|
     -> Result<Vec<GeneratedText>, Vec<Diagnostic>> {
        let t = Instant::now();
        let (texts, g) = generate(results, table, program, interner, base.as_ref())
            .map_err(|e| vec![Diagnostic::new(format!("assembling the macro augmentation: {e}"), dartforge_diagnostics::Span { start: 0, end: 0 })])?;
        measurement.assembly += t.elapsed();
        measurement.assemblies += 1;
        measurement.assembled_results += results.len();
        let t = Instant::now();
        let d = reload.reload(program, interner, g.clone(), &texts, measurement);
        measurement.reload += t.elapsed();
        measurement.reloads += 1;
        if !d.is_empty() {
            return Err(d);
        }
        *generation = Some(g);
        Ok(texts)
    };

    let mut texts = Vec::new();
    for phase in Phase::ALL {
        let sequence = order(phase, &apps, &View { program: &program, interner });
        let mut changed = false;
        for i in sequence {
            let app = &apps[i];
            // The interfaces decide the phases: from the executor, if it is
            // already started; otherwise, from the cache; otherwise, it is
            // time to start it.
            let interfaces = match executor.instances.get(&keys[i]) {
                Some((_, itf)) => itf.clone(),
                None => match recorded_interfaces.get(&keys[i]).filter(|_| enabled) {
                    Some(itf) => itf.clone(),
                    None => {
                        executor.ensure_started(&apps, &keys, &mut measurement)?;
                        if enabled {
                            for (k, (_, itf)) in &executor.instances {
                                recorded_interfaces.insert(k.clone(), itf.clone());
                            }
                        }
                        executor.instances[&keys[i]].1.clone()
                    }
                },
            };
            if !applies_in_phase(&interfaces, &app.target, phase) {
                continue;
            }
            let record_key = (identities[i].clone(), phase);
            let result = {
                let view = View { program: &program, interner };
                let t = Instant::now();
                let (target, model) = match target_and_model(&view, table, &app.target) {
                    Ok(x) => x,
                    Err(e) => {
                        errors.push(diagnostic(app, &e));
                        continue;
                    }
                };
                measurement.model += t.elapsed();
                let request = enabled.then(|| {
                    let t = Instant::now();
                    let d = request_digest(executor_identity, &keys[i], phase, &target, &model);
                    measurement.revalidation += t.elapsed();
                    d
                });
                // Revalidation: the recorded queries replayed against the
                // current program.
                let reused = match &request {
                    Some(d) => {
                        let t = Instant::now();
                        let mut handler = QueryHandler { view: &view, table: &mut *table, statics: &mut *statics };
                        let r = revalidate(records, &record_key, d, &mut handler);
                        measurement.revalidation += t.elapsed();
                        match r {
                            Revalidation::Valid { result, replayed } => {
                                measurement.revalidated_queries += replayed;
                                Some(*result)
                            }
                            Revalidation::QueryChanged { replayed } => {
                                measurement.revalidated_queries += replayed;
                                None
                            }
                            Revalidation::Missing | Revalidation::RequestChanged => None,
                        }
                    }
                    None => None,
                };
                used.insert(record_key.clone());
                match reused {
                    Some(r) => {
                        measurement.reused += 1;
                        expansions.push(Expansion { application: identities[i].clone(), phase, reused: true });
                        Ok(r)
                    }
                    None => {
                        executor.ensure_started(&apps, &keys, &mut measurement)?;
                        let instance = executor.instances[&keys[i]].0;
                        let execution_request = ExecutionRequest { instance, phase, target, model };
                        let mut handler = QueryHandler { view: &view, table: &mut *table, statics: &mut *statics };
                        let mut timed = Timed { inner: &mut handler, elapsed: Duration::ZERO, count: 0 };
                        let mut recorder = Recorder::new(&mut timed, enabled);
                        executed += 1;
                        let t = Instant::now();
                        let r = executor.executor.execute(&execution_request, &mut recorder);
                        let elapsed = t.elapsed();
                        let queries = std::mem::take(&mut recorder.queries);
                        drop(recorder);
                        measurement.execution += elapsed.saturating_sub(timed.elapsed);
                        measurement.queries += timed.elapsed;
                        measurement.query_count += timed.count;
                        if let (Ok(r), Some(d)) = (&r, request)
                            && record(records, record_key, d, queries, r)
                        {
                            measurement.rerun_unchanged += 1;
                        }
                        expansions.push(Expansion { application: identities[i].clone(), phase, reused: false });
                        r
                    }
                }
            };
            let r = match result {
                Ok(r) => r,
                Err(e) => {
                    errors.push(diagnostic(app, &e));
                    continue;
                }
            };
            report(app, &r, &mut errors, &mut warnings);
            if is_non_empty(&r) {
                results.push((app.library.clone(), r));
                changed = true;
                // Phase 2: the next application sees what this one declared.
                if phase == Phase::Declarations && errors.is_empty() {
                    texts = match reload_with(&mut program, &mut generation, interner, table, &results, &mut measurement) {
                        Ok(texts) => texts,
                        Err(diagnostics) => {
                            executor.shut_down();
                            return Err(diagnostics);
                        }
                    };
                    changed = false;
                }
            }
        }
        if !errors.is_empty() {
            executor.shut_down();
            return Err(errors);
        }
        if changed {
            texts = match reload_with(&mut program, &mut generation, interner, table, &results, &mut measurement) {
                Ok(texts) => texts,
                Err(diagnostics) => {
                    executor.shut_down();
                    return Err(diagnostics);
                }
            };
        }
    }
    executor.shut_down();
    if enabled {
        prune(records, recorded_interfaces, &used, &keys.iter().cloned().collect());
    }
    measurement.executions = executed;
    Ok(Output {
        program,
        generation,
        texts,
        executed_macros: executed,
        warnings,
        expansions,
        measurement,
    })
}

/// The target as JSON and the prefetch model: the target's library and, for
/// a class, its members (`@JsonCodable` makes no round trip).
fn target_and_model(v: &View<'_>, t: &mut Table, target: &Target) -> Result<(Value, Value), String> {
    match target {
        Target::Library(uri) => {
            let lib = v.library_by_uri(uri).ok_or_else(|| format!("library {uri} not found"))?;
            let b = v.library_json(t, lib);
            Ok((b.clone(), json!({"bibliotecas": [b]})))
        }
        Target::Declaration(c) => {
            let d = v.declaration_json(t, c).ok_or_else(|| format!("target {} not supported or not found", c.name()))?;
            let lib = match c {
                Key::Type { lib, .. } | Key::Method { lib, .. } | Key::Field { lib, .. } | Key::Constructor { lib, .. }
                | Key::TopLevelFunction { lib, .. } | Key::TopLevelVariable { lib, .. } => lib,
                _ => return Err(format!("target {} not supported yet", c.name())),
            };
            let lib_id = v.library_by_uri(lib).ok_or_else(|| format!("library {lib} not found"))?;
            let libraries = json!([v.library_json(t, lib_id)]);
            let mut members = serde_json::Map::new();
            let owner = match c {
                Key::Type { .. } => Some(c.clone()),
                Key::Method { lib, owner, .. } | Key::Field { lib, owner, .. } | Key::Constructor { lib, owner, .. } => {
                    Some(Key::Type { lib: lib.clone(), name: owner.clone() })
                }
                _ => None,
            };
            if let Some(owner) = owner {
                if let Some(cid) = v.class_of(&owner) {
                    let id = t.id(owner);
                    members.insert(
                        id.to_string(),
                        json!({
                            "campos": v.members_json(t, cid, "campos"),
                            "metodos": v.members_json(t, cid, "metodos"),
                            "construtores": v.members_json(t, cid, "construtores"),
                        }),
                    );
                }
            }
            Ok((d, json!({"bibliotecas": libraries, "membros": members})))
        }
    }
}

/// Diagnostics and exception of a result, pointing at the annotation.
fn report(app: &Application, r: &ExecutionResult, errors: &mut Vec<Diagnostic>, warnings: &mut Vec<Diagnostic>) {
    for d in &r.diagnostics {
        let text = d["mensagem"]["texto"].as_str().unwrap_or("");
        let correction = d["correcao"].as_str().map(|c| format!(" ({c})")).unwrap_or_default();
        let context: Vec<&str> = d["contexto"].as_array().map(|l| l.iter().filter_map(|m| m["texto"].as_str()).collect()).unwrap_or_default();
        let mut msg = format!("{text}{correction}");
        for c in context {
            msg.push_str("\n  ");
            msg.push_str(c);
        }
        match d["severidade"].as_str() {
            Some("error") => errors.push(diagnostic(app, &msg)),
            _ => warnings.push(diagnostic(app, &msg)),
        }
    }
    if let Some(e) = &r.exception {
        let kind = e["tipo"].as_str().unwrap_or("");
        let msg = e["mensagem"].as_str().unwrap_or("");
        errors.push(diagnostic(app, &format!("exception ({kind}): {msg}")));
    }
}

/// Does library `uri` already have, among the units loaded from disk, the
/// materialized augmentation `<library>.macro.dart`?
fn already_materialized(program: &Program, uri: &str) -> bool {
    let Some(lib) = program.libraries.iter().find(|l| l.uri == uri) else { return false };
    let Some(main) = lib.units.first().and_then(|u| program.unit(*u).path.clone()) else { return false };
    let target = dartforge_elements::gerado::chave(&dartforge_elements::load::caminho_da_augmentation_de_macro(&main));
    lib.units.iter().skip(1).any(|u| program.unit(*u).path.as_deref().map(dartforge_elements::gerado::chave).as_ref() == Some(&target))
}
