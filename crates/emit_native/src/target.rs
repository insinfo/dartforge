//! The immutable destination selected for a compilation: one of the five native
//! systems or WebAssembly (`wasm32-linear`, docs/PLANO-WASM-MVP.md §3.1).
//!
//! Everything that changes between Windows, Linux, macOS and WebAssembly lives here — the header of the
//! LLVM IR, the extensions of the artifacts, the Clang and link flags and the
//! format of the shared library of the source SDK. The rest of the backend
//! asks this module instead of testing scattered `cfg!`s.
//!
//! A process selects one destination before compilation. Native builds preserve
//! their historical headers; cross builds carry an explicit LLVM triple.

/// The systems supported by the native backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    /// COFF, MSVC ABI, `.obj`/`.lib`/`.dll`/`.exe`.
    Windows,
    /// ELF, `.o`/`.a`/`.so`, executable without extension.
    Linux,
    /// Mach-O, `.o`/`.a`/`.dylib`, executable without extension.
    MacOs,
    /// No operating system: a WebAssembly module (`.o`/`.a`, module `.wasm`)
    /// over a 32-bit linear memory, whose only outside world is the host ABI
    /// (`dartforge_host_v1`, docs/WASM-HOST-ABI-V1.md). Not Unix and not Windows:
    /// no shared libraries, threads, native stack walking or system linker.
    Wasm,
}

/// A supported destination: the five 64-bit native systems and WebAssembly.
/// Windows currently uses x64, including when its executable runs under Windows
/// ARM64's x64 compatibility layer.
///
/// ```
/// use dartforge_emit_native::target::{NativeTarget, Os};
/// let target = NativeTarget::parse("linux-arm64").unwrap();
/// assert_eq!(target.triple(), "aarch64-unknown-linux-gnu");
/// assert_eq!(target.os(), Os::Linux);
/// assert!(target.is_arm64());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeTarget {
    WindowsX64,
    LinuxX64,
    LinuxArm64,
    MacosX64,
    MacosArm64,
    /// `wasm32-linear`: WebAssembly with a 32-bit linear memory and the runtime's
    /// own collector, never WasmGC (docs/PLANO-WASM-MVP.md §3.1).
    Wasm32,
}

impl NativeTarget {
    /// The destination of a native compilation without `--target`.
    pub fn host() -> Self {
        if cfg!(windows) {
            Self::WindowsX64
        } else if cfg!(target_os = "macos") {
            if cfg!(target_arch = "aarch64") {
                Self::MacosArm64
            } else {
                Self::MacosX64
            }
        } else if cfg!(target_arch = "aarch64") {
            Self::LinuxArm64
        } else {
            Self::LinuxX64
        }
    }

    /// Parses a Rust/LLVM triple (or the documented short platform name).
    ///
    /// # Errors
    /// Rejects unknown architectures, ABIs and operating systems.
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "windows-x64" | "x86_64-pc-windows-msvc" => Ok(Self::WindowsX64),
            "linux-x64" | "x86_64-unknown-linux-gnu" => Ok(Self::LinuxX64),
            "linux-arm64" | "aarch64-unknown-linux-gnu" => Ok(Self::LinuxArm64),
            "macos-x64" | "x86_64-apple-darwin" => Ok(Self::MacosX64),
            "macos-arm64" | "aarch64-apple-darwin" | "arm64-apple-darwin" => Ok(Self::MacosArm64),
            "wasm32" | "wasm32-linear" | "wasm32-unknown-unknown" => Ok(Self::Wasm32),
            _ => Err(format!(
                "unsupported target {name:?}; use windows-x64, linux-x64, linux-arm64, macos-x64, macos-arm64 or wasm32"
            )),
        }
    }

    /// Canonical LLVM/Rust triple of this destination.
    pub fn triple(self) -> &'static str {
        match self {
            Self::WindowsX64 => "x86_64-pc-windows-msvc",
            Self::LinuxX64 => "x86_64-unknown-linux-gnu",
            Self::LinuxArm64 => "aarch64-unknown-linux-gnu",
            Self::MacosX64 => "x86_64-apple-darwin",
            Self::MacosArm64 => "aarch64-apple-darwin",
            Self::Wasm32 => "wasm32-unknown-unknown",
        }
    }

    /// Operating system whose ABI and library conventions apply.
    pub fn os(self) -> Os {
        match self {
            Self::WindowsX64 => Os::Windows,
            Self::LinuxX64 | Self::LinuxArm64 => Os::Linux,
            Self::MacosX64 | Self::MacosArm64 => Os::MacOs,
            Self::Wasm32 => Os::Wasm,
        }
    }

    /// Whether code generation and C FFI use AArch64.
    pub fn is_arm64(self) -> bool {
        matches!(self, Self::LinuxArm64 | Self::MacosArm64)
    }

    /// Whether this is the WebAssembly destination.
    pub fn is_wasm(self) -> bool {
        self == Self::Wasm32
    }

    /// The width of a pointer of the generated code, in bytes. Dart values stay
    /// 64-bit everywhere (`int`, `double` and references in 8-byte slots); only
    /// addresses narrow on wasm32.
    pub fn pointer_bytes(self) -> u32 {
        if self.is_wasm() { 4 } else { 8 }
    }
}

