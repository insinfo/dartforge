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
pub fn coletar(m: &Motor<'_>, cx: &Ctx, e: ExprId, nos: &mut Vec<Span>) {
    coletar_em(m, cx, e, cx.lexico.is_some(), true, nos);
}

/// Como [`coletar`], dizendo se os parâmetros valem e se o nó está num
/// contexto constante.
pub fn coletar_em(m: &Motor<'_>, cx: &Ctx, e: ExprId, parametros: bool, em_const: bool, nos: &mut Vec<Span>) {
    let u = cx.unidade;
    let a = m.ast(u);
    let span = a.expr(e).span;
    match &a.expr(e).kind {
        ExprKind::Int(_) | ExprKind::Double(_) | ExprKind::Bool(_) | ExprKind::Null | ExprKind::Symbol(_) => {}
        ExprKind::String(lit) => {
            for p in lit.parts.iter() {
                if let ast::StringPart::Interpolation(x) = p {
                    coletar_em(m, cx, *x, parametros, em_const, nos);
                }
            }
        }
        ExprKind::Identifier(_) => identificador(m, cx, e, parametros, nos),
        ExprKind::InstanceCreation { keyword, .. } => {
            if !matches!(keyword, Some(ast::CreationKeyword::Const)) {
                nos.push(span);
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
                if !em_const {
                    nos.push(span);
                }
                return;
            }
            // `identical(a, b)`.
            if arguments.args.len() == 2 {
                if let ExprKind::Identifier(n) = &a.expr(*target).kind {
                    if m.interner.resolve(n.sym) == "identical" {
                        if let Some(Resolved::Element(Element::Function(f))) = m.resolvido(u, *target) {
                            if Some(m.program.function(*f).library) == m.core.core_library {
                                coletar_em(m, cx, arguments.args[0].value, parametros, em_const, nos);
                                coletar_em(m, cx, arguments.args[1].value, parametros, em_const, nos);
                                return;
                            }
                        }
                    }
                }
            }
            nos.push(span);
        }
        ExprKind::List { const_, elements, type_args } | ExprKind::SetOrMap { const_, elements, type_args } => {
            if !(*const_ || em_const) {
                nos.push(span);
                return;
            }
            // `_typedLiteral` (`potentially_constant.dart:340-383`): com um
            // argumento de tipo (lista, conjunto), ele é tipo potencialmente
            // constante; com dois (mapa), a chave e o valor são tipos
            // constantes — o parâmetro de tipo não serve.
            match &type_args[..] {
                [t] => {
                    if !tipo_constante(m, u, *t, true) {
                        nos.push(a.ty(*t).span);
                    }
                }
                [k, v] => {
                    for t in [*k, *v] {
                        if !tipo_constante(m, u, t, false) {
                            nos.push(a.ty(t).span);
                        }
                    }
                }
                _ => {}
            }
            for el in elements.iter() {
                elemento(m, cx, el, parametros, nos);
            }
        }
        ExprKind::Parenthesized(x) => coletar_em(m, cx, *x, parametros, em_const, nos),
        ExprKind::Record { const_, positional, named } => {
            let c = *const_ || em_const;
            for x in positional.iter() {
                coletar_em(m, cx, *x, parametros, c, nos);
            }
            for (_, x) in named.iter() {
                coletar_em(m, cx, *x, parametros, c, nos);
            }
        }
        ExprKind::Binary { left, right, .. } => {
            coletar_em(m, cx, *left, parametros, em_const, nos);
            coletar_em(m, cx, *right, parametros, em_const, nos);
        }
        ExprKind::Unary { op, operand } => {
            if matches!(op, UnaryOp::Not | UnaryOp::Neg | UnaryOp::BitNot) {
                coletar_em(m, cx, *operand, parametros, em_const, nos);
            } else {
                nos.push(span);
            }
        }
        ExprKind::Conditional { condition, then, else_ } => {
            coletar_em(m, cx, *condition, parametros, em_const, nos);
            coletar_em(m, cx, *then, parametros, em_const, nos);
            coletar_em(m, cx, *else_, parametros, em_const, nos);
        }
        ExprKind::Property { target, name, .. } => {
            if matches!(a.expr(*target).kind, ExprKind::Identifier(_)) {
                identificador(m, cx, e, parametros, nos);
                return;
            }
            // `PropertyAccess`.
            if m.interner.resolve(name.sym) == "length" {
                coletar_em(m, cx, *target, parametros, em_const, nos);
                return;
            }
            if let ExprKind::Property { target: t2, .. } = &a.expr(*target).kind {
                if matches!(a.expr(*t2).kind, ExprKind::Identifier(_)) {
                    match variavel_do_getter(m, cx, e) {
                        Some(Some(true)) | Some(None) => return,
                        Some(Some(false)) => {
                            nos.push(name.span);
                            return;
                        }
                        None => {}
                    }
                }
            }
            nos.push(span);
        }
        ExprKind::As { value, .. } | ExprKind::Is { value, .. } => coletar_em(m, cx, *value, parametros, em_const, nos),
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
                nos.push(name.span);
            }
        }
        ExprKind::TypeArguments { target, .. } => {
            // `FunctionReference`/`TypeLiteral`: os argumentos de tipo são
            // aceitos; a função é coletada.
            if !matches!(m.resolvido(u, *target), Some(Resolved::Element(Element::Class(_) | Element::Typedef(_)))) {
                coletar_em(m, cx, *target, parametros, em_const, nos);
            }
        }
        _ => nos.push(span),
    }
}

