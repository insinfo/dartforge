//! The builder executor on the **Dart VM** (`docs/BUILD-PROTOCOLO.md` §3):
//! a persistent process that speaks `dfexec/1` and serves the `build.*`
//! service with the Dart side of `packages/build_executor`. This is the
//! interim path of Phase 4 (BUILD-RUST.md): the same protocol the
//! self-hosted executor will speak, with the official VM in place of our
//! runtime. That is why it is optional and explicit
//! (`dartforge build --dart <exe>`, or `DARTFORGE_BUILD_DART`): without it
//! the engine keeps the [`crate::executor::Indisponivel`] executor.
//!
//! The host generates, in the project's `.dart_tool/dartforge/build/`:
//!
//! * the `dartforge_build_executor` package (its sources are embedded in the
//!   binary);
//! * a `package_config.json` that is the project's plus that package — the
//!   project uses `build_runner`, so `build`, `build_resolvers` and their
//!   dependencies are already there, at the locked versions;
//! * the *bootstrap* (`bootstrap.dart`): the equivalent of the official
//!   `.dart_tool/build/entrypoint/build.dart`, with the same factory imports
//!   and a key → factory map, at the same directory depth (the plan's
//!   relative imports work without rewriting);
//! * a *kernel* compiled once (`dart compile kernel`), reused across
//!   sessions as long as no file in the `--depfile` changes.
use crate::cliente::ClienteBuild;
use crate::executor::{
    Disponibilidade, ErroExecutor, ExecutorDart, ExtensoesDeExecucao, PedidoAcao, PedidoExtensoes,
    ResultadoAcao, ScriptDeBuilders, ServicoBuildStep,
};
use dartforge_dfexec::CanalDeProcesso;
use serde_json::{Value, json};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// Name of the executor's Dart package in the generated `package_config.json`.
pub const EXECUTOR_PACKAGE: &str = "dartforge_build_executor";

/// Embedded sources of the Dart side: (path under `lib/`, text).
const SOURCES: &[(&str, &str)] = &[
    (
        "executor.dart",
        include_str!("../../../packages/build_executor/lib/executor.dart"),
    ),
    (
        "src/canal.dart",
        include_str!("../../../packages/build_executor/lib/src/canal.dart"),
    ),
    (
        "src/servico.dart",
        include_str!("../../../packages/build_executor/lib/src/servico.dart"),
    ),
];

/// Environment variable that enables the VM executor outside `dartforge build`.
pub const ENV_VAR: &str = "DARTFORGE_BUILD_DART";