static DESTINATION: std::sync::OnceLock<NativeTarget> = std::sync::OnceLock::new();

/// Selects one immutable destination before starting compiler worker threads.
///
/// # Errors
/// A process cannot switch targets after its first target-dependent operation.
pub fn select(target: NativeTarget) -> Result<(), String> {
    match DESTINATION.set(target) {
        Ok(()) => Ok(()),
        Err(_) if current() == target => Ok(()),
        Err(_) => {
            Err("native target was already initialized; use a separate compilation process".into())
        }
    }
}

/// The frozen selection, initialized to the host when no destination was selected.
pub fn current() -> NativeTarget {
    *DESTINATION.get_or_init(NativeTarget::host)
}
/// Whether this process is compiling for a different native destination.
pub fn is_cross() -> bool {
    // An unsupported compiler host must never supply its own runtime archive
    // for a supported destination with a different ABI (notably Windows ARM64).
    let native_host = (cfg!(windows) && cfg!(target_arch = "x86_64"))
        || ((cfg!(target_os = "linux") || cfg!(target_os = "macos"))
            && (cfg!(target_arch = "x86_64") || cfg!(target_arch = "aarch64")));
    !native_host || current() != NativeTarget::host()
}
/// Architecture of the generated code, independent of the compiler CPU.
pub fn is_aarch64() -> bool {
    current().is_arm64()
}
/// Whether the generated code uses a supported Unix ABI.
pub fn is_unix() -> bool {
    matches!(target_os(), Os::Linux | Os::MacOs)
}

/// Whether the generated code is WebAssembly.
pub fn is_wasm() -> bool {
    current().is_wasm()
}

/// The operating system of the generated code, independent of compiler host.
///
/// ```
/// use dartforge_emit_native::target::{NativeTarget, target_os};
/// assert_eq!(target_os(), NativeTarget::host().os());
/// ```
pub fn target_os() -> Os {
    current().os()
}

/// The exception model of the generated code
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13), through the variable
/// `DARTFORGE_EXCEPTIONS` (the `--exceptions=` of `compile-native` sets it):
///
/// * absent, empty or `check`: the exception is a pending state of the runtime that
///   the caller checks after each call — the usual IR, byte for byte;
/// * `tables`: a Dart call that throws unwinds the stack up to the
///   `landingpad` of whoever handles it (`optimize/tables.rs`), with the
///   runtime's personality (`dartforge_personality`). Only on Windows
///   x86-64, the only target with the personality written, and without the function
///   call trace (`DARTFORGE_CALL_TRACE=1` counts entries and exits, and
///   unwinding skips the exits).
///
/// The program and the SDK have to come out in the same model: the variable goes into the key
/// of the compiled SDK (`sdk_module::sdk_key`).
///
/// # Errors
/// An unknown value, or `tables` where the mode does not exist.
pub fn table_exceptions() -> Result<bool, String> {
    let value = std::env::var("DARTFORGE_EXCEPTIONS").unwrap_or_default();
    match value.as_str() {
        "" | "check" => Ok(false),
        "tables" if is_wasm() => Err(
            "exceptions by tables (--exceptions=tables) are not available for WebAssembly yet; use the default model".to_string(),
        ),
        "tables" => {
            // Windows x86-64 (SEH) and, through the runtime's Itanium
            // personality (Step 4, written and never run), Linux and macOS
            // on x86-64 and aarch64.
            let seh = target_os() == Os::Windows && !is_aarch64();
            let itanium = is_unix();
            if !(seh || itanium) {
                return Err(
                    "exceptions by tables (--exceptions=tables) only exist on Windows x86-64 and, on Linux and macOS, on x86-64 and aarch64".to_string()
                );
            }
            if std::env::var("DARTFORGE_CALL_TRACE").is_ok_and(|v| v == "1") {
                return Err("exceptions by tables (--exceptions=tables) do not combine with DARTFORGE_CALL_TRACE=1".to_string());
            }
            Ok(true)
        }
        other => Err(format!(
            "DARTFORGE_EXCEPTIONS={other}: the exception models are `check` and `tables`"
        )),
    }
}

