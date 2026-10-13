//! The engine: plan + graph + per-action fingerprint + deterministic
//! schedule + executors, publishing into an in-memory `Geracao`.
//!
//! * Phases run in series; the actions of a phase run in parallel
//!   (`std::thread::scope`, atomic index, up to `trabalhadores`), with results
//!   gathered in action order — the result does not depend on the number of
//!   workers.
//! * An action that already ran only runs again if the digest of some query
//!   it made changed (revalidation), and an output equal to the previous one
//!   invalidates nobody (output cutoff).
//! * Laziness (D-B6): under [`Demanda::Carregador`] only the outputs the
//!   loader can import (`.dart`) and the `build_to: source` ones are
//!   computed; the rest (a served `.css`, `--comparar`) goes through
//!   [`Motor::materializar`] or [`Demanda::Tudo`]. With the Dart executor,
//!   the hidden phases it runs are included too (a later builder may read
//!   them); only the optional ones stay lazy.
use crate::consulta::{BancoSemantico, Consulta, Digest, digest_bytes};
use crate::executor::{
    ActionService, CtxGerador, Disponibilidade, ExecutorDart, GeradorNativo, Indisponivel,
    NativeAction, PedidoAcao, PedidoExtensoes, PedidoNativo, ScriptDeBuilders, ServicoBuildStep,
    glob_candidates, query_digest,
};
use crate::grafo::{AssetId, Grafo};
use crate::pacotes::GrafoPacotes;
use crate::plano::{Configs, Fase, NoAlvo, Plano, fases};
use dartforge_elements::config::PackageConfig;
use dartforge_elements::gerado::{Construtor, Geracao, chave};
use dartforge_elements::model::Program;
use dartforge_elements::unidades::Marca;
use dartforge_intern::Interner;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct OpcoesMotor {
    pub release: bool,
    pub trabalhadores: usize,
    /// A stale fallback becomes an error (D-B5, `dartforge build --estrito`).
    pub estrito: bool,
    /// Also runs the native generators that are not verified yet, only to
    /// measure them (`--comparar`).
    pub medir_nao_verificados: bool,
    /// Reads and writes the state across processes in
    /// `.dart_tool/dartforge/build/estado` (§4.1). Off by default: only when
    /// the user asks for it (`dartforge build --estado`,
    /// `DARTFORGE_BUILD_ESTADO=1`), per governing rule 6 of `PLANO.md`.
    pub persistir: bool,
    /// `--config <name>`: `build.<name>.yaml` in place of the root's
    /// `build.yaml` (targets, `global_options` and `triggers`).
    pub config: Option<String>,
    /// `--define <builder>=<option>=<value>`, on top of the `global_options`.
    pub definicoes: crate::linha_de_comando::Definicoes,
    /// `--build-filter`: only the actions with an output some filter matches
    /// run from the start; the others only if a running step reads their
    /// output (`shouldBuildForDirs` and the official on-demand build).
    pub filtros: Vec<crate::linha_de_comando::FiltroBuild>,
}

impl Default for OpcoesMotor {
    fn default() -> Self {
        OpcoesMotor {
            release: false,
            trabalhadores: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .min(8),
            estrito: false,
            medir_nao_verificados: false,
            persistir: false,
            config: None,
            definicoes: Default::default(),
            filtros: Vec::new(),
        }
    }
}

/// An action's queries with the digest of each answer.
type Queries = Vec<(Consulta, Option<Digest>)>;

