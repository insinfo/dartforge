//! Interface de linha de comando do compilador DartForge.
use std::{env, fs, path::PathBuf, process::ExitCode};
mod nativo;
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "abi-info") {
        return nativo::abi_info(&args);
    }
    if args.is_empty() || args[0] == "--help" {
        println!(
            "DartForge - compilador Dart para JavaScript\nUsage: dartforge compile-js <input.dart> -o <dir> [--sdk <lib>] [--packages <cfg>] [--timings]\n       dartforge dev <input.dart> -o <dir> [--packages <cfg>] [--sdk <lib>] [--intervalo <ms>] [--uma-vez]\n       dartforge aot|run|reload|abi-info ...  (compile com --features nativo)\n\ncompile-js emite um modulo ES por biblioteca no contrato do DDC.\ndev mantem a sessao viva e recompila so o que a edicao afeta."
        );
        return Ok(());
    }
    if args[0] == "dev" {
        return run_dev(&args[1..]);
    }
    if args[0] == "compile-native" { return nativo::run_compile_native(&args[1..]); }
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
    Err(format!("comando desconhecido: {}", args[0].to_string_lossy()).into())
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

/// `compile-js`: pipeline inteiro e escrita dos módulos ES no diretório dado.
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
