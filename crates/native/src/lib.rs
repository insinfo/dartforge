//! Driver AOT local: LLVM IR → objeto Clang → executável com runtime Rust.
//! Não usa shell, não sobrescreve saídas e remove somente seu diretório temporário.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// Ferramentas locais e nível de otimização do novo executável.
#[derive(Debug, Clone)]
pub struct NativeOptions {
    pub clang: PathBuf,
    pub rustc: PathBuf,
    pub optimize: bool,
}
impl Default for NativeOptions {
    /// Usa variáveis DARTFORGE_CLANG/DARTFORGE_RUSTC ou nomes encontrados no PATH.
    fn default() -> Self {
        Self {
            clang: std::env::var_os("DARTFORGE_CLANG")
                .map_or_else(|| "clang".into(), PathBuf::from),
            rustc: std::env::var_os("DARTFORGE_RUSTC")
                .map_or_else(|| "rustc".into(), PathBuf::from),
            optimize: false,
        }
    }
}

/// Tempos de parede de uma compilação completa, sem cache entre chamadas.
#[derive(Debug, Clone)]
pub struct BuildReport {
    /// Gravação do IR e do harness Rust no staging.
    pub write_ir_runtime: Duration,
    /// Execução do Clang, incluindo inicialização e coleta de saída do processo.
    pub clang: Duration,
    /// Compilação do harness e ligação, incluindo inicialização do rustc.
    pub rustc_link: Duration,
    /// Cópia exclusiva, permissões e sincronização da saída.
    pub publish: Duration,
    /// Chamada completa, incluindo preparação, validações e limpeza do staging.
    pub total: Duration,
    /// Bytes efetivamente copiados para o executável publicado.
    pub executable_bytes: u64,
}

/// Diagnóstico completo de uma etapa do driver ou ferramenta externa.
#[derive(Debug)]
pub struct NativeError {
    pub stage: &'static str,
    pub message: String,
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}
impl std::fmt::Display for NativeError {
    /// Apresenta a etapa e mantém os diagnósticos originais das ferramentas.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.stage, self.message)?;
        if !self.stdout.is_empty() {
            write!(f, "\nstdout:\n{}", self.stdout)?;
        }
        if !self.stderr.is_empty() {
            write!(f, "\nstderr:\n{}", self.stderr)?;
        }
        Ok(())
    }
}
impl std::error::Error for NativeError {}

/// Compila IR do subconjunto AOT para o host sem executar o programa produzido.
///
/// Requer void @dartforge_entry(), com chamadas C para print_i64(i64) e
/// print_bool(i8). Rejeita triple explícito: cross-compilation não é suportada.
/// Publica a saída por create_new apenas após as duas ferramentas concluírem.
///
/// # Erros
///
/// Saída existente, diretório inexistente, ferramenta ausente, IR inválido ou
/// falha de ligação produzem diagnóstico; falhas de build não deixam executável.
///
///     # if false {
///     let options = dartforge_native::NativeOptions::default();
///     dartforge_native::build_executable(
///         "define void @dartforge_entry() { ret void }",
///         std::path::Path::new("program.exe"),
///         &options,
///     )?;
///     # }
///     # Ok::<(), dartforge_native::NativeError>(())
pub fn build_executable(
    ir: &str,
    output: &Path,
    options: &NativeOptions,
) -> Result<(), NativeError> {
    build_executable_with_report(ir, output, options).map(|_| ())
}

/// Compila e mede fases disjuntas com relógio monotônico.
///
/// Inclui inicialização e espera das ferramentas; não inclui análise Dart ou
/// emissão do IR. O total também inclui preparação e limpeza do staging.
///
/// # Erros
///
/// Retorna os mesmos diagnósticos de [`build_executable`], sem relatório parcial.
///
///     # if false {
///     let report = dartforge_native::build_executable_with_report(
///         "define void @dartforge_entry() { ret void }",
///         std::path::Path::new("program.exe"),
///         &dartforge_native::NativeOptions::default(),
///     )?;
///     assert!(report.total >= report.clang);
///     # }
///     # Ok::<(), dartforge_native::NativeError>(())
pub fn build_executable_with_report(
    ir: &str,
    output: &Path,
    options: &NativeOptions,
) -> Result<BuildReport, NativeError> {
    build_executable_with_report_and_objects(ir, output, options, &[])
}

/// Liga objetos nativos explicitamente fornecidos, preservando as opções existentes.
/// Os objetos devem usar a ABI C do mesmo host; handles gerenciados não são ponteiros C.
///
/// # Erros
/// Rejeita caminhos inexistentes/não arquivos e propaga erros das ferramentas de ligação.
pub fn build_executable_with_objects(
    ir: &str,
    output: &Path,
    options: &NativeOptions,
    objects: &[PathBuf],
) -> Result<(), NativeError> {
    build_executable_with_report_and_objects(ir, output, options, objects).map(|_| ())
}

