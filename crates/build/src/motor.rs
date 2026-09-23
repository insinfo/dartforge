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
//!   [`Motor::materializar`] ou [`Demanda::Tudo`].
use crate::consulta::{digest_bytes, BancoSemantico, Consulta, Digest};
use crate::executor::{
    digest_de, AcaoNativa, CtxGerador, Disponibilidade, ExecutorDart, GeradorNativo, Indisponivel, PedidoNativo,
};
use crate::grafo::{AssetId, Grafo};
use crate::pacotes::GrafoPacotes;
use crate::plano::{fases, Configs, Fase, NoAlvo, Plano};
use dartforge_elements::config::PackageConfig;
use dartforge_elements::gerado::{chave, Construtor, Geracao};
use dartforge_elements::model::Program;
use dartforge_elements::unidades::Marca;
use dartforge_intern::Interner;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
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
}

impl Default for OpcoesMotor {
    fn default() -> Self {
        OpcoesMotor {
            release: false,
            trabalhadores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8),
            estrito: false,
            medir_nao_verificados: false,
        }
    }
}

/// Quem produziu uma saída.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origem {
    Nativo(&'static str),
    Apoio,
    Pendente,
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
    pub apoio: usize,
    pub pendentes_por_motivo: BTreeMap<String, usize>,
    pub tempo: Duration,
    /// Revalidação das rodadas por pacote (digests das consultas afetadas).
    pub tempo_revalidar: Duration,
    /// Geradores nativos por pacote (o estágio A do ngdart).
    pub tempo_nativo: Duration,
}

