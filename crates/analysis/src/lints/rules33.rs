//! The thirty-third batch of lint rules
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the 3.6.2
//! emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//!
//! * `use_build_context_synchronously`: outside the test directories, the use
//!   of a `BuildContext` (the target of a method invocation, the prefix of a
//!   `PrefixedIdentifier` that is not `.mounted`, the argument of an
//!   invocation or creation) after an async gap without the guard of the
//!   associated `mounted` (`State.mounted` for a `State`'s `context`,
//!   otherwise the type's `mounted`), through the whole `AsyncStateVisitor`
//!   over the tree in the analyzer's shape ([`crate::arvore_analyzer`], in the
//!   resolved form): from the node up to the function body, each parent gives
//!   the state relative to the child; an `await` already seen as a parent
//!   stays in the cache. In the body of a closure passed to a protected
//!   constructor or method (`Future`, `Stream`, `StreamController`,
//!   `StreamSubscription`), the use is reported too. Another element's
//!   `mounted` switches the code to `wrong_mounted`. `terminatesControl` uses
//!   the `ExitDetector` ([`crate::saida`]); `constantBoolValue`, the
//!   `true`/`false` literals, the parentheses, `!` and top-level constants
//!   with a boolean initializer.
//!
//! Written without compiling or running (2026-10-05).

use super::codigos_g as c;
use super::rules::LintReport;
use super::CodigoLint;
use crate::arvore_analyzer::{Arvore, Ligacao, Marca};
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionElementId, FunctionKind, FunctionRef, VariableId, VariableRef};
use dartforge_frontend::ast::{DeclKind, ExprId, ExprKind, MemberKind};
use dartforge_intern::Interner;
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::{Type, TypeId};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AsyncState {
    Async,
    Mounted,
    NotMounted,
}

/// `asynchronousOrNull`.
fn only_async(e: Option<AsyncState>) -> Option<AsyncState> {
    e.filter(|x| *x == AsyncState::Async)
}

const BODIES: &[&str] = &["BlockFunctionBody", "ExpressionFunctionBody", "EmptyFunctionBody", "NativeFunctionBody"];

/// The tree, the text and the semantics.
struct Cx<'a> {
    tree: &'a Arvore,
    source: &'a str,
    s: &'a super::Semantics<'a>,
    interner: &'a Interner,
}

impl<'a> Cx<'a> {
    fn kind(&self, n: usize) -> &'static str {
        self.tree.nos[n].especie
    }

    fn children(&self, n: usize) -> &'a [usize] {
        &self.tree.nos[n].filhos
    }

    fn parent(&self, n: usize) -> Option<usize> {
        self.tree.nos[n].pai
    }

    fn text(&self, n: usize) -> &'a str {
        let node = &self.tree.nos[n];
        &self.source[node.inicio..node.fim]
    }

    fn span(&self, n: usize) -> Span {
        Span { start: self.tree.nos[n].inicio, end: self.tree.nos[n].fim }
    }

    /// The operator between two children (`BinaryExpression`,
    /// `AssignmentExpression`).
    fn operator_between(&self, l: usize, r: usize) -> &'a str {
        self.source[self.tree.nos[l].fim..self.tree.nos[r].inicio].trim()
    }

    /// The DartForge expression that the node represents.
    fn expr(&self, n: usize) -> Option<ExprId> {
        match self.tree.nos[n].ligacao {
            Ligacao::Expr(e) => Some(e),
            _ => match self.tree.nos[n].marca {
                Marca::Expr(e) => Some(e),
                _ => None,
            },
        }
    }

    /// The node's `staticType`.
    fn static_type(&self, n: usize) -> Option<TypeId> {
        self.s.body.get_type(self.expr(n)?)
    }

    /// The resolved element of an identifier (its mark).
    fn resolved(&self, n: usize) -> Option<&'a Resolved> {
        match self.tree.nos[n].marca {
            Marca::Expr(e) => self.s.body.get_resolved(e),
            _ => None,
        }
    }

    /// The `methodName` of a `MethodInvocation`: the last `SimpleIdentifier`.
    fn method_name(&self, n: usize) -> Option<usize> {
        self.children(n).iter().rev().copied().find(|&f| self.kind(f) == "SimpleIdentifier")
    }

    /// The `target` of a `MethodInvocation` (without the cascade's).
    fn method_target(&self, n: usize) -> Option<usize> {
        let name = self.method_name(n);
        self.children(n).first().copied().filter(|&a| Some(a) != name && self.kind(a) != "ArgumentList" && self.kind(a) != "TypeArgumentList")
    }

    /// The arguments (children of the child `ArgumentList`).
    fn arguments(&self, n: usize) -> Vec<usize> {
        self.children(n).iter().copied().find(|&k| self.kind(k) == "ArgumentList").map(|l| self.children(l).to_vec()).unwrap_or_default()
    }

    /// `realTarget` of a cascade section without a written target.
    fn cascade_target(&self, n: usize) -> Option<usize> {
        let mut k = n;
        while let Some(p) = self.parent(k) {
            if self.kind(p) == "CascadeExpression" {
                return self.children(p).first().copied().filter(|&a| a != k);
            }
            if !matches!(self.kind(p), "MethodInvocation" | "PropertyAccess" | "IndexExpression" | "AssignmentExpression") {
                return None;
            }
            k = p;
        }
        None
    }

    /// `constantBoolValue`.
    fn constant_bool(&self, n: usize) -> Option<bool> {
        match self.kind(n) {
            "BooleanLiteral" => Some(self.text(n) == "true"),
            "ParenthesizedExpression" => self.children(n).first().and_then(|&f| self.constant_bool(f)),
            "PrefixExpression" if self.text(n).trim_start().starts_with('!') => self.children(n).first().and_then(|&f| self.constant_bool(f)).map(|b| !b),
            "SimpleIdentifier" | "PrefixedIdentifier" => {
                let target = if self.kind(n) == "PrefixedIdentifier" { *self.children(n).last()? } else { n };
                let v = match self.resolved(target)? {
                    Resolved::Element(Element::Variable(v)) | Resolved::Member { member: MemberRef::Variable(v), .. } => *v,
                    Resolved::Element(Element::Function(f)) | Resolved::Member { member: MemberRef::Function(f), .. } => self.s.program.function(*f).variable?,
                    _ => return None,
                };
                constant_bool_value(self.s, v, 0)
            }
            _ => None,
        }
    }

    /// `terminatesControl`.
    fn terminates(&self, n: usize) -> bool {
        match self.kind(n) {
            "Block" => self.children(n).last().is_some_and(|&u| self.terminates(u)),
            "ReturnStatement" | "BreakStatement" | "ContinueStatement" => true,
            _ => {
                let element = |m: usize| element_exits(self, m);
                crate::saida::Saida::novo(self.tree, self.source, &element).sai(n).unwrap_or(false)
            }
        }
    }
}

