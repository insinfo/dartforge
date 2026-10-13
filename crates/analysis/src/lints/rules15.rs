//! The fifteenth batch of lint rules (docs/ANALYZER-ESPECIFICACAO-INFRA.md
//! §8), written directly from the 3.6.2 emitters
//! (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`), of the `core`
//! set, through the unit's semantics:
//!
//! * `implicit_call_tearoffs`: the expressions that the resolver replaces with
//!   an `ImplicitCallReference` (`UnitBodyTypes::implicit_calls`), except
//!   the forms that `_shouldSkipImplicitCallReferenceDueToForm` skips (cascade
//!   target, `?:` branch, `??` operand, through parentheses).
//! * `prefer_iterable_whereType`: `target.where((x) => x is T)` with the
//!   target (the `realTarget`) of an interface type that is or implements
//!   `dart:core`'s `Iterable`.
//! * `null_check_on_nullable_type_parameter`: the `!` (postfix and pattern)
//!   on a nullable type parameter, with the `getExpectedType` of
//!   `unnecessary_null_checks.dart` (return and expression body through the
//!   nearest `FunctionExpression`, which a method is not; `yield`; `=`
//!   assignment; variable declaration; right operand of a binary; direct list
//!   and set element; map entry; argument, through the
//!   `staticParameterElement`), and the syntactic `isNullable`/`isNonNullable`
//!   of `TypeSystemImpl`.
//!
//! Written without compiling or running (2026-10-05).

use super::walk::{walk, Node};
use super::codes_g as c;
use super::rules::LintReport;
use super::LintCode;
use crate::Unit;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, FunctionRef, VariableRef};
use dartforge_frontend::ast::{
    AssignOp, AsyncModifier, Ast, BinaryOp, CollectionElement, DeclKind, ExprId, ExprKind, ForInit, FunctionBody, FunctionId, MemberKind,
    PatternId, PatternKind, StmtKind, UnaryOp,
};
use dartforge_intern::Interner;
use dartforge_types::resolved::Resolved;
use dartforge_types::table::{Type, TypeId, TypeTable};
use std::collections::HashMap;

/// `TypeSystemImpl.isNullable` (syntactic).
fn is_nullable(table: &TypeTable, core: &dartforge_types::table::CoreTypes, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => true,
        Type::Intersection { bound, .. } => is_nullable(table, core, *bound),
        Type::Interface { nullable, .. } | Type::TypeParameter { nullable, .. } | Type::ExtensionType { nullable, .. } => *nullable,
        Type::Function { nullable, .. } | Type::Record { nullable, .. } => *nullable,
        Type::FutureOr { arg, nullable } => *nullable || is_nullable(table, core, *arg),
        _ => t == core.unknown,
    }
}

/// `TypeSystemImpl.isNonNullable` (syntactic).
fn is_non_nullable(table: &TypeTable, core: &dartforge_types::table::CoreTypes, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => false,
        Type::Intersection { bound, .. } => is_non_nullable(table, core, *bound),
        Type::TypeParameter { param, nullable } => !*nullable && is_non_nullable(table, core, table.param(*param).bound),
        Type::FutureOr { arg, nullable } => !*nullable && is_non_nullable(table, core, *arg),
        Type::Interface { nullable, .. } | Type::ExtensionType { nullable, .. } => !*nullable,
        Type::Function { nullable, .. } | Type::Record { nullable, .. } => !*nullable,
        _ => t != core.unknown,
    }
}

/// `promoteToNonNull(a) == promoteToNonNull(b)` when `a` is a type parameter:
/// the same parameter (and the same promoted bound) without the `?`.
fn same_without_null(table: &TypeTable, a: TypeId, b: TypeId) -> bool {
    let key = |t: TypeId| match table.get(t) {
        Type::TypeParameter { param, .. } => Some((*param, None)),
        Type::Intersection { param, bound } => Some((*param, Some(*bound))),
        _ => None,
    };
    match (key(a), key(b)) {
        (Some((pa, ba)), Some((pb, bb))) => pa == pb && ba == bb,
        _ => false,
    }
}

