//! `getNotPotentiallyConstants` of analyzer 6.11
//! (`src/dart/constant/potentially_constant.dart`): the nodes of an
//! expression that are not potentially constant.

use super::evaluation::{Ctx, Engine};
use crate::resolved::{MemberRef, Resolved};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Element, FunctionKind};
use dartforge_frontend::ast::{self, CollectionElement, ExprId, ExprKind, UnaryOp};

/// Where the constructor's parameters count as constants: inside the
/// initializers of a `const` constructor (the evaluator's lexical
/// environment, or the verification of the initializers).
pub fn the_collect(m: &Engine<'_>, cx: &Ctx, e: ExprId, nodes: &mut Vec<Span>) {
    collect_into(m, cx, e, cx.lexical.is_some(), true, nodes);
}

/// Like [`coletar`], saying whether the parameters count and whether the
/// node is in a constant context.
pub fn collect_into(m: &Engine<'_>, cx: &Ctx, e: ExprId, params_list: bool, in_const: bool, nodes: &mut Vec<Span>) {
    let u = cx.unit;
    let a = m.ast(u);
    let span = a.expr(e).span;
    match &a.expr(e).kind {
        ExprKind::Int(_) | ExprKind::Double(_) | ExprKind::Bool(_) | ExprKind::Null | ExprKind::Symbol(_) => {}
        ExprKind::String(lit) => {
            for p in lit.parts.iter() {
                if let ast::StringPart::Interpolation(x) = p {
                    collect_into(m, cx, *x, params_list, in_const, nodes);
                }
            }
        }
        ExprKind::Identifier(_) => identifier(m, cx, e, params_list, nodes),
        ExprKind::InstanceCreation { keyword, .. } => {
            if !matches!(keyword, Some(ast::CreationKeyword::Const)) {
                nodes.push(span);
            }
        }
        ExprKind::Call { target, arguments } => {
            // `const .id(…)` is always a constant `DotShorthandConstructorInvocation`
            // (`isConst` by the keyword), resolved or not
            // (`potentially_constant.dart`, 3.13).
            if matches!(a.expr(*target).kind, ExprKind::DotShorthand { const_: true, .. }) {
                return;
            }
            if let Some(Resolved::Constructor(_)) = m.resolved(u, e) {
                if !in_const {
                    nodes.push(span);
                }
                return;
            }
            // `identical(a, b)`.
            if arguments.args.len() == 2 {
                if let ExprKind::Identifier(n) = &a.expr(*target).kind {
                    if m.interner.resolve(n.sym) == "identical" {
                        if let Some(Resolved::Element(Element::Function(f))) = m.resolved(u, *target) {
                            if Some(m.program.function(*f).library) == m.core.core_library {
                                collect_into(m, cx, arguments.args[0].value, params_list, in_const, nodes);
                                collect_into(m, cx, arguments.args[1].value, params_list, in_const, nodes);
                                return;
                            }
                        }
                    }
                }
            }
            nodes.push(span);
        }
        ExprKind::List { const_, elements, type_args } | ExprKind::SetOrMap { const_, elements, type_args } => {
            if !(*const_ || in_const) {
                nodes.push(span);
                return;
            }
            // `_typedLiteral` (`potentially_constant.dart:340-383`): with one type
            // argument (list, set), it is a potentially constant type; with two
            // (map), the key and the value are constant types: a type parameter
            // does not qualify.
            match &type_args[..] {
                [t] => {
                    if !constant_type(m, u, *t, true) {
                        nodes.push(a.ty(*t).span);
                    }
                }
                [k, v] => {
                    for t in [*k, *v] {
                        if !constant_type(m, u, t, false) {
                            nodes.push(a.ty(t).span);
                        }
                    }
                }
                _ => {}
            }
            for el in elements.iter() {
                element(m, cx, el, params_list, nodes);
            }
        }
        ExprKind::Parenthesized(x) => collect_into(m, cx, *x, params_list, in_const, nodes),
        ExprKind::Record { const_, positional, named } => {
            let c = *const_ || in_const;
            for x in positional.iter() {
                collect_into(m, cx, *x, params_list, c, nodes);
            }
            for (_, x) in named.iter() {
                collect_into(m, cx, *x, params_list, c, nodes);
            }
        }
        ExprKind::Binary { left, right, .. } => {
            collect_into(m, cx, *left, params_list, in_const, nodes);
            collect_into(m, cx, *right, params_list, in_const, nodes);
        }
        ExprKind::Unary { op, operand } => {
            if matches!(op, UnaryOp::Not | UnaryOp::Neg | UnaryOp::BitNot) {
                collect_into(m, cx, *operand, params_list, in_const, nodes);
            } else {
                nodes.push(span);
            }
        }
        ExprKind::Conditional { condition, then, else_ } => {
            collect_into(m, cx, *condition, params_list, in_const, nodes);
            collect_into(m, cx, *then, params_list, in_const, nodes);
            collect_into(m, cx, *else_, params_list, in_const, nodes);
        }
        ExprKind::Property { target, name, .. } => {
            if matches!(a.expr(*target).kind, ExprKind::Identifier(_)) {
                identifier(m, cx, e, params_list, nodes);
                return;
            }
            // `PropertyAccess`.
            if m.interner.resolve(name.sym) == "length" {
                collect_into(m, cx, *target, params_list, in_const, nodes);
                return;
            }
            if let ExprKind::Property { target: t2, .. } = &a.expr(*target).kind {
                if matches!(a.expr(*t2).kind, ExprKind::Identifier(_)) {
                    match getter_variable(m, cx, e) {
                        Some(Some(true)) | Some(None) => return,
                        Some(Some(false)) => {
                            nodes.push(name.span);
                            return;
                        }
                        None => {}
                    }
                }
            }
            nodes.push(span);
        }
        ExprKind::As { value, .. } | ExprKind::Is { value, .. } => collect_into(m, cx, *value, params_list, in_const, nodes),
        // `DotShorthandNameExpression` (3.13): the getter of a `const`
        // variable (the enum constant, the `const` static field) and the
        // tear-off of a static method are potentially constant; the rest
        // (getter, non-constant variable, name that does not resolve) reports
        // the name after the dot (`propertyName`).
        ExprKind::DotShorthand { name, .. } => {
            let ok = match m.resolved(u, e) {
                Some(Resolved::Element(Element::Class(d))) => {
                    let k = m.program.class(*d);
                    match k.static_members.get(&name.sym) {
                        Some(f) => {
                            let fe = m.program.function(*f);
                            match fe.kind {
                                FunctionKind::ImplicitAccessor => fe.variable.is_some_and(|v| m.program.variable(v).const_),
                                FunctionKind::Getter | FunctionKind::Setter => false,
                                _ => true,
                            }
                        }
                        None => k
                            .enum_constants
                            .iter()
                            .chain(k.fields.iter())
                            .any(|&v| m.program.variable(v).name == name.sym && m.program.variable(v).static_ && m.program.variable(v).const_),
                    }
                }
                _ => false,
            };
            if !ok {
                nodes.push(name.span);
            }
        }
        ExprKind::TypeArguments { target, .. } => {
            // `FunctionReference`/`TypeLiteral`: the type arguments are accepted;
            // the function is collected.
            if !matches!(m.resolved(u, *target), Some(Resolved::Element(Element::Class(_) | Element::Typedef(_)))) {
                collect_into(m, cx, *target, params_list, in_const, nodes);
            }
        }
        _ => nodes.push(span),
    }
}