/// The memory policy of the generated code (docs/ARC-CICLOS-ESPECIFICACAO.md
/// §18.1, decision 1, and §24; docs/ARC-IMPLEMENTACAO.md), through the variable
/// `DARTFORGE_MEMORY` (the `--memory=` of `aot` and `compile-native` sets it):
///
/// * absent, empty or `tracing`: the usual tracing collector — the usual
///   IR, byte for byte;
/// * `arc`: reference counting with cycle collection. The entry turns
///   ARC on in the runtime (`dartforge_memory_arc_v1`, the version of the memory
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
    let value = std::env::var("DARTFORGE_MEMORY").unwrap_or_default();
    match value.as_str() {
        "" | "tracing" => Ok(false),
        "arc" if is_wasm() => Err(
            "the WebAssembly target uses the tracing collector (--memory=arc is native only)"
                .to_string(),
        ),
        "arc" => Ok(true),
        other => Err(format!(
            "DARTFORGE_MEMORY={other}: the memory policies are `tracing` and `arc`"
        )),
    }
}

/// The young generation of ARC memory (docs/ARC-IMPLEMENTACAO.md): the entry
/// turns ARC on through `dartforge_memory_arc_nursery_v1`, and young objects are
/// not counted until a minor collection promotes them. `DARTFORGE_ARC_NURSERY=0`
/// keeps the pure ARC (`dartforge_memory_arc_v1`, every write counted on the
/// spot), for measurement; the variable goes into the key of the compiled SDK.
pub fn arc_nursery() -> bool {
    !std::env::var("DARTFORGE_ARC_NURSERY").is_ok_and(|v| v == "0")
}

/// Under the nursery, the unprepared code writes into a young object inline, as
/// in tracing memory, and only a write into an older object goes through the
/// runtime, which counts the replacement ([`crate::hir::Module::arc_nursery_writes`]).
/// The prepared code keeps every write in the runtime (its proofs count the
/// owners of each one).
pub fn arc_nursery_writes() -> bool {
    arc_nursery() && !arc_prepared().unwrap_or(true)
}

/// The published ARC preparation (owners in the HIR, `optimize::arc`), through the
/// variable `DARTFORGE_ARC_PREPARED=1`: only with ARC memory. Absent,
/// ARC follows the legacy regime of the CLI (no tokens in the code).
pub fn arc_prepared() -> Result<bool, String> {
    if !std::env::var("DARTFORGE_ARC_PREPARED").is_ok_and(|v| v == "1") {
        return Ok(false);
    }
    if !arc_memory()? {
        return Err("DARTFORGE_ARC_PREPARED=1 requires ARC memory (`--memory=arc`)".into());
    }
    Ok(true)
}

/// `DARTFORGE_ARC_TOKENS=virtual`: the opt-in fast mode of the prepared ARC,
/// *without prompt death*. The tokens are virtual
/// ([`crate::hir::Module::arc_virtual_tokens`]): proved by the preparation, free at
/// run time, and the deferred roots protect what they would; an object dies at the
/// synchronization after its last reference drops, as in the plain ARC. Without
/// the variable the prepared code keeps exact tokens: it calls the runtime for
/// every token (`dartforge_arc_retain`/`dartforge_arc_release`), which keeps the
/// inventory and counts them. The variable goes into the key of the compiled SDK.
pub fn arc_virtual_tokens() -> bool {
    std::env::var("DARTFORGE_ARC_TOKENS").is_ok_and(|v| v == "virtual")
}

