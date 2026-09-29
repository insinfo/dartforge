//! O motor: plano + grafo + impressão digital por ação + agenda
//! determinística + executores, publicando numa `Geracao` em memória.
//!
//! * Fases em série; ações de uma fase em paralelo (`std::thread::scope`,
//!   índice atômico, até `trabalhadores`), resultados reunidos na ordem das
//!   ações — o resultado não depende do número de trabalhadores.
//! * Uma ação já executada só volta a executar se o digest de alguma consulta
//!   que ela fez mudou (revalidação), e uma saída igual à anterior não
//!   invalida ninguém (corte pela saída).
//! * Preguiça (D-B6): na demanda [`Demanda::Carregador`] só se calculam as
//!   saídas que o carregador pode importar (`.dart`) e as `build_to: source`;
//!   o resto (um `.css` servido, o `--comparar`) sai por
//!   [`Motor::materializar`] ou [`Demanda::Tudo`]. Com o executor Dart, as
//!   fases ocultas que ele executa também entram (um builder posterior pode
//!   lê-las); só as opcionais ficam preguiçosas.
use crate::consulta::{BancoSemantico, Consulta, Digest, digest_bytes};
use crate::executor::{
    AcaoNativa, CtxGerador, Disponibilidade, ExecutorDart, GeradorNativo, Indisponivel, PedidoAcao,
    PedidoExtensoes, PedidoNativo, ScriptDeBuilders, ServicoAcao, ServicoBuildStep,
    candidatos_do_glob, digest_de,
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
    /// Apoio desatualizado vira erro (D-B5, `dartforge build --estrito`).
    pub estrito: bool,
    /// Executa também os geradores nativos ainda não verificados, só para
    /// medir (`--comparar`).
    pub medir_nao_verificados: bool,
    /// Lê e grava o estado entre processos em
    /// `.dart_tool/dartforge/build/estado` (§4.1). Desligado por padrão: só
    /// quando o usuário pede (`dartforge build --estado`,
    /// `DARTFORGE_BUILD_ESTADO=1`), pela regra governante 6 do `PLANO.md`.
    pub persistir: bool,
    /// `--config <nome>`: `build.<nome>.yaml` no lugar do `build.yaml` da raiz
    /// (alvos, `global_options` e `triggers`).
    pub config: Option<String>,
    /// `--define <builder>=<opção>=<valor>`, por cima das `global_options`.
    pub definicoes: crate::linha_de_comando::Definicoes,
    /// `--build-filter`: só as ações com saída que algum filtro casa rodam
    /// de início; as outras, só se um passo que roda ler a saída delas
    /// (`shouldBuildForDirs` e a construção sob demanda do oficial).
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

/// Quem produziu uma saída.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origem {
    Nativo(&'static str),
    Dart,
    Apoio,
    Pendente,
    /// O builder (ou o executor, no meio da ação) falhou: sem saídas, e a
    /// geração inteira falha (DF-BUILD-005).
    Falha,
    /// Entrada gerada que não foi escrita (ou falhou): o oficial não executa
    /// o passo (`_runPostProcessAction` e `_matchingPrimaryInputs`,
    /// `wasOutput`).
    Omitida,
    /// `run_only_if_triggered: true` e nenhum trigger disparou
    /// ([`crate::gatilhos`]): o passo não roda, e nada é escrito.
    NaoDisparada,
}

/// Por que uma ação não saiu do executor Dart.
#[derive(Debug, Clone)]
enum FalhaDart {
    /// Não há executor (desligado, não sobe): vale a política de apoio.
    Indisponivel(String),
    /// O builder falhou, ou o executor caiu no meio da ação.
    Builder(String),
}

/// O que se sabe de uma ação executada.
#[derive(Debug, Clone)]
pub struct Registro {
    pub impressao: Digest,
    pub consultas: Vec<(Consulta, Option<Digest>)>,
    /// Saídas esperadas e o conteúdo (`None` = não escrita).
    pub saidas: Vec<(AssetId, Option<Arc<[u8]>>)>,
    pub origem: Origem,
    /// Motivo de pendência (ou de recusa do nativo, quando o apoio supriu).
    pub motivo: Option<String>,
    /// Medição de um nativo não verificado (só com `medir_nao_verificados`).
    pub medido: Vec<(AssetId, Arc<[u8]>)>,
    /// Numa ação nativa, as saídas que o nativo não escreve e vieram do
    /// apoio (o `.css.dart` que o ngdart oficial escreve ao lado do
    /// `.css.shim.dart`): contam como pendentes, não como iguais.
    pub do_apoio: Vec<AssetId>,
    /// Pós-processador: entradas marcadas por `deletePrimaryInput`.
    pub apagados: Vec<AssetId>,
    /// Veio do estado salvo por outro processo e ainda não foi conferido
    /// nesta sessão: a primeira verificação recalcula **todas** as
    /// consultas, não só as que os eventos tocam (§4.1).
    pub restaurado: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Demanda {
    /// O que o carregador pode importar (`.dart`) e as saídas `source`.
    Carregador,
    /// Tudo (`dartforge build`, `--comparar`).
    Tudo,
}

/// Relatório de uma atualização (vai para o `Relatorio` do `dev`).
#[derive(Debug, Clone, Default)]
pub struct RelMotor {
    pub acoes_verificadas: usize,
    pub acoes_executadas: usize,
    pub consultas_reavaliadas: usize,
    pub saidas_alteradas: usize,
    pub nativas: usize,
    pub dart: usize,
    /// Unidades efetivamente regeneradas dentro de um gerador por pacote.
    pub unidades_nativas: usize,
    /// Consultas registradas por geradores nesta atualização.
    pub consultas_gerador: usize,
    pub apoio: usize,
    /// Das ações de apoio, as que deram conteúdo lido do disco do
    /// `build_runner` (as outras: o oficial não escreveu nada ali).
    pub apoio_com_saida: usize,
    pub pendentes_por_motivo: BTreeMap<String, usize>,
    /// Passos com `run_only_if_triggered` que nenhum trigger disparou.
    pub nao_disparadas: usize,
    /// Ações cujo builder falhou nesta atualização: a geração falhou.
    pub falhas: Vec<String>,
    pub tempo: Duration,
    /// Revalidação das rodadas por pacote (digests das consultas afetadas).
    pub tempo_revalidar: Duration,
    /// Geradores nativos por pacote (o estágio A do ngdart).
    pub tempo_nativo: Duration,
}

impl RelMotor {
    /// Soma uma passada seguinte (a construção sob demanda do
    /// `--build-filter` repete a atualização).
    fn somar(&mut self, o: RelMotor) {
        self.acoes_verificadas += o.acoes_verificadas;
        self.acoes_executadas += o.acoes_executadas;
        self.consultas_reavaliadas += o.consultas_reavaliadas;
        self.nativas += o.nativas;
        self.dart += o.dart;
        self.unidades_nativas += o.unidades_nativas;
        self.consultas_gerador += o.consultas_gerador;
        self.apoio += o.apoio;
        self.apoio_com_saida += o.apoio_com_saida;
        for (m, n) in o.pendentes_por_motivo {
            *self.pendentes_por_motivo.entry(m).or_default() += n;
        }
        self.nao_disparadas += o.nao_disparadas;
        self.falhas.extend(o.falhas);
        self.tempo += o.tempo;
        self.tempo_revalidar += o.tempo_revalidar;
        self.tempo_nativo += o.tempo_nativo;
    }

    pub fn texto(&self) -> String {
        let mut s = format!(
            "motor: {} ações verificadas, {} executadas ({} nativas, {} Dart, {} apoio, {} unidades regeneradas), {} consultas reavaliadas, {} consultas do gerador, {} saídas alteradas, {:.1} ms (revalidar {:.1} ms, nativo por pacote {:.1} ms)",
            self.acoes_verificadas,
            self.acoes_executadas,
            self.nativas,
            self.dart,
            self.apoio,
            self.unidades_nativas,
            self.consultas_reavaliadas,
            self.consultas_gerador,
            self.saidas_alteradas,
            self.tempo.as_secs_f64() * 1000.0,
            self.tempo_revalidar.as_secs_f64() * 1000.0,
            self.tempo_nativo.as_secs_f64() * 1000.0,
        );
        if self.nao_disparadas > 0 {
            s.push_str(&format!("\n  não disparadas: {}", self.nao_disparadas));
        }
        for (m, n) in &self.pendentes_por_motivo {
            s.push_str(&format!("\n  pendentes ({n}): {m}"));
        }
        for f in &self.falhas {
            s.push_str(&format!("\n  falha: {f}"));
        }
        s
    }
}

pub struct Atualizacao {
    pub geracao: Arc<Geracao>,
    /// Caminhos naturais cujo conteúdo mudou (ou sumiu) nesta atualização.
    pub alterados: Vec<PathBuf>,
    pub rel: RelMotor,
    pub avisos: Vec<String>,
}

/// Contexto de uma atualização: o banco semântico e, na sessão, o programa
/// já carregado (o `BuildStep.resolver` sem carga extra).
pub struct Contexto<'a> {
    pub banco: &'a dyn BancoSemantico,
    pub programa: Option<(&'a Program, &'a Interner)>,
}

/// Execução de um gerador por pacote numa rodada.
struct RodadaPacote {
    registro_consultas: Vec<(Consulta, Option<Digest>)>,
    saidas: BTreeMap<PathBuf, Arc<[u8]>>,
    recusas: BTreeMap<PathBuf, String>,
    erro: Option<String>,
    unidades_geradas: usize,
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
    nativos: Vec<Arc<dyn GeradorNativo>>,
    dart: std::sync::Mutex<Box<dyn ExecutorDart>>,
    dart_preparado: AtomicBool,
    /// As extensões de execução já foram conferidas com o executor Dart
    /// ([`Motor::conferir_extensoes`]).
    extensoes_conferidas: bool,
    registros: Vec<Option<Registro>>,
    /// Ações por fase (índices em `grafo.acoes`).
    por_fase: Vec<Vec<usize>>,
    /// Rodadas dos geradores por pacote: (gerador, pacote).
    pacotes: HashMap<(usize, Arc<str>), RodadaPacote>,
    memoria: HashMap<PathBuf, Arc<[u8]>>,
    /// Saída → caminho natural (chave da geração).
    pub naturais: BTreeMap<AssetId, PathBuf>,
    /// Caminhos naturais das saídas `.dart` (o que o carregador pode pedir).
    esperadas: HashSet<PathBuf>,
    /// Caminhos naturais das fontes listadas (arquivo novo ou apagado refaz o
    /// grafo, como `graph.dart:343-356` reavalia os globs).
    fontes: HashSet<PathBuf>,
    /// Diretórios com fontes do pacote raiz: a marca de um diretório muda
    /// quando um arquivo entra ou sai dele.
    dirs: Vec<PathBuf>,
    /// Arquivos de configuração: mudou um, o plano é refeito.
    configuracao: Vec<PathBuf>,
    marcas: HashMap<PathBuf, Option<Marca>>,
    /// O que [`Motor::mudancas`] olha (recalculado a cada atualização).
    observados: Vec<PathBuf>,
    geracao: Arc<Geracao>,
    avisados: HashSet<PathBuf>,
    rotulos: HashMap<String, &'static str>,
    /// Saídas que a reconstrução do grafo tirou da memória (a ação ou a
    /// âncora deixou de existir): entram nos alterados da atualização, para
    /// que a geração seja republicada sem elas.
    retirados: Vec<PathBuf>,
    /// Estado salvo lido no início, aplicado na primeira atualização (depois
    /// que as extensões e as âncoras foram conferidas com o executor Dart).
    restaurar: Option<crate::persistencia::Estado>,
    /// O código dos builders Dart do estado restaurado, conferido: vale
    /// para regravar ações Dart restauradas sem que o executor tenha rodado.
    codigo_restaurado: Option<Vec<PathBuf>>,
    /// Algum registro mudou desde a última gravação do estado.
    estado_sujo: bool,
    /// Com `--build-filter`: as ações pedidas (as de saída filtrada, a cadeia
    /// das entradas geradas delas e as que um passo que rodou leu).
    pedidas: HashSet<usize>,
}

impl Drop for Motor {
    fn drop(&mut self) {
        if let Ok(mut dart) = self.dart.lock() {
            dart.encerrar();
        }
    }
}

fn natural(grafo: &GrafoPacotes, id: &AssetId) -> PathBuf {
    let raiz = grafo
        .no(&id.pacote)
        .map(|n| n.raiz.clone())
        .unwrap_or_default();
    chave(&raiz.join(id.caminho.as_ref()))
}

struct Indices {
    por_fase: Vec<Vec<usize>>,
    naturais: BTreeMap<AssetId, PathBuf>,
    esperadas: HashSet<PathBuf>,
    fontes: HashSet<PathBuf>,
    dirs: Vec<PathBuf>,
}

fn indices(gp: &GrafoPacotes, grafo: &Grafo, n_fases: usize) -> Indices {
    let mut por_fase = vec![Vec::new(); n_fases];
    for (i, a) in grafo.acoes.iter().enumerate() {
        if a.viva() {
            por_fase[a.fase].push(i);
        }
    }
    let mut naturais = BTreeMap::new();
    let mut esperadas = HashSet::new();
    for a in &grafo.acoes {
        for s in &a.saidas {
            let n = natural(gp, s);
            if s.caminho.ends_with(".dart") {
                esperadas.insert(n.clone());
            }
            naturais.insert(s.clone(), n);
        }
    }
    let mut fontes = HashSet::new();
    let mut dirs = BTreeSet::new();
    let raiz = &gp.nos[gp.raiz];
    for (pacote, caminhos) in &grafo.fontes {
        let Some(no) = gp.no(pacote) else { continue };
        for c in caminhos {
            if c.contains('$') {
                continue;
            }
            let n = chave(&no.raiz.join(c.as_ref()));
            if no.nome == raiz.nome {
                let mut d = n.parent();
                while let Some(x) = d {
                    if !x.starts_with(&raiz.raiz) || x == raiz.raiz {
                        break;
                    }
                    dirs.insert(x.to_path_buf());
                    d = x.parent();
                }
            }
            fontes.insert(n);
        }
    }
    Indices {
        por_fase,
        naturais,
        esperadas,
        fontes,
        dirs: dirs.into_iter().collect(),
    }
}

/// Forma canônica de um caminho de evento (o arquivo pode ter sido apagado:
/// então canoniza o diretório).
fn canonico(p: &Path) -> Option<PathBuf> {
    let c = match std::fs::canonicalize(p) {
        Ok(c) => c,
        Err(_) => std::fs::canonicalize(p.parent()?)
            .ok()?
            .join(p.file_name()?),
    };
    Some(chave(&dartforge_elements::config::sem_verbatim(c)))
}

/// A consulta pode ter mudado com estes eventos?
fn afetada(
    c: &Consulta,
    mudados: &HashSet<PathBuf>,
    dart_mudou: bool,
    estruturais: &HashSet<PathBuf>,
) -> bool {
    match c {
        // Uma listagem só muda quando entra ou sai arquivo (diretório observado
        // que mudou, ou arquivo novo/apagado).
        Consulta::Glob { dir, .. } => estruturais.iter().any(|m| m.starts_with(dir)),
        Consulta::GlobAtivos {
            dir, candidatos, ..
        } => {
            estruturais.iter().any(|m| m.starts_with(dir))
                || candidatos
                    .iter()
                    .any(|(rel, _)| mudados.contains(&chave(&dir.join(rel))))
        }
        _ => c.caminho().is_some_and(|p| mudados.contains(p)) || (c.semantica() && dart_mudou),
    }
}

impl Motor {
    /// Constrói o motor de um projeto: lê a configuração e monta plano, fases
    /// e grafo. Só chamado quando [`crate::detectar`] disse sim.
    pub fn novo(raiz: &Path, cfg: &PackageConfig, opcoes: OpcoesMotor) -> Result<Motor, String> {
        crate::contar_instancia();
        let grafo_pacotes = GrafoPacotes::montar(raiz, cfg, None)?;
        let configs_script = Configs::ler(&grafo_pacotes, false)?;
        let plano = Plano::do_script(&grafo_pacotes, &configs_script)?;
        let configs = Configs::ler_com(&grafo_pacotes, true, opcoes.config.as_deref())?;
        let (fases, alvos) = fases(
            &grafo_pacotes,
            &configs,
            &plano,
            opcoes.release,
            &opcoes.definicoes,
        )?;
        let grafo = Grafo::montar(&grafo_pacotes, &configs, &fases, &alvos)?;
        let Indices {
            por_fase,
            naturais,
            esperadas,
            fontes,
            dirs,
        } = indices(&grafo_pacotes, &grafo, fases.len());
        let mut configuracao = vec![
            chave(&grafo_pacotes.dir_raiz.join("pubspec.lock")),
            chave(
                &grafo_pacotes
                    .dir_raiz
                    .join(".dart_tool")
                    .join("package_config.json"),
            ),
        ];
        let r = &grafo_pacotes.nos[grafo_pacotes.raiz].raiz;
        configuracao.push(chave(&r.join("pubspec.yaml")));
        // O `build.yaml` de cada pacote, exista ou não (criá-lo muda o
        // plano), e o diretório raiz, cuja marca muda quando um override
        // `<pacote>.build.yaml` aparece ou some (DF-BUILD-018).
        for no in &grafo_pacotes.nos {
            configuracao.push(chave(&no.raiz.join("build.yaml")));
        }
        configuracao.push(chave(r));
        if let Some(k) = &opcoes.config {
            configuracao.push(chave(&r.join(format!("build.{k}.yaml"))));
        }
        if let Ok(ls) = std::fs::read_dir(r) {
            for e in ls.flatten() {
                if e.file_name().to_string_lossy().ends_with(".build.yaml") {
                    configuracao.push(chave(&e.path()));
                }
            }
        }
        configuracao.sort();
        configuracao.dedup();
        let n_acoes = grafo.acoes.len();
        let mut m = Motor {
            raiz: raiz.to_path_buf(),
            grafo_pacotes,
            configs,
            plano,
            fases,
            alvos,
            grafo,
            opcoes,
            nativos: Vec::new(),
            dart: std::sync::Mutex::new(Box::new(Indisponivel::default())),
            dart_preparado: AtomicBool::new(false),
            extensoes_conferidas: false,
            registros: vec![None; n_acoes],
            por_fase,
            pacotes: HashMap::new(),
            memoria: HashMap::new(),
            naturais,
            esperadas,
            fontes,
            dirs,
            configuracao,
            marcas: HashMap::new(),
            observados: Vec::new(),
            geracao: Arc::new(Geracao::default()),
            avisados: HashSet::new(),
            rotulos: HashMap::new(),
            retirados: Vec::new(),
            restaurar: None,
            codigo_restaurado: None,
            estado_sujo: false,
            pedidas: HashSet::new(),
        };
        m.pedir_pelos_filtros();
        for c in m.configuracao.iter().chain(&m.dirs) {
            m.marcas.insert(c.clone(), Marca::ler(c));
        }
        m.recalcular_observados();
        if m.opcoes.persistir {
            m.restaurar = crate::persistencia::ler(&crate::persistencia::diretorio(
                &m.grafo_pacotes.dir_raiz,
            ));
        }
        #[cfg(feature = "nativos")]
        {
            for g in crate::nativos::todos() {
                m.registrar_nativo(g);
            }
        }
        Ok(m)
    }

    pub fn registrar_nativo(&mut self, g: Arc<dyn GeradorNativo>) {
        self.nativos.push(g);
    }

    pub fn definir_executor_dart(&mut self, e: Box<dyn ExecutorDart>) {
        if let Ok(mut anterior) = self.dart.lock() {
            anterior.encerrar();
        }
        self.dart = std::sync::Mutex::new(e);
        self.dart_preparado.store(false, Ordering::Release);
    }

    pub fn geracao(&self) -> Arc<Geracao> {
        self.geracao.clone()
    }

    /// Caminhos naturais das saídas `.dart` esperadas: a primeira carga não
    /// pode abortar por "não foi possível ler" de uma delas.
    pub fn saidas_esperadas(&self) -> &HashSet<PathBuf> {
        &self.esperadas
    }

    pub fn registro(&self, acao: usize) -> Option<&Registro> {
        self.registros.get(acao).and_then(|r| r.as_ref())
    }

    /// Entradas que não são Dart e que o motor observa (o carregador já
    /// observa os `.dart`): os arquivos de configuração e o que as ações
    /// consultaram fora da memória.
    pub fn observados(&self) -> Vec<PathBuf> {
        self.observados.clone()
    }

    /// Recalcula a lista observada (depois de cada atualização) e anota a
    /// marca de quem entrou nela agora — o estado que as ações leram.
    fn recalcular_observados(&mut self) {
        let v = self.calcular_observados();
        for p in &v {
            if !self.marcas.contains_key(p) {
                self.marcas.insert(p.clone(), Marca::ler(p));
            }
        }
        self.observados = v;
    }

    fn calcular_observados(&self) -> Vec<PathBuf> {
        let mut v: BTreeSet<PathBuf> = self
            .configuracao
            .iter()
            .chain(&self.dirs)
            .cloned()
            .collect();
        let consultas = self
            .registros
            .iter()
            .flatten()
            .flat_map(|r| r.consultas.iter())
            .chain(
                self.pacotes
                    .values()
                    .flat_map(|p| p.registro_consultas.iter()),
            );
        for (c, _) in consultas {
            if let Consulta::Arquivo(p) = c {
                if !p.to_string_lossy().ends_with(".dart")
                    && !self.memoria.contains_key(p)
                    && !self.apoio_de(p)
                {
                    v.insert(p.clone());
                }
            }
        }
        v.into_iter().collect()
    }

    fn apoio_de(&self, p: &Path) -> bool {
        p.starts_with(self.grafo_pacotes.dir_raiz.join(".dart_tool"))
    }

    /// Observados cuja marca mudou desde a última olhada (mtime e tamanho).
    pub fn mudancas(&mut self) -> Vec<PathBuf> {
        // Um `stat` por observado, em paralelo como o `CacheUnidades`
        // (no Windows cada `stat` passa pelo antivírus).
        let caminhos = &self.observados;
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(8);
        let agora: Vec<Option<Marca>> = if caminhos.len() < 64 || threads < 2 {
            caminhos.iter().map(|p| Marca::ler(p)).collect()
        } else {
            let pedaco = caminhos.len().div_ceil(threads);
            std::thread::scope(|s| {
                let hs: Vec<_> = caminhos
                    .chunks(pedaco)
                    .map(|c| s.spawn(move || c.iter().map(|p| Marca::ler(p)).collect::<Vec<_>>()))
                    .collect();
                hs.into_iter()
                    .flat_map(|h| h.join().unwrap_or_default())
                    .collect()
            })
        };
        let mut v = Vec::new();
        for (p, m) in self.observados.iter().zip(agora) {
            match self.marcas.get(p) {
                Some(antes) if *antes == m => {}
                _ => {
                    v.push(p.clone());
                    self.marcas.insert(p.clone(), m);
                }
            }
        }
        v
    }

    fn rotulo(&mut self, s: &str) -> &'static str {
        if let Some(r) = self.rotulos.get(s) {
            return r;
        }
        let r: &'static str = Box::leak(s.to_string().into_boxed_str());
        self.rotulos.insert(s.to_string(), r);
        r
    }

    pub fn caminho_de_apoio(&self, id: &AssetId, oculto: bool) -> PathBuf {
        if oculto {
            self.grafo_pacotes
                .dir_raiz
                .join(".dart_tool/build/generated")
                .join(id.pacote.as_ref())
                .join(id.caminho.as_ref())
        } else {
            self.naturais.get(id).cloned().unwrap_or_default()
        }
    }

    /// A ação entra nesta atualização? `dart_ativo`: há executor Dart. Um
    /// builder Dart pode ler uma saída `cache` de fase anterior que não é
    /// `.dart` (o combining_builder lê as partes `.g.part` por glob) e o motor
    /// não interrompe uma ação para calcular outra; então, com o executor,
    /// as fases ocultas sem gerador nativo não são preguiçosas — só as
    /// opcionais, como no `build_runner` (`is_optional`).
    fn demandada(&self, a: usize, demanda: Demanda, dart_ativo: bool) -> bool {
        if self.registros[a].is_some() {
            return true;
        }
        if !self.opcoes.filtros.is_empty() {
            return self.pedidas.contains(&a);
        }
        if demanda == Demanda::Tudo {
            return true;
        }
        let fi = self.grafo.acoes[a].fase;
        let f = &self.fases[fi];
        !f.oculta
            || self.grafo.acoes[a]
                .saidas
                .iter()
                .any(|s| s.caminho.ends_with(".dart"))
            || (dart_ativo && !f.opcional && self.nativo_da_fase(fi).is_none())
    }

    /// As saídas que o builder `chave` (ex. `ngdart:ngdart`) não enxerga ao
    /// resolver o programa: as de todas as fases a partir da primeira dele,
    /// pelo caminho natural ([`dartforge_elements::gerado::chave`]). No
    /// `build_runner` uma ação só lê saídas de fases anteriores à dela; um
    /// `build_to: source` que já está no disco (o `messages.i18n.dart` do
    /// `i18n`, fase posterior ao ngdart no limitless_ui) não existe para o
    /// resolver do ngdart, e o tipo que viria dele é `dynamic`.
    ///
    /// O programa é um só para todos os pacotes; conta a primeira fase do
    /// builder, e uma saída de fase entre a do primeiro pacote e a de outro
    /// fica oculta também para o segundo.
    pub fn saidas_invisiveis_a(&self, chave: &str) -> HashSet<PathBuf> {
        let Some(primeira) = (0..self.fases.len())
            .find(|&fi| self.plano.aplicacoes[self.fases[fi].aplicacao].chave == chave)
        else {
            return HashSet::new();
        };
        self.grafo
            .gerados
            .iter()
            .filter(|(_, g)| g.fase >= primeira)
            .map(|(id, _)| dartforge_elements::gerado::chave(&natural(&self.grafo_pacotes, id)))
            .collect()
    }

    /// As entradas `.dart` das ações do builder `chave` (ex. `ngdart:ngdart`),
    /// pelo caminho natural: os arquivos que ele gera — alcançáveis ou não
    /// a partir do `main`. O `build_runner` resolve cada um; o programa dos
    /// geradores nativos precisa tê-los todos (um componente que ninguém
    /// importa também ganha `.template.dart`).
    pub fn entradas_de(&self, chave: &str) -> Vec<PathBuf> {
        let mut saida: Vec<PathBuf> = self
            .grafo
            .acoes
            .iter()
            .filter(|a| a.viva())
            .filter(|a| self.plano.aplicacoes[self.fases[a.fase].aplicacao].chave == chave)
            .filter(|a| a.entrada.caminho.ends_with(".dart"))
            .map(|a| natural(&self.grafo_pacotes, &a.entrada))
            .collect();
        saida.sort();
        saida.dedup();
        saida
    }

    /// O gerador nativo que cobre a fase, se a versão do lock é a imitada e o
    /// pacote é o publicado no pub.dev (um fork com a mesma versão pode ter
    /// outra fábrica: vai ao executor, DF-BUILD-022).
    fn nativo_da_fase(&self, fi: usize) -> Option<usize> {
        let f = &self.fases[fi];
        let chave = f
            .equivalente
            .unwrap_or(&self.plano.aplicacoes[f.aplicacao].chave);
        self.nativos.iter().position(|g| {
            g.chave() == chave
                && g.cobre(&f.fabrica)
                && (g.verificado() || self.opcoes.medir_nao_verificados)
                && crate::descritor::imita(chave).iter().all(|(p, vs)| {
                    self.grafo_pacotes
                        .lock
                        .get(*p)
                        .is_some_and(|t| t.do_pub_dev() && vs.contains(&t.versao.as_str()))
                })
        })
    }

    /// As versões do `pubspec.lock` dos pacotes que o gerador nativo `chave`
    /// imita (as que escolhem o modo dele; ver [`crate::descritor::imita`]).
    fn versoes_imitadas(&self, chave: &str) -> BTreeMap<String, String> {
        crate::descritor::imita(chave)
            .iter()
            .filter_map(|(p, _)| {
                let t = self.grafo_pacotes.lock.get(*p)?;
                Some(((*p).to_owned(), t.versao.clone()))
            })
            .collect()
    }

    /// A configuração mudou (pubspec, lock, build.yaml)? Então o plano é
    /// refeito do zero, como o `build_runner` faz quando o script muda.
    fn configuracao_mudou(&self, mudados: &HashSet<PathBuf>) -> bool {
        self.configuracao.iter().any(|c| mudados.contains(c))
            || mudados.iter().any(|m| {
                m.file_name()
                    .is_some_and(|n| n.to_string_lossy().ends_with("build.yaml"))
            })
    }

    /// Arquivo que entrou ou saiu (ou diretório observado que mudou): as
    /// fontes, e com elas as saídas esperadas, podem ter mudado.
    fn estrutural(&self, m: &Path) -> bool {
        if self.dirs.iter().any(|d| d == m) {
            return true;
        }
        if self.fontes.contains(m) {
            return !m.is_file();
        }
        // Arquivo novo num pacote listado (fora do `.dart_tool` e sem ser
        // saída conhecida escrita no disco).
        let listado = self.grafo.listados.iter().any(|p| {
            self.grafo_pacotes.no(p).is_some_and(|n| {
                m.starts_with(&n.raiz) && !m.starts_with(n.raiz.join(".dart_tool"))
            })
        });
        listado && m.is_file() && !self.naturais.values().any(|n| n == m)
    }

    /// Refaz a listagem de fontes e as saídas esperadas com o mesmo plano;
    /// os registros das ações que continuam existindo (mesma fase e mesma
    /// entrada) são mantidos.
    fn reconstruir_grafo(&mut self, mudados: &mut HashSet<PathBuf>) -> Result<(), String> {
        let grafo = Grafo::montar(&self.grafo_pacotes, &self.configs, &self.fases, &self.alvos)?;
        let mut antigos: HashMap<(usize, AssetId), Registro> = HashMap::new();
        for (i, r) in std::mem::take(&mut self.registros).into_iter().enumerate() {
            if let Some(r) = r {
                let a = &self.grafo.acoes[i];
                antigos.insert((a.fase, a.entrada.clone()), r);
            }
        }
        let Indices {
            por_fase,
            naturais,
            esperadas,
            fontes,
            dirs,
        } = indices(&self.grafo_pacotes, &grafo, self.fases.len());
        self.registros = grafo
            .acoes
            .iter()
            .map(|a| {
                if a.viva() {
                    antigos.remove(&(a.fase, a.entrada.clone()))
                } else {
                    None
                }
            })
            .collect();
        // Saídas que deixaram de existir saem da memória. As de um
        // pós-processador não estão no grafo: valem as das âncoras que
        // continuam existindo.
        self.estado_sujo = true;
        let dinamicas: Vec<PathBuf> = grafo
            .acoes
            .iter()
            .zip(&self.registros)
            .filter(|(a, _)| a.pos)
            .filter_map(|(_, r)| r.as_ref())
            .flat_map(|r| {
                r.saidas
                    .iter()
                    .map(|(s, _)| natural(&self.grafo_pacotes, s))
            })
            .collect();
        let validas: HashSet<&PathBuf> = naturais.values().chain(&dinamicas).collect();
        let sumiram: Vec<PathBuf> = self
            .memoria
            .keys()
            .filter(|k| !validas.contains(k))
            .cloned()
            .collect();
        for k in sumiram {
            self.memoria.remove(&k);
            self.retirados.push(k.clone());
            mudados.insert(k);
        }
        for d in &dirs {
            self.marcas
                .entry(d.clone())
                .or_insert_with(|| Marca::ler(d));
        }
        self.grafo = grafo;
        self.por_fase = por_fase;
        self.naturais = naturais;
        self.esperadas = esperadas;
        self.fontes = fontes;
        self.dirs = dirs;
        self.pedir_pelos_filtros();
        Ok(())
    }

    /// Recalcula o que for preciso e publica a geração.
    pub fn atualizar(
        &mut self,
        ctx: &Contexto<'_>,
        mudados: &[PathBuf],
        demanda: Demanda,
    ) -> Result<Atualizacao, String> {
        let mut at = self.atualizar_passo(ctx, mudados, demanda)?;
        // `--build-filter`: um passo que rodou leu a saída de uma ação que
        // não estava pedida — o oficial a constrói na hora (`_isReadableNode`
        // → `_runLazyPhaseForInput`). Aqui ela entra nas pedidas e a
        // atualização se repete: a saída nova acorda quem a leu.
        while !self.opcoes.filtros.is_empty() {
            let novas = self.pedidas_por_leitura();
            if novas.is_empty() {
                break;
            }
            self.pedidas.extend(novas);
            let mais = self.atualizar_passo(ctx, &[], demanda)?;
            let mut alterados: BTreeSet<PathBuf> = at.alterados.into_iter().collect();
            alterados.extend(mais.alterados);
            at.alterados = alterados.into_iter().collect();
            at.rel.somar(mais.rel);
            at.rel.saidas_alteradas = at.alterados.len();
            at.avisos.extend(mais.avisos);
            at.geracao = mais.geracao;
        }
        Ok(at)
    }

    /// As ações que o `--build-filter` pede de início
    /// (`_matchingPrimaryInputs` com `shouldBuildForDirs`): as de fase não
    /// opcional com alguma saída que um filtro casa e que é visível no build
    /// (fora da raiz, só os assets públicos), e, para cada uma, a ação que
    /// produz a entrada gerada dela. Âncoras de pós-processador rodam sempre.
    fn pedir_pelos_filtros(&mut self) {
        self.pedidas.clear();
        if self.opcoes.filtros.is_empty() {
            return;
        }
        let mut pilha = Vec::new();
        for (a, acao) in self.grafo.acoes.iter().enumerate() {
            if !acao.viva() {
                continue;
            }
            let fase = &self.fases[acao.fase];
            let pedida = acao.pos
                || !fase.opcional
                    && acao.saidas.iter().any(|s| {
                        self.opcoes.filtros.iter().any(|f| f.casa(s)) && self.visivel_no_build(s)
                    });
            if pedida {
                pilha.push(a);
            }
        }
        while let Some(a) = pilha.pop() {
            if !self.pedidas.insert(a) {
                continue;
            }
            if let Some(g) = self.grafo.gerados.get(&self.grafo.acoes[a].entrada) {
                pilha.push(g.acao);
            }
        }
    }

    /// `isVisibleInBuild`: na raiz tudo; fora dela, os assets públicos
    /// (`lib/**`, `bin/**`, … e `additional_public_assets`).
    fn visivel_no_build(&self, id: &AssetId) -> bool {
        let Some(&p) = self.grafo_pacotes.por_nome.get(id.pacote.as_ref()) else {
            return false;
        };
        if p == self.grafo_pacotes.raiz {
            return true;
        }
        crate::grafo::VISIVEIS_FORA_DA_RAIZ
            .iter()
            .map(|s| s.to_string())
            .chain(self.configs.por_pacote[p].publicos_adicionais.iter().cloned())
            .any(|g| crate::glob::Glob::novo(&g).is_ok_and(|g| g.casa(&id.caminho)))
    }

    /// As ações ainda não pedidas cuja saída um passo executado consultou:
    /// por caminho (`canRead`, leitura) ou como candidato de um glob
    /// (`findAssets`), de fase anterior à do leitor.
    fn pedidas_por_leitura(&self) -> Vec<usize> {
        let por_natural: HashMap<&Path, &AssetId> = self
            .naturais
            .iter()
            .map(|(id, n)| (n.as_path(), id))
            .collect();
        let mut novas = BTreeSet::new();
        for (a, r) in self.registros.iter().enumerate() {
            let Some(r) = r else { continue };
            let fase = self.grafo.acoes[a].fase;
            let mut pedir = |id: &AssetId| {
                if let Some(g) = self.grafo.gerados.get(id)
                    && g.fase < fase
                    && self.registros[g.acao].is_none()
                    && !self.pedidas.contains(&g.acao)
                {
                    novas.insert(g.acao);
                }
            };
            for (c, _) in &r.consultas {
                match c {
                    Consulta::GlobAtivos {
                        dir, candidatos, ..
                    } => {
                        for (rel, gerado) in candidatos {
                            if *gerado
                                && let Some(id) = por_natural.get(chave(&dir.join(rel)).as_path())
                            {
                                pedir(id);
                            }
                        }
                    }
                    _ => {
                        if let Some(id) = c.caminho().and_then(|p| por_natural.get(p)) {
                            pedir(id);
                        }
                    }
                }
            }
        }
        novas.into_iter().collect()
    }

    /// Uma passada do motor sobre as fases (ver [`Motor::atualizar`]).
    fn atualizar_passo(
        &mut self,
        ctx: &Contexto<'_>,
        mudados: &[PathBuf],
        demanda: Demanda,
    ) -> Result<Atualizacao, String> {
        let t0 = Instant::now();
        // Os caminhos do motor saem do `package_config.json` (canônicos); um
        // evento pode vir por outro nome do mesmo arquivo (nome curto 8.3 do
        // Windows, link): entra também a forma canônica.
        let mut mudados: HashSet<PathBuf> = {
            let mut m = HashSet::with_capacity(mudados.len() * 2);
            for p in mudados {
                m.insert(chave(p));
                if let Some(c) = canonico(p) {
                    m.insert(c);
                }
            }
            m
        };
        if self.configuracao_mudou(&mudados) {
            let antiga = std::mem::take(&mut self.memoria);
            let opcoes = self.opcoes.clone();
            let nativos = std::mem::take(&mut self.nativos);
            let dart = std::mem::replace(
                &mut self.dart,
                std::sync::Mutex::new(Box::new(Indisponivel::default())),
            );
            // O `package_config.json` relido do disco (uma dependência
            // `path` pode ter mudado de lugar ou de versão): o guardado é o
            // do começo da sessão. Ilegível agora, a atualização falha, como
            // num processo novo.
            let arquivo = self
                .grafo_pacotes
                .dir_raiz
                .join(".dart_tool")
                .join("package_config.json");
            let cfg = PackageConfig::load(&arquivo)?;
            *self = Motor::novo(&self.raiz.clone(), &cfg, opcoes)?;
            self.nativos = nativos;
            self.dart = dart;
            self.dart_preparado.store(false, Ordering::Release);
            // Tudo o que existia conta como possivelmente alterado.
            for k in antiga.keys() {
                mudados.insert(k.clone());
            }
            self.memoria = antiga;
        }
        if !self.extensoes_conferidas {
            self.conferir_extensoes(&mut mudados)?;
        }
        // Eventos que mudam listagens: diretório observado, arquivo novo ou
        // apagado. Também refazem as fontes e as saídas esperadas.
        let estruturais: HashSet<PathBuf> = mudados
            .iter()
            .filter(|m| self.estrutural(m))
            .cloned()
            .collect();
        if !estruturais.is_empty() {
            self.reconstruir_grafo(&mut mudados)?;
        }
        if self.dart_preparado.load(Ordering::Acquire)
            && let Ok(mut d) = self.dart.lock()
        {
            d.nova_rodada();
        }
        let mut rel = RelMotor::default();
        let mut avisos = Vec::new();
        let mut alterados: BTreeSet<PathBuf> = self.retirados.drain(..).collect();
        let dart_mudou = mudados
            .iter()
            .any(|p| p.to_string_lossy().ends_with(".dart"));
        let mut pacotes_feitos: HashSet<(usize, Arc<str>)> = HashSet::new();
        let mut pacotes_rodaram: HashSet<(usize, Arc<str>)> = HashSet::new();
        let motivo_dart = self
            .dart
            .lock()
            .map(|d| match d.disponibilidade() {
                Disponibilidade::Disponivel => None,
                Disponibilidade::Indisponivel(m) => Some(m),
            })
            .unwrap_or_else(|_| Some("executor Dart envenenado".into()));
        if self.restaurar.is_some() {
            self.aplicar_restauracao(motivo_dart.as_deref());
        }

        for fi in 0..self.fases.len() {
            let acoes: Vec<usize> = self.por_fase[fi]
                .iter()
                .copied()
                .filter(|&a| self.demandada(a, demanda, motivo_dart.is_none()))
                .collect();
            if acoes.is_empty() {
                continue;
            }
            let nativo = self.nativo_da_fase(fi);
            // Gerador por pacote: uma rodada por (gerador, pacote), feita (ou
            // reaproveitada) na primeira fase que ele cobre.
            let mut rodou: Option<bool> = None;
            if let Some(g) = nativo.filter(|&g| self.nativos[g].por_pacote()) {
                let pacote: Arc<str> = self.grafo_pacotes.nos[self.fases[fi].pacote]
                    .nome
                    .as_str()
                    .into();
                let k = (g, pacote.clone());
                if !pacotes_feitos.contains(&k) {
                    pacotes_feitos.insert(k.clone());
                    if self.rodar_pacote(
                        ctx,
                        g,
                        &pacote,
                        &mudados,
                        dart_mudou,
                        &estruturais,
                        &mut rel,
                    )? {
                        pacotes_rodaram.insert(k.clone());
                    }
                }
                rodou = Some(pacotes_rodaram.contains(&k));
            }
            // Quais ações precisam (re)executar.
            let mut executar = Vec::new();
            for &a in &acoes {
                rel.acoes_verificadas += 1;
                let sujo = match (&self.registros[a], rodou) {
                    (None, _) => true,
                    // Coberta por rodada de pacote: refaz se a rodada refez.
                    (Some(_), Some(r)) => r,
                    // Do estado salvo: todas as consultas, e as saídas
                    // `source` no disco.
                    (Some(r), None) if r.restaurado => {
                        rel.consultas_reavaliadas += r.consultas.len();
                        let (sujo, fora_do_disco) = self.conferir_restaurado(a, ctx.banco);
                        for n in fora_do_disco {
                            self.memoria.remove(&n);
                        }
                        if !sujo && let Some(r) = self.registros[a].as_mut() {
                            r.restaurado = false;
                        }
                        sujo
                    }
                    (Some(r), None) => {
                        // Só as consultas que os eventos podem ter mudado.
                        let memoria = |p: &Path| self.memoria.get(p).cloned();
                        let mut refeitas = Vec::new();
                        let sujo = r
                            .consultas
                            .iter()
                            .enumerate()
                            .filter(|(_, (c, _))| {
                                afetada(c, &mudados, dart_mudou, &estruturais)
                                    && !self.invisivel(a, c)
                            })
                            .any(|(i, (c, d))| {
                                rel.consultas_reavaliadas += 1;
                                if let Consulta::GlobAtivos { dir, padrao, .. } = c {
                                    // O grafo pode ter ganhado (ou perdido) um
                                    // candidato: a lista é refeita no grafo novo
                                    // e só a resposta diferente suja a ação.
                                    if estruturais.iter().any(|m| m.starts_with(dir)) {
                                        let Ok(g) = crate::glob::Glob::novo(padrao) else {
                                            return true;
                                        };
                                        let candidatos = candidatos_do_glob(&self.grafo, a, &g)
                                            .into_iter()
                                            .map(|id| {
                                                (
                                                    id.caminho.to_string(),
                                                    self.grafo.gerados.contains_key(&id),
                                                )
                                            })
                                            .collect();
                                        let refeita = Consulta::GlobAtivos {
                                            dir: dir.clone(),
                                            padrao: padrao.clone(),
                                            candidatos,
                                        };
                                        let mudou = digest_de(&refeita, ctx.banco, &memoria) != *d;
                                        refeitas.push((i, refeita));
                                        return mudou;
                                    }
                                }
                                digest_de(c, ctx.banco, &memoria) != *d
                            });
                        if !sujo && !refeitas.is_empty() {
                            // A resposta é a mesma, mas os candidatos são os
                            // do grafo novo: um evento futuro num deles tem
                            // de acordar a ação.
                            if let Some(r) = self.registros[a].as_mut() {
                                for (i, c) in refeitas {
                                    r.consultas[i].0 = c;
                                }
                            }
                        }
                        sujo
                    }
                };
                if sujo {
                    executar.push(a);
                }
            }
            if executar.is_empty() {
                continue;
            }
            // Passos que não rodam: entrada gerada que não foi escrita
            // (`wasOutput`) e, com `run_only_if_triggered`, os que nenhum
            // trigger dispara. Os disparados guardam as leituras do trigger
            // (a entrada e as partes), que decidem a próxima vez.
            let mut previos: HashMap<usize, Registro> = HashMap::new();
            let mut do_gatilho: HashMap<usize, Vec<(Consulta, Option<Digest>)>> = HashMap::new();
            if !self.fases[fi].pos {
                for &a in &executar {
                    if let Some(r) = self.entrada_nao_escrita(a) {
                        previos.insert(a, r);
                    } else if let Some((disparou, consultas)) = self.disparo(a) {
                        if disparou {
                            do_gatilho.insert(a, consultas);
                        } else {
                            previos.insert(a, self.sem_saidas(a, consultas, Origem::NaoDisparada));
                        }
                    }
                }
            }
            let rodar: Vec<usize> = executar
                .iter()
                .copied()
                .filter(|a| !previos.contains_key(a))
                .collect();
            let rodados: Vec<Registro> = match nativo {
                _ if rodar.is_empty() => Vec::new(),
                Some(g) if self.nativos[g].por_pacote() => rodar
                    .iter()
                    .map(|&a| self.de_pacote(g, a, motivo_dart.as_deref()))
                    .collect(),
                Some(g) => {
                    let mut v = Vec::with_capacity(rodar.len());
                    for &a in &rodar {
                        v.push(self.nativo_por_acao(ctx, &mudados, g, a, motivo_dart.as_deref()));
                    }
                    v
                }
                None if motivo_dart.is_none() && self.fases[fi].pos => self.pos_em_serie(&rodar),
                None if motivo_dart.is_none() => {
                    rodar.iter().map(|&a| self.dart_ou_apoio(a)).collect()
                }
                None => self.apoio_em_paralelo(&rodar, motivo_dart.as_deref()),
            };
            let mut rodados: HashMap<usize, Registro> = rodar.into_iter().zip(rodados).collect();
            let novos: Vec<Registro> = executar
                .iter()
                .map(|a| {
                    previos.remove(a).unwrap_or_else(|| {
                        let mut r = rodados.remove(a).expect("registro de ação rodada");
                        if let Some(c) = do_gatilho.remove(a) {
                            r.consultas.extend(c);
                        }
                        r
                    })
                })
                .collect();
            for (a, mut r) in executar.into_iter().zip(novos) {
                if !matches!(r.origem, Origem::Omitida | Origem::NaoDisparada) {
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
                    Origem::NaoDisparada => rel.nao_disparadas += 1,
                    Origem::Pendente | Origem::Omitida => {}
                }
                if let Some(m) = &r.motivo {
                    if r.origem == Origem::Pendente {
                        *rel.pendentes_por_motivo.entry(m.clone()).or_default() += 1;
                    }
                }
                // Apoio desatualizado (D-B5).
                if r.origem == Origem::Apoio {
                    let entrada = self.naturais_de_entrada(a);
                    for (s, _) in &r.saidas {
                        let loc = self.caminho_de_apoio(s, self.fases[fi].oculta);
                        if mais_nova(&entrada, &loc) && self.avisados.insert(loc.clone()) {
                            let msg = format!(
                                "{}: {} pode estar desatualizado — {}; rode 'dart run build_runner build'",
                                self.plano.aplicacoes[self.fases[fi].aplicacao].chave,
                                s.texto(),
                                r.motivo
                                    .as_deref()
                                    .unwrap_or("executor Dart não produziu saída atual")
                            );
                            if self.opcoes.estrito {
                                return Err(msg);
                            }
                            avisos.push(msg);
                        }
                    }
                }
                // Saídas de um pós-processador que a nova execução não
                // escreveu somem (`_cleanUpStaleOutputs` da âncora).
                if let Some(antigo) = &self.registros[a]
                    && self.grafo.acoes[a].pos
                {
                    for (s, _) in &antigo.saidas {
                        if !r.saidas.iter().any(|(x, _)| x == s) {
                            let n = self.natural_de(s);
                            if self.memoria.remove(&n).is_some() {
                                alterados.insert(n.clone());
                                mudados.insert(n);
                            }
                        }
                    }
                }
                // `deletePrimaryInput` (`deletedBy`): o `FinalizedReader` do
                // oficial — o `serve` e o diretório mesclado — deixa de
                // enxergar a entrada; os builders continuam lendo. Numa
                // entrada gerada o efeito é o mesmo aqui: ela sai da geração
                // publicada (`publicar`). Uma fonte apagada continuaria no
                // disco servido pelo DartForge: aviso, e erro no estrito.
                let antes: Vec<AssetId> = self.registros[a]
                    .as_ref()
                    .map(|x| x.apagados.clone())
                    .unwrap_or_default();
                if antes != r.apagados {
                    for id in antes.iter().chain(&r.apagados) {
                        if self.grafo.gerados.contains_key(id) {
                            let n = self.natural_de(id);
                            alterados.insert(n);
                        }
                    }
                }
                for apagado in r
                    .apagados
                    .iter()
                    .filter(|x| !self.grafo.gerados.contains_key(*x))
                {
                    let msg = format!(
                        "{}: a fonte {} foi marcada por deletePrimaryInput; o DartForge não tem o diretório mesclado (`build -o`) nem esconde fontes no `serve`, então ela continua visível",
                        self.plano.aplicacoes[self.fases[fi].aplicacao].chave,
                        apagado.texto()
                    );
                    if self.opcoes.estrito {
                        return Err(msg);
                    }
                    avisos.push(msg);
                }
                // Corte pela saída: só o que mudou de conteúdo segue adiante.
                for (s, c) in &r.saidas {
                    let n = self.natural_de(s);
                    let antes = self.memoria.get(&n).cloned();
                    let igual = match (&antes, c) {
                        (Some(x), Some(y)) => x == y,
                        (None, None) => true,
                        _ => false,
                    };
                    if !igual {
                        alterados.insert(n.clone());
                        mudados.insert(n.clone());
                        match c {
                            Some(c) => {
                                self.memoria.insert(n, c.clone());
                            }
                            None => {
                                self.memoria.remove(&n);
                            }
                        }
                    }
                }
                let impressao = self.impressao(a, &r.consultas);
                r.impressao = impressao;
                self.registros[a] = Some(r);
                self.estado_sujo = true;
            }
        }
        rel.saidas_alteradas = alterados.len();
        if !alterados.is_empty() || self.geracao.vazia() {
            self.publicar();
        }
        if self.opcoes.persistir && self.estado_sujo {
            match self.salvar() {
                Ok(()) => self.estado_sujo = false,
                Err(e) => avisos.push(format!("estado do motor não gravado: {e}")),
            }
        }
        rel.tempo = t0.elapsed();
        self.recalcular_observados();
        Ok(Atualizacao {
            geracao: self.geracao.clone(),
            alterados: alterados.into_iter().collect(),
            rel,
            avisos,
        })
    }

    /// As extensões de execução vêm do objeto `Builder`, não do `build.yaml`
    /// (`expected_outputs.dart`). Com o executor Dart disponível, o motor
    /// pergunta a cada fase que nenhum gerador nativo verificado cobre quais
    /// são (`build.extensoes`) e, se alguma diverge do descritor ou do
    /// `build.yaml` — uma fábrica de um builder com várias, extensões que
    /// dependem das opções —, refaz o grafo com as do `Builder`. Uma vez por
    /// plano; sem executor, ficam as previstas.
    fn conferir_extensoes(&mut self, mudados: &mut HashSet<PathBuf>) -> Result<(), String> {
        self.extensoes_conferidas = true;
        let candidatas: Vec<usize> = (0..self.fases.len())
            .filter(|&fi| self.fases[fi].extensoes.is_some() && self.nativo_da_fase(fi).is_none())
            .collect();
        let pos: Vec<usize> = (0..self.fases.len())
            .filter(|&fi| self.fases[fi].pos && self.fases[fi].substituido.is_none())
            .collect();
        let disponivel = self
            .dart
            .lock()
            .is_ok_and(|d| d.disponibilidade() == Disponibilidade::Disponivel);
        if (candidatas.is_empty() && pos.is_empty())
            || !disponivel
            || self.preparar_dart().is_some()
        {
            return Ok(());
        }
        let mut mudou = false;
        // Pós-processadores: as `inputExtensions` do objeto decidem as
        // âncoras (`_actionMatches` de uma `PostBuildAction`).
        for fi in pos {
            let f = &self.fases[fi];
            let chave = self.plano.aplicacoes[f.aplicacao].chave.clone();
            let pedido = PedidoExtensoes {
                chave,
                fabrica: f.fabrica.clone(),
                opcoes: f.opcoes.clone(),
                raiz: f.raiz,
            };
            let resposta = self
                .dart
                .lock()
                .map_err(|_| "executor Dart envenenado")?
                .entradas_pos(&pedido);
            let Ok(Some(v)) = resposta else { continue };
            if self.fases[fi].entradas_pos.as_ref() != Some(&v) {
                self.fases[fi].entradas_pos = Some(v);
                mudou = true;
            }
        }
        for fi in candidatas {
            let f = &self.fases[fi];
            let chave = self.plano.aplicacoes[f.aplicacao].chave.clone();
            let pedido = PedidoExtensoes {
                chave: chave.clone(),
                fabrica: f.fabrica.clone(),
                opcoes: f.opcoes.clone(),
                raiz: f.raiz,
            };
            let resposta = self
                .dart
                .lock()
                .map_err(|_| "executor Dart envenenado")?
                .extensoes(&pedido);
            // Sem resposta (fábrica que falha ao instanciar, executor que não
            // sabe): ficam as previstas, e a ação dirá o erro ao executar.
            let Ok(Some(v)) = resposta else { continue };
            if self.fases[fi]
                .extensoes
                .as_ref()
                .is_some_and(|e| e.declaradas != v)
            {
                self.fases[fi].extensoes = Some(crate::extensoes::Extensoes::novas(&v, &chave)?);
                mudou = true;
            }
        }
        if mudou {
            self.reconstruir_grafo(mudados)?;
        }
        Ok(())
    }

    /// A consulta é de arquivo sobre uma saída que a ação não enxerga pela
    /// fase — de fase posterior, ou da mesma fase e de outra ação, ou a
    /// própria saída (`build_impl.dart:443-463`)? Então a resposta que a ação
    /// recebeu não depende do conteúdo (é "ilegível", ou o que ela mesma
    /// escreveu) e não muda enquanto o grafo não muda; o digest do disco ou
    /// da memória não a representa.
    fn invisivel(&self, a: usize, c: &Consulta) -> bool {
        let (Consulta::Arquivo(p) | Consulta::Existe(p)) = c else {
            return false;
        };
        let fase = self.grafo.acoes[a].fase;
        let Some(no) = self
            .grafo_pacotes
            .nos
            .iter()
            .filter(|n| !n.raiz.as_os_str().is_empty() && p.starts_with(&n.raiz))
            .max_by_key(|n| n.raiz.as_os_str().len())
        else {
            return false;
        };
        let Ok(rel) = p.strip_prefix(&no.raiz) else {
            return false;
        };
        let caminho = rel.to_string_lossy().replace('\\', "/");
        self.grafo
            .gerados
            .get(&AssetId::novo(&no.nome, &caminho))
            .is_some_and(|g| g.fase >= fase)
    }

    /// Chave global do estado salvo: o que nenhuma consulta vê — versão do
    /// motor, o executável (código dos geradores nativos), o plano, o lock e
    /// o modo.
    fn chave_do_estado(&self) -> String {
        let mut h = blake3::Hasher::new();
        h.update(crate::VERSAO.as_bytes());
        h.update(crate::persistencia::identidade_do_executavel().as_bytes());
        h.update(self.plano.texto_canonico().as_bytes());
        let mut lock: Vec<_> = self.grafo_pacotes.lock.iter().collect();
        lock.sort_by_key(|(nome, _)| *nome);
        for (nome, t) in lock {
            h.update(format!("{nome} {} {:?}\n", t.versao, t.tipo).as_bytes());
        }
        h.update(&[u8::from(self.opcoes.release)]);
        // `--config` e os `triggers` acumulados decidem o que roda sem
        // aparecer em consulta nenhuma (o `buildTriggersDigest` do oficial).
        h.update(format!("{:?}", self.opcoes.config).as_bytes());
        h.update(format!("{:?}", self.configs.gatilhos.por_builder).as_bytes());
        h.finalize().to_hex().to_string()
    }

    /// Chave de uma ação no estado salvo: a fase (com as extensões e as
    /// entradas de pós-processador já conferidas), as opções, a entrada e as
    /// saídas previstas.
    fn chave_da_acao(&self, a: usize) -> String {
        let acao = &self.grafo.acoes[a];
        let f = &self.fases[acao.fase];
        let mut h = blake3::Hasher::new();
        h.update(f.identidade(&self.plano, &self.grafo_pacotes).as_bytes());
        h.update(f.opcoes.texto_canonico().as_bytes());
        h.update(&[u8::from(f.raiz), u8::from(acao.pos)]);
        h.update(acao.entrada.texto().as_bytes());
        for s in &acao.saidas {
            h.update(b"\n");
            h.update(s.texto().as_bytes());
        }
        h.finalize().to_hex().to_string()
    }

    /// O código dos builders Dart (arquivos do depfile do bootstrap) fora
    /// dos pacotes `hosted`, que o lock já fixa: arquivo e digest do
    /// conteúdo. `None` se não se sabe (o executor não compilou o bootstrap
    /// nesta sessão e nada foi restaurado) ou se um deles sumiu.
    fn codigo_dart(&self) -> Option<Vec<(PathBuf, Digest)>> {
        let lista = self
            .dart
            .lock()
            .ok()
            .and_then(|d| d.codigo())
            .or_else(|| self.codigo_restaurado.clone())?;
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
        for p in lista {
            if hosted.iter().any(|h| p.starts_with(h)) {
                continue;
            }
            v.push((p.clone(), crate::consulta::digest_arquivo(&p)?));
        }
        v.sort();
        Some(v)
    }

    /// O registro pode ir para o estado salvo (§4.1)?
    fn persistivel(&self, a: usize, r: &Registro) -> bool {
        let fi = self.grafo.acoes[a].fase;
        !matches!(r.origem, Origem::Pendente)
            && r.medido.is_empty()
            && self
                .nativo_da_fase(fi)
                .is_none_or(|g| !self.nativos[g].por_pacote())
            && r.consultas
                .iter()
                .all(|(c, _)| crate::persistencia::persistivel(c))
    }

    /// Grava o estado: as ações persistíveis e o conteúdo das saídas.
    fn salvar(&mut self) -> Result<(), String> {
        let tem_dart = self
            .registros
            .iter()
            .flatten()
            .any(|r| r.origem == Origem::Dart);
        let codigo_dart = if tem_dart { self.codigo_dart() } else { None };
        let mut acoes = Vec::new();
        let mut conteudos = Vec::new();
        for (a, r) in self.registros.iter().enumerate() {
            let Some(r) = r else { continue };
            if !self.persistivel(a, r) {
                continue;
            }
            let origem = match r.origem {
                Origem::Nativo(n) => crate::persistencia::OrigemSalva::Nativo(n.to_string()),
                Origem::Dart if codigo_dart.is_some() => crate::persistencia::OrigemSalva::Dart,
                Origem::Apoio => crate::persistencia::OrigemSalva::Apoio,
                Origem::Omitida if self.grafo.acoes[a].pos => {
                    crate::persistencia::OrigemSalva::Omitida
                }
                Origem::Dart
                | Origem::Pendente
                | Origem::Falha
                | Origem::Omitida
                | Origem::NaoDisparada => continue,
            };
            let saidas = r
                .saidas
                .iter()
                .map(|(id, c)| {
                    let d = c.as_ref().map(|c| {
                        let d = digest_bytes(c);
                        conteudos.push((d, c.clone()));
                        d
                    });
                    (id.clone(), d)
                })
                .collect();
            acoes.push(crate::persistencia::AcaoSalva {
                chave: self.chave_da_acao(a),
                origem,
                motivo: r.motivo.clone(),
                consultas: r.consultas.clone(),
                saidas,
                do_apoio: r.do_apoio.clone(),
                apagados: r.apagados.clone(),
            });
        }
        let estado = crate::persistencia::Estado {
            chave: self.chave_do_estado(),
            codigo_dart,
            acoes,
        };
        crate::persistencia::gravar(
            &crate::persistencia::diretorio(&self.grafo_pacotes.dir_raiz),
            &estado,
            &conteudos,
        )
    }

    /// Aplica o estado lido no início: cada ação salva cuja chave ainda
    /// existe e cuja origem continua sendo a que este motor escolheria volta
    /// como registro `restaurado`, com as saídas na memória. A validade é
    /// conferida na primeira verificação de cada uma
    /// ([`Motor::conferir_restaurado`]).
    fn aplicar_restauracao(&mut self, motivo_dart: Option<&str>) {
        let Some(estado) = self.restaurar.take() else {
            return;
        };
        if estado.chave != self.chave_do_estado() {
            return;
        }
        let dir = crate::persistencia::diretorio(&self.grafo_pacotes.dir_raiz);
        let dart_valido = estado.codigo_dart.as_ref().is_some_and(|l| {
            l.iter()
                .all(|(p, d)| crate::consulta::digest_arquivo(p).as_ref() == Some(d))
        });
        if dart_valido {
            self.codigo_restaurado = estado
                .codigo_dart
                .as_ref()
                .map(|l| l.iter().map(|(p, _)| p.clone()).collect());
        }
        let por_chave: HashMap<String, usize> = (0..self.grafo.acoes.len())
            .filter(|&a| self.grafo.acoes[a].viva() && self.registros[a].is_none())
            .map(|a| (self.chave_da_acao(a), a))
            .collect();
        for s in estado.acoes {
            let Some(&a) = por_chave.get(&s.chave) else {
                continue;
            };
            let fi = self.grafo.acoes[a].fase;
            let nativo = self.nativo_da_fase(fi);
            let origem = match &s.origem {
                crate::persistencia::OrigemSalva::Nativo(n) => match nativo {
                    Some(g) if self.nativos[g].chave() == n && !self.nativos[g].por_pacote() => {
                        Origem::Nativo(self.nativos[g].chave())
                    }
                    _ => continue,
                },
                crate::persistencia::OrigemSalva::Dart if dart_valido && nativo.is_none() => {
                    Origem::Dart
                }
                crate::persistencia::OrigemSalva::Apoio
                    if nativo.is_none() && motivo_dart.is_some() =>
                {
                    Origem::Apoio
                }
                crate::persistencia::OrigemSalva::Omitida if self.grafo.acoes[a].pos => {
                    Origem::Omitida
                }
                _ => continue,
            };
            let mut saidas = Vec::with_capacity(s.saidas.len());
            for (id, d) in &s.saidas {
                match d {
                    None => saidas.push((id.clone(), None)),
                    Some(d) => match crate::persistencia::ler_blob(&dir, d) {
                        Some(b) => saidas.push((id.clone(), Some(b))),
                        None => break,
                    },
                }
            }
            if saidas.len() != s.saidas.len() {
                continue;
            }
            for (id, c) in &saidas {
                if let Some(c) = c {
                    let n = self.natural_de(id);
                    self.memoria.insert(n, c.clone());
                }
            }
            let impressao = self.impressao(a, &s.consultas);
            self.registros[a] = Some(Registro {
                impressao,
                consultas: s.consultas,
                saidas,
                origem,
                motivo: s.motivo,
                medido: Vec::new(),
                do_apoio: s.do_apoio,
                apagados: s.apagados,
                restaurado: true,
            });
        }
    }

    /// Confere um registro restaurado: todas as consultas (um `GlobAtivos`
    /// com os candidatos do grafo atual) e, numa saída `source` de gerador
    /// nativo ou do executor Dart, o arquivo no disco, que a CLI só regrava
    /// quando o conteúdo muda. Devolve se está sujo e os caminhos de saída
    /// que divergem do disco (a memória os esquece, para que a reexecução os
    /// conte como alterados e os regrave).
    fn conferir_restaurado(&self, a: usize, banco: &dyn BancoSemantico) -> (bool, Vec<PathBuf>) {
        let Some(r) = self.registros[a].as_ref() else {
            return (true, Vec::new());
        };
        let memoria = |p: &Path| self.memoria.get(p).cloned();
        let sujo = r
            .consultas
            .iter()
            .filter(|(c, _)| !self.invisivel(a, c))
            .any(|(c, d)| {
                let atual = match c {
                    Consulta::GlobAtivos { dir, padrao, .. } => {
                        let Ok(g) = crate::glob::Glob::novo(padrao) else {
                            return true;
                        };
                        let candidatos = candidatos_do_glob(&self.grafo, a, &g)
                            .into_iter()
                            .map(|id| {
                                (id.caminho.to_string(), self.grafo.gerados.contains_key(&id))
                            })
                            .collect();
                        digest_de(
                            &Consulta::GlobAtivos {
                                dir: dir.clone(),
                                padrao: padrao.clone(),
                                candidatos,
                            },
                            banco,
                            &memoria,
                        )
                    }
                    _ => digest_de(c, banco, &memoria),
                };
                atual != *d
            });
        let mut fora_do_disco = Vec::new();
        let f = &self.fases[self.grafo.acoes[a].fase];
        if !f.oculta && matches!(r.origem, Origem::Nativo(_) | Origem::Dart) {
            for (s, c) in &r.saidas {
                let n = self.natural_de(s);
                if let Some(c) = c
                    && std::fs::read(&n).ok().as_deref() != Some(&c[..])
                {
                    fora_do_disco.push(n);
                }
            }
        }
        (sujo || !fora_do_disco.is_empty(), fora_do_disco)
    }

    /// Caminho natural de uma saída: a prevista pelo grafo ou, numa saída de
    /// pós-processador, o caminho do asset no pacote.
    fn natural_de(&self, s: &AssetId) -> PathBuf {
        self.naturais
            .get(s)
            .cloned()
            .unwrap_or_else(|| natural(&self.grafo_pacotes, s))
    }

    /// As âncoras de pós-processamento de uma fase, em série e na ordem do
    /// grafo: cada uma enxerga, como "já existentes", as saídas das outras
    /// (o `addAsset` do oficial recusa um asset que o grafo já tem).
    fn pos_em_serie(&self, executar: &[usize]) -> Vec<Registro> {
        let mut donos: BTreeMap<AssetId, usize> = BTreeMap::new();
        for (a, (acao, r)) in self.grafo.acoes.iter().zip(&self.registros).enumerate() {
            if let (true, Some(r)) = (acao.pos, r) {
                for (s, _) in &r.saidas {
                    donos.insert(s.clone(), a);
                }
            }
        }
        let mut v = Vec::with_capacity(executar.len());
        for &a in executar {
            let ocupadas = donos
                .iter()
                .filter(|(_, d)| **d != a)
                .map(|(s, _)| s.clone())
                .collect();
            let r = self.pos_por_acao(a, ocupadas).unwrap_or_else(|e| Registro {
                impressao: [0; 32],
                consultas: Vec::new(),
                saidas: Vec::new(),
                origem: Origem::Pendente,
                motivo: Some(e),
                medido: Vec::new(),
                do_apoio: Vec::new(),
                apagados: Vec::new(),
                restaurado: false,
            });
            donos.retain(|_, d| *d != a);
            for (s, _) in &r.saidas {
                donos.insert(s.clone(), a);
            }
            v.push(r);
        }
        v
    }

    /// `_runPostProcessBuilderForAnchor`: uma entrada gerada só é processada
    /// se foi escrita (`wasOutput`, sem falha); então o pós-processador roda
    /// pelo executor Dart com o `PostProcessBuildStep` servido pelo motor.
    fn pos_por_acao(&self, a: usize, ocupadas: BTreeSet<AssetId>) -> Result<Registro, String> {
        let acao = self.grafo.acoes.get(a).ok_or("âncora inexistente")?;
        if self.grafo.gerados.contains_key(&acao.entrada) {
            let n = self.natural_de(&acao.entrada);
            if !self.memoria.contains_key(&n) {
                // A consulta negativa acorda a âncora quando a entrada for
                // escrita.
                return Ok(Registro {
                    impressao: [0; 32],
                    consultas: vec![(Consulta::Arquivo(n), None)],
                    saidas: Vec::new(),
                    origem: Origem::Omitida,
                    motivo: None,
                    medido: Vec::new(),
                    do_apoio: Vec::new(),
                    apagados: Vec::new(),
                    restaurado: false,
                });
            }
        }
        if let Some(e) = self.preparar_dart() {
            return Err(e);
        }
        let fase = &self.fases[acao.fase];
        let memoria = |id: &AssetId| {
            self.naturais
                .get(id)
                .and_then(|p| self.memoria.get(p))
                .cloned()
        };
        let mut servico = ServicoAcao::novo(&self.grafo, &self.grafo_pacotes, a, &memoria);
        servico.ocupadas = ocupadas;
        let pedido = PedidoAcao {
            fase: acao.fase,
            chave: self.plano.aplicacoes[fase.aplicacao].chave.clone(),
            fabrica: fase.fabrica.clone(),
            opcoes: fase.opcoes.clone(),
            raiz: fase.raiz,
            entrada: acao.entrada.clone(),
            saidas_permitidas: Vec::new(),
        };
        let mut dart = self.dart.lock().map_err(|_| "executor Dart envenenado")?;
        let resultado = dart.pos_processar(&pedido, &mut servico).map_err(|e| e.0)?;
        if resultado.falhou {
            let detalhes = resultado
                .logs
                .iter()
                .filter(|(nivel, _)| nivel == "severo" || nivel == "erro")
                .map(|(_, mensagem)| mensagem.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(if detalhes.is_empty() {
                format!("{}: pós-processador Dart falhou", pedido.chave)
            } else {
                format!("{}: {detalhes}", pedido.chave)
            });
        }
        for (id, bytes) in resultado.saidas {
            if servico
                .escritas
                .get(&id)
                .is_some_and(|b| b.as_ref() != bytes.as_ref())
            {
                return Err(format!(
                    "pós-processador Dart retornou bytes diferentes para {}",
                    id.texto()
                ));
            }
            if !servico.escritas.contains_key(&id) {
                servico.escrever(&id, bytes).map_err(|e| {
                    format!(
                        "pós-processador escreveu asset já existente: {}",
                        e.0.texto()
                    )
                })?;
            }
        }
        if let Some(x) = resultado.apagados.iter().find(|x| **x != acao.entrada) {
            return Err(format!(
                "pós-processador apagou {}, que não é a entrada primária",
                x.texto()
            ));
        }
        let saidas = servico
            .escritas
            .into_iter()
            .map(|(id, b)| (id, Some(b)))
            .collect();
        // A âncora depende do digest da entrada primária mesmo que o
        // pós-processador não a leia (`_postProcessBuildShouldRun` compara o
        // `previousInputsDigest`): o `part_cleanup` só a apaga.
        let n = self.natural_de(&acao.entrada);
        let d = self
            .memoria
            .get(&n)
            .map(|b| digest_bytes(b))
            .or_else(|| crate::consulta::digest_arquivo(&n));
        let mut consultas = vec![(Consulta::Arquivo(n), d)];
        consultas.extend(servico.consultas);
        Ok(Registro {
            impressao: [0; 32],
            consultas,
            saidas,
            origem: Origem::Dart,
            motivo: None,
            medido: Vec::new(),
            do_apoio: Vec::new(),
            apagados: resultado.apagados,
            restaurado: false,
        })
    }

    /// Registro de um passo que não roda: nenhuma saída escrita, só as
    /// consultas que decidiram isso.
    fn sem_saidas(
        &self,
        a: usize,
        consultas: Vec<(Consulta, Option<Digest>)>,
        origem: Origem,
    ) -> Registro {
        Registro {
            impressao: [0; 32],
            consultas,
            saidas: self.grafo.acoes[a]
                .saidas
                .iter()
                .map(|s| (s.clone(), None))
                .collect(),
            origem,
            motivo: None,
            medido: Vec::new(),
            do_apoio: Vec::new(),
            apagados: Vec::new(),
            restaurado: false,
        }
    }

    /// A entrada primária é saída de uma fase anterior que não foi escrita
    /// (ou falhou): o passo não roda (`_matchingPrimaryInputs`: `if
    /// (!input.wasOutput) return`; no 2.16.1, `skipMissingPrimaryInput`). A
    /// consulta negativa o acorda quando a entrada for escrita.
    fn entrada_nao_escrita(&self, a: usize) -> Option<Registro> {
        let entrada = &self.grafo.acoes[a].entrada;
        let g = self.grafo.gerados.get(entrada)?;
        // Só quando quem produz a entrada rodou de fato: sem executor (a
        // ação pendente, ou lida do apoio) não se sabe o que o builder
        // escreveria, e o passo segue a política de apoio.
        let conhecida = self.registros[g.acao].as_ref().is_some_and(|r| {
            matches!(
                r.origem,
                Origem::Dart
                    | Origem::Nativo(_)
                    | Origem::Falha
                    | Origem::NaoDisparada
                    | Origem::Omitida
            )
        });
        if !conhecida {
            return None;
        }
        let n = self.natural_de(entrada);
        if self.memoria.contains_key(&n) {
            return None;
        }
        Some(self.sem_saidas(a, vec![(Consulta::Arquivo(n), None)], Origem::Omitida))
    }

    /// `_allowedByTriggers`: `None` se a fase não tem `run_only_if_triggered:
    /// true` (ou o perfil não tem triggers); senão, se algum trigger do
    /// builder dispara, com as leituras que a decisão fez (a entrada e, para
    /// trigger de anotação, as partes legíveis pela fase — inclusive as
    /// geradas por fases anteriores).
    fn disparo(&self, a: usize) -> Option<(bool, Vec<(Consulta, Option<Digest>)>)> {
        let acao = &self.grafo.acoes[a];
        let fase = &self.fases[acao.fase];
        if !fase.so_se_disparada {
            return None;
        }
        let chave = &self.plano.aplicacoes[fase.aplicacao].chave;
        let abreviado = self.grafo_pacotes.perfil().trigger_pelo_nome_abreviado();
        let Some(gatilhos) = self.configs.gatilhos.do_builder(chave, abreviado) else {
            return Some((false, Vec::new()));
        };
        let memoria = |id: &AssetId| {
            self.naturais
                .get(id)
                .and_then(|p| self.memoria.get(p))
                .cloned()
        };
        let mut servico = ServicoAcao::novo(&self.grafo, &self.grafo_pacotes, a, &memoria);
        let primaria = servico
            .ler(&acao.entrada)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default();
        let disparou = crate::gatilhos::disparado(gatilhos, &primaria, |uri| {
            let id = resolver_uri(uri, &acao.entrada)?;
            if !servico.can_read(&id) {
                return None;
            }
            servico
                .ler(&id)
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        });
        Some((disparou, servico.consultas))
    }

    fn naturais_de_entrada(&self, a: usize) -> PathBuf {
        natural(&self.grafo_pacotes, &self.grafo.acoes[a].entrada)
    }

    fn preparar_dart(&self) -> Option<String> {
        let Ok(mut dart) = self.dart.lock() else {
            return Some("executor Dart envenenado".into());
        };
        match dart.disponibilidade() {
            Disponibilidade::Indisponivel(m) => return Some(m),
            Disponibilidade::Disponivel if self.dart_preparado.load(Ordering::Acquire) => {
                return None;
            }
            Disponibilidade::Disponivel => {}
        }
        // Só o que pode virar ação Dart (builders e pós-processadores): os
        // substituídos nunca executam (§6 do BUILD-MOTOR.md); importá-los só
        // aumentaria a compilação do script.
        let aplicacoes = self
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
        lock.sort_by_key(|(nome, _)| *nome);
        for (nome, pacote) in lock {
            h.update(nome.as_bytes());
            h.update(pacote.versao.as_bytes());
            h.update(format!("{:?}", pacote.tipo).as_bytes());
        }
        let script = ScriptDeBuilders {
            aplicacoes,
            chave_de_cache: h.finalize().to_hex().to_string(),
        };
        match dart.preparar(&script) {
            Ok(()) => {
                self.dart_preparado.store(true, Ordering::Release);
                None
            }
            Err(e) => Some(e.0),
        }
    }

    /// A ação pelo executor Dart; sem executor, o apoio. A falha do builder
    /// (exceção, erro severo, saída não permitida) ou do executor no meio da
    /// ação é falha da geração, nunca o resultado antigo do `build_runner`
    /// (DF-BUILD-005).
    fn dart_ou_apoio(&self, a: usize) -> Registro {
        match self.dart_por_acao(a) {
            Ok(r) => r,
            Err(FalhaDart::Builder(e)) => self.falha(a, e),
            Err(FalhaDart::Indisponivel(e)) => self.apoio(a, Some(&e)),
        }
    }

    /// O registro de uma ação que falhou: nenhuma saída (a antiga, do disco
    /// ou da memória, não vale), a falha como motivo.
    fn falha(&self, a: usize, e: String) -> Registro {
        let acao = &self.grafo.acoes[a];
        let entrada = natural(&self.grafo_pacotes, &acao.entrada);
        let d = self
            .memoria
            .get(&entrada)
            .map(|b| digest_bytes(b))
            .or_else(|| crate::consulta::digest_arquivo(&entrada));
        Registro {
            impressao: [0; 32],
            consultas: vec![(Consulta::Arquivo(entrada), d)],
            saidas: acao.saidas.iter().map(|s| (s.clone(), None)).collect(),
            origem: Origem::Falha,
            motivo: Some(e),
            medido: Vec::new(),
            do_apoio: Vec::new(),
            apagados: Vec::new(),
            restaurado: false,
        }
    }

    fn dart_por_acao(&self, a: usize) -> Result<Registro, FalhaDart> {
        if let Some(e) = self.preparar_dart() {
            return Err(FalhaDart::Indisponivel(e));
        }
        let acao = self
            .grafo
            .acoes
            .get(a)
            .ok_or(FalhaDart::Indisponivel("ação Dart inexistente".into()))?;
        let fase = &self.fases[acao.fase];
        let memoria = |id: &AssetId| {
            self.naturais
                .get(id)
                .and_then(|p| self.memoria.get(p))
                .cloned()
        };
        let mut servico = ServicoAcao::novo(&self.grafo, &self.grafo_pacotes, a, &memoria);
        let pedido = PedidoAcao {
            fase: acao.fase,
            chave: self.plano.aplicacoes[fase.aplicacao].chave.clone(),
            fabrica: fase.fabrica.clone(),
            opcoes: fase.opcoes.clone(),
            raiz: fase.raiz,
            entrada: acao.entrada.clone(),
            saidas_permitidas: acao.saidas.clone(),
        };
        let mut dart = self
            .dart
            .lock()
            .map_err(|_| FalhaDart::Indisponivel("executor Dart envenenado".into()))?;
        // O executor caiu no meio da ação: falha, não ausência de executor.
        let resultado = dart.executar(&pedido, &mut servico).map_err(|e| {
            FalhaDart::Builder(format!("{}: falha do executor: {}", pedido.chave, e.0))
        })?;
        if resultado.falhou {
            let detalhes = resultado
                .logs
                .iter()
                .filter(|(nivel, _)| nivel == "severo" || nivel == "erro")
                .map(|(_, mensagem)| mensagem.as_str())
                .chain(
                    servico
                        .logs
                        .iter()
                        .filter(|(nivel, _)| *nivel == crate::executor::Nivel::Severo)
                        .map(|(_, mensagem)| mensagem.as_str()),
                )
                .collect::<Vec<_>>()
                .join("; ");
            return Err(FalhaDart::Builder(if detalhes.is_empty() {
                format!("{}: builder Dart falhou", pedido.chave)
            } else {
                format!("{}: {detalhes}", pedido.chave)
            }));
        }
        for (id, bytes) in resultado.saidas {
            if let Some(ja_escritos) = servico.escritas.get(&id) {
                if ja_escritos.as_ref() != bytes.as_ref() {
                    return Err(FalhaDart::Builder(format!(
                        "builder Dart retornou bytes diferentes para {} após escrever via BuildStep",
                        id.texto()
                    )));
                }
            }
            servico.escrever(&id, bytes).map_err(|e| {
                FalhaDart::Builder(format!(
                    "builder Dart escreveu saída não permitida: {}",
                    e.0.texto()
                ))
            })?;
        }
        let saidas = acao
            .saidas
            .iter()
            .map(|id| (id.clone(), servico.escritas.get(id).cloned()))
            .collect();
        Ok(Registro {
            impressao: [0; 32],
            consultas: servico.consultas,
            saidas,
            origem: Origem::Dart,
            motivo: None,
            medido: Vec::new(),
            do_apoio: Vec::new(),
            apagados: Vec::new(),
            restaurado: false,
        })
    }

    /// `blake3(versão ‖ chave ‖ fábrica ‖ imita ‖ opções ‖ identidade ‖
    /// entrada ‖ consultas)`.
    fn impressao(&self, a: usize, consultas: &[(Consulta, Option<Digest>)]) -> Digest {
        let acao = &self.grafo.acoes[a];
        let f = &self.fases[acao.fase];
        let ap = &self.plano.aplicacoes[f.aplicacao];
        let mut h = blake3::Hasher::new();
        h.update(crate::VERSAO.as_bytes());
        h.update(ap.chave.as_bytes());
        h.update(f.fabrica.as_bytes());
        for (p, v) in self.versoes_imitadas(&ap.chave) {
            h.update(p.as_bytes());
            h.update(v.as_bytes());
        }
        h.update(f.opcoes.texto_canonico().as_bytes());
        h.update(&[u8::from(f.raiz)]);
        h.update(f.identidade(&self.plano, &self.grafo_pacotes).as_bytes());
        h.update(acao.entrada.texto().as_bytes());
        for (c, d) in consultas {
            h.update(format!("{c:?}").as_bytes());
            h.update(d.as_ref().map(|d| &d[..]).unwrap_or(b"-"));
        }
        *h.finalize().as_bytes()
    }

    /// Roda (ou reaproveita) o gerador por pacote `g` sobre `pacote`.
    /// Devolve se rodou de fato.
    fn rodar_pacote(
        &mut self,
        ctx: &Contexto<'_>,
        g: usize,
        pacote: &Arc<str>,
        mudados: &HashSet<PathBuf>,
        dart_mudou: bool,
        estruturais: &HashSet<PathBuf>,
        rel: &mut RelMotor,
    ) -> Result<bool, String> {
        let k = (g, pacote.clone());
        if let Some(r) = self.pacotes.get(&k) {
            let talvez = r
                .registro_consultas
                .iter()
                .any(|(c, _)| afetada(c, mudados, dart_mudou, estruturais));
            if !talvez {
                return Ok(false);
            }
            // Só as consultas que os eventos podem ter mudado são refeitas.
            let t = Instant::now();
            let memoria = |p: &Path| self.memoria.get(p).cloned();
            let mut iguais = true;
            for (c, d) in r
                .registro_consultas
                .iter()
                .filter(|(c, _)| afetada(c, mudados, dart_mudou, estruturais))
            {
                rel.consultas_reavaliadas += 1;
                if digest_de(c, ctx.banco, &memoria) != *d {
                    iguais = false;
                    break;
                }
            }
            rel.tempo_revalidar += t.elapsed();
            if iguais {
                return Ok(false);
            }
        }
        let t_gerar = Instant::now();
        let gerador = self.nativos[g].clone();
        // Todas as ações das fases que o gerador cobre no pacote.
        let mut acoes = Vec::new();
        for (fi, f) in self.fases.iter().enumerate() {
            if self.grafo_pacotes.nos[f.pacote].nome != **pacote
                || self.nativo_da_fase(fi) != Some(g)
            {
                continue;
            }
            for &a in &self.por_fase[fi] {
                let acao = &self.grafo.acoes[a];
                acoes.push(AcaoNativa {
                    fabrica: f.fabrica.clone(),
                    entrada: acao.entrada.clone(),
                    entrada_natural: natural(&self.grafo_pacotes, &acao.entrada),
                    saidas: acao
                        .saidas
                        .iter()
                        .map(|s| (s.clone(), self.naturais[s].clone()))
                        .collect(),
                    opcoes: f.opcoes.clone(),
                });
            }
        }
        let no = self.grafo_pacotes.no(pacote).ok_or("pacote desconhecido")?;
        let pedido = PedidoNativo {
            pacote: pacote.to_string(),
            raiz_do_pacote: no.raiz.clone(),
            acoes,
            raizes: self.raizes_dos_pacotes(),
            versoes: self.versoes_imitadas(gerador.chave()),
        };
        let (resultado, consultas) = {
            let memoria = |p: &Path| self.memoria.get(p).cloned();
            let mut c = CtxGerador::novo(ctx.programa, mudados, ctx.banco, &memoria);
            let r = gerador.gerar(&mut c, &pedido);
            (r, std::mem::take(&mut c.consultas))
        };
        rel.consultas_gerador += consultas.len();
        let rodada = match resultado {
            Ok(s) => RodadaPacote {
                registro_consultas: if s.reutilizar_consultas {
                    let anterior = self
                        .pacotes
                        .get(&k)
                        .ok_or("ngdart: consultas anteriores ausentes")?;
                    let mut mantidas = anterior.registro_consultas.clone();
                    for (nova, digest) in consultas {
                        if let Some((_, antigo)) = mantidas.iter_mut().find(|(c, _)| *c == nova) {
                            *antigo = digest;
                        } else {
                            mantidas.push((nova, digest));
                        }
                    }
                    mantidas
                } else {
                    consultas
                },
                saidas: s
                    .saidas
                    .into_iter()
                    .map(|(p, b)| (chave(&p), Arc::from(b)))
                    .collect(),
                recusas: s.recusas.into_iter().map(|(p, m)| (chave(&p), m)).collect(),
                erro: None,
                unidades_geradas: s.unidades_geradas,
            },
            Err(e) => RodadaPacote {
                registro_consultas: consultas,
                saidas: BTreeMap::new(),
                recusas: BTreeMap::new(),
                erro: Some(e),
                unidades_geradas: 0,
            },
        };
        rel.unidades_nativas += rodada.unidades_geradas;
        self.pacotes.insert(k, rodada);
        rel.tempo_nativo += t_gerar.elapsed();
        Ok(true)
    }

    /// Registro de uma ação coberta por um gerador por pacote: as saídas
    /// geradas vêm da rodada; o que ele recusou vai para o apoio.
    fn de_pacote(&self, g: usize, a: usize, motivo_dart: Option<&str>) -> Registro {
        let acao = &self.grafo.acoes[a];
        let pacote: Arc<str> = self.grafo_pacotes.nos[self.fases[acao.fase].pacote]
            .nome
            .as_str()
            .into();
        let rodada = self.pacotes.get(&(g, pacote));
        let gerador = &self.nativos[g];
        let saidas: Vec<(AssetId, Option<Arc<[u8]>>)> = acao
            .saidas
            .iter()
            .map(|s| {
                (
                    s.clone(),
                    rodada
                        .and_then(|r| r.saidas.get(&self.naturais[s]))
                        .cloned(),
                )
            })
            .collect();
        let entrada = natural(&self.grafo_pacotes, &acao.entrada);
        // As consultas ficam na rodada do pacote (é ela que se revalida);
        // copiá-las para cada ação custava centenas de ms por edição.
        let consultas = Vec::new();
        // Nativo cobre a ação quando gerou alguma saída dela e não a recusou.
        let recusa =
            rodada.and_then(|r| r.erro.clone().or_else(|| r.recusas.get(&entrada).cloned()));
        let gerou_algo = saidas.iter().any(|(_, c)| c.is_some());
        if gerador.verificado() && recusa.is_none() && gerou_algo {
            if saidas.iter().any(|(_, c)| c.is_none()) && motivo_dart.is_none() {
                match self.dart_por_acao(a) {
                    Ok(r) => return r,
                    Err(FalhaDart::Builder(e)) => return self.falha(a, e),
                    Err(FalhaDart::Indisponivel(_)) => {}
                }
            }
            // Saída que o nativo não escreve e o oficial sim (o `.css.dart`
            // ao lado do `.css.shim.dart`) vem do apoio e é marcada como tal.
            let mut saidas = saidas;
            let mut do_apoio = Vec::new();
            let oculta = self.fases[acao.fase].oculta;
            for (s, c) in saidas.iter_mut() {
                if c.is_none() {
                    if let Ok(b) = std::fs::read(self.caminho_de_apoio(s, oculta)) {
                        *c = Some(Arc::from(b));
                        do_apoio.push(s.clone());
                    }
                }
            }
            return Registro {
                impressao: [0; 32],
                consultas,
                saidas,
                origem: Origem::Nativo(gerador.chave()),
                motivo: None,
                medido: Vec::new(),
                do_apoio,
                apagados: Vec::new(),
                restaurado: false,
            };
        }
        // O apoio de uma ação que já vinha dele não é relido: o disco do
        // `build_runner` não muda sob a sessão (limitação declarada: rodá-lo
        // à parte pede reiniciar a sessão).
        if motivo_dart.is_none() {
            match self.dart_por_acao(a) {
                Ok(r) => return r,
                Err(FalhaDart::Builder(e)) => return self.falha(a, e),
                Err(FalhaDart::Indisponivel(_)) => {}
            }
        }
        let mut r = match &self.registros[a] {
            Some(ant) if ant.origem == Origem::Apoio => {
                let mut r = ant.clone();
                r.motivo = motivo_dart.map(|m| format!("apoio do build_runner: {m}"));
                r
            }
            _ => self.apoio(a, motivo_dart),
        };
        if !gerador.verificado() {
            r.medido = saidas
                .into_iter()
                .filter_map(|(s, c)| c.map(|c| (s, c)))
                .collect();
        }
        let motivo_nativo =
            recusa.unwrap_or_else(|| format!("{}: não gera esta saída", gerador.chave()));
        r.motivo = Some(match r.motivo.take() {
            Some(m) => format!("{motivo_nativo}; {m}"),
            None => motivo_nativo,
        });
        r
    }

    /// Nome → raiz de cada pacote do grafo.
    fn raizes_dos_pacotes(&self) -> BTreeMap<String, PathBuf> {
        self.grafo_pacotes
            .nos
            .iter()
            .map(|n| (n.nome.clone(), n.raiz.clone()))
            .collect()
    }

    fn nativo_por_acao(
        &self,
        ctx: &Contexto<'_>,
        mudados: &HashSet<PathBuf>,
        g: usize,
        a: usize,
        motivo_dart: Option<&str>,
    ) -> Registro {
        let acao = &self.grafo.acoes[a];
        let f = &self.fases[acao.fase];
        let gerador = self.nativos[g].clone();
        let pedido = PedidoNativo {
            pacote: acao.entrada.pacote.to_string(),
            raiz_do_pacote: self.grafo_pacotes.nos[f.pacote].raiz.clone(),
            acoes: vec![AcaoNativa {
                fabrica: f.fabrica.clone(),
                entrada: acao.entrada.clone(),
                entrada_natural: natural(&self.grafo_pacotes, &acao.entrada),
                saidas: acao
                    .saidas
                    .iter()
                    .map(|s| (s.clone(), self.naturais[s].clone()))
                    .collect(),
                opcoes: f.opcoes.clone(),
            }],
            raizes: self.raizes_dos_pacotes(),
            versoes: self.versoes_imitadas(gerador.chave()),
        };
        let memoria = |p: &Path| self.memoria.get(p).cloned();
        let mut c = CtxGerador::novo(ctx.programa, mudados, ctx.banco, &memoria);
        let res = gerador.gerar(&mut c, &pedido);
        let consultas = std::mem::take(&mut c.consultas);
        match res {
            Ok(s)
                if gerador.verificado()
                    && s.recusas.is_empty()
                    && !s.saidas.is_empty()
                    && (motivo_dart.is_some()
                        || acao
                            .saidas
                            .iter()
                            .all(|id| s.saidas.contains_key(&self.naturais[id]))) =>
            {
                Registro {
                    impressao: [0; 32],
                    consultas,
                    saidas: acao
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
                    origem: Origem::Nativo(gerador.chave()),
                    motivo: None,
                    medido: Vec::new(),
                    do_apoio: Vec::new(),
                    apagados: Vec::new(),
                    restaurado: false,
                }
            }
            Ok(s) => {
                if motivo_dart.is_none() {
                    match self.dart_por_acao(a) {
                        Ok(r) => return r,
                        Err(FalhaDart::Builder(e)) => return self.falha(a, e),
                        Err(FalhaDart::Indisponivel(_)) => {}
                    }
                }
                let mut r = self.apoio(a, motivo_dart);
                r.medido = acao
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
                        "{}: saída ainda não verificada byte a byte",
                        gerador.chave()
                    )
                });
                r.motivo = Some(match r.motivo.take() {
                    Some(x) => format!("{m}; {x}"),
                    None => m,
                });
                r
            }
            Err(m) => {
                if motivo_dart.is_none() {
                    match self.dart_por_acao(a) {
                        Ok(r) => return r,
                        Err(FalhaDart::Builder(e)) => return self.falha(a, e),
                        Err(FalhaDart::Indisponivel(_)) => {}
                    }
                }
                let mut r = self.apoio(a, motivo_dart);
                r.motivo = Some(match r.motivo.take() {
                    Some(x) => format!("{m}; {x}"),
                    None => m,
                });
                r
            }
        }
    }

    fn apoio_em_paralelo(&self, acoes: &[usize], motivo_dart: Option<&str>) -> Vec<Registro> {
        let n = self.opcoes.trabalhadores.max(1).min(acoes.len());
        if n <= 1 || acoes.len() < 8 {
            return acoes.iter().map(|&a| self.apoio(a, motivo_dart)).collect();
        }
        let proximo = AtomicUsize::new(0);
        let mut todos: Vec<(usize, Registro)> = std::thread::scope(|s| {
            let hs: Vec<_> = (0..n)
                .map(|_| {
                    s.spawn(|| {
                        let mut meus = Vec::new();
                        loop {
                            let i = proximo.fetch_add(1, Ordering::Relaxed);
                            let Some(&a) = acoes.get(i) else { break };
                            meus.push((i, self.apoio(a, motivo_dart)));
                        }
                        meus
                    })
                })
                .collect();
            hs.into_iter()
                .flat_map(|h| h.join().unwrap_or_default())
                .collect()
        });
        todos.sort_by_key(|(i, _)| *i);
        todos.into_iter().map(|(_, r)| r).collect()
    }

    /// Executor de apoio: o que o `build_runner` deixou no disco.
    fn apoio(&self, a: usize, motivo_dart: Option<&str>) -> Registro {
        let acao = &self.grafo.acoes[a];
        let f = &self.fases[acao.fase];
        let entrada = natural(&self.grafo_pacotes, &acao.entrada);
        let memoria = |p: &Path| self.memoria.get(p).cloned();
        let mut consultas = Vec::with_capacity(1 + acao.saidas.len());
        let d = memoria(&entrada)
            .map(|b| digest_bytes(&b))
            .or_else(|| crate::consulta::digest_arquivo(&entrada));
        consultas.push((Consulta::Arquivo(entrada), d));
        let mut saidas = Vec::with_capacity(acao.saidas.len());
        for s in &acao.saidas {
            let loc = self.caminho_de_apoio(s, f.oculta);
            let c: Option<Arc<[u8]>> = std::fs::read(&loc).ok().map(Arc::from);
            consultas.push((
                Consulta::Arquivo(chave(&loc)),
                c.as_deref().map(digest_bytes),
            ));
            saidas.push((s.clone(), c));
        }
        let tem_build = self
            .grafo_pacotes
            .dir_raiz
            .join(".dart_tool/build")
            .is_dir();
        let algum = saidas.iter().any(|(_, c)| c.is_some());
        if !algum && !tem_build {
            let m = format!(
                "sem apoio: {} (não há .dart_tool/build; rode 'dart run build_runner build')",
                motivo_dart.unwrap_or("sem executor")
            );
            return Registro {
                impressao: [0; 32],
                consultas,
                saidas,
                origem: Origem::Pendente,
                motivo: Some(m),
                medido: Vec::new(),
                do_apoio: Vec::new(),
                apagados: Vec::new(),
                restaurado: false,
            };
        }
        Registro {
            impressao: [0; 32],
            consultas,
            saidas,
            origem: Origem::Apoio,
            motivo: motivo_dart.map(|m| format!("apoio do build_runner: {m}")),
            medido: Vec::new(),
            do_apoio: Vec::new(),
            apagados: Vec::new(),
            restaurado: false,
        }
    }

    /// Publica a geração: saídas com conteúdo UTF-8 que o carregador lê da
    /// memória — as `cache`, e as `source` de gerador nativo (as de apoio já
    /// estão no disco, no caminho natural).
    fn publicar(&mut self) {
        // Saídas marcadas por `deletePrimaryInput`: fora da geração, como o
        // `FinalizedReader` do oficial (`finalized_reader.dart:44`).
        let apagados: HashSet<&AssetId> = self
            .registros
            .iter()
            .flatten()
            .flat_map(|r| r.apagados.iter())
            .collect();
        let apagados: HashSet<AssetId> = apagados.into_iter().cloned().collect();
        let mut c = Construtor::nova();
        let mut h = blake3::Hasher::new();
        let mut itens: Vec<(PathBuf, Arc<[u8]>, &'static str, PathBuf)> = Vec::new();
        let mut rotulos: Vec<(usize, String)> = Vec::new();
        for (a, r) in self.registros.iter().enumerate() {
            let Some(r) = r else { continue };
            let acao = &self.grafo.acoes[a];
            let f = &self.fases[acao.fase];
            if !f.oculta && !matches!(r.origem, Origem::Nativo(_) | Origem::Dart) {
                continue;
            }
            for (s, conteudo) in &r.saidas {
                let Some(conteudo) = conteudo else { continue };
                if apagados.contains(s) {
                    continue;
                }
                let rotulo = match r.origem {
                    Origem::Nativo(_) | Origem::Dart => {
                        self.plano.aplicacoes[f.aplicacao].chave.clone()
                    }
                    _ => "build_runner".to_string(),
                };
                rotulos.push((a, rotulo));
                itens.push((
                    self.natural_de(s),
                    conteudo.clone(),
                    "",
                    natural(&self.grafo_pacotes, &acao.entrada),
                ));
            }
        }
        let rotulos: Vec<&'static str> =
            rotulos.into_iter().map(|(_, k)| self.rotulo(&k)).collect();
        for (i, (n, conteudo, _, entrada)) in itens.into_iter().enumerate() {
            let Ok(texto) = std::str::from_utf8(&conteudo) else {
                continue;
            };
            h.update(n.to_string_lossy().as_bytes());
            h.update(&digest_bytes(&conteudo));
            c.por(n, texto, rotulos[i], vec![entrada]);
        }
        let id = u64::from_le_bytes(h.finalize().as_bytes()[..8].try_into().unwrap_or([0; 8]));
        match c.concluir(id) {
            Ok(g) => self.geracao = g,
            Err(e) => {
                for m in e {
                    eprintln!("erro do motor de build: {m}");
                }
            }
        }
    }

    /// Conteúdo de uma saída pelo caminho natural, calculando-a se ainda não
    /// foi (a demanda de um `.css` pedido pelo navegador).
    pub fn materializar(&mut self, ctx: &Contexto<'_>, caminho: &Path) -> Option<Arc<[u8]>> {
        let mut k = chave(caminho);
        if let Some(c) = self.memoria.get(&k) {
            return Some(c.clone());
        }
        // `package_config.json` usa a raiz canônica; a URL servida pode
        // chegar pela grafia lexical (alias 8.3 ou link no Windows). O
        // arquivo gerado ainda não existe, então canonizamos só o pai.
        if !self.naturais.values().any(|n| n == &k) {
            if let (Some(pai), Some(nome)) = (caminho.parent(), caminho.file_name()) {
                if let Ok(pai) = std::fs::canonicalize(pai) {
                    k = chave(&dartforge_elements::config::sem_verbatim(pai).join(nome));
                }
            }
        }
        if let Some(c) = self.memoria.get(&k) {
            return Some(c.clone());
        }
        let id = self
            .naturais
            .iter()
            .find(|(_, n)| **n == k)
            .map(|(id, _)| id.clone())?;
        let g = self.grafo.gerados.get(&id)?.clone();
        if self.registros[g.acao]
            .as_ref()
            .is_some_and(|r| r.restaurado)
        {
            let (sujo, fora_do_disco) = self.conferir_restaurado(g.acao, ctx.banco);
            for n in fora_do_disco {
                self.memoria.remove(&n);
            }
            match self.registros[g.acao].as_mut() {
                Some(r) if !sujo => r.restaurado = false,
                _ => self.registros[g.acao] = None,
            }
        }
        if self.registros[g.acao].is_none() {
            let a = g.acao;
            let fi = self.grafo.acoes[a].fase;
            let motivo_dart = self
                .dart
                .lock()
                .map(|d| match d.disponibilidade() {
                    Disponibilidade::Disponivel => None,
                    Disponibilidade::Indisponivel(m) => Some(m),
                })
                .unwrap_or_else(|_| Some("executor Dart envenenado".into()));
            let mut r = match self.nativo_da_fase(fi) {
                Some(n) if !self.nativos[n].por_pacote() => {
                    self.nativo_por_acao(ctx, &HashSet::new(), n, a, motivo_dart.as_deref())
                }
                _ if motivo_dart.is_none() => self.dart_ou_apoio(a),
                _ => self.apoio(a, motivo_dart.as_deref()),
            };
            r.impressao = self.impressao(a, &r.consultas);
            for (s, c) in &r.saidas {
                if let Some(c) = c {
                    self.memoria.insert(self.naturais[s].clone(), c.clone());
                }
            }
            self.registros[a] = Some(r);
            self.estado_sujo = true;
            self.publicar();
            self.recalcular_observados();
            if self.opcoes.persistir && self.salvar().is_ok() {
                self.estado_sujo = false;
            }
        }
        self.memoria.get(&k).cloned()
    }

    /// Saídas `build_to: source` de gerador nativo entre `alterados`, com o
    /// conteúdo: vão ao disco (D-B2). As de apoio já estão lá.
    pub fn saidas_source_nativas(&self, alterados: &[PathBuf]) -> Vec<(PathBuf, Arc<[u8]>)> {
        if alterados.is_empty() {
            return Vec::new();
        }
        let alt: HashSet<&PathBuf> = alterados.iter().collect();
        let mut v = Vec::new();
        for (a, r) in self.registros.iter().enumerate() {
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

    /// Estado canônico (ações, origem, motivo, digest de cada saída e a
    /// geração publicada): o que os invariantes de determinismo e
    /// "incremental = do zero" comparam.
    pub fn estado_canonico(&self) -> String {
        let mut linhas: Vec<String> = Vec::new();
        for (i, a) in self.grafo.acoes.iter().enumerate() {
            if !a.viva() {
                continue;
            }
            let f = &self.fases[a.fase];
            let cab = format!(
                "{}#{} {}",
                self.plano.aplicacoes[f.aplicacao].chave,
                f.fabrica,
                a.entrada.texto()
            );
            match &self.registros[i] {
                None => linhas.push(format!("{cab} -")),
                Some(r) => {
                    let saidas: Vec<String> = r
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
                    let apagados: Vec<String> = r.apagados.iter().map(AssetId::texto).collect();
                    linhas.push(format!(
                        "{cab} {:?} {} [{}]{}",
                        r.origem,
                        r.motivo.as_deref().unwrap_or(""),
                        saidas.join(" "),
                        if apagados.is_empty() {
                            String::new()
                        } else {
                            format!(" apagados=[{}]", apagados.join(" "))
                        }
                    ));
                }
            }
        }
        linhas.sort();
        let mut g: Vec<String> = self
            .geracao
            .iter()
            .map(|(p, f)| {
                let rel = p
                    .strip_prefix(&self.raiz)
                    .unwrap_or(p)
                    .to_string_lossy()
                    .replace('\\', "/");
                format!(
                    "gerado {rel} {}",
                    &blake3::hash(f.conteudo.as_bytes()).to_hex()[..16]
                )
            })
            .collect();
        g.sort();
        linhas.extend(g);
        linhas.join("\n")
    }

    /// Placar contra uma referência (o oráculo do corpus, ou o apoio no
    /// disco): por saída de referência, igual/pendente/diferente.
    pub fn placar(&self, referencia: &BTreeMap<AssetId, Vec<u8>>) -> Placar {
        let mut p = Placar::default();
        for (id, esperado) in referencia {
            let Some(g) = self.grafo.gerados.get(id) else {
                // Um pós-processador escreve saídas que o grafo não prevê
                // (`PostProcessBuildStep`): estão no registro da âncora.
                let da_ancora = self
                    .grafo
                    .acoes
                    .iter()
                    .zip(&self.registros)
                    .filter(|(a, _)| a.pos)
                    .filter_map(|(_, r)| r.as_ref())
                    .find_map(|r| {
                        r.saidas
                            .iter()
                            .find(|(s, _)| s == id)
                            .map(|(_, c)| (r, c.clone()))
                    });
                if let Some((r, c)) = da_ancora {
                    match (&r.origem, c) {
                        (Origem::Dart, Some(c)) if c.as_ref() == esperado.as_slice() => {
                            p.iguais.push(id.clone())
                        }
                        _ => p
                            .diferentes
                            .push((id.clone(), "pós-processador: difere do oficial".into())),
                    }
                    continue;
                }
                let tem_pos = self
                    .fases
                    .iter()
                    .any(|f| f.pos && self.grafo_pacotes.nos[f.pacote].nome == *id.pacote);
                if tem_pos {
                    p.pendentes.push((
                        id.clone(),
                        "pós-processador: saída só com o executor Dart".into(),
                    ));
                } else {
                    p.diferentes
                        .push((id.clone(), "saída não prevista pelo plano".into()));
                }
                continue;
            };
            match self.registros[g.acao].as_ref() {
                None => p.pendentes.push((id.clone(), "não calculada".into())),
                Some(r) => {
                    let c = r
                        .saidas
                        .iter()
                        .find(|(s, _)| s == id)
                        .and_then(|(_, c)| c.clone());
                    match (&r.origem, c) {
                        (Origem::Nativo(n), _) if r.do_apoio.contains(id) => p.pendentes.push((
                            id.clone(),
                            format!("{n}: saída não gerada pelo nativo; apoio do build_runner"),
                        )),
                        (Origem::Nativo(_) | Origem::Dart, Some(c))
                            if c.as_ref() == esperado.as_slice() =>
                        {
                            p.iguais.push(id.clone())
                        }
                        (Origem::Nativo(n), _) => p
                            .diferentes
                            .push((id.clone(), format!("{n}: difere do oficial"))),
                        (Origem::Dart, _) => p
                            .diferentes
                            .push((id.clone(), "executor Dart: difere do oficial".into())),
                        (Origem::NaoDisparada, _) => p.diferentes.push((
                            id.clone(),
                            "nenhum trigger disparou aqui; o oficial escreveu".into(),
                        )),
                        (Origem::Omitida, _) => p.diferentes.push((
                            id.clone(),
                            "entrada gerada não escrita aqui; o oficial escreveu".into(),
                        )),
                        _ => {
                            if let Some((_, m)) = r.medido.iter().find(|(s, _)| s == id) {
                                if m.as_ref() == esperado.as_slice() {
                                    p.medidos_iguais += 1;
                                } else {
                                    p.medidos_diferentes += 1;
                                }
                            }
                            p.pendentes.push((
                                id.clone(),
                                r.motivo.clone().unwrap_or_else(|| "apoio".into()),
                            ))
                        }
                    }
                }
            }
        }
        // Saída nativa a mais (que a referência não tem). A de um builder
        // opcional (`is_optional`) só existe no oficial se alguém a pediu: sem
        // falha dele naquela entrada (`error_cache`), é uma saída que o grafo
        // oficial nunca solicitou (não alcançável) — fora da conta, não
        // pendente: não é esperada.
        for (i, r) in self.registros.iter().enumerate() {
            let Some(r) = r else { continue };
            let n = match r.origem {
                Origem::Nativo(n) => n,
                Origem::Dart => "executor Dart",
                _ => continue,
            };
            let acao = &self.grafo.acoes[i];
            let opcional = self.fases[acao.fase].opcional;
            for (s, c) in &r.saidas {
                if c.is_none() || referencia.contains_key(s) {
                    continue;
                }
                if opcional && !self.falhou_no_oficial(&acao.entrada) {
                    p.nao_solicitados.push((
                        s.clone(),
                        format!("{n}: saída opcional que o grafo oficial não solicita"),
                    ));
                } else {
                    p.diferentes.push((s.clone(), format!("{n}: saída a mais")));
                }
            }
        }
        p
    }

    /// O `build_runner` oficial registrou erro de algum builder nesta
    /// entrada (`.dart_tool/build/<hash>/error_cache/<pacote>/<fase>/<caminho>`).
    fn falhou_no_oficial(&self, entrada: &AssetId) -> bool {
        let base = self.grafo_pacotes.dir_raiz.join(".dart_tool/build");
        let Ok(hashes) = std::fs::read_dir(&base) else {
            return false;
        };
        for h in hashes.flatten() {
            let pacote = h.path().join("error_cache").join(entrada.pacote.as_ref());
            let Ok(fases) = std::fs::read_dir(&pacote) else {
                continue;
            };
            for f in fases.flatten() {
                if f.path().join(entrada.caminho.as_ref()).is_file() {
                    return true;
                }
            }
        }
        false
    }
}

/// `AssetId.resolve(Uri.parse(uri), from: de)`: `package:p/x` é `p|lib/x`,
/// `asset:p/x` é `p|x`, um caminho relativo é relativo ao diretório de `de`.
/// `None` para outro esquema ou caminho que sai do pacote.
fn resolver_uri(uri: &str, de: &AssetId) -> Option<AssetId> {
    if let Some(resto) = uri.strip_prefix("package:") {
        let (p, c) = resto.split_once('/')?;
        return Some(AssetId::novo(p, &format!("lib/{c}")));
    }
    if let Some(resto) = uri.strip_prefix("asset:") {
        let (p, c) = resto.split_once('/')?;
        return Some(AssetId::novo(p, c));
    }
    if uri.contains(':') {
        return None;
    }
    let mut partes: Vec<&str> = if uri.starts_with('/') {
        Vec::new()
    } else {
        let mut v: Vec<&str> = de.caminho.split('/').collect();
        v.pop();
        v
    };
    for s in uri.split('/') {
        match s {
            "" | "." => {}
            ".." => {
                partes.pop()?;
            }
            x => partes.push(x),
        }
    }
    Some(AssetId::novo(&de.pacote, &partes.join("/")))
}

fn mais_nova(entrada: &Path, apoio: &Path) -> bool {
    let (Some(e), Some(a)) = (Marca::ler(entrada), Marca::ler(apoio)) else {
        return false;
    };
    e.mtime_ns > a.mtime_ns
}

#[derive(Debug, Default, Clone)]
pub struct Placar {
    pub iguais: Vec<AssetId>,
    pub pendentes: Vec<(AssetId, String)>,
    pub diferentes: Vec<(AssetId, String)>,
    /// Saídas que o builder poderia produzir mas o grafo oficial nunca pede
    /// (builder opcional sem ninguém que leia a saída): fora da conta — não
    /// são esperadas, nem pendentes, nem diferentes.
    pub nao_solicitados: Vec<(AssetId, String)>,
    /// Nativos não verificados, medidos contra a referência.
    pub medidos_iguais: usize,
    pub medidos_diferentes: usize,
}

impl Placar {
    pub fn resumo(&self) -> String {
        let mut s = format!(
            "{} iguais / {} pendentes / {} diferentes",
            self.iguais.len(),
            self.pendentes.len(),
            self.diferentes.len()
        );
        if !self.nao_solicitados.is_empty() {
            s.push_str(&format!(
                " ({} não solicitados, fora da conta)",
                self.nao_solicitados.len()
            ));
        }
        if self.medidos_iguais + self.medidos_diferentes > 0 {
            s.push_str(&format!(
                " (nativos não verificados, medidos: {} iguais, {} diferentes)",
                self.medidos_iguais, self.medidos_diferentes
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
