//! Os executores atrás de uma escolha só (`docs/BUILD-MOTOR.md` §6):
//! **nativo** (Rust, lê o banco semântico), **Dart** (`dfexec/1`, hoje só
//! [`Indisponivel`]) e **apoio** (o que o `build_runner` deixou no disco, em
//! `motor.rs`). Quem não sabe gerar recusa com motivo.
use crate::consulta::{digest_arquivo, digest_bytes, BancoSemantico, Consulta, Digest};
use crate::grafo::{AssetId, Grafo};
use crate::pacotes::GrafoPacotes;
use crate::valor::Mapa;
use dartforge_elements::model::Program;
use dartforge_intern::Interner;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

// ------------------------------------------------------------------ nativo

/// Uma ação pedida a um gerador nativo.
#[derive(Debug, Clone)]
pub struct AcaoNativa {
    pub fabrica: String,
    pub entrada: AssetId,
    pub entrada_natural: PathBuf,
    /// Saídas esperadas e o caminho natural de cada uma.
    pub saidas: Vec<(AssetId, PathBuf)>,
    pub opcoes: Mapa,
}

/// Um pedido: uma ação (gerador por arquivo) ou todas as ações das fases que
/// o gerador cobre num pacote (gerador por pacote — o estágio A do ngdart).
#[derive(Debug, Clone)]
pub struct PedidoNativo {
    pub pacote: String,
    pub raiz_do_pacote: PathBuf,
    pub acoes: Vec<AcaoNativa>,
}

/// O que um gerador nativo devolve: conteúdo por caminho natural de saída, e
/// recusas por caminho natural de **entrada** (o placar conta os motivos).
#[derive(Debug, Default)]
pub struct SaidaNativa {
    pub saidas: BTreeMap<PathBuf, Vec<u8>>,
    pub recusas: BTreeMap<PathBuf, String>,
    /// Arquivos que o gerador realmente regenerou nesta chamada.
    pub unidades_geradas: usize,
    /// O gerador confirmou que as consultas da rodada anterior continuam
    /// válidas; `CtxGerador` registra só as respostas que mudaram.
    pub reutilizar_consultas: bool,
}

/// Gerador em Rust que imita um builder do ecossistema byte a byte.
pub trait GeradorNativo: Send + Sync {
    /// Chave do builder (`ngdart:ngdart`).
    fn chave(&self) -> &'static str;
    /// Fábricas cobertas.
    fn cobre(&self, fabrica: &str) -> bool;
    /// Uma chamada por pacote (com todas as ações) em vez de uma por ação.
    fn por_pacote(&self) -> bool;
    /// Porta de igualdade: `false` = a saída ainda não é verificada byte a
    /// byte contra o oficial; o motor só a usa para medir (`--comparar`) e
    /// publica o apoio no lugar.
    fn verificado(&self) -> bool;
    fn gerar(&self, ctx: &mut CtxGerador<'_>, pedido: &PedidoNativo) -> Result<SaidaNativa, String>;
}

/// O único canal de leitura de um gerador nativo: cada leitura vira
/// consulta (regra de solidez — o que ele não consultou não pode
/// invalidá-lo).
pub struct CtxGerador<'a> {
    pub programa: Option<(&'a Program, &'a Interner)>,
    /// Eventos desta atualização, já normalizados pelo motor.
    pub mudados: &'a HashSet<PathBuf>,
    pub(crate) banco: &'a dyn BancoSemantico,
    pub(crate) memoria: &'a dyn Fn(&Path) -> Option<Arc<[u8]>>,
    pub(crate) consultas: Vec<(Consulta, Option<Digest>)>,
}

impl<'a> CtxGerador<'a> {
    pub(crate) fn novo(
        programa: Option<(&'a Program, &'a Interner)>,
        mudados: &'a HashSet<PathBuf>,
        banco: &'a dyn BancoSemantico,
        memoria: &'a dyn Fn(&Path) -> Option<Arc<[u8]>>,
    ) -> Self {
        CtxGerador { programa, mudados, banco, memoria, consultas: Vec::new() }
    }

    /// Registra uma consulta com o digest da resposta de agora.
    pub fn registrar(&mut self, c: Consulta) {
        let d = digest_de(&c, self.banco, self.memoria);
        self.consultas.push((c, d));
    }

