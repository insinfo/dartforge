//! Interface de linha de comando do compilador DartForge.
use std::{env, fs, path::PathBuf, process::ExitCode};
mod analisar;
mod jit;
mod motor;
mod nativo;
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "abi-info") {
        return nativo::abi_info(&args);
    }
    if args.is_empty() || args[0] == "--help" {
        println!(
            "DartForge - compilador Dart para JavaScript\nUsage: dartforge compile-js <input.dart> -o <dir> [--sdk <lib>] [--packages <cfg>] [--timings]\n       dartforge dev <input.dart> -o <dir> [--packages <cfg>] [--sdk <lib>] [--intervalo <ms>] [--uma-vez]
       dartforge serve <input.dart> -o <dir> [--web <dir>] [--porta N] [--packages <cfg>]\n       dartforge build [<entrada.dart>] [--raiz <dir>] [--plano] [--comparar] [--release] [--estrito] [--trabalhadores N] [--escrever-cache <dir>]\n       dartforge aot|abi-info ...  (compile com --features nativo)
       dartforge run|reload <input.dart> ...  (compile com --features jit)\n       dartforge analyze [--format=json] [--todos] [<dir|arquivo>]  (o JSON do dart analyze)\n\ncompile-js emite um modulo ES por biblioteca no contrato do DDC.\ndev mantem a sessao viva e recompila so o que a edicao afeta."
        );
        return Ok(());
    }
    if args[0] == "dev" {
        return run_dev(&args[1..], false);
    }
    if args[0] == "serve" {
        return run_dev(&args[1..], true);
    }
    if args[0] == "compile-native" { return nativo::run_compile_native(&args[1..]); }
    if args[0] == "build" {
        return motor::run_build(&args[1..]);
    }
    if args[0] == "compile-js" {
        return run_compile_js(&args[1..]);
    }
    if args[0] == "run" {
        return jit::run(&args[1..]);
    }
    if args[0] == "reload" {
        return jit::reload(&args[1..]);
    }
    if args[0] == "aot" {
        return nativo::aot(&args);
    }
    Err(format!("comando desconhecido: {}", args[0].to_string_lossy()).into())
}

/// Converte o resultado do comando em mensagem e código de saída do processo.
fn main() -> ExitCode {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.first().is_some_and(|a| a == "analyze") {
        return match analisar::run(&args[1..]) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::from(64)
            }
        };
    }
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
    let usage = "usage: dartforge compile-js <input.dart> -o <dir> [--sdk <lib>] [--packages <package_config.json>] [--timings] [--versao-linguagem x.y] [--enable-experiment=a,b]";
    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut timings = false;
    let mut linguagem = dartforge_emit_js::Linguagem::default();
    let textos: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
    let mut it = textos.iter().map(String::as_str);
    while let Some(a) = it.next() {
        if linguagem.ler_opcao(a, &mut it)? {
            continue;
        }
        match a {
            "-o" => out = Some(PathBuf::from(it.next().ok_or(usage)?)),
            "--sdk" => sdk = Some(PathBuf::from(it.next().ok_or(usage)?)),
            "--packages" => packages = Some(PathBuf::from(it.next().ok_or(usage)?)),
            "--timings" => timings = true,
            _ if input.is_none() => input = Some(PathBuf::from(a)),
            _ => return Err(usage.into()),
        }
    }
    let (Some(input), Some(out)) = (input, out) else { return Err(usage.into()) };
    // Corpos profundos (cadeias longas de `+`, árvores de widgets) recursam fundo: pilha própria.
    let (i2, s2, p2) = (input.clone(), sdk.clone(), packages.clone());
    let (emitido, mut relatorio, rel_motor) = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || -> Result<_, String> {
            // Motor de build só se o projeto usa builders (custo zero).
            let motor = motor::detectar(&i2, p2.as_deref());
            let rel_motor = std::cell::RefCell::new(None);
            let gerar = |programa: &dartforge_elements::model::Program, nomes: &dartforge_intern::Interner| {
                let (raiz, cfg) = motor.as_ref().ok_or("sem motor")?;
                match motor::gerar_uma_vez(raiz, cfg, programa, nomes) {
                    Ok((g, texto)) => {
                        *rel_motor.borrow_mut() = Some(texto);
                        Ok(g)
                    }
                    // O motor não é pré-requisito: sem ele, o carregador lê
                    // o que estiver no disco, como antes.
                    Err(e) => {
                        *rel_motor.borrow_mut() = Some(format!("aviso: motor de build desligado: {e}"));
                        Ok(std::sync::Arc::new(dartforge_elements::gerado::Geracao::default()))
                    }
                }
            };
            let gerador: Option<dartforge_emit_js::Gerador<'_>> = motor.as_ref().map(|_| &gerar as _);
            let (e, r) = dartforge_emit_js::compilar_com_relatorio_e_gerador(&i2, s2.as_deref(), p2.as_deref(), &linguagem, gerador)?;
            Ok((e, r, rel_motor.into_inner()))
        })
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
    if let Some(texto) = rel_motor {
        println!("{texto}");
    }
    if timings {
        print!("{}", relatorio.texto());
    }
    Ok(())
}

