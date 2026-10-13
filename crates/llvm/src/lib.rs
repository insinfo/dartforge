//! LLVM inside the dartforge process: the link with the library (`build.rs`), the
//! initialization of the native target, once per process, and the **AOT code
//! generator**: textual LLVM IR → native object or bitcode for link-time
//! optimization (LTO), without `clang` on the dartforge user's machine.
//!
//! [`gerar`] does what `clang -x ir -c` did in the AOT driver: it reads the module,
//! runs the optimization pipeline of the requested level (the same `default<On>`
//! of the `PassBuilder` that Clang runs) and emits through the host's target
//! machine, with the CPU that Clang assumes without `-march`. The JIT
//! (`crates/jit`) uses the same library and the same initialization
//! ([`init_native_target`]).
//!
//! # Why `unsafe`
//!
//! Every call of the LLVM C API is `unsafe`, and the ownership of the resources
//! (context, module, target machine, buffers, messages) follows the contract of
//! `llvm-c/*.h`, not the borrow checker. Each resource here belongs to a Rust
//! value with `Drop` (`Context`, `IrModule`, `TargetMachine`); nothing raw leaves
//! this file: the public API only exchanges `&str`, `Vec<u8>` and `String`.
//
// The crate's only `allow`: any new `unsafe` outside here is a compile error (the
// workspace's `unsafe_code = "deny"`).
#![allow(unsafe_code)]

pub mod check_rs4gc;

use llvm_sys::bit_writer::LLVMWriteBitcodeToMemoryBuffer;
use llvm_sys::core::{
    LLVMConstNull, LLVMContextCreate, LLVMContextDispose, LLVMCreateMemoryBufferWithMemoryRangeCopy, LLVMDeleteFunction,
    LLVMDisposeMemoryBuffer, LLVMDisposeMessage, LLVMDisposeModule, LLVMDisposeOperandBundle, LLVMGetBufferSize,
    LLVMGetBufferStart, LLVMGetDataLayoutStr, LLVMGetFirstBasicBlock, LLVMGetFirstFunction, LLVMGetFirstInstruction,
    LLVMGetFirstUse, LLVMGetGC, LLVMGetInstructionOpcode, LLVMGetNamedFunction, LLVMGetNextBasicBlock, LLVMGetNextFunction,
    LLVMGetNextInstruction, LLVMGetNumArgOperands, LLVMGetNumOperandBundleArgs, LLVMGetNumOperandBundles, LLVMGetNumOperands,
    LLVMGetOperand, LLVMGetOperandBundleAtIndex, LLVMGetOperandBundleTag, LLVMGetTarget, LLVMGetVersion, LLVMIsACallInst,
    LLVMIsAConstantExpr, LLVMIsAConstantInt, LLVMIsAGlobalValue, LLVMIsAInstruction, LLVMIsAInvokeInst, LLVMPrintModuleToString,
    LLVMSetDataLayout, LLVMSetOperand, LLVMSetTarget, LLVMTypeOf, LLVMConstIntGetZExtValue, LLVMGetCalledValue, LLVMGetValueName2,
    LLVMConstInt,
};
use llvm_sys::LLVMOpcode;
use llvm_sys::error::{LLVMDisposeErrorMessage, LLVMErrorRef, LLVMGetErrorMessage};
use llvm_sys::ir_reader::LLVMParseIRInContext2;
use llvm_sys::support::LLVMParseCommandLineOptions;
use llvm_sys::prelude::{LLVMContextRef, LLVMMemoryBufferRef, LLVMModuleRef};
use llvm_sys::target::{
    LLVM_InitializeNativeAsmParser, LLVM_InitializeNativeAsmPrinter, LLVM_InitializeNativeTarget, LLVMDisposeTargetData, LLVMSetModuleDataLayout,
};
use llvm_sys::target_machine::{
    LLVMCodeGenFileType, LLVMCodeGenOptLevel, LLVMCodeModel, LLVMCreateTargetDataLayout, LLVMCreateTargetMachine,
    LLVMDisposeTargetMachine, LLVMGetDefaultTargetTriple, LLVMGetHostCPUFeatures, LLVMGetHostCPUName, LLVMGetTargetFromTriple, LLVMRelocMode,
    LLVMTargetMachineEmitToMemoryBuffer, LLVMTargetMachineRef,
};
use llvm_sys::transforms::pass_builder::{
    LLVMCreatePassBuilderOptions, LLVMDisposePassBuilderOptions, LLVMRunPasses,
};
use std::ffi::{CStr, CString, c_char};
use std::ptr;
use std::sync::OnceLock;

