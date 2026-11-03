//! Bounded semantic session: the last loaded [`Project`] is reused by the
//! following queries while nothing it read has changed.
//!
//! Without it, every `hover`, `definition`, `references`, `prepareRename` +
//! `rename` and `codeAction` reloads the program (the `dart:` libraries
//! dominate the cost) and infers the bodies again, even when the editor
//! sends several queries on the same text version (VS Code requests
//! `codeAction` at every cursor move, and `rename` comes after
//! `prepareRename`). The memory policy stays explicit:
//!
//! * **one** entry, at most: the retained program is that of one query, not
//!   a cache per file or per version;
//! * the entry drops (the `drop` is immediate) at every `didOpen`,
//!   `didChange` or `didClose` — no tree of an old version stays alive;
//! * before reusing it, the key is checked: the version and size of each
//!   open document, and the date and size of each disk file that took part
//!   in the load (except the SDK and the pub cache, which nothing edits in
//!   place), plus the modification time of each folder of the project (a new
//!   `.dart` file may satisfy an import);
//! * it only retains a program whose total source fits in the budget
//!   (`DARTFORGE_LSP_SESSION_MIB`, in MiB of source; default
//!   [`DEFAULT_BUDGET_MIB`]; `0` turns the session off). The measured live
//!   cost is proportional to the source (`docs/LSP.md`, "Sessão semântica").

use crate::DocumentStore;
use crate::project::{Project, project_dirs};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Default budget, in MiB of loaded source (SDK included). A library's load
/// reads the libraries outside the project without their bodies
/// (`semantic::load`). The program of a component of a large Angular package
/// (`new_sali/frontend`, 2,835 libraries) has grown past 32 MiB: every hover
/// reloaded it (~950 ms), and the transient loads left the server at 830 MiB
/// with the allocator's pages, against 408 MiB with the program kept
/// (docs/LSP.md, "Décima quinta rodada").
pub(crate) const DEFAULT_BUDGET_MIB: usize = 48;

/// Default budget of the wide load of non-local searches, in MiB of source
/// (the analyzer's SDK has ~8 MiB; the project's packages add the rest).
pub(crate) const DEFAULT_WIDE_BUDGET_MIB: usize = 48;

/// What the entry loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Scope {
    /// Only the file's library (definition, hover, actions).
    Library(PathBuf),
    /// All libraries of the project with this root (references, rename).
    Project(PathBuf),
    /// The load of one reference search for `name` in the project with this
    /// root (`load_wide_project` with `citing`).
    Search { root: PathBuf, name: String },
}

/// What must stay the same for the entry to be valid.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Key {
    scope: Scope,
    /// (URI, version, size) of each open document, in order.
    open: Vec<(String, i32, usize)>,
    /// Files read from disk during the load, with (date, size).
    disk: Vec<(PathBuf, Option<(SystemTime, u64)>)>,
    /// The folders of the project at load time, with their modification
    /// times (a file created or removed in one changes it).
    dirs: Vec<(PathBuf, Option<SystemTime>)>,
}

/// Session counters (measurement and tests).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SessionStats {
    /// Queries served by the retained entry.
    pub reused: u64,
    /// Queries that loaded the program.
    pub loaded: u64,
    /// Entries discarded because of a change in the text or on disk.
    pub invalidated: u64,
    /// Loads not retained for exceeding the budget.
    pub above_budget: u64,
    /// Source bytes of the entry retained now (0 without an entry).
    pub retained_source: usize,
    /// Edits of a document applied to the retained entry without reloading
    /// (only bodies changed, docs/LSP-ESPECIFICACAO.md §16.10 I4).
    pub body_swaps: u64,
}

/// The session: at most one retained program.
pub(crate) struct Session {
    entry: Option<(Key, Project)>,
    budget: usize,
    pub(crate) stats: SessionStats,
}

impl Session {
    /// Session with the budget from `DARTFORGE_LSP_SESSION_MIB` (or the default).
    pub(crate) fn new() -> Self {
        let mib = std::env::var("DARTFORGE_LSP_SESSION_MIB")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(DEFAULT_BUDGET_MIB);
        Self::with_budget(mib)
    }

    /// The session of the wide load (the whole SDK and the packages), with the
    /// budget from `DARTFORGE_LSP_WIDE_MIB` (default
    /// [`DEFAULT_WIDE_BUDGET_MIB`]; `0` goes back to loading on every search).
    pub(crate) fn wide() -> Self {
        let mib = std::env::var("DARTFORGE_LSP_WIDE_MIB")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(DEFAULT_WIDE_BUDGET_MIB);
        Self::with_budget(mib)
    }

    /// Session with a budget of `mib` MiB of source (`0`: never retains).
    pub(crate) fn with_budget(mib: usize) -> Self {
        Self {
            entry: None,
            budget: mib.saturating_mul(1024 * 1024),
            stats: SessionStats::default(),
        }
    }

    /// The budget, in bytes of source.
    pub(crate) fn budget(&self) -> usize {
        self.budget
    }

