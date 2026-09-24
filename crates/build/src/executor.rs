//! Os executores atrás de uma escolha só (`docs/BUILD-MOTOR.md` §6):
//! **nativo** (Rust, lê o banco semântico), **Dart** (`dfexec/1`, hoje só
//! [`Indisponivel`]) e **apoio** (o que o `build_runner` deixou no disco, em
//! `motor.rs`). Quem não sabe gerar recusa com motivo.
use crate::consulta::{digest_arquivo, digest_bytes, BancoSemantico, Consulta, Digest};
use crate::grafo::AssetId;
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