    pub fn ler(&mut self, p: &Path) -> Option<Arc<[u8]>> {
        let chave = dartforge_elements::gerado::chave(p);
        let v = (self.memoria)(&chave).or_else(|| std::fs::read(p).ok().map(Arc::from));
        let d = v.as_deref().map(digest_bytes);
        self.consultas.push((Consulta::Arquivo(chave), d));
        v
    }

    pub fn existe(&mut self, p: &Path) -> bool {
        let chave = dartforge_elements::gerado::chave(p);
        let sim = (self.memoria)(&chave).is_some() || p.is_file();
        self.consultas.push((Consulta::Existe(chave), sim.then(|| digest_bytes(b"1"))));
        sim
    }
}

/// Digest atual da resposta de uma consulta.
pub(crate) fn digest_de(
    c: &Consulta,
    banco: &dyn BancoSemantico,
    memoria: &dyn Fn(&Path) -> Option<Arc<[u8]>>,
) -> Option<Digest> {
    match c {
        Consulta::Arquivo(p) => memoria(p).map(|b| digest_bytes(&b)).or_else(|| digest_arquivo(p)),
        Consulta::Existe(p) => (memoria(p).is_some() || p.is_file()).then(|| digest_bytes(b"1")),
        Consulta::Glob { dir, padrao } => {
            let g = crate::glob::Glob::novo(padrao).ok()?;
            let lista = crate::grafo::listar(dir, std::slice::from_ref(&g));
            Some(digest_bytes(lista.into_iter().collect::<Vec<_>>().join("\n").as_bytes()))
        }
        _ => banco.digest(c),
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

/// O equivalente ao `.dart_tool/build/entrypoint/build.dart`: imports das
/// fábricas e o mapa chave → fábricas (`build_script_generate.dart`).
#[derive(Debug, Clone)]
pub struct ScriptDeBuilders {
    pub aplicacoes: Vec<(String, String, Vec<String>)>,
    /// `blake3(fontes + versões do lock + versão do DartForge + ABI)`.
    pub chave_de_cache: String,
}

/// `build.executar` do `dfexec/1` (`docs/BUILD-PROTOCOLO.md` §3).
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

#[derive(Debug, Clone, Default)]
pub struct ResultadoAcao {
    pub saidas: Vec<(AssetId, Arc<[u8]>)>,
    pub logs: Vec<(String, String)>,
    pub falhou: bool,
}

/// O `Resolver` servido pelo banco semântico (Fase 3 do BUILD-RUST.md).
pub trait ServicoResolver {}

/// O `BuildStep` servido pelo motor: toda chamada vira `Consulta`.
pub trait ServicoBuildStep {
    fn can_read(&mut self, id: &AssetId) -> bool;
    fn ler(&mut self, id: &AssetId) -> Option<Arc<[u8]>>;
    fn find_assets(&mut self, glob: &str) -> Vec<AssetId>;
    fn digest(&mut self, id: &AssetId) -> Option<[u8; 32]>;
    fn escrever(&mut self, id: &AssetId, bytes: Arc<[u8]>) -> Result<(), SaidaNaoPermitida>;
    /// `None` ⇒ o analyzer roda dentro do executor.
    fn resolver(&mut self) -> Option<&mut dyn ServicoResolver>;
    fn log(&mut self, nivel: Nivel, msg: &str);
}

/// Visão de uma ação sobre o grafo. O executor nunca recebe caminhos livres:
/// um `AssetId` precisa existir no grafo e saídas da própria fase só ficam
/// legíveis depois de escritas pela ação corrente (`build_impl.dart:443-463`).
pub struct ServicoAcao<'a> {
    pub grafo: &'a Grafo,
    pub pacotes: &'a GrafoPacotes,
    pub acao: usize,
    pub memoria: &'a BTreeMap<AssetId, Arc<[u8]>>,
    pub escritas: BTreeMap<AssetId, Arc<[u8]>>,
    pub consultas: Vec<(Consulta, Option<Digest>)>,
    pub logs: Vec<(Nivel, String)>,
}

impl<'a> ServicoAcao<'a> {
    pub fn novo(
        grafo: &'a Grafo,
        pacotes: &'a GrafoPacotes,
        acao: usize,
        memoria: &'a BTreeMap<AssetId, Arc<[u8]>>,
    ) -> Self {
        Self { grafo, pacotes, acao, memoria, escritas: BTreeMap::new(), consultas: Vec::new(), logs: Vec::new() }
    }

    fn caminho(&self, id: &AssetId) -> Option<PathBuf> {
        let no = self.pacotes.no(&id.pacote)?;
        // Só os nós do grafo entram aqui; caminhos arbitrários do executor
        // não podem escapar da raiz do pacote.
        if !self.grafo.existe(id) || id.caminho.split('/').any(|p| p == ".." || p == ".") {
            return None;
        }
        Some(dartforge_elements::gerado::chave(&no.raiz.join(id.caminho.as_ref())))
    }

    fn bytes(&self, id: &AssetId) -> Option<Arc<[u8]>> {
        let fase = self.grafo.acoes.get(self.acao)?.fase;
        if let Some(g) = self.grafo.gerados.get(id) {
            if g.fase > fase || (g.fase == fase && g.acao != self.acao) {
                return None;
            }
            return if g.fase == fase { self.escritas.get(id).cloned() } else { self.memoria.get(id).cloned() };
        }
        if !self.grafo.tem_fonte(id) {
            return None;
        }
        std::fs::read(self.caminho(id)?).ok().map(Arc::from)
    }

    fn disponivel(&self, id: &AssetId) -> bool {
        let Some(fase) = self.grafo.acoes.get(self.acao).map(|a| a.fase) else { return false };
        if let Some(g) = self.grafo.gerados.get(id) {
            return if g.fase > fase {
                false
            } else if g.fase == fase {
                g.acao == self.acao && self.escritas.contains_key(id)
            } else {
                self.memoria.contains_key(id)
            };
        }
        self.grafo.tem_fonte(id) && self.caminho(id).is_some_and(|p| p.is_file())
    }

    fn registrar(&mut self, id: &AssetId, existe: bool, bytes: Option<&[u8]>) {
        if let Some(p) = self.caminho(id) {
            let c = if existe { Consulta::Existe(p) } else { Consulta::Arquivo(p) };
            let d = if existe { bytes.map(|_| digest_bytes(b"1")) } else { bytes.map(digest_bytes) };
            self.consultas.push((c, d));
        }
    }
}

impl ServicoBuildStep for ServicoAcao<'_> {
    fn can_read(&mut self, id: &AssetId) -> bool {
        let sim = self.disponivel(id);
        self.registrar(id, true, sim.then_some(&[]));
        sim
    }

    fn ler(&mut self, id: &AssetId) -> Option<Arc<[u8]>> {
        let b = self.bytes(id);
        self.registrar(id, false, b.as_deref());
        b
    }

    fn find_assets(&mut self, glob: &str) -> Vec<AssetId> {
        let Ok(padrao) = crate::glob::Glob::novo(glob) else { return Vec::new() };
        let mut ids: Vec<_> = self.grafo.fontes.iter().flat_map(|(p, cs)| cs.iter().map(|c| AssetId { pacote: p.clone(), caminho: c.clone() }))
            .chain(self.grafo.gerados.keys().cloned())
            .filter(|id| padrao.casa(&id.caminho)).collect();
        ids.sort();
        ids.dedup();
        ids.retain(|id| self.can_read(id));
        // Uma listagem vazia também depende da estrutura dos diretórios.
        for no in &self.pacotes.nos {
            if !no.raiz.as_os_str().is_empty() {
                let disco = crate::grafo::listar(&no.raiz, std::slice::from_ref(&padrao));
                self.consultas.push((Consulta::Glob { dir: no.raiz.clone(), padrao: glob.to_string() },
                    Some(digest_bytes(disco.into_iter().collect::<Vec<_>>().join("\n").as_bytes()))));
            }
        }
        ids
    }

    fn digest(&mut self, id: &AssetId) -> Option<[u8; 32]> {
        self.ler(id).as_deref().map(digest_bytes)
    }

    fn escrever(&mut self, id: &AssetId, bytes: Arc<[u8]>) -> Result<(), SaidaNaoPermitida> {
        if self.grafo.acoes.get(self.acao).is_none_or(|a| !a.saidas.contains(id)) {
            return Err(SaidaNaoPermitida(id.clone()));
        }
        self.escritas.insert(id.clone(), bytes);
        Ok(())
    }

    fn resolver(&mut self) -> Option<&mut dyn ServicoResolver> { None }

    fn log(&mut self, nivel: Nivel, msg: &str) {
        self.logs.push((nivel, msg.to_string()));
    }
}