/// Who produced an output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origem {
    Nativo(&'static str),
    Dart,
    Apoio,
    Pendente,
    /// The builder (or the executor, in the middle of the action) failed: no
    /// outputs, and the whole generation fails (DF-BUILD-005).
    Falha,
    /// A generated input that was not written (or failed): the official tool
    /// does not run the step (`_runPostProcessAction` and
    /// `_matchingPrimaryInputs`, `wasOutput`).
    Omitida,
    /// `run_only_if_triggered: true` and no trigger fired
    /// ([`crate::gatilhos`]): the step does not run, and nothing is written.
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
pub struct Registro {
    pub fingerprint: Digest,
    pub consultas: Vec<(Consulta, Option<Digest>)>,
    /// Expected outputs and their content (`None` = not written).
    pub saidas: Vec<(AssetId, Option<Arc<[u8]>>)>,
    pub origem: Origem,
    /// Reason it is pending (or why the native generator refused, when the
    /// fallback supplied it).
    pub motivo: Option<String>,
    /// Measurement of an unverified native generator (only with
    /// `medir_nao_verificados`).
    pub measured: Vec<(AssetId, Arc<[u8]>)>,
    /// In a native action, the outputs the native generator does not write
    /// and that came from the fallback (the `.css.dart` the official ngdart
    /// writes next to the `.css.shim.dart`): they count as pending, not as
    /// equal.
    pub from_fallback: Vec<AssetId>,
    /// Post-processor: inputs marked by `deletePrimaryInput`.
    pub apagados: Vec<AssetId>,
    /// Came from the state saved by another process and has not been checked
    /// in this session yet: the first verification recomputes **all**
    /// queries, not only the ones the events touch (§4.1).
    pub restored: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Demanda {
    /// What the loader can import (`.dart`) and the `source` outputs.
    Carregador,
    /// Everything (`dartforge build`, `--comparar`).
    Tudo,
}

/// Report of one update (goes into the `dev` crate's `Relatorio`).
#[derive(Debug, Clone, Default)]
pub struct RelMotor {
    pub checked_actions: usize,
    pub acoes_executadas: usize,
    pub reevaluated_queries: usize,
    pub saidas_alteradas: usize,
    pub nativas: usize,
    pub dart: usize,
    /// Units actually regenerated inside a per-package generator.
    pub unidades_nativas: usize,
    /// Queries recorded by generators in this update.
    pub consultas_gerador: usize,
    pub apoio: usize,
    /// Of the fallback actions, those that produced content read from
    /// `build_runner`'s disk (for the others, the official tool wrote nothing
    /// there).
    pub apoio_com_saida: usize,
    pub pendentes_por_motivo: BTreeMap<String, usize>,
    /// Steps with `run_only_if_triggered` that no trigger fired.
    pub not_triggered: usize,
    /// Actions whose builder failed in this update: the generation failed.
    pub falhas: Vec<String>,
    pub tempo: Duration,
    /// Revalidation of the per-package rounds (digests of the affected
    /// queries).
    pub revalidation_time: Duration,
    /// Per-package native generators (ngdart's stage A).
    pub tempo_nativo: Duration,
}

impl RelMotor {
    /// Adds a following pass (the on-demand build of `--build-filter`
    /// repeats the update).
    fn add(&mut self, o: RelMotor) {
        self.checked_actions += o.checked_actions;
        self.acoes_executadas += o.acoes_executadas;
        self.reevaluated_queries += o.reevaluated_queries;
        self.nativas += o.nativas;
        self.dart += o.dart;
        self.unidades_nativas += o.unidades_nativas;
        self.consultas_gerador += o.consultas_gerador;
        self.apoio += o.apoio;
        self.apoio_com_saida += o.apoio_com_saida;
        for (m, n) in o.pendentes_por_motivo {
            *self.pendentes_por_motivo.entry(m).or_default() += n;
        }
        self.not_triggered += o.not_triggered;
        self.falhas.extend(o.falhas);
        self.tempo += o.tempo;
        self.revalidation_time += o.revalidation_time;
        self.tempo_nativo += o.tempo_nativo;
    }

    pub fn texto(&self) -> String {
        let mut s = format!(
            "engine: {} actions checked, {} run ({} native, {} Dart, {} fallback, {} units regenerated), {} queries reevaluated, {} generator queries, {} outputs changed, {:.1} ms (revalidation {:.1} ms, per-package native {:.1} ms)",
            self.checked_actions,
            self.acoes_executadas,
            self.nativas,
            self.dart,
            self.apoio,
            self.unidades_nativas,
            self.reevaluated_queries,
            self.consultas_gerador,
            self.saidas_alteradas,
            self.tempo.as_secs_f64() * 1000.0,
            self.revalidation_time.as_secs_f64() * 1000.0,
            self.tempo_nativo.as_secs_f64() * 1000.0,
        );
        if self.not_triggered > 0 {
            s.push_str(&format!("\n  not triggered: {}", self.not_triggered));
        }
        for (m, n) in &self.pendentes_por_motivo {
            s.push_str(&format!("\n  pending ({n}): {m}"));
        }
        for f in &self.falhas {
            s.push_str(&format!("\n  failure: {f}"));
        }
        s
    }
}

pub struct Atualizacao {
    pub geracao: Arc<Geracao>,
    /// Natural paths whose content changed (or disappeared) in this update.
    pub alterados: Vec<PathBuf>,
    pub rel: RelMotor,
    pub avisos: Vec<String>,
}

/// Context of an update: the semantic database and, in a session, the
/// already loaded program (`BuildStep.resolver` without an extra load).
pub struct Contexto<'a> {
    pub banco: &'a dyn BancoSemantico,
    pub programa: Option<(&'a Program, &'a Interner)>,
}

/// One run of a per-package generator in a round.
struct PackageRound {
    queries: Vec<(Consulta, Option<Digest>)>,
    outputs: BTreeMap<PathBuf, Arc<[u8]>>,
    refusals: BTreeMap<PathBuf, String>,
    error: Option<String>,
    generated_units: usize,
}

pub struct Motor {
    pub raiz: PathBuf,
    pub grafo_pacotes: GrafoPacotes,
    pub configs: Configs,
    pub plano: Plano,
    pub fases: Vec<Fase>,
    pub alvos: Vec<NoAlvo>,
    pub grafo: Grafo,
    pub opcoes: OpcoesMotor,
    natives: Vec<Arc<dyn GeradorNativo>>,
    dart: std::sync::Mutex<Box<dyn ExecutorDart>>,
    dart_prepared: AtomicBool,
    /// The run-time extensions were already checked with the Dart executor
    /// ([`Motor::check_extensions`]).
    extensions_checked: bool,
    records: Vec<Option<Registro>>,
    /// Actions per phase (indices into `grafo.acoes`).
    per_phase: Vec<Vec<usize>>,
    /// Rounds of the per-package generators: (generator, package).
    package_rounds: HashMap<(usize, Arc<str>), PackageRound>,
    memory: HashMap<PathBuf, Arc<[u8]>>,
    /// Output → natural path (the generation key).
    pub naturais: BTreeMap<AssetId, PathBuf>,
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
    marks: HashMap<PathBuf, Option<Marca>>,
    /// What [`Motor::mudancas`] looks at (recomputed on every update).
    watched: Vec<PathBuf>,
    generation: Arc<Geracao>,
    warned: HashSet<PathBuf>,
    labels: HashMap<String, &'static str>,
    /// Outputs that the graph rebuild removed from memory (the action or the
    /// anchor no longer exists): they join the update's changed paths, so
    /// that the generation is republished without them.
    removed: Vec<PathBuf>,
    /// Saved state read at startup, applied on the first update (after the
    /// extensions and the anchors were checked with the Dart executor).
    to_restore: Option<crate::persistencia::Estado>,
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

impl Drop for Motor {
    fn drop(&mut self) {
        if let Ok(mut dart) = self.dart.lock() {
            dart.encerrar();
        }
    }
}

fn natural_path(graph: &GrafoPacotes, id: &AssetId) -> PathBuf {
    let root = graph
        .no(&id.pacote)
        .map(|n| n.raiz.clone())
        .unwrap_or_default();
    chave(&root.join(id.caminho.as_ref()))
}

struct Indices {
    per_phase: Vec<Vec<usize>>,
    natural_paths: BTreeMap<AssetId, PathBuf>,
    expected: HashSet<PathBuf>,
    sources: HashSet<PathBuf>,
    dirs: Vec<PathBuf>,
}

fn build_indices(gp: &GrafoPacotes, graph: &Grafo, phase_count: usize) -> Indices {
    let mut per_phase = vec![Vec::new(); phase_count];
    for (i, a) in graph.acoes.iter().enumerate() {
        if a.viva() {
            per_phase[a.fase].push(i);
        }
    }
    let mut natural_paths = BTreeMap::new();
    let mut expected = HashSet::new();
    for a in &graph.acoes {
        for s in &a.saidas {
            let n = natural_path(gp, s);
            if s.caminho.ends_with(".dart") {
                expected.insert(n.clone());
            }
            natural_paths.insert(s.clone(), n);
        }
    }
    let mut sources = HashSet::new();
    let mut dirs = BTreeSet::new();
    let root = &gp.nos[gp.raiz];
    for (package, paths) in &graph.fontes {
        let Some(node) = gp.no(package) else { continue };
        for c in paths {
            if c.contains('$') {
                continue;
            }
            let n = chave(&node.raiz.join(c.as_ref()));
            if node.nome == root.nome {
                let mut d = n.parent();
                while let Some(x) = d {
                    if !x.starts_with(&root.raiz) || x == root.raiz {
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
    Some(chave(&dartforge_elements::config::sem_verbatim(c)))
}

/// Could the query have changed with these events?
fn affected(
    c: &Consulta,
    changed: &HashSet<PathBuf>,
    dart_changed: bool,
    structural: &HashSet<PathBuf>,
) -> bool {
    match c {
        // A listing only changes when a file enters or leaves (a watched
        // directory that changed, or a new/deleted file).
        Consulta::Glob { dir, .. } => structural.iter().any(|m| m.starts_with(dir)),
        Consulta::GlobAtivos {
            dir, candidatos, ..
        } => {
            structural.iter().any(|m| m.starts_with(dir))
                || candidatos
                    .iter()
                    .any(|(rel, _)| changed.contains(&chave(&dir.join(rel))))
        }
        _ => c.caminho().is_some_and(|p| changed.contains(p)) || (c.semantica() && dart_changed),
    }
}

impl Motor {
    /// Builds the engine of a project: reads the configuration and assembles
    /// plan, phases and graph. Only called when [`crate::detectar`] said yes.
    pub fn novo(root: &Path, cfg: &PackageConfig, options: OpcoesMotor) -> Result<Motor, String> {
        crate::contar_instancia();
        let package_graph = GrafoPacotes::montar(root, cfg, None)?;
        let script_configs = Configs::ler(&package_graph, false)?;
        let plan = Plano::do_script(&package_graph, &script_configs)?;
        let configs = Configs::read_with_config(&package_graph, true, options.config.as_deref())?;
        let (phases, targets) = fases(
            &package_graph,
            &configs,
            &plan,
            options.release,
            &options.definicoes,
        )?;
        let graph = Grafo::montar(&package_graph, &configs, &phases, &targets)?;
        let Indices {
            per_phase,
            natural_paths,
            expected,
            sources,
            dirs,
        } = build_indices(&package_graph, &graph, phases.len());
        let mut config_files = vec![
            chave(&package_graph.dir_raiz.join("pubspec.lock")),
            chave(
                &package_graph
                    .dir_raiz
                    .join(".dart_tool")
                    .join("package_config.json"),
            ),
        ];
        let r = &package_graph.nos[package_graph.raiz].raiz;
        config_files.push(chave(&r.join("pubspec.yaml")));
        // Each package's `build.yaml`, whether it exists or not (creating it
        // changes the plan), and the root directory, whose mark changes when
        // a `<package>.build.yaml` override appears or disappears
        // (DF-BUILD-018).
        for node in &package_graph.nos {
            config_files.push(chave(&node.raiz.join("build.yaml")));
        }
        config_files.push(chave(r));
        if let Some(k) = &options.config {
            config_files.push(chave(&r.join(format!("build.{k}.yaml"))));
        }
        if let Ok(ls) = std::fs::read_dir(r) {
            for e in ls.flatten() {
                if e.file_name().to_string_lossy().ends_with(".build.yaml") {
                    config_files.push(chave(&e.path()));
                }
            }
        }
        config_files.sort();
        config_files.dedup();
        let action_count = graph.acoes.len();
        let mut m = Motor {
            raiz: root.to_path_buf(),
            grafo_pacotes: package_graph,
            configs,
            plano: plan,
            fases: phases,
            alvos: targets,
            grafo: graph,
            opcoes: options,
            natives: Vec::new(),
            dart: std::sync::Mutex::new(Box::new(Indisponivel::default())),
            dart_prepared: AtomicBool::new(false),
            extensions_checked: false,
            records: vec![None; action_count],
            per_phase,
            package_rounds: HashMap::new(),
            memory: HashMap::new(),
            naturais: natural_paths,
            expected,
            sources,
            dirs,
            config_files,
            marks: HashMap::new(),
            watched: Vec::new(),
            generation: Arc::new(Geracao::default()),
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
            m.marks.insert(c.clone(), Marca::ler(c));
        }
        m.recompute_watched();
        if m.opcoes.persistir {
            m.to_restore = crate::persistencia::ler(&crate::persistencia::diretorio(
                &m.grafo_pacotes.dir_raiz,
            ));
        }
        #[cfg(feature = "nativos")]
        {
            for g in crate::nativos::todos() {
                m.register_native(g);
            }
        }
        Ok(m)
    }

    pub fn register_native(&mut self, g: Arc<dyn GeradorNativo>) {
        self.natives.push(g);
    }

    pub fn definir_executor_dart(&mut self, e: Box<dyn ExecutorDart>) {
        if let Ok(mut previous) = self.dart.lock() {
            previous.encerrar();
        }
        self.dart = std::sync::Mutex::new(e);
        self.dart_prepared.store(false, Ordering::Release);
    }

    pub fn geracao(&self) -> Arc<Geracao> {
        self.generation.clone()
    }

    /// Natural paths of the expected `.dart` outputs: the first load must
    /// not abort with "could not read" for one of them.
    pub fn saidas_esperadas(&self) -> &HashSet<PathBuf> {
        &self.expected
    }

    pub fn registro(&self, action: usize) -> Option<&Registro> {
        self.records.get(action).and_then(|r| r.as_ref())
    }

    /// Non-Dart inputs the engine watches (the loader already watches the
    /// `.dart` files): the configuration files and what the actions queried
    /// outside memory.
    pub fn observados(&self) -> Vec<PathBuf> {
        self.watched.clone()
    }

    /// Recomputes the watched list (after each update) and records the mark
    /// of whatever just entered it — the state the actions read.
    fn recompute_watched(&mut self) {
        let v = self.compute_watched();
        for p in &v {
            if !self.marks.contains_key(p) {
                self.marks.insert(p.clone(), Marca::ler(p));
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
            .flat_map(|r| r.consultas.iter())
            .chain(self.package_rounds.values().flat_map(|p| p.queries.iter()));
        for (c, _) in queries {
            if let Consulta::Arquivo(p) = c {
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
        p.starts_with(self.grafo_pacotes.dir_raiz.join(".dart_tool"))
    }

    /// Watched paths whose mark changed since the last look (mtime and size).
    pub fn mudancas(&mut self) -> Vec<PathBuf> {
        // One `stat` per watched path, in parallel like `CacheUnidades` (on
        // Windows each `stat` goes through the antivirus).
        let paths = &self.watched;
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(8);
        let now: Vec<Option<Marca>> = if paths.len() < 64 || threads < 2 {
            paths.iter().map(|p| Marca::ler(p)).collect()
        } else {
            let chunk = paths.len().div_ceil(threads);
            std::thread::scope(|s| {
                let hs: Vec<_> = paths
                    .chunks(chunk)
                    .map(|c| s.spawn(move || c.iter().map(|p| Marca::ler(p)).collect::<Vec<_>>()))
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

    pub fn caminho_de_apoio(&self, id: &AssetId, hidden: bool) -> PathBuf {
        if hidden {
            self.grafo_pacotes
                .dir_raiz
                .join(".dart_tool/build/generated")
                .join(id.pacote.as_ref())
                .join(id.caminho.as_ref())
        } else {
            self.naturais.get(id).cloned().unwrap_or_default()
        }
    }

    /// Is the action part of this update? `dart_active`: there is a Dart
    /// executor. A Dart builder may read a `cache` output of an earlier phase
    /// that is not `.dart` (the combining_builder reads the `.g.part` parts by
    /// glob), and the engine does not interrupt an action to compute another
    /// one; so, with the executor, the hidden phases without a native
    /// generator are not lazy — only the optional ones, as in `build_runner`
    /// (`is_optional`).
    fn is_demanded(&self, a: usize, demand: Demanda, dart_active: bool) -> bool {
        if self.records[a].is_some() {
            return true;
        }
        if !self.opcoes.filtros.is_empty() {
            return self.requested.contains(&a);
        }
        if demand == Demanda::Tudo {
            return true;
        }
        let fi = self.grafo.acoes[a].fase;
        let f = &self.fases[fi];
        !f.oculta
            || self.grafo.acoes[a]
                .saidas
                .iter()
                .any(|s| s.caminho.ends_with(".dart"))
            || (dart_active && !f.opcional && self.native_for_phase(fi).is_none())
    }

    /// The outputs that builder `key` (e.g. `ngdart:ngdart`) does not see
    /// when resolving the program: those of every phase from its first one
    /// on, by natural path ([`dartforge_elements::gerado::chave`]). In
    /// `build_runner` an action only reads outputs of phases before its own;
    /// a `build_to: source` output already on disk (the `i18n`'s
    /// `messages.i18n.dart`, a phase after ngdart in limitless_ui) does not
    /// exist for ngdart's resolver, and the type that would come from it is
    /// `dynamic`.
    ///
    /// The program is a single one for all packages; the builder's first
    /// phase is what counts, and an output of a phase between the first
    /// package's and another's is hidden from the second as well.
    pub fn saidas_invisiveis_a(&self, key: &str) -> HashSet<PathBuf> {
        let Some(first) = (0..self.fases.len())
            .find(|&fi| self.plano.aplicacoes[self.fases[fi].aplicacao].chave == key)
        else {
            return HashSet::new();
        };
        self.grafo
            .gerados
            .iter()
            .filter(|(_, g)| g.fase >= first)
            .map(|(id, _)| {
                dartforge_elements::gerado::chave(&natural_path(&self.grafo_pacotes, id))
            })
            .collect()
    }

    /// The `.dart` inputs of the actions of builder `key` (e.g.
    /// `ngdart:ngdart`), by natural path: the files it generates for —
    /// reachable from `main` or not. `build_runner` resolves each one; the
    /// native generators' program must have all of them (a component nobody
    /// imports also gets a `.template.dart`).
    pub fn entradas_de(&self, key: &str) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = self
            .grafo
            .acoes
            .iter()
            .filter(|a| a.viva())
            .filter(|a| self.plano.aplicacoes[self.fases[a.fase].aplicacao].chave == key)
            .filter(|a| a.entrada.caminho.ends_with(".dart"))
            .map(|a| natural_path(&self.grafo_pacotes, &a.entrada))
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
        let f = &self.fases[fi];
        let key = f
            .equivalente
            .unwrap_or(&self.plano.aplicacoes[f.aplicacao].chave);
        self.natives.iter().position(|g| {
            g.chave() == key
                && g.cobre(&f.fabrica)
                && (g.verificado() || self.opcoes.medir_nao_verificados)
                && crate::descritor::imita(key).iter().all(|(p, vs)| {
                    self.grafo_pacotes
                        .lock
                        .get(*p)
                        .is_some_and(|t| t.is_from_pub_dev() && vs.contains(&t.versao.as_str()))
                })
        })
    }

    /// The `pubspec.lock` versions of the packages the native generator `key`
    /// imitates (the ones that pick its mode; see [`crate::descritor::imita`]).
    fn imitated_versions(&self, key: &str) -> BTreeMap<String, String> {
        crate::descritor::imita(key)
            .iter()
            .filter_map(|(p, _)| {
                let t = self.grafo_pacotes.lock.get(*p)?;
                Some(((*p).to_owned(), t.versao.clone()))
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
        let listed = self.grafo.listados.iter().any(|p| {
            self.grafo_pacotes.no(p).is_some_and(|n| {
                m.starts_with(&n.raiz) && !m.starts_with(n.raiz.join(".dart_tool"))
            })
        });
        listed && m.is_file() && !self.naturais.values().any(|n| n == m)
    }

    /// Redoes the source listing and the expected outputs with the same
    /// plan; the records of the actions that still exist (same phase and
    /// same input) are kept.
    fn rebuild_graph(&mut self, changed: &mut HashSet<PathBuf>) -> Result<(), String> {
        let graph = Grafo::montar(&self.grafo_pacotes, &self.configs, &self.fases, &self.alvos)?;
        let mut old: HashMap<(usize, AssetId), Registro> = HashMap::new();
        for (i, r) in std::mem::take(&mut self.records).into_iter().enumerate() {
            if let Some(r) = r {
                let a = &self.grafo.acoes[i];
                old.insert((a.fase, a.entrada.clone()), r);
            }
        }
        let Indices {
            per_phase,
            natural_paths,
            expected,
            sources,
            dirs,
        } = build_indices(&self.grafo_pacotes, &graph, self.fases.len());
        self.records = graph
            .acoes
            .iter()
            .map(|a| {
                if a.viva() {
                    old.remove(&(a.fase, a.entrada.clone()))
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
            .acoes
            .iter()
            .zip(&self.records)
            .filter(|(a, _)| a.pos)
            .filter_map(|(_, r)| r.as_ref())
            .flat_map(|r| {
                r.saidas
                    .iter()
                    .map(|(s, _)| natural_path(&self.grafo_pacotes, s))
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
            self.marks.entry(d.clone()).or_insert_with(|| Marca::ler(d));
        }
        self.grafo = graph;
        self.per_phase = per_phase;
        self.naturais = natural_paths;
        self.expected = expected;
        self.sources = sources;
        self.dirs = dirs;
        self.request_by_filters();
        Ok(())
    }

    /// Recomputes whatever is needed and publishes the generation.
    pub fn atualizar(
        &mut self,
        ctx: &Contexto<'_>,
        changed: &[PathBuf],
        demand: Demanda,
    ) -> Result<Atualizacao, String> {
        let mut at = self.update_pass(ctx, changed, demand)?;
        // `--build-filter`: a step that ran read the output of an action that
        // was not requested — the official tool builds it on the spot
        // (`_isReadableNode` → `_runLazyPhaseForInput`). Here it joins the
        // requested ones and the update repeats: the new output wakes up
        // whoever read it.
        while !self.opcoes.filtros.is_empty() {
            let new = self.requested_by_reads();
            if new.is_empty() {
                break;
            }
            self.requested.extend(new);
            let more = self.update_pass(ctx, &[], demand)?;
            let mut altered: BTreeSet<PathBuf> = at.alterados.into_iter().collect();
            altered.extend(more.alterados);
            at.alterados = altered.into_iter().collect();
            at.rel.add(more.rel);
            at.rel.saidas_alteradas = at.alterados.len();
            at.avisos.extend(more.avisos);
            at.geracao = more.geracao;
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
        if self.opcoes.filtros.is_empty() {
            return;
        }
        let mut stack = Vec::new();
        for (a, action) in self.grafo.acoes.iter().enumerate() {
            if !action.viva() {
                continue;
            }
            let phase = &self.fases[action.fase];
            let requested = action.pos
                || !phase.opcional
                    && action.saidas.iter().any(|s| {
                        self.opcoes.filtros.iter().any(|f| f.casa(s)) && self.visible_in_build(s)
                    });
            if requested {
                stack.push(a);
            }
        }
        while let Some(a) = stack.pop() {
            if !self.requested.insert(a) {
                continue;
            }
            if let Some(g) = self.grafo.gerados.get(&self.grafo.acoes[a].entrada) {
                stack.push(g.acao);
            }
        }
    }

    /// `isVisibleInBuild`: everything at the root; outside it, the public
    /// assets (`lib/**`, `bin/**`, … and `additional_public_assets`).
    fn visible_in_build(&self, id: &AssetId) -> bool {
        let Some(&p) = self.grafo_pacotes.por_nome.get(id.pacote.as_ref()) else {
            return false;
        };
        if p == self.grafo_pacotes.raiz {
            return true;
        }
        crate::grafo::VISIVEIS_FORA_DA_RAIZ
            .iter()
            .map(|s| s.to_string())
            .chain(
                self.configs.por_pacote[p]
                    .publicos_adicionais
                    .iter()
                    .cloned(),
            )
            .any(|g| crate::glob::Glob::novo(&g).is_ok_and(|g| g.casa(&id.caminho)))
    }

    /// The not yet requested actions whose output a step that ran queried:
    /// by path (`canRead`, a read) or as a glob candidate (`findAssets`),
    /// from a phase before the reader's.
    fn requested_by_reads(&self) -> Vec<usize> {
        let by_natural: HashMap<&Path, &AssetId> = self
            .naturais
            .iter()
            .map(|(id, n)| (n.as_path(), id))
            .collect();
        let mut new = BTreeSet::new();
        for (a, r) in self.records.iter().enumerate() {
            let Some(r) = r else { continue };
            let phase = self.grafo.acoes[a].fase;
            let mut request = |id: &AssetId| {
                if let Some(g) = self.grafo.gerados.get(id)
                    && g.fase < phase
                    && self.records[g.acao].is_none()
                    && !self.requested.contains(&g.acao)
                {
                    new.insert(g.acao);
                }
            };
            for (c, _) in &r.consultas {
                match c {
                    Consulta::GlobAtivos {
                        dir, candidatos, ..
                    } => {
                        for (rel, generated) in candidatos {
                            if *generated
                                && let Some(id) = by_natural.get(chave(&dir.join(rel)).as_path())
                            {
                                request(id);
                            }
                        }
                    }
                    _ => {
                        if let Some(id) = c.caminho().and_then(|p| by_natural.get(p)) {
                            request(id);
                        }
                    }
                }
            }
        }
        new.into_iter().collect()
    }

    /// One engine pass over the phases (see [`Motor::atualizar`]).
    fn update_pass(
        &mut self,
        ctx: &Contexto<'_>,
        changed: &[PathBuf],
        demand: Demanda,
    ) -> Result<Atualizacao, String> {
        let t0 = Instant::now();
        // The engine's paths come from `package_config.json` (canonical); an
        // event may arrive under another name for the same file (Windows 8.3
        // short name, link): the canonical form is added as well.
        let mut changed: HashSet<PathBuf> = {
            let mut m = HashSet::with_capacity(changed.len() * 2);
            for p in changed {
                m.insert(chave(p));
                if let Some(c) = canonical(p) {
                    m.insert(c);
                }
            }
            m
        };
        if self.config_changed(&changed) {
            let old_memory = std::mem::take(&mut self.memory);
            let options = self.opcoes.clone();
            let natives = std::mem::take(&mut self.natives);
            let dart = std::mem::replace(
                &mut self.dart,
                std::sync::Mutex::new(Box::new(Indisponivel::default())),
            );
            // The `package_config.json` reread from disk (a `path`
            // dependency may have moved or changed version): the stored one
            // is from the start of the session. If it is unreadable now, the
            // update fails, as in a new process.
            let file = self
                .grafo_pacotes
                .dir_raiz
                .join(".dart_tool")
                .join("package_config.json");
            let cfg = PackageConfig::load(&file)?;
            *self = Motor::novo(&self.raiz.clone(), &cfg, options)?;
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
            d.nova_rodada();
        }
        let mut rel = RelMotor::default();
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
            .map(|d| match d.disponibilidade() {
                Disponibilidade::Disponivel => None,
                Disponibilidade::Indisponivel(m) => Some(m),
            })
            .unwrap_or_else(|_| Some("Dart executor poisoned".into()));
        if self.to_restore.is_some() {
            self.apply_restoration(dart_reason.as_deref());
        }

        for fi in 0..self.fases.len() {
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
            if let Some(g) = native.filter(|&g| self.natives[g].por_pacote()) {
                let package: Arc<str> = self.grafo_pacotes.nos[self.fases[fi].pacote]
                    .nome
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
                        rel.reevaluated_queries += r.consultas.len();
                        let (dirty, off_disk) = self.check_restored(a, ctx.banco);
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
                            .consultas
                            .iter()
                            .enumerate()
                            .filter(|(_, (c, _))| {
                                affected(c, &changed, dart_changed, &structural)
                                    && !self.is_invisible(a, c)
                            })
                            .any(|(i, (c, d))| {
                                rel.reevaluated_queries += 1;
                                if let Consulta::GlobAtivos { dir, padrao, .. } = c {
                                    // The graph may have gained (or lost) a
                                    // candidate: the list is redone on the new
                                    // graph and only a different answer
                                    // dirties the action.
                                    if structural.iter().any(|m| m.starts_with(dir)) {
                                        let Ok(g) = crate::glob::Glob::novo(padrao) else {
                                            return true;
                                        };
                                        let candidates = glob_candidates(&self.grafo, a, &g)
                                            .into_iter()
                                            .map(|id| {
                                                (
                                                    id.caminho.to_string(),
                                                    self.grafo.gerados.contains_key(&id),
                                                )
                                            })
                                            .collect();
                                        let redone_query = Consulta::GlobAtivos {
                                            dir: dir.clone(),
                                            padrao: padrao.clone(),
                                            candidatos: candidates,
                                        };
                                        let differs =
                                            query_digest(&redone_query, ctx.banco, &memory) != *d;
                                        redone.push((i, redone_query));
                                        return differs;
                                    }
                                }
                                query_digest(c, ctx.banco, &memory) != *d
                            });
                        if !dirty && !redone.is_empty() {
                            // The answer is the same, but the candidates are
                            // those of the new graph: a future event on one of
                            // them has to wake the action up.
                            if let Some(r) = self.records[a].as_mut() {
                                for (i, c) in redone {
                                    r.consultas[i].0 = c;
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
            let mut preset: HashMap<usize, Registro> = HashMap::new();
            let mut from_trigger: HashMap<usize, Queries> = HashMap::new();
            if !self.fases[fi].pos {
                for &a in &to_run {
                    if let Some(r) = self.unwritten_input(a) {
                        preset.insert(a, r);
                    } else if let Some((fired, queries)) = self.trigger_decision(a) {
                        if fired {
                            from_trigger.insert(a, queries);
                        } else {
                            preset
                                .insert(a, self.without_outputs(a, queries, Origem::NotTriggered));
                        }
                    }
                }
            }
            let run: Vec<usize> = to_run
                .iter()
                .copied()
                .filter(|a| !preset.contains_key(a))
                .collect();
            let ran: Vec<Registro> = match native {
                _ if run.is_empty() => Vec::new(),
                Some(g) if self.natives[g].por_pacote() => run
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
                None if dart_reason.is_none() && self.fases[fi].pos => {
                    self.post_process_serial(&run)
                }
                None if dart_reason.is_none() => {
                    run.iter().map(|&a| self.dart_or_fallback(a)).collect()
                }
                None => self.fallback_in_parallel(&run, dart_reason.as_deref()),
            };
            let mut ran: HashMap<usize, Registro> = run.into_iter().zip(ran).collect();
            let new_records: Vec<Registro> = to_run
                .iter()
                .map(|a| {
                    preset.remove(a).unwrap_or_else(|| {
                        let mut r = ran.remove(a).expect("record of an action that ran");
                        if let Some(c) = from_trigger.remove(a) {
                            r.consultas.extend(c);
                        }
                        r
                    })
                })
                .collect();
            for (a, mut r) in to_run.into_iter().zip(new_records) {
                if !matches!(r.origem, Origem::Omitida | Origem::NotTriggered) {
                    rel.acoes_executadas += 1;
                }
                match r.origem {
                    Origem::Nativo(_) => rel.nativas += 1,
                    Origem::Dart => rel.dart += 1,
                    Origem::Apoio => {
                        rel.apoio += 1;
                        if r.saidas.iter().any(|(_, c)| c.is_some()) {
                            rel.apoio_com_saida += 1;
                        }
                    }
                    Origem::Falha => rel.falhas.push(r.motivo.clone().unwrap_or_default()),
                    Origem::NotTriggered => rel.not_triggered += 1,
                    Origem::Pendente | Origem::Omitida => {}
                }
                if let Some(m) = &r.motivo {
                    if r.origem == Origem::Pendente {
                        *rel.pendentes_por_motivo.entry(m.clone()).or_default() += 1;
                    }
                }
                // Stale fallback (D-B5).
                if r.origem == Origem::Apoio {
                    let input = self.input_natural_path(a);
                    for (s, _) in &r.saidas {
                        let loc = self.caminho_de_apoio(s, self.fases[fi].oculta);
                        if is_newer(&input, &loc) && self.warned.insert(loc.clone()) {
                            let msg = format!(
                                "{}: {} may be out of date — {}; run 'dart run build_runner build'",
                                self.plano.aplicacoes[self.fases[fi].aplicacao].chave,
                                s.texto(),
                                r.motivo
                                    .as_deref()
                                    .unwrap_or("the Dart executor produced no current output")
                            );
                            if self.opcoes.estrito {
                                return Err(msg);
                            }
                            warnings.push(msg);
                        }
                    }
                }
                // A post-processor's outputs that the new run did not write
                // disappear (the anchor's `_cleanUpStaleOutputs`).
                if let Some(old) = &self.records[a]
                    && self.grafo.acoes[a].pos
                {
                    for (s, _) in &old.saidas {
                        if !r.saidas.iter().any(|(x, _)| x == s) {
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
                    .map(|x| x.apagados.clone())
                    .unwrap_or_default();
                if before != r.apagados {
                    for id in before.iter().chain(&r.apagados) {
                        if self.grafo.gerados.contains_key(id) {
                            let n = self.natural_path_of(id);
                            altered.insert(n);
                        }
                    }
                }
                for deleted in r
                    .apagados
                    .iter()
                    .filter(|x| !self.grafo.gerados.contains_key(*x))
                {
                    let msg = format!(
                        "{}: the source {} was marked by deletePrimaryInput; DartForge has no merged output directory (`build -o`) and does not hide sources in `serve`, so it stays visible",
                        self.plano.aplicacoes[self.fases[fi].aplicacao].chave,
                        deleted.texto()
                    );
                    if self.opcoes.estrito {
                        return Err(msg);
                    }
                    warnings.push(msg);
                }
                // Output cutoff: only what changed content moves on.
                for (s, c) in &r.saidas {
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
                let fingerprint = self.fingerprint(a, &r.consultas);
                r.fingerprint = fingerprint;
                self.records[a] = Some(r);
                self.state_dirty = true;
            }
        }
        rel.saidas_alteradas = altered.len();
        if !altered.is_empty() || self.generation.vazia() {
            self.publish();
        }
        if self.opcoes.persistir && self.state_dirty {
            match self.save() {
                Ok(()) => self.state_dirty = false,
                // Kept in Portuguese: `crates/build/tests/corpus.rs` matches
                // "não gravado".
                Err(e) => warnings.push(format!("estado do motor não gravado: {e}")),
            }
        }
        rel.tempo = t0.elapsed();
        self.recompute_watched();
        Ok(Atualizacao {
            geracao: self.generation.clone(),
            alterados: altered.into_iter().collect(),
            rel,
            avisos: warnings,
        })
    }

    /// The run-time extensions come from the `Builder` object, not from
    /// `build.yaml` (`expected_outputs.dart`). With the Dart executor
    /// available, the engine asks, for each phase that no verified native
    /// generator covers, what they are (`build.extensoes`) and, if any of
    /// them differs from the descriptor or from `build.yaml` — one factory of
    /// a builder with several, extensions that depend on the options —,
    /// rebuilds the graph with the `Builder`'s. Once per plan; without an
    /// executor, the predicted ones stay.
    fn check_extensions(&mut self, changed: &mut HashSet<PathBuf>) -> Result<(), String> {
        self.extensions_checked = true;
        let candidates: Vec<usize> = (0..self.fases.len())
            .filter(|&fi| self.fases[fi].extensoes.is_some() && self.native_for_phase(fi).is_none())
            .collect();
        let post: Vec<usize> = (0..self.fases.len())
            .filter(|&fi| self.fases[fi].pos && self.fases[fi].substituido.is_none())
            .collect();
        let available = self
            .dart
            .lock()
            .is_ok_and(|d| d.disponibilidade() == Disponibilidade::Disponivel);
        if (candidates.is_empty() && post.is_empty()) || !available || self.prepare_dart().is_some()
        {
            return Ok(());
        }
        let mut differs = false;
        // Post-processors: the object's `inputExtensions` decide the anchors
        // (`_actionMatches` of a `PostBuildAction`).
        for fi in post {
            let f = &self.fases[fi];
            let key = self.plano.aplicacoes[f.aplicacao].chave.clone();
            let request = PedidoExtensoes {
                chave: key,
                fabrica: f.fabrica.clone(),
                opcoes: f.opcoes.clone(),
                raiz: f.raiz,
            };
            let answer = self
                .dart
                .lock()
                .map_err(|_| "Dart executor poisoned")?
                .entradas_pos(&request);
            let Ok(Some(v)) = answer else { continue };
            if self.fases[fi].entradas_pos.as_ref() != Some(&v) {
                self.fases[fi].entradas_pos = Some(v);
                differs = true;
            }
        }
        for fi in candidates {
            let f = &self.fases[fi];
            let key = self.plano.aplicacoes[f.aplicacao].chave.clone();
            let request = PedidoExtensoes {
                chave: key.clone(),
                fabrica: f.fabrica.clone(),
                opcoes: f.opcoes.clone(),
                raiz: f.raiz,
            };
            let answer = self
                .dart
                .lock()
                .map_err(|_| "Dart executor poisoned")?
                .extensoes(&request);
            // No answer (a factory that fails to instantiate, an executor
            // that does not know): the predicted ones stay, and the action
            // will report the error when it runs.
            let Ok(Some(v)) = answer else { continue };
            if self.fases[fi]
                .extensoes
                .as_ref()
                .is_some_and(|e| e.declaradas != v)
            {
                self.fases[fi].extensoes = Some(crate::extensoes::Extensoes::novas(&v, &key)?);
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
    fn is_invisible(&self, a: usize, c: &Consulta) -> bool {
        let (Consulta::Arquivo(p) | Consulta::Existe(p)) = c else {
            return false;
        };
        let phase = self.grafo.acoes[a].fase;
        let Some(node) = self
            .grafo_pacotes
            .nos
            .iter()
            .filter(|n| !n.raiz.as_os_str().is_empty() && p.starts_with(&n.raiz))
            .max_by_key(|n| n.raiz.as_os_str().len())
        else {
            return false;
        };
        let Ok(rel) = p.strip_prefix(&node.raiz) else {
            return false;
        };
        let path = rel.to_string_lossy().replace('\\', "/");
        self.grafo
            .gerados
            .get(&AssetId::novo(&node.nome, &path))
            .is_some_and(|g| g.fase >= phase)
    }

    /// Global key of the saved state: what no query sees — engine version,
    /// the executable (the native generators' code), the plan, the lock and
    /// the mode.
    fn state_key(&self) -> String {
        let mut h = blake3::Hasher::new();
        h.update(crate::VERSAO.as_bytes());
        h.update(crate::persistencia::identidade_do_executavel().as_bytes());
        h.update(self.plano.texto_canonico().as_bytes());
        let mut lock: Vec<_> = self.grafo_pacotes.lock.iter().collect();
        lock.sort_by_key(|(name, _)| *name);
        for (name, t) in lock {
            h.update(format!("{name} {} {:?}\n", t.versao, t.tipo).as_bytes());
        }
        h.update(&[u8::from(self.opcoes.release)]);
        // `--config` and the accumulated `triggers` decide what runs without
        // showing up in any query (the official `buildTriggersDigest`).
        h.update(format!("{:?}", self.opcoes.config).as_bytes());
        h.update(format!("{:?}", self.configs.gatilhos.por_builder).as_bytes());
        h.finalize().to_hex().to_string()
    }

    /// Key of an action in the saved state: the phase (with the extensions
    /// and the post-processor inputs already checked), the options, the
    /// input and the predicted outputs.
    fn action_key(&self, a: usize) -> String {
        let action = &self.grafo.acoes[a];
        let f = &self.fases[action.fase];
        let mut h = blake3::Hasher::new();
        h.update(f.identity(&self.plano, &self.grafo_pacotes).as_bytes());
        h.update(f.opcoes.texto_canonico().as_bytes());
        h.update(&[u8::from(f.raiz), u8::from(action.pos)]);
        h.update(action.entrada.texto().as_bytes());
        for s in &action.saidas {
            h.update(b"\n");
            h.update(s.texto().as_bytes());
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
            .and_then(|d| d.codigo())
            .or_else(|| self.restored_code.clone())?;
        let hosted: Vec<&Path> = self
            .grafo_pacotes
            .nos
            .iter()
            .filter(|n| {
                n.tipo == crate::config::TipoDependencia::Hosted && !n.raiz.as_os_str().is_empty()
            })
            .map(|n| n.raiz.as_path())
            .collect();
        let mut v = Vec::new();
        for p in list {
            if hosted.iter().any(|h| p.starts_with(h)) {
                continue;
            }
            v.push((p.clone(), crate::consulta::digest_arquivo(&p)?));
        }
        v.sort();
        Some(v)
    }

    /// Can the record go into the saved state (§4.1)?
    fn is_persistable(&self, a: usize, r: &Registro) -> bool {
        let fi = self.grafo.acoes[a].fase;
        !matches!(r.origem, Origem::Pendente)
            && r.measured.is_empty()
            && self
                .native_for_phase(fi)
                .is_none_or(|g| !self.natives[g].por_pacote())
            && r.consultas
                .iter()
                .all(|(c, _)| crate::persistencia::persistivel(c))
    }

    /// Writes the state: the persistable actions and the outputs' content.
    fn save(&mut self) -> Result<(), String> {
        let has_dart = self
            .records
            .iter()
            .flatten()
            .any(|r| r.origem == Origem::Dart);
        let dart_code = if has_dart { self.dart_code() } else { None };
        let mut actions = Vec::new();
        let mut contents = Vec::new();
        for (a, r) in self.records.iter().enumerate() {
            let Some(r) = r else { continue };
            if !self.is_persistable(a, r) {
                continue;
            }
            let origin = match r.origem {
                Origem::Nativo(n) => crate::persistencia::OrigemSalva::Nativo(n.to_string()),
                Origem::Dart if dart_code.is_some() => crate::persistencia::OrigemSalva::Dart,
                Origem::Apoio => crate::persistencia::OrigemSalva::Apoio,
                Origem::Omitida if self.grafo.acoes[a].pos => {
                    crate::persistencia::OrigemSalva::Omitida
                }
                Origem::Dart
                | Origem::Pendente
                | Origem::Falha
                | Origem::Omitida
                | Origem::NotTriggered => continue,
            };
            let outputs = r
                .saidas
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
            actions.push(crate::persistencia::AcaoSalva {
                chave: self.action_key(a),
                origem: origin,
                motivo: r.motivo.clone(),
                consultas: r.consultas.clone(),
                saidas: outputs,
                do_apoio: r.from_fallback.clone(),
                apagados: r.apagados.clone(),
            });
        }
        let state = crate::persistencia::Estado {
            chave: self.state_key(),
            codigo_dart: dart_code,
            acoes: actions,
        };
        crate::persistencia::gravar(
            &crate::persistencia::diretorio(&self.grafo_pacotes.dir_raiz),
            &state,
            &contents,
        )
    }

    /// Applies the state read at startup: each saved action whose key still
    /// exists and whose origin is still the one this engine would choose
    /// comes back as a `restored` record, with its outputs in memory. Its
    /// validity is checked on the first verification of each one
    /// ([`Motor::check_restored`]).
    fn apply_restoration(&mut self, dart_reason: Option<&str>) {
        let Some(state) = self.to_restore.take() else {
            return;
        };
        if state.chave != self.state_key() {
            return;
        }
        let dir = crate::persistencia::diretorio(&self.grafo_pacotes.dir_raiz);
        let dart_valid = state.codigo_dart.as_ref().is_some_and(|l| {
            l.iter()
                .all(|(p, d)| crate::consulta::digest_arquivo(p).as_ref() == Some(d))
        });
        if dart_valid {
            self.restored_code = state
                .codigo_dart
                .as_ref()
                .map(|l| l.iter().map(|(p, _)| p.clone()).collect());
        }
        let by_key: HashMap<String, usize> = (0..self.grafo.acoes.len())
            .filter(|&a| self.grafo.acoes[a].viva() && self.records[a].is_none())
            .map(|a| (self.action_key(a), a))
            .collect();
        for s in state.acoes {
            let Some(&a) = by_key.get(&s.chave) else {
                continue;
            };
            let fi = self.grafo.acoes[a].fase;
            let native = self.native_for_phase(fi);
            let origin = match &s.origem {
                crate::persistencia::OrigemSalva::Nativo(n) => match native {
                    Some(g) if self.natives[g].chave() == n && !self.natives[g].por_pacote() => {
                        Origem::Nativo(self.natives[g].chave())
                    }
                    _ => continue,
                },
                crate::persistencia::OrigemSalva::Dart if dart_valid && native.is_none() => {
                    Origem::Dart
                }
                crate::persistencia::OrigemSalva::Apoio
                    if native.is_none() && dart_reason.is_some() =>
                {
                    Origem::Apoio
                }
                crate::persistencia::OrigemSalva::Omitida if self.grafo.acoes[a].pos => {
                    Origem::Omitida
                }
                _ => continue,
            };
            let mut outputs = Vec::with_capacity(s.saidas.len());
            for (id, d) in &s.saidas {
                match d {
                    None => outputs.push((id.clone(), None)),
                    Some(d) => match crate::persistencia::ler_blob(&dir, d) {
                        Some(b) => outputs.push((id.clone(), Some(b))),
                        None => break,
                    },
                }
            }
            if outputs.len() != s.saidas.len() {
                continue;
            }
            for (id, c) in &outputs {
                if let Some(c) = c {
                    let n = self.natural_path_of(id);
                    self.memory.insert(n, c.clone());
                }
            }
            let fingerprint = self.fingerprint(a, &s.consultas);
            self.records[a] = Some(Registro {
                fingerprint,
                consultas: s.consultas,
                saidas: outputs,
                origem: origin,
                motivo: s.motivo,
                measured: Vec::new(),
                from_fallback: s.do_apoio,
                apagados: s.apagados,
                restored: true,
            });
        }
    }

    /// Checks a restored record: all queries (a `GlobAtivos` with the
    /// candidates of the current graph) and, for a `source` output of a
    /// native generator or of the Dart executor, the file on disk, which the
    /// CLI only rewrites when the content changes. Returns whether it is
    /// dirty and the output paths that differ from disk (memory forgets them,
    /// so that the rerun counts them as changed and rewrites them).
    fn check_restored(&self, a: usize, database: &dyn BancoSemantico) -> (bool, Vec<PathBuf>) {
        let Some(r) = self.records[a].as_ref() else {
            return (true, Vec::new());
        };
        let memory = |p: &Path| self.memory.get(p).cloned();
        let dirty = r
            .consultas
            .iter()
            .filter(|(c, _)| !self.is_invisible(a, c))
            .any(|(c, d)| {
                let current = match c {
                    Consulta::GlobAtivos { dir, padrao, .. } => {
                        let Ok(g) = crate::glob::Glob::novo(padrao) else {
                            return true;
                        };
                        let candidates = glob_candidates(&self.grafo, a, &g)
                            .into_iter()
                            .map(|id| {
                                (id.caminho.to_string(), self.grafo.gerados.contains_key(&id))
                            })
                            .collect();
                        query_digest(
                            &Consulta::GlobAtivos {
                                dir: dir.clone(),
                                padrao: padrao.clone(),
                                candidatos: candidates,
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
        let f = &self.fases[self.grafo.acoes[a].fase];
        if !f.oculta && matches!(r.origem, Origem::Nativo(_) | Origem::Dart) {
            for (s, c) in &r.saidas {
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
        self.naturais
            .get(s)
            .cloned()
            .unwrap_or_else(|| natural_path(&self.grafo_pacotes, s))
    }

    /// The post-processing anchors of a phase, in series and in graph order:
    /// each one sees the others' outputs as "already existing" (the official
    /// `addAsset` rejects an asset the graph already has).
    fn post_process_serial(&self, to_run: &[usize]) -> Vec<Registro> {
        let mut owners: BTreeMap<AssetId, usize> = BTreeMap::new();
        for (a, (action, r)) in self.grafo.acoes.iter().zip(&self.records).enumerate() {
            if let (true, Some(r)) = (action.pos, r) {
                for (s, _) in &r.saidas {
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
                .unwrap_or_else(|e| Registro {
                    fingerprint: [0; 32],
                    consultas: Vec::new(),
                    saidas: Vec::new(),
                    origem: Origem::Pendente,
                    motivo: Some(e),
                    measured: Vec::new(),
                    from_fallback: Vec::new(),
                    apagados: Vec::new(),
                    restored: false,
                });
            owners.retain(|_, d| *d != a);
            for (s, _) in &r.saidas {
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
    /// The error messages that start with "pós-processador" stay in
    /// Portuguese: `crates/build/tests/corpus.rs` matches that prefix on the
    /// pending reasons.
    fn post_process_action(
        &self,
        a: usize,
        occupied: BTreeSet<AssetId>,
    ) -> Result<Registro, String> {
        let action = self.grafo.acoes.get(a).ok_or("nonexistent anchor")?;
        if self.grafo.gerados.contains_key(&action.entrada) {
            let n = self.natural_path_of(&action.entrada);
            if !self.memory.contains_key(&n) {
                // The negative query wakes the anchor up when the input is
                // written.
                return Ok(Registro {
                    fingerprint: [0; 32],
                    consultas: vec![(Consulta::Arquivo(n), None)],
                    saidas: Vec::new(),
                    origem: Origem::Omitida,
                    motivo: None,
                    measured: Vec::new(),
                    from_fallback: Vec::new(),
                    apagados: Vec::new(),
                    restored: false,
                });
            }
        }
        if let Some(e) = self.prepare_dart() {
            return Err(e);
        }
        let phase = &self.fases[action.fase];
        let memory = |id: &AssetId| {
            self.naturais
                .get(id)
                .and_then(|p| self.memory.get(p))
                .cloned()
        };
        let mut service = ActionService::new(&self.grafo, &self.grafo_pacotes, a, &memory);
        service.occupied = occupied;
        let request = PedidoAcao {
            fase: action.fase,
            chave: self.plano.aplicacoes[phase.aplicacao].chave.clone(),
            fabrica: phase.fabrica.clone(),
            opcoes: phase.opcoes.clone(),
            raiz: phase.raiz,
            entrada: action.entrada.clone(),
            saidas_permitidas: Vec::new(),
        };
        let mut dart = self.dart.lock().map_err(|_| "Dart executor poisoned")?;
        let result = dart
            .pos_processar(&request, &mut service)
            .map_err(|e| e.0)?;
        if result.falhou {
            // "severo" and "erro" are log levels of the dfexec/1 protocol.
            let details = result
                .logs
                .iter()
                .filter(|(level, _)| level == "severo" || level == "erro")
                .map(|(_, message)| message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(if details.is_empty() {
                format!("{}: Dart post-processor failed", request.chave)
            } else {
                format!("{}: {details}", request.chave)
            });
        }
        for (id, bytes) in result.saidas {
            if service
                .writes
                .get(&id)
                .is_some_and(|b| b.as_ref() != bytes.as_ref())
            {
                return Err(format!(
                    "pós-processador Dart retornou bytes diferentes para {}",
                    id.texto()
                ));
            }
            if !service.writes.contains_key(&id) {
                service.escrever(&id, bytes).map_err(|e| {
                    format!(
                        "pós-processador escreveu asset já existente: {}",
                        e.0.texto()
                    )
                })?;
            }
        }
        if let Some(x) = result.apagados.iter().find(|x| **x != action.entrada) {
            return Err(format!(
                "pós-processador apagou {}, que não é a entrada primária",
                x.texto()
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
        let n = self.natural_path_of(&action.entrada);
        let d = self
            .memory
            .get(&n)
            .map(|b| digest_bytes(b))
            .or_else(|| crate::consulta::digest_arquivo(&n));
        let mut queries = vec![(Consulta::Arquivo(n), d)];
        queries.extend(service.queries);
        Ok(Registro {
            fingerprint: [0; 32],
            consultas: queries,
            saidas: outputs,
            origem: Origem::Dart,
            motivo: None,
            measured: Vec::new(),
            from_fallback: Vec::new(),
            apagados: result.apagados,
            restored: false,
        })
    }

    /// Record of a step that does not run: no output written, only the
    /// queries that decided so.
    fn without_outputs(
        &self,
        a: usize,
        queries: Vec<(Consulta, Option<Digest>)>,
        origin: Origem,
    ) -> Registro {
        Registro {
            fingerprint: [0; 32],
            consultas: queries,
            saidas: self.grafo.acoes[a]
                .saidas
                .iter()
                .map(|s| (s.clone(), None))
                .collect(),
            origem: origin,
            motivo: None,
            measured: Vec::new(),
            from_fallback: Vec::new(),
            apagados: Vec::new(),
            restored: false,
        }
    }

    /// The primary input is an output of an earlier phase that was not
    /// written (or failed): the step does not run (`_matchingPrimaryInputs`:
    /// `if (!input.wasOutput) return`; in 2.16.1, `skipMissingPrimaryInput`).
    /// The negative query wakes it up when the input is written.
    fn unwritten_input(&self, a: usize) -> Option<Registro> {
        let input = &self.grafo.acoes[a].entrada;
        let g = self.grafo.gerados.get(input)?;
        // Only when whoever produces the input actually ran: without an
        // executor (the action pending, or read from the fallback) it is
        // unknown what the builder would write, and the step follows the
        // fallback policy.
        let known = self.records[g.acao].as_ref().is_some_and(|r| {
            matches!(
                r.origem,
                Origem::Dart
                    | Origem::Nativo(_)
                    | Origem::Falha
                    | Origem::NotTriggered
                    | Origem::Omitida
            )
        });
        if !known {
            return None;
        }
        let n = self.natural_path_of(input);
        if self.memory.contains_key(&n) {
            return None;
        }
        Some(self.without_outputs(a, vec![(Consulta::Arquivo(n), None)], Origem::Omitida))
    }

    /// `_allowedByTriggers`: `None` if the phase does not have
    /// `run_only_if_triggered: true` (or the profile has no triggers);
    /// otherwise, whether some trigger of the builder fires, with the reads
    /// the decision made (the input and, for an annotation trigger, the parts
    /// readable by the phase — including those generated by earlier phases).
    fn trigger_decision(&self, a: usize) -> Option<(bool, Queries)> {
        let action = &self.grafo.acoes[a];
        let phase = &self.fases[action.fase];
        if !phase.only_if_triggered {
            return None;
        }
        let key = &self.plano.aplicacoes[phase.aplicacao].chave;
        let short_name = self.grafo_pacotes.perfil().trigger_pelo_nome_abreviado();
        let Some(triggers) = self.configs.gatilhos.do_builder(key, short_name) else {
            return Some((false, Vec::new()));
        };
        let memory = |id: &AssetId| {
            self.naturais
                .get(id)
                .and_then(|p| self.memory.get(p))
                .cloned()
        };
        let mut service = ActionService::new(&self.grafo, &self.grafo_pacotes, a, &memory);
        let primary = service
            .ler(&action.entrada)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default();
        let fired = crate::gatilhos::disparado(triggers, &primary, |uri| {
            let id = resolve_uri(uri, &action.entrada)?;
            if !service.can_read(&id) {
                return None;
            }
            service
                .ler(&id)
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        });
        Some((fired, service.queries))
    }

    fn input_natural_path(&self, a: usize) -> PathBuf {
        natural_path(&self.grafo_pacotes, &self.grafo.acoes[a].entrada)
    }

    fn prepare_dart(&self) -> Option<String> {
        let Ok(mut dart) = self.dart.lock() else {
            return Some("Dart executor poisoned".into());
        };
        match dart.disponibilidade() {
            Disponibilidade::Indisponivel(m) => return Some(m),
            Disponibilidade::Disponivel if self.dart_prepared.load(Ordering::Acquire) => {
                return None;
            }
            Disponibilidade::Disponivel => {}
        }
        // Only what can become a Dart action (builders and post-processors):
        // the replaced ones never run (§6 of BUILD-MOTOR.md); importing them
        // would only make the script's compilation bigger.
        let applications = self
            .plano
            .aplicacoes
            .iter()
            .filter(|a| crate::descritor::substituido(&a.chave).is_none())
            .map(|a| (a.chave.clone(), a.import.clone(), a.fabricas.clone()))
            .collect();
        let mut h = blake3::Hasher::new();
        h.update(crate::VERSAO.as_bytes());
        h.update(self.plano.texto_canonico().as_bytes());
        let mut lock: Vec<_> = self.grafo_pacotes.lock.iter().collect();
        lock.sort_by_key(|(name, _)| *name);
        for (name, package) in lock {
            h.update(name.as_bytes());
            h.update(package.versao.as_bytes());
            h.update(format!("{:?}", package.tipo).as_bytes());
        }
        let script = ScriptDeBuilders {
            aplicacoes: applications,
            chave_de_cache: h.finalize().to_hex().to_string(),
        };
        match dart.preparar(&script) {
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
    fn dart_or_fallback(&self, a: usize) -> Registro {
        match self.dart_action(a) {
            Ok(r) => r,
            Err(DartFailure::Builder(e)) => self.failure(a, e),
            Err(DartFailure::Unavailable(e)) => self.fallback(a, Some(&e)),
        }
    }

    /// The record of an action that failed: no output (the old one, from
    /// disk or memory, does not count), the failure as the reason.
    fn failure(&self, a: usize, e: String) -> Registro {
        let action = &self.grafo.acoes[a];
        let input = natural_path(&self.grafo_pacotes, &action.entrada);
        let d = self
            .memory
            .get(&input)
            .map(|b| digest_bytes(b))
            .or_else(|| crate::consulta::digest_arquivo(&input));
        Registro {
            fingerprint: [0; 32],
            consultas: vec![(Consulta::Arquivo(input), d)],
            saidas: action.saidas.iter().map(|s| (s.clone(), None)).collect(),
            origem: Origem::Falha,
            motivo: Some(e),
            measured: Vec::new(),
            from_fallback: Vec::new(),
            apagados: Vec::new(),
            restored: false,
        }
    }

    fn dart_action(&self, a: usize) -> Result<Registro, DartFailure> {
        if let Some(e) = self.prepare_dart() {
            return Err(DartFailure::Unavailable(e));
        }
        let action = self
            .grafo
            .acoes
            .get(a)
            .ok_or(DartFailure::Unavailable("nonexistent Dart action".into()))?;
        let phase = &self.fases[action.fase];
        let memory = |id: &AssetId| {
            self.naturais
                .get(id)
                .and_then(|p| self.memory.get(p))
                .cloned()
        };
        let mut service = ActionService::new(&self.grafo, &self.grafo_pacotes, a, &memory);
        let request = PedidoAcao {
            fase: action.fase,
            chave: self.plano.aplicacoes[phase.aplicacao].chave.clone(),
            fabrica: phase.fabrica.clone(),
            opcoes: phase.opcoes.clone(),
            raiz: phase.raiz,
            entrada: action.entrada.clone(),
            saidas_permitidas: action.saidas.clone(),
        };
        let mut dart = self
            .dart
            .lock()
            .map_err(|_| DartFailure::Unavailable("Dart executor poisoned".into()))?;
        // The executor crashed in the middle of the action: a failure, not a
        // missing executor.
        let result = dart.executar(&request, &mut service).map_err(|e| {
            DartFailure::Builder(format!("{}: executor failure: {}", request.chave, e.0))
        })?;
        if result.falhou {
            // "severo" and "erro" are log levels of the dfexec/1 protocol.
            let details = result
                .logs
                .iter()
                .filter(|(level, _)| level == "severo" || level == "erro")
                .map(|(_, message)| message.as_str())
                .chain(
                    service
                        .logs
                        .iter()
                        .filter(|(level, _)| *level == crate::executor::Nivel::Severo)
                        .map(|(_, message)| message.as_str()),
                )
                .collect::<Vec<_>>()
                .join("; ");
            return Err(DartFailure::Builder(if details.is_empty() {
                format!("{}: Dart builder failed", request.chave)
            } else {
                format!("{}: {details}", request.chave)
            }));
        }
        for (id, bytes) in result.saidas {
            if let Some(already_written) = service.writes.get(&id) {
                if already_written.as_ref() != bytes.as_ref() {
                    return Err(DartFailure::Builder(format!(
                        "Dart builder returned different bytes for {} after writing through BuildStep",
                        id.texto()
                    )));
                }
            }
            service.escrever(&id, bytes).map_err(|e| {
                DartFailure::Builder(format!(
                    "Dart builder wrote a disallowed output: {}",
                    e.0.texto()
                ))
            })?;
        }
        let outputs = action
            .saidas
            .iter()
            .map(|id| (id.clone(), service.writes.get(id).cloned()))
            .collect();
        Ok(Registro {
            fingerprint: [0; 32],
            consultas: service.queries,
            saidas: outputs,
            origem: Origem::Dart,
            motivo: None,
            measured: Vec::new(),
            from_fallback: Vec::new(),
            apagados: Vec::new(),
            restored: false,
        })
    }

    /// `blake3(version ‖ key ‖ factory ‖ imitated ‖ options ‖ identity ‖
    /// input ‖ queries)`.
    fn fingerprint(&self, a: usize, queries: &[(Consulta, Option<Digest>)]) -> Digest {
        let action = &self.grafo.acoes[a];
        let f = &self.fases[action.fase];
        let ap = &self.plano.aplicacoes[f.aplicacao];
        let mut h = blake3::Hasher::new();
        h.update(crate::VERSAO.as_bytes());
        h.update(ap.chave.as_bytes());
        h.update(f.fabrica.as_bytes());
        for (p, v) in self.imitated_versions(&ap.chave) {
            h.update(p.as_bytes());
            h.update(v.as_bytes());
        }
        h.update(f.opcoes.texto_canonico().as_bytes());
        h.update(&[u8::from(f.raiz)]);
        h.update(f.identity(&self.plano, &self.grafo_pacotes).as_bytes());
        h.update(action.entrada.texto().as_bytes());
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
        ctx: &Contexto<'_>,
        g: usize,
        package: &Arc<str>,
        changed: &HashSet<PathBuf>,
        dart_changed: bool,
        structural: &HashSet<PathBuf>,
        rel: &mut RelMotor,
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
                if query_digest(c, ctx.banco, &memory) != *d {
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
        for (fi, f) in self.fases.iter().enumerate() {
            if self.grafo_pacotes.nos[f.pacote].nome != **package
                || self.native_for_phase(fi) != Some(g)
            {
                continue;
            }
            for &a in &self.per_phase[fi] {
                let action = &self.grafo.acoes[a];
                actions.push(NativeAction {
                    fabrica: f.fabrica.clone(),
                    entrada: action.entrada.clone(),
                    entrada_natural: natural_path(&self.grafo_pacotes, &action.entrada),
                    saidas: action
                        .saidas
                        .iter()
                        .map(|s| (s.clone(), self.naturais[s].clone()))
                        .collect(),
                    opcoes: f.opcoes.clone(),
                });
            }
        }
        let node = self.grafo_pacotes.no(package).ok_or("unknown package")?;
        let request = PedidoNativo {
            pacote: package.to_string(),
            raiz_do_pacote: node.raiz.clone(),
            acoes: actions,
            raizes: self.package_roots(),
            versoes: self.imitated_versions(generator.chave()),
        };
        let (result, queries) = {
            let memory = |p: &Path| self.memory.get(p).cloned();
            let mut c = CtxGerador::new(ctx.programa, changed, ctx.banco, &memory);
            let r = generator.gerar(&mut c, &request);
            (r, std::mem::take(&mut c.consultas))
        };
        rel.consultas_gerador += queries.len();
        let round = match result {
            Ok(s) => PackageRound {
                queries: if s.reutilizar_consultas {
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
                    .saidas
                    .into_iter()
                    .map(|(p, b)| (chave(&p), Arc::from(b)))
                    .collect(),
                refusals: s.recusas.into_iter().map(|(p, m)| (chave(&p), m)).collect(),
                error: None,
                generated_units: s.unidades_geradas,
            },
            Err(e) => PackageRound {
                queries,
                outputs: BTreeMap::new(),
                refusals: BTreeMap::new(),
                error: Some(e),
                generated_units: 0,
            },
        };
        rel.unidades_nativas += round.generated_units;
        self.package_rounds.insert(k, round);
        rel.tempo_nativo += t_generate.elapsed();
        Ok(true)
    }

    /// Record of an action covered by a per-package generator: the generated
    /// outputs come from the round; whatever it refused goes to the
    /// fallback.
    fn package_round_record(&self, g: usize, a: usize, dart_reason: Option<&str>) -> Registro {
        let action = &self.grafo.acoes[a];
        let package: Arc<str> = self.grafo_pacotes.nos[self.fases[action.fase].pacote]
            .nome
            .as_str()
            .into();
        let round = self.package_rounds.get(&(g, package));
        let generator = &self.natives[g];
        let outputs: Vec<(AssetId, Option<Arc<[u8]>>)> = action
            .saidas
            .iter()
            .map(|s| {
                (
                    s.clone(),
                    round
                        .and_then(|r| r.outputs.get(&self.naturais[s]))
                        .cloned(),
                )
            })
            .collect();
        let input = natural_path(&self.grafo_pacotes, &action.entrada);
        // The queries stay in the package round (that is what gets
        // revalidated); copying them into each action cost hundreds of ms per
        // edit.
        let queries = Vec::new();
        // The native generator covers the action when it generated some output
        // of it and did not refuse it.
        let refusal =
            round.and_then(|r| r.error.clone().or_else(|| r.refusals.get(&input).cloned()));
        let generated_any = outputs.iter().any(|(_, c)| c.is_some());
        if generator.verificado() && refusal.is_none() && generated_any {
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
            let hidden = self.fases[action.fase].oculta;
            for (s, c) in outputs.iter_mut() {
                if c.is_none() {
                    if let Ok(b) = std::fs::read(self.caminho_de_apoio(s, hidden)) {
                        *c = Some(Arc::from(b));
                        from_fallback.push(s.clone());
                    }
                }
            }
            return Registro {
                fingerprint: [0; 32],
                consultas: queries,
                saidas: outputs,
                origem: Origem::Nativo(generator.chave()),
                motivo: None,
                measured: Vec::new(),
                from_fallback,
                apagados: Vec::new(),
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
            Some(prev) if prev.origem == Origem::Apoio => {
                let mut r = prev.clone();
                r.motivo = dart_reason.map(|m| format!("build_runner fallback: {m}"));
                r
            }
            _ => self.fallback(a, dart_reason),
        };
        if !generator.verificado() {
            r.measured = outputs
                .into_iter()
                .filter_map(|(s, c)| c.map(|c| (s, c)))
                .collect();
        }
        let native_reason = refusal
            .unwrap_or_else(|| format!("{}: does not generate this output", generator.chave()));
        r.motivo = Some(match r.motivo.take() {
            Some(m) => format!("{native_reason}; {m}"),
            None => native_reason,
        });
        r
    }

    /// Name → root of each package in the graph.
    fn package_roots(&self) -> BTreeMap<String, PathBuf> {
        self.grafo_pacotes
            .nos
            .iter()
            .map(|n| (n.nome.clone(), n.raiz.clone()))
            .collect()
    }

    fn native_action(
        &self,
        ctx: &Contexto<'_>,
        changed: &HashSet<PathBuf>,
        g: usize,
        a: usize,
        dart_reason: Option<&str>,
    ) -> Registro {
        let action = &self.grafo.acoes[a];
        let f = &self.fases[action.fase];
        let generator = self.natives[g].clone();
        let request = PedidoNativo {
            pacote: action.entrada.pacote.to_string(),
            raiz_do_pacote: self.grafo_pacotes.nos[f.pacote].raiz.clone(),
            acoes: vec![NativeAction {
                fabrica: f.fabrica.clone(),
                entrada: action.entrada.clone(),
                entrada_natural: natural_path(&self.grafo_pacotes, &action.entrada),
                saidas: action
                    .saidas
                    .iter()
                    .map(|s| (s.clone(), self.naturais[s].clone()))
                    .collect(),
                opcoes: f.opcoes.clone(),
            }],
            raizes: self.package_roots(),
            versoes: self.imitated_versions(generator.chave()),
        };
        let memory = |p: &Path| self.memory.get(p).cloned();
        let mut c = CtxGerador::new(ctx.programa, changed, ctx.banco, &memory);
        let res = generator.gerar(&mut c, &request);
        let queries = std::mem::take(&mut c.consultas);
        match res {
            Ok(s)
                if generator.verificado()
                    && s.recusas.is_empty()
                    && !s.saidas.is_empty()
                    && (dart_reason.is_some()
                        || action
                            .saidas
                            .iter()
                            .all(|id| s.saidas.contains_key(&self.naturais[id]))) =>
            {
                Registro {
                    fingerprint: [0; 32],
                    consultas: queries,
                    saidas: action
                        .saidas
                        .iter()
                        .map(|id| {
                            (
                                id.clone(),
                                s.saidas
                                    .get(&self.naturais[id])
                                    .map(|b| Arc::from(b.clone())),
                            )
                        })
                        .collect(),
                    origem: Origem::Nativo(generator.chave()),
                    motivo: None,
                    measured: Vec::new(),
                    from_fallback: Vec::new(),
                    apagados: Vec::new(),
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
                    .saidas
                    .iter()
                    .filter_map(|id| {
                        s.saidas
                            .get(&self.naturais[id])
                            .map(|b| (id.clone(), Arc::from(b.clone())))
                    })
                    .collect();
                let m = s.recusas.values().next().cloned().unwrap_or_else(|| {
                    format!(
                        "{}: output not yet verified byte for byte",
                        generator.chave()
                    )
                });
                r.motivo = Some(match r.motivo.take() {
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
                r.motivo = Some(match r.motivo.take() {
                    Some(x) => format!("{m}; {x}"),
                    None => m,
                });
                r
            }
        }
    }

    fn fallback_in_parallel(&self, actions: &[usize], dart_reason: Option<&str>) -> Vec<Registro> {
        let n = self.opcoes.trabalhadores.max(1).min(actions.len());
        if n <= 1 || actions.len() < 8 {
            return actions
                .iter()
                .map(|&a| self.fallback(a, dart_reason))
                .collect();
        }
        let next = AtomicUsize::new(0);
        let mut all: Vec<(usize, Registro)> = std::thread::scope(|s| {
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
    fn fallback(&self, a: usize, dart_reason: Option<&str>) -> Registro {
        let action = &self.grafo.acoes[a];
        let f = &self.fases[action.fase];
        let input = natural_path(&self.grafo_pacotes, &action.entrada);
        let memory = |p: &Path| self.memory.get(p).cloned();
        let mut queries = Vec::with_capacity(1 + action.saidas.len());
        let d = memory(&input)
            .map(|b| digest_bytes(&b))
            .or_else(|| crate::consulta::digest_arquivo(&input));
        queries.push((Consulta::Arquivo(input), d));
        let mut outputs = Vec::with_capacity(action.saidas.len());
        for s in &action.saidas {
            let loc = self.caminho_de_apoio(s, f.oculta);
            let c: Option<Arc<[u8]>> = std::fs::read(&loc).ok().map(Arc::from);
            queries.push((
                Consulta::Arquivo(chave(&loc)),
                c.as_deref().map(digest_bytes),
            ));
            outputs.push((s.clone(), c));
        }
        let has_build_dir = self
            .grafo_pacotes
            .dir_raiz
            .join(".dart_tool/build")
            .is_dir();
        let any = outputs.iter().any(|(_, c)| c.is_some());
        if !any && !has_build_dir {
            let m = format!(
                "no fallback: {} (there is no .dart_tool/build; run 'dart run build_runner build')",
                dart_reason.unwrap_or("no executor")
            );
            return Registro {
                fingerprint: [0; 32],
                consultas: queries,
                saidas: outputs,
                origem: Origem::Pendente,
                motivo: Some(m),
                measured: Vec::new(),
                from_fallback: Vec::new(),
                apagados: Vec::new(),
                restored: false,
            };
        }
        Registro {
            fingerprint: [0; 32],
            consultas: queries,
            saidas: outputs,
            origem: Origem::Apoio,
            motivo: dart_reason.map(|m| format!("build_runner fallback: {m}")),
            measured: Vec::new(),
            from_fallback: Vec::new(),
            apagados: Vec::new(),
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
            .flat_map(|r| r.apagados.iter())
            .collect();
        let deleted: HashSet<AssetId> = deleted.into_iter().cloned().collect();
        let mut c = Construtor::nova();
        let mut h = blake3::Hasher::new();
        let mut items: Vec<(PathBuf, Arc<[u8]>, &'static str, PathBuf)> = Vec::new();
        let mut labels: Vec<(usize, String)> = Vec::new();
        for (a, r) in self.records.iter().enumerate() {
            let Some(r) = r else { continue };
            let action = &self.grafo.acoes[a];
            let f = &self.fases[action.fase];
            if !f.oculta && !matches!(r.origem, Origem::Nativo(_) | Origem::Dart) {
                continue;
            }
            for (s, content) in &r.saidas {
                let Some(content) = content else { continue };
                if deleted.contains(s) {
                    continue;
                }
                let label = match r.origem {
                    Origem::Nativo(_) | Origem::Dart => {
                        self.plano.aplicacoes[f.aplicacao].chave.clone()
                    }
                    _ => "build_runner".to_string(),
                };
                labels.push((a, label));
                items.push((
                    self.natural_path_of(s),
                    content.clone(),
                    "",
                    natural_path(&self.grafo_pacotes, &action.entrada),
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
            c.por(n, text, labels[i], vec![input]);
        }
        let id = u64::from_le_bytes(h.finalize().as_bytes()[..8].try_into().unwrap_or([0; 8]));
        match c.concluir(id) {
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
    pub fn materializar(&mut self, ctx: &Contexto<'_>, path: &Path) -> Option<Arc<[u8]>> {
        let mut k = chave(path);
        if let Some(c) = self.memory.get(&k) {
            return Some(c.clone());
        }
        // `package_config.json` uses the canonical root; the served URL may
        // arrive with the lexical spelling (8.3 alias or link on Windows).
        // The generated file does not exist yet, so only the parent is
        // canonicalized.
        if !self.naturais.values().any(|n| n == &k) {
            if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
                if let Ok(parent) = std::fs::canonicalize(parent) {
                    k = chave(&dartforge_elements::config::sem_verbatim(parent).join(name));
                }
            }
        }
        if let Some(c) = self.memory.get(&k) {
            return Some(c.clone());
        }
        let id = self
            .naturais
            .iter()
            .find(|(_, n)| **n == k)
            .map(|(id, _)| id.clone())?;
        let g = self.grafo.gerados.get(&id)?.clone();
        if self.records[g.acao].as_ref().is_some_and(|r| r.restored) {
            let (dirty, off_disk) = self.check_restored(g.acao, ctx.banco);
            for n in off_disk {
                self.memory.remove(&n);
            }
            match self.records[g.acao].as_mut() {
                Some(r) if !dirty => r.restored = false,
                _ => self.records[g.acao] = None,
            }
        }
        if self.records[g.acao].is_none() {
            let a = g.acao;
            let fi = self.grafo.acoes[a].fase;
            let dart_reason = self
                .dart
                .lock()
                .map(|d| match d.disponibilidade() {
                    Disponibilidade::Disponivel => None,
                    Disponibilidade::Indisponivel(m) => Some(m),
                })
                .unwrap_or_else(|_| Some("Dart executor poisoned".into()));
            let mut r = match self.native_for_phase(fi) {
                Some(n) if !self.natives[n].por_pacote() => {
                    self.native_action(ctx, &HashSet::new(), n, a, dart_reason.as_deref())
                }
                _ if dart_reason.is_none() => self.dart_or_fallback(a),
                _ => self.fallback(a, dart_reason.as_deref()),
            };
            r.fingerprint = self.fingerprint(a, &r.consultas);
            for (s, c) in &r.saidas {
                if let Some(c) = c {
                    self.memory.insert(self.naturais[s].clone(), c.clone());
                }
            }
            self.records[a] = Some(r);
            self.state_dirty = true;
            self.publish();
            self.recompute_watched();
            if self.opcoes.persistir && self.save().is_ok() {
                self.state_dirty = false;
            }
        }
        self.memory.get(&k).cloned()
    }

    /// `build_to: source` outputs of native generators among `altered`, with
    /// their content: they go to disk (D-B2). The fallback ones are already
    /// there.
    pub fn saidas_source_nativas(&self, altered: &[PathBuf]) -> Vec<(PathBuf, Arc<[u8]>)> {
        if altered.is_empty() {
            return Vec::new();
        }
        let alt: HashSet<&PathBuf> = altered.iter().collect();
        let mut v = Vec::new();
        for (a, r) in self.records.iter().enumerate() {
            let Some(r) = r else { continue };
            if !matches!(r.origem, Origem::Nativo(_) | Origem::Dart)
                || self.fases[self.grafo.acoes[a].fase].oculta
            {
                continue;
            }
            for (s, c) in &r.saidas {
                let n = &self.naturais[s];
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
    pub fn estado_canonico(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        for (i, a) in self.grafo.acoes.iter().enumerate() {
            if !a.viva() {
                continue;
            }
            let f = &self.fases[a.fase];
            let head = format!(
                "{}#{} {}",
                self.plano.aplicacoes[f.aplicacao].chave,
                f.fabrica,
                a.entrada.texto()
            );
            match &self.records[i] {
                None => lines.push(format!("{head} -")),
                Some(r) => {
                    let outputs: Vec<String> = r
                        .saidas
                        .iter()
                        .map(|(s, c)| {
                            let d = c
                                .as_deref()
                                .map(|b| blake3::hash(b).to_hex().to_string())
                                .unwrap_or_else(|| "∅".into());
                            format!("{}={}", s.caminho, &d[..d.len().min(16)])
                        })
                        .collect();
                    let deleted: Vec<String> = r.apagados.iter().map(AssetId::texto).collect();
                    lines.push(format!(
                        "{head} {:?} {} [{}]{}",
                        r.origem,
                        r.motivo.as_deref().unwrap_or(""),
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
                    .strip_prefix(&self.raiz)
                    .unwrap_or(p)
                    .to_string_lossy()
                    .replace('\\', "/");
                format!(
                    "generated {rel} {}",
                    &blake3::hash(f.conteudo.as_bytes()).to_hex()[..16]
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
    /// The reasons that start with "pós-processador" stay in Portuguese:
    /// `crates/build/tests/corpus.rs` matches that prefix on the pending
    /// reasons.
    pub fn placar(&self, reference: &BTreeMap<AssetId, Vec<u8>>) -> Placar {
        let mut p = Placar::default();
        for (id, expected) in reference {
            let Some(g) = self.grafo.gerados.get(id) else {
                // A post-processor writes outputs the graph does not predict
                // (`PostProcessBuildStep`): they are in the anchor's record.
                let from_anchor = self
                    .grafo
                    .acoes
                    .iter()
                    .zip(&self.records)
                    .filter(|(a, _)| a.pos)
                    .filter_map(|(_, r)| r.as_ref())
                    .find_map(|r| {
                        r.saidas
                            .iter()
                            .find(|(s, _)| s == id)
                            .map(|(_, c)| (r, c.clone()))
                    });
                if let Some((r, c)) = from_anchor {
                    match (&r.origem, c) {
                        (Origem::Dart, Some(c)) if c.as_ref() == expected.as_slice() => {
                            p.iguais.push(id.clone())
                        }
                        _ => p
                            .diferentes
                            .push((id.clone(), "pós-processador: difere do oficial".into())),
                    }
                    continue;
                }
                let has_post = self
                    .fases
                    .iter()
                    .any(|f| f.pos && self.grafo_pacotes.nos[f.pacote].nome == *id.pacote);
                if has_post {
                    p.pendentes.push((
                        id.clone(),
                        "pós-processador: saída só com o executor Dart".into(),
                    ));
                } else {
                    p.diferentes
                        .push((id.clone(), "output not predicted by the plan".into()));
                }
                continue;
            };
            match self.records[g.acao].as_ref() {
                None => p.pendentes.push((id.clone(), "not computed".into())),
                Some(r) => {
                    let c = r
                        .saidas
                        .iter()
                        .find(|(s, _)| s == id)
                        .and_then(|(_, c)| c.clone());
                    match (&r.origem, c) {
                        (Origem::Nativo(n), _) if r.from_fallback.contains(id) => {
                            p.pendentes.push((
                                id.clone(),
                                format!(
                                    "{n}: output not generated by the native generator; build_runner fallback"
                                ),
                            ))
                        }
                        (Origem::Nativo(_) | Origem::Dart, Some(c))
                            if c.as_ref() == expected.as_slice() =>
                        {
                            p.iguais.push(id.clone())
                        }
                        (Origem::Nativo(n), _) => p
                            .diferentes
                            .push((id.clone(), format!("{n}: differs from the official output"))),
                        (Origem::Dart, _) => p.diferentes.push((
                            id.clone(),
                            "Dart executor: differs from the official output".into(),
                        )),
                        (Origem::NotTriggered, _) => p.diferentes.push((
                            id.clone(),
                            "no trigger fired here; the official tool wrote it".into(),
                        )),
                        (Origem::Omitida, _) => p.diferentes.push((
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
                            p.pendentes.push((
                                id.clone(),
                                r.motivo.clone().unwrap_or_else(|| "fallback".into()),
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
            let n = match r.origem {
                Origem::Nativo(n) => n,
                Origem::Dart => "Dart executor",
                _ => continue,
            };
            let action = &self.grafo.acoes[i];
            let optional = self.fases[action.fase].opcional;
            for (s, c) in &r.saidas {
                if c.is_none() || reference.contains_key(s) {
                    continue;
                }
                if optional && !self.failed_in_official(&action.entrada) {
                    p.nao_solicitados.push((
                        s.clone(),
                        format!("{n}: optional output the official graph does not request"),
                    ));
                } else {
                    p.diferentes.push((s.clone(), format!("{n}: extra output")));
                }
            }
        }
        p
    }

    /// The official `build_runner` recorded an error of some builder on this
    /// input (`.dart_tool/build/<hash>/error_cache/<package>/<phase>/<path>`).
    fn failed_in_official(&self, input: &AssetId) -> bool {
        let base = self.grafo_pacotes.dir_raiz.join(".dart_tool/build");
        let Ok(hashes) = std::fs::read_dir(&base) else {
            return false;
        };
        for h in hashes.flatten() {
            let package = h.path().join("error_cache").join(input.pacote.as_ref());
            let Ok(phases) = std::fs::read_dir(&package) else {
                continue;
            };
            for f in phases.flatten() {
                if f.path().join(input.caminho.as_ref()).is_file() {
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
        return Some(AssetId::novo(p, &format!("lib/{c}")));
    }
    if let Some(rest) = uri.strip_prefix("asset:") {
        let (p, c) = rest.split_once('/')?;
        return Some(AssetId::novo(p, c));
    }
    if uri.contains(':') {
        return None;
    }
    let mut parts: Vec<&str> = if uri.starts_with('/') {
        Vec::new()
    } else {
        let mut v: Vec<&str> = from.caminho.split('/').collect();
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
    Some(AssetId::novo(&from.pacote, &parts.join("/")))
}

fn is_newer(input: &Path, fallback: &Path) -> bool {
    let (Some(e), Some(a)) = (Marca::ler(input), Marca::ler(fallback)) else {
        return false;
    };
    e.mtime_ns > a.mtime_ns
}

#[derive(Debug, Default, Clone)]
pub struct Placar {
    pub iguais: Vec<AssetId>,
    pub pendentes: Vec<(AssetId, String)>,
    pub diferentes: Vec<(AssetId, String)>,
    /// Outputs the builder could produce but the official graph never asks
    /// for (an optional builder with nobody reading the output): not counted
    /// — they are neither expected, nor pending, nor different.
    pub nao_solicitados: Vec<(AssetId, String)>,
    /// Unverified native generators, measured against the reference.
    pub measured_equal: usize,
    pub measured_different: usize,
}

impl Placar {
    pub fn resumo(&self) -> String {
        let mut s = format!(
            "{} equal / {} pending / {} different",
            self.iguais.len(),
            self.pendentes.len(),
            self.diferentes.len()
        );
        if !self.nao_solicitados.is_empty() {
            s.push_str(&format!(
                " ({} not requested, not counted)",
                self.nao_solicitados.len()
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

    pub fn motivos(&self) -> BTreeMap<String, usize> {
        let mut m = BTreeMap::new();
        for (_, x) in self.pendentes.iter().chain(&self.diferentes) {
            *m.entry(x.clone()).or_default() += 1;
        }
        m
    }
}