/// The real ARC: the prepared code with exact tokens over the pure ARC with prompt
/// death (`dartforge_memory_arc_prompt_v1`): every reference counts from its first
/// write, the tokens are counted owners, and an object dies at its last release, not
/// at the next collection. `DARTFORGE_ARC_PROMPT=0` keeps the earlier prepared ARC
/// with exact tokens over the nursery (death at the synchronization), for
/// comparison. The variable goes into the key of the compiled SDK.
pub fn arc_prompt_death() -> bool {
    arc_prepared().unwrap_or(false)
        && !arc_virtual_tokens()
        && !std::env::var("DARTFORGE_ARC_PROMPT").is_ok_and(|v| v == "0")
}

/// Where the collector roots live in the generated code
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.8), through the variable
/// `DARTFORGE_ROOTS` (the `--roots=` of `compile-native` sets it):
///
/// * absent, empty or `shadow`: the roots frame of each function (the
///   shadow stack) — the usual IR, byte for byte;
/// * `maps`: the SSA values live across a call come out in the LLVM stack
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
/// An unknown value, or `maps` where the mode does not exist.
pub fn roots_by_maps() -> Result<bool, String> {
    let value = std::env::var("DARTFORGE_ROOTS").unwrap_or_default();
    match value.as_str() {
        "" | "shadow" => Ok(false),
        "maps" if is_wasm() => Err(
            "roots by maps (--roots=maps) need a native stack walk; WebAssembly keeps the roots in the shadow stack".to_string(),
        ),
        "maps" => {
            let target_with_walker = (target_os() == Os::Windows && !is_aarch64()) || is_unix();
            if !target_with_walker {
                return Err(
                    "roots by maps (--roots=maps) exist on Windows x86-64 and on Linux and macOS on x86-64 and aarch64".to_string(),
                );
            }
            if !crate::generator::EMBEDDED_GENERATOR
                || std::env::var("DARTFORGE_GENERATOR").is_ok_and(|g| g == "clang")
            {
                return Err("roots by maps (--roots=maps) require dartforge's embedded generator (Clang does not run the maps pass)".to_string());
            }
            Ok(true)
        }
        other => Err(format!(
            "DARTFORGE_ROOTS={other}: the roots modes are `shadow` and `maps`"
        )),
    }
}

/// The stack trace in the VM format (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
/// §13.14), through the variable `DARTFORGE_STACK_TRACE` (the `--stack-trace=` of
/// `compile-native` sets it):
///
/// * absent, empty or `none`: no table, the usual IR, and the text of the
///   stack trace is today's;
/// * `symbolic`: each call that may appear in a stack trace gets a label
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
/// An unknown value, or `symbolic` on a target without the walker.
pub fn symbolic_stack_trace() -> Result<bool, String> {
    let value = std::env::var("DARTFORGE_STACK_TRACE").unwrap_or_default();
    match value.as_str() {
        "" | "none" => Ok(false),
        "symbolic" if is_wasm() => Err(
            "the symbolic stack trace (--stack-trace=symbolic) walks the native stack, which WebAssembly does not expose".to_string(),
        ),
        "symbolic" => {
            let with_walker = (target_os() == Os::Windows && !is_aarch64()) || is_unix();
            if !with_walker {
                return Err("the symbolic stack trace (--stack-trace=symbolic) exists on Windows x86-64 and on Linux and macOS on x86-64 and aarch64".to_string());
            }
            Ok(true)
        }
        other => Err(format!(
            "DARTFORGE_STACK_TRACE={other}: the stack traces are `symbolic` and `none`"
        )),
    }
}

