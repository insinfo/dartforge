//! The string natives done inline (P1, docs/NATIVO-ESPACO-UNIFICADO.md
//! §3.5, §3.8 and §4.3): the reads and writes the VM does as intrinsics.
//!
//! Every `String` is a `_OneByteString`/`_TwoByteString` block of the object
//! space: the length, the code units and the hash are at fixed offsets from the
//! handle, and the generated code reads them through the `@df.string_*` helpers
//! (`llvm/ir_strings.rs`, `alwaysinline`), without a runtime call:
//!
//! * `length` (`String_getLength`) → `@df.string_len`;
//! * `codeUnitAt` → `@df.string_len` and `@df.string_code_unit` with the bounds
//!   test; out of range, the native (`DartForge_string_codeUnitAt`), which throws
//!   the VM's `RangeError`;
//! * `allocateOneByteString`/`allocateTwoByteString` → `@df.string_allocate`;
//! * `writeIntoOneByteString`/`writeIntoTwoByteString` → `@df.string_write`;
//! * `hashCode` (`String_getHashCode`) → `@df.string_hash`;
//! * `==` (`DartForge_string_equals_object`, `DartForge_string_equals`) →
//!   `@df.string_equals_object`/`@df.string_equals`.
//!
//! The block does not move and the code units of a published string do not change, so
//! there is no cache per read point: LLVM merges repeated loads and hoists them
//! out of loops.

use super::fn_builder::FnBuilder;
use crate::hir::*;

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// The external string member done inline, or `None` (the usual
    /// call). `member` is `Class.name` (or just the name, in a top-level function) and
    /// `native` the name of the native, if any.
    pub(super) fn inline_string(
        &mut self,
        member: &str,
        native: Option<&str>,
        this: Option<&Operand>,
        args: &[Operand],
    ) -> Option<Operand> {
        match (member, native, this) {
            ("_StringBase.codeUnitAt", None, Some(s)) if args.len() == 1 && self.is_ref(s) => {
                let i = self.coerce(args[0].clone(), Type::I64);
                Some(self.inline_unit(s.clone(), i))
            }
            (_, Some("String_getLength"), Some(s)) if args.is_empty() && self.is_ref(s) => {
                Some(self.helper("df.string_len", vec![(s.clone(), Type::Ref)], Type::I64))
            }
            (_, Some("String_getHashCode"), Some(s)) if args.is_empty() && self.is_ref(s) => {
                Some(self.helper("df.string_hash", vec![(s.clone(), Type::Ref)], Type::I64))
            }
            (_, Some(n @ ("Internal_allocateOneByteString" | "Internal_allocateTwoByteString")), None) if args.len() == 1 => {
                let len = self.coerce(args[0].clone(), Type::I64);
                let two = Operand::Constant(Constant::Bool(n == "Internal_allocateTwoByteString"));
                Some(self.helper("df.string_allocate", vec![(len, Type::I64), (two, Type::I1)], Type::Ref))
            }
            (_, Some("Internal_writeIntoOneByteString" | "Internal_writeIntoTwoByteString"), None) if args.len() == 3 => {
                let s = self.coerce(args[0].clone(), Type::Ref);
                let i = self.coerce(args[1].clone(), Type::I64);
                let u = self.coerce(args[2].clone(), Type::I64);
                self.helper("df.string_write", vec![(s, Type::Ref), (i, Type::I64), (u, Type::I64)], Type::Void);
                Some(Operand::Constant(Constant::Null))
            }
            (_, Some(n @ ("DartForge_string_equals_object" | "DartForge_string_equals")), Some(s)) if args.len() == 1 && self.is_ref(s) => {
                let other = self.coerce(args[0].clone(), Type::Ref);
                let name = if n == "DartForge_string_equals" { "df.string_equals" } else { "df.string_equals_object" };
                Some(self.helper(name, vec![(s.clone(), Type::Ref), (other, Type::Ref)], Type::I1))
            }
            _ => None,
        }
    }

    /// The operand is a `Ref` (the receiver of a string native always is; another
    /// representation falls back to the usual call).
    fn is_ref(&self, op: &Operand) -> bool {
        self.operand_type(op) == Type::Ref
    }

    /// The call to a `@df.string_*` helper: it does not throw (the effects are in
    /// `llvm/ir_strings.rs`, `HELPER_EFFECTS`), so there is no exception
    /// check.
    fn helper(&mut self, name: &str, args: Vec<(Operand, Type)>, ret: Type) -> Operand {
        self.emit(Instruction::CallRuntime { name: name.to_string(), args, ret_ty: ret }, ret)
    }

    /// `s.codeUnitAt(i)`: the code unit read inline when `0 <= i < length`; out of
    /// range, the native, which throws.
    fn inline_unit(&mut self, s: Operand, i: Operand) -> Operand {
        let n = self.helper("df.string_len", vec![(s.clone(), Type::Ref)], Type::I64);
        let inside = self.emit(Instruction::ICmp(ICmpOp::Ult, i.clone(), n), Type::I1);
        let fast = self.new_block();
        let slow = self.new_block();
        let end = self.new_block();
        self.terminate(Terminator::CondBranch { cond: inside, then_block: fast, else_block: slow });

        self.set_block(fast);
        let u = self.helper("df.string_code_unit", vec![(s.clone(), Type::Ref), (i.clone(), Type::I64)], Type::I64);
        let fast_end = self.current_block;
        self.terminate(Terminator::Branch(end));

        self.set_block(slow);
        let r = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: crate::natives::symbol("DartForge_string_codeUnitAt"),
                args: vec![(s, Type::Ref), (i, Type::I64)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let slow_end = self.current_block;
        self.terminate(Terminator::Branch(end));
        self.set_block(end);
        self.emit(Instruction::Phi { incoming: vec![(fast_end, u), (slow_end, r)], ty: Type::I64 }, Type::I64)
    }
}
