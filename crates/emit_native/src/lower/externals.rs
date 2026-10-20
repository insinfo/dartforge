//! `external`s of the libraries compiled from source (P6): the body is the call
//! to the runtime native.
//!
//! In the VM, an `external` with `@pragma("vm:external-name", "Name")` is a
//! C++ function of the runtime (`runtime/lib/*.cc`). Here it is the Rust function
//! `dartforge_native_<Name>` (the `crate::natives` table, from δ), with the
//! signature in the HIR representation of the declared types (`int` → `i64`,
//! `double` → `double`, `bool` → `i8` — the C ABI `bool` —, the rest,
//! receiver included, → `Ref`). The error leaves through the pending exception.
//!
//! A native that the table still marks `Pending` is a diagnostic (N1) — it only
//! counts if the pruning keeps the function (`source.rs`).

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_diagnostics::Span;

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// The body of an `external` with a native: the parameters (and `this`) go to
    /// `dartforge_native_<Name>` and the result comes back in the representation of the
    /// return. Returns `false` if the function has no native.
    pub fn lower_external(&mut self, fid: usize, span: Span) -> bool {
        let (native, recognized) =
            crate::source::native_pragma(self.ctx.program, self.ctx.interner, fid);
        if (recognized || native.is_some()) && self.lower_intrinsic(fid) {
            return true;
        }
        let Some(name) = native else {
            return false;
        };
        match crate::natives::native(&name) {
            Some(n) if n.state == crate::natives::State::Runtime => {}
            _ => {
                self.unsupported(
                    &format!("native `{name}` without an implementation in the runtime"),
                    span,
                );
                self.terminate(Terminator::Return(None));
                return true;
            }
        }
        let mut args: Vec<(Operand, Type)> = Vec::new();
        for (vid, _, ty) in self.func.params.clone() {
            let abi = if ty == Type::I1 { Type::I8 } else { ty };
            let op = self.coerce(Operand::Val(vid), abi);
            args.push((op, abi));
        }
        let ret = self.func.return_ty;
        // A native that the SDK declares with more than one return type always
        // returns the widest shape (`Ref`); the declared type comes from the coercion.
        let ret_abi = if crate::natives::RETURN_REF.contains(&name.as_str()) && ret != Type::Void {
            Type::Ref
        } else if ret == Type::I1 {
            Type::I8
        } else {
            ret
        };
        let r = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: crate::natives::symbol(&name),
                args,
                ret_ty: ret_abi,
            },
            ret_abi,
        );
        if ret == Type::Void {
            self.terminate(Terminator::Return(None));
        } else {
            let r = self.coerce(r, ret);
            self.terminate(Terminator::Return(Some(r)));
        }
        true
    }

    /// Intrinsics (`@pragma("vm:recognized")` without a native) that the source code
    /// uses: in the VM the compiler generates them in place of the call; here they are
    /// a short body. `unsafeCast<T>(v)` is `v` (the caller guarantees the type).
    fn lower_intrinsic(&mut self, fid: usize) -> bool {
        let f = &self.ctx.program.functions[fid];
        let lib = &self.ctx.program.library(f.library).uri;
        let name = self.ctx.symbol_name(f.name);
        match (lib.as_str(), name) {
            ("dart:_internal", "unsafeCast") => {
                let Some(&(v, _, _)) = self.func.params.first() else {
                    return false;
                };
                let ret = self.func.return_ty;
                let r = self.coerce(Operand::Val(v), ret);
                self.terminate(Terminator::Return(Some(r)));
                true
            }
            _ => false,
        }
    }
}