/// Registers the native target, its `AsmPrinter` and its `AsmParser`, only once
/// per process. The `AsmParser` is what assembles `module asm` and inline `asm` in
/// the object (the minimal Windows CRT, `ligador_windows.rs`); without it LLVM
/// aborts the process ("Inline asm not supported by this streamer").
///
/// LLVM's routines are not safe under concurrency; the [`OnceLock`] guarantees a
/// single execution and memoizes a failure.
///
/// # Errors
/// When the linked LLVM has no backend for the host's architecture.
pub fn init_native_target() -> Result<(), String> {
    static ESTADO: OnceLock<Result<(), String>> = OnceLock::new();
    ESTADO
        .get_or_init(|| {
            // SAFETY: `get_or_init` guarantees a single, exclusive execution; the
            // routines only register the target in LLVM's global tables.
            let has_failed = unsafe {
                LLVM_InitializeNativeTarget() != 0
                    || LLVM_InitializeNativeAsmPrinter() != 0
                    || LLVM_InitializeNativeAsmParser() != 0
            };
            if has_failed {
                return Err("LLVM has no native backend for this architecture".to_owned());
            }
            // The code generator options, global to the process and read once
            // (`cl::opt` rejects a second occurrence).
            let arguments: Vec<CString> = generator_options().iter().map(|a| CString::new(a.as_str()).expect("option without NUL")).collect();
            let pointers: Vec<*const c_char> = arguments.iter().map(|a| a.as_ptr()).collect();
            // SAFETY: `argv` points to C strings alive until the end of the call.
            unsafe { LLVMParseCommandLineOptions(pointers.len() as i32, pointers.as_ptr(), c"dartforge".as_ptr()) };
            Ok(())
        })
        .clone()
}

/// The roots of a statepoint (the `"deopt"` operands,
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.12) may stay in the callee-saved
/// register where they already are (`use-registers-for-deopt-values`; the map says
/// which, with the `Register` location): without it, LLVM's default, each one is
/// written to a slot before every collecting call — which the shadow stack does not
/// pay. `DARTFORGE_MAPAS_REGISTRADORES=0` goes back to the slot (measurement).
pub fn roots_in_registers() -> bool {
    static SIM: OnceLock<bool> = OnceLock::new();
    *SIM.get_or_init(|| std::env::var("DARTFORGE_MAPAS_REGISTRADORES").map_or(true, |v| v.trim() != "0"))
}

/// The code generator options that dartforge passes to LLVM (the embedded one, and
/// `ld64.lld` through `-mllvm`), without the program name.
pub fn generator_options_without_program() -> Vec<String> {
    vec![format!("-use-registers-for-deopt-values={}", roots_in_registers())]
}

fn generator_options() -> Vec<String> {
    let mut v = vec!["dartforge".to_owned()];
    v.extend(generator_options_without_program());
    v
}

/// The version of the linked LLVM (`22.1.8`).
pub fn version() -> String {
    let (mut larger, mut minor, mut correctness) = (0u32, 0u32, 0u32);
    // SAFETY: the function only writes into the three integers.
    unsafe { LLVMGetVersion(&mut larger, &mut minor, &mut correctness) };
    format!("{larger}.{minor}.{correctness}")
}

/// What [`gerar`] produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formato {
    /// A native object for the host (COFF, ELF or Mach-O).
    Objeto,
    /// Bitcode for link-time optimization (`lld`'s LTO): the module goes through
    /// the pre-link pipeline (`lto-pre-link<O2>`) and the rest of the optimization
    /// happens with the whole program in view.
    Bitcode,
}

/// How [`gerar`] compiles a module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opcoes {
    /// `-O2` (`default<O2>` and the code generator at the default level) instead of
    /// `-O0`.
    pub otimizar: bool,
    pub formato: Formato,
    /// The target CPU (LLVM's name, like Clang's `-march`; `native` is this
    /// machine's, with its features); `None`, Clang's default (`default_cpu`).
    pub cpu: Option<&'static str>,
}

impl Opcoes {
    /// What goes into the key of a cached object: two equal modules with options of
    /// the same description give the same bytes.
    pub fn description(&self) -> String {
        let cpu = match self.cpu {
            None => String::new(),
            Some("native") => {
                let (name, resources) = machine_cpu();
                format!(" cpu=native({name};{resources})")
            }
            Some(c) => format!(" cpu={c}"),
        };
        format!(
            "{} {}{cpu}",
            if self.otimizar { "O2" } else { "O0" },
            match self.formato {
                Formato::Objeto => "objeto",
                Formato::Bitcode => "bitcode-lto",
            }
        )
    }
}

/// The name and the features of this machine's CPU, as LLVM detects them (Clang's
/// `-march=native`).
fn machine_cpu() -> (String, String) {
    // SAFETY: the returned strings are ours and freed by `take_message`.
    unsafe { (take_message(LLVMGetHostCPUName()), take_message(LLVMGetHostCPUFeatures())) }
}

/// The generator's identity for the cache keys: the LLVM version, the host's triple
/// and CPU. Another LLVM or another target machine does not reuse an object from
/// this one.
pub fn identidade() -> Result<String, String> {
    static ID: OnceLock<Result<String, String>> = OnceLock::new();
    ID.get_or_init(|| {
        init_native_target()?;
        let triple = default_triple();
        Ok(format!("llvm-embutido {} {} {} {}", version(), triple, default_cpu(&triple), generator_options_without_program().join(" ")))
    })
    .clone()
}

