//! What the file system said about a path, kept between loads while the
//! caller knows the disk did not change.
//!
//! A load asks the disk whether each relative import's target is a file,
//! where the package configuration and the options files are, and the date
//! and size of the files it reads. On macOS each of those `stat`s costs a
//! couple of microseconds; on Windows, with the file system filters in the
//! way, tens: a sampling profile of the LSP's resident analysis process on
//! `new_sali` (docs/LSP.md, "Décima sétima rodada") spent ~10% of its time in
//! `std::fs::metadata` alone, ~12 ms per load.
//!
//! The cache is off by default, and then every call goes to the disk. A
//! process that knows when the disk changes (the LSP resident: the editor's
//! watcher reports each change, and the resident scans the disk at least every
//! 30 s) turns it on with [`trust`], drops what changed with [`forget`] and
//! drops everything with [`clear`] before a scan of its own.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// What one `stat` said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry {
    Missing,
    File(crate::unit_cache::FileStamp),
    Dir,
    Other,
}

#[derive(Default)]
struct State {
    trusted: bool,
    entries: HashMap<PathBuf, Entry>,
    canonical: HashMap<PathBuf, Option<PathBuf>>,
}

fn state() -> std::sync::MutexGuard<'static, State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Turns the cache on (`true`) or off (`false`, which also empties it).
///
/// ```
/// dartforge_elements::fs_cache::trust(false);
/// assert!(!dartforge_elements::fs_cache::is_file(std::path::Path::new("/no/such/file.dart")));
/// ```
pub fn trust(on: bool) {
    let mut s = state();
    s.trusted = on;
    if !on {
        s.entries.clear();
        s.canonical.clear();
    }
}

/// Empties the cache (it stays on or off): the caller is about to look at the
/// whole disk again.
pub fn clear() {
    let mut s = state();
    s.entries.clear();
    s.canonical.clear();
}

/// `paths` changed on the disk: the cache forgets everything. A created file
/// may also create its folder or change what another path canonicalizes to,
/// and the caller's spelling of a path (the editor's) need not be the one the
/// loader asked about, so nothing finer is safe. An edit in the editor
/// changes no file on the disk: only a change there costs the next load its
/// `stat`s.
pub fn forget(paths: &[PathBuf]) {
    if !paths.is_empty() {
        clear();
    }
}

fn entry(path: &Path) -> Entry {
    let read = || match std::fs::metadata(path) {
        Err(_) => Entry::Missing,
        Ok(m) if m.is_file() => Entry::File(crate::unit_cache::FileStamp::of(&m)),
        Ok(m) if m.is_dir() => Entry::Dir,
        Ok(_) => Entry::Other,
    };
    {
        let s = state();
        if !s.trusted {
            drop(s);
            return read();
        }
        if let Some(e) = s.entries.get(path) {
            return *e;
        }
    }
    let e = read();
    let mut s = state();
    if s.trusted {
        s.entries.insert(path.to_path_buf(), e);
    }
    e
}

/// [`Path::is_file`], from the cache when it is on.
pub fn is_file(path: &Path) -> bool {
    matches!(entry(path), Entry::File(_))
}

/// [`Path::is_dir`], from the cache when it is on.
pub fn is_dir(path: &Path) -> bool {
    entry(path) == Entry::Dir
}

/// The date and size of the file at `path` (`None` when it is not a file
/// there), from the cache when it is on.
pub fn stamp(path: &Path) -> Option<crate::unit_cache::FileStamp> {
    match entry(path) {
        Entry::File(s) => Some(s),
        _ => None,
    }
}

/// [`crate::config::canonicalize`], from the cache when it is on.
///
/// # Errors
/// That of the canonicalization (a path that does not exist).
pub fn canonicalize(path: &Path) -> std::io::Result<PathBuf> {
    {
        let s = state();
        if s.trusted
            && let Some(c) = s.canonical.get(path)
        {
            return c
                .clone()
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound));
        }
    }
    let c = std::fs::canonicalize(path).map(crate::config::without_verbatim);
    let mut s = state();
    if s.trusted {
        s.canonical
            .insert(path.to_path_buf(), c.as_ref().ok().cloned());
    }
    c
}
