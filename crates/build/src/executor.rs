//! The executors behind a single choice (`docs/BUILD-MOTOR.md` §6):
//! **native** (Rust, reads the semantic database), **Dart** (`dfexec/1`: the
//! VM process of [`crate::vm`], or [`Indisponivel`] when it was not enabled)
//! and **fallback** (what `build_runner` left on disk, in `motor.rs`). An
//! executor that cannot generate refuses with a reason.
use crate::consulta::{BancoSemantico, Consulta, Digest, digest_arquivo, digest_bytes};
use crate::grafo::{AssetId, Grafo};
use crate::pacotes::GrafoPacotes;
use crate::valor::Mapa;
use dartforge_elements::model::Program;
use dartforge_intern::Interner;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

// ------------------------------------------------------------------ native

/// An action requested from a native generator.
#[derive(Debug, Clone)]
pub struct NativeAction {
    pub fabrica: String,
    pub entrada: AssetId,
    pub entrada_natural: PathBuf,
    /// Expected outputs and the natural path of each one.
    pub saidas: Vec<(AssetId, PathBuf)>,
    pub opcoes: Mapa,
}

/// A request: one action (per-file generator) or all the actions of the
/// phases the generator covers in a package (per-package generator — ngdart's
/// stage A).
#[derive(Debug, Clone)]
pub struct PedidoNativo {
    pub pacote: String,
    pub raiz_do_pacote: PathBuf,
    pub acoes: Vec<NativeAction>,
    /// The root of each package in `package_config.json` (Sass's `package:`
    /// resolves through them, like `AssetId.resolve` in `build`).
    pub raizes: BTreeMap<String, PathBuf>,
    /// The `pubspec.lock` version of each package the generator imitates
    /// ([`crate::descritor::imita`]): `sass_builder` picks the dart-sass
    /// version by it.
    pub versoes: BTreeMap<String, String>,
}

/// What a native generator returns: content by natural output path, and
/// refusals by natural **input** path (the scoreboard counts the reasons).
#[derive(Debug, Default)]
pub struct SaidaNativa {
    pub saidas: BTreeMap<PathBuf, Vec<u8>>,
    pub recusas: BTreeMap<PathBuf, String>,
    /// Files the generator actually regenerated in this call.
    pub unidades_geradas: usize,
    /// The generator confirmed that the previous round's queries are still
    /// valid; `CtxGerador` records only the answers that changed.
    pub reutilizar_consultas: bool,
}

/// A Rust generator that imitates an ecosystem builder byte for byte.
pub trait GeradorNativo: Send + Sync {
    /// Builder key (`ngdart:ngdart`).
    fn chave(&self) -> &'static str;
    /// Covered factories.
    fn cobre(&self, factory: &str) -> bool;
    /// One call per package (with all the actions) instead of one per action.
    fn por_pacote(&self) -> bool;
    /// Equality gate: `false` = the output is not yet verified byte for byte
    /// against the official one; the engine only uses it to measure
    /// (`--comparar`) and publishes the fallback instead.
    fn verificado(&self) -> bool;
    fn gerar(&self, ctx: &mut CtxGerador<'_>, request: &PedidoNativo)
    -> Result<SaidaNativa, String>;
}

/// The only read channel of a native generator: every read becomes a query
/// (soundness rule — what it did not query cannot invalidate it).
pub struct CtxGerador<'a> {
    pub programa: Option<(&'a Program, &'a Interner)>,
    /// Events of this update, already normalized by the engine.
    pub mudados: &'a HashSet<PathBuf>,
    pub(crate) banco: &'a dyn BancoSemantico,
    pub(crate) memoria: &'a dyn Fn(&Path) -> Option<Arc<[u8]>>,
    pub(crate) consultas: Vec<(Consulta, Option<Digest>)>,
}

impl<'a> CtxGerador<'a> {
    pub(crate) fn new(
        program: Option<(&'a Program, &'a Interner)>,
        changed: &'a HashSet<PathBuf>,
        database: &'a dyn BancoSemantico,
        memory: &'a dyn Fn(&Path) -> Option<Arc<[u8]>>,
    ) -> Self {
        CtxGerador {
            programa: program,
            mudados: changed,
            banco: database,
            memoria: memory,
            consultas: Vec::new(),
        }
    }

