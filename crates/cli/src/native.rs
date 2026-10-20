//! Commands of the native backend (ORCv2 JIT, AOT, ABI), behind the `native` feature.
//!
//! The JavaScript path does not depend on LLVM: without the feature, the
//! commands exist but explain how to enable them. This keeps `llvm-sys` out of
//! the CLI's default build (it was what cost minutes and required `LLVM-C.dll`).
#![cfg_attr(not(feature = "native"), allow(unused))]
use std::{fs, path::PathBuf};

type Outcome = Result<(), Box<dyn std::error::Error>>;

#[cfg(not(feature = "native"))]
fn disabled(command: &str) -> Outcome {
    Err(format!("`dartforge {command}` requires the native backend: build with `cargo build -p dartforge-cli --features native` (set DARTFORGE_CLANG to LLVM 22)").into())
}

#[cfg(not(feature = "native"))]
pub fn abi_info(_args: &[std::ffi::OsString]) -> Outcome {
    disabled("abi-info")
}
#[cfg(not(feature = "native"))]
pub fn aot(_args: &[std::ffi::OsString]) -> Outcome {
    disabled("aot")
}
#[cfg(not(feature = "native"))]
pub fn run_compile_native(_args: &[std::ffi::OsString]) -> Outcome {
    disabled("compile-native")
}

#[cfg(feature = "native")]
pub fn abi_info(args: &[std::ffi::OsString]) -> Outcome {
    use dartforge_abi::{NativeType, Target};
    if args.len() != 2 {
        return Err("usage: dartforge abi-info <windows-x64|linux-x64|wasm32>".into());
    }
    let target = match args[1].to_str() {
        Some("windows-x64") => Target::WindowsX64,
        Some("linux-x64") => Target::LinuxX64,
        Some("wasm32") => Target::Wasm32,
        _ => return Err("unknown ABI profile".into()),
    };
    let layouts: Vec<_> = [NativeType::Int32, NativeType::Int64, NativeType::Double, NativeType::Pointer]
            .iter().map(|ty| {
                let (size, alignment) = ty.layout(target).expect("scalar type");
                serde_json::json!({"native_type":format!("{ty:?}"),"llvm":ty.llvm(),"size":size,"alignment":alignment})
            }).collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_version":1,"triple":target.triple(),"pointer_bits":target.pointer_bits(),
            "native_type_layouts":layouts,"profile_supports_dynamic_libraries":target.supports_dynamic_libraries(),
            "ffi_execution_implemented":false,"native_static_scalar_bindings_implemented":true,"wasm_emission_implemented":false,
            "note":"Full FFI still pending. Host AOT accepts @Native Int32/Int64/Void with explicitly linked objects; it does not load libraries dynamically. Cross AOT uses --target and --target-bundle; abi-info profiles only describe FFI layouts."
        }))?
    );
    Ok(())
}

#[cfg(feature = "native")]
/// `--exceptions check|tables`: the exception model of the generated code
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13). The emitter reads it from the
/// `DARTFORGE_EXCEPTIONS` variable (`dartforge_emit_native::target::table_exceptions`),
/// which is also part of the compiled SDK's key — the program and the SDK
/// always come out in the same model; the option only sets it, for this process.
#[allow(unsafe_code)]
fn set_exception_model(value: &str) -> Result<(), String> {
    if value != "check" && value != "tables" {
        return Err(format!(
            "--exceptions={value}: the exception models are `check` and `tables`"
        ));
    }
    // SAFETY: the CLI is still reading the options, on the main thread, before
    // creating the compilation thread: no other thread reads the environment.
    unsafe { std::env::set_var("DARTFORGE_EXCEPTIONS", value) };
    Ok(())
}

#[cfg(feature = "native")]
/// `--memory tracing|arc`: the memory policy of the generated code
/// (docs/ARC-CICLOS-ESPECIFICACAO.md §18.1 and §24; docs/ARC-IMPLEMENTACAO.md).
/// Like [`set_exception_model`]: the emitter reads `DARTFORGE_MEMORY`, which
/// is part of the compiled SDK's key.
#[allow(unsafe_code)]
pub(crate) fn set_memory_policy(value: &str) -> Result<(), String> {
    if value != "tracing" && value != "arc" {
        return Err(format!(
            "--memory={value}: the memory policies are `tracing` and `arc`"
        ));
    }
    // SAFETY: the CLI is still reading the options, on the main thread, before
    // creating the compilation thread: no other thread reads the environment.
    unsafe { std::env::set_var("DARTFORGE_MEMORY", value) };
    Ok(())
}

