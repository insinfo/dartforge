//! The crate's single `unsafe` boundary: the LLVM C API (ORCv2) and the adapters
//! that expose the native runtime to the in-memory generated code.
//!
//! # Why `unsafe` is unavoidable here
//!
//! There is no safe JIT compiler for this job. Three operations are
//! unsafe by nature and no abstraction removes them:
//!
//! 1. **The LLVM C API.** `llvm-sys` publishes the ORCv2 `extern "C"` signatures
//!    as they are in `llvm-c/Orc.h` and `llvm-c/LLJIT.h`. Every call is
//!    `unsafe`, and resource ownership (context, module, tracker) follows the
//!    contract of that documentation, not the borrow checker.
//! 2. **Publishing addresses of Rust functions as symbols.** JIT code calls
//!    `@dartforge_print_i64` and company by absolute address. Registering those
//!    addresses requires turning them into integers and handing them to LLVM.
//! 3. **Calling the generated code.** The address returned by `LLVMOrcLLJITLookup`
//!    is a `u64`; invoking it requires a `transmute` to a function pointer.
//!
//! # Invariants this module preserves
//!
//! * Each LLVM resource belongs to exactly one Rust value with a matching
//!   `Drop` ([`Lljit`], [`ResourceTracker`], [`ThreadSafeModule`]). What
//!   the C API consumes is handed over by value and never freed twice.
//! * Raw handles never cross the crate boundary: [`crate::JitSession`]
//!   exposes only `Result`, `Duration` and `String`.
//! * The types here contain raw pointers, so they are neither `Send` nor `Sync`.
//!   The runtime state is `thread_local`; the program runs on its own
//!   thread per execution ([`execute_program`]).
//! * The runtime published to the JIT **is not rewritten here**: it is the source of the AOT
//!   harness (the fragments of `crates/runtime/src`), compiled as the module
//!   `dartforge_runtime::abi`; the symbol table is generated from the same source
//!   by that crate's `build.rs` (single source).
//!
//! This module is **not** a safety checker for arbitrary IR, for the
//! same reason the AOT driver is not: the guarantee holds for IR emitted by the
//! compiler itself (`crates/emit_native`), with the signatures it declares.
//
// The crate's only `allow`. `src/lib.rs` and `src/reload.rs` stay under the
// workspace's `unsafe_code = "deny"`, so any new `unsafe` outside here
// shows up as a compilation error, not as a detail hidden in a review.
#![allow(unsafe_code)]

use llvm_sys::core::{
    LLVMAddFunction, LLVMConstIntGetSExtValue, LLVMContextCreate, LLVMContextDispose,
    LLVMCountParamTypes, LLVMCreateMemoryBufferWithMemoryRangeCopy, LLVMDisposeMemoryBuffer,
    LLVMDisposeMessage, LLVMDisposeModule, LLVMGetCalledValue, LLVMGetFirstBasicBlock,
    LLVMGetFirstFunction, LLVMGetFirstInstruction, LLVMGetNextBasicBlock, LLVMGetNextFunction,
    LLVMGetNextInstruction, LLVMGetNumOperands, LLVMGetOperand, LLVMGetParamTypes,
    LLVMGetReturnType, LLVMGetValueName2, LLVMGlobalGetValueType, LLVMIsACallInst,
    LLVMIsAConstantInt, LLVMIsDeclaration, LLVMIsFunctionVarArg, LLVMPrintTypeToString,
    LLVMReplaceAllUsesWith, LLVMSetValueName2, LLVMGetInitializer, LLVMIsAGlobalVariable,
    LLVMIsAConstantDataSequential, LLVMGetAsString,
};
use llvm_sys::error::{LLVMDisposeErrorMessage, LLVMErrorRef, LLVMGetErrorMessage};
use llvm_sys::ir_reader::LLVMParseIRInContext2;
use llvm_sys::orc2::lljit::{
    LLVMOrcCreateLLJIT, LLVMOrcCreateLLJITBuilder, LLVMOrcDisposeLLJIT,
    LLVMOrcLLJITAddLLVMIRModuleWithRT,
    LLVMOrcLLJITBuilderSetJITTargetMachineBuilder, LLVMOrcLLJITGetDataLayoutStr,
    LLVMOrcLLJITGetMainJITDylib, LLVMOrcLLJITGetTripleString, LLVMOrcLLJITLookup,
    LLVMOrcLLJITMangleAndIntern, LLVMOrcLLJITRef,
};
use llvm_sys::orc2::{
    LLVMJITEvaluatedSymbol, LLVMJITSymbolFlags, LLVMJITSymbolGenericFlags, LLVMOrcAbsoluteSymbols,
    LLVMOrcCSymbolMapPair, LLVMOrcDisposeMaterializationUnit, LLVMOrcDisposeThreadSafeContext,
    LLVMOrcDisposeThreadSafeModule, LLVMOrcJITDylibCreateResourceTracker, LLVMOrcJITDylibDefine,
    LLVMOrcJITDylibRef, LLVMOrcJITTargetMachineBuilderCreateFromTargetMachine,
    LLVMOrcReleaseResourceTracker, LLVMOrcResourceTrackerRef, LLVMOrcResourceTrackerRemove,
    LLVMOrcThreadSafeModuleRef,
};
use llvm_sys::prelude::{LLVMContextRef, LLVMModuleRef, LLVMTypeRef, LLVMValueRef};
use llvm_sys::target_machine::{
    LLVMCodeGenFileType, LLVMCodeGenOptLevel, LLVMCodeModel, LLVMCreateTargetMachine,
    LLVMDisposeTargetMachine, LLVMGetDefaultTargetTriple, LLVMGetTargetFromTriple, LLVMRelocMode,
    LLVMTargetMachineEmitToMemoryBuffer, LLVMTargetMachineRef,
};
use std::ffi::{CStr, CString, c_char};
use std::mem::ManuallyDrop;
use std::ptr;
use std::time::{Duration, Instant};

/// Converts an `LLVMErrorRef` into `Option<String>`, consuming the error.
///
/// `LLVMGetErrorMessage` consumes the `LLVMErrorRef` and transfers ownership of the
/// message; returning it requires copying the bytes and freeing the original.
fn take_error(error: LLVMErrorRef) -> Option<String> {
    if error.is_null() {
        return None;
    }
    // SAFETY: `error` is non-null and not yet consumed, so
    // `LLVMGetErrorMessage` can consume it and return a valid C string, whose
    // ownership passes to us. `llvm-c/Error.h` requires freeing it with
    // `LLVMDisposeErrorMessage`, and not with the core's `LLVMDisposeMessage`.
    unsafe {
        let message = LLVMGetErrorMessage(error);
        let text = CStr::from_ptr(message).to_string_lossy().into_owned();
        LLVMDisposeErrorMessage(message);
        Some(text)
    }
}

/// Copies a `char*` message from LLVM and frees the original.
///
/// # Safety
/// `message` must be non-null and come from an LLVM API that transfers
/// ownership of the string to the caller.
unsafe fn take_message(message: *mut c_char) -> String {
    // SAFETY: the caller guarantees the pointer is valid and ours to free.
    unsafe {
        let text = CStr::from_ptr(message).to_string_lossy().into_owned();
        LLVMDisposeMessage(message);
        text
    }
}

/// Initializes the native target once per process — the same
/// initialization as the AOT object generator (`dartforge-llvm`), which may
/// run in the same process (`dartforge run` compiles the source SDK).
///
/// # Errors
/// Fails when the linked LLVM does not include the backend for the host architecture.
pub(crate) fn initialize_native_target() -> Result<(), String> {
    dartforge_llvm::init_native_target()
}

/// An LLVM module already parsed and ready to go into a `JITDylib`.
///
/// The LLVM context that hosts the module lives inside the `ThreadSafeContext`
/// created during parsing; there is no context shared between modules.
pub(crate) struct ThreadSafeModule {
    handle: LLVMOrcThreadSafeModuleRef,
}
impl Drop for ThreadSafeModule {
    /// Frees the module when it was never handed to `LLJIT`.
    fn drop(&mut self) {
        // SAFETY: `handle` was created by `LLVMOrcCreateNewThreadSafeModule`,
        // was not yet consumed (consumption uses `ManuallyDrop`) and is freed
        // only once, since `ThreadSafeModule` is neither `Copy` nor `Clone`.
        unsafe { LLVMOrcDisposeThreadSafeModule(self.handle) }
    }
}

/// An LLVM module already parsed, still **outside** the session.
///
/// It exists to give an inspection and rewrite point between parsing the IR and
/// entering the `LLJIT`: this is where hot reload reads the contract
/// fingerprint ([`ParsedModule::signatures`], [`ParsedModule::class_layouts`]) and
/// versions the implementations ([`ParsedModule::version_definitions`]) before
/// any effect on the running code.
///
/// The context and the module belong to this value until [`ParsedModule::into_thread_safe`];
/// after that they belong to the returned `ThreadSafeModule`.
pub(crate) struct ParsedModule {
    context: LLVMContextRef,
    module: LLVMModuleRef,
}

impl Drop for ParsedModule {
    /// Frees module and context when the module never reached `LLJIT`.
    ///
    /// The order matters: the module belongs to the context, so it is freed first.
    /// `into_thread_safe` uses `ManuallyDrop` to not go through here.
    fn drop(&mut self) {
        // SAFETY: both pointers come from `parse_module`, which only builds this
        // value after a successful parse, and neither was consumed
        // (consumption in `into_thread_safe` suppresses this `Drop`).
        unsafe {
            LLVMDisposeModule(self.module);
            LLVMContextDispose(self.context);
        }
    }
}

/// Signature of a module function, in LLVM IR text.
///
/// It is the **calling contract fingerprint** of a function: what must
/// stay the same between generations for the implementation swap to be safe.
/// The body hash is deliberately left out — the body is exactly what
/// hot reload swaps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FunctionSignature {
    /// Symbol name as the emitter produced it (`df_fn_0`, `dartforge_entry`).
    pub(crate) name: String,
    /// Return type in IR text (`i64`, `void`).
    pub(crate) ret: String,
    /// Parameter types in IR text, in call order.
    pub(crate) params: Vec<String>,
    /// Variadic function; outside the native emitter's contract.
    pub(crate) var_arg: bool,
}