/// `_ConstantTypeChecker.check` (`potentially_constant.dart:393-476`): the
/// written type `t` is constant (or, with `potential`, potentially constant:
/// a type parameter also qualifies). A named type is constant when it names
/// a class or alias without a deferred prefix (and its arguments are too),
/// or when it is `dynamic`, `Never` or `void`; a function, through its
/// return, bounds and simple parameters; a record, through its fields.
fn constant_type(m: &Engine<'_>, u: dartforge_elements::model::UnitId, t: ast::TypeId, potential: bool) -> bool {
    use crate::table::Type;
    let a = &m.program.unit(u).ast;
    let resolved = m.body.units.get(u.0 as usize).and_then(|b| b.annotation_types.get(&t).copied());
    match &a.ty(t).kind {
        ast::TypeKind::Void => true,
        ast::TypeKind::Named { name, args } => {
            let is_parameter = resolved.is_some_and(|x| matches!(m.table.get(x), Type::TypeParameter { .. }));
            if is_parameter {
                return potential;
            }
            let lib = m.program.unit(u).library;
            let (binding, deferred) = match &name[..] {
                [n] => (m.program.lookup_in_unit(u, n.sym), false),
                [p, n] => (
                    m.program.lookup_prefixed_in_unit(u, p.sym, n.sym),
                    m.program.library(lib).imports.iter().any(|i| i.prefix == Some(p.sym) && i.deferred),
                ),
                _ => (None, false),
            };
            let named_one = matches!(binding.and_then(|b| b.getter), Some(Element::Class(_) | Element::Typedef(_)));
            if named_one {
                return !deferred && args.iter().all(|&x| constant_type(m, u, x, potential));
            }
            resolved.is_some_and(|x| matches!(m.table.get(x), Type::Dynamic | Type::Never | Type::Void))
        }
        ast::TypeKind::Function { return_type, type_params, parameters } => {
            if let Some(r) = return_type
                && !constant_type(m, u, *r, potential)
            {
                return false;
            }
            if type_params.iter().filter_map(|p| p.bound).any(|b| !constant_type(m, u, b, potential)) {
                return false;
            }
            parameters.iter().filter(|p| p.function_parameters.is_none()).all(|p| p.ty.is_none_or(|x| constant_type(m, u, x, potential)))
        }
        ast::TypeKind::Record { positional, named } => {
            positional.iter().chain(named.iter().map(|(_, t)| t)).all(|&x| constant_type(m, u, x, potential))
        }
    }
}