    /// Records a query with the digest of its current answer.
    pub fn registrar(&mut self, c: Consulta) {
        let d = query_digest(&c, self.banco, self.memoria);
        self.consultas.push((c, d));
    }

    pub fn ler(&mut self, p: &Path) -> Option<Arc<[u8]>> {
        let key = dartforge_elements::gerado::chave(p);
        let v = (self.memoria)(&key).or_else(|| std::fs::read(p).ok().map(Arc::from));
        let d = v.as_deref().map(digest_bytes);
        self.consultas.push((Consulta::Arquivo(key), d));
        v
    }

    pub fn existe(&mut self, p: &Path) -> bool {
        let key = dartforge_elements::gerado::chave(p);
        let yes = (self.memoria)(&key).is_some() || p.is_file();
        self.consultas
            .push((Consulta::Existe(key), yes.then(|| digest_bytes(b"1"))));
        yes
    }
}

/// The candidates of a `findAssets` call by action `action`, in order:
/// sources and predicted outputs of the input's package that match the
/// pattern, without the outputs the action cannot see because of its phase
/// (`build_impl.dart:443-463`). The engine recomputes the list after a
/// structural event to know whether the glob's answer actually changed.
pub(crate) fn glob_candidates(
    graph: &Grafo,
    action: usize,
    pattern: &crate::glob::Glob,
) -> Vec<AssetId> {
    let Some(a) = graph.acoes.get(action) else {
        return Vec::new();
    };
    let (package, phase) = (a.entrada.pacote.clone(), a.fase);
    let sources = graph.fontes.get(&package).into_iter().flat_map(|cs| {
        cs.iter().map(|c| AssetId {
            pacote: package.clone(),
            caminho: c.clone(),
        })
    });
    let generated = graph
        .gerados
        .range(AssetId::novo(&package, "")..)
        .take_while(|(id, _)| id.pacote == package)
        .map(|(id, _)| id.clone());
    let mut ids: Vec<_> = sources
        .chain(generated)
        .filter(|id| {
            pattern.casa(&id.caminho)
                && graph
                    .gerados
                    .get(id)
                    .is_none_or(|g| g.fase < phase || (g.fase == phase && g.acao == action))
        })
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// Current digest of a query's answer.
pub(crate) fn query_digest(
    c: &Consulta,
    database: &dyn BancoSemantico,
    memory: &dyn Fn(&Path) -> Option<Arc<[u8]>>,
) -> Option<Digest> {
    match c {
        Consulta::Arquivo(p) => memory(p)
            .map(|b| digest_bytes(&b))
            .or_else(|| digest_arquivo(p)),
        Consulta::Existe(p) => (memory(p).is_some() || p.is_file()).then(|| digest_bytes(b"1")),
        Consulta::Glob { dir, padrao } => {
            let g = crate::glob::Glob::novo(padrao).ok()?;
            let list = crate::grafo::listar(dir, std::slice::from_ref(&g));
            Some(digest_bytes(
                list.into_iter().collect::<Vec<_>>().join("\n").as_bytes(),
            ))
        }
        Consulta::GlobAtivos {
            dir, candidatos, ..
        } => {
            let list = candidatos
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
pub enum Disponibilidade {
    Disponivel,
    Indisponivel(String),
}

#[derive(Debug, Clone)]
pub struct ErroExecutor(pub String);

#[derive(Debug, Clone)]
pub struct SaidaNaoPermitida(pub AssetId);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nivel {
    Fino,
    Info,
    Aviso,
    Severo,
}

/// The equivalent of `.dart_tool/build/entrypoint/build.dart`: the factory
/// imports and the key → factories map (`build_script_generate.dart`).
#[derive(Debug, Clone)]
pub struct ScriptDeBuilders {
    pub aplicacoes: Vec<(String, String, Vec<String>)>,
    /// Interim key from the plan, lock versions and engine version; sources
    /// and ABI will be added before enabling the real process cache.
    pub chave_de_cache: String,
}

/// `build.executar` of `dfexec/1` (`docs/BUILD-PROTOCOLO.md` §3).
#[derive(Debug, Clone)]
pub struct PedidoAcao {
    pub fase: usize,
    pub chave: String,
    pub fabrica: String,
    pub opcoes: Mapa,
    pub raiz: bool,
    pub entrada: AssetId,
    pub saidas_permitidas: Vec<AssetId>,
}

/// Run-time extensions in the order of the `buildExtensions` map: input →
/// outputs.
pub type ExtensoesDeExecucao = Vec<(String, Vec<String>)>;

/// `build.extensoes` of `dfexec/1`: a phase's factory, with its options.
#[derive(Debug, Clone)]
pub struct PedidoExtensoes {
    pub chave: String,
    pub fabrica: String,
    pub opcoes: Mapa,
    pub raiz: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ResultadoAcao {
    pub saidas: Vec<(AssetId, Arc<[u8]>)>,
    pub logs: Vec<(String, String)>,
    pub falhou: bool,
    /// Post-processor: the inputs marked by `deletePrimaryInput` (the node's
    /// `deletedBy`; only the merged output directory and the official
    /// `serve` stop seeing them).
    pub apagados: Vec<AssetId>,
}

/// The `Resolver` served by the semantic database (Phase 3 of BUILD-RUST.md).
pub trait ServicoResolver {}

/// The `BuildStep` served by the engine: every call becomes a `Consulta`.
pub trait ServicoBuildStep {
    fn can_read(&mut self, id: &AssetId) -> bool;
    fn ler(&mut self, id: &AssetId) -> Option<Arc<[u8]>>;
    fn find_assets(&mut self, glob: &str) -> Vec<AssetId>;
    fn digest(&mut self, id: &AssetId) -> Option<[u8; 32]>;
    fn escrever(&mut self, id: &AssetId, bytes: Arc<[u8]>) -> Result<(), SaidaNaoPermitida>;
    /// `None` ⇒ the analyzer runs inside the executor.
    fn resolver(&mut self) -> Option<&mut dyn ServicoResolver>;
    fn log(&mut self, level: Nivel, msg: &str);
}

/// An action's view of the graph. The executor never receives free paths:
/// an `AssetId` must exist in the graph, and outputs of the action's own
/// phase only become readable after the current action writes them
/// (`build_impl.dart:443-463`).
pub struct ActionService<'a> {
    pub graph: &'a Grafo,
    pub packages: &'a GrafoPacotes,
    pub action: usize,
    pub memory: &'a dyn Fn(&AssetId) -> Option<Arc<[u8]>>,
    pub writes: BTreeMap<AssetId, Arc<[u8]>>,
    pub queries: Vec<(Consulta, Option<Digest>)>,
    pub logs: Vec<(Nivel, String)>,
    /// In a post-processor anchor: the outputs other anchors already wrote
    /// (the official `addAsset` rejects an asset the graph already has).
    pub occupied: std::collections::BTreeSet<AssetId>,
}

impl<'a> ActionService<'a> {
    pub fn new(
        graph: &'a Grafo,
        packages: &'a GrafoPacotes,
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
        self.graph.tem_fonte(id) || self.graph.fonte_externa(id)
    }

    fn path(&self, id: &AssetId) -> Option<PathBuf> {
        if !self.graph.existe(id) && !self.graph.fonte_externa(id) {
            return None;
        }
        self.query_path(id)
    }

    fn query_path(&self, id: &AssetId) -> Option<PathBuf> {
        let node = self.packages.no(&id.pacote)?;
        // Negative queries must observe files that have not entered the
        // graph yet; even so, they cannot escape the package.
        if id
            .caminho
            .split('/')
            .any(|p| p.is_empty() || p == ".." || p == "." || p.contains('\\') || p.contains(':'))
        {
            return None;
        }
        Some(dartforge_elements::gerado::chave(
            &node.raiz.join(id.caminho.as_ref()),
        ))
    }

    fn bytes(&self, id: &AssetId) -> Option<Arc<[u8]>> {
        let phase = self.graph.acoes.get(self.action)?.fase;
        if let Some(g) = self.graph.gerados.get(id) {
            if g.fase > phase || (g.fase == phase && g.acao != self.action) {
                return None;
            }
            return if g.fase == phase {
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
        let Some(phase) = self.graph.acoes.get(self.action).map(|a| a.fase) else {
            return false;
        };
        if let Some(g) = self.graph.gerados.get(id) {
            return if g.fase > phase {
                false
            } else if g.fase == phase {
                g.acao == self.action && self.writes.contains_key(id)
            } else {
                (self.memory)(id).is_some()
            };
        }
        self.is_source(id) && self.path(id).is_some_and(|p| p.is_file())
    }

    fn record(&mut self, id: &AssetId, exists: bool, bytes: Option<&[u8]>) {
        if let Some(p) = self.query_path(id) {
            let c = if exists {
                Consulta::Existe(p)
            } else {
                Consulta::Arquivo(p)
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

impl ServicoBuildStep for ActionService<'_> {
    fn can_read(&mut self, id: &AssetId) -> bool {
        let yes = self.is_available(id);
        self.record(id, true, yes.then_some(&[]));
        yes
    }

    fn ler(&mut self, id: &AssetId) -> Option<Arc<[u8]>> {
        let b = self.bytes(id);
        self.record(id, false, b.as_deref());
        b
    }

    fn find_assets(&mut self, glob: &str) -> Vec<AssetId> {
        let Ok(pattern) = crate::glob::Glob::novo(glob) else {
            return Vec::new();
        };
        let Some(package) = self
            .graph
            .acoes
            .get(self.action)
            .map(|a| a.entrada.pacote.clone())
        else {
            return Vec::new();
        };
        let mut ids = glob_candidates(self.graph, self.action, &pattern);
        let candidates: Vec<_> = ids
            .iter()
            .map(|id| (id.caminho.to_string(), self.graph.gerados.contains_key(id)))
            .collect();
        ids.retain(|id| self.can_read(id));
        // An empty listing also depends on the generated outputs that do not
        // exist yet. An old fallback file does not count as a current output.
        if let Some(node) = self.packages.no(&package) {
            self.queries.push((
                Consulta::GlobAtivos {
                    dir: node.raiz.clone(),
                    padrao: glob.to_string(),
                    candidatos: candidates,
                },
                Some(digest_bytes(
                    ids.iter()
                        .map(|id| id.caminho.as_ref())
                        .collect::<Vec<_>>()
                        .join("\n")
                        .as_bytes(),
                )),
            ));
        }
        ids
    }

    fn digest(&mut self, id: &AssetId) -> Option<[u8; 32]> {
        self.ler(id).as_deref().map(digest_bytes)
    }

    fn escrever(&mut self, id: &AssetId, bytes: Arc<[u8]>) -> Result<(), SaidaNaoPermitida> {
        let Some(action) = self.graph.acoes.get(self.action) else {
            return Err(SaidaNaoPermitida(id.clone()));
        };
        if action.pos {
            // `PostProcessBuildStep.writeAsBytes` → `addAsset`
            // (`build_impl.dart:643-656`): any asset the graph does not have
            // yet — not a source, not a predicted output, not another
            // anchor's output, nor the same asset twice in the action.
            let new = !self.graph.existe(id)
                && !self.graph.fonte_externa(id)
                && !self.occupied.contains(id)
                && !self.writes.contains_key(id)
                && self.packages.no(&id.pacote).is_some()
                && self.query_path(id).is_some();
            if !new {
                return Err(SaidaNaoPermitida(id.clone()));
            }
            self.writes.insert(id.clone(), bytes);
            return Ok(());
        }
        if self.path(id).is_none() || !action.saidas.contains(id) {
            return Err(SaidaNaoPermitida(id.clone()));
        }
        self.writes.insert(id.clone(), bytes);
        Ok(())
    }

    fn resolver(&mut self) -> Option<&mut dyn ServicoResolver> {
        None
    }

    fn log(&mut self, level: Nivel, msg: &str) {
        self.logs.push((level, msg.to_string()));
    }
}

/// The Dart builder executor (Phase 4): a persistent process that speaks
/// `dfexec/1`. It will come from the self-hosted native executor, shared
/// with macros.
pub trait ExecutorDart: Send {
    fn disponibilidade(&self) -> Disponibilidade;
    fn preparar(&mut self, script: &ScriptDeBuilders) -> Result<(), ErroExecutor>;
    /// Starts an engine update: the interval between two `build_runner`
    /// builds, in which it resets the per-build state of the resolvers
    /// (`build_impl.dart:94-96`). Only called after a successful `preparar`.
    fn nova_rodada(&mut self) {}
    /// The run-time extensions of the `Builder` the factory returns with
    /// these options (`builder.buildExtensions`, in map order): what
    /// `build_runner` uses to predict the outputs. `None` when the executor
    /// cannot answer; the engine keeps the ones from the descriptor or from
    /// `build.yaml`.
    fn extensoes(
        &mut self,
        _request: &PedidoExtensoes,
    ) -> Result<Option<ExtensoesDeExecucao>, ErroExecutor> {
        Ok(None)
    }
    fn executar(
        &mut self,
        request: &PedidoAcao,
        service: &mut dyn ServicoBuildStep,
    ) -> Result<ResultadoAcao, ErroExecutor>;
    /// The `inputExtensions` of the `PostProcessBuilder` the factory returns
    /// with these options. `None` when the executor cannot answer: the
    /// post-processing phase gets no anchors and the scoreboard counts it as
    /// pending.
    fn entradas_pos(
        &mut self,
        _request: &PedidoExtensoes,
    ) -> Result<Option<Vec<String>>, ErroExecutor> {
        Ok(None)
    }
    /// `build.posprocessar`: `package:build`'s `runPostProcessBuilder` on the
    /// anchor's input. `saidas_permitidas` comes empty; writes go through
    /// [`ServicoBuildStep::escrever`], which applies the `addAsset` rule.
    /// The files the builders' code depends on (the bootstrap's depfile), so
    /// that the state saved across processes knows when a Dart action became
    /// stale without any of its queries changing. `None` when the executor
    /// does not know: Dart actions are not saved.
    fn codigo(&self) -> Option<Vec<PathBuf>> {
        None
    }
    fn pos_processar(
        &mut self,
        _request: &PedidoAcao,
        _service: &mut dyn ServicoBuildStep,
    ) -> Result<ResultadoAcao, ErroExecutor> {
        Err(ErroExecutor(
            "this executor does not run post-processors".into(),
        ))
    }
    fn encerrar(&mut self);
}

/// The executor when there is no Dart process: every Dart action is refused
/// with the reason (the engine falls back). The VM executor is in
/// [`crate::vm`].
pub struct Indisponivel(pub String);

impl Default for Indisponivel {
    fn default() -> Self {
        Indisponivel(
            "Dart builder executor disabled (the self-hosted one does not exist yet, BUILD-RUST.md Phase 4; through the VM: `dartforge build --dart <exe>` or DARTFORGE_BUILD_DART)"
                .into(),
        )
    }
}

impl ExecutorDart for Indisponivel {
    fn disponibilidade(&self) -> Disponibilidade {
        Disponibilidade::Indisponivel(self.0.clone())
    }
    fn preparar(&mut self, _script: &ScriptDeBuilders) -> Result<(), ErroExecutor> {
        Err(ErroExecutor(self.0.clone()))
    }
    fn executar(
        &mut self,
        _p: &PedidoAcao,
        _s: &mut dyn ServicoBuildStep,
    ) -> Result<ResultadoAcao, ErroExecutor> {
        Err(ErroExecutor(self.0.clone()))
    }
    fn encerrar(&mut self) {}
}

#[cfg(test)]
mod service_tests {
    use super::*;
    use crate::config::TipoDependencia;
    use crate::grafo::{Acao, NoGerado};
    use crate::pacotes::No;

    #[test]
    fn phase_visibility_and_the_actions_own_output() {
        let dir = tempfile::tempdir().unwrap();
        let packages = GrafoPacotes {
            nos: vec![
                No {
                    nome: "p".into(),
                    raiz: dir.path().join("p"),
                    tipo: TipoDependencia::Path,
                    e_raiz: true,
                    deps: vec![],
                },
                No {
                    nome: "q".into(),
                    raiz: dir.path().join("q"),
                    tipo: TipoDependencia::Path,
                    e_raiz: false,
                    deps: vec![],
                },
            ],
            raiz: 0,
            por_nome: [("p".into(), 0), ("q".into(), 1)].into(),
            lock: Default::default(),
            dir_raiz: dir.path().to_path_buf(),
        };
        let source = AssetId::novo("p", "lib/a.dart");
        let first = AssetId::novo("p", "lib/a.g.dart");
        let foreign = AssetId::novo("q", "lib/other.dart");
        let second = AssetId::novo("p", "lib/a.h.dart");
        let future = AssetId::novo("p", "lib/a.i.dart");
        let escape = AssetId::novo("p", "lib/../escape.dart");
        std::fs::create_dir_all(dir.path().join("p/lib")).unwrap();
        std::fs::create_dir_all(dir.path().join("q/lib")).unwrap();
        std::fs::write(dir.path().join("p/lib/a.dart"), b"source").unwrap();
        std::fs::write(dir.path().join("q/lib/other.dart"), b"foreign").unwrap();
        let mut graph = Grafo::default();
        graph
            .fontes
            .entry("p".into())
            .or_default()
            .insert("lib/a.dart".into());
        graph
            .fontes
            .entry("q".into())
            .or_default()
            .insert("lib/other.dart".into());
        graph.acoes = vec![
            Acao {
                fase: 0,
                entrada: source.clone(),
                saidas: vec![first.clone()],
                pos: false,
            },
            Acao {
                fase: 1,
                entrada: first.clone(),
                saidas: vec![second.clone(), escape.clone()],
                pos: false,
            },
            Acao {
                fase: 2,
                entrada: second.clone(),
                saidas: vec![future.clone()],
                pos: false,
            },
        ];
        for (id, action, phase) in [(&first, 0, 0), (&second, 1, 1), (&future, 2, 2)] {
            graph.gerados.insert(
                id.clone(),
                NoGerado {
                    acao: action,
                    fase: phase,
                    oculto: false,
                },
            );
        }
        graph.gerados.insert(
            escape.clone(),
            NoGerado {
                acao: 1,
                fase: 1,
                oculto: false,
            },
        );
        let memory: BTreeMap<AssetId, Arc<[u8]>> = [
            (first.clone(), Arc::from(&b"prior"[..])),
            (future.clone(), Arc::from(&b"future"[..])),
        ]
        .into();
        let read_memory = |id: &AssetId| memory.get(id).cloned();
        let mut s = ActionService::new(&graph, &packages, 1, &read_memory);
        assert_eq!(s.ler(&source).as_deref(), Some(&b"source"[..]));
        assert_eq!(s.ler(&first).as_deref(), Some(&b"prior"[..]));
        assert!(!s.can_read(&second));
        assert!(!s.can_read(&future));
        assert!(s.escrever(&future, Arc::from(&b"bad"[..])).is_err());
        assert!(s.escrever(&escape, Arc::from(&b"bad"[..])).is_err());
        let new = AssetId::novo("p", "lib/new.dart");
        assert!(!s.can_read(&new));
        assert!(
            s.queries.iter().any(
                |(c, d)| matches!(c, Consulta::Existe(p) if p.ends_with("new.dart"))
                    && d.is_none()
            ),
            "a negative canRead must invalidate when the source appears"
        );
        s.escrever(&second, Arc::from(&b"own"[..])).unwrap();
        assert_eq!(s.ler(&second).as_deref(), Some(&b"own"[..]));
        assert_eq!(s.find_assets("lib/**"), vec![source, first, second]);
        assert!(
            !s.find_assets("lib/**").contains(&foreign),
            "findAssets stays in the input's package, like build 2.4.2"
        );
        assert!(
            s.queries
                .iter()
                .any(|(c, d)| matches!(c, Consulta::Existe(_)) && d.is_none())
        );
    }

    #[test]
    fn post_processor_anchor_writes_only_new_assets() {
        let dir = tempfile::tempdir().unwrap();
        let packages = GrafoPacotes {
            nos: vec![No {
                nome: "p".into(),
                raiz: dir.path().join("p"),
                tipo: TipoDependencia::Path,
                e_raiz: true,
                deps: vec![],
            }],
            raiz: 0,
            por_nome: [("p".into(), 0)].into(),
            lock: Default::default(),
            dir_raiz: dir.path().to_path_buf(),
        };
        std::fs::create_dir_all(dir.path().join("p/lib")).unwrap();
        std::fs::write(dir.path().join("p/lib/a.draft"), b"x").unwrap();
        let input = AssetId::novo("p", "lib/a.draft");
        let predicted = AssetId::novo("p", "lib/a.g.dart");
        let mut graph = Grafo::default();
        graph
            .fontes
            .entry("p".into())
            .or_default()
            .insert("lib/a.draft".into());
        graph.acoes = vec![
            Acao {
                fase: 0,
                entrada: input.clone(),
                saidas: vec![predicted.clone()],
                pos: false,
            },
            Acao {
                fase: 1,
                entrada: input.clone(),
                saidas: vec![],
                pos: true,
            },
        ];
        graph.gerados.insert(
            predicted.clone(),
            NoGerado {
                acao: 0,
                fase: 0,
                oculto: true,
            },
        );
        let memory = |_: &AssetId| None;
        let mut s = ActionService::new(&graph, &packages, 1, &memory);
        s.occupied.insert(AssetId::novo("p", "lib/from_other.txt"));
        let b = || Arc::from(&b"r"[..]);
        assert!(
            s.escrever(&input, b()).is_err(),
            "the source already exists in the graph"
        );
        assert!(
            s.escrever(&predicted, b()).is_err(),
            "predicted output of another phase"
        );
        assert!(
            s.escrever(&AssetId::novo("p", "lib/from_other.txt"), b())
                .is_err(),
            "output of another anchor"
        );
        assert!(
            s.escrever(&AssetId::novo("p", "lib/../outside.txt"), b())
                .is_err()
        );
        assert!(
            s.escrever(&AssetId::novo("q", "lib/x.txt"), b()).is_err(),
            "unknown package"
        );
        let new = AssetId::novo("p", "lib/a.draft.summary");
        s.escrever(&new, b()).unwrap();
        assert!(s.escrever(&new, b()).is_err(), "the same asset twice");
        assert_eq!(s.writes.keys().collect::<Vec<_>>(), vec![&new]);
    }

    #[test]
    fn unlisted_dependency_is_readable_through_the_package_filter() {
        let dir = tempfile::tempdir().unwrap();
        let packages = GrafoPacotes {
            nos: vec![
                No {
                    nome: "p".into(),
                    raiz: dir.path().join("p"),
                    tipo: TipoDependencia::Path,
                    e_raiz: true,
                    deps: vec![],
                },
                No {
                    nome: "q".into(),
                    raiz: dir.path().join("q"),
                    tipo: TipoDependencia::Hosted,
                    e_raiz: false,
                    deps: vec![],
                },
            ],
            raiz: 0,
            por_nome: [("p".into(), 0), ("q".into(), 1)].into(),
            lock: Default::default(),
            dir_raiz: dir.path().to_path_buf(),
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
        let mut graph = Grafo::default();
        graph
            .fontes
            .entry("p".into())
            .or_default()
            .insert("lib/a.dart".into());
        let lib = || vec![crate::glob::Glob::novo("lib/**").unwrap()];
        graph.externos.insert(
            "q".into(),
            crate::grafo::FiltroFontes {
                alvos: vec![crate::plano::Casador {
                    include: Some(lib()),
                    exclude: None,
                }],
                visiveis: Some(lib()),
            },
        );
        let input = AssetId::novo("p", "lib/a.dart");
        graph.acoes = vec![Acao {
            fase: 0,
            entrada: input.clone(),
            saidas: vec![AssetId::novo("p", "lib/a.g.dart")],
            pos: false,
        }];
        let memory = |_: &AssetId| None;
        let mut s = ActionService::new(&graph, &packages, 0, &memory);
        assert!(s.can_read(&AssetId::novo("q", "lib/q.dart")));
        assert_eq!(
            s.ler(&AssetId::novo("q", "lib/q.dart")).as_deref(),
            Some(&b"class Q {}"[..])
        );
        assert!(
            !s.can_read(&AssetId::novo("q", "tool/t.dart")),
            "outside a dependency's visible set"
        );
        assert!(s.ler(&AssetId::novo("q", "lib/missing.dart")).is_none());
        assert!(
            s.queries.iter().any(
                |(c, d)| matches!(c, Consulta::Arquivo(p) if p.ends_with("q.dart")) && d.is_some()
            ),
            "reading the dependency is a query of the action"
        );
    }

    #[test]
    fn buildstep_glob_revalidates_generated_output_in_memory() {
        let dir = tempfile::tempdir().unwrap();
        let query = Consulta::GlobAtivos {
            dir: dir.path().to_path_buf(),
            padrao: "lib/**".into(),
            candidatos: vec![("lib/a.g.dart".into(), true)],
        };
        let empty = |_: &Path| None;
        let full =
            |p: &Path| (p == dir.path().join("lib/a.g.dart")).then(|| Arc::from(&b"generated"[..]));
        assert_ne!(
            query_digest(&query, &crate::consulta::SemBanco, &empty),
            query_digest(&query, &crate::consulta::SemBanco, &full)
        );
        // An old fallback file on disk does not make the current round's
        // output present when the generator did not publish it in memory.
        std::fs::create_dir(dir.path().join("lib")).unwrap();
        std::fs::write(dir.path().join("lib/a.g.dart"), b"old").unwrap();
        assert_eq!(
            query_digest(&query, &crate::consulta::SemBanco, &empty),
            Some(digest_bytes(b""))
        );
    }
}