/// The value of a top-level or static boolean constant by its literal
/// initializer.
fn constant_bool_value(s: &super::Semantics<'_>, v: VariableId, depth: u32) -> Option<bool> {
    if depth > 8 {
        return None;
    }
    let x = s.program.variable(v);
    if !x.const_ {
        return None;
    }
    let (unit, init) = match x.node {
        VariableRef::TopLevel { unit, decl, index } => match &s.program.unit(unit).ast.decl(decl).kind {
            DeclKind::Variables(l) => (unit, l.variables.get(index)?.initializer?),
            _ => return None,
        },
        VariableRef::Field { unit, member, index } => match &s.program.unit(unit).ast.member(member).kind {
            MemberKind::Field(l) => (unit, l.variables.get(index)?.initializer?),
            _ => return None,
        },
        _ => return None,
    };
    let a = &s.program.unit(unit).ast;
    let mut e = init;
    while let ExprKind::Parenthesized(i) = &a.expr(e).kind {
        e = *i;
    }
    match &a.expr(e).kind {
        ExprKind::Bool(b) => Some(*b),
        _ => None,
    }
}

/// `_elementExits`: the executable returns `Never` or has `@alwaysThrows`.
fn element_exits(cx: &Cx<'_>, m: usize) -> bool {
    let s = cx.s;
    match cx.resolved(m) {
        Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::Element(Element::Function(f))) | Some(Resolved::ExtensionMember { member: f, .. }) => {
            let never = s.outline.functions.get(f.0 as usize).is_some_and(|d| matches!(s.table.get(d.return_type), Type::Never));
            never || function_annotations(s, *f).is_some_and(|(meta, u)| meta.iter().any(|a| dartforge_types::anotacoes::e_getter_de(s.program, cx.interner, u, a, "meta", "alwaysThrows")))
        }
        Some(Resolved::Local(_)) => {
            let Marca::Expr(e) = cx.tree.nos[m].marca else { return false };
            let Some(d) = s.body.declaracao_local(e) else { return false };
            matches!(s.body.tipo_local(d).map(|t| s.table.get(t)), Some(Type::Function { ret, .. }) if matches!(s.table.get(*ret), Type::Never))
        }
        _ => false,
    }
}

/// The annotations of the declaration of a function, method or constructor.
fn function_annotations<'p>(s: &super::Semantics<'p>, f: FunctionElementId) -> Option<(&'p [dartforge_frontend::ast::Annotation], dartforge_elements::model::UnitId)> {
    let program: &'p dartforge_elements::model::Program = s.program;
    match program.function(f).node {
        FunctionRef::Constructor { unit, member } => Some((&program.unit(unit).ast.member(member).metadata[..], unit)),
        FunctionRef::Function { unit, function } => {
            let a = &program.unit(unit).ast;
            if let Some(m) = a.members.iter().find(|m| matches!(&m.kind, MemberKind::Method(g) if *g == function)) {
                return Some((&m.metadata[..], unit));
            }
            a.decls.iter().find(|d| matches!(&d.kind, DeclKind::Function(g) if *g == function)).map(|d| (&d.metadata[..], unit))
        }
        FunctionRef::None => None,
    }
}

/// The `AsyncStateVisitor`.
struct AsyncStateVisitor<'c, 'a> {
    cx: &'c Cx<'a>,
    reference: usize,
    mounted: FunctionElementId,
    cache: HashMap<usize, Option<AsyncState>>,
    unrelated: bool,
}

impl AsyncStateVisitor<'_, '_> {
    /// `_asynchronousIfAnyIsAsync`.
    fn any_async(&mut self, nodes: &[usize]) -> Option<AsyncState> {
        let until = nodes.iter().position(|&n| n == self.reference).unwrap_or(nodes.len());
        for &n in &nodes[..until] {
            if self.visit(n) == Some(AsyncState::Async) {
                return Some(AsyncState::Async);
            }
        }
        None
    }

    /// `_inOrderAsyncState`.
    fn in_order(&mut self, nodes: &[(Option<usize>, bool)]) -> Option<AsyncState> {
        if nodes.is_empty() {
            return None;
        }
        if nodes[0].0 == Some(self.reference) {
            return None;
        }
        let index = nodes.iter().position(|(n, _)| *n == Some(self.reference));
        let start = match index {
            Some(i) if i > 0 => i - 1,
            _ => nodes.len() - 1,
        };
        for i in (0..=start).rev() {
            let (n, guard) = nodes[i];
            let Some(n) = n else { continue };
            let state = self.visit(n);
            if state == Some(AsyncState::Async) {
                return Some(AsyncState::Async);
            }
            if guard && state.is_some() {
                return state;
            }
        }
        None
    }

    fn in_order_guardable(&mut self, nodes: &[usize]) -> Option<AsyncState> {
        let v: Vec<(Option<usize>, bool)> = nodes.iter().map(|&n| (Some(n), true)).collect();
        self.in_order(&v)
    }

