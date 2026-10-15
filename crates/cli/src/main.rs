//! Command-line interface of the DartForge compiler.
use std::{env, path::PathBuf, process::ExitCode};
mod analyze;
#[cfg(feature = "native")]
mod pack;
mod engine;
mod format;
mod jit;
mod macros;
mod native;
mod pub_get;
mod test_runner;
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "abi-info") {
        return native::abi_info(&args);
    }
    if args.is_empty() || args[0] == "--help" {
        println!(
            "DartForge - Dart to JavaScript compiler\nUsage: dartforge compile-js <input.dart> -o <dir> [--sdk <lib>] [--packages <cfg>] [--timings]\n       dartforge dev <input.dart> -o <dir> [--packages <cfg>] [--sdk <lib>] [--interval <ms>] [--once]
       dartforge serve <input.dart> -o <dir> [--web <dir>] [--port N] [--packages <cfg>]\n       dartforge build [<input.dart>] [--root <dir>] [--plan] [--compare] [--release] [--strict] [--workers N] [--write-cache <dir>] [--dart <exe>] [--state]\n       dartforge aot|abi-info ...  (build with --features native)
       dartforge run|reload <input.dart> ...  (build with --features jit)
       dartforge pack <destination>  (the distribution: executable, runtime, SDK, linker)\n       dartforge analyze [--format=json] [--all] [<dir|file>]  (the JSON of dart analyze)\n       dartforge format [-o write|show|json|none] [--set-exit-if-changed] [-l N] <files|dirs>  (dart format 3.6.2)\n       dartforge test [<file|directory> ...] [--name <regex>] [--sdk <lib>] [--packages <cfg>]\n       dartforge pub get [--offline] [--directory <dir>] [--sdk <lib>] [--sdk-version <version>]\n       dartforge macros <input.dart> [--materialize] [--shape 3.6|current] [--dart <exe>]\n\ncompile-js emits one ES module per library under the DDC contract.\ndev keeps the session alive and recompiles only what the edit affects."
        );
        return Ok(());
    }
    if args[0] == "dev" {
        return run_dev(&args[1..], false);
    }
    if args[0] == "serve" {
        return run_dev(&args[1..], true);
    }
    if args[0] == "compile-native" { return native::run_compile_native(&args[1..]); }
    if args[0] == "build" {
        return engine::run_build(&args[1..]);
    }
    if args[0] == "compile-js" {
        return run_compile_js(&args[1..]);
    }
    if args[0] == "macros" {
        return macros::run(&args[1..]);
    }
    if args[0] == "run" {
        return jit::run(&args[1..]);
    }
    if args[0] == "reload" {
        return jit::reload(&args[1..]);
    }
    if args[0] == "aot" {
        return native::aot(&args);
    }
    if args[0] == "pack" {
        #[cfg(feature = "native")]
        return pack::run(&args[1..]);
        #[cfg(not(feature = "native"))]
        return Err("pack: build with --features native".into());
    }
    Err(format!("unknown command: {}", args[0].to_string_lossy()).into())
}