#[cfg(feature = "native")]
/// Select the PE subsystem before compilation starts; the CLI default ignores
/// an inherited linker policy, just like the memory policy.
#[allow(unsafe_code)]
fn set_subsystem(value: &str) -> Result<(), String> {
    use dartforge_emit_native::linker_windows::Subsystem;
    let subsystem = Subsystem::from_name(value)?;
    if subsystem == Subsystem::Gui && !cfg!(windows) {
        return Err("--subsystem=gui is only supported on Windows".into());
    }
    // SAFETY: options are parsed on the main thread before the compilation thread
    // is created, and no other CLI thread reads the environment yet.
    unsafe { std::env::set_var("DARTFORGE_SUBSYSTEM", value) };
    Ok(())
}

#[cfg(feature = "native")]
/// `--roots shadow|maps`: where the collector's roots live in the generated
/// code (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.8). Like
/// [`set_exception_model`]: the emitter reads `DARTFORGE_ROOTS`.
#[allow(unsafe_code)]
fn set_roots_mode(value: &str) -> Result<(), String> {
    if value != "shadow" && value != "maps" {
        return Err(format!(
            "--roots={value}: the root modes are `shadow` and `maps`"
        ));
    }
    // SAFETY: the CLI is still reading the options, on the main thread, before
    // creating the compilation thread: no other thread reads the environment.
    unsafe { std::env::set_var("DARTFORGE_ROOTS", value) };
    Ok(())
}

#[cfg(feature = "native")]
/// `--stack-trace symbolic|none`: the stack trace in the VM's format
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14): `symbolic` emits the
/// address → (function, line, column) table and the `StackTrace` comes out
/// with the Dart frames; `none` (the default) emits no table. Like
/// [`set_exception_model`]: the emitter reads `DARTFORGE_STACK_TRACE`.
#[allow(unsafe_code)]
fn set_stack_trace(value: &str) -> Result<(), String> {
    if value != "symbolic" && value != "none" {
        return Err(format!(
            "--stack-trace={value}: the stack traces are `symbolic` and `none`"
        ));
    }
    // SAFETY: the CLI is still reading the options, on the main thread, before
    // creating the compilation thread: no other thread reads the environment.
    unsafe { std::env::set_var("DARTFORGE_STACK_TRACE", value) };
    Ok(())
}

#[cfg(feature = "native")]
/// The asserts and `dart.vm.product` of the compiled program
/// (`dartforge_emit_native::DartFlags`). `--optimize` is the profile of
/// `dart compile exe`: asserts off (unless `--enable-asserts`) and
/// `dart.vm.product` true; without it, the profile of the corpus oracle
/// `dart run --enable-asserts` (asserts on, `dart.vm.product` false), which
/// `run`/JIT and the differential harness keep.
fn dart_flags(optimize: bool, enable_asserts: bool) -> dartforge_emit_native::DartFlags {
    use dartforge_emit_native::DartFlags;
    if optimize {
        DartFlags {
            enable_asserts,
            ..DartFlags::PRODUCTION
        }
    } else {
        DartFlags::DEVELOPMENT
    }
}

