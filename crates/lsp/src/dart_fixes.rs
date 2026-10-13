//! Fix producers ported from the Dart 3.6.2 server over the analyzer tree
//! ([`crate::analyzer_tree`]): `RemoveUnusedLocalVariable`,
//! `RemoveUnusedElement` and `RemoveUnusedField` (`COR:remove_unused*.dart`),
//! with the range utilities of `RangeFactory` (docs/LSP-ESPECIFICACAO.md
//! §13.7.4.0 and §13.7.4.5).
//! Written without compiling or running (2026-10-05).

use crate::analyzer_tree::Mark;
use crate::project::{Target, Concrete, word};
use crate::refactorings::Context;
use crate::refactorings_exec::Text;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionElementId, TypedefId, VariableId};
use dartforge_frontend::ast::{self, ExprKind, MemberKind};
use dartforge_frontend::token::Kind;
use dartforge_types::{MemberRef, Resolved};

/// An `Add required argument`: the parameter name, the insertion and the
/// cursor (`Ok`: selection at the text byte; `Err`: the `VALUE` group, start
/// and length).
pub(crate) type RequiredArgument = (String, Span, String, Result<usize, (usize, usize)>);

/// The element declared by a node (its `declaredElement`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Declared {
    Class(ClassId),
    Typedef(TypedefId),
    Function(FunctionElementId),
    /// Local function, by the name offset.
    LocalFunction(usize),
    Variable(VariableId),
}

