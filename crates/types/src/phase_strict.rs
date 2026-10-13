//! `inference_failure_on_untyped_parameter` and
//! `inference_failure_on_function_return_type` (docs/ANALYZER-ESPECIFICACAO-INFRA.md,
//! batch II.7 and §4.6): the 3.6.2 `BestPracticesVerifier`
//! (`_checkStrictInferenceInParameters` and `_checkStrictInferenceReturnType`,
//! `analyzer/lib/src/error/best_practices_verifier.dart:1359-1431`).
//!
//! They only apply with `analyzer: language: strict-inference: true`; the pass
//! always reports and whoever publishes (`dartforge_parity::json_diagnostics`)
//! removes the two codes without the option.
//!
//! * Simple parameter without a type (not `this.`, not `super.`, not function
//!   form), referenced: the parameter node without the default value, with the
//!   name. In constructors (`visitConstructorDeclaration`), in top-level and
//!   local function declarations (`visitFunctionDeclaration`), in methods that
//!   do not override (`visitMethodDeclaration`), in function expressions whose
//!   context was not a function type (`visitFunctionExpression`), in old-style
//!   typedefs (`visitFunctionTypeAlias`) and in function-typed parameters
//!   (`visitFunctionTypedFormalParameter`, anywhere in the tree, also inside
//!   an overriding method, a function expression and a `Function(…)` type).
//! * Without a return type: the top-level function (not a setter) and the
//!   method (not a setter, not overriding), at the name; the old-style typedef
//!   and the function-typed parameter, at the node; the `Function(…)` type that
//!   is not that of a `= Function(…)` typedef, at the node, with its `toSource`
//!   (`dartforge_frontend::source`); the `= Function(…)` typedef, at the node,
//!   with the name.
//!
//! "Referenced" is `isParameterReferenced`: without a body (or with a `;` body)
//! and without an initializer list (typedef, abstract, external, function-typed
//! parameter), always; otherwise, the `_UsedParameterVisitor` over the body and
//! the initializers: an identifier whose resolved local is the parameter itself
//! (its declaration, `UnitBodyTypes::local_declaration`). A constructor always
//! has the initializer list (never null in the analyzer), so it is always
//! visited; the `native` body too.
//! "Overrides" is `getOverridden2` (`phase_override::overrides`: the key,
//! `name` or `nome_=`, in some supertype, with a private name only from the
//! same library; the analyzer's `overridden` are the inherited candidates, so
//! it also applies to a static member). A function expression is checked when
//! the context was not a function type (`UnitBodyTypes::with_function_type`,
//! recorded by the inference as `wasFunctionTypeSupplied`). Repeated reports
//! are output once, as in the analyzer's error set.
//! Written without compiling or running (2026-10-05).

use crate::resolve::OutlineTypes;
use crate::resolved::UnitBodyTypes;
use dartforge_diagnostics::codes::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{ClassId, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, ExprId, ExprKind, FunctionBody, Initializer, MemberKind, Parameter, TypeKind, TypedefKind};
use dartforge_frontend::source::function_parameter_end;
use dartforge_intern::{Interner, SymbolId};

