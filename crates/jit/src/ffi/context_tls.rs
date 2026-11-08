//! Replace the emitter's private AOT context cache with its runtime getter.
//!
//! RuntimeDyld cannot materialize ELF native TLS (notably R_X86_64_TLSLD).
//! Match the verified instruction graph, including its only TLS use, before
//! removing this cache. Other TLS globals and altered helpers stay untouched.

use llvm_sys::core::*;
use llvm_sys::prelude::{LLVMBasicBlockRef, LLVMModuleRef, LLVMValueRef};
use llvm_sys::{LLVMAtomicOrdering, LLVMIntPredicate, LLVMLinkage, LLVMOpcode, LLVMTypeKind};

/// The module has already passed LLVM's verifier and remains caller-owned.
pub(super) fn lower(module: LLVMModuleRef) {
    // SAFETY: the verified module owns every inspected value. The checks below
    // establish instruction kinds before using kind-specific LLVM accessors.
    unsafe {
        let helper = LLVMGetNamedFunction(module, c"df.context".as_ptr());
        let runtime = LLVMGetNamedFunction(module, c"dartforge_context".as_ptr());
        let intrinsic = LLVMGetNamedFunction(module, c"llvm.threadlocal.address.p0".as_ptr());
        if helper.is_null()
            || runtime.is_null()
            || intrinsic.is_null()
            || LLVMGetLinkage(helper) != LLVMLinkage::LLVMInternalLinkage
            || LLVMIsDeclaration(runtime) == 0
            || LLVMGlobalGetValueType(helper) != LLVMGlobalGetValueType(runtime)
            || LLVMGetTypeKind(LLVMGetReturnType(LLVMGlobalGetValueType(helper)))
                != LLVMTypeKind::LLVMPointerTypeKind
            || LLVMCountParams(helper) != 0
            || LLVMGetFunctionCallConv(helper) != 0
            || LLVMGetFunctionCallConv(runtime) != 0
        {
            return;
        }
        let entry = LLVMGetFirstBasicBlock(helper);
        if entry.is_null() {
            return;
        }
        let slow = LLVMGetNextBasicBlock(entry);
        if slow.is_null() {
            return;
        }
        let ready = LLVMGetNextBasicBlock(slow);
        if ready.is_null() || !LLVMGetNextBasicBlock(ready).is_null() {
            return;
        }
        let first = instructions(entry);
        let second = instructions(slow);
        let third = instructions(ready);
        let ([address, cached, empty, branch], [fresh, store, slow_return], [ready_return]) =
            (first.as_slice(), second.as_slice(), third.as_slice())
        else {
            return;
        };
        let (address, cached, empty, branch, fresh, store, slow_return, ready_return) = (
            *address,
            *cached,
            *empty,
            *branch,
            *fresh,
            *store,
            *slow_return,
            *ready_return,
        );
        let expected = [
            LLVMOpcode::LLVMCall,
            LLVMOpcode::LLVMLoad,
            LLVMOpcode::LLVMICmp,
            LLVMOpcode::LLVMBr,
            LLVMOpcode::LLVMCall,
            LLVMOpcode::LLVMStore,
            LLVMOpcode::LLVMRet,
            LLVMOpcode::LLVMRet,
        ];
        if [
            address,
            cached,
            empty,
            branch,
            fresh,
            store,
            slow_return,
            ready_return,
        ]
        .into_iter()
        .zip(expected)
        .any(|(value, opcode)| LLVMGetInstructionOpcode(value) != opcode)
        {
            return;
        }
        if LLVMGetCalledValue(address) != intrinsic
            || LLVMGetNumArgOperands(address) != 1
            || LLVMGetNumOperands(address) != 2
            || LLVMGetInstructionCallConv(address) != 0
            || LLVMGetCalledValue(fresh) != runtime
            || LLVMGetNumArgOperands(fresh) != 0
            || LLVMGetNumOperands(fresh) != 1
            || LLVMGetInstructionCallConv(fresh) != 0
            || LLVMGetNumSuccessors(branch) != 2
            || LLVMGetNumOperands(slow_return) != 1
            || LLVMGetNumOperands(ready_return) != 1
        {
            return;
        }
        let global = LLVMGetOperand(address, 0);
        if global.is_null()
            || ![c"df.ctx.tls.program", c"df.ctx.tls"]
                .into_iter()
                .any(|name| LLVMGetNamedGlobal(module, name.as_ptr()) == global)
            || LLVMGetLinkage(global) != LLVMLinkage::LLVMInternalLinkage
            || LLVMIsThreadLocal(global) == 0
            || LLVMIsExternallyInitialized(global) != 0
            || LLVMGetTypeKind(LLVMGlobalGetValueType(global)) != LLVMTypeKind::LLVMPointerTypeKind
            || LLVMGetInitializer(global).is_null()
            || LLVMIsNull(LLVMGetInitializer(global)) == 0
        {
            return;
        }
        let usage = LLVMGetFirstUse(global);
        if usage.is_null()
            || LLVMGetUser(usage) != address
            || !LLVMGetNextUse(usage).is_null()
            || LLVMGetOperand(cached, 0) != address
            || LLVMGetVolatile(cached) != 0
            || LLVMGetVolatile(store) != 0
            || LLVMGetOrdering(cached) != LLVMAtomicOrdering::LLVMAtomicOrderingNotAtomic
            || LLVMGetOrdering(store) != LLVMAtomicOrdering::LLVMAtomicOrderingNotAtomic
            || LLVMGetICmpPredicate(empty) != LLVMIntPredicate::LLVMIntEQ
            || LLVMGetOperand(empty, 0) != cached
            || LLVMIsNull(LLVMGetOperand(empty, 1)) == 0
            || LLVMGetCondition(branch) != empty
            || LLVMGetSuccessor(branch, 0) != slow
            || LLVMGetSuccessor(branch, 1) != ready
            || LLVMGetOperand(store, 0) != fresh
            || LLVMGetOperand(store, 1) != address
            || LLVMGetOperand(slow_return, 0) != fresh
            || LLVMGetOperand(ready_return, 0) != cached
        {
            return;
        }
        // Replacing a helper address would change observable function identity.
        // Compiler-owned context helpers are used only as direct C calls.
        let mut usage = LLVMGetFirstUse(helper);
        while !usage.is_null() {
            let user = LLVMGetUser(usage);
            if LLVMIsACallInst(user).is_null()
                || LLVMGetCalledValue(user) != helper
                || LLVMGetNumArgOperands(user) != 0
                || LLVMGetNumOperands(user) != 1
                || LLVMGetInstructionCallConv(user) != 0
            {
                return;
            }
            usage = LLVMGetNextUse(usage);
        }
        LLVMReplaceAllUsesWith(helper, runtime);
        LLVMDeleteFunction(helper);
        LLVMDeleteGlobal(global);
    }
}

