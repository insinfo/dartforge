//! Interface de linha de comando do compilador DartForge.
use std::{env, fs, path::PathBuf, process::ExitCode};
mod nativo;
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
        return nativo::abi_info(&args);
    }
    if args.is_empty() || args[0] == "--help" {
        println!(
            "DartForge\nUsage: dartforge compile <input.dart> <output.mjs> [--optimize] [--merge-identical-functions] [--tree-shake|--no-tree-shake] [--timings]\n       dartforge watch <input.dart> <output.mjs> [--optimize] [--merge-identical-functions] [--tree-shake] [--interval <ms>]\n       dartforge emit-llvm <input.dart> <output.ll> [--merge-identical-functions]\n       dartforge aot <input.dart> <output.exe> [--optimize] [--merge-identical-functions] [--timings] [--link-object <path>]\n       dartforge run <input.dart> [--merge-identical-functions] [--timings]\n       dartforge reload <inicial.dart> <edicao.dart> [<edicao.dart>...] [--timings]\n       dartforge compile-js <input.dart> -o <dir> [--sdk <lib>] [--packages <package_config.json>]
       dartforge dev <input.dart> -o <dir> [--packages <package_config.json>] [--sdk <lib>] [--intervalo <ms>] [--uma-vez]\n       dartforge abi-info <windows-x64|linux-x64|wasm32>\n       dartforge macro-info <input.dart>\n       dartforge graph <input.dart> [--target js|native|wasm]\nSubconjunto: funções tipadas, variáveis, expressões, condicionais, laços e print.\nrun executa em memória pelo JIT (perfil de desenvolvimento); aot produz executável (perfil de produção)."
        );
        return Ok(());
    }
    if args[0] == "macro-info" {
        if args.len() != 2 {
            return Err("usage: dartforge macro-info <input.dart>".into());
        }
        let source = fs::read_to_string(&args[1])?;
        let report = dartforge_compiler::macro_expansion_report(&source)?;
        let phases: Vec<_> = report
            .phases
            .iter()
            .map(|phase| {
                serde_json::json!({
                    "phase":format!("{:?}",phase.phase),"applications":phase.applications,
                    "generated_declarations":phase.generated_declarations
                })
            })
            .collect();
        let origins: Vec<_> = report
            .origins
            .iter()
            .map(|origin| {
                serde_json::json!({
                    "generated": {"start":origin.generated.start,"end":origin.generated.end},
                    "annotation": {"start":origin.annotation.start,"end":origin.annotation.end}
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version":1,"experimental":true,"macro_host":"rust_builtin",
                "applications":report.applications,"generated_declarations":report.generated_declarations,
                "virtual_source_extent":report.extent,"origins":origins,
                "phases":phases,"plan_hits":report.plan_hits,"plan_misses":report.plan_misses,
                "materialized_nodes":report.materialized_nodes,
                "note":"Unidade isolada; macros Dart arbitrárias e resolução de pacotes de macros ainda não implementadas."
            }))?
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
                "note": "Grafo selecionado pelo perfil do SDK. As flags não implementam APIs dart:; há subconjuntos de dart:core e Native escalar de dart:ffi no AOT. Wasm permite inspeção do grafo, sem emissão."
            }))?
        );
        return Ok(());
    }
    if args[0] == "dev" {
        return run_dev(&args[1..]);
    }
    if args[0] == "compile-js" {
        return run_compile_js(&args[1..]);
    }
    if args[0] == "run" {
        return nativo::run_jit(&args[1..]);
    }
    if args[0] == "reload" {
        return nativo::run_hot_reload(&args[1..]);
    }
    if args[0] == "aot" {
        return nativo::aot(&args);
    }
    if args.len() < 3 || !matches!(args[0].to_str(), Some("compile" | "emit-llvm" | "watch")) {
        return Err("usage: dartforge compile|watch|emit-llvm <input> <output> [--optimize] [--merge-identical-functions] [--timings]".into());
    }
    let mut options = dartforge_compiler::CompileOptions::default();
    let mut tree_flag_seen = false;
    let mut timings = false;
    let mut interval_ms = 150u64;
    let mut flags = args[3..].iter();
    while let Some(flag) = flags.next() {
        if flag == "--optimize"
            && options.optimization == dartforge_compiler::Optimization::None
            && args[0] == "compile"
        {
            options.optimization = dartforge_compiler::Optimization::Constants;
        } else if (flag == "--tree-shake" || flag == "--no-tree-shake")
            && !tree_flag_seen
            && args[0] == "compile"
        {
            tree_flag_seen = true;
            options.tree_shaking = flag == "--tree-shake";
        } else if flag == "--merge-identical-functions" && !options.merge_identical_functions {
            options.merge_identical_functions = true;
        } else if flag == "--timings" && !timings && args[0] != "emit-llvm" {
            timings = true;
        } else if flag == "--interval" && args[0] == "watch" {
            interval_ms = flags
                .next()
                .and_then(|value| value.to_str())
                .and_then(|value| value.parse().ok())
                .filter(|value| (10..=10_000).contains(value))
                .ok_or("--interval exige um valor em milissegundos entre 10 e 10000")?;
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
    if args[0] == "watch" {
        return watch(&input, &output, options, timings, interval_ms);
    }
    if timings {
        let (js, report) = dartforge_compiler::compile_path_with_report(&input, options)?;
        write_new(&output, &js)?;
        println!(
            "{} -> {} ({} bytes)",
            input.display(),
            output.display(),
            js.len()
        );
        println!("{}", serde_json::to_string_pretty(&report_json(&report))?);
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

/// Serializa o relatório de custo por fase, com os contadores de trabalho.
///
/// Os tempos são cronometrados no próprio trecho de cada fase; nenhum é obtido
/// por subtração. Os contadores existem porque tempo sozinho não distingue reuso
/// de cache de uma máquina mais rápida.
fn report_json(report: &dartforge_compiler::CompileReport) -> serde_json::Value {
    let link = report.link;
    serde_json::json!({
        "cache_hit": report.cache_hit,
        "total_ns": report.total_ns,
        "phases_ns": {
            "load": report.load_ns, "lex": link.lex_ns, "outline": link.outline_ns,
            "namespace": link.namespace_ns, "parse": link.parse_ns, "macros": link.macros_ns,
            "merge": link.merge_ns, "analyze": link.analyze_ns, "optimize": link.optimize_ns,
            "emit": link.emit_ns
        },
        "work": {
            "units": link.units, "source_bytes": link.source_bytes, "tokens": link.tokens,
            "classes": link.classes, "functions": link.functions, "output_bytes": link.output_bytes
        }
    })
}

/// Mantém o compilador aberto e recompila quando o conteúdo das fontes muda.
///
/// Este é o cenário de "edição com o compilador já aberto": a sessão reaproveita
/// a estrutura do grafo quando só os corpos mudaram, e o arquivo de saída só é
/// reescrito quando o JavaScript realmente muda, para não acordar quem observa o
/// diretório de saída.
///
/// A detecção é por releitura e comparação de conteúdo, nunca por mtime ou
/// tamanho: uma edição que restaure os metadados continua sendo percebida. Por
/// isso o intervalo é uma sondagem, não um observador do sistema de arquivos.
///
/// # Erros
/// Erros de compilação são impressos e o laço continua; erros de escrita encerram.
fn watch(
    input: &std::path::Path,
    output: &std::path::Path,
    options: dartforge_compiler::CompileOptions,
    timings: bool,
    interval_ms: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut session = dartforge_compiler::CompilerSession::new();
    let mut last: Option<String> = None;
    println!(
        "observando {} a cada {interval_ms} ms; Ctrl+C encerra",
        input.display()
    );
    loop {
        match session.compile_path_with_options(input, options) {
            Ok(compilation) => {
                let changed = last.as_deref() != Some(&*compilation.javascript);
                if changed {
                    write_or_replace(output, &compilation.javascript)?;
                    last = Some(compilation.javascript.to_string());
                    println!(
                        "{} -> {} ({} bytes) em {:.2} ms",
                        input.display(),
                        output.display(),
                        compilation.javascript.len(),
                        compilation.report.total_ns as f64 / 1e6
                    );
                }
                if timings && changed {
                    println!(
                        "{}",
                        serde_json::to_string(&report_json(&compilation.report))?
                    );
                }
            }
            Err(error) => {
                let span = error
                    .span
                    .map(|span| format!(" ({}..{})", span.start, span.end))
                    .unwrap_or_default();
                eprintln!("{}{span}: {}", error.path.display(), error.message);
                last = None;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(interval_ms));
    }
}

/// Grava a saída substituindo o conteúdo anterior, para o laço de observação.
fn write_or_replace(output: &std::path::Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, text)
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

/// `compile-js`: emite módulos ES6 no contrato do DDC em `<dir>` e copia o `dart_sdk.js`.
fn run_compile_js(args: &[std::ffi::OsString]) -> Result<(), Box<dyn std::error::Error>> {
    let usage = "usage: dartforge compile-js <input.dart> -o <dir> [--sdk <lib>] [--packages <package_config.json>] [--timings]";
    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut timings = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.to_str() {
            Some("-o") => out = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--sdk") => sdk = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--packages") => packages = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--timings") => timings = true,
            _ if input.is_none() => input = Some(PathBuf::from(a)),
            _ => return Err(usage.into()),
        }
    }
    let (Some(input), Some(out)) = (input, out) else { return Err(usage.into()) };
    // Corpos profundos (cadeias longas de `+`, árvores de widgets) recursam fundo: pilha própria.
    let (i2, s2, p2) = (input.clone(), sdk.clone(), packages.clone());
    let (emitido, mut relatorio) = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || dartforge_emit_js::compilar_com_relatorio(&i2, s2.as_deref(), p2.as_deref()))
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "a compilação abortou")??;
    let t = std::time::Instant::now();
    let escritos = dartforge_emit_js::escrever(&emitido, &out, &dartforge_emit_js::dart_sdk_js_padrao())?;
    relatorio.fase("escrita", t);
    println!(
        "{} -> {} ({} módulos, {} arquivo(s) reescrito(s))",
        input.display(),
        out.display(),
        emitido.modulos.len(),
        escritos
    );
    if timings {
        print!("{}", relatorio.texto());
    }
    Ok(())
}

