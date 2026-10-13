//! The sixteenth batch of lint rules (docs/ANALYZER-ESPECIFICACAO-INFRA.md
//! §8), written directly from the 3.6.2 emitters
//! (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`), of the
//! `recommended` set, through the unit's semantics:
//!
//! * `avoid_function_literals_in_foreach_calls`: `target.forEach((…) {…})`
//!   with an `Iterable` target, without `?.` in the chain
//!   (`containsNullAwareInvocationInChain`), without a method invocation on the
//!   target (`_hasMethodChaining`) and outside a cascade (the nearest ancestor
//!   that is a statement or a cascade).
//! * `avoid_renaming_method_parameters`: `lookUpInheritedMethod` (the
//!   superclass chain with the mixins, without the class itself) and the
//!   positional ones.
//! * `implementation_imports`: the imported library's URI with `src` in the
//!   second segment, imported from a `package:` unit of another package.
//! * `null_closures`: the emitter's tables (constructors, static and instance
//!   methods), with `extendsClass` along the superclass chain and the
//!   positional index counted over all the arguments.
//! * `prefer_collection_literals`: `Map()`, `LinkedHashMap()`, `Set()`,
//!   `LinkedHashSet()`, `Set.from([…])`/`Set.of([…])` and `[…].toSet()`, with
//!   the linter's `approximateContextType`.
//! * `prefer_interpolation_to_compose_strings`: the `+` chain
//!   (`chainedAdditions`), each binary node visited.
//! * `unnecessary_nullable_for_final_variable_declarations`: private or static
//!   fields, `final`/`const` top-level and local variables, and `final`
//!   record and list declaration patterns.
//! * `unnecessary_this`: the `this.` of a field initializer, and the `this.x`
//!   whose name the scope (`super::scope`, `resolveNameInScope`) resolves to
//!   the same element, or to nothing, or to a class's other accessor.
//! * `prefer_final_fields`: the private fields that no write in the library
//!   changes, by element.
//! * `overridden_fields`: the order of `_findAllSupertypesAndMixins` (the
//!   superclass before the class's mixins).
//!
//! Written without compiling or running (2026-10-05).

use super::walk::{walk, Node};
use super::codigos_g as c;
use super::scope::{EntryKind, Scopes};
use super::rules::LintReport;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionElementId, FunctionKind, FunctionRef, LibraryId, VariableId, VariableRef};
use dartforge_frontend::ast::{
    Ast, BinaryOp, DeclId, DeclKind, DirectiveKind, ExprId, ExprKind, ForInit, FunctionBody, Initializer, MemberKind, ParameterKind,
    PatternKind, StmtKind, UnaryOp,
};
use dartforge_intern::Interner;
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::{Type, TypeId, TypeTable};
use std::collections::{HashMap, HashSet};

fn library_name(s: &super::Semantics<'_>, interner: &Interner, l: LibraryId) -> Option<String> {
    s.program.library(l).name.as_ref().map(|n| n.iter().map(|x| interner.resolve(*x)).collect::<Vec<_>>().join("."))
}

/// `isSameAs(name, library)`: the element of the interface type.
fn is_class_named(s: &super::Semantics<'_>, interner: &Interner, t: TypeId, name: &str, library: &str) -> bool {
    match s.table.get(t) {
        Type::Interface { class, .. } => {
            let k = s.program.class(*class);
            interner.resolve(k.name) == name && library_name(s, interner, k.library).as_deref() == Some(library)
        }
        _ => false,
    }
}

/// `implementsInterface(nome, biblioteca)`.
fn implements_class(s: &super::Semantics<'_>, interner: &Interner, t: TypeId, name: &str, library: &str) -> bool {
    let (Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. }) = s.table.get(t) else { return false };
    let e = |k: ClassId| {
        let x = s.program.class(k);
        interner.resolve(x.name) == name && library_name(s, interner, x.library).as_deref() == Some(library)
    };
    e(*class) || (s.program.class(*class).decl.is_some() && s.outline.hierarchy.get(*class).is_some_and(|d| d.supertypes.keys().any(|k| e(*k))))
}

/// The call is a `MethodInvocation` in the analyzer (a name resolved to a
/// variable or a getter becomes a `FunctionExpressionInvocation`).
pub(super) fn is_method_invocation(s: &super::Semantics<'_>, a: &Ast, recv: ExprId) -> bool {
    if !matches!(a.expr(recv).kind, ExprKind::Identifier(_) | ExprKind::Property { .. }) {
        return false;
    }
    let not_getter = |f: FunctionElementId| s.program.function(f).kind != FunctionKind::Getter;
    match s.body.get_resolved(recv) {
        Some(Resolved::Member { member: MemberRef::Function(f), .. }) => not_getter(*f),
        Some(Resolved::Member { member: MemberRef::Variable(_), .. }) => false,
        Some(Resolved::Element(Element::Function(f))) => not_getter(*f),
        Some(Resolved::Element(_)) => false,
        Some(Resolved::ExtensionMember { member, .. }) => not_getter(*member),
        Some(Resolved::Local(_)) => s.body.declaracao_local(recv).is_some_and(|d| {
            a.stmts.iter().any(|x| matches!(&x.kind, StmtKind::Function(g) if a.function(*g).name.is_some_and(|n| n.span.start == d)))
        }),
        Some(Resolved::Parameter { .. }) => false,
        _ => true,
    }
}

/// The syntactic `TypeSystemImpl.isNullable`/`isNonNullable`.
pub(super) fn is_nullable(table: &TypeTable, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => true,
        Type::Intersection { bound, .. } => is_nullable(table, *bound),
        Type::Interface { nullable, .. } | Type::TypeParameter { nullable, .. } | Type::ExtensionType { nullable, .. } => *nullable,
        Type::Function { nullable, .. } | Type::Record { nullable, .. } => *nullable,
        Type::FutureOr { arg, nullable } => *nullable || is_nullable(table, *arg),
        Type::Never => false,
    }
}

pub(super) fn is_non_nullable(table: &TypeTable, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => false,
        Type::Intersection { bound, .. } => is_non_nullable(table, *bound),
        Type::TypeParameter { param, nullable } => !*nullable && is_non_nullable(table, table.param(*param).bound),
        Type::FutureOr { arg, nullable } => !*nullable && is_non_nullable(table, *arg),
        Type::Interface { nullable, .. } | Type::ExtensionType { nullable, .. } => !*nullable,
        Type::Function { nullable, .. } | Type::Record { nullable, .. } => !*nullable,
        Type::Never => true,
    }
}

/// The `ClassId` of a declaration of the unit.
fn class_of_decl(s: &super::Semantics<'_>, d: DeclId) -> Option<ClassId> {
    (0..s.program.classes.len()).map(|i| ClassId(i as u32)).find(|c| s.program.class(*c).decl.is_some_and(|r| r.unit == s.unit && r.decl == d))
}

