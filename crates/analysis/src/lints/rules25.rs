//! The twenty-fifth batch of lint rules
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the 3.6.2
//! emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`),
//! through the unit's semantics:
//!
//! * `discarded_futures`: a call that returns `Future`/`FutureOr` in a
//!   synchronous body (constructor, function, method, initializer closure),
//!   outside the async closures inside it and `unawaited`.
//! * `diagnostic_describe_all_properties`: the public properties of a
//!   `Diagnosticable` class that do not appear in `debugFillProperties` nor
//!   in `debugDescribeChildren`.
//!
//! Written without compiling or running (2026-10-05).

use super::LintCode;
use super::codes_g as c;
use super::rules::LintReport;
use crate::Unit;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionKind};
use dartforge_frontend::ast::{
    Ast, AsyncModifier, DeclId, DeclKind, ExprId, ExprKind, FunctionBody, MemberKind, StmtKind,
};
use dartforge_intern::Interner;
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::{Type, TypeId};
use std::collections::HashSet;

fn class_of_decl(s: &super::Semantics<'_>, d: DeclId) -> Option<ClassId> {
    (0..s.program.classes.len())
        .map(|i| ClassId(i as u32))
        .find(|c| {
            s.program
                .class(*c)
                .decl
                .is_some_and(|r| r.unit == s.unit && r.decl == d)
        })
}

