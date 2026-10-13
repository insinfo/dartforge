//! The **native** macro executor (D4, `docs/MACROS-PROTOCOLO.md` §3): the
//! same *bootstrap* and the same `package_config.json` as the VM executor
//! ([`crate::vm`]), compiled by DartForge's own native backend
//! (`dartforge compile-native`, `emit_native`) into an executable that speaks
//! `dfexec/1` over stdio. No Dart VM is involved.
//!
//! The executable is cached in the work directory, keyed by everything it
//! compiles: the compiler (path, size and date of the executable), the SDK,
//! and the contents of each file the bootstrap program loads outside the SDK
//! — the macro API, the macros' libraries and what they import. If any of
//! them changes, it is compiled again.
use crate::applications::Application;
use crate::executor::DfexecExecutor;
use crate::protocol::CanalDeProcesso;
use std::path::{Path, PathBuf};

/// Who compiles, and where.
#[derive(Debug, Clone)]
pub struct NativeConfig {
    /// The `dartforge` with the native backend (the `nativo` feature).
    pub compiler: PathBuf,
    /// The SDK's `lib/` (the same one `compile-native` receives in `--sdk`).
    pub sdk_lib: PathBuf,
    /// The package root of our API (`packages/macros`).
    pub api: PathBuf,
    /// Where to write the bootstrap, the `package_config.json` and the
    /// executable.
    pub work_dir: PathBuf,
}

/// Writes the bootstrap, compiles (or reuses) the native executable and
/// starts it.
///
/// # Errors
///
/// Writing to the work directory, loading the bootstrap program,
/// compilation (the compiler's message goes into the error) or starting the
/// process.
pub fn start(
    cfg: &NativeConfig,
    apps: &[Application],
    project_package_config: Option<&Path>,
) -> Result<DfexecExecutor<CanalDeProcesso>, String> {
    let exe = compile(cfg, apps, project_package_config)?;
    Ok(DfexecExecutor::new(CanalDeProcesso::iniciar(std::process::Command::new(&exe))?))
}

/// The native executable of the bootstrap for `apps` (compiled now or taken
/// from the cache).
///
/// # Errors
///
/// Those of [`start`], except starting the process.
pub fn compile(
    cfg: &NativeConfig,
    apps: &[Application],
    project_package_config: Option<&Path>,
) -> Result<PathBuf, String> {
    let error = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
    std::fs::create_dir_all(&cfg.work_dir).map_err(|e| error(&cfg.work_dir, e))?;
    let main = cfg.work_dir.join("bootstrap.dart");
    let text = crate::vm::bootstrap(apps);
    if std::fs::read_to_string(&main).ok().as_deref() != Some(text.as_str()) {
        std::fs::write(&main, &text).map_err(|e| error(&main, e))?;
    }
    let pc = cfg.work_dir.join("package_config.json");
    let json = serde_json::to_string_pretty(&crate::vm::package_config(project_package_config, &cfg.api)?)
        .unwrap_or_default();
    if std::fs::read_to_string(&pc).ok().as_deref() != Some(json.as_str()) {
        std::fs::write(&pc, &json).map_err(|e| error(&pc, e))?;
    }
    let key = cache_key(cfg, &main, &pc)?;
    let exe = cfg.work_dir.join(format!("executor-{key}{}", std::env::consts::EXE_SUFFIX));
    if exe.is_file() {
        return Ok(exe);
    }
    let provisional = cfg.work_dir.join(format!("executor-{key}.tmp{}", std::env::consts::EXE_SUFFIX));
    let output = std::process::Command::new(&cfg.compiler)
        .arg("compile-native")
        .arg(&main)
        .arg("-o")
        .arg(&provisional)
        .arg("--packages")
        .arg(&pc)
        .arg("--sdk")
        .arg(&cfg.sdk_lib)
        // Macros belong to language 3.6 (the experiment was dropped from the
        // following versions): the program's current version is the
        // target's, 3.6.
        .arg("--versao-linguagem")
        .arg("3.6")
        .arg("--enable-experiment=macros")
        .output()
        .map_err(|e| format!("could not run {}: {e}", cfg.compiler.display()))?;
    if !output.status.success() || !provisional.is_file() {
        return Err(format!(
            "the native compilation of the macro executor failed:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    std::fs::rename(&provisional, &exe).map_err(|e| error(&exe, e))?;
    // Executables with old keys are of no use anymore.
    if let Ok(ls) = std::fs::read_dir(&cfg.work_dir) {
        for e in ls.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("executor-") && !n.contains(&key) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    Ok(exe)
}

/// The executable's key: the compiler, the SDK and the contents of each file
/// the bootstrap program loads outside the SDK (the load is the one from
/// `elements`, with the macros experiment, the same one `compile-native`
/// does).
fn cache_key(cfg: &NativeConfig, main: &Path, pc: &Path) -> Result<String, String> {
    let mut h = blake3::Hasher::new();
    let meta = std::fs::metadata(&cfg.compiler).map_err(|e| format!("{}: {e}", cfg.compiler.display()))?;
    let date = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    h.update(format!("{}|{}|{date}\n", cfg.compiler.display(), meta.len()).as_bytes());
    h.update(cfg.sdk_lib.to_string_lossy().as_bytes());
    let mut sdk = dartforge_elements::sdk::SdkLayout::load(&cfg.sdk_lib, "vm")?;
    sdk.current_version = dartforge_frontend::LanguageVersion::new(3, 6);
    sdk.experiments.push(dartforge_frontend::Feature::Macros);
    let mut names = dartforge_intern::Interner::new();
    let (program, diags) = dartforge_elements::load::load_lenient(main, &sdk, Some(pc), &mut names);
    if let Some(d) = diags.first() {
        return Err(format!("the macro executor's bootstrap does not load: {d}"));
    }
    let mut files: Vec<&Path> = program
        .units
        .iter()
        .filter(|u| !program.library(u.library).is_sdk)
        .filter_map(|u| u.path.as_deref())
        .collect();
    files.sort();
    files.dedup();
    for a in files {
        let bytes = std::fs::read(a).map_err(|e| format!("{}: {e}", a.display()))?;
        h.update(a.to_string_lossy().as_bytes());
        h.update(b"\0");
        h.update(blake3::hash(&bytes).as_bytes());
    }
    Ok(h.finalize().to_hex()[..16].to_string())
}
