//! The completion context type: the analysis server 3.6.2
//! `FeatureComputer.computeContextType` and `_ContextTypeVisitor`
//! (`AS:src/services/completion/dart/feature_computer.dart:174-182`,
//! `:439-1270`; docs/LSP-ESPECIFICACAO.md §14.6.5), over the tree of the
//! real text, with the query's types. `bodyContext.contextType` is that of
//! `BodyInferenceContext` (`AN:src/dart/resolve/body_inference_context.dart`):
//! the imposed type (the declared return of the method, constructor or
//! function; the return of a closure's context type, when it is a function
//! type) adjusted for `async`/`sync*`/`async*`.
//! Written without compiling or running (2026-10-05).

use super::isp::Isp;
use super::*;
use crate::analyzer_tree::Binding;
use dartforge_frontend::token::Op;

/// The names with an implied type (`_impliedDartTypeWithName`, `FC:28-44`).
const NAMES_INT: &[&str] = &["i", "j", "index", "length"];
const NAMES_NUM: &[&str] = &["height", "width"];
const NAMES_LIST: &[&str] = &["list", "items"];
const NAMES_STRING: &[&str] = &["key", "text", "url", "uri", "name", "str", "string"];

impl<'a, 'c> Isp<'a, 'c> {
    /// `computeContextType(node, offset)`: `null`/`dynamic` → none; otherwise
    /// `resolveToBound`.
    pub(super) fn context_type_in(&mut self, node: usize, o: usize) -> Option<TypeId> {
        let t = self.ctx(node, o)?;
        if matches!(self.query.table.get(t), Type::Dynamic) {
            return None;
        }
        Some(self.resolve_to_limit(t))
    }

    /// `typeSystem.resolveToBound`.
    fn resolve_to_limit(&mut self, t: TypeId) -> TypeId {
        let mut current = t;
        for _ in 0..32 {
            match self.query.table.get(current).clone() {
                Type::TypeParameter { param, nullable } => {
                    let b = self.query.table.param(param).bound;
                    current = if nullable { dartforge_types::nullable(b, &mut self.query.table) } else { b };
                }
                Type::Intersection { bound, .. } => current = bound,
                _ => break,
            }
        }
        current
    }

    /// `_visitParent`.
    fn ctx_parent(&mut self, n: usize, o: usize) -> Option<TypeId> {
        let p = self.parent(n)?;
        self.ctx(p, o)
    }

    /// The visitor's `node.contains(o)` (`offset <= o <= end`).
    fn contains_it(&self, n: usize, o: usize) -> bool {
        self.start(n) <= o && o <= self.end(n)
    }

    /// The offset of a token (that of the next real token when it is missing).
    fn pos_op(&self, op: Op, from: usize, until: usize) -> (usize, usize) {
        match self.op_in(op, from, until) {
            Some(i) => (self.span_tok(i).start, self.span_tok(i).end),
            None => {
                let p = self.synthetic(from).start;
                (p, p)
            }
        }
    }

    /// `(` and `)` of a node (those of the first `(` at the node's level).
    fn parens(&self, n: usize) -> ((usize, usize), (usize, usize)) {
        match self.op_in(Op::LParen, self.start(n), self.end(n)) {
            Some(a) => {
                let opens = (self.span_tok(a).start, self.span_tok(a).end);
                let closes = match self.pair(a).filter(|&f| self.span_tok(f).end <= self.end(n)) {
                    Some(f) => (self.span_tok(f).start, self.span_tok(f).end),
                    None => {
                        let p = self.synthetic(self.end(n)).start;
                        (p, p)
                    }
                };
                (opens, closes)
            }
            None => {
                let p = self.synthetic(self.start(n)).start;
                ((p, p), (p, p))
            }
        }
    }

    /// `range.endStart(a, b).contains(o)`: `a.end <= o <= b.offset`.
    fn between(a: (usize, usize), b: (usize, usize), o: usize) -> bool {
        a.1 <= o && o <= b.0
    }

    /// The static type of an expression node.
    fn is_static(&self, n: usize) -> Option<TypeId> {
        self.real_expr(n).and_then(|e| self.static_type(e))
    }

    /// An interface type with its arguments.
    fn interface(&mut self, c: Option<ClassId>, args: &[TypeId]) -> Option<TypeId> {
        let c = c?;
        Some(self.query.table.intern(Type::Interface { class: c, args: args.into(), nullable: false }))
    }

