//! The native AOT runtime and the directory of the backend caches.
//!
//! The runtime is the `staticlib` of `crates/runtime_static`, compiled by the
//! `build.rs` of this crate with the build's `cargo` (three variants: the
//! executable one, with the C `main`, the one for the shared library of the
//! source SDK, and the production one, which is the library one with `panic=abort`). It
//! ships with dartforge: in the distribution's `lib/`, or in the build
//! directory in a development tree. Users of dartforge do not compile
//! Rust. The name carries the hash of the runtime source and of
//! `Cargo.lock`: only the runtime of this compiler matches.

use std::path::{Path, PathBuf};

/// Directory of the native backend caches (objects, source SDK):
/// `DARTFORGE_NATIVE_CACHE`; in the distribution, the user cache
/// (`~/.cache/dartforge/native`…); in a development tree,
/// `$CARGO_TARGET_DIR/native_cache` or `target/native_cache` of the repository
/// that compiled this binary.
pub fn native_cache_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("DARTFORGE_NATIVE_CACHE") {
        return PathBuf::from(d);
    }
    if dartforge_elements::distribution::root().is_some()
        && let Some(c) = dartforge_elements::distribution::user_cache()
    {
        return c.join("native");
    }
    if let Some(t) = std::env::var_os("CARGO_TARGET_DIR") {
        return PathBuf::from(t).join("native_cache");
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/native_cache")
}

pub struct RuntimeCache {
    pub lib_path: PathBuf,
}

impl RuntimeCache {
    /// The executable runtime (with the C `main`).
    pub fn get_or_compile() -> Result<Self, String> {
        precompiled_runtime(env!("DARTFORGE_RUNTIME_MAIN")).map(|lib_path| Self { lib_path })
    }

    /// The runtime variant that goes into the DLL of the source SDK (P5c): without the
    /// C `main` (cfg `dartforge_runtime_dll`); the `main` is the executable's.
    pub fn for_dll() -> Result<Self, String> {
        precompiled_runtime(env!("DARTFORGE_RUNTIME_DLL")).map(|lib_path| Self { lib_path })
    }

    /// The variant of [`RuntimeCache::for_dll`] that the production executable
    /// with the source SDK links statically: compiled with `panic=abort`
    /// (`build.rs`, `ABORT`). The DLL one, which the JIT also loads, unwinds.
    pub fn for_production() -> Result<Self, String> {
        precompiled_runtime(env!("DARTFORGE_RUNTIME_PRODUCTION")).map(|lib_path| Self { lib_path })
    }
}

/// Where the distribution keeps the precompiled runtime: `lib/runtime/`
/// (`dartforge_elements::distribution`), or `DARTFORGE_LIB`.
pub fn distribution_lib_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("DARTFORGE_LIB") {
        return Some(PathBuf::from(d));
    }
    dartforge_elements::distribution::in_lib("runtime")
}

/// The precompiled runtime `name`: in the distribution, otherwise in the build
/// directory.
fn precompiled_runtime(name: &str) -> Result<PathBuf, String> {
    let candidates: Vec<PathBuf> = [distribution_lib_dir(), Some(PathBuf::from(env!("DARTFORGE_RUNTIME_BUILD_DIR")))]
        .into_iter()
        .flatten()
        .collect();
    candidates.iter().map(|d| d.join(name)).find(|p| p.is_file()).ok_or_else(|| {
        let location: Vec<String> = candidates.iter().map(|d| d.display().to_string()).collect();
        format!(
            "the runtime {name} is not in the distribution (looked in {}); was this dartforge compiled with DARTFORGE_RUNTIME_NO_PRECOMPILE=1?",
            location.join(", ")
        )
    })
}