fn element(m: &Engine<'_>, cx: &Ctx, el: &CollectionElement, params_list: bool, nodes: &mut Vec<Span>) {
    match el {
        CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => collect_into(m, cx, *x, params_list, true, nodes),
        CollectionElement::MapEntry { key, value, .. } => {
            collect_into(m, cx, *key, params_list, true, nodes);
            collect_into(m, cx, *value, params_list, true, nodes);
        }
        CollectionElement::Spread { value, .. } => collect_into(m, cx, *value, params_list, true, nodes),
        CollectionElement::If { condition, then, else_, case_pattern, .. } => {
            if case_pattern.is_some() {
                nodes.push(m.element_span(cx.unit, el));
                return;
            }
            collect_into(m, cx, *condition, params_list, true, nodes);
            element(m, cx, then, params_list, nodes);
            if let Some(x) = else_ {
                element(m, cx, x, params_list, nodes);
            }
        }
        CollectionElement::For { .. } | CollectionElement::ForIn { .. } => nodes.push(m.element_span(cx.unit, el)),
    }
}

/// A getter that reads a variable: `Some(Some(const))`; a getter without a
/// variable: `Some(None)`; something else: `None`.
fn getter_variable(m: &Engine<'_>, cx: &Ctx, e: ExprId) -> Option<Option<bool>> {
    match m.resolved(cx.unit, e)? {
        Resolved::Element(Element::Variable(v)) | Resolved::Member { member: MemberRef::Variable(v), .. } => {
            Some(Some(m.program.variable(*v).const_))
        }
        Resolved::Element(Element::Function(f)) | Resolved::Member { member: MemberRef::Function(f), .. } => {
            let fe = m.program.function(*f);
            match fe.kind {
                FunctionKind::ImplicitAccessor => Some(fe.variable.map(|v| m.program.variable(v).const_)),
                FunctionKind::Getter => Some(Some(false)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// `_identifier` (simple or prefixed identifier `a.b`).
fn identifier(m: &Engine<'_>, cx: &Ctx, e: ExprId, params_list: bool, nodes: &mut Vec<Span>) {
    let u = cx.unit;
    let a = m.ast(u);
    let span = a.expr(e).span;
    let res = m.resolved(u, e);
    if let ExprKind::Property { target, name, .. } = &a.expr(e).kind {
        if m.interner.resolve(name.sym) == "length" {
            collect_into(m, cx, *target, params_list, true, nodes);
            return;
        }
        if let Some(Resolved::Member { member: MemberRef::Function(f), .. }) = res {
            let fe = m.program.function(*f);
            if fe.static_ && matches!(fe.kind, FunctionKind::Function | FunctionKind::Operator) {
                if !matches!(m.resolved(u, *target), Some(Resolved::Element(Element::Class(_) | Element::Typedef(_)))) {
                    nodes.push(span);
                }
                return;
            }
        }
    }
    match res {
        Some(Resolved::Parameter { .. }) => {
            if !params_list {
                nodes.push(span);
            }
        }
        Some(Resolved::Local(_)) => {
            if m.parameter_local(u, e) {
                if !params_list {
                    nodes.push(span);
                }
            } else if !m.constant_local(u, e) {
                nodes.push(span);
            }
        }
        Some(Resolved::Element(Element::Variable(v))) | Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => {
            if !m.program.variable(*v).const_ {
                nodes.push(span);
            }
        }
        Some(Resolved::Element(Element::Function(f))) | Some(Resolved::Member { member: MemberRef::Function(f), .. }) => {
            let fe = m.program.function(*f);
            match fe.kind {
                FunctionKind::ImplicitAccessor => {
                    if fe.variable.is_some_and(|v| !m.program.variable(v).const_) {
                        nodes.push(span);
                    }
                }
                FunctionKind::Getter | FunctionKind::Setter => nodes.push(span),
                _ => {
                    let top_level = fe.class.is_none() && fe.extension.is_none();
                    if !(top_level || fe.static_) {
                        nodes.push(span);
                    }
                }
            }
        }
        Some(Resolved::Element(Element::Class(_) | Element::Typedef(_))) => {}
        Some(Resolved::TypeParameter(_)) => {}
        Some(Resolved::Constructor(_)) => {}
        _ => nodes.push(span),
    }
}