/// The cap of the `rewrite-statepoints-for-gc` budget per function
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §3.4): `live values × collection
/// points`. Above it the function stays on the shadow stack (the residual frame,
/// §3.7). `DARTFORGE_MAPS_BUDGET=<n>` changes the cap (it goes into the key of the compiled
/// SDK); the default, 250 000, is 500 values rooted across 500
/// points.
pub fn maps_budget() -> u64 {
    std::env::var("DARTFORGE_MAPS_BUDGET")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(250_000)
}

/// The check build of the walk by maps (§15.2, E2.5):
/// `DARTFORGE_ROOTS_CHECK=1` at compile time, with `--roots=maps`. Each
/// `gc` function also writes, before each collection point, its live values
/// into slots of the shadow-stack frame; with `DARTFORGE_GC_WALK=check`
/// at run time, the runtime requires the walk by maps to have visited them.
pub fn check_roots() -> bool {
    std::env::var("DARTFORGE_ROOTS_CHECK").is_ok_and(|v| v == "1")
}

/// A test sabotage turned on at compile time (`DARTFORGE_SABOTAGE=a,b,…`,
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3 and §14.7). Each directed
/// test of maps mode must fail with its own; a test that does not fail
/// with the sabotage does not count. Those of the emitter:
///
/// * `leaf:<extern>`: declares the extern `"gc-leaf-function"` even though it collects
///   (the map loses the records of the calls to it);
/// * `pad_without_top`: the landing pad of an `invoke` does not restore the top of the
///   shadow stack;
/// * `no_fake_use`: the `"deopt"` operand of the collecting calls comes out empty
///   (the operands and the live values leave the map);
/// * `dirty_frame` (D5): the shadow-stack frame is born with 4098 in every
///   slot instead of zero (a slot read before being written becomes an invalid root);
/// * `raw_in_map`: an even raw value (4098) live as a root in every
///   `gc` function. The value has the shape of an object handle (remainder 2 mod 8,
///   `layout::e_objeto`) and points to no block: the handle validation
///   of the collector rejects it.
///
/// The variable goes into the key of the compiled SDK.
pub fn sabotage(name: &str) -> bool {
    std::env::var("DARTFORGE_SABOTAGE")
        .unwrap_or_default()
        .split(',')
        .any(|s| s.trim() == name)
}

/// Extern `name` is in the `leaf:<name>` sabotage.
pub fn sabotaged_leaf(name: &str) -> bool {
    std::env::var("DARTFORGE_SABOTAGE")
        .unwrap_or_default()
        .split(',')
        .any(|s| s.trim().strip_prefix("leaf:") == Some(name))
}

