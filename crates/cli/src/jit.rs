//! `dartforge run` and `dartforge reload`: the development profile of the
//! native backend, through the ORCv2 JIT (`crates/jit`), behind the `jit` feature.
//!
//! The feature is separate from `native` because it links `dartforge` to the
//! LLVM of the full distribution (on Windows, to `LLVM-C.dll`: without it the
//! executable does not start, not even for `compile-js`; on Linux and macOS,
//! statically or to the package's `libLLVM`). Whoever only wants AOT does not
//! pay for it.
//!
//! Both commands consume the **same** LLVM IR that `dartforge aot` hands to
//! Clang (`dartforge_emit_native::emit_ir`). See `docs/JIT.md`.
#![cfg_attr(not(feature = "jit"), allow(unused))]
use std::path::{Path, PathBuf};

type Outcome = Result<(), Box<dyn std::error::Error>>;

#[cfg(not(feature = "jit"))]
fn disabled(command: &str) -> Outcome {
    Err(format!(
        "`dartforge {command}` requires the JIT: build with `cargo build -p dartforge-cli --features jit` \
         (needs the full LLVM 22.1.8 distribution; on Windows, with LLVM-C.dll on the PATH; see docs/JIT.md)"
    )
    .into())
}
#[cfg(not(feature = "jit"))]
pub fn run(_args: &[std::ffi::OsString]) -> Outcome {
    disabled("run")
}
#[cfg(not(feature = "jit"))]
pub fn reload(_args: &[std::ffi::OsString]) -> Outcome {
    disabled("reload")
}

/// Emits the IR of `input` on a large-stack thread, as `aot` does: the
/// lowering recurses over the AST.
#[cfg(feature = "jit")]
fn emit(
    input: &Path,
    sdk: Option<&Path>,
    packages: Option<&Path>,
) -> Result<dartforge_emit_native::EmittedIr, String> {
    emit_generation(input, sdk, packages, None)
}

/// [`emit`] of a new generation of the running program: `previous` is the IR
/// of the live generation, whose static layout the new one extends.
#[cfg(feature = "jit")]
fn emit_generation(
    input: &Path,
    sdk: Option<&Path>,
    packages: Option<&Path>,
    previous: Option<std::sync::Arc<String>>,
) -> Result<dartforge_emit_native::EmittedIr, String> {
    let (input, sdk, packages) = (input.to_path_buf(), sdk.map(Path::to_path_buf), packages.map(Path::to_path_buf));
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            // The project's build engine, if it uses builders (DF-BUILD-009):
            // each generation of the program redoes the generation of the sources.
            crate::engine::with_generator(&input, packages.as_deref(), |generator| {
                let options = dartforge_emit_native::CompileOptions {
                    sdk: sdk.as_deref(),
                    packages: packages.as_deref(),
                    timings: false,
                    optimize: false,
                    language_version: None,
                    experiments: Vec::new(),
                    debug: false,
                    generator: generator,
                    cpu: None,
                };
                dartforge_emit_native::emit_reloadable_ir(&input, &options, previous.as_deref().map(String::as_str))
            })
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "the IR emission aborted".to_string())?
}

/// The shared library of the source-built SDK that the IR imports:
/// `DARTFORGE_SDK_DLL` if set, otherwise the cached one (compiled now if
/// missing); `None` when the IR does not use the source-built SDK.
#[cfg(feature = "jit")]
fn sdk_library(ir: &str) -> Result<Option<PathBuf>, String> {
    if !dartforge_jit::ir_uses_source_sdk(ir) {
        return Ok(None);
    }
    if let Some(d) = std::env::var_os("DARTFORGE_SDK_DLL") {
        return Ok(Some(PathBuf::from(d)));
    }
    dartforge_emit_native::sdk_module::source_sdk_dll().map(Some)
}