#[cfg(feature = "native")]
/// `dartforge aot <input.dart> <output.exe>` — production AOT.
///
/// This command used to call `dartforge_compiler::compile_path_llvm_*`, from
/// the old track (`lexer`/`parser`/`hir`/`llvm`), which is no longer a
/// dependency of the CLI: the crate left `Cargo.toml` and the command stopped
/// compiling with the `native` feature. The live native compiler is
/// `emit_native` (frontend -> elements/types -> its own HIR -> LLVM IR ->
/// Clang), the same as `compile-native`; `aot` becomes its production alias,
/// so that there is a single native track instead of two measuring different
/// things.
///
/// The `--merge-identical-functions` and `--link-object` options belonged to
/// the old track and have no equivalent here; they are rejected explicitly
/// instead of accepted and ignored, because silently not doing what the flag
/// says is worse than not having the flag.
pub fn aot(args: &[std::ffi::OsString]) -> Outcome {
    // The CLI option selects ARC; an inherited environment does not change the default.
    set_memory_policy("tracing")?;
    set_subsystem("console")?;
    let usage = "usage: dartforge aot <input.dart> <output.exe> [--optimize] [--enable-asserts] [--debug] [--timings] [--cpu x86-64|x86-64-v2|x86-64-v3|x86-64-v4|native] [--exceptions check|tables] [--memory tracing|arc] [--subsystem console|gui] [--dartforge-contracts verify|ignore] [--stack-trace symbolic|none] [--sdk <lib>] [--packages <package_config.json>]
  --target <triple> --target-bundle <directory>  cross compilation (see export-target)
  --optimize        production: asserts off and dart.vm.product true (dart compile exe)
  --enable-asserts  keeps the asserts on with --optimize (dart compile exe --enable-asserts)";
    if args.len() < 3 {
        return Err(usage.into());
    }
    let input = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);
    let mut optimize = false;
    let mut enable_asserts = false;
    let mut debug = false;
    let mut timings = false;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut cpu: Option<dartforge_emit_native::generator::Cpu> = None;
    let mut contracts = dartforge_emit_native::contracts::ContractMode::Verify;
    let mut flags = args[3..].iter();
    while let Some(flag) = flags.next() {
        match flag.to_str() {
            Some("--optimize") => optimize = true,
            Some("--enable-asserts") => enable_asserts = true,
            Some("--debug") => debug = true,
            Some("--timings") => timings = true,
            Some("--cpu") => {
                let name = flags
                    .next()
                    .and_then(|v| v.to_str())
                    .ok_or("--cpu requires the CPU name")?;
                cpu = Some(dartforge_emit_native::generator::Cpu::from_name(name)?);
            }
            Some("--sdk") => {
                sdk = Some(PathBuf::from(flags.next().ok_or("--sdk requires a path")?))
            }
            Some("--packages") => {
                packages = Some(PathBuf::from(
                    flags.next().ok_or("--packages requires a path")?,
                ))
            }
            Some("--memory") => {
                let value = flags
                    .next()
                    .and_then(|v| v.to_str())
                    .ok_or("--memory requires tracing or arc")?;
                set_memory_policy(value)?;
            }
            Some(option) if option.starts_with("--memory=") => {
                set_memory_policy(&option["--memory=".len()..])?;
            }
            Some("--dartforge-contracts") => {
                let value = flags
                    .next()
                    .and_then(|v| v.to_str())
                    .ok_or("--dartforge-contracts requires verify or ignore")?;
                contracts = dartforge_emit_native::contracts::ContractMode::from_name(value)?;
            }
            Some(option) if option.starts_with("--dartforge-contracts=") => {
                contracts = dartforge_emit_native::contracts::ContractMode::from_name(
                    &option["--dartforge-contracts=".len()..],
                )?;
            }
            Some("--subsystem") => {
                let value = flags
                    .next()
                    .and_then(|v| v.to_str())
                    .ok_or("--subsystem requires console or gui")?;
                set_subsystem(value)?;
            }
            Some(option) if option.starts_with("--subsystem=") => {
                set_subsystem(&option["--subsystem=".len()..])?;
            }
            Some("--roots") => {
                let value = flags
                    .next()
                    .and_then(|v| v.to_str())
                    .ok_or("--roots requires shadow or maps")?;
                set_roots_mode(value)?;
            }
            Some(option) if option.starts_with("--roots=") => {
                set_roots_mode(&option["--roots=".len()..])?;
            }
            Some("--exceptions") => {
                let value = flags
                    .next()
                    .and_then(|v| v.to_str())
                    .ok_or("--exceptions requires check or tables")?;
                set_exception_model(value)?;
            }
            Some(option) if option.starts_with("--exceptions=") => {
                set_exception_model(&option["--exceptions=".len()..])?;
            }
            Some("--stack-trace") => {
                let value = flags
                    .next()
                    .and_then(|v| v.to_str())
                    .ok_or("--stack-trace requires symbolic or none")?;
                set_stack_trace(value)?;
            }
            Some(option) if option.starts_with("--stack-trace=") => {
                set_stack_trace(&option["--stack-trace=".len()..])?;
            }
            Some("--merge-identical-functions") | Some("--link-object") => return Err(format!(
                "{} belonged to the old track and does not exist in the current native compiler",
                flag.to_string_lossy()
            )
            .into()),
            _ => return Err(format!("unknown AOT option: {}", flag.to_string_lossy()).into()),
        }
    }

    // Large stack: the lowering recurses over the AST and real programs
    // overflow the main thread's default 8 MiB stack.
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            // The project's build engine, if it uses builders (DF-BUILD-009).
            crate::engine::with_generator(&input, packages.as_deref(), |generator| {
                let options = dartforge_emit_native::CompileOptions {
                    contracts,
                    sdk: sdk.as_deref(),
                    packages: packages.as_deref(),
                    timings,
                    optimize,
                    language_version: None,
                    experiments: Vec::new(),
                    debug,
                    generator,
                    cpu,
                };
                let flags = dart_flags(optimize, enable_asserts);
                dartforge_emit_native::compile_with_flags(&input, &output, &options, flags)
                    .map_err(|e| e.to_string())
            })
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "the AOT compilation aborted".to_string())??;
    Ok(())
}

