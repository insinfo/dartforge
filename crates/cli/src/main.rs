//! Interface de linha de comando do compilador DartForge.
use std::{env, fs, path::PathBuf, process::ExitCode};
/// Lê os argumentos, compila a entrada e grava uma saída que ainda não existe.
///
/// # Erros
///
/// Propaga argumentos inválidos, erros de leitura/escrita e diagnósticos do compilador.
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.is_empty() || args[0] == "--help" {
        println!(
            "DartForge\nUsage: dartforge compile <input.dart> <output.mjs> [--optimize]\n       dartforge graph <input.dart>\nSubconjunto: funções tipadas, variáveis, expressões, condicionais, laços e print."
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
                let imports: Vec<_> = unit
                    .imports
                    .iter()
                    .map(|import| {
                        serde_json::json!({
                            "uri": import.uri, "target": import.target,
                            "span": {"start": import.span.start, "end": import.span.end}
                        })
                    })
                    .collect();
                serde_json::json!({"id": id, "path": unit.path, "imports": imports})
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
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)?;
    file.write_all(js.as_bytes())?;
    println!(
        "{} -> {} ({} bytes)",
        input.display(),
        output.display(),
        js.len()
    );
    Ok(())
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