/// Compiles the textual LLVM IR `ir` (`name` appears in the messages and as the
/// module's identifier) and returns the bytes of the object or of the bitcode.
///
/// A module without a `target triple` (the emitter omits it on macOS, where the
/// triple carries the system version) gets the host's, as Clang would do; without a
/// `target datalayout`, the target machine's.
///
/// # Errors
/// IR the reader rejects, a target without a backend, a failure of the pipeline or
/// of the code generator — the message is LLVM's.
pub fn gerar(name: &str, ir: &str, options: &Opcoes) -> Result<Vec<u8>, String> {
    init_native_target()?;
    let contexto = Context::new();
    let modulo = contexto.do_read(name, ir)?;
    let triple = modulo.triple().unwrap_or_else(default_triple);
    // Roots by stack maps (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.8): a module
    // with `gc "statepoint-example"` functions goes through
    // `rewrite-statepoints-for-gc` — after the optimization, so that inlining and
    // the rest of the pipeline see ordinary calls — and through the verifier (the
    // pass has already produced invalid IR that only died in instruction
    // selection). The code generator of those functions is never the `-O0` one
    // (FastISel has known defects with `gc.relocate`).
    let with_maps = ir.contains(MARCA_DE_GC);
    let llvm_machine = TargetMachine::from_triple(&triple, options.otimizar || with_maps, options.cpu)?;
    modulo.complete_target(&triple, &llvm_machine, with_maps);
    // Production bitcode never goes through the maps pass here: it runs after all
    // the link-time optimization — on Windows and Linux at the end of the
    // distributed ThinLTO (`emit_native/src/lto_distribuida.rs`), on Mach-O in
    // `ld64.lld`'s LTO (`--lto-newpm-passes`, `ligador_macos.rs`)
    // (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §3.4). Rewritten earlier, the IR
    // would be optimized again in the LTO with the statepoints already in place,
    // and the optimizer does not know their roots: the executable loses roots.
    let pass_at_linker = options.formato == Formato::Bitcode;
    // `DARTFORGE_PIPELINE_OBJETO` replaces the pipeline of the optimized object
    // (time and size measurement: `default<O1>`, `default<Os>`;
    // docs/NATIVO-PRODUCAO-GRANDE.md).
    let chosen = std::env::var("DARTFORGE_PIPELINE_OBJETO").ok().filter(|p| !p.is_empty());
    let pipeline = match (options.formato, options.otimizar) {
        (Formato::Bitcode, _) => "lto-pre-link<O2>",
        (Formato::Objeto, true) => chosen.as_deref().unwrap_or("default<O2>"),
        (Formato::Objeto, false) => "default<O0>",
    };
    if with_maps && !pass_at_linker {
        modulo.otimizar(pipeline, &llvm_machine)?;
        modulo.remove_statics_from_deopt();
        modulo.otimizar("rewrite-statepoints-for-gc,verify", &llvm_machine)?;
        // The pass declares `@__tmp_use` for the provisional uses that it deletes
        // itself. The leftover declaration does not change an object, but in the
        // production bitcode (`lld-link`'s LTO) it enters the symbol table as
        // undefined, and linking fails.
        modulo.remove_unused_declaration("__tmp_use");
        check_after_rs4gc(name, &modulo)?;
    } else {
        modulo.otimizar(pipeline, &llvm_machine)?;
    }
    match options.formato {
        Formato::Objeto => {
            dump_before_generator(name, &modulo);
            llvm_machine.emit_object(&modulo).map_err(|e| dump_if_requested(name, "object", &modulo, e))
        }
        Formato::Bitcode => Ok(modulo.bitcode()),
    }
}

/// The value `v` is a fixed address of the image: a global, a constant expression,
/// or the conversion, the `getelementptr` with constant indices or the sum with a
/// constant of one of them; or the load of one of the canonical `true`/`false`
/// objects of the thread context (`getelementptr` over `dartforge_context()` at
/// 360 or 368, `layout::contexto`, directly or through the `select` of
/// `df.box_bool`), statics of the runtime image — the load is `!invariant.load`, and
/// the code generator rematerializes it as a constant
/// ([`IrModule::remove_statics_from_deopt`]).
///
/// # Safety
/// `v` is a live value of a module.
unsafe fn static_address(v: llvm_sys::prelude::LLVMValueRef, depth: u32) -> bool {
    // SAFETY: the function's contract; only reads.
    unsafe {
        if !LLVMIsAGlobalValue(v).is_null() || !LLVMIsAConstantExpr(v).is_null() {
            return true;
        }
        if depth > 4 || LLVMIsAInstruction(v).is_null() {
            return false;
        }
        let constant = |x| !LLVMIsAConstantInt(x).is_null();
        match LLVMGetInstructionOpcode(v) {
            LLVMOpcode::LLVMPtrToInt | LLVMOpcode::LLVMIntToPtr | LLVMOpcode::LLVMBitCast | LLVMOpcode::LLVMAddrSpaceCast => {
                static_address(LLVMGetOperand(v, 0), depth + 1)
            }
            LLVMOpcode::LLVMGetElementPtr => {
                (1..LLVMGetNumOperands(v) as u32).all(|k| constant(LLVMGetOperand(v, k)))
                    && static_address(LLVMGetOperand(v, 0), depth + 1)
            }
            LLVMOpcode::LLVMAdd => {
                let (a, b) = (LLVMGetOperand(v, 0), LLVMGetOperand(v, 1));
                (constant(b) && static_address(a, depth + 1)) || (constant(a) && static_address(b, depth + 1))
            }
            LLVMOpcode::LLVMLoad => canonical_load(LLVMGetOperand(v, 0)),
            _ => false,
        }
    }
}

