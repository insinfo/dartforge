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
    if args.is_empty() || args[0] == "--help" {
        println!(
            "DartForge\nUsage: dartforge compile <input.dart> <output.mjs> [--optimize]\n       dartforge emit-llvm <input.dart> <output.ll>\n       dartforge aot <input.dart> <output.exe> [--optimize] [--timings]\n       dartforge graph <input.dart>\nSubconjunto: funções tipadas, variáveis, expressões, condicionais, laços e print."
        );
        return Ok(());
    }
    if args.len() == 2 && args[0] == "graph" {
        let graph = dartforge_packages::load(std::path::Path::new(&args[1]))?;
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
                "entry": graph.entry, "units": units,
                "note": "Grafo de arquivos; use compile para resolver e compilar as bibliotecas."
            }))?
        );
        return Ok(());
    }
    if args[0] == "aot" {
        if args.len() < 3 {
            return Err(
                "usage: dartforge aot <input.dart> <output.exe> [--optimize] [--timings]".into(),
            );
        }
        let mut optimize = false;
        let mut timings = false;
        for flag in &args[3..] {
            if flag == "--optimize" && !optimize {
                optimize = true;
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
        let ir = dartforge_compiler::compile_path_llvm(&input)?;
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
                    "schema_version": 1, "backend": "llvm", "optimization": if optimize { "O2" } else { "O0" },
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
    if args.len() == 3 && args[0] == "emit-llvm" {
        let ir = dartforge_compiler::compile_path_llvm(std::path::Path::new(&args[1]))?;
        write_new(std::path::Path::new(&args[2]), &ir)?;
        return Ok(());
    }
    let optimized = args.len() == 4 && args[3] == "--optimize";
    if (args.len() != 3 && !optimized) || args[0] != "compile" {
        return Err("usage: dartforge compile <input.dart> <output.mjs> [--optimize]".into());
    }
    let input = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);

    let mode = if optimized {
        dartforge_compiler::Optimization::Constants
    } else {
        dartforge_compiler::Optimization::None
    };
    let js = dartforge_compiler::compile_path(&input, mode)?;
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
