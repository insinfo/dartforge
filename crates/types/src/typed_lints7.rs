//! `unnecessary_await_in_return` (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8),
//! written directly from the 3.6.2 emitter: the `await` of a `Future` in the
//! return (or in the expression body) of a function or method that returns
//! `Future`, when the awaited type is a subtype of the return type; the
//! nearest thing that decides is the function, the method or the block of a
//! `try` (which silences it).
//! Written without compiling or running (2026-10-05).

use crate::typed_lints::Finding;
use crate::resolve::OutlineTypes;
use crate::resolved::UnitBodyTypes;
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{FunctionRef, Program, UnitId};
use dartforge_frontend::ast::{ExprId, ExprKind, FunctionBody, StmtKind};
use std::collections::HashMap;

fn without_parentheses(a: &dartforge_frontend::ast::Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

/// The findings in unit `u`.
pub fn findings(program: &Program, table: &mut TypeTable, core: &CoreTypes, outline: &OutlineTypes, the_body: &UnitBodyTypes, u: UnitId) -> Vec<Finding> {
    let mut out = Vec::new();
    let a = &program.unit(u).ast;
    let is_future = |table: &TypeTable, t: TypeId| matches!(table.get(t), Type::Interface { class, .. } if Some(*class) == core.future_class);
    // The returns: (the node that looks for the ancestor, the expression).
    let mut cases: Vec<(Span, ExprId)> = Vec::new();
    for f in a.functions.iter() {
        if let FunctionBody::Expression(e) = &f.body {
            cases.push((a.expr(*e).span, *e));
        }
    }
    for s in a.stmts.iter() {
        if let StmtKind::Return(Some(e)) = &s.kind {
            cases.push((s.span, *e));
        }
    }
    if cases.is_empty() {
        return out;
    }
    // The `try` blocks (the body and the `finally`).
    let mut try_blocks: Vec<Span> = Vec::new();
    for s in a.stmts.iter() {
        if let StmtKind::Try { body, finally_, .. } = &s.kind {
            try_blocks.push(a.stmt(*body).span);
            if let Some(f) = finally_ {
                try_blocks.push(a.stmt(*f).span);
            }
        }
    }
    let literals: HashMap<dartforge_frontend::ast::FunctionId, ExprId> = a
        .exprs
        .iter()
        .enumerate()
        .filter_map(|(k, e)| match &e.kind {
            ExprKind::FunctionExpression(f) => Some((*f, ExprId(k as u32))),
            _ => None,
        })
        .collect();
    let elements: HashMap<dartforge_frontend::ast::FunctionId, usize> = program.index().unit_functions[u.0 as usize]
        .iter()
        .filter_map(|&k| match program.function(k).node {
            FunctionRef::Function { unit, function } if unit == u => Some((function, k.0 as usize)),
            _ => None,
        })
        .collect();
    for (no, e) in cases {
        let x = without_parentheses(a, e);
        let ExprKind::Await(target) = &a.expr(x).kind else { continue };
        let Some(t) = the_body.get_type(*target).filter(|t| !core.is_unknown(table, *t)) else { continue };
        if !is_future(table, t) {
            continue;
        }
        // The nearest ancestor: a function (or method) or a `try`
        // block.
        let func = a
            .functions
            .iter()
            .enumerate()
            .filter(|(_, f)| f.span.start <= no.start && no.end <= f.span.end)
            .min_by_key(|(_, f)| f.span.end - f.span.start)
            .map(|(k, f)| (dartforge_frontend::ast::FunctionId(k as u32), f.span));
        let block = try_blocks.iter().filter(|b| b.start <= no.start && no.end <= b.end).min_by_key(|b| b.end - b.start).copied();
        let Some((fid, fspan)) = func else { continue };
        if block.is_some_and(|b| b.end - b.start < fspan.end - fspan.start && b.start >= fspan.start) {
            continue;
        }
        // The element's return type.
        let return_ = if let Some(&k) = elements.get(&fid) {
            outline.functions.get(k).map(|d| d.return_type)
        } else if let Some(&fe) = literals.get(&fid) {
            match the_body.get_type(fe).map(|t| table.get(t).clone()) {
                Some(Type::Function { ret, .. }) => Some(ret),
                _ => None,
            }
        } else {
            a.function(fid).name.and_then(|n| the_body.local_type(n.span.start)).and_then(|t| match table.get(t) {
                Type::Function { ret, .. } => Some(*ret),
                _ => None,
            })
        };
        let Some(r) = return_ else { continue };
        if !is_future(table, r) {
            continue;
        }
        let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
        if is_subtype(t, r, &mut env) {
            let start = a.expr(x).span.start;
            out.push((Span { start: start, end: start + "await".len() }, "unnecessary_await_in_return", Vec::new()));
        }
    }
    out
}
