//! Cache of already analyzed units, for a resident session
//! (`dartforge dev`).
//!
//! Between two compilations most files did not change: in `new_sali/core`
//! there are 2,423 units (SDK included) and one edit touches one. By keeping
//! `(source, AST, CompilationUnit)` per path, the next compilation reanalyzes
//! only what was invalidated.
//!
//! **The caller decides what changed**, not the loader: asking the file
//! system for every unit cost one `stat` per file inside the load (2,423 in
//! `new_sali`, 7–20 s of wall clock on Windows with an antivirus). The
//! session calls [`CacheUnidades::alterados`] — which does the scan **in
//! parallel** — or [`CacheUnidades::invalidar`] when it already knows the
//! file.
//!
//! Condition of use: the same [`dartforge_intern::Interner`] as the previous
//! analyses, because the nodes keep `SymbolId`s from that arena.
use crate::model::{Program, Unit, UnitRole};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Cheap identity of a file: what the `mtime` and the size say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Marca {
    /// Modification time in nanoseconds since the epoch (0 when unavailable).
    pub mtime_ns: u128,
    pub tamanho: u64,
}

impl Marca {
    /// Reads the stamp of a path; `None` if the file does not exist.
    pub fn ler(path: &Path) -> Option<Marca> {
        let md = std::fs::metadata(path).ok()?;
        let mtime_ns = md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        Some(Marca { mtime_ns, tamanho: md.len() })
    }
}

/// Units kept between compilations, by path.
#[derive(Default)]
pub struct CacheUnidades {
    unidades: HashMap<PathBuf, Unit>,
    /// The file's stamp when the unit was analyzed.
    marks: HashMap<PathBuf, Marca>,
    /// Units reused in the current load.
    pub reaproveitadas: usize,
}

impl CacheUnidades {
    pub fn nova() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.unidades.len()
    }

    pub fn is_empty(&self) -> bool {
        self.unidades.is_empty()
    }

    /// Forgets a unit (edited or deleted file).
    pub fn invalidar(&mut self, path: &Path) {
        self.unidades.remove(path);
        self.marks.remove(path);
    }

    pub fn limpar(&mut self) {
        self.unidades.clear();
        self.marks.clear();
    }

    /// Source bytes retained — the share that grows with the project, not with
    /// the number of edits (the `medir` example watches this).
    pub fn bytes_fonte(&self) -> usize {
        self.unidades.values().map(|u| u.source.len()).sum()
    }

    /// Resets the load counters (the loader calls this when starting).
    pub fn start_load(&mut self) {
        self.reaproveitadas = 0;
    }

    /// Kept paths whose file changed (or disappeared) since the analysis.
    ///
    /// It is the `dartforge dev` watcher's scan: one `stat` per unit, **in
    /// parallel** (on Windows each `stat` goes through the antivirus and in
    /// series would cost seconds), without reading content.
    pub fn alterados(&self) -> Vec<PathBuf> {
        let caminhos: Vec<(&PathBuf, Marca)> = self.marks.iter().map(|(p, m)| (p, *m)).collect();
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
        if caminhos.len() < 32 || threads < 2 {
            return caminhos
                .into_iter()
                .filter(|(p, m)| Marca::ler(p).is_none_or(|atual| atual != *m))
                .map(|(p, _)| p.clone())
                .collect();
        }
        let proximo = std::sync::atomic::AtomicUsize::new(0);
        let batches: Vec<Vec<PathBuf>> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|_| {
                    s.spawn(|| {
                        let mut mine = Vec::new();
                        loop {
                            let i = proximo.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some((p, m)) = caminhos.get(i) else { break };
                            if Marca::ler(p).is_none_or(|atual| atual != *m) {
                                mine.push((*p).clone());
                            }
                        }
                        mine
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap_or_default()).collect()
        });
        batches.into_iter().flatten().collect()
    }

    /// Kept paths (what the session watches).
    pub fn caminhos(&self) -> Vec<PathBuf> {
        self.unidades.keys().cloned().collect()
    }

    /// Whether there is a kept unit for `path` (without touching the disk).
    pub fn contem(&self, path: &Path) -> bool {
        self.unidades.contains_key(path)
    }

    /// Takes out the unit of `path`, if any. Does not check the disk: what was
    /// invalidated has already left.
    pub(crate) fn take(&mut self, path: &Path) -> Option<Unit> {
        let u = self.unidades.remove(path);
        if u.is_some() {
            self.reaproveitadas += 1;
        }
        u
    }

    /// Records the stamp of a freshly read file (one `stat`, at read time — it
    /// is the only disk query the load makes per new file).
    pub(crate) fn annotate_marker(&mut self, path: &Path) {
        if let Some(m) = Marca::ler(path) {
            self.marks.insert(path.to_path_buf(), m);
        }
    }

    /// Collects the units of a [`Program`] that is no longer used — including
    /// those of the SDK, which do not change between edits either.
    pub fn recolher(&mut self, program: Program) {
        let Program { units, .. } = program;
        for u in units {
            let Some(path) = u.path.clone() else { continue };
            self.unidades.insert(path, u);
        }
    }
}

/// Role and URI expected of a reused unit, checked by the loader before
/// accepting it (the same file may come in as a part of another library
/// after a directive edit).
pub(crate) fn serve(u: &Unit, uri: &str, role: UnitRole) -> bool {
    u.uri == uri && u.role == role
}
