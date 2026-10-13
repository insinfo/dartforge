//! The lint rules that need the static types from inference
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8). They return neutral findings
//! (the position, the unique name of the code and the arguments): the caller
//! keeps them and only emits them when the rule is enabled.
//!
//! Here: `prefer_is_empty` (`linter/lib/src/rules/prefer_is_empty.dart`,
//! read from the SDK's `main`; to be checked against 3.6.2), with its four
//! codes. The constant context is the analyzer's `inConstantContext`
//! (`dartforge_frontend::pais`), and a `const` constructor initializer is
//! left out as in the original's `_check`.
//! Written without compiling or running (2026-10-05).

use crate::resolve::OutlineTypes;
use crate::resolved::UnitBodyTypes;
use crate::table::{CoreTypes, Type, TypeTable};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Program, UnitId};
use dartforge_frontend::ast::{self, BinaryOp, ExprId, ExprKind, Initializer, MemberKind, UnaryOp};

/// A finding: the position, the unique name of the lint code and the
/// arguments.
pub type Finding = (Span, &'static str, Vec<String>);

/// `getIntValue`: the value of an integer literal, with a leading `-`.
fn integer_value(a: &ast::Ast, source: &str, e: ExprId) -> Option<i64> {
    let literal = |x: ExprId| match &a.expr(x).kind {
        ExprKind::Int(s) => {
            let text: String = source.get(s.start..s.end)?.chars().filter(|c| *c != '_').collect();
            match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
                // Hexadecimal goes up to 64 bits (`0xFFFFFFFFFFFFFFFF` is -1).
                Some(hex) => u64::from_str_radix(hex, 16).ok().map(|v| v as i64),
                None => text.parse::<i64>().ok(),
            }
        }
        _ => None,
    };
    match &a.expr(e).kind {
        ExprKind::Unary { op: UnaryOp::Neg, operand } => literal(*operand).map(|v| -v),
        _ => literal(e),
    }
}

/// The parents of the expressions of unit `u` (`dartforge_frontend::pais`),
/// with the old `case` in libraries from before 3.0.
pub(crate) fn unit_parents(program: &Program, u: UnitId) -> dartforge_frontend::parents::Parents {
    let unit = program.unit(u);
    let version = program.library(unit.library).features.version();
    let before_3 = version < dartforge_frontend::features::LanguageVersion::new(3, 0);
    dartforge_frontend::parents::Parents::new(&unit.ast, &unit.unit, &unit.source, before_3)
}

/// The findings of the typed rules in unit `u`.
pub fn achados(program: &Program, table: &TypeTable, core: &CoreTypes, outline: &OutlineTypes, body: &UnitBodyTypes, u: UnitId) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    let unit = program.unit(u);
    let a = &unit.ast;
    let source = unit.source.as_str();

    // `_check`: inside a `const` constructor initializer, nothing.
    let mut of_const: Vec<Span> = Vec::new();
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind
            && k.const_
        {
            of_const.extend(k.initializers.iter().map(|i| match i {
                Initializer::Field { span, .. } | Initializer::Super { span, .. } | Initializer::Redirect { span, .. } | Initializer::Assert { span, .. } => *span,
            }));
        }
    }
    // `inConstantContext`.
    let pais = unit_parents(program, u);
    let in_constant = |e: ExprId| pais.in_constant_context(a, e) || of_const.iter().any(|k| k.start <= a.expr(e).span.start && a.expr(e).span.end <= k.end);

    // `_isLengthAccess`: `x.length` with `x` an `Iterable`, a `Map` or a
    // `String` (through parentheses and `as`).
    let of_length = |mut e: ExprId| {
        loop {
            match &a.expr(e).kind {
                ExprKind::Parenthesized(x) => e = *x,
                ExprKind::As { value, .. } => e = *value,
                _ => break,
            }
        }
        let ExprKind::Property { target, name, .. } = &a.expr(e).kind else { return false };
        if source.get(name.span.start..name.span.end) != Some("length") {
            return false;
        }
        let Some(ty) = body.get_type(*target) else { return false };
        let Type::Interface { class, .. } = table.get(ty) else { return false };
        let implements = |tgt: Option<dartforge_elements::model::ClassId>| {
            tgt.is_some_and(|k| *class == k || outline.hierarchy.get(*class).is_some_and(|d| d.supertypes.contains_key(&k)))
        };
        Some(*class) == core.string_class || implements(core.iterable_class) || implements(core.map_class)
    };

    for (k, e) in a.exprs.iter().enumerate() {
        let ExprKind::Binary { op, left, right } = &e.kind else { continue };
        // The constant on one side and the `length` on the other.
        let (the_value, to_the_right) = match integer_value(a, source, *right) {
            Some(v) if of_length(*left) => (v, true),
            Some(_) => continue,
            None => match integer_value(a, source, *left) {
                Some(v) if of_length(*right) => (v, false),
                _ => continue,
            },
        };
        if in_constant(ExprId(k as u32)) {
            continue;
        }
        let (empty, non_empty, false_, true_) = (
            "prefer_is_empty_use_is_empty",
            "prefer_is_empty_use_is_not_empty",
            "prefer_is_empty_always_false",
            "prefer_is_empty_always_true",
        );
        let code = if the_value == 0 {
            match op {
                BinaryOp::Eq | BinaryOp::LtEq => Some(empty),
                BinaryOp::Gt | BinaryOp::NotEq => Some(non_empty),
                BinaryOp::Lt => Some(false_),
                BinaryOp::GtEq => Some(true_),
                _ => None,
            }
        } else if the_value == 1 {
            match (to_the_right, op) {
                (true, BinaryOp::GtEq) | (false, BinaryOp::LtEq) => Some(non_empty),
                (true, BinaryOp::Lt) | (false, BinaryOp::Gt) => Some(empty),
                _ => None,
            }
        } else if the_value < 0 {
            match (to_the_right, op) {
                (true, BinaryOp::Eq | BinaryOp::LtEq | BinaryOp::Lt) | (false, BinaryOp::Eq | BinaryOp::GtEq | BinaryOp::Gt) => Some(false_),
                (true, BinaryOp::NotEq | BinaryOp::GtEq | BinaryOp::Gt) | (false, BinaryOp::NotEq | BinaryOp::LtEq | BinaryOp::Lt) => Some(true_),
                _ => None,
            }
        } else {
            None
        };
        if let Some(c) = code {
            out.push((e.span, c, Vec::new()));
        }
    }
    out
}