    /// `_visitBlockLike`.
    fn as_block(&mut self, stmts: &[usize], pai: Option<usize>) -> Option<AsyncState> {
        let r = self.reference;
        if crate::arvore_analyzer::e_comando(self.cx.kind(r))
            && let Some(i) = stmts.iter().position(|&c| c == r)
        {
            let previous = self.in_order_guardable(stmts);
            if previous.is_some() {
                return previous;
            }
            if pai.is_some_and(|p| matches!(self.cx.kind(p), "DoStatement" | "ForStatement" | "WhileStatement")) {
                return only_async(self.in_order_guardable(&stmts[i + 1..]));
            }
            return None;
        }
        for &c in stmts.iter().rev() {
            let e = self.visit(c);
            if e.is_some() {
                return e;
            }
        }
        None
    }

    /// `_visitIdentifier`.
    fn identifier(&mut self, n: usize) -> Option<AsyncState> {
        if self.cx.text(n) != "mounted" {
            return None;
        }
        let f = match self.cx.resolved(n) {
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::Element(Element::Function(f))) | Some(Resolved::ExtensionMember { member: f, .. }) => {
                Some(self.cx.s.program.public(*f))
            }
            _ => None,
        };
        if f == Some(self.cx.s.program.public(self.mounted)) {
            return Some(AsyncState::Mounted);
        }
        self.unrelated = true;
        None
    }

    /// `_constantEquality`.
    fn constant_equality(state: Option<AsyncState>, constant: bool) -> Option<AsyncState> {
        match (state, constant) {
            (Some(AsyncState::Mounted), true) | (Some(AsyncState::NotMounted), false) => Some(AsyncState::Mounted),
            (Some(AsyncState::NotMounted), true) | (Some(AsyncState::Mounted), false) => Some(AsyncState::NotMounted),
            _ => None,
        }
    }

    /// `_visitIfLike`.
    fn as_if(&mut self, expr: usize, case: Option<usize>, then_branch: usize, else_branch: Option<usize>) -> Option<AsyncState> {
        if self.reference == expr {
            return None;
        }
        let e = self.visit(expr);
        if Some(self.reference) == case {
            return match e {
                Some(AsyncState::Async) => Some(AsyncState::Async),
                Some(AsyncState::Mounted) => Some(AsyncState::Mounted),
                _ => None,
            };
        }
        let c = case.and_then(|k| self.visit(k));
        let condition = match (e, c) {
            (None, _) => c,
            (_, None) => e,
            (_, Some(AsyncState::Async)) => Some(AsyncState::Async),
            (Some(AsyncState::Async), _) => c,
            (Some(AsyncState::Mounted), _) => Some(AsyncState::Mounted),
            (Some(AsyncState::NotMounted), _) => Some(AsyncState::NotMounted),
        };
        if self.reference == then_branch {
            return match condition {
                Some(AsyncState::Async) => Some(AsyncState::Async),
                Some(AsyncState::Mounted) => Some(AsyncState::Mounted),
                _ => None,
            };
        }
        if Some(self.reference) == else_branch {
            return match condition {
                Some(AsyncState::Async) => Some(AsyncState::Async),
                Some(AsyncState::NotMounted) => Some(AsyncState::Mounted),
                _ => None,
            };
        }
        let then_state = self.visit(then_branch);
        let else_state = else_branch.and_then(|k| self.visit(k));
        let then_terminates = self.cx.terminates(then_branch);
        let else_terminates = else_branch.is_some_and(|k| self.cx.terminates(k));
        if then_state == Some(AsyncState::NotMounted) && (else_state == Some(AsyncState::NotMounted) || else_terminates) {
            return Some(AsyncState::NotMounted);
        }
        if else_state == Some(AsyncState::NotMounted) && then_terminates {
            return Some(AsyncState::NotMounted);
        }
        if then_state == Some(AsyncState::Async) && !then_terminates {
            return Some(AsyncState::Async);
        }
        if else_state == Some(AsyncState::Async) && !else_terminates {
            return Some(AsyncState::Async);
        }
        if condition == Some(AsyncState::Async) {
            return Some(AsyncState::Async);
        }
        if condition == Some(AsyncState::Mounted) && else_terminates {
            return Some(AsyncState::NotMounted);
        }
        if condition == Some(AsyncState::NotMounted) && then_terminates {
            return Some(AsyncState::NotMounted);
        }
        None
    }

    /// The parts of a `for`, statement or element.
    fn for_parts(&mut self, n: usize) -> Option<AsyncState> {
        let cx = self.cx;
        let f = cx.children(n);
        let (Some(&parts), body) = (f.first(), f.get(1).copied()) else { return None };
        let body_and_reference = body == Some(self.reference);
        match cx.kind(parts) {
            "ForPartsWithDeclarations" | "ForPartsWithExpression" => {
                let p = cx.tree.partes_de_for.get(&parts).cloned().unwrap_or_default();
                let mut nodes: Vec<(Option<usize>, bool)> = Vec::new();
                if cx.kind(parts) == "ForPartsWithDeclarations" {
                    if let Some(list) = p.inicio {
                        for &v in cx.children(list).iter().filter(|&&v| cx.kind(v) == "VariableDeclaration") {
                            nodes.push((Some(v), false));
                        }
                    }
                } else {
                    nodes.push((p.inicio, false));
                }
                nodes.push((p.condicao, body_and_reference));
                for &u in &p.atualizacoes {
                    nodes.push((Some(u), false));
                }
                nodes.push((body, false));
                self.in_order(&nodes)
            }
            "ForEachPartsWithDeclaration" | "ForEachPartsWithIdentifier" | "ForEachPartsWithPattern" => {
                let iterable = cx.children(parts).last().copied();
                self.in_order(&[(iterable, false), (body, false)])
            }
            _ => None,
        }
    }

    /// `node.accept(AsyncStateVisitor)`.
    fn visit(&mut self, n: usize) -> Option<AsyncState> {
        let cx = self.cx;
        let f = cx.children(n);
        let r = self.reference;
        let first = f.first().copied();
        match cx.kind(n) {
            "AdjacentStrings" | "CascadeExpression" | "RecordLiteral" | "StringInterpolation" => self.any_async(f),
            "AsExpression" | "IsExpression" | "SpreadElement" | "InterpolationExpression" | "PostfixExpression" | "YieldStatement" => {
                let e = first.and_then(|k| self.visit(k));
                only_async(e)
            }
            "AssignmentExpression" => {
                let (Some(&l), Some(&d)) = (f.first(), f.get(1)) else { return None };
                self.in_order(&[(Some(l), false), (Some(d), true)])
            }
            "AwaitExpression" => {
                if let Some(e) = self.cache.get(&n) {
                    return *e;
                }
                if first == Some(r) { None } else { Some(AsyncState::Async) }
            }
            "BinaryExpression" => {
                let (Some(&l), Some(&d)) = (f.first(), f.get(1)) else { return None };
                let op = cx.operator_between(l, d);
                if l == r {
                    return None;
                }
                if d == r {
                    let e = self.visit(l);
                    return match e {
                        Some(AsyncState::Async) => Some(AsyncState::Async),
                        Some(AsyncState::Mounted) if op == "&&" => Some(AsyncState::Mounted),
                        Some(AsyncState::NotMounted) if op == "||" => Some(AsyncState::NotMounted),
                        _ => None,
                    };
                }
                match op {
                    "&&" => {
                        let a = self.visit(l);
                        let b = self.visit(d);
                        match (a, b) {
                            (None, _) => b,
                            (_, None) => a,
                            (_, Some(AsyncState::Async)) => Some(AsyncState::Async),
                            (Some(AsyncState::Async), _) => b,
                            (Some(AsyncState::Mounted), _) => Some(AsyncState::Mounted),
                            (Some(AsyncState::NotMounted), _) => Some(AsyncState::NotMounted),
                        }
                    }
                    "||" => {
                        let a = self.visit(l);
                        let b = self.visit(d);
                        match (a, b) {
                            (_, Some(AsyncState::Async)) => Some(AsyncState::Async),
                            (_, Some(AsyncState::NotMounted)) => Some(AsyncState::NotMounted),
                            (Some(AsyncState::Async), _) => Some(AsyncState::Async),
                            (Some(AsyncState::Mounted), Some(AsyncState::Mounted)) => Some(AsyncState::Mounted),
                            (Some(AsyncState::NotMounted), _) => Some(AsyncState::NotMounted),
                            _ => None,
                        }
                    }
                    "==" | "!=" => {
                        let negated = op == "!=";
                        let a = self.visit(l);
                        let b = self.visit(d);
                        if a == Some(AsyncState::Async) || b == Some(AsyncState::Async) {
                            return Some(AsyncState::Async);
                        }
                        if matches!(a, Some(AsyncState::Mounted | AsyncState::NotMounted)) {
                            let v = cx.constant_bool(d)?;
                            return Self::constant_equality(a, if negated { !v } else { v });
                        }
                        if matches!(b, Some(AsyncState::Mounted | AsyncState::NotMounted)) {
                            let v = cx.constant_bool(l)?;
                            return Self::constant_equality(b, if negated { !v } else { v });
                        }
                        None
                    }
                    _ => {
                        let a = only_async(self.visit(l));
                        if a.is_some() {
                            return a;
                        }
                        only_async(self.visit(d))
                    }
                }
            }
            "Block" => {
                let pai = cx.parent(n);
                self.as_block(f, pai)
            }
            "BlockFunctionBody" | "ExpressionFunctionBody" => None,
            "CaseClause" => f.iter().copied().find(|&k| cx.kind(k) == "GuardedPattern").and_then(|k| self.visit(k)),
            "CatchClause" => {
                let e = f.last().copied().and_then(|k| self.visit(k));
                only_async(e)
            }
            "ConditionalExpression" => {
                let (Some(&c), Some(&t), Some(&e)) = (f.first(), f.get(1), f.get(2)) else { return None };
                self.as_if(c, None, t, Some(e))
            }
            "DoStatement" => {
                let (Some(&body), Some(&cond)) = (f.first(), f.get(1)) else { return None };
                if body == r {
                    only_async(self.visit(cond))
                } else if cond == r {
                    only_async(self.visit(body))
                } else {
                    let a = only_async(self.visit(cond));
                    if a.is_some() {
                        return a;
                    }
                    only_async(self.visit(body))
                }
            }
            "ExpressionStatement" => {
                let e = first?;
                if e == r { None } else { only_async(self.visit(e)) }
            }
            "ExtensionOverride" => {
                let args = cx.arguments(n);
                self.any_async(&args)
            }
            "ForElement" | "ForStatement" => self.for_parts(n),
            "FunctionExpressionInvocation" => {
                let mut nodes: Vec<usize> = first.into_iter().filter(|&k| cx.kind(k) != "ArgumentList").collect();
                nodes.extend(cx.arguments(n));
                self.any_async(&nodes)
            }
            "GuardedPattern" => f.iter().copied().find(|&k| cx.kind(k) == "WhenClause").and_then(|k| self.visit(k)),
            "IfElement" | "IfStatement" => {
                let mut it = f.iter().copied();
                let expr = it.next()?;
                let mut rest: Vec<usize> = it.collect();
                let case = if rest.first().is_some_and(|&k| cx.kind(k) == "CaseClause") { Some(rest.remove(0)) } else { None };
                let then_branch = *rest.first()?;
                let else_branch = rest.get(1).copied();
                self.as_if(expr, case, then_branch, else_branch)
            }
            "IndexExpression" => {
                let nodes: Vec<usize> = f.to_vec();
                self.any_async(&nodes)
            }
            "InstanceCreationExpression" => {
                let args = cx.arguments(n);
                self.any_async(&args)
            }
            "LabeledStatement" => f.last().copied().and_then(|k| self.visit(k)),
            "ListLiteral" | "SetOrMapLiteral" => {
                let nodes: Vec<usize> = f.iter().copied().filter(|&k| cx.kind(k) != "TypeArgumentList").collect();
                self.any_async(&nodes)
            }
            "MapLiteralEntry" => self.any_async(f),
            "MethodInvocation" => {
                let mut nodes: Vec<usize> = cx.method_target(n).into_iter().collect();
                nodes.extend(cx.arguments(n));
                self.any_async(&nodes)
            }
            "NamedExpression" => {
                let e = f.get(1).copied().and_then(|k| self.visit(k));
                only_async(e)
            }
            "ParenthesizedExpression" => first.and_then(|k| self.visit(k)),
            "PrefixedIdentifier" => {
                let id = *f.last()?;
                self.identifier(id)
            }
            "PrefixExpression" => {
                if cx.text(n).trim_start().starts_with('!') {
                    match first.and_then(|k| self.visit(k)) {
                        Some(AsyncState::Mounted) => Some(AsyncState::NotMounted),
                        Some(AsyncState::NotMounted) => Some(AsyncState::Mounted),
                        e => e,
                    }
                } else {
                    None
                }
            }
            "PropertyAccess" => {
                let name = *f.last()?;
                let target = if f.len() >= 2 { Some(f[0]) } else { None };
                let a = only_async(target.and_then(|k| self.visit(k)));
                if cx.text(name) == "mounted" {
                    if a.is_some() {
                        return a;
                    }
                    return self.identifier(name);
                }
                a
            }
            "SimpleIdentifier" => self.identifier(n),
            "SwitchCase" => {
                let nodes: Vec<usize> = f.iter().copied().filter(|&k| cx.kind(k) != "Label").collect();
                self.in_order_guardable(&nodes)
            }
            "SwitchDefault" => {
                let nodes: Vec<usize> = f.iter().copied().filter(|&k| cx.kind(k) != "Label").collect();
                self.in_order_guardable(&nodes)
            }
            "SwitchExpression" => self.any_async(f),
            "SwitchExpressionCase" => {
                let (Some(&gp), Some(&expr)) = (f.first(), f.get(1)) else { return None };
                if gp == r {
                    return None;
                }
                let when = cx.children(gp).iter().copied().find(|&k| cx.kind(k) == "WhenClause").and_then(|k| self.visit(k));
                if expr == r {
                    if matches!(when, Some(AsyncState::Async | AsyncState::Mounted)) {
                        return when;
                    }
                    return None;
                }
                let a = only_async(when);
                if a.is_some() {
                    return a;
                }
                only_async(self.visit(expr))
            }
            "SwitchPatternCase" => {
                let gp = f.iter().copied().find(|&k| cx.kind(k) == "GuardedPattern")?;
                if gp == r {
                    return None;
                }
                let stmts: Vec<usize> = f.iter().copied().filter(|&k| crate::arvore_analyzer::e_comando(cx.kind(k))).collect();
                let pai = cx.parent(n);
                let e = self.as_block(&stmts, pai);
                if e.is_some() {
                    return e;
                }
                if stmts.contains(&r) {
                    return None;
                }
                let when = cx.children(gp).iter().copied().find(|&k| cx.kind(k) == "WhenClause").and_then(|k| self.visit(k));
                only_async(when)
            }
            "SwitchStatement" => {
                let expr = first?;
                let members: Vec<usize> = f.iter().copied().skip(1).filter(|&k| matches!(cx.kind(k), "SwitchCase" | "SwitchDefault" | "SwitchPatternCase")).collect();
                // The emitter's first computation is discarded, but it visits
                // (and may mark the unrelated `mounted`).
                if only_async(self.visit(expr)).is_none() {
                    let _ = self.any_async(&members);
                }
                if let Some(index) = members.iter().position(|&m| m == r) {
                    let mut fall = true;
                    let mut all_mounted = true;
                    for i in (0..=index).rev() {
                        let case = members[i];
                        if cx.kind(case) != "SwitchPatternCase" {
                            continue;
                        }
                        let gp = cx.children(case).iter().copied().find(|&k| cx.kind(k) == "GuardedPattern");
                        let when = gp.and_then(|g| cx.children(g).iter().copied().find(|&k| cx.kind(k) == "WhenClause")).and_then(|k| self.visit(k));
                        if when == Some(AsyncState::Async) {
                            return Some(AsyncState::Async);
                        }
                        if fall {
                            let empty = !cx.children(case).iter().any(|&k| crate::arvore_analyzer::e_comando(cx.kind(k)));
                            let falls = i == index || empty;
                            if falls {
                                all_mounted &= when == Some(AsyncState::Mounted);
                            } else if all_mounted {
                                return Some(AsyncState::Mounted);
                            }
                            fall &= falls;
                        }
                    }
                    if fall && all_mounted {
                        return Some(AsyncState::Mounted);
                    }
                    return None;
                }
                let a = only_async(self.visit(expr));
                if a.is_some() {
                    return a;
                }
                self.any_async(&members)
            }
            "TryStatement" => {
                let body = first?;
                let captures: Vec<usize> = f.iter().copied().filter(|&k| cx.kind(k) == "CatchClause").collect();
                let finally_block = f.last().copied().filter(|&k| f.len() > 1 && cx.kind(k) == "Block");
                if body == r {
                    return None;
                }
                let mut body_and_captures = vec![body];
                body_and_captures.extend(captures.iter().copied());
                if captures.contains(&r) {
                    return only_async(self.visit(body));
                }
                if finally_block == Some(r) {
                    return self.any_async(&body_and_captures);
                }
                if let Some(fin) = finally_block {
                    let e = self.visit(fin);
                    if e.is_some() {
                        return e;
                    }
                }
                self.any_async(&body_and_captures)
            }
            "VariableDeclaration" => {
                let init = f.iter().copied().find(|&k| crate::arvore_analyzer::e_expressao(cx.kind(k)));
                only_async(init.and_then(|k| self.visit(k)))
            }
            "VariableDeclarationStatement" => {
                let list = f.iter().copied().find(|&k| cx.kind(k) == "VariableDeclarationList")?;
                let inits: Vec<usize> = cx
                    .children(list)
                    .iter()
                    .copied()
                    .filter(|&v| cx.kind(v) == "VariableDeclaration")
                    .filter_map(|v| cx.children(v).iter().copied().find(|&k| crate::arvore_analyzer::e_expressao(cx.kind(k))))
                    .collect();
                self.any_async(&inits)
            }
            "WhenClause" => first.and_then(|k| self.visit(k)),
            "WhileStatement" => {
                let (Some(&cond), Some(&body)) = (f.first(), f.get(1)) else { return None };
                let a = only_async(self.visit(cond));
                if a.is_some() {
                    return a;
                }
                only_async(self.visit(body))
            }
            _ => None,
        }
    }
}