/// `dev`: compilador residente. Compila uma vez, observa os arquivos do
/// projeto por mtime e recompila o mínimo, gravando só os módulos cujo texto
/// mudou (PLANO.md, "o compilador residente").
fn run_dev(args: &[std::ffi::OsString], servir: bool) -> Result<(), Box<dyn std::error::Error>> {
    let usage = if servir {
        "usage: dartforge serve <input.dart> -o <dir> [--web <dir>] [--porta N] [--packages <cfg>] [--sdk <lib>] [--intervalo <ms>]"
    } else {
        "usage: dartforge dev <input.dart> -o <dir> [--packages <package_config.json>] [--sdk <lib>] [--intervalo <ms>] [--uma-vez]"
    };
    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut intervalo_ms = 200u64;
    let mut uma_vez = false;
    let mut web: Option<PathBuf> = None;
    let mut porta = 8080u16;
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
            Some("--web") => web = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--porta") => {
                porta = it
                    .next()
                    .and_then(|v| v.to_str())
                    .and_then(|v| v.parse().ok())
                    .ok_or("--porta exige um número")?;
            }
            _ if input.is_none() => input = Some(PathBuf::from(a)),
            _ => return Err(usage.into()),
        }
    }
    let (Some(input), Some(out)) = (input, out) else { return Err(usage.into()) };

    // Corpos profundos recursam fundo: a sessão roda numa thread com pilha própria.
    let saida = out.clone();
    let entrada = input.clone();
    let web = web.clone();
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
            // `serve`: o servidor sobe depois da primeira compilação, para que
            // a primeira visita já encontre a saída pronta.
            let recarga = std::sync::Arc::new(dartforge_dev::servidor::Recarga::default());
            if servir {
                let p = dartforge_dev::servidor::servir_com_gerados(
                    &saida,
                    web.as_deref(),
                    porta,
                    recarga.clone(),
                    sessao.provedor_de_gerados(),
                )?;
                println!("servindo em http://127.0.0.1:{p}/ (recarga automática por WebSocket)");
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
                    Ok(rel) => {
                        print!("{}", rel.texto());
                        if servir {
                            // Só recarrega o navegador se algum módulo mudou.
                            // Módulo novo, ou saída gerada servida (um `.css`).
                            let gerou = rel.motor.as_ref().is_some_and(|m| m.saidas_alteradas > 0);
                            if rel.modulos_escritos > 0 || gerou {
                                recarga.disparar();
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("erro: {e}");
                        if servir {
                            // O navegador mostra o erro no console e segue
                            // ligado: a próxima compilação boa recarrega.
                            recarga.erro(&e);
                        }
                    }
                }
            }
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "a sessão abortou")??;
    Ok(())
}