/// O executor de builders Dart (Fase 4): processo persistente que fala
/// `dfexec/1`. Virá do executor nativo auto-hospedado, compartilhado com as
/// macros.
pub trait ExecutorDart: Send {
    fn disponibilidade(&self) -> Disponibilidade;
    fn preparar(&mut self, script: &ScriptDeBuilders) -> Result<(), ErroExecutor>;
    fn executar(&mut self, pedido: &PedidoAcao, servico: &mut dyn ServicoBuildStep)
        -> Result<ResultadoAcao, ErroExecutor>;
    fn encerrar(&mut self);
}

/// A única implementação de hoje.
pub struct Indisponivel(pub String);

impl Default for Indisponivel {
    fn default() -> Self {
        Indisponivel(
            "o DartForge ainda não executa builders Dart (BUILD-RUST.md §3, Fase 1; executor nativo compartilhado com as macros)"
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
    fn executar(&mut self, _p: &PedidoAcao, _s: &mut dyn ServicoBuildStep) -> Result<ResultadoAcao, ErroExecutor> {
        Err(ErroExecutor(self.0.clone()))
    }
    fn encerrar(&mut self) {}
}

#[cfg(test)]
mod testes_servico {
    use super::*;
    use crate::config::TipoDependencia;
    use crate::grafo::{Acao, NoGerado};
    use crate::pacotes::No;

    #[test]
    fn visibilidade_de_fases_e_saida_da_propria_acao() {
        let dir = tempfile::tempdir().unwrap();
        let pacotes = GrafoPacotes {
            nos: vec![No { nome: "p".into(), raiz: dir.path().to_path_buf(), tipo: TipoDependencia::Path, e_raiz: true, deps: vec![] }],
            raiz: 0,
            por_nome: [("p".into(), 0)].into(),
            lock: Default::default(),
            dir_raiz: dir.path().to_path_buf(),
        };
        let fonte = AssetId::novo("p", "lib/a.dart");
        let primeiro = AssetId::novo("p", "lib/a.g.dart");
        let segundo = AssetId::novo("p", "lib/a.h.dart");
        let futuro = AssetId::novo("p", "lib/a.i.dart");
        std::fs::create_dir(dir.path().join("lib")).unwrap();
        std::fs::write(dir.path().join("lib/a.dart"), b"source").unwrap();
        let mut grafo = Grafo::default();
        grafo.fontes.entry("p".into()).or_default().insert("lib/a.dart".into());
        grafo.acoes = vec![
            Acao { fase: 0, entrada: fonte.clone(), saidas: vec![primeiro.clone()] },
            Acao { fase: 1, entrada: primeiro.clone(), saidas: vec![segundo.clone()] },
            Acao { fase: 2, entrada: segundo.clone(), saidas: vec![futuro.clone()] },
        ];
        for (id, acao, fase) in [(&primeiro, 0, 0), (&segundo, 1, 1), (&futuro, 2, 2)] {
            grafo.gerados.insert(id.clone(), NoGerado { acao, fase, oculto: false });
        }
        let memoria = [(primeiro.clone(), Arc::from(&b"prior"[..])), (futuro.clone(), Arc::from(&b"future"[..]))].into();
        let mut s = ServicoAcao::novo(&grafo, &pacotes, 1, &memoria);
        assert_eq!(s.ler(&fonte).as_deref(), Some(&b"source"[..]));
        assert_eq!(s.ler(&primeiro).as_deref(), Some(&b"prior"[..]));
        assert!(!s.can_read(&segundo));
        assert!(!s.can_read(&futuro));
        assert!(s.escrever(&futuro, Arc::from(&b"bad"[..])).is_err());
        s.escrever(&segundo, Arc::from(&b"own"[..])).unwrap();
        assert_eq!(s.ler(&segundo).as_deref(), Some(&b"own"[..]));
        assert_eq!(s.find_assets("lib/**"), vec![fonte, primeiro, segundo]);
        assert!(s.consultas.iter().any(|(c, d)| matches!(c, Consulta::Existe(_)) && d.is_none()));
    }
}
