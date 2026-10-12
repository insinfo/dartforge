//! Queries: what an action actually read. An action's fingerprint is the
//! list `(query, digest of the answer)`; revalidating means recomputing only
//! those digests (`docs/BUILD-MOTOR.md` §4). It is the discipline of the
//! official `InputTracker`, with a **semantic** dependency in place of the
//! import closure.
use std::path::{Path, PathBuf};

pub type Digest = [u8; 32];

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Consulta {
    /// Bytes of a file (natural path; if it is the output of another action,
    /// the in-memory content).
    Arquivo(PathBuf),
    /// Existence (`canRead`), including the negative answer.
    Existe(PathBuf),
    /// Sorted list of what matches `padrao` under `dir`.
    Glob { dir: PathBuf, padrao: String },
    /// `BuildStep.findAssets`: graph candidates in the input's package.
    /// The `bool` marks a generated output, whose presence comes from memory,
    /// not from an old support file on disk.
    GlobAtivos {
        dir: PathBuf,
        padrao: String,
        candidatos: Vec<(String, bool)>,
    },
    /// Public API of a library (plus the annotations).
    ApiBiblioteca(String),
    /// Surface of a declaration: signature, annotations, public members.
    Declaracao { biblioteca: String, nome: String },
    /// Term of an engine index (e.g. `("ng.seletor", "x-y")`).
    Indice { espaco: &'static str, termo: String },
    /// Conservative: the text of all the library's units.
    FonteBiblioteca(String),
}

impl Consulta {
    /// File path that, when it changes, may change the answer.
    pub fn caminho(&self) -> Option<&Path> {
        match self {
            Consulta::Arquivo(p) | Consulta::Existe(p) => Some(p),
            Consulta::Glob { dir, .. } | Consulta::GlobAtivos { dir, .. } => Some(dir),
            _ => None,
        }
    }

    /// Answered by the semantic database (not by the disk).
    pub fn semantica(&self) -> bool {
        matches!(
            self,
            Consulta::ApiBiblioteca(_)
                | Consulta::Declaracao { .. }
                | Consulta::Indice { .. }
                | Consulta::FonteBiblioteca(_)
        )
    }
}

/// Who answers the semantic queries: the `dev` session (over the already
/// loaded `Program` — `BuildStep.resolver` without extra loading) or, in a
/// single pass, [`SemBanco`].
pub trait BancoSemantico: Sync {
    fn digest(&self, c: &Consulta) -> Option<Digest>;
}

/// Single pass (`compile-js`, `build`): there is no revalidation, so the
/// semantic queries need no answer.
pub struct SemBanco;

impl BancoSemantico for SemBanco {
    fn digest(&self, _c: &Consulta) -> Option<Digest> {
        None
    }
}

/// Digest of a file on disk; missing = `None`.
pub fn digest_arquivo(p: &Path) -> Option<Digest> {
    std::fs::read(p).ok().map(|b| *blake3::hash(&b).as_bytes())
}

pub fn digest_bytes(b: &[u8]) -> Digest {
    *blake3::hash(b).as_bytes()
}
