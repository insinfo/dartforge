//! The nineteenth batch of lint rules (docs/ANALYZER-ESPECIFICACAO-INFRA.md
//! §8), written directly from the 3.6.2 emitters
//! (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`), through the
//! unit's semantics:
//!
//! * `prefer_const_constructors`: the creation that can be `const`
//!   (`Semantics::can_be_const`), except a deferred prefix type, an
//!   `@literal` constructor, `Object`'s and a generic class without type
//!   arguments whose approximate context has a type parameter.
//! * `prefer_const_literals_to_create_immutables`: the literal that can be
//!   `const`, an argument (through parentheses, literals, entries and named
//!   ones) of a creation of an `@immutable` class (on it or along the
//!   superclass chain).
//! * `prefer_const_constructors_in_immutables`: the empty-bodied constructor
//!   of an `@immutable` class without mixins that can be `const`, or a
//!   factory that redirects to a `const` one; and an `@immutable` extension
//!   type without `const`.
//! * `use_to_and_as_if_applicable`: the parameterless method that returns
//!   `C(this)`.
//! * `use_decorated_box`, `use_colored_box` and `sized_box_for_whitespace`:
//!   the arguments of a `Container`.
//! * `no_logic_in_create_state`: the `createState` of a `StatefulWidget` that
//!   does not return just a creation without arguments.
//! * `use_setters_to_change_properties`: a `void` method with one parameter
//!   that only assigns the parameter to a field.
//!
//! Written without compiling or running (2026-10-05).

use super::walk::Node;
use super::codes_g as c;
use super::rules::LintReport;
use super::LintCode;
use crate::Unit;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionKind, FunctionRef};
use dartforge_frontend::ast::{self, AssignOp, Ast, CollectionElement, DeclId, DeclKind, ExprId, ExprKind, FunctionBody, Initializer, MemberKind, StmtKind, TypeKind};
use dartforge_intern::Interner;
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::Type;
use std::collections::HashSet;

