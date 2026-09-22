//! Comandos do backend nativo (JIT ORCv2, AOT, ABI), atrás do feature `nativo`.
//!
//! O caminho JavaScript não depende do LLVM: sem o feature, os comandos
//! existem mas explicam como habilitá-los. Isso tira o `llvm-sys` do build
//! padrão do CLI (era ele que custava minutos e exigia `LLVM-C.dll`).
#![cfg_attr(not(feature = "nativo"), allow(unused))]
use std::{fs, path::PathBuf};

type Resultado = Result<(), Box<dyn std::error::Error>>;

#[cfg(not(feature = "nativo"))]
fn desabilitado(comando: &str) -> Resultado {
    Err(format!("`dartforge {comando}` exige o backend nativo: compile com `cargo build -p dartforge-cli --features nativo` (precisa do LLVM em D:/LLVM/22.1.8)").into())
}

#[cfg(not(feature = "nativo"))]
pub fn abi_info(_args: &[std::ffi::OsString]) -> Resultado { desabilitado("abi-info") }
#[cfg(not(feature = "nativo"))]
pub fn aot(_args: &[std::ffi::OsString]) -> Resultado { desabilitado("aot") }
#[cfg(not(feature = "nativo"))]
pub fn run_jit(_args: &[std::ffi::OsString]) -> Resultado { desabilitado("run") }
#[cfg(not(feature = "nativo"))]
pub fn run_hot_reload(_args: &[std::ffi::OsString]) -> Resultado { desabilitado("reload") }
#[cfg(not(feature = "nativo"))]
pub fn run_compile_native(_args: &[std::ffi::OsString]) -> Resultado { desabilitado("compile-native") }

#[cfg(feature = "nativo")]
pub fn abi_info(args: &[std::ffi::OsString]) -> Resultado {
        use dartforge_abi::{NativeType, Target};
        if args.len() != 2 {
            return Err("usage: dartforge abi-info <windows-x64|linux-x64|wasm32>".into());
        }
        let target = match args[1].to_str() {
            Some("windows-x64") => Target::WindowsX64,
            Some("linux-x64") => Target::LinuxX64,
            Some("wasm32") => Target::Wasm32,
            _ => return Err("perfil ABI desconhecido".into()),
        };
        let layouts: Vec<_> = [NativeType::Int32, NativeType::Int64, NativeType::Double, NativeType::Pointer]
            .iter().map(|ty| {
                let (size, alignment) = ty.layout(target).expect("tipo escalar");
                serde_json::json!({"native_type":format!("{ty:?}"),"llvm":ty.llvm(),"size":size,"alignment":alignment})
            }).collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version":1,"triple":target.triple(),"pointer_bits":target.pointer_bits(),
                "native_type_layouts":layouts,"profile_supports_dynamic_libraries":target.supports_dynamic_libraries(),
                "ffi_execution_implemented":false,"native_static_scalar_bindings_implemented":true,"wasm_emission_implemented":false,
                "note":"FFI completo ainda pendente. AOT host aceita @Native Int32/Int64/Void com objetos ligados explicitamente; não carrega bibliotecas dinamicamente. Perfis não habilitam cross-compilation."
            }))?
        );
        return Ok(());
}

