//! Etapas de geração na sessão do `dartforge dev`: o ponto em que o motor de
//! build (`crates/build`) — e, depois, o hospedeiro de macros — entram na
//! compilação sem reabrir o `Sessao::compilar`.
//!
//! **Custo zero para quem não usa** (PLANO.md, regra governante): a sessão
//! só cria uma etapa se [`etapa_de_build`] detectar `build_runner` no
//! `package_config.json` — uma consulta a um `HashMap`, uma vez por sessão.
//! Sem etapa, `Sessao.etapas` é vazio e o custo por edição é o de iterar um
//! `Vec` vazio.
use dartforge_build::consulta::{BancoSemantico, Consulta, Digest};
use dartforge_build::{Contexto, Demanda, Motor, OpcoesMotor, RelMotor};
use dartforge_elements::gerado::{chave, Geracao};
use dartforge_elements::model::Program;
use dartforge_intern::Interner;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// O que a sessão oferece a uma etapa: o programa já carregado (é o
/// `BuildStep.resolver` sem carga extra) e os arquivos sujos.
pub struct CtxSessao<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
}

pub struct AtualizacaoEtapa {
    pub geracao: Arc<Geracao>,
    /// Caminhos naturais cujo texto gerado mudou: só essas unidades são
    /// invalidadas.
    pub alterados: Vec<PathBuf>,
    pub rel: RelMotor,
    pub avisos: Vec<String>,
}

/// Leitor de saídas geradas pelo caminho natural, para o `serve` (um `.css`
/// pedido pelo navegador é demanda que materializa a ação).
pub type Provedor = Arc<dyn Fn(&Path) -> Option<Arc<[u8]>> + Send + Sync>;

pub trait EtapaDeGeracao: Send {
    /// `p` (caminho natural) é uma saída esperada? A primeira carga não aborta
    /// por "não foi possível ler" dela.
    fn espera(&self, p: &Path) -> bool;
    /// Entradas que não são Dart e que a etapa observa.
    fn observados(&self) -> Vec<PathBuf>;
    /// Observados que mudaram desde a última olhada.
    fn mudancas(&mut self) -> Vec<PathBuf>;
    fn atualizar(&mut self, ctx: &CtxSessao<'_>, mudados: &[PathBuf]) -> Result<AtualizacaoEtapa, String>;
    fn provedor(&self) -> Option<Provedor>;
}

/// A etapa do motor de build.
pub struct EtapaBuild {
    motor: Arc<Mutex<Motor>>,
    esperadas: HashSet<PathBuf>,
    /// Digest do texto de cada biblioteca (`FonteBiblioteca`), memorizado
    /// entre compilações e invalidado quando uma unidade dela fica suja.
    fontes: Mutex<HashMap<String, Digest>>,
}

/// Detecta e constrói a etapa do motor de build. `None` = o projeto não usa
/// builders (nada é lido além do `package_config.json`).
pub fn etapa_de_build(entrada: &Path, packages: Option<&Path>) -> Option<Result<EtapaBuild, String>> {
    let caminho = packages.map(Path::to_path_buf).or_else(|| dartforge_elements::config::PackageConfig::discover(entrada))?;
    let cfg = dartforge_elements::config::PackageConfig::load(&caminho).ok()?;
    if !dartforge_build::detectar(&cfg) {
        return None;
    }
    let raiz = dartforge_build::raiz_do_pacote(entrada)?;
    Some(Motor::novo(&raiz, &cfg, OpcoesMotor::default()).map(|m| EtapaBuild {
        esperadas: m.saidas_esperadas().clone(),
        motor: Arc::new(Mutex::new(m)),
        fontes: Mutex::new(HashMap::new()),
    }))
}

impl EtapaBuild {
    pub fn motor(&self) -> Arc<Mutex<Motor>> {
        self.motor.clone()
    }
}

/// O banco semântico sobre o `Program` da sessão.
struct BancoSessao<'a> {
    program: &'a Program,
    por_uri: HashMap<&'a str, usize>,
    fontes: &'a Mutex<HashMap<String, Digest>>,
}