impl FunctionSignature {
    /// Shows the signature as LLVM would write it: `i64 (i64, i64)`.
    pub(crate) fn text(&self) -> String {
        format!("{} ({})", self.ret, self.params.join(", "))
    }
    /// Parameter list named `%a0, %a1, ...`, to generate the trampoline.
    pub(crate) fn declaration_params(&self) -> String {
        self.params
            .iter()
            .enumerate()
            .map(|(index, ty)| format!("{ty} %a{index}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Parses textual IR and returns the module open for inspection.
///
/// Uses `LLVMParseIRInContext2`, which does not consume the `MemoryBuffer`; the copy of the
/// text belongs to the buffer and is freed before returning on every path.
///
/// # Errors
/// Returns LLVM's own diagnostic when the IR is invalid, and refuses names
/// or texts with a null byte, which do not survive the C boundary.
pub(crate) fn parse_module(name: &str, ir: &str) -> Result<ParsedModule, String> {
    let name = CString::new(name).map_err(|_| "module name contains a null byte".to_owned())?;
    if ir.as_bytes().contains(&0) {
        return Err("the IR contains a null byte and cannot be parsed".to_owned());
    }
    // SAFETY: the whole block keeps the ownership of each resource explicit.
    // `buffer` is a copy of the text and is freed on all paths;
    // `context` is handed to the `ThreadSafeContext`, which is freed right after
    // `LLVMOrcCreateNewThreadSafeModule` keeps its own reference; `module`
    // is only consumed by that same call, and only when parsing succeeded.
    unsafe {
        let context = LLVMContextCreate();
        let buffer = LLVMCreateMemoryBufferWithMemoryRangeCopy(
            ir.as_ptr().cast::<c_char>(),
            ir.len(),
            name.as_ptr(),
        );
        let mut module = ptr::null_mut();
        let mut message = ptr::null_mut();
        let failed = LLVMParseIRInContext2(context, buffer, &mut module, &mut message) != 0;
        LLVMDisposeMemoryBuffer(buffer);
        if failed {
            let detail = if message.is_null() {
                "invalid IR".to_owned()
            } else {
                take_message(message)
            };
            LLVMContextDispose(context);
            return Err(detail);
        }
        if !message.is_null() {
            // Non-fatal diagnostic; the module is valid and the message is freed.
            drop(take_message(message));
        }
        // The textual IR parser does not run the LLVM verifier, and ORC
        // does not either. Clang does, and refuses the module. Without this call, the JIT
        // generated code for IR the AOT refuses, like an instruction that does not
        // dominate all its uses, and the program ran or crashed in the JIT and did not
        // compile in the AOT. The whole corpus in CI showed 13 such programs
        // (pesado.yml, job `jit`). Accepting the same IR the AOT accepts is part
        // of the contract.
        let mut detail_msg = ptr::null_mut();
        let invalid_one = llvm_sys::analysis::LLVMVerifyModule(
            module,
            llvm_sys::analysis::LLVMVerifierFailureAction::LLVMReturnStatusAction,
            &mut detail_msg,
        ) != 0;
        let text = if detail_msg.is_null() { String::new() } else { take_message(detail_msg) };
        if invalid_one {
            LLVMDisposeModule(module);
            LLVMContextDispose(context);
            let first_one = text.lines().next().unwrap_or("invalid module").to_owned();
            return Err(format!("the LLVM verifier refused the module: {first_one}"));
        }
        Ok(ParsedModule { context, module })
    }
}

/// Parses textual IR and returns a module ready to go into a `JITDylib`.
///
/// # Errors
/// The same as [`parse_module`].
pub(crate) fn parse_ir(name: &str, ir: &str) -> Result<ThreadSafeModule, String> {
    Ok(parse_module(name, ir)?.into_thread_safe())
}

impl ParsedModule {
    /// Packs the module into a `ThreadSafeModule`, transferring ownership.
    pub(crate) fn into_thread_safe(self) -> ThreadSafeModule {
        use llvm_sys::orc2::{
            LLVMOrcCreateNewThreadSafeContextFromLLVMContext, LLVMOrcCreateNewThreadSafeModule,
        };
        // The `ThreadSafeContext` takes over the context and the `ThreadSafeModule` takes over the
        // module; neither of them can be freed by `Drop` after this point.
        let this = ManuallyDrop::new(self);
        // SAFETY: `context` and `module` are valid and not yet consumed.
        // `LLVMOrcCreateNewThreadSafeContextFromLLVMContext` becomes the owner
        // of the context; `LLVMOrcCreateNewThreadSafeModule` keeps its own
        // reference to the safe context, so ours can be returned right
        // after, as the non-reloadable path already did.
        unsafe {
            let thread_safe_context =
                LLVMOrcCreateNewThreadSafeContextFromLLVMContext(this.context);
            let handle = LLVMOrcCreateNewThreadSafeModule(this.module, thread_safe_context);
            LLVMOrcDisposeThreadSafeContext(thread_safe_context);
            ThreadSafeModule { handle }
        }
    }

    /// Signatures of the functions **exported** by the module, in IR order.
    ///
    /// External declarations are left out: they are what the module consumes (the
    /// runtime), not what it offers to be reloaded. `internal`/`private`
    /// definitions belong to the generation and are also left out.
    pub(crate) fn signatures(&self) -> Vec<FunctionSignature> {
        self.definitions()
            .into_iter()
            .filter(|&function| !self.local_function(function))
            // SAFETY: each `function` is a live definition of this module,
            // returned by LLVM's own traversal.
            .map(|function| unsafe { self.signature_of(function) })
            .collect()
    }

    /// Gives the defined function `of` the name `to` (J03: the signature changed and the
    /// new stable entry has another name). Its uses in the module follow the
    /// value, so they start calling `to`. `false` if `of` is not defined
    /// here or `to` already exists.
    pub(crate) fn rename_function(&self, of: &str, to: &str) -> bool {
        // SAFETY: the definitions are live values of this module; the new name is
        // copied by LLVM.
        unsafe {
            let definition = self.definitions().into_iter().find(|&f| value_name(f) == of);
            let busy = self.definitions().into_iter().any(|f| value_name(f) == to);
            let Some(function) = definition.filter(|_| !busy) else { return false };
            self.rename_definition(function, to);
            value_name(function) == to
        }
    }

    /// Gives the name `to` to the definition `function`, taking its `comdat` along.
    ///
    /// A `linkonce_odr` function of the source SDK (tear-off entry,
    /// canonical constant) lives in a `comdat` with its name. ELF accepts
    /// any member in a group, but in COFF the member whose name is not the key
    /// of the `comdat` is "associative" and requires the key in the module — without it LLVM
    /// aborts code generation (`Associative COMDAT symbol '…' is not a key
    /// for its COMDAT`). The renamed function moves to a `comdat` with the new
    /// name, with the same selection: deduplication across modules still holds
    /// for those with the same name.
    ///
    /// # Safety
    /// `function` must be a live definition of this module.
    unsafe fn rename_definition(&self, function: LLVMValueRef, to: &str) {
        // SAFETY: the caller guarantees validity; the name is copied by LLVM and
        // the `comdat` belongs to the module.
        unsafe {
            LLVMSetValueName2(function, to.as_ptr().cast::<c_char>(), to.len());
            let old = llvm_sys::comdat::LLVMGetComdat(function);
            if old.is_null() {
                return;
            }
            let Ok(name) = CString::new(to) else { return };
            let selection = llvm_sys::comdat::LLVMGetComdatSelectionKind(old);
            let new = llvm_sys::comdat::LLVMGetOrInsertComdat(self.module, name.as_ptr());
            llvm_sys::comdat::LLVMSetComdatSelectionKind(new, selection);
            llvm_sys::comdat::LLVMSetComdat(function, new);
        }
    }

    /// Names the module **declares without defining**, that is, what it consumes.
    ///
    /// In a JIT session this set must be contained in the runtime table,
    /// in the CRT list ([`CRT_SYMBOLS`]) and in what other modules already define. Checking this before
    /// touching the session replaces an ORC link error with a diagnostic carrying the
    /// missing name.
    pub(crate) fn declarations(&self) -> Vec<String> {
        let mut names = Vec::new();
        // SAFETY: traversal through the LLVM API, ended at the first null; the
        // values are valid while this module is not consumed.
        unsafe {
            let mut function = LLVMGetFirstFunction(self.module);
            while !function.is_null() {
                if LLVMIsDeclaration(function) != 0 {
                    names.push(value_name(function));
                }
                function = LLVMGetNextFunction(function);
            }
        }
        names
    }

    /// Data globals declared without a definition in this module.
    ///
    /// The function pre-check does not cover `@name = external global ...`.
    /// A missing data reference can also fail during ORC materialization
    /// after a promotion has already removed the old module.
    pub(crate) fn external_globals(&self) -> Vec<String> {
        let mut names = Vec::new();
        // SAFETY: traversal through the LLVM API over the live module; each name is
        // copied before the module reference is consumed by ORC.
        unsafe {
            let mut global = llvm_sys::core::LLVMGetFirstGlobal(self.module);
            while !global.is_null() {
                if LLVMIsDeclaration(global) != 0 {
                    names.push(value_name(global));
                }
                global = llvm_sys::core::LLVMGetNextGlobal(global);
            }
        }
        names
    }

    /// The module's `target datalayout` and `target triple`, empty when absent.
    pub(crate) fn target(&self) -> (String, String) {
        // SAFETY: both functions return strings that belong to the live module;
        // they are copied before any other LLVM call.
        unsafe {
            (
                CStr::from_ptr(llvm_sys::core::LLVMGetDataLayoutStr(self.module))
                    .to_string_lossy()
                    .into_owned(),
                CStr::from_ptr(llvm_sys::core::LLVMGetTarget(self.module))
                    .to_string_lossy()
                    .into_owned(),
            )
        }
    }

    /// Nominal layout of the classes the module constructs: `(class_id, fields)`.
    ///
    /// It comes from the calls to `@dartforge_object_new(i64 class_id, i64 field_count)`
    /// with both arguments constant, which is the form the native emitter emits
    /// in constructors. Two generations that disagree on the number of fields of the
    /// same class have incompatible layouts: the objects already alive in the managed
    /// heap would keep the old layout, and the new code would read fields
    /// that do not exist. That is why this divergence refuses the reload instead of
    /// silently corrupting the heap.
    ///
    /// Classes the new module does not construct do not appear, and therefore are not
    /// checked — a limit declared in `docs/JIT.md`.
    pub(crate) fn class_layouts(&self) -> Vec<(i64, i64)> {
        let mut layouts = Vec::new();
        for function in self.definitions() {
            // SAFETY: traversal of the module structure through LLVM's own API.
            // Each returned pointer is valid while the module exists, and the
            // loop ends at the first null, as `llvm-c/Core.h` requires.
            unsafe {
                let mut block = LLVMGetFirstBasicBlock(function);
                while !block.is_null() {
                    let mut instruction = LLVMGetFirstInstruction(block);
                    while !instruction.is_null() {
                        if let Some(layout) = object_new_layout(instruction) {
                            layouts.push(layout);
                        }
                        instruction = LLVMGetNextInstruction(instruction);
                    }
                    block = LLVMGetNextBasicBlock(block);
                }
            }
        }
        layouts.sort_unstable();
        layouts.dedup();
        layouts
    }

    /// The names of the classes the module registers: `(class_id, name)`, from the
    /// calls `dartforge_register_class_name(id, @.str, n)`.
    ///
    /// The program's class ids are positional; an edit that inserts
    /// or reorders classes changes the id of a class that already has objects in the
    /// heap, and the reload needs to know that to refuse instead of changing the
    /// class of the live objects.
    pub(crate) fn class_names(&self) -> Vec<(i64, String)> {
        let mut names = Vec::new();
        for function in self.definitions() {
            // SAFETY: the same traversal as `class_layouts`.
            unsafe {
                let mut block = LLVMGetFirstBasicBlock(function);
                while !block.is_null() {
                    let mut instruction = LLVMGetFirstInstruction(block);
                    while !instruction.is_null() {
                        if let Some(name) = class_name_registration(instruction) {
                            names.push(name);
                        }
                        instruction = LLVMGetNextInstruction(instruction);
                    }
                    block = LLVMGetNextBasicBlock(block);
                }
            }
        }
        names.sort_unstable();
        names.dedup();
        names
    }

    /// Versions the implementations and redirects all calls to the entries.
    ///
    /// For each defined function `F` the module comes to contain:
    ///
    /// * the **implementation** renamed to `F$suffix`, which is what this generation
    ///   materializes and what this generation's `ResourceTracker` unloads;
    /// * an **external declaration** `F`, which is the stable entry, resolved by the
    ///   session's trampoline module.
    ///
    /// `LLVMReplaceAllUsesWith` replaces *all* uses of the implementation with the
    /// declaration, including those inside the module itself and recursions. That
    /// is what enforces the plan's rule — «callers go through the entry,
    /// never through the body».
    ///
    /// Today this internal indirection is **not observable**: `hot_reload` takes
    /// `&mut self` and running takes `&self`, so no reload happens with
    /// generated code on the stack, and the case «old frame makes a new call» does not
    /// exist. It is here so that the rule keeps holding when the reload
    /// is triggered from outside the flow — from a file watcher, or from inside
    /// a runtime function called by the program. Without it, that step
    /// would require rewriting already materialized call sites.
    ///
    /// Returns the signatures in IR order, with the **stable** names (without the
    /// suffix), which is the key of the session's entry table.
    pub(crate) fn version_definitions(&self, suffix: &str) -> Vec<FunctionSignature> {
        let mut signatures = Vec::new();
        // The list of definitions is snapshotted before the first rewrite: the
        // values stay valid while the module exists, and the declarations
        // added in the loop are not visited again.
        for function in self.definitions() {
            // `internal`/`private` functions belong only to this generation.
            // They get no trampoline, nor can they be resolved with LLJITLookup.
            if self.local_function(function) {
                continue;
            }
            // SAFETY: `function` is a live definition of this module.
            let signature = unsafe { self.signature_of(function) };
            let versioned = format!("{}{suffix}", signature.name);
            let Ok(stable) = CString::new(signature.name.as_str()) else {
                continue;
            };
            // SAFETY: both names live until the end of the iteration. `AddFunction`
            // creates a declaration (without a body) with exactly the type of the
            // implementation, which keeps all calls well typed after the
            // `RAUW`. Renaming before declaring is mandatory: two global
            // values cannot share a name, and LLVM would resolve the conflict
            // by suffixing the second one, silently breaking the stable entry.
            unsafe {
                let kind = LLVMGlobalGetValueType(function);
                self.rename_definition(function, &versioned);
                let entry = LLVMAddFunction(self.module, stable.as_ptr(), kind);
                LLVMReplaceAllUsesWith(function, entry);
            }
            signatures.push(signature);
        }
        signatures
    }

    /// Reads the signature of a defined function.
    ///
    /// # Safety
    /// `function` must be a live function of this module.
    unsafe fn signature_of(&self, function: LLVMValueRef) -> FunctionSignature {
        // SAFETY: the caller guarantees validity; `GlobalGetValueType` of a
        // function returns its function type, and the following queries only
        // read that type.
        unsafe {
            let kind = LLVMGlobalGetValueType(function);
            FunctionSignature {
                name: value_name(function),
                ret: type_text(LLVMGetReturnType(kind)),
                params: param_types(kind)
                    .into_iter()
                    .map(|param| type_text(param))
                    .collect(),
                var_arg: LLVMIsFunctionVarArg(kind) != 0,
            }
        }
    }

    /// Functions with a body, in IR order.
    fn definitions(&self) -> Vec<LLVMValueRef> {
        let mut functions = Vec::new();
        // SAFETY: traversal through the LLVM API, ended at the first null; the
        // values are valid while this module is not consumed.
        unsafe {
            let mut function = LLVMGetFirstFunction(self.module);
            while !function.is_null() {
                if LLVMIsDeclaration(function) == 0 {
                    functions.push(function);
                }
                function = LLVMGetNextFunction(function);
            }
        }
        functions
    }

    /// Makes all defined functions local to the module, except `keep`.
    ///
    /// A program run at once (`dartforge run`) only needs to export
    /// the entry; exporting the tens of thousands of functions of a program
    /// with large packages makes ORC track the dependency of each emitted
    /// symbol (`WaitingOnGraph`), which grows more than linearly and took
    /// minutes. ORC does not track local functions, and LLVM can still
    /// discard those nothing reaches.
    pub(crate) fn internalize_functions(&self, keep: &[&str]) {
        for function in self.definitions() {
            // SAFETY: `function` came from the traversal of this module's definitions;
            // changing the linkage and removing the comdat are the operations LLVM's
            // `internalize` pass does.
            unsafe {
                if keep.contains(&value_name(function).as_str()) {
                    continue;
                }
                llvm_sys::comdat::LLVMSetComdat(function, ptr::null_mut());
                llvm_sys::core::LLVMSetLinkage(function, llvm_sys::LLVMLinkage::LLVMInternalLinkage);
                llvm_sys::core::LLVMSetVisibility(function, llvm_sys::LLVMVisibility::LLVMDefaultVisibility);
            }
        }
        // What the entry does not reach goes away before code generation (a
        // large package imported whole — all the algorithms of an
        // `export.dart` — cost the machine code of everything).
        let pipeline = CString::new("globaldce").expect("no NUL");
        // SAFETY: the module belongs to this value; the options are created and freed
        // here, and the returned error is consumed.
        unsafe {
            let options = llvm_sys::transforms::pass_builder::LLVMCreatePassBuilderOptions();
            let error = llvm_sys::transforms::pass_builder::LLVMRunPasses(self.module, pipeline.as_ptr(), ptr::null_mut(), options);
            llvm_sys::transforms::pass_builder::LLVMDisposePassBuilderOptions(options);
            if !error.is_null() {
                let msg = LLVMGetErrorMessage(error);
                LLVMDisposeErrorMessage(msg);
            }
        }
    }

    fn local_function(&self, function: LLVMValueRef) -> bool {
        // SAFETY: `function` came from the traversal of this module's definitions.
        let linkage = unsafe { llvm_sys::core::LLVMGetLinkage(function) };
        matches!(linkage, llvm_sys::LLVMLinkage::LLVMInternalLinkage | llvm_sys::LLVMLinkage::LLVMPrivateLinkage)
    }
}

/// Extracts `(class_id, fields)` from a constant call to `@dartforge_object_new`.
///
/// # Safety
/// `instruction` must be a live instruction of a module not yet consumed.
unsafe fn object_new_layout(instruction: LLVMValueRef) -> Option<(i64, i64)> {
    // SAFETY: the queries below only read the instruction and its operands. `IsACall`
    // returns null when the instruction is not a call, and the last operand of a
    // call is the callee — that is why the arguments are read by explicit index and
    // the operand arithmetic is checked first.
    unsafe {
        if LLVMIsACallInst(instruction).is_null() {
            return None;
        }
        let callee = LLVMGetCalledValue(instruction);
        // `dartforge_object_new_t` is the source SDK's (registers the method
        // table on the first allocation); the first two arguments are the
        // same.
        if callee.is_null() || !matches!(value_name(callee).as_str(), "dartforge_object_new" | "dartforge_object_new_t") {
            return None;
        }
        if LLVMGetNumOperands(instruction) < 3 {
            return None;
        }
        let class_id = LLVMGetOperand(instruction, 0);
        let fields = LLVMGetOperand(instruction, 1);
        if LLVMIsAConstantInt(class_id).is_null() || LLVMIsAConstantInt(fields).is_null() {
            return None;
        }
        Some((
            LLVMConstIntGetSExtValue(class_id) as i64,
            LLVMConstIntGetSExtValue(fields) as i64,
        ))
    }
}

/// `(class_id, name)` of a `dartforge_register_class_name` call with
/// constant arguments.
///
/// # Safety
/// `instruction` must be a live instruction of a module not yet consumed.
unsafe fn class_name_registration(instruction: LLVMValueRef) -> Option<(i64, String)> {
    // SAFETY: as in `object_new_layout`; the text is the initializer of a
    // constant global of the module itself, read with the length LLVM
    // reports and copied.
    unsafe {
        if LLVMIsACallInst(instruction).is_null() {
            return None;
        }
        let callee = LLVMGetCalledValue(instruction);
        if callee.is_null() || value_name(callee) != "dartforge_register_class_name" {
            return None;
        }
        if LLVMGetNumOperands(instruction) < 4 {
            return None;
        }
        let class_id = LLVMGetOperand(instruction, 0);
        let text = LLVMGetOperand(instruction, 1);
        if LLVMIsAConstantInt(class_id).is_null() || LLVMIsAGlobalVariable(text).is_null() {
            return None;
        }
        let initial = LLVMGetInitializer(text);
        if initial.is_null() || LLVMIsAConstantDataSequential(initial).is_null() {
            return None;
        }
        let mut len = 0usize;
        let bytes = LLVMGetAsString(initial, &mut len);
        if bytes.is_null() {
            return None;
        }
        let name = String::from_utf8_lossy(std::slice::from_raw_parts(bytes.cast::<u8>(), len)).into_owned();
        Some((LLVMConstIntGetSExtValue(class_id) as i64, name))
    }
}

/// Reads the name of an LLVM global value.
///
/// # Safety
/// `value` must be a live value of a module not yet consumed.
unsafe fn value_name(value: LLVMValueRef) -> String {
    let mut len = 0usize;
    // SAFETY: `GetValueName2` returns pointer and length of a string that
    // belongs to LLVM and lives as long as the value; the copy is made here.
    unsafe {
        let name = LLVMGetValueName2(value, &mut len);
        if name.is_null() {
            return String::new();
        }
        String::from_utf8_lossy(std::slice::from_raw_parts(name.cast::<u8>(), len)).into_owned()
    }
}

/// Prints an LLVM type as IR text.
///
/// # Safety
/// `kind` must be a live type of the context of a module not yet consumed.
unsafe fn type_text(kind: LLVMTypeRef) -> String {
    // SAFETY: `PrintTypeToString` transfers ownership of the string, freed
    // here with `LLVMDisposeMessage`, as `llvm-c/Core.h` requires.
    unsafe { take_message(LLVMPrintTypeToString(kind)) }
}

/// Lists the parameter types of a function type.
///
/// # Safety
/// `kind` must be a live function type.
unsafe fn param_types(kind: LLVMTypeRef) -> Vec<LLVMTypeRef> {
    // SAFETY: `CountParamTypes` reports the exact size `GetParamTypes`
    // writes, and the vector is sized before the write.
    unsafe {
        let count = LLVMCountParamTypes(kind) as usize;
        let mut types = vec![ptr::null_mut::<llvm_sys::LLVMType>(); count];
        if count > 0 {
            LLVMGetParamTypes(kind, types.as_mut_ptr());
        }
        types
    }
}

/// Resource tracker of a single module added to the session.
///
/// # Removal
/// [`ResourceTracker::remove`] unloads the module's code and invalidates the
/// addresses already resolved from it. **It is only safe to remove when no
/// function of that module is running, directly or indirectly**, including
/// on another thread or in a stack frame further below the caller. LLVM does not
/// check this: calling an address obtained before the removal is a use after
/// free. This is why [`crate::JitSession::remove_module`] requires
/// `&mut self` — running the generated code takes `&self` and the two cannot
/// coexist.
pub(crate) struct ResourceTracker {
    handle: LLVMOrcResourceTrackerRef,
}
impl ResourceTracker {
    /// Unloads the resources of the module associated with this tracker.
    ///
    /// # Errors
    /// Propagates LLVM's failure if dematerialization cannot be completed.
    pub(crate) fn remove(&self) -> Result<(), String> {
        // SAFETY: `handle` is a live tracker of this session; `Remove` is idempotent
        // from LLVM's point of view and returns the error instead of aborting.
        match take_error(unsafe { LLVMOrcResourceTrackerRemove(self.handle) }) {
            Some(message) => Err(message),
            None => Ok(()),
        }
    }
}
impl Drop for ResourceTracker {
    /// Returns the reference to the tracker; the `LLJIT` must still be alive.
    fn drop(&mut self) {
        // SAFETY: `crate::JitSession` declares the modules before the `LLJIT`, so
        // the trackers are freed while the `LLJIT` that created them exists.
        unsafe { LLVMOrcReleaseResourceTracker(self.handle) }
    }
}

/// Marks an absolute symbol as visible outside the module and callable.
///
/// `Exported` makes it reachable by whoever looks the name up in the dylib; `Callable`
/// tells ORC the address is code, not data — a distinction that matters
/// for the indirect stubs hot reload will use in a future stage.
fn exported_callable() -> LLVMJITSymbolFlags {
    LLVMJITSymbolFlags {
        GenericFlags: LLVMJITSymbolGenericFlags::LLVMJITSymbolGenericFlagsExported as u8
            | LLVMJITSymbolGenericFlags::LLVMJITSymbolGenericFlagsCallable as u8,
        TargetFlags: 0,
    }
}

/// Marks an absolute symbol as data visible outside the module.
///
/// Without `Callable`: it is the address of a pointer cell, read by the trampoline
/// of the stable entry, and not code anyone can call.
fn exported_data() -> LLVMJITSymbolFlags {
    LLVMJITSymbolFlags {
        GenericFlags: LLVMJITSymbolGenericFlags::LLVMJITSymbolGenericFlagsExported as u8,
        TargetFlags: 0,
    }
}

/// The JIT target machine: host triple, generic `x86-64` CPU and code
/// generation **without optimization**, the counterpart of the AOT's `clang -O0`. It is the same for the
/// session ([`Lljit::new`]) and for the cached object ([`compile_object`]): an
/// object compiled by one and loaded into the other must have the same code.
///
/// The code model is LLVM's JIT default, which on x86-64 avoids
/// 32-bit relocations between the generated code and the process runtime.
///
/// Returns ownership of the `TargetMachine` to the caller.
fn target_machine() -> Result<LLVMTargetMachineRef, String> {
    initialize_native_target()?;
    // SAFETY: the triple and the error message come back to us and are freed
    // with `LLVMDisposeMessage`; the created `TargetMachine` is returned.
    unsafe {
        let triple = LLVMGetDefaultTargetTriple();
        let mut target = ptr::null_mut();
        let mut message = ptr::null_mut();
        if LLVMGetTargetFromTriple(triple, &mut target, &mut message) != 0 {
            let detail = if message.is_null() { "unknown target".to_owned() } else { take_message(message) };
            LLVMDisposeMessage(triple);
            return Err(format!("LLVM has no target for the host: {detail}"));
        }
        let machine = LLVMCreateTargetMachine(
            target,
            triple,
            if cfg!(target_arch = "x86_64") { c"x86-64" } else { c"generic" }.as_ptr(),
            c"".as_ptr(),
            // With map-based roots, never `-O0` (FastISel has known defects
            // with `gc.relocate`, §3.4).
            if roots_by_maps() { LLVMCodeGenOptLevel::LLVMCodeGenLevelLess } else { LLVMCodeGenOptLevel::LLVMCodeGenLevelNone },
            LLVMRelocMode::LLVMRelocDefault,
            LLVMCodeModel::LLVMCodeModelJITDefault,
        );
        LLVMDisposeMessage(triple);
        if machine.is_null() {
            return Err("LLVM did not create the host target machine".to_owned());
        }
        Ok(machine)
    }
}

/// Live instance of the `LLJIT` and its main `JITDylib`.
pub(crate) struct Lljit {
    handle: LLVMOrcLLJITRef,
    main: LLVMOrcJITDylibRef,
    /// With map-based roots or with the symbolic stack trace, the context of the object
    /// layer (which registers the maps and the trace sections); null without
    /// them.
    layer: *mut LayerContext,
    maps: bool,
    trace: bool,
}

impl Lljit {
    /// Creates an `LLJIT` for the host triple and resolves its main `JITDylib`.
    ///
    /// The target machine is built explicitly, not detected: generic CPU
    /// `x86-64` (the same the AOT's `clang` assumes without `-march`) and code
    /// generation **without optimization** (`CodeGenLevelNone`), the counterpart of `clang -O0` in the
    /// development cycle. Host detection would give the machine's CPU and
    /// `CodeGenLevelDefault` — code different from the AOT and slower to generate.
    /// The code model is LLVM's JIT default, which on x86-64 avoids
    /// 32-bit relocations between the generated code and the process runtime.
    ///
    /// # Errors
    /// Fails when the native target is not available or building the
    /// `LLJIT` returns an error, always with the original LLVM message attached.
    pub(crate) fn new() -> Result<Self, String> {
        let machine = target_machine()?;
        let maps = roots_by_maps();
        // The symbolic stack trace (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
        // §13.14): the `dfpcl` section of each object is registered by the
        // memory manager of our own layer.
        let trace = symbolic_trace();
        let layer: *mut LayerContext = if maps || trace {
            Box::into_raw(Box::new(LayerContext { rec: std::sync::Mutex::new(None), trace: std::sync::Mutex::new(None) }))
        } else {
            ptr::null_mut()
        };
        let release_layer = || {
            if !layer.is_null() {
                // SAFETY: `layer` came from `Box::into_raw` and was not handed
                // to any live `LLJIT`.
                drop(unsafe { Box::from_raw(layer) });
            }
        };
        let mut handle: LLVMOrcLLJITRef = ptr::null_mut();
        // SAFETY: each object created here is handed to the next, which takes
        // ownership: the `TargetMachine` to the JTMB, the JTMB to the builder and the builder
        // to `LLVMOrcCreateLLJIT` (which consumes it even when it fails). The
        // object layer with maps receives the context, which lives until `Drop`.
        unsafe {
            let builder = LLVMOrcCreateLLJITBuilder();
            LLVMOrcLLJITBuilderSetJITTargetMachineBuilder(
                builder,
                LLVMOrcJITTargetMachineBuilderCreateFromTargetMachine(machine),
            );
            if maps || trace {
                llvm_sys::orc2::lljit::LLVMOrcLLJITBuilderSetObjectLinkingLayerCreator(builder, create_layer, layer.cast());
            }
            if let Some(message) = take_error(LLVMOrcCreateLLJIT(&mut handle, builder)) {
                release_layer();
                return Err(message);
            }
        }
        if handle.is_null() {
            release_layer();
            return Err("LLVM returned a null LLJIT without a diagnostic".to_owned());
        }
        if maps {
            // SAFETY: the transform layer belongs to the live `LLJIT`; the
            // function uses no context.
            unsafe {
                llvm_sys::orc2::LLVMOrcIRTransformLayerSetTransform(
                    llvm_sys::orc2::lljit::LLVMOrcLLJITGetIRTransformLayer(handle),
                    transform_ir,
                    ptr::null_mut(),
                );
            }
        }
        // SAFETY: `handle` was just created successfully; the main dylib
        // belongs to the `LLJIT` and is valid while this structure lives.
        let main = unsafe { LLVMOrcLLJITGetMainJITDylib(handle) };
        Ok(Self { handle, main, layer, maps, trace })
    }

    /// With map-based roots: the objects' maps go to this process's
    /// runtime (the session that publishes its symbols).
    pub(crate) fn use_local_registry(&self) {
        if self.layer.is_null() {
            return;
        }
        // SAFETY: the layer lives as long as the session.
        let layer = unsafe { &*self.layer };
        if self.maps {
            *layer.rec.lock().unwrap_or_else(|e| e.into_inner()) = Some((register_local, deregister_local));
        }
        if self.trace {
            *layer.trace.lock().unwrap_or_else(|e| e.into_inner()) = Some((register_local_trace, deregister_local_trace));
        }
    }

    /// With map-based roots: the objects' maps go to the runtime of the
    /// source SDK library (it is the one that collects).
    ///
    /// # Errors
    /// The library does not load or does not export the registration.
    pub(crate) fn use_library_registry(&self, dll: &std::path::Path) -> Result<(), String> {
        if self.layer.is_null() {
            return Ok(());
        }
        let module = load_library(dll);
        if module.is_null() {
            return Err(format!("could not load the SDK library {}", dll.display()));
        }
        if self.maps {
            let register = address_in_library(module, c"dartforge_register_map_llvm");
            let deregister = address_in_library(module, c"dartforge_unregister_map");
            if register.is_null() || deregister.is_null() {
                return Err("the SDK library does not export the stack map registration (dartforge_register_map_llvm)".to_owned());
            }
            // SAFETY: the layer lives as long as the session; the symbols are the
            // runtime functions with these signatures (`gc_roots.rs`), and the
            // library is never unloaded.
            unsafe {
                let layer = &*self.layer;
                *layer.rec.lock().unwrap_or_else(|e| e.into_inner()) = Some((
                    std::mem::transmute::<*mut std::ffi::c_void, RegisterFunction>(register),
                    std::mem::transmute::<*mut std::ffi::c_void, DeregisterFunction>(deregister),
                ));
            }
        }
        if self.trace {
            let register = address_in_library(module, c"dartforge_register_stack_trace");
            let deregister = address_in_library(module, c"dartforge_unregister_stack_trace");
            if register.is_null() || deregister.is_null() {
                return Err("the SDK library does not export the stack trace registration (dartforge_register_stack_trace)".to_owned());
            }
            // SAFETY: as above (the runtime's `stack_trace.rs`).
            unsafe {
                let layer = &*self.layer;
                *layer.trace.lock().unwrap_or_else(|e| e.into_inner()) = Some((
                    std::mem::transmute::<*mut std::ffi::c_void, RegisterFunction>(register),
                    std::mem::transmute::<*mut std::ffi::c_void, DeregisterFunction>(deregister),
                ));
            }
        }
        Ok(())
    }

    /// `target datalayout` and `target triple` that this `LLJIT` imposes on modules.
    pub(crate) fn target(&self) -> (String, String) {
        // SAFETY: both strings belong to the live `LLJIT` and are copied right away.
        unsafe {
            (
                CStr::from_ptr(LLVMOrcLLJITGetDataLayoutStr(self.handle))
                    .to_string_lossy()
                    .into_owned(),
                CStr::from_ptr(LLVMOrcLLJITGetTripleString(self.handle))
                    .to_string_lossy()
                    .into_owned(),
            )
        }
    }

    /// Publishes the addresses of the runtime functions as absolute symbols.
    ///
    /// This is the JIT counterpart of the linking the AOT driver does with the
    /// runtime: instead of a linker resolving `@dartforge_print_i64` in an object,
    /// the `JITDylib` comes to contain that name pointing to the Rust function already
    /// loaded in this process. The table is generated from the runtime source
    /// ([`RUNTIME_SYMBOLS`]); along with it go the data the static CRT would give the
    /// AOT executable ([`crt_data_symbols`]).
    ///
    /// # Errors
    /// Fails if some name is already defined in the dylib, in which case the materialization
    /// unit is freed without changing the session.
    pub(crate) fn define_runtime_symbols(&self) -> Result<(), String> {
        let names: Vec<(CString, u64)> = runtime_symbol_addresses()
            .into_iter()
            .map(|(name, address)| {
                (
                    CString::new(name).expect("runtime name without a null byte"),
                    address as u64,
                )
            })
            .collect();
        let pairs: Vec<(&CStr, u64)> =
            names.iter().map(|(name, address)| (name.as_c_str(), *address)).collect();
        self.define_absolute(&pairs, exported_callable())?;
        self.define_absolute(&crt_data_symbols(), exported_data())
    }

    /// Publishes addresses of **data** of this thread as absolute symbols.
    ///
    /// It is how hot reload's implementation pointers enter the `JITDylib`:
    /// each name comes to designate the address of a Rust cell, and the generated
    /// trampoline reads that cell on every call. The symbols are exported but
    /// **not** marked as `Callable`, because they are data, not code — the
    /// distinction `llvm-c/Orc.h` asks for and that prevents a name mistake from
    /// turning a cell into a call target.
    ///
    /// # Errors
    /// Fails if some name is already defined in the dylib; in that case the materialization
    /// unit is freed and the session does not change.
    pub(crate) fn define_data_symbols(&self, pairs: &[(CString, u64)]) -> Result<(), String> {
        let borrowed: Vec<(&CStr, u64)> = pairs
            .iter()
            .map(|(name, address)| (name.as_c_str(), *address))
            .collect();
        self.define_absolute(&borrowed, exported_data())
    }

    /// Defines a list of absolute symbols with the same flags.
    ///
    /// # Errors
    /// Propagates LLVM's diagnostic, typically a duplicate definition.
    fn define_absolute(
        &self,
        symbols: &[(&CStr, u64)],
        flags: LLVMJITSymbolFlags,
    ) -> Result<(), String> {
        if symbols.is_empty() {
            return Ok(());
        }
        let mut pairs = Vec::with_capacity(symbols.len());
        for (name, address) in symbols {
            // SAFETY: `name` is alive during the call and `self.handle` is valid.
            // The returned entry comes already retained and its ownership is transferred
            // to `LLVMOrcAbsoluteSymbols` below, which consumes it.
            let interned = unsafe { LLVMOrcLLJITMangleAndIntern(self.handle, name.as_ptr()) };
            pairs.push(LLVMOrcCSymbolMapPair {
                Name: interned,
                Sym: LLVMJITEvaluatedSymbol {
                    Address: *address,
                    Flags: LLVMJITSymbolFlags {
                        GenericFlags: flags.GenericFlags,
                        TargetFlags: flags.TargetFlags,
                    },
                },
            });
        }
        // SAFETY: `pairs` is a contiguous vector of valid pairs, whose size is
        // reported exactly. `LLVMOrcAbsoluteSymbols` consumes the entries of the
        // name pool, so `pairs` cannot be reused after this point.
        let unit = unsafe { LLVMOrcAbsoluteSymbols(pairs.as_mut_ptr(), pairs.len()) };
        // SAFETY: `unit` was just created and is consumed by `Define` on
        // success; on error the ownership comes back to us and the unit is
        // freed explicitly, as documented in `llvm-c/Orc.h`.
        match take_error(unsafe { LLVMOrcJITDylibDefine(self.main, unit) }) {
            None => Ok(()),
            Some(message) => {
                // SAFETY: `Define` failed, so `unit` is still ours.
                unsafe { LLVMOrcDisposeMaterializationUnit(unit) };
                Err(message)
            }
        }
    }

    /// Creates a new tracker for the resources of a module.
    pub(crate) fn create_tracker(&self) -> ResourceTracker {
        // SAFETY: `self.main` belongs to this live `LLJIT`; the returned tracker
        // comes already retained and is freed by `ResourceTracker::drop`.
        let handle = unsafe { LLVMOrcJITDylibCreateResourceTracker(self.main) };
        ResourceTracker { handle }
    }

    /// Hands the module to the `LLJIT` under the given tracker.
    ///
    /// # Errors
    /// Propagates LLVM's message, typically a duplicate symbol definition.
    /// In any case the module was consumed and cannot be reused.
    pub(crate) fn add_module(
        &self,
        tracker: &ResourceTracker,
        module: ThreadSafeModule,
    ) -> Result<(), String> {
        // LLVM takes ownership of the module even when it returns an error,
        // which is why `ManuallyDrop` suppresses the local `Drop` before the call.
        let module = ManuallyDrop::new(module);
        // SAFETY: `module.handle` is valid and was not consumed before; from
        // this call on it belongs to the `LLJIT` and is no longer freed by us.
        match take_error(unsafe {
            LLVMOrcLLJITAddLLVMIRModuleWithRT(self.handle, tracker.handle, module.handle)
        }) {
            Some(message) => Err(message),
            None => Ok(()),
        }
    }

    /// Resolves a symbol, materializing the necessary code.
    ///
    /// The name is given without the platform's global prefix: `LLJITLookup`
    /// applies the same decoration used in the definition of the absolute symbols.
    ///
    /// # Errors
    /// Fails when the symbol does not exist, when the on-demand compilation of the
    /// module that defines it fails, or when the name contains a null byte.
    pub(crate) fn lookup(&self, symbol: &str) -> Result<u64, String> {
        let symbol =
            CString::new(symbol).map_err(|_| "symbol name contains a null byte".to_owned())?;
        let mut address = 0u64;
        // SAFETY: `address` is a valid destination and `symbol` is alive during the
        // call. Materializing may run LLVM code, not program code.
        match take_error(unsafe { LLVMOrcLLJITLookup(self.handle, &mut address, symbol.as_ptr()) })
        {
            Some(message) => Err(message),
            None if address == 0 => Err("symbol resolved to address zero".to_owned()),
            None => Ok(address),
        }
    }

    /// Calls a stable entry whose signature was checked right here.
    ///
    /// The check is the reason the function is safe. The caller gives the
    /// address and the signature the session registered for that entry — read
    /// from the IR itself by [`ParsedModule::signatures`] —, and only two forms are
    /// accepted: `i64 ()` and `i64 (i64)`. Anything else becomes an error, never a
    /// call with the wrong ABI. It is the same reasoning as [`Lljit::run_entry`], which
    /// does not accept a symbol name from the caller, applied to signatures with a return
    /// value.
    ///
    /// The address must be that of a **stable entry** (the trampoline), never
    /// that of a generation implementation: the trampoline is not unloaded, and it is what
    /// guarantees that a call made after a reload reaches the
    /// new implementation.
    ///
    /// # Errors
    /// Signature outside the two supported forms, or an argument present/absent
    /// in disagreement with the arity.
    pub(crate) fn call_stable(
        &self,
        address: u64,
        signature: &FunctionSignature,
        argument: Option<i64>,
    ) -> Result<i64, String> {
        if signature.var_arg || signature.ret != "i64" {
            return Err(format!(
                "the stable entry has signature {} and this crate only calls i64 () and i64 (i64)",
                signature.text()
            ));
        }
        let pointer = usize::try_from(address)
            .map_err(|_| "stable entry address does not fit in usize".to_owned())?
            as *const ();
        match (signature.params.as_slice(), argument) {
            ([], None) => {
                // SAFETY: the signature registered for this address is `i64 ()`,
                // checked above, and the address came from a `lookup` of this live
                // `LLJIT`, whose trampoline is never unloaded. `extern "C"` is the ABI
                // declared in the IR by the emitter.
                Ok(unsafe { std::mem::transmute::<*const (), extern "C" fn() -> i64>(pointer)() })
            }
            ([param], Some(value)) if param == "i64" => {
                // SAFETY: likewise, for the signature `i64 (i64)`.
                Ok(unsafe {
                    std::mem::transmute::<*const (), extern "C" fn(i64) -> i64>(pointer)(value)
                })
            }
            _ => Err(format!(
                "the stable entry has signature {} and the given argument does not match",
                signature.text()
            )),
        }
    }

    /// Invokes a reloadable trampoline with ABI `void ()`, checked before the FFI.
    pub(crate) fn call_stable_void(&self, address: u64, signature: &FunctionSignature) -> Result<(), String> {
        if signature.var_arg || signature.ret != "void" || !signature.params.is_empty() {
            return Err(format!("the stable entry has signature {}, expected void ()", signature.text()));
        }
        let pointer = usize::try_from(address)
            .map_err(|_| "stable entry address does not fit in usize".to_owned())?
            as *const ();
        // SAFETY: the signature `void ()` was checked above; the address is the
        // trampoline published by this session's LLJIT and lives until it ends.
        unsafe { std::mem::transmute::<*const (), extern "C" fn()>(pointer)() };
        Ok(())
    }

    /// Calls a stable entry with ABI `i32 ()` (the source SDK's `main`).
    pub(crate) fn call_stable_i32(&self, address: u64, signature: &FunctionSignature) -> Result<i32, String> {
        if signature.var_arg || signature.ret != "i32" || !signature.params.is_empty() {
            return Err(format!("the stable entry has signature {}, expected i32 ()", signature.text()));
        }
        let pointer = usize::try_from(address)
            .map_err(|_| "stable entry address does not fit in usize".to_owned())?
            as *const ();
        // SAFETY: ABI `i32 ()` checked above; the trampoline belongs to the live LLJIT
        // of this session, and the caller only reloads when nothing is running.
        Ok(unsafe { std::mem::transmute::<*const (), extern "C" fn() -> i32>(pointer)() })
    }

    /// Resolves and runs the module's entry, measuring both phases.
    ///
    /// The signature is safe on purpose: the symbol name does **not** come from the
    /// caller. Resolving [`crate::ENTRY_SYMBOL`] in this live `LLJIT` and calling it
    /// as `void(void)` is correct because `crates/emit_native` emits
    /// `@dartforge_entry` with exactly that signature, and `crates/jit` only
    /// accepts IR from that emitter — the same assumption the AOT driver makes. If the
    /// caller could choose the name, the function would have to be `unsafe`.
    ///
    /// After the entry comes the runtime's `finalize_program` (the same source as the
    /// AOT), on its own thread with `stack_bytes` of stack: see
    /// [`execute_program`]. `&self` guarantees that the `LLJIT` and the entry module
    /// stay alive until the thread ends, because removing requires `&mut`.
    ///
    /// Returns `(lookup, exit code, execute)`; `lookup` includes the code
    /// generation of the module that defines the entry.
    ///
    /// # Errors
    /// Fails when no module of the session defines the entry, when the
    /// on-demand compilation of that module fails, or when the program
    /// thread cannot be created.
    pub(crate) fn run_entry(
        &self,
        stack_bytes: usize,
        globals: &[&MutableGlobal],
    ) -> Result<(Duration, i32, Duration), String> {
        let phase = Instant::now();
        let address = self.lookup(crate::ENTRY_SYMBOL)?;
        let lookup = phase.elapsed();
        // The statics of the previous execution (if any) go back to zero: each
        // execution starts like a new AOT process.
        for global in globals {
            self.zero_global(global)?;
        }
        let (code, execute) = execute_program(address, stack_bytes)?;
        Ok((lookup, code, execute))
    }
}

impl Drop for Lljit {
    /// Ends the LLVM session, unloading all still resident code.
    fn drop(&mut self) {
        // SAFETY: all trackers of this session were already freed, because
        // `crate::JitSession` declares the module vector before this field and
        // fields are dropped in declaration order.
        let _ = take_error(unsafe { LLVMOrcDisposeLLJIT(self.handle) });
        // The objects (and their maps) went away with the `LLJIT`; the layer's
        // context goes afterwards.
        if !self.layer.is_null() {
            // SAFETY: it came from `Box::into_raw` in `new`, and the `LLJIT` that
            // used it was already freed.
            drop(unsafe { Box::from_raw(self.layer) });
        }
    }
}

// ─── Source SDK (DLL) ──────────────────────────────────────────────────────

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut std::ffi::c_void;
    fn GetProcAddress(module: *mut std::ffi::c_void, name: *const std::ffi::c_char) -> *mut std::ffi::c_void;
}

#[cfg(unix)]
#[cfg_attr(target_os = "linux", link(name = "dl"))]
unsafe extern "C" {
    fn dlopen(name: *const std::ffi::c_char, mode: std::ffi::c_int) -> *mut std::ffi::c_void;
    fn dlsym(module: *mut std::ffi::c_void, name: *const std::ffi::c_char) -> *mut std::ffi::c_void;
}

/// `RTLD_NOW | RTLD_LOCAL`: everything resolved at load time, and the library's names
/// do not enter the process's global scope (its runtime does not mix with
/// this executable's `dartforge_runtime::abi`). `RTLD_LOCAL` is 0 on Linux and
/// 4 on macOS; `RTLD_NOW` is 2 on both.
#[cfg(unix)]
const RTLD_NOW_LOCAL: std::ffi::c_int = if cfg!(target_os = "macos") { 2 | 4 } else { 2 };

/// Loads the SDK shared library and returns the handle, or null.
#[cfg(windows)]
fn load_library(file_path: &std::path::Path) -> *mut std::ffi::c_void {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = file_path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: `wide` is a zero-terminated path; the DLL stays loaded
    // until the end of the process (it is never unloaded).
    unsafe { LoadLibraryW(wide.as_ptr()) }
}

/// Loads the SDK shared library and returns the handle, or null.
#[cfg(unix)]
fn load_library(file_path: &std::path::Path) -> *mut std::ffi::c_void {
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = CString::new(file_path.as_os_str().as_bytes()) else { return std::ptr::null_mut() };
    // SAFETY: `c` is a zero-terminated path; the library stays loaded
    // until the end of the process (it is never unloaded).
    unsafe { dlopen(c.as_ptr(), RTLD_NOW_LOCAL) }
}

/// The address of `name` in the library `module`, or null.
fn address_in_library(module: *mut std::ffi::c_void, name: &CStr) -> *mut std::ffi::c_void {
    // SAFETY: `module` is a handle returned by `load_library` and
    // `name` is zero-terminated.
    #[cfg(windows)]
    let p = unsafe { GetProcAddress(module, name.as_ptr()) };
    #[cfg(unix)]
    let p = unsafe { dlsym(module, name.as_ptr()) };
    p
}

/// Calls `dartforge_set_main_arguments` of the SDK library's
/// runtime (`data`: the arguments in UTF-8, each terminated by NUL).
pub(crate) fn set_arguments_in_library(dll: &std::path::Path, data: &[u8]) -> Result<(), String> {
    let module = load_library(dll);
    if module.is_null() {
        return Err(format!("could not load the SDK library {}", dll.display()));
    }
    let p = address_in_library(module, c"dartforge_set_main_arguments");
    if p.is_null() && data.is_empty() {
        // With no arguments to hand over, a library without the whole runtime
        // (the test ones) will do.
        return Ok(());
    }
    if p.is_null() {
        return Err("the SDK DLL does not export dartforge_set_main_arguments".to_string());
    }
    // SAFETY: the symbol is the runtime function with this signature
    // (`io_platform.rs`); `data` lives during the call, which copies it.
    unsafe {
        let f: unsafe extern "C" fn(*const u8, usize) = std::mem::transmute(p);
        f(data.as_ptr(), data.len());
    }
    Ok(())
}

/// Calls `dartforge_set_program_script` of the SDK library's
/// runtime (the `Platform.script`). A library without the whole runtime (the test
/// ones) does not have the symbol, and the script stays the process's.
pub(crate) fn set_script_in_library(dll: &std::path::Path, file_path: &[u8]) -> Result<(), String> {
    let module = load_library(dll);
    if module.is_null() {
        return Err(format!("could not load the SDK library {}", dll.display()));
    }
    let p = address_in_library(module, c"dartforge_set_program_script");
    if p.is_null() {
        return Ok(());
    }
    // SAFETY: the symbol is the runtime function with this signature
    // (`io_platform.rs`); `file_path` lives during the call, which copies it.
    unsafe {
        let f: unsafe extern "C" fn(*const u8, usize) = std::mem::transmute(p);
        f(file_path.as_ptr(), file_path.len());
    }
    Ok(())
}

// ─── Live reload ───────────────────────────────────────────────────────────
//
// The program runs on its own thread while the watcher compiles the
// new generation; publication (the cells, the statics, the registrations) runs
// on the program thread, at the main isolate's safepoint — between two
// loop events (`dartforge_request_at_safepoint`, `ports.rs`), or with the
// program stopped. Everything that crosses the boundary is an address: the publication
// task owns its data and borrows nothing from the session.

/// A task that runs on the program thread.
pub(crate) type Task = Box<dyn FnOnce() + Send>;

/// The functions of the session's runtime that live reload uses: those of this
/// process's runtime, or those of the source SDK library (the runtime state
/// of the program is the library's).
#[derive(Clone, Copy, Debug)]
pub(crate) struct ReloadRuntime {
    /// `dartforge_request_at_safepoint`.
    request_fn: usize,
    /// `dartforge_publish_generation`.
    publish: usize,
    /// `dartforge_stop_isolates` and `dartforge_release_isolates`.
    stop: usize,
    free: usize,
    /// `dartforge_set_migration` (J03); 0 in an old SDK library.
    migrate: usize,
}

/// The request handed to the runtime: the task (taken by whoever runs it) and the
/// answer, which the watcher waits for.
struct ReloadRequest {
    task: std::sync::Mutex<Option<Task>>,
    /// `Some(true)`: it ran at the safepoint; `Some(false)`: the isolate ended
    /// without serving it and the task is still in `task`.
    response: std::sync::Mutex<Option<bool>>,
    sig_num: std::sync::Condvar,
}

/// What the runtime calls at the safepoint (`execute_fn` = 1) or on discard
/// (`execute_fn` = 0), exactly once per accepted request.
extern "C" fn serve_reload_request(datum: usize, execute_fn: i32) {
    // SAFETY: `datum` is the address of the `ReloadRequest` that
    // `ReloadRuntime::at_safepoint` keeps alive until reading the answer, and the
    // runtime calls this function only once per accepted request.
    let request = unsafe { &*(datum as *const ReloadRequest) };
    if execute_fn != 0 {
        let task = request.task.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(task) = task {
            task();
        }
    }
    *request.response.lock().unwrap_or_else(|e| e.into_inner()) = Some(execute_fn != 0);
    request.sig_num.notify_all();
}

impl ReloadRuntime {
    /// This process's runtime (`dartforge_runtime::abi`).
    pub(crate) fn embedded() -> Self {
        Self {
            request_fn: dartforge_runtime::abi::dartforge_request_at_safepoint as *const () as usize,
            publish: dartforge_runtime::abi::dartforge_publish_generation as *const () as usize,
            stop: dartforge_runtime::abi::dartforge_stop_isolates as *const () as usize,
            free: dartforge_runtime::abi::dartforge_release_isolates as *const () as usize,
            migrate: dartforge_runtime::abi::dartforge_set_migration as *const () as usize,
        }
    }

    /// The runtime of the source SDK library.
    pub(crate) fn from_library(dll: &std::path::Path) -> Result<Self, String> {
        let module = load_library(dll);
        if module.is_null() {
            return Err(format!("could not load the SDK library {}", dll.display()));
        }
        let request_fn = address_in_library(module, c"dartforge_request_at_safepoint");
        let publish = address_in_library(module, c"dartforge_publish_generation");
        let stop = address_in_library(module, c"dartforge_stop_isolates");
        let free = address_in_library(module, c"dartforge_release_isolates");
        if request_fn.is_null() || publish.is_null() || stop.is_null() || free.is_null() {
            return Err(format!(
                "the SDK library {} does not export live reload (dartforge_request_at_safepoint); \
                 recompile it with this dartforge",
                dll.display()
            ));
        }
        let migrate = address_in_library(module, c"dartforge_set_migration");
        Ok(Self {
            request_fn: request_fn as usize,
            publish: publish as usize,
            stop: stop as usize,
            free: free as usize,
            migrate: migrate as usize,
        })
    }

    /// Does the runtime know how to migrate live instances (J03)?
    pub(crate) fn migrates(&self) -> bool {
        self.migrate != 0
    }

    /// `dartforge_set_migration`: the plan of the publication in progress, which
    /// each isolate applies to its heap when publishing (`migration.rs`).
    pub(crate) fn set_migration(&self, data: &[i64]) {
        if self.migrate == 0 {
            return;
        }
        // SAFETY: `migrate` is `dartforge_set_migration` (`selectors.rs`),
        // which copies the `data.len()` words.
        unsafe {
            std::mem::transmute::<usize, unsafe extern "C" fn(*const i64, i64)>(self.migrate)(data.as_ptr(), data.len() as i64);
        }
    }

    /// Runs `task` at the main isolate's safepoint, on its thread, and
    /// waits. `Err(task)` (the task did NOT run) when there is no main
    /// isolate in the loop, or when it ended before serving it.
    /// `waiting` is called once if the isolate takes long to reach the
    /// safepoint (long synchronous code).
    pub(crate) fn at_safepoint(&self, task: Task, waiting: &mut dyn FnMut()) -> Result<(), Task> {
        let request = ReloadRequest {
            task: std::sync::Mutex::new(Some(task)),
            response: std::sync::Mutex::new(None),
            sig_num: std::sync::Condvar::new(),
        };
        // SAFETY: `request_fn` is `dartforge_request_at_safepoint` (`ports.rs`),
        // with this signature; the request lives until the answer below.
        let accepted = unsafe {
            let request_fn: extern "C" fn(extern "C" fn(usize, i32), usize) -> i32 = std::mem::transmute(self.request_fn);
            request_fn(serve_reload_request, &request as *const ReloadRequest as usize)
        };
        if accepted == 0 {
            let task = request.task.lock().unwrap_or_else(|e| e.into_inner()).take();
            return Err(task.expect("a refused request keeps the task"));
        }
        let mut response = request.response.lock().unwrap_or_else(|e| e.into_inner());
        let mut notified = false;
        // An accepted request is answered exactly once (at the safepoint
        // or at the end of the isolate): waiting without a deadline is what keeps the request
        // alive while the runtime has its address.
        while response.is_none() {
            let (r, deadline) = request
                .sig_num
                .wait_timeout(response, Duration::from_secs(2))
                .unwrap_or_else(|e| e.into_inner());
            response = r;
            if deadline.timed_out() && response.is_none() && !notified {
                notified = true;
                waiting();
            }
        }
        let ran = response.expect("respondido");
        drop(response);
        if ran {
            return Ok(());
        }
        let task = request.task.lock().unwrap_or_else(|e| e.into_inner()).take();
        Err(task.expect("a discarded request keeps the task"))
    }

    /// Stops the other isolates at their safepoint (called on the thread of the
    /// main isolate, at its safepoint) and returns the stop.
    pub(crate) fn stop_isolates(&self) -> usize {
        // SAFETY: `stop` is `dartforge_stop_isolates` (`ports.rs`).
        unsafe { std::mem::transmute::<usize, extern "C" fn() -> usize>(self.stop)() }
    }

    /// Releases the stop: each isolate redoes the registrations of the new generation.
    pub(crate) fn release_isolates(&self, stopped_flag: usize, area: Option<u64>, register: Option<u64>, rti: Option<u64>) {
        let e = |x: Option<u64>| x.map_or(0, |v| v as usize);
        // SAFETY: `free` is `dartforge_release_isolates` (`ports.rs`), and
        // `stopped_flag` came from `stop_isolates`, once.
        unsafe {
            std::mem::transmute::<usize, extern "C" fn(usize, usize, usize, usize)>(self.free)(stopped_flag, e(area), e(register), e(rti));
        }
    }

    /// `dartforge_publish_generation(area, register, rti)`: the globals area
    /// and the registrations of the new generation, on the program thread.
    pub(crate) fn publish_records(&self, area: Option<u64>, register: Option<u64>, rti: Option<u64>) {
        let as_fn = |address: Option<u64>| {
            // SAFETY: the addresses are `void ()` trampolines of the session
            // (`df.prepare_area`, `df.register.program`,
            // `dartforge_rti_init`), alive
            // while it exists.
            address.map(|e| unsafe { std::mem::transmute::<usize, extern "C" fn()>(e as usize) })
        };
        // SAFETY: `publish` is `dartforge_publish_generation` (`selectors.rs`).
        unsafe {
            let publish: extern "C" fn(Option<extern "C" fn()>, Option<extern "C" fn()>, Option<extern "C" fn()>) =
                std::mem::transmute(self.publish);
            publish(as_fn(area), as_fn(register), as_fn(rti));
        }
    }
}

/// The cell of a stable entry, by address (the session's `Box<AtomicUsize>`,
/// stable while it exists).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cell(usize);

impl Cell {
    pub(crate) fn of(cell: &std::sync::atomic::AtomicUsize) -> Self {
        Self(cell as *const std::sync::atomic::AtomicUsize as usize)
    }

    /// Points the entry to `implementation`: the following calls use it.
    pub(crate) fn publish(self, implementation: usize) {
        // SAFETY: the cell belongs to an entry of the session, which outlives
        // the program thread (`JitSession::with_live_program`).
        let cell = unsafe { &*(self.0 as *const std::sync::atomic::AtomicUsize) };
        cell.store(implementation, std::sync::atomic::Ordering::Release);
    }
}

/// The copy of a static from one generation to the next.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GlobalCopy {
    of: usize,
    to: usize,
    size: usize,
}

impl GlobalCopy {
    pub(crate) fn apply(self) {
        // SAFETY: both addresses are materialized LLJIT globals, of the
        // same size (`Lljit::global_copy`), of retained generations.
        unsafe { std::ptr::copy_nonoverlapping(self.of as *const u8, self.to as *mut u8, self.size) };
    }
}

/// Calls the source SDK's `i32 ()` `main` on the current thread (it calls the
/// library's `dartforge_init`, which marks the main isolate).
pub(crate) fn call_source_main(address: u64) -> i32 {
    // SAFETY: `address` is the `main` trampoline, checked `i32 ()` by the
    // session, which outlives the program thread.
    unsafe { std::mem::transmute::<usize, extern "C" fn() -> i32>(address as usize)() }
}

/// Calls the embedded runtime's `dartforge_entry` on the current thread, as the
/// main isolate, and finalizes the program like the AOT.
pub(crate) fn call_embedded_entry(address: u64) -> i32 {
    dartforge_runtime::abi::mark_main_isolate();
    // SAFETY: `address` is the `dartforge_entry` trampoline, checked
    // `void ()` by the session, which outlives the program thread.
    unsafe { std::mem::transmute::<usize, extern "C" fn()>(address as usize)() };
    dartforge_runtime::abi::unmark_main_isolate();
    dartforge_runtime::abi::finalize_program()
}

impl Lljit {
    /// Requested names that appear in the SDK DLL's export list.
    /// The list is consulted before publishing a new generation, so that
    /// an unknown reference refuses the reload without changing the pointers.
    pub(crate) fn exported_symbols_in_dll(dll: &std::path::Path, used_count: &[String]) -> Result<Vec<String>, String> {
        let def = dll.with_file_name("exportados.def");
        let text = std::fs::read_to_string(&def).map_err(|e| format!("{}: {e}", def.display()))?;
        Ok(text
            .lines()
            .skip_while(|l| l.trim() != "EXPORTS")
            .skip(1)
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && used_count.contains(l))
            .collect())
    }

    /// Loads the source SDK DLL and publishes in the session the address of each
    /// name it exports (the list is the `exportados.def` next to it).
    /// Returns the names.
    ///
    /// # Errors
    /// Missing DLL or list, or a name in the list the DLL does not export.
    pub(crate) fn define_symbols_from_dll(&self, dll: &std::path::Path, used_count: &[String], publish_crt: bool) -> Result<Vec<String>, String> {
        let names = Self::exported_symbols_in_dll(dll, used_count)?;
        let module = load_library(dll);
        if module.is_null() {
            return Err(format!("could not load the SDK library {}", dll.display()));
        }
        let mut pair_list = Vec::with_capacity(names.len());
        for n in &names {
            let c = CString::new(n.as_str()).map_err(|_| format!("name with a null byte: {n}"))?;
            let p = address_in_library(module, &c);
            if p.is_null() {
                return Err(format!("the SDK DLL does not export {n}"));
            }
            pair_list.push((c, p as u64));
        }
        let borrowed_list: Vec<(&CStr, u64)> = pair_list.iter().map(|(n, a)| (n.as_c_str(), *a)).collect();
        self.define_absolute(&borrowed_list, exported_callable())?;
        if publish_crt {
            self.define_absolute(&crt_data_symbols(), exported_data())?;
        }
        Ok(names)
    }

    /// Resolves the program's `main` (source SDK) and runs it on a new thread
    /// with a stack of `stack_bytes`; the code is what it returns. The
    /// globals of the received JIT modules go back to zero as in `run_entry`.
    /// This operation does not reset the SDK DLL's internal statics.
    pub(crate) fn run_main(&self, stack_bytes: usize, globals: &[&MutableGlobal]) -> Result<(Duration, i32, Duration), String> {
        let phase = Instant::now();
        let address = usize::try_from(self.lookup("main")?).map_err(|_| "main address".to_owned())?;
        let lookup = phase.elapsed();
        for global in globals {
            self.zero_global(global)?;
        }
        let (code, execute) = std::thread::scope(|scope| {
            let handle = std::thread::Builder::new()
                .stack_size(stack_bytes)
                .spawn_scoped(scope, move || {
                    let started = Instant::now();
                    // SAFETY: `address` came from `LLVMOrcLLJITLookup(main)`, which the
                    // emitter writes with the C signature `i32(void)`.
                    let code = unsafe { std::mem::transmute::<usize, extern "C" fn() -> i32>(address)() };
                    (code, started.elapsed())
                })
                .map_err(|error| format!("could not create the thread of the program: {error}"))?;
            handle.join().map_err(|_| "the program thread ended in a panic".to_owned())
        })?;
        Ok((lookup, code, execute))
    }
}

// ─── Runtime ───────────────────────────────────────────────────────────────
//
// The runtime is the fragments of `crates/runtime/src`, the same source the AOT
// compiles with standalone `rustc`, here compiled as the module
// `dartforge_runtime::abi` (single source, JIT plan §3.1). The symbol
// table is generated by that crate's `build.rs` from the
// `#[unsafe(no_mangle)]`s of the fragments.

/// Runtime names published in the session, in source order.
pub(crate) const RUNTIME_SYMBOLS: &[&str] = dartforge_runtime::symbols::NAMES;

/// Address of each runtime function, in the order of [`RUNTIME_SYMBOLS`].
fn runtime_symbol_addresses() -> Vec<(&'static str, usize)> {
    dartforge_runtime::symbols::table()
}

/// Runs the program whose entry is at `address`, like the AOT `main`.
///
/// The program runs on a **new** thread, with a stack of `stack_bytes`. All
/// runtime state is `thread_local` (heap, registered classes, pending
/// exception), so each execution starts from a clean runtime without any
/// `reset` to keep in sync with the runtime — and the stack is the same the
/// AOT executable receives, so that deep recursion fails the same way.
///
/// After the entry comes `dartforge_runtime::abi::finalize_program`, the same
/// code the AOT's C `main` runs (uncaught exception → 255, collector
/// statistics). The code it returns is returned here; the endings the
/// runtime handles with `process::exit` during execution — non-null
/// assertion and heap ceiling (255, like the VM) — end **this process**, as in the
/// AOT. That is why tests and the differential harness run through a subprocess
/// (`dartforge-executar-ir`).
///
/// # Errors
/// Failure to create the thread, or if the thread ends in a panic outside the
/// `extern "C"` code (inside it a panic aborts the process, as in the AOT).
pub(crate) fn execute_program(address: u64, stack_bytes: usize) -> Result<(i32, Duration), String> {
    let address = usize::try_from(address).map_err(|_| "entry address does not fit in usize".to_owned())?;
    std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .stack_size(stack_bytes)
            .spawn_scoped(scope, move || {
                let started = Instant::now();
                // SAFETY: `address` came from `LLVMOrcLLJITLookup(dartforge_entry)`
                // in an `LLJIT` the caller keeps alive during the whole thread
                // (`Lljit::run_entry` takes `&self` and only returns after the
                // `join`), and `@dartforge_entry` is emitted with the C signature
                // `void(void)`.
                unsafe { std::mem::transmute::<usize, extern "C" fn()>(address)() };
                let code = dartforge_runtime::abi::finalize_program();
                (code, started.elapsed())
            })
            .map_err(|error| format!("could not create the thread of the program: {error}"))?;
        handle
            .join()
            .map_err(|_| "the program thread ended in a panic".to_owned())
    })
}