/// The `target datalayout`/`target triple` header of the module.
///
/// Windows and Linux x86-64 pin the exact strings that the process's `LLJIT`
/// uses (the JIT rejects a module with a target different from its own). On the other
/// hosts (macOS, Linux aarch64) the module comes out without a header and Clang and
/// `LLJIT` impose the host's: the macOS triple carries the system version
/// (`arm64-apple-darwin24.1.0`), which does not fit in a constant.
pub fn ir_header() -> &'static str {
    if is_cross() {
        return match current() {
            NativeTarget::WindowsX64 => "target triple = \"x86_64-pc-windows-msvc\"\n\n",
            NativeTarget::LinuxX64 => "target triple = \"x86_64-unknown-linux-gnu\"\n\n",
            NativeTarget::LinuxArm64 => "target triple = \"aarch64-unknown-linux-gnu\"\n\n",
            NativeTarget::MacosX64 => "target triple = \"x86_64-apple-darwin\"\n\n",
            NativeTarget::MacosArm64 => "target triple = \"aarch64-apple-darwin\"\n\n",
            NativeTarget::Wasm32 => {
                "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"\ntarget triple = \"wasm32-unknown-unknown\"\n\n"
            }
        };
    }
    match target_os() {
        Os::Windows => {
            "target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\ntarget triple = \"x86_64-pc-windows-msvc\"\n\n"
        }
        Os::Linux if cfg!(target_arch = "x86_64") => {
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
pub fn image_section() -> &'static str {
    match target_os() {
        Os::Windows => ".dfimg$m",
        Os::Linux => "dfimg",
        Os::MacOs => "__DATA_CONST,__dfimg",
        // A data segment; `wasm-ld` defines `__start_dfimg`/`__stop_dfimg`.
        Os::Wasm => "dfimg",
    }
}

/// The start and end markers of the section of the statics of an image
/// ([`image_section`]): the module-level declarations that define them (or
/// reference them) and the two `ptr` operands. Each module that registers the image
/// writes them; in COFF they are `linkonce_odr` in a `comdat` (one copy per image), and
/// in ELF and Mach-O the linker defines the section start and end symbols — the
/// anchor guarantees that the section exists even without any literal.
pub fn image_markers() -> (String, &'static str, &'static str) {
    image_markers_for(target_os())
}

/// The marker protocol of an explicit object format, independent of host state.
pub(crate) fn image_markers_for(os: Os) -> (String, &'static str, &'static str) {
    match os {
        Os::Windows => (
            "$df.img.a = comdat any\n$df.img.z = comdat any\n\
             @df.img.a = linkonce_odr hidden constant [2 x i64] zeroinitializer, section \".dfimg$a\", comdat, align 8\n\
             @df.img.z = linkonce_odr hidden constant [2 x i64] zeroinitializer, section \".dfimg$z\", comdat, align 8\n"
                .to_string(),
            "@df.img.a",
            "@df.img.z",
        ),
        Os::Linux => (
            "$df.img.anchor = comdat any\n\
             @df.img.anchor = linkonce_odr hidden constant [2 x i64] zeroinitializer, section \"dfimg\", comdat, align 8\n\
             @__start_dfimg = external hidden global i8\n@__stop_dfimg = external hidden global i8\n"
                .to_string(),
            "@__start_dfimg",
            "@__stop_dfimg",
        ),
        Os::MacOs => (
            "@df.img.anchor = linkonce_odr hidden constant [2 x i64] zeroinitializer, section \"__DATA_CONST,__dfimg\", align 8\n\
             @\"\\01section$start$__DATA_CONST$__dfimg\" = external hidden global i8\n\
             @\"\\01section$end$__DATA_CONST$__dfimg\" = external hidden global i8\n"
                .to_string(),
            "@\"\\01section$start$__DATA_CONST$__dfimg\"",
            "@\"\\01section$end$__DATA_CONST$__dfimg\"",
        ),
        // `wasm-ld` defines the markers of a live segment and drops an
        // unreferenced one (an anchor would be dropped with it): weak markers
        // resolve to 0..0, an empty image, when no static object exists.
        Os::Wasm => (
            "@__start_dfimg = extern_weak hidden global i8\n@__stop_dfimg = extern_weak hidden global i8\n"
                .to_string(),
            "@__start_dfimg",
            "@__stop_dfimg",
        ),
    }
}

/// Whether the object format has `comdat`. Mach-O does not: there a
/// `linkonce_odr` definition alone becomes a weak symbol, and the linker keeps one.
pub fn has_comdat() -> bool {
    !matches!(target_os(), Os::MacOs)
}

/// Extension of the object (without the dot).
pub fn ext_object() -> &'static str {
    match target_os() {
        Os::Windows => "obj",
        _ => "o",
    }
}

/// Extension of the static library (without the dot).
pub fn static_ext() -> &'static str {
    match target_os() {
        Os::Windows => "lib",
        _ => "a",
    }
}

/// File name of the shared library `base` (`base.dll`,
/// `libbase.so`, `libbase.dylib`).
///
/// ```
/// let n = dartforge_emit_native::target::shared_name("dfsdk_0");
/// assert!(n.contains("dfsdk_0"));
/// ```
///
/// # Panics
/// On WebAssembly, which has no shared libraries: the SDK goes into the module.
pub fn shared_name(base: &str) -> String {
    match target_os() {
        Os::Windows => format!("{base}.dll"),
        Os::Linux => format!("lib{base}.so"),
        Os::MacOs => format!("lib{base}.dylib"),
        Os::Wasm => panic!("WebAssembly has no shared libraries ({base})"),
    }
}

/// Name of the executable `base` (`base.exe` on Windows, `base.wasm` for
/// WebAssembly, `base` on the others).
pub fn executable_name(base: &str) -> String {
    match target_os() {
        Os::Windows => format!("{base}.exe"),
        Os::Wasm => format!("{base}.wasm"),
        Os::Linux | Os::MacOs => base.to_string(),
    }
}

/// The LLVM CPU of `wasm32-linear`: `generic` brings sign extension,
/// saturating conversions, bulk memory, multi-value, reference types and
/// mutable globals (docs/PLANO-WASM-MVP.md §3.1).
pub const WASM_CPU: &str = "generic";

/// The features `wasm32-linear` adds to [`WASM_CPU`]. Exception handling with
/// `exnref` is in the profile too, but only the exception model by tables turns
/// it on (it also needs LLVM's `-wasm-enable-eh`).
pub const WASM_FEATURES: &str = "+simd128,+tail-call,+extended-const";

