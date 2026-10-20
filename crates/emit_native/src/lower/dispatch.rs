//! The receiver without a useful static type (`dynamic`, `Object`, `num`, the body of
//! a closure): the member by the **dynamic** class of the receiver
//! (specification §17.21.1 "Ordinary Invocation", `dynamic`), which is the
//! call by selector (`sdk_source.rs`, `llvm/selectors.rs`): the method table of
//! each class, with the receiver's `noSuchMethod` as fallback.
//!
//! Dispatch by name in a closed world (a `switch` over the program's classes
//! that have the member, with the hand-written runtime in the `default`) only served
//! the mode without the source SDK and left with it (docs/NATIVO-ESPACO-UNIFICADO.md
//! §4.7).

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{BinaryOp, UnaryOp};

/// The text of the operator (the member name, `operator +`) and its code (the
/// one of `dartforge_dyn_op`, the runtime operator over `num`/`String`).
pub fn operator(op: BinaryOp) -> Option<(&'static str, i64)> {
    Some(match op {
        BinaryOp::Add => ("+", 0),
        BinaryOp::Sub => ("-", 1),
        BinaryOp::Mul => ("*", 2),
        BinaryOp::Div => ("/", 3),
        BinaryOp::TruncDiv => ("~/", 4),
        BinaryOp::Rem => ("%", 5),
        BinaryOp::Lt => ("<", 6),
        BinaryOp::LtEq => ("<=", 7),
        BinaryOp::Gt => (">", 8),
        BinaryOp::GtEq => (">=", 9),
        BinaryOp::BitAnd => ("&", 10),
        BinaryOp::BitOr => ("|", 11),
        BinaryOp::BitXor => ("^", 12),
        BinaryOp::Shl => ("<<", 13),
        BinaryOp::Shr => (">>", 14),
        BinaryOp::UShr => (">>>", 15),
        _ => return None,
    })
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `NoSuchMethodError` at run time (the receiver does not have the
    /// member).
    pub fn throw_nsm(&mut self, name: &str) -> Operand {
        let n = self.emit(
            Instruction::Const(Constant::String(name.to_string())),
            Type::Ref,
        );
        let e = self.new_runtime_error("dartforge_no_such_method_error_new", vec![(n, Type::Ref)]);
        self.emit_throw_op(e);
        Operand::Constant(Constant::Null)
    }

    /// `a op b` with some side without a static numeric type: the `operator op`
    /// by the dynamic class of `a`. With both sides' types guaranteed by the
    /// static type: the typed selector (`typed_entry.rs`).
    pub fn operate_dynamic(&mut self, op: BinaryOp, a: Operand, b: Operand, span: Span) -> Operand {
        let Some((name, _)) = operator(op) else {
            return self.unsupported("operator on num/dynamic/object", span);
        };
        let typed = self.all_checked(&[(None, a.clone())]);
        let r = self.call_by_name_with_typed_receiver(
            a,
            typed,
            super::sdk_source::AccessKind::Call,
            name,
            &[(None, b)],
        );
        // Comparison: the context wants the `bool`.
        if matches!(
            op,
            BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq
        ) {
            return self.coerce(r, Type::I1);
        }
        r
    }

    /// `-a`/`~a` over a reference: by the dynamic class.
    pub fn dynamic_unary(&mut self, op: UnaryOp, a: Operand) -> Operand {
        let name = if op == UnaryOp::Neg { "unary-" } else { "~" };
        self.call_by_name(a, super::sdk_source::AccessKind::Call, name, &[])
    }

    /// The receiver has no useful static type to find the member: no type
    /// (closure body), `dynamic`, `Object`/`Object?`.
    pub fn dynamic_receiver(&self, e: dartforge_frontend::ast::ExprId) -> bool {
        let Some(t) = self.ctx.get_type(self.unit_id, e) else {
            return true;
        };
        t == self.ctx.core.dynamic_
            || t == self.ctx.core.object
            || t == self.ctx.core.object_nullable
    }
}