/// The call is a `MethodInvocation` (and not a `FunctionExpressionInvocation`).
fn is_method_invocation(s: &super::Semantics<'_>, a: &Ast, callee: ExprId) -> bool {
    if !matches!(
        a.expr(callee).kind,
        ExprKind::Identifier(_) | ExprKind::Property { .. }
    ) {
        return false;
    }
    let not_getter = |f: dartforge_elements::model::FunctionElementId| {
        s.program.function(f).kind != FunctionKind::Getter
    };
    match s.body.get_resolved(callee) {
        Some(Resolved::Member { member: MemberRef::Function(f), .. }) => not_getter(*f),
        Some(Resolved::Member { member: MemberRef::Variable(_), .. }) => false,
        Some(Resolved::Element(Element::Function(f))) => not_getter(*f),
        Some(Resolved::Element(_)) => false,
        Some(Resolved::ExtensionMember { member, .. }) => not_getter(*member),
        Some(Resolved::Local(_)) => s.body.local_declaration(callee).is_some_and(|d| {
            a.stmts.iter().any(|x| matches!(&x.kind, StmtKind::Function(g) if a.function(*g).name.is_some_and(|n| n.span.start == d)))
        }),
        Some(Resolved::Parameter { .. }) => false,
        _ => true,
    }
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(
    u: Unit<'_>,
    interner: &Interner,
    enabled: &dyn Fn(&str) -> bool,
    sem: Option<&super::Semantics<'_>>,
) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let Some(s) = sem else { return out };
    let a = u.ast;
    let program = s.program;
    let table = s.table;
    let mut report = |code: &'static LintCode, span: Span, args: &[&str]| {
        out.push(LintReport {
            code,
            span,
            args: args.iter().map(|x| x.to_string()).collect(),
        });
    };

    // `discarded_futures`.
    if enabled("discarded_futures") {
        let is_future = |t: TypeId| -> bool {
            let r = match table.get(t) {
                Type::Function { ret, .. } => *ret,
                Type::Interface { .. } | Type::FutureOr { .. } => t,
                _ => return false,
            };
            matches!(table.get(r), Type::Interface { class, .. } if Some(*class) == s.core.future_class)
                || matches!(table.get(r), Type::FutureOr { .. })
        };
        let written_future = |t: Option<dartforge_frontend::ast::TypeId>| {
            t.and_then(|t| {
                s.body
                    .annotation_types
                    .get(&t)
                    .copied()
                    .or_else(|| s.outline.written_types.get(&(s.unit, t)).copied())
            })
            .is_some_and(is_future)
        };
        let is_async =
            |m: AsyncModifier| matches!(m, AsyncModifier::Async | AsyncModifier::AsyncStar);
        // The checked bodies: (region, ok).
        let mut bodies: Vec<Span> = Vec::new();
        let region_of = |b: &FunctionBody| match b {
            FunctionBody::Block(x) => Some(a.stmt(*x).span),
            FunctionBody::Expression(e) => Some(a.expr(*e).span),
            _ => None,
        };
        for m in a.members.iter() {
            match &m.kind {
                MemberKind::Constructor(k) => bodies.extend(region_of(&k.body)),
                MemberKind::Method(f) => {
                    let f = a.function(*f);
                    if !written_future(f.return_type) && !is_async(f.modifier) {
                        bodies.extend(region_of(&f.body));
                    }
                }
                MemberKind::Field(l) => {
                    if written_future(l.ty) {
                        continue;
                    }
                    for v in l.variables.iter() {
                        if let Some(i) = v.initializer
                            && let ExprKind::FunctionExpression(g) = &a.expr(i).kind
                            && !is_async(a.function(*g).modifier)
                        {
                            bodies.extend(region_of(&a.function(*g).body));
                        }
                    }
                }
            }
        }
        for d in a.decls.iter() {
            match &d.kind {
                DeclKind::Function(f) => {
                    let f = a.function(*f);
                    if !written_future(f.return_type) && !is_async(f.modifier) {
                        bodies.extend(region_of(&f.body));
                    }
                }
                DeclKind::Variables(l) => {
                    if written_future(l.ty) {
                        continue;
                    }
                    for v in l.variables.iter() {
                        if let Some(i) = v.initializer
                            && let ExprKind::FunctionExpression(g) = &a.expr(i).kind
                            && !is_async(a.function(*g).modifier)
                        {
                            bodies.extend(region_of(&a.function(*g).body));
                        }
                    }
                }
                _ => {}
            }
        }
        for st in a.stmts.iter() {
            if let StmtKind::Function(f) = &st.kind {
                let f = a.function(*f);
                if !written_future(f.return_type) && !is_async(f.modifier) {
                    bodies.extend(region_of(&f.body));
                }
            }
        }
        // The async closures (not visited inside) and the `unawaited`s.
        let async_bodies: Vec<Span> = a
            .functions
            .iter()
            .filter(|f| is_async(f.modifier))
            .filter_map(|f| region_of(&f.body))
            .collect();
        let mut skipped: Vec<Span> = async_bodies.clone();
        for e in a.exprs.iter() {
            if let ExprKind::Call { target, .. } = &e.kind
                && let Some(Resolved::Element(Element::Function(f))) = s.body.get_resolved(*target)
                && interner.resolve(program.function(*f).name) == "unawaited"
                && program.library(program.function(*f).library).uri == "dart:async"
            {
                // The whole `unawaited(...)` is not descended into.
                skipped.push(e.span);
            }
        }
        let within = |sp: Span, r: Span| sp.start >= r.start && sp.end <= r.end;
        let mut findings: HashSet<(usize, usize)> = HashSet::new();
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            let ExprKind::Call { target, .. } = &e.kind else {
                continue;
            };
            if matches!(s.body.get_resolved(id), Some(Resolved::Constructor(_))) {
                continue;
            }
            // Some checked body reaches the call without passing through an
            // async closure or an `unawaited` inside it.
            let reached = bodies.iter().any(|c| {
                within(e.span, *c)
                    && !skipped
                        .iter()
                        .any(|r| r != c && within(*r, *c) && within(e.span, *r))
            });
            if !reached {
                continue;
            }
            let Some(t) = s.body.get_type(id) else {
                continue;
            };
            let returns_future = matches!(table.get(t), Type::Interface { class, .. } if Some(*class) == s.core.future_class)
                || matches!(table.get(t), Type::FutureOr { .. });
            if !returns_future {
                continue;
            }
            let sp = if is_method_invocation(s, a, *target) {
                match &a.expr(*target).kind {
                    ExprKind::Property { name, .. } | ExprKind::Identifier(name) => name.span,
                    _ => a.expr(*target).span,
                }
            } else {
                a.expr(*target).span
            };
            findings.insert((sp.start, sp.end));
        }
        let mut v: Vec<(usize, usize)> = findings.into_iter().collect();
        v.sort();
        for (start, end) in v {
            report(&c::DISCARDED_FUTURES, Span { start, end }, &[]);
        }
    }

    // `diagnostic_describe_all_properties`.
    if enabled("diagnostic_describe_all_properties") {
        let implements_diagnosticable = |cl: ClassId| {
            let f = |k: ClassId| {
                let x = program.class(k);
                interner.resolve(x.name) == "Diagnosticable"
                    && program
                        .library(x.library)
                        .name
                        .as_ref()
                        .is_none_or(|n| n.is_empty())
            };
            f(cl)
                || s.outline
                    .hierarchy
                    .get(cl)
                    .is_some_and(|d| d.supertypes.keys().any(|k| f(*k)))
        };
        for (k, d) in a.decls.iter().enumerate() {
            let DeclKind::Class(x) = &d.kind else {
                continue;
            };
            let Some(class_id) = class_of_decl(s, DeclId(k as u32)) else {
                continue;
            };
            if !implements_diagnosticable(class_id) {
                continue;
            }
            // `_isOverridingMember`: an inherited member with the same name.
            let inherited = |n: dartforge_intern::SymbolId| {
                let mut sup: Vec<ClassId> = s
                    .outline
                    .hierarchy
                    .get(class_id)
                    .map(|h| h.supertypes.keys().copied().collect())
                    .unwrap_or_default();
                sup.extend(s.core.object_class);
                sup.iter()
                    .any(|sc| program.class(*sc).instance_members.contains_key(&n))
            };
            let mut properties: Vec<dartforge_frontend::ast::Name> = Vec::new();
            for &mid in &x.members {
                match &a.member(mid).kind {
                    MemberKind::Method(f) => {
                        let f = a.function(*f);
                        if f.kind != dartforge_frontend::ast::FunctionKind::Getter || f.static_ {
                            continue;
                        }
                        let Some(n) = f.name else { continue };
                        let ty = f
                            .return_type
                            .and_then(|t| s.outline.written_types.get(&(s.unit, t)).copied());
                        if interner.resolve(n.sym).starts_with('_')
                            || inherited(n.sym)
                            || ty.is_some_and(|t| {
                                super::rules20::is_widget_property(s, interner, t, 0)
                            })
                        {
                            continue;
                        }
                        properties.push(n);
                    }
                    MemberKind::Field(l) => {
                        if l.static_ {
                            continue;
                        }
                        for (index, v) in l.variables.iter().enumerate() {
                            let r = dartforge_elements::model::VariableRef::Field {
                                unit: s.unit,
                                member: mid,
                                index,
                            };
                            let ty = program
                                .variable_of_node(r)
                                .map(|v| v.0 as usize)
                                .and_then(|i| s.outline.variables.get(i))
                                .and_then(|d| d.declared_type.or(d.inferred));
                            if interner.resolve(v.name.sym).starts_with('_')
                                || inherited(v.name.sym)
                                || ty.is_some_and(|t| {
                                    super::rules20::is_widget_property(s, interner, t, 0)
                                })
                            {
                                continue;
                            }
                            properties.push(v.name);
                        }
                    }
                    MemberKind::Constructor(_) => {}
                }
            }
            if properties.is_empty() {
                continue;
            }
            // The identifiers of the bodies of `debugFillProperties` and
            // `debugDescribeChildren`.
            let mut mentioned: Vec<String> = Vec::new();
            for &mid in &x.members {
                let MemberKind::Method(f) = &a.member(mid).kind else {
                    continue;
                };
                let f = a.function(*f);
                if !f.name.is_some_and(|n| {
                    matches!(
                        interner.resolve(n.sym),
                        "debugFillProperties" | "debugDescribeChildren"
                    )
                }) {
                    continue;
                }
                let body = match &f.body {
                    FunctionBody::Block(b) => a.stmt(*b).span,
                    FunctionBody::Expression(e) => a.expr(*e).span,
                    _ => continue,
                };
                for e in a.exprs.iter() {
                    if e.span.start < body.start || e.span.end > body.end {
                        continue;
                    }
                    match &e.kind {
                        ExprKind::Identifier(n) | ExprKind::Property { name: n, .. } => {
                            mentioned.push(interner.resolve(n.sym).to_string())
                        }
                        ExprKind::Call { arguments, .. }
                        | ExprKind::InstanceCreation { arguments, .. } => {
                            for arg in arguments.args.iter() {
                                if let Some(n) = arg.name {
                                    mentioned.push(interner.resolve(n.sym).to_string());
                                }
                            }
                            if let ExprKind::InstanceCreation {
                                constructor: Some(n),
                                ..
                            } = &e.kind
                            {
                                mentioned.push(interner.resolve(n.sym).to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }
            for name in &mentioned {
                let (debug, simple) = if name.starts_with("debug") && name.len() > 5 {
                    let rest = &name[5..];
                    let mut cs = rest.chars();
                    let first = cs
                        .next()
                        .map(|c| c.to_lowercase().collect::<String>())
                        .unwrap_or_default();
                    (name.clone(), format!("{first}{}", cs.as_str()))
                } else {
                    let mut cs = name.chars();
                    let first = cs
                        .next()
                        .map(|c| c.to_uppercase().collect::<String>())
                        .unwrap_or_default();
                    (format!("debug{first}{}", cs.as_str()), name.clone())
                };
                properties.retain(|p| {
                    let t = interner.resolve(p.sym);
                    t != debug && t != simple
                });
            }
            for p in properties {
                report(&c::DIAGNOSTIC_DESCRIBE_ALL_PROPERTIES, p.span, &[]);
            }
        }
    }

    out
}
