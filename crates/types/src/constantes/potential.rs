//! `getNotPotentiallyConstants` do analyzer 6.11
//! (`src/dart/constant/potentially_constant.dart`): os nós de uma expressão
//! que não são potencialmente constantes.

use super::avaliador::{Ctx, Motor};
use crate::resolved::{MemberRef, Resolved};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Element, FunctionKind};
use dartforge_frontend::ast::{self, CollectionElement, ExprId, ExprKind, UnaryOp};

/// Onde os parâmetros do construtor valem como constantes: dentro dos
/// inicializadores de um construtor `const` (o ambiente léxico do
/// avaliador, ou a verificação dos inicializadores).
pub fn coletar(m: &Motor<'_>, cx: &Ctx, e: ExprId, nodes: &mut Vec<Span>) {
    collect_into(m, cx, e, cx.lexical.is_some(), true, nodes);
}

/// Como [`coletar`], dizendo se os parâmetros valem e se o nó está num
/// contexto constante.
pub fn collect_into(m: &Motor<'_>, cx: &Ctx, e: ExprId, params_list: bool, in_const: bool, nodes: &mut Vec<Span>) {
    let u = cx.unidade;
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
        ExprKind::Identifier(_) => identificador(m, cx, e, params_list, nodes),
        ExprKind::InstanceCreation { keyword, .. } => {
            if !matches!(keyword, Some(ast::CreationKeyword::Const)) {
                nodes.push(span);
            }
        }
        ExprKind::Call { target, arguments } => {
            // `const .id(…)` é sempre um `DotShorthandConstructorInvocation`
            // constante (`isConst` pela palavra), resolvido ou não
            // (`potentially_constant.dart`, 3.13).
            if matches!(a.expr(*target).kind, ExprKind::DotShorthand { const_: true, .. }) {
                return;
            }
            if let Some(Resolved::Constructor(_)) = m.resolvido(u, e) {
                if !in_const {
                    nodes.push(span);
                }
                return;
            }
            // `identical(a, b)`.
            if arguments.args.len() == 2 {
                if let ExprKind::Identifier(n) = &a.expr(*target).kind {
                    if m.interner.resolve(n.sym) == "identical" {
                        if let Some(Resolved::Element(Element::Function(f))) = m.resolvido(u, *target) {
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
            // `_typedLiteral` (`potentially_constant.dart:340-383`): com um
            // argumento de tipo (lista, conjunto), ele é tipo potencialmente
            // constante; com dois (mapa), a chave e o valor são tipos
            // constantes — o parâmetro de tipo não serve.
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
                elemento(m, cx, el, params_list, nodes);
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
                identificador(m, cx, e, params_list, nodes);
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
        // `DotShorthandNameExpression` (3.13): o getter de uma variável
        // `const` (a constante de enum, o campo estático `const`) e o tear-off
        // de um método estático são potencialmente constantes; o resto
        // (getter, variável não constante, nome que não resolve) acusa o
        // nome depois do ponto (`propertyName`).
        ExprKind::DotShorthand { name, .. } => {
            let ok = match m.resolvido(u, e) {
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
            // `FunctionReference`/`TypeLiteral`: os argumentos de tipo são
            // aceitos; a função é coletada.
            if !matches!(m.resolvido(u, *target), Some(Resolved::Element(Element::Class(_) | Element::Typedef(_)))) {
                collect_into(m, cx, *target, params_list, in_const, nodes);
            }
        }
        _ => nodes.push(span),
    }
}

/// `_ConstantTypeChecker.check` (`potentially_constant.dart:393-476`): o
/// tipo escrito `t` é constante (ou, com `potencial`, potencialmente
/// constante: o parâmetro de tipo também serve). O nomeado é constante
/// quando nomeia classe ou alias sem prefixo diferido (e os argumentos
/// também), ou quando é `dynamic`, `Never` ou `void`; a função, pelo
/// retorno, os limites e os parâmetros simples; o record, pelos campos.
fn constant_type(m: &Motor<'_>, u: dartforge_elements::model::UnitId, t: ast::TypeId, potential: bool) -> bool {
    use crate::table::Type;
    let a = &m.program.unit(u).ast;
    let resolved = m.body.units.get(u.0 as usize).and_then(|b| b.tipos_de_anotacoes.get(&t).copied());
    match &a.ty(t).kind {
        ast::TypeKind::Void => true,
        ast::TypeKind::Named { name, args } => {
            let is_parameter = resolved.is_some_and(|x| matches!(m.table.get(x), Type::TypeParameter { .. }));
            if is_parameter {
                return potential;
            }
            let lib = m.program.unit(u).library;
            let (binding, deferred) = match &name[..] {
                [n] => (m.program.lookup_na_unidade(u, n.sym), false),
                [p, n] => (
                    m.program.lookup_prefixed_na_unidade(u, p.sym, n.sym),
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

fn elemento(m: &Motor<'_>, cx: &Ctx, el: &CollectionElement, params_list: bool, nodes: &mut Vec<Span>) {
    match el {
        CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => collect_into(m, cx, *x, params_list, true, nodes),
        CollectionElement::MapEntry { key, value, .. } => {
            collect_into(m, cx, *key, params_list, true, nodes);
            collect_into(m, cx, *value, params_list, true, nodes);
        }
        CollectionElement::Spread { value, .. } => collect_into(m, cx, *value, params_list, true, nodes),
        CollectionElement::If { condition, then, else_, case_pattern, .. } => {
            if case_pattern.is_some() {
                nodes.push(m.element_span(cx.unidade, el));
                return;
            }
            collect_into(m, cx, *condition, params_list, true, nodes);
            elemento(m, cx, then, params_list, nodes);
            if let Some(x) = else_ {
                elemento(m, cx, x, params_list, nodes);
            }
        }
        CollectionElement::For { .. } | CollectionElement::ForIn { .. } => nodes.push(m.element_span(cx.unidade, el)),
    }
}

/// Um getter que lê uma variável: `Some(Some(const))`; getter sem
/// variável: `Some(None)`; outra coisa: `None`.
fn getter_variable(m: &Motor<'_>, cx: &Ctx, e: ExprId) -> Option<Option<bool>> {
    match m.resolvido(cx.unidade, e)? {
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

/// `_identifier` (identificador simples ou prefixado `a.b`).
fn identificador(m: &Motor<'_>, cx: &Ctx, e: ExprId, params_list: bool, nodes: &mut Vec<Span>) {
    let u = cx.unidade;
    let a = m.ast(u);
    let span = a.expr(e).span;
    let res = m.resolvido(u, e);
    if let ExprKind::Property { target, name, .. } = &a.expr(e).kind {
        if m.interner.resolve(name.sym) == "length" {
            collect_into(m, cx, *target, params_list, true, nodes);
            return;
        }
        if let Some(Resolved::Member { member: MemberRef::Function(f), .. }) = res {
            let fe = m.program.function(*f);
            if fe.static_ && matches!(fe.kind, FunctionKind::Function | FunctionKind::Operator) {
                if !matches!(m.resolvido(u, *target), Some(Resolved::Element(Element::Class(_) | Element::Typedef(_)))) {
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