/// `dartforge run [--sdk <lib>] [--packages <cfg>] [--timings] <input.dart>
/// [<main arguments>…]` or `dartforge run --ir <program.ll> [--timings]
/// [<arguments>…]`. What comes after the program belongs to `main(List<String>
/// args)`, as in `dart run`; a `--timings` right after the input still
/// belongs to dartforge (the old form).
///
/// Emits the IR and runs it in an ORCv2 session of **this** process, through
/// the path without trampolines (`dartforge_jit::run_ir`): calls are direct,
/// as in the AOT executable. The stdout is the program's, and so is the exit
/// code. The runtime ends the process with 255 in the same cases as AOT and
/// the VM.
///
/// `--timings` writes to stderr, after the run, a JSON object with the
/// phases of the emission and of the JIT (`docs/JIT.md`, «Medição por fase»).
#[cfg(feature = "jit")]
pub fn run(args: &[std::ffi::OsString]) -> Outcome {
    crate::native::set_memory_policy("tracing")?;
    let usage = "usage: dartforge run [--sdk <lib>] [--packages <package_config.json>] [--timings] [--memoria=tracing|arc] <input.dart> [<main arguments>…]\n       dartforge run --ir <program.ll> [--timings] [<main arguments>…]";
    let mut input: Option<PathBuf> = None;
    let mut ready_ir: Option<PathBuf> = None;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut timings = false;
    // As in `dart run`: the options before the program belong to dartforge;
    // what comes after it belongs to `main(List<String> args)`.
    let mut program_args: Vec<String> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if input.is_some() || ready_ir.is_some() {
            match a.to_str() {
                Some("--timings") if program_args.is_empty() => timings = true,
                _ => program_args.push(a.to_string_lossy().into_owned()),
            }
            continue;
        }
        match a.to_str() {
            Some("--ir") => ready_ir = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--sdk") => sdk = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--packages") => packages = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--timings") => timings = true,
            Some("--memoria") => {
                let value = it.next().and_then(|v| v.to_str()).ok_or("--memoria requires tracing or arc")?;
                crate::native::set_memory_policy(value)?;
            }
            Some(option) if option.starts_with("--memoria=") => {
                crate::native::set_memory_policy(&option["--memoria=".len()..])?;
            }
            _ => input = Some(PathBuf::from(a)),
        }
    }
    dartforge_jit::set_arguments(program_args);
    if let Some(input) = &input {
        dartforge_jit::set_script(input);
    }
    let (ir, emission) = match (input, ready_ir) {
        (Some(input), None) => {
            let ir = emit(&input, sdk.as_deref(), packages.as_deref())?;
            let times = ir.times;
            (ir.text, Some(times))
        }
        (None, Some(file)) => (
            std::fs::read_to_string(&file).map_err(|e| format!("could not read {}: {e}", file.display()))?,
            None,
        ),
        _ => return Err(usage.into()),
    };
    let sdk_dll = sdk_library(&ir)?;
    let report = dartforge_jit::run_ir_with(&ir, sdk_dll.as_deref())?;
    use std::io::Write;
    let _ = std::io::stdout().flush();
    if timings {
        let (major, minor, patch) = dartforge_jit::JitSession::llvm_version();
        let phase = |d: Option<std::time::Duration>| d.map_or(0, |d| d.as_nanos());
        eprintln!(
            "{{\"llvm\":\"{major}.{minor}.{patch}\",\"frontend_ns\":{},\"hir_ns\":{},\"llvm_ir_ns\":{},\"ir_bytes\":{},\
             \"session_ns\":{},\"parse_ir_ns\":{},\"add_module_ns\":{},\"lookup_ns\":{},\"execute_ns\":{},\"jit_total_ns\":{}}}",
            phase(emission.map(|t| t.frontend)),
            phase(emission.map(|t| t.hir)),
            phase(emission.map(|t| t.llvm_ir)),
            report.module.ir_bytes,
            report.session.as_nanos(),
            report.module.parse_ir.as_nanos(),
            report.module.add_module.as_nanos(),
            report.entry.lookup.as_nanos(),
            report.entry.execute.as_nanos(),
            report.total.as_nanos(),
        );
    }
    if report.entry.exit_code != 0 {
        std::process::exit(report.entry.exit_code);
    }
    Ok(())
}

