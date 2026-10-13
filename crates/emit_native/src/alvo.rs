//! The system the native backend compiles for: that of the compiling process.
//!
//! Everything that changes between Windows, Linux and macOS lives here — the header of the
//! LLVM IR, the extensions of the artifacts, the Clang and link flags and the
//! format of the shared library of the source SDK. The rest of the backend
//! asks this module instead of testing scattered `cfg!`s.
//!
//! There is no cross-compilation: the target is always the host. The IR on Windows is
//! byte for byte that of before the port (same header, same flags), so
//! that the determinism digests and the cache keys stay valid.

/// The systems supported by the native backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sistema {
    /// COFF, MSVC ABI, `.obj`/`.lib`/`.dll`/`.exe`.
    Windows,
    /// ELF, `.o`/`.a`/`.so`, executable without extension.
    Linux,
    /// Mach-O, `.o`/`.a`/`.dylib`, executable without extension.
    MacOs,
}

/// The system of the current process.
///
/// ```
/// use dartforge_emit_native::alvo::{Sistema, sistema};
/// let s = sistema();
/// assert_eq!(s == Sistema::Windows, cfg!(windows));
/// ```
pub const fn sistema() -> Sistema {
    if cfg!(windows) {
        Sistema::Windows
    } else if cfg!(target_os = "macos") {
        Sistema::MacOs
    } else {
        Sistema::Linux
    }
}

/// The exception model of the generated code
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13), through the variable
/// `DARTFORGE_EXCECOES` (the `--excecoes=` of `compile-native` sets it):
///
/// * absent, empty or `checagem`: the exception is a pending state of the runtime that
///   the caller checks after each call — the usual IR, byte for byte;
/// * `tabelas`: a Dart call that throws unwinds the stack up to the
///   `landingpad` of whoever handles it (`optimize/tables.rs`), with the
///   runtime's personality (`dartforge_personalidade`). Only on Windows
///   x86-64, the only target with the personality written, and without the function
///   call trace (`DARTFORGE_RASTRO=1` counts entries and exits, and
///   unwinding skips the exits).
///
/// The program and the SDK have to come out in the same model: the variable goes into the key
/// of the compiled SDK (`sdk_modulo::sdk_key`).
///
/// # Errors
/// An unknown value, or `tabelas` where the mode does not exist.
pub fn excecoes_por_tabelas() -> Result<bool, String> {
    let value = std::env::var("DARTFORGE_EXCECOES").unwrap_or_default();
    match value.as_str() {
        "" | "checagem" => Ok(false),
        "tabelas" => {
            // Windows x86-64 (SEH) and, through the runtime's Itanium
            // personality (Step 4, written and never run), Linux and macOS
            // on x86-64 and aarch64.
            let seh = cfg!(windows) && cfg!(target_arch = "x86_64");
            let itanium = cfg!(unix) && (cfg!(target_arch = "x86_64") || cfg!(target_arch = "aarch64"));
            if !(seh || itanium) {
                return Err(
                    "exceptions by tables (--excecoes=tabelas) only exist on Windows x86-64 and, on Linux and macOS, on x86-64 and aarch64".to_string()
                );
            }
            if std::env::var("DARTFORGE_RASTRO").is_ok_and(|v| v == "1") {
                return Err("exceptions by tables (--excecoes=tabelas) do not combine with DARTFORGE_RASTRO=1".to_string());
            }
            Ok(true)
        }
        other => Err(format!("DARTFORGE_EXCECOES={other}: the exception models are `checagem` and `tabelas`")),
    }
}