/// The `dart` executable requested by the environment ([`ENV_VAR`]), if any.
pub fn dart_do_ambiente() -> Option<PathBuf> {
    std::env::var_os(ENV_VAR)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Enables the VM executor on `motor` with the given `dart`: the process is
/// only spawned on the session's first Dart action. `package_config` is the
/// project's (the one the engine read).
pub fn ligar(motor: &mut crate::Motor, dart: PathBuf, package_config: &Path) {
    let root = motor.grafo_pacotes.dir_raiz.clone();
    let cfg = ConfigDaVm {
        package_config: package_config.to_path_buf(),
        ..ConfigDaVm::do_projeto(dart, &root)
    };
    motor.definir_executor_dart(Box::new(ExecutorVm::novo(cfg)));
}

/// [`ligar`] with the `dart` from [`ENV_VAR`], if the environment asked for
/// it; returns whether it was enabled.
pub fn ligar_do_ambiente(motor: &mut crate::Motor, package_config: &Path) -> bool {
    match dart_do_ambiente() {
        Some(d) => {
            ligar(motor, d, package_config);
            true
        }
        None => false,
    }
}

/// Where the VM and the project are.
#[derive(Debug, Clone)]
pub struct ConfigDaVm {
    /// The `dart` executable.
    pub dart: PathBuf,
    /// Directory of the root package (the process runs in it, like
    /// `build_runner`: `build_resolvers` keeps the SDK summary in
    /// `.dart_tool/`).
    pub root: PathBuf,
    /// The project's `package_config.json`.
    pub package_config: PathBuf,
    /// Where to write the bootstrap, the executor package and the kernel.
    pub work_dir: PathBuf,
}

impl ConfigDaVm {
    /// The default configuration of a project: `.dart_tool/package_config.json`
    /// and work directory `.dart_tool/dartforge/build` (the same depth as the
    /// official `.dart_tool/build/entrypoint`).
    ///
    /// ```
    /// use dartforge_build::vm::ConfigDaVm;
    /// use std::path::Path;
    /// let c = ConfigDaVm::do_projeto("dart".into(), Path::new("/p"));
    /// assert!(c.work_dir.ends_with(".dart_tool/dartforge/build"));
    /// ```
    pub fn do_projeto(dart: PathBuf, root: &Path) -> ConfigDaVm {
        ConfigDaVm {
            dart,
            root: root.to_path_buf(),
            package_config: root.join(".dart_tool").join("package_config.json"),
            work_dir: root.join(".dart_tool").join("dartforge").join("build"),
        }
    }
}

/// Single-quoted Dart string literal.
fn literal(s: &str) -> String {
    let mut r = String::with_capacity(s.len() + 2);
    r.push('\'');
    for c in s.chars() {
        match c {
            '\\' => r.push_str("\\\\"),
            '\'' => r.push_str("\\'"),
            '$' => r.push_str("\\$"),
            '\n' => r.push_str("\\n"),
            '\r' => r.push_str("\\r"),
            c => r.push(c),
        }
    }
    r.push('\'');
    r
}

/// The bootstrap text: the factory imports (as the official `build.dart`
/// writes them) and the key → factory map.
///
/// ```
/// use dartforge_build::executor::ScriptDeBuilders;
/// use std::path::Path;
/// let s = ScriptDeBuilders {
///     aplicacoes: vec![("p:b".into(), "package:p/builder.dart".into(), vec!["b".into()])],
///     chave_de_cache: "x".into(),
/// };
/// let text = dartforge_build::vm::bootstrap(&s, Path::new("/p/.dart_tool/package_config.json"));
/// assert!(text.contains("import 'package:p/builder.dart' as _i0;"));
/// assert!(text.contains("'p:b': {'b': _i0.b}"));
/// ```
pub fn bootstrap(script: &ScriptDeBuilders, package_config: &Path) -> String {
    let mut imports: Vec<&str> = Vec::new();
    for (_, import, _) in &script.aplicacoes {
        if !imports.contains(&import.as_str()) {
            imports.push(import);
        }
    }
    let mut s = String::from(
        "// Gerado pelo DartForge: bootstrap do executor de builders (dfexec/1, serviço build.*).\n\
         // ignore_for_file: directives_ordering, no_leading_underscores_for_library_prefixes\n\
         import 'package:dartforge_build_executor/executor.dart' as _df;\n",
    );
    for (i, imp) in imports.iter().enumerate() {
        s.push_str(&format!("import {} as _i{i};\n", literal(imp)));
    }
    s.push_str("\nFuture<void> main() => _df.servir({\n");
    for (key, import, factories) in &script.aplicacoes {
        let i = imports.iter().position(|x| x == import).unwrap_or(0);
        let map: Vec<String> = factories
            .iter()
            .map(|f| format!("{}: _i{i}.{f}", literal(f)))
            .collect();
        s.push_str(&format!("  {}: {{{}}},\n", literal(key), map.join(", ")));
    }
    s.push_str(&format!(
        "}}, packageConfig: {});\n",
        literal(&package_config.to_string_lossy())
    ));
    s
}

/// Absolute `file:` URI of a directory (ends in `/`).
fn directory_uri(p: &Path) -> String {
    let abs = std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    url::Url::from_directory_path(&abs)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| {
            format!(
                "file:///{}/",
                abs.to_string_lossy()
                    .replace('\\', "/")
                    .trim_start_matches('/')
            )
        })
}