/// Name of Clang in the `bin` folder of an LLVM distribution.
pub fn clang_name() -> &'static str {
    match NativeTarget::host().os() {
        Os::Windows => "clang.exe",
        _ => "clang",
    }
}

/// Clang flags that depend on the object format, for every IR.
///
/// On Windows, `-mno-incremental-linker-compatible` zeroes the `TimeDateStamp` of the
/// COFF header: without it, the same IR gave different objects at byte 4
/// (measured), and the object was no longer a function of the key. ELF and Mach-O have no
/// timestamp. WebAssembly takes the profile's CPU and features ([`WASM_CPU`],
/// [`WASM_FEATURES`]).
pub fn object_flags() -> &'static [&'static str] {
    object_flags_for(target_os())
}

/// [`object_flags`] of an explicit object format, independent of host state.
pub(crate) fn object_flags_for(os: Os) -> &'static [&'static str] {
    match os {
        Os::Windows => &["-mno-incremental-linker-compatible"],
        Os::Wasm => &[
            "-mcpu=generic",
            "-msimd128",
            "-mtail-call",
            "-mextended-const",
        ],
        Os::Linux | Os::MacOs => &[],
    }
}

/// Clang flags for the objects that go into the shared library
/// of the SDK: position-independent code outside Windows.
///
/// # Panics
/// On WebAssembly, which has no shared libraries.
pub fn shared_object_flags() -> &'static [&'static str] {
    match target_os() {
        Os::Windows => &[],
        Os::Linux | Os::MacOs => &["-fPIC"],
        Os::Wasm => panic!("WebAssembly has no shared libraries"),
    }
}

/// System libraries that the runtime (a Rust `staticlib`) requires at
/// link time, in the Clang driver format.
///
/// It is the list `rustc --print native-static-libs` prints for each
/// host. The AOT link no longer goes through the Clang driver (each
/// system has its own linker: `linker.rs`, `linker_windows.rs`,
/// `linker_macos.rs`); the list serves whoever links through Clang in a
/// development tree (the JIT tests).
pub fn system_libraries() -> &'static [&'static str] {
    match target_os() {
        // `crypt32`: the system certificate store (the TLS
        // roots, `tls.rs`).
        Os::Windows => &[
            "-lws2_32",
            "-luserenv",
            "-lntdll",
            "-liphlpapi",
            "-lbcrypt",
            "-ladvapi32",
            "-lcrypt32",
            "-lkernel32",
        ],
        Os::Linux => &[
            "-lgcc_s",
            "-lutil",
            "-lrt",
            "-lpthread",
            "-lm",
            "-ldl",
            "-lc",
        ],
        // The runtime of a module imports only the host ABI.
        Os::Wasm => &[],
        // `CoreFoundation`: `Platform.localeName` (io_plataforma.rs);
        // `Security`: the system keychain (the TLS roots, `tls.rs`).
        Os::MacOs => &[
            "-lSystem",
            "-lc",
            "-lm",
            "-liconv",
            "-framework",
            "CoreFoundation",
            "-framework",
            "Security",
        ],
    }
}

/// The root of the macOS SDK in a development tree: the `SDKROOT` of the
/// environment or the SDK that `xcrun` points to (queried once). The
/// distribution does not use it: it carries the `.tbd` files in the link sysroot
/// (`linker_macos.rs`). `None` outside macOS or without Xcode.
pub fn macos_sdk_root() -> Option<&'static std::path::Path> {
    static ROOT: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        if target_os() != Os::MacOs {
            return None;
        }
        if let Some(r) = std::env::var_os("SDKROOT").filter(|r| !r.is_empty()) {
            return Some(std::path::PathBuf::from(r));
        }
        let exit = std::process::Command::new("xcrun")
            .args(["--sdk", "macosx", "--show-sdk-path"])
            .output()
            .ok()?;
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
pub fn link_arguments() -> Vec<std::ffi::OsString> {
    let mut v: Vec<std::ffi::OsString> = Vec::new();
    if let Some(root) = macos_sdk_root() {
        v.push("-isysroot".into());
        v.push(root.as_os_str().to_owned());
    }
    v.extend(system_libraries().iter().map(std::ffi::OsString::from));
    v
}