/// External names the IR may **declare** without the runtime defining them.
///
/// They are CRT functions the process already loads (`vcruntime140`/`ucrtbase`) and
/// that an LLVM IR emitter may call directly: memory copy and the
/// `double` math library. They are in an explicit list, and not in
/// «anything the process has», so that an unknown name fails
/// loudly instead of being resolved by chance to something homonymous. Symbols the
/// **code generator** introduces without appearing in the IR (`__chkstk`, `memcpy` of
/// intrinsics) do not go through this list: `LLJIT` resolves them in the process.
pub(crate) const CRT_SYMBOLS: &[&str] = &[
    "memcpy", "memmove", "memset", "memcmp", "fmod", "pow", "sqrt", "floor", "ceil", "trunc",
    "round", "exp", "log", "log10", "sin", "cos", "tan", "asin", "acos", "atan", "atan2",
];

/// Tells whether `name` is an external the session knows how to resolve without another module.
pub(crate) fn is_known_external(name: &str) -> bool {
    RUNTIME_SYMBOLS.contains(&name) || CRT_SYMBOLS.contains(&name) || name.starts_with("llvm.")
}

/// Value of `_fltused`, the marker MSVC requires from whoever uses floating point.
///
/// The COFF code generator references `_fltused` in every object that uses
/// `double`. In the AOT it comes from the static CRT linked to the executable; in a Rust
/// process it is not exported by any DLL, so the session publishes it. The value is
/// the same as the CRT's; only the address matters, nobody reads it.
static FLTUSED: i32 = 0x9875;