/// The path segments of a URI (`Uri.pathSegments`).
fn path_segments(uri: &str) -> Vec<&str> {
    let Some((_, rest)) = uri.split_once(':') else { return uri.split('/').filter(|x| !x.is_empty()).collect() };
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let path = match rest.strip_prefix("//") {
        Some(r) => r.find('/').map_or("", |i| &r[i..]),
        None => rest,
    };
    let path = path.strip_prefix('/').unwrap_or(path);
    if path.is_empty() {
        return Vec::new();
    }
    path.split('/').collect()
}

/// `approximateContextType`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ApproxContext {
    NoContext,
    Invalid,
    Typed(TypeId),
}

/// The parents of the expressions and the functions (through the walk).
pub(super) struct NodeParents {
    pub(super) expr: HashMap<ExprId, Node>,
    pub(super) function: HashMap<dartforge_frontend::ast::FunctionId, Node>,
    pub(super) ancestors: HashMap<ExprId, Vec<Node>>,
}

pub(super) fn unit_parents(u: Unidade<'_>, keep: &dyn Fn(ExprId) -> bool) -> NodeParents {
    let mut p = NodeParents { expr: HashMap::new(), function: HashMap::new(), ancestors: HashMap::new() };
    walk(u, &mut |node, ancestors| {
        match node {
            Node::Expr(e) => {
                if let Some(x) = ancestors.last() {
                    p.expr.insert(e, *x);
                }
                if keep(e) {
                    p.ancestors.insert(e, ancestors.to_vec());
                }
            }
            Node::Function(f) => {
                if let Some(x) = ancestors.last() {
                    p.function.insert(f, *x);
                }
            }
            _ => {}
        }
    });
    p
}

/// The written return type of a function (`returnType?.type`).
fn written_return(s: &super::Semantics<'_>, a: &Ast, f: dartforge_frontend::ast::FunctionId) -> Option<TypeId> {
    let t = a.function(f).return_type?;
    s.body.tipos_de_anotacoes.get(&t).copied().or_else(|| s.outline.tipos_escritos.get(&(s.unit, t)).copied())
}

/// The return type of the element of a declared function (method, top-level
/// or local).
fn element_return(s: &super::Semantics<'_>, a: &Ast, f: dartforge_frontend::ast::FunctionId) -> Option<TypeId> {
    for (k, fe) in s.program.functions.iter().enumerate() {
        if let FunctionRef::Function { unit, function } = fe.node
            && unit == s.unit
            && function == f
        {
            return s.outline.functions.get(k).map(|d| d.return_type);
        }
    }
    let n = a.function(f).name?;
    match s.table.get(s.body.tipo_local(n.span.start)?) {
        Type::Function { ret, .. } => Some(*ret),
        _ => None,
    }
}

/// `_expectedReturnableOrYieldableType`.
fn returnable(s: &super::Semantics<'_>, interner: &Interner, a: &Ast, f: dartforge_frontend::ast::FunctionId, ret: Option<TypeId>) -> Option<TypeId> {
    let ret = ret?;
    if !matches!(s.table.get(ret), Type::Interface { .. }) {
        return None;
    }
    let first = || match s.table.get(ret) {
        Type::Interface { args, .. } => args.first().copied(),
        _ => None,
    };
    use dartforge_frontend::ast::AsyncModifier as M;
    match a.function(f).modifier {
        M::Async if is_class_named(s, interner, ret, "Future", "dart.async") => return first(),
        M::AsyncStar if is_class_named(s, interner, ret, "Stream", "dart.async") => return first(),
        M::SyncStar if is_class_named(s, interner, ret, "Iterable", "dart.core") => return first(),
        _ => {}
    }
    Some(ret)
}

/// `FunctionBody.expectedReturnType` of the body of function `f`.
fn expected_return(s: &super::Semantics<'_>, interner: &Interner, a: &Ast, parents: &NodeParents, f: dartforge_frontend::ast::FunctionId) -> Option<TypeId> {
    let is_method = a.members.iter().any(|m| matches!(m.kind, MemberKind::Method(g) if g == f));
    let literal_expr = a.exprs.iter().position(|e| matches!(e.kind, ExprKind::FunctionExpression(g) if g == f)).map(|k| ExprId(k as u32));
    if is_method {
        return returnable(s, interner, a, f, element_return(s, a, f));
    }
    match literal_expr {
        // A loose `FunctionExpression`: its approximate context.
        Some(e) => match approximate_context(s, interner, a, parents, e) {
            ApproxContext::Typed(t) => match s.table.get(t) {
                Type::Function { ret, .. } => returnable(s, interner, a, f, Some(*ret)),
                _ => None,
            },
            _ => None,
        },
        // That of a `FunctionDeclaration` (top-level or local).
        None => returnable(s, interner, a, f, element_return(s, a, f)),
    }
}

