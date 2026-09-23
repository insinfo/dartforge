//! Cascata (P4): `e..a = 1..b()` e `e?..a()`.
//!
//! A especificação (§17.23 "Cascades"): `e` é avaliado uma vez; cada seção é
//! avaliada com o receptor implícito `e` (o marcador `CascadeTarget` no
//! AST), em ordem; o valor da cascata é o de `e`. Em `e?..s`, se `e` é null
//! nenhuma seção é avaliada e o valor é null.
//!
//! As seções não têm tipo inferido (o receptor implícito é `dynamic` para a
//! inferência de hoje): os membros delas vão pelo despacho dinâmico
//! (`despacho.rs`), que acha o membro pela classe do receptor.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_frontend::ast::{self, ExprId};

impl<'a, 'c> FnBuilder<'a, 'c> {
    pub fn lower_cascata(
        &mut self,
        ast: &ast::Ast,
        target: ExprId,
        sections: &[ExprId],
        null_aware: bool,
    ) -> Operand {
        let alvo = self.lower_expr(ast, target);
        let salvo = self.current_cascade_target.replace(alvo.clone());
        if null_aware && self.operand_type(&alvo) == Type::Ref {
            let nulo = self.emit(
                Instruction::ICmp(ICmpOp::Eq, alvo.clone(), Operand::Constant(Constant::Int(0))),
                Type::I1,
            );
            let b_secoes = self.new_block();
            let fim = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: nulo,
                then_block: fim,
                else_block: b_secoes,
            });
            self.set_block(b_secoes);
            for &s in sections {
                self.lower_expr(ast, s);
            }
            self.terminate(Terminator::Branch(fim));
            self.set_block(fim);
        } else {
            for &s in sections {
                self.lower_expr(ast, s);
            }
        }
        self.current_cascade_target = salvo;
        alvo
    }
}