/// Data symbols the session publishes besides the runtime.
/// Only COFF references `_fltused`; on ELF and Mach-O the list is empty.
fn crt_data_symbols() -> Vec<(&'static CStr, u64)> {
    if cfg!(windows) { vec![(c"_fltused", std::ptr::addr_of!(FLTUSED) as u64)] } else { Vec::new() }
}

/// Version of the `LLVM-C.dll` actually loaded, `(major, minor, patch)`.
pub(crate) fn llvm_version() -> (u32, u32, u32) {
    let (mut major, mut minor, mut patch) = (0, 0, 0);
    // SAFETY: the three destinations are valid; the function only writes to them.
    unsafe { llvm_sys::core::LLVMGetVersion(&mut major, &mut minor, &mut patch) };
    (major, minor, patch)
}

// ─── Mutable module state and cached objects ─────────────────────────────

/// A mutable global of the module that must go back to zero before each
/// execution: name and size in bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MutableGlobal {
    /// Emitter name before the module/generation suffix.
    pub(crate) logical_name: String,
    pub(crate) name: String,
    pub(crate) size: usize,
}

impl ParsedModule {
    /// Prepares the mutable globals to be reset between executions.
    ///
    /// The emitter keeps lazy statics in module globals
    /// (`@dfg_<id>` and `@dfg_<id>_ok`, `internal`, initialized with zero). In a
    /// session that runs the same module several times, the second execution would see
    /// the statics already «initialized» with handles of the previous execution's heap,
    /// which no longer exists. Each execution must start from zero,
    /// like a new AOT process.
    ///
    /// So that the session can find those globals, their linkage changes from
    /// `internal` to external **in this copy** of the module. The emitted IR does not
    /// change. Only globals with a null initializer are accepted: that is what the emitter
    /// produces, and resetting is writing zeros. Any other form is refused with the
    /// name, instead of a silently wrong reset.
    pub(crate) fn prepare_mutable_globals(&self) -> Result<Vec<MutableGlobal>, String> {
        self.prepare_mutable_globals_with_suffix("")
    }