/// The integer value of `v`, if it is constant: a `ConstantInt` or an integer
/// operation (add, sub, mul, shifts, `and`, `or`, `xor`) of constant operands, with
/// LLVM's 64-bit arithmetic ([`IrModule::remove_statics_from_deopt`]).
///
/// # Safety
/// `v` is a live value of a module.
unsafe fn constant_value(v: llvm_sys::prelude::LLVMValueRef, depth: u32) -> Option<u64> {
    // SAFETY: the function's contract; only reads.
    unsafe {
        if !LLVMIsAConstantInt(v).is_null() {
            return Some(LLVMConstIntGetZExtValue(v));
        }
        if depth > 6 || LLVMIsAInstruction(v).is_null() || LLVMGetNumOperands(v) != 2 {
            return None;
        }
        let a = constant_value(LLVMGetOperand(v, 0), depth + 1)?;
        let b = constant_value(LLVMGetOperand(v, 1), depth + 1)?;
        Some(match LLVMGetInstructionOpcode(v) {
            LLVMOpcode::LLVMAdd => a.wrapping_add(b),
            LLVMOpcode::LLVMSub => a.wrapping_sub(b),
            LLVMOpcode::LLVMMul => a.wrapping_mul(b),
            LLVMOpcode::LLVMShl if b < 64 => a << b,
            LLVMOpcode::LLVMLShr if b < 64 => a >> b,
            LLVMOpcode::LLVMAShr if b < 64 => ((a as i64) >> b) as u64,
            LLVMOpcode::LLVMAnd => a & b,
            LLVMOpcode::LLVMOr => a | b,
            LLVMOpcode::LLVMXor => a ^ b,
            _ => return None,
        })
    }
}

/// The pointer `p` is the field of the canonical `true` or `false` in the context:
/// `getelementptr` over the call to `dartforge_context` with offset 360 or 368,
/// constant or chosen by a `select` between the two.
///
/// # Safety
/// `p` is a live value of a module.
unsafe fn canonical_load(p: llvm_sys::prelude::LLVMValueRef) -> bool {
    const TRUE: u64 = 360;
    const FALSE: u64 = 368;
    // SAFETY: the function's contract; only reads.
    unsafe {
        if LLVMIsAInstruction(p).is_null() || LLVMGetInstructionOpcode(p) != LLVMOpcode::LLVMGetElementPtr || LLVMGetNumOperands(p) != 2 {
            return false;
        }
        let base = LLVMGetOperand(p, 0);
        if LLVMIsACallInst(base).is_null() {
            return false;
        }
        let callee = LLVMGetCalledValue(base);
        let mut n = 0usize;
        let name = LLVMGetValueName2(callee, &mut n);
        if name.is_null() || std::slice::from_raw_parts(name.cast::<u8>(), n) != b"dartforge_context" {
            return false;
        }
        let canonical = |x| !LLVMIsAConstantInt(x).is_null() && matches!(LLVMConstIntGetZExtValue(x), TRUE | FALSE);
        let ix = LLVMGetOperand(p, 1);
        if canonical(ix) {
            return true;
        }
        !LLVMIsAInstruction(ix).is_null()
            && LLVMGetInstructionOpcode(ix) == LLVMOpcode::LLVMSelect
            && canonical(LLVMGetOperand(ix, 1))
            && canonical(LLVMGetOperand(ix, 2))
    }
}

/// The attribute of the functions with roots in the stack map, as the emitter
/// writes it (`emit_native/src/llvm/mod.rs`).
pub const MARCA_DE_GC: &str = "gc \"statepoint-example\"";