/// The project's `package_config.json` (relative URIs become absolute,
/// since the new file lives in another directory) plus the executor package.
///
/// # Errors
///
/// The project's file cannot be read or is not a `package_config`.
pub fn package_config(of_project: &Path, executor: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(of_project)
        .map_err(|e| format!("{}: {e}", of_project.display()))?;
    let mut v: Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", of_project.display()))?;
    let base = url::Url::from_directory_path(
        std::path::absolute(of_project.parent().unwrap_or(Path::new(".")))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|_| format!("{}: path has no URI", of_project.display()))?;
    let ps = v
        .get_mut("packages")
        .and_then(Value::as_array_mut)
        .ok_or("package_config without 'packages'")?;
    for p in ps.iter_mut() {
        if let Some(r) = p.get("rootUri").and_then(Value::as_str)
            && let Ok(abs) = base.join(r)
        {
            p["rootUri"] = json!(abs.to_string());
        }
    }
    ps.retain(|p| p["name"] != EXECUTOR_PACKAGE);
    ps.push(json!({"name": EXECUTOR_PACKAGE, "rootUri": directory_uri(executor), "packageUri": "lib/", "languageVersion": "3.6"}));
    Ok(v)
}

/// Normalizes `.`/`..` without touching the disk.
fn normalize(p: &Path) -> PathBuf {
    let mut r = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                r.pop();
            }
            Component::CurDir => {}
            c => r.push(c),
        }
    }
    r
}