impl Context<'_> {
    // -- Tokens and ranges ----------------------------------------------------

    /// The first token that starts at `pos` or after (without the end of file).
    pub(crate) fn next_token(&self, pos: usize) -> Option<Span> {
        let i = self.tokens.partition_point(|t| t.span.start < pos);
        self.tokens.get(i).filter(|t| t.kind != Kind::Eof).map(|t| t.span)
    }

    /// The last token that ends at `pos` or before.
    pub(crate) fn previous_token(&self, pos: usize) -> Option<Span> {
        let i = self.tokens.partition_point(|t| t.span.end <= pos);
        i.checked_sub(1).map(|k| self.tokens[k].span)
    }

    fn lexeme(&self, s: Span) -> &str {
        &self.source[s.start..s.end]
    }

    /// The children of `n` of one kind.
    pub(crate) fn kind_children(&self, n: usize, kind: &str) -> Vec<usize> {
        self.children(n).iter().copied().filter(|&f| self.kind(f) == kind).collect()
    }

    /// `thisOrAncestorMatching`.
    fn this_or_ancestor(&self, n: usize, pred: impl Fn(&str) -> bool) -> Option<usize> {
        let mut current = Some(n);
        while let Some(k) = current {
            if pred(self.kind(k)) {
                return Some(k);
            }
            current = self.parent(k);
        }
        None
    }

    /// `thisOrAncestorOfType` by kind.
    pub(crate) fn this_or_ancestor_pub(&self, n: usize, kind: &str) -> Option<usize> {
        self.this_or_ancestor(n, |e| e == kind)
    }

    /// `range.nodeInList(list, item)`.
    pub(crate) fn node_in_list(&self, list: &[usize], item: usize) -> Span {
        let s = self.tree.span(item);
        if list.len() == 1 {
            if let Some(t) = self.next_token(s.end)
                && self.lexeme(t) == ","
            {
                return Span { start: s.start, end: t.end };
            }
            // The owner is the `ConstructorDeclaration` (the initializers): from the
            // token before the `:` to the body.
            if let Some(owner) = self.parent(item)
                && self.kind(owner) == "ConstructorDeclaration"
                && let Some(sep) = self.previous_token(self.tree.nodes[list[0]].start)
                && self.lexeme(sep) == ":"
                && let Some(before) = self.previous_token(sep.start)
                && let Some(body) = self.children(owner).iter().copied().find(|&f| self.kind(f).ends_with("FunctionBody"))
            {
                return Span { start: before.end, end: self.tree.nodes[body].start };
            }
            return s;
        }
        let i = list.iter().position(|&x| x == item).unwrap_or(0);
        if i == 0 {
            Span { start: s.start, end: self.tree.nodes[list[1]].start }
        } else {
            Span { start: self.tree.nodes[list[i - 1]].end, end: s.end }
        }
    }

    /// `utils.getLinesRange(range.node(n))`.
    fn node_lines(&self, n: usize) -> Span {
        let s = self.tree.span(n);
        Text::new(self.source).line_range(s.start, s.end)
    }

    /// The name (the last identifier) of a `DeclaredVariablePattern`.
    fn pattern_name(&self, n: usize) -> Option<Span> {
        let end = self.tree.nodes[n].end;
        word(self.source, end.checked_sub(1)?).filter(|s| s.end == end)
    }

    /// The `DartPattern.patternContext`.
    fn pattern_context(&self, pattern: usize) -> Option<usize> {
        let mut current = pattern;
        loop {
            let mut parent = self.parent(current)?;
            if matches!(self.kind(parent), "MapPatternEntry" | "PatternField" | "RestPatternElement") {
                parent = self.parent(parent)?;
            }
            match self.kind(parent) {
                "ForEachPartsWithPattern" | "PatternVariableDeclaration" | "PatternAssignment" | "GuardedPattern" => return Some(parent),
                e if e.ends_with("Pattern") => current = parent,
                _ => return None,
            }
        }
    }

    /// `AssignedVariablePattern`: the bare `x` of an assignment pattern.
    fn assigned_pattern(&self, n: usize) -> bool {
        self.kind(n) == "DeclaredVariablePattern"
            && self.children(n).is_empty()
            && self.pattern_context(n).is_some_and(|c| self.kind(c) == "PatternAssignment")
            && !self.node_text(n).contains(char::is_whitespace)
    }

    /// The local visible at `pos` with name `name` is the one declared at `decl`
    /// (the innermost range of the `VisibleRangesComputer` covering `pos` covers
    /// the declaration).
    fn local_in(&self, body: usize, pos: usize, name: &str, decl: usize) -> bool {
        crate::refactorings_inline::visible_ranges(self, body)
            .into_iter()
            .filter(|(n, (a, b))| n == name && *a <= pos && pos <= *b)
            .min_by_key(|(_, (a, b))| b - a)
            .is_some_and(|(_, (a, b))| a <= decl && decl <= b)
    }

    // -- RemoveUnusedLocalVariable ---------------------------------------------

    /// `RemoveUnusedLocalVariable.compute` (`remove_unused_local_variable.dart`):
    /// the edits (range, text), or `None` when Dart does not offer it.
    pub(crate) fn remove_local_variable(&self, error: Span) -> Option<Vec<(Span, String)>> {
        let node = self.tree.locate(error.start, error.end)?;
        let mut commands: Vec<(Span, String)> = Vec::new();
        // `_deleteDeclaration`.
        let declared = match self.kind(node) {
            "VariableDeclaration" => {
                let list = self.parent(node).filter(|&l| self.kind(l) == "VariableDeclarationList")?;
                let command = self.parent(list).filter(|&c| self.kind(c) == "VariableDeclarationStatement")?;
                let variables = self.kind_children(list, "VariableDeclaration");
                if variables.len() == 1 {
                    let initializer = self.children(variables[0]).first().copied();
                    match initializer {
                        Some(i) if self.kind(i) == "MethodInvocation" => {
                            commands.push((Span { start: self.tree.nodes[command].start, end: self.tree.nodes[i].start }, String::new()));
                        }
                        _ => commands.push((self.node_lines(command), String::new())),
                    }
                } else {
                    commands.push((self.node_in_list(&variables, node), String::new()));
                }
                true
            }
            "DeclaredVariablePattern" => self.exclude_declared_pattern(node, &mut commands),
            _ => false,
        };
        if !declared {
            return None;
        }
        // `_deleteReferences`: the element (on the `VariableDeclaration`, only with
        // the error on the name).
        let (decl, name) = match self.kind(node) {
            "VariableDeclaration" => {
                let s = word(self.source, self.tree.nodes[node].start)?;
                if error.start < s.start || error.start > s.end {
                    return None;
                }
                (s.start, self.lexeme(s).to_string())
            }
            _ => {
                let s = self.pattern_name(node)?;
                (s.start, self.lexeme(s).to_string())
            }
        };
        let body = self.this_or_ancestor(node, |e| e.ends_with("FunctionBody"))?;
        let mut deleted: Vec<Span> = Vec::new();
        for reference in self.local_reference_offsets(body, decl, &name) {
            let range = self.reference_range(reference)?;
            let mut covered = false;
            for other in &deleted {
                if other.start <= range.start && range.end <= other.end {
                    covered = true;
                    break;
                } else if range.start < other.end && other.start < range.end {
                    return None;
                }
            }
            if covered {
                continue;
            }
            commands.push((range, String::new()));
            deleted.push(range);
        }
        Some(commands)
    }

    /// The declaration of a `DeclaredVariablePattern` through its parent (the
    /// list, the `&&`, the object or record field).
    fn exclude_declared_pattern(&self, node: usize, commands: &mut Vec<(Span, String)>) -> bool {
        let Some(parent) = self.parent(node) else { return false };
        let ty = self.children(node).first().copied();
        let s = self.tree.span(node);
        match self.kind(parent) {
            "ListPattern" | "MapPatternEntry" => {
                let new = match ty {
                    Some(t) => format!("{} _", self.node_text(t)),
                    None => "_".to_string(),
                };
                commands.push((s, new));
                true
            }
            "LogicalAndPattern" => {
                let children = self.children(parent);
                let (Some(&left), Some(&dir)) = (children.first(), children.get(1)) else { return false };
                if let Some(t) = ty {
                    commands.push((s, format!("{} _", self.node_text(t))));
                } else if left == node {
                    commands.push((Span { start: s.start, end: self.tree.nodes[dir].start }, String::new()));
                } else {
                    commands.push((Span { start: self.tree.nodes[left].end, end: s.end }, String::new()));
                }
                true
            }
            "PatternField" => {
                let field = parent;
                let Some(owner) = self.parent(field) else { return false };
                let field_name = self.kind_children(field, "PatternFieldName").first().copied();
                let Some(name) = self.pattern_name(node) else { return false };
                // `:x` → `x: ` before the pattern.
                let explicit = |commands: &mut Vec<(Span, String)>, n: usize| {
                    let colon = self.tree.nodes[n].end - 1;
                    commands.push((Span { start: colon, end: s.start }, format!("{}: ", self.lexeme(name))));
                };
                let unnamed = |n: usize| self.node_text(n).trim() == ":";
                match self.kind(owner) {
                    "ObjectPattern" => {
                        let Some(n) = field_name else { return false };
                        let fields = self.kind_children(owner, "PatternField");
                        if fields.len() == 1
                            && let Some(decl) = self.parent(owner).filter(|&d| self.kind(d) == "PatternVariableDeclaration")
                            && let Some(cmd) = self.parent(decl).filter(|&c| self.kind(c) == "PatternVariableDeclarationStatement")
                        {
                            commands.push((self.node_lines(cmd), String::new()));
                            return true;
                        }
                        if ty.is_some() && self.pattern_context(owner).is_some_and(|c| self.kind(c) == "GuardedPattern") {
                            if unnamed(n) {
                                explicit(commands, n);
                            }
                            commands.push((name, "_".to_string()));
                            return true;
                        }
                        commands.push((self.node_in_list(&fields, field), String::new()));
                        true
                    }
                    "RecordPattern" => {
                        if let Some(n) = field_name
                            && unnamed(n)
                        {
                            explicit(commands, n);
                        }
                        commands.push((name, "_".to_string()));
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// `findLocalElementReferences3(body, element)`: the local's
    /// `SimpleIdentifier`s and its `AssignedVariablePattern`s that are direct
    /// elements of a `ListPattern` or the pattern (without parentheses) of a
    /// `RecordPattern` field (the visitor does not descend into those two
    /// patterns).
    fn local_reference_offsets(&self, body: usize, decl: usize, name: &str) -> Vec<usize> {
        let mut v = Vec::new();
        let mut stack = vec![body];
        let mut order: Vec<usize> = Vec::new();
        while let Some(k) = stack.pop() {
            order.push(k);
            match self.kind(k) {
                "ListPattern" => {
                    for &f in self.children(k) {
                        if self.assigned_pattern(f) && self.node_text(f) == name && self.local_in(body, self.tree.nodes[f].start, name, decl) {
                            v.push(f);
                        }
                    }
                    continue;
                }
                "RecordPattern" => {
                    for &field in self.children(k) {
                        let Some(&pattern) = self.children(field).iter().rev().find(|&&f| self.kind(f) != "PatternFieldName") else { continue };
                        let mut internal = pattern;
                        while self.kind(internal) == "ParenthesizedPattern" {
                            let Some(&i) = self.children(internal).first() else { break };
                            internal = i;
                        }
                        if self.assigned_pattern(internal) && self.node_text(internal) == name && self.local_in(body, self.tree.nodes[internal].start, name, decl) {
                            v.push(pattern);
                        }
                    }
                    continue;
                }
                "SimpleIdentifier" => {
                    if let Mark::Expr(x) = self.tree.nodes[k].mark
                        && self.bodies.local_declaration(x) == Some(decl)
                    {
                        v.push(k);
                    }
                }
                _ => {}
            }
            for &f in self.children(k).iter().rev() {
                stack.push(f);
            }
        }
        v.sort_by_key(|&k| self.tree.nodes[k].start);
        v
    }

    /// `_referenceRangeToDelete`: only the left side of an assignment.
    fn reference_range(&self, reference: usize) -> Option<Span> {
        let parent = self.parent(reference)?;
        if self.kind(parent) != "AssignmentExpression" || self.children(parent).first() != Some(&reference) {
            return None;
        }
        let grandparent = self.parent(parent)?;
        if self.kind(grandparent) == "ArgumentList" {
            let operator = self.next_token(self.tree.nodes[reference].end)?;
            let after = self.next_token(operator.end)?;
            return Some(Span { start: self.tree.nodes[parent].start, end: after.start });
        }
        Some(self.node_lines(grandparent))
    }

    // -- RemoveUnusedElement / RemoveUnusedField --------------------------------

    /// The `declaredElement` of the nodes that `RemoveUnusedElement` accepts.
    fn declared(&self, n: usize) -> Option<Declared> {
        let p = self.p.program();
        match (self.kind(n), self.tree.nodes[n].mark) {
            ("ClassDeclaration" | "EnumDeclaration", Mark::Decl(d)) => self.declaration_class(self.unit, d).map(Declared::Class),
            ("FunctionTypeAlias", Mark::Decl(d)) => (0..p.typedefs.len())
                .map(|i| TypedefId(i as u32))
                .find(|&t| p.typedef(t).decl.unit == self.unit && p.typedef(t).decl.decl == d)
                .map(Declared::Typedef),
            ("FunctionDeclaration", Mark::Function(fid)) => {
                if self.parent(n).is_some_and(|x| self.kind(x) == "FunctionDeclarationStatement") {
                    self.ast.function(fid).name.map(|nm| Declared::LocalFunction(nm.span.start))
                } else {
                    self.p.node_function(self.unit, fid).map(Declared::Function)
                }
            }
            ("MethodDeclaration", Mark::Function(fid)) => self.p.node_function(self.unit, fid).map(Declared::Function),
            ("VariableDeclaration", _) => {
                let s = word(self.source, self.tree.nodes[n].start)?;
                crate::document_highlight::variable_declared_in(self.p, self.unit, s.start).map(Declared::Variable)
            }
            _ => None,
        }
    }

    /// The resolution matches the element (the `writeOrReadElement`, the
    /// variable's accessor).
    fn resolve_to(&self, r: &Resolved, target: Declared) -> bool {
        let p = self.p.program();
        let function = |f: FunctionElementId| match target {
            Declared::Function(g) => f == g,
            Declared::Variable(v) => p.function(f).variable == Some(v),
            _ => false,
        };
        match r {
            Resolved::Element(Element::Class(c)) => target == Declared::Class(*c),
            Resolved::Element(Element::Typedef(t)) => target == Declared::Typedef(*t),
            Resolved::Element(Element::Function(f)) => function(*f),
            Resolved::Element(Element::Variable(v)) | Resolved::Member { member: MemberRef::Variable(v), .. } => target == Declared::Variable(*v),
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => function(*f),
            _ => false,
        }
    }

    /// `_ElementReferenceCollector` over the unit: the ranges of the
    /// `SimpleIdentifier`s that refer to the element (expressions, field names
    /// in initializers, the `returnType` of the class's constructors,
    /// annotations, documentation references), of the `NamedType`s and of the
    /// field's `this.x`s.
    fn declared_references(&self, target_: Declared) -> Vec<Span> {
        let p = self.p.program();
        let u = p.unit(self.unit);
        let ast = self.ast;
        let mut v: Vec<Span> = Vec::new();
        for (i, e) in ast.exprs.iter().enumerate() {
            let id = ast::ExprId(i as u32);
            let name_ = match &e.kind {
                ExprKind::Identifier(n) => n.span,
                ExprKind::Property { name, .. } => name.span,
                _ => continue,
            };
            let by_local = matches!(target_, Declared::LocalFunction(d) if self.bodies.local_declaration(id) == Some(d));
            let resolved = self.bodies.get_resolved(id).is_some_and(|r| self.resolve_to(r, target_));
            // The call `o.m()`/`m()` keeps the resolution on the call.
            let by_call = ast.exprs.iter().enumerate().any(|(j, c)| match &c.kind {
                ExprKind::Call { target, .. } if *target == id => self.bodies.get_resolved(ast::ExprId(j as u32)).is_some_and(|r| match r {
                    Resolved::Constructor(f) => matches!(target_, Declared::Class(c) if p.function(*f).class == Some(c)) && matches!(e.kind, ExprKind::Identifier(_)),
                    other => self.resolve_to(other, target_),
                }),
                _ => false,
            });
            if by_local || resolved || by_call {
                v.push(name_);
            }
        }
        let ty = match target_ {
            Declared::Class(c) => Some(Element::Class(c)),
            Declared::Typedef(t) => Some(Element::Typedef(t)),
            _ => None,
        };
        if let Some(el) = ty {
            for t in &ast.types {
                let ast::TypeKind::Named { name, .. } = &t.kind else { continue };
                let (b, n) = match &name[..] {
                    [n] if crate::project::type_parameter_declaration(ast, n.span.start, n.sym).is_none() => (p.lookup(u.library, n.sym), *n),
                    [pr, n] => (p.lookup_prefixed(u.library, pr.sym, n.sym), *n),
                    _ => continue,
                };
                if b.and_then(|b| b.getter) == Some(el) {
                    v.push(n.span);
                }
            }
        }
        if let Declared::Class(c) = target_ {
            for (mi, m) in ast.members.iter().enumerate() {
                if let MemberKind::Constructor(k) = &m.kind
                    && self.p.node_constructor(self.unit, ast::MemberId(mi as u32)).and_then(|f| p.function(f).class) == Some(c)
                {
                    v.push(k.class_name.span);
                }
            }
        }
        if let Declared::Variable(field) = target_
            && let Some(class) = p.variable(field).class
        {
            let name_ = p.variable(field).name;
            for (mi, m) in ast.members.iter().enumerate() {
                let MemberKind::Constructor(k) = &m.kind else { continue };
                if self.p.node_constructor(self.unit, ast::MemberId(mi as u32)).and_then(|f| p.function(f).class) != Some(class) {
                    continue;
                }
                for q in k.parameters.iter() {
                    if q.this_ && q.name.is_some_and(|n| n.sym == name_) {
                        v.push(q.span);
                    }
                }
                for start in k.initializers.iter() {
                    if let ast::Initializer::Field { name, .. } = start
                        && name.sym == name_
                    {
                        v.push(name.span);
                    }
                }
            }
        }
        // Annotations: the name resolves to the class or to the accessor.
        for a in crate::project::metadata(ast, &u.unit) {
            let b = match &a.name[..] {
                [n, ..] if p.lookup(u.library, n.sym).is_some() => p.lookup(u.library, n.sym).map(|b| (b, n.span)),
                [pr, n, ..] => p.lookup_prefixed(u.library, pr.sym, n.sym).map(|b| (b, n.span)),
                _ => None,
            };
            if let Some((b, s)) = b
                && let Some(g) = b.getter
                && self.resolve_to(&Resolved::Element(g), target_)
            {
                v.push(s);
            }
        }
        // `[x]` in documentation.
        for c in crate::dartdoc::comments(self.source) {
            for r in &c.references {
                for i in 0..r.len() {
                    let Some((a, concrete)) = self.p.resolve_doc_reference(self.unit, c.span, &r[..=i]) else { continue };
                    let matches_ = match (target_, &a, concrete) {
                        (_, Target::Top(el), _) => self.resolve_to(&Resolved::Element(*el), target_),
                        (_, _, Some(Concrete::Function(f))) => self.resolve_to(&Resolved::Element(Element::Function(f)), target_),
                        (Declared::Variable(v0), _, Some(Concrete::Variable(v1))) => v0 == v1,
                        (Declared::LocalFunction(d), Target::Local { unit, declaration }, _) => *unit == self.unit && *declaration == d,
                        _ => false,
                    };
                    if matches_ {
                        v.push(r[i].0);
                    }
                }
            }
        }
        v.sort_by_key(|s| (s.start, s.end));
        v.dedup();
        v
    }

    /// `RemoveUnusedElement.compute`: the deletions, or `None` without a fix.
    pub(crate) fn remove_element(&self, error: Span) -> Option<Vec<Span>> {
        let node = self.tree.locate(error.start, error.end)?;
        let kind = self.kind(node);
        if kind == "ConstructorDeclaration" {
            let owner = self.parent(node)?;
            if !matches!(self.kind(owner), "ClassDeclaration" | "EnumDeclaration") {
                return None;
            }
            let members: Vec<usize> = self
                .children(owner)
                .iter()
                .copied()
                .filter(|&f| matches!(self.kind(f), "ConstructorDeclaration" | "MethodDeclaration" | "FieldDeclaration"))
                .collect();
            return Some(vec![self.node_in_list(&members, node)]);
        }
        if !matches!(kind, "ClassDeclaration" | "EnumDeclaration" | "FunctionDeclaration" | "FunctionTypeAlias" | "MethodDeclaration" | "VariableDeclaration") {
            return None;
        }
        let target = self.declared(node)?;
        if !self.declared_references(target).is_empty() {
            return None;
        }
        let parent = self.parent(node);
        let grandparent = parent.and_then(|x| self.parent(x));
        let range = match (kind, parent, grandparent) {
            ("VariableDeclaration", Some(list), Some(grandparent)) if self.kind(list) == "VariableDeclarationList" => {
                let variables = self.kind_children(list, "VariableDeclaration");
                if variables.len() == 1 { self.node_lines(grandparent) } else { self.node_in_list(&variables, node) }
            }
            _ => self.node_lines(node),
        };
        Some(vec![range])
    }

    /// `RemoveUnusedField.compute`: the deletions, or `None` without a fix.
    pub(crate) fn remove_field(&self, error: Span) -> Option<Vec<Span>> {
        let node = self.tree.locate(error.start, error.end)?;
        if self.kind(node) != "VariableDeclaration" {
            return None;
        }
        let Declared::Variable(field) = self.declared(node)? else { return None };
        if self.p.program().variable(field).class.is_none() {
            return None;
        }
        let mut references = vec![node];
        for s in self.declared_references(Declared::Variable(field)) {
            references.push(self.tree.locate(s.start, s.end)?);
        }
        let mut ranges: Vec<Span> = Vec::new();
        for r in references {
            let node_ = self.this_or_ancestor(r, |e| matches!(e, "VariableDeclaration" | "ExpressionStatement" | "ConstructorFieldInitializer" | "FieldFormalParameter"))?;
            let parent = self.parent(node_);
            let grandparent = parent.and_then(|x| self.parent(x));
            let range = match self.kind(node_) {
                "VariableDeclaration" if parent.is_some_and(|l| self.kind(l) == "VariableDeclarationList") && grandparent.is_some() => {
                    let variables = self.kind_children(parent?, "VariableDeclaration");
                    if variables.len() == 1 { self.node_lines(grandparent?) } else { self.node_in_list(&variables, node_) }
                }
                "ConstructorFieldInitializer" => {
                    let constructor = parent?;
                    let initializers: Vec<usize> = self
                        .children(constructor)
                        .iter()
                        .copied()
                        .filter(|&f| matches!(self.kind(f), "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer"))
                        .collect();
                    if initializers.len() == 1 {
                        let parameters = self.kind_children(constructor, "FormalParameterList").first().copied()?;
                        Span { start: self.tree.nodes[parameters].end, end: self.tree.nodes[node_].end }
                    } else {
                        self.node_in_list(&initializers, node_)
                    }
                }
                "FieldFormalParameter" => self.field_parameter_range(node_)?,
                _ => self.node_lines(node_),
            };
            ranges.push(range);
        }
        // `_uniqueSourceRanges`: a range covered by another one (another entry of
        // the list, even if equal) goes away.
        let unique: Vec<Span> = ranges
            .iter()
            .enumerate()
            .filter(|(i, c)| !ranges.iter().enumerate().any(|(j, o)| j != *i && o.start <= c.start && c.end <= o.end))
            .map(|(_, c)| *c)
            .collect();
        // Partial overlap: `ConflictingEditException`.
        for (i, a) in unique.iter().enumerate() {
            for b in &unique[i + 1..] {
                if a.start < b.end && b.start < a.end {
                    return None;
                }
            }
        }
        Some(unique)
    }

    // -- AddAsync ------------------------------------------------------------------

    /// `AddAsync` (`add_async.dart`, outside `missingReturn`) with the
    /// `DartFileEditBuilder`'s `convertFunctionFromSyncToAsync`: the enclosing
    /// `FunctionBody` without a keyword gets `async ` (with a space before it
    /// if it is attached to the previous token; not an empty body); the written
    /// return of the enclosing function or method (a closure has none) becomes
    /// `Future<flatten(T)>`, except `dynamic` and `Future`. The edits and the
    /// libraries to import.
    pub(crate) fn add_async(&self, error: Span) -> Option<(Vec<(Span, String)>, std::collections::BTreeSet<dartforge_elements::model::LibraryId>)> {
        use dartforge_types::Type;
        let node = self.tree.locate(error.start, error.end)?;
        let body = self.this_or_ancestor(node, |e| e.ends_with("FunctionBody"))?;
        let text = self.node_text(body);
        let keyword_ = |k: &str| text.strip_prefix(k).is_some_and(|r| !r.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$'));
        if keyword_("async") || keyword_("sync") {
            return None;
        }
        let mut edits = Vec::new();
        let start = self.tree.nodes[body].start;
        if self.kind(body) != "EmptyFunctionBody" {
            let attached = self.previous_token(start).is_some_and(|t| t.end == start);
            edits.push((Span { start: start, end: start }, format!("{}async ", if attached { " " } else { "" })));
        }
        let mut writer = crate::write_type::Writer::new(self, start);
        // `_replaceReturnTypeWithFuture`.
        let mut current = self.parent(body);
        while let Some(k) = current {
            match self.kind(k) {
                "FunctionDeclaration" | "MethodDeclaration" => {
                    let annotation = self
                        .children(k)
                        .iter()
                        .copied()
                        .find(|&f| matches!(self.kind(f), "NamedType" | "GenericFunctionType" | "RecordTypeAnnotation"));
                    let Some(annotation) = annotation else { break };
                    let element = self.declared_element(k);
                    let table = &self.p.query.table;
                    let ty = match element {
                        crate::refactorings::Elem::Function(f) => self.p.query.outline.functions.get(f.0 as usize).map(|d| d.return_type),
                        crate::refactorings::Elem::LocalFunction(fid) => self.ast.function(fid).name.and_then(|n| match self.bodies.local_type(n.span.start).map(|t| table.get(t)) {
                            Some(Type::Function { ret, .. }) => Some(*ret),
                            _ => None,
                        }),
                        _ => None,
                    };
                    let Some(ty) = ty else { break };
                    let future = self.p.query.core.future_class;
                    let (flattened, nullable_) = match table.get(ty) {
                        Type::Dynamic => break,
                        Type::Interface { class, .. } if Some(*class) == future => break,
                        Type::FutureOr { arg, nullable } => (*arg, *nullable),
                        _ => (ty, false),
                    };
                    let new_text = writer.write_future(flattened, nullable_).unwrap_or_else(|| "void".to_string());
                    edits.push((self.tree.span(annotation), new_text));
                    break;
                }
                "FunctionExpression" if self.parent(k).is_none_or(|x| self.kind(x) != "FunctionDeclaration") => break,
                _ => {}
            }
            current = self.parent(k);
        }
        Some((edits, writer.import))
    }

    // -- ReplaceWithNotNullAware, ChangeToStaticAccess, RemoveAssertion ----------

    /// `ReplaceWithNotNullAware.compute` on the `coveringNode`: the range, the
    /// new text and the title's `{0}`.
    pub(crate) fn swap_null_aware_operator(&self, n: usize) -> Option<(Span, String, &'static str)> {
        let s = self.tree.span(n);
        // The operator: the first token of the cascade section, or the token after
        // the target.
        let operator_after_target = |n: usize| -> Option<Span> {
            let first = self.next_token(s.start)?;
            if matches!(self.lexeme(first), "?.." | ".." | "?." | ".") && first.start == s.start {
                return Some(first);
            }
            let target = *self.children(n).first()?;
            self.next_token(self.tree.nodes[target].end)
        };
        match self.kind(n) {
            "MethodInvocation" | "PropertyAccess" => {
                let op = operator_after_target(n)?;
                if !matches!(self.lexeme(op), "?." | "?.." | "." | "..") {
                    return None;
                }
                let new = if self.lexeme(op) == "?." { "." } else { ".." };
                Some((op, new.to_string(), new))
            }
            "IndexExpression" => {
                let first = self.next_token(s.start)?;
                if first.start == s.start && matches!(self.lexeme(first), "?.." | "..") {
                    return Some((first, "..".to_string(), ".."));
                }
                let target = *self.children(n).first()?;
                let q = self.next_token(self.tree.nodes[target].end)?;
                if self.lexeme(q) == "?" {
                    return Some((q, String::new(), "["));
                }
                None
            }
            "SpreadElement" => {
                let op = self.next_token(s.start)?;
                Some((op, "...".to_string(), "..."))
            }
            _ => None,
        }
    }

    /// `ChangeToStaticAccess.compute`: the class or extension name (`{0}`), the
    /// target's range, its `writeReference` and the libraries to import.
    pub(crate) fn static_access(&self, error: Span) -> Option<(String, Span, String, std::collections::BTreeSet<dartforge_elements::model::LibraryId>)> {
        use dartforge_elements::model::Element;
        let node = self.tree.locate(error.start, error.end)?;
        if self.kind(node) != "SimpleIdentifier" {
            return None;
        }
        let parent = self.parent(node)?;
        let target = match self.kind(parent) {
            "MethodInvocation" if self.method_name(parent) == Some(node) => {
                let f = self.children(parent);
                let first = *f.first()?;
                (first != node).then_some(first)?
            }
            "PrefixedIdentifier" if self.children(parent).get(1) == Some(&node) => *self.children(parent).first()?,
            _ => return None,
        };
        let f = match self.identifier_element(node, true) {
            crate::refactorings::Elem::Function(f) => f,
            crate::refactorings::Elem::Variable => {
                let Mark::Expr(x) = self.tree.nodes[node].mark else { return None };
                match self.bodies.get_resolved(x)? {
                    Resolved::Member { member: MemberRef::Variable(v), .. } => self.p.program().variable(*v).getter?,
                    Resolved::Member { member: MemberRef::Function(f), .. } => *f,
                    _ => return None,
                }
            }
            _ => return None,
        };
        let prog = self.p.program();
        let fe = prog.function(f);
        let mut writer = crate::write_type::Writer::new(self, self.tree.nodes[target].start);
        let (name, reference) = if let Some(c) = fe.class {
            let name = self.p.name(prog.class(c).name).to_string();
            (name.clone(), writer.reference_of(Element::Class(c), &name))
        } else if let Some(x) = fe.extension {
            let name = self.p.name(prog.extension(x).name?).to_string();
            (name.clone(), writer.reference_of(Element::Extension(x), &name))
        } else {
            return None;
        };
        Some((name, self.tree.span(target), reference, writer.import))
    }

    /// `RemoveAssertion`: the `AssertInitializer` child of a constructor goes
    /// away through `nodeInList`.
    pub(crate) fn remove_assertion(&self, error: Span) -> Option<Span> {
        let node = self.tree.locate(error.start, error.end)?;
        let parent = self.parent(node)?;
        if self.kind(parent) != "ConstructorDeclaration" {
            return None;
        }
        let initializers: Vec<usize> = self
            .children(parent)
            .iter()
            .copied()
            .filter(|&f| matches!(self.kind(f), "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer"))
            .collect();
        if !initializers.contains(&node) {
            return None;
        }
        Some(self.node_in_list(&initializers, node))
    }

    /// `MakeFinal` (`make_final.dart:84-97`) on the `node`'s list
    /// (`_getVariableDeclarationList`) with one variable: `var` → `final`;
    /// `late` → ` final` after it; without a keyword → `final ` at the start of
    /// the list.
    pub(crate) fn make_final(&self, error: Span) -> Option<(Span, String)> {
        let node = self.tree.locate(error.start, error.end)?;
        let parent = self.parent(node);
        let list = match self.kind(node) {
            "VariableDeclarationList" => node,
            "VariableDeclaration" | "NamedType" if parent.is_some_and(|x| self.kind(x) == "VariableDeclarationList") => parent?,
            _ => {
                let grandparent = parent.and_then(|x| self.parent(x))?;
                if parent.is_some_and(|x| self.kind(x) == "NamedType") && self.kind(grandparent) == "VariableDeclarationList" {
                    grandparent
                } else {
                    return None;
                }
            }
        };
        if self.kind_children(list, "VariableDeclaration").len() != 1 {
            return None;
        }
        let s = self.tree.span(list);
        // The keywords at the start of the list (`late`, `var`/`final`/`const`).
        let mut late = None;
        let mut word = None;
        let mut pos = s.start;
        while let Some(t) = self.next_token(pos) {
            if t.start >= s.end {
                break;
            }
            match self.lexeme(t) {
                "late" => late = Some(t),
                "var" | "final" | "const" => word = Some(t),
                _ => break,
            }
            pos = t.end;
        }
        match (word, late) {
            (Some(k), _) if self.lexeme(k) == "var" => Some((k, "final".to_string())),
            (_, Some(l)) => Some((Span { start: l.end, end: l.end }, " final".to_string())),
            (None, None) => Some((Span { start: s.start, end: s.start }, "final ".to_string())),
            _ => None,
        }
    }

    // -- Comments and ranges with comments ------------------------------------

    /// The line (0-based) of the offset.
    fn line(&self, o: usize) -> usize {
        let b = self.source.as_bytes();
        let end = o.min(b.len());
        let mut n = 0;
        let mut i = 0;
        while i < end {
            match b[i] {
                b'\r' if b.get(i + 1) == Some(&b'\n') => {
                    n += 1;
                    i += 2;
                    continue;
                }
                b'\r' | b'\n' => n += 1,
                _ => {}
            }
            i += 1;
        }
        n
    }

    /// The comments between the previous token and token `t`
    /// (`precedingComments`, in order).
    pub(crate) fn comments_before(&self, t: Span) -> Vec<Span> {
        let a = self.previous_token(t.start).map_or(0, |p| p.end);
        self.comments.iter().copied().filter(|c| c.start >= a && c.end <= t.start).collect()
    }

    /// The first and the last token of the node.
    fn node_tokens(&self, n: usize) -> Option<(Span, Span)> {
        let s = self.tree.span(n);
        Some((self.next_token(s.start)?, self.previous_token(s.end)?))
    }

    /// `range.deletionRange(node, overrideEnd)`.
    pub(crate) fn exclusion_range(&self, n: usize, forced_end: Option<Span>) -> Option<Span> {
        let (first, last) = self.node_tokens(n)?;
        let start = self.comments_before(first).first().copied().unwrap_or(first);
        let initial_end = forced_end.unwrap_or(last);
        let next = self.next_token(initial_end.end);
        let after: Option<Span> = match next {
            Some(t) => Some(self.comments_before(t).first().copied().unwrap_or(t)),
            None => self.comments.iter().copied().find(|c| c.start >= initial_end.end),
        };
        match after {
            Some(e) => Some(Span { start: start.start, end: e.start }),
            None => {
                // `end.isEof`.
                let is_comment = start != first;
                let annotated = matches!(self.kind(n), "ClassDeclaration" | "MixinDeclaration" | "EnumDeclaration" | "ExtensionDeclaration" | "ExtensionTypeDeclaration" | "FunctionDeclaration" | "MethodDeclaration" | "FieldDeclaration" | "ConstructorDeclaration" | "TopLevelVariableDeclaration" | "FunctionTypeAlias" | "GenericTypeAlias" | "ClassTypeAlias");
                let start_ = if annotated && is_comment {
                    let first_after = self.first_token_after_comment_and_metadata(n)?;
                    self.previous_token(first_after.start).map_or(first_after.start, |p| p.end)
                } else {
                    match self.previous_token(start.start) {
                        None => start.start,
                        Some(p) => p.end,
                    }
                };
                Some(Span { start: start_, end: initial_end.end })
            }
        }
    }

    /// `firstTokenAfterCommentAndMetadata`.
    pub(crate) fn first_token_after_comment_and_metadata(&self, n: usize) -> Option<Span> {
        let mut pos = self.tree.nodes[n].start;
        for &f in self.children(n) {
            if matches!(self.kind(f), "Comment" | "Annotation") {
                pos = pos.max(self.tree.nodes[f].end);
            }
        }
        self.next_token(pos)
    }

    /// `_leadingComment(token)`.
    fn initial_comment(&self, t: Span) -> Span {
        let before = self.comments_before(t);
        let Some(previous) = self.previous_token(t.start) else {
            return before.first().copied().unwrap_or(t);
        };
        let line_t = self.line(t.start);
        let previous_line = self.line(previous.start);
        let mut i = 0;
        if line_t != previous_line {
            while i < before.len() && self.line(before[i].start) == previous_line {
                i += 1;
            }
        }
        before.get(i).copied().unwrap_or(t)
    }

    /// `_shouldIncludeCommentsAfterComma`.
    fn include_comments_after_comma(&self, comma: Span) -> bool {
        let Some(after) = self.next_token(comma.end) else { return false };
        if matches!(self.lexeme(after), "}" | ")" | "]") {
            return true;
        }
        self.line(comma.start) != self.line(after.start)
    }

    /// `trailingComment(token, returnComma)`: the token (or comment) and whether
    /// it includes the comma.
    fn trailing_comment(&self, t: Span, return_comma: bool) -> (Span, bool) {
        let mut last = t;
        let mut next = self.next_token(last.end);
        let includes_comma = next.is_some_and(|s| self.lexeme(s) == "," && self.include_comments_after_comma(s));
        if includes_comma {
            last = next.unwrap_or(t);
            next = self.next_token(last.end);
        }
        let preceding = |s: Option<Span>, after: usize| -> Vec<Span> {
            match s {
                Some(s) => self.comments_before(s),
                None => self.comments.iter().copied().filter(|c| c.start >= after).collect(),
            }
        };
        let mut chain = preceding(next, last.end);
        if chain.is_empty() && includes_comma && self.line(t.start) != self.line(last.start) {
            chain = self.comments_before(last);
            last = t;
        }
        if let Some(&first) = chain.first() {
            let line = self.line(last.start);
            if self.line(first.start) == line {
                let mut c = first;
                for &next_ in &chain[1..] {
                    if self.line(next_.start) != line {
                        break;
                    }
                    c = next_;
                }
                return (c, includes_comma);
            }
        }
        (if return_comma { last } else { t }, false)
    }

    /// `range.nodeInListWithComments(lineInfo, list, node)`.
    fn node_in_list_with_comments(&self, list: &[usize], node: usize) -> Option<Span> {
        let (start, end) = self.node_tokens(node)?;
        if list.len() == 1 {
            let initial = self.initial_comment(start);
            let (final_, _) = self.trailing_comment(end, true);
            return Some(Span { start: initial.start, end: final_.end });
        }
        let i = list.iter().position(|&x| x == node)?;
        if i == 0 {
            let this = self.initial_comment(start);
            let (next_start, _) = self.node_tokens(list[1])?;
            let next = self.initial_comment(next_start);
            return Some(Span { start: this.start, end: next.start });
        }
        let (_, previous_end) = self.node_tokens(list[i - 1])?;
        let (previous, previous_comma) = self.trailing_comment(previous_end, false);
        let (this, this_comma) = self.trailing_comment(end, previous_comma);
        let mut previous_token = previous;
        if !previous_comma && this_comma {
            previous_token = self.next_token(previous_token.end)?;
        }
        Some(Span { start: previous_token.end, end: this.end })
    }

    /// `range.nodeWithComments(lineInfo, node)`.
    fn node_with_comments(&self, node: usize) -> Option<Span> {
        let (start, end) = self.node_tokens(node)?;
        let unit_first = self.next_token(0) == Some(start);
        let initial = if unit_first { start } else { self.initial_comment(start) };
        let (final_, _) = self.trailing_comment(end, false);
        Some(Span { start: initial.start, end: final_.end })
    }

    // -- Producers of the published codes without a fix ---------------------------------

    /// `RemoveNameFromDeclarationClause`: the title and the deletion.
    pub(crate) fn remove_clause_name(&self, error: Span) -> Option<(String, Span)> {
        let ty = self.tree.locate(error.start, error.end)?;
        let clause = self.parent(ty)?;
        let name = match self.kind(clause) {
            "ExtendsClause" => return Some(("Remove 'extends' clause".to_string(), self.exclusion_range(clause, None)?)),
            "ImplementsClause" => "implements",
            "MixinOnClause" => "on",
            "WithClause" => "with",
            _ => return None,
        };
        let names = self.kind_children(clause, "NamedType");
        if names.len() == 1 {
            return Some((format!("Remove '{name}' clause"), self.exclusion_range(clause, None)?));
        }
        if !names.contains(&ty) {
            return None;
        }
        Some((format!("Remove name from '{name}' clause"), self.node_in_list(&names, ty)))
    }

    /// `AddClassModifier`: the modifier's insertion point.
    pub(crate) fn modifier_dot(&self, error: Span) -> Option<usize> {
        let node = self.tree.locate(error.start, error.end)?;
        if !matches!(
            self.kind(node),
            "ClassDeclaration" | "ClassTypeAlias" | "MixinDeclaration" | "EnumDeclaration" | "ExtensionTypeDeclaration" | "FunctionDeclaration" | "FunctionTypeAlias" | "GenericTypeAlias"
        ) || self.parent(node) != Some(0)
        {
            return None;
        }
        Some(self.first_token_after_comment_and_metadata(node)?.start)
    }

    /// `UseEqEqNull`/`UseNotEqNull`: `[expression.end, is.end)`.
    pub(crate) fn is_null_range(&self, error: Span) -> Option<Span> {
        let n = self.tree.locate2(error.start, error.end.saturating_sub(1))?;
        if self.kind(n) != "IsExpression" {
            return None;
        }
        let e = *self.children(n).first()?;
        Some(Span { start: self.tree.nodes[e].end, end: self.tree.nodes[n].end })
    }

    /// `RemoveExtendsClause`: `[extends, {)`.
    pub(crate) fn remove_extends(&self, error: Span) -> Option<Span> {
        let node = self.tree.locate(error.start, error.end)?;
        let class = self.this_or_ancestor(node, |e| e == "ClassDeclaration")?;
        let extends = self.kind_children(class, "ExtendsClause").first().copied()?;
        let mut pos = self.tree.nodes[extends].end;
        loop {
            let t = self.next_token(pos)?;
            if self.lexeme(t) == "{" {
                return Some(Span { start: self.tree.nodes[extends].start, end: t.start });
            }
            pos = t.end;
        }
    }

    /// `ExtendClassForMixin`: the name (the last quoted part of the message) and
    /// the point after the type parameters or the name.
    pub(crate) fn extend_to_mixin(&self, error: Span, message: &str) -> Option<(String, usize)> {
        let node = self.tree.locate(error.start, error.end)?;
        let class = self.this_or_ancestor(node, |e| e == "ClassDeclaration")?;
        if !self.kind_children(class, "ExtendsClause").is_empty() {
            return None;
        }
        let end = message.rfind('\'')?;
        let start = message[..end].rfind('\'').map_or(0, |i| i + 1);
        let name = message[start..end].to_string();
        let dot = match self.kind_children(class, "TypeParameterList").first() {
            Some(&tp) => self.tree.nodes[tp].end,
            None => {
                let Mark::Decl(d) = self.tree.nodes[class].mark else { return None };
                match &self.ast.decl(d).kind {
                    ast::DeclKind::Class(c) => c.name.span.end,
                    _ => return None,
                }
            }
        };
        Some((name, dot))
    }

    /// `ReplaceWithExtensionName`: the name (with the prefix) and the target.
    pub(crate) fn extension_name(&self, error: Span) -> Option<(String, Span)> {
        let node = self.tree.locate(error.start, error.end)?;
        if self.kind(node) != "SimpleIdentifier" {
            return None;
        }
        let parent = self.parent(node)?;
        let target = match self.kind(parent) {
            "MethodInvocation" if self.method_name(parent) == Some(node) => *self.children(parent).first()?,
            "PropertyAccess" if self.children(parent).get(1) == Some(&node) => *self.children(parent).first()?,
            _ => return None,
        };
        if target == node {
            return None;
        }
        // The `ExtensionOverride`: `E(x)`, `E<T>(x)` or `p.E(x)`, with the name
        // resolved to the extension.
        let ids: Vec<usize> = self.children(target).iter().copied().filter(|&f| self.kind(f) == "SimpleIdentifier").collect();
        let extension = ids.iter().copied().find(|&i| match self.tree.nodes[i].mark {
            Mark::Expr(x) => matches!(self.bodies.get_resolved(x), Some(Resolved::Element(dartforge_elements::model::Element::Extension(_)))),
            _ => false,
        })?;
        let prefix = ids.iter().copied().find(|&i| i != extension && self.tree.nodes[i].end <= self.tree.nodes[extension].start);
        let name = match prefix {
            Some(p) => format!("{}.{}", self.node_text(p), self.node_text(extension)),
            None => self.node_text(extension).to_string(),
        };
        Some((name, self.tree.span(target)))
    }

    /// `RemoveParenthesesInGetterInvocation`: the argument list of the
    /// `FunctionExpressionInvocation` parent of the `coveringNode`.
    pub(crate) fn getter_parens(&self, error: Span) -> Option<Span> {
        let n = self.tree.locate2(error.start, error.end.saturating_sub(1))?;
        let parent = self.parent(n)?;
        if self.kind(parent) != "FunctionExpressionInvocation" {
            return None;
        }
        let args = self.kind_children(parent, "ArgumentList").first().copied()?;
        Some(self.tree.span(args))
    }

    /// `MakeSuperInvocationLast`: the deletion and the insertion `, <text>`.
    pub(crate) fn super_last(&self, error: Span) -> Option<Vec<(Span, String)>> {
        let node = self.tree.locate(error.start, error.end)?;
        if !matches!(self.kind(node), "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "ConstructorFieldInitializer" | "AssertInitializer") {
            return None;
        }
        let constructor = self.parent(node).filter(|&c| self.kind(c) == "ConstructorDeclaration")?;
        let initializers: Vec<usize> = self
            .children(constructor)
            .iter()
            .copied()
            .filter(|&f| matches!(self.kind(f), "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer"))
            .collect();
        let exclusion = self.node_in_list_with_comments(&initializers, node)?;
        let range = self.node_with_comments(node)?;
        let text = self.source[range.start..range.end].to_string();
        let (_, last_end_) = self.node_tokens(*initializers.last()?)?;
        let (t, _) = self.trailing_comment(last_end_, false);
        Some(vec![(exclusion, String::new()), (Span { start: t.end, end: t.end }, format!(", {text}"))])
    }

    /// `RemoveDeadCode` on a `SwitchMember` (the `coveringNode` of the
    /// `unreachable_switch_*` codes): `deletionRange(member)`, up to the `:`
    /// when the previous member has no statements.
    pub(crate) fn remove_dead_member(&self, error: Span) -> Option<Span> {
        let n = self.tree.locate2(error.start, error.end.saturating_sub(1))?;
        if !matches!(self.kind(n), "SwitchCase" | "SwitchDefault" | "SwitchPatternCase") {
            return None;
        }
        let switch = self.parent(n)?;
        if self.kind(switch) != "SwitchStatement" {
            return None;
        }
        let members: Vec<usize> = self.children(switch).iter().copied().filter(|&f| matches!(self.kind(f), "SwitchCase" | "SwitchDefault" | "SwitchPatternCase")).collect();
        let i = members.iter().position(|&m| m == n)?;
        let without_commands = |m: usize| !self.children(m).iter().any(|&f| self.is_command(f));
        let mut forced_end = None;
        if i > 0 && without_commands(members[i - 1]) {
            // The member's `:`: after the pattern, the guard or the `default`.
            let after = self
                .children(n)
                .iter()
                .copied()
                .filter(|&f| !self.is_command(f) && self.kind(f) != "Label")
                .map(|f| self.tree.nodes[f].end)
                .max()
                .unwrap_or(self.tree.nodes[n].start);
            let mut pos = after;
            loop {
                let t = self.next_token(pos)?;
                if self.lexeme(t) == ":" {
                    forced_end = Some(t);
                    break;
                }
                pos = t.end;
            }
        }
        self.exclusion_range(n, forced_end)
    }

    /// `RemoveComparison` (`remove_comparison.dart`): `is_true` for the
    /// `…_TRUE` codes, false for the `…_FALSE` ones.
    pub(crate) fn remove_comparison(&self, error: Span, is_true: bool) -> Option<Vec<(Span, String)>> {
        let node = self.tree.locate(error.start, error.end)?;
        let parent = self.parent(node)?;
        let tx = Text::new(self.source);
        let remove_indent = |text: &str| -> String {
            let eol = tx.eol();
            text
                .split(eol)
                .map(|l| l.strip_prefix(crate::refactorings_exec::ONE_INDENT).unwrap_or(l))
                .collect::<Vec<_>>()
                .join(eol)
        };
        let with_comments = |n: usize| -> Option<String> {
            let (start, end) = self.node_tokens(n)?;
            let first = self.comments_before(start).first().copied().unwrap_or(start);
            Some(self.source[first.start..end.end].to_string())
        };
        match self.kind(parent) {
            "AssertInitializer" if is_true => {
                let constructor = self.parent(parent)?;
                let list: Vec<usize> = self
                    .children(constructor)
                    .iter()
                    .copied()
                    .filter(|&f| matches!(self.kind(f), "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer"))
                    .collect();
                if list.len() == 1 {
                    let parameters = self.kind_children(constructor, "FormalParameterList").first().copied()?;
                    return Some(vec![(Span { start: self.tree.nodes[parameters].end, end: self.tree.nodes[parent].end }, String::new())]);
                }
                Some(vec![(self.node_in_list(&list, parent), String::new())])
            }
            "AssertStatement" if is_true => Some(vec![(self.node_lines(parent), String::new())]),
            "BinaryExpression" => {
                let f = self.children(parent);
                let (left, dir) = (*f.first()?, *f.get(1)?);
                let operator = self.next_token(self.tree.nodes[left].end)?;
                let is_and = self.lexeme(operator) == "&&";
                let or = self.lexeme(operator) == "||";
                if !((is_and && is_true) || (or && !is_true)) {
                    return None;
                }
                let s = if left == node {
                    Span { start: self.tree.nodes[node].start, end: self.tree.nodes[dir].start }
                } else {
                    Span { start: self.tree.nodes[left].end, end: self.tree.nodes[node].end }
                };
                Some(vec![(s, String::new())])
            }
            "IfElement" => {
                let branches: Vec<usize> = self.children(parent).iter().copied().filter(|&f| f != node).collect();
                let (then, else_) = (branches.first().copied(), branches.get(1).copied());
                if is_true {
                    let t = with_comments(then?)?;
                    return Some(vec![(self.tree.span(parent), remove_indent(&t))]);
                }
                match else_ {
                    Some(e) => {
                        let t = with_comments(e)?;
                        Some(vec![(self.tree.span(parent), remove_indent(&t))])
                    }
                    None => {
                        let collection = self.parent(parent)?;
                        if !matches!(self.kind(collection), "ListLiteral" | "SetOrMapLiteral") {
                            return None;
                        }
                        let elements: Vec<usize> = self.children(collection).iter().copied().filter(|&f| self.kind(f) != "TypeArgumentList").collect();
                        Some(vec![(self.node_in_list(&elements, parent), String::new())])
                    }
                }
            }
            "IfStatement" => {
                let branches: Vec<usize> = self.children(parent).iter().copied().filter(|&f| f != node).collect();
                let (then, else_) = (branches.first().copied(), branches.get(1).copied());
                let replace = |r: usize| -> Option<Vec<(Span, String)>> {
                    if self.kind(r) == "Block" {
                        let s = self.tree.span(r);
                        // The lines of the block's statements (the Dart 3.6.2 server, checked
                        // against the oracle, does not take the line break that follows the `{`);
                        // an empty block: what lies between the braces.
                        let commands = self.children(r);
                        let lines = match (commands.first(), commands.last()) {
                            (Some(&a), Some(&b)) => tx.line_range(self.tree.nodes[a].start, self.tree.nodes[b].end),
                            _ => tx.line_range(s.start + 1, s.end - 1),
                        };
                        let text = remove_indent(&self.source[lines.start..lines.end]);
                        return Some(vec![(self.node_lines(parent), text)]);
                    }
                    let t = with_comments(r)?;
                    Some(vec![(self.tree.span(parent), remove_indent(&t))])
                };
                if is_true {
                    return replace(then?);
                }
                match else_ {
                    Some(e) => replace(e),
                    None => {
                        let block = self.parent(parent).filter(|&b| self.kind(b) == "Block")?;
                        let commands: Vec<usize> = self.children(block).to_vec();
                        Some(vec![(self.node_in_list(&commands, parent), String::new())])
                    }
                }
            }
            _ => None,
        }
    }

    /// `MakeReturnTypeNullable`: the end of the return annotation (or of the
    /// type argument, in an async or generator body).
    pub(crate) fn nullable_return(&self, error: Span, strict_conversions: bool) -> Option<usize> {
        use dartforge_types::Type;
        let node = self.tree.locate(error.start, error.end)?;
        if !self.is_expression(node) {
            return None;
        }
        let ty = self.node_type(node)?;
        let body = self.this_or_ancestor(node, |e| e.ends_with("FunctionBody"))?;
        let mut function = self.parent(body)?;
        if self.kind(function) == "FunctionExpression" {
            function = self.parent(function)?;
        }
        if !matches!(self.kind(function), "MethodDeclaration" | "FunctionDeclaration") {
            return None;
        }
        let annotation = self
            .children(function)
            .iter()
            .copied()
            .find(|&f| matches!(self.kind(f), "NamedType" | "GenericFunctionType" | "RecordTypeAnnotation"))?;
        let element = self.declared_element(function);
        let table = &self.p.query.table;
        let mut declared = match element {
            crate::refactorings::Elem::Function(f) => self.p.query.outline.functions.get(f.0 as usize).map(|d| d.return_type)?,
            crate::refactorings::Elem::LocalFunction(fid) => match self.ast.function(fid).name.and_then(|n| self.bodies.local_type(n.span.start)).map(|t| table.get(t)) {
                Some(Type::Function { ret, .. }) => *ret,
                _ => return None,
            },
            _ => return None,
        };
        let mut target = annotation;
        let body_text = self.node_text(body);
        if body_text.starts_with("async") || body_text.starts_with("sync") {
            if self.kind(annotation) != "NamedType" {
                return None;
            }
            let args = self.kind_children(annotation, "TypeArgumentList").first().copied()?;
            let list = self.children(args);
            if list.len() != 1 {
                return None;
            }
            target = list[0];
            declared = match table.get(declared) {
                Type::Interface { args, .. } if args.len() == 1 => args[0],
                Type::FutureOr { arg, .. } => *arg,
                _ => return None,
            };
        }
        if self.kind(node) != "NullLiteral" {
            let mut t = table.clone();
            let non_null = dartforge_types::non_nullable(ty, &mut t);
            let assignable = if !strict_conversions && matches!(t.get(declared), Type::Dynamic) {
                true
            } else {
                let mut env = dartforge_types::SubtypeEnv::new(&mut t, &self.p.query.outline.hierarchy, &self.p.query.core);
                dartforge_types::is_subtype(declared, non_null, &mut env)
            };
            if !assignable {
                return None;
            }
        }
        Some(self.tree.nodes[target].end)
    }

    // -- RemoveUnusedParameter and AddConst ------------------------------------------

    /// The kind (`isRequiredPositional`) of a parameter node, through the parser
    /// parameter in its range.
    fn positional_required(&self, n: usize) -> bool {
        let s = self.tree.span(n);
        let inside = |q: &&ast::Parameter| q.span.start >= s.start && q.span.end <= s.end;
        let q = self
            .ast
            .functions
            .iter()
            .flat_map(|f| f.parameters.iter().flatten())
            .find(inside)
            .or_else(|| {
                self.ast.members.iter().find_map(|m| match &m.kind {
                    MemberKind::Constructor(k) => k.parameters.iter().find(inside),
                    _ => None,
                })
            });
        q.is_some_and(|q| q.kind == ast::ParameterKind::Required)
    }

    /// `RemoveUnusedParameter.compute`: the deletion.
    pub(crate) fn remove_parameter(&self, error: Span) -> Option<Span> {
        let mut maybe = self.tree.locate(error.start, error.end)?;
        if self.kind(maybe) == "SimpleIdentifier"
            && let Some(p) = self.parent(maybe)
        {
            maybe = p;
        }
        if !matches!(self.kind(maybe), "SimpleFormalParameter" | "FieldFormalParameter" | "SuperFormalParameter" | "FunctionTypedFormalParameter" | "DefaultFormalParameter") {
            return None;
        }
        let mut parameter = maybe;
        if let Some(p) = self.parent(parameter)
            && self.kind(p) == "DefaultFormalParameter"
        {
            parameter = p;
        }
        let list = self.parent(parameter).filter(|&l| self.kind(l) == "FormalParameterList")?;
        let parameters: Vec<usize> = self.children(list).to_vec();
        let i = parameters.iter().position(|&x| x == parameter)?;
        let s = |n: usize| self.tree.span(n);
        let ls = s(list);
        if i == 0 {
            if parameters.len() == 1 {
                return Some(Span { start: ls.start + 1, end: ls.end - 1 });
            }
            let next = parameters[1];
            if self.positional_required(parameter) && !self.positional_required(next) {
                // The `[`/`{` delimiter: the token before the first optional one.
                let delimiter = self.previous_token(s(next).start).filter(|t| matches!(&self.source[t.start..t.end], "[" | "{"));
                return Some(match delimiter {
                    Some(d) => Span { start: s(parameter).start, end: d.start },
                    None => Span { start: s(parameter).start, end: s(next).start },
                });
            }
            return Some(Span { start: s(parameter).start, end: s(next).start });
        }
        let previous = parameters[i - 1];
        if self.positional_required(previous) && !self.positional_required(parameter) {
            if i == parameters.len() - 1 {
                // `)` or the trailing comma before it.
                let closes = Span { start: ls.end - 1, end: ls.end };
                let before = self.previous_token(closes.start);
                let target = match before {
                    Some(a) if &self.source[a.start..a.end] == "," => a,
                    _ => closes,
                };
                return Some(Span { start: s(previous).end, end: target.start });
            }
            let next = parameters[i + 1];
            return Some(Span { start: s(parameter).start, end: s(next).start });
        }
        Some(Span { start: s(previous).end, end: s(parameter).end })
    }

    /// `AddConst.compute` on the paths that `non_constant_map_pattern_key`
    /// reaches (the key of the `MapPatternEntry`): binary or prefix →
    /// `const (…)` on the entry; list, set/map or creation without a keyword →
    /// `const ` and the inner `const`s deleted.
    pub(crate) fn add_const(&self, error: Span) -> Option<Vec<(Span, String)>> {
        let mut target = self.tree.locate(error.start, error.end)?;
        if self.kind(target) == "SimpleIdentifier" {
            target = self.parent(target)?;
        }
        if self.kind(target) == "ConstructorDeclaration" {
            let t = self.first_token_after_comment_and_metadata(target)?;
            return Some(vec![(Span { start: t.start, end: t.start }, "const ".to_string())]);
        }
        let parens_and_const = |n: usize| {
            let s = self.tree.span(n);
            vec![(Span { start: s.end, end: s.end }, ")".to_string()), (Span { start: s.start, end: s.start }, "const (".to_string())]
        };
        if self.kind(target) == "TypeArgumentList" {
            target = self.this_or_ancestor(target, |e| e == "ConstantPattern")?;
        }
        if self.kind(target) == "ConstantPattern" {
            // `canBeConst`/type literal: outside the published codes.
            return None;
        }
        if matches!(self.kind(target), "BinaryExpression" | "PrefixExpression") {
            let parent = self.parent(target)?;
            if let Some(grandparent) = self.parent(parent)
                && self.kind(grandparent) == "ParenthesizedPattern"
            {
                let o = self.tree.nodes[grandparent].start;
                return Some(vec![(Span { start: o, end: o }, "const ".to_string())]);
            }
            return Some(parens_and_const(parent));
        }
        let insert = |n: usize| -> Vec<(Span, String)> {
            let o = self.tree.nodes[n].start;
            let mut v = vec![(Span { start: o, end: o }, "const ".to_string())];
            // `_ConstRangeFinder`: the `const`s of inner creations and literals
            // (without entering closures).
            let mut stack = vec![n];
            while let Some(k) = stack.pop() {
                if self.kind(k) == "FunctionExpression" {
                    continue;
                }
                if matches!(self.kind(k), "InstanceCreationExpression" | "ListLiteral" | "SetOrMapLiteral")
                    && let Some(t) = self.next_token(self.tree.nodes[k].start)
                    && &self.source[t.start..t.end] == "const"
                    && let Some(next) = self.next_token(t.end)
                {
                    v.push((Span { start: t.start, end: next.start }, String::new()));
                }
                for &f in self.children(k).iter().rev() {
                    stack.push(f);
                }
            }
            v
        };
        if matches!(self.kind(target), "ListLiteral" | "SetOrMapLiteral") {
            return Some(insert(target));
        }
        if self.kind(target) == "NamedType" {
            target = self.parent(target)?;
        }
        if self.kind(target) == "ConstructorName" {
            target = self.parent(target)?;
        }
        if self.kind(target) == "InstanceCreationExpression" {
            let without_word = self.next_token(self.tree.nodes[target].start).is_some_and(|t| !matches!(&self.source[t.start..t.end], "new" | "const"));
            if without_word {
                return Some(insert(target));
            }
        }
        None
    }

    // -- AddMissingSwitchCases --------------------------------------------------------

    /// `AddMissingSwitchCases.compute` with `insertCaseClauseAtEnd`: the `node`
    /// is the `switch`; the witnesses with parts come from the constant checker
    /// (the `diagnostic.data`). The edit and the libraries to import.
    pub(crate) fn missing_cases(&self, error: Span) -> Option<(Span, String, std::collections::BTreeSet<dartforge_elements::model::LibraryId>)> {
        use dartforge_types::const_eval::WitnessPart as P;
        let node = self.tree.locate(error.start, error.end)?;
        let expression = match self.kind(node) {
            "SwitchStatement" => false,
            "SwitchExpression" => true,
            _ => return None,
        };
        let prog = self.p.program();
        let lib = prog.unit(self.unit).library;
        let query = &self.p.query;
        let mut table = query.table.clone();
        let map = dartforge_types::const_eval::switch_witnesses(
            prog,
            &query.names,
            &mut table,
            &query.core,
            &query.outline,
            &query.bodies,
            &self.p.libraries,
            lib,
        );
        let list = map.get(&(self.unit, self.tree.nodes[node].start))?;
        if list.is_empty() {
            return None;
        }
        let tx = Text::new(self.source);
        let eol = tx.eol();
        let line_indent = tx.line_prefix(self.tree.nodes[node].start).to_string();
        let one = crate::refactorings_exec::ONE_INDENT;
        // The `)`, `{` and `}` tokens of the `switch`.
        let s = self.tree.span(node);
        let closes = Span { start: s.end - 1, end: s.end };
        let scrutinee = *self.children(node).first()?;
        let paren = self.next_token(self.tree.nodes[scrutinee].end)?;
        let opens = self.next_token(paren.end)?;
        if &self.source[opens.start..opens.end] != "{" || &self.source[closes.start..closes.end] != "}" {
            return None;
        }
        let one_line = self.line(opens.start) == self.line(closes.start);
        let offset = if one_line { opens.end } else { tx.line_start(closes.start) };
        let mut writer = crate::write_type::Writer::new(self, offset);
        // A private enum value from another library: the witness goes away and the
        // `default` comes in.
        let inaccessible = |parts: &[P]| {
            parts.iter().any(|q| match q {
                P::EnumValue { enumeration, value } => {
                    let c = prog.class(*enumeration);
                    (self.p.name(c.name).starts_with('_') || self.p.name(prog.variable(*value).name).starts_with('_')) && c.library != lib
                }
                _ => false,
            })
        };
        let mut text = String::new();
        if one_line {
            text.push_str(eol);
        }
        let mut needs_default = false;
        for parts in list {
            if inaccessible(parts) {
                needs_default = true;
                continue;
            }
            let mut pattern = String::new();
            for q in parts {
                match q {
                    P::Text(t) => pattern.push_str(t),
                    P::EnumValue { enumeration, value } => {
                        let name = self.p.name(prog.class(*enumeration).name).to_string();
                        pattern.push_str(&writer.reference_of(dartforge_elements::model::Element::Class(*enumeration), &name));
                        pattern.push('.');
                        pattern.push_str(self.p.name(prog.variable(*value).name));
                    }
                    P::Type(t) => pattern.push_str(&writer.write_type(Some(*t), false).unwrap_or_default()),
                }
            }
            if expression {
                text.push_str(&format!("{line_indent}{one}// TODO: Handle this case.{eol}{line_indent}{one}{pattern} => throw UnimplementedError(),{eol}"));
            } else {
                text.push_str(&format!(
                    "{line_indent}{one}case {pattern}:{eol}{line_indent}{one}{one}// TODO: Handle this case.{eol}{line_indent}{one}{one}throw UnimplementedError();{eol}"
                ));
            }
        }
        if needs_default {
            if expression {
                text.push_str(&format!("{line_indent}{one}// TODO: Handle this case.{eol}{line_indent}{one}_ => throw UnimplementedError(),{eol}"));
            } else {
                text.push_str(&format!(
                    "{line_indent}{one}default:{eol}{line_indent}{one}{one}// TODO: Handle this case.{eol}{line_indent}{one}{one}throw UnimplementedError();{eol}"
                ));
            }
        }
        if one_line {
            // `linePrefix(switchKeyword.offset)`.
            text.push_str(&line_indent);
        }
        Some((Span { start: offset, end: offset }, text, writer.import))
    }

    // -- AddMissingRequiredArgument -------------------------------------------------

    /// The type is Flutter's `Widget` or a subtype (`isWidgetType`).
    fn is_widget(&self, t: dartforge_types::TypeId) -> bool {
        let prog = self.p.program();
        let dartforge_types::Type::Interface { class, .. } = self.p.query.table.get(t) else { return false };
        let widget = |c: dartforge_elements::model::ClassId| {
            let k = prog.class(c);
            self.p.name(k.name) == "Widget" && prog.library(k.library).uri == "package:flutter/src/widgets/framework.dart"
        };
        widget(*class) || self.p.supertypes(*class).into_iter().any(widget)
    }

    /// `getDefaultStringParameterValue2(parameter, quote)`: the text and the
    /// cursor position.
    fn argument_default_value(&self, ty: dartforge_types::TypeId, quotes: &str, annotation: Option<(&ast::Ast, ast::TypeId)>) -> Option<String> {
        use dartforge_types::Type;
        let core = &self.p.query.core;
        let table = &self.p.query.table;
        match table.get(ty) {
            Type::Interface { class, .. } if Some(*class) == core.list_class => Some("[]".to_string()),
            Type::Interface { class, .. } if Some(*class) == core.map_class => Some("{}".to_string()),
            Type::Interface { class, .. } if Some(*class) == core.string_class => Some(format!("{quotes}{quotes}")),
            Type::Function { positional, optional, named, .. } => {
                // The parameter names come from the function type annotation.
                let names: Vec<String> = match annotation.map(|(a, t)| &a.ty(t).kind) {
                    Some(ast::TypeKind::Function { parameters, .. }) => parameters.iter().map(|q| q.name.map(|n| self.p.name(n.sym).to_string()).unwrap_or_default()).collect(),
                    _ => Vec::new(),
                };
                let mut parts = Vec::new();
                let mut i = 0usize;
                for &q in positional.iter().chain(optional.iter()) {
                    let ty = if matches!(table.get(q), Type::Dynamic) { String::new() } else { format!("{} ", self.p.query.format(q)) };
                    parts.push(format!("{ty}{}", names.get(i).cloned().unwrap_or_default()));
                    i += 1;
                }
                for (n, q, _) in named.iter() {
                    let ty = if matches!(table.get(*q), Type::Dynamic) { String::new() } else { format!("{} ", self.p.query.format(*q)) };
                    parts.push(format!("{ty}{}", self.p.name(*n)));
                }
                Some(format!("({}) {{  }}", parts.join(", ")))
            }
            _ => None,
        }
    }

    /// `AddMissingRequiredArgument` for each required named parameter without an
    /// argument in the calls whose name is on lines `start..end` (the
    /// `missing_required_argument` code is not published: the calls are
    /// examined on request). The parameter name, the insertion and where the
    /// cursor goes: `Ok(k)`, the selection at byte `k` of the default value with
    /// `cursorPosition`; `Err((k, n))`, the linked group `VALUE` of the value
    /// without a cursor (`add_missing_required_argument.dart:105-121`).
    pub(crate) fn required_arguments(&self, start: usize, end: usize) -> Vec<RequiredArgument> {
        use dartforge_types::Resolved;
        let prog = self.p.program();
        let quotes = if crate::refactorings_exec::enabled_rule(self.p, self.unit, "prefer_double_quotes") { "\"" } else { "'" };
        let mut output = Vec::new();
        for (n, node) in self.tree.nodes.iter().enumerate() {
            let (error_target, function_, list, creation) = match node.kind {
                "MethodInvocation" => {
                    let Some(m) = self.method_name(n) else { continue };
                    let crate::refactorings::Elem::Function(f) = self.identifier_element(m, true) else { continue };
                    let Some(l) = self.kind_children(n, "ArgumentList").first().copied() else { continue };
                    (m, f, l, None)
                }
                "InstanceCreationExpression" => {
                    let Some(cn) = self.kind_children(n, "ConstructorName").first().copied() else { continue };
                    let Some(x) = self.node_expr(n) else { continue };
                    let f = match self.bodies.get_resolved(x) {
                        Some(Resolved::Constructor(f)) => *f,
                        _ => match self.constructors_by_target.iter().find(|(target, _)| self.ast.expr(**target).span.start == node.start) {
                            Some((_, f)) => *f,
                            None => continue,
                        },
                    };
                    let Some(l) = self.kind_children(n, "ArgumentList").first().copied() else { continue };
                    (cn, f, l, Some(n))
                }
                _ => continue,
            };
            let s = self.tree.span(error_target);
            if !(s.start <= end && start <= s.end) {
                continue;
            }
            let Some(data) = self.p.query.outline.functions.get(function_.0 as usize) else { continue };
            let arguments: Vec<usize> = self.children(list).to_vec();
            let named_data: Vec<String> = arguments
                .iter()
                .filter(|&&a| self.kind(a) == "NamedExpression")
                .filter_map(|&a| self.children(a).first().and_then(|&r| self.children(r).first()).map(|&id| self.node_text(id).to_string()))
                .collect();
            // The annotations of the declared parameters (for the names of a function
            // type).
            let (ast_decl, declared): (Option<&ast::Ast>, Vec<&ast::Parameter>) = match prog.function(function_).node {
                dartforge_elements::model::FunctionRef::Function { unit, function } => {
                    (Some(&prog.unit(unit).ast), prog.unit(unit).ast.function(function).parameters.iter().flatten().collect())
                }
                dartforge_elements::model::FunctionRef::Constructor { unit, member } => match &prog.unit(unit).ast.member(member).kind {
                    MemberKind::Constructor(k) => (Some(&prog.unit(unit).ast), k.parameters.iter().collect()),
                    _ => (None, Vec::new()),
                },
                _ => (None, Vec::new()),
            };
            let widget = creation.and_then(|c| self.node_type(c)).is_some_and(|t| self.is_widget(t));
            for q in data.parameters.iter() {
                if !(q.required && q.kind == ast::ParameterKind::Named) {
                    continue;
                }
                let Some(name) = q.external_name.or(q.name).map(|s| self.p.name(s).to_string()) else { continue };
                if named_data.contains(&name) {
                    continue;
                }
                let list_s = self.tree.span(list);
                let (mut offset, mut trailing_comma, mut between) = (list_s.start + 1, false, false);
                if let Some(&last) = arguments.last() {
                    offset = self.tree.nodes[last].end;
                    trailing_comma = self.next_token(offset).is_some_and(|t| &self.source[t.start..t.end] == ",");
                    if self.kind(last) == "NamedExpression" && widget {
                        let label = self.children(last).first().and_then(|&r| self.children(r).first()).map(|&id| self.node_text(id).to_string());
                        if matches!(label.as_deref(), Some("child") | Some("children")) {
                            offset = self.tree.nodes[last].start;
                            trailing_comma = true;
                            between = true;
                        }
                    }
                }
                let annotation = declared.iter().find(|p| p.name.is_some_and(|n| self.p.name(n.sym) == name)).and_then(|p| p.ty).and_then(|t| ast_decl.map(|a| (a, t)));
                let pattern = self.argument_default_value(q.ty, quotes, annotation);
                let mut text = String::new();
                if !arguments.is_empty() && !between {
                    text.push_str(", ");
                }
                text.push_str(&format!("{name}: "));
                // `DefaultArgument.cursorPosition`: 1 in `[]`, `{}` and `''`; between the
                // braces of `(…) {  }`.
                let cursor = match &pattern {
                    Some(v) if v.starts_with('(') => Ok(text.len() + v.len() - 2),
                    Some(_) => Ok(text.len() + 1),
                    None => Err((text.len(), 4)),
                };
                text.push_str(pattern.as_deref().unwrap_or("null"));
                if widget {
                    if !trailing_comma {
                        text.push(',');
                    } else if between {
                        let eol = Text::new(self.source).eol();
                        text.push(',');
                        text.push_str(eol);
                        text.push_str(Text::new(self.source).line_prefix(offset));
                    }
                }
                output.push((name, Span { start: offset, end: offset }, text, cursor));
            }
        }
        output
    }

    // -- ConvertIntoBlockBody.missingBody -----------------------------------------

    /// `ConvertIntoBlockBody._computeMissingBody` with `node` on the error: the
    /// enclosing body (`getEnclosingFunctionBody`), empty or an expression,
    /// becomes a block (`// TODO: implement x` and `throw
    /// UnimplementedError();`, or `[return ]e;`).
    pub(crate) fn convert_to_block_body(&self, error: Span) -> Option<(Span, String)> {
        let node = self.tree.locate(error.start, error.end)?;
        // The enclosing closure, function, constructor or method.
        let owner = ["FunctionExpression", "FunctionDeclaration", "ConstructorDeclaration", "MethodDeclaration"]
            .iter()
            .find_map(|e| self.this_or_ancestor(node, |x| x == *e))?;
        let body_of = |n: usize| self.children(n).iter().copied().rev().find(|&f| self.kind(f).ends_with("FunctionBody"));
        let body = match self.kind(owner) {
            "FunctionDeclaration" => {
                let expr = self.children(owner).iter().copied().find(|&f| self.kind(f) == "FunctionExpression")?;
                body_of(expr)?
            }
            _ => body_of(owner)?,
        };
        let body_text = self.node_text(body);
        if body_text.starts_with("sync*") || body_text.starts_with("async*") {
            return None;
        }
        // `_getFunctionElement(body.parent)`: method, constructor or function
        // expression.
        let parent = self.parent(body)?;
        let element = match self.kind(parent) {
            "MethodDeclaration" | "FunctionExpression" | "ConstructorDeclaration" => self.body_element(parent)?,
            _ => return None,
        };
        let lines: Vec<String> = match self.kind(body) {
            "EmptyFunctionBody" => {
                let mut l = vec![format!("// TODO: implement {}", self.displayed_name(parent)?)];
                if !self.returns_void(element) {
                    l.push("throw UnimplementedError();".to_string());
                }
                l
            }
            "ExpressionFunctionBody" => {
                let expr = *self.children(body).first()?;
                if error.start >= self.tree.nodes[expr].start {
                    return None;
                }
                let ty = self.node_type(expr).map(|t| self.p.query.table.get(t).clone());
                let void_or_bottom = matches!(ty, Some(dartforge_types::Type::Void) | Some(dartforge_types::Type::Never));
                let return_ = if !void_or_bottom && !self.returns_void(element) { "return " } else { "" };
                vec![format!("{return_}{};", self.node_text(expr))]
            }
            _ => return None,
        };
        let tx = Text::new(self.source);
        let prefix = self.node_prefix(parent);
        let eol = tx.eol();
        let previous = self.previous_token(self.tree.nodes[body].start)?;
        let mut s = String::from(" ");
        if body_text.starts_with("async") {
            s.push_str("async ");
        }
        s.push('{');
        for l in &lines {
            s.push_str(eol);
            s.push_str(&prefix);
            s.push_str(crate::refactorings_exec::ONE_INDENT);
            s.push_str(l);
        }
        s.push_str(eol);
        s.push_str(&prefix);
        s.push('}');
        Some((Span { start: previous.end, end: self.tree.nodes[body].end }, s))
    }

    /// The executable element of a method, constructor or function expression.
    fn body_element(&self, n: usize) -> Option<crate::refactorings::Elem> {
        use crate::refactorings::Elem;
        match (self.kind(n), self.tree.nodes[n].mark) {
            ("MethodDeclaration", _) => Some(self.declared_element(n)).filter(|e| *e != Elem::None),
            ("FunctionExpression", Mark::Function(fid)) => Some(match self.p.node_function(self.unit, fid) {
                Some(f) => Elem::Function(f),
                None => Elem::LocalFunction(fid),
            }),
            ("FunctionExpression", _) => {
                // The expression of a `FunctionDeclaration`: the mark is on the parent.
                let parent = self.parent(n)?;
                Some(self.declared_element(parent)).filter(|e| *e != Elem::None)
            }
            ("ConstructorDeclaration", _) => {
                let s = self.tree.span(n);
                let (mi, _) = self.ast.members.iter().enumerate().find(|(_, m)| matches!(m.kind, MemberKind::Constructor(_)) && m.span.start >= s.start && m.span.end <= s.end)?;
                self.p.node_constructor(self.unit, ast::MemberId(mi as u32)).map(Elem::Function)
            }
            _ => None,
        }
    }

    /// The executable's `displayName` (the name without `=`; a constructor is
    /// the class's with `.name`).
    fn displayed_name(&self, n: usize) -> Option<String> {
        use crate::refactorings::Elem;
        match self.body_element(n)? {
            Elem::Function(f) => {
                let fe = self.p.program().function(f);
                // The `displayName` of the new element (`ElementImpl2`): the fragment's
                // name, without the class for a constructor (empty for an unnamed one;
                // checked against the Dart 3.6.2 server).
                Some(crate::project::base_name(self.p.name(fe.name)).to_string())
            }
            Elem::LocalFunction(fid) => Some(self.ast.function(fid).name.map(|x| self.lexeme(x.span).to_string()).unwrap_or_default()),
            _ => None,
        }
    }

    /// `_forFieldFormalParameter`.
    fn field_parameter_range(&self, node: usize) -> Option<Span> {
        let parameter = match self.parent(node) {
            Some(p) if self.kind(p) == "DefaultFormalParameter" => p,
            _ => node,
        };
        let list = self.parent(parameter)?;
        let s = self.tree.span(parameter);
        let parameters: Vec<usize> = self.children(list).to_vec();
        if parameters.len() == 1 {
            let ls = self.tree.span(list);
            return Some(Span { start: ls.start + 1, end: ls.end - 1 });
        }
        let mut previous = self.previous_token(s.start)?;
        let mut next = self.next_token(s.end)?;
        if self.lexeme(next) == "," {
            next = self.next_token(next.end)?;
            return Some(Span { start: s.start, end: next.start });
        }
        let first_optional = matches!(self.lexeme(previous), "{" | "[");
        if first_optional {
            previous = self.previous_token(previous.start)?;
            if matches!(self.lexeme(next), "}" | "]") {
                next = self.next_token(next.end)?;
            }
        }
        let before = self.previous_token(previous.start)?;
        Some(Span { start: before.end, end: next.start })
    }
}
