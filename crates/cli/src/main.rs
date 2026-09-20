//! Interface de linha de comando do compilador DartForge.
use std::{env, fs, path::PathBuf, process::ExitCode};
/// Serializa arestas e filtros mantendo a ordem das diretivas da fonte.
fn edges_json(edges: &[dartforge_packages::Import]) -> Vec<serde_json::Value> {
    edges
        .iter()
        .map(|edge| {
            let filters: Vec<_> = edge
                .combinators
                .iter()
                .map(|filter| match filter {
                    dartforge_packages::Combinator::Show(names) => {
                        serde_json::json!({"show": names})
                    }
                    dartforge_packages::Combinator::Hide(names) => {
                        serde_json::json!({"hide": names})
                    }
                })
                .collect();
            serde_json::json!({"uri": edge.uri, "target": edge.target, "combinators": filters,
            "span": {"start": edge.span.start, "end": edge.span.end}})
        })
        .collect()
}
/// Lê os argumentos, compila a entrada e grava uma saída que ainda não existe.
///
/// # Erros
///
/// Propaga argumentos inválidos, erros de leitura/escrita e diagnósticos do compilador.
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "abi-info") {
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
                "ffi_execution_implemented":false,"wasm_emission_implemented":false,
                "note":"Contrato de tipos/ABI; o driver AOT atual compila somente para o host. Não carrega bibliotecas nem compila dart:ffi."
            }))?
        );
        return Ok(());
    }
    if args.is_empty() || args[0] == "--help" {
        println!(
            "DartForge\nUsage: dartforge compile <input.dart> <output.mjs> [--optimize] [--merge-identical-functions]\n       dartforge emit-llvm <input.dart> <output.ll> [--merge-identical-functions]\n       dartforge aot <input.dart> <output.exe> [--optimize] [--merge-identical-functions] [--timings]\n       dartforge abi-info <windows-x64|linux-x64|wasm32>\n       dartforge graph <input.dart> [--target js|native|wasm]\nSubconjunto: funções tipadas, variáveis, expressões, condicionais, laços e print."
        );
        return Ok(());
    }
    if args[0] == "graph" {
        if args.len() < 2 {
            return Err("usage: dartforge graph <input.dart> [--target js|native|wasm]".into());
        }
        let environment = graph_environment(&args[2..])?;
        let graph = dartforge_packages::load_with_environment(
            std::path::Path::new(&args[1]),
            &environment,
        )?;
        let units: Vec<_> = graph
            .units
            .iter()
            .enumerate()
            .map(|(id, unit)| {
                serde_json::json!({"id": id, "path": unit.path,
                    "imports": edges_json(&unit.imports), "exports": edges_json(&unit.exports)})
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "entry": graph.entry, "units": units,
                "target": format!("{:?}", environment.target()),
                "dart_library_flags": environment.library_flags(),
                "sdk_profile_version": "3.6.2",
                "wasm_emission_implemented": false,
                "note": "Grafo selecionado pelo perfil do SDK. As flags não implementam APIs dart:; o subconjunto atual oferece somente dart:core. Wasm permite inspeção do grafo, sem emissão."
            }))?
        );
        return Ok(());
    }
    if args[0] == "aot" {
        if args.len() < 3 {
            return Err(
                "usage: dartforge aot <input.dart> <output.exe> [--optimize] [--merge-identical-functions] [--timings]".into(),
            );
        }
        let mut optimize = false;
        let mut merge_identical_functions = false;
        let mut timings = false;
        for flag in &args[3..] {
            if flag == "--optimize" && !optimize {
                optimize = true;
            } else if flag == "--merge-identical-functions" && !merge_identical_functions {
                merge_identical_functions = true;
            } else if flag == "--timings" && !timings {
                timings = true;
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
        let report = dartforge_native::build_executable_with_report(&ir, &output, &options)?;
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
    if args.len() < 3 || (args[0] != "compile" && args[0] != "emit-llvm") {
        return Err("usage: dartforge compile|emit-llvm <input> <output> [--optimize] [--merge-identical-functions]".into());
    }
    let mut options = dartforge_compiler::CompileOptions::default();
    for flag in &args[3..] {
        if flag == "--optimize"
            && options.optimization == dartforge_compiler::Optimization::None
            && args[0] == "compile"
        {
            options.optimization = dartforge_compiler::Optimization::Constants;
        } else if flag == "--merge-identical-functions" && !options.merge_identical_functions {
            options.merge_identical_functions = true;
        } else {
            return Err(
                format!("opção desconhecida ou repetida: {}", flag.to_string_lossy()).into(),
            );
        }
    }
    let input = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);
    if args[0] == "emit-llvm" {
        let ir = dartforge_compiler::compile_path_llvm_with_options(&input, options)?;
        write_new(&output, &ir)?;
        return Ok(());
    }
    let js = dartforge_compiler::compile_path_with_options(&input, options)?;
    write_new(&output, &js)?;
    println!(
        "{} -> {} ({} bytes)",
        input.display(),
        output.display(),
        js.len()
    );
    Ok(())
}
/// Grava um artefato textual sem sobrescrever arquivos existentes.
fn write_new(output: &std::path::Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(text.as_bytes())
}
/// Converte o resultado do comando em mensagem e código de saída do processo.
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Seleciona um perfil de inspeção sem alterar o backend fixo dos comandos de emissão.
fn graph_environment(
    flags: &[std::ffi::OsString],
) -> Result<dartforge_packages::CompilationEnvironment, Box<dyn std::error::Error>> {
    use dartforge_packages::CompilationEnvironment;
    let target = match flags {
        [] => "js",
        [flag, target] if flag == "--target" => target.to_str().ok_or("alvo deve ser UTF-8")?,
        [flag] => flag
            .to_str()
            .and_then(|s| s.strip_prefix("--target="))
            .ok_or("usage: graph <input.dart> [--target js|native|wasm]")?,
        _ => return Err("usage: graph <input.dart> [--target js|native|wasm]".into()),
    };
    match target {
        "js" => Ok(CompilationEnvironment::javascript()),
        "native" => Ok(CompilationEnvironment::native()),
        "wasm" => Ok(CompilationEnvironment::wasm()),
        _ => Err(format!("perfil desconhecido: {target}; use js, native ou wasm").into()),
    }
}

#[cfg(test)]
mod environment_tests {
    use super::*;
    /// A inspeção Wasm não muda os comandos de emissão e rejeita opções ambíguas.
    #[test]
    fn graph_profile_arguments_are_explicit() {
        use dartforge_packages::CompilationTarget;
        assert_eq!(
            graph_environment(&[]).unwrap().target(),
            CompilationTarget::JavaScript
        );
        assert_eq!(
            graph_environment(&["--target".into(), "native".into()])
                .unwrap()
                .target(),
            CompilationTarget::Native
        );
        assert_eq!(
            graph_environment(&["--target=wasm".into()])
                .unwrap()
                .target(),
            CompilationTarget::Wasm
        );
        for flags in [
            vec!["--target"],
            vec!["--target", "unknown"],
            vec!["--target=wasm", "--target=js"],
            vec!["-Dfoo=true"],
        ] {
            assert!(
                graph_environment(&flags.into_iter().map(Into::into).collect::<Vec<_>>()).is_err()
            );
        }
    }
}