/// `dev`: compilador residente. Compila uma vez, observa os arquivos do
/// projeto por mtime e recompila o mínimo, gravando só os módulos cujo texto
/// mudou (PLANO.md, "o compilador residente").
fn run_dev(args: &[std::ffi::OsString]) -> Result<(), Box<dyn std::error::Error>> {
    let usage = "usage: dartforge dev <input.dart> -o <dir> [--packages <package_config.json>] [--sdk <lib>] [--intervalo <ms>] [--uma-vez]";
    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut intervalo_ms = 200u64;
    let mut uma_vez = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.to_str() {
            Some("-o") => out = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--sdk") => sdk = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--packages") => packages = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--intervalo") => {
                intervalo_ms = it
                    .next()
                    .and_then(|v| v.to_str())
                    .and_then(|v| v.parse().ok())
                    .filter(|v| (10..=10_000).contains(v))
                    .ok_or("--intervalo exige milissegundos entre 10 e 10000")?;
            }
            Some("--uma-vez") => uma_vez = true,
            _ if input.is_none() => input = Some(PathBuf::from(a)),
            _ => return Err(usage.into()),
        }
    }
    let (Some(input), Some(out)) = (input, out) else { return Err(usage.into()) };

    // Corpos profundos recursam fundo: a sessão roda numa thread com pilha própria.
    let saida = out.clone();
    let entrada = input.clone();
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || -> Result<(), String> {
            let mut sessao =
                dartforge_dev::Sessao::nova(&entrada, sdk.as_deref(), packages.as_deref(), &saida)?;
            let rel = sessao.compilar()?;
            println!(
                "{} -> {} ({} módulos, {} escritos)",
                entrada.display(),
                saida.display(),
                rel.modulos,
                rel.modulos_escritos
            );
            print!("{}", rel.texto());
            if uma_vez {
                return Ok(());
            }
            println!("observando {} arquivos a cada {intervalo_ms} ms (ctrl+c para sair)…", sessao.observados().len());
            loop {
                std::thread::sleep(std::time::Duration::from_millis(intervalo_ms));
                let mudados = sessao.mudancas();
                if mudados.is_empty() {
                    continue;
                }
                for p in &mudados {
                    println!("mudou: {}", p.display());
                    sessao.arquivo_mudou(p);
                }
                match sessao.compilar() {
                    Ok(rel) => print!("{}", rel.texto()),
                    Err(e) => eprintln!("erro: {e}"),
                }
            }
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "a sessão abortou")??;
    Ok(())
}