/// `_ConstantTypeChecker.check` (`potentially_constant.dart:393-476`): o
/// tipo escrito `t` é constante (ou, com `potencial`, potencialmente
/// constante: o parâmetro de tipo também serve). O nomeado é constante
/// quando nomeia classe ou alias sem prefixo diferido (e os argumentos
/// também), ou quando é `dynamic`, `Never` ou `void`; a função, pelo
/// retorno, os limites e os parâmetros simples; o record, pelos campos.
fn tipo_constante(m: &Motor<'_>, u: dartforge_elements::model::UnitId, t: ast::TypeId, potencial: bool) -> bool {
    use crate::table::Type;
    let a = &m.program.unit(u).ast;
    let resolvido = m.body.units.get(u.0 as usize).and_then(|b| b.tipos_de_anotacoes.get(&t).copied());
    match &a.ty(t).kind {
        ast::TypeKind::Void => true,
        ast::TypeKind::Named { name, args } => {
            let e_parametro = resolvido.is_some_and(|x| matches!(m.table.get(x), Type::TypeParameter { .. }));
            if e_parametro {
                return potencial;
            }
            let lib = m.program.unit(u).library;
            let (ligacao, diferido) = match &name[..] {
                [n] => (m.program.lookup_na_unidade(u, n.sym), false),
                [p, n] => (
                    m.program.lookup_prefixed_na_unidade(u, p.sym, n.sym),
                    m.program.library(lib).imports.iter().any(|i| i.prefix == Some(p.sym) && i.deferred),
                ),
                _ => (None, false),
            };
            let nomeado = matches!(ligacao.and_then(|b| b.getter), Some(Element::Class(_) | Element::Typedef(_)));
            if nomeado {
                return !diferido && args.iter().all(|&x| tipo_constante(m, u, x, potencial));
            }
            resolvido.is_some_and(|x| matches!(m.table.get(x), Type::Dynamic | Type::Never | Type::Void))
        }
        ast::TypeKind::Function { return_type, type_params, parameters } => {
            if let Some(r) = return_type
                && !tipo_constante(m, u, *r, potencial)
            {
                return false;
            }
            if type_params.iter().filter_map(|p| p.bound).any(|b| !tipo_constante(m, u, b, potencial)) {
                return false;
            }
            parameters.iter().filter(|p| p.function_parameters.is_none()).all(|p| p.ty.is_none_or(|x| tipo_constante(m, u, x, potencial)))
        }
        ast::TypeKind::Record { positional, named } => {
            positional.iter().chain(named.iter().map(|(_, t)| t)).all(|&x| tipo_constante(m, u, x, potencial))
        }
    }
}

