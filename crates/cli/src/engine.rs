//! The code generation engine on the command line: `dartforge build` and the
//! hookup of the engine in `compile-js` (`crates/build`, `docs/BUILD-MOTOR.md`).
use dartforge_build::query::NoDb;
use dartforge_build::{UpdateContext, Demand, Engine, EngineOptions};
use dartforge_elements::config::PackageConfig;
use dartforge_elements::generated::Generation;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Variable that the native builder executor sets when compiling its own
/// bootstrap: that program does not go through the engine (the bootstrap
/// lives in the project that uses builders, and the engine would compile the
/// executor again).
const COMPILING_EXECUTOR: &str = "DARTFORGE_BUILD_COMPILING_EXECUTOR";

/// Hooks the Dart builder executor into `engine`: through the VM when asked
/// (`--dart`/`DARTFORGE_BUILD_DART`, the compatibility path), otherwise the
/// **native** one (B01): the bootstrap compiled by the `compile-native` of
/// this same executable, without `dart` or `build_runner`. `default`: the
/// native one applies without being asked (`dartforge build`); in the
/// compilers (`compile-js`, `compile-native`, `run`), only with
/// `DARTFORGE_BUILD_NATIVE=1`, because the executor's first compilation costs
/// minutes. `DARTFORGE_BUILD_NATIVE=0` always turns it off (the fallback
/// remains). Without the `native` feature, only the VM.
pub fn attach_executor(engine: &mut Engine, dart: Option<PathBuf>, sdk: Option<PathBuf>, cfg_path: &Path, default: bool) {
    if let Some(d) = dart {
        dartforge_build::vm::attach(engine, d, cfg_path);
        return;
    }
    let requested = std::env::var_os(dartforge_build::native_executor::ENV_VAR);
    let enabled = match requested.as_ref().and_then(|v| v.to_str()) {
        Some("0") => false,
        Some(_) => true,
        None => default,
    };
    if cfg!(feature = "native") && enabled && let Ok(exe) = std::env::current_exe() {
        dartforge_build::native_executor::attach(engine, exe, sdk, cfg_path);
    }
}

/// Does the input's project use builders? It costs reading the
/// `package_config.json` (which the loader also reads) and the `build.yaml`
/// files of the resolved packages.
pub fn detect(
    input: &Path,
    packages: Option<&Path>,
) -> Option<(PathBuf, PackageConfig, PathBuf)> {
    if std::env::var_os(COMPILING_EXECUTOR).is_some() {
        return None;
    }
    let path = packages
        .map(Path::to_path_buf)
        .or_else(|| PackageConfig::discover(input))?;
    let cfg = PackageConfig::load(&path).ok()?;
    if !dartforge_build::detect(&cfg) {
        return None;
    }
    Some((dartforge_build::package_root(input)?, cfg, path))
}

/// One pass of the engine (`compile-js`): the program loaded without the
/// generated files plays the role of `BuildStep.resolver`.
pub fn generate_once(
    root: &Path,
    cfg: &PackageConfig,
    cfg_path: &Path,
    program: &dartforge_elements::model::Program,
    names: &dartforge_intern::Interner,
) -> Result<(Arc<Generation>, String), String> {
    let mut m = Engine::new(
        root,
        cfg,
        EngineOptions {
            persist: dartforge_build::persistence::requested_in_env(),
            ..EngineOptions::default()
        },
    )?;
    // Dart builders: through the VM when asked (DARTFORGE_BUILD_DART),
    // otherwise through the native executor.
    attach_executor(&mut m, dartforge_build::vm::dart_from_env(), None, cfg_path, false);
    let up = m.update(
        &UpdateContext {
            db: &NoDb,
            program: Some((program, names)),
        },
        &[],
        Demand::Loader,
    )?;
    let mut text = up.rel.text();
    let n = up.warnings.len();
    for a in up.warnings.iter().take(5) {
        text.push_str(&format!("\nwarning: {a}"));
    }
    if n > 5 {
        text.push_str(&format!(
            "\n({} warnings about possibly stale fallback)",
            n
        ));
    }
    // A builder that failed: the generation failed (DF-BUILD-005), not a
    // partial generation presented as ready.
    if !up.rel.failures.is_empty() {
        return Err(format!("{text}\n{} builder action(s) failed", up.rel.failures.len()));
    }
    Ok((up.generation, text))
}