    /// The reloadable generation keeps a copy of the program's statics.
    /// The suffix avoids collisions between generations retained in the same `JITDylib`.
    pub(crate) fn version_mutable_globals(&self, suffix: &str) -> Result<Vec<MutableGlobal>, String> {
        self.prepare_mutable_globals_with_suffix(suffix)
    }

    fn prepare_mutable_globals_with_suffix(&self, suffix: &str) -> Result<Vec<MutableGlobal>, String> {
        use llvm_sys::LLVMLinkage;
        let mut globals = Vec::new();
        // SAFETY: traversal through the LLVM API over the live module, ended at the
        // first null; the linkage change is made before the module is
        // handed to `LLJIT` or emitted.
        unsafe {
            let mut global = llvm_sys::core::LLVMGetFirstGlobal(self.module);
            while !global.is_null() {
                let next = llvm_sys::core::LLVMGetNextGlobal(global);
                if LLVMIsDeclaration(global) == 0 && llvm_sys::core::LLVMIsGlobalConstant(global) == 0 {
                    let name = value_name(global);
                    let init = llvm_sys::core::LLVMGetInitializer(global);
                    if init.is_null() || llvm_sys::core::LLVMIsNull(init) == 0 {
                        return Err(format!(
                            "the mutable global @{name} does not start at zero; the session cannot reset it between executions"
                        ));
                    }
                    let kind = type_text(LLVMGlobalGetValueType(global));
                    let size = match kind.as_str() {
                        "i1" | "i8" => 1,
                        "i16" => 2,
                        "i32" | "float" => 4,
                        "i64" | "double" | "ptr" => 8,
                        // The cache of a call site by selector (source
                        // SDK, `llvm/seletores.rs`): two `i64`s.
                        "[2 x i64]" => 16,
                        other_one => {
                            return Err(format!(
                                "the mutable global @{name} has type {other_one}, which the session cannot reset"
                            ));
                        }
                    };
                    let linkage = llvm_sys::core::LLVMGetLinkage(global);
                    if matches!(linkage, LLVMLinkage::LLVMInternalLinkage | LLVMLinkage::LLVMPrivateLinkage) {
                        llvm_sys::core::LLVMSetLinkage(global, LLVMLinkage::LLVMExternalLinkage);
                    }
                    let logical_name = name.clone();
                    let name = if suffix.is_empty() { name } else {
                        let versioned = format!("{name}{suffix}");
                        llvm_sys::core::LLVMSetValueName2(global, versioned.as_ptr().cast::<c_char>(), versioned.len());
                        versioned
                    };
                    globals.push(MutableGlobal { logical_name, name, size });
                }
                global = next;
            }
        }
        Ok(globals)
    }
}