unsafe fn instructions(block: LLVMBasicBlockRef) -> Vec<LLVMValueRef> {
    // SAFETY: the caller supplies a live block from its verified module.
    unsafe {
        let mut values = Vec::new();
        let mut value = LLVMGetFirstInstruction(block);
        while !value.is_null() {
            values.push(value);
            value = LLVMGetNextInstruction(value);
        }
        values
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ir() -> String {
        format!(
            "{}\ndefine i64 @check_context() {{\n %a = call ptr @df.context()\n %b = call ptr @dartforge_context()\n %same = icmp eq ptr %a, %b\n %result = zext i1 %same to i64\n ret i64 %result\n}}\n",
            dartforge_emit_native::sdk_module::with_context_in_tls(
                "declare ptr @dartforge_context()\ndefine ptr @placeholder() {\n %c = call ptr @dartforge_context()\n ret ptr %c\n}\n",
                dartforge_emit_native::sdk_module::ContextInTls::Inline,
                "df.ctx.tls.program",
                false
            )
        )
    }

    // Always use the ELF/COFF-style helper, even on ARM64 macOS where the
    // normal emitter uses a pthread slot instead of a native TLS global.
    fn native_ir() -> String {
        let emitted = ir();
        let start = emitted.find("define internal ptr @df.context()").unwrap();
        let end = emitted[start..].find("\n}\n").unwrap() + start + 3;
        let body = "define internal ptr @df.context() nounwind alwaysinline {\n %p = call ptr @llvm.threadlocal.address.p0(ptr @df.ctx.tls.program)\n %c = load ptr, ptr %p, align 8\n %z = icmp eq ptr %c, null\n br i1 %z, label %slow, label %ready\nslow:\n %n = call ptr @dartforge_context()\n store ptr %n, ptr %p, align 8\n ret ptr %n\nready:\n ret ptr %c\n}\n";
        let mut text = emitted[..start].to_owned();
        text.push_str(body);
        text.push_str(&emitted[end..]);
        if !text.contains("@df.ctx.tls.program = internal thread_local") {
            text.push_str("@df.ctx.tls.program = internal thread_local global ptr null, align 8\n");
        }
        if !text.contains("declare nonnull ptr @llvm.threadlocal.address.p0") {
            text.push_str("declare ptr @llvm.threadlocal.address.p0(ptr)\n");
        }
        text
    }

    #[test]
    fn canonical_aot_context_cache_runs_without_native_tls_relocations() {
        let text = native_ir();
        let parsed = crate::ffi::parse_module("context", &text).unwrap();
        // SAFETY: parsed owns the inspected module.
        unsafe {
            assert!(LLVMGetNamedGlobal(parsed.module, c"df.ctx.tls.program".as_ptr()).is_null());
            assert!(LLVMGetNamedFunction(parsed.module, c"df.context".as_ptr()).is_null());
        }
        let mut session = crate::JitSession::new().unwrap();
        session.add_ir_module("context", &text).unwrap();
        let address = session.lookup("check_context").unwrap();
        // SAFETY: check_context has the checked no-argument i64 signature and
        // its owning session remains alive throughout the call.
        let call: unsafe extern "C" fn() -> i64 = unsafe { std::mem::transmute(address as usize) };
        assert_eq!(unsafe { call() }, 1);
    }

    #[test]
    fn modified_or_shared_context_cache_is_preserved() {
        let original = native_ir();
        for text in [
            original.replace("icmp eq ptr %c, null", "icmp ne ptr %c, null"),
            original.replace("load ptr, ptr %p", "load volatile ptr, ptr %p"),
            original.replace("store ptr %n, ptr %p", "store ptr null, ptr %p"),
            original.replace("internal thread_local global", "thread_local global"),
            format!("{original}\ndefine ptr @expose() {{ ret ptr @df.ctx.tls.program }}\n"),
            format!("{original}\ndefine ptr @expose_helper() {{ ret ptr @df.context }}\n"),
        ] {
            let parsed = crate::ffi::parse_module("changed-context", &text).unwrap();
            // SAFETY: parsed owns the inspected module.
            unsafe {
                assert!(
                    !LLVMGetNamedGlobal(parsed.module, c"df.ctx.tls.program".as_ptr()).is_null()
                );
                assert!(!LLVMGetNamedFunction(parsed.module, c"df.context".as_ptr()).is_null());
            }
        }
    }
}
