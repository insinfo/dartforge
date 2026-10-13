//! Ajuda de assinatura (`textDocument/signatureHelp`) no formato do servidor
//! do Dart (`DartUnitSignatureComputer` + `toSignatureHelp`, ver
//! `docs/LSP.md`, "Paridade com o servidor do Dart"):
//!
//! * a lista de argumentos mais interna que contém o cursor, de uma chamada
//!   de método ou função (`f(…)`, `a.m(…)`) ou de uma criação de instância
//!   (`A(…)`, `A.name(…)`, `new p.A<T>.name(…)`); uma expressão de função
//!   entre o cursor e a lista interrompe a busca (o cursor no corpo de um
//!   *closure* passado como argumento não mostra a chamada de fora);
//! * o rótulo é `name(int a, [int b = 0], {required String c})`, cada
//!   parâmetro `ty name = padrão` com o tipo já substituído pelo receptor
//!   (`List<int>.add` mostra `int value`) e o padrão como escrito;
//! * o parâmetro ativo é o do argumento sob o cursor (nomeado pelo nome,
//!   posicional pela posição) ou, entre argumentos, o próximo posicional;
//! * a documentação é a do hover (inclusive a herdada de um membro
//!   sobrescrito);
//! * disparo automático por `(` só responde quando o `(` digitado é o que abre
//!   a lista.
//!
//! Tudo vem da inferência comum (`get_resolved`, `get_type`,
//! `MemberResolver::lookup_member`); nada é resolvido por nome aqui.

use crate::project::{Concrete, Project};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{FunctionElementId, FunctionKind, FunctionRef, UnitId};
use dartforge_frontend::ast::{self, ExprKind, MemberKind, ParameterKind};
use dartforge_intern::SymbolId;
use dartforge_types::{Type, TypeId};
use std::collections::HashMap;

/// Uma assinatura pronta para o protocolo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// `name(int a, {int b = 0})`.
    pub label: String,
    /// O rótulo de cada parâmetro, na ordem declarada.
    pub parameters: Vec<String>,
    pub documentation: Option<String>,
    /// Índice em `parameters` do parâmetro ativo, quando há.
    pub active: Option<usize>,
}

/// Um parâmetro como a assinatura o mostra.
struct Parameter {
    label: String,
    ty: ParameterKind,
    /// Nome externo (o da chamada).
    name: Option<String>,
}

impl Project {
    /// A ajuda de assinatura em `offset` de `unit`. `auto`: pedida
    /// pelo editor ao digitar `(` (sem a ajuda já aberta).
    pub(crate) fn signature(
        &mut self,
        unit: UnitId,
        offset: usize,
        auto: bool,
    ) -> Option<Signature> {
        let (call, arguments) = self.argument_list(unit, offset)?;
        if auto && offset != arguments.start + 1 {
            return None;
        }
        let (name, parameters, documentation) = self.callee(unit, call)?;
        let active = self.active_parameter(unit, call, offset, &parameters);
        let mut required = Vec::new();
        let mut optional = Vec::new();
        let mut named = Vec::new();
        for p in &parameters {
            match p.ty {
                ParameterKind::Required => required.push(p.label.clone()),
                ParameterKind::Optional => optional.push(p.label.clone()),
                ParameterKind::Named => named.push(p.label.clone()),
            }
        }
        let mut parts = Vec::new();
        if !required.is_empty() {
            parts.push(required.join(", "));
        }
        if !optional.is_empty() {
            parts.push(format!("[{}]", optional.join(", ")));
        }
        if !named.is_empty() {
            parts.push(format!("{{{}}}", named.join(", ")));
        }
        Some(Signature {
            label: format!("{name}({})", parts.join(", ")),
            parameters: parameters.into_iter().map(|p| p.label).collect(),
            documentation,
            active,
        })
    }

    /// A chamada (`Call` ou `InstanceCreation`) cuja lista de argumentos é a
    /// mais interna que contém `offset` (depois do `(` e até o `)`), e o
    /// intervalo da lista. `None` se uma expressão de função dentro da lista
    /// contém o cursor.
    fn argument_list(&self, unit: UnitId, offset: usize) -> Option<(ast::ExprId, Span)> {
        let u = self.program().unit(unit);
        let ast = &u.ast;
        let source = u.source.as_bytes();
        let mut best: Option<(ast::ExprId, Span)> = None;
        for (i, e) in ast.exprs.iter().enumerate() {
            let arguments_ = match &e.kind {
                ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => arguments,
                _ => continue,
            };
            let s = arguments_.span;
            let closed = s.end > s.start && source.get(s.end - 1) == Some(&b')');
            let inside = s.start < offset && if closed { offset < s.end } else { offset <= s.end };
            if inside && best.is_none_or(|(_, m)| s.end - s.start < m.end - m.start) {
                best = Some((ast::ExprId(i as u32), s));
            }
        }
        let (call, s) = best?;
        // `FunctionExpression` entre o cursor e a lista: o analyzer para.
        let slashed = ast
            .functions
            .iter()
            .any(|f| f.name.is_none() && s.start < f.span.start && f.span.end <= s.end && f.span.start <= offset && offset <= f.span.end);
        (!slashed).then_some((call, s))
    }

