//! O vigésimo quinto lote de regras de lint
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`), pela
//! semântica da unidade:
//!
//! * `discarded_futures`: a chamada que devolve `Future`/`FutureOr` num
//!   corpo síncrono (construtor, função, método, closure de inicializador),
//!   fora das closures assíncronas de dentro e do `unawaited`.
//! * `diagnostic_describe_all_properties`: as propriedades públicas de uma
//!   classe `Diagnosticable` que não aparecem no `debugFillProperties` nem
//!   no `debugDescribeChildren`.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionKind};
use dartforge_frontend::ast::{AsyncModifier, Ast, DeclId, DeclKind, ExprId, ExprKind, FunctionBody, MemberKind, StmtKind};
use dartforge_intern::Interner;
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::{Type, TypeId};
use std::collections::HashSet;

fn classe_da_decl(s: &super::Semantica<'_>, d: DeclId) -> Option<ClassId> {
    (0..s.program.classes.len()).map(|i| ClassId(i as u32)).find(|c| s.program.class(*c).decl.is_some_and(|r| r.unit == s.unidade && r.decl == d))
}

/// A chamada é `MethodInvocation` (e não `FunctionExpressionInvocation`).
fn e_invocacao_de_metodo(s: &super::Semantica<'_>, a: &Ast, alvo: ExprId) -> bool {
    if !matches!(a.expr(alvo).kind, ExprKind::Identifier(_) | ExprKind::Property { .. }) {
        return false;
    }
    let nao_getter = |f: dartforge_elements::model::FunctionElementId| s.program.function(f).kind != FunctionKind::Getter;
    match s.corpo.get_resolved(alvo) {
        Some(Resolved::Member { member: MemberRef::Function(f), .. }) => nao_getter(*f),
        Some(Resolved::Member { member: MemberRef::Variable(_), .. }) => false,
        Some(Resolved::Element(Element::Function(f))) => nao_getter(*f),
        Some(Resolved::Element(_)) => false,
        Some(Resolved::ExtensionMember { member, .. }) => nao_getter(*member),
        Some(Resolved::Local(_)) => s.corpo.declaracao_local(alvo).is_some_and(|d| {
            a.stmts.iter().any(|x| matches!(&x.kind, StmtKind::Function(g) if a.function(*g).name.is_some_and(|n| n.span.start == d)))
        }),
        Some(Resolved::Parameter { .. }) => false,
        _ => true,
    }
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let Some(s) = sem else { return out };
    let a = u.ast;
    let program = s.program;
    let table = s.table;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `discarded_futures`.
    if ligada("discarded_futures") {
        let e_future = |t: TypeId| -> bool {
            let r = match table.get(t) {
                Type::Function { ret, .. } => *ret,
                Type::Interface { .. } | Type::FutureOr { .. } => t,
                _ => return false,
            };
            matches!(table.get(r), Type::Interface { class, .. } if Some(*class) == s.core.future_class) || matches!(table.get(r), Type::FutureOr { .. })
        };
        let escrito_future = |t: Option<dartforge_frontend::ast::TypeId>| {
            t.and_then(|t| s.corpo.tipos_de_anotacoes.get(&t).copied().or_else(|| s.outline.tipos_escritos.get(&(s.unidade, t)).copied())).is_some_and(e_future)
        };
        let assincrono = |m: AsyncModifier| matches!(m, AsyncModifier::Async | AsyncModifier::AsyncStar);
        // Os corpos verificados: (região, ok).
        let mut corpos: Vec<Span> = Vec::new();
        let regiao = |b: &FunctionBody| match b {
            FunctionBody::Block(x) => Some(a.stmt(*x).span),
            FunctionBody::Expression(e) => Some(a.expr(*e).span),
            _ => None,
        };
        for m in a.members.iter() {
            match &m.kind {
                MemberKind::Constructor(k) => corpos.extend(regiao(&k.body)),
                MemberKind::Method(f) => {
                    let f = a.function(*f);
                    if !escrito_future(f.return_type) && !assincrono(f.modifier) {
                        corpos.extend(regiao(&f.body));
                    }
                }
                MemberKind::Field(l) => {
                    if escrito_future(l.ty) {
                        continue;
                    }
                    for v in l.variables.iter() {
                        if let Some(i) = v.initializer
                            && let ExprKind::FunctionExpression(g) = &a.expr(i).kind
                            && !assincrono(a.function(*g).modifier)
                        {
                            corpos.extend(regiao(&a.function(*g).body));
                        }
                    }
                }
            }
        }
        for d in a.decls.iter() {
            match &d.kind {
                DeclKind::Function(f) => {
                    let f = a.function(*f);
                    if !escrito_future(f.return_type) && !assincrono(f.modifier) {
                        corpos.extend(regiao(&f.body));
                    }
                }
                DeclKind::Variables(l) => {
                    if escrito_future(l.ty) {
                        continue;
                    }
                    for v in l.variables.iter() {
                        if let Some(i) = v.initializer
                            && let ExprKind::FunctionExpression(g) = &a.expr(i).kind
                            && !assincrono(a.function(*g).modifier)
                        {
                            corpos.extend(regiao(&a.function(*g).body));
                        }
                    }
                }
                _ => {}
            }
        }
        for st in a.stmts.iter() {
            if let StmtKind::Function(f) = &st.kind {
                let f = a.function(*f);
                if !escrito_future(f.return_type) && !assincrono(f.modifier) {
                    corpos.extend(regiao(&f.body));
                }
            }
        }
        // As closures assíncronas (não visitadas por dentro) e os `unawaited`.
        let assincronas: Vec<Span> = a.functions.iter().filter(|f| assincrono(f.modifier)).filter_map(|f| regiao(&f.body)).collect();
        let mut nao_visitar: Vec<Span> = assincronas.clone();
        for e in a.exprs.iter() {
            if let ExprKind::Call { target, .. } = &e.kind
                && let Some(Resolved::Element(Element::Function(f))) = s.corpo.get_resolved(*target)
                && interner.resolve(program.function(*f).name) == "unawaited"
                && program.library(program.function(*f).library).uri == "dart:async"
            {
                // O `unawaited(...)` inteiro não é descido.
                nao_visitar.push(e.span);
            }
        }
        let dentro = |sp: Span, r: Span| sp.start >= r.start && sp.end <= r.end;
        let mut achados: HashSet<(usize, usize)> = HashSet::new();
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            let ExprKind::Call { target, .. } = &e.kind else { continue };
            if matches!(s.corpo.get_resolved(id), Some(Resolved::Constructor(_))) {
                continue;
            }
            // Algum corpo verificado alcança a chamada sem passar por uma
            // closure assíncrona ou por um `unawaited` dentro dele.
            let alcancada = corpos.iter().any(|c| dentro(e.span, *c) && !nao_visitar.iter().any(|r| r != c && dentro(*r, *c) && dentro(e.span, *r)));
            if !alcancada {
                continue;
            }
            let Some(t) = s.corpo.get_type(id) else { continue };
            let retorna_future = matches!(table.get(t), Type::Interface { class, .. } if Some(*class) == s.core.future_class) || matches!(table.get(t), Type::FutureOr { .. });
            if !retorna_future {
                continue;
            }
            let sp = if e_invocacao_de_metodo(s, a, *target) {
                match &a.expr(*target).kind {
                    ExprKind::Property { name, .. } | ExprKind::Identifier(name) => name.span,
                    _ => a.expr(*target).span,
                }
            } else {
                a.expr(*target).span
            };
            achados.insert((sp.start, sp.end));
        }
        let mut v: Vec<(usize, usize)> = achados.into_iter().collect();
        v.sort();
        for (ini, fim) in v {
            relatar(&c::DISCARDED_FUTURES, Span { start: ini, end: fim }, &[]);
        }
    }

    // `diagnostic_describe_all_properties`.
    if ligada("diagnostic_describe_all_properties") {
        let implementa_diagnosticable = |cl: ClassId| {
            let f = |k: ClassId| {
                let x = program.class(k);
                interner.resolve(x.name) == "Diagnosticable" && program.library(x.library).name.as_ref().is_none_or(|n| n.is_empty())
            };
            f(cl) || s.outline.hierarchy.get(cl).is_some_and(|d| d.supertypes.keys().any(|k| f(*k)))
        };
        for (k, d) in a.decls.iter().enumerate() {
            let DeclKind::Class(x) = &d.kind else { continue };
            let Some(classe) = classe_da_decl(s, DeclId(k as u32)) else { continue };
            if !implementa_diagnosticable(classe) {
                continue;
            }
            // `_isOverridingMember`: um membro herdado de mesmo nome.
            let herdado = |n: dartforge_intern::SymbolId| {
                let mut sup: Vec<ClassId> = s.outline.hierarchy.get(classe).map(|h| h.supertypes.keys().copied().collect()).unwrap_or_default();
                sup.extend(s.core.object_class);
                sup.iter().any(|sc| program.class(*sc).instance_members.contains_key(&n))
            };
            let mut propriedades: Vec<dartforge_frontend::ast::Name> = Vec::new();
            for &mid in &x.members {
                match &a.member(mid).kind {
                    MemberKind::Method(f) => {
                        let f = a.function(*f);
                        if f.kind != dartforge_frontend::ast::FunctionKind::Getter || f.static_ {
                            continue;
                        }
                        let Some(n) = f.name else { continue };
                        let tipo = f.return_type.and_then(|t| s.outline.tipos_escritos.get(&(s.unidade, t)).copied());
                        if interner.resolve(n.sym).starts_with('_') || herdado(n.sym) || tipo.is_some_and(|t| super::regras20::propriedade_de_widget(s, interner, t, 0)) {
                            continue;
                        }
                        propriedades.push(n);
                    }
                    MemberKind::Field(l) => {
                        if l.static_ {
                            continue;
                        }
                        for (index, v) in l.variables.iter().enumerate() {
                            let r = dartforge_elements::model::VariableRef::Field { unit: s.unidade, member: mid, index };
                            let tipo = program.variables.iter().position(|y| y.node == r).and_then(|i| s.outline.variables.get(i)).and_then(|d| d.declared_type.or(d.inferred));
                            if interner.resolve(v.name.sym).starts_with('_') || herdado(v.name.sym) || tipo.is_some_and(|t| super::regras20::propriedade_de_widget(s, interner, t, 0)) {
                                continue;
                            }
                            propriedades.push(v.name);
                        }
                    }
                    MemberKind::Constructor(_) => {}
                }
            }
            if propriedades.is_empty() {
                continue;
            }
            // Os identificadores dos corpos de `debugFillProperties` e
            // `debugDescribeChildren`.
            let mut citados: Vec<String> = Vec::new();
            for &mid in &x.members {
                let MemberKind::Method(f) = &a.member(mid).kind else { continue };
                let f = a.function(*f);
                if !f.name.is_some_and(|n| matches!(interner.resolve(n.sym), "debugFillProperties" | "debugDescribeChildren")) {
                    continue;
                }
                let corpo = match &f.body {
                    FunctionBody::Block(b) => a.stmt(*b).span,
                    FunctionBody::Expression(e) => a.expr(*e).span,
                    _ => continue,
                };
                for e in a.exprs.iter() {
                    if e.span.start < corpo.start || e.span.end > corpo.end {
                        continue;
                    }
                    match &e.kind {
                        ExprKind::Identifier(n) | ExprKind::Property { name: n, .. } => citados.push(interner.resolve(n.sym).to_string()),
                        ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => {
                            for arg in arguments.args.iter() {
                                if let Some(n) = arg.name {
                                    citados.push(interner.resolve(n.sym).to_string());
                                }
                            }
                            if let ExprKind::InstanceCreation { constructor: Some(n), .. } = &e.kind {
                                citados.push(interner.resolve(n.sym).to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }
            for nome in &citados {
                let (debug, simples) = if nome.starts_with("debug") && nome.len() > 5 {
                    let resto = &nome[5..];
                    let mut cs = resto.chars();
                    let primeira = cs.next().map(|c| c.to_lowercase().collect::<String>()).unwrap_or_default();
                    (nome.clone(), format!("{primeira}{}", cs.as_str()))
                } else {
                    let mut cs = nome.chars();
                    let primeira = cs.next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default();
                    (format!("debug{primeira}{}", cs.as_str()), nome.clone())
                };
                propriedades.retain(|p| {
                    let t = interner.resolve(p.sym);
                    t != debug && t != simples
                });
            }
            for p in propriedades {
                relatar(&c::DIAGNOSTIC_DESCRIBE_ALL_PROPERTIES, p.span, &[]);
            }
        }
    }

    out
}