/// The memory policy of the generated code (docs/ARC-CICLOS-ESPECIFICACAO.md
/// §18.1, decision 1, and §24; docs/ARC-IMPLEMENTACAO.md), through the variable
/// `DARTFORGE_MEMORIA` (the `--memoria=` of `aot` and `compile-native` sets it):
///
/// * absent, empty or `tracing`: the usual tracing collector — the usual
///   IR, byte for byte;
/// * `arc`: reference counting with cycle collection. The entry turns
///   ARC on in the runtime (`dartforge_memoria_arc_v1`, the version of the memory
///   ABI) and the reference writes into an object that may be old
///   go through the runtime, which counts the replacement (decision 6 of §18.1: no
///   inline write without the contract).
///
/// The program and the SDK have to come out in the same mode: the variable goes into the key
/// of the compiled SDK.
///
/// # Errors
/// An unknown value.
pub fn arc_memory() -> Result<bool, String> {
    let value = std::env::var("DARTFORGE_MEMORIA").unwrap_or_default();
    match value.as_str() {
        "" | "tracing" => Ok(false),
        "arc" => Ok(true),
        other => Err(format!("DARTFORGE_MEMORIA={other}: the memory policies are `tracing` and `arc`")),
    }
}

/// The published ARC preparation (owners in the HIR, `optimize::arc`), through the
/// variable `DARTFORGE_ARC_PREPARADO=1`: only with ARC memory. Absent,
/// ARC follows the legacy regime of the CLI (no tokens in the code).
pub fn arc_prepared() -> Result<bool, String> {
    if !std::env::var("DARTFORGE_ARC_PREPARADO").is_ok_and(|v| v == "1") {
        return Ok(false);
    }
    if !arc_memory()? {
        return Err("DARTFORGE_ARC_PREPARADO=1 requires ARC memory (`--memoria=arc`)".into());
    }
    Ok(true)
}

/// Where the collector roots live in the generated code
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.8), through the variable
/// `DARTFORGE_RAIZES` (the `--raizes=` of `compile-native` sets it):
///
/// * absent, empty or `sombra`: the roots frame of each function (the
///   shadow stack) — the usual IR, byte for byte;
/// * `mapas`: the SSA values live across a call come out in the LLVM stack
///   maps (`rewrite-statepoints-for-gc`), and the collector finds them
///   by walking the native stack. On Windows x86-64 (the SEH walker,
///   `RtlVirtualUnwind`) and, since Step 4, on Linux and macOS on x86-64 and
///   aarch64 (the `_Unwind_Backtrace` walker); Windows arm64 stays on the
///   shadow stack until a walker for it is tested. Only with the embedded
///   generator (`clang` does not run the pass).
///
/// The program and the SDK have to come out in the same mode: the variable goes into the key
/// of the compiled SDK.
///
/// # Errors
/// An unknown value, or `mapas` where the mode does not exist.
pub fn raizes_por_mapas() -> Result<bool, String> {
    let value = std::env::var("DARTFORGE_RAIZES").unwrap_or_default();
    match value.as_str() {
        "" | "sombra" => Ok(false),
        "mapas" => {
            let target_with_walker = (cfg!(windows) && cfg!(target_arch = "x86_64"))
                || ((cfg!(target_os = "linux") || cfg!(target_os = "macos")) && (cfg!(target_arch = "x86_64") || cfg!(target_arch = "aarch64")));
            if !target_with_walker {
                return Err(
                    "roots by maps (--raizes=mapas) exist on Windows x86-64 and on Linux and macOS on x86-64 and aarch64".to_string(),
                );
            }
            if !crate::gerador::GERADOR_EMBUTIDO || std::env::var("DARTFORGE_GERADOR").is_ok_and(|g| g == "clang") {
                return Err("roots by maps (--raizes=mapas) require dartforge's embedded generator (Clang does not run the maps pass)".to_string());
            }
            Ok(true)
        }
        other => Err(format!("DARTFORGE_RAIZES={other}: the roots modes are `sombra` and `mapas`")),
    }
}

