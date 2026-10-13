//! Commands of the native backend (ORCv2 JIT, AOT, ABI), behind the `nativo` feature.
//!
//! The JavaScript path does not depend on LLVM: without the feature, the
//! commands exist but explain how to enable them. This keeps `llvm-sys` out of
//! the CLI's default build (it was what cost minutes and required `LLVM-C.dll`).
#![cfg_attr(not(feature = "nativo"), allow(unused))]
use std::{fs, path::PathBuf};

type Outcome = Result<(), Box<dyn std::error::Error>>;

#[cfg(not(feature = "nativo"))]
fn disabled(command: &str) -> Outcome {
    Err(format!("`dartforge {command}` requires the native backend: build with `cargo build -p dartforge-cli --features nativo` (set DARTFORGE_CLANG to LLVM 22)").into())
}

#[cfg(not(feature = "nativo"))]
pub fn abi_info(_args: &[std::ffi::OsString]) -> Outcome { disabled("abi-info") }
#[cfg(not(feature = "nativo"))]
pub fn aot(_args: &[std::ffi::OsString]) -> Outcome { disabled("aot") }
#[cfg(not(feature = "nativo"))]
pub fn run_compile_native(_args: &[std::ffi::OsString]) -> Outcome { disabled("compile-native") }

#[cfg(feature = "nativo")]
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
                "note":"Full FFI still pending. Host AOT accepts @Native Int32/Int64/Void with explicitly linked objects; it does not load libraries dynamically. Profiles do not enable cross-compilation."
            }))?
        );
        return Ok(());
}

#[cfg(feature = "nativo")]
/// `--excecoes checagem|tabelas`: the exception model of the generated code
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13). The emitter reads it from the
/// `DARTFORGE_EXCECOES` variable (`dartforge_emit_native::alvo::excecoes_por_tabelas`),
/// which is also part of the compiled SDK's key — the program and the SDK
/// always come out in the same model; the option only sets it, for this process.
#[allow(unsafe_code)]
fn set_exception_model(value: &str) -> Result<(), String> {
    if value != "checagem" && value != "tabelas" {
        return Err(format!("--excecoes={value}: the exception models are `checagem` and `tabelas`"));
    }
    // SAFETY: the CLI is still reading the options, on the main thread, before
    // creating the compilation thread: no other thread reads the environment.
    unsafe { std::env::set_var("DARTFORGE_EXCECOES", value) };
    Ok(())
}

#[cfg(feature = "nativo")]
/// `--memoria tracing|arc`: the memory policy of the generated code
/// (docs/ARC-CICLOS-ESPECIFICACAO.md §18.1 and §24; docs/ARC-IMPLEMENTACAO.md).
/// Like [`set_exception_model`]: the emitter reads `DARTFORGE_MEMORIA`, which
/// is part of the compiled SDK's key.
#[allow(unsafe_code)]
pub(crate) fn set_memory_policy(value: &str) -> Result<(), String> {
    if value != "tracing" && value != "arc" {
        return Err(format!("--memoria={value}: the memory policies are `tracing` and `arc`"));
    }
    // SAFETY: the CLI is still reading the options, on the main thread, before
    // creating the compilation thread: no other thread reads the environment.
    unsafe { std::env::set_var("DARTFORGE_MEMORIA", value) };
    Ok(())
}

#[cfg(feature = "nativo")]
/// `--raizes sombra|mapas`: where the collector's roots live in the generated
/// code (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.8). Like
/// [`set_exception_model`]: the emitter reads `DARTFORGE_RAIZES`.
#[allow(unsafe_code)]
fn set_roots_mode(value: &str) -> Result<(), String> {
    if value != "sombra" && value != "mapas" {
        return Err(format!("--raizes={value}: the root modes are `sombra` and `mapas`"));
    }
    // SAFETY: the CLI is still reading the options, on the main thread, before
    // creating the compilation thread: no other thread reads the environment.
    unsafe { std::env::set_var("DARTFORGE_RAIZES", value) };
    Ok(())
}

#[cfg(feature = "nativo")]
/// `--rastro simbolico|nenhum`: the stack trace in the VM's format
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14): `simbolico` emits the
/// address → (function, line, column) table and the `StackTrace` comes out
/// with the Dart frames; `nenhum` (the default) emits no table. Like
/// [`set_exception_model`]: the emitter reads `DARTFORGE_RASTRO_VM`.
#[allow(unsafe_code)]
fn set_stack_trace(value: &str) -> Result<(), String> {
    if value != "simbolico" && value != "nenhum" {
        return Err(format!("--rastro={value}: the stack traces are `simbolico` and `nenhum`"));
    }
    // SAFETY: the CLI is still reading the options, on the main thread, before
    // creating the compilation thread: no other thread reads the environment.
    unsafe { std::env::set_var("DARTFORGE_RASTRO_VM", value) };
    Ok(())
}

