//! The **native** builder executor (B01, `docs/BUILD-RUST.md` Phase 4): the
//! same *bootstrap* and the same `package_config.json` as the VM executor
//! ([`crate::vm`]), compiled by DartForge's own native backend
//! (`dartforge compile-native`, `emit_native`) into an executable that speaks
//! `dfexec/1` over stdio and serves the `build.*` service with the Dart side
//! of `packages/build_executor`. No Dart VM, no `dart` and no `build_runner`
//! are on the path: the process is born at the session's first Dart action
//! and stays warm until [`DartExecutor::shutdown`], like the VM one.
//!
//! The executable is cached in `.dart_tool/dartforge/build/` (the same
//! directory as the VM bootstrap: the plan's relative imports work without
//! rewriting), as `executor-<key>`, keyed by the script, the executor
//! sources, the `package_config.json` and the compiler (path, size and
//! date). Next to it, a *depfile* lists every file outside the SDK that the
//! bootstrap program loads (the equivalent of `dart compile kernel`'s
//! `--depfile`): if one of them is newer than the executable, or gone, it is
//! compiled again. It is also the builders' code that the engine records
//! ([`DartExecutor::code`]).
//!
//! Before compiling, the host checks that the project's `package_config.json`
//! resolves the packages the executor needs ([`DEPENDENCIES`]); the missing
//! ones (the project does not need to use `build_runner` or
//! `build_resolvers`, DF-BUILD-004) come from the pub cache
//! ([`crate::executor_deps`]), and what the cache lacks is diagnosed
//! by name.
use crate::client::BuildClient;
use crate::executor_deps::DEPENDENCIES;
use crate::executor::{
    Availability, ExecutorError, DartExecutor, RuntimeExtensions, ActionRequest, ExtensionsRequest,
    ActionResult, BuilderScript, BuildStepService,
};
use crate::vm::{WrittenBootstrap, depfile_deps, write_bootstrap};
use dartforge_dfexec::ProcessChannel;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// Environment variable of the native executor: `0` turns it off where it
/// would be the default (`dartforge build`); any other value also turns it on
/// in the compilers.
pub const ENV_VAR: &str = "DARTFORGE_BUILD_NATIVE";

/// Who compiles, and where.
#[derive(Debug, Clone)]
pub struct NativeConfig {
    /// The `dartforge` with the native backend (the CLI's `native` feature).
    pub compiler: PathBuf,
    /// The SDK's `lib/` for `compile-native` (`--sdk`); `None` uses the one
    /// the compiler discovers.
    pub sdk_lib: Option<PathBuf>,
    /// Directory of the root package: the process runs in it, like
    /// `build_runner`.
    pub root: PathBuf,
    /// The project's `package_config.json`.
    pub package_config: PathBuf,
    /// Where to write the bootstrap, the executor package and the executable.
    pub work_dir: PathBuf,
}

impl NativeConfig {
    /// A project's default configuration: `.dart_tool/package_config.json`
    /// and the work directory in `.dart_tool/dartforge/build` (the same depth
    /// as the official `.dart_tool/build/entrypoint`, like the VM).
    ///
    /// ```
    /// use dartforge_build::native_executor::NativeConfig;
    /// use std::path::Path;
    /// let c = NativeConfig::for_project("dartforge".into(), None, Path::new("/p"));
    /// assert!(c.work_dir.ends_with(".dart_tool/dartforge/build"));
    /// ```
    pub fn for_project(compiler: PathBuf, sdk_lib: Option<PathBuf>, root: &Path) -> NativeConfig {
        NativeConfig {
            compiler,
            sdk_lib,
            root: root.to_path_buf(),
            package_config: root.join(".dart_tool").join("package_config.json"),
            work_dir: root.join(".dart_tool").join("dartforge").join("build"),
        }
    }
}