    /// `asInstanceOf(class).typeArguments`.
    fn arguments_as(&mut self, t: TypeId, c: Option<ClassId>) -> Option<Vec<TypeId>> {
        let c = c?;
        let Query { table, outline, core, .. } = &mut *self.query;
        let s = outline.hierarchy.supertype_of(t, c, table, core)?;
        match table.get(s) {
            Type::Interface { args, .. } => Some(args.to_vec()),
            _ => None,
        }
    }

    /// The first parameter of operator `name` on type `target`
    /// (`staticParameterElement`/`element.formalParameters[0]`).
    fn operator_parameter(&mut self, target: TypeId, name: &str) -> Option<TypeId> {
        let s = self.query.names.lookup(name)?;
        let lib = self.library();
        let (_, t) = self.query.resolver().lookup_member(target, s, false, lib)?;
        match self.query.table.get(t) {
            Type::Function { positional, .. } => positional.first().copied(),
            _ => None,
        }
    }

    /// `node.writeType` of an assignment: the declared type of the target
    /// (local, field, top-level variable, setter), not the promoted one.
    fn write_type(&mut self, target: usize) -> Option<TypeId> {
        let e = self.real_expr(target)?;
        let c = self.query_expr(e)?;
        let bodies = &self.query.bodies.units[self.unit.0 as usize];
        if let Some(&off) = bodies.local_declarations.get(&c)
            && let Some(t) = bodies.local_type(off)
        {
            return Some(t);
        }
        match bodies.get_resolved(c).cloned() {
            Some(Resolved::Element(Element::Variable(v))) => self.query.variable_type(v),
            Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => self.query.variable_type(v),
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::Element(Element::Function(f))) | Some(Resolved::ExtensionMember { member: f, .. }) => {
                let data = &self.query.outline.functions[f.0 as usize];
                match self.query.program.function(f).kind {
                    FunctionKind::Setter => data.parameters.first().map(|p| p.ty),
                    FunctionKind::ImplicitAccessor => self.query.program.function(f).variable.and_then(|v| self.query.variable_type(v)),
                    _ => bodies.get_type(c),
                }
            }
            _ => bodies.get_type(c),
        }
    }

    /// `futureValueType`.
    fn future_value(&mut self, t: TypeId) -> TypeId {
        match self.query.table.get(t).clone() {
            Type::Void | Type::Dynamic => t,
            Type::FutureOr { arg, .. } => arg,
            Type::Interface { class, args, .. } if Some(class) == self.query.core.future_class && args.len() == 1 => args[0],
            _ => self.query.core.object_nullable,
        }
    }

    /// `_contextTypeForImposed`.
    fn imposed_context(&mut self, imposed: Option<TypeId>, modifier: ast::AsyncModifier) -> Option<TypeId> {
        let r = imposed?;
        let (is_async, generator) = match modifier {
            ast::AsyncModifier::None => (false, false),
            ast::AsyncModifier::Async => (true, false),
            ast::AsyncModifier::SyncStar => (false, true),
            ast::AsyncModifier::AsyncStar => (true, true),
        };
        if !is_async && !generator {
            return Some(r);
        }
        if generator && is_async {
            let stream = self.query.core.stream_class;
            if let Some(a) = self.arguments_as(r, stream) {
                return a.first().copied();
            }
        }
        if generator && !is_async {
            let iterable = self.query.core.iterable_class;
            if let Some(a) = self.arguments_as(r, iterable) {
                return a.first().copied();
            }
        }
        let v = self.future_value(r);
        Some(self.query.table.intern(Type::FutureOr { arg: v, nullable: false }))
    }

    /// `functionBody.bodyContext?.contextType` of the body `body`.
    fn body_context(&mut self, body: usize) -> Option<TypeId> {
        let owner = self.parent(body)?;
        let (imposed, modifier) = match self.kind(owner) {
            "MethodDeclaration" => {
                let f = self.real_function(owner)?;
                let modifier = f.modifier;
                let el = f.name.and_then(|n| self.declared_function(n.span.start));
                let r = el.map(|e| self.query.outline.functions[e.0 as usize].return_type);
                (r.filter(|&t| !matches!(self.query.table.get(t), Type::Dynamic)), modifier)
            }
            "ConstructorDeclaration" => {
                let c = self.parent(owner).and_then(|k| self.node_class(k));
                (c.map(|c| self.type_this(c)), ast::AsyncModifier::None)
            }
            "FunctionExpression" => {
                let f = self.real_function(owner)?;
                let modifier = f.modifier;
                let declaration = self.parent(owner).filter(|&d| self.kind(d) == "FunctionDeclaration");
                let imposed = match declaration {
                    Some(_) => {
                        // The element's type: the written return (local), or the outline's (top level).
                        let local = f.name.and_then(|n| self.local_type(n.span.start));
                        match local {
                            Some(t) => match f.return_type {
                                Some(_) => match self.query.table.get(t) {
                                    Type::Function { ret, .. } => Some(*ret),
                                    _ => None,
                                },
                                None => None,
                            },
                            None => f.name.and_then(|n| self.declared_function(n.span.start)).map(|e| self.query.outline.functions[e.0 as usize].return_type),
                        }
                    }
                    None => {
                        // A closure: the return of its context type.
                        let ctx = self.parent(owner).and_then(|p| self.ctx(p, self.start(owner)));
                        match ctx.map(|t| self.query.table.get(t).clone()) {
                            Some(Type::Function { ret, .. }) => Some(ret),
                            _ => None,
                        }
                    }
                };
                (imposed.filter(|&t| !matches!(self.query.table.get(t), Type::Dynamic)), modifier)
            }
            _ => (None, ast::AsyncModifier::None),
        };
        self.imposed_context(imposed, modifier)
    }

    /// `_impliedDartTypeWithName`.
    fn type_by_name(&mut self, name: &str) -> Option<TypeId> {
        let core = &self.query.core;
        if name.is_empty() {
            return None;
        }
        if NAMES_INT.contains(&name) {
            return Some(core.int);
        }
        if NAMES_NUM.contains(&name) {
            return Some(core.num);
        }
        if NAMES_STRING.contains(&name) {
            return Some(core.string);
        }
        let d = core.dynamic_;
        if NAMES_LIST.contains(&name) {
            let l = core.list_class;
            return self.interface(l, &[d]);
        }
        if name == "iterator" {
            let i = core.iterable_class;
            return self.interface(i, &[d]);
        }
        if name == "map" {
            let m = core.map_class;
            return self.interface(m, &[d, d]);
        }
        None
    }

    /// The written type of a variable list (`parent.type?.type`).
    fn list_type(&self, list: usize) -> Option<TypeId> {
        let l = self.real_list(list)?;
        let t = l.ty?;
        let node = self.children(list).iter().copied().find(|&k| self.link(k) == Binding::Type(t))?;
        self.type_node_type(node)
    }

    /// `visitVariableDeclaration` of a `VariableDeclaration`.
    fn ctx_variable(&mut self, v: usize, o: usize) -> Option<TypeId> {
        let var = self.real_variable(v)?;
        let equal = var.initializer.map(|_| self.pos_op(Op::Assign, var.name.span.end, self.end(v).max(var.name.span.end + 1)));
        let (_, end_equal) = equal?;
        if end_equal > o {
            return None;
        }
        let list = self.parent(v).filter(|&l| self.kind(l) == "VariableDeclarationList")?;
        if let Some(t) = self.list_type(list) {
            return Some(t);
        }
        let name = self.real_name(var.name).to_string();
        self.type_by_name(&name)
    }

    /// The `requiredType` of a pattern (`pattern_types`).
    fn required_type(&self, pattern: usize) -> Option<TypeId> {
        let Binding::Pattern(pid) = self.link(pattern) else { return None };
        let s = self.a.pattern(pid).span;
        let (start, end) = (self.map(s.start), self.map_end(s.end));
        let ast_c = &self.query.program.unit(self.unit).ast;
        let id = ast_c.patterns.iter().position(|x| x.span.start == start && x.span.end == end)?;
        self.query.bodies.units[self.unit.0 as usize].pattern_types.get(&ast::PatternId(id as u32)).copied()
    }

    /// `_requiredTypeOfPattern`.
    fn pattern_required_type(&mut self, mut p: usize) -> Option<TypeId> {
        while self.kind(p) == "ParenthesizedPattern" {
            p = *self.children(p).first()?;
        }
        match self.kind(p) {
            "AssignedVariablePattern" => {
                let i = self.tok_after(self.start(p))?;
                let e = self.query.bodies.units[self.unit.0 as usize].pattern_declarations.iter().find(|(pid, _)| {
                    let s = self.query.program.unit(self.unit).ast.pattern(**pid).span;
                    s.start == self.map(self.span_tok(i).start)
                });
                let off = e.map(|(_, &off)| off)?;
                self.query.bodies.units[self.unit.0 as usize].local_type(off)
            }
            "DeclaredVariablePattern" => {
                let i = self.variable_pattern_name(p)?;
                self.local_type(self.span_tok(i).start)
            }
            "ListPattern" => self.required_type(p),
            _ => None,
        }
    }

    /// `_ContextTypeVisitor.visit*` on node `n` with offset `o`.
    pub(super) fn ctx(&mut self, n: usize, o: usize) -> Option<TypeId> {
        let core_bool = self.query.core.bool_;
        match self.kind(n) {
            "AdjacentStrings" => {
                if o == self.start(n) {
                    self.ctx_parent(n, o)
                } else {
                    Some(self.query.core.string)
                }
            }
            "ArgumentList" => self.ctx_arguments(n, o),
            "AsExpression" => {
                let expr = *self.children(n).first()?;
                let as_kw = self.word_in("as", self.end(expr), self.end(n))?;
                if self.span_tok(as_kw).end < o { self.is_static(expr) } else { None }
            }
            "AssertInitializer" | "AssertStatement" => {
                let (opens, closes) = self.parens(n);
                let comma = self.op_in(Op::Comma, opens.1, closes.0.max(opens.1));
                let right = comma.map_or(closes, |v| (self.span_tok(v).start, self.span_tok(v).end));
                if Self::between(opens, right, o) { Some(core_bool) } else { None }
            }
            "AssignmentExpression" => {
                let (&l, &r) = (self.children(n).first()?, self.children(n).get(1)?);
                let op = self.tok_after(self.end(l)).filter(|&i| self.span_tok(i).end <= self.start(r))?;
                if self.span_tok(op).end > o {
                    return None;
                }
                let text = self.text_tok(op);
                if text == "=" {
                    return self.write_type(l);
                }
                // Compound: the operator's method (`+=` → `+`; `??=` has none).
                let method = text.trim_end_matches('=');
                if method == "??" {
                    return None;
                }
                let target = self.is_static(l)?;
                self.operator_parameter(target, method)
            }
            "AwaitExpression" | "ConstructorName" | "ConstructorReference" | "PrefixedIdentifier" | "PropertyAccess" | "SimpleIdentifier" => self.ctx_parent(n, o),
            "BinaryExpression" => {
                let (&l, &r) = (self.children(n).first()?, self.children(n).get(1)?);
                let op = self.tok_after(self.end(l)).filter(|&i| self.span_tok(i).end <= self.start(r));
                match op {
                    Some(i) if self.span_tok(i).end <= o => {
                        let text = self.text_tok(i);
                        if matches!(text, "&&" | "||" | "??") {
                            return None;
                        }
                        let method = if text == "!=" { "==" } else { text };
                        let target = self.is_static(l)?;
                        self.operator_parameter(target, method)
                    }
                    _ => self.ctx_parent(n, o),
                }
            }
            "CascadeExpression" => {
                let target = *self.children(n).first()?;
                if o == self.start(target) { self.ctx_parent(n, o) } else { None }
            }
            "ConditionalExpression" => {
                let c = *self.children(n).first()?;
                let (q, _) = self.pos_op(Op::Question, self.end(c), self.end(n));
                if o <= q { Some(core_bool) } else { self.ctx_parent(n, o) }
            }
            "ConstructorFieldInitializer" => {
                let (_, end_equal) = self.pos_op(Op::Assign, self.start(n), self.end(n).max(self.start(n) + 1));
                if end_equal > o {
                    return None;
                }
                let ctor = self.parent(n)?;
                let v = self.initializer_field(ctor, n)?;
                self.query.variable_type(v)
            }
            "DefaultFormalParameter" => {
                let internal = *self.children(n).first()?;
                let sep = self.op_in(Op::Assign, self.end(internal), self.end(n)).or_else(|| self.op_in(Op::Colon, self.end(internal), self.end(n)))?;
                if self.span_tok(sep).end > o {
                    return None;
                }
                let prm = self.real_parameter(internal)?;
                let name = prm.name?;
                self.local_type(name.span.start).or_else(|| {
                    let text = self.real_name(name).to_string();
                    self.declared_parameter_type(internal, &text)
                })
            }
            "DoStatement" | "IfElement" | "IfStatement" | "WhileStatement" => {
                let (opens, closes) = self.parens(n);
                if Self::between(opens, closes, o) { Some(core_bool) } else { None }
            }
            "ExpressionFunctionBody" => {
                let arrow = self.op_in(Op::Arrow, self.start(n), self.end(n))?;
                if !(self.span_tok(arrow).end <= o && o <= self.end(n)) {
                    return None;
                }
                let parent = self.parent(n)?;
                match self.kind(parent) {
                    "MethodDeclaration" => self.body_context(n),
                    "FunctionExpression" => {
                        if let Some(t) = self.body_context(n) {
                            return Some(t);
                        }
                        if self.parent(parent).is_some_and(|d| self.kind(d) == "FunctionDeclaration") {
                            return None;
                        }
                        self.ctx_parent(parent, o)
                    }
                    _ => None,
                }
            }
            "FieldDeclaration" | "TopLevelVariableDeclaration" => {
                let l = self.child(n, "VariableDeclarationList")?;
                if self.contains_it(l, o) { self.ctx(l, o) } else { None }
            }
            "ForEachPartsWithDeclaration" | "ForEachPartsWithIdentifier" | "ForEachPartsWithPattern" => {
                let at = self.word_in("in", self.start(n), self.end(n))?;
                if !(self.span_tok(at).end <= o && o <= self.end(n)) {
                    return None;
                }
                let d = self.query.core.dynamic_;
                let with_await = self.kind(n) != "ForEachPartsWithPattern"
                    && self.parent(n).is_some_and(|p| matches!(self.kind(p), "ForStatement" | "ForElement") && self.tok_after(self.start(p)).is_some_and(|i| self.text_tok(i) == "await"));
                if with_await {
                    let s = self.query.core.stream_class;
                    self.interface(s, &[d])
                } else {
                    let i = self.query.core.iterable_class;
                    self.interface(i, &[d])
                }
            }
            "ForPartsWithDeclarations" | "ForPartsWithExpression" | "ForPartsWithPattern" => {
                let p1 = self.op_in(Op::Semicolon, self.start(n), self.end(n).max(self.start(n) + 1));
                let a = p1.map_or_else(|| (self.synthetic(self.start(n)).start, self.synthetic(self.start(n)).start), |i| (self.span_tok(i).start, self.span_tok(i).end));
                let p2 = p1.and_then(|i| self.op_in(Op::Semicolon, self.span_tok(i).end, self.end(n).max(self.span_tok(i).end + 1)));
                let b = p2.map_or_else(|| (self.synthetic(a.1).start, self.synthetic(a.1).start), |i| (self.span_tok(i).start, self.span_tok(i).end));
                if Self::between(a, b, o) { Some(core_bool) } else { None }
            }
            "FunctionExpressionInvocation" => {
                let f = *self.children(n).first()?;
                if self.contains_it(f, o) { self.ctx_parent(n, o) } else { None }
            }
            "IndexExpression" => {
                let target = *self.children(n).first()?;
                let opens = self.op_in(Op::LBracket, self.end(target), self.end(n))?;
                let closes = self.pair(opens).map_or_else(|| self.synthetic(self.end(n)).start, |f| self.span_tok(f).start);
                if !(self.span_tok(opens).end <= o && o <= closes) {
                    return None;
                }
                let t = self.is_static(target)?;
                self.operator_parameter(t, "[]")
            }
            "IsExpression" => {
                let expr = *self.children(n).first()?;
                let e = self.word_in("is", self.end(expr), self.end(n))?;
                if self.span_tok(e).end < o { self.is_static(expr) } else { None }
            }
            "Label" => {
                if o == self.start(n) || self.end(n) <= o {
                    self.ctx_parent(n, o)
                } else {
                    None
                }
            }
            "ListLiteral" => {
                let opens = self.op_in(Op::LBracket, self.start(n), self.end(n))?;
                let closes = self.pair(opens).map_or_else(|| self.synthetic(self.end(n)).start, |f| self.span_tok(f).start);
                if !(self.span_tok(opens).end <= o && o <= closes) {
                    return None;
                }
                match self.is_static(n).map(|t| self.query.table.get(t).clone()) {
                    Some(Type::Interface { args, .. }) => args.first().copied(),
                    _ => None,
                }
            }
            "ListPattern" => {
                let opens = self.op_in(Op::LBracket, self.start(n), self.end(n))?;
                let closes = self.pair(opens).map_or_else(|| self.synthetic(self.end(n)).start, |f| self.span_tok(f).start);
                if !(self.span_tok(opens).end <= o && o <= closes) {
                    return None;
                }
                match self.required_type(n).map(|t| self.query.table.get(t).clone()) {
                    Some(Type::Interface { args, .. }) => args.first().copied(),
                    _ => None,
                }
            }
            "MapLiteralEntry" => {
                let lit = self.ancestor(n, &["SetOrMapLiteral"])?;
                let t = self.is_static(lit)?;
                let Type::Interface { class, args, .. } = self.query.table.get(t).clone() else { return None };
                if Some(class) != self.query.core.map_class || args.len() != 2 {
                    return None;
                }
                let key = *self.children(n).first()?;
                let (sep, _) = self.pos_op(Op::Colon, self.end(key), self.end(n).max(self.end(key) + 1));
                Some(if o <= sep { args[0] } else { args[1] })
            }
            "MapPattern" => {
                let opens = self.op_in(Op::LBrace, self.start(n), self.end(n))?;
                let closes = self.pair(opens).map_or_else(|| self.synthetic(self.end(n)).start, |f| self.span_tok(f).start);
                if !(self.span_tok(opens).end <= o && o <= closes) {
                    return None;
                }
                match self.required_type(n).map(|t| self.query.table.get(t).clone()) {
                    Some(Type::Interface { class, args, .. }) if Some(class) == self.query.core.map_class && args.len() == 2 => Some(args[0]),
                    _ => None,
                }
            }
            "MapPatternEntry" => {
                let pattern = self.parent(n).filter(|&p| self.kind(p) == "MapPattern")?;
                let Type::Interface { class, args, .. } = self.query.table.get(self.required_type(pattern)?).clone() else { return None };
                if Some(class) != self.query.core.map_class || args.len() != 2 {
                    return None;
                }
                let key = *self.children(n).first()?;
                let (sep, _) = self.pos_op(Op::Colon, self.end(key), self.end(n).max(self.end(key) + 1));
                Some(if o <= sep { args[0] } else { args[1] })
            }
            "MethodInvocation" => {
                if o == self.start(n) {
                    self.ctx_parent(n, o)
                } else {
                    None
                }
            }
            "NamedExpression" => {
                let label = self.child(n, "Label");
                if o == self.start(n) || label.is_some_and(|r| self.end(r) <= o) {
                    self.ctx_parent(n, o)
                } else {
                    None
                }
            }
            "ParenthesizedExpression" => {
                let t = self.ctx_parent(n, o)?;
                match self.query.table.get(t).clone() {
                    Type::Record { positional, .. } => positional.first().copied(),
                    _ => Some(t),
                }
            }
            "PatternAssignment" | "PatternVariableDeclaration" => {
                let pattern = self.children(n).iter().copied().find(|&k| super::isp::is_pattern(self.kind(k)))?;
                let (_, end_equal) = self.pos_op(Op::Assign, self.end(pattern), self.end(n).max(self.end(pattern) + 1));
                if o >= end_equal { self.pattern_required_type(pattern) } else { None }
            }
            "PatternField" => {
                let parent = self.parent(n)?;
                match self.kind(parent) {
                    "ObjectPattern" => self.ctx_object_field(parent, n, o),
                    "RecordPattern" => self.ctx_record_field(parent, n, o),
                    _ => None,
                }
            }
            "PostfixExpression" | "PrefixExpression" => None,
            "RecordLiteral" => self.ctx_record(n, o),
            "RecordPattern" => {
                let (opens, closes) = self.parens(n);
                if !Self::between(opens, closes, o) {
                    return None;
                }
                let Type::Record { positional, .. } = self.query.table.get(self.matched_type(n)?).clone() else { return None };
                let i = self.positional_index(n, o);
                positional.get(i).copied()
            }
            "ReturnStatement" => {
                let word = self.tok_after(self.start(n))?;
                if self.span_tok(word).end < o {
                    let body = self.containing_body(n)?;
                    self.body_context(body)
                } else {
                    None
                }
            }
            "SetOrMapLiteral" => {
                let t = self.is_static(n)?;
                let opens = self.op_in(Op::LBrace, self.start(n), self.end(n))?;
                let closes = self.pair(opens).map_or_else(|| self.synthetic(self.end(n)).start, |f| self.span_tok(f).start);
                let Type::Interface { class, args, .. } = self.query.table.get(t).clone() else { return None };
                let map_or_set = Some(class) == self.query.core.map_class || Some(class) == self.query.core.set_class;
                if self.span_tok(opens).end <= o && o <= closes && map_or_set { args.first().copied() } else { None }
            }
            "SimpleStringLiteral" => None,
            "SpreadElement" => {
                let op = self.tok_after(self.start(n))?;
                if self.span_tok(op).end > o {
                    return None;
                }
                let d = self.query.core.dynamic_;
                let mut current = self.parent(n);
                while let Some(k) = current {
                    match self.kind(k) {
                        "ListLiteral" => {
                            let i = self.query.core.iterable_class;
                            return self.interface(i, &[d]);
                        }
                        "SetOrMapLiteral" => {
                            if self.is_set(k) {
                                let i = self.query.core.iterable_class;
                                return self.interface(i, &[d]);
                            }
                            let m = self.query.core.map_class;
                            return self.interface(m, &[d, d]);
                        }
                        _ => {}
                    }
                    current = self.parent(k);
                }
                None
            }
            "SwitchCase" => {
                let word = self.tok_after(self.start(n))?;
                let (dp, _) = self.pos_op(Op::Colon, self.span_tok(word).end, self.end(n).max(self.span_tok(word).end + 1));
                if !(self.span_tok(word).end <= o && o <= dp) {
                    return None;
                }
                let sw = self.parent(n).filter(|&p| self.kind(p) == "SwitchStatement")?;
                let expr = self.children(sw).iter().copied().find(|&k| super::isp::EXPRESSION_KINDS.contains(&self.kind(k)))?;
                self.is_static(expr)
            }
            "VariableDeclaration" => self.ctx_variable(n, o),
            "VariableDeclarationList" => {
                for v in self.children_of(n, &["VariableDeclaration"]) {
                    if self.contains_it(v, o) {
                        let var = self.real_variable(v)?;
                        let equal = var.initializer.map(|_| self.pos_op(Op::Assign, var.name.span.end, self.end(v).max(var.name.span.end + 1)));
                        if let Some((_, end_equal)) = equal
                            && end_equal <= o
                        {
                            if let Some(t) = self.list_type(n) {
                                return Some(t);
                            }
                            let name = self.real_name(var.name).to_string();
                            return self.type_by_name(&name);
                        }
                    }
                }
                None
            }
            "WhenClause" => Some(core_bool),
            "YieldStatement" => {
                let word = self.tok_after(self.start(n))?;
                let pv = self.tok_before(self.end(n)).filter(|&i| self.is_op(i, Op::Semicolon)).map_or_else(|| self.synthetic(self.end(n)).start, |i| self.span_tok(i).start);
                if self.span_tok(word).end <= o && o <= pv {
                    let body = self.containing_body(n)?;
                    self.body_context(body)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// `SetOrMapLiteral.isSet` (by the static type; without a type, by the
    /// elements: no `MapLiteralEntry`).
    fn is_set(&self, n: usize) -> bool {
        match self.is_static(n).map(|t| self.query.table.get(t)) {
            Some(Type::Interface { class, .. }) => Some(*class) == self.query.core.set_class,
            _ => !self.children(n).iter().any(|&k| self.kind(k) == "MapLiteralEntry"),
        }
    }

    /// `visitArgumentList`.
    fn ctx_arguments(&mut self, n: usize, o: usize) -> Option<TypeId> {
        let opens = self.tok_at(self.start(n)).filter(|&i| self.is_op(i, Op::LParen))?;
        let closes = self.tok_before(self.end(n)).filter(|&i| self.is_op(i, Op::RParen) && i != opens).map_or_else(|| self.synthetic(self.end(n)).start, |i| self.span_tok(i).start);
        if !(self.span_tok(opens).end <= o && o <= closes) {
            return None;
        }
        let parent = self.parent(n)?;
        if !matches!(self.kind(parent), "InstanceCreationExpression" | "MethodInvocation" | "FunctionExpressionInvocation") {
            return None;
        }
        let parameters = self.invoked_parameters(n)?;
        let mut index = 0usize;
        let positional_ = |index: usize| parameters.get(index).filter(|p| !p.named).map(|p| p.ty);
        let mut previous: Option<usize> = None;
        let mut positional_count = 0usize;
        for a in self.children(n).to_vec() {
            if self.kind(a) == "NamedExpression" {
                if o <= self.start(a) {
                    return positional_(index);
                }
                if self.contains_it(a, o) {
                    let label = self.child(a, "Label")?;
                    let name_end = self.tok_after(self.start(label)).map_or(self.start(label), |i| self.span_tok(i).end);
                    if o >= name_end {
                        let name = self.source[self.start(label)..self.end(label)].trim_end_matches(':').trim().to_string();
                        return parameters.iter().find(|p| p.named && p.name == name).map(|p| p.ty);
                    }
                    return None;
                }
            } else {
                if previous.is_none_or(|p| self.end(p) < o) && o <= self.end(a) {
                    // `staticParameterElement`: the positional at the same index.
                    let positional: Vec<TypeId> = parameters.iter().filter(|p| !p.named).map(|p| p.ty).collect();
                    return positional.get(positional_count).copied();
                }
                previous = Some(a);
                index += 1;
                positional_count += 1;
            }
        }
        positional_(index)
    }

    /// `visitRecordLiteral`.
    fn ctx_record(&mut self, n: usize, o: usize) -> Option<TypeId> {
        let p = self.parent(n)?;
        let t = self.ctx(p, o)?;
        let Type::Record { positional, named, .. } = self.query.table.get(t).clone() else { return None };
        let mut index = 0usize;
        for a in self.children(n).to_vec() {
            if self.kind(a) == "NamedExpression" {
                if o <= self.start(a) {
                    return positional.get(index).copied();
                }
                if self.contains_it(a, o) {
                    let label = self.child(a, "Label")?;
                    if o >= self.end(label) {
                        let name = self.source[self.start(label)..self.end(label)].trim_end_matches(':').trim().to_string();
                        return named.iter().find(|(s, _)| self.query.name(*s) == name).map(|(_, t)| *t);
                    }
                    return None;
                }
            } else {
                if o <= self.end(a) {
                    return positional.get(index).copied();
                }
                index += 1;
            }
        }
        positional.get(index).copied()
    }

    /// `_computePositionalIndex`.
    fn positional_index(&self, n: usize, o: usize) -> usize {
        let fields = self.children_of(n, &["PatternField"]);
        let mut index = 0usize;
        for c in fields {
            let mut right = self.last_token(c).map_or(self.end(c), |i| self.span_tok(i).start);
            if let Some(v) = self.tok_after(self.end(c))
                && self.is_op(v, Op::Comma)
            {
                right = self.span_tok(v).start;
            }
            if o <= right {
                return index;
            }
            if self.child(c, "PatternFieldName").is_none() {
                index += 1;
            }
        }
        index
    }

    /// `_visitFieldInObjectPattern`.
    fn ctx_object_field(&mut self, object: usize, field: usize, o: usize) -> Option<TypeId> {
        let name = self.child(field, "PatternFieldName")?;
        if o < self.end(name) {
            return None;
        }
        let i = self.tok_after(self.start(name))?;
        if !(self.word(i) && self.span_tok(i).end <= self.end(name)) {
            return None;
        }
        let text = self.text_tok(i).to_string();
        let t = self.type_node_type(self.child(object, "NamedType")?)?;
        if !matches!(self.query.table.get(t), Type::Interface { .. }) {
            return None;
        }
        let s = self.query.names.lookup(&text)?;
        let lib = self.library();
        let (r, ty) = self.query.resolver().lookup_member(t, s, false, lib)?;
        match r {
            Resolved::Member { member: MemberRef::Function(f), .. } => match self.query.program.function(f).kind {
                FunctionKind::Getter | FunctionKind::ImplicitAccessor | FunctionKind::Function => Some(ty),
                _ => None,
            },
            Resolved::Member { member: MemberRef::Variable(_), .. } => Some(ty),
            _ => None,
        }
    }

    /// `_visitFieldInRecordPattern`.
    fn ctx_record_field(&mut self, record: usize, field: usize, o: usize) -> Option<TypeId> {
        let Type::Record { positional, named, .. } = self.query.table.get(self.matched_type(record)?).clone() else { return None };
        let Some(name) = self.child(field, "PatternFieldName") else {
            let fields = self.children_of(record, &["PatternField"]);
            let i = fields.iter().position(|&c| c == field)?;
            let k = fields[..i].iter().filter(|&&c| self.child(c, "PatternFieldName").is_none()).count();
            return positional.get(k).copied();
        };
        if o < self.end(name) {
            return None;
        }
        let i = self.tok_after(self.start(name))?;
        if !(self.word(i) && self.span_tok(i).end <= self.end(name)) {
            return None;
        }
        let text = self.text_tok(i);
        named.iter().find(|(s, _)| self.query.name(*s) == text).map(|(_, t)| *t)
    }
}
