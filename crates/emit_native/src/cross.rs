//! Portable link inputs exported by a compiler built from the same runtime sources.
//! A cross compilation never falls back to the host's runtime or system libraries.
use crate::target::{NativeTarget, Os};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static BUNDLE: OnceLock<PathBuf> = OnceLock::new();

/// Validates and selects a target bundle before compiling.
///
/// ```
/// use dartforge_emit_native::{cross, target::NativeTarget};
/// let empty = tempfile::tempdir().unwrap();
/// assert!(cross::select_bundle(empty.path(), NativeTarget::host()).is_err());
/// ```
///
/// # Errors
/// Rejects a missing/incomplete bundle, a foreign runtime revision, or a target
/// different from the one requested. A process has one immutable selection.
pub fn select_bundle(path: &Path, target: NativeTarget) -> Result<(), String> {
    let root = path
        .canonicalize()
        .map_err(|e| format!("target bundle {}: {e}", path.display()))?;
    let manifest = manifest(&root)?;
    if manifest["schema"] != 1 || manifest["target"] != target.triple() {
        return Err(format!(
            "target bundle {} does not describe {}",
            root.display(),
            target.triple()
        ));
    }
    if manifest["runtime_sources"] != env!("DARTFORGE_RUNTIME_SOURCE_HASH") {
        return Err("target bundle runtime sources do not match this compiler; export it again from the same checkout".into());
    }
    for role in ["main", "dll", "production"] {
        runtime_in(&root, role)?;
    }
    if !root.join("sysroot").is_dir() {
        return Err("target bundle has no sysroot".into());
    }
    BUNDLE
        .set(root)
        .map_err(|_| "target bundle already selected".to_string())
}