    /// The session holds a program that serves a query on `file`'s library
    /// (its library's entry, or its project's).
    pub(crate) fn holds_library(&self, file: &Path) -> bool {
        self.entry
            .as_ref()
            .is_some_and(|(key, _)| match &key.scope {
                Scope::Library(f) => f == file,
                Scope::Project(root) => file.starts_with(root),
                Scope::Search { .. } => false,
            })
    }

    /// Discards the entry (a text changed). The program drops here, not later.
    pub(crate) fn invalidate(&mut self) {
        if self.entry.take().is_some() {
            self.stats.invalidated += 1;
            self.stats.retained_source = 0;
        }
    }

    /// `didChange` of `uri`: if the retained entry has the unit and the edit
    /// only changed bodies (class Body, docs/LSP-ESPECIFICACAO.md §16.6), swaps
    /// the unit in place, rebuilds its tables and the entry stays valid (§16.10
    /// I4); otherwise the entry drops.
    pub(crate) fn update(&mut self, documents: &DocumentStore, uri: &str) {
        if !self.swap(documents, uri) {
            self.invalidate();
        }
    }

    /// `didOpen` of `uri`: the entry stays valid when only this document joined
    /// the list of open ones and its text is what the load already read from
    /// disk for the unit (opening a file in the editor does not reload the
    /// program); otherwise it drops.
    pub(crate) fn open(&mut self, documents: &DocumentStore, uri: &str) {
        if !self.keep(documents, uri, documents.get(uri).map(str::to_string)) {
            self.invalidate();
        }
    }

    /// `didClose` of `uri` (already out of `documents`): the entry stays valid
    /// when the disk text is what the unit has; otherwise it drops.
    pub(crate) fn close(&mut self, documents: &DocumentStore, uri: &str) {
        let disk = crate::project::uri_file(uri).and_then(|c| std::fs::read_to_string(c).ok());
        if !self.keep(documents, uri, disk) {
            self.invalidate();
            return;
        }
        // The file now comes from disk: changes to it drop the entry.
        if let Some((key, project)) = self.entry.as_mut()
            && let Some(u) = project.uri_unit(uri)
            && let Some(p) = project.program().unit(u).path.clone()
            && !key.disk.iter().any(|(q, _)| *q == p)
        {
            let m = metadata(&p);
            key.disk.push((p, m));
            key.disk.sort();
        }
    }

    /// The retained entry is valid with the current open documents if only
    /// `uri` joined or left the list and its unit has `text` (or it is a file
    /// on disk outside the load).
    fn keep(&mut self, documents: &DocumentStore, uri: &str, text: Option<String>) -> bool {
        let Some((key, project)) = self.entry.as_mut() else {
            return true;
        };
        let Some(text) = text else { return false };
        let new = open(documents);
        let without = |l: &[(String, i32, usize)]| {
            l.iter().filter(|a| a.0 != uri).cloned().collect::<Vec<_>>()
        };
        if without(&new) != without(&key.open) {
            return false;
        }
        let Some(u) = project.uri_unit(uri) else {
            // A file outside the load (another library's, for a library entry):
            // its text took no part in it. One on disk cannot be an import the
            // load failed to resolve either; one that is not may be, and reloads.
            let on_disk = crate::project::uri_file(uri).is_some_and(|p| p.is_file());
            if on_disk {
                key.open = new;
            }
            return on_disk;
        };
        if project.program().unit(u).source != text {
            return false;
        }
        key.open = new;
        true
    }

    fn swap(&mut self, documents: &DocumentStore, uri: &str) -> bool {
        let Some((key, project)) = self.entry.as_mut() else {
            return false;
        };
        let Some(text) = documents.get(uri) else {
            return false;
        };
        let new = open(documents);
        // Only the edited document may have changed.
        if new.len() != key.open.len()
            || new
                .iter()
                .zip(key.open.iter())
                .any(|(a, b)| a.0 != b.0 || (a.0 != uri && a != b))
        {
            return false;
        }
        // A document outside the load may become the owner of a part (`part`):
        // reloads.
        let Some(u) = project.uri_unit(uri) else {
            return false;
        };
        if project.program().unit(u).source != text {
            let inferred_bodies = project
                .libraries
                .contains(&project.program().unit(u).library);
            let mode = crate::query::SwapMode::Final {
                inferred_bodies,
                register_locals: true,
            };
            if project.query.swap_unit(u, text, mode).is_none() {
                return false;
            }
            project.forget_unit(u);
        }
        let source: usize = project.program().units.iter().map(|u| u.source.len()).sum();
        if source > self.budget {
            return false;
        }
        key.open = new;
        self.stats.body_swaps += 1;
        self.stats.retained_source = source;
        true
    }

