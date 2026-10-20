//! The twenty-second batch of lint rules
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the 3.6.2
//! emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`),
//! through the unit's semantics:
//!
//! * `avoid_positional_boolean_parameters`: the first positional `bool` of
//!   public constructors, functions and methods that do not override, and of
//!   `Function` types.
//! * `omit_local_variable_types`: the written type equal to the initializer's
//!   (except an integer literal with a type other than `int` and a generic
//!   call that depends on it), and that of a `for-in` variable equal to the
//!   elements'.
//! * `use_key_in_widget_constructors`: a public widget without a constructor,
//!   and a public constructor that does not pass the `key` on.
//! * `avoid_type_to_string`: `Object`'s `toString` on a `Type`, with the
//!   `thisType` of the last visited class, mixin or extension.
//! * `prefer_int_literals`: an integral `double` literal where the `double`
//!   type is written.
//!
//! Written without compiling or running (2026-10-05).

use super::LintCode;
use super::codes_g as c;
use super::rules::LintReport;
use super::walk::{Node, walk};
use crate::Unit;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionKind, FunctionRef};
use dartforge_frontend::ast::{
    DeclId, DeclKind, ExprId, ExprKind, ForInTarget, ForInit, FunctionBody, Initializer,
    MemberKind, Parameter, ParameterKind, StmtKind, TypeKind,
};
use dartforge_intern::Interner;
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::{Type, TypeId};
use std::collections::{HashMap, HashSet};

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

fn is_private(interner: &Interner, s: dartforge_intern::SymbolId) -> bool {
    interner.resolve(s).starts_with('_')
}

/// The type is the class `bool` (with or without `?`).
fn is_bool(s: &super::Semantics<'_>, t: TypeId) -> bool {
    matches!(s.table.get(t), Type::Interface { class, .. } if Some(*class) == s.core.bool_class)
}

fn is_double(s: &super::Semantics<'_>, t: TypeId) -> bool {
    matches!(s.table.get(t), Type::Interface { class, .. } if Some(*class) == s.core.double_class)
}

/// The written type of an annotation.
fn written_type(s: &super::Semantics<'_>, t: dartforge_frontend::ast::TypeId) -> Option<TypeId> {
    s.body
        .annotation_types
        .get(&t)
        .copied()
        .or_else(|| s.outline.written_types.get(&(s.unit, t)).copied())
}

/// The first positional `bool` (`_isBoolean`).
fn first_bool<'a>(s: &super::Semantics<'_>, ps: &'a [Parameter]) -> Option<&'a Parameter> {
    ps.iter().find(|p| {
        p.kind != ParameterKind::Named
            && p.function_parameters.is_none()
            && p.ty
                .and_then(|t| written_type(s, t))
                .is_some_and(|t| is_bool(s, t))
    })
}