fn unparenthesized(a: &Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

fn class_of_decl(s: &super::Semantics<'_>, d: DeclId) -> Option<ClassId> {
    (0..s.program.classes.len()).map(|i| ClassId(i as u32)).find(|c| s.program.class(*c).decl.is_some_and(|r| r.unit == s.unit && r.decl == d))
}

/// The class has `@immutable` (the element's `hasImmutable`).
fn is_immutable(s: &super::Semantics<'_>, interner: &Interner, c: ClassId) -> bool {
    let Some(r) = s.program.class(c).decl else { return false };
    s.program.unit(r.unit).ast.decl(r.decl).metadata.iter().any(|m| dartforge_types::annotations::is_getter_of(s.program, interner, r.unit, m, "meta", "immutable"))
}

/// The class or its superclass chain has `@immutable`.
fn is_immutable_in_chain(s: &super::Semantics<'_>, interner: &Interner, c: ClassId) -> bool {
    let mut seen = HashSet::new();
    let mut current = Some(c);
    while let Some(x) = current {
        if !seen.insert(x) {
            break;
        }
        if is_immutable(s, interner, x) {
            return true;
        }
        current = s.program.class(x).supertype_class;
    }
    false
}

fn token_at(source: &str, pos: usize) -> Span {
    let b = source.as_bytes();
    let mut end = pos;
    while end < b.len() && (b[end].is_ascii_alphanumeric() || b[end] == b'_' || b[end] == b'$') {
        end += 1;
    }
    if end == pos {
        end = (pos + 1).min(b.len());
    }
    Span { start: pos, end }
}

fn constructor_name_span(a: &Ast, e: ExprId) -> Span {
    match &a.expr(e).kind {
        ExprKind::InstanceCreation { ty, constructor, .. } => {
            let t = a.ty(*ty).span;
            Span { start: t.start, end: constructor.map_or(t.end, |n| n.span.end) }
        }
        ExprKind::Call { target, .. } => a.expr(*target).span,
        _ => a.expr(e).span,
    }
}

/// `lookUpInheritedMethod`: a (non-static, accessible) method along the
/// superclass chain with the mixins, without the class itself.
fn has_inherited_method(s: &super::Semantics<'_>, interner: &Interner, class_id: ClassId, name: dartforge_intern::SymbolId) -> bool {
    let program = s.program;
    let lib = program.unit(s.unit).library;
    let is_private = interner.resolve(name).starts_with('_');
    let mut seen: HashSet<ClassId> = HashSet::new();
    let mut current = Some(class_id);
    while let Some(cl) = current {
        if !seen.insert(cl) {
            break;
        }
        let e = program.class(cl);
        for cc in std::iter::once(cl).chain(e.mixin_classes.iter().rev().copied()) {
            if cc == class_id {
                continue;
            }
            let k = program.class(cc);
            if let Some(&g) = k.instance_members.get(&name) {
                let ge = program.function(g);
                if matches!(ge.kind, FunctionKind::Function | FunctionKind::Operator) && !ge.static_ && (!is_private || k.library == lib) {
                    return true;
                }
            }
        }
        current = e.supertype_class;
    }
    false
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unit<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let Some(s) = sem else { return out };
    let a = u.ast;
    let source = u.source;
    let program = s.program;
    let table = s.table;
    let lib = program.unit(s.unit).library;
    let mut report = |code: &'static LintCode, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    let empty = dartforge_types::typed_lints6::CanBeConst::default();
    let can = s.can_be_const.unwrap_or(&empty);

    // `prefer_const_constructors`.
    if enabled("prefer_const_constructors") {
        let parents = super::rules16::unit_parents(u, &|e| can.creations.contains(&e));
        let mut creations: Vec<ExprId> = can.creations.iter().copied().collect();
        creations.sort_by_key(|e| a.expr(*e).span.start);
        for e in creations {
            let Some(Resolved::Constructor(f)) = s.body.get_resolved(e) else { continue };
            let g = program.function(*f);
            // The type with a deferred prefix.
            let prefix = match &a.expr(e).kind {
                ExprKind::InstanceCreation { ty, .. } => match &a.ty(*ty).kind {
                    TypeKind::Named { name, .. } if name.len() == 2 => Some(name[0].sym),
                    _ => None,
                },
                ExprKind::Call { target, .. } => {
                    let mut t = *target;
                    if let ExprKind::TypeArguments { target: t2, .. } = &a.expr(t).kind {
                        t = *t2;
                    }
                    // `p.C()` or `p.C.name()`.
                    match &a.expr(t).kind {
                        ExprKind::Property { target: t3, .. } => match &a.expr(*t3).kind {
                            ExprKind::Identifier(p) if matches!(program.lookup_in_unit(s.unit, p.sym).and_then(|b| b.getter), Some(Element::Prefix(..))) => Some(p.sym),
                            ExprKind::Property { target: t4, .. } => match &a.expr(*t4).kind {
                                ExprKind::Identifier(p) => Some(p.sym),
                                _ => None,
                            },
                            _ => None,
                        },
                        _ => None,
                    }
                }
                _ => None,
            };
            let deferred = prefix.is_some_and(|p| program.library(lib).imports.iter().any(|i| i.unit == s.unit && i.prefix == Some(p) && i.deferred));
            if deferred {
                continue;
            }
            // `@literal` on the constructor.
            let is_literal = match g.node {
                FunctionRef::Constructor { unit, member } => program
                    .unit(unit)
                    .ast
                    .member(member)
                    .metadata
                    .iter()
                    .any(|m| dartforge_types::annotations::is_getter_of(program, interner, unit, m, "meta", "literal")),
                _ => false,
            };
            if is_literal {
                continue;
            }
            let Some(class_id) = g.class else { continue };
            if Some(class_id) == s.core.object_class {
                continue;
            }
            // A generic class without written type arguments: the context.
            let has_type_arguments = match &a.expr(e).kind {
                ExprKind::InstanceCreation { ty, .. } => matches!(&a.ty(*ty).kind, TypeKind::Named { args, .. } if !args.is_empty()),
                ExprKind::Call { target, .. } => matches!(a.expr(*target).kind, ExprKind::TypeArguments { .. }),
                _ => false,
            };
            let params = s.outline.classes.get(class_id.0 as usize).map(|x| x.type_params.len()).unwrap_or(0);
            if params > 0 && !has_type_arguments {
                let ctx = super::rules16::approximate_context(s, interner, a, &parents, e);
                if let super::rules16::ApproxContext::Typed(t) = ctx
                    && let Type::Interface { class: cc, args: cargs, .. } = table.get(t)
                {
                    // `asInstanceOf(class)`: the arguments seen in the class.
                    let type_args: Option<Vec<dartforge_types::table::TypeId>> = if *cc == class_id {
                        Some(cargs.to_vec())
                    } else {
                        s.outline.hierarchy.get(*cc).and_then(|d| {
                            let template = *d.supertypes.get(&class_id)?;
                            let Type::Interface { args: margs, .. } = table.get(template) else { return None };
                            Some(
                                margs
                                    .iter()
                                    .map(|m| match table.get(*m) {
                                        Type::TypeParameter { param, .. } => d.type_params.iter().position(|q| q == param).and_then(|j| cargs.get(j).copied()).unwrap_or(*m),
                                        _ => *m,
                                    })
                                    .collect(),
                            )
                        })
                    };
                    if type_args.is_some_and(|v| v.iter().any(|x| matches!(table.get(*x), Type::TypeParameter { .. } | Type::Intersection { .. }))) {
                        continue;
                    }
                }
            }
            report(&c::PREFER_CONST_CONSTRUCTORS, a.expr(e).span, &[]);
        }
    }

    // `prefer_const_literals_to_create_immutables`.
    if enabled("prefer_const_literals_to_create_immutables") {
        let parents = super::rules16::unit_parents(u, &|_| false);
        let mut literals: Vec<ExprId> = can.literals.iter().copied().collect();
        literals.sort_by_key(|e| a.expr(*e).span.start);
        for lit in literals {
            // Goes up through parentheses, arguments, literals, entries and
            // named ones to a creation.
            let mut node = lit;
            let creation = loop {
                let Some(Node::Expr(p)) = parents.expr.get(&node).copied() else { break None };
                if matches!(s.body.get_resolved(p), Some(Resolved::Constructor(_))) {
                    // Argument of the creation (its `ArgumentList`).
                    let is_argument = match &a.expr(p).kind {
                        ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => arguments.args.iter().any(|x| x.value == node),
                        _ => false,
                    };
                    break if is_argument { Some(p) } else { None };
                }
                let passes = match &a.expr(p).kind {
                    ExprKind::Parenthesized(_) => true,
                    ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => elements.iter().any(|el| match el {
                        CollectionElement::Expression(x) => *x == node,
                        CollectionElement::MapEntry { key, value, .. } => *key == node || *value == node,
                        _ => false,
                    }),
                    _ => false,
                };
                if !passes {
                    break None;
                }
                node = p;
            };
            let Some(cr) = creation else { continue };
            let Some(t) = s.body.get_type(cr) else { continue };
            let Type::Interface { class, .. } = table.get(t) else { continue };
            if is_immutable_in_chain(s, interner, *class) {
                report(&c::PREFER_CONST_LITERALS_TO_CREATE_IMMUTABLES, a.expr(lit).span, &[]);
            }
        }
    }

    // `prefer_const_constructors_in_immutables`.
    if enabled("prefer_const_constructors_in_immutables") {
        for (k, d) in a.decls.iter().enumerate() {
            let (members, with_mixins) = match &d.kind {
                DeclKind::Class(x) => (&x.members, !x.with.is_empty() || x.modifiers.macro_),
                DeclKind::Enum(x) => (&x.members, !x.with.is_empty()),
                DeclKind::ExtensionType(x) => {
                    // An `@immutable` extension type without `const`.
                    if !x.const_
                        && let Some(cl) = class_of_decl(s, DeclId(k as u32))
                        && is_immutable(s, interner, cl)
                    {
                        report(&c::PREFER_CONST_CONSTRUCTORS_IN_IMMUTABLES, x.name.span, &[]);
                    }
                    (&x.members, false)
                }
                _ => continue,
            };
            if with_mixins {
                continue;
            }
            let Some(class_id) = class_of_decl(s, DeclId(k as u32)) else { continue };
            if !is_immutable_in_chain(s, interner, class_id) {
                continue;
            }
            let const_constructor = |cl: ClassId, name: Option<dartforge_intern::SymbolId>| -> bool {
                let key = name.or_else(|| interner.lookup(""));
                key.and_then(|ch| program.class(cl).constructors.get(&ch)).is_some_and(|f| program.function(*f).const_)
            };
            for &mid in members {
                let m = a.member(mid);
                let MemberKind::Constructor(kc) = &m.kind else { continue };
                if kc.const_ || !matches!(kc.body, FunctionBody::Empty) {
                    continue;
                }
                let first = match m.metadata.last() {
                    Some(x) => dartforge_frontend::source::skip_whitespace(source.as_bytes(), x.span.end),
                    None => m.span.start,
                };
                // The redirecting factory.
                if kc.factory
                    && let Some(r) = &kc.redirect
                {
                    let target_class = s.outline.written_types.get(&(s.unit, r.ty)).and_then(|t| match table.get(*t) {
                        Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => Some(*class),
                        _ => None,
                    });
                    if target_class.is_some_and(|cl| const_constructor(cl, r.constructor.map(|n| n.sym))) {
                        report(&c::PREFER_CONST_CONSTRUCTORS_IN_IMMUTABLES, token_at(source, first), &[]);
                    }
                    continue;
                }
                // `_hasConstConstructorInvocation`.
                let mut invokes_const = None;
                for i in kc.initializers.iter() {
                    match i {
                        Initializer::Super { constructor, .. } if invokes_const.is_none() => {
                            invokes_const = Some(program.class(class_id).supertype_class.is_some_and(|sc| const_constructor(sc, constructor.map(|n| n.sym))));
                        }
                        _ => {}
                    }
                }
                if invokes_const.is_none() {
                    for i in kc.initializers.iter() {
                        if let Initializer::Redirect { constructor, .. } = i {
                            invokes_const = Some(const_constructor(class_id, constructor.map(|n| n.sym)));
                            break;
                        }
                    }
                }
                let invokes_const = invokes_const.unwrap_or_else(|| match &d.kind {
                    DeclKind::ExtensionType(x) => x.const_,
                    _ => program.class(class_id).supertype_class.is_some_and(|sc| const_constructor(sc, None)),
                });
                if invokes_const && can.constructors.contains(&mid) {
                    report(&c::PREFER_CONST_CONSTRUCTORS_IN_IMMUTABLES, token_at(source, first), &[]);
                }
            }
        }
    }

    // `use_to_and_as_if_applicable`.
    if enabled("use_to_and_as_if_applicable") {
        let prefix_ok = |n: &str| {
            ["to", "as", "_to", "_as"].iter().any(|p| n.strip_prefix(p).and_then(|r| r.chars().next()).is_some_and(|c| c.is_ascii_uppercase()))
        };
        for (k, d) in a.decls.iter().enumerate() {
            let members = match &d.kind {
                DeclKind::Class(x) => &x.members,
                DeclKind::Mixin(x) => &x.members,
                DeclKind::Enum(x) => &x.members,
                DeclKind::Extension(x) => &x.members,
                DeclKind::ExtensionType(x) => &x.members,
                _ => continue,
            };
            let class_id = if matches!(d.kind, DeclKind::Extension(_)) { None } else { class_of_decl(s, DeclId(k as u32)) };
            for &mid in members {
                let MemberKind::Method(fid) = &a.member(mid).kind else { continue };
                let f = a.function(*fid);
                if f.kind == ast::FunctionKind::Getter {
                    continue;
                }
                let (Some(n), Some(ps)) = (f.name, &f.parameters) else { continue };
                if !ps.is_empty() {
                    continue;
                }
                if f.return_type.is_some_and(|t| matches!(a.ty(t).kind, TypeKind::Void)) {
                    continue;
                }
                let text = interner.resolve(n.sym);
                if prefix_ok(text) {
                    continue;
                }
                if class_id.is_some_and(|cl| has_inherited_method(s, interner, cl, n.sym)) {
                    continue;
                }
                let expr = match &f.body {
                    FunctionBody::Expression(e) => Some(*e),
                    FunctionBody::Block(b) => match &a.stmt(*b).kind {
                        StmtKind::Block(cmds) if cmds.len() == 1 => match &a.stmt(cmds[0]).kind {
                            StmtKind::Return(v) => *v,
                            _ => None,
                        },
                        _ => None,
                    },
                    _ => None,
                };
                let Some(e) = expr.map(|e| unparenthesized(a, e)) else { continue };
                if !matches!(s.body.get_resolved(e), Some(Resolved::Constructor(_))) {
                    continue;
                }
                let args = match &a.expr(e).kind {
                    ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => &**arguments,
                    _ => continue,
                };
                if let [x] = &args.args[..]
                    && x.name.is_none()
                    && matches!(a.expr(x.value).kind, ExprKind::This)
                {
                    report(&c::USE_TO_AND_AS_IF_APPLICABLE, n.span, &[]);
                }
            }
        }
    }

    // `use_decorated_box`, `use_colored_box` and `sized_box_for_whitespace`.
    let boxes = enabled("use_decorated_box") || enabled("use_colored_box") || enabled("sized_box_for_whitespace");
    if boxes {
        for (k, _) in a.exprs.iter().enumerate() {
            let e = ExprId(k as u32);
            if !matches!(s.body.get_resolved(e), Some(Resolved::Constructor(_))) {
                continue;
            }
            if !s.body.get_type(e).is_some_and(|t| super::flutter::is_container(s, interner, t)) {
                continue;
            }
            let args = match &a.expr(e).kind {
                ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => &**arguments,
                _ => continue,
            };
            // All named.
            if args.args.iter().any(|x| x.name.is_none()) {
                continue;
            }
            let names: Vec<(&str, ExprId)> = args.args.iter().map(|x| (interner.resolve(x.name.expect("named").sym), x.value)).collect();
            let only = |allowed: &[&str]| names.iter().all(|(n, _)| allowed.contains(n));
            let has = |n: &str| names.iter().any(|(x, _)| *x == n);
            if enabled("use_decorated_box") && only(&["child", "decoration", "key"]) && has("child") && has("decoration") {
                report(&c::USE_DECORATED_BOX, constructor_name_span(a, e), &[]);
            }
            if enabled("use_colored_box") {
                // `color` only counts with a non-nullable type; with a nullable
                // type, the argument falls into `case _` and silences the rule.
                let mut ok = true;
                let mut has_color = false;
                for (n, v) in &names {
                    match *n {
                        "child" | "key" => {}
                        "color" => {
                            let is_nullable = s.body.get_type(*v).is_some_and(|t| match table.get(t) {
                                Type::Interface { nullable, .. } | Type::TypeParameter { nullable, .. } | Type::ExtensionType { nullable, .. } => *nullable,
                                Type::Function { nullable, .. } | Type::Record { nullable, .. } | Type::FutureOr { nullable, .. } => *nullable,
                                Type::Null | Type::Dynamic | Type::Void => false,
                                _ => false,
                            });
                            if is_nullable {
                                ok = false;
                            } else {
                                has_color = true;
                            }
                        }
                        _ => ok = false,
                    }
                }
                if ok && has_color && has("child") {
                    report(&c::USE_COLORED_BOX, constructor_name_span(a, e), &[]);
                }
            }
            if enabled("sized_box_for_whitespace") && only(&["child", "height", "width", "key"]) {
                let (child, height, width) = (has("child"), has("height"), has("width"));
                if (child && (width || height)) || (width && height) {
                    report(&c::SIZED_BOX_FOR_WHITESPACE, constructor_name_span(a, e), &[]);
                }
            }
        }
    }

    // `no_logic_in_create_state`.
    if enabled("no_logic_in_create_state") {
        for (k, d) in a.decls.iter().enumerate() {
            let DeclKind::Class(x) = &d.kind else { continue };
            let Some(class_id) = class_of_decl(s, DeclId(k as u32)) else { continue };
            if !super::flutter::is_stateful_widget(s, interner, class_id) {
                continue;
            }
            for &mid in &x.members {
                let MemberKind::Method(fid) = &a.member(mid).kind else { continue };
                let f = a.function(*fid);
                if f.name.is_none_or(|n| interner.resolve(n.sym) != "createState") {
                    continue;
                }
                let (expr, body_span) = match &f.body {
                    FunctionBody::Block(b) => {
                        let e = match &a.stmt(*b).kind {
                            StmtKind::Block(cmds) if cmds.len() == 1 => match &a.stmt(cmds[0]).kind {
                                StmtKind::Return(v) => *v,
                                _ => None,
                            },
                            _ => None,
                        };
                        (e, Some(a.stmt(*b).span))
                    }
                    FunctionBody::Expression(e) => (Some(*e), None),
                    FunctionBody::Empty => continue,
                    _ => (None, None),
                };
                if let Some(e) = expr
                    && matches!(s.body.get_resolved(e), Some(Resolved::Constructor(_)))
                    && matches!(&a.expr(e).kind, ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } if arguments.args.is_empty())
                {
                    continue;
                }
                let report_span = match (expr, body_span) {
                    (Some(e), _) => a.expr(e).span,
                    (None, Some(sp)) => sp,
                    _ => continue,
                };
                report(&c::NO_LOGIC_IN_CREATE_STATE, report_span, &[]);
            }
        }
    }

    // `use_setters_to_change_properties`.
    if enabled("use_setters_to_change_properties") {
        for (k, d) in a.decls.iter().enumerate() {
            let members = match &d.kind {
                DeclKind::Class(x) => &x.members,
                DeclKind::Mixin(x) => &x.members,
                DeclKind::Enum(x) => &x.members,
                DeclKind::Extension(x) => &x.members,
                DeclKind::ExtensionType(x) => &x.members,
                _ => continue,
            };
            let class_id = if matches!(d.kind, DeclKind::Extension(_)) { None } else { class_of_decl(s, DeclId(k as u32)) };
            for &mid in members {
                let MemberKind::Method(fid) = &a.member(mid).kind else { continue };
                let f = a.function(*fid);
                if matches!(f.kind, ast::FunctionKind::Getter | ast::FunctionKind::Setter) {
                    continue;
                }
                let (Some(n), Some(ps)) = (f.name, &f.parameters) else { continue };
                let [p] = &ps[..] else { continue };
                if !f.return_type.is_some_and(|t| matches!(a.ty(t).kind, TypeKind::Void)) {
                    continue;
                }
                // `isOverride`: a supertype has a method with the same name.
                let overrides = class_id.is_some_and(|cl| {
                    s.outline.hierarchy.get(cl).is_some_and(|h| {
                        h.supertypes.keys().any(|sc| {
                            program.class(*sc).instance_members.get(&n.sym).is_some_and(|g| matches!(program.function(*g).kind, FunctionKind::Function | FunctionKind::Operator))
                        })
                    })
                });
                if overrides {
                    continue;
                }
                let expr = match &f.body {
                    FunctionBody::Expression(e) => Some(*e),
                    FunctionBody::Block(b) => match &a.stmt(*b).kind {
                        StmtKind::Block(cmds) if cmds.len() == 1 => match &a.stmt(cmds[0]).kind {
                            StmtKind::Expression(e) => Some(*e),
                            _ => None,
                        },
                        _ => None,
                    },
                    _ => None,
                };
                let Some(e) = expr else { continue };
                let ExprKind::Assign { op: AssignOp::Assign, target, value } = &a.expr(e).kind else { continue };
                // The right side is the parameter.
                let v = unparenthesized(a, *value);
                let is_param = matches!(&a.expr(v).kind, ExprKind::Identifier(m) if Some(m.sym) == p.name.map(|q| q.sym))
                    && matches!(s.body.get_resolved(v), Some(Resolved::Parameter { .. }));
                if !is_param {
                    continue;
                }
                // The left side writes a field.
                let field = match s.body.get_resolved(*target) {
                    Some(Resolved::Member { member: MemberRef::Variable(_), .. }) => true,
                    Some(Resolved::Member { member: MemberRef::Function(g), .. }) => {
                        let ge = program.function(*g);
                        ge.variable.is_some() && matches!(ge.node, FunctionRef::None)
                    }
                    _ => false,
                };
                if field {
                    report(&c::USE_SETTERS_TO_CHANGE_PROPERTIES, n.span, &[]);
                }
            }
        }
    }

    out
}
