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
        // `<E>[…]..add(a)..add(b)`: the target is a fresh `_GrowableList<E>`,
        // so each `add` whose argument is statically an `E` is the append of a
        // literal element (`dartforge_list_append`), one runtime call without
        // the type tests, the growth path and the dispatch fallback of the
        // inline `add` — a cascade of 20 000 of them (`regress/regress41983`)
        // was a function of ~240 000 blocks.
        let literal_element = match &ast.expr(target).kind {
            ast::ExprKind::List { const_: false, .. }
                if !null_aware && !self.in_const_context && !self.ctx.arc_memory =>
            {
                self.literal_element_types(target).first().copied()
            }
            _ => None,
        };
        let target = self.lower_expr(ast, target);
        if let Some(element) = literal_element
            && self.operand_type(&target) == Type::Ref
        {
            let saved = self.current_cascade_target.replace(target.clone());
            for &s in sections {
                if self.is_terminated() {
                    break;
                }
                match self.literal_append_argument(ast, s, element) {
                    Some(arg) => {
                        let v = self.lower_expr(ast, arg);
                        if self.is_terminated() {
                            break;
                        }
                        let v = self.coerce(v, Type::Ref);
                        self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_list_append".to_string(),
                                args: vec![(target.clone(), Type::Ref), (v, Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    }
                    None => {
                        self.lower_expr(ast, s);
                    }
                }
            }
            self.current_cascade_target = saved;
            return target;
        }
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

    /// The argument of the cascade section `s` when it is `..add(x)` on the
    /// implicit receiver with `x` statically an `element` (the same type, or a
    /// top element type): appending it needs no covariance check.
    fn literal_append_argument(
        &self,
        ast: &ast::Ast,
        s: ExprId,
        element: dartforge_types::table::TypeId,
    ) -> Option<ExprId> {
        use dartforge_types::table::Type as T;
        let ast::ExprKind::Call { target, arguments } = &ast.expr(s).kind else {
            return None;
        };
        if arguments.args.len() != 1
            || arguments.args[0].name.is_some()
            || !arguments.type_args.is_empty()
        {
            return None;
        }
        let ast::ExprKind::Property {
            target: recv,
            name,
            null_aware: false,
        } = &ast.expr(*target).kind
        else {
            return None;
        };
        if !matches!(ast.expr(*recv).kind, ast::ExprKind::CascadeTarget)
            || self.ctx.symbol_name(name.sym) != "add"
        {
            return None;
        }
        let arg = arguments.args[0].value;
        let arg_ty = self.ctx.get_type(self.unit_id, arg)?;
        if matches!(self.ctx.table.get(arg_ty), T::Dynamic | T::Never | T::Void) {
            return None;
        }
        let top = matches!(self.ctx.table.get(element), T::Dynamic)
            || element == self.ctx.core.object_nullable;
        (arg_ty == element || top).then_some(arg)
    }
}
