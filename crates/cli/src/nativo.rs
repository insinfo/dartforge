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
pub fn run_jit(args: &[std::ffi::OsString]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        return Err(
            "usage: dartforge run <input.dart> [--merge-identical-functions] [--timings]".into(),
        );
    }
    let mut merge_identical_functions = false;
    let mut timings = false;
    for flag in &args[1..] {
        if flag == "--merge-identical-functions" && !merge_identical_functions {
            merge_identical_functions = true;
        } else if flag == "--timings" && !timings {
            timings = true;
        } else {
            return Err(format!(
                "opção de execução desconhecida ou repetida: {}",
                flag.to_string_lossy()
            )
            .into());
        }
    }
    let total_start = std::time::Instant::now();
    let input = PathBuf::from(&args[0]);
    let frontend_start = std::time::Instant::now();
    let ir = dartforge_compiler::compile_path_llvm_with_options(
        &input,
        dartforge_compiler::CompileOptions {
            merge_identical_functions,
            ..Default::default()
        },
    )?;
    let frontend = frontend_start.elapsed();
    let report = dartforge_jit::run_ir(&ir)?;
    let total = total_start.elapsed();
    if timings {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 1, "backend": "llvm-orcv2", "profile": "jit",
                "merge_identical_functions": merge_identical_functions,
                "frontend_ns": frontend.as_nanos(), "session_ns": report.session.as_nanos(),
                "parse_ir_ns": report.module.parse_ir.as_nanos(),
                "add_module_ns": report.module.add_module.as_nanos(),
                "lookup_ns": report.entry.lookup.as_nanos(),
                "execute_ns": report.entry.execute.as_nanos(),
                "jit_total_ns": report.total.as_nanos(), "total_ns": total.as_nanos(),
                "ir_bytes": report.module.ir_bytes,
            }))?
        );
    }
    Ok(())
}

#[cfg(feature = "nativo")]
/// Executa a primeira entrada e recarrega a sessão com as edições seguintes.
///
/// É o laço de desenvolvimento em forma de comando: a sessão JIT fica aberta, o
/// heap gerenciado é preservado entre as versões e cada arquivo seguinte é uma
/// edição publicada por hot reload. Sem `--timings` imprime apenas a saída do
/// programa; com `--timings`, um objeto JSON por recarga, com o custo de cada
/// etapa do ciclo — o front-end medido aqui, o resto medido dentro da sessão.
///
/// # Erros
/// Propaga diagnósticos do compilador e da sessão JIT. Uma recarga recusada
/// (contrato incompatível, IR inválido) encerra o comando com erro, e a versão
/// anterior continuaria valendo se o laço prosseguisse.
pub fn run_hot_reload(args: &[std::ffi::OsString]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        return Err(
            "usage: dartforge reload <inicial.dart> <edicao.dart> [<edicao.dart>...] [--timings]"
                .into(),
        );
    }
    let timings = args.last().is_some_and(|flag| flag == "--timings");
    let arquivos = &args[..args.len() - usize::from(timings)];
    if arquivos.len() < 2 {
        return Err("reload exige a versão inicial e pelo menos uma edição".into());
    }
    let frontend_start = std::time::Instant::now();
    let ir = dartforge_compiler::compile_path_llvm(std::path::Path::new(&arquivos[0]))?;
    let frontend = frontend_start.elapsed();
    let mut session = dartforge_jit::JitSession::new()?;
    let inicial = session.add_reloadable_module("app", &ir)?;
    let entrada = session.run_entry()?;
    if timings {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "schema_version": 1, "backend": "llvm-orcv2", "profile": "jit-reload",
                "file": arquivos[0].to_string_lossy(), "generation": inicial.generation,
                "frontend_ns": frontend.as_nanos(), "parse_ir_ns": inicial.parse_ir.as_nanos(),
                "contract_ns": inicial.contract.as_nanos(),
                "add_module_ns": inicial.add_module.as_nanos(),
                "stubs_ns": inicial.stubs.as_nanos(), "link_ns": inicial.link.as_nanos(),
                "publish_ns": inicial.publish.as_nanos(), "retire_ns": inicial.retire.as_nanos(),
                "reload_total_ns": inicial.total.as_nanos(),
                "execute_ns": entrada.execute.as_nanos(), "ir_bytes": inicial.ir_bytes,
                "entries": inicial.entries, "retained_generations": inicial.retained_generations,
            }))?
        );
    }
    for arquivo in &arquivos[1..] {
        let frontend_start = std::time::Instant::now();
        let ir = dartforge_compiler::compile_path_llvm(std::path::Path::new(arquivo))?;
        let frontend = frontend_start.elapsed();
        let relatorio = session.hot_reload("app", &ir)?;
        let entrada = session.run_entry()?;
        if timings {
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({
                    "schema_version": 1, "backend": "llvm-orcv2", "profile": "jit-reload",
                    "file": arquivo.to_string_lossy(), "generation": relatorio.generation,
                    "frontend_ns": frontend.as_nanos(), "parse_ir_ns": relatorio.parse_ir.as_nanos(),
                    "contract_ns": relatorio.contract.as_nanos(),
                    "add_module_ns": relatorio.add_module.as_nanos(),
                    "stubs_ns": relatorio.stubs.as_nanos(), "link_ns": relatorio.link.as_nanos(),
                    "publish_ns": relatorio.publish.as_nanos(),
                    "retire_ns": relatorio.retire.as_nanos(),
                    "reload_total_ns": relatorio.total.as_nanos(),
                    "execute_ns": entrada.execute.as_nanos(), "ir_bytes": relatorio.ir_bytes,
                    "entries": relatorio.entries, "new_entries": relatorio.new_entries,
                    "retained_generations": relatorio.retained_generations,
                }))?
            );
        }
    }
    Ok(())
}