impl Lljit {
    /// The transfer of a static from the previous generation to the new one, which the
    /// publication applies before redirecting the stable entries — with the
    /// program stopped or at the safepoint, never in the middle of an event.
    pub(crate) fn global_copy(&self, previous: &MutableGlobal, next: &MutableGlobal) -> Result<GlobalCopy, String> {
        if previous.size != next.size {
            return Err(format!("the global {} changed from {} to {} bytes", previous.logical_name, previous.size, next.size));
        }
        let of = usize::try_from(self.lookup(&previous.name)?).map_err(|_| "global address".to_owned())?;
        let to = usize::try_from(self.lookup(&next.name)?).map_err(|_| "global address".to_owned())?;
        Ok(GlobalCopy { of, to, size: next.size })
    }

    /// Writes zeros into the mutable global, already materialized.
    ///
    /// # Errors
    /// Fails if the symbol cannot be resolved.
    pub(crate) fn zero_global(&self, global: &MutableGlobal) -> Result<(), String> {
        let address = self.lookup(&global.name)?;
        // SAFETY: `address` was resolved in this live `LLJIT` to a data global
        // with exactly `size` bytes (read from its type in the IR). No
        // generated code is running: `run_entry` only zeroes before creating the
        // program thread, and the session is borrowed through `&self`.
        unsafe {
            std::ptr::write_bytes(usize::try_from(address).unwrap_or(usize::MAX) as *mut u8, 0, global.size);
        }
        Ok(())
    }

    /// Hands an already compiled object to the `LLJIT`, under the given tracker.
    ///
    /// # Errors
    /// Propagates LLVM's message (invalid object, duplicate symbol).
    pub(crate) fn add_object(&self, tracker: &ResourceTracker, object: &[u8], name: &str) -> Result<(), String> {
        let name = CString::new(name).map_err(|_| "module name contains a null byte".to_owned())?;
        // SAFETY: the buffer is a copy of `object` and comes to belong to the
        // `LLJIT` in the next call, even when it returns an error.
        unsafe {
            let buffer = LLVMCreateMemoryBufferWithMemoryRangeCopy(
                object.as_ptr().cast::<c_char>(),
                object.len(),
                name.as_ptr(),
            );
            match take_error(llvm_sys::orc2::lljit::LLVMOrcLLJITAddObjectFileWithRT(
                self.handle,
                tracker.handle,
                buffer,
            )) {
                Some(message) => Err(message),
                None => Ok(()),
            }
        }
    }
}