/// `lookUpInheritedMethod` (the superclass chain with the mixins, without the
/// class itself).
fn has_inherited_method(
    s: &super::Semantics<'_>,
    interner: &Interner,
    class_id: ClassId,
    name: dartforge_intern::SymbolId,
) -> bool {
    let program = s.program;
    let lib = program.unit(s.unit).library;
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
                if matches!(ge.kind, FunctionKind::Function | FunctionKind::Operator)
                    && !ge.static_
                    && (!is_private(interner, name) || k.library == lib)
                {
                    return true;
                }
            }
        }
        current = e.supertype_class;
    }
    false
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
    let source = u.source;
    let program = s.program;
    let table = s.table;
    let mut report = |code: &'static LintCode, span: Span, args: &[&str]| {
        out.push(LintReport {
            code,
            span,
            args: args.iter().map(|x| x.to_string()).collect(),
        });
    };

    // `avoid_positional_boolean_parameters`.
    if enabled("avoid_positional_boolean_parameters") {
        let mut findings: Vec<Span> = Vec::new();
        // The methods and constructors of the declarations.
        let mut methods: HashSet<dartforge_frontend::ast::FunctionId> = HashSet::new();
        for (k, d) in a.decls.iter().enumerate() {
            let members = match &d.kind {
                DeclKind::Class(x) => &x.members,
                DeclKind::Mixin(x) => &x.members,
                DeclKind::Enum(x) => &x.members,
                DeclKind::Extension(x) => &x.members,
                DeclKind::ExtensionType(x) => &x.members,
                _ => continue,
            };
            let class_id = if matches!(d.kind, DeclKind::Extension(_)) {
                None
            } else {
                class_of_decl(s, DeclId(k as u32))
            };
            let is_class = matches!(d.kind, DeclKind::Class(_));
            for &mid in members {
                let m = a.member(mid);
                if m.augment {
                    continue;
                }
                match &m.kind {
                    MemberKind::Constructor(kc) => {
                        if kc.name.is_some_and(|n| is_private(interner, n.sym)) {
                            continue;
                        }
                        if let Some(p) = first_bool(s, &kc.parameters) {
                            findings.push(p.span);
                        }
                    }
                    MemberKind::Method(fid) => {
                        methods.insert(*fid);
                        let f = a.function(*fid);
                        let Some(n) = f.name else { continue };
                        if matches!(
                            f.kind,
                            dartforge_frontend::ast::FunctionKind::Setter
                                | dartforge_frontend::ast::FunctionKind::Operator
                        ) || is_private(interner, n.sym)
                        {
                            continue;
                        }
                        if class_id.is_some_and(|cl| has_inherited_method(s, interner, cl, n.sym)) {
                            continue;
                        }
                        // `_isOverridingMember`: an inherited member with the
                        // same name (only in classes).
                        let overrides = is_class
                            && class_id.is_some_and(|cl| {
                                s.outline.hierarchy.get(cl).is_some_and(|h| {
                                    h.supertypes.keys().any(|sc| {
                                        program.class(*sc).instance_members.contains_key(&n.sym)
                                    })
                                }) || s.core.object_class.is_some_and(|o| {
                                    program.class(o).instance_members.contains_key(&n.sym)
                                })
                            });
                        if overrides {
                            continue;
                        }
                        if let Some(ps) = &f.parameters
                            && let Some(p) = first_bool(s, ps)
                        {
                            findings.push(p.span);
                        }
                    }
                    MemberKind::Field(_) => {}
                }
            }
        }
        // The declared functions (top-level and local).
        let mut declared: Vec<dartforge_frontend::ast::FunctionId> = Vec::new();
        for d in a.decls.iter() {
            if let DeclKind::Function(f) = &d.kind
                && !d.augment
            {
                declared.push(*f);
            }
        }
        for st in a.stmts.iter() {
            if let StmtKind::Function(f) = &st.kind {
                declared.push(*f);
            }
        }
        for fid in declared {
            let f = a.function(fid);
            if f.name.is_some_and(|n| is_private(interner, n.sym)) {
                continue;
            }
            if let Some(ps) = &f.parameters
                && let Some(p) = first_bool(s, ps)
            {
                findings.push(p.span);
            }
        }
        // The `Function` types.
        for t in a.types.iter() {
            if let TypeKind::Function { parameters, .. } = &t.kind
                && let Some(p) = first_bool(s, parameters)
            {
                findings.push(p.span);
            }
        }
        let _ = methods;
        findings.sort_by_key(|x| x.start);
        for sp in findings {
            report(&c::AVOID_POSITIONAL_BOOLEAN_PARAMETERS, sp, &[]);
        }
    }

    // `omit_local_variable_types`.
    if enabled("omit_local_variable_types") {
        let mut lists: Vec<&dartforge_frontend::ast::VariableList> = Vec::new();
        for st in a.stmts.iter() {
            match &st.kind {
                StmtKind::Variables(l) => lists.push(l),
                StmtKind::For {
                    init: Some(ForInit::Variables(l)),
                    ..
                } => lists.push(l),
                StmtKind::ForIn {
                    target: ForInTarget::Declared { ty: Some(ty), .. },
                    iterable,
                    ..
                } => {
                    let Some(tl) = written_type(s, *ty) else {
                        continue;
                    };
                    if matches!(table.get(tl), Type::Dynamic) {
                        continue;
                    }
                    let Some(ti) = s.body.get_type(*iterable) else {
                        continue;
                    };
                    if !matches!(table.get(ti), Type::Interface { .. }) {
                        continue;
                    }
                    let Some(iterable_class) = s.core.iterable_class else {
                        continue;
                    };
                    if super::obvious::argument_as_instance(s, ti, iterable_class, 0) == Some(tl) {
                        report(&c::OMIT_LOCAL_VARIABLE_TYPES, a.ty(*ty).span, &[]);
                    }
                }
                _ => {}
            }
        }
        for l in lists {
            let Some(ty) = l.ty else { continue };
            let Some(tl) = written_type(s, ty) else {
                continue;
            };
            if matches!(table.get(tl), Type::Dynamic | Type::Null) {
                continue;
            }
            let is_int = matches!(table.get(tl), Type::Interface { class, .. } if Some(*class) == s.core.int_class);
            let all_match = l.variables.iter().all(|v| {
                let Some(i) = v.initializer else { return false };
                if s.body.get_type(i) != Some(tl) {
                    return false;
                }
                if matches!(a.expr(i).kind, ExprKind::Int(_)) && !is_int {
                    return false;
                }
                // `dependsOnDeclaredTypeForInference`: a generic function call
                // without type arguments that returns a type parameter.
                if let ExprKind::Call { target, .. } = &a.expr(i).kind
                    && !matches!(a.expr(*target).kind, ExprKind::TypeArguments { .. })
                {
                    let function = match s.body.get_resolved(*target) {
                        Some(Resolved::Element(Element::Function(f))) => Some(*f),
                        _ => None,
                    };
                    if let Some(f) = function
                        && program.function(f).class.is_none()
                        && let Some(d) = s.outline.functions.get(f.0 as usize)
                        && matches!(table.get(d.return_type), Type::TypeParameter { .. })
                    {
                        return false;
                    }
                    // The generic local function.
                    if matches!(s.body.get_resolved(*target), Some(Resolved::Local(_)))
                        && let Some(decl) = s.body.local_declaration(*target)
                        && let Some(g) = a.stmts.iter().find_map(|x| match &x.kind {
                            StmtKind::Function(g)
                                if a.function(*g).name.is_some_and(|n| n.span.start == decl) =>
                            {
                                Some(*g)
                            }
                            _ => None,
                        })
                        && let Some(rt) = a.function(g).return_type
                        && written_type(s, rt)
                            .is_some_and(|t| matches!(table.get(t), Type::TypeParameter { .. }))
                    {
                        return false;
                    }
                }
                true
            });
            if all_match {
                report(&c::OMIT_LOCAL_VARIABLE_TYPES, a.ty(ty).span, &[]);
            }
        }
    }

    // `use_key_in_widget_constructors`.
    if enabled("use_key_in_widget_constructors") {
        let is_key = |t: TypeId| -> bool {
            let f = |k: ClassId| {
                let x = program.class(k);
                interner.resolve(x.name) == "Key"
                    && program
                        .library(x.library)
                        .name
                        .as_ref()
                        .is_none_or(|n| n.is_empty())
            };
            match table.get(t) {
                Type::Interface { class, .. } => {
                    f(*class)
                        || (program.class(*class).decl.is_some()
                            && s.outline
                                .hierarchy
                                .get(*class)
                                .is_some_and(|d| d.supertypes.keys().any(|k| f(*k))))
                }
                _ => false,
            }
        };
        // The constructor has a `key` parameter of type `Key`.
        let define_key = |f: dartforge_elements::model::FunctionElementId| -> bool {
            let FunctionRef::Constructor { unit, member } = program.function(f).node else {
                return false;
            };
            let MemberKind::Constructor(k) = &program.unit(unit).ast.member(member).kind else {
                return false;
            };
            let Some(data) = s.outline.functions.get(f.0 as usize) else {
                return false;
            };
            let Type::Function {
                positional,
                optional,
                named,
                ..
            } = table.get(data.signature)
            else {
                return false;
            };
            let mut position = 0usize;
            for p in k.parameters.iter() {
                let t = match p.kind {
                    ParameterKind::Named => p.name.and_then(|n| {
                        named
                            .iter()
                            .find(|(x, _, _)| *x == n.sym)
                            .map(|(_, t, _)| *t)
                    }),
                    _ => {
                        position += 1;
                        positional
                            .iter()
                            .chain(optional.iter())
                            .nth(position - 1)
                            .copied()
                    }
                };
                if p.name.is_some_and(|n| interner.resolve(n.sym) == "key") && t.is_some_and(is_key)
                {
                    return true;
                }
            }
            false
        };
        for (k, d) in a.decls.iter().enumerate() {
            let DeclKind::Class(x) = &d.kind else {
                continue;
            };
            let Some(class_id) = class_of_decl(s, DeclId(k as u32)) else {
                continue;
            };
            if is_private(interner, x.name.sym)
                || !super::flutter::has_widget_as_ascendant(s, interner, class_id)
            {
                continue;
            }
            let constructors: Vec<&dartforge_frontend::ast::Constructor> = x
                .members
                .iter()
                .filter_map(|m| match &a.member(*m).kind {
                    MemberKind::Constructor(kc) => Some(kc),
                    _ => None,
                })
                .collect();
            if constructors.is_empty() {
                report(&c::USE_KEY_IN_WIDGET_CONSTRUCTORS, x.name.span, &[]);
                continue;
            }
            if super::flutter::is_exactly_widget(s, interner, class_id) {
                continue;
            }
            for &mid in &x.members {
                let m = a.member(mid);
                let MemberKind::Constructor(kc) = &m.kind else {
                    continue;
                };
                if m.augment || kc.factory || kc.name.is_some_and(|n| is_private(interner, n.sym)) {
                    continue;
                }
                if kc
                    .parameters
                    .iter()
                    .any(|p| p.super_ && p.name.is_some_and(|n| interner.resolve(n.sym) == "key"))
                {
                    continue;
                }
                let sup = program.class(class_id).supertype_class;
                let silenced = kc.initializers.iter().any(|i| {
                    let (owner, name, args) = match i {
                        Initializer::Super {
                            constructor,
                            arguments,
                            ..
                        } => (sup, *constructor, arguments),
                        Initializer::Redirect {
                            constructor,
                            arguments,
                            ..
                        } => (Some(class_id), *constructor, arguments),
                        _ => return false,
                    };
                    let Some(owner) = owner else { return false };
                    let key = name.map(|n| n.sym).or_else(|| interner.lookup(""));
                    let Some(&f) = key.and_then(|ch| program.class(owner).constructors.get(&ch))
                    else {
                        return false;
                    };
                    // `_defineKeyArgument`: an argument for a `key` parameter.
                    let passes_key = args
                        .args
                        .iter()
                        .any(|x| x.name.is_some_and(|n| interner.resolve(n.sym) == "key"));
                    !define_key(f) || passes_key
                });
                if silenced {
                    continue;
                }
                let recv_type = kc.name.unwrap_or(kc.class_name).span;
                report(&c::USE_KEY_IN_WIDGET_CONSTRUCTORS, recv_type, &[]);
            }
        }
    }

    // `avoid_type_to_string`.
    if enabled("avoid_type_to_string") {
        let type_class = match table.get(s.core.type_) {
            Type::Interface { class, .. } => Some(*class),
            _ => None,
        };
        let is_type_subtype = |t: TypeId| match table.get(t) {
            Type::Interface {
                class,
                nullable: false,
                ..
            } => {
                Some(*class) == type_class
                    || type_class.is_some_and(|ty| {
                        s.outline
                            .hierarchy
                            .get(*class)
                            .is_some_and(|d| d.supertypes.contains_key(&ty))
                    })
            }
            _ => false,
        };
        // The `thisType` in effect at each position: that of the last class,
        // mixin or extension (in source order).
        let mut this_types: Vec<(usize, Option<TypeId>)> = Vec::new();
        for (k, d) in a.decls.iter().enumerate() {
            let t = match &d.kind {
                DeclKind::Class(_) | DeclKind::Mixin(_) => class_of_decl(s, DeclId(k as u32))
                    .and_then(|cl| {
                        let x = Type::Interface {
                            class: cl,
                            args: s
                                .outline
                                .classes
                                .get(cl.0 as usize)
                                .map(|c| {
                                    c.type_params
                                        .iter()
                                        .filter_map(|p| {
                                            table.intern_lookup(Type::TypeParameter {
                                                param: *p,
                                                nullable: false,
                                            })
                                        })
                                        .collect()
                                })
                                .unwrap_or_default(),
                            nullable: false,
                        };
                        table.intern_lookup(x)
                    }),
                DeclKind::Extension(x) => s
                    .outline
                    .written_types
                    .get(&(s.unit, x.on))
                    .copied()
                    .filter(|t| matches!(table.get(*t), Type::Interface { .. })),
                _ => continue,
            };
            this_types.push((d.span.start, t));
        }
        this_types.sort_by_key(|x| x.0);
        let this_at = |pos: usize| {
            this_types
                .iter()
                .rev()
                .find(|(p, _)| *p <= pos)
                .and_then(|(_, t)| *t)
        };
        let is_object_to_string = |e: ExprId| -> bool {
            match s.body.get_resolved(e) {
                Some(Resolved::Member {
                    member: MemberRef::Function(f),
                    ..
                }) => program.function(*f).class == s.core.object_class,
                _ => false,
            }
        };
        let check = |recv_type: Option<TypeId>,
                     name: dartforge_frontend::ast::Name,
                     e: ExprId,
                     findings: &mut Vec<Span>| {
            if interner.resolve(name.sym) == "toString"
                && is_object_to_string(e)
                && recv_type.is_some_and(is_type_subtype)
            {
                findings.push(name.span);
            }
        };
        let mut findings: Vec<Span> = Vec::new();
        let interface_type = |e: ExprId| {
            s.body
                .get_type(e)
                .filter(|t| matches!(table.get(*t), Type::Interface { .. }))
        };
        for e in a.exprs.iter() {
            let args = match &e.kind {
                ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => {
                    &**arguments
                }
                _ => continue,
            };
            let check_arg = |x: ExprId, findings: &mut Vec<Span>| match &a.expr(x).kind {
                ExprKind::Property { target, name, .. } => {
                    let recv_type =
                        interface_type(*target).or_else(|| this_at(a.expr(x).span.start));
                    check(recv_type, *name, x, findings);
                }
                ExprKind::Identifier(n) => check(this_at(a.expr(x).span.start), *n, x, findings),
                _ => {}
            };
            for x in args.args.iter().filter(|x| x.name.is_none()) {
                check_arg(x.value, &mut findings);
            }
            if let ExprKind::Call { target, .. } = &e.kind {
                match &a.expr(*target).kind {
                    ExprKind::Property {
                        target: t, name, ..
                    } => {
                        let recv_type = interface_type(*t).or_else(|| this_at(e.span.start));
                        check(recv_type, *name, *target, &mut findings);
                    }
                    ExprKind::Identifier(n) => {
                        check(this_at(e.span.start), *n, *target, &mut findings)
                    }
                    _ => {}
                }
            }
        }
        findings.sort_by_key(|x| x.start);
        findings.dedup();
        for sp in findings {
            report(&c::AVOID_TYPE_TO_STRING, sp, &[]);
        }
    }

    // `prefer_int_literals`.
    if enabled("prefer_int_literals") {
        let mut parent: HashMap<ExprId, Node> = HashMap::new();
        let mut ancestors_of: HashMap<ExprId, Vec<Node>> = HashMap::new();
        walk(u, &mut |node, anc| {
            if let Node::Expr(e) = node {
                if let Some(p) = anc.last() {
                    parent.insert(e, *p);
                }
                if matches!(a.expr(e).kind, ExprKind::Double(_)) {
                    ancestors_of.insert(e, anc.to_vec());
                }
            }
        });
        // The written return type is `double` (declared function or method).
        let returns_double = |f: dartforge_frontend::ast::FunctionId| -> bool {
            let is_literal = a
                .exprs
                .iter()
                .any(|e| matches!(e.kind, ExprKind::FunctionExpression(g) if g == f));
            if is_literal {
                return false;
            }
            a.function(f)
                .return_type
                .and_then(|t| written_type(s, t))
                .is_some_and(|t| is_double(s, t))
        };
        let mut lits: Vec<(&ExprId, &Vec<Node>)> = ancestors_of.iter().collect();
        lits.sort_by_key(|(e, _)| a.expr(**e).span.start);
        for (&e, anc) in lits {
            let ExprKind::Double(sp) = &a.expr(e).kind else {
                continue;
            };
            let text: String = source[sp.start..sp.end]
                .chars()
                .filter(|c| *c != '_')
                .collect();
            let negative = text.starts_with('-');
            let body = text.trim_start_matches('-').trim_start();
            let Ok(v) = body.parse::<f64>() else { continue };
            if !v.is_finite() || v != v.trunc() {
                continue;
            }
            let start = if negative {
                sp.start
                    + (source[sp.start..sp.end].len()
                        - source[sp.start..sp.end]
                            .trim_start_matches('-')
                            .trim_start()
                            .len())
            } else {
                sp.start
            };
            // The node's `hasTypeDouble` (the literal, or the `-` that contains it).
            let has_double_type = match parent.get(&e).copied() {
                Some(Node::Expr(p)) => match &a.expr(p).kind {
                    ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => {
                        arguments.args.iter().any(|x| x.value == e) && s.body.parameter_types.get(&e).is_some_and(|t| is_double(s, *t))
                    }
                    ExprKind::List { type_args, elements, .. } => {
                        elements.iter().any(|el| matches!(el, dartforge_frontend::ast::CollectionElement::Expression(x) if *x == e))
                            && type_args.len() == 1
                            && written_type(s, type_args[0]).is_some_and(|t| is_double(s, t))
                    }
                    _ => false,
                },
                Some(Node::Function(f)) => matches!(a.function(f).body, FunctionBody::Expression(x) if x == e) && returns_double(f),
                Some(Node::Stmt(st)) => match &a.stmt(st).kind {
                    StmtKind::Return(Some(_)) => anc.iter().rev().find_map(|n| match n {
                        Node::Function(f) => Some(*f),
                        _ => None,
                    })
                    .is_some_and(|f| matches!(a.function(f).body, FunctionBody::Block(_)) && returns_double(f)),
                    StmtKind::Variables(l) => l.ty.and_then(|t| written_type(s, t)).is_some_and(|t| is_double(s, t)),
                    StmtKind::For { init: Some(ForInit::Variables(l)), .. } => l.ty.and_then(|t| written_type(s, t)).is_some_and(|t| is_double(s, t)),
                    _ => false,
                },
                Some(Node::Member(m)) => match &a.member(m).kind {
                    MemberKind::Field(l) => l.ty.and_then(|t| written_type(s, t)).is_some_and(|t| is_double(s, t)),
                    _ => false,
                },
                Some(Node::Decl(d)) => match &a.decl(d).kind {
                    DeclKind::Variables(l) => l.ty.and_then(|t| written_type(s, t)).is_some_and(|t| is_double(s, t)),
                    _ => false,
                },
                _ => false,
            };
            if has_double_type {
                report(&c::PREFER_INT_LITERALS, Span { start, end: sp.end }, &[]);
            }
        }
    }

    out
}
