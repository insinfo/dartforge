//! Signature help (`textDocument/signatureHelp`) in the Dart server's format
//! (`DartUnitSignatureComputer` + `toSignatureHelp`, see `docs/LSP.md`,
//! "Paridade com o servidor do Dart"):
//!
//! * the innermost argument list containing the cursor, of a method or
//!   function call (`f(…)`, `a.m(…)`) or of an instance creation (`A(…)`,
//!   `A.name(…)`, `new p.A<T>.name(…)`); a function expression between the
//!   cursor and the list stops the search (the cursor in the body of a
//!   *closure* passed as an argument does not show the outer call);
//! * the label is `name(int a, [int b = 0], {required String c})`, each
//!   parameter `ty name = default` with the type already substituted by the
//!   receiver (`List<int>.add` shows `int value`) and the default as
//!   written;
//! * the active parameter is that of the argument under the cursor (named
//!   by name, positional by position) or, between arguments, the next
//!   positional;
//! * the documentation is the hover's (including one inherited from an
//!   overridden member);
//! * automatic triggering by `(` only answers when the typed `(` is the one
//!   that opens the list.
//!
//! Everything comes from the common inference (`get_resolved`, `get_type`,
//! `MemberResolver::lookup_member`); nothing is resolved by name here.

use crate::project::{Concrete, Project};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{FunctionElementId, FunctionKind, FunctionRef, UnitId};
use dartforge_frontend::ast::{self, ExprKind, MemberKind, ParameterKind};
use dartforge_intern::SymbolId;
use dartforge_types::{Type, TypeId};
use std::collections::HashMap;

/// A signature ready for the protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// `name(int a, {int b = 0})`.
    pub label: String,
    /// The label of each parameter, in declaration order.
    pub parameters: Vec<String>,
    pub documentation: Option<String>,
    /// Index in `parameters` of the active parameter, when there is one.
    pub active: Option<usize>,
}

/// A parameter as the signature shows it.
struct Parameter {
    label: String,
    ty: ParameterKind,
    /// External name (the one used at the call).
    name: Option<String>,
}

impl Project {
    /// Signature help at `offset` of `unit`. `auto`: requested by the editor on
    /// typing `(` (with no help already open).
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

    /// The call (`Call` or `InstanceCreation`) whose argument list is the
    /// innermost one containing `offset` (after the `(` and up to the `)`), and
    /// the range of the list. `None` if a function expression inside the list
    /// contains the cursor.
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
        // A `FunctionExpression` between the cursor and the list: the analyzer stops.
        let slashed = ast
            .functions
            .iter()
            .any(|f| f.name.is_none() && s.start < f.span.start && f.span.end <= s.end && f.span.start <= offset && offset <= f.span.end);
        (!slashed).then_some((call, s))
    }

    /// Displayed name, parameters and documentation of what the call invokes.
    fn callee(&mut self, unit: UnitId, call: ast::ExprId) -> Option<(String, Vec<Parameter>, Option<String>)> {
        /// What the call is, copied from the tree before the mutable queries.
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
                    // `A<int>(…)` and `f<int>(…)`: the name is inside the explicit
                    // instantiation.
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
                        // A getter or field returning a function: the analyzer has no parameter
                        // list to show.
                        FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => None,
                        _ => {
                            let name_ = source[name_span.start..name_span.end].to_string();
                            let ty_ = self.member_type(unit, receiver, target_, f);
                            Some((name_, self.function_parameters(f, ty_), documentation))
                        }
                    },
                    Some(Concrete::Variable(_)) => None,
                    None => {
                        // Local function: the tree gives the names, the local's type gives the types.
                        let crate::project::Target::Local { unit: ul, declaration } = d.target else {
                            return None;
                        };
                        let name_ = source[name_span.start..name_span.end].to_string();
                        let ty_ = self.query.bodies.units[ul.0 as usize].local_type(declaration)?;
                        let ast_local = &self.program().unit(ul).ast;
                        let parameters = match ast_local.functions.iter().find(|f| f.name.is_some_and(|n| n.span.start == declaration)) {
                            Some(f) => {
                                let ps = f.parameters.as_deref().unwrap_or(&[]);
                                let types = aligned_types(&self.query.table, ps, ty_);
                                self.labels(ul, ps, &types)
                            }
                            // Local variable of function type: only the types.
                            None => self.type_parameters(ty_),
                        };
                        Some((name_, parameters, documentation))
                    }
                }
            }
        }
    }

    /// The hover documentation on the name at `name`.
    fn documentation_of(&self, unit: UnitId, name: Span) -> Option<String> {
        let d = self.identify(unit, name.start).ok()??;
        self.hover(unit, &d).and_then(|h| h.documentation)
    }

    /// The function type of the called member, already substituted by the
    /// receiver: through the common inference's member lookup when there is a
    /// receiver, otherwise the static type of the call target.
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

    /// Parameters of a function, method or operator; `ty` (substituted) wins
    /// over the declared types when it has the same shape.
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

    /// Parameters of a constructor with the class's type parameters replaced by
    /// the arguments of the created type (`List<int>.filled` shows `int fill`).
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

    /// Parameters from the function type only (no positional names).
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

    /// `required int c = 0` of each parameter written in `u`.
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

    /// The active parameter: that of the argument under the cursor, or the next
    /// positional between arguments.
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
            let (from, until) = range(a);
            if from <= offset && offset <= until {
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

/// The types of `ty` (a function) aligned with the written parameters:
/// positionals by order, named ones by name.
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

/// The qualified name as the analyzer shows it: without type arguments, `?`
/// or spaces (`p.A<int>.name` becomes `p.A.name`).
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