fn elemento(m: &Motor<'_>, cx: &Ctx, el: &CollectionElement, parametros: bool, nos: &mut Vec<Span>) {
    match el {
        CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => coletar_em(m, cx, *x, parametros, true, nos),
        CollectionElement::MapEntry { key, value, .. } => {
            coletar_em(m, cx, *key, parametros, true, nos);
            coletar_em(m, cx, *value, parametros, true, nos);
        }
        CollectionElement::Spread { value, .. } => coletar_em(m, cx, *value, parametros, true, nos),
        CollectionElement::If { condition, then, else_, case_pattern, .. } => {
            if case_pattern.is_some() {
                nos.push(m.span_de_elemento(cx.unidade, el));
                return;
            }
            coletar_em(m, cx, *condition, parametros, true, nos);
            elemento(m, cx, then, parametros, nos);
            if let Some(x) = else_ {
                elemento(m, cx, x, parametros, nos);
            }
        }
        CollectionElement::For { .. } | CollectionElement::ForIn { .. } => nos.push(m.span_de_elemento(cx.unidade, el)),
    }
}

/// Um getter que lê uma variável: `Some(Some(const))`; getter sem
/// variável: `Some(None)`; outra coisa: `None`.
fn variavel_do_getter(m: &Motor<'_>, cx: &Ctx, e: ExprId) -> Option<Option<bool>> {
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
fn identificador(m: &Motor<'_>, cx: &Ctx, e: ExprId, parametros: bool, nos: &mut Vec<Span>) {
    let u = cx.unidade;
    let a = m.ast(u);
    let span = a.expr(e).span;
    let res = m.resolvido(u, e);
    if let ExprKind::Property { target, name, .. } = &a.expr(e).kind {
        if m.interner.resolve(name.sym) == "length" {
            coletar_em(m, cx, *target, parametros, true, nos);
            return;
        }
        if let Some(Resolved::Member { member: MemberRef::Function(f), .. }) = res {
            let fe = m.program.function(*f);
            if fe.static_ && matches!(fe.kind, FunctionKind::Function | FunctionKind::Operator) {
                if !matches!(m.resolvido(u, *target), Some(Resolved::Element(Element::Class(_) | Element::Typedef(_)))) {
                    nos.push(span);
                }
                return;
            }
        }
    }
    match res {
        Some(Resolved::Parameter { .. }) => {
            if !parametros {
                nos.push(span);
            }
        }
        Some(Resolved::Local(_)) => {
            if m.local_parametro(u, e) {
                if !parametros {
                    nos.push(span);
                }
            } else if !m.local_constante(u, e) {
                nos.push(span);
            }
        }
        Some(Resolved::Element(Element::Variable(v))) | Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => {
            if !m.program.variable(*v).const_ {
                nos.push(span);
            }
        }
        Some(Resolved::Element(Element::Function(f))) | Some(Resolved::Member { member: MemberRef::Function(f), .. }) => {
            let fe = m.program.function(*f);
            match fe.kind {
                FunctionKind::ImplicitAccessor => {
                    if fe.variable.is_some_and(|v| !m.program.variable(v).const_) {
                        nos.push(span);
                    }
                }
                FunctionKind::Getter | FunctionKind::Setter => nos.push(span),
                _ => {
                    let de_topo = fe.class.is_none() && fe.extension.is_none();
                    if !(de_topo || fe.static_) {
                        nos.push(span);
                    }
                }
            }
        }
        Some(Resolved::Element(Element::Class(_) | Element::Typedef(_))) => {}
        Some(Resolved::TypeParameter(_)) => {}
        Some(Resolved::Constructor(_)) => {}
        _ => nos.push(span),
    }
}