/// What [`compile_object`] produces: the object and what the session needs to know
/// about it without the IR (externals, definitions, globals to reset, target).
pub(crate) struct ObjectParts {
    pub(crate) object: Vec<u8>,
    pub(crate) declarations: Vec<String>,
    pub(crate) signatures: Vec<FunctionSignature>,
    pub(crate) layouts: Vec<(i64, i64)>,
    pub(crate) globals: Vec<MutableGlobal>,
    pub(crate) target: (String, String),
}

/// Compiles textual IR into a COFF object with the JIT target machine, without a session.
///
/// It is the piece of the module cache: compile once and load the object into
/// as many sessions as desired ([`Lljit::add_object`]), without paying again for parsing
/// and code generation. The code is the same the session would generate, because the
/// target machine is the same ([`target_machine`]).
///
/// # Errors
/// Invalid IR (including by the verifier), a global that cannot be reset, or
/// a code generator failure.
pub(crate) fn compile_object(name: &str, ir: &str) -> Result<ObjectParts, String> {
    let parsed = parse_module(name, ir)?;
    let globals = parsed.prepare_mutable_globals()?;
    let declarations = parsed.declarations();
    let signatures = parsed.signatures();
    let layouts = parsed.class_layouts();
    let machine = target_machine()?;
    // Map-based roots: the cached object goes through the same pass the
    // session runs on modules (`transform_ir`).
    if roots_by_maps() {
        // SAFETY: the module belongs to `parsed`, alive; the machine is ours and
        // freed on error.
        if let Some(m) = take_error(unsafe { maps_pass(parsed.module, machine) }) {
            // SAFETY: the machine is ours and nobody else uses it.
            unsafe { LLVMDisposeTargetMachine(machine) };
            return Err(format!("the stack maps pass failed: {m}"));
        }
    }
    // SAFETY: `machine` is ours until the `Dispose` at the end; the module belongs to
    // `parsed`, alive during emission; the data layout created for a
    // module without a layout is freed right after being copied into it; the returned
    // buffer is copied and freed.
    let object = unsafe {
        if CStr::from_ptr(llvm_sys::core::LLVMGetDataLayoutStr(parsed.module)).to_bytes().is_empty() {
            let data = llvm_sys::target_machine::LLVMCreateTargetDataLayout(machine);
            llvm_sys::target::LLVMSetModuleDataLayout(parsed.module, data);
            llvm_sys::target::LLVMDisposeTargetData(data);
        }
        let mut message = ptr::null_mut();
        let mut buffer = ptr::null_mut();
        let has_failed = LLVMTargetMachineEmitToMemoryBuffer(
            machine,
            parsed.module,
            LLVMCodeGenFileType::LLVMObjectFile,
            &mut message,
            &mut buffer,
        ) != 0;
        let result = if has_failed {
            Err(if message.is_null() { "the code generator failed".to_owned() } else { take_message(message) })
        } else {
            let start = llvm_sys::core::LLVMGetBufferStart(buffer).cast::<u8>();
            let size = llvm_sys::core::LLVMGetBufferSize(buffer);
            let bytes = std::slice::from_raw_parts(start, size).to_vec();
            LLVMDisposeMemoryBuffer(buffer);
            Ok(bytes)
        };
        LLVMDisposeTargetMachine(machine);
        result
    }?;
    let target = parsed.target();
    Ok(ObjectParts { object, declarations, signatures, layouts, globals, target })
}


// ─── Map-based roots in the JIT (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §3.8) ───
//
// With `DARTFORGE_RAIZES=mapas`, the session:
//
// * generates code with minimal optimization (`CodeGenLevelLess`): `-O0` uses
//   FastISel, which has known defects with `gc.relocate` (§3.4);
// * runs `rewrite-statepoints-for-gc` and the verifier on each module, in the
//   IR transform layer of the `LLJIT` (and on the cached object,
//   [`compile_object`]);
// * links the objects through the RTDyld layer with its own memory manager
//   ([`JitObject`]): one region per object, code in one half and
//   data in the other (in COFF, `.pdata` and `.xdata` need 32-bit RVAs
//   relative to the image base, which RTDyld computes as the lowest address
//   of the sections). It finds the stack map section by name at allocation
//   (StackMaps.rst), registers it in the runtime after RTDyld applied the
//   relocations (the map stays in LLVM format, with the absolute addresses
//   of the functions), and removes it when the object's memory is freed — a retired
//   reload generation. On Windows x64 it also registers the object's `.pdata`
//   (`RtlAddFunctionTable`): without it the stack walk
//   (`RtlVirtualUnwind`) does not cross a JIT frame. On ELF and Mach-O
//   RTDyld registers `.eh_frame` with the process's unwinder.
//
// The registration goes to the runtime that collects: this process's
// ([`Lljit::use_local_registry`]) or the source SDK library's
// ([`Lljit::use_library_registry`]).

/// `DARTFORGE_RAIZES=mapas` in this process (the emitter reads the same variable,
/// `dartforge_emit_native::target::roots_by_maps`).
pub(crate) fn roots_by_maps() -> bool {
    static SIM: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SIM.get_or_init(|| std::env::var("DARTFORGE_RAIZES").is_ok_and(|v| v == "mapas"))
}

/// The symbolic stack trace (`DARTFORGE_RASTRO_VM=simbolico`, the same as
/// `emit_native::target::symbolic_trace`).
pub(crate) fn symbolic_trace() -> bool {
    static SIM: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SIM.get_or_init(|| std::env::var("DARTFORGE_RASTRO_VM").is_ok_and(|v| v == "simbolico"))
}

/// Registers the map section `[start, end)` in the runtime.
type RegisterFunction = unsafe extern "C" fn(*const u8, *const u8);
/// Removes the map registered with the same `start`.
type DeregisterFunction = unsafe extern "C" fn(*const u8);

/// What the session's object layer shares with each object: who
/// registers the maps. It lives as long as the session (the [`Lljit`] frees it after the
/// `LLJIT`, which frees the objects first).
struct LayerContext {
    rec: std::sync::Mutex<Option<(RegisterFunction, DeregisterFunction)>>,
    /// Registering and removing the stack trace section (§13.14).
    trace: std::sync::Mutex<Option<(RegisterFunction, DeregisterFunction)>>,
}

unsafe extern "C" fn register_local_trace(start: *const u8, end: *const u8) {
    // SAFETY: the object's relocated section, alive as long as it.
    unsafe { dartforge_runtime::abi::dartforge_register_stack_trace(start, end) };
}

unsafe extern "C" fn deregister_local_trace(start: *const u8) {
    dartforge_runtime::abi::dartforge_unregister_stack_trace(start);
}

unsafe extern "C" fn register_local(start: *const u8, end: *const u8) {
    dartforge_runtime::heap::register_llvm_section(start as usize, end as usize);
}

unsafe extern "C" fn deregister_local(start: *const u8) {
    dartforge_runtime::heap::unregister_map(start as usize);
}

/// The size of the region reserved for an object (code in the first
/// half, data in the second). Only reserved address space: the pages are
/// committed as the sections arrive.
const OBJECT_REGION: usize = 256 << 20;
const HALF_REGION: usize = OBJECT_REGION / 2;

/// System memory for the JIT code.
#[cfg(windows)]
mod virtual_memory {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn VirtualAlloc(address: *mut u8, size: usize, ty: u32, protection: u32) -> *mut u8;
        fn VirtualProtect(address: *mut u8, size: usize, protection: u32, old_value: *mut u32) -> i32;
        fn VirtualFree(address: *mut u8, size: usize, ty: u32) -> i32;
        fn FlushInstructionCache(process: *mut u8, address: *const u8, size: usize) -> i32;
        fn GetCurrentProcess() -> *mut u8;
    }
    const MEM_COMMIT: u32 = 0x1000;
    const MEM_RESERVE: u32 = 0x2000;
    const MEM_RELEASE: u32 = 0x8000;
    const PAGE_NOACCESS: u32 = 0x01;
    const PAGE_READWRITE: u32 = 0x04;
    const PAGE_EXECUTE_READ: u32 = 0x20;

    pub(super) fn page() -> usize {
        4096
    }

    /// Reserves `n` bytes of address space (null on failure).
    pub(super) fn reserve_space(n: usize) -> *mut u8 {
        // SAFETY: only reserves new address space.
        unsafe { VirtualAlloc(std::ptr::null_mut(), n, MEM_RESERVE, PAGE_NOACCESS) }
    }

    /// Commits `[p, p + n)`, already reserved, for reading and writing.
    ///
    /// # Safety
    /// `[p, p + n)` is in a region from [`reserve_space`].
    pub(super) unsafe fn confirm(p: *mut u8, n: usize) -> bool {
        // SAFETY: the function contract.
        !unsafe { VirtualAlloc(p, n, MEM_COMMIT, PAGE_READWRITE) }.is_null()
    }

    /// Makes `[p, p + n)` executable and read-only, and flushes its instruction
    /// cache.
    ///
    /// # Safety
    /// `[p, p + n)` is committed.
    pub(super) unsafe fn make_executable(p: *mut u8, n: usize) -> bool {
        let mut old_value = 0u32;
        // SAFETY: the function contract.
        unsafe { VirtualProtect(p, n, PAGE_EXECUTE_READ, &mut old_value) != 0 && FlushInstructionCache(GetCurrentProcess(), p, n) != 0 }
    }

    /// Frees the whole region.
    ///
    /// # Safety
    /// `p` is the start of a region from [`reserve_space`], which nothing else uses.
    pub(super) unsafe fn release(p: *mut u8, _n: usize) {
        // SAFETY: the function contract.
        unsafe { VirtualFree(p, 0, MEM_RELEASE) };
    }
}

/// System memory for the JIT code.
#[cfg(unix)]
mod virtual_memory {
    unsafe extern "C" {
        fn mmap(address: *mut u8, size: usize, protection: i32, flag_bits: i32, file: i32, offset: i64) -> *mut u8;
        fn mprotect(address: *mut u8, size: usize, protection: i32) -> i32;
        fn munmap(address: *mut u8, size: usize) -> i32;
        fn sysconf(name: i32) -> i64;
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    unsafe extern "C" {
        fn __clear_cache(start: *mut u8, end: *mut u8);
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    unsafe extern "C" {
        fn sys_icache_invalidate(start: *mut u8, size: usize);
    }
    const PROT_READ: i32 = 1;
    const PROT_WRITE: i32 = 2;
    const PROT_EXEC: i32 = 4;
    const MAP_PRIVATE: i32 = 2;
    const MAP_ANON: i32 = if cfg!(target_os = "macos") { 0x1000 } else { 0x20 };
    /// Without reserving swap for the whole region (Linux only).
    const MAP_NORESERVE: i32 = if cfg!(target_os = "linux") { 0x4000 } else { 0 };
    /// `_SC_PAGESIZE`.
    const SC_PAGESIZE: i32 = if cfg!(target_os = "macos") { 29 } else { 30 };

    pub(super) fn page() -> usize {
        // SAFETY: only queries the system.
        let n = unsafe { sysconf(SC_PAGESIZE) };
        if n > 0 { n as usize } else { 4096 }
    }

    /// Maps `n` bytes for reading and writing, without committing the pages
    /// (null on failure).
    pub(super) fn reserve_space(n: usize) -> *mut u8 {
        // SAFETY: new anonymous mapping.
        let p = unsafe { mmap(std::ptr::null_mut(), n, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON | MAP_NORESERVE, -1, 0) };
        if p as isize == -1 { std::ptr::null_mut() } else { p }
    }

    /// The pages of an anonymous mapping are born when touched.
    ///
    /// # Safety
    /// None.
    pub(super) unsafe fn confirm(_p: *mut u8, _n: usize) -> bool {
        true
    }

    /// Makes `[p, p + n)` executable and read-only, and flushes its instruction
    /// cache (on aarch64; x86-64 is coherent).
    ///
    /// # Safety
    /// `[p, p + n)` is mapped and page-aligned.
    pub(super) unsafe fn make_executable(p: *mut u8, n: usize) -> bool {
        // SAFETY: the function contract.
        let ok = unsafe { mprotect(p, n, PROT_READ | PROT_EXEC) } == 0;
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        // SAFETY: the range was just written and protected.
        unsafe {
            __clear_cache(p, p.add(n))
        };
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        // SAFETY: the range was just written and protected.
        unsafe {
            sys_icache_invalidate(p, n)
        };
        ok
    }

    /// Unmaps the whole mapping.
    ///
    /// # Safety
    /// `[p, p + n)` is a mapping from [`reserve_space`], which nothing else uses.
    pub(super) unsafe fn release(p: *mut u8, n: usize) {
        // SAFETY: the function contract.
        unsafe { munmap(p, n) };
    }
}

#[cfg(all(windows, target_arch = "x86_64"))]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn RtlAddFunctionTable(table: *const u8, n: u32, base: u64) -> u8;
    fn RtlDeleteFunctionTable(table: *const u8) -> u8;
}

/// The memory of an object loaded by RTDyld (the per-object context of the
/// layer's memory manager).
struct JitObject {
    layer: *const LayerContext,
    /// The start of the region (null if the reservation failed: every allocation fails).
    base: *mut u8,
    /// The end of the code and that of the data, as offsets relative to `base`.
    code: usize,
    data: usize,
    /// The stack map section and the `.pdata` one (COFF).
    map: Option<(*const u8, usize)>,
    #[cfg_attr(not(all(windows, target_arch = "x86_64")), allow(dead_code))]
    pdata: Option<(*const u8, usize)>,
    registered_map: bool,
    #[cfg_attr(not(all(windows, target_arch = "x86_64")), allow(dead_code))]
    registered_table: bool,
    /// The stack trace sections (`.dfpcl$m`, `dfpcl`, `__dfpcl`; on ELF, one
    /// per `comdat` group).
    trace: Vec<(*const u8, usize)>,
    registered_trace: bool,
    finished: bool,
}

impl JitObject {
    fn new(layer: *const LayerContext) -> Self {
        JitObject {
            layer,
            base: virtual_memory::reserve_space(OBJECT_REGION),
            code: 0,
            data: HALF_REGION,
            map: None,
            pdata: None,
            registered_map: false,
            registered_table: false,
            trace: Vec::new(),
            registered_trace: false,
            finished: false,
        }
    }