impl RelMotor {
    pub fn texto(&self) -> String {
        let mut s = format!(
            "motor: {} ações verificadas, {} executadas ({} nativas, {} apoio), {} consultas reavaliadas, {} saídas alteradas, {:.1} ms (revalidar {:.1} ms, nativo por pacote {:.1} ms)",
            self.acoes_verificadas,
            self.acoes_executadas,
            self.nativas,
            self.apoio,
            self.consultas_reavaliadas,
            self.saidas_alteradas,
            self.tempo.as_secs_f64() * 1000.0,
            self.tempo_revalidar.as_secs_f64() * 1000.0,
            self.tempo_nativo.as_secs_f64() * 1000.0,
        );
        for (m, n) in &self.pendentes_por_motivo {
            s.push_str(&format!("\n  pendentes ({n}): {m}"));
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
}

pub struct Motor {
    pub raiz: PathBuf,
    cfg: PackageConfig,
    pub grafo_pacotes: GrafoPacotes,
    pub configs: Configs,
    pub plano: Plano,
    pub fases: Vec<Fase>,
    pub alvos: Vec<NoAlvo>,
    pub grafo: Grafo,
    pub opcoes: OpcoesMotor,
    nativos: Vec<Arc<dyn GeradorNativo>>,
    dart: std::sync::Mutex<Box<dyn ExecutorDart>>,
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
}

fn natural(grafo: &GrafoPacotes, id: &AssetId) -> PathBuf {
    let raiz = grafo.no(&id.pacote).map(|n| n.raiz.clone()).unwrap_or_default();
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
        if !a.saidas.is_empty() {
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
    Indices { por_fase, naturais, esperadas, fontes, dirs: dirs.into_iter().collect() }
}

/// A consulta pode ter mudado com estes eventos?
fn afetada(c: &Consulta, mudados: &HashSet<PathBuf>, dart_mudou: bool, estruturais: &HashSet<PathBuf>) -> bool {
    match c {
        // Uma listagem só muda quando entra ou sai arquivo (diretório observado
        // que mudou, ou arquivo novo/apagado).
        Consulta::Glob { dir, .. } => estruturais.iter().any(|m| m.starts_with(dir)),
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
        let configs = Configs::ler(&grafo_pacotes, true)?;
        let (fases, alvos) = fases(&grafo_pacotes, &configs, &plano, opcoes.release)?;
        let grafo = Grafo::montar(&grafo_pacotes, &configs, &fases, &alvos)?;
        let Indices { por_fase, naturais, esperadas, fontes, dirs } = indices(&grafo_pacotes, &grafo, fases.len());
        let mut configuracao = vec![
            chave(&grafo_pacotes.dir_raiz.join("pubspec.lock")),
            chave(&grafo_pacotes.dir_raiz.join(".dart_tool").join("package_config.json")),
        ];
        let r = &grafo_pacotes.nos[grafo_pacotes.raiz].raiz;
        configuracao.push(chave(&r.join("pubspec.yaml")));
        configuracao.push(chave(&r.join("build.yaml")));
        let n_acoes = grafo.acoes.len();
        let mut m = Motor {
            raiz: raiz.to_path_buf(),
            cfg: cfg.clone(),
            grafo_pacotes,
            configs,
            plano,
            fases,
            alvos,
            grafo,
            opcoes,
            nativos: Vec::new(),
            dart: std::sync::Mutex::new(Box::new(Indisponivel::default())),
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
        };
        for c in m.configuracao.iter().chain(&m.dirs) {
            m.marcas.insert(c.clone(), Marca::ler(c));
        }
        m.recalcular_observados();
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
        self.dart = std::sync::Mutex::new(e);
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
        let mut v: BTreeSet<PathBuf> = self.configuracao.iter().chain(&self.dirs).cloned().collect();
        let consultas = self
            .registros
            .iter()
            .flatten()
            .flat_map(|r| r.consultas.iter())
            .chain(self.pacotes.values().flat_map(|p| p.registro_consultas.iter()));
        for (c, _) in consultas {
            if let Consulta::Arquivo(p) = c {
                if !p.to_string_lossy().ends_with(".dart") && !self.memoria.contains_key(p) && !self.apoio_de(p) {
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
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
        let agora: Vec<Option<Marca>> = if caminhos.len() < 64 || threads < 2 {
            caminhos.iter().map(|p| Marca::ler(p)).collect()
        } else {
            let pedaco = caminhos.len().div_ceil(threads);
            std::thread::scope(|s| {
                let hs: Vec<_> = caminhos
                    .chunks(pedaco)
                    .map(|c| s.spawn(move || c.iter().map(|p| Marca::ler(p)).collect::<Vec<_>>()))
                    .collect();
                hs.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
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

    fn demandada(&self, a: usize, demanda: Demanda) -> bool {
        if demanda == Demanda::Tudo || self.registros[a].is_some() {
            return true;
        }
        let f = &self.fases[self.grafo.acoes[a].fase];
        !f.oculta || self.grafo.acoes[a].saidas.iter().any(|s| s.caminho.ends_with(".dart"))
    }

    /// O gerador nativo que cobre a fase, se a versão do lock é a imitada.
    fn nativo_da_fase(&self, fi: usize) -> Option<usize> {
        let f = &self.fases[fi];
        let chave = &self.plano.aplicacoes[f.aplicacao].chave;
        self.nativos.iter().position(|g| {
            g.chave() == chave
                && g.cobre(&f.fabrica)
                && (g.verificado() || self.opcoes.medir_nao_verificados)
                && crate::descritor::imita(chave)
                    .iter()
                    .all(|(p, v)| self.grafo_pacotes.lock.get(*p).is_some_and(|t| t.versao == *v))
        })
    }

    /// A configuração mudou (pubspec, lock, build.yaml)? Então o plano é
    /// refeito do zero, como o `build_runner` faz quando o script muda.
    fn configuracao_mudou(&self, mudados: &HashSet<PathBuf>) -> bool {
        self.configuracao.iter().any(|c| mudados.contains(c))
            || mudados.iter().any(|m| m.file_name().is_some_and(|n| n.to_string_lossy().ends_with("build.yaml")))
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
            self.grafo_pacotes.no(p).is_some_and(|n| m.starts_with(&n.raiz) && !m.starts_with(n.raiz.join(".dart_tool")))
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
        let Indices { por_fase, naturais, esperadas, fontes, dirs } =
            indices(&self.grafo_pacotes, &grafo, self.fases.len());
        self.registros = grafo
            .acoes
            .iter()
            .map(|a| if a.saidas.is_empty() { None } else { antigos.remove(&(a.fase, a.entrada.clone())) })
            .collect();
        // Saídas que deixaram de existir saem da memória.
        let validas: HashSet<&PathBuf> = naturais.values().collect();
        let sumiram: Vec<PathBuf> = self.memoria.keys().filter(|k| !validas.contains(k)).cloned().collect();
        for k in sumiram {
            self.memoria.remove(&k);
            mudados.insert(k);
        }
        for d in &dirs {
            self.marcas.entry(d.clone()).or_insert_with(|| Marca::ler(d));
        }
        self.grafo = grafo;
        self.por_fase = por_fase;
        self.naturais = naturais;
        self.esperadas = esperadas;
        self.fontes = fontes;
        self.dirs = dirs;
        Ok(())
    }

    /// Recalcula o que for preciso e publica a geração.
    pub fn atualizar(&mut self, ctx: &Contexto<'_>, mudados: &[PathBuf], demanda: Demanda) -> Result<Atualizacao, String> {
        let t0 = Instant::now();
        let mut mudados: HashSet<PathBuf> = mudados.iter().map(|p| chave(p)).collect();
        if self.configuracao_mudou(&mudados) {
            let antiga = std::mem::take(&mut self.memoria);
            let opcoes = self.opcoes.clone();
            let nativos = std::mem::take(&mut self.nativos);
            let dart = std::mem::replace(&mut self.dart, std::sync::Mutex::new(Box::new(Indisponivel::default())));
            *self = Motor::novo(&self.raiz.clone(), &self.cfg.clone(), opcoes)?;
            self.nativos = nativos;
            self.dart = dart;
            // Tudo o que existia conta como possivelmente alterado.
            for k in antiga.keys() {
                mudados.insert(k.clone());
            }
            self.memoria = antiga;
        }
        // Eventos que mudam listagens: diretório observado, arquivo novo ou
        // apagado. Também refazem as fontes e as saídas esperadas.
        let estruturais: HashSet<PathBuf> = mudados.iter().filter(|m| self.estrutural(m)).cloned().collect();
        if !estruturais.is_empty() {
            self.reconstruir_grafo(&mut mudados)?;
        }
        let mut rel = RelMotor::default();
        let mut avisos = Vec::new();
        let mut alterados: BTreeSet<PathBuf> = BTreeSet::new();
        let dart_mudou = mudados.iter().any(|p| p.to_string_lossy().ends_with(".dart"));
        let mut pacotes_feitos: HashSet<(usize, Arc<str>)> = HashSet::new();
        let mut pacotes_rodaram: HashSet<(usize, Arc<str>)> = HashSet::new();
        let disp = self.dart.lock().map(|d| d.disponibilidade()).unwrap_or(Disponibilidade::Indisponivel("executor Dart envenenado".into()));
        let motivo_dart = match disp {
            Disponibilidade::Disponivel => None,
            Disponibilidade::Indisponivel(m) => Some(m),
        };

        for fi in 0..self.fases.len() {
            let acoes: Vec<usize> = self.por_fase[fi].iter().copied().filter(|&a| self.demandada(a, demanda)).collect();
            if acoes.is_empty() {
                continue;
            }
            let nativo = self.nativo_da_fase(fi);
            // Gerador por pacote: uma rodada por (gerador, pacote), feita (ou
            // reaproveitada) na primeira fase que ele cobre.
            let mut rodou: Option<bool> = None;
            if let Some(g) = nativo.filter(|&g| self.nativos[g].por_pacote()) {
                let pacote: Arc<str> = self.grafo_pacotes.nos[self.fases[fi].pacote].nome.as_str().into();
                let k = (g, pacote.clone());
                if !pacotes_feitos.contains(&k) {
                    pacotes_feitos.insert(k.clone());
                    if self.rodar_pacote(ctx, g, &pacote, &mudados, dart_mudou, &estruturais, &mut rel)? {
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
                    (Some(r), None) => {
                        // Só as consultas que os eventos podem ter mudado.
                        let memoria = |p: &Path| self.memoria.get(p).cloned();
                        r.consultas.iter().filter(|(c, _)| afetada(c, &mudados, dart_mudou, &estruturais)).any(|(c, d)| {
                            rel.consultas_reavaliadas += 1;
                            digest_de(c, ctx.banco, &memoria) != *d
                        })
                    }
                };
                if sujo {
                    executar.push(a);
                }
            }
            if executar.is_empty() {
                continue;
            }
            let novos: Vec<Registro> = match nativo {
                Some(g) if self.nativos[g].por_pacote() => {
                    executar.iter().map(|&a| self.de_pacote(g, a, motivo_dart.as_deref())).collect()
                }
                Some(g) => {
                    let mut v = Vec::with_capacity(executar.len());
                    for &a in &executar {
                        v.push(self.nativo_por_acao(ctx, g, a, motivo_dart.as_deref()));
                    }
                    v
                }
                None => self.apoio_em_paralelo(&executar, motivo_dart.as_deref()),
            };
            for (a, mut r) in executar.into_iter().zip(novos) {
                rel.acoes_executadas += 1;
                match r.origem {
                    Origem::Nativo(_) => rel.nativas += 1,
                    Origem::Apoio => rel.apoio += 1,
                    Origem::Pendente => {}
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
                                "{}: {} pode estar desatualizado — o DartForge ainda não executa builders Dart (BUILD-RUST.md §3, Fase 1); rode 'dart run build_runner build'",
                                self.plano.aplicacoes[self.fases[fi].aplicacao].chave,
                                s.texto()
                            );
                            if self.opcoes.estrito {
                                return Err(msg);
                            }
                            avisos.push(msg);
                        }
                    }
                }
                // Corte pela saída: só o que mudou de conteúdo segue adiante.
                for (s, c) in &r.saidas {
                    let n = self.naturais[s].clone();
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
            }
        }
        rel.saidas_alteradas = alterados.len();
        if !alterados.is_empty() || self.geracao.vazia() {
            self.publicar();
        }
        rel.tempo = t0.elapsed();
        self.recalcular_observados();
        Ok(Atualizacao { geracao: self.geracao.clone(), alterados: alterados.into_iter().collect(), rel, avisos })
    }

    fn naturais_de_entrada(&self, a: usize) -> PathBuf {
        natural(&self.grafo_pacotes, &self.grafo.acoes[a].entrada)
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
        for (p, v) in crate::descritor::imita(&ap.chave) {
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
            let talvez = r.registro_consultas.iter().any(|(c, _)| afetada(c, mudados, dart_mudou, estruturais));
            if !talvez {
                return Ok(false);
            }
            // Só as consultas que os eventos podem ter mudado são refeitas.
            let t = Instant::now();
            let memoria = |p: &Path| self.memoria.get(p).cloned();
            let mut iguais = true;
            for (c, d) in r.registro_consultas.iter().filter(|(c, _)| afetada(c, mudados, dart_mudou, estruturais)) {
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
            if self.grafo_pacotes.nos[f.pacote].nome != **pacote || self.nativo_da_fase(fi) != Some(g) {
                continue;
            }
            for &a in &self.por_fase[fi] {
                let acao = &self.grafo.acoes[a];
                acoes.push(AcaoNativa {
                    fabrica: f.fabrica.clone(),
                    entrada: acao.entrada.clone(),
                    entrada_natural: natural(&self.grafo_pacotes, &acao.entrada),
                    saidas: acao.saidas.iter().map(|s| (s.clone(), self.naturais[s].clone())).collect(),
                    opcoes: f.opcoes.clone(),
                });
            }
        }
        let no = self.grafo_pacotes.no(pacote).ok_or("pacote desconhecido")?;
        let pedido = PedidoNativo { pacote: pacote.to_string(), raiz_do_pacote: no.raiz.clone(), acoes };
        let (resultado, consultas) = {
            let memoria = |p: &Path| self.memoria.get(p).cloned();
            let mut c = CtxGerador::novo(ctx.programa, ctx.banco, &memoria);
            let r = gerador.gerar(&mut c, &pedido);
            (r, std::mem::take(&mut c.consultas))
        };
        let rodada = match resultado {
            Ok(s) => RodadaPacote {
                registro_consultas: consultas,
                saidas: s.saidas.into_iter().map(|(p, b)| (chave(&p), Arc::from(b))).collect(),
                recusas: s.recusas.into_iter().map(|(p, m)| (chave(&p), m)).collect(),
                erro: None,
            },
            Err(e) => RodadaPacote {
                registro_consultas: consultas,
                saidas: BTreeMap::new(),
                recusas: BTreeMap::new(),
                erro: Some(e),
            },
        };
        self.pacotes.insert(k, rodada);
        rel.tempo_nativo += t_gerar.elapsed();
        Ok(true)
    }

    /// Registro de uma ação coberta por um gerador por pacote: as saídas
    /// geradas vêm da rodada; o que ele recusou vai para o apoio.
    fn de_pacote(&self, g: usize, a: usize, motivo_dart: Option<&str>) -> Registro {
        let acao = &self.grafo.acoes[a];
        let pacote: Arc<str> = self.grafo_pacotes.nos[self.fases[acao.fase].pacote].nome.as_str().into();
        let rodada = self.pacotes.get(&(g, pacote));
        let gerador = &self.nativos[g];
        let saidas: Vec<(AssetId, Option<Arc<[u8]>>)> = acao
            .saidas
            .iter()
            .map(|s| (s.clone(), rodada.and_then(|r| r.saidas.get(&self.naturais[s])).cloned()))
            .collect();
        let entrada = natural(&self.grafo_pacotes, &acao.entrada);
        // As consultas ficam na rodada do pacote (é ela que se revalida);
        // copiá-las para cada ação custava centenas de ms por edição.
        let consultas = Vec::new();
        // Nativo cobre a ação quando gerou alguma saída dela e não a recusou.
        let recusa = rodada.and_then(|r| r.erro.clone().or_else(|| r.recusas.get(&entrada).cloned()));
        let gerou_algo = saidas.iter().any(|(_, c)| c.is_some());
        if gerador.verificado() && recusa.is_none() && gerou_algo {
            // Saídas que o nativo não escreveu (ex.: o `.css.dart` não
            // compartilhado, que nenhum template importa) ficam não escritas.
            return Registro {
                impressao: [0; 32],
                consultas,
                saidas,
                origem: Origem::Nativo(gerador.chave()),
                motivo: None,
                medido: Vec::new(),
            };
        }
        // O apoio de uma ação que já vinha dele não é relido: o disco do
        // `build_runner` não muda sob a sessão (limitação declarada: rodá-lo
        // à parte pede reiniciar a sessão).
        let mut r = match &self.registros[a] {
            Some(ant) if ant.origem == Origem::Apoio => {
                let mut r = ant.clone();
                r.motivo = motivo_dart.map(|m| format!("apoio do build_runner: {m}"));
                r
            }
            _ => self.apoio(a, motivo_dart),
        };
        if !gerador.verificado() {
            r.medido = saidas.into_iter().filter_map(|(s, c)| c.map(|c| (s, c))).collect();
        }
        let motivo_nativo = recusa.unwrap_or_else(|| format!("{}: não gera esta saída", gerador.chave()));
        r.motivo = Some(match r.motivo.take() {
            Some(m) => format!("{motivo_nativo}; {m}"),
            None => motivo_nativo,
        });
        r
    }

    fn nativo_por_acao(&self, ctx: &Contexto<'_>, g: usize, a: usize, motivo_dart: Option<&str>) -> Registro {
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
                saidas: acao.saidas.iter().map(|s| (s.clone(), self.naturais[s].clone())).collect(),
                opcoes: f.opcoes.clone(),
            }],
        };
        let memoria = |p: &Path| self.memoria.get(p).cloned();
        let mut c = CtxGerador::novo(ctx.programa, ctx.banco, &memoria);
        let res = gerador.gerar(&mut c, &pedido);
        let consultas = std::mem::take(&mut c.consultas);
        match res {
            Ok(s) if gerador.verificado() && !s.saidas.is_empty() => Registro {
                impressao: [0; 32],
                consultas,
                saidas: acao
                    .saidas
                    .iter()
                    .map(|id| (id.clone(), s.saidas.get(&self.naturais[id]).map(|b| Arc::from(b.clone()))))
                    .collect(),
                origem: Origem::Nativo(gerador.chave()),
                motivo: None,
                medido: Vec::new(),
            },
            Ok(s) => {
                let mut r = self.apoio(a, motivo_dart);
                r.medido = acao
                    .saidas
                    .iter()
                    .filter_map(|id| s.saidas.get(&self.naturais[id]).map(|b| (id.clone(), Arc::from(b.clone()))))
                    .collect();
                let m = s
                    .recusas
                    .values()
                    .next()
                    .cloned()
                    .unwrap_or_else(|| format!("{}: saída ainda não verificada byte a byte", gerador.chave()));
                r.motivo = Some(match r.motivo.take() {
                    Some(x) => format!("{m}; {x}"),
                    None => m,
                });
                r
            }
            Err(m) => {
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
            hs.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
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
        let d = memoria(&entrada).map(|b| digest_bytes(&b)).or_else(|| crate::consulta::digest_arquivo(&entrada));
        consultas.push((Consulta::Arquivo(entrada), d));
        let mut saidas = Vec::with_capacity(acao.saidas.len());
        for s in &acao.saidas {
            let loc = self.caminho_de_apoio(s, f.oculta);
            let c: Option<Arc<[u8]>> = std::fs::read(&loc).ok().map(Arc::from);
            consultas.push((Consulta::Arquivo(chave(&loc)), c.as_deref().map(digest_bytes)));
            saidas.push((s.clone(), c));
        }
        let tem_build = self.grafo_pacotes.dir_raiz.join(".dart_tool/build").is_dir();
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
            };
        }
        Registro {
            impressao: [0; 32],
            consultas,
            saidas,
            origem: Origem::Apoio,
            motivo: motivo_dart.map(|m| format!("apoio do build_runner: {m}")),
            medido: Vec::new(),
        }
    }

    /// Publica a geração: saídas com conteúdo UTF-8 que o carregador lê da
    /// memória — as `cache`, e as `source` de gerador nativo (as de apoio já
    /// estão no disco, no caminho natural).
    fn publicar(&mut self) {
        let mut c = Construtor::nova();
        let mut h = blake3::Hasher::new();
        let mut itens: Vec<(PathBuf, Arc<[u8]>, &'static str, PathBuf)> = Vec::new();
        let mut rotulos: Vec<(usize, String)> = Vec::new();
        for (a, r) in self.registros.iter().enumerate() {
            let Some(r) = r else { continue };
            let acao = &self.grafo.acoes[a];
            let f = &self.fases[acao.fase];
            if !f.oculta && !matches!(r.origem, Origem::Nativo(_)) {
                continue;
            }
            for (s, conteudo) in &r.saidas {
                let Some(conteudo) = conteudo else { continue };
                let rotulo = match r.origem {
                    Origem::Nativo(_) => self.plano.aplicacoes[f.aplicacao].chave.clone(),
                    _ => "build_runner".to_string(),
                };
                rotulos.push((a, rotulo));
                itens.push((self.naturais[s].clone(), conteudo.clone(), "", natural(&self.grafo_pacotes, &acao.entrada)));
            }
        }
        let rotulos: Vec<&'static str> = rotulos.into_iter().map(|(_, k)| self.rotulo(&k)).collect();
        for (i, (n, conteudo, _, entrada)) in itens.into_iter().enumerate() {
            let Ok(texto) = std::str::from_utf8(&conteudo) else { continue };
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
        let k = chave(caminho);
        if let Some(c) = self.memoria.get(&k) {
            return Some(c.clone());
        }
        let id = self.naturais.iter().find(|(_, n)| **n == k).map(|(id, _)| id.clone())?;
        let g = self.grafo.gerados.get(&id)?;
        if self.registros[g.acao].is_none() {
            let r = self.apoio(g.acao, None);
            for (s, c) in &r.saidas {
                if let Some(c) = c {
                    self.memoria.insert(self.naturais[s].clone(), c.clone());
                }
            }
            let _ = ctx;
            self.registros[g.acao] = Some(r);
            self.recalcular_observados();
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
            if !matches!(r.origem, Origem::Nativo(_)) || self.fases[self.grafo.acoes[a].fase].oculta {
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
            if a.saidas.is_empty() {
                continue;
            }
            let f = &self.fases[a.fase];
            let cab = format!("{}#{} {}", self.plano.aplicacoes[f.aplicacao].chave, f.fabrica, a.entrada.texto());
            match &self.registros[i] {
                None => linhas.push(format!("{cab} -")),
                Some(r) => {
                    let saidas: Vec<String> = r
                        .saidas
                        .iter()
                        .map(|(s, c)| {
                            let d = c.as_deref().map(|b| blake3::hash(b).to_hex().to_string()).unwrap_or_else(|| "∅".into());
                            format!("{}={}", s.caminho, &d[..d.len().min(16)])
                        })
                        .collect();
                    linhas.push(format!("{cab} {:?} {} [{}]", r.origem, r.motivo.as_deref().unwrap_or(""), saidas.join(" ")));
                }
            }
        }
        linhas.sort();
        let mut g: Vec<String> = self
            .geracao
            .iter()
            .map(|(p, f)| {
                let rel = p.strip_prefix(&self.raiz).unwrap_or(p).to_string_lossy().replace('\\', "/");
                format!("gerado {rel} {}", &blake3::hash(f.conteudo.as_bytes()).to_hex()[..16])
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
                // Um pós-processador pode escrever saídas que o grafo não
                // prevê (`PostProcessBuildStep`): só o executor Dart as faz.
                let tem_pos = self.fases.iter().any(|f| f.pos && self.grafo_pacotes.nos[f.pacote].nome == *id.pacote);
                if tem_pos {
                    p.pendentes.push((id.clone(), "pós-processador: saída só com o executor Dart".into()));
                } else {
                    p.diferentes.push((id.clone(), "saída não prevista pelo plano".into()));
                }
                continue;
            };
            match self.registros[g.acao].as_ref() {
                None => p.pendentes.push((id.clone(), "não calculada".into())),
                Some(r) => {
                    let c = r.saidas.iter().find(|(s, _)| s == id).and_then(|(_, c)| c.clone());
                    match (&r.origem, c) {
                        (Origem::Nativo(_), Some(c)) if c.as_ref() == esperado.as_slice() => p.iguais.push(id.clone()),
                        (Origem::Nativo(n), _) => p.diferentes.push((id.clone(), format!("{n}: difere do oficial"))),
                        _ => {
                            if let Some((_, m)) = r.medido.iter().find(|(s, _)| s == id) {
                                if m.as_ref() == esperado.as_slice() {
                                    p.medidos_iguais += 1;
                                } else {
                                    p.medidos_diferentes += 1;
                                }
                            }
                            p.pendentes.push((id.clone(), r.motivo.clone().unwrap_or_else(|| "apoio".into())))
                        }
                    }
                }
            }
        }
        // Saída nativa a mais (que a referência não tem).
        for r in self.registros.iter().flatten() {
            if let Origem::Nativo(n) = r.origem {
                for (s, c) in &r.saidas {
                    if c.is_some() && !referencia.contains_key(s) {
                        p.diferentes.push((s.clone(), format!("{n}: saída a mais")));
                    }
                }
            }
        }
        p
    }
}

fn mais_nova(entrada: &Path, apoio: &Path) -> bool {
    let (Some(e), Some(a)) = (Marca::ler(entrada), Marca::ler(apoio)) else { return false };
    e.mtime_ns > a.mtime_ns
}

#[derive(Debug, Default, Clone)]
pub struct Placar {
    pub iguais: Vec<AssetId>,
    pub pendentes: Vec<(AssetId, String)>,
    pub diferentes: Vec<(AssetId, String)>,
    /// Nativos não verificados, medidos contra a referência.
    pub medidos_iguais: usize,
    pub medidos_diferentes: usize,
}

impl Placar {
    pub fn resumo(&self) -> String {
        let mut s = format!("{} iguais / {} pendentes / {} diferentes", self.iguais.len(), self.pendentes.len(), self.diferentes.len());
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