/// `approximateContextType` (`extensions.dart:377`).
pub(super) fn approximate_context(s: &super::Semantics<'_>, interner: &Interner, a: &Ast, parents: &NodeParents, me: ExprId) -> ApproxContext {
    let mut child = me;
    let mut parent = parents.expr.get(&me).copied();
    while let Some(Node::Expr(p)) = parent {
        match &a.expr(p).kind {
            ExprKind::Parenthesized(_) => {}
            ExprKind::Cascade { target, .. } if *target == child => {}
            _ => break,
        }
        child = p;
        parent = parents.expr.get(&p).copied();
    }
    let type_ctx = |e: ExprId| s.body.get_type(e).map_or(ApproxContext::NoContext, ApproxContext::Typed);
    match parent {
        Some(Node::Expr(p)) => match &a.expr(p).kind {
            // Argument (positional or named).
            ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => {
                let Some(arg) = arguments.args.iter().find(|x| x.value == child) else { return ApproxContext::NoContext };
                // The `staticParameterElement` of the node itself (positional)
                // or of the `NamedExpression`.
                let key = if arg.name.is_some() { child } else { me };
                if arg.name.is_none() && child != me {
                    return ApproxContext::Invalid;
                }
                s.body.tipos_de_parametros.get(&key).copied().map_or(ApproxContext::Invalid, ApproxContext::Typed)
            }
            ExprKind::Assign { .. } | ExprKind::Conditional { .. } => type_ctx(p),
            _ => ApproxContext::NoContext,
        },
        // Expression body.
        Some(Node::Function(f)) if matches!(a.function(f).body, FunctionBody::Expression(x) if x == child) => {
            let literal_expr = a.exprs.iter().position(|e| matches!(e.kind, ExprKind::FunctionExpression(g) if g == f)).map(|k| ExprId(k as u32));
            match literal_expr {
                Some(fe) => match approximate_context(s, interner, a, parents, fe) {
                    ApproxContext::Typed(t) => match s.table.get(t) {
                        Type::Function { ret, .. } => ApproxContext::Typed(*ret),
                        _ => ApproxContext::NoContext,
                    },
                    _ => ApproxContext::NoContext,
                },
                None => written_return(s, a, f).map_or(ApproxContext::NoContext, ApproxContext::Typed),
            }
        }
        // Constructor field initializer: the field's type.
        Some(Node::Member(m)) => {
            let MemberKind::Constructor(k) = &a.member(m).kind else {
                return variable_list_context(s, a, Some(m), None, child);
            };
            let Some(name) = k.initializers.iter().find_map(|i| match i {
                Initializer::Field { name, value, .. } if *value == child => Some(*name),
                _ => None,
            }) else {
                return ApproxContext::NoContext;
            };
            // The field of the same class: the one with the name among the
            // members of the declaration that contains the constructor.
            let class_id = a.decls.iter().find_map(|d| {
                let members = match &d.kind {
                    DeclKind::Class(x) => &x.members,
                    DeclKind::Enum(x) => &x.members,
                    DeclKind::Mixin(x) => &x.members,
                    DeclKind::ExtensionType(x) => &x.members,
                    _ => return None,
                };
                members.contains(&m).then_some(members)
            });
            let Some(members) = class_id else { return ApproxContext::NoContext };
            for &mm in members {
                if let MemberKind::Field(l) = &a.member(mm).kind
                    && let Some(index) = l.variables.iter().position(|x| x.name.sym == name.sym)
                {
                    return declared_variable_type(s, VariableRef::Field { unit: s.unit, member: mm, index }).map_or(ApproxContext::NoContext, ApproxContext::Typed);
                }
            }
            ApproxContext::NoContext
        }
        Some(Node::Stmt(st)) => match &a.stmt(st).kind {
            StmtKind::Return(Some(_)) | StmtKind::Yield { .. } => {
                let Some(anc) = parents.ancestors.get(&me) else { return ApproxContext::NoContext };
                // The nearest function body.
                let f = anc.iter().rev().find_map(|n| match n {
                    Node::Function(f) => Some(*f),
                    _ => None,
                });
                match f.and_then(|f| expected_return(s, interner, a, parents, f)) {
                    Some(t) => ApproxContext::Typed(t),
                    None => ApproxContext::NoContext,
                }
            }
            StmtKind::Variables(l) => {
                if l.ty.is_none() {
                    return ApproxContext::NoContext;
                }
                let t = l.ty.and_then(|t| s.body.tipos_de_anotacoes.get(&t).copied());
                t.map_or(ApproxContext::NoContext, ApproxContext::Typed)
            }
            StmtKind::For { init: Some(ForInit::Variables(l)), .. } => {
                l.ty.and_then(|t| s.body.tipos_de_anotacoes.get(&t).copied()).map_or(ApproxContext::NoContext, ApproxContext::Typed)
            }
            _ => ApproxContext::NoContext,
        },
        Some(Node::Decl(d)) => variable_list_context(s, a, None, Some(d), child),
        _ => ApproxContext::NoContext,
    }
}

/// The written type of the variable list (field or top-level) that `child`
/// initializes.
fn variable_list_context(s: &super::Semantics<'_>, a: &Ast, m: Option<dartforge_frontend::ast::MemberId>, d: Option<DeclId>, child: ExprId) -> ApproxContext {
    let list = match (m, d) {
        (Some(m), _) => match &a.member(m).kind {
            MemberKind::Field(l) => l,
            _ => return ApproxContext::NoContext,
        },
        (_, Some(d)) => match &a.decl(d).kind {
            DeclKind::Variables(l) => l,
            _ => return ApproxContext::NoContext,
        },
        _ => return ApproxContext::NoContext,
    };
    if !list.variables.iter().any(|v| v.initializer == Some(child)) {
        return ApproxContext::NoContext;
    }
    let Some(t) = list.ty else { return ApproxContext::NoContext };
    s.outline.tipos_escritos.get(&(s.unit, t)).copied().map_or(ApproxContext::NoContext, ApproxContext::Typed)
}

/// The element type of a top-level variable or field.
pub(super) fn declared_variable_type(s: &super::Semantics<'_>, r: VariableRef) -> Option<TypeId> {
    let v = s.program.variables.iter().position(|x| x.node == r)?;
    let d = s.outline.variables.get(v)?;
    d.declared_type.or(d.inferred)
}