impl BancoSemantico for BancoSessao<'_> {
    fn digest(&self, c: &Consulta) -> Option<Digest> {
        match c {
            Consulta::FonteBiblioteca(uri) => {
                if let Some(d) = self.fontes.lock().ok()?.get(uri) {
                    return Some(*d);
                }
                let &i = self.por_uri.get(uri.as_str())?;
                let l = &self.program.libraries[i];
                let mut h = blake3::Hasher::new();
                for &u in &l.units {
                    h.update(self.program.unit(u).source.as_bytes());
                    h.update(&[0]);
                }
                let d = *h.finalize().as_bytes();
                self.fontes.lock().ok()?.insert(uri.clone(), d);
                Some(d)
            }
            // API pública (o hash estrutural do `hashes.rs`, que ignora corpo,
            // comentário e espaço) **mais** as linhas de anotação e de
            // diretiva de cada unidade: o hash de API não visita metadados, e
            // para um gerador a anotação é tudo (uma `@Component` nova num
            // arquivo que era trivial tem de acordar o ngdart).
            Consulta::ApiBiblioteca(uri) => {
                let chave = format!("api {uri}");
                if let Some(d) = self.fontes.lock().ok()?.get(&chave) {
                    return Some(*d);
                }
                let &i = self.por_uri.get(uri.as_str())?;
                let lib = dartforge_elements::model::LibraryId(i as u32);
                let mut h = blake3::Hasher::new();
                h.update(&crate::hashes::hashes_da_biblioteca(self.program, lib).api.to_le_bytes());
                for &u in &self.program.libraries[i].units {
                    for linha in self.program.unit(u).source.lines() {
                        let t = linha.trim_start();
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
                self.fontes.lock().ok()?.insert(chave, d);
                Some(d)
            }
            // `Declaracao` e `Indice` entram com o estágio B do ngdart
            // (consultas finas); até lá ninguém as registra.
            _ => None,
        }
    }
}

impl EtapaDeGeracao for EtapaBuild {
    fn espera(&self, p: &Path) -> bool {
        self.esperadas.contains(&chave(p))
    }

    fn observados(&self) -> Vec<PathBuf> {
        self.motor.lock().map(|m| m.observados()).unwrap_or_default()
    }

    fn mudancas(&mut self) -> Vec<PathBuf> {
        self.motor.lock().map(|mut m| m.mudancas()).unwrap_or_default()
    }

    fn atualizar(&mut self, ctx: &CtxSessao<'_>, mudados: &[PathBuf]) -> Result<AtualizacaoEtapa, String> {
        // A memória de `FonteBiblioteca` perde as bibliotecas que mudaram.
        if !mudados.is_empty() {
            let sujos: HashSet<PathBuf> = mudados.iter().map(|p| chave(p)).collect();
            let mut f = self.fontes.lock().map_err(|_| "memória do banco envenenada")?;
            for l in &ctx.program.libraries {
                let sujo = l
                    .units
                    .iter()
                    .any(|&u| ctx.program.unit(u).path.as_ref().is_some_and(|p| sujos.contains(&chave(p))));
                if sujo {
                    f.remove(&l.uri);
                    f.remove(&format!("api {}", l.uri));
                }
            }
        }
        let banco = BancoSessao {
            program: ctx.program,
            por_uri: ctx.program.libraries.iter().enumerate().map(|(i, l)| (l.uri.as_str(), i)).collect(),
            fontes: &self.fontes,
        };
        let mut m = self.motor.lock().map_err(|_| "motor envenenado")?;
        let at = m.atualizar(
            &Contexto { banco: &banco, programa: Some((ctx.program, ctx.interner)) },
            mudados,
            Demanda::Carregador,
        )?;
        self.esperadas = m.saidas_esperadas().clone();
        // D-B2: saídas `build_to: source` de gerador nativo vão ao disco
        // quando o texto muda (como o `build_runner watch`).
        for (p, c) in m.saidas_source_nativas(&at.alterados) {
            if std::fs::read(&p).ok().as_deref() != Some(&c[..]) {
                std::fs::write(&p, &c[..]).map_err(|e| format!("{}: {e}", p.display()))?;
            }
        }
        Ok(AtualizacaoEtapa { geracao: at.geracao, alterados: at.alterados, rel: at.rel, avisos: at.avisos })
    }

    fn provedor(&self) -> Option<Provedor> {
        let motor = self.motor.clone();
        Some(Arc::new(move |p: &Path| {
            let mut m = motor.lock().ok()?;
            m.materializar(&Contexto { banco: &dartforge_build::consulta::SemBanco, programa: None }, p)
        }))
    }
}