/// The native object of one part of the **distributed ThinLTO**
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §3.4 and Stage 3): `bitcode` is the
/// module **after** ThinLTO's import and backend optimization
/// (`clang -x ir parte.o -fthinlto-index=… -emit-llvm -c`). Only what `lld-link`
/// cannot run comes in here: `rewrite-statepoints-for-gc`, at the end of all the
/// optimization — the inlining between the program and the SDK has already
/// happened —, the verifier and code generation (never the `-O0` one).
///
/// Returns the object and whether the module had functions with roots in the map
/// (the object then carries `.llvm_stackmaps`, which the caller converts). The
/// question is asked of the parsed module: in the bitcode the name of the
/// collection strategy may be encoded, and looking for it in the bytes fails
/// (Clang's).
///
/// # Errors
/// Bitcode the reader rejects, a failure of the pass, of the verifier or of the
/// code generator — the message is LLVM's.
pub fn gerar_de_bitcode(name: &str, bitcode: &[u8], cpu: Option<&'static str>) -> Result<(Vec<u8>, bool), String> {
    init_native_target()?;
    let contexto = Context::new();
    let modulo = contexto.read_bytes(name, bitcode)?;
    let triple = modulo.triple().unwrap_or_else(default_triple);
    let llvm_machine = TargetMachine::from_triple(&triple, true, cpu)?;
    let with_maps = modulo.has_gc_function();
    modulo.complete_target(&triple, &llvm_machine, with_maps);
    if with_maps {
        modulo.remove_statics_from_deopt();
        modulo.otimizar("rewrite-statepoints-for-gc,verify", &llvm_machine)?;
        modulo.remove_unused_declaration("__tmp_use");
        check_after_rs4gc(name, &modulo)?;
    } else {
        modulo.otimizar("verify", &llvm_machine)?;
    }
    dump_before_generator(name, &modulo);
    Ok((llvm_machine.emit_object(&modulo).map_err(|e| dump_if_requested(name, "object", &modulo, e))?, with_maps))
}

/// `DARTFORGE_CONFERIR_RS4GC=1`: the root checker after
/// `rewrite-statepoints-for-gc` ([`check_rs4gc`],
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.4) on the module text. Expensive
/// (prints the whole module): enabled in the maps-mode tests.
fn check_after_rs4gc(name: &str, modulo: &IrModule<'_>) -> Result<(), String> {
    if !std::env::var("DARTFORGE_CONFERIR_RS4GC").is_ok_and(|v| v == "1") {
        return Ok(());
    }
    check_rs4gc::check(&modulo.text()).map_err(|e| dump_if_requested(name, "rs4gc", modulo, format!("{name}: {e}")))
}

/// `DARTFORGE_DESPEJAR_MODULO=<trecho>`: the module whose name contains the chunk
/// goes to the temporary directory right before the code generator
/// (`gerador-<nome>.ll`) — for the failure where LLVM brings the process down after
/// printing the error ("ran out of registers").
fn dump_before_generator(name: &str, modulo: &IrModule<'_>) {
    if let Ok(chunk) = std::env::var("DARTFORGE_DESPEJAR_MODULO")
        && !chunk.is_empty()
        && name.contains(&chunk)
    {
        let file_name: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
        let dest = std::env::temp_dir().join(format!("gerador-{file_name}.ll"));
        let _ = std::fs::write(&dest, modulo.text());
        eprintln!("dartforge: the module before the code generator was saved at {}", dest.display());
    }
}

/// With `DARTFORGE_KEEP_IR`, the module of a failure (of the checker, of the code
/// generator) goes to the temporary directory, `<etapa>-<nome>.ll`, for diagnosis.
/// Returns the error.
fn dump_if_requested(name: &str, stage: &str, modulo: &IrModule<'_>, error: String) -> String {
    if std::env::var_os("DARTFORGE_KEEP_IR").is_some() {
        let file_name: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
        let dest = std::env::temp_dir().join(format!("{stage}-{file_name}.ll"));
        let _ = std::fs::write(&dest, modulo.text());
        eprintln!("dartforge: the failing module was saved at {}", dest.display());
    }
    error
}

/// The host's triple, as LLVM describes it (on macOS, with the version of the
/// running system).
fn default_triple() -> String {
    // SAFETY: the returned string is ours and freed by `take_message`.
    unsafe { take_message(LLVMGetDefaultTargetTriple()) }
}

/// The CPU that Clang assumes without `-march` for the triple: `x86-64` on x86-64,
/// `apple-m1` on Apple's arm64, `generic` on the others.
fn default_cpu(triple: &str) -> &'static str {
    let architecture = triple.split('-').next().unwrap_or_default();
    let apple = triple.contains("-apple-");
    match architecture {
        "x86_64" => "x86-64",
        "arm64" | "aarch64" if apple => "apple-m1",
        _ => "generic",
    }
}

/// Copies an LLVM `char*` message and frees the original.
///
/// # Safety
/// `message` is non-null and came from an API that transfers ownership to the caller.
unsafe fn take_message(message: *mut c_char) -> String {
    // SAFETY: guaranteed by the caller.
    unsafe {
        let text = CStr::from_ptr(message).to_string_lossy().into_owned();
        LLVMDisposeMessage(message);
        text
    }
}

/// Consumes an `LLVMErrorRef` and returns the message (`None` without an error).
fn take_error(error: LLVMErrorRef) -> Option<String> {
    if error.is_null() {
        return None;
    }
    // SAFETY: `error` is non-null and was not consumed; the returned message is ours
    // and goes out with `LLVMDisposeErrorMessage` (`llvm-c/Error.h`).
    unsafe {
        let message = LLVMGetErrorMessage(error);
        let text = CStr::from_ptr(message).to_string_lossy().into_owned();
        LLVMDisposeErrorMessage(message);
        Some(text)
    }
}

