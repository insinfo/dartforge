//! Cascade (P4): `e..a = 1..b()` and `e?..a()`.
//!
//! The specification (§17.23 "Cascades"): `e` is evaluated once; each section is
//! evaluated with the implicit receiver `e` (the `CascadeTarget` marker in the
//! AST), in order; the value of the cascade is that of `e`. In `e?..s`, if `e` is null
//! no section is evaluated and the value is null.
//!
//! The sections have no inferred type (the implicit receiver is `dynamic` for
//! today's inference): their members go through dynamic dispatch
//! (`dispatch.rs`), which finds the member by the receiver's class.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_frontend::ast::{self, ExprId};

impl<'a, 'c> FnBuilder<'a, 'c> {
    pub fn lower_cascade(
        &mut self,
        ast: &ast::Ast,
        target: ExprId,
        sections: &[ExprId],
        null_aware: bool,
    ) -> Operand {
        let target = self.lower_expr(ast, target);
        let saved = self.current_cascade_target.replace(target.clone());
        if null_aware && self.operand_type(&target) == Type::Ref {
            let null = self.emit(
                Instruction::ICmp(
                    ICmpOp::Eq,
                    target.clone(),
                    Operand::Constant(Constant::Int(0)),
                ),
                Type::I1,
            );
            let b_sections = self.new_block();
            let end = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: null,
                then_block: end,
                else_block: b_sections,
            });
            self.set_block(b_sections);
            for &s in sections {
                self.lower_expr(ast, s);
            }
            self.terminate(Terminator::Branch(end));
            self.set_block(end);
        } else {
            for &s in sections {
                self.lower_expr(ast, s);
            }
        }
        self.current_cascade_target = saved;
        target
    }
}