/// The stack trace in the VM format (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
/// §13.14), through the variable `DARTFORGE_RASTRO_VM` (the `--rastro=` of
/// `compile-native` sets it):
///
/// * absent, empty or `nenhum`: no table, the usual IR, and the text of the
///   stack trace is today's;
/// * `simbolico`: each call that may appear in a stack trace gets a label
///   before it and an entry in the `dfpcl` section (`llvm/stack_trace.rs`): the
///   address of the label, the function (name and url) and the line and column. The
///   runtime keeps the return addresses at the `throw` and symbolizes only when
///   the text is requested.
///
/// Only where the runtime walks the native stack (Windows x86-64; Linux and macOS
/// on x86-64 and aarch64). The JIT does not emit the table. The variable goes into the key
/// of the compiled SDK.
///
/// # Errors
/// An unknown value, or `simbolico` on a target without the walker.
pub fn symbolic_stack_trace() -> Result<bool, String> {
    let value = std::env::var("DARTFORGE_RASTRO_VM").unwrap_or_default();
    match value.as_str() {
        "" | "nenhum" => Ok(false),
        "simbolico" => {
            let with_walker = (cfg!(windows) && cfg!(target_arch = "x86_64"))
                || ((cfg!(target_os = "linux") || cfg!(target_os = "macos")) && (cfg!(target_arch = "x86_64") || cfg!(target_arch = "aarch64")));
            if !with_walker {
                return Err("the symbolic stack trace (--rastro=simbolico) exists on Windows x86-64 and on Linux and macOS on x86-64 and aarch64".to_string());
            }
            Ok(true)
        }
        other => Err(format!("DARTFORGE_RASTRO_VM={other}: the stack traces are `simbolico` and `nenhum`")),
    }
}

/// The cap of the `rewrite-statepoints-for-gc` budget per function
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §3.4): `live values × collection
/// points`. Above it the function stays on the shadow stack (the residual frame,
/// §3.7). `DARTFORGE_ORCAMENTO_MAPAS=<n>` changes the cap (it goes into the key of the compiled
/// SDK); the default, 250 000, is 500 values rooted across 500
/// points.
pub fn maps_budget() -> u64 {
    std::env::var("DARTFORGE_ORCAMENTO_MAPAS").ok().and_then(|v| v.trim().parse().ok()).unwrap_or(250_000)
}

/// The check build of the walk by maps (§15.2, E2.5):
/// `DARTFORGE_RAIZES_CONFERIR=1` at compile time, with `--raizes=mapas`. Each
/// `gc` function also writes, before each collection point, its live values
/// into slots of the shadow-stack frame; with `DARTFORGE_GC_PERCURSO=conferir`
/// at run time, the runtime requires the walk by maps to have visited them.
pub fn check_roots() -> bool {
    std::env::var("DARTFORGE_RAIZES_CONFERIR").is_ok_and(|v| v == "1")
}

/// A test sabotage turned on at compile time (`DARTFORGE_SABOTAGEM=a,b,…`,
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3 and §14.7). Each directed
/// test of maps mode must fail with its own; a test that does not fail
/// with the sabotage does not count. Those of the emitter:
///
/// * `folha:<extern>`: declares the extern `"gc-leaf-function"` even though it collects
///   (the map loses the records of the calls to it);
/// * `pouso_sem_topo`: the landing pad of an `invoke` does not restore the top of the
///   shadow stack;
/// * `sem_uso_ficticio`: the `"deopt"` operand of the collecting calls comes out empty
///   (the operands and the live values leave the map);
/// * `quadro_sujo` (D5): the shadow-stack frame is born with 4098 in every
///   slot instead of zero (a slot read before being written becomes an invalid root);
/// * `bruto_no_mapa`: an even raw value (4098) live as a root in every
///   `gc` function. The value has the shape of an object handle (remainder 2 mod 8,
///   `layout::e_objeto`) and points to no block: the handle validation
///   of the collector rejects it.
///
/// The variable goes into the key of the compiled SDK.
pub fn sabotage(name: &str) -> bool {
    std::env::var("DARTFORGE_SABOTAGEM").unwrap_or_default().split(',').any(|s| s.trim() == name)
}

/// Extern `name` is in the `folha:<name>` sabotage.
pub fn sabotaged_leaf(name: &str) -> bool {
    std::env::var("DARTFORGE_SABOTAGEM").unwrap_or_default().split(',').any(|s| s.trim().strip_prefix("folha:") == Some(name))
}

