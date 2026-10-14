//! Explicit location of the required zstd dependency of static LLVM packages.
use std::path::Path;

/// Produces Cargo link directives for a conventional zstd archive/shared library.
/// Invalid or missing overrides fail before the final linker is invoked.
pub fn override_directives(path: &Path) -> Result<Vec<String>, String> {
    if !path.is_file() {
        return Err(format!(
            "DARTFORGE_LLVM_ZSTD_LIBRARY={} is not a library file",
            path.display()
        ));
    }
    let kind = match path.file_name().and_then(|value| value.to_str()) {
        Some("libzstd.a") => "static",
        Some("libzstd.dylib" | "libzstd.so") => "dylib",
        _ => {
            return Err(
                "DARTFORGE_LLVM_ZSTD_LIBRARY must name libzstd.a, libzstd.dylib or libzstd.so"
                    .to_owned(),
            );
        }
    };
    let directory = path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .ok_or_else(|| "DARTFORGE_LLVM_ZSTD_LIBRARY must include its directory".to_owned())?;
    Ok(vec![
        format!("cargo::rerun-if-changed={}", path.display()),
        format!("cargo::rustc-link-search=native={}", directory.display()),
        format!("cargo::rustc-link-lib={kind}=zstd"),
    ])
}
