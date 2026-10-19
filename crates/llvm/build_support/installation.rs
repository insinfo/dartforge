//! Select LLVM without inheriting a machine-specific installation path.
use std::ffi::OsString;
use std::path::PathBuf;

pub fn compatible_version(version: &str) -> bool {
    let mut parts = version.trim().split('.');
    parts.next() == Some("22") && parts.next() == Some("1")
}

pub fn select_prefix(
    preferred: Option<OsString>,
    legacy: Option<OsString>,
    mut probe: impl FnMut(&str, &str) -> Result<String, String>,
) -> Result<PathBuf, String> {
    for value in [preferred, legacy].into_iter().flatten() {
        if !value.is_empty() {
            // Explicit installations are authoritative, even if broken.
            return Ok(PathBuf::from(value));
        }
    }
    // Same name precedence as llvm-sys 221.1.0's PATH discovery.
    for binary in [
        "llvm-config-22",
        "llvm-config22",
        "llvm22-config",
        "llvm-config-22.1",
        "llvm-config221",
        "llvm-config",
    ] {
        let Ok(version) = probe(binary, "--version") else {
            continue;
        };
        if !compatible_version(&version) {
            continue;
        }
        if let Ok(prefix) = probe(binary, "--prefix")
            && !prefix.trim().is_empty()
        {
            return Ok(PathBuf::from(prefix.trim()));
        }
    }
    Err("LLVM 22.1 was not found. Set DARTFORGE_LLVM_DIR (or LLVM_SYS_221_PREFIX) to a complete installation, or put its llvm-config on PATH.".into())
}
