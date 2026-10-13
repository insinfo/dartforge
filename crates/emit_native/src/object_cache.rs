//! Object cache: the `.obj` that Clang produces from an LLVM IR, by hash.
//!
//! It is the discipline of the runtime cache (`cache.rs`) applied to the program:
//! same input, same key, same `.obj`, and Clang does not run again
//! (PESQUISA-OTIMIZACAO §6 and §11). The key is the whole IR plus everything that
//! changes the object without changing the IR: the identity of Clang (the full
//! `clang --version`), the flags, the DartForge version and that of the format of this cache.
//!
//! Today the module is ONE per program, with the SDK bodies inside and global
//! function and class indices, so the cache unit is the whole
//! program: it hits when the same program repeats (the harness passes, a
//! new pass after changing only the runtime or the harness, local iteration) and
//! misses on any change of the emitter. A per-module cache, with the SDK
//! shared between programs, depends on splitting the SDK into its own module
//! with stable symbols (ESTADO.md §2.5).
//!
//! Disk: `<native_cache_dir>/obj/<2 hex>/<32 hex>.obj`. The object only appears
//! with its whole final name (`rename` of a temporary), it is never
//! overwritten, and pruning deletes the oldest by usage date when the total
//! exceeds the cap — never an object this process obtained (the link will still
//! read it) nor one used less than [`MIN_AGE_TO_PRUNE`] ago (that of another
//! process still linking). A program larger than the cap (the builders
//! runner, with `analyzer`) stays in the cache until the next pruning after that.

use crate::cache::native_cache_dir;
use crate::resumo::Fnv128;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

/// Changes when what goes into the cache changes meaning (implicit flags,
/// shape of the file name): invalidates everything written before.
pub const FORMAT_VERSION: u32 = 1;

/// Default cap of the disk cache; `DARTFORGE_CACHE_OBJ_MB` adjusts it. It fits the
/// parts of a large program (new_sali/backend: ~60 objects, ~600 MB,
/// docs/NATIVO-PROJETOS-REAIS.md C9) — with 256 MB pruning removed the parts
/// and recompilation never found them.
pub const DEFAULT_CAP_MB: u64 = 2048;

/// Every how many insertions the pruning runs again.
const PRUNE_EVERY: usize = 16;

/// Temporaries older than this are leftovers of a process that died.
const ORPHAN_TEMP_AGE: Duration = Duration::from_secs(3600);

/// An object used less than this ago is not pruned: another process may have
/// just obtained it and not linked yet (the link of a large program
/// takes minutes).
pub const MIN_AGE_TO_PRUNE: Duration = Duration::from_secs(10 * 60);

/// The whole `clang --version` (version, target, thread model, installation
/// directory), once per Clang path and per process.
pub fn clang_identity(clang: &Path) -> Result<String, String> {
    static IDENTITIES: OnceLock<Mutex<HashMap<PathBuf, String>>> = OnceLock::new();
    let map = IDENTITIES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(id) = map.lock().unwrap().get(clang) {
        return Ok(id.clone());
    }
    let exit = std::process::Command::new(clang)
        .arg("--version")
        .output()
        .map_err(|e| format!("falha ao executar {} --version: {e}", clang.display()))?;
    if !exit.status.success() {
        return Err(format!("{} --version falhou (código {:?})", clang.display(), exit.status));
    }
    let id = String::from_utf8_lossy(&exit.stdout).into_owned();
    map.lock().unwrap().insert(clang.to_path_buf(), id.clone());
    Ok(id)
}

/// The key of an object. Each component is separated by `\0`, so that two
/// different splits of the same bytes do not collide.
pub fn key(ir: &str, clang_id: &str, args: &[&str]) -> u128 {
    bytes_key(ir.as_bytes(), clang_id, args)
}

/// Like [`key`], for an input that is not text (the bitcode of a part
/// of the distributed ThinLTO).
pub fn bytes_key(ir: &[u8], clang_id: &str, args: &[&str]) -> u128 {
    let mut h = Fnv128::default();
    h.write(b"dartforge-obj\0")
        .write(&FORMAT_VERSION.to_le_bytes())
        .write(env!("CARGO_PKG_VERSION").as_bytes())
        .write(b"\0")
        .write(clang_id.as_bytes())
        .write(b"\0");
    for a in args {
        h.write(a.as_bytes()).write(b"\0");
    }
    h.write(ir);
    h.end()
}

