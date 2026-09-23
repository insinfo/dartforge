//! Consultas: o que uma ação de fato leu. A impressão digital de uma ação é
//! a lista `(consulta, digest da resposta)`; revalidar é recalcular só esses
//! digests (`docs/BUILD-MOTOR.md` §4). É a disciplina do `InputTracker`
//! oficial com dependência **semântica** no lugar do fecho de imports.
use std::path::{Path, PathBuf};

pub type Digest = [u8; 32];

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Consulta {
    /// Bytes de um arquivo (caminho natural; se for saída de outra ação,
    /// o conteúdo em memória).
    Arquivo(PathBuf),
    /// Existência (`canRead`), também a negativa.
    Existe(PathBuf),
    /// Lista ordenada do que casa `padrao` sob `dir`.
    Glob { dir: PathBuf, padrao: String },
    /// API pública de uma biblioteca (mais as anotações).
    ApiBiblioteca(String),
    /// Superfície de uma declaração: assinatura, anotações, membros públicos.
    Declaracao { biblioteca: String, nome: String },
    /// Termo de um índice do motor (ex.: `("ng.seletor", "x-y")`).
    Indice { espaco: &'static str, termo: String },
    /// Conservador: o texto de todas as unidades da biblioteca.
    FonteBiblioteca(String),
}

impl Consulta {
    /// Caminho de arquivo que, ao mudar, pode mudar a resposta.
    pub fn caminho(&self) -> Option<&Path> {
        match self {
            Consulta::Arquivo(p) | Consulta::Existe(p) => Some(p),
            Consulta::Glob { dir, .. } => Some(dir),
            _ => None,
        }
    }

    /// Responde ao banco semântico (e não ao disco).
    pub fn semantica(&self) -> bool {
        matches!(
            self,
            Consulta::ApiBiblioteca(_) | Consulta::Declaracao { .. } | Consulta::Indice { .. } | Consulta::FonteBiblioteca(_)
        )
    }
}

/// Quem responde às consultas semânticas: a sessão do `dev` (sobre o
/// `Program` já carregado — o `BuildStep.resolver` sem carga extra) ou,
/// numa passada única, [`SemBanco`].
pub trait BancoSemantico: Sync {
    fn digest(&self, c: &Consulta) -> Option<Digest>;
}

/// Passada única (`compile-js`, `build`): não há revalidação, então as
/// consultas semânticas não precisam de resposta.
pub struct SemBanco;

impl BancoSemantico for SemBanco {
    fn digest(&self, _c: &Consulta) -> Option<Digest> {
        None
    }
}

/// Digest de um arquivo no disco; ausente = `None`.
pub fn digest_arquivo(p: &Path) -> Option<Digest> {
    std::fs::read(p).ok().map(|b| *blake3::hash(&b).as_bytes())
}

pub fn digest_bytes(b: &[u8]) -> Digest {
    *blake3::hash(b).as_bytes()
}