#[cfg(feature = "nativo")]
/// `dartforge aot <input.dart> <output.exe>` — production AOT.
///
/// This command used to call `dartforge_compiler::compile_path_llvm_*`, from
/// the old track (`lexer`/`parser`/`hir`/`llvm`), which is no longer a
/// dependency of the CLI: the crate left `Cargo.toml` and the command stopped
/// compiling with the `nativo` feature. The live native compiler is
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
    let usage = "usage: dartforge aot <input.dart> <output.exe> [--optimize] [--depuracao] [--timings] [--cpu x86-64|x86-64-v2|x86-64-v3|x86-64-v4|native] [--excecoes checagem|tabelas] [--memoria tracing|arc] [--rastro simbolico|nenhum] [--sdk <lib>] [--packages <package_config.json>]";
    if args.len() < 3 {
        return Err(usage.into());
    }
    let input = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);
    let mut optimize = false;
    let mut debug = false;
    let mut timings = false;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut cpu: Option<dartforge_emit_native::gerador::Cpu> = None;
    let mut flags = args[3..].iter();
    while let Some(flag) = flags.next() {
        match flag.to_str() {
            Some("--optimize") => optimize = true,
            Some("--depuracao") => debug = true,
            Some("--timings") => timings = true,
            Some("--cpu") => {
                let name = flags.next().and_then(|v| v.to_str()).ok_or("--cpu requires the CPU name")?;
                cpu = Some(dartforge_emit_native::gerador::Cpu::do_nome(name)?);
            }
            Some("--sdk") => sdk = Some(PathBuf::from(flags.next().ok_or("--sdk requires a path")?)),
            Some("--packages") => {
                packages = Some(PathBuf::from(flags.next().ok_or("--packages requires a path")?))
            }
            Some("--memoria") => {
                let value = flags.next().and_then(|v| v.to_str()).ok_or("--memoria requires tracing or arc")?;
                set_memory_policy(value)?;
            }
            Some(option) if option.starts_with("--memoria=") => {
                set_memory_policy(&option["--memoria=".len()..])?;
            }
            Some("--raizes") => {
                let value = flags.next().and_then(|v| v.to_str()).ok_or("--raizes requires sombra or mapas")?;
                set_roots_mode(value)?;
            }
            Some(option) if option.starts_with("--raizes=") => {
                set_roots_mode(&option["--raizes=".len()..])?;
            }
            Some("--excecoes") => {
                let value = flags.next().and_then(|v| v.to_str()).ok_or("--excecoes requires checagem or tabelas")?;
                set_exception_model(value)?;
            }
            Some(option) if option.starts_with("--excecoes=") => {
                set_exception_model(&option["--excecoes=".len()..])?;
            }
            Some("--rastro") => {
                let value = flags.next().and_then(|v| v.to_str()).ok_or("--rastro requires simbolico or nenhum")?;
                set_stack_trace(value)?;
            }
            Some(option) if option.starts_with("--rastro=") => {
                set_stack_trace(&option["--rastro=".len()..])?;
            }
            Some("--merge-identical-functions") | Some("--link-object") => {
                return Err(format!(
                    "{} belonged to the old track and does not exist in the current native compiler",
                    flag.to_string_lossy()
                )
                .into())
            }
            _ => {
                return Err(
                    format!("unknown AOT option: {}", flag.to_string_lossy()).into()
                )
            }
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
                    sdk: sdk.as_deref(),
                    packages: packages.as_deref(),
                    timings,
                    optimize,
                    versao_linguagem: None,
                    experimentos: Vec::new(),
                    depuracao: debug,
                    gerador: generator,
                    cpu,
                };
                dartforge_emit_native::compilar(&input, &output, &options).map_err(|e| e.to_string())
            })
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "the AOT compilation aborted".to_string())??;
    Ok(())
}