/// Stamp of the Dart sources under `root` and of the package configuration
/// used by the loader: names, modification instants and sizes in a stable
/// order. It changes when a file changes, appears, disappears or is renamed,
/// even if the sum of sizes and dates stays the same.
///
/// It is deliberately simple: R0 recompiles the whole program on every
/// change, so knowing **which** file changed is of no use yet. The root is
/// the input's directory; `.dart_tool`, `build` and hidden directories are
/// left out of the source scan. `package_config.json` is watched separately.
#[cfg(feature = "jit")]
fn stamp(root: &Path, packages: Option<&Path>) -> (u64, usize) {
    use std::hash::{Hash, Hasher};

    fn walk(dir: &Path, sources: &mut Vec<(PathBuf, u128, u64)>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let path = e.path();
            let name = e.file_name();
            let name = name.to_string_lossy();
            let Ok(kind) = e.file_type() else { continue };
            if kind.is_dir() {
                if !name.starts_with('.') && name != "build" {
                    walk(&path, sources);
                }
            } else if name.ends_with(".dart") {
                let Ok(meta) = e.metadata() else { continue };
                if !meta.is_file() { continue }
                let t = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_nanos());
                sources.push((path, t, meta.len()));
            }
        }
    }
    let mut sources = Vec::new();
    walk(root, &mut sources);
    sources.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    sources.hash(&mut hash);
    let config = packages.map(Path::to_path_buf)
        .or_else(|| dartforge_elements::config::PackageConfig::discover(root));
    config.hash(&mut hash);
    if let Some(config) = config {
        let state = std::fs::metadata(&config).ok().map(|meta| {
            let modified = meta.modified().ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos());
            (modified, meta.len())
        });
        state.hash(&mut hash);
    }
    (hash.finish(), sources.len())
}

