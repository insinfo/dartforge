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
//! session calls [`AnalyzedUnitCache::changed`] — which does the scan **in
//! parallel** — or [`AnalyzedUnitCache::invalidate`] when it already knows the
//! file.
//!
//! Condition of use: the same [`dartforge_intern::Interner`] as the previous
//! analyses, because the nodes keep `SymbolId`s from that arena.
use crate::model::{Program, Unit, UnitRole};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Cheap identity of a file: what the `mtime` and the size say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStamp {
    /// Modification time in nanoseconds since the epoch (0 when unavailable).
    pub mtime_ns: u128,
    pub size: u64,
}

impl FileStamp {
    /// Reads the stamp of a path; `None` if the file does not exist.
    pub fn read(path: &Path) -> Option<FileStamp> {
        let md = std::fs::metadata(path).ok()?;
        let mtime_ns = md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        Some(FileStamp {
            mtime_ns,
            size: md.len(),
        })
    }
}

/// Units kept between compilations, by path.
#[derive(Default)]
pub struct AnalyzedUnitCache {
    units: HashMap<PathBuf, Unit>,
    /// The file's stamp when the unit was analyzed.
    marks: HashMap<PathBuf, FileStamp>,
    /// Units reused in the current load.
    pub reused: usize,
    /// The paths of the units of each reclaimed `dart:` library, in order, by
    /// library name: a load serves the whole library from the kept units
    /// ([`AnalyzedUnitCache::take_sdk_library`]) instead of decoding it from
    /// the SDK cache again.
    sdk_libraries: HashMap<String, Vec<PathBuf>>,
}

impl AnalyzedUnitCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.units.len()
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    /// Forgets a unit (edited or deleted file).
    pub fn invalidate(&mut self, path: &Path) {
        self.units.remove(path);
        self.marks.remove(path);
    }

    pub fn clear(&mut self) {
        self.units.clear();
        self.marks.clear();
        self.sdk_libraries.clear();
    }

    /// Takes every kept unit out of the cache, which ends up empty (the
    /// resident's memory measure drops them group by group).
    pub fn take_all(&mut self) -> Vec<Unit> {
        self.marks.clear();
        self.sdk_libraries.clear();
        self.units.drain().map(|(_, u)| u).collect()
    }

    /// Source bytes retained — the share that grows with the project, not with
    /// the number of edits (the `measure` example watches this).
    pub fn source_bytes(&self) -> usize {
        self.units.values().map(|u| u.source.len()).sum()
    }

    /// Resets the load counters (the loader calls this when starting).
    pub fn start_load(&mut self) {
        self.reused = 0;
    }

    /// Kept paths whose file changed (or disappeared) since the analysis.
    ///
    /// It is the `dartforge dev` watcher's scan: one `stat` per unit, **in
    /// parallel** (on Windows each `stat` goes through the antivirus and in
    /// series would cost seconds), without reading content.
    pub fn changed(&self) -> Vec<PathBuf> {
        let paths: Vec<(&PathBuf, FileStamp)> = self.marks.iter().map(|(p, m)| (p, *m)).collect();
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(8);
        if paths.len() < 32 || threads < 2 {
            return paths
                .into_iter()
                .filter(|(p, m)| FileStamp::read(p).is_none_or(|current| current != *m))
                .map(|(p, _)| p.clone())
                .collect();
        }
        let next_index = std::sync::atomic::AtomicUsize::new(0);
        let batches: Vec<Vec<PathBuf>> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|_| {
                    s.spawn(|| {
                        let mut mine = Vec::new();
                        loop {
                            let i = next_index.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some((p, m)) = paths.get(i) else { break };
                            if FileStamp::read(p).is_none_or(|current| current != *m) {
                                mine.push((*p).clone());
                            }
                        }
                        mine
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap_or_default())
                .collect()
        });
        batches.into_iter().flatten().collect()
    }

    /// Kept paths (what the session watches).
    pub fn paths(&self) -> Vec<PathBuf> {
        self.units.keys().cloned().collect()
    }

    /// Whether there is a kept unit for `path` (without touching the disk).
    pub fn contains(&self, path: &Path) -> bool {
        self.units.contains_key(path)
    }

    /// Takes out the unit of `path`, if any. Does not check the disk: what was
    /// invalidated has already left.
    pub(crate) fn take(&mut self, path: &Path) -> Option<Unit> {
        let u = self.units.remove(path);
        if u.is_some() {
            self.reused += 1;
        }
        u
    }

    /// The library `dart:<name>` has all its units kept.
    pub(crate) fn has_sdk_library(&self, name: &str) -> bool {
        self.sdk_libraries
            .get(name)
            .is_some_and(|paths| paths.iter().all(|p| self.units.contains_key(p)))
    }

    /// Takes out the units of `dart:<name>`, in order, when all of them are
    /// kept (the SDK does not change during a session).
    pub(crate) fn take_sdk_library(&mut self, name: &str) -> Option<Vec<Unit>> {
        if !self.has_sdk_library(name) {
            return None;
        }
        let paths = self.sdk_libraries.get(name)?.clone();
        let units: Vec<Unit> = paths.iter().filter_map(|p| self.units.remove(p)).collect();
        self.reused += units.len();
        Some(units)
    }

    /// Records the stamp of a freshly read file (one `stat`, at read time — it
    /// is the only disk query the load makes per new file).
    pub(crate) fn annotate_marker(&mut self, path: &Path) {
        if let Some(m) = FileStamp::read(path) {
            self.marks.insert(path.to_path_buf(), m);
        }
    }

    /// Collects units taken out of a program that stays in use (those after its
    /// frozen prefix, before a load over the prefix).
    pub fn reclaim_units(&mut self, units: Vec<crate::model::Unit>) {
        for u in units {
            let Some(path) = u.path.clone() else { continue };
            self.units.insert(path, u);
        }
    }

    /// Collects the units of a [`Program`] that is no longer used — including
    /// those of the SDK, which do not change between edits either.
    pub fn reclaim(&mut self, program: Program) {
        let Program {
            units, libraries, ..
        } = program;
        for l in &libraries {
            if let Some(name) = l.uri.strip_prefix("dart:") {
                let paths: Option<Vec<PathBuf>> = l
                    .units
                    .iter()
                    .map(|u| units.get(u.0 as usize).and_then(|u| u.path.clone()))
                    .collect();
                if let Some(paths) = paths {
                    self.sdk_libraries.insert(name.to_string(), paths);
                }
            }
        }
        for u in units {
            let Some(path) = u.path.clone() else { continue };
            self.units.insert(path, u);
        }
    }
}

/// Role and URI expected of a reused unit, checked by the loader before
/// accepting it (the same file may come in as a part of another library
/// after a directive edit).
pub(crate) fn serve(u: &Unit, uri: &str, role: UnitRole) -> bool {
    u.uri == uri && u.role == role
}