/// The packages of [`DEPENDENCIES`] that `package_config.json` does not
/// resolve.
///
/// # Errors
///
/// The file cannot be read or is not a `package_config`.
pub fn missing_dependencies(package_config: &Path) -> Result<Vec<&'static str>, String> {
    let text = std::fs::read_to_string(package_config)
        .map_err(|e| format!("{}: {e}", package_config.display()))?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", package_config.display()))?;
    let names: Vec<&str> = v["packages"]
        .as_array()
        .ok_or_else(|| format!("{}: no 'packages'", package_config.display()))?
        .iter()
        .filter_map(|p| p["name"].as_str())
        .collect();
    Ok(DEPENDENCIES
        .iter()
        .map(|(n, _)| *n)
        .filter(|d| !names.contains(d))
        .collect())
}

/// Modification time, if the file exists.
fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// Is the executable still valid? It exists, and no file that the bootstrap
/// program loads is newer than it (or gone).
fn executable_is_valid(exe: &Path, depfile: &Path) -> bool {
    let (Some(t), Ok(text)) = (mtime(exe), std::fs::read_to_string(depfile)) else {
        return false;
    };
    let deps = depfile_deps(&text);
    !deps.is_empty() && deps.iter().all(|d| mtime(d).is_some_and(|m| m <= t))
}

/// The `libdfsdk_…` names (the source SDK library) that an executable's bytes
/// mention.
fn referenced_libraries(bytes: &[u8]) -> Vec<String> {
    let marker = b"libdfsdk_";
    let mut v = Vec::new();
    let mut i = 0;
    while let Some(p) = bytes[i..].windows(marker.len()).position(|w| w == marker) {
        let start = i + p;
        let end = bytes[start..]
            .iter()
            .position(|b| !(b.is_ascii_alphanumeric() || *b == b'_' || *b == b'.'))
            .map_or(bytes.len(), |k| start + k);
        let name = String::from_utf8_lossy(&bytes[start..end]).to_string();
        if !v.contains(&name) {
            v.push(name);
        }
        i = end.max(start + 1);
    }
    v
}

/// Writes a Ninja depfile (`output: dep…`, with spaces escaped by `\`).
fn depfile(output: &Path, deps: &[PathBuf]) -> String {
    let esc = |p: &Path| {
        p.to_string_lossy()
            .replace('\\', "\\\\")
            .replace(' ', "\\ ")
    };
    let mut s = format!("{}:", esc(output));
    for d in deps {
        s.push(' ');
        s.push_str(&esc(d));
    }
    s.push('\n');
    s
}

/// The files outside the SDK that the bootstrap program loads (the same
/// loading as `compile-native`).
fn program_files(
    main: &Path,
    pc: &Path,
    sdk_lib: Option<&Path>,
) -> Result<Vec<PathBuf>, String> {
    let dir = sdk_lib
        .map(Path::to_path_buf)
        .or_else(dartforge_elements::sdk::SdkLayout::discover)
        .ok_or(
            "Dart SDK not found: set DARTFORGE_SDK_LIB (the SDK's lib/) or DART_SDK",
        )?;
    let sdk = dartforge_elements::sdk::SdkLayout::load(&dir, "vm")?;
    let mut names = dartforge_intern::Interner::new();
    let (program, _) =
        dartforge_elements::load::load_lenient(main, &sdk, Some(pc), &mut names);
    let mut v: Vec<PathBuf> = program
        .units
        .iter()
        .filter(|u| !program.library(u.library).is_sdk)
        .filter_map(|u| u.path.clone())
        .collect();
    v.push(pc.to_path_buf());
    v.sort();
    v.dedup();
    Ok(v)
}

/// What the native executor measured (for the report and the tests).
#[derive(Debug, Clone, Default)]
pub struct NativeMeasurement {
    /// Time of `compile-native` (zero when the executable was reused).
    pub compilation: Duration,
    /// From the start of the process to `build.loaded`.
    pub startup: Duration,
    pub executable_reused: bool,
}

/// The native executor: compiles (or reuses) the executable at the session's
/// first Dart action, creates the process and keeps it warm.
pub struct NativeExecutor {
    cfg: NativeConfig,
    client: Option<BuildClient<ProcessChannel>>,
    /// Key of the executable that the current process runs.
    key: Option<String>,
    failure: Option<String>,
    pub measurement: NativeMeasurement,
}