/// Directory of the `reload` IR generations, removed on any exit that
/// unwinds (return, error through `?`, panic). `process::exit` does not
/// unwind: whoever exits that way removes it before, explicitly.
#[cfg(feature = "jit")]
struct GenerationsDir(PathBuf);
#[cfg(feature = "jit")]
impl Drop for GenerationsDir {
    /// Removes the whole directory; only this process writes to it (the name carries the pid).
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `dartforge reload <input.dart> [--sdk] [--packages] [--intervalo <ms>] [--uma-vez] [--timings] [--reiniciar]`
///
/// **Stateful hot reload (default).** The program runs on its own thread of
/// a `JitSession`; on each change to the `.dart` sources in the input's
/// directory, the program is recompiled and the new generation is published
/// at the main isolate's safepoint — between two event loop events —,
/// without running `main` again: heap, statics, timers, ports and
/// connections carry on, and the next event already calls the new code
/// (`dartforge_jit::LiveProgram`). If the program has already finished, the
/// new generation is published and `main` runs again on the same state.
///
/// An edit that does not compile brings nothing down: the diagnostic is shown
/// and the running generation carries on. An edit that the reload cannot
/// apply over the live objects (a function's signature changed, a class
/// gained fields or was renumbered) is rejected with the reason, and the
/// program **restarts** with the new code: the watcher is a supervisor, and
/// the session runs in a child process (`--filho`) that it recreates.
///
/// `--reiniciar` is R0: a hot restart on each edit, without state, each
/// generation in its own process (`dartforge run --ir`).
#[cfg(feature = "jit")]
pub fn reload(args: &[std::ffi::OsString]) -> Outcome {
    crate::native::set_memory_policy("tracing")?;
    let usage = "usage: dartforge reload <input.dart> [--sdk <lib>] [--packages <package_config.json>] [--intervalo <ms>] [--uma-vez] [--timings] [--reiniciar] [--memoria=tracing|arc]";
    let mut input: Option<PathBuf> = None;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut interval_ms = 300u64;
    let mut once = false;
    let mut timings = false;
    let mut restart = false;
    let mut child = false;
    let mut supervisor: Option<u16> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.to_str() {
            Some("--sdk") => sdk = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--packages") => packages = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--intervalo") => {
                interval_ms = it
                    .next()
                    .and_then(|v| v.to_str())
                    .and_then(|v| v.parse().ok())
                    .filter(|v| (10..=10_000).contains(v))
                    .ok_or("--intervalo requires milliseconds between 10 and 10000")?;
            }
            Some("--uma-vez") => once = true,
            Some("--timings") => timings = true,
            Some("--memoria") => {
                let value = it.next().and_then(|v| v.to_str()).ok_or("--memoria requires tracing or arc")?;
                crate::native::set_memory_policy(value)?;
            }
            Some(option) if option.starts_with("--memoria=") => {
                crate::native::set_memory_policy(&option["--memoria=".len()..])?;
            }
            Some("--reiniciar") => restart = true,
            // The old form of stateful hot reload, which is now the default.
            Some("--preservar-estado") => {}
            Some("--filho") => child = true,
            Some("--supervisor") => {
                supervisor = Some(it.next().and_then(|v| v.to_str()).and_then(|v| v.parse().ok()).ok_or(usage)?);
            }
            _ if input.is_none() => input = Some(PathBuf::from(a)),
            _ => return Err(usage.into()),
        }
    }
    let input = input.ok_or(usage)?;
    dartforge_jit::set_script(&input);
    if !restart {
        if child {
            if let Some(port) = supervisor {
                watch_supervisor(port)?;
            }
            return live_reload(&input, sdk.as_deref(), packages.as_deref(), interval_ms, once, timings);
        }
        return supervise(args);
    }
    let root = input
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let own_exe = std::env::current_exe()?;
    let temp = GenerationsDir(std::env::temp_dir().join(format!("dartforge-reload-{}", std::process::id())));
    std::fs::create_dir_all(&temp.0)?;

    let mut generation = 0u32;
    let mut child: Option<std::process::Child> = None;
    let mut end_reported = false;
    let mut last = None;
    loop {
        let now = stamp(&root, packages.as_deref());
        if last != Some(now) {
            last = Some(now);
            let start = std::time::Instant::now();
            match emit(&input, sdk.as_deref(), packages.as_deref()) {
                Err(error) => {
                    eprintln!("[reload] compilation error:\n{error}");
                    if generation == 0 {
                        eprintln!("[reload] no generation running; waiting for the next edit");
                    } else {
                        eprintln!("[reload] generation {generation} carries on as it was");
                    }
                }
                Ok(ir) => {
                    let emission = start.elapsed();
                    if let Some(mut previous) = child.take() {
                        if previous.try_wait()?.is_none() {
                            let _ = previous.kill();
                        }
                        let _ = previous.wait();
                    }
                    // The previous generation's process has ended: its IR is no
                    // longer needed, and a long session does not pile up one `.ll` per edit.
                    if generation > 0 {
                        let _ = std::fs::remove_file(temp.0.join(format!("generation-{generation}.ll")));
                    }
                    generation += 1;
                    let file = temp.0.join(format!("generation-{generation}.ll"));
                    std::fs::write(&file, &ir.text)?;
                    eprintln!(
                        "[reload] generation {generation}: hot restart — state is NOT preserved \
                         (the program starts over from main; stateful hot reload does not exist yet)"
                    );
                    if timings {
                        eprintln!(
                            "[reload] generation {generation}: IR emission {:.1} ms ({} bytes)",
                            emission.as_secs_f64() * 1000.0,
                            ir.text.len()
                        );
                    }
                    let mut command = std::process::Command::new(&own_exe);
                    // The child also starts from tracing; it carries over the
                    // explicit selection to keep the same SDK variant in the cache.
                    command.arg("run")
                        .arg(format!("--memoria={}", std::env::var("DARTFORGE_MEMORY").unwrap_or_else(|_| "tracing".into())))
                        .arg("--ir").arg(&file);
                    if timings {
                        command.arg("--timings");
                    }
                    child = Some(command.spawn()?);
                    end_reported = false;
                }
            }
        }
        if let Some(current) = child.as_mut() {
            if !end_reported {
                if let Some(status) = current.try_wait()? {
                    end_reported = true;
                    let code = status.code().unwrap_or(-1);
                    eprintln!("[reload] generation {generation} ended with code {code}");
                    if once {
                        if code != 0 {
                            drop(temp);
                            std::process::exit(code);
                        }
                        return Ok(());
                    }
                }
            }
        } else if once && generation == 0 {
            return Err("the first compilation failed".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(interval_ms));
    }
}

