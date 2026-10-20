//! Prove nonnegative, unescaped counters before lowering positive modulo.
//!
//! A fresh JS Number starts at +0 and only adds +1. It cannot wrap negative:
//! at the precision limit it stops advancing. All binding mutations, closures,
//! declarations and unsupported statements are rejected before recording nodes.

use crate::body::FnEmitter;
use dartforge_frontend::ast::{
    self, AssignOp, BinaryOp, ExprId, ExprKind, StmtId, StmtKind, UnaryOp,
};
use dartforge_intern::SymbolId;
use std::collections::HashSet;

impl FnEmitter<'_, '_> {
    fn counter_name(&self, expression: ExprId, name: SymbolId) -> bool {
        matches!(self.expr(expression).kind, ExprKind::Identifier(n) if n.sym == name)
    }

    fn counter_decimal(&self, expression: ExprId, expected: Option<u32>) -> bool {
        let ExprKind::Int(span) = self.expr(expression).kind else {
            return false;
        };
        let text = self.text(span);
        if !text.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
        let Ok(value) = text.parse::<u32>() else {
            return false;
        };
        expected.map_or(value > 0 && value <= 1_000_000_000, |wanted| {
            value == wanted
        })
    }

    fn counter_expression(
        &self,
        expression: ExprId,
        index: SymbolId,
        nodes: &mut HashSet<ExprId>,
    ) -> bool {
        match &self.expr(expression).kind {
            ExprKind::Int(_)
            | ExprKind::Double(_)
            | ExprKind::Bool(_)
            | ExprKind::Null
            | ExprKind::Identifier(_)
            | ExprKind::This
            | ExprKind::Super
            | ExprKind::Symbol(_) => true,
            ExprKind::String(literal) => literal.parts.iter().all(|part| match part {
                ast::StringPart::Text(_) => true,
                ast::StringPart::Interpolation(e) => self.counter_expression(*e, index, nodes),
            }),
            ExprKind::Parenthesized(e)
            | ExprKind::As { value: e, .. }
            | ExprKind::Is { value: e, .. }
            | ExprKind::TypeArguments { target: e, .. }
            | ExprKind::Property { target: e, .. } => self.counter_expression(*e, index, nodes),
            ExprKind::Unary {
                op: UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot | UnaryOp::NullAssert,
                operand,
            } => self.counter_expression(*operand, index, nodes),
            ExprKind::Binary { op, left, right } => {
                if !self.counter_expression(*left, index, nodes)
                    || !self.counter_expression(*right, index, nodes)
                {
                    return false;
                }
                if *op == BinaryOp::Rem
                    && self.counter_name(*left, index)
                    && self.counter_decimal(*right, None)
                {
                    nodes.insert(expression);
                }
                true
            }
            ExprKind::Index {
                target,
                index: offset,
                ..
            } => {
                self.counter_expression(*target, index, nodes)
                    && self.counter_expression(*offset, index, nodes)
            }
            ExprKind::Call { target, arguments } => {
                let intrinsic = crate::own_sdk::called_function(self, *target).is_some_and(|fid| {
                    self.ctx
                        .sdk
                        .as_ref()
                        .is_some_and(|sdk| sdk.intrinsics.contains_key(&fid))
                });
                !intrinsic
                    && self.counter_expression(*target, index, nodes)
                    && arguments
                        .args
                        .iter()
                        .all(|arg| self.counter_expression(arg.value, index, nodes))
            }
            ExprKind::InstanceCreation { arguments, .. } => arguments
                .args
                .iter()
                .all(|arg| self.counter_expression(arg.value, index, nodes)),
            ExprKind::Conditional {
                condition,
                then,
                else_,
            } => {
                self.counter_expression(*condition, index, nodes)
                    && self.counter_expression(*then, index, nodes)
                    && self.counter_expression(*else_, index, nodes)
            }
            // Calls cannot mutate an unescaped lexical Number binding. Reject
            // every closure, write, declaration or unsupported AST form rather
            // than infer capture or side-effect behavior from its spelling.
            _ => false,
        }
    }

    fn counter_body(
        &self,
        statement: StmtId,
        index: SymbolId,
        nodes: &mut HashSet<ExprId>,
    ) -> bool {
        match &self.ast().stmt(statement).kind {
            StmtKind::Block(statements) => statements
                .iter()
                .all(|s| self.counter_body(*s, index, nodes)),
            StmtKind::Expression(e) => self.counter_expression(*e, index, nodes),
            StmtKind::Empty => true,
            _ => false,
        }
    }

    pub(crate) fn positive_counter_modulo(
        &self,
        init: &Option<ast::ForInit>,
        condition: Option<ExprId>,
        updates: &[ExprId],
        body: StmtId,
    ) -> HashSet<ExprId> {
        let empty = HashSet::new;
        if !self.ctx.production()
            || self
                .ctx
                .program
                .library(self.ctx.program.unit(self.unit).library)
                .uri
                .starts_with("dart:")
            || std::env::var("DARTFORGE_JSPROD_COUNTER_MODULO").is_ok_and(|v| v == "0")
        {
            return empty();
        }
        let Some(ast::ForInit::Variables(list)) = init else {
            return empty();
        };
        let [variable] = list.variables.as_ref() else {
            return empty();
        };
        if list.late
            || list.final_
            || list.const_
            || list.external
            || !self
                .lookup_local(variable.name.sym)
                .is_some_and(|local| self.ctx.is_int(&local.ty))
        {
            return empty();
        }
        if !variable
            .initializer
            .is_some_and(|value| self.counter_decimal(value, Some(0)))
        {
            return empty();
        }
        let index = variable.name.sym;
        let Some(condition) = condition else {
            return empty();
        };
        let ExprKind::Binary {
            op: BinaryOp::Lt,
            left,
            right,
        } = self.expr(condition).kind
        else {
            return empty();
        };
        if !self.counter_name(left, index) {
            return empty();
        }
        let [update] = updates else { return empty() };
        match self.expr(*update).kind {
            ExprKind::Unary {
                op: UnaryOp::PrefixInc | UnaryOp::PostfixInc,
                operand,
            } if self.counter_name(operand, index) => {}
            ExprKind::Assign {
                op: AssignOp::Compound(BinaryOp::Add),
                target,
                value,
            } if self.counter_name(target, index) && self.counter_decimal(value, Some(1)) => {}
            _ => return empty(),
        }
        let mut nodes = HashSet::new();
        if !self.counter_expression(right, index, &mut nodes)
            || !self.counter_body(body, index, &mut nodes)
            || nodes.is_empty()
        {
            return empty();
        }
        if !crate::own_sdk::certified_number_modulo(self.ctx) {
            return empty();
        }
        nodes
    }
}