#[cfg(feature = "native")]
pub fn run_compile_native(args: &[std::ffi::OsString]) -> Result<(), Box<dyn std::error::Error>> {
    set_memory_policy("tracing")?;
    set_subsystem("console")?;
    use dartforge_elements::sdk::LanguageOptions;
    let usage = "usage: dartforge compile-native <input.dart> -o <output.exe> [--sdk <lib>] [--packages <package_config.json>] [--timings] [--optimize] [--enable-asserts] [--debug] [--cpu <cpu>] [--exceptions check|tables] [--memory tracing|arc] [--subsystem console|gui] [--dartforge-contracts verify|ignore] [--stack-trace symbolic|none] [--language-version x.y] [--enable-experiment=a,b]
       dartforge compile-native <input.dart> --emit-ir -o <output.ll> [--digest] [...]
       dartforge compile-native <input.dart> --digest [...]
  --target <triple> --target-bundle <directory>  cross compilation (see export-target)
  --optimize  production: asserts off and dart.vm.product true, as `dart compile exe`;
             without it, asserts on and dart.vm.product false, as `dart run --enable-asserts`
  --enable-asserts  keeps the asserts on with --optimize
  --debug  line tables for the native debugger (gdb, lldb, Visual Studio)
  --cpu      target CPU: x86-64 (the baseline, SSE2), x86-64-v2, x86-64-v3, x86-64-v4 or native
             (the executable only runs on machines that have it)
  --roots   where the collector's roots live: shadow (the default: each function's
             root frame) or maps (LLVM stack maps, Windows x86-64 only)
  --exceptions exception model of the generated code: check (the default: the pending
             exception checked after each call) or tables (table-based unwinding,
             Windows x86-64 only; docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md)
  --stack-trace   the StackTrace: none (the default: today's fixed text) or symbolic (the
             Dart frames in the VM's format, `#0      f (url:line:column)`, through
             the address table the executable carries; §13.14)
  --dartforge-contracts verify (default) or ignore: known requests require proof; ignore produces no certificate
  --subsystem console (default) or gui: Windows PE subsystem, set by the linker
             (gui starts without allocating a console; Windows only)
  --emit-ir  writes the LLVM IR to -o, without Clang or linking
  --digest   prints `<32-digit hash>  <bytes>` of the LLVM IR (the same digest
             as `dartforge-differential determinism --native`), without Clang";
    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut cpu: Option<dartforge_emit_native::generator::Cpu> = None;
    let mut contracts = dartforge_emit_native::contracts::ContractMode::Verify;
    let mut timings = false;
    let mut optimize = false;
    let mut enable_asserts = false;
    let mut debug = false;
    let mut emit_ir = false;
    let mut digest = false;
    let mut language = LanguageOptions::default();

    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.to_str() {
            Some("-o") => out = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--sdk") => sdk = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--packages") => packages = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--timings") => timings = true,
            Some("--cpu") => {
                let name = it
                    .next()
                    .and_then(|v| v.to_str())
                    .ok_or("--cpu requires the CPU name")?;
                cpu = Some(dartforge_emit_native::generator::Cpu::from_name(name)?);
            }
            Some("--optimize") => optimize = true,
            Some("--enable-asserts") => enable_asserts = true,
            Some("--debug") => debug = true,
            Some("--emit-ir") => emit_ir = true,
            Some("--digest") => digest = true,
            Some("--memory") => {
                let value = it
                    .next()
                    .and_then(|v| v.to_str())
                    .ok_or("--memory requires tracing or arc")?;
                set_memory_policy(value)?;
            }
            Some(option) if option.starts_with("--memory=") => {
                set_memory_policy(&option["--memory=".len()..])?;
            }
            Some("--dartforge-contracts") => {
                let value = it
                    .next()
                    .and_then(|v| v.to_str())
                    .ok_or("--dartforge-contracts requires verify or ignore")?;
                contracts = dartforge_emit_native::contracts::ContractMode::from_name(value)?;
            }
            Some(option) if option.starts_with("--dartforge-contracts=") => {
                contracts = dartforge_emit_native::contracts::ContractMode::from_name(
                    &option["--dartforge-contracts=".len()..],
                )?;
            }
            Some("--subsystem") => {
                let value = it
                    .next()
                    .and_then(|v| v.to_str())
                    .ok_or("--subsystem requires console or gui")?;
                set_subsystem(value)?;
            }
            Some(option) if option.starts_with("--subsystem=") => {
                set_subsystem(&option["--subsystem=".len()..])?;
            }
            Some("--roots") => {
                let value = it
                    .next()
                    .and_then(|v| v.to_str())
                    .ok_or("--roots requires shadow or maps")?;
                set_roots_mode(value)?;
            }
            Some(option) if option.starts_with("--roots=") => {
                set_roots_mode(&option["--roots=".len()..])?;
            }
            Some("--exceptions") => {
                let value = it
                    .next()
                    .and_then(|v| v.to_str())
                    .ok_or("--exceptions requires check or tables")?;
                set_exception_model(value)?;
            }
            Some(option) if option.starts_with("--exceptions=") => {
                set_exception_model(&option["--exceptions=".len()..])?;
            }
            Some("--stack-trace") => {
                let value = it
                    .next()
                    .and_then(|v| v.to_str())
                    .ok_or("--stack-trace requires symbolic or none")?;
                set_stack_trace(value)?;
            }
            Some(option) if option.starts_with("--stack-trace=") => {
                set_stack_trace(&option["--stack-trace=".len()..])?;
            }
            Some("--language-version" | "--enable-experiment") => {
                let value = it.next().and_then(|v| v.to_str()).ok_or(usage)?;
                language
                    .read_option(a.to_str().unwrap_or_default(), &mut std::iter::once(value))?;
            }
            Some(option)
                if option.starts_with("--language-version=")
                    || option.starts_with("--enable-experiment=") =>
            {
                language.read_option(option, &mut std::iter::empty::<&str>())?;
            }
            _ if input.is_none() => input = Some(PathBuf::from(a)),
            _ => return Err(usage.into()),
        }
    }
    let Some(input) = input else {
        return Err(usage.into());
    };
    let flags = dart_flags(optimize, enable_asserts);
    if emit_ir || digest {
        if emit_ir && out.is_none() {
            return Err(usage.into());
        }
        return emit_native_ir(IrEmission {
            input,
            out: out.filter(|_| emit_ir),
            sdk,
            packages,
            timings,
            digest,
            language,
            debug,
            optimize,
            contracts,
            flags,
        });
    }
    let Some(out) = out else {
        return Err(usage.into());
    };

    let (i2, o2) = (input.clone(), out.clone());
    let s2 = sdk.clone();
    let p2 = packages.clone();
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            crate::engine::with_generator(&i2, p2.as_deref(), |generator| {
                let options = dartforge_emit_native::CompileOptions {
                    contracts,
                    sdk: s2.as_deref(),
                    packages: p2.as_deref(),
                    timings,
                    optimize,
                    language_version: language.current_version,
                    experiments: language.experiments,
                    debug,
                    generator,
                    cpu,
                };
                dartforge_emit_native::compile_with_flags(&i2, &o2, &options, flags)
                    .map_err(|e| e.to_string())
            })
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "the native compilation aborted".to_string())??;

    Ok(())
}