/// Copies and frees a `MemoryBuffer`.
///
/// # Safety
/// `buffer` is non-null and ours.
unsafe fn take_buffer(buffer: LLVMMemoryBufferRef) -> Vec<u8> {
    // SAFETY: guaranteed by the caller; start and size describe the buffer alive
    // until `Dispose`.
    unsafe {
        let start = LLVMGetBufferStart(buffer).cast::<u8>();
        let bytes = std::slice::from_raw_parts(start, LLVMGetBufferSize(buffer)).to_vec();
        LLVMDisposeMemoryBuffer(buffer);
        bytes
    }
}

/// An `LLVMContext` of our own: each generation has its own, and generations on
/// different threads do not share LLVM state.
struct Context(LLVMContextRef);

impl Context {
    fn new() -> Self {
        // SAFETY: creates a new context, freed in `Drop`.
        Context(unsafe { LLVMContextCreate() })
    }

    /// Parses textual IR. The returned module lives inside this context (the
    /// `IrModule` borrows `self`).
    fn do_read(&self, name: &str, ir: &str) -> Result<IrModule<'_>, String> {
        self.read_bytes(name, ir.as_bytes())
    }

    /// Parses textual IR or bitcode (`LLVMParseIRInContext2` recognizes both by the
    /// start of the buffer).
    fn read_bytes(&self, name: &str, ir: &[u8]) -> Result<IrModule<'_>, String> {
        let c_name = CString::new(name).map_err(|_| format!("module name with NUL: {name:?}"))?;
        // SAFETY: the buffer copies the IR bytes; `LLVMParseIRInContext2` does not
        // consume it, and it is freed right after. The created module is ours and
        // goes to the `IrModule`; the error message, if any, is ours.
        unsafe {
            let buffer = LLVMCreateMemoryBufferWithMemoryRangeCopy(ir.as_ptr().cast(), ir.len(), c_name.as_ptr());
            let mut modulo = ptr::null_mut();
            let mut message = ptr::null_mut();
            let has_failed = LLVMParseIRInContext2(self.0, buffer, &mut modulo, &mut message) != 0;
            LLVMDisposeMemoryBuffer(buffer);
            if has_failed {
                let detail = if message.is_null() { "invalid IR".to_owned() } else { take_message(message) };
                return Err(format!("LLVM rejected the IR of {name}: {detail}"));
            }
            Ok(IrModule { m: modulo, _context: self })
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        // SAFETY: the modules of this context borrow `self` and have already been
        // freed; the context is freed only once.
        unsafe { LLVMContextDispose(self.0) }
    }
}

/// A parsed module, owner of the `LLVMModuleRef`.
struct IrModule<'c> {
    m: LLVMModuleRef,
    _context: &'c Context,
}