fn inside(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

/// The regions the `_UsedParameterVisitor` walks, or `None` when
/// `isParameterReferenced` considers every parameter referenced: without a
/// body (or with the `;` body) and without an initializer list.
fn regions_of(a: &ast::Ast, body: &FunctionBody, initializers: Option<&[Initializer]>) -> Option<Vec<Span>> {
    if matches!(body, FunctionBody::Empty) && initializers.is_none() {
        return None;
    }
    let mut v = Vec::new();
    match body {
        FunctionBody::Block(s) => v.push(a.stmt(*s).span),
        FunctionBody::Expression(e) => v.push(a.expr(*e).span),
        // The `native 'x';` body only has the literal: no identifier.
        FunctionBody::Empty | FunctionBody::Native(_) => {}
    }
    for i in initializers.unwrap_or(&[]) {
        let sp = match i {
            Initializer::Assert { span, .. } | Initializer::Redirect { span, .. } | Initializer::Super { span, .. } => *span,
            Initializer::Field { value, .. } => a.expr(*value).span,
        };
        v.push(sp);
    }
    Some(v)
}

/// The reports of unit `u` (without looking at the option).
#[allow(clippy::too_many_arguments)]
pub fn inference_failures(
    program: &Program,
    interner: &Interner,
    table: &mut crate::table::TypeTable,
    core: &crate::table::CoreTypes,
    outline: &OutlineTypes,
    body: &UnitBodyTypes,
    u: UnitId,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let unit = program.unit(u);
    let a = &unit.ast;
    let source = unit.source.as_str();
    let name = |s: SymbolId| interner.resolve(s).to_string();

    // `_UsedParameterVisitor`: an identifier in the regions whose resolved
    // local is the parameter declared at `declaration`.
    let cited = |regions: &[Span], declaration: usize| {
        a.exprs.iter().enumerate().any(|(k, e)| {
            matches!(e.kind, ExprKind::Identifier(_))
                && regions.iter().any(|&r| inside(e.span, r))
                && body.local_declaration(ExprId(k as u32)) == Some(declaration)
        })
    };
    // `_checkStrictInferenceInParameters`: the simple parameters of the list
    // (also inside the `DefaultFormalParameter`).
    let params_list = |ps: &[Parameter], regions: Option<&[Span]>, out: &mut Vec<Diagnostic>| {
        for p in ps {
            if p.ty.is_some() || p.this_ || p.super_ || p.function_parameters.is_some() {
                continue;
            }
            let Some(n) = p.name else { continue };
            let referenced = match regions {
                None => true,
                Some(r) => cited(r, n.span.start),
            };
            if referenced {
                // The node of the `SimpleFormalParameter`, without the default value: the
                // name is its last token.
                let end = if p.default_value.is_some() { n.span.end } else { p.span.end };
                out.push(Diagnostic::with_code(w::INFERENCE_FAILURE_ON_UNTYPED_PARAMETER, Span { start: p.span.start, end }, [name(n.sym)]));
            }
        }
    };
    // The class of the declaration (the `InterfaceElement` that contains it).
    let decl_class = |d: ast::DeclId| -> Option<ClassId> {
        program.class_of_decl(u, d)
    };
    // `getOverridden2` (the same as in `phase_override`), by the member key.
    let lib = unit.library;
    let mut inheritance = crate::inheritance::InheritanceManager::default();
    let mut provider = crate::inheritance::OutlineProvider { program, interner, core, outline, table };
    let mut overrides =
        |c: ClassId, key: Option<SymbolId>| crate::phase_override::the_overrides(&mut inheritance, &mut provider, c, lib, key);

    // `visitFunctionDeclaration`: the top-level and the local functions.
    let mut the_declared: std::collections::HashSet<ast::FunctionId> = std::collections::HashSet::new();
    for d in a.decls.iter() {
        match &d.kind {
            DeclKind::Function(f) => {
                the_declared.insert(*f);
                let g = a.function(*f);
                // Only the top-level one has its return type checked.
                if g.return_type.is_none() && g.kind != ast::FunctionKind::Setter && let Some(n) = g.name {
                    out.push(Diagnostic::with_code(w::INFERENCE_FAILURE_ON_FUNCTION_RETURN_TYPE, n.span, [name(n.sym)]));
                }
                if let Some(ps) = &g.parameters {
                    params_list(ps, regions_of(a, &g.body, None).as_deref(), &mut out);
                }
            }
            // `visitFunctionTypeAlias`: the return type and the parameters, no body.
            DeclKind::Typedef(t) => match &t.kind {
                TypedefKind::Legacy { return_type, parameters } => {
                    if return_type.is_none() {
                        out.push(Diagnostic::with_code(w::INFERENCE_FAILURE_ON_FUNCTION_RETURN_TYPE, d.span, [name(t.name.sym)]));
                    }
                    params_list(parameters, None, &mut out);
                }
                // `visitGenericTypeAlias`.
                TypedefKind::Alias(ty) => {
                    if let TypeKind::Function { return_type: None, .. } = &a.ty(*ty).kind {
                        out.push(Diagnostic::with_code(w::INFERENCE_FAILURE_ON_FUNCTION_RETURN_TYPE, d.span, [name(t.name.sym)]));
                    }
                }
            },
            _ => {}
        }
    }
    for s in a.stmts.iter() {
        if let ast::StmtKind::Function(f) = &s.kind {
            the_declared.insert(*f);
            let g = a.function(*f);
            if let Some(ps) = &g.parameters {
                params_list(ps, regions_of(a, &g.body, None).as_deref(), &mut out);
            }
        }
    }
    // `visitMethodDeclaration` and `visitConstructorDeclaration`.
    for (di, d) in a.decls.iter().enumerate() {
        let members: &[ast::MemberId] = match &d.kind {
            DeclKind::Class(x) => &x.members,
            DeclKind::Mixin(x) => &x.members,
            DeclKind::Enum(x) => &x.members,
            DeclKind::Extension(x) => &x.members,
            DeclKind::ExtensionType(x) => &x.members,
            _ => continue,
        };
        let class = decl_class(ast::DeclId(di as u32));
        for &m in members {
            match &a.member(m).kind {
                MemberKind::Method(f) => {
                    let g = a.function(*f);
                    let Some(n) = g.name else { continue };
                    // The element's `Name`: that of a setter is `name=`.
                    let key = if g.kind == ast::FunctionKind::Setter { interner.lookup(&format!("{}_=", name(n.sym))) } else { Some(n.sym) };
                    if class.is_some_and(|c| overrides(c, key)) {
                        continue;
                    }
                    if g.return_type.is_none() && g.kind != ast::FunctionKind::Setter {
                        out.push(Diagnostic::with_code(w::INFERENCE_FAILURE_ON_FUNCTION_RETURN_TYPE, n.span, [name(n.sym)]));
                    }
                    if let Some(ps) = &g.parameters {
                        params_list(ps, regions_of(a, &g.body, None).as_deref(), &mut out);
                    }
                }
                MemberKind::Constructor(k) => {
                    params_list(&k.parameters, regions_of(a, &k.body, Some(&k.initializers[..])).as_deref(), &mut out);
                }
                MemberKind::Field(_) => {}
            }
        }
    }
    // `visitFunctionExpression`: only without a function type from the context.
    for e in a.exprs.iter() {
        let ExprKind::FunctionExpression(f) = &e.kind else { continue };
        if the_declared.contains(f) || body.with_function_type.contains(f) {
            continue;
        }
        let g = a.function(*f);
        if let Some(ps) = &g.parameters {
            params_list(ps, regions_of(a, &g.body, None).as_deref(), &mut out);
        }
    }
    // `visitFunctionTypedFormalParameter`, in every parameter list of the unit:
    // functions (top-level, local, methods, function expressions),
    // constructors, old-style typedefs and `Function(…)` types.
    let mut lists: Vec<&[Parameter]> = Vec::new();
    lists.extend(a.functions.iter().filter_map(|f| f.parameters.as_deref()));
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind {
            lists.push(&k.parameters[..]);
        }
    }
    for d in a.decls.iter() {
        if let DeclKind::Typedef(t) = &d.kind
            && let TypedefKind::Legacy { parameters, .. } = &t.kind
        {
            lists.push(&parameters[..]);
        }
    }
    for t in a.types.iter() {
        if let TypeKind::Function { parameters, .. } = &t.kind {
            lists.push(&parameters[..]);
        }
    }
    let mut stack: Vec<&Parameter> = lists.into_iter().flatten().filter(|p| p.function_parameters.is_some()).collect();
    while let Some(q) = stack.pop() {
        let Some(fs) = &q.function_parameters else { continue };
        // `this.f(…)` and `super.f(…)` are not `FunctionTypedFormalParameter`,
        // but their list is walked.
        if !q.this_
            && !q.super_
            && let Some(n) = q.name
        {
            if q.ty.is_none() {
                let end = if q.default_value.is_some() { function_parameter_end(source, n.span.end) } else { q.span.end };
                out.push(Diagnostic::with_code(w::INFERENCE_FAILURE_ON_FUNCTION_RETURN_TYPE, Span { start: q.span.start, end }, [name(n.sym)]));
            }
            params_list(fs, None, &mut out);
        }
        stack.extend(fs.iter().filter(|x| x.function_parameters.is_some()));
    }
    // `visitGenericFunctionType`: the `Function(…)` type without a return type
    // that is not that of a `= Function(…)` typedef, with `node.toString()`.
    let from_typedefs: Vec<ast::TypeId> = a
        .decls
        .iter()
        .filter_map(|d| match &d.kind {
            DeclKind::Typedef(t) => match &t.kind {
                TypedefKind::Alias(ty) => Some(*ty),
                _ => None,
            },
            _ => None,
        })
        .collect();
    for (k, t) in a.types.iter().enumerate() {
        let id = ast::TypeId(k as u32);
        if let TypeKind::Function { return_type: None, .. } = &t.kind
            && !from_typedefs.contains(&id)
        {
            let text = dartforge_frontend::source::from_type(a, source, interner, id);
            out.push(Diagnostic::with_code(w::INFERENCE_FAILURE_ON_FUNCTION_RETURN_TYPE, t.span, [text]));
        }
    }
    // The analyzer's error set: the same report is output once.
    out.sort_by_key(|d| (d.span.start, d.span.end));
    out.dedup_by(|x, y| x.span == y.span && x.code == y.code && x.message == y.message);
    out
}