fn unparenthesized(a: &Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

/// The function (the analyzer's `FunctionExpression`) and its type: literal,
/// local or top-level (a method is not a `FunctionExpression`).
pub(super) struct FunctionIndex {
    literals: HashMap<FunctionId, ExprId>,
    locals: HashMap<FunctionId, usize>,
    top_level: HashMap<FunctionId, dartforge_elements::model::FunctionElementId>,
    methods: std::collections::HashSet<FunctionId>,
}

impl FunctionIndex {
    pub(super) fn new(s: &super::Semantics<'_>, a: &Ast) -> FunctionIndex {
        let mut f = FunctionIndex { literals: HashMap::new(), locals: HashMap::new(), top_level: HashMap::new(), methods: Default::default() };
        for (k, e) in a.exprs.iter().enumerate() {
            if let ExprKind::FunctionExpression(g) = &e.kind {
                f.literals.insert(*g, ExprId(k as u32));
            }
        }
        for st in a.stmts.iter() {
            if let StmtKind::Function(g) = &st.kind
                && let Some(n) = a.function(*g).name
            {
                f.locals.insert(*g, n.span.start);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Method(g) = &m.kind {
                f.methods.insert(*g);
            }
        }
        for (k, fe) in s.program.functions.iter().enumerate() {
            if let FunctionRef::Function { unit, function } = fe.node
                && unit == s.unit
                && !f.methods.contains(&function)
            {
                f.top_level.insert(function, dartforge_elements::model::FunctionElementId(k as u32));
            }
        }
        f
    }

    /// The static type of `g`'s `FunctionExpression`, if `g` is one.
    fn fn_type(&self, s: &super::Semantics<'_>, g: FunctionId) -> Option<Option<TypeId>> {
        if self.methods.contains(&g) {
            return None;
        }
        if let Some(&e) = self.literals.get(&g) {
            return Some(s.body.get_type(e));
        }
        if let Some(&pos) = self.locals.get(&g) {
            return Some(s.body.local_type(pos));
        }
        if let Some(fe) = self.top_level.get(&g) {
            return Some(s.outline.functions.get(fe.0 as usize).map(|d| d.signature));
        }
        None
    }
}

/// The declared type of a variable's element (`declaredElement.type`).
fn variable_type(s: &super::Semantics<'_>, ref_: VariableRef, name_pos: usize, initializer: ExprId) -> Option<TypeId> {
    let v = s.program.variables.iter().position(|x| x.node == ref_)?;
    let d = s.outline.variables.get(v)?;
    d.declared_type.or(d.inferred).or_else(|| s.body.get_type(initializer)).or_else(|| s.body.local_type(name_pos))
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unit<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let Some(s) = sem else { return out };
    let a = u.ast;
    let mut report = |code: &'static LintCode, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    let any_enabled = ["implicit_call_tearoffs", "prefer_iterable_whereType", "null_check_on_nullable_type_parameter"].iter().any(|r| enabled(r));
    if !any_enabled {
        return out;
    }
    // The parent of each expression and the ancestors of the `!`s.
    let mut parent: HashMap<ExprId, Node> = HashMap::new();
    let mut bang_ancestors: HashMap<ExprId, Vec<Node>> = HashMap::new();
    walk(u, &mut |node, ancestors| {
        if let Node::Expr(e) = node {
            if let Some(p) = ancestors.last() {
                parent.insert(e, *p);
            }
            if matches!(a.expr(e).kind, ExprKind::Unary { op: UnaryOp::NullAssert, .. }) {
                bang_ancestors.insert(e, ancestors.to_vec());
            }
        }
    });
    let parent_expr = |e: ExprId| match parent.get(&e) {
        Some(Node::Expr(p)) => Some(*p),
        _ => None,
    };
    // The `realTarget` of a cascade section.
    let mut cascades: HashMap<ExprId, ExprId> = HashMap::new();
    for e in a.exprs.iter() {
        let ExprKind::Cascade { target, sections, .. } = &e.kind else { continue };
        for &sec in sections.iter() {
            let mut x = sec;
            loop {
                let next = match &a.expr(x).kind {
                    ExprKind::Property { target, .. }
                    | ExprKind::Index { target, .. }
                    | ExprKind::Call { target, .. }
                    | ExprKind::TypeArguments { target, .. }
                    | ExprKind::Assign { target, .. } => *target,
                    ExprKind::Unary { operand, .. } => *operand,
                    ExprKind::CascadeTarget => {
                        cascades.insert(x, *target);
                        break;
                    }
                    _ => break,
                };
                x = next;
            }
        }
    }
    let real = |x: ExprId| if matches!(a.expr(x).kind, ExprKind::CascadeTarget) { cascades.get(&x).copied() } else { Some(x) };
    let table = s.table;
    let core = s.core;

    // `implicit_call_tearoffs`.
    if enabled("implicit_call_tearoffs") {
        let mut implicit: Vec<ExprId> = s.body.implicit_calls.iter().copied().collect();
        implicit.sort_by_key(|e| (a.expr(*e).span.start, a.expr(*e).span.end));
        for e in implicit {
            let mut x = e;
            let mut p = parent_expr(x);
            while let Some(q) = p
                && matches!(a.expr(q).kind, ExprKind::Parenthesized(_))
            {
                x = q;
                p = parent_expr(q);
            }
            let skipped = p.is_some_and(|q| match &a.expr(q).kind {
                ExprKind::Cascade { target, .. } => *target == x,
                ExprKind::Conditional { then, else_, .. } => *then == x || *else_ == x,
                ExprKind::Binary { op: BinaryOp::IfNull, .. } => true,
                _ => false,
            });
            if !skipped {
                report(&c::IMPLICIT_CALL_TEAROFFS, a.expr(e).span, &[]);
            }
        }
    }

    // `prefer_iterable_whereType`.
    if enabled("prefer_iterable_whereType") {
        let implements_iterable = |t: TypeId| match table.get(t) {
            Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => {
                let is_iterable = |k: dartforge_elements::model::ClassId| {
                    let cl = s.program.class(k);
                    interner.resolve(cl.name) == "Iterable" && s.program.library(cl.library).uri == "dart:core"
                };
                is_iterable(*class)
                    || (s.program.class(*class).decl.is_some()
                        && s.outline.hierarchy.get(*class).is_some_and(|d| d.supertypes.keys().any(|k| is_iterable(*k))))
            }
            _ => false,
        };
        for e in a.exprs.iter() {
            let ExprKind::Call { target, arguments } = &e.kind else { continue };
            let ExprKind::Property { target: recv, name, .. } = &a.expr(*target).kind else { continue };
            if interner.resolve(name.sym) != "where" {
                continue;
            }
            let Some(recv) = real(*recv) else { continue };
            if !s.body.get_type(recv).is_some_and(implements_iterable) {
                continue;
            }
            let [arg] = &arguments.args[..] else { continue };
            let ExprKind::FunctionExpression(f) = &a.expr(arg.value).kind else { continue };
            let f = a.function(*f);
            let Some(ps) = &f.parameters else { continue };
            let [p] = &ps[..] else { continue };
            let expr = match &f.body {
                FunctionBody::Block(b) => match &a.stmt(*b).kind {
                    StmtKind::Block(cmds) if cmds.len() == 1 => match &a.stmt(cmds[0]).kind {
                        StmtKind::Return(v) => *v,
                        _ => None,
                    },
                    _ => None,
                },
                FunctionBody::Expression(x) => Some(*x),
                _ => None,
            };
            let Some(x) = expr.map(|x| unparenthesized(a, x)) else { continue };
            if let ExprKind::Is { value, negated: false, .. } = &a.expr(x).kind
                && let ExprKind::Identifier(n) = &a.expr(*value).kind
                && p.name.is_some_and(|pn| pn.sym == n.sym)
            {
                report(&c::PREFER_ITERABLE_WHERETYPE, name.span, &[]);
            }
        }
    }

    // `null_check_on_nullable_type_parameter`.
    if enabled("null_check_on_nullable_type_parameter") {
        let is_nullable_type_param = |t: TypeId| matches!(table.get(t), Type::TypeParameter { .. } | Type::Intersection { .. }) && is_nullable(table, core, t);
        // The `p!` pattern: the matched type.
        for (k, p) in a.patterns.iter().enumerate() {
            if let PatternKind::NullAssert(_) = &p.kind
                && let Some(&t) = s.body.matched_types.get(&PatternId(k as u32))
                && is_nullable_type_param(t)
            {
                report(&c::NULL_CHECK_ON_NULLABLE_TYPE_PARAMETER, Span { start: p.span.end - 1, end: p.span.end }, &[]);
            }
        }
        let functions = FunctionIndex::new(s, a);
        let mut bangs: Vec<(&ExprId, &Vec<Node>)> = bang_ancestors.iter().collect();
        bangs.sort_by_key(|(e, _)| a.expr(**e).span.start);
        for (&node, ancestors) in bangs {
            let ExprKind::Unary { operand, .. } = &a.expr(node).kind else { continue };
            let Some(t) = s.body.get_type(*operand) else { continue };
            if !is_nullable_type_param(t) {
                continue;
            }
            let expected = expected_type(s, a, interner, &functions, &parent, ancestors, node, &real);
            if let Some(exp) = expected
                && !is_non_nullable(table, core, exp)
                && same_without_null(table, t, exp)
            {
                let sp = a.expr(node).span;
                report(&c::NULL_CHECK_ON_NULLABLE_TYPE_PARAMETER, Span { start: sp.end - 1, end: sp.end }, &[]);
            }
        }
    }

    out
}

/// The type argument `i` of an interface type (of `Future`, `Iterable`…).
fn type_argument(table: &TypeTable, t: TypeId, i: usize) -> Option<TypeId> {
    match table.get(t) {
        Type::Interface { args, .. } => args.get(i).copied(),
        Type::FutureOr { arg, .. } if i == 0 => Some(*arg),
        _ => None,
    }
}

/// Argument `i` of `receiver.asInstanceOf(dc)`, when the receiver is an
/// interface: directly on the class itself; through the supertype
/// instantiated in terms of the receiver class's parameters, otherwise.
fn super_type_argument(s: &super::Semantics<'_>, receiver: TypeId, dc: ClassId, i: usize) -> Option<TypeId> {
    let table = s.table;
    let (rc, args) = match table.get(receiver) {
        Type::Interface { class, args, .. } => (*class, args.to_vec()),
        _ => return None,
    };
    if rc == dc {
        return args.get(i).copied();
    }
    let data = s.outline.hierarchy.get(rc)?;
    let template = *data.supertypes.get(&dc)?;
    let Type::Interface { args: margs, .. } = table.get(template) else { return None };
    let m = *margs.get(i)?;
    match table.get(m) {
        Type::TypeParameter { param, .. } => data.type_params.iter().position(|q| q == param).and_then(|j| args.get(j).copied()),
        _ => Some(m),
    }
}

/// `getExpectedType` (`unnecessary_null_checks.dart:15`).
#[allow(clippy::too_many_arguments)]
pub(super) fn expected_type(
    s: &super::Semantics<'_>,
    a: &Ast,
    interner: &Interner,
    functions: &FunctionIndex,
    parent: &HashMap<ExprId, Node>,
    ancestors: &[Node],
    node: ExprId,
    real_target: &dyn Fn(ExprId) -> Option<ExprId>,
) -> Option<TypeId> {
    let table = s.table;
    let core = s.core;
    let is_class = |t: TypeId, name: &str, lib: &str| match table.get(t) {
        Type::Interface { class, .. } => {
            let cl = s.program.class(*class);
            interner.resolve(cl.name) == name && s.program.library(cl.library).uri == lib
        }
        _ => false,
    };
    // `realNode`: the outermost parenthesis.
    let mut real = node;
    while let Some(Node::Expr(p)) = parent.get(&real)
        && matches!(a.expr(*p).kind, ExprKind::Parenthesized(_))
    {
        real = *p;
    }
    let mut child = real;
    let mut parent_node = parent.get(&real).copied();
    let with_await = matches!(parent_node, Some(Node::Expr(p)) if matches!(a.expr(p).kind, ExprKind::Await(_)));
    if with_await && let Some(Node::Expr(p)) = parent_node {
        child = p;
        parent_node = parent.get(&p).copied();
    }
    // The nearest `FunctionExpression` (a method is not one).
    let nearest_function = || -> Option<(FunctionId, Option<TypeId>)> {
        for n in ancestors.iter().rev() {
            if let Node::Function(g) = n {
                return functions.fn_type(s, *g).map(|t| (*g, t));
            }
        }
        None
    };
    let return_of = |t: Option<TypeId>| -> Option<TypeId> {
        match table.get(t?) {
            Type::Function { ret, .. } => Some(*ret),
            _ => None,
        }
    };
    // Return and expression body.
    let is_return = match parent_node {
        Some(Node::Stmt(st)) => matches!(a.stmt(st).kind, StmtKind::Return(Some(v)) if v == child),
        Some(Node::Function(g)) => matches!(a.function(g).body, FunctionBody::Expression(v) if v == child),
        _ => false,
    };
    if is_return {
        let (g, t) = nearest_function()?;
        let ret = return_of(t)?;
        if with_await || matches!(a.function(g).modifier, AsyncModifier::Async | AsyncModifier::AsyncStar) {
            return if is_class(ret, "Future", "dart:async") || matches!(table.get(ret), Type::FutureOr { .. }) { type_argument(table, ret, 0) } else { None };
        }
        return Some(ret);
    }
    // `yield`.
    if let Some(Node::Stmt(st)) = parent_node
        && matches!(a.stmt(st).kind, StmtKind::Yield { .. })
    {
        let (_, t) = nearest_function()?;
        let ret = return_of(t)?;
        return if is_class(ret, "Iterable", "dart:core") || is_class(ret, "Stream", "dart:async") { type_argument(table, ret, 0) } else { None };
    }
    let Some(parent_node) = parent_node else { return None };
    match parent_node {
        Node::Expr(p) => match &a.expr(p).kind {
            // `=` assignment (except `x = x!`).
            ExprKind::Assign { op: AssignOp::Assign, target, .. } => {
                let dotted_name = |e: ExprId| -> Option<String> {
                    match &a.expr(e).kind {
                        ExprKind::Identifier(n) => Some(interner.resolve(n.sym).to_string()),
                        ExprKind::Property { target, name, null_aware: false } => match &a.expr(*target).kind {
                            ExprKind::Identifier(m) => Some(format!("{}.{}", interner.resolve(m.sym), interner.resolve(name.sym))),
                            _ => None,
                        },
                        _ => None,
                    }
                };
                let ExprKind::Unary { operand, .. } = &a.expr(node).kind else { return None };
                match (dotted_name(*target), dotted_name(*operand)) {
                    (Some(x), Some(y)) if x == y => None,
                    _ => s.body.write_types.get(&p).copied(),
                }
            }
            // Right operand of a binary: the operator's first parameter.
            ExprKind::Binary { right, left, .. } if *right == real => {
                let Some(Resolved::Member { member: dartforge_types::resolved::MemberRef::Function(f), .. }) = s.body.get_resolved(p) else {
                    return None;
                };
                let sig = s.outline.functions.get(f.0 as usize)?.signature;
                let Type::Function { positional, .. } = table.get(sig) else { return None };
                let declared = *positional.first()?;
                // The member of the receiver's type (`MethodMember`): a type
                // parameter of the class that declares the operator becomes the
                // receiver's corresponding argument (the rest cannot be equal to
                // a type parameter of the `!`).
                let owner = s.program.function(*f).class;
                match (table.get(declared), owner, s.body.get_type(*left)) {
                    (Type::TypeParameter { param, .. }, Some(dc), Some(receiver)) => {
                        let i = s.outline.classes.get(dc.0 as usize).and_then(|d| d.type_params.iter().position(|q| q == param));
                        match i {
                            Some(i) => super_type_argument(s, receiver, dc, i).or(Some(declared)),
                            None => Some(declared),
                        }
                    }
                    _ => Some(declared),
                }
            }
            // Direct list and set element.
            ExprKind::List { elements, .. } => {
                if !elements.iter().any(|el| matches!(el, CollectionElement::Expression(x) if *x == real)) {
                    return None;
                }
                type_argument(table, s.body.get_type(p)?, 0)
            }
            ExprKind::SetOrMap { elements, .. } => {
                if elements.iter().any(|el| matches!(el, CollectionElement::Expression(x) if *x == real)) {
                    let t = s.body.get_type(p)?;
                    return if is_class(t, "Set", "dart:core") { type_argument(table, t, 0) } else { None };
                }
                // The map entry (also inside `for`/`if`).
                fn map_entry(els: &[CollectionElement], x: ExprId) -> Option<(ExprId, ExprId)> {
                    for el in els {
                        let found = match el {
                            CollectionElement::MapEntry { key, value, .. } if *key == x || *value == x => Some((*key, *value)),
                            CollectionElement::If { then, else_, .. } => {
                                map_entry(std::slice::from_ref(then), x).or_else(|| else_.as_ref().and_then(|e| map_entry(std::slice::from_ref(e), x)))
                            }
                            CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => map_entry(std::slice::from_ref(body), x),
                            _ => None,
                        };
                        if found.is_some() {
                            return found;
                        }
                    }
                    None
                }
                let (key, _) = map_entry(elements, real)?;
                let t = s.body.get_type(p)?;
                type_argument(table, t, if key == node { 0 } else { 1 })
            }
            // Argument (positional or named).
            ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } if arguments.args.iter().any(|x| x.value == real) => {
                if let Some(Resolved::Constructor(f)) = s.body.get_resolved(p) {
                    // `Future.value`.
                    let g = s.program.function(*f);
                    if let Some(cl) = g.class
                        && interner.resolve(g.name) == "value"
                    {
                        let k = s.program.class(cl);
                        if interner.resolve(k.name) == "Future" && s.program.library(k.library).uri == "dart:async" {
                            return None;
                        }
                    }
                } else if let ExprKind::Call { target, .. } = &a.expr(p).kind
                    && let ExprKind::Property { target: recv, name, .. } = &a.expr(*target).kind
                    && interner.resolve(name.sym) == "complete"
                    && let Some(recv) = real_target(*recv)
                    && let Some(t) = s.body.get_type(recv)
                    && is_class(t, "Completer", "dart:async")
                {
                    return None;
                }
                s.body.parameter_types.get(&real).copied()
            }
            _ => None,
        },
        // Variable declaration.
        Node::Stmt(st) => {
            let list = match &a.stmt(st).kind {
                StmtKind::Variables(l) => l,
                StmtKind::For { init: Some(ForInit::Variables(l)), .. } => l,
                _ => return None,
            };
            let v = list.variables.iter().find(|v| v.initializer == Some(real))?;
            s.body.local_type(v.name.span.start).or_else(|| s.body.get_type(real)).filter(|t| *t != core.unknown)
        }
        Node::Member(m) => {
            let MemberKind::Field(l) = &a.member(m).kind else { return None };
            let index = l.variables.iter().position(|v| v.initializer == Some(real))?;
            variable_type(s, VariableRef::Field { unit: s.unit, member: m, index }, l.variables[index].name.span.start, real)
        }
        Node::Decl(d) => {
            let DeclKind::Variables(l) = &a.decl(d).kind else { return None };
            let index = l.variables.iter().position(|v| v.initializer == Some(real))?;
            variable_type(s, VariableRef::TopLevel { unit: s.unit, decl: d, index }, l.variables[index].name.span.start, real)
        }
        _ => None,
    }
}