/// The exit code with which the session process asks to be recreated: an
/// edit that the reload does not apply over the live objects.
#[cfg(feature = "jit")]
const RESTART_CODE: i32 = 75;

/// The hot reload supervisor: runs the session in a child process and
/// recreates it when it asks (`RESTART_CODE`); any other end is the program's.
///
/// The child connects to a local socket of the supervisor and ends when the
/// connection closes: killing the supervisor (Ctrl+C, an IDE's `kill`) does
/// not leave the program running orphaned.
#[cfg(feature = "jit")]
fn supervise(args: &[std::ffi::OsString]) -> Outcome {
    let own_exe = std::env::current_exe()?;
    let watcher = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    let port = watcher.local_addr()?.port();
    watcher.set_nonblocking(true)?;
    loop {
        let mut child = std::process::Command::new(&own_exe)
            .arg("reload")
            .args(args)
            .arg("--filho")
            .arg("--supervisor")
            .arg(port.to_string())
            .spawn()?;
        // The child's connection stays open while the supervisor lives.
        let mut _connection = None;
        let status = loop {
            if _connection.is_none()
                && let Ok((c, _)) = watcher.accept()
            {
                _connection = Some(c);
            }
            if let Some(status) = child.try_wait()? {
                break status;
            }
            std::thread::sleep(std::time::Duration::from_millis(if _connection.is_some() { 100 } else { 10 }));
        };
        match status.code() {
            Some(RESTART_CODE) => eprintln!("[reload] restarting the program with the new code"),
            Some(0) => return Ok(()),
            Some(code) => std::process::exit(code),
            None => return Err("the reload session ended by a signal".into()),
        }
    }
}

/// The child: connects to the supervisor and exits when the connection closes.
#[cfg(feature = "jit")]
fn watch_supervisor(port: u16) -> Outcome {
    use std::io::Read;
    let mut connection = std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))?;
    std::thread::Builder::new().name("dartforge-watchdog".into()).spawn(move || {
        let mut byte = [0u8; 1];
        // Nothing is written on this connection: `read` only returns when it closes.
        let _ = connection.read(&mut byte);
        std::process::exit(RESTART_CODE + 1);
    })?;
    Ok(())
}

/// Ends the session asking the supervisor to recreate it. Exits right away:
/// the running program (a session thread) has no way to be interrupted, and
/// the process is the unit of restart — open ports and files close with it.
#[cfg(feature = "jit")]
fn request_restart() -> ! {
    use std::io::Write;
    let _ = std::io::stdout().flush();
    std::process::exit(RESTART_CODE);
}