fn manifest(root: &Path) -> Result<serde_json::Value, String> {
    let bytes = std::fs::read(root.join("target.json"))
        .map_err(|e| format!("target bundle manifest: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("target bundle manifest: {e}"))
}

fn runtime_in(root: &Path, role: &str) -> Result<PathBuf, String> {
    let manifest = manifest(root)?;
    let name = manifest["runtime"][role]
        .as_str()
        .ok_or_else(|| format!("target bundle has no {role} runtime"))?;
    if Path::new(name).components().count() != 1
        || name.contains(['/', '\\'])
        || name == "."
        || name == ".."
    {
        return Err("target bundle runtime name must be a basename".into());
    }
    let path = root.join("runtime").join(name);
    if !path.is_file() {
        return Err(format!("target runtime missing: {}", path.display()));
    }
    Ok(path)
}

pub(crate) fn sysroot() -> Option<PathBuf> {
    BUNDLE.get().map(|p| p.join("sysroot"))
}

pub(crate) fn runtime(role: &str) -> Result<Option<PathBuf>, String> {
    if let Some(root) = BUNDLE.get() {
        return runtime_in(root, role).map(Some);
    }
    if crate::target::is_cross() {
        return Err(format!(
            "cross compilation to {} requires --target-bundle <directory>; export-target on that system first",
            crate::target::current().triple()
        ));
    }
    Ok(None)
}

/// Exports native runtime libraries and link inputs, without copying a compiler,
/// a host executable linker, the Dart SDK, or any application files.
///
/// # Errors
/// Requires an empty destination and the native link inputs/toolchain. Fails on
/// any copy/generation error. Export always describes the compiler host.
pub fn export(destination: &Path) -> Result<(), String> {
    if crate::target::is_cross() {
        return Err("export-target must run natively on its destination system".into());
    }
    if destination.exists()
        && std::fs::read_dir(destination)
            .map_err(|e| e.to_string())?
            .next()
            .is_some()
    {
        return Err(format!("{} is not empty", destination.display()));
    }
    let runtime_dir = destination.join("runtime");
    let sysroot = destination.join("sysroot");
    std::fs::create_dir_all(&runtime_dir).map_err(|e| e.to_string())?;
    let mut runtimes = serde_json::Map::new();
    for (role, runtime) in [
        ("main", crate::cache::RuntimeCache::get_or_compile()?),
        ("dll", crate::cache::RuntimeCache::for_dll()?),
        ("production", crate::cache::RuntimeCache::for_production()?),
    ] {
        let name = runtime
            .lib_path
            .file_name()
            .ok_or("runtime has no file name")?;
        std::fs::copy(&runtime.lib_path, runtime_dir.join(name)).map_err(|e| e.to_string())?;
        runtimes.insert(role.into(), name.to_string_lossy().into_owned().into());
    }
    let clang = crate::driver::NativeDriverOptions::default().clang;
    match crate::target::target_os() {
        Os::Linux => crate::linker::SysrootLinux::from_system(&clang)?.copy_to(&sysroot)?,
        Os::Windows => crate::linker_windows::generate(
            &sysroot,
            &crate::linker_windows::CrtCompiler::Generator(&crate::generator::Generator::choose(
                &clang,
            )),
        )?,
        Os::MacOs => crate::linker_macos::generate(&sysroot, crate::linker_macos::Arch::host())?,
        Os::Wasm => unreachable!("export-target describes a native compiler host"),
    }
    let value = serde_json::json!({"schema":1, "target":NativeTarget::host().triple(),
        "runtime_sources":env!("DARTFORGE_RUNTIME_SOURCE_HASH"), "runtime":runtimes});
    std::fs::write(
        destination.join("target.json"),
        serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_targets_round_trip_without_initializing_global_state() {
        for (name, arm, os) in [
            ("windows-x64", false, Os::Windows),
            ("linux-x64", false, Os::Linux),
            ("linux-arm64", true, Os::Linux),
            ("macos-x64", false, Os::MacOs),
            ("macos-arm64", true, Os::MacOs),
        ] {
            let target = NativeTarget::parse(name).unwrap();
            assert_eq!(NativeTarget::parse(target.triple()).unwrap(), target);
            assert_eq!(target.os(), os);
            assert_eq!(target.is_arm64(), arm);
        }
        for name in ["wasm32", "wasm32-linear", "wasm32-unknown-unknown"] {
            let target = NativeTarget::parse(name).unwrap();
            assert_eq!(target, NativeTarget::Wasm32);
            assert_eq!(target.os(), Os::Wasm);
            assert_eq!(target.pointer_bytes(), 4);
            assert!(!target.is_arm64());
        }
        assert_eq!(NativeTarget::host().pointer_bytes(), 8);
        for name in [
            "wasm64",
            "windows-arm64",
            "x86_64-pc-windows-gnu",
            "",
            "linux/../../x",
        ] {
            assert!(NativeTarget::parse(name).is_err(), "{name}");
        }
    }

    #[test]
    fn bundles_reject_wrong_revision_target_and_runtime_paths() {
        let dir = tempfile::tempdir().unwrap();
        let host = NativeTarget::host();
        let write = |value: serde_json::Value| {
            std::fs::write(dir.path().join("target.json"), value.to_string()).unwrap()
        };
        write(serde_json::json!({"schema":1,"target":host.triple(),"runtime_sources":"wrong"}));
        assert!(
            select_bundle(dir.path(), host)
                .unwrap_err()
                .contains("runtime sources")
        );
        write(serde_json::json!({"schema":1,"target":"wrong"}));
        assert!(
            select_bundle(dir.path(), host)
                .unwrap_err()
                .contains("does not describe")
        );
        for name in ["../runtime.a", "a/b.lib", "a\\b.lib", ".", ".."] {
            write(serde_json::json!({"schema":1,"target":host.triple(),
                "runtime_sources":env!("DARTFORGE_RUNTIME_SOURCE_HASH"),"runtime":{"main":name}}));
            assert!(
                select_bundle(dir.path(), host)
                    .unwrap_err()
                    .contains("basename"),
                "{name}"
            );
        }
    }
}