/// Turns the command's result into a message and the process exit code.
fn main() -> ExitCode {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.first().is_some_and(|a| a == "test") {
        return match test_runner::run(&args[1..]) {
            Ok(code) => code,
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::FAILURE
            }
        };
    }
    if args.first().is_some_and(|a| a == "pub") {
        if args.get(1).is_none_or(|a| a != "get") {
            eprintln!("error: usage: dartforge pub get [--offline] [--directory <dir>] [--sdk <lib>] [--sdk-version <version>]");
            return ExitCode::from(64);
        }
        return match pub_get::run(&args[2..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::FAILURE
            }
        };
    }
    if args.first().is_some_and(|a| a == "format") {
        return format::run(&args[1..]);
    }
    if args.first().is_some_and(|a| a == "analyze") {
        return match analyze::run(&args[1..]) {
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

/// `compile-js`: the whole pipeline and the writing of the ES modules into the given directory.
fn run_compile_js(args: &[std::ffi::OsString]) -> Result<(), Box<dyn std::error::Error>> {
    let usage = "usage: dartforge compile-js <input.dart> -o <dir> [--sdk <lib>] [--packages <package_config.json>] [--timings] [--language-version x.y] [--enable-experiment=a,b]";
    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut timings = false;
    let mut language = dartforge_emit_js::LanguageOptions::default();
    let texts: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
    let mut it = texts.iter().map(String::as_str);
    while let Some(a) = it.next() {
        if language.read_option(a, &mut it)? {
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
    // Deep bodies (long chains of `+`, widget trees) recurse deeply: own stack.
    let (i2, s2, p2) = (input.clone(), sdk.clone(), packages.clone());
    let (emitted, mut report, engine_report) = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || -> Result<_, String> {
            // Build engine only if the project uses builders (zero cost).
            let engine = engine::detect(&i2, p2.as_deref());
            let engine_report = std::cell::RefCell::new(None);
            let generate = |program: &dartforge_elements::model::Program, names: &dartforge_intern::Interner| {
                let (root, cfg, cfg_path) = engine.as_ref().ok_or("no engine")?;
                match engine::generate_once(root, cfg, cfg_path, program, names) {
                    Ok((g, text)) => {
                        *engine_report.borrow_mut() = Some(text);
                        Ok(g)
                    }
                    // An engine error (a builder that failed, an unreadable
                    // configuration) fails the compilation: an empty generation
                    // would make the loader read old outputs from disk as if they
                    // belonged to this compilation (DF-BUILD-005).
                    Err(e) => Err(format!("build engine: {e}")),
                }
            };
            let generator: Option<dartforge_emit_js::Generator<'_>> = engine.as_ref().map(|_| &generate as _);
            let (e, r) = dartforge_emit_js::compile_with_report_and_generator(&i2, s2.as_deref(), p2.as_deref(), &language, generator)?;
            Ok((e, r, engine_report.into_inner()))
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "the compilation aborted")??;
    let t = std::time::Instant::now();
    let written = dartforge_emit_js::write(&emitted, &out, &dartforge_emit_js::default_dart_sdk_js())?;
    report.phase("write", t);
    println!(
        "{} -> {} ({} modules, {} file(s) rewritten)",
        input.display(),
        out.display(),
        emitted.modules.len(),
        written
    );
    if let Some(text) = engine_report {
        println!("{text}");
    }
    if timings {
        print!("{}", report.text());
    }
    Ok(())
}

/// `dev`: resident compiler. Compiles once, watches the project files by
/// mtime and recompiles the minimum, writing only the modules whose text
/// changed (PLANO.md, "o compilador residente").
fn run_dev(args: &[std::ffi::OsString], serve: bool) -> Result<(), Box<dyn std::error::Error>> {
    let usage = if serve {
        "usage: dartforge serve <input.dart> -o <dir> [--web <dir>] [--port N] [--packages <cfg>] [--sdk <lib>] [--interval <ms>]"
    } else {
        "usage: dartforge dev <input.dart> -o <dir> [--packages <package_config.json>] [--sdk <lib>] [--interval <ms>] [--once]"
    };
    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut interval_ms = 200u64;
    let mut once = false;
    let mut web: Option<PathBuf> = None;
    let mut port = 8080u16;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.to_str() {
            Some("-o") => out = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--sdk") => sdk = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--packages") => packages = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--interval") => {
                interval_ms = it
                    .next()
                    .and_then(|v| v.to_str())
                    .and_then(|v| v.parse().ok())
                    .filter(|v| (10..=10_000).contains(v))
                    .ok_or("--interval requires milliseconds between 10 and 10000")?;
            }
            Some("--once") => once = true,
            Some("--web") => web = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--port") => {
                port = it
                    .next()
                    .and_then(|v| v.to_str())
                    .and_then(|v| v.parse().ok())
                    .ok_or("--port requires a number")?;
            }
            _ if input.is_none() => input = Some(PathBuf::from(a)),
            _ => return Err(usage.into()),
        }
    }
    let (Some(input), Some(out)) = (input, out) else { return Err(usage.into()) };

    // Deep bodies recurse deeply: the session runs on a thread with its own stack.
    let output = out.clone();
    let entry = input.clone();
    let web = web.clone();
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || -> Result<(), String> {
            let mut session =
                dartforge_dev::Session::new(&entry, sdk.as_deref(), packages.as_deref(), &output)?;
            let rep = session.compile()?;
            println!(
                "{} -> {} ({} modules, {} written)",
                entry.display(),
                output.display(),
                rep.modules,
                rep.written_modules
            );
            print!("{}", rep.text());
            if once {
                return Ok(());
            }
            // `serve`: the server starts after the first compilation, so that
            // the first visit already finds the output ready.
            let reload = std::sync::Arc::new(dartforge_dev::server::Reload::default());
            if serve {
                let p = dartforge_dev::server::serve_with_generated(
                    &output,
                    web.as_deref(),
                    port,
                    reload.clone(),
                    session.generated_provider(),
                )?;
                println!("serving at http://127.0.0.1:{p}/ (automatic reload over WebSocket)");
            }
            println!("watching {} files every {interval_ms} ms (ctrl+c to quit)…", session.watched().len());
            loop {
                std::thread::sleep(std::time::Duration::from_millis(interval_ms));
                let changed = session.changes();
                if changed.is_empty() {
                    continue;
                }
                for p in &changed {
                    println!("changed: {}", p.display());
                    session.file_changed(p);
                }
                match session.compile() {
                    Ok(rep) => {
                        print!("{}", rep.text());
                        if serve {
                            // Only reload the browser if some module changed:
                            // a new module, or a served generated output (a `.css`).
                            let generated = rep.engine.as_ref().is_some_and(|m| m.changed_outputs > 0);
                            if rep.written_modules > 0 || generated {
                                reload.trigger();
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        if serve {
                            // The browser shows the error in the console and stays
                            // connected: the next good compilation reloads.
                            reload.error(&e);
                        }
                    }
                }
            }
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "the session aborted")??;
    Ok(())
}