/// The object cache of a directory.
#[derive(Debug)]
pub struct ObjectCache {
    dir: PathBuf,
    cap_bytes: u64,
    /// Keys being created in this process: a second thread with the same key
    /// waits for the first instead of running another Clang.
    in_progress: Mutex<HashSet<u128>>,
    finished: Condvar,
    insertions: AtomicUsize,
    tmp_counter: AtomicUsize,
    /// Objects this process obtained: pruning does not delete them.
    in_use: Mutex<HashSet<PathBuf>>,
}

impl ObjectCache {
    /// A cache in `dir` (the `obj/` directory lives inside it).
    pub fn new(dir: PathBuf, cap_bytes: u64) -> ObjectCache {
        let c = ObjectCache {
            dir: dir.join("obj"),
            cap_bytes,
            in_progress: Mutex::new(HashSet::new()),
            finished: Condvar::new(),
            insertions: AtomicUsize::new(0),
            tmp_counter: AtomicUsize::new(0),
            in_use: Mutex::new(HashSet::new()),
        };
        c.prune();
        c
    }

    /// The process cache in [`native_cache_dir`], or `None` with
    /// `DARTFORGE_CACHE_OBJ=0`. The cap comes from `DARTFORGE_CACHE_OBJ_MB`.
    pub fn from_env() -> Option<&'static ObjectCache> {
        static CACHE: OnceLock<Option<ObjectCache>> = OnceLock::new();
        CACHE
            .get_or_init(|| {
                if std::env::var("DARTFORGE_CACHE_OBJ").is_ok_and(|v| v.trim() == "0") {
                    return None;
                }
                let mb = std::env::var("DARTFORGE_CACHE_OBJ_MB")
                    .ok()
                    .and_then(|v| v.trim().parse::<u64>().ok())
                    .unwrap_or(DEFAULT_CAP_MB);
                Some(ObjectCache::new(native_cache_dir(), mb * 1024 * 1024))
            })
            .as_ref()
    }

    pub fn path(&self, key: u128) -> PathBuf {
        let hex = format!("{key:032x}");
        self.dir.join(&hex[..2]).join(format!("{hex}.obj"))
    }

    /// Returns the object of `key`, creating it with `create` if it does not exist yet.
    ///
    /// `create` receives the temporary path where it must write the object, inside
    /// a directory of its own (where it may leave whatever it wants, like the `.ll`:
    /// the whole directory is deleted afterwards). The object is only published if
    /// `create` succeeded and wrote something. The `bool` says whether it was a hit.
    pub fn get_or_create(
        &self,
        key: u128,
        create: impl FnOnce(&Path) -> Result<(), String>,
    ) -> Result<(PathBuf, bool), String> {
        let final_ = self.path(key);
        {
            let mut progress = self.in_progress.lock().unwrap();
            loop {
                if whole_exists(&final_) {
                    drop(progress);
                    touch(&final_);
                    self.in_use.lock().unwrap_or_else(|e| e.into_inner()).insert(final_.clone());
                    return Ok((final_, true));
                }
                if progress.insert(key) {
                    break;
                }
                progress = self.finished.wait(progress).unwrap();
            }
        }
        // From here to the end the key is ours; the guard gives it back even on panic.
        let _guard = InProgress { cache: self, key };

        let hex = format!("{key:032x}");
        let n = self.tmp_counter.fetch_add(1, Ordering::Relaxed);
        let dir_tmp = self.dir.join("tmp").join(format!("{hex}.{}.{n}", std::process::id()));
        std::fs::create_dir_all(&dir_tmp)
            .map_err(|e| format!("não foi possível criar {}: {e}", dir_tmp.display()))?;
        let tmp = dir_tmp.join(format!("{hex}.obj"));
        let r = create(&tmp).and_then(|()| self.publish(&tmp, &final_));
        let _ = std::fs::remove_dir_all(&dir_tmp);
        let hit = r?;
        self.in_use.lock().unwrap_or_else(|e| e.into_inner()).insert(final_.clone());
        if !hit && self.insertions.fetch_add(1, Ordering::Relaxed) % PRUNE_EVERY == PRUNE_EVERY - 1 {
            self.prune();
        }
        Ok((final_, hit))
    }

    /// Moves the temporary to the final name. `Ok(true)` when another process
    /// published the same key first (its object counts, which has the same content).
    fn publish(&self, tmp: &Path, final_: &Path) -> Result<bool, String> {
        if !std::fs::metadata(tmp).is_ok_and(|m| m.len() > 0) {
            return Err(format!("o objeto {} não foi escrito", tmp.display()));
        }
        if let Some(parent) = final_.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("não foi possível criar {}: {e}", parent.display()))?;
        }
        if whole_exists(final_) {
            return Ok(true);
        }
        match std::fs::rename(tmp, final_) {
            Ok(()) => Ok(false),
            Err(_) if whole_exists(final_) => Ok(true),
            Err(e) => Err(format!("não foi possível publicar {}: {e}", final_.display())),
        }
    }

    /// Removes an entry (an object the linker rejected).
    pub fn remove(&self, key: u128) {
        let _ = std::fs::remove_file(self.path(key));
    }

    /// Deletes the least used objects until the total fits the cap, and orphaned
    /// temporaries of processes that died.
    pub fn prune(&self) {
        let now = SystemTime::now();
        if let Ok(tmps) = std::fs::read_dir(self.dir.join("tmp")) {
            for t in tmps.flatten() {
                let old_one = t
                    .metadata()
                    .and_then(|m| m.modified())
                    .is_ok_and(|m| now.duration_since(m).is_ok_and(|d| d > ORPHAN_TEMP_AGE));
                if old_one {
                    let _ = std::fs::remove_dir_all(t.path());
                }
            }
        }
        let mut objects: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
        let in_use = self.in_use.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let Ok(subdirs) = std::fs::read_dir(&self.dir) else { return };
        for sub in subdirs.flatten() {
            if sub.file_name() == "tmp" {
                continue;
            }
            let Ok(files) = std::fs::read_dir(sub.path()) else { continue };
            for a in files.flatten() {
                let p = a.path();
                if p.extension().is_some_and(|e| e == "obj")
                    && let Ok(m) = a.metadata()
                {
                    objects.push((m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len(), p));
                }
            }
        }
        let mut total: u64 = objects.iter().map(|o| o.1).sum();
        if total <= self.cap_bytes {
            return;
        }
        objects.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.2.cmp(&b.2)));
        for (data, size, p) in objects {
            if total <= self.cap_bytes {
                break;
            }
            let recent = now.duration_since(data).map_or(true, |d| d < MIN_AGE_TO_PRUNE);
            if recent || in_use.contains(&p) {
                continue;
            }
            if std::fs::remove_file(&p).is_ok() {
                total -= size;
            }
        }
    }
}