    /// Nome exibido, parâmetros e documentação do que a chamada invoca.
    fn callee(&mut self, unit: UnitId, call: ast::ExprId) -> Option<(String, Vec<Parameter>, Option<String>)> {
        /// O que a chamada é, copiado da árvore antes das consultas mutáveis.
        enum Shape {
            Creation { ty: Span, constructor: Option<ast::Name> },
            Call { target: ast::ExprId, target_span: Span, name: Span, receiver: Option<ast::ExprId> },
        }
        let source = self.program().unit(unit).source.clone();
        let text = |s: Span| clear_name(&source[s.start..s.end]);
        let shape = {
            let ast = &self.program().unit(unit).ast;
            match &ast.expr(call).kind {
                ExprKind::InstanceCreation { ty, constructor, .. } => Shape::Creation { ty: ast.ty(*ty).span, constructor: *constructor },
                ExprKind::Call { target, .. } => {
                    // `A<int>(…)` e `f<int>(…)`: o nome está dentro da
                    // instanciação explícita.
                    let target_ = match &ast.expr(*target).kind {
                        ExprKind::TypeArguments { target: t, .. } => ast.expr(*t),
                        _ => ast.expr(*target),
                    };
                    let target_span = ast.expr(*target).span;
                    let (name_, receiver) = match &target_.kind {
                        ExprKind::Identifier(n) => (n.span, None),
                        ExprKind::Property { target: r, name, .. } => (name.span, Some(*r)),
                        _ => return None,
                    };
                    Shape::Call { target: *target, target_span, name: name_, receiver }
                }
                _ => return None,
            }
        };
        match shape {
            Shape::Creation { ty: ty_, constructor: constructor_ } => {
                let mut name_ = text(ty_);
                if let Some(c) = constructor_ {
                    name_ = format!("{name_}.{}", &source[c.span.start..c.span.end]);
                }
                let f = match self.query.bodies.units[unit.0 as usize].get_resolved(call) {
                    Some(dartforge_types::Resolved::Constructor(f)) => *f,
                    _ => return None,
                };
                let parameters = self.constructor_parameters(unit, call, f);
                let documentation = self.documentation_of(unit, constructor_.map_or(ty_, |c| c.span));
                Some((name_, parameters, documentation))
            }
            Shape::Call { target: target_, target_span, name: name_span, receiver } => {
                let d = self.identify(unit, name_span.start).ok()??;
                let documentation = self.hover(unit, &d).and_then(|h| h.documentation);
                match d.concrete {
                    Some(Concrete::Function(f)) => match { self.program().function(f).kind } {
                        FunctionKind::Constructor | FunctionKind::SyntheticConstructor => {
                            let parameters = self.constructor_parameters(unit, call, f);
                            Some((text(target_span), parameters, documentation))
                        }
                        // Getter ou campo que devolve função: o analyzer
                        // não tem lista de parâmetros para mostrar.
                        FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => None,
                        _ => {
                            let name_ = source[name_span.start..name_span.end].to_string();
                            let ty_ = self.member_type(unit, receiver, target_, f);
                            Some((name_, self.function_parameters(f, ty_), documentation))
                        }
                    },
                    Some(Concrete::Variable(_)) => None,
                    None => {
                        // Função local: a árvore dá os nomes, o tipo do local
                        // dá os tipos.
                        let crate::project::Target::Local { unit: ul, declaration } = d.target else {
                            return None;
                        };
                        let name_ = source[name_span.start..name_span.end].to_string();
                        let ty_ = self.query.bodies.units[ul.0 as usize].tipo_local(declaration)?;
                        let ast_local = &self.program().unit(ul).ast;
                        let parameters = match ast_local.functions.iter().find(|f| f.name.is_some_and(|n| n.span.start == declaration)) {
                            Some(f) => {
                                let ps = f.parameters.as_deref().unwrap_or(&[]);
                                let types = aligned_types(&self.query.table, ps, ty_);
                                self.labels(ul, ps, &types)
                            }
                            // Variável local de tipo função: só os tipos.
                            None => self.type_parameters(ty_),
                        };
                        Some((name_, parameters, documentation))
                    }
                }
            }
        }
    }

