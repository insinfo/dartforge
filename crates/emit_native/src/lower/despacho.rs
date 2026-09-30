//! O receptor sem tipo estático útil (`dynamic`, `Object`, `num`, o corpo de
//! uma closure): o membro pela classe **dinâmica** do receptor
//! (especificação §17.21.1 "Ordinary Invocation", `dynamic`), que é a
//! chamada por seletor (`sdk_fonte.rs`, `llvm/seletores.rs`): a tabela de
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
pub fn operador(op: BinaryOp) -> Option<(&'static str, i64)> {
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
    pub fn lancar_nsm(&mut self, nome: &str) -> Operand {
        let n = self.emit(Instruction::Const(Constant::String(nome.to_string())), Type::Ref);
        let e = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_no_such_method_error_new".to_string(),
                args: vec![(n, Type::Ref)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        self.emit_throw_op(e);
        Operand::Constant(Constant::Null)
    }

    /// `a op b` com algum lado sem tipo numérico estático: o `operator op`
    /// pela classe dinâmica de `a`. Os dois lados com o tipo garantido pelo
    /// estático: o seletor tipado (`entrada_tipada.rs`).
    pub fn operar_dinamico(&mut self, op: BinaryOp, a: Operand, b: Operand, span: Span) -> Operand {
        let Some((nome, _)) = operador(op) else {
            return self.nao_suportado("operador sobre num/dynamic/objeto", span);
        };
        let tipado = self.todos_conferidos(&[(None, a.clone())]);
        let r = self.chamar_por_nome_com_receptor_tipado(a, tipado, super::sdk_fonte::Tipo::Chamar, nome, &[(None, b)]);
        // Comparação: o contexto quer o `bool`.
        if matches!(op, BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq) {
            return self.coagir(r, Type::I1);
        }
        r
    }

    /// `-a`/`~a` sobre uma referência: pela classe dinâmica.
    pub fn unario_dinamico(&mut self, op: UnaryOp, a: Operand) -> Operand {
        let nome = if op == UnaryOp::Neg { "unary-" } else { "~" };
        self.chamar_por_nome(a, super::sdk_fonte::Tipo::Chamar, nome, &[])
    }

    /// O receptor não tem tipo estático útil para achar o membro: sem tipo
    /// (corpo de closure), `dynamic`, `Object`/`Object?`.
    pub fn receptor_dinamico(&self, e: dartforge_frontend::ast::ExprId) -> bool {
        let Some(t) = self.ctx.get_type(self.unit_id, e) else {
            return true;
        };
        t == self.ctx.core.dynamic_ || t == self.ctx.core.object || t == self.ctx.core.object_nullable
    }
}