impl IrModule<'_> {
    /// The module's `target triple`, if declared.
    fn triple(&self) -> Option<String> {
        // SAFETY: the string belongs to the module, which is alive; it is copied.
        let t = unsafe { CStr::from_ptr(LLVMGetTarget(self.m)) }.to_string_lossy().into_owned();
        (!t.is_empty()).then_some(t)
    }

    /// Completes the triple and the data layout that the module does not declare
    /// (the emitter omits them on macOS and on Linux aarch64). With roots by maps
    /// (`with_maps`), the data layout carries `-ni:1`: the roots' `addrspace(1)` is
    /// one of non-integral pointers, and the optimizer does not fabricate
    /// `ptrtoint`/`inttoptr` with them (§14.8).
    fn complete_target(&self, triple: &str, llvm_machine: &TargetMachine, with_maps: bool) {
        // SAFETY: the module and the machine are alive; the created data layout is
        // copied into the module and freed; the triple and the data layout text are
        // copied by LLVM.
        unsafe {
            if self.triple().is_none()
                && let Ok(t) = CString::new(triple)
            {
                LLVMSetTarget(self.m, t.as_ptr());
            }
            if CStr::from_ptr(LLVMGetDataLayoutStr(self.m)).to_bytes().is_empty() {
                let data = LLVMCreateTargetDataLayout(llvm_machine.0);
                LLVMSetModuleDataLayout(self.m, data);
                LLVMDisposeTargetData(data);
            }
            if with_maps {
                let current = CStr::from_ptr(LLVMGetDataLayoutStr(self.m)).to_string_lossy().into_owned();
                if !current.split('-').any(|p| p.starts_with("ni:"))
                    && let Ok(new) = CString::new(format!("{current}-ni:1"))
                {
                    LLVMSetDataLayout(self.m, new.as_ptr());
                }
            }
        }
    }

    /// Runs the `PassBuilder`'s textual pipeline (`default<O2>`…) with the target
    /// machine (the target's costs and legality, as in Clang).
    fn otimizar(&self, pipeline: &str, llvm_machine: &TargetMachine) -> Result<(), String> {
        let text = CString::new(pipeline).expect("pipeline without NUL");
        // SAFETY: module and machine alive; the options are created and freed here;
        // the returned error is consumed by `take_error`.
        let error = unsafe {
            let options = LLVMCreatePassBuilderOptions();
            let error = LLVMRunPasses(self.m, text.as_ptr(), llvm_machine.0, options);
            LLVMDisposePassBuilderOptions(options);
            error
        };
        match take_error(error) {
            Some(e) => Err(format!("the LLVM pipeline {pipeline} failed: {e}")),
            None => Ok(()),
        }
    }

    /// Some function of the module has a collection strategy (`gc "…"`).
    fn has_gc_function(&self) -> bool {
        // SAFETY: module alive; the function list is walked without changes.
        unsafe {
            let mut f = LLVMGetFirstFunction(self.m);
            while !f.is_null() {
                if !LLVMGetGC(f).is_null() {
                    return true;
                }
                f = LLVMGetNextFunction(f);
            }
        }
        false
    }

    /// Roots by maps (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.12): a `"deopt"`
    /// operand that optimization turned into a static address — the handle of an
    /// image object (`ptrtoint (gep @df.s…, 2)`), which arrives through the
    /// parameter of a function copied by the inliner — becomes null. The image
    /// object is never collected and needs no root; and, with the roots in
    /// registers, the constant would have to be materialized in a register at the
    /// call, unable to go to the stack (the allocator rematerializes it instead of
    /// spilling): with several, the code generator fails with "ran out of
    /// registers". For the same reason, an operand that is an instruction whose
    /// operands are all constant (the unoptimized development IR: the `Smi` of a
    /// literal, `or (shl 3, 1), 1`) becomes the folded constant, which goes into the
    /// map as a `Constant`, without a register. Runs after the optimization and
    /// before `rewrite-statepoints-for-gc`. Returns how many operands it replaced.
    fn remove_statics_from_deopt(&self) -> usize {
        let mut swapped = 0;
        // SAFETY: module alive; the instructions are walked without changing the
        // list, and only a bundle operand is replaced by a value of the same type.
        // The obtained bundle is freed right after being read.
        unsafe {
            let mut f = LLVMGetFirstFunction(self.m);
            while !f.is_null() {
                let mut b = LLVMGetFirstBasicBlock(f);
                while !b.is_null() {
                    let mut i = LLVMGetFirstInstruction(b);
                    while !i.is_null() {
                        if !LLVMIsACallInst(i).is_null() || !LLVMIsAInvokeInst(i).is_null() {
                            let mut pos = LLVMGetNumArgOperands(i);
                            for k in 0..LLVMGetNumOperandBundles(i) {
                                let ob = LLVMGetOperandBundleAtIndex(i, k);
                                let mut n = 0usize;
                                let tag = LLVMGetOperandBundleTag(ob, &mut n);
                                let is_deopt = std::slice::from_raw_parts(tag.cast::<u8>(), n) == b"deopt";
                                let args = LLVMGetNumOperandBundleArgs(ob);
                                LLVMDisposeOperandBundle(ob);
                                if is_deopt {
                                    for j in pos..pos + args {
                                        let v = LLVMGetOperand(i, j);
                                        if static_address(v, 0) {
                                            LLVMSetOperand(i, j, LLVMConstNull(LLVMTypeOf(v)));
                                            swapped += 1;
                                        } else if !LLVMIsAInstruction(v).is_null()
                                            && let Some(c) = constant_value(v, 0)
                                        {
                                            LLVMSetOperand(i, j, LLVMConstInt(LLVMTypeOf(v), c, 0));
                                            swapped += 1;
                                        }
                                    }
                                }
                                pos += args;
                            }
                        }
                        i = LLVMGetNextInstruction(i);
                    }
                    b = LLVMGetNextBasicBlock(b);
                }
                f = LLVMGetNextFunction(f);
            }
        }
        swapped
    }

    /// Deletes the declaration `name`, if it exists and has no uses.
    fn remove_unused_declaration(&self, name: &str) {
        let Ok(c) = CString::new(name) else { return };
        // SAFETY: module alive; the function is only deleted without uses.
        unsafe {
            let f = LLVMGetNamedFunction(self.m, c.as_ptr());
            if !f.is_null() && LLVMGetFirstUse(f).is_null() {
                LLVMDeleteFunction(f);
            }
        }
    }

    fn bitcode(&self) -> Vec<u8> {
        // SAFETY: the created buffer is ours and goes out through `take_buffer`.
        unsafe { take_buffer(LLVMWriteBitcodeToMemoryBuffer(self.m)) }
    }

    /// The module's text (the IR as LLVM prints it).
    fn text(&self) -> String {
        // SAFETY: module alive; the returned message is ours and freed by
        // `take_message`.
        unsafe { take_message(LLVMPrintModuleToString(self.m)) }
    }
}

impl Drop for IrModule<'_> {
    fn drop(&mut self) {
        // SAFETY: the module is ours and freed only once, before the context (the
        // borrow guarantees the order).
        unsafe { LLVMDisposeModule(self.m) }
    }
}

/// The target machine of a generation.
struct TargetMachine(LLVMTargetMachineRef);