/// Compila e mede a ligação com objetos externos, sem shell ou concatenação de comandos.
/// Caminhos absolutos são passados individualmente ao linker, inclusive quando contêm espaços.
/// Os bytes dos objetos continuam sob responsabilidade do chamador durante esta operação.
///
/// # Erros
/// Retorna diagnósticos de objetos, compilação e publicação sem substituir saída existente.
pub fn build_executable_with_report_and_objects(
    ir: &str,
    output: &Path,
    options: &NativeOptions,
    objects: &[PathBuf],
) -> Result<BuildReport, NativeError> {
    let started = Instant::now();
    let objects = objects
        .iter()
        .map(|path| {
            let canonical = std::fs::canonicalize(path)
                .map_err(|error| failure("objects", format!("{}: {error}", path.display())))?;
            if !canonical.is_file() {
                return Err(failure(
                    "objects",
                    format!("objeto não é arquivo: {}", path.display()),
                ));
            }
            Ok(canonical)
        })
        .collect::<Result<Vec<_>, NativeError>>()?;
    if ir.lines().any(|line| {
        let mut words = line.split_whitespace();
        words.next() == Some("target") && matches!(words.next(), Some("triple" | "datalayout"))
    }) {
        return Err(failure(
            "target",
            "target triple/datalayout explícito não suportado; somente host nativo",
        ));
    }
    let filename = output
        .file_name()
        .ok_or_else(|| failure("output", "nome de saída inválido"))?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = std::fs::canonicalize(parent).map_err(|e| failure("output", e.to_string()))?;
    let output = parent.join(filename);
    match std::fs::symlink_metadata(&output) {
        Ok(_) => {
            return Err(failure(
                "output",
                format!("saída já existe: {}", output.display()),
            ));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(failure("output", e.to_string())),
    }
    let work = WorkDir::new(&parent)?;
    let input = work.0.join("program.ll");
    let object = work.0.join("program.obj");
    let runtime = work.0.join("runtime_main.rs");
    let executable = work.0.join(if cfg!(windows) {
        "program.exe"
    } else {
        "program"
    });
    let phase = Instant::now();
    std::fs::write(&input, ir).map_err(|e| failure("write-ir", e.to_string()))?;
    std::fs::write(&runtime, dartforge_runtime::RUNTIME_MAIN)
        .map_err(|e| failure("write-runtime", e.to_string()))?;
    let write_ir_runtime = phase.elapsed();
    let phase = Instant::now();
    run(
        "clang",
        Command::new(&options.clang)
            .arg("-x")
            .arg("ir")
            .arg("-c")
            .arg(if options.optimize { "-O2" } else { "-O0" })
            .arg(&input)
            .arg("-o")
            .arg(&object),
    )?;
    let clang = phase.elapsed();
    let phase = Instant::now();
    let mut link_argument = std::ffi::OsString::from("link-arg=");
    link_argument.push(&object);
    let mut rustc = Command::new(&options.rustc);
    rustc
        .arg("--edition=2024")
        .arg("--crate-name")
        .arg("dartforge_native_program")
        .arg(&runtime)
        .arg("-C")
        .arg(link_argument)
        .arg("-C")
        .arg(if options.optimize {
            "opt-level=2"
        } else {
            "opt-level=0"
        })
        .arg("-o")
        .arg(&executable);
    for external in &objects {
        let mut argument = std::ffi::OsString::from("link-arg=");
        argument.push(external);
        rustc.arg("-C").arg(argument);
    }
    run("rustc", &mut rustc)?;
    let rustc_link = phase.elapsed();
    let phase = Instant::now();
    let mut source =
        std::fs::File::open(&executable).map_err(|e| failure("publish", e.to_string()))?;
    let permissions = source
        .metadata()
        .map_err(|e| failure("publish", e.to_string()))?
        .permissions();
    let mut destination = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .map_err(|e| failure("publish", format!("{}: {e}", output.display())))?;
    let publication = std::io::copy(&mut source, &mut destination)
        .and_then(|bytes| destination.set_permissions(permissions).map(|()| bytes))
        .and_then(|bytes| destination.sync_all().map(|()| bytes));
    drop(destination);
    let executable_bytes = publication.map_err(|error| {
        let _ = std::fs::remove_file(&output);
        failure("publish", error.to_string())
    })?;
    let publish = phase.elapsed();
    drop(source);
    drop(work);
    Ok(BuildReport {
        write_ir_runtime,
        clang,
        rustc_link,
        publish,
        total: started.elapsed(),
        executable_bytes,
    })
}

/// Constrói erro local sem inventar stdout/stderr de uma ferramenta.
fn failure(stage: &'static str, message: impl Into<String>) -> NativeError {
    NativeError {
        stage,
        message: message.into(),
        status: None,
        stdout: String::new(),
        stderr: String::new(),
    }
}

/// Executa argumentos diretamente e preserva ambas as saídas e o código de erro.
fn run(stage: &'static str, command: &mut Command) -> Result<(), NativeError> {
    let result = command.output().map_err(|e| {
        failure(
            stage,
            format!("não foi possível executar {:?}: {e}", command.get_program()),
        )
    })?;
    if result.status.success() {
        return Ok(());
    }
    Err(NativeError {
        stage,
        message: format!(
            "ferramenta {:?} falhou ({})",
            command.get_program(),
            result.status
        ),
        status: result.status.code(),
        stdout: String::from_utf8_lossy(&result.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&result.stderr).into_owned(),
    })
}

/// Diretório exclusivo pertencente a uma única chamada do driver.
struct WorkDir(PathBuf);
impl WorkDir {
    /// Reserva um diretório filho novo sem reutilizar nomes existentes.
    fn new(parent: &Path) -> Result<Self, NativeError> {
        loop {
            let path = parent.join(format!(
                ".dartforge-native-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(failure("temporary", e.to_string())),
            }
        }
    }
}
impl Drop for WorkDir {
    /// Limpa somente o diretório reservado com create_dir por esta instância.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Cria a área exclusiva de um teste usando o mesmo protocolo de propriedade.
    fn fixture() -> WorkDir {
        WorkDir::new(&std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap()
    }
    /// Compila, liga e executa ambas as variantes de otimização usando o toolchain local.
    #[test]
    #[ignore = "requer Clang e rustc nativos no PATH ou DARTFORGE_CLANG/DARTFORGE_RUSTC"]
    fn real_executable_prints_ints_and_bools() {
        let root = fixture();
        let ir = "declare void @dartforge_print_i64(i64)\ndeclare void @dartforge_print_bool(i8)\ndefine void @dartforge_entry() {\n call void @dartforge_print_i64(i64 -42)\n call void @dartforge_print_bool(i8 1)\n call void @dartforge_print_bool(i8 0)\n ret void\n}\n";
        for optimize in [false, true] {
            let output = root.0.join(if optimize {
                "optimized.exe"
            } else {
                "plain.exe"
            });
            let options = NativeOptions {
                optimize,
                ..Default::default()
            };
            let report = build_executable_with_report(ir, &output, &options).unwrap();
            assert_eq!(
                report.executable_bytes,
                std::fs::metadata(&output).unwrap().len()
            );
            assert!(report.executable_bytes > 0);
            assert!(
                report.total
                    >= report.write_ir_runtime + report.clang + report.rustc_link + report.publish
            );
            let result = Command::new(&output).output().unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                String::from_utf8(result.stdout)
                    .unwrap()
                    .replace("\r\n", "\n"),
                "-42\ntrue\nfalse\n"
            );
        }
        assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 2);
    }
    /// Caminhos inválidos são recusados antes de executar qualquer ferramenta.
    #[test]
    fn external_objects_require_existing_files() {
        let root = fixture();
        for object in [root.0.join("missing.obj"), root.0.clone()] {
            let error = build_executable_with_objects(
                "define void @dartforge_entry(){ret void}",
                &root.0.join("out.exe"),
                &NativeOptions::default(),
                &[object],
            )
            .unwrap_err();
            assert_eq!(error.stage, "objects");
            assert!(!root.0.join("out.exe").exists());
        }
    }

    /// A ausência da ferramenta não publica saída nem mantém staging.
    #[test]
    fn missing_tool_cleans_up_without_output() {
        let root = fixture();
        let output = root.0.join("output.exe");
        let options = NativeOptions {
            clang: root.0.join("missing-clang"),
            rustc: "unused".into(),
            optimize: false,
        };
        let error = build_executable(
            "define void @dartforge_entry(){ret void}",
            &output,
            &options,
        )
        .unwrap_err();
        assert_eq!(error.stage, "clang");
        assert!(!output.exists());
        assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 0);
    }
    /// Uma saída existente permanece byte a byte intacta.
    #[test]
    fn existing_output_is_preserved() {
        let root = fixture();
        let output = root.0.join("output.exe");
        std::fs::write(&output, b"preserve me").unwrap();
        let error = build_executable("", &output, &NativeOptions::default()).unwrap_err();
        assert_eq!(error.stage, "output");
        assert_eq!(std::fs::read(output).unwrap(), b"preserve me");
        assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 1);
    }
    /// Uma ferramenta que retorna falha entrega status e stderr e limpa staging.
    #[test]
    fn tool_failure_keeps_diagnostic_and_removes_temporary_files() {
        let root = fixture();
        let output = root.0.join("output.exe");
        let options = NativeOptions {
            clang: std::env::current_exe().unwrap(),
            rustc: "unused".into(),
            optimize: true,
        };
        let error = build_executable("", &output, &options).unwrap_err();
        assert_eq!(error.stage, "clang");
        assert!(error.status.is_some());
        assert!(!error.stderr.is_empty() || !error.stdout.is_empty());
        assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 0);
    }
    /// O driver não aceita selecionar um destino LLVM de outra arquitetura.
    #[test]
    fn explicit_target_is_rejected() {
        let root = fixture();
        let error = build_executable(
            "target triple = \"aarch64-unknown-linux-gnu\"",
            &root.0.join("out"),
            &NativeOptions::default(),
        )
        .unwrap_err();
        assert_eq!(error.stage, "target");
    }
}
