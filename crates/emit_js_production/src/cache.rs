//! On-disk cache of the `dart_sdk.js` index ([`SdkIndex`]).
//!
//! Classifying the runtime — scanning the ~14 thousand declarations,
//! splitting per member, the references and selectors of the ~45 thousand
//! units — depends only on the text of `dart_sdk.js` and on `per_member`, not
//! on the program (`docs/JS-PRODUCAO.md` §1.6, "índice do `dart_sdk.js` em
//! cache"). Every production compilation used to redo it; now it is stored in
//! `<cache>/jsprod-sdk-<key>.bin` and read back.
//!
//! **Key.** A 64-bit FNV-1a hash over: the format version, the whole text of
//! `dart_sdk.js`, `per_member` and the **code** of the classification
//! (`sdk.rs`, `scan.rs`, `reach.rs`, `bundle.rs`, embedded at compile time).
//! Another runtime (another SDK, another DDC build), another configuration or
//! another version of the classification give another key, and the old index
//! is never used for them.
//!
//! **Check on read.** The header repeats the key, the text length and a
//! second hash of the text (another seed); a file that does not decode, is
//! from another version or from another text (name collision) is ignored and
//! rebuilt. Atomic write (temporary file and `rename`), because parallel
//! compilations (`crates/differential` runs four) compete for the same file.
//!
//! **Eviction.** At most [`MAX_ENTRIES`] indexes in the directory: when a new
//! one is written, the oldest go (`docs/PESQUISA-OTIMIZACAO.md` §2.1: no
//! cache grows without an eviction policy).
//!
//! `DARTFORGE_JSPROD_CACHE=0` turns it off (always builds, never writes);
//! `DARTFORGE_CACHE_DIR` changes the directory, as for the Dart SDK cache.

use crate::sdk::{SdkIndex, index};
use std::path::{Path, PathBuf};

/// Format version; changing it invalidates every index.
const FORMAT: u32 = 1;

/// Maximum number of indexes kept in the directory.
pub const MAX_ENTRIES: usize = 4;

/// Where the index came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Read from disk.
    Cache,
    /// Built now (and written, if the cache is on).
    Built,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CacheFile {
    format: u32,
    key: u64,
    text_len: u64,
    /// Hash of the text with another seed: checks that the file belongs to
    /// this text even when the key collides.
    check: u64,
    index: SdkIndex,
}

fn fnv(seed: u64, parts: &[&[u8]]) -> u64 {
    let mut h = seed;
    for p in parts {
        for b in *p {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h ^= 0xff;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// The key of the index of `text` with `per_member`.
pub fn key(text: &str, per_member: bool) -> u64 {
    fnv(
        0xcbf2_9ce4_8422_2325,
        &[
            &FORMAT.to_le_bytes(),
            text.as_bytes(),
            &[u8::from(per_member)],
            include_str!("sdk.rs").as_bytes(),
            include_str!("scan.rs").as_bytes(),
            include_str!("reach.rs").as_bytes(),
            include_str!("bundle.rs").as_bytes(),
        ],
    )
}

fn check_hash(text: &str) -> u64 {
    fnv(0x84222325cbf29ce4, &[text.as_bytes()])
}

/// The cache directory (the same as the Dart SDK cache).
pub fn directory() -> PathBuf {
    dartforge_elements::sdk_cache::SdkCache::directory()
}

/// The index file of `text` with `per_member` in `dir`.
pub fn path(dir: &Path, text: &str, per_member: bool) -> PathBuf {
    dir.join(format!("jsprod-sdk-{:016x}.bin", key(text, per_member)))
}

/// Reads the index from `file`, if it belongs to this `text` with
/// `per_member`.
pub fn read(file: &Path, text: &str, per_member: bool) -> Option<SdkIndex> {
    let bytes = std::fs::read(file).ok()?;
    let a: CacheFile = postcard::from_bytes(&bytes).ok()?;
    let valid = a.format == FORMAT
        && a.key == key(text, per_member)
        && a.text_len == text.len() as u64
        && a.check == check_hash(text);
    valid.then_some(a.index)
}

/// Writes the index of `text` to `dir` and evicts the oldest ones.
///
/// # Errors
///
/// A write or serialization failure; the caller goes on without the cache.
pub fn write(
    dir: &Path,
    text: &str,
    per_member: bool,
    index: &SdkIndex,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let a = CacheFile {
        format: FORMAT,
        key: key(text, per_member),
        text_len: text.len() as u64,
        check: check_hash(text),
        index: index.clone(),
    };
    let bytes = postcard::to_allocvec(&a).map_err(|e| e.to_string())?;
    let destination = path(dir, text, per_member);
    // Temporary file with the process id: two simultaneous compilations do
    // not write to the same file, and `rename` is atomic.
    let tmp = dir.join(format!(
        ".jsprod-sdk-{}-{:016x}.tmp",
        std::process::id(),
        a.key
    ));
    std::fs::write(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &destination).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{}: {e}", destination.display())
    })?;
    evict_old(dir, &destination);
    Ok(destination)
}

/// Keeps at most [`MAX_ENTRIES`] indexes in `dir`, the most recent ones;
/// `keep` (the one just written) never goes.
fn evict_old(dir: &Path, keep: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut indexes: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("jsprod-sdk-") && n.ends_with(".bin"))
        })
        .filter_map(|p| {
            std::fs::metadata(&p)
                .and_then(|m| m.modified())
                .ok()
                .map(|t| (t, p))
        })
        .collect();
    // Most recent first; ties broken by name, so the order does not depend
    // on the file system.
    indexes.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let mut kept = usize::from(indexes.iter().any(|(_, p)| p == keep));
    for (_, p) in indexes {
        if p == keep {
            continue;
        }
        if kept < MAX_ENTRIES {
            kept += 1;
        } else {
            let _ = std::fs::remove_file(&p);
        }
    }
}