#[cfg(feature = "native")]
/// Resolved options for the IR and digest views of native compilation.
struct IrEmission {
    input: PathBuf,
    out: Option<PathBuf>,
    sdk: Option<PathBuf>,
    packages: Option<PathBuf>,
    timings: bool,
    digest: bool,
    language: dartforge_elements::sdk::LanguageOptions,
    debug: bool,
    // Production IR includes static literals and per-call helpers; link-time
    // pruning still belongs to executable compilation.
    optimize: bool,
    contracts: dartforge_emit_native::contracts::ContractMode,
    flags: dartforge_emit_native::DartFlags,
}

#[cfg(feature = "native")]
/// Emit the same generated sources as executable compilation, without linking.
fn emit_native_ir(command: IrEmission) -> Outcome {
    let IrEmission {
        input,
        out,
        sdk,
        packages,
        timings,
        digest,
        language,
        debug,
        optimize,
        contracts,
        flags,
    } = command;
    let ir = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            crate::engine::with_generator(&input, packages.as_deref(), |generator| {
                let options = dartforge_emit_native::CompileOptions {
                    contracts,
                    sdk: sdk.as_deref(),
                    packages: packages.as_deref(),
                    timings,
                    optimize,
                    language_version: language.current_version,
                    experiments: language.experiments,
                    debug,
                    generator,
                    cpu: None,
                };
                dartforge_emit_native::emit_ir_with_flags(&input, &options, flags)
            })
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "the LLVM IR emission aborted".to_string())??;
    if timings {
        eprintln!("--- Native Emission Timings ---");
        ir.print_times();
    }
    if let Some(out) = out {
        fs::write(&out, &ir.text).map_err(|e| format!("failed to write {}: {e}", out.display()))?;
        let mut report = ir.contracts.json().clone();
        report["artifact"] = serde_json::json!({
            "kind": "llvm_ir", "path": out.display().to_string(),
            "ir_digest": dartforge_emit_native::summary::IrSummary::of(&ir.text).hex(),
        });
        let manifest = dartforge_emit_native::contracts::manifest_path(&out);
        fs::write(&manifest, serde_json::to_vec_pretty(&report)?)
            .map_err(|e| format!("failed to write {}: {e}", manifest.display()))?;
    }
    if digest {
        let r = dartforge_emit_native::summary::IrSummary::of(&ir.text);
        println!("{}  {}", r.hex(), r.bytes);
    }
    Ok(())
}