/// The compilation of `input` with the project's build engine, if it uses
/// builders (DF-BUILD-009): `compile` receives the generator (or `None`) and
/// the engine's report goes to `stderr`. An engine error fails the
/// compilation — never an empty generation that lets the loader read the disk.
#[cfg(feature = "native")]
pub fn with_generator<R>(
    input: &Path,
    packages: Option<&Path>,
    compile: impl FnOnce(Option<dartforge_emit_native::Generator<'_>>) -> Result<R, String>,
) -> Result<R, String> {
    let Some((root, cfg, cfg_path)) = detect(input, packages) else {
        return compile(None);
    };
    let text = std::sync::Mutex::new(None);
    let generate = |program: &dartforge_elements::model::Program, names: &dartforge_intern::Interner| {
        let (g, t) = generate_once(&root, &cfg, &cfg_path, program, names)?;
        if let Ok(mut x) = text.lock() {
            *x = Some(t);
        }
        Ok(g)
    };
    let r = compile(Some(&generate));
    if let Some(t) = text.into_inner().ok().flatten() {
        eprintln!("{t}");
    }
    r
}

/// `dartforge build [<input.dart>] [--raiz <dir>] [--packages <cfg>] [--plano]
/// [--comparar] [--release] [--estrito] [--trabalhadores N]
/// [--escrever-cache <dir>] [--dart <exe>] [--estado]`. With `--estado` (or
/// `DARTFORGE_BUILD_STATE=1`) the engine's state is saved in
/// `.dart_tool/dartforge/build/state` and reused by the next process;
/// without asking, nothing goes to disk (governing rule 6). With `--dart` (or
/// `DARTFORGE_BUILD_DART`) the builders without a native generator run on the
/// Dart VM (`dartforge_build::vm`); without it, on the native executor
/// (`dartforge_build::native_executor`, requires the `native` feature).
pub fn run_build(args: &[std::ffi::OsString]) -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<std::ffi::OsString> = args.to_vec();
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || build(&args))
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "the build aborted")?
        .map_err(|e| e.into())
}

