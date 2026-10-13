//! The layout of the dartforge distribution: everything the compiler needs
//! at run time, next to the executable itself — dartforge users do not
//! install Rust, LLVM or the Dart SDK.
//!
//! ```text
//! <root>/bin/dartforge[.exe]
//! <root>/lib/dartforge-distribution.json   distribution marker and version
//! <root>/lib/dart-sdk/lib/…                the Dart SDK libraries (source)
//! <root>/lib/native_sdk/…                  the native backend overlay
//! <root>/lib/runtime/…                     the precompiled runtime (staticlib)
//! <root>/lib/llvm/bin/…                    the LLVM linker (and Clang, without the embedded generator)
//! <root>/lib/sysroot/<triple>/…            what linking needs from the system
//! ```
//!
//! In a development tree there is no marker, and each query falls back to
//! the environment variables and the repository (see the callers).

use std::path::{Path, PathBuf};

/// The name of the file that marks the root of a distribution.
pub const MARKER: &str = "dartforge-distribuicao.json";

/// The distribution root of the running executable: `DARTFORGE_HOME`, or
/// the directory above the executable's `bin/` when `lib/` has the
/// [`MARKER`]. `None` in a development tree.
pub fn root() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("DARTFORGE_HOME").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    let exe = std::env::current_exe().ok()?;
    let exe = exe.canonicalize().unwrap_or(exe);
    let root = exe.parent()?.parent()?;
    root.join("lib").join(MARKER).is_file().then(|| root.to_path_buf())
}

/// `<root>/lib/<rel>`, if the distribution exists and so does the path.
pub fn in_lib(rel: &str) -> Option<PathBuf> {
    let p = root()?.join("lib").join(rel);
    p.exists().then_some(p)
}

/// The Dart SDK libraries the distribution carries.
pub fn dart_sdk() -> Option<PathBuf> {
    in_lib("dart-sdk/lib").filter(|p| p.join("libraries.json").is_file())
}

/// The native backend overlay (`native_sdk/`).
pub fn native_overlay() -> Option<PathBuf> {
    in_lib("native_sdk").filter(|p| p.join("libraries.json").is_file())
}

/// An LLVM executable of the distribution (`clang`, `ld.lld`…), with the
/// system's extension.
pub fn llvm_tool(name_: &str) -> Option<PathBuf> {
    let file = if cfg!(windows) { format!("{name_}.exe") } else { name_.to_string() };
    in_lib(&format!("llvm/bin/{file}")).filter(|p| p.is_file())
}

/// The user's cache directory for dartforge: `%LOCALAPPDATA%` on Windows,
/// `~/Library/Caches` on macOS, `$XDG_CACHE_HOME` or `~/.cache` elsewhere.
pub fn user_cache() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| Path::new(&h).join("Library/Caches"))
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".cache")))
    }?;
    Some(base.join("dartforge"))
}