/// The `target datalayout`/`target triple` header of the module.
///
/// Windows and Linux x86-64 pin the exact strings that the process's `LLJIT`
/// uses (the JIT rejects a module with a target different from its own). On the other
/// hosts (macOS, Linux aarch64) the module comes out without a header and Clang and
/// `LLJIT` impose the host's: the macOS triple carries the system version
/// (`arm64-apple-darwin24.1.0`), which does not fit in a constant.
pub fn cabecalho_ir() -> &'static str {
    match sistema() {
        Sistema::Windows => {
            "target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\ntarget triple = \"x86_64-pc-windows-msvc\"\n\n"
        }
        Sistema::Linux if cfg!(target_arch = "x86_64") => {
            "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\ntarget triple = \"x86_64-unknown-linux-gnu\"\n\n"
        }
        _ => "",
    }
}

/// The section of the static objects of the image (the string literals,
/// docs/NATIVO-ESPACO-UNIFICADO.md §2.11), read-only: `.dfimg$m` in COFF
/// (the linker merges `.dfimg$a`, `$m` and `$z` in order), `dfimg` in ELF (a
/// C identifier name, so that the linker defines `__start_dfimg`/`__stop_dfimg`) and
/// `__DATA_CONST,__dfimg` in Mach-O.
pub const fn image_section() -> &'static str {
    match sistema() {
        Sistema::Windows => ".dfimg$m",
        Sistema::Linux => "dfimg",
        Sistema::MacOs => "__DATA_CONST,__dfimg",
    }
}

/// The start and end markers of the section of the statics of an image
/// ([`image_section`]): the module-level declarations that define them (or
/// reference them) and the two `ptr` operands. Each module that registers the image
/// writes them; in COFF they are `linkonce_odr` in a `comdat` (one copy per image), and
/// in ELF and Mach-O the linker defines the section start and end symbols — the
/// anchor guarantees that the section exists even without any literal.
pub fn image_markers() -> (String, &'static str, &'static str) {
    match sistema() {
        Sistema::Windows => (
            "$df.img.a = comdat any\n$df.img.z = comdat any\n\
             @df.img.a = linkonce_odr hidden constant [2 x i64] zeroinitializer, section \".dfimg$a\", comdat, align 8\n\
             @df.img.z = linkonce_odr hidden constant [2 x i64] zeroinitializer, section \".dfimg$z\", comdat, align 8\n"
                .to_string(),
            "@df.img.a",
            "@df.img.z",
        ),
        Sistema::Linux => (
            "$df.img.ancora = comdat any\n\
             @df.img.ancora = linkonce_odr hidden constant [2 x i64] zeroinitializer, section \"dfimg\", comdat, align 8\n\
             @__start_dfimg = external hidden global i8\n@__stop_dfimg = external hidden global i8\n"
                .to_string(),
            "@__start_dfimg",
            "@__stop_dfimg",
        ),
        Sistema::MacOs => (
            "@df.img.ancora = linkonce_odr hidden constant [2 x i64] zeroinitializer, section \"__DATA_CONST,__dfimg\", align 8\n\
             @\"\\01section$start$__DATA_CONST$__dfimg\" = external hidden global i8\n\
             @\"\\01section$end$__DATA_CONST$__dfimg\" = external hidden global i8\n"
                .to_string(),
            "@\"\\01section$start$__DATA_CONST$__dfimg\"",
            "@\"\\01section$end$__DATA_CONST$__dfimg\"",
        ),
    }
}

/// Whether the object format has `comdat`. Mach-O does not: there a
/// `linkonce_odr` definition alone becomes a weak symbol, and the linker keeps one.
pub const fn has_comdat() -> bool {
    !matches!(sistema(), Sistema::MacOs)
}

/// Extension of the object (without the dot).
pub const fn ext_object() -> &'static str {
    match sistema() {
        Sistema::Windows => "obj",
        _ => "o",
    }
}

/// Extension of the static library (without the dot).
pub const fn static_ext() -> &'static str {
    match sistema() {
        Sistema::Windows => "lib",
        _ => "a",
    }
}

/// File name of the shared library `base` (`base.dll`,
/// `libbase.so`, `libbase.dylib`).
///
/// ```
/// let n = dartforge_emit_native::alvo::shared_name("dfsdk_0");
/// assert!(n.contains("dfsdk_0"));
/// ```
pub fn shared_name(base: &str) -> String {
    match sistema() {
        Sistema::Windows => format!("{base}.dll"),
        Sistema::Linux => format!("lib{base}.so"),
        Sistema::MacOs => format!("lib{base}.dylib"),
    }
}