fn build(args: &[std::ffi::OsString]) -> Result<(), String> {
    let usage = "usage: dartforge build [<input.dart>] [--raiz <dir>] [--packages <cfg>] [--sdk <lib>] [--plano] [--comparar] [--release] [--estrito] [--aceitar-pendentes] [--trabalhadores N] [--escrever-cache <dir>] [--dart <exe>] [--estado] [--define <builder>=<option>=<value>] [--config <name>] [--build-filter <glob>]";
    let (mut input, mut root, mut packages, mut sdk, mut cache) = (None, None, None, None, None);
    let mut dart = dartforge_build::vm::dart_from_env();
    let (mut plan, mut compare, mut release, mut strict) = (false, false, false, false);
    let mut accept_pending = false;
    let mut persist = dartforge_build::persistence::requested_in_env();
    let mut workers = EngineOptions::default().workers;
    // Those of `build_runner` (`--define`, `--config`, `--build-filter`), with
    // the same shape: `--x v` or `--x=v`; `--define` and `--build-filter` repeat.
    let (mut config, mut defines, mut filters) = (None, Vec::new(), Vec::new());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if let Some((option, value)) = a.to_str().and_then(|s| s.split_once('='))
            && matches!(option, "--define" | "--config" | "--build-filter")
        {
            match option {
                "--define" => defines.push(value.to_string()),
                "--config" => config = Some(value.to_string()),
                _ => filters.push(value.to_string()),
            }
            continue;
        }
        let next = |it: &mut std::slice::Iter<'_, std::ffi::OsString>| {
            it.next().map(PathBuf::from).ok_or(usage)
        };
        match a.to_str() {
            Some("--raiz") => root = Some(next(&mut it)?),
            Some("--packages") => packages = Some(next(&mut it)?),
            Some("--sdk") => sdk = Some(next(&mut it)?),
            Some("--escrever-cache") => cache = Some(next(&mut it)?),
            Some("--dart") => dart = Some(next(&mut it)?),
            Some("--plano") => plan = true,
            Some("--comparar") => compare = true,
            Some("--release") => release = true,
            Some("--estrito") => strict = true,
            // Finishes successfully even with outputs that DartForge did not
            // produce (scoreboard, transition); without it, that is a failure
            // (DF-BUILD-007).
            Some("--aceitar-pendentes") => accept_pending = true,
            Some("--estado") => persist = true,
            Some(o @ ("--define" | "--config" | "--build-filter")) => {
                let v = it
                    .next()
                    .and_then(|v| v.to_str())
                    .map(str::to_string)
                    .ok_or(usage)?;
                match o {
                    "--define" => defines.push(v),
                    "--config" => config = Some(v),
                    _ => filters.push(v),
                }
            }
            Some("--trabalhadores") => {
                workers = it
                    .next()
                    .and_then(|v| v.to_str())
                    .and_then(|v| v.parse().ok())
                    .filter(|&n: &usize| n >= 1)
                    .ok_or("--trabalhadores requires a number ≥ 1")?;
            }
            _ if input.is_none() && !a.to_string_lossy().starts_with("--") => {
                input = Some(PathBuf::from(a))
            }
            _ => return Err(usage.into()),
        }
    }
    let base = input
        .clone()
        .or_else(|| std::env::current_dir().ok())
        .ok_or(usage)?;
    let root = root
        .or_else(|| dartforge_build::package_root(&base))
        .ok_or("pubspec.yaml not found")?;
    let cfg_path = packages
        .clone()
        .unwrap_or_else(|| root.join(".dart_tool/package_config.json"));
    let cfg = PackageConfig::load(&cfg_path)?;
    if !dartforge_build::detect(&cfg) {
        println!(
            "{}: the project does not use builders (no resolved package defines builders)",
            root.display()
        );
        return Ok(());
    }
    if plan {
        let (_, _, p) = dartforge_build::project_plan(&root, &cfg)?;
        print!("{}", p.canonical_text());
        for a in &p.warnings {
            eprintln!("warning: {a}");
        }
        let bd = root.join(".dart_tool/build/entrypoint/build.dart");
        if let Ok(text) = std::fs::read_to_string(&bd)
            && text.contains("BuilderFactories(")
        {
            // `build_runner` ≥ 2.9: the script only registers the factories by
            // key, without the applications; the set of keys is checked.
            let mut official: Vec<&str> = text
                .split('\'')
                .collect::<Vec<_>>()
                .windows(2)
                .filter(|w| {
                    w[1].trim_start().starts_with(':') && w[0].contains(':') && !w[0].contains('/')
                })
                .map(|w| w[0])
                .collect();
            official.sort_unstable();
            let mut ours: Vec<&str> = p.applications.iter().map(|a| a.key.as_str()).collect();
            ours.sort_unstable();
            if official != ours {
                return Err(format!(
                    "builders differ from {}: official {official:?}, DartForge {ours:?}",
                    bd.display()
                ));
            }
            println!(
                "builders equal to those of {} ({}; the build_runner ≥ 2.9 script does not carry the applications)",
                bd.display(),
                ours.len()
            );
        } else if let Ok(text) = std::fs::read_to_string(&bd) {
            match dartforge_build::oracle::compare(&p.applications, &text)? {
                Ok(()) => println!(
                    "plan equal to {} ({} applications)",
                    bd.display(),
                    p.applications.len()
                ),
                Err(d) => {
                    return Err(format!(
                        "plan differs from {}:\n{}",
                        bd.display(),
                        d.join("\n")
                    ));
                }
            }
        }
        return Ok(());
    }
    let mut options = EngineOptions {
        release,
        workers,
        strict,
        measure_unchecked: compare,
        persist,
        ..EngineOptions::default()
    };
    dartforge_build::command_line::BuildOptions::for_project(
        &root,
        config.as_deref(),
        &defines,
        &filters,
    )?
    .apply(&mut options);
    let mut engine = Engine::new(&root, &cfg, options)?;
    attach_executor(&mut engine, dart, sdk.clone(), &cfg_path, true);
    // The program serves as `BuildStep.resolver` to the native generators.
    let input = input.or_else(|| Some(root.join("web/main.dart")).filter(|p| p.is_file()));
    let mut names = dartforge_intern::Interner::new();
    let program = match &input {
        Some(e) => {
            let dir = sdk.or_else(dartforge_elements::sdk::SdkLayout::discover).ok_or(
                "Dart SDK not found: set DARTFORGE_SDK_LIB (the SDK's lib/) or DART_SDK",
            )?;
            let layout = dartforge_elements::sdk::SdkLayout::load(&dir, "dartdevc")?;
            // The plan's ngdart builder: the one from `ngdart` 8 or the one from
            // the `ngx_dart` 9 fork (`docs/NGDART-COMPILADOR-DE-VISOES.md` part B).
            let ng_key = if engine.inputs_of("ngx_dart:ngx_dart").is_empty() {
                "ngdart:ngdart"
            } else {
                "ngx_dart:ngx_dart"
            };
            // The ngdart resolver does not see the outputs of phases that run
            // after it (`Engine::outputs_invisible_to`).
            let hidden = std::sync::Arc::new(engine.outputs_invisible_to(ng_key));
            // Every file that ngdart generates enters as a root: `build_runner`
            // resolves each one, whether reachable from `main` or not (a
            // component that nobody imports also gets a `.template.dart`).
            let extras = engine.inputs_of(ng_key);
            let mut roots: Vec<&std::path::Path> = vec![e.as_path()];
            roots.extend(
                extras
                    .iter()
                    .filter(|p| !hidden.contains(*p))
                    .map(|p| p.as_path()),
            );
            Some(
                dartforge_elements::load::load_lenient_entries_hiding(
                    &roots,
                    &layout,
                    packages.as_deref(),
                    &mut names,
                    hidden.clone(),
                )
                .0,
            )
        }
        None => None,
    };
    let up = engine.update(
        &UpdateContext {
            db: &NoDb,
            program: program.as_ref().map(|p| (p, &names)),
        },
        &[],
        Demand::Everything,
    )?;
    // Changed `build_to: source` outputs of a native generator go to disk.
    for (p, c) in engine.native_source_outputs(&up.altered_outputs) {
        if std::fs::read(&p).ok().as_deref() != Some(&c[..]) {
            std::fs::write(&p, &c[..]).map_err(|e| format!("{}: {e}", p.display()))?;
        }
    }
    println!("{}", up.rel.text());
    for a in &up.warnings {
        eprintln!("warning: {a}");
    }
    // A builder that failed: the generation failed, with or without an old
    // output on disk (DF-BUILD-005).
    if !up.rel.failures.is_empty() {
        return Err(format!(
            "{} builder action(s) failed:\n  {}",
            up.rel.failures.len(),
            up.rel.failures.join("\n  ")
        ));
    }
    // Outputs that no DartForge executor produced — pending, or read from
    // what `build_runner` left on disk (fallback): without
    // `--aceitar-pendentes`, the command fails instead of announcing success
    // (DF-BUILD-006/007). With `--comparar`, the scoreboard comes out first.
    // A fallback without content (the official one wrote nothing for that
    // input) delivers no outside output, and only shows up in the report.
    let pending: usize =
        up.rel.pending_by_reason.values().sum::<usize>() + up.rel.support_with_output;
    let pending_failure = (pending > 0 && !accept_pending).then(|| {
        format!(
            "{pending} action(s) without a DartForge producer ({} with output read from the build_runner fallback); use --aceitar-pendentes to accept",
            up.rel.support_with_output
        )
    });
    if let Some(e) = &pending_failure
        && !compare
    {
        return Err(e.clone());
    }
    if let Some(dir) = cache {
        let mut n = 0;
        for (id, g) in &engine.graph.generated {
            if !g.hidden {
                continue;
            }
            let Some(c) = engine
                .record(g.action)
                .and_then(|r| r.outputs.iter().find(|(s, _)| s == id))
                .and_then(|(_, c)| c.clone())
            else {
                continue;
            };
            let destination = dir.join(id.package.as_ref()).join(id.path.as_ref());
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&destination, &c[..]).map_err(|e| e.to_string())?;
            n += 1;
        }
        println!("{n} cache outputs written to {}", dir.display());
    }
    if compare {
        // Reference: what `build_runner` left on disk.
        let mut reference = BTreeMap::new();
        for (id, g) in &engine.graph.generated {
            if let Ok(b) = std::fs::read(engine.support_path(id, g.hidden)) {
                reference.insert(id.clone(), b);
            }
        }
        let p = engine.scoreboard(&reference);
        println!("scoreboard against the build_runner fallback: {}", p.summary());
        let mut by_ext: BTreeMap<String, [usize; 3]> = BTreeMap::new();
        let ext = |c: &str| {
            let name = c.rsplit('/').next().unwrap_or(c);
            name.find('.')
                .map(|i| name[i..].to_string())
                .unwrap_or_default()
        };
        for id in &p.equal {
            by_ext.entry(ext(&id.path)).or_default()[0] += 1;
        }
        for (id, _) in &p.pending {
            by_ext.entry(ext(&id.path)).or_default()[1] += 1;
        }
        for (id, _) in &p.different {
            by_ext.entry(ext(&id.path)).or_default()[2] += 1;
        }
        for (e, [i, pe, d]) in &by_ext {
            println!("  {e:<24} {i:>6} equal {pe:>6} pending {d:>6} different");
        }
        let mut reasons: Vec<(String, usize)> = p.reasons().into_iter().collect();
        reasons.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        for (m, n) in &reasons {
            println!("  {n:>6}× {m}");
        }
        for (id, m) in &p.unrequested {
            println!("  not requested: {} ({m})", id.text());
        }
        // Each pending output with its reason, to find the file of a refusal.
        for (id, m) in &p.pending {
            println!("  pending: {} ({m})", id.text());
        }
        if !p.different.is_empty() {
            for (id, m) in p.different.iter().take(20) {
                eprintln!("different: {} ({m})", id.text());
            }
            return Err(format!(
                "{} outputs differ from the official ones",
                p.different.len()
            ));
        }
        if let Some(e) = pending_failure {
            return Err(e);
        }
    }
    Ok(())
}
