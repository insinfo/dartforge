//! O receptor sem tipo estático útil (`dynamic`, `Object`, `num`, o corpo de
//! uma closure): o membro pela classe **dinâmica** do receptor
//! (especificação §17.21.1 "Ordinary Invocation", `dynamic`), que é a
//! chamada por seletor (`sdk_source.rs`, `llvm/selectors.rs`): a tabela de
//! métodos de cada classe, com o `noSuchMethod` do receptor na falta.
//!
//! O despacho pelo nome em mundo fechado (um `switch` pelas classes do
//! programa com o membro, e o runtime escrito à mão no `default`) só servia
//! ao modo sem SDK da fonte e saiu com ele (docs/NATIVO-ESPACO-UNIFICADO.md
//! §4.7).

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{BinaryOp, UnaryOp};

/// O texto do operador (o nome do membro, `operator +`) e o código dele (o
/// de `dartforge_dyn_op`, o operador do runtime sobre `num`/`String`).
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
    /// `NoSuchMethodError` em tempo de execução (o receptor não tem o
    /// membro).
    pub fn throw_nsm(&mut self, name: &str) -> Operand {
        let n = self.emit(Instruction::Const(Constant::String(name.to_string())), Type::Ref);
        let e = self.new_runtime_error("dartforge_no_such_method_error_new", vec![(n, Type::Ref)]);
        self.emit_throw_op(e);
        Operand::Constant(Constant::Null)
    }

    /// `a op b` com algum lado sem tipo numérico estático: o `operator op`
    /// pela classe dinâmica de `a`. Os dois lados com o tipo garantido pelo
    /// estático: o seletor tipado (`typed_entry.rs`).
    pub fn operate_dynamic(&mut self, op: BinaryOp, a: Operand, b: Operand, span: Span) -> Operand {
        let Some((name, _)) = operator(op) else {
            return self.unsupported("operador sobre num/dynamic/objeto", span);
        };
        let typed = self.all_checked(&[(None, a.clone())]);
        let r = self.call_by_name_with_typed_receiver(a, typed, super::sdk_source::Tipo::Call, name, &[(None, b)]);
        // Comparação: o contexto quer o `bool`.
        if matches!(op, BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq) {
            return self.coerce(r, Type::I1);
        }
        r
    }

    /// `-a`/`~a` sobre uma referência: pela classe dinâmica.
    pub fn dynamic_unary(&mut self, op: UnaryOp, a: Operand) -> Operand {
        let name = if op == UnaryOp::Neg { "unary-" } else { "~" };
        self.call_by_name(a, super::sdk_source::Tipo::Call, name, &[])
    }

    /// O receptor não tem tipo estático útil para achar o membro: sem tipo
    /// (corpo de closure), `dynamic`, `Object`/`Object?`.
    pub fn dynamic_receiver(&self, e: dartforge_frontend::ast::ExprId) -> bool {
        let Some(t) = self.ctx.get_type(self.unit_id, e) else {
            return true;
        };
        t == self.ctx.core.dynamic_ || t == self.ctx.core.object || t == self.ctx.core.object_nullable
    }
}