/// Milliseconds, for the reports.
#[cfg(feature = "jit")]
fn ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// The hot reload session (the supervisor's child process).
#[cfg(feature = "jit")]
fn live_reload(
    input: &Path,
    sdk: Option<&Path>,
    packages: Option<&Path>,
    interval_ms: u64,
    once: bool,
    timings: bool,
) -> Outcome {
    let root = input.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let interval = std::time::Duration::from_millis(interval_ms);
    // The first generation: the session depends on the IR (the embedded
    // runtime or the source-built SDK library). A first compilation that
    // fails waits for the next edit.
    let mut last = None;
    let (mut session, ir) = loop {
        let now = stamp(root, packages);
        if last != Some(now) {
            last = Some(now);
            match emit(input, sdk, packages) {
                Ok(ir) => {
                    let sdk_dll = sdk_library(&ir.text)?;
                    let session = dartforge_jit::JitSession::new_for_ir_with(&ir.text, sdk_dll.as_deref())?;
                    break (session, ir);
                }
                Err(error) if once => return Err(error.into()),
                Err(error) => {
                    eprintln!("[reload] compilation error:\n{error}");
                    eprintln!("[reload] no generation running; waiting for the next edit");
                }
            }
        }
        std::thread::sleep(interval);
    };
    let from_source = session.uses_source_sdk();
    let mut live_ir = std::sync::Arc::new(ir.text);
    let result = session.with_live_program(|live| -> Result<i32, Box<dyn std::error::Error>> {
        live.on_safepoint_wait(|| {
            eprintln!("[reload] waiting for the program to reach a safepoint (between two events)…");
        });
        let report = live.publish("app", &live_ir)?;
        eprintln!("[reload] generation {}: state preserved in the same session", report.generation);
        live.run_main()?;
        loop {
            if let Some(end) = live.finished() {
                if once {
                    return Ok(end.exit_code);
                }
                eprintln!("[reload] the program ended with code {}; waiting for edits", end.exit_code);
            }
            let now = stamp(root, packages);
            if last != Some(now) {
                last = Some(now);
                let start = std::time::Instant::now();
                let ir = match emit_generation(input, sdk, packages, Some(live_ir.clone())) {
                    Ok(ir) => ir,
                    Err(error) => {
                        eprintln!("[reload] compilation error:\n{error}");
                        eprintln!("[reload] the running version carries on as it was");
                        std::thread::sleep(interval);
                        continue;
                    }
                };
                let emission = start.elapsed();
                let times = ir.times;
                if dartforge_jit::ir_uses_source_sdk(&ir.text) != from_source {
                    eprintln!("[reload] the edit changed the program's runtime profile");
                    request_restart();
                }
                let running = live.running();
                match live.publish("app", &ir.text) {
                    Ok(report) => {
                        live_ir = std::sync::Arc::new(ir.text);
                        if running {
                            eprintln!(
                                "[reload] generation {}: published into the running program (state preserved)",
                                report.generation
                            );
                        } else {
                            eprintln!("[reload] generation {}: state preserved in the same session", report.generation);
                        }
                        if timings {
                            eprintln!(
                                "[reload] generation {}: emission {:.1} ms (front-end {:.1}, HIR {:.1}, IR {:.1}; {} bytes of IR); \
                                 reload {:.1} ms (parse {:.1}, contract {:.1}, module {:.1}, \
                                 trampolines {:.1}, link {:.1}, publish {:.1}, safepoint wait {:.1}); \
                                 functions compiled {}, kept {}; generations retained {}",
                                report.generation,
                                ms(emission),
                                ms(times.frontend),
                                ms(times.hir),
                                ms(times.llvm_ir),
                                live_ir.len(),
                                ms(report.total),
                                ms(report.parse_ir),
                                ms(report.contract),
                                ms(report.add_module),
                                ms(report.stubs),
                                ms(report.link),
                                ms(report.publish),
                                ms(report.safepoint_wait),
                                report.entries,
                                report.kept,
                                report.retained_generations
                            );
                        }
                        if !live.running() {
                            // The program had ended: it runs again, on the
                            // same state.
                            live.run_main()?;
                        }
                    }
                    Err(error) if matches!(error.stage, "contract" | "poisoned") => {
                        eprintln!("[reload] the edit cannot be applied to the running program: {error}");
                        request_restart();
                    }
                    Err(error) => {
                        eprintln!("[reload] the reload failed: {error}");
                        eprintln!("[reload] the running version carries on as it was");
                    }
                }
            }
            std::thread::sleep(interval);
        }
    })?;
    let code = result?;
    use std::io::Write;
    let _ = std::io::stdout().flush();
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}