impl TargetMachine {
    /// The machine of `triple` with the requested CPU, or Clang's default
    /// ([`default_cpu`]). Position-independent code outside Windows: Linux
    /// executables are PIE (Clang's default) and the SDK's shared libraries require
    /// it; Mach-O is always PIC.
    fn from_triple(triple: &str, otimizar: bool, cpu: Option<&str>) -> Result<Self, String> {
        let triple_c = CString::new(triple).map_err(|_| format!("triple with NUL: {triple:?}"))?;
        // `native`: the detected name and features (what Clang does with
        // `-march=native`); another name, its implicit features.
        let (cpu, resources) = match cpu {
            Some("native") => machine_cpu(),
            Some(c) => (c.to_owned(), String::new()),
            None => (default_cpu(triple).to_owned(), String::new()),
        };
        let cpu = CString::new(cpu).map_err(|_| "CPU with NUL".to_owned())?;
        let resources = CString::new(resources).map_err(|_| "features with NUL".to_owned())?;
        let windows = triple.contains("windows");
        // SAFETY: the returned target is a static reference of LLVM's registry; the
        // error message is ours; the created machine goes to the `TargetMachine`,
        // which frees it.
        unsafe {
            let mut target = ptr::null_mut();
            let mut message = ptr::null_mut();
            if LLVMGetTargetFromTriple(triple_c.as_ptr(), &mut target, &mut message) != 0 {
                let detail = if message.is_null() { "unknown target".to_owned() } else { take_message(message) };
                return Err(format!("LLVM does not have the target {triple}: {detail}"));
            }
            let llvm_machine = LLVMCreateTargetMachine(
                target,
                triple_c.as_ptr(),
                cpu.as_ptr(),
                resources.as_ptr(),
                if otimizar { LLVMCodeGenOptLevel::LLVMCodeGenLevelDefault } else { LLVMCodeGenOptLevel::LLVMCodeGenLevelNone },
                if windows { LLVMRelocMode::LLVMRelocDefault } else { LLVMRelocMode::LLVMRelocPIC },
                LLVMCodeModel::LLVMCodeModelDefault,
            );
            if llvm_machine.is_null() {
                return Err(format!("LLVM did not create the target machine for {triple}"));
            }
            Ok(TargetMachine(llvm_machine))
        }
    }

    fn emit_object(&self, modulo: &IrModule<'_>) -> Result<Vec<u8>, String> {
        // SAFETY: machine and module alive; the returned error message and buffer
        // are ours and freed.
        unsafe {
            let mut message = ptr::null_mut();
            let mut buffer = ptr::null_mut();
            let has_failed = LLVMTargetMachineEmitToMemoryBuffer(
                self.0,
                modulo.m,
                LLVMCodeGenFileType::LLVMObjectFile,
                &mut message,
                &mut buffer,
            ) != 0;
            if has_failed {
                return Err(if message.is_null() { "the LLVM code generator failed".to_owned() } else { take_message(message) });
            }
            Ok(take_buffer(buffer))
        }
    }
}

impl Drop for TargetMachine {
    fn drop(&mut self) {
        // SAFETY: the machine is ours and freed only once.
        unsafe { LLVMDisposeTargetMachine(self.0) }
    }
}

#[cfg(test)]
mod tests_mod {
    use super::*;

    const IR: &str = "define i32 @soma(i32 %a, i32 %b) {\n  %s = add i32 %a, %b\n  ret i32 %s\n}\n";

    #[test]
    fn produces_object_and_bitcode() {
        for otimizar in [false, true] {
            let obj = gerar("soma", IR, &Opcoes { otimizar, formato: Formato::Objeto, cpu: None }).unwrap();
            let magic = &obj[..4];
            // ELF, Mach-O (64 bits) or COFF x86-64/arm64.
            assert!(
                magic == b"\x7fELF" || magic == [0xcf, 0xfa, 0xed, 0xfe] || obj[..2] == [0x64, 0x86] || obj[..2] == [0x64, 0xaa],
                "unexpected header: {magic:x?}"
            );
        }
        let bc = gerar("soma", IR, &Opcoes { otimizar: true, formato: Formato::Bitcode, cpu: None }).unwrap();
        assert_eq!(&bc[..4], b"BC\xc0\xde");
    }

    #[test]
    fn same_ir_same_bytes() {
        let op = Opcoes { otimizar: true, formato: Formato::Objeto, cpu: None };
        assert_eq!(gerar("a", IR, &op).unwrap(), gerar("a", IR, &op).unwrap());
    }

    #[test]
    fn invalid_ir_gives_error_with_llvm_message() {
        let e = gerar("ruim", "define i32 @f( {", &Opcoes { otimizar: false, formato: Formato::Objeto, cpu: None }).unwrap_err();
        assert!(e.contains("ruim"), "{e}");
    }

    #[test]
    fn cpu_do_clang() {
        assert_eq!(default_cpu("x86_64-pc-windows-msvc"), "x86-64");
        assert_eq!(default_cpu("arm64-apple-darwin24.1.0"), "apple-m1");
        assert_eq!(default_cpu("aarch64-unknown-linux-gnu"), "generic");
    }
}