    /// The project for `scope`: the retained one, if the key is still valid (a
    /// whole-project entry also serves a library query of the same project),
    /// otherwise what `load` returns — retained if it fits in the budget, or
    /// returned only for this query.
    pub(crate) fn get(
        &mut self,
        scope: Scope,
        documents: &DocumentStore,
        load: impl FnOnce() -> Option<Project>,
    ) -> Option<Use<'_>> {
        let open = open(documents);
        let serve = self.entry.as_ref().is_some_and(|(key, _)| {
            let scope_serves = match (&key.scope, &scope) {
                (a, b) if a == b => true,
                (Scope::Project(root), Scope::Library(file)) => file.starts_with(root),
                _ => false,
            };
            scope_serves && key.open == open && disk_equal(key)
        });
        if serve {
            self.stats.reused += 1;
            return self.entry.as_mut().map(|(_, p)| Use::Retained(p));
        }
        self.invalidate();
        let project = load()?;
        self.stats.loaded += 1;
        let source: usize = project.program().units.iter().map(|u| u.source.len()).sum();
        // The load of one search serves only that name again: it is not worth
        // the memory it would keep.
        let single_use = matches!(scope, Scope::Search { .. });
        if self.budget == 0 || source > self.budget || single_use {
            // It does not fit: valid only for this query and dropped with the response.
            self.stats.above_budget += 1;
            return Some(Use::Own(Box::new(project)));
        }
        let key = Key {
            disk: read_files(&project, documents),
            dirs: project_dirs(&project.root),
            scope,
            open,
        };
        self.stats.retained_source = source;
        self.entry = Some((key, project));
        self.entry.as_mut().map(|(_, p)| Use::Retained(p))
    }
}

/// A project borrowed from the session or owned by the query (over the
/// budget), used the same way. Mutable because some queries (signature
/// help) intern substituted types in the program's table; the table only
/// grows while the version is valid and drops with it.
pub(crate) enum Use<'a> {
    Retained(&'a mut Project),
    Own(Box<Project>),
}

impl std::ops::Deref for Use<'_> {
    type Target = Project;

    fn deref(&self) -> &Project {
        match self {
            Use::Retained(p) => p,
            Use::Own(p) => p,
        }
    }
}

impl std::ops::DerefMut for Use<'_> {
    fn deref_mut(&mut self) -> &mut Project {
        match self {
            Use::Retained(p) => p,
            Use::Own(p) => p,
        }
    }
}

/// (URI, version, size) of the open documents, in order.
fn open(documents: &DocumentStore) -> Vec<(String, i32, usize)> {
    let mut v: Vec<(String, i32, usize)> = documents
        .uris()
        .map(|u| {
            (
                u.to_string(),
                documents.version(u).unwrap_or(0),
                documents.get(u).map_or(0, str::len),
            )
        })
        .collect();
    v.sort();
    v
}

/// Modification date and size of a file.
fn metadata(path: &Path) -> Option<(SystemTime, u64)> {
    let m = std::fs::metadata(path).ok()?;
    Some((m.modified().ok()?, m.len()))
}

/// The roots of the pub cache: `PUB_CACHE`, or pub's default for the platform.
fn pub_cache_roots() -> &'static [PathBuf] {
    static ROOTS: std::sync::OnceLock<Vec<PathBuf>> = std::sync::OnceLock::new();
    ROOTS.get_or_init(|| {
        let mut roots = Vec::new();
        if let Some(p) = std::env::var_os("PUB_CACHE") {
            roots.push(PathBuf::from(p));
        } else if cfg!(windows) {
            if let Some(p) = std::env::var_os("LOCALAPPDATA") {
                roots.push(PathBuf::from(p).join("Pub").join("Cache"));
            }
        } else if let Some(p) = std::env::var_os("HOME") {
            roots.push(PathBuf::from(p).join(".pub-cache"));
        }
        roots
            .into_iter()
            .map(|r| dartforge_elements::config::canonicalize(&r).unwrap_or(r))
            .collect()
    })
}

/// A file that no edit is expected to change while the server runs: an SDK
/// library or a package in the pub cache (`hosted/`, `git/`), which pub never
/// rewrites in place and the Dart server does not watch. Checking their dates
/// at every query was most of the cost of a reused query on a large program
/// (thousands of `stat`s).
fn immutable(project: &Project, unit: &dartforge_elements::model::Unit) -> bool {
    if project.program().library(unit.library).is_sdk {
        return true;
    }
    unit.path.as_deref().is_some_and(|p| {
        pub_cache_roots()
            .iter()
            .any(|r| p.starts_with(r.join("hosted")) || p.starts_with(r.join("git")))
    })
}

/// The files the load read from disk (the open ones came from the editor),
/// without the [`immutable`] ones.
fn read_files(
    project: &Project,
    documents: &DocumentStore,
) -> Vec<(PathBuf, Option<(SystemTime, u64)>)> {
    let mut v: Vec<(PathBuf, Option<(SystemTime, u64)>)> = project
        .program()
        .units
        .iter()
        .filter(|u| !immutable(project, u))
        .filter_map(|u| u.path.clone())
        .filter(|p| {
            url::Url::from_file_path(p)
                .ok()
                .is_none_or(|u| documents.get(u.as_str()).is_none())
        })
        .map(|p| {
            let m = metadata(&p);
            (p, m)
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The disk is still as it was at load time.
fn disk_equal(key: &Key) -> bool {
    key.disk.iter().all(|(p, m)| metadata(p) == *m)
        && key
            .dirs
            .iter()
            .all(|(d, m)| std::fs::metadata(d).and_then(|x| x.modified()).ok() == *m)
}