impl NativeExecutor {
    pub fn new(cfg: NativeConfig) -> NativeExecutor {
        NativeExecutor {
            cfg,
            client: None,
            key: None,
            failure: None,
            measurement: NativeMeasurement::default(),
        }
    }

    /// The executable of `script`'s bootstrap, compiled now or taken from
    /// the cache, and its key.
    ///
    /// # Errors
    ///
    /// Executor dependencies that the project does not resolve, writing to
    /// the work directory, or the compilation (the compiler's message goes
    /// into the error).
    pub fn compile(&mut self, script: &BuilderScript) -> Result<(PathBuf, String), String> {
        let t = &self.cfg.work_dir;
        std::fs::create_dir_all(t).map_err(|e| format!("{}: {e}", t.display()))?;
        let project_config = self.complete_package_config()?;
        let WrittenBootstrap {
            principal: main,
            package_config: pc,
            mut hasher,
        } = write_bootstrap(t, &project_config, script)?;
        let meta = std::fs::metadata(&self.cfg.compiler)
            .map_err(|e| format!("{}: {e}", self.cfg.compiler.display()))?;
        let date = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        hasher.update(
            format!("{}|{}|{date}\n", self.cfg.compiler.display(), meta.len()).as_bytes(),
        );
        if let Some(s) = &self.cfg.sdk_lib {
            hasher.update(s.to_string_lossy().as_bytes());
        }
        let key = hasher.finalize().to_hex()[..16].to_string();
        let exe = t.join(format!("executor-{key}{}", std::env::consts::EXE_SUFFIX));
        let dep = t.join(format!("executor-{key}.d"));
        self.measurement = NativeMeasurement::default();
        if executable_is_valid(&exe, &dep) {
            self.measurement.executable_reused = true;
            return Ok((exe, key));
        }
        let t0 = Instant::now();
        let temporary = t.join(format!(
            "executor-{key}.tmp{}",
            std::env::consts::EXE_SUFFIX
        ));
        let mut cmd = std::process::Command::new(&self.cfg.compiler);
        // The bootstrap lives in the project that uses builders: compiling it
        // does not go through the engine (which would compile this executor
        // again).
        cmd.env("DARTFORGE_BUILD_COMPILING_EXECUTOR", "1");
        // Without the HIR optimizer (inlining): the executor is a huge program
        // (the whole `analyzer`) that runs once per session, and inlining
        // more than doubles the IR (1.34 GB versus 0.58 GB on
        // json_serializable) and the compilation memory (12 GB, killed by the
        // OOM killer). Recompiling the builders with heavy optimization is
        // what BUILD-RUST.md §4 rules out.
        cmd.env("DARTFORGE_OPTIMIZE_HIR", "0");
        cmd.arg("compile-native")
            .arg(&main)
            .arg("-o")
            .arg(&temporary)
            .arg("--packages")
            .arg(&pc);
        if let Some(s) = &self.cfg.sdk_lib {
            cmd.arg("--sdk").arg(s);
        }
        let output = cmd.output().map_err(|e| {
            format!(
                "could not run {}: {e}",
                self.cfg.compiler.display()
            )
        })?;
        if !output.status.success() || !temporary.is_file() {
            return Err(format!(
                "the native compilation of the builder executor failed:\n{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        std::fs::rename(&temporary, &exe).map_err(|e| format!("{}: {e}", exe.display()))?;
        let deps = program_files(&main, &pc, self.cfg.sdk_lib.as_deref())?;
        std::fs::write(&dep, depfile(&exe, &deps))
            .map_err(|e| format!("{}: {e}", dep.display()))?;
        // Executables with old keys are no longer useful, nor are the source
        // SDK libraries that `compile-native` copied next to them
        // (`libdfsdk_<hash>`, ~40 MB each): only the one the new executable
        // mentions is kept.
        let referenced = std::fs::read(&exe)
            .map(|b| referenced_libraries(&b))
            .unwrap_or_default();
        if let Ok(ls) = std::fs::read_dir(t) {
            for e in ls.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                let old_exe = n.starts_with("executor-") && !n.contains(&key);
                let old_lib =
                    n.starts_with("libdfsdk_") && !referenced.is_empty() && !referenced.contains(&n);
                if old_exe || old_lib {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        self.measurement.compilation = t0.elapsed();
        Ok((exe, key))
    }

    /// The `package_config.json` the executor uses: the project's own when it
    /// resolves [`DEPENDENCIES`]; otherwise a copy (with absolute URIs, in
    /// `package_config.projeto.json` in the work directory) completed from
    /// the pub cache (DF-BUILD-004).
    ///
    /// # Errors
    ///
    /// The project's file cannot be read, or some package is missing that
    /// the pub cache does not have (the message says which).
    pub fn complete_package_config(&self) -> Result<PathBuf, String> {
        if missing_dependencies(&self.cfg.package_config)?.is_empty() {
            return Ok(self.cfg.package_config.clone());
        }
        let cache = crate::executor_deps::pub_cache_dir()
            .ok_or("the pub cache was not found (set PUB_CACHE)")?;
        // The project's relative URIs become absolute (the new file lives in
        // another directory); the executor package comes out again in the
        // bootstrap.
        let absolute = crate::vm::package_config(&self.cfg.package_config, &self.cfg.work_dir)?;
        let (v, added) = crate::executor_deps::complete(&absolute, &cache)?;
        eprintln!(
            "builder executor: {} come from the pub cache ({})",
            added.join(", "),
            cache.display()
        );
        let destination = self.cfg.work_dir.join("package_config.projeto.json");
        let json = serde_json::to_string_pretty(&v).unwrap_or_default();
        if std::fs::read_to_string(&destination).ok().as_deref() != Some(json.as_str()) {
            std::fs::write(&destination, &json)
                .map_err(|e| format!("{}: {e}", destination.display()))?;
        }
        Ok(destination)
    }

    /// Compiles (or reuses) and starts the process; returns the key.
    fn start(&mut self, script: &BuilderScript) -> Result<String, String> {
        let (exe, key) = self.compile(script)?;
        let mut cmd = std::process::Command::new(&exe);
        // The VM's `--packages`: the executable's `Isolate.resolvePackageUri`
        // (`build_resolvers` finds the packages through it) reads this file.
        cmd.current_dir(&self.cfg.root).env(
            "DARTFORGE_PACKAGE_CONFIG",
            self.cfg.work_dir.join("package_config.json"),
        );
        // The SDK the builders' analyzer reads (the SDK summary,
        // `packages/build_executor`): the root of the `lib/` used for
        // compiling. Rebuilt from its components: the analyzer's
        // `buildSdkSummary` requires a normalized system path (`C:/sdk` on
        // Windows is rejected with "Path must be normalized", on the VM too),
        // and `--sdk`/`DARTFORGE_SDK_LIB` may arrive with `/`.
        if let Some(root) = self
            .cfg
            .sdk_lib
            .clone()
            .or_else(dartforge_elements::sdk::SdkLayout::discover)
            .and_then(|l| l.parent().map(|p| p.components().collect::<PathBuf>()))
        {
            cmd.env("DARTFORGE_DART_SDK", root);
        }
        self.client = Some(BuildClient::start(cmd)?);
        Ok(key)
    }

    /// One action (build or post-processing) through the process.
    fn action(
        &mut self,
        request: &ActionRequest,
        service: &mut dyn BuildStepService,
        post: bool,
    ) -> Result<ActionResult, ExecutorError> {
        let client = self
            .client
            .as_mut()
            .ok_or_else(|| ExecutorError("build.run before build.load".into()))?;
        let r = if post {
            client.post_process_step(request, service)
        } else {
            client.execute(request, service)
        };
        if let Err(e) = &r {
            // Broken channel (the process died, the protocol was violated):
            // the session no longer uses this process.
            self.failure = Some(format!("native builder executor: {}", e.0));
            self.client = None;
            self.key = None;
        }
        r
    }
}

impl DartExecutor for NativeExecutor {
    fn availability(&self) -> Availability {
        match &self.failure {
            Some(m) => Availability::Unavailable(m.clone()),
            None => Availability::Available,
        }
    }

    fn prepare(&mut self, script: &BuilderScript) -> Result<(), ExecutorError> {
        if let Some(m) = &self.failure {
            return Err(ExecutorError(m.clone()));
        }
        if self.key.is_some() {
            self.shutdown();
        }
        let t0 = Instant::now();
        let r = self.start(script).and_then(|key| {
            let client = self.client.as_mut().ok_or("executor without a process")?;
            client.prepare(script).map_err(|e| e.0)?;
            self.key = Some(key);
            Ok(())
        });
        self.measurement.startup = t0.elapsed().saturating_sub(self.measurement.compilation);
        r.map_err(|m| {
            self.client = None;
            self.failure = Some(m.clone());
            ExecutorError(m)
        })
    }

    fn new_round(&mut self) {
        if let Some(c) = self.client.as_mut() {
            c.new_round();
        }
    }

    fn extensions(
        &mut self,
        request: &ExtensionsRequest,
    ) -> Result<Option<RuntimeExtensions>, ExecutorError> {
        match self.client.as_mut() {
            Some(c) => c.extensions(request),
            None => Ok(None),
        }
    }

    fn execute(
        &mut self,
        request: &ActionRequest,
        service: &mut dyn BuildStepService,
    ) -> Result<ActionResult, ExecutorError> {
        self.action(request, service, false)
    }

    fn code(&self) -> Option<Vec<PathBuf>> {
        let key = self.key.as_ref()?;
        let text =
            std::fs::read_to_string(self.cfg.work_dir.join(format!("executor-{key}.d"))).ok()?;
        let deps = depfile_deps(&text);
        (!deps.is_empty()).then_some(deps)
    }

    fn post_inputs(
        &mut self,
        request: &ExtensionsRequest,
    ) -> Result<Option<Vec<String>>, ExecutorError> {
        match self.client.as_mut() {
            Some(c) => c.post_inputs(request),
            None => Ok(None),
        }
    }

    fn post_process_step(
        &mut self,
        request: &ActionRequest,
        service: &mut dyn BuildStepService,
    ) -> Result<ActionResult, ExecutorError> {
        self.action(request, service, true)
    }

    fn shutdown(&mut self) {
        if let Some(mut c) = self.client.take() {
            c.shutdown();
        }
        self.key = None;
    }
}

/// Hooks the native executor into `engine`, with the given `compiler` (the
/// `dartforge` itself with the `native` feature): the process is only born at
/// the session's first Dart action.
pub fn attach(
    engine: &mut crate::Engine,
    compiler: PathBuf,
    sdk_lib: Option<PathBuf>,
    package_config: &Path,
) {
    let root = engine.package_graph.root_dir.clone();
    let cfg = NativeConfig {
        package_config: package_config.to_path_buf(),
        ..NativeConfig::for_project(compiler, sdk_lib, &root)
    };
    engine.set_dart_executor(Box::new(NativeExecutor::new(cfg)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdk_libraries_referenced_in_the_executable() {
        let b = b"\x00xx libdfsdk_0123abcd.so\x00yy libdfsdk_0123abcd.so\x00libdfsdk_ff.dll\x00";
        assert_eq!(
            referenced_libraries(b),
            vec!["libdfsdk_0123abcd.so", "libdfsdk_ff.dll"]
        );
    }

    #[test]
    fn depfile_round_trip_with_spaces() {
        let deps = vec![
            PathBuf::from("/a/b.dart"),
            PathBuf::from("/c/with space.dart"),
        ];
        let text = depfile(Path::new("/t/executor"), &deps);
        assert_eq!(depfile_deps(&text), deps);
    }

    #[test]
    fn missing_executor_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        let pc = dir.path().join("package_config.json");
        std::fs::write(
            &pc,
            r#"{"configVersion":2,"packages":[{"name":"build","rootUri":"file:///b/","packageUri":"lib/"},
            {"name":"glob","rootUri":"file:///g/","packageUri":"lib/"}]}"#,
        )
        .unwrap();
        assert_eq!(
            missing_dependencies(&pc).unwrap(),
            vec!["analyzer", "build_resolvers", "logging", "package_config"]
        );
    }
}
