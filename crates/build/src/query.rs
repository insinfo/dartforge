//! Queries: what an action actually read. An action's fingerprint is the
//! list `(query, digest of the answer)`; revalidating means recomputing only
//! those digests (`docs/BUILD-MOTOR.md` §4). It is the discipline of the
//! official `InputTracker`, with a **semantic** dependency in place of the
//! import closure.
use std::path::{Path, PathBuf};

pub type Digest = [u8; 32];

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Query {
    /// Bytes of a file (natural path; if it is the output of another action,
    /// the in-memory content).
    File(PathBuf),
    /// Existence (`canRead`), including the negative answer.
    Exists(PathBuf),
    /// Sorted list of what matches `pattern` under `dir`.
    Glob { dir: PathBuf, pattern: String },
    /// `BuildStep.findAssets`: graph candidates in the input's package.
    /// The `bool` marks a generated output, whose presence comes from memory,
    /// not from an old support file on disk.
    ActiveGlobs {
        dir: PathBuf,
        pattern: String,
        candidates: Vec<(String, bool)>,
    },
    /// Public API of a library (plus the annotations).
    LibraryApi(String),
    /// Surface of a declaration: signature, annotations, public members.
    Declaration { library: String, name: String },
    /// Term of an engine index (e.g. `("ng.seletor", "x-y")`).
    Index { space: &'static str, term: String },
    /// Conservative: the text of all the library's units.
    LibrarySource(String),
}

impl Query {
    /// File path that, when it changes, may change the answer.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Query::File(p) | Query::Exists(p) => Some(p),
            Query::Glob { dir, .. } | Query::ActiveGlobs { dir, .. } => Some(dir),
            _ => None,
        }
    }

    /// Answered by the semantic database (not by the disk).
    pub fn semantic(&self) -> bool {
        matches!(
            self,
            Query::LibraryApi(_)
                | Query::Declaration { .. }
                | Query::Index { .. }
                | Query::LibrarySource(_)
        )
    }
}

/// Who answers the semantic queries: the `dev` session (over the already
/// loaded `Program` — `BuildStep.resolver` without extra loading) or, in a
/// single pass, [`NoDb`].
pub trait SemanticDb: Sync {
    fn digest(&self, c: &Query) -> Option<Digest>;
}

/// Single pass (`compile-js`, `build`): there is no revalidation, so the
/// semantic queries need no answer.
pub struct NoDb;

impl SemanticDb for NoDb {
    fn digest(&self, _c: &Query) -> Option<Digest> {
        None
    }
}

/// Digest of a file on disk; missing = `None`.
pub fn digest_file(p: &Path) -> Option<Digest> {
    std::fs::read(p).ok().map(|b| *blake3::hash(&b).as_bytes())
}

pub fn digest_bytes(b: &[u8]) -> Digest {
    *blake3::hash(b).as_bytes()
}