/// Returns the in-progress key and wakes whoever waits for it.
struct InProgress<'a> {
    cache: &'a ObjectCache,
    key: u128,
}

impl Drop for InProgress<'_> {
    fn drop(&mut self) {
        self.cache.in_progress.lock().unwrap_or_else(|e| e.into_inner()).remove(&self.key);
        self.cache.finished.notify_all();
    }
}

fn whole_exists(p: &Path) -> bool {
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() > 0)
}

/// Marks the use of an object (pruning deletes the least used first). Fails
/// silently: the object may be open by another process's linker.
fn touch(p: &Path) {
    if let Ok(f) = std::fs::File::options().write(true).open(p) {
        let _ = f.set_modified(SystemTime::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(content: &'static [u8]) -> impl FnOnce(&Path) -> Result<(), String> {
        move |p: &Path| std::fs::write(p, content).map_err(|e| e.to_string())
    }

    fn files_in(dir: &Path) -> Vec<PathBuf> {
        let mut v = Vec::new();
        if let Ok(es) = std::fs::read_dir(dir) {
            for e in es.flatten() {
                let p = e.path();
                if p.is_dir() {
                    v.extend(files_in(&p));
                } else {
                    v.push(p);
                }
            }
        }
        v
    }

    #[test]
    fn key_changes_with_each_component_and_is_stable() {
        let base = key("ir", "clang 22", &["-c", "-O0"]);
        assert_eq!(base, key("ir", "clang 22", &["-c", "-O0"]));
        assert_ne!(base, key("ir2", "clang 22", &["-c", "-O0"]));
        assert_ne!(base, key("ir", "clang 23", &["-c", "-O0"]));
        assert_ne!(base, key("ir", "clang 22", &["-c", "-O2"]));
        assert_ne!(base, key("ir", "clang 22", &["-c"]));
        // The separation between components counts: moving a byte from one to another
        // cannot give the same key.
        assert_ne!(key("ir", "clang 22", &["-c-O0"]), key("ir", "clang 22", &["-c", "-O0"]));
    }

    #[test]
    fn hit_does_not_call_the_closer() {
        let dir = tempfile::tempdir().unwrap();
        let c = ObjectCache::new(dir.path().to_path_buf(), u64::MAX);
        let (p, hit) = c.get_or_create(7, write(b"obj")).unwrap();
        assert!(!hit);
        assert_eq!(std::fs::read(&p).unwrap(), b"obj");
        let (p2, hit) = c.get_or_create(7, |_| panic!("não devia criar de novo")).unwrap();
        assert!(hit);
        assert_eq!(p, p2);
    }

    #[test]
    fn eight_threads_same_key_one_clang() {
        let dir = tempfile::tempdir().unwrap();
        let c = ObjectCache::new(dir.path().to_path_buf(), u64::MAX);
        let calls = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    c.get_or_create(42, |p| {
                        calls.fetch_add(1, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(20));
                        std::fs::write(p, b"objeto").map_err(|e| e.to_string())
                    })
                    .unwrap()
                });
            }
        });
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let files = files_in(&dir.path().join("obj"));
        assert_eq!(files, vec![c.path(42)], "sobrou temporário: {files:?}");
    }

    #[test]
    fn failure_does_not_publish() {
        let dir = tempfile::tempdir().unwrap();
        let c = ObjectCache::new(dir.path().to_path_buf(), u64::MAX);
        assert!(c.get_or_create(1, |_| Err("clang falhou".into())).is_err());
        // Success without writing anything does not publish either.
        assert!(c.get_or_create(1, |_| Ok(())).is_err());
        assert!(c.get_or_create(1, write(b"")).is_err());
        assert!(files_in(&dir.path().join("obj")).is_empty());
        // And the key does not stay locked: the next attempt creates it.
        assert!(!c.get_or_create(1, write(b"x")).unwrap().1);
    }

    #[test]
    fn prune_respects_cap() {
        let dir = tempfile::tempdir().unwrap();
        let c = ObjectCache::new(dir.path().to_path_buf(), 25);
        for k in 0..5u128 {
            c.get_or_create(k, write(b"0123456789")).unwrap();
            // Distinct dates, so that the usage order is the creation order.
            let f = std::fs::File::options().write(true).open(c.path(k)).unwrap();
            f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1000 + k as u64)).unwrap();
        }
        // The pruning of another process (the objects of this one are in use).
        ObjectCache::new(dir.path().to_path_buf(), 25).prune();
        let remaining = files_in(&dir.path().join("obj"));
        let total: u64 = remaining.iter().map(|p| std::fs::metadata(p).unwrap().len()).sum();
        assert!(total <= 25, "{total} bytes");
        // The last used ones stay.
        assert_eq!(remaining.len(), 2);
        assert!(remaining.contains(&c.path(3)) && remaining.contains(&c.path(4)), "{remaining:?}");
    }

    /// An object larger than the cap, obtained by this process or used
    /// recently by another, survives pruning: the link will still read it.
    #[test]
    fn prune_spares_what_is_in_use() {
        let dir = tempfile::tempdir().unwrap();
        let c = ObjectCache::new(dir.path().to_path_buf(), 5);
        c.get_or_create(1, write(b"0123456789")).unwrap();
        let f = std::fs::File::options().write(true).open(c.path(1)).unwrap();
        f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1000)).unwrap();
        c.prune();
        assert!(c.path(1).is_file(), "o objeto deste processo foi podado");
        // Another process (a new cache over the same directory): the old one
        // goes, the recent one stays.
        let other = ObjectCache::new(dir.path().to_path_buf(), 5);
        assert!(!c.path(1).is_file());
        c.get_or_create(2, write(b"0123456789")).unwrap();
        other.prune();
        assert!(c.path(2).is_file(), "objeto recente de outro processo podado");
    }
}
