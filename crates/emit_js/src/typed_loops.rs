//! Version pure, canonical typed-list read loops with a checked fallback.
//!
//! The guard excludes wrappers, custom lists, shared/resizable buffers and
//! shorter secondary arrays. The original loop remains responsible for their
//! observable access order and errors; a failed guard never throws early.

use crate::body::FnEmitter;
use crate::ty::Ty;
use dartforge_frontend::ast::{self, AssignOp, BinaryOp, ExprId, ExprKind, StmtId, StmtKind, UnaryOp};
use dartforge_intern::SymbolId;
use std::collections::HashMap;

pub(crate) struct ReadLoop {
    pub guard: String,
    pub raw: HashMap<ExprId, Ty>,
}

impl FnEmitter<'_, '_> {
    fn loop_identifier(&self, e: ExprId) -> Option<SymbolId> {
        match self.expr(e).kind {
            ExprKind::Identifier(n) => Some(n.sym),
            _ => None,
        }
    }

    fn loop_array(&self, s: SymbolId) -> Option<(&'static str, Ty)> {
        let local = self.lookup_local(s)?;
        if local.lazy_init.is_some() || local.late_check || local.late_final { return None; }
        let ty = &local.ty;
        let Ty::Iface { class, nullable: false, .. } = ty else { return None };
        let c = self.ctx.program.class(*class);
        if self.ctx.program.library(c.library).uri != "dart:typed_data" {
            return None;
        }
        let brand = match self.ctx.class_name(*class) {
            "Float32List" => "Float32Array",
            "Float64List" => "Float64Array",
            "Int8List" => "Int8Array",
            "Uint8List" => "Uint8Array",
            "Uint8ClampedList" => "Uint8ClampedArray",
            "Int16List" => "Int16Array",
            "Uint16List" => "Uint16Array",
            "Int32List" => "Int32Array",
            "Uint32List" => "Uint32Array",
            _ => return None,
        };
        Some((brand, if matches!(brand, "Float32Array" | "Float64Array") { self.ctx.t_double() } else { self.ctx.t_int() }))
    }

    fn numeric_loop_value(&self, e: ExprId, index: SymbolId, arrays: &mut Vec<SymbolId>, raw: &mut HashMap<ExprId, Ty>) -> bool {
        match &self.expr(e).kind {
            ExprKind::Int(_) | ExprKind::Double(_) => true,
            ExprKind::Identifier(n) => self.lookup_local(n.sym).is_some_and(|l| self.ctx.is_num_like(&l.ty) && !l.ty.is_nullable() && l.lazy_init.is_none() && !l.late_check && !l.late_final),
            ExprKind::Parenthesized(v) => self.numeric_loop_value(*v, index, arrays, raw),
            ExprKind::Unary { op: UnaryOp::Neg | UnaryOp::BitNot, operand } => self.numeric_loop_value(*operand, index, arrays, raw),
            ExprKind::Binary { op: BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::TruncDiv | BinaryOp::Rem | BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr | BinaryOp::UShr, left, right } => {
                self.numeric_loop_value(*left, index, arrays, raw) && self.numeric_loop_value(*right, index, arrays, raw)
            }
            ExprKind::Index { target, index: i, null_aware: false } if self.loop_identifier(*i) == Some(index) => {
                let Some(s) = self.loop_identifier(*target) else { return false };
                let Some((_, ty)) = self.loop_array(s) else { return false };
                if !arrays.contains(&s) { arrays.push(s); }
                raw.insert(e, ty);
                true
            }
            _ => false,
        }
    }

    fn numeric_loop_body(&self, s: StmtId, index: SymbolId, arrays: &mut Vec<SymbolId>, raw: &mut HashMap<ExprId, Ty>) -> bool {
        match &self.ast().stmt(s).kind {
            StmtKind::Block(stmts) => stmts.iter().all(|s| self.numeric_loop_body(*s, index, arrays, raw)),
            StmtKind::Expression(e) => match &self.expr(*e).kind {
                ExprKind::Assign { op: AssignOp::Assign | AssignOp::Compound(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::TruncDiv | BinaryOp::Rem | BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr | BinaryOp::UShr), target, value } => {
                    let Some(s) = self.loop_identifier(*target) else { return false };
                    s != index && self.lookup_local(s).is_some_and(|l| self.ctx.is_num_like(&l.ty) && !l.ty.is_nullable() && l.lazy_init.is_none() && !l.late_check && !l.late_final)
                        && self.numeric_loop_value(*value, index, arrays, raw)
                }
                _ => false,
            },
            _ => false,
        }
    }

    pub(crate) fn typed_read_loop(&mut self, init: &Option<ast::ForInit>, condition: Option<ExprId>, updates: &[ExprId], body: StmtId) -> Option<ReadLoop> {
        if !self.ctx.production() || std::env::var("DARTFORGE_JSPROD_TYPED_LOOPS").is_ok_and(|v| v == "0") { return None; }
        let Some(ast::ForInit::Variables(list)) = init else { return None };
        if list.variables.len() != 1 || list.late || list.final_ || list.const_ { return None; }
        let v = &list.variables[0];
        let ExprKind::Int(span) = self.expr(v.initializer?).kind else { return None };
        if self.text(span) != "0" || !self.ctx.is_int(&self.lookup_local(v.name.sym)?.ty) { return None; }
        let index = v.name.sym;
        let [update] = updates else { return None };
        match &self.expr(*update).kind {
            ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PostfixInc, operand } if self.loop_identifier(*operand) == Some(index) => {}
            _ => return None,
        }
        let ExprKind::Binary { op: BinaryOp::Lt, left, right } = self.expr(condition?).kind else { return None };
        if self.loop_identifier(left) != Some(index) { return None; }
        let ExprKind::Property { target, name, null_aware: false } = self.expr(right).kind else { return None };
        if self.name(name.sym) != "length" { return None; }
        let bound = self.loop_identifier(target)?;
        self.loop_array(bound)?;
        let mut arrays = vec![bound];
        let mut raw = HashMap::new();
        if !self.numeric_loop_body(body, index, &mut arrays, &mut raw) || raw.is_empty() { return None; }
        raw.insert(right, self.ctx.t_int());
        self.m.typed_read_guard_libraries.borrow_mut().insert(self.m.lib.0);
        let helper = crate::typed_read_guards::name(self.m.lib.0);
        let mut guards = Vec::new();
        let mut lengths = Vec::new();
        for s in &arrays {
            let js = self.lookup_local(*s)?.js.clone();
            let (brand, _) = self.loop_array(*s)?;
            let brand = match brand {
                "Float32Array" => 0, "Float64Array" => 1, "Int8Array" => 2,
                "Uint8Array" => 3, "Uint8ClampedArray" => 4, "Int16Array" => 5,
                "Uint16Array" => 6, "Int32Array" => 7, "Uint32Array" => 8,
                _ => return None,
            };
            let length = self.temp();
            guards.push(format!("({length} = {helper}({js}, {brand})) >= 0"));
            lengths.push(length);
        }
        guards.push(format!("{} <= 4294967296", lengths[0]));
        for length in lengths.iter().skip(1) {
            guards.push(format!("{} <= {length}", lengths[0]));
        }
        Some(ReadLoop { guard: guards.join(" && "), raw })
    }
}
