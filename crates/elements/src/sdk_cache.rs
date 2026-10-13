//! Cache of the analyzed SDK: the units (`source + AST`) of each `dart:x`,
//! serialized once into `target/dartforge/sdk-<hash>.bin`.
//!
//! What is saved is reading and parsing ~180 SDK files on every compilation;
//! the outline (`build_outline`) and type resolution still run over the
//! `Program` as before, because they depend on the whole program and cost a
//! few milliseconds.
//!
//! Format: a `postcard` header with the list of interned symbols (in the
//! order the `Interner` created them while analyzing the whole SDK) and, per
//! library, an independent `postcard` blob with the units — only the
//! libraries the program imports are decoded. Each library also carries the
//! `dart:` libraries it imports or exports, so the load can decode, at once
//! and in parallel, the closure of everything it will need.
//!
//! Invalidation: the file name carries a hash of `libraries.json`, of the
//! SDK's `version` file, of the section name (`dartdevc`) and of `ast.rs`
//! itself (any change to the tree changes the hash). A file that does not
//! decode is ignored and rebuilt.
use crate::load::string_lit_value;
use crate::model::UnitRole;
use crate::sdk::SdkLayout;
use dartforge_frontend::ast::{Ast, CompilationUnit, DirectiveKind};
use dartforge_intern::Interner;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Format version; changing it invalidates every cache.
const FORMAT: u32 = 4; // 4: `Ast::local_metadata`; 3: patch parts with the `Patch` role (main had already gone to 2)

/// An already analyzed SDK unit.
#[derive(Serialize, Deserialize)]
pub struct UnitCache {
    pub uri: String,
    pub path: PathBuf,
    pub source: String,
    pub ast: Ast,
    pub unit: CompilationUnit,
    pub role: UnitRole,
}

#[derive(Serialize, Deserialize)]
struct Header<'a> {
    format: u32,
    /// The `Interner`'s texts in `SymbolId` order.
    #[serde(borrow)]
    symbols: Vec<&'a str>,
    /// Library name (`core`) → `postcard` blob of `Vec<UnitCache>` and the
    /// names of the `dart:` libraries its units import or export.
    #[serde(borrow)]
    libraries: Vec<(&'a str, &'a [u8], Vec<&'a str>)>,
}

/// Open cache: the whole file in memory and, on top of it, the ranges of
/// the symbols and of each library's blob (nothing is copied when opening;
/// each blob is decoded when the load asks for it).
pub struct SdkCache {
    bytes: Vec<u8>,
    symbols: Vec<(usize, usize)>,
    blobs: HashMap<String, (usize, usize)>,
    /// `dart:` libraries imported or exported per library (without the prefix).
    dependencies: HashMap<String, Vec<String>>,
    /// Blobs already decoded, taken by the load.
    pub decoded_units: usize,
}