/// Name of the executable `base` (`base.exe` on Windows, `base` on the others).
pub fn executable_name(base: &str) -> String {
    match sistema() {
        Sistema::Windows => format!("{base}.exe"),
        _ => base.to_string(),
    }
}

/// Name of Clang in the `bin` folder of an LLVM distribution.
pub const fn nome_clang() -> &'static str {
    match sistema() {
        Sistema::Windows => "clang.exe",
        _ => "clang",
    }
}

/// Clang flags that depend on the object format, for every IR.
///
/// On Windows, `-mno-incremental-linker-compatible` zeroes the `TimeDateStamp` of the
/// COFF header: without it, the same IR gave different objects at byte 4
/// (measured), and the object was no longer a function of the key. ELF and Mach-O have no
/// timestamp.
pub const fn object_flags() -> &'static [&'static str] {
    match sistema() {
        Sistema::Windows => &["-mno-incremental-linker-compatible"],
        _ => &[],
    }
}

/// Clang flags for the objects that go into the shared library
/// of the SDK: position-independent code outside Windows.
pub const fn shared_object_flags() -> &'static [&'static str] {
    match sistema() {
        Sistema::Windows => &[],
        _ => &["-fPIC"],
    }
}

/// System libraries that the runtime (a Rust `staticlib`) requires at
/// link time, in the Clang driver format.
///
/// It is the list `rustc --print native-static-libs` prints for each
/// host. The AOT link no longer goes through the Clang driver (each
/// system has its own linker: `ligador.rs`, `ligador_windows.rs`,
/// `ligador_macos.rs`); the list serves whoever links through Clang in a
/// development tree (the JIT tests).
pub const fn system_libraries() -> &'static [&'static str] {
    match sistema() {
        // `crypt32`: the system certificate store (the TLS
        // roots, `tls.rs`).
        Sistema::Windows => &["-lws2_32", "-luserenv", "-lntdll", "-liphlpapi", "-lbcrypt", "-ladvapi32", "-lcrypt32", "-lkernel32"],
        Sistema::Linux => &["-lgcc_s", "-lutil", "-lrt", "-lpthread", "-lm", "-ldl", "-lc"],
        // `CoreFoundation`: `Platform.localeName` (io_plataforma.rs);
        // `Security`: the system keychain (the TLS roots, `tls.rs`).
        Sistema::MacOs => &["-lSystem", "-lc", "-lm", "-liconv", "-framework", "CoreFoundation", "-framework", "Security"],
    }
}

/// The root of the macOS SDK in a development tree: the `SDKROOT` of the
/// environment or the SDK that `xcrun` points to (queried once). The
/// distribution does not use it: it carries the `.tbd` files in the link sysroot
/// (`ligador_macos.rs`). `None` outside macOS or without Xcode.
pub fn macos_sdk_root() -> Option<&'static std::path::Path> {
    static ROOT: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        if sistema() != Sistema::MacOs {
            return None;
        }
        if let Some(r) = std::env::var_os("SDKROOT").filter(|r| !r.is_empty()) {
            return Some(std::path::PathBuf::from(r));
        }
        let exit = std::process::Command::new("xcrun").args(["--sdk", "macosx", "--show-sdk-path"]).output().ok()?;
        let path = String::from_utf8(exit.stdout).ok()?;
        let path = path.trim();
        (exit.status.success() && !path.is_empty()).then(|| std::path::PathBuf::from(path))
    })
    .as_deref()
}

/// The link arguments that the system requires in every Clang link
/// (executable, SDK DLL, runtime): the system libraries and, on
/// macOS, the SDK root (`-isysroot`), without which neither Apple's `ld`
/// called by LLVM's Clang nor `ld64.lld` finds `-lSystem`.
pub fn argumentos_de_ligacao() -> Vec<std::ffi::OsString> {
    let mut v: Vec<std::ffi::OsString> = Vec::new();
    if let Some(root) = macos_sdk_root() {
        v.push("-isysroot".into());
        v.push(root.as_os_str().to_owned());
    }
    v.extend(system_libraries().iter().map(std::ffi::OsString::from));
    v
}