/// A function with protected callback parameters.
struct Protected {
    type_name: &'static str,
    name: Option<&'static str>,
    positional: &'static [usize],
    named: &'static [&'static str],
}

const fn p(type_name: &'static str, name: Option<&'static str>, positional: &'static [usize], named: &'static [&'static str]) -> Protected {
    Protected { type_name, name, positional, named }
}

const CONSTRUCTORS: &[Protected] = &[
    p("Future", None, &[0], &[]),
    p("Future", Some("new"), &[0], &[]),
    p("Future", Some("delayed"), &[1], &[]),
    p("Future", Some("microtask"), &[0], &[]),
    p("Stream", Some("eventTransformed"), &[1], &[]),
    p("Stream", Some("multi"), &[0], &[]),
    p("Stream", Some("periodic"), &[1], &[]),
    p("StreamController", None, &[], &["onListen", "onPause", "onResume", "onCancel"]),
    p("StreamController", Some("new"), &[], &["onListen", "onPause", "onResume", "onCancel"]),
    p("StreamController", Some("broadcast"), &[], &["onListen", "onCancel"]),
];

const INSTANCE_METHODS: &[Protected] = &[
    p("Future", Some("catchError"), &[0], &["test"]),
    p("Future", Some("onError"), &[0], &["test"]),
    p("Future", Some("then"), &[0], &["onError"]),
    p("Future", Some("timeout"), &[], &["onTimeout"]),
    p("Future", Some("whenComplete"), &[0], &[]),
    p("Stream", Some("any"), &[0], &[]),
    p("Stream", Some("asBroadcastStream"), &[], &["onListen", "onCancel"]),
    p("Stream", Some("asyncExpand"), &[0], &[]),
    p("Stream", Some("asyncMap"), &[0], &[]),
    p("Stream", Some("distinct"), &[0], &[]),
    p("Stream", Some("expand"), &[0], &[]),
    p("Stream", Some("firstWhere"), &[0], &["orElse"]),
    p("Stream", Some("fold"), &[1], &[]),
    p("Stream", Some("forEach"), &[0], &[]),
    p("Stream", Some("handleError"), &[0], &["test"]),
    p("Stream", Some("lastWhere"), &[0], &["orElse"]),
    p("Stream", Some("listen"), &[0], &["onError", "onDone"]),
    p("Stream", Some("map"), &[0], &[]),
    p("Stream", Some("reduce"), &[0], &[]),
    p("Stream", Some("singleWhere"), &[0], &["orElse"]),
    p("Stream", Some("skipWhile"), &[0], &[]),
    p("Stream", Some("takeWhile"), &[0], &[]),
    p("Stream", Some("timeout"), &[], &["onTimeout"]),
    p("Stream", Some("where"), &[0], &[]),
    p("StreamSubscription", Some("onData"), &[0], &[]),
    p("StreamSubscription", Some("onDone"), &[0], &[]),
    p("StreamSubscription", Some("onError"), &[0], &[]),
];

const STATIC_METHODS: &[Protected] = &[
    p("Future", Some("doWhile"), &[0], &[]),
    p("Future", Some("forEach"), &[1], &[]),
    p("Future", Some("wait"), &[], &["cleanUp"]),
];

/// The `BuildContext` element (the declaration) of an expression
/// (`buildContextTypedElement`): the getter, the variable or the parameter,
/// and its type.
#[derive(Clone, Copy)]
enum ContextElement {
    /// An accessor (of a field or written) of a class.
    Accessor { class_id: Option<ClassId>, ty: TypeId },
    /// Variable, parameter or function.
    Other { ty: TypeId },
}

fn context_element(cx: &Cx<'_>, interner: &Interner, mut n: usize) -> Option<ContextElement> {
    let s = cx.s;
    if cx.kind(n) == "NamedExpression" {
        n = *cx.children(n).get(1)?;
    }
    if cx.kind(n) == "PropertyAccess" {
        n = *cx.children(n).last()?;
    }
    match cx.kind(n) {
        "SimpleIdentifier" | "PrefixedIdentifier" => {
            let id = if cx.kind(n) == "PrefixedIdentifier" { *cx.children(n).last()? } else { n };
            let Marca::Expr(e) = cx.tree.nos[id].marca else { return None };
            let r = s.body.get_resolved(e)?;
            let function = match r {
                Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::Element(Element::Function(f)) | Resolved::ExtensionMember { member: f, .. } => Some(*f),
                _ => None,
            };
            let (ty, accessor, class_id) = match r {
                _ if function.is_some_and(|f| s.program.function(f).kind == FunctionKind::Getter) => {
                    let f = function?;
                    let class_id = match r {
                        Resolved::Member { class, .. } => Some(*class),
                        _ => None,
                    };
                    (s.outline.functions.get(f.0 as usize)?.return_type, true, s.program.function(f).class.or(class_id))
                }
                _ if function.is_some() => (s.outline.functions.get(function?.0 as usize)?.return_type, false, None),
                Resolved::Member { member: MemberRef::Variable(v), .. } | Resolved::Element(Element::Variable(v)) => {
                    let i = v.0 as usize;
                    let d = s.outline.variables.get(i)?;
                    // The identifier read resolves to the synthetic getter.
                    (d.declared_type.or(d.inferred)?, true, s.program.variable(*v).class)
                }
                Resolved::Local(_) => (s.body.declaracao_local(e).and_then(|d| s.body.tipo_local(d)).or_else(|| s.body.get_type(e))?, false, None),
                Resolved::Parameter { .. } => (s.body.get_type(e)?, false, None),
                _ => return None,
            };
            if !super::flutter::is_build_context(s, interner, ty, accessor) {
                return None;
            }
            Some(if accessor { ContextElement::Accessor { class_id, ty } } else { ContextElement::Other { ty } })
        }
        "ParenthesizedExpression" => context_element(cx, interner, *cx.children(n).first()?),
        "PostfixExpression" if cx.text(n).trim_end().ends_with('!') => context_element(cx, interner, *cx.children(n).first()?),
        _ => None,
    }
}

/// `lookUpGetter('mounted')`: in the class, in the mixins and along the
/// superclass chain.
fn mounted_getter(s: &super::Semantics<'_>, interner: &Interner, c: ClassId) -> Option<FunctionElementId> {
    let name = interner.lookup("mounted")?;
    let mut seen: Vec<ClassId> = Vec::new();
    let mut current = Some(c);
    while let Some(k) = current {
        if seen.contains(&k) {
            return None;
        }
        seen.push(k);
        let x = s.program.class(k);
        if let Some(f) = x.instance_members.get(&name)
            && s.program.function(*f).kind == FunctionKind::Getter
        {
            return Some(*f);
        }
        for m in x.mixin_classes.iter().rev() {
            if let Some(f) = s.program.class(*m).instance_members.get(&name)
                && s.program.function(*f).kind == FunctionKind::Getter
            {
                return Some(*f);
            }
        }
        current = x.supertype_class;
    }
    None
}

/// `associatedMountedGetter`.
fn associated_getter(s: &super::Semantics<'_>, interner: &Interner, c: ContextElement) -> Option<FunctionElementId> {
    if let ContextElement::Accessor { class_id: Some(k), .. } = c
        && super::flutter::is_state(s, interner, k)
    {
        return mounted_getter(s, interner, k);
    }
    let ty = match c {
        ContextElement::Accessor { ty, .. } | ContextElement::Other { ty } => ty,
    };
    match s.table.get(ty) {
        Type::Interface { class, .. } => mounted_getter(s, interner, *class),
        _ => None,
    }
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unidade<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let Some(s) = sem else { return out };
    if !enabled("use_build_context_synchronously") {
        return out;
    }
    let program = s.program;
    // `inTestDir`, through the unit that defines the library.
    let lib = program.library(program.unit(s.unit).library);
    if let Some(path) = lib.units.first().and_then(|x| program.unit(*x).path.as_ref()) {
        let text = path.to_string_lossy();
        let sep = std::path::MAIN_SEPARATOR;
        if ["test", "integration_test", "test_driver", "testing"].iter().any(|d| text.contains(&format!("{sep}{d}{sep}"))) {
            return out;
        }
    }
    let a = u.ast;
    let body = s.body;
    // The resolved tree: the creations without `new` and the prefixes.
    let mut constructors: HashSet<ExprId> = HashSet::new();
    let mut prefixes: HashSet<ExprId> = HashSet::new();
    for (i, e) in a.exprs.iter().enumerate() {
        let id = ExprId(i as u32);
        match &e.kind {
            ExprKind::Call { .. } if matches!(body.get_resolved(id), Some(Resolved::Constructor(_))) => {
                constructors.insert(id);
            }
            ExprKind::InstanceCreation { keyword: None, constructor: None, .. } if !matches!(body.get_resolved(id), Some(r) if !matches!(r, Resolved::Constructor(_))) => {
                constructors.insert(id);
            }
            ExprKind::Identifier(_) if matches!(body.get_resolved(id), Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..)))) => {
                prefixes.insert(id);
            }
            _ => {}
        }
    }
    let before_3_0 = lib.features.version().major < 3;
    let tree = crate::arvore_analyzer::construir_resolvida(u.fonte, a, u.unit, before_3_0, &constructors, &prefixes);
    let cx = Cx { tree: &tree, source: u.fonte, s, interner };
    let mut findings: Vec<(Span, &'static CodigoLint)> = Vec::new();

    // `check`.
    let check = |node: usize, mounted: FunctionElementId, findings: &mut Vec<(Span, &'static CodigoLint)>| {
        let mut v = AsyncStateVisitor { cx: &cx, reference: node, mounted, cache: HashMap::new(), unrelated: false };
        let mut child = node;
        loop {
            if BODIES.contains(&cx.kind(child)) {
                break;
            }
            let Some(pai) = cx.parent(child) else { break };
            v.reference = child;
            let state = v.visit(pai);
            v.cache.insert(pai, state);
            if matches!(state, Some(AsyncState::Mounted | AsyncState::NotMounted)) {
                return;
            }
            if state == Some(AsyncState::Async) {
                let code = if v.unrelated { &c::USE_BUILD_CONTEXT_SYNCHRONOUSLY_WRONG_MOUNTED } else { &c::USE_BUILD_CONTEXT_SYNCHRONOUSLY_ASYNC_USE };
                findings.push((cx.span(node), code));
                return;
            }
            child = pai;
        }
        if !BODIES.contains(&cx.kind(child)) {
            return;
        }
        // The body of a closure passed to a protected function.
        let Some(function) = cx.parent(child) else { return };
        if cx.kind(function) != "FunctionExpression" {
            return;
        }
        let Some(mut grandparent) = cx.parent(function) else { return };
        if cx.kind(grandparent) == "NamedExpression" {
            let Some(x) = cx.parent(grandparent) else { return };
            grandparent = x;
        }
        if cx.kind(grandparent) != "ArgumentList" {
            return;
        }
        let Some(invocation) = cx.parent(grandparent) else { return };
        let argumentos = cx.children(grandparent);
        let positional: Vec<usize> = argumentos.iter().copied().filter(|&k| cx.kind(k) != "NamedExpression").collect();
        let named: Vec<usize> = argumentos.iter().copied().filter(|&k| cx.kind(k) == "NamedExpression").collect();
        let protected = |p: &Protected, findings: &mut Vec<(Span, &'static CodigoLint)>| {
            for &i in p.positional {
                if positional.get(i) == Some(&function) {
                    findings.push((cx.span(node), &c::USE_BUILD_CONTEXT_SYNCHRONOUSLY_ASYNC_USE));
                }
            }
            for name in p.named {
                let arg = named.iter().copied().find(|&k| cx.children(k).first().is_some_and(|&l| cx.text(l).trim_end_matches(':').trim() == *name));
                if arg.is_some_and(|k| cx.children(k).get(1) == Some(&function)) {
                    findings.push((cx.span(node), &c::USE_BUILD_CONTEXT_SYNCHRONOUSLY_ASYNC_USE));
                }
            }
        };
        let is_async_class = |t: TypeId, name: &str| -> bool {
            matches!(s.table.get(t), Type::Interface { class, .. }
                if interner.resolve(program.class(*class).name) == name
                    && program.library(program.class(*class).library).name.as_ref().is_some_and(|n| n.iter().map(|x| interner.resolve(*x)).collect::<Vec<_>>().join(".") == "dart.async"))
        };
        match cx.kind(invocation) {
            "InstanceCreationExpression" => {
                let Some(ty) = cx.static_type(invocation) else { return };
                let constructor_name = cx
                    .children(invocation)
                    .iter()
                    .copied()
                    .find(|&k| cx.kind(k) == "ConstructorName")
                    .and_then(|k| cx.children(k).iter().copied().find(|&i| cx.kind(i) == "SimpleIdentifier"))
                    .map(|k| cx.text(k));
                for k in CONSTRUCTORS {
                    if constructor_name == k.name && is_async_class(ty, k.type_name) {
                        protected(k, findings);
                    }
                }
            }
            "MethodInvocation" => {
                let Some(name) = cx.method_name(invocation).map(|k| cx.text(k)) else { return };
                let target = cx.method_target(invocation).or_else(|| cx.cascade_target(invocation));
                let target_class = target.filter(|&k| matches!(cx.kind(k), "SimpleIdentifier" | "PrefixedIdentifier")).and_then(|k| {
                    let id = if cx.kind(k) == "PrefixedIdentifier" { *cx.children(k).last()? } else { k };
                    match cx.resolved(id) {
                        Some(Resolved::Element(Element::Class(c))) if program.class(*c).kind == dartforge_elements::model::ClassKind::Class => Some(*c),
                        _ => None,
                    }
                });
                if let Some(c) = target_class {
                    for m in STATIC_METHODS {
                        if Some(name) == m.name && interner.resolve(program.class(c).name) == m.type_name {
                            protected(m, findings);
                        }
                    }
                } else {
                    let Some(ty) = target.and_then(|k| cx.static_type(k)) else { return };
                    let type_name = match s.table.get(ty) {
                        Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => Some(interner.resolve(program.class(*class).name)),
                        Type::TypeParameter { param, .. } => Some(interner.resolve(s.table.param(*param).name)),
                        _ => None,
                    };
                    for m in INSTANCE_METHODS {
                        if Some(name) == m.name && type_name == Some(m.type_name) {
                            protected(m, findings);
                        }
                    }
                }
            }
            _ => {}
        }
    };

    // The visited nodes: `MethodInvocation`, `PrefixedIdentifier` and the
    // argument lists of invocations and creations.
    for n in 0..tree.nos.len() {
        match cx.kind(n) {
            "MethodInvocation" => {
                if let Some(target) = cx.method_target(n)
                    && cx.static_type(target).is_some_and(|t| super::flutter::is_build_context(s, interner, t, true))
                    && let Some(el) = context_element(&cx, interner, target)
                    && let Some(m) = associated_getter(s, interner, el)
                {
                    check(target, m, &mut findings);
                }
                for arg in cx.arguments(n) {
                    if let Some(el) = context_element(&cx, interner, arg)
                        && let Some(m) = associated_getter(s, interner, el)
                    {
                        check(arg, m, &mut findings);
                    }
                }
            }
            "InstanceCreationExpression" | "FunctionExpressionInvocation" => {
                for arg in cx.arguments(n) {
                    if let Some(el) = context_element(&cx, interner, arg)
                        && let Some(m) = associated_getter(s, interner, el)
                    {
                        check(arg, m, &mut findings);
                    }
                }
            }
            "PrefixedIdentifier" => {
                let (Some(&prefix), Some(&id)) = (cx.children(n).first(), cx.children(n).last()) else { continue };
                if cx.text(id) == "mounted" {
                    continue;
                }
                if cx.static_type(prefix).is_some_and(|t| super::flutter::is_build_context(s, interner, t, true))
                    && let Some(el) = context_element(&cx, interner, prefix)
                    && let Some(m) = associated_getter(s, interner, el)
                {
                    check(prefix, m, &mut findings);
                }
            }
            _ => {}
        }
    }
    findings.sort_by_key(|(sp, c)| (sp.start, sp.end, c.unico));
    findings.dedup_by(|x, y| x.0 == y.0 && x.1.unico == y.1.unico);
    for (span, code) in findings {
        out.push(LintReport { code, span, args: Vec::new() });
    }
    out
}