    /// A section of `size` bytes aligned to `alignment`, in the code half
    /// or in the data half; null if it does not fit.
    fn allocate(&mut self, size: usize, alignment: usize, code: bool) -> *mut u8 {
        if self.base.is_null() {
            return ptr::null_mut();
        }
        let alignment = alignment.max(16).next_power_of_two();
        let (cursor, limit) = if code { (&mut self.code, HALF_REGION) } else { (&mut self.data, OBJECT_REGION) };
        let start = (*cursor + alignment - 1) & !(alignment - 1);
        let Some(end) = start.checked_add(size.max(1)) else { return ptr::null_mut() };
        if end > limit {
            return ptr::null_mut();
        }
        let page = virtual_memory::page();
        let of = start & !(page - 1);
        let until = (end + page - 1) & !(page - 1);
        // SAFETY: `[of, until)` is inside the reserved region.
        if !unsafe { virtual_memory::confirm(self.base.add(of), until - of) } {
            return ptr::null_mut();
        }
        *cursor = end;
        // SAFETY: `start` is inside the region.
        unsafe { self.base.add(start) }
    }

    /// After the relocations: the code becomes executable, `.pdata` enters the
    /// system function table (Windows x64) and the map is registered.
    fn finalize(&mut self) -> Result<(), String> {
        if self.finished {
            return Ok(());
        }
        self.finished = true;
        if self.code > 0 {
            let page = virtual_memory::page();
            let n = (self.code + page - 1) & !(page - 1);
            // SAFETY: `[base, base + n)` is this object's committed code.
            if !unsafe { virtual_memory::make_executable(self.base, n) } {
                return Err("could not make the JIT code executable".to_owned());
            }
        }
        #[cfg(all(windows, target_arch = "x86_64"))]
        if let Some((p, n)) = self.pdata
            && n >= 12
        {
            // SAFETY: the object's relocated `.pdata`; its RVAs are relative to the
            // region base (the lowest address of the sections, as RTDyld
            // computes it: the first code section starts there).
            if unsafe { RtlAddFunctionTable(p, (n / 12) as u32, self.base as u64) } == 0 {
                return Err("the system refused the function table of the JIT object".to_owned());
            }
            self.registered_table = true;
        }
        if let Some((p, n)) = self.map {
            // SAFETY: the layer outlives its objects.
            let layer = unsafe { &*self.layer };
            let rec = *layer.rec.lock().unwrap_or_else(|e| e.into_inner());
            let Some((register, _)) = rec else {
                return Err("map-based roots in the JIT without the runtime registration".to_owned());
            };
            // SAFETY: the map section, relocated and alive as long as the object.
            unsafe { register(p, p.add(n)) };
            self.registered_map = true;
        }
        if !self.trace.is_empty() {
            // SAFETY: the layer outlives its objects.
            let layer = unsafe { &*self.layer };
            if let Some((register, _)) = *layer.trace.lock().unwrap_or_else(|e| e.into_inner()) {
                for &(p, n) in &self.trace {
                    // SAFETY: the stack trace section, relocated and alive as long as the
                    // object.
                    unsafe { register(p, p.add(n)) };
                }
                self.registered_trace = true;
            }
        }
        Ok(())
    }
}

impl Drop for JitObject {
    fn drop(&mut self) {
        if self.registered_trace {
            // SAFETY: the layer outlives its objects.
            let layer = unsafe { &*self.layer };
            if let Some((_, deregister)) = *layer.trace.lock().unwrap_or_else(|e| e.into_inner()) {
                for &(p, _) in &self.trace {
                    // SAFETY: the section was registered with this start.
                    unsafe { deregister(p) };
                }
            }
        }
        if self.registered_map
            && let Some((p, _)) = self.map
        {
            // SAFETY: the layer outlives its objects.
            let layer = unsafe { &*self.layer };
            if let Some((_, deregister)) = *layer.rec.lock().unwrap_or_else(|e| e.into_inner()) {
                // SAFETY: the map was registered with this start.
                unsafe { deregister(p) };
            }
        }
        #[cfg(all(windows, target_arch = "x86_64"))]
        if self.registered_table
            && let Some((p, _)) = self.pdata
        {
            // SAFETY: the table was registered with this address.
            unsafe { RtlDeleteFunctionTable(p) };
        }
        if !self.base.is_null() {
            // SAFETY: this object's region, which RTDyld already freed.
            unsafe { virtual_memory::release(self.base, OBJECT_REGION) };
        }
    }
}

/// `CreateContext`: one context per object. The signature is that of
/// `llvm-c/OrcEE.h` (returns the context); `llvm-sys` declares the type without the
/// return value, and the conversion below fixes it.
extern "C" fn create_context(layer: *mut std::ffi::c_void) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(JitObject::new(layer.cast::<LayerContext>()))).cast()
}

extern "C" fn notify_end(_layer: *mut std::ffi::c_void) {}

extern "C" fn allocate_code(opaque: *mut std::ffi::c_void, size: usize, alignment: u32, _id: u32, _name: *const c_char) -> *mut u8 {
    // SAFETY: `opaque` is the `JitObject` from `create_context`, alive until `destroy`.
    let o = unsafe { &mut *opaque.cast::<JitObject>() };
    o.allocate(size, alignment as usize, true)
}

extern "C" fn allocate_data(
    opaque: *mut std::ffi::c_void,
    size: usize,
    alignment: u32,
    _id: u32,
    name: *const c_char,
    _read_only: llvm_sys::prelude::LLVMBool,
) -> *mut u8 {
    // SAFETY: `opaque` is the `JitObject` from `create_context`, alive until `destroy`.
    let o = unsafe { &mut *opaque.cast::<JitObject>() };
    let p = o.allocate(size, alignment as usize, false);
    if !p.is_null() && !name.is_null() {
        // SAFETY: the section name, zero-terminated, alive during the call.
        let n = unsafe { CStr::from_ptr(name) }.to_bytes();
        if n == b".llvm_stackmaps" || n == b"__llvm_stackmaps" {
            o.map = Some((p, size));
        } else if n == b".pdata" {
            o.pdata = Some((p, size));
        } else if n == b".dfpcl$m" || n == b"dfpcl" || n == b"__dfpcl" {
            o.trace.push((p, size));
        }
    }
    p
}

extern "C" fn finalize_memory(opaque: *mut std::ffi::c_void, error: *mut *mut c_char) -> llvm_sys::prelude::LLVMBool {
    // SAFETY: `opaque` is the `JitObject` from `create_context`, alive until `destroy`.
    let o = unsafe { &mut *opaque.cast::<JitObject>() };
    match o.finalize() {
        Ok(()) => 0,
        Err(m) => {
            if !error.is_null() {
                let c = CString::new(m).unwrap_or_default();
                // SAFETY: LLVM frees the message with `free`; the one from
                // `LLVMCreateMessage` comes from `strdup`.
                unsafe { *error = llvm_sys::core::LLVMCreateMessage(c.as_ptr()) };
            }
            1
        }
    }
}

extern "C" fn destroy(opaque: *mut std::ffi::c_void) {
    // SAFETY: `opaque` came from `Box::into_raw` in `create_context` and is freed
    // once, here.
    drop(unsafe { Box::from_raw(opaque.cast::<JitObject>()) });
}

unsafe extern "C" {
    /// The function as LLVM 22's C header declares it (`llvm-c/OrcEE.h`):
    /// `llvm-sys` 221 declares it without `CreateContextCtx` (the context
    /// `CreateContext` receives) and with `CreateContext` without a return value.
    #[link_name = "LLVMOrcCreateRTDyldObjectLinkingLayerWithMCJITMemoryManagerLikeCallbacks"]
    fn create_rtdyld_layer_with_manager(
        session: llvm_sys::orc2::LLVMOrcExecutionSessionRef,
        creation_context: *mut std::ffi::c_void,
        create_context: extern "C" fn(*mut std::ffi::c_void) -> *mut std::ffi::c_void,
        notify_end: extern "C" fn(*mut std::ffi::c_void),
        allocate_code: llvm_sys::execution_engine::LLVMMemoryManagerAllocateCodeSectionCallback,
        allocate_data: llvm_sys::execution_engine::LLVMMemoryManagerAllocateDataSectionCallback,
        finalize_memory: llvm_sys::execution_engine::LLVMMemoryManagerFinalizeMemoryCallback,
        destroy: llvm_sys::execution_engine::LLVMMemoryManagerDestroyCallback,
    ) -> llvm_sys::orc2::LLVMOrcObjectLayerRef;
}

/// The session's object layer with maps: RTDyld with the memory manager
/// above.
extern "C" fn create_layer(
    layer: *mut std::ffi::c_void,
    session: llvm_sys::orc2::LLVMOrcExecutionSessionRef,
    _triple: *const c_char,
) -> llvm_sys::orc2::LLVMOrcObjectLayerRef {
    // SAFETY: the C header signature (above); `layer` is the session's context,
    // alive until its `Drop`, and the functions live for the whole process.
    unsafe {
        create_rtdyld_layer_with_manager(
            session,
            layer,
            create_context,
            notify_end,
            allocate_code,
            allocate_data,
            finalize_memory,
            Some(destroy),
        )
    }
}

/// The maps pass and the verifier over a module.
///
/// # Safety
/// `module` is a live module; `llvm_machine` is null or a live target machine.
unsafe fn maps_pass(module: LLVMModuleRef, llvm_machine: LLVMTargetMachineRef) -> LLVMErrorRef {
    // SAFETY: the function contract; the options are created and freed here.
    unsafe {
        let options = llvm_sys::transforms::pass_builder::LLVMCreatePassBuilderOptions();
        let error = llvm_sys::transforms::pass_builder::LLVMRunPasses(module, c"rewrite-statepoints-for-gc,verify".as_ptr(), llvm_machine, options);
        llvm_sys::transforms::pass_builder::LLVMDisposePassBuilderOptions(options);
        error
    }
}

extern "C" fn pass_in_module(_ctx: *mut std::ffi::c_void, module: LLVMModuleRef) -> LLVMErrorRef {
    // SAFETY: the module of the `ThreadSafeModule`, with the context locked.
    unsafe { maps_pass(module, ptr::null_mut()) }
}

/// The IR transform of the session with maps: the maps pass on each
/// module before code generation.
extern "C" fn transform_ir(
    _ctx: *mut std::ffi::c_void,
    module: *mut LLVMOrcThreadSafeModuleRef,
    _responsibility: llvm_sys::orc2::LLVMOrcMaterializationResponsibilityRef,
) -> LLVMErrorRef {
    // SAFETY: `module` points to the `ThreadSafeModule` being materialized.
    unsafe { llvm_sys::orc2::LLVMOrcThreadSafeModuleWithModuleDo(*module, pass_in_module, ptr::null_mut()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The generated table covers exactly the announced names, with real addresses.
    #[test]
    fn symbol_table_matches_the_announced_names() {
        let addresses = runtime_symbol_addresses();
        assert_eq!(addresses.len(), RUNTIME_SYMBOLS.len());
        for (index, (name, address)) in addresses.iter().enumerate() {
            assert_eq!(*name, RUNTIME_SYMBOLS[index]);
            assert_ne!(*address, 0);
        }
        assert!(!RUNTIME_SYMBOLS.contains(&"main"));
        assert!(!RUNTIME_SYMBOLS.contains(&"dartforge_entry"));
    }
    /// A `linkonce_odr` function in a `comdat` (tear-off entry of the source
    /// SDK) versioned by the reload still generates a COFF object (the
    /// Windows one on the host architecture, the only one LLVM initializes here):
    /// the `comdat` follows the new name. Before, the name `f$g1` in a `comdat` `f`
    /// without a key aborted LLVM on Windows ("Associative COMDAT symbol
    /// 'f$g1' is not a key for its COMDAT"), and ELF reported nothing.
    #[test]
    fn function_version_in_comdat_produces_coff_object() {
        let (triple, cpu) = if cfg!(target_arch = "x86_64") {
            (c"x86_64-pc-windows-msvc", c"x86-64")
        } else {
            (c"aarch64-pc-windows-msvc", c"generic")
        };
        let ir = "$\"C.m$tearm\" = comdat any\n\
                  define linkonce_odr i64 @\"C.m$tearm\"(i64 %x) comdat {\n  ret i64 %x\n}\n\
                  define i64 @usa(i64 %x) {\n  %r = call i64 @\"C.m$tearm\"(i64 %x)\n  ret i64 %r\n}\n";
        let parsed = parse_module("comdat", ir).expect("IR");
        let fn_signatures = parsed.version_definitions("$g1");
        assert_eq!(fn_signatures.len(), 2);
        initialize_native_target().expect("alvo");
        // SAFETY: triple, machine and buffer are created and freed here; the
        // module belongs to `parsed`, alive during emission.
        unsafe {
            let mut target = ptr::null_mut();
            let mut message = ptr::null_mut();
            assert_eq!(LLVMGetTargetFromTriple(triple.as_ptr(), &mut target, &mut message), 0, "no target {triple:?}");
            llvm_sys::core::LLVMSetTarget(parsed.module, triple.as_ptr());
            let machine = LLVMCreateTargetMachine(
                target,
                triple.as_ptr(),
                cpu.as_ptr(),
                c"".as_ptr(),
                LLVMCodeGenOptLevel::LLVMCodeGenLevelNone,
                LLVMRelocMode::LLVMRelocDefault,
                LLVMCodeModel::LLVMCodeModelDefault,
            );
            assert!(!machine.is_null());
            let mut buffer = ptr::null_mut();
            let has_failed = LLVMTargetMachineEmitToMemoryBuffer(
                machine,
                parsed.module,
                LLVMCodeGenFileType::LLVMObjectFile,
                &mut message,
                &mut buffer,
            ) != 0;
            LLVMDisposeTargetMachine(machine);
            assert!(!has_failed, "{}", take_message(message));
            LLVMDisposeMemoryBuffer(buffer);
        }
    }

    /// Externals: runtime, listed CRT and intrinsics pass; the rest does not.
    #[test]
    fn only_listed_externals_are_known() {
        assert!(is_known_external("dartforge_print_i64"));
        assert!(is_known_external("fmod"));
        assert!(is_known_external("llvm.memcpy.p0.p0.i64"));
        assert!(!is_known_external("sqlite3_open"));
        assert!(!is_known_external("dartforge_entry"));
    }
}