/// The index of `text`: from the cache in `dir` when there is a valid one,
/// otherwise built (and written). `dir` `None` turns the cache off.
pub fn load_in(dir: Option<&Path>, text: &str, per_member: bool) -> (SdkIndex, Origin) {
    if let Some(d) = dir
        && let Some(i) = read(&path(d, text, per_member), text, per_member)
    {
        return (i, Origin::Cache);
    }
    let idx = index(text, per_member);
    if let Some(d) = dir
        && let Err(e) = write(d, text, per_member, &idx)
    {
        eprintln!("warning: runtime index cache: {e}");
    }
    (idx, Origin::Built)
}

/// Like [`load_in`], in the default directory, unless
/// `DARTFORGE_JSPROD_CACHE=0`.
pub fn load(text: &str, per_member: bool) -> (SdkIndex, Origin) {
    let enabled = !std::env::var("DARTFORGE_JSPROD_CACHE").is_ok_and(|v| v == "0");
    let dir = enabled.then(directory);
    load_in(dir.as_deref(), text, per_member)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SDK: &str = concat!(
        "var core = Object.create(dart.library);\n",
        "export { dart, core };\n",
        "core.print = function print(o) { dart.dsend(o, \"toString\", []); };\n",
        "core.Morta = class Morta { f() { return 1; } };\n",
    );

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "dartforge-jsprod-cache-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// The second compilation reads the index from disk, and it is the same
    /// one the build would give.
    #[test]
    fn second_read_comes_from_the_cache_and_is_equal() {
        let d = temp_dir("reuse");
        let (a, o1) = load_in(Some(&d), SDK, true);
        let (b, o2) = load_in(Some(&d), SDK, true);
        assert_eq!(o1, Origin::Built);
        assert_eq!(o2, Origin::Cache);
        assert_eq!(a, b);
        assert_eq!(a, index(SDK, true));
        assert_eq!(a.dynamic_selectors(), vec!["toString".to_string()]);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Another runtime or another configuration does not accept the stored
    /// index: neither by the file name nor by forcing a read of the wrong
    /// file.
    #[test]
    fn incompatible_runtime_or_configuration_rebuilds() {
        let d = temp_dir("incompatible");
        let (_, o) = load_in(Some(&d), SDK, true);
        assert_eq!(o, Origin::Built);
        let other = SDK.replace("Morta", "Viva");
        let (i, o) = load_in(Some(&d), &other, true);
        assert_eq!(o, Origin::Built, "another runtime text");
        assert_eq!(i, index(&other, true));
        let (_, o) = load_in(Some(&d), SDK, false);
        assert_eq!(o, Origin::Built, "another granularity");
        // The file of one text, read as if it were another's, is rejected.
        assert!(read(&path(&d, SDK, true), &other, true).is_none());
        assert!(read(&path(&d, SDK, true), SDK, false).is_none());
        assert!(read(&path(&d, SDK, true), SDK, true).is_some());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A corrupted file is ignored and rebuilt.
    #[test]
    fn corrupted_file_rebuilds() {
        let d = temp_dir("corrupted");
        let (_, _) = load_in(Some(&d), SDK, true);
        std::fs::write(path(&d, SDK, true), b"garbage").unwrap();
        let (i, o) = load_in(Some(&d), SDK, true);
        assert_eq!(o, Origin::Built);
        assert_eq!(i, index(SDK, true));
        let (_, o) = load_in(Some(&d), SDK, true);
        assert_eq!(o, Origin::Cache, "rewritten");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The directory never holds more than [`MAX_ENTRIES`] indexes.
    #[test]
    fn evicts_the_oldest() {
        let d = temp_dir("eviction");
        for k in 0..MAX_ENTRIES + 3 {
            let t = format!("{SDK}core.X{k} = class X{k} {{}};\n");
            load_in(Some(&d), &t, true);
        }
        let n = std::fs::read_dir(&d)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with("jsprod-sdk-"))
            .count();
        assert_eq!(n, MAX_ENTRIES);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Pruning with the index read from disk gives the same text as pruning
    /// from scratch.
    #[test]
    fn pruning_through_the_cache_is_identical() {
        let d = temp_dir("prune");
        load_in(Some(&d), SDK, true);
        let (i, o) = load_in(Some(&d), SDK, true);
        assert_eq!(o, Origin::Cache);
        let roots = vec!["core.print".to_string()];
        assert_eq!(
            crate::sdk::prune_with_index(SDK, &i, &roots),
            crate::sdk::prune(SDK, &roots, true)
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