/// Remove destination flags before the per-command option parser. Selection is
/// process-wide and happens before compiler workers or target-dependent caches.
#[cfg(feature = "native")]
pub fn select_destination(
    args: &mut Vec<std::ffi::OsString>,
) -> Result<(), Box<dyn std::error::Error>> {
    use dartforge_emit_native::target::{self, NativeTarget};
    let allowed = args
        .first()
        .is_some_and(|a| a == "aot" || a == "compile-native");
    if cfg!(windows)
        && !cfg!(target_arch = "x86_64")
        && args.first().is_some_and(|a| {
            a == "aot"
                || a == "compile-native"
                || a == "run"
                || a == "reload"
                || a == "pack"
                || a == "export-target"
        })
    {
        return Err("the native Windows compiler/runtime currently requires x64; build DartForge with --target x86_64-pc-windows-msvc (runs on Windows ARM64 through x64 compatibility)".into());
    }
    let mut target = None;
    let mut bundle = None;
    let mut index = 1;
    while index < args.len() {
        if args[index] == "--" {
            break;
        }
        let text = args[index].to_str().unwrap_or("");
        let (key, inline) = text
            .split_once('=')
            .map_or((text, None), |(k, v)| (k, Some(v.to_string())));
        if key != "--target" && key != "--target-bundle" {
            index += 1;
            continue;
        }
        if !allowed {
            return Err("--target and --target-bundle are only valid for aot and compile-native; JIT runs on its host".into());
        }
        let key = key.to_string();
        args.remove(index);
        let value = match inline {
            Some(value) => std::ffi::OsString::from(value),
            None if index < args.len() => args.remove(index),
            None => return Err(format!("{key} requires a value").into()),
        };
        if key == "--target" {
            if target.is_some() {
                return Err("--target specified twice".into());
            }
            target = Some(NativeTarget::parse(
                value.to_str().ok_or("target must be UTF-8")?,
            )?);
        } else {
            if bundle.is_some() {
                return Err("--target-bundle specified twice".into());
            }
            bundle = Some(std::path::PathBuf::from(value));
        }
    }
    let target = target.unwrap_or_else(NativeTarget::host);
    if let Some(bundle) = bundle {
        dartforge_emit_native::cross::select_bundle(&bundle, target)?;
    }
    target::select(target)?;
    Ok(())
}