    /// A documentação do hover no nome em `name`.
    fn documentation_of(&self, unit: UnitId, name: Span) -> Option<String> {
        let d = self.identify(unit, name.start).ok()??;
        self.hover(unit, &d).and_then(|h| h.documentation)
    }

    /// O tipo de função do membro chamado, já substituído pelo receptor:
    /// pela busca de membros da inferência comum quando há receptor, senão
    /// o tipo estático do alvo da chamada.
    fn member_type(
        &mut self,
        unit: UnitId,
        receiver: Option<ast::ExprId>,
        target: ast::ExprId,
        f: FunctionElementId,
    ) -> Option<TypeId> {
        let fe = self.program().function(f);
        if fe.static_ || (fe.class.is_none() && fe.extension.is_none()) {
            return None;
        }
        let symbol: SymbolId = fe.name;
        let lib = self.program().unit(unit).library;
        let type_r = receiver.and_then(|r| self.query.bodies.units[unit.0 as usize].get_type(r));
        if let Some(type_r) = type_r
            && let Some((_, t)) = self.query.resolver().lookup_member(type_r, symbol, false, lib)
            && matches!(self.query.table.get(t), Type::Function { .. })
        {
            return Some(t);
        }
        self.query.bodies.units[unit.0 as usize]
            .get_type(target)
            .filter(|t| matches!(self.query.table.get(*t), Type::Function { .. }))
    }

    /// Parâmetros de uma função, método ou operador; `ty` (substituído)
    /// vale mais que os tipos declarados quando tem a mesma forma.
    fn function_parameters(&self, f: FunctionElementId, ty: Option<TypeId>) -> Vec<Parameter> {
        let p = self.program();
        let FunctionRef::Function { unit, function } = p.function(f).node else {
            return Vec::new();
        };
        let ps = p.unit(unit).ast.function(function).parameters.as_deref().unwrap_or(&[]);
        let declared: Vec<Option<TypeId>> = self.query.outline.functions[f.0 as usize].parameters.iter().map(|q| Some(q.ty)).collect();
        let types = match ty {
            Some(t) => {
                let aligned = aligned_types(&self.query.table, ps, t);
                if aligned.iter().all(Option::is_some) { aligned } else { declared }
            }
            None => declared,
        };
        self.labels(unit, ps, &types)
    }

    /// Parâmetros de um construtor com os parâmetros de tipo da classe
    /// trocados pelos argumentos do tipo criado (`List<int>.filled` mostra
    /// `int fill`).
    fn constructor_parameters(&mut self, unit_: UnitId, call: ast::ExprId, f: FunctionElementId) -> Vec<Parameter> {
        let p = self.program();
        let FunctionRef::Constructor { unit, member } = p.function(f).node else {
            return Vec::new();
        };
        let MemberKind::Constructor(k) = &p.unit(unit).ast.member(member).kind else {
            return Vec::new();
        };
        let n = k.parameters.len();
        let declared: Vec<TypeId> = self.query.outline.functions[f.0 as usize].parameters.iter().map(|q| q.ty).collect();
        let mut map = HashMap::new();
        if let Some(c) = p.function(f).class
            && let Some(t) = self.query.bodies.units[unit_.0 as usize].get_type(call)
            && let Type::Interface { class, args, .. } = self.query.table.get(t).clone()
            && class == c
            && let Some(data) = self.query.outline.classes.get(c.0 as usize)
            && data.type_params.len() == args.len()
        {
            for (tp, a) in data.type_params.iter().zip(args.iter()) {
                map.insert(*tp, *a);
            }
        }
        let types: Vec<Option<TypeId>> = declared
            .into_iter()
            .map(|t| Some(dartforge_types::ops::substitute(t, &map, &mut self.query.table)))
            .chain(std::iter::repeat(None))
            .take(n)
            .collect();
        let p = self.program();
        let MemberKind::Constructor(k) = &p.unit(unit).ast.member(member).kind else {
            return Vec::new();
        };
        self.labels(unit, &k.parameters, &types)
    }