#[cfg(feature = "nativo")]
pub fn run_compile_native(args: &[std::ffi::OsString]) -> Result<(), Box<dyn std::error::Error>> {
    set_memory_policy("tracing")?;
    use dartforge_elements::sdk::Linguagem;
    let usage = "usage: dartforge compile-native <input.dart> -o <output.exe> [--sdk <lib>] [--packages <package_config.json>] [--timings] [--optimize] [--depuracao] [--cpu <cpu>] [--excecoes checagem|tabelas] [--memoria tracing|arc] [--rastro simbolico|nenhum] [--versao-linguagem x.y] [--enable-experiment=a,b]
       dartforge compile-native <input.dart> --emit-ir -o <output.ll> [--resumo] [...]
       dartforge compile-native <input.dart> --resumo [...]
  --depuracao  line tables for the native debugger (gdb, lldb, Visual Studio)
  --cpu      target CPU: x86-64 (the baseline, SSE2), x86-64-v2, x86-64-v3, x86-64-v4 or native
             (the executable only runs on machines that have it)
  --raizes   where the collector's roots live: sombra (the default: each function's
             root frame) or mapas (LLVM stack maps, Windows x86-64 only)
  --excecoes exception model of the generated code: checagem (the default: the pending
             exception checked after each call) or tabelas (table-based unwinding,
             Windows x86-64 only; docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md)
  --rastro   the StackTrace: nenhum (the default: today's fixed text) or simbolico (the
             Dart frames in the VM's format, `#0      f (url:line:column)`, through
             the address table the executable carries; §13.14)
  --emit-ir  writes the LLVM IR to -o, without Clang or linking
  --resumo   prints `<32-digit hash>  <bytes>` of the LLVM IR (the same digest
             as `dartforge-differential determinismo --nativo`), without Clang";
    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut cpu: Option<dartforge_emit_native::gerador::Cpu> = None;
    let mut timings = false;
    let mut optimize = false;
    let mut debug = false;
    let mut emit_ir = false;
    let mut digest = false;
    let mut language = Linguagem::default();

    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.to_str() {
            Some("-o") => out = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--sdk") => sdk = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--packages") => packages = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--timings") => timings = true,
            Some("--cpu") => {
                let name = it.next().and_then(|v| v.to_str()).ok_or("--cpu requires the CPU name")?;
                cpu = Some(dartforge_emit_native::gerador::Cpu::do_nome(name)?);
            }
            Some("--optimize") => optimize = true,
            Some("--depuracao") => debug = true,
            Some("--emit-ir") => emit_ir = true,
            Some("--resumo") => digest = true,
            Some("--memoria") => {
                let value = it.next().and_then(|v| v.to_str()).ok_or("--memoria requires tracing or arc")?;
                set_memory_policy(value)?;
            }
            Some(option) if option.starts_with("--memoria=") => {
                set_memory_policy(&option["--memoria=".len()..])?;
            }
            Some("--raizes") => {
                let value = it.next().and_then(|v| v.to_str()).ok_or("--raizes requires sombra or mapas")?;
                set_roots_mode(value)?;
            }
            Some(option) if option.starts_with("--raizes=") => {
                set_roots_mode(&option["--raizes=".len()..])?;
            }
            Some("--excecoes") => {
                let value = it.next().and_then(|v| v.to_str()).ok_or("--excecoes requires checagem or tabelas")?;
                set_exception_model(value)?;
            }
            Some(option) if option.starts_with("--excecoes=") => {
                set_exception_model(&option["--excecoes=".len()..])?;
            }
            Some("--rastro") => {
                let value = it.next().and_then(|v| v.to_str()).ok_or("--rastro requires simbolico or nenhum")?;
                set_stack_trace(value)?;
            }
            Some(option) if option.starts_with("--rastro=") => {
                set_stack_trace(&option["--rastro=".len()..])?;
            }
            Some("--versao-linguagem" | "--enable-experiment") => {
                let value = it.next().and_then(|v| v.to_str()).ok_or(usage)?;
                language.ler_opcao(a.to_str().unwrap_or_default(), &mut std::iter::once(value))?;
            }
            Some(option) if option.starts_with("--versao-linguagem=")
                || option.starts_with("--enable-experiment=") => {
                language.ler_opcao(option, &mut std::iter::empty::<&str>())?;
            }
            _ if input.is_none() => input = Some(PathBuf::from(a)),
            _ => return Err(usage.into()),
        }
    }
    let Some(input) = input else {
        return Err(usage.into());
    };
    if emit_ir || digest {
        if emit_ir && out.is_none() {
            return Err(usage.into());
        }
        return emit_native_ir(input, out.filter(|_| emit_ir), sdk, packages, timings, digest, language, debug, optimize);
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
                    sdk: s2.as_deref(),
                    packages: p2.as_deref(),
                    timings,
                    optimize,
                    versao_linguagem: language.versao_corrente,
                    experimentos: language.experimentos,
                    depuracao: debug,
                    gerador: generator,
                    cpu,
                };
                dartforge_emit_native::compilar(&i2, &o2, &options).map_err(|e| e.to_string())
            })
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "the native compilation aborted".to_string())??;

    Ok(())
}

#[cfg(feature = "nativo")]
/// `compile-native --emit-ir` / `--resumo`: emission only, without Clang or
/// linking — what the determinism test compares, and what changes when the
/// emitter changes.
fn emit_native_ir(
    input: PathBuf,
    out: Option<PathBuf>,
    sdk: Option<PathBuf>,
    packages: Option<PathBuf>,
    timings: bool,
    digest: bool,
    language: dartforge_elements::sdk::Linguagem,
    debug: bool,
    // The production IR (`--optimize`): static literals, the large program
    // with fields and per-call helpers. Link-time pruning comes later.
    optimize: bool,
) -> Outcome {
    let ir = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let options = dartforge_emit_native::CompileOptions {
                sdk: sdk.as_deref(),
                packages: packages.as_deref(),
                timings,
                optimize,
                versao_linguagem: language.versao_corrente,
                experimentos: language.experimentos,
                depuracao: debug,
                gerador: None,
                cpu: None,
            };
            dartforge_emit_native::emitir_ir(&input, &options)
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "the LLVM IR emission aborted".to_string())??;
    if timings {
        eprintln!("--- Native Emission Timings ---");
        ir.imprimir_tempos();
    }
    if let Some(out) = out {
        fs::write(&out, &ir.texto).map_err(|e| format!("failed to write {}: {e}", out.display()))?;
    }
    if digest {
        let r = dartforge_emit_native::resumo::ResumoIr::de(&ir.texto);
        println!("{}  {}", r.hex(), r.bytes);
    }
    Ok(())
}