#[cfg(feature = "nativo")]
pub fn aot(args: &[std::ffi::OsString]) -> Resultado {
        if args.len() < 3 {
            return Err(
                "usage: dartforge aot <input.dart> <output.exe> [--optimize] [--merge-identical-functions] [--timings] [--link-object <path>]".into(),
            );
        }
        let mut optimize = false;
        let mut merge_identical_functions = false;
        let mut timings = false;
        let mut objects = Vec::new();
        let mut flags = args[3..].iter();
        while let Some(flag) = flags.next() {
            if flag == "--optimize" && !optimize {
                optimize = true;
            } else if flag == "--merge-identical-functions" && !merge_identical_functions {
                merge_identical_functions = true;
            } else if flag == "--timings" && !timings {
                timings = true;
            } else if flag == "--link-object" {
                let path = flags
                    .next()
                    .ok_or("--link-object exige caminho de objeto nativo")?;
                objects.push(PathBuf::from(path));
            } else {
                return Err(format!(
                    "opção AOT desconhecida ou repetida: {}",
                    flag.to_string_lossy()
                )
                .into());
            }
        }
        let total_start = std::time::Instant::now();
        let input = PathBuf::from(&args[1]);
        let output = PathBuf::from(&args[2]);
        let frontend_start = std::time::Instant::now();
        let ir = dartforge_compiler::compile_path_llvm_with_options(
            &input,
            dartforge_compiler::CompileOptions {
                merge_identical_functions,
                ..Default::default()
            },
        )?;
        let frontend = frontend_start.elapsed();
        let options = dartforge_native::NativeOptions {
            optimize,
            ..Default::default()
        };
        if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let report = dartforge_native::build_executable_with_report_and_objects(
            &ir, &output, &options, &objects,
        )?;
        let total = total_start.elapsed();
        if timings {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "schema_version": 1, "backend": "llvm", "merge_identical_functions": merge_identical_functions, "optimization": if optimize { "O2" } else { "O0" },
                    "frontend_ns": frontend.as_nanos(), "prepare_ns": report.write_ir_runtime.as_nanos(),
                    "clang_ns": report.clang.as_nanos(), "rustc_link_ns": report.rustc_link.as_nanos(),
                    "publish_ns": report.publish.as_nanos(), "driver_total_ns": report.total.as_nanos(),
                    "total_ns": total.as_nanos(), "executable_bytes": report.executable_bytes,
                }))?
            );
        } else {
            println!(
                "{} -> {} (AOT LLVM {})",
                input.display(),
                output.display(),
                if optimize { "O2" } else { "O0" }
            );
        }
        return Ok(());
}

#[cfg(feature = "nativo")]
/// Compila a entrada para LLVM IR e a executa em memória pelo JIT ORCv2.
///
/// É o perfil de desenvolvimento: nada é gravado em disco e o programa executa
/// dentro deste processo. O perfil de produção continua sendo `dartforge aot`,
/// que produz um executável ligado ao runtime Rust. Os dois consomem o mesmo IR.
///
/// `--timings` imprime o custo por fase em JSON, depois da saída do programa,
/// no mesmo formato de campos `*_ns` usado por `dartforge aot --timings`.
///
/// # Erros
/// Propaga diagnósticos do compilador e falhas da sessão JIT, incluindo IR
/// recusado pelo LLVM e símbolo de entrada ausente.
pub fn run_jit(_args: &[std::ffi::OsString]) -> Result<(), Box<dyn std::error::Error>> {
    Err("JIT desabilitado temporariamente durante desenvolvimento AOT".into())
}

#[cfg(feature = "nativo")]
pub fn run_hot_reload(_args: &[std::ffi::OsString]) -> Result<(), Box<dyn std::error::Error>> {
    Err("JIT reload desabilitado temporariamente durante desenvolvimento AOT".into())
}

#[cfg(feature = "nativo")]
pub fn run_compile_native(args: &[std::ffi::OsString]) -> Result<(), Box<dyn std::error::Error>> {
    let usage = "usage: dartforge compile-native <input.dart> -o <output.exe> [--sdk <lib>] [--packages <package_config.json>] [--timings] [--optimize]";
    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut timings = false;
    let mut optimize = false;

    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.to_str() {
            Some("-o") => out = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--sdk") => sdk = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--packages") => packages = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--timings") => timings = true,
            Some("--optimize") => optimize = true,
            _ if input.is_none() => input = Some(PathBuf::from(a)),
            _ => return Err(usage.into()),
        }
    }
    let (Some(input), Some(out)) = (input, out) else {
        return Err(usage.into());
    };

    let (i2, o2) = (input.clone(), out.clone());
    let s2 = sdk.clone();
    let p2 = packages.clone();
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let options = dartforge_emit_native::CompileOptions {
                sdk: s2.as_deref(),
                packages: p2.as_deref(),
                timings,
                optimize,
            };
            dartforge_emit_native::compilar(&i2, &o2, &options)
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "a compilação nativa abortou".to_string())??;

    Ok(())
}