    /// Parâmetros só pelo tipo de função (sem nomes posicionais).
    fn type_parameters(&self, ty: TypeId) -> Vec<Parameter> {
        let Type::Function { positional, optional, named, .. } = self.query.table.get(ty).clone() else {
            return Vec::new();
        };
        let fmt = |t: TypeId| self.query.format(t);
        let mut output: Vec<Parameter> = positional
            .iter()
            .map(|t| Parameter { label: format!("{} ", fmt(*t)), ty: ParameterKind::Required, name: None })
            .chain(optional.iter().map(|t| Parameter { label: format!("{} ", fmt(*t)), ty: ParameterKind::Optional, name: None }))
            .collect();
        for (n, t, required) in named.iter() {
            let name = self.name(*n).to_string();
            let prefix = if *required { "required " } else { "" };
            output.push(Parameter { label: format!("{prefix}{} {name}", fmt(*t)), ty: ParameterKind::Named, name: Some(name) });
        }
        output
    }

    /// `required int c = 0` de cada parâmetro escrito em `u`.
    fn labels(&self, u: UnitId, ps: &[ast::Parameter], types: &[Option<TypeId>]) -> Vec<Parameter> {
        let unit = self.program().unit(u);
        let source = unit.source.as_str();
        ps.iter()
            .enumerate()
            .map(|(i, p)| {
                let ty = types
                    .get(i)
                    .copied()
                    .flatten()
                    .map_or_else(|| "dynamic".to_string(), |t| self.query.format(t));
                let name = p.public_name.or(p.name).map(|n| source[n.span.start..n.span.end].to_string());
                let pattern = p.default_value.map_or(String::new(), |e| {
                    let s = unit.ast.expr(e).span;
                    format!(" = {}", &source[s.start..s.end])
                });
                let prefix = if p.kind == ParameterKind::Named && p.required { "required " } else { "" };
                Parameter {
                    label: format!("{prefix}{ty} {}{pattern}", name.as_deref().unwrap_or("")),
                    ty: p.kind,
                    name,
                }
            })
            .collect()
    }

    /// O parâmetro ativo: o do argumento sob o cursor, ou o próximo
    /// posicional entre argumentos.
    fn active_parameter(&self, unit: UnitId, call: ast::ExprId, offset: usize, parameters: &[Parameter]) -> Option<usize> {
        let ast = &self.program().unit(unit).ast;
        let arguments_ = match &ast.expr(call).kind {
            ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => arguments,
            _ => return None,
        };
        let range = |a: &ast::Argument| {
            let v = ast.expr(a.value).span;
            (a.name.map_or(v.start, |n| n.span.start), v.end)
        };
        let positional_n = |k: usize| {
            parameters
                .iter()
                .enumerate()
                .filter(|(_, p)| p.ty != ParameterKind::Named)
                .nth(k)
                .map(|(i, _)| i)
        };
        let mut positional_before = 0;
        for a in arguments_.args.iter() {
            let (de, until) = range(a);
            if de <= offset && offset <= until {
                return match a.name {
                    Some(n) => {
                        let name = &self.program().unit(unit).source[n.span.start..n.span.end];
                        parameters.iter().position(|p| p.ty == ParameterKind::Named && p.name.as_deref() == Some(name))
                    }
                    None => positional_n(positional_before),
                };
            }
            if a.name.is_none() {
                positional_before += 1;
            }
        }
        let before = arguments_.args.iter().filter(|a| a.name.is_none() && range(a).1 < offset).count();
        positional_n(before)
    }
}

/// Os tipos de `ty` (uma função) alinhados aos parâmetros escritos:
/// posicionais pela ordem, nomeados pelo nome.
fn aligned_types(table: &dartforge_types::TypeTable, ps: &[ast::Parameter], ty: TypeId) -> Vec<Option<TypeId>> {
    let Type::Function { positional, optional, named, .. } = table.get(ty).clone() else {
        return vec![None; ps.len()];
    };
    let mut positional_ = positional.iter().chain(optional.iter());
    ps.iter()
        .map(|p| match p.kind {
            ParameterKind::Named => {
                let symbol = p.public_name.or(p.name)?.sym;
                named.iter().find(|(n, _, _)| *n == symbol).map(|(_, t, _)| *t)
            }
            _ => positional_.next().copied(),
        })
        .collect()
}

/// O nome qualificado como o analyzer mostra: sem argumentos de tipo, `?`
/// nem espaços (`p.A<int>.name` vira `p.A.name`).
fn clear_name(text: &str) -> String {
    let mut output = String::new();
    let mut level = 0usize;
    for c in text.chars() {
        match c {
            '<' => level += 1,
            '>' => level = level.saturating_sub(1),
            _ if level > 0 => {}
            c if c.is_whitespace() || c == '?' => {}
            c => output.push(c),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::clear_name;

    #[test]
    fn qualified_name_without_type_arguments() {
        assert_eq!(clear_name("p.A<int, List<String>>.nome"), "p.A.nome");
        assert_eq!(clear_name("Mapa<K,\n V>"), "Mapa");
    }
}