/// The `VariableId` of the field written by `e` (`writeElement.canonicalElement`).
fn written_field(s: &super::Semantics<'_>, body: &dartforge_types::resolved::UnitBodyTypes, e: ExprId) -> Option<VariableId> {
    match body.get_resolved(e)? {
        Resolved::Member { member: MemberRef::Variable(v), .. } => Some(*v),
        Resolved::Member { member: MemberRef::Function(f), .. } => s.program.function(*f).variable,
        Resolved::Element(Element::Variable(v)) => Some(*v),
        Resolved::Element(Element::Function(f)) => s.program.function(*f).variable,
        _ => None,
    }
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unidade<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let Some(s) = sem else { return out };
    let a = u.ast;
    let source = u.fonte;
    let program = s.program;
    let table = s.table;
    let mut report = |code: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    let rules = [
        "avoid_function_literals_in_foreach_calls",
        "avoid_renaming_method_parameters",
        "implementation_imports",
        "null_closures",
        "prefer_collection_literals",
        "prefer_interpolation_to_compose_strings",
        "unnecessary_nullable_for_final_variable_declarations",
        "unnecessary_this",
        "prefer_final_fields",
        "overridden_fields",
    ];
    if !rules.iter().any(|r| enabled(r)) {
        return out;
    }
    let parents = unit_parents(u, &|e| {
        matches!(a.expr(e).kind, ExprKind::Call { .. } | ExprKind::InstanceCreation { .. } | ExprKind::This)
    });
    let lib = program.unit(s.unit).library;
    let doc = dartforge_frontend::comentarios::Comentarios::de(source);
    let has_doc = |metadata: &[dartforge_frontend::ast::Annotation], start: usize| {
        let after = match metadata.last() {
            Some(m) => dartforge_frontend::fonte::pular_brancos(source.as_bytes(), m.span.end),
            None => start,
        };
        doc.dart_doc(source, after).is_some() || metadata.iter().rev().any(|m| doc.dart_doc(source, m.span.start).is_some())
    };

    // `avoid_function_literals_in_foreach_calls`.
    if enabled("avoid_function_literals_in_foreach_calls") {
        // `containsNullAwareInvocationInChain`.
        fn null_aware_in_chain(a: &Ast, e: ExprId) -> bool {
            match &a.expr(e).kind {
                ExprKind::Property { target, null_aware, .. } => {
                    if *null_aware {
                        return true;
                    }
                    // `a.b` with `a` an identifier is a `PrefixedIdentifier`.
                    if matches!(a.expr(*target).kind, ExprKind::Identifier(_)) {
                        return false;
                    }
                    null_aware_in_chain(a, *target)
                }
                ExprKind::Call { target, .. } => match &a.expr(*target).kind {
                    ExprKind::Property { target: t, null_aware, .. } => *null_aware || null_aware_in_chain(a, *t),
                    _ => false,
                },
                ExprKind::Index { target, null_aware, .. } => *null_aware || null_aware_in_chain(a, *target),
                _ => false,
            }
        }
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            let ExprKind::Call { target, arguments } = &e.kind else { continue };
            let ExprKind::Property { target: recv, name, null_aware } = &a.expr(*target).kind else { continue };
            if interner.resolve(name.sym) != "forEach" || matches!(a.expr(*recv).kind, ExprKind::CascadeTarget) || !is_method_invocation(s, a, *target) {
                continue;
            }
            let Some(first) = arguments.args.first() else { continue };
            if first.name.is_some() || !matches!(a.expr(first.value).kind, ExprKind::FunctionExpression(_)) {
                continue;
            }
            if !s.body.get_type(*recv).is_some_and(|t| implements_class(s, interner, t, "Iterable", "dart.core")) {
                continue;
            }
            if *null_aware || null_aware_in_chain(a, *recv) {
                continue;
            }
            // `_hasMethodChaining`.
            let mut x = *recv;
            let chained = loop {
                match &a.expr(x).kind {
                    ExprKind::Property { target, .. } => x = *target,
                    ExprKind::Call { target, .. } if is_method_invocation(s, a, *target) => break true,
                    _ => break false,
                }
            };
            if chained {
                continue;
            }
            // `_isInsideCascade`.
            let in_cascade = parents.ancestors.get(&id).is_some_and(|anc| {
                for n in anc.iter().rev() {
                    match n {
                        Node::Stmt(_) => return false,
                        Node::Expr(p) if matches!(a.expr(*p).kind, ExprKind::Cascade { .. }) => return true,
                        _ => {}
                    }
                }
                false
            });
            if !in_cascade {
                report(&c::AVOID_FUNCTION_LITERALS_IN_FOREACH_CALLS, name.span, &[]);
            }
        }
    }

    // `avoid_renaming_method_parameters`.
    if enabled("avoid_renaming_method_parameters") {
        for (k, d) in a.decls.iter().enumerate() {
            let (name, members) = match &d.kind {
                DeclKind::Class(x) => (x.name, &x.members),
                DeclKind::Mixin(x) => (x.name, &x.members),
                DeclKind::Enum(x) => (x.name, &x.members),
                DeclKind::ExtensionType(x) => (x.name, &x.members),
                _ => continue,
            };
            if interner.resolve(name.sym).starts_with('_') {
                continue;
            }
            let Some(class_id) = class_of_decl(s, DeclId(k as u32)) else { continue };
            for &mid in members {
                let m = a.member(mid);
                let MemberKind::Method(fid) = &m.kind else { continue };
                let f = a.function(*fid);
                if f.static_ || has_doc(&m.metadata, m.span.start) {
                    continue;
                }
                let (Some(n), Some(ps)) = (f.name, &f.parameters) else { continue };
                let text = interner.resolve(n.sym);
                // `lookUpInheritedMethod`: the chain, without the class itself.
                let mut parent: Option<FunctionElementId> = None;
                let mut seen: HashSet<ClassId> = HashSet::new();
                let mut current = Some(class_id);
                'search: while let Some(cl) = current {
                    if !seen.insert(cl) {
                        break;
                    }
                    let e = program.class(cl);
                    let candidates = std::iter::once(cl).chain(e.mixin_classes.iter().rev().copied());
                    for cc in candidates {
                        if cc == class_id {
                            continue;
                        }
                        let k2 = program.class(cc);
                        if let Some(&g) = k2.instance_members.get(&n.sym) {
                            let ge = program.function(g);
                            let method = matches!(ge.kind, FunctionKind::Function | FunctionKind::Operator);
                            if method && !ge.static_ && (!text.starts_with('_') || k2.library == lib) {
                                parent = Some(g);
                                break 'search;
                            }
                        }
                    }
                    current = e.supertype_class;
                }
                let Some(parent) = parent else { continue };
                let FunctionRef::Function { unit, function } = program.function(parent).node else { continue };
                let a2 = &program.unit(unit).ast;
                let Some(parent_params) = &a2.function(function).parameters else { continue };
                let positional: Vec<_> = ps.iter().filter(|p| p.kind != ParameterKind::Named).collect();
                let from_parent: Vec<_> = parent_params.iter().filter(|p| p.kind != ParameterKind::Named).collect();
                for (p, q) in positional.iter().zip(from_parent.iter()) {
                    let (Some(pn), Some(qn)) = (p.name, q.name) else { continue };
                    let (pt, qt) = (interner.resolve(pn.sym), interner.resolve(qn.sym));
                    if pt != qt {
                        report(&c::AVOID_RENAMING_METHOD_PARAMETERS, pn.span, &[pt, qt]);
                    }
                }
            }
        }
    }

    // `implementation_imports`.
    if enabled("implementation_imports") {
        let source_uri = program.unit(s.unit).uri.clone();
        if source_uri.starts_with("package:") {
            for imp in program.library(lib).imports.iter().filter(|i| i.unit == s.unit) {
                let Some(d) = u.unit.directives.get(imp.directive) else { continue };
                let DirectiveKind::Import { uri, .. } = &d.kind else { continue };
                let imported = &program.library(imp.library).uri;
                let si = path_segments(imported);
                let implementation = si.len() > 2 && si[1] == "src";
                let sf = path_segments(&source_uri);
                let same = !si.is_empty() && !sf.is_empty() && si[0] == sf[0];
                if implementation && !same {
                    report(&c::IMPLEMENTATION_IMPORTS, uri.span, &[]);
                }
            }
        }
    }

    // `null_closures`.
    if enabled("null_closures") {
        // (library, class, name, positional, named)
        type Def = (&'static str, Option<&'static str>, Option<&'static str>, &'static [usize], &'static [&'static str]);
        const CONSTRUCTORS: [Def; 6] = [
            ("dart.async", Some("Future"), None, &[0], &[]),
            ("dart.async", Some("Future"), Some("microtask"), &[0], &[]),
            ("dart.async", Some("Future"), Some("sync"), &[0], &[]),
            ("dart.async", Some("Timer"), None, &[1], &[]),
            ("dart.async", Some("Timer"), Some("periodic"), &[1], &[]),
            ("dart.core", Some("List"), Some("generate"), &[1], &[]),
        ];
        const STATICS: [Def; 5] = [
            ("dart.async", None, Some("scheduleMicrotask"), &[0], &[]),
            ("dart.async", Some("Future"), Some("doWhile"), &[0], &[]),
            ("dart.async", Some("Future"), Some("forEach"), &[1], &[]),
            ("dart.async", Some("Future"), Some("wait"), &[], &["cleanUp"]),
            ("dart.async", Some("Timer"), Some("run"), &[0], &[]),
        ];
        const INSTANCE_METHODS: [Def; 26] = [
            ("dart.core", Some("Iterable"), Some("any"), &[0], &[]),
            ("dart.async", Some("Future"), Some("complete"), &[0], &[]),
            ("dart.core", Some("Iterable"), Some("every"), &[0], &[]),
            ("dart.core", Some("Iterable"), Some("expand"), &[0], &[]),
            ("dart.core", Some("Iterable"), Some("firstWhere"), &[0], &["orElse"]),
            ("dart.core", Some("Iterable"), Some("forEach"), &[0], &[]),
            ("dart.core", Some("Map"), Some("forEach"), &[0], &[]),
            ("dart.core", Some("Iterable"), Some("fold"), &[1], &[]),
            ("dart.core", Some("Iterable"), Some("lastWhere"), &[0], &["orElse"]),
            ("dart.core", Some("Iterable"), Some("map"), &[0], &[]),
            ("dart.core", Some("Map"), Some("putIfAbsent"), &[1], &[]),
            ("dart.core", Some("Iterable"), Some("reduce"), &[0], &[]),
            ("dart.collection", Some("Queue"), Some("removeWhere"), &[0], &[]),
            ("dart.core", Some("List"), Some("removeWhere"), &[0], &[]),
            ("dart.core", Some("Set"), Some("removeWhere"), &[0], &[]),
            ("dart.core", Some("String"), Some("replaceAllMapped"), &[1], &[]),
            ("dart.core", Some("String"), Some("replaceFirstMapped"), &[1], &[]),
            ("dart.collection", Some("Queue"), Some("retainWhere"), &[0], &[]),
            ("dart.core", Some("List"), Some("retainWhere"), &[0], &[]),
            ("dart.core", Some("Set"), Some("retainWhere"), &[0], &[]),
            ("dart.core", Some("Iterable"), Some("singleWhere"), &[0], &["orElse"]),
            ("dart.core", Some("Iterable"), Some("skipWhile"), &[0], &[]),
            ("dart.core", Some("String"), Some("splitMapJoin"), &[], &["onMatch", "onNonMatch"]),
            ("dart.core", Some("Iterable"), Some("takeWhile"), &[0], &[]),
            ("dart.async", Some("Future"), Some("then"), &[0], &["onError"]),
            ("dart.core", Some("Iterable"), Some("where"), &[0], &[]),
        ];
        let check = |arguments: &dartforge_frontend::ast::Arguments, positions: &[usize], names: &[&str], report: &mut dyn FnMut(Span)| {
            for (i, arg) in arguments.args.iter().enumerate() {
                let is_null = matches!(a.expr(arg.value).kind, ExprKind::Null);
                match arg.name {
                    Some(n) => {
                        if is_null && names.contains(&interner.resolve(n.sym)) {
                            report(Span { start: n.span.start, end: a.expr(arg.value).span.end });
                        }
                    }
                    None => {
                        if is_null && positions.contains(&i) {
                            report(a.expr(arg.value).span);
                        }
                    }
                }
            }
        };
        let mut findings: Vec<Span> = Vec::new();
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            // Instance creation.
            if let Some(Resolved::Constructor(f)) = s.body.get_resolved(id)
                && let ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } = &e.kind
            {
                let constructor_name = interner.resolve(program.function(*f).name);
                let constructor_name = (!constructor_name.is_empty()).then_some(constructor_name);
                let Some(t) = s.body.get_type(id) else { continue };
                for (library, class_name_opt, name, positions, names) in CONSTRUCTORS {
                    if name != constructor_name {
                        continue;
                    }
                    // `extendsClass`: the superclass chain.
                    let extends = match table.get(t) {
                        Type::Interface { class, .. } => {
                            let mut seen = HashSet::new();
                            let mut current = Some(*class);
                            let mut found_class = false;
                            while let Some(cl) = current {
                                if !seen.insert(cl) {
                                    break;
                                }
                                let x = program.class(cl);
                                if Some(interner.resolve(x.name)) == class_name_opt && library_name(s, interner, x.library).as_deref() == Some(library) {
                                    found_class = true;
                                    break;
                                }
                                current = x.supertype_class;
                            }
                            found_class
                        }
                        _ => false,
                    };
                    if extends {
                        check(arguments, positions, names, &mut |sp| findings.push(sp));
                    }
                }
                continue;
            }
            let ExprKind::Call { target, arguments } = &e.kind else { continue };
            let ExprKind::Property { target: recv, name, .. } = &a.expr(*target).kind else { continue };
            if !is_method_invocation(s, a, *target) {
                continue;
            }
            let method = interner.resolve(name.sym);
            // A target that is a class: the static ones.
            let target_class = match (&a.expr(*recv).kind, s.body.get_resolved(*recv)) {
                (ExprKind::Identifier(_) | ExprKind::Property { .. }, Some(Resolved::Element(Element::Class(cl)))) if program.class(*cl).kind == ClassKind::Class => Some(*cl),
                _ => None,
            };
            if let Some(cl) = target_class {
                let class_name = interner.resolve(program.class(cl).name);
                for (_, class_name_opt, name, positions, names) in STATICS {
                    if name == Some(method) && class_name_opt == Some(class_name) {
                        check(arguments, positions, names, &mut |sp| findings.push(sp));
                    }
                }
                continue;
            }
            let Some(t) = s.body.get_type(*recv) else { continue };
            let (Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. }) = table.get(t) else { continue };
            if program.class(*class).decl.is_none() && program.class(*class).kind == ClassKind::MixinApplication {
                continue;
            }
            let mut classes = vec![*class];
            if let Some(d) = s.outline.hierarchy.get(*class) {
                let mut sup: Vec<ClassId> = d.supertypes.keys().copied().collect();
                sup.sort();
                classes.extend(sup);
            }
            let def = classes.iter().find_map(|cl| {
                let x = program.class(*cl);
                let b = library_name(s, interner, x.library)?;
                INSTANCE_METHODS.iter().find(|(lib_name, class_name_opt, name, _, _)| *lib_name == b && *class_name_opt == Some(interner.resolve(x.name)) && *name == Some(method))
            });
            if let Some((_, _, _, positions, names)) = def {
                check(arguments, positions, names, &mut |sp| findings.push(sp));
            }
        }
        for sp in findings {
            report(&c::NULL_CLOSURES, sp, &[]);
        }
    }

    // `prefer_collection_literals`.
    if enabled("prefer_collection_literals") {
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            match &e.kind {
                ExprKind::InstanceCreation { .. } | ExprKind::Call { .. } => {
                    let arguments = match &e.kind {
                        ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => arguments,
                        _ => continue,
                    };
                    if let Some(Resolved::Constructor(f)) = s.body.get_resolved(id) {
                        // The written type that is a `typedef`.
                        let written_alias = match &e.kind {
                            ExprKind::InstanceCreation { ty, .. } => match &a.ty(*ty).kind {
                                dartforge_frontend::ast::TypeKind::Named { name, .. } => {
                                    let element = match &name[..] {
                                        [n] => program.lookup_na_unidade(s.unit, n.sym),
                                        [p, n] => program.lookup_prefixed_na_unidade(s.unit, p.sym, n.sym),
                                        _ => None,
                                    };
                                    matches!(element.and_then(|b| b.getter), Some(Element::Typedef(_)))
                                }
                                _ => false,
                            },
                            ExprKind::Call { target, .. } => {
                                let root = match &a.expr(*target).kind {
                                    ExprKind::Identifier(n) => Some(*n),
                                    ExprKind::Property { target: t2, .. } => match &a.expr(*t2).kind {
                                        ExprKind::Identifier(n) => Some(*n),
                                        _ => None,
                                    },
                                    _ => None,
                                };
                                root.is_some_and(|n| matches!(program.lookup_na_unidade(s.unit, n.sym).and_then(|b| b.getter), Some(Element::Typedef(_))))
                            }
                            _ => false,
                        };
                        if written_alias {
                            continue;
                        }
                        let Some(t) = s.body.get_type(id) else { continue };
                        let constructor_name = interner.resolve(program.function(*f).name);
                        let unnamed = constructor_name.is_empty();
                        let is_hash_map = is_class_named(s, interner, t, "LinkedHashMap", "dart.collection");
                        let is_hash_set = is_class_named(s, interner, t, "LinkedHashSet", "dart.collection");
                        let is_map = is_class_named(s, interner, t, "Map", "dart.core");
                        let is_set = is_class_named(s, interner, t, "Set", "dart.core");
                        if is_hash_map || is_hash_set {
                            let ctx = approximate_context(s, interner, a, &parents, id);
                            if ctx == ApproxContext::Invalid {
                                continue;
                            }
                            if let ApproxContext::Typed(c2) = ctx
                                && is_class_named(s, interner, c2, if is_hash_map { "LinkedHashMap" } else { "LinkedHashSet" }, "dart.collection")
                            {
                                continue;
                            }
                        }
                        if is_map || is_hash_map {
                            if unnamed && arguments.args.is_empty() {
                                report(&c::PREFER_COLLECTION_LITERALS, e.span, &[]);
                            }
                            continue;
                        }
                        if is_set || is_hash_set {
                            if unnamed {
                                if arguments.args.is_empty() {
                                    report(&c::PREFER_COLLECTION_LITERALS, e.span, &[]);
                                }
                            } else if (constructor_name == "from" || constructor_name == "of")
                                && arguments.args.len() == 1
                                && arguments.args[0].name.is_none()
                                && matches!(a.expr(arguments.args[0].value).kind, ExprKind::List { .. })
                            {
                                report(&c::PREFER_COLLECTION_LITERALS, e.span, &[]);
                            }
                        }
                        continue;
                    }
                    // `[…].toSet()`.
                    if let ExprKind::Call { target, .. } = &e.kind
                        && let ExprKind::Property { target: recv, name, .. } = &a.expr(*target).kind
                        && interner.resolve(name.sym) == "toSet"
                        && matches!(a.expr(*recv).kind, ExprKind::List { .. })
                        && is_method_invocation(s, a, *target)
                    {
                        report(&c::PREFER_COLLECTION_LITERALS, e.span, &[]);
                    }
                }
                _ => {}
            }
        }
    }

    // `prefer_interpolation_to_compose_strings`.
    if enabled("prefer_interpolation_to_compose_strings") {
        fn adicoes(a: &Ast, e: ExprId) -> Vec<ExprId> {
            match &a.expr(e).kind {
                ExprKind::Binary { op: BinaryOp::Add, left, right } => {
                    let mut v = adicoes(a, *left);
                    v.push(*right);
                    v
                }
                ExprKind::Binary { .. } => Vec::new(),
                _ => vec![e],
            }
        }
        let is_literal = |e: ExprId| matches!(a.expr(e).kind, ExprKind::String(_));
        let simple_raw = |e: ExprId| match &a.expr(e).kind {
            ExprKind::String(l) => {
                let lits = super::strings::literals(source, l.span);
                lits.len() == 1 && !lits[0].is_interpolated() && lits[0].raw
            }
            _ => false,
        };
        let to_string_with_args = |e: ExprId| match &a.expr(e).kind {
            ExprKind::Call { target, arguments } => {
                let name = match &a.expr(*target).kind {
                    ExprKind::Property { name, .. } | ExprKind::Identifier(name) => Some(*name),
                    _ => None,
                };
                name.is_some_and(|n| interner.resolve(n.sym) == "toString") && !arguments.args.is_empty() && is_method_invocation(s, a, *target)
            }
            _ => false,
        };
        let is_string = |e: ExprId| s.body.get_type(e).is_some_and(|t| is_class_named(s, interner, t, "String", "dart.core"));
        for (k, e) in a.exprs.iter().enumerate() {
            if !matches!(e.kind, ExprKind::Binary { op: BinaryOp::Add, .. }) {
                continue;
            }
            let chain = adicoes(a, ExprId(k as u32));
            let mut i = 0;
            while i + 1 < chain.len() {
                let (l, r) = (chain[i], chain[i + 1]);
                let skip = (!is_literal(l) && !is_literal(r))
                    || simple_raw(l)
                    || simple_raw(r)
                    || (is_literal(l) && is_literal(r))
                    || to_string_with_args(l)
                    || to_string_with_args(r);
                if !skip && is_string(l) {
                    report(&c::PREFER_INTERPOLATION_TO_COMPOSE_STRINGS, Span { start: a.expr(l).span.start, end: a.expr(r).span.end }, &[]);
                    i += 1;
                }
                i += 1;
            }
        }
    }

    // `unnecessary_nullable_for_final_variable_declarations`.
    if enabled("unnecessary_nullable_for_final_variable_declarations") {
        let mut check_decl = |final_or_const: bool, name: dartforge_frontend::ast::Name, init: Option<ExprId>, declared: Option<TypeId>| {
            if !final_or_const {
                return;
            }
            let (Some(i), Some(d)) = (init.and_then(|i| s.body.get_type(i)), declared) else { return };
            if matches!(table.get(d), Type::Dynamic) {
                return;
            }
            if is_nullable(table, d) && is_non_nullable(table, i) {
                report(&c::UNNECESSARY_NULLABLE_FOR_FINAL_VARIABLE_DECLARATIONS, name.span, &[]);
            }
        };
        for (k, d) in a.decls.iter().enumerate() {
            match &d.kind {
                DeclKind::Variables(l) => {
                    for (index, v) in l.variables.iter().enumerate() {
                        let t = declared_variable_type(s, VariableRef::TopLevel { unit: s.unit, decl: DeclId(k as u32), index });
                        check_decl(l.final_ || l.const_, v.name, v.initializer, t);
                    }
                }
                DeclKind::Class(_) | DeclKind::Mixin(_) | DeclKind::Enum(_) | DeclKind::Extension(_) | DeclKind::ExtensionType(_) => {}
                _ => {}
            }
        }
        for (k, m) in a.members.iter().enumerate() {
            let MemberKind::Field(l) = &m.kind else { continue };
            for (index, v) in l.variables.iter().enumerate() {
                if interner.resolve(v.name.sym).starts_with('_') || l.static_ {
                    let t = declared_variable_type(s, VariableRef::Field { unit: s.unit, member: dartforge_frontend::ast::MemberId(k as u32), index });
                    check_decl(l.final_ || l.const_, v.name, v.initializer, t);
                }
            }
        }
        for st in a.stmts.iter() {
            if let StmtKind::Variables(l) = &st.kind {
                for v in l.variables.iter() {
                    check_decl(l.final_ || l.const_, v.name, v.initializer, s.body.tipo_local(v.name.span.start));
                }
            }
        }
        // The `final` record and list declaration patterns.
        let mut patterns: Vec<dartforge_frontend::ast::PatternId> = Vec::new();
        for st in a.stmts.iter() {
            match &st.kind {
                StmtKind::PatternVariables { final_: true, pattern, .. } => patterns.push(*pattern),
                StmtKind::For { init: Some(ForInit::Pattern { final_: true, pattern, .. }), .. } => patterns.push(*pattern),
                _ => {}
            }
        }
        for p in patterns {
            let children: Vec<dartforge_frontend::ast::PatternId> = match &a.pattern(p).kind {
                PatternKind::Record { fields } => fields.iter().map(|f| f.pattern).collect(),
                PatternKind::List { elements, .. } => elements
                    .iter()
                    .filter_map(|el| match el {
                        dartforge_frontend::ast::ListPatternElement::Pattern(x) => Some(*x),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            for f in children {
                let PatternKind::Variable { name, .. } = &a.pattern(f).kind else { continue };
                let (Some(t), Some(&value)) = (s.body.tipo_local(name.span.start), s.body.tipos_casados.get(&f)) else { continue };
                if matches!(table.get(t), Type::Dynamic) {
                    continue;
                }
                if is_nullable(table, t) && is_non_nullable(table, value) {
                    report(&c::UNNECESSARY_NULLABLE_FOR_FINAL_VARIABLE_DECLARATIONS, name.span, &[]);
                }
            }
        }
    }

    // `unnecessary_this`.
    if enabled("unnecessary_this") {
        let mut findings: Vec<Span> = Vec::new();
        // The `this.` of a field initializer.
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                for i in k.initializers.iter() {
                    if let Initializer::Field { span, this_: true, .. } = i {
                        findings.push(Span { start: span.start, end: span.start + 4 });
                    }
                }
            }
        }
        let scopes = Scopes::new(a);
        // The writes (the `writeElement`).
        let mut written: HashSet<ExprId> = HashSet::new();
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::Assign { target, .. } => {
                    written.insert(*target);
                }
                ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } => {
                    written.insert(*operand);
                }
                _ => {}
            }
        }
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            if !matches!(e.kind, ExprKind::This) {
                continue;
            }
            let Some(Node::Expr(p)) = parents.expr.get(&id).copied() else { continue };
            let ExprKind::Property { target, name, null_aware: false } = &a.expr(p).kind else { continue };
            if *target != id {
                continue;
            }
            // `PropertyAccess` (the `propertyName`) or `MethodInvocation` (the
            // `methodName`): the property's element.
            let setter = written.contains(&p);
            let element = s.body.get_resolved(p);
            let element_owner: Option<(u32, DeclId)> = match element {
                Some(Resolved::Member { class, .. }) => program.class(*class).decl.map(|r| (r.unit.0, r.decl)),
                Some(Resolved::ExtensionMember { extension, .. }) => {
                    let x = &program.extensions[extension.0 as usize];
                    Some((x.decl.unit.0, x.decl.decl))
                }
                _ => None,
            };
            if element.is_none() || matches!(element, Some(Resolved::Dynamic)) {
                continue;
            }
            let found_entries = scopes.lookup(name.sym, e.span.start);
            let mut requested = Vec::new();
            let mut other = Vec::new();
            for r in found_entries.iter() {
                let matches = match r.kind {
                    EntryKind::Member { setter: st, .. } => st == setter,
                    _ => !setter,
                };
                if matches {
                    requested.push(r);
                } else {
                    other.push(r);
                }
            }
            let reported = if found_entries.is_empty() {
                // The unit's scope.
                match program.lookup_na_unidade(s.unit, name.sym) {
                    // A top-level element (the requested one or the other
                    // accessor) is not the member, nor is it a class's.
                    None => true,
                    Some(_) => false,
                }
            } else if let Some(r) = requested.first() {
                match r.kind {
                    EntryKind::Member { owner, .. } => element_owner == Some((s.unit.0, owner)),
                    _ => false,
                }
            } else if let Some(r) = other.first() {
                match r.kind {
                    EntryKind::Member { owner, .. } => matches!(a.decl(owner).kind, DeclKind::Class(_)),
                    _ => false,
                }
            } else {
                true
            };
            if reported {
                findings.push(e.span);
            }
        }
        findings.sort_by_key(|s| s.start);
        for sp in findings {
            report(&c::UNNECESSARY_THIS, sp, &[]);
        }
    }

    // `prefer_final_fields`.
    if enabled("prefer_final_fields") {
        // The candidate fields, in source order: (field, member, index, class
        // of the declaration, is a `ClassDeclaration`).
        let mut fields: Vec<(VariableId, dartforge_frontend::ast::MemberId, usize, Option<DeclId>)> = Vec::new();
        for (k, d) in a.decls.iter().enumerate() {
            let (members, is_class, ext_type) = match &d.kind {
                DeclKind::Class(x) => (&x.members, true, false),
                DeclKind::Mixin(x) => (&x.members, false, false),
                DeclKind::Extension(x) => (&x.members, false, false),
                DeclKind::ExtensionType(x) => (&x.members, false, true),
                _ => continue,
            };
            let class_id = class_of_decl(s, DeclId(k as u32));
            for &mid in members {
                let MemberKind::Field(l) = &a.member(mid).kind else { continue };
                if (ext_type && !l.static_) || l.final_ || l.const_ {
                    continue;
                }
                for (index, v) in l.variables.iter().enumerate() {
                    let text = interner.resolve(v.name.sym);
                    if !text.starts_with('_') {
                        continue;
                    }
                    let Some(vid) = program.variables.iter().position(|x| x.node == VariableRef::Field { unit: s.unit, member: mid, index }) else { continue };
                    // `overridesField`: an inherited concrete setter.
                    let overrides = class_id.is_some_and(|cl| {
                        let key = interner.lookup(&format!("{text}_="));
                        let Some(key) = key else { return false };
                        let mut seen: HashSet<ClassId> = HashSet::new();
                        let e0 = program.class(cl);
                        let mut queue: Vec<ClassId> = e0.mixin_classes.iter().rev().copied().collect();
                        queue.extend(e0.supertype_class);
                        queue.extend(e0.on_classes.iter().copied());
                        let mut i = 0;
                        while i < queue.len() {
                            let x = queue[i];
                            i += 1;
                            if !seen.insert(x) {
                                continue;
                            }
                            let kx = program.class(x);
                            if kx.library == lib
                                && let Some(&g) = kx.instance_members.get(&key)
                                && !program.function(g).abstract_
                            {
                                return true;
                            }
                            queue.extend(kx.mixin_classes.iter().rev().copied());
                            queue.extend(kx.supertype_class);
                        }
                        false
                    });
                    if overrides {
                        continue;
                    }
                    fields.push((VariableId(vid as u32), mid, index, is_class.then_some(DeclId(k as u32))));
                }
            }
        }
        if !fields.is_empty() {
            // The writes of the whole library.
            let mut mutated: HashSet<VariableId> = HashSet::new();
            for &un in &program.library(lib).units {
                let Some(body) = s.bodies.units.get(un.0 as usize) else { continue };
                let a2 = &program.unit(un).ast;
                for e in a2.exprs.iter() {
                    let recv = match &e.kind {
                        ExprKind::Assign { target, .. } => *target,
                        ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } => *operand,
                        _ => continue,
                    };
                    if let Some(v) = written_field(s, body, recv) {
                        mutated.insert(v);
                    }
                }
            }
            for (vid, mid, index, class_id) in fields {
                if mutated.contains(&vid) {
                    continue;
                }
                let MemberKind::Field(l) = &a.member(mid).kind else { continue };
                let v = &l.variables[index];
                let constructors: Vec<&dartforge_frontend::ast::Constructor> = match class_id.map(|d| &a.decl(d).kind) {
                    Some(DeclKind::Class(x)) => x
                        .members
                        .iter()
                        .filter_map(|m| match &a.member(*m).kind {
                            MemberKind::Constructor(k) => Some(k),
                            _ => None,
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                let defined_in = |k: &dartforge_frontend::ast::Constructor| {
                    k.initializers.iter().any(|i| matches!(i, Initializer::Field { name, .. } if name.sym == v.name.sym))
                        || k.parameters.iter().any(|p| p.this_ && p.name.is_some_and(|n| n.sym == v.name.sym))
                };
                let in_any = constructors.iter().any(|k| defined_in(k));
                let reported = if in_any { constructors.iter().all(|k| defined_in(k)) } else { v.initializer.is_some() };
                if reported {
                    let end = v.initializer.map_or(v.name.span.end, |i| a.expr(i).span.end);
                    let text = interner.resolve(v.name.sym);
                    report(&c::PREFER_FINAL_FIELDS, Span { start: v.name.span.start, end }, &[text]);
                }
            }
        }
    }

    // `overridden_fields`.
    if enabled("overridden_fields") {
        for (k, d) in a.decls.iter().enumerate() {
            let members = match &d.kind {
                DeclKind::Class(x) => &x.members,
                DeclKind::Mixin(x) => &x.members,
                DeclKind::Enum(x) => &x.members,
                DeclKind::ExtensionType(x) => &x.members,
                _ => continue,
            };
            if d.augment {
                continue;
            }
            let Some(class_id) = class_of_decl(s, DeclId(k as u32)) else { continue };
            // `_findAllSupertypesAndMixins` / `_findAllSupertypesInMixin`.
            let order = supertypes_and_mixins(s, class_id);
            for &mid in members {
                let m = a.member(mid);
                let MemberKind::Field(l) = &m.kind else { continue };
                if l.static_ || m.augment {
                    continue;
                }
                for v in l.variables.iter() {
                    let text = interner.resolve(v.name.sym);
                    let found = order.iter().find_map(|cl| {
                        let x = program.class(*cl);
                        let g = *x.instance_members.get(&v.name.sym)?;
                        let ge = program.function(g);
                        // The synthetic getter of a field.
                        let field = ge.variable?;
                        if !matches!(ge.node, FunctionRef::None) || ge.static_ || ge.kind != FunctionKind::Getter {
                            return None;
                        }
                        if text.starts_with('_') && x.library != lib {
                            return None;
                        }
                        Some((*cl, field))
                    });
                    let Some((dona, field)) = found else { continue };
                    // `isAbstract` of the accessor: the `abstract` field.
                    let is_abstract = match program.variable(field).node {
                        VariableRef::Field { unit, member, .. } => matches!(&program.unit(unit).ast.member(member).kind, MemberKind::Field(l2) if l2.abstract_),
                        _ => false,
                    };
                    if !is_abstract {
                        let owner_name = interner.resolve(program.class(dona).name);
                        report(&c::OVERRIDDEN_FIELDS, v.name.span, &[owner_name]);
                    }
                }
            }
        }
    }

    out
}

/// The order of `_findAllSupertypesAndMixins` (class) or of
/// `_findAllSupertypesInMixin` (mixin), without repetition and without the
/// class itself.
fn supertypes_and_mixins(s: &super::Semantics<'_>, class_id: ClassId) -> Vec<ClassId> {
    let program = s.program;
    let object = s.core.object_class;
    // `f(I)`: [superclass, mixins of I, f(superclass)…].
    fn f(program: &dartforge_elements::model::Program, object: Option<ClassId>, i: ClassId, acc: &mut Vec<ClassId>, out: &mut Vec<ClassId>) {
        if Some(i) == object || acc.contains(&i) {
            return;
        }
        acc.push(i);
        let e = program.class(i);
        if let Some(sup) = e.supertype_class {
            out.push(sup);
        }
        out.extend(e.mixin_classes.iter().copied());
        if let Some(sup) = e.supertype_class {
            f(program, object, sup, acc, out);
        }
    }
    let mut out: Vec<ClassId> = Vec::new();
    let mut acc: Vec<ClassId> = Vec::new();
    let e = program.class(class_id);
    if e.kind == ClassKind::Mixin {
        for &t in &e.on_classes {
            out.push(t);
            f(program, object, t, &mut acc, &mut out);
        }
    } else {
        f(program, object, class_id, &mut acc, &mut out);
    }
    let mut seen: HashSet<ClassId> = HashSet::new();
    out.into_iter().filter(|c| *c != class_id && seen.insert(*c)).collect()
}