impl SdkCache {
    /// Decodes the header of `bytes` and keeps only ranges.
    fn from_bytes_(bytes: Vec<u8>) -> Option<SdkCache> {
        let base = bytes.as_ptr() as usize;
        let range = |p: *const u8, n: usize| ((p as usize) - base, (p as usize) - base + n);
        let (symbols, blobs, dependencies) = {
            let cab: Header<'_> = postcard::from_bytes(&bytes).ok()?;
            if cab.format != FORMAT {
                return None;
            }
            let symbols = cab.symbols.iter().map(|s| range(s.as_ptr(), s.len())).collect();
            let blobs = cab
                .libraries
                .iter()
                .map(|(name_, b, _)| (name_.to_string(), range(b.as_ptr(), b.len())))
                .collect();
            let dependencies = cab
                .libraries
                .iter()
                .map(|(name_, _, deps)| (name_.to_string(), deps.iter().map(|d| d.to_string()).collect()))
                .collect();
            (symbols, blobs, dependencies)
        };
        Some(SdkCache { bytes, symbols, blobs, dependencies, decoded_units: 0 })
    }
    /// FNV-1a hash of everything that invalidates the cache.
    pub fn hash(sdk: &SdkLayout, target: &str) -> String {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |bytes: &[u8]| {
            for b in bytes {
                h ^= u64::from(*b);
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
            h ^= 0xff;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        };
        mix(&FORMAT.to_le_bytes());
        mix(target.as_bytes());
        // The root goes into the key: the stored units carry each file's absolute
        // path, and two SDKs with the same `libraries.json` in different
        // directories (a simulated test SDK, for example) would produce the same
        // hash with paths that no longer exist.
        mix(sdk.root.to_string_lossy().as_bytes());
        mix(&std::fs::read(sdk.root.join("libraries.json")).unwrap_or_default());
        mix(&std::fs::read(sdk.root.join("../version")).unwrap_or_default());
        // Overlay (`load_with_overlay`): each swap and the substitute's
        // content, in a stable order.
        let mut swaps: Vec<(&PathBuf, &PathBuf)> = sdk.replacements.iter().collect();
        swaps.sort();
        for (original, new_) in swaps {
            mix(original.to_string_lossy().as_bytes());
            mix(&std::fs::read(new_).unwrap_or_default());
        }
        mix(include_str!("../../frontend/src/ast.rs").as_bytes());
        mix(include_str!("../../frontend/src/text.rs").as_bytes());
        mix(include_str!("../../frontend/src/features.rs").as_bytes());
        format!("{h:016x}")
    }

    /// Cache directory: `DARTFORGE_CACHE_DIR`; otherwise the `target/` the
    /// executable runs from (`target/release/dartforge.exe`, tests in
    /// `target/debug/deps/`) plus `dartforge/`; otherwise `target/dartforge` in
    /// the current directory.
    pub fn directory() -> PathBuf {
        if let Some(d) = std::env::var_os("DARTFORGE_CACHE_DIR") {
            return PathBuf::from(d);
        }
        if let Ok(exe) = std::env::current_exe() {
            let mut p = exe.as_path();
            while let Some(parent) = p.parent() {
                if parent.file_name().is_some_and(|n| n == "target") {
                    return parent.join("dartforge");
                }
                p = parent;
            }
        }
        PathBuf::from("target").join("dartforge")
    }

    /// Path of the cache file for this SDK and section.
    pub fn path(sdk: &SdkLayout, target: &str) -> PathBuf {
        Self::directory().join(format!("sdk-{}.bin", Self::hash(sdk, target)))
    }

    /// Reads and decodes the header of `path`; `None` if it does not exist or
    /// does not decode.
    pub fn open(path: &Path) -> Option<SdkCache> {
        let bytes = std::fs::read(path).ok()?;
        Self::from_bytes_(bytes)
    }

    /// Analyzes the whole SDK (every library of `sdk`, with patches and parts)
    /// with a new `Interner` and writes the cache to `path`.
    ///
    /// Returns the cache ready for use (without rereading the file).
    pub fn build_and_write(sdk: &SdkLayout, path: &Path) -> Result<SdkCache, String> {
        let mut interner = Interner::new();
        let mut names_: Vec<&String> = sdk.libraries.keys().collect();
        names_.sort();
        let mut libraries = Vec::with_capacity(names_.len());
        for name_ in names_ {
            let lib = &sdk.libraries[name_];
            let uri = format!("dart:{name_}");
            let mut units = Vec::new();
            let Some(principal) = parse(sdk, &lib.path, &uri, UnitRole::Library, &mut interner)? else {
                // A library without a source file: it does not go into the cache; the
                // normal load emits the diagnostic.
                continue;
            };
            units.push(principal);
            for patch in &lib.patches {
                let patch_uri = format!(
                    "{uri}#patch_{}",
                    patch.file_name().unwrap_or_default().to_string_lossy()
                );
                if let Some(u) = parse(sdk, patch, &patch_uri, UnitRole::Patch, &mut interner)? {
                    units.push(u);
                }
            }
            // Parts in the same order in which `load_lenient` discovers them: walks the
            // units already loaded (main, patches, parts) and appends each one's parts
            // at the end.
            let mut i = 0;
            while i < units.len() {
                let new_ones = parts_of(sdk, &units[i], &mut interner)?;
                units.extend(new_ones);
                i += 1;
            }
            let blob = postcard::to_allocvec(&units).map_err(|e| format!("serializing dart:{name_}: {e}"))?;
            let mut deps: Vec<String> = Vec::new();
            for u in &units {
                for d in &u.unit.directives {
                    let uri = match &d.kind {
                        DirectiveKind::Import { uri, .. } | DirectiveKind::Export { uri, .. } => uri,
                        _ => continue,
                    };
                    if let Some(dep) = string_lit_value(uri).and_then(|v| v.strip_prefix("dart:").map(str::to_string)) {
                        if !deps.contains(&dep) {
                            deps.push(dep);
                        }
                    }
                }
            }
            libraries.push((name_.clone(), blob, deps));
        }
        let bytes = {
            let cab = Header {
                format: FORMAT,
                symbols: interner.texts().collect(),
                libraries: libraries
                    .iter()
                    .map(|(n, b, d)| (n.as_str(), b.as_slice(), d.iter().map(String::as_str).collect()))
                    .collect(),
            };
            postcard::to_allocvec(&cab).map_err(|e| format!("serializing the SDK cache: {e}"))?
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        // Atomic write: another process may be reading the file.
        let tmp = path.with_extension(format!("bin.{}", std::process::id()));
        std::fs::write(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
        if std::fs::rename(&tmp, path).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        Self::from_bytes_(bytes).ok_or_else(|| "freshly written SDK cache does not decode".to_string())
    }

    /// Opens the cache of `sdk` (at [`SdkCache::path`]) or builds and writes
    /// it. `Err` only when not even the SDK could be analyzed.
    pub fn open_or_build(sdk: &SdkLayout, target: &str) -> Result<(SdkCache, bool), String> {
        let path = Self::path(sdk, target);
        if let Some(c) = Self::open(&path) {
            return Ok((c, true));
        }
        Self::build_and_write(sdk, &path).map(|c| (c, false))
    }

    /// Interns the cache's symbols into `interner`, in the original order. Fails
    /// (returns `false`) if `interner` already had other symbols and the ids do
    /// not match — in that case the cache cannot be used in this load.
    pub fn prepare_interner(&self, interner: &mut Interner) -> bool {
        for (i, &(a, b)) in self.symbols.iter().enumerate() {
            // The header decoded these bytes as `&str`: they are UTF-8.
            let text = std::str::from_utf8(&self.bytes[a..b]).unwrap_or("");
            if interner.intern(text).as_u32() as usize != i {
                return false;
            }
        }
        true
    }

    /// Takes out and decodes the units of `dart:<name_>`; `None` if it is not in
    /// the cache or was already taken.
    pub fn take(&mut self, name_: &str) -> Option<Vec<UnitCache>> {
        let (a, b) = self.blobs.remove(name_)?;
        let units: Vec<UnitCache> = postcard::from_bytes(&self.bytes[a..b]).ok()?;
        self.decoded_units += units.len();
        Some(units)
    }

    /// Takes out and decodes at once the libraries of `names_` and the closure
    /// of the `dart:` libraries they import or export (whatever is still in the
    /// cache), in up to 8 threads: the blobs are independent and decoding does
    /// not touch the `Interner`. Those that do not decode are left out — as in
    /// [`SdkCache::take`], the load then reads them from the files. One from
    /// the closure that the load does not ask for (a conditional import resolved
    /// to another) only costs the decoding.
    ///
    /// Decoding serially cost as much as reanalyzing the SDK in the `dev`
    /// profile (unoptimized `serde`: ~120 ms against ~145 ms of reading + parsing
    /// the 21 libraries `dart:core`/`convert`/`math`… pull in), and the cache
    /// performance test became a coin flip on the runner.
    pub fn remove_several(&mut self, names_: &[&str]) -> HashMap<String, Vec<UnitCache>> {
        let mut requests: Vec<(String, (usize, usize))> = Vec::new();
        let mut pending: Vec<String> = names_.iter().map(|n| n.to_string()).collect();
        while let Some(name_) = pending.pop() {
            let Some(r) = self.blobs.remove(&name_) else { continue };
            if let Some(deps) = self.dependencies.get(&name_) {
                pending.extend(deps.iter().filter(|d| self.blobs.contains_key(*d)).cloned());
            }
            requests.push((name_, r));
        }
        // Largest first: the largest blob (`dart:core`) is not left for last.
        requests.sort_by_key(|(_, (a, b))| std::cmp::Reverse(b - a));
        let bytes = &self.bytes;
        let decode = |(a, b): (usize, usize)| postcard::from_bytes::<Vec<UnitCache>>(&bytes[a..b]).ok();
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8).min(requests.len());
        let mut output = HashMap::new();
        if threads < 2 {
            for (name_, r) in requests {
                if let Some(u) = decode(r) {
                    output.insert(name_, u);
                }
            }
        } else {
            let next_index = std::sync::atomic::AtomicUsize::new(0);
            let batches: Vec<Vec<(String, Vec<UnitCache>)>> = std::thread::scope(|s| {
                let handles: Vec<_> = (0..threads)
                    .map(|_| {
                        s.spawn(|| {
                            let mut mine = Vec::new();
                            loop {
                                let i = next_index.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                let Some((name_, r)) = requests.get(i) else { break };
                                if let Some(u) = decode(*r) {
                                    mine.push((name_.clone(), u));
                                }
                            }
                            mine
                        })
                    })
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap_or_default()).collect()
            });
            output.extend(batches.into_iter().flatten());
        }
        self.decoded_units += output.values().map(Vec::len).sum::<usize>();
        output
    }

    pub fn has(&self, name_: &str) -> bool {
        self.blobs.contains_key(name_)
    }

    pub fn libraries(&self) -> usize {
        self.blobs.len()
    }

    pub fn symbols(&self) -> usize {
        self.symbols.len()
    }
}

/// Reads and parses a file; `Ok(None)` if it does not exist, `Err` if it has
/// a syntax error (the official SDK has none: an error here is a parser gap
/// and does not go into the cache).
fn parse(sdk: &SdkLayout, path: &Path, uri: &str, role: UnitRole, interner: &mut Interner) -> Result<Option<UnitCache>, String> {
    // The overlay swaps the content, not the path (`SdkLayout::replacement`).
    let Ok(source) = std::fs::read_to_string(sdk.replacement(path).unwrap_or(path)) else {
        return Ok(None);
    };
    // `dart:*` is always analyzed at the floor (docs/VERSOES-LINGUAGEM.md, D1).
    let parsed = dartforge_frontend::parser::parse_with(&source, interner, dartforge_frontend::LibraryFeatures::floor());
    if let Some(d) = parsed.diagnostics.first() {
        return Err(format!("{}:{}: {}", path.display(), d.span.start, d.message));
    }
    // The same `path` `load_lenient` stores: the `libraries.json` one for a
    // library and a patch, the canonical one for parts (the caller already
    // canonicalized it).
    Ok(Some(UnitCache { uri: uri.to_string(), path: path.to_path_buf(), source, ast: parsed.ast, unit: parsed.unit, role }))
}

/// The parts declared by `part 'x.dart';` in `unit`, parsed.
fn parts_of(sdk: &SdkLayout, unit: &UnitCache, interner: &mut Interner) -> Result<Vec<UnitCache>, String> {
    let mut output = Vec::new();
    for d in &unit.unit.directives {
        let DirectiveKind::Part { uri } = &d.kind else { continue };
        let Some(rel) = string_lit_value(uri) else { continue };
        let path = unit.path.parent().unwrap_or(Path::new(".")).join(&rel);
        // The same lexical normalization as the loader: the path is the key of the
        // resident session's cache and has to match what `load.rs` builds.
        let canonical = crate::load::normalize(&path);
        let part_uri = url::Url::from_file_path(&canonical)
            .map(|u| u.to_string())
            .unwrap_or_else(|_| canonical.to_string_lossy().to_string());
        // A patch part is a patch (see `load.rs`).
        let role_ = if unit.role == UnitRole::Patch { UnitRole::Patch } else { UnitRole::Part };
        if let Some(p) = parse(sdk, &canonical, &part_uri, role_, interner)? {
            output.push(p);
        }
    }
    Ok(output)
}
