//! The twelfth batch of lint rules that only look at the tree and the text
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the 3.6.2
//! emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `prefer_if_elements_to_conditional_expressions`,
//! `prefer_for_elements_to_map_fromIterable`, `prefer_final_parameters`,
//! `prefer_foreach` and `prefer_asserts_in_initializer_lists`.
//!
//! With the same data as the original (those that need the element only
//! report with the unit's semantics):
//! - `prefer_for_elements_to_map_fromIterable`: the resolved constructor of
//!   `dart:core`'s `Map`; the required parameter also named.
//! - `prefer_final_parameters`: the mutation by element.
//! - `prefer_foreach`: the argument and the target by element; single-statement
//!   blocks are traversed.
//! - `prefer_asserts_in_initializer_lists`: the instance methods and accessors
//!   of the classes of the set by element (also as a property name), the set
//!   of the last visited class, the fields of the `this.x`s (and through
//!   `super.x`).
//! Written without compiling or running (2026-10-05).

use super::codigos_g as c;
use super::rules::LintReport;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, Ast, CollectionElement, DeclKind, ExprId, ExprKind, ForInTarget, FunctionBody, MemberKind, Parameter, StmtKind};
use dartforge_intern::Interner;

fn within(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

fn unparenthesized(a: &Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = a.expr(e).kind {
        e = x;
    }
    e
}

/// The body of a single expression (`=> e`, or `{ return e; }`).
fn is_single_expression_body(a: &Ast, b: &FunctionBody) -> bool {
    match b {
        FunctionBody::Expression(_) => true,
        FunctionBody::Block(s) => match &a.stmt(*s).kind {
            StmtKind::Block(l) => matches!(&l[..], [x] if matches!(a.stmt(*x).kind, StmtKind::Return(_))),
            _ => false,
        },
        _ => false,
    }
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unidade<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let a = u.ast;
    let source = u.fonte;
    let mut report = |code: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `prefer_if_elements_to_conditional_expressions`: `c ? a : b` (maybe in
    // parentheses) as a direct element of a list or set.
    if enabled("prefer_if_elements_to_conditional_expressions") {
        for e in a.exprs.iter() {
            let (elems, is_set) = match &e.kind {
                ExprKind::List { elements, .. } => (elements, true),
                ExprKind::SetOrMap { elements, type_args, .. } => {
                    let is_map = type_args.len() == 2
                        || (type_args.is_empty()
                            && (elements.is_empty() || elements.iter().any(|x| matches!(x, CollectionElement::MapEntry { .. }))));
                    (elements, !is_map)
                }
                _ => continue,
            };
            if !is_set {
                continue;
            }
            for el in elements_iter(elems) {
                if let CollectionElement::Expression(x) = el
                    && matches!(a.expr(unparenthesized(a, *x)).kind, ExprKind::Conditional { .. })
                {
                    report(&c::PREFER_IF_ELEMENTS_TO_CONDITIONAL_EXPRESSIONS, a.expr(*x).span, &[]);
                }
            }
        }
    }

    // `prefer_for_elements_to_map_fromIterable`: the creation (with or without
    // `new`) resolved to the `fromIterable` constructor of `dart:core`'s
    // `Map`, with three arguments and the `key:` and `value:` closures of one
    // required parameter and an expression body. Needs the unit's semantics.
    if enabled("prefer_for_elements_to_map_fromIterable")
        && let Some(s) = sem
    {
        let is_closure = |name: &str, arg: &ast::Argument| -> bool {
            if arg.name.is_none_or(|n| interner.resolve(n.sym) != name) {
                return false;
            }
            let ExprKind::FunctionExpression(f) = a.expr(unparenthesized(a, arg.value)).kind else { return false };
            let f = a.function(f);
            // `isRequired`: a required positional or a `required` named one.
            let one_required = f.parameters.as_ref().is_some_and(|ps| {
                ps.len() == 1 && (ps[0].kind == ast::ParameterKind::Required || (ps[0].kind == ast::ParameterKind::Named && ps[0].required))
            });
            one_required && is_single_expression_body(a, &f.body)
        };
        for (k, e) in a.exprs.iter().enumerate() {
            let arguments = match &e.kind {
                ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => arguments,
                _ => continue,
            };
            let Some(dartforge_types::resolved::Resolved::Constructor(f)) = s.body.get_resolved(ExprId(k as u32)) else { continue };
            let g = s.program.function(*f);
            if interner.resolve(g.name) != "fromIterable" || g.class != s.core.map_class {
                continue;
            }
            let [_, second, third] = &arguments.args[..] else { continue };
            let key = is_closure("key", second) || is_closure("key", third);
            let value = is_closure("value", third) || is_closure("value", second);
            if key && value {
                report(&c::PREFER_FOR_ELEMENTS_TO_MAP_FROMITERABLE, e.span, &[]);
            }
        }
    }

    // `prefer_final_parameters`: the parameter (not `final`, `const`, `this.x`
    // or `super.x`) of a constructor, function or method that no write
    // changes (by element). Needs the unit's semantics.
    if enabled("prefer_final_parameters")
        && let Some(s) = sem
    {
        let mut findings: Vec<(Span, String)> = Vec::new();
        let mut check = |ps: &[Parameter]| {
            for p in ps {
                if p.final_ || p.const_ || p.this_ || p.super_ {
                    continue;
                }
                let Some(n) = p.name else { continue };
                if super::is_mutated(s, a, n.span.start) {
                    continue;
                }
                // The parameter's node without the default value.
                let end = match p.default_value {
                    None => p.span.end,
                    Some(_) if p.function_parameters.is_some() => dartforge_frontend::fonte::fim_do_parametro_funcao(source, n.span.end),
                    Some(_) => n.span.end,
                };
                findings.push((Span { start: p.span.start, end }, interner.resolve(n.sym).to_string()));
            }
        };
        // Top-level and local functions, function expressions and methods.
        for f in a.functions.iter() {
            if let Some(ps) = &f.parameters {
                check(ps);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind
                && !k.parte_primaria
            {
                check(&k.parameters);
            }
        }
        findings.sort_by_key(|x| (x.0.start, x.0.end));
        findings.dedup_by_key(|x| x.0);
        for (x, name) in findings {
            report(&c::PREFER_FINAL_PARAMETERS, x, &[&name]);
        }
    }

    // `prefer_foreach`: the `for (… x in e)` whose body (traversing
    // single-statement blocks and parentheses) is the call with the single
    // argument `x` (by element), and, in a method call, the target does not
    // mention `x`. Needs the unit's semantics.
    if enabled("prefer_foreach")
        && let Some(s) = sem
    {
        for st in a.stmts.iter() {
            let StmtKind::ForIn { target: ForInTarget::Declared { name, .. }, body, .. } = &st.kind else { continue };
            let variable = name.span.start;
            let is_the_variable = |e: ExprId| matches!(a.expr(e).kind, ExprKind::Identifier(_)) && s.body.declaracao_local(e) == Some(variable);
            let mut body = *body;
            while let StmtKind::Block(l) = &a.stmt(body).kind {
                let [single] = &l[..] else { break };
                body = *single;
            }
            let StmtKind::Expression(x) = &a.stmt(body).kind else { continue };
            let ExprKind::Call { target, arguments } = &a.expr(unparenthesized(a, *x)).kind else { continue };
            let [arg] = &arguments.args[..] else { continue };
            if arg.name.is_some() || !is_the_variable(arg.value) {
                continue;
            }
            // In a method call, the target cannot mention the variable
            // (`_ReferenceFinder`).
            let target_mentions = match &a.expr(*target).kind {
                ExprKind::Property { target: recv, .. } => {
                    let r = a.expr(*recv).span;
                    a.exprs.iter().enumerate().any(|(j, e)| within(e.span, r) && is_the_variable(ExprId(j as u32)))
                }
                _ => false,
            };
            if !target_mentions {
                report(&c::PREFER_FOREACH, st.span, &[]);
            }
        }
    }

    // `prefer_asserts_in_initializer_lists`: the `assert`s at the start of the
    // block body of a (non-`factory`) constructor that do not need the
    // instance: neither `this`, nor an identifier (also a property name) of
    // an instance method or accessor of the classes of the set (the class of
    // the last visited class declaration, its mixins and superclasses),
    // except the accessor of the field of a `this.x` (or of the superclass
    // constructor's `this.x`, through `super.x`). Needs the unit's semantics.
    if enabled("prefer_asserts_in_initializer_lists")
        && let Some(s) = sem
    {
        use dartforge_elements::model::{ClassId, FunctionKind as FnKind};
        use dartforge_types::resolved::{MemberRef, Resolved};
        let p = s.program;
        let class_of_decl = |d: ast::DeclId| {
            p.classes.iter().position(|c| c.decl.is_some_and(|r| r.unit == s.unit && r.decl == d)).map(|i| ClassId(i as u32))
        };
        let class_set_of = |c: ClassId| {
            let mut v: std::collections::HashSet<ClassId> = std::collections::HashSet::new();
            let mut stack = vec![c];
            while let Some(k) = stack.pop() {
                if v.insert(k) {
                    stack.extend(p.class(k).mixin_classes.iter().copied());
                    stack.extend(p.class(k).supertype_class);
                }
            }
            v
        };
        let mut last_set: Option<std::collections::HashSet<ClassId>> = None;
        for &d in u.unit.declarations.iter() {
            let decl = a.decl(d);
            let members: &[ast::MemberId] = match &decl.kind {
                DeclKind::Class(x) => {
                    last_set = class_of_decl(d).map(class_set_of);
                    &x.members
                }
                DeclKind::Enum(x) => &x.members,
                DeclKind::ExtensionType(x) => &x.members,
                DeclKind::Mixin(x) => &x.members,
                _ => continue,
            };
            let in_set = |c: Option<ClassId>| c.is_some_and(|c| last_set.as_ref().is_some_and(|v| v.contains(&c)));
            for &m in members {
                let MemberKind::Constructor(k) = &a.member(m).kind else { continue };
                if k.factory {
                    continue;
                }
                let FunctionBody::Block(b) = &k.body else { continue };
                let StmtKind::Block(stmts) = &a.stmt(*b).kind else { continue };
                // The fields of the `this.x`s (and of the superclass's `this.x`s
                // through `super.x`).
                let mut fields: Vec<dartforge_elements::model::VariableId> = Vec::new();
                for (i, q) in k.parameters.iter().enumerate() {
                    let Some(n) = q.name else { continue };
                    if q.this_ {
                        fields.extend(super::class_field(s, m, n.sym));
                    } else if q.super_
                        && let Some((f, j)) = super::super_parameter(s, interner, m, i)
                        && let dartforge_elements::model::FunctionRef::Constructor { unit, member } = p.function(f).node
                        && let MemberKind::Constructor(kk) = &p.unit(unit).ast.member(member).kind
                        && let Some(qq) = kk.parameters.get(j)
                        && qq.this_
                        && let Some(nn) = qq.name
                        && let Some(super_class) = p.function(f).class
                    {
                        fields.extend(p.class(super_class).fields.iter().copied().find(|v| p.variable(*v).name == nn.sym));
                    }
                }
                let needs_instance = |e: ExprId| -> bool {
                    match s.body.get_resolved(e) {
                        Some(Resolved::Member { member: MemberRef::Function(f), .. }) => {
                            let g = p.function(*f);
                            match g.kind {
                                FnKind::Function | FnKind::Operator => !g.static_ && in_set(g.class),
                                FnKind::Getter | FnKind::Setter | FnKind::ImplicitAccessor => {
                                    !g.static_ && in_set(g.class) && !g.variable.is_some_and(|v| fields.contains(&v))
                                }
                                _ => false,
                            }
                        }
                        Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => {
                            let x = p.variable(*v);
                            !x.static_ && in_set(x.class) && !fields.contains(v)
                        }
                        _ => false,
                    }
                };
                for &st in stmts.iter() {
                    if !matches!(a.stmt(st).kind, StmtKind::Assert { .. }) {
                        break;
                    }
                    let r = a.stmt(st).span;
                    let uses = a.exprs.iter().enumerate().filter(|(_, e)| within(e.span, r)).any(|(j, e)| match &e.kind {
                        ExprKind::This => true,
                        ExprKind::Identifier(_) | ExprKind::Property { .. } => needs_instance(ExprId(j as u32)),
                        _ => false,
                    });
                    if !uses {
                        report(&c::PREFER_ASSERTS_IN_INITIALIZER_LISTS, Span { start: r.start, end: r.start + "assert".len() }, &[]);
                    }
                }
            }
        }
    }

    out
}

/// The direct elements of a literal (those of `if` and `for` are not direct).
fn elements_iter(l: &[CollectionElement]) -> impl Iterator<Item = &CollectionElement> {
    l.iter()
}