/// Files of a Ninja depfile (`output: dep1 dep2`, spaces escaped with `\`).
pub(crate) fn deps_do_depfile(text: &str) -> Vec<PathBuf> {
    let Some((_, rest)) = text.split_once(": ") else {
        return Vec::new();
    };
    let mut v = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for c in rest.chars() {
        match c {
            _ if escaped => {
                current.push(c);
                escaped = false;
            }
            '\\' => escaped = true,
            ' ' | '\n' | '\r' | '\t' => {
                if !current.is_empty() {
                    v.push(PathBuf::from(std::mem::take(&mut current)));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        v.push(PathBuf::from(current));
    }
    v
}

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// Is the kernel still valid? It exists, and no file the compilation read
/// is newer than it (or has disappeared).
fn kernel_is_valid(dill: &Path, depfile: &Path) -> bool {
    let (Some(t), Ok(text)) = (mtime(dill), std::fs::read_to_string(depfile)) else {
        return false;
    };
    let deps = deps_do_depfile(&text);
    !deps.is_empty() && deps.iter().all(|d| mtime(d).is_some_and(|m| m <= t))
}

/// The files the builder bootstrap needs, already written.
pub(crate) struct Gravados {
    /// The `bootstrap.dart`.
    pub principal: PathBuf,
    /// The project's `package_config.json` plus the executor package.
    pub package_config: PathBuf,
    /// Hash of the script, the executor sources, and the written bootstrap
    /// and `package_config.json`: the base of the key of what gets compiled.
    pub hasher: blake3::Hasher,
}

/// Writes into `work_dir` the executor package (the embedded sources), the
/// `package_config.json` (the project's, `package_config`, plus the
/// executor) and the bootstrap for `script`, rewriting only what changed (the
/// file dates decide whether the compiled artifact is still valid). Shared by
/// the VM and the native executor.
pub(crate) fn gravar_bootstrap(
    work_dir: &Path,
    project_package_config: &Path,
    script: &ScriptDeBuilders,
) -> Result<Gravados, String> {
    let error = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
    let package = work_dir.join(EXECUTOR_PACKAGE);
    let mut h = blake3::Hasher::new();
    h.update(script.chave_de_cache.as_bytes());
    for (rel, text) in SOURCES {
        let destination = package.join("lib").join(rel);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|e| error(parent, e))?;
        }
        if std::fs::read_to_string(&destination).ok().as_deref() != Some(*text) {
            std::fs::write(&destination, text).map_err(|e| error(&destination, e))?;
        }
        h.update(rel.as_bytes());
        h.update(text.as_bytes());
    }
    let pc = work_dir.join("package_config.json");
    let json = serde_json::to_string_pretty(&package_config(project_package_config, &package)?)
        .unwrap_or_default();
    if std::fs::read_to_string(&pc).ok().as_deref() != Some(json.as_str()) {
        std::fs::write(&pc, &json).map_err(|e| error(&pc, e))?;
    }
    let main = work_dir.join("bootstrap.dart");
    let text = bootstrap(
        script,
        &normalize(&std::path::absolute(project_package_config).unwrap_or_default()),
    );
    if std::fs::read_to_string(&main).ok().as_deref() != Some(text.as_str()) {
        std::fs::write(&main, &text).map_err(|e| error(&main, e))?;
    }
    h.update(text.as_bytes());
    h.update(json.as_bytes());
    Ok(Gravados {
        principal: main,
        package_config: pc,
        hasher: h,
    })
}

/// What the executor measured (for the report and the tests).
#[derive(Debug, Clone, Default)]
pub struct VmMeasurement {
    /// Time of `dart compile kernel` (zero when the kernel was reused).
    pub compilation: Duration,
    /// From process start to `build.carregado`.
    pub startup: Duration,
    pub kernel_reused: bool,
}

/// The VM executor: spawns the process on the session's first Dart action
/// and keeps it warm until [`ExecutorDart::encerrar`].
pub struct ExecutorVm {
    cfg: ConfigDaVm,
    client: Option<ClienteBuild<CanalDeProcesso>>,
    /// Key of the kernel the current process loaded.
    key: Option<String>,
    failure: Option<String>,
    pub measurement: VmMeasurement,
}

impl ExecutorVm {
    pub fn novo(cfg: ConfigDaVm) -> ExecutorVm {
        ExecutorVm {
            cfg,
            client: None,
            key: None,
            failure: None,
            measurement: VmMeasurement::default(),
        }
    }

    /// Writes the files, compiles (or reuses) the kernel and starts the
    /// process. Returns the kernel key.
    fn start(&mut self, script: &ScriptDeBuilders) -> Result<String, String> {
        let t = &self.cfg.work_dir;
        let error = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
        let Gravados {
            principal: main,
            package_config: pc,
            mut hasher,
        } = gravar_bootstrap(t, &self.cfg.package_config, script)?;
        let h = &mut hasher;
        h.update(self.cfg.dart.to_string_lossy().as_bytes());
        let key = h.finalize().to_hex()[..16].to_string();
        let dill = t.join(format!("bootstrap-{key}.dill"));
        let depfile = t.join(format!("bootstrap-{key}.d"));
        self.measurement = VmMeasurement::default();
        if kernel_is_valid(&dill, &depfile) {
            self.measurement.kernel_reused = true;
        } else {
            let t0 = Instant::now();
            let temporary = t.join(format!("bootstrap-{key}.dill.tmp"));
            let output = std::process::Command::new(&self.cfg.dart)
                .current_dir(&self.cfg.root)
                .arg("compile")
                .arg("kernel")
                .arg("--verbosity=error")
                .arg(format!("--packages={}", pc.display()))
                .arg(format!("--depfile={}", depfile.display()))
                .arg("-o")
                .arg(&temporary)
                .arg(&main)
                .output()
                .map_err(|e| format!("could not run {}: {e}", self.cfg.dart.display()))?;
            if !output.status.success() {
                return Err(format!(
                    "compiling the builder bootstrap failed:\n{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            std::fs::rename(&temporary, &dill).map_err(|e| error(&dill, e))?;
            // Kernels with old keys are no longer useful.
            if let Ok(ls) = std::fs::read_dir(t) {
                for e in ls.flatten() {
                    let n = e.file_name().to_string_lossy().to_string();
                    if n.starts_with("bootstrap-") && !n.contains(&key) {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
            }
            self.measurement.compilation = t0.elapsed();
        }
        let mut cmd = std::process::Command::new(&self.cfg.dart);
        cmd.current_dir(&self.cfg.root)
            .arg(format!("--packages={}", pc.display()))
            .arg(&dill);
        self.client = Some(ClienteBuild::iniciar(cmd)?);
        Ok(key)
    }
}

impl ExecutorVm {
    /// One action (build or post-processing) through the process.
    fn action(
        &mut self,
        request: &PedidoAcao,
        service: &mut dyn ServicoBuildStep,
        post: bool,
    ) -> Result<ResultadoAcao, ErroExecutor> {
        let client = self
            .client
            .as_mut()
            .ok_or_else(|| ErroExecutor("build.executar before build.carregar".into()))?;
        let r = if post {
            client.pos_processar(request, service)
        } else {
            client.executar(request, service)
        };
        if let Err(e) = &r {
            // Broken channel (process died, protocol violated): the session
            // no longer uses this process.
            self.failure = Some(format!("VM builder executor: {}", e.0));
            self.client = None;
            self.key = None;
        }
        r
    }
}

impl ExecutorDart for ExecutorVm {
    fn disponibilidade(&self) -> Disponibilidade {
        match &self.failure {
            Some(m) => Disponibilidade::Indisponivel(m.clone()),
            None => Disponibilidade::Disponivel,
        }
    }

    fn preparar(&mut self, script: &ScriptDeBuilders) -> Result<(), ErroExecutor> {
        if let Some(m) = &self.failure {
            return Err(ErroExecutor(m.clone()));
        }
        // A new script (the configuration changed) requires another process.
        if self.key.is_some() {
            self.encerrar();
        }
        let t0 = Instant::now();
        let r = self.start(script).and_then(|key| {
            let client = self.client.as_mut().ok_or("executor has no process")?;
            client.preparar(script).map_err(|e| e.0)?;
            self.key = Some(key);
            Ok(())
        });
        self.measurement.startup = t0.elapsed().saturating_sub(self.measurement.compilation);
        r.map_err(|m| {
            self.client = None;
            self.failure = Some(m.clone());
            ErroExecutor(m)
        })
    }

    fn nova_rodada(&mut self) {
        if let Some(c) = self.client.as_mut() {
            c.nova_rodada();
        }
    }

    fn extensoes(
        &mut self,
        request: &PedidoExtensoes,
    ) -> Result<Option<ExtensoesDeExecucao>, ErroExecutor> {
        match self.client.as_mut() {
            Some(c) => c.extensoes(request),
            None => Ok(None),
        }
    }

    fn executar(
        &mut self,
        request: &PedidoAcao,
        service: &mut dyn ServicoBuildStep,
    ) -> Result<ResultadoAcao, ErroExecutor> {
        self.action(request, service, false)
    }

    fn codigo(&self) -> Option<Vec<PathBuf>> {
        let key = self.key.as_ref()?;
        let text =
            std::fs::read_to_string(self.cfg.work_dir.join(format!("bootstrap-{key}.d"))).ok()?;
        let deps = deps_do_depfile(&text);
        (!deps.is_empty()).then_some(deps)
    }

    fn entradas_pos(
        &mut self,
        request: &PedidoExtensoes,
    ) -> Result<Option<Vec<String>>, ErroExecutor> {
        match self.client.as_mut() {
            Some(c) => c.entradas_pos(request),
            None => Ok(None),
        }
    }

    fn pos_processar(
        &mut self,
        request: &PedidoAcao,
        service: &mut dyn ServicoBuildStep,
    ) -> Result<ResultadoAcao, ErroExecutor> {
        self.action(request, service, true)
    }

    fn encerrar(&mut self) {
        if let Some(mut c) = self.client.take() {
            c.encerrar();
        }
        self.key = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depfile_with_escaped_spaces() {
        let d = deps_do_depfile("/t/x.dill: /a/b.dart /c/with\\ space.dart\n");
        assert_eq!(
            d,
            vec![
                PathBuf::from("/a/b.dart"),
                PathBuf::from("/c/with space.dart")
            ]
        );
    }

    #[test]
    fn absolute_package_config_with_the_executor() {
        let dir = tempfile::tempdir().unwrap();
        let dt = dir.path().join(".dart_tool");
        std::fs::create_dir_all(&dt).unwrap();
        std::fs::write(
            dt.join("package_config.json"),
            r#"{"configVersion":2,"packages":[
            {"name":"p","rootUri":"../","packageUri":"lib/","languageVersion":"3.6"},
            {"name":"q","rootUri":"file:///c/q/","packageUri":"lib/"}]}"#,
        )
        .unwrap();
        let v = package_config(&dt.join("package_config.json"), &dir.path().join("exec")).unwrap();
        let ps = v["packages"].as_array().unwrap();
        assert_eq!(
            ps[0]["rootUri"],
            json!(
                url::Url::from_directory_path(std::path::absolute(dir.path()).unwrap())
                    .unwrap()
                    .to_string()
            )
        );
        assert_eq!(ps[1]["rootUri"], "file:///c/q/");
        assert_eq!(ps[2]["name"], EXECUTOR_PACKAGE);
    }

    #[test]
    fn dart_literal_escapes_interpolation() {
        assert_eq!(literal("a'b$c\\"), "'a\\'b\\$c\\\\'");
    }
}
