//! The thirteenth batch of lint rules that only look at the tree
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the 3.6.2
//! emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `avoid_shadowing_type_parameters` and `parameter_assignments`.
//!
//! `parameter_assignments` links the identifier and the pattern variable to
//! the parameter by element, and "potentially mutated" is the analyzer's
//! (`super::is_mutated`); it needs the unit's semantics. The exemption of the
//! `_` wildcard in `avoid_shadowing_type_parameters` depends on the 3.7
//! feature, which a 3.6 library does not enable.
//! Written without compiling or running (2026-10-05).

use super::codigos_g as c;
use super::rules::LintReport;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    Ast, DeclKind, ExprKind, FunctionBody, FunctionId, ListPatternElement, MemberKind, ParameterKind, PatternId, PatternKind, StmtKind,
    TypeKind, TypeParameter, TypedefKind,
};
use dartforge_intern::Interner;

fn within(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

/// The body's region (`None` for a `;` body).
fn body_region(a: &Ast, b: &FunctionBody) -> Option<Span> {
    match b {
        FunctionBody::Block(s) => Some(a.stmt(*s).span),
        FunctionBody::Expression(e) => Some(a.expr(*e).span),
        _ => None,
    }
}

/// The type parameter containers that enclose `span`, from the inside out:
/// the type declarations, the declared functions and the methods.
fn enclosing_containers<'x>(
    a: &'x Ast,
    containers: &[(Span, &'x [TypeParameter], &'static str)],
    declared: &[FunctionId],
    methods: &[(FunctionId, bool)],
    span: Span,
) -> Vec<(Span, &'x [TypeParameter], &'static str)> {
    let mut v: Vec<(Span, &'x [TypeParameter], &'static str)> = Vec::new();
    for &(r, tps, kind) in containers {
        if within(span, r) && r != span {
            v.push((r, tps, kind));
        }
    }
    for &f in declared {
        let r = a.function(f).span;
        if within(span, r) && r != span {
            v.push((r, &a.function(f).type_params[..], "function"));
        }
    }
    for &(f, _) in methods {
        let r = a.function(f).span;
        if within(span, r) && r != span {
            v.push((r, &a.function(f).type_params[..], "method"));
        }
    }
    // From the inside out: the smallest first.
    v.sort_by_key(|x| x.0.end - x.0.start);
    v
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unidade<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let a = u.ast;
    let mut report = |code: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `avoid_shadowing_type_parameters`.
    if enabled("avoid_shadowing_type_parameters") {
        let mut findings: Vec<(Span, String, &'static str)> = Vec::new();
        let shadows = |tps: &[TypeParameter], outer: &[TypeParameter], kind: &'static str, findings: &mut Vec<(Span, String, &'static str)>| {
            for p in tps {
                if outer.iter().any(|q| q.name.sym == p.name.sym) {
                    findings.push((p.span, interner.resolve(p.name.sym).to_string(), kind));
                }
            }
        };
        // The unit's type containers, with the region and the kind.
        let mut containers: Vec<(Span, &[TypeParameter], &'static str)> = Vec::new();
        for d in a.decls.iter() {
            match &d.kind {
                DeclKind::Class(x) => containers.push((d.span, &x.type_params[..], "class")),
                DeclKind::Enum(x) => containers.push((d.span, &x.type_params[..], "enum")),
                DeclKind::Extension(x) => containers.push((d.span, &x.type_params[..], "extension")),
                DeclKind::ExtensionType(x) => containers.push((d.span, &x.type_params[..], "extension type")),
                DeclKind::Mixin(x) => containers.push((d.span, &x.type_params[..], "mixin")),
                _ => {}
            }
        }
        // The declared functions (top-level and local) and the methods.
        let mut declared: Vec<FunctionId> = Vec::new();
        let mut methods: Vec<(FunctionId, bool)> = Vec::new();
        for d in a.decls.iter() {
            if let DeclKind::Function(f) = &d.kind {
                declared.push(*f);
            }
        }
        for s in a.stmts.iter() {
            if let StmtKind::Function(f) = &s.kind {
                declared.push(*f);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Method(f) = &m.kind {
                methods.push((*f, a.function(*f).static_));
            }
        }
        let ancestors = |span: Span| enclosing_containers(a, &containers, &declared, &methods, span);
        // A generic local function, against all ancestors.
        for s in a.stmts.iter() {
            let StmtKind::Function(f) = &s.kind else { continue };
            let f = a.function(*f);
            if f.type_params.is_empty() {
                continue;
            }
            for (_, outer, kind) in ancestors(f.span) {
                shadows(&f.type_params, outer, kind, &mut findings);
            }
        }
        // A generic instance method.
        for &(fid, is_static) in &methods {
            let f = a.function(fid);
            if f.type_params.is_empty() || is_static {
                continue;
            }
            for (_, outer, kind) in ancestors(f.span) {
                shadows(&f.type_params, outer, kind, &mut findings);
            }
        }
        // A typedef with a generic function type.
        for d in a.decls.iter() {
            if let DeclKind::Typedef(x) = &d.kind
                && let TypedefKind::Alias(t) = &x.kind
                && let TypeKind::Function { type_params, .. } = &a.ty(*t).kind
            {
                shadows(type_params, &x.type_params, "typedef", &mut findings);
            }
        }
        for (s, name, kind) in findings {
            report(&c::AVOID_SHADOWING_TYPE_PARAMETERS, s, &[&name, kind]);
        }
    }

    // `parameter_assignments`: in declared functions (top-level and local) and
    // methods, the parameter that some write changes (by element) and that is
    // the plain required positional one or has a default value (never null
    // at the start), or has no default value (optional and named ones, also
    // `required`: null at the start, and the first assignment passes). In
    // text order: the assignment to the identifier, the postfix and prefix
    // on it (only "never null"), and an assignment pattern with it in a field
    // or direct element. Needs the unit's semantics.
    if enabled("parameter_assignments")
        && let Some(sem) = sem
    {
        let mut targets: Vec<FunctionId> = Vec::new();
        for d in a.decls.iter() {
            if let DeclKind::Function(f) = &d.kind {
                targets.push(*f);
            }
        }
        for s in a.stmts.iter() {
            if let StmtKind::Function(f) = &s.kind {
                targets.push(*f);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Method(f) = &m.kind {
                targets.push(*f);
            }
        }
        let mut findings: Vec<(Span, String)> = Vec::new();
        for fid in targets {
            let f = a.function(fid);
            let (Some(ps), Some(body)) = (&f.parameters, body_region(a, &f.body)) else { continue };
            for p in ps.iter() {
                let Some(n) = p.name else { continue };
                let decl = n.span.start;
                if !super::is_mutated(sem, a, decl) {
                    continue;
                }
                // `SimpleFormalParameter` without `DefaultFormalParameter` (the
                // plain required positional one), or with a default value: never
                // null at the start; `DefaultFormalParameter` without a default
                // value: null at the start.
                let plain = p.kind == ParameterKind::Required && p.function_parameters.is_none() && !p.this_ && !p.super_;
                let never_null = plain || (p.kind != ParameterKind::Required && p.default_value.is_some());
                let starts_null = p.kind != ParameterKind::Required && p.default_value.is_none();
                if !never_null && !starts_null {
                    continue;
                }
                let is_the_param = |e: dartforge_frontend::ast::ExprId| {
                    matches!(a.expr(e).kind, ExprKind::Identifier(_)) && sem.body.declaracao_local(e) == Some(decl)
                };
                let pattern_is_the_param = |q: PatternId| sem.body.declaracoes_de_padroes.get(&q) == Some(&decl);
                let name = interner.resolve(n.sym).to_string();
                let mut already_assigned = false;
                // In source order (the `RecursiveAstVisitor`).
                let mut in_body: Vec<(usize, &dartforge_frontend::ast::Expr)> =
                    a.exprs.iter().enumerate().filter(|(_, e)| within(e.span, body)).collect();
                in_body.sort_by_key(|(_, e)| (e.span.start, std::cmp::Reverse(e.span.end)));
                for (_, e) in in_body {
                    match &e.kind {
                        ExprKind::Assign { target, .. } if is_the_param(*target) => {
                            if never_null {
                                findings.push((e.span, name.clone()));
                            } else if starts_null {
                                if already_assigned {
                                    findings.push((e.span, name.clone()));
                                }
                                already_assigned = true;
                            }
                        }
                        // Every postfix and prefix on the parameter (`p++`,
                        // `p!`, `-p`, `!p`).
                        ExprKind::Unary { operand, .. } if never_null && is_the_param(*operand) => findings.push((e.span, name.clone())),
                        ExprKind::PatternAssign { pattern, .. } => {
                            let reported = match &a.pattern(*pattern).kind {
                                PatternKind::Record { fields } | PatternKind::Object { fields, .. } => fields.iter().any(|f| pattern_is_the_param(f.pattern)),
                                PatternKind::List { elements, .. } => elements.iter().any(|el| match el {
                                    ListPatternElement::Pattern(q) => pattern_is_the_param(*q),
                                    // The `RestPatternElement` is not the variable.
                                    ListPatternElement::Rest(_) => false,
                                }),
                                PatternKind::Map { entries, .. } => entries.iter().any(|en| pattern_is_the_param(en.value)),
                                _ => false,
                            };
                            if reported {
                                findings.push((a.pattern(*pattern).span, name.clone()));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        findings.sort_by_key(|x| (x.0.start, x.0.end));
        for (s, name) in findings {
            report(&c::PARAMETER_ASSIGNMENTS, s, &[&name]);
        }
    }

    out
}
