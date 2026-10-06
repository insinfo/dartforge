//! `unnecessary_await_in_return` (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8),
//! escrita direto do emissor da 3.6.2: o `await` de um `Future` no retorno
//! (ou no corpo de expressão) de uma função ou método que devolve `Future`,
//! quando o tipo esperado é subtipo do de retorno; o mais próximo que manda
//! é a função, o método ou o bloco de um `try` (que cala).
//! Escrito sem compilar nem executar (2026-10-05).

use crate::lints_tipados::Achado;
use crate::resolve::OutlineTypes;
use crate::resolved::UnitBodyTypes;
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{FunctionRef, Program, UnitId};
use dartforge_frontend::ast::{ExprId, ExprKind, FunctionBody, StmtKind};
use std::collections::HashMap;

fn sem_parenteses(a: &dartforge_frontend::ast::Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

/// Os achados na unidade `u`.
pub fn achados(program: &Program, table: &mut TypeTable, core: &CoreTypes, outline: &OutlineTypes, corpo: &UnitBodyTypes, u: UnitId) -> Vec<Achado> {
    let mut out = Vec::new();
    let a = &program.unit(u).ast;
    let e_future = |table: &TypeTable, t: TypeId| matches!(table.get(t), Type::Interface { class, .. } if Some(*class) == core.future_class);
    // Os retornos: (o nó que procura o ancestral, a expressão).
    let mut casos: Vec<(Span, ExprId)> = Vec::new();
    for f in a.functions.iter() {
        if let FunctionBody::Expression(e) = &f.body {
            casos.push((a.expr(*e).span, *e));
        }
    }
    for s in a.stmts.iter() {
        if let StmtKind::Return(Some(e)) = &s.kind {
            casos.push((s.span, *e));
        }
    }
    if casos.is_empty() {
        return out;
    }
    // Os blocos de `try` (o corpo e o `finally`).
    let mut blocos_de_try: Vec<Span> = Vec::new();
    for s in a.stmts.iter() {
        if let StmtKind::Try { body, finally_, .. } = &s.kind {
            blocos_de_try.push(a.stmt(*body).span);
            if let Some(f) = finally_ {
                blocos_de_try.push(a.stmt(*f).span);
            }
        }
    }
    let literais: HashMap<dartforge_frontend::ast::FunctionId, ExprId> = a
        .exprs
        .iter()
        .enumerate()
        .filter_map(|(k, e)| match &e.kind {
            ExprKind::FunctionExpression(f) => Some((*f, ExprId(k as u32))),
            _ => None,
        })
        .collect();
    let elementos: HashMap<dartforge_frontend::ast::FunctionId, usize> = program
        .functions
        .iter()
        .enumerate()
        .filter_map(|(k, f)| match f.node {
            FunctionRef::Function { unit, function } if unit == u => Some((function, k)),
            _ => None,
        })
        .collect();
    for (no, e) in casos {
        let x = sem_parenteses(a, e);
        let ExprKind::Await(alvo) = &a.expr(x).kind else { continue };
        let Some(t) = corpo.get_type(*alvo).filter(|t| !core.is_unknown(table, *t)) else { continue };
        if !e_future(table, t) {
            continue;
        }
        // O ancestral mais próximo: uma função (ou método) ou um bloco de
        // `try`.
        let funcao = a
            .functions
            .iter()
            .enumerate()
            .filter(|(_, f)| f.span.start <= no.start && no.end <= f.span.end)
            .min_by_key(|(_, f)| f.span.end - f.span.start)
            .map(|(k, f)| (dartforge_frontend::ast::FunctionId(k as u32), f.span));
        let bloco = blocos_de_try.iter().filter(|b| b.start <= no.start && no.end <= b.end).min_by_key(|b| b.end - b.start).copied();
        let Some((fid, fspan)) = funcao else { continue };
        if bloco.is_some_and(|b| b.end - b.start < fspan.end - fspan.start && b.start >= fspan.start) {
            continue;
        }
        // O tipo de retorno do elemento.
        let retorno = if let Some(&k) = elementos.get(&fid) {
            outline.functions.get(k).map(|d| d.return_type)
        } else if let Some(&fe) = literais.get(&fid) {
            match corpo.get_type(fe).map(|t| table.get(t).clone()) {
                Some(Type::Function { ret, .. }) => Some(ret),
                _ => None,
            }
        } else {
            a.function(fid).name.and_then(|n| corpo.tipo_local(n.span.start)).and_then(|t| match table.get(t) {
                Type::Function { ret, .. } => Some(*ret),
                _ => None,
            })
        };
        let Some(r) = retorno else { continue };
        if !e_future(table, r) {
            continue;
        }
        let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
        if is_subtype(t, r, &mut env) {
            let ini = a.expr(x).span.start;
            out.push((Span { start: ini, end: ini + "await".len() }, "unnecessary_await_in_return", Vec::new()));
        }
    }
    out
}
