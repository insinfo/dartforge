//! `dartforge-jsprod` — compilation in the production profile.
//!
//! A separate binary, not a `dartforge` subcommand, because
//! `crates/cli/src/main.rs` was another agent's territory
//! (`docs/historico/briefs/BRIEF-JS-PRODUCAO.md` §0.2).

use std::path::PathBuf;

use dartforge_emit_js_production::{Options, compile};

const USAGE: &str = "usage: dartforge-jsprod <input.dart> -o <output.js> \
[--sdk <lib>] [--packages <package_config.json>] [--dart-sdk-js <file>] \
[--sem-poda] [--sem-membros] [--sem-poda-usuario] [--sem-minificar] [--verificar-stub] [--sdk-proprio|--sdk-ddc] [--enable-asserts] [--omitir-checagens|-O4] \
[--versao-linguagem x.y] [--enable-experiment=a,b]";

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut dart_sdk_js: Option<PathBuf> = None;
    let mut op = Options::default();
    let mut language = dartforge_elements::sdk::LanguageOptions::default();
    let mut i = 0;
    while i < args.len() {
        let next = |i: &mut usize| -> Result<PathBuf, String> {
            *i += 1;
            args.get(*i).map(PathBuf::from).ok_or_else(|| USAGE.to_string())
        };
        match args[i].as_str() {
            "-o" => output = Some(next(&mut i)?),
            "--sdk" => sdk = Some(next(&mut i)?),
            "--packages" => packages = Some(next(&mut i)?),
            "--dart-sdk-js" => dart_sdk_js = Some(next(&mut i)?),
            "--sem-poda" => op.prune_sdk = false,
            "--sem-membros" => op.per_member = false,
            "--sem-poda-usuario" => op.prune_user = false,
            "--sem-minificar" => op.minify = false,
            "--sdk-proprio" => op.own_sdk = true,
            "--sdk-ddc" => op.own_sdk = false,
            "--enable-asserts" => op.asserts = true,
            // The `omitImplicitChecks` of `dart2js -O4`: unsafe (see `Options`).
            "--omitir-checagens" | "-O4" => op.omit_checks = true,
            "--verificar-stub" => {
                op.stub = true;
                op.own_sdk = false;
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other if other.starts_with("--versao-linguagem") || other.starts_with("--enable-experiment") => {
                let mut rest = args[i + 1..].iter().map(String::as_str);
                let before = rest.len();
                language.read_option(other, &mut rest)?;
                i += before - rest.len();
            }
            other if input.is_none() && !other.starts_with('-') => input = Some(PathBuf::from(other)),
            other => return Err(format!("unknown argument: {other}\n{USAGE}")),
        }
        i += 1;
    }
    let (Some(input), Some(output)) = (input, output) else { return Err(USAGE.to_string()) };
    let runtime = dart_sdk_js.unwrap_or_else(dartforge_emit_js::default_dart_sdk_js);

    // Deep bodies recurse deeply, as in `compile-js`: a dedicated stack.
    let (e2, s2, p2, r2) = (input.clone(), sdk.clone(), packages.clone(), runtime.clone());
    let prod = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            if op.own_sdk {
                dartforge_emit_js_production::compile_own_sdk(&e2, s2.as_deref(), p2.as_deref(), op, &language)
            } else {
                compile(&e2, s2.as_deref(), p2.as_deref(), &r2, op, &language)
            }
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "the compilation aborted")??;

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
    }
    std::fs::write(&output, &prod.js).map_err(|e| format!("{}: {e}", output.display()))?;
    for c in &prod.cycles {
        eprintln!("warning: import cycle between modules: {c}");
    }
    let kb = |n: usize| n as f64 / 1024.0;
    println!(
        "{} -> {} ({:.0} KB; {} modules; user {:.0} KB; runtime {:.0} -> {:.0} KB, {}/{} live units)",
        input.display(),
        output.display(),
        kb(prod.js.len()),
        prod.modules,
        kb(prod.user),
        kb(prod.sdk_before),
        kb(prod.sdk_after),
        prod.sdk_live,
        prod.sdk_units,
    );
    if prod.before_compaction != prod.js.len() {
        println!("compaction: {:.0} -> {:.0} KB", kb(prod.before_compaction), kb(prod.js.len()));
    }
    if let Some((origin, t)) = prod.index {
        let from = match origin {
            dartforge_emit_js_production::cache::Origin::Cache => "from the cache",
            dartforge_emit_js_production::cache::Origin::Built => "built",
        };
        println!(
            "runtime: index {from} in {:.0} ms; pruning {:.0} ms",
            t.as_secs_f64() * 1000.0,
            prod.prune_time.as_secs_f64() * 1000.0
        );
    }
    if let Some(m) = &prod.world {
        let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
        let e = &m.stats;
        println!(
            "world: classes {}/{} instantiated + {} type only; functions {}/{}; {} selectors; {} round(s); world {:.0} ms, emission {:.0} ms, verifier {:.0} ms",
            e.instantiated_classes, e.user_classes, e.type_classes, e.live_functions, e.user_functions, e.selectors, m.rounds,
            ms(m.world_time), ms(m.emit_time), ms(m.verify_time),
        );
        let limit = std::env::var("DARTFORGE_JSPROD_CURAS").ok().and_then(|v| v.parse().ok()).unwrap_or(20usize);
        if m.fixes.len() > limit {
            eprintln!("warning: verifier: {} gap(s) fixed; DARTFORGE_JSPROD_CURAS=N shows more", m.fixes.len());
        }
        for c in m.fixes.iter().take(limit) {
            eprintln!("warning: verifier: {c}");
        }
        for s in m.no_element.iter().take(20) {
            eprintln!("warning: pending reference with no element: {s}");
        }
        if let Some(n) = m.inconsistencies {
            println!("world check: {n} inconsistency(ies)");
        }
        if m.pruning_disabled {
            eprintln!(
                "warning: the reachability analysis did not converge in {} round(s): the file went out without pruning the user code",
                m.rounds
            );
            // `DARTFORGE_JSPROD_ESTRITO=1`: not converging is an error (the
            // production CI).
            if std::env::var("DARTFORGE_JSPROD_ESTRITO").is_ok_and(|v| v == "1") {
                return Err("the reachability analysis did not converge (DARTFORGE_JSPROD_ESTRITO=1)".into());
            }
        }
    }
    Ok(())
}
