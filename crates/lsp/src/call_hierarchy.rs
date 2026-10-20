//! Call hierarchy (`textDocument/prepareCallHierarchy`,
//! `callHierarchy/incomingCalls`, `callHierarchy/outgoingCalls`) like the
//! Dart 3.6.2 `DartCallHierarchyComputer`
//! (`AS:src/computer/computer_call_hierarchy.dart`, read in full;
//! docs/LSP-ESPECIFICACAO.md §12) over the analyzer tree:
//!
//! * `_findTargetNode`: the `NodeLocator` node at the offset; a
//!   `SimpleIdentifier` whose parent is neither a `VariableDeclaration` nor
//!   an assignment walks up to the parent (`x` in `x.foo()` gives `foo`; `a`
//!   in `a == b`, the `==`); the type name before the constructor name is
//!   not a target;
//! * `_getElementOfNode`: the constructor of a `NamedType`/`ConstructorName`,
//!   the property of a `PropertyAccess`, else the `ElementLocator`; a
//!   synthetic accessor is not a target;
//! * the item of an element (`CallHierarchyItem.forElement`): the displayed
//!   name (`get x`, `set x`, the file name), the kind, the container, the
//!   code (with the comment and annotations) and the name;
//! * incoming: the element's references grouped by container;
//! * outgoing: the `_OutboundCallVisitor` on the target's body, grouped by
//!   the called element.
//!
//! Written without compiling or running (2026-10-05).

use crate::analyzer_tree::Mark;
use crate::project::{Project, Target};
use crate::refactorings::Context;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{
    ClassId, ClassKind, Element, ExtensionId, FunctionElementId, FunctionKind, UnitId,
};
use dartforge_frontend::ast;
use dartforge_types::{MemberRef, Resolved};

/// `CallHierarchyKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallKind {
    Class,
    Constructor,
    Extension,
    File,
    Function,
    Method,
    Mixin,
    Property,
    Unknown,
}

impl CallKind {
    /// `toSymbolKindMapping` (without the client fallback).
    pub fn symbol(self) -> Option<u8> {
        Some(match self {
            CallKind::Class | CallKind::Extension | CallKind::Mixin => 5,
            CallKind::Constructor => 9,
            CallKind::File => 1,
            CallKind::Function => 12,
            CallKind::Method => 6,
            CallKind::Property => 7,
            CallKind::Unknown => return None,
        })
    }
}

/// An item of the call hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallItem {
    pub name: String,
    pub kind: CallKind,
    pub detail: Option<String>,
    pub uri: String,
    /// The code (with the documentation comment and the annotations).
    pub range: Span,
    /// The name.
    pub selection: Span,
}

/// An element of the hierarchy (one that has an item).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum CallElem {
    Function(FunctionElementId),
    /// Local function, by unit and parser function.
    Local(UnitId, u32),
    Class(ClassId),
    Extension(ExtensionId),
    File(UnitId),
}

impl Project {
    /// The `ElementKind` of an element, as a `CallHierarchyKind`.
    fn call_kind(&self, e: CallElem) -> CallKind {
        let p = self.program();
        match e {
            CallElem::Function(f) => match p.function(f).kind {
                FunctionKind::Constructor | FunctionKind::SyntheticConstructor => {
                    CallKind::Constructor
                }
                FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => {
                    CallKind::Property
                }
                _ if p.function(f).class.is_none() && p.function(f).extension.is_none() => {
                    CallKind::Function
                }
                _ => CallKind::Method,
            },
            CallElem::Local(..) => CallKind::Function,
            CallElem::Class(c) => match p.class(c).kind {
                ClassKind::Mixin => CallKind::Mixin,
                ClassKind::Enum | ClassKind::ExtensionType => CallKind::Unknown,
                _ => CallKind::Class,
            },
            CallElem::Extension(_) => CallKind::Extension,
            CallElem::File(_) => CallKind::File,
        }
    }

    /// `_getDisplayName`.
    fn displayed_name(&self, e: CallElem) -> String {
        let p = self.program();
        match e {
            CallElem::Function(f) => {
                let fe = p.function(f);
                let base = crate::project::base_name(self.name(fe.name)).to_string();
                match fe.kind {
                    FunctionKind::Getter => format!("get {base}"),
                    FunctionKind::Setter => format!("set {base}"),
                    FunctionKind::ImplicitAccessor => {
                        if self.name(fe.name).ends_with('=') {
                            format!("set {base}")
                        } else {
                            format!("get {base}")
                        }
                    }
                    FunctionKind::Constructor | FunctionKind::SyntheticConstructor => {
                        let class = fe
                            .class
                            .map_or(String::new(), |c| self.name(p.class(c).name).to_string());
                        if base.is_empty() {
                            class
                        } else {
                            format!("{class}.{base}")
                        }
                    }
                    _ => base,
                }
            }
            CallElem::Local(u, f) => {
                let a = &p.unit(u).ast;
                a.function(ast::FunctionId(f))
                    .name
                    .map(|n| self.name(n.sym).to_string())
                    .unwrap_or_default()
            }
            CallElem::Class(c) => self.name(p.class(c).name).to_string(),
            CallElem::Extension(x) => p
                .extension(x)
                .name
                .map(|n| self.name(n).to_string())
                .unwrap_or_default(),
            CallElem::File(u) => p
                .unit(u)
                .path
                .as_ref()
                .and_then(|c| c.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        }
    }

    /// `_getContainer(element.enclosingElement)`: the owning class, extension
    /// or mixin; otherwise the file.
    fn element_container(&self, e: CallElem) -> Option<CallElem> {
        let p = self.program();
        match e {
            CallElem::Function(f) => {
                let fe = p.function(f);
                if let Some(c) = fe.class {
                    return Some(CallElem::Class(c));
                }
                if let Some(x) = fe.extension {
                    return Some(CallElem::Extension(x));
                }
                self.function_name(f).map(|(u, _)| CallElem::File(u))
            }
            CallElem::Local(u, f) => {
                // The function or executable that contains the local function.
                let s = p.unit(u).ast.function(ast::FunctionId(f)).span;
                Some(self.container_at_offset(u, s.start, Some(f)))
            }
            CallElem::Class(c) => p.class(c).decl.map(|d| CallElem::File(d.unit)),
            CallElem::Extension(x) => Some(CallElem::File(p.extension(x).decl.unit)),
            CallElem::File(_) => None,
        }
    }

    /// The name and the code of an element (`nonSynthetic`).
    fn ranges(&self, e: CallElem) -> Option<(UnitId, Span, Span)> {
        let p = self.program();
        match e {
            CallElem::Function(f) => {
                let fe = p.function(f);
                if fe.kind == FunctionKind::SyntheticConstructor {
                    return self.ranges(CallElem::Class(fe.class?));
                }
                let (u, name) = self.function_name(f)?;
                let cx = Context::new(self, u);
                let node = cx.tree.locate(name.start, name.end)?;
                let decl = cx.tree.chain(node).into_iter().find(|&k| {
                    matches!(
                        cx.kind(k),
                        "FunctionDeclaration"
                            | "MethodDeclaration"
                            | "ConstructorDeclaration"
                            | "VariableDeclaration"
                    )
                })?;
                Some((u, name, cx.tree.span(decl)))
            }
            CallElem::Local(u, f) => {
                let name = p.unit(u).ast.function(ast::FunctionId(f)).name?.span;
                let cx = Context::new(self, u);
                let node = cx.tree.locate(name.start, name.end)?;
                let decl = cx
                    .tree
                    .chain(node)
                    .into_iter()
                    .find(|&k| cx.kind(k) == "FunctionDeclaration")?;
                Some((u, name, cx.tree.span(decl)))
            }
            CallElem::Class(c) => {
                let (u, name) = self.top_element_name(Element::Class(c))?;
                let d = p.class(c).decl?;
                let cx = Context::new(self, u);
                let node = cx.tree.nodes.iter().position(|n| {
                    matches!(n.mark, Mark::Decl(x) if x == d.decl)
                        && n.kind.ends_with("Declaration")
                })?;
                Some((u, name, cx.tree.span(node)))
            }
            CallElem::Extension(x) => {
                let d = p.extension(x).decl;
                let cx = Context::new(self, d.unit);
                let node = cx.tree.nodes.iter().position(|n| {
                    matches!(n.mark, Mark::Decl(k) if k == d.decl)
                        && n.kind == "ExtensionDeclaration"
                })?;
                let name = match &p.unit(d.unit).ast.decl(d.decl).kind {
                    ast::DeclKind::Extension(e) => e.name.map_or(
                        Span {
                            start: cx.tree.nodes[node].start,
                            end: cx.tree.nodes[node].start,
                        },
                        |n| n.span,
                    ),
                    _ => return None,
                };
                Some((d.unit, name, cx.tree.span(node)))
            }
            CallElem::File(u) => Some((
                u,
                Span { start: 0, end: 0 },
                Span {
                    start: 0,
                    end: p.unit(u).source.len(),
                },
            )),
        }
    }

    /// `CallHierarchyItem.forElement`.
    pub(crate) fn call_item(&self, e: CallElem) -> Option<CallItem> {
        let (u, selection, range) = self.ranges(e)?;
        let detail = self.element_container(e).map(|c| self.displayed_name(c));
        Some(CallItem {
            name: self.displayed_name(e),
            kind: self.call_kind(e),
            detail,
            uri: self.unit_uri(u)?,
            range,
            selection,
        })
    }

    /// The resolved element of a `SimpleIdentifier` of the tree.
    fn call_identifier_element(&self, cx: &Context<'_>, n: usize) -> Option<CallElem> {
        let x = match cx.tree.nodes[n].mark {
            Mark::Expr(x) => x,
            _ => return None,
        };
        if let Some(&f) = cx.constructors_by_target.get(&x) {
            return Some(CallElem::Function(f));
        }
        if let Some(d) = cx.bodies.local_declaration(x)
            && let Some(i) = cx
                .ast
                .functions
                .iter()
                .position(|f| f.name.is_some_and(|nm| nm.span.start == d))
        {
            return Some(CallElem::Local(cx.unit, i as u32));
        }
        self.resolution_element(cx.bodies.get_resolved(x)?)
    }

    fn resolution_element(&self, r: &Resolved) -> Option<CallElem> {
        Some(match r {
            Resolved::Element(Element::Function(f))
            | Resolved::Member {
                member: MemberRef::Function(f),
                ..
            }
            | Resolved::ExtensionMember { member: f, .. }
            | Resolved::Constructor(f) => CallElem::Function(*f),
            Resolved::Element(Element::Class(c)) => CallElem::Class(*c),
            Resolved::Element(Element::Extension(x)) => CallElem::Extension(*x),
            _ => return None,
        })
    }

    /// The `==` of `a == b` is `Object`'s in the inference; the analyzer's
    /// `staticElement` is the receiver interface's (`lookUpMethod2`): `num.==`
    /// for an `int`, `String.==` for a `String`.
    fn specific_equality(
        &self,
        cx: &Context<'_>,
        x: dartforge_frontend::ast::ExprId,
        e: CallElem,
    ) -> CallElem {
        let p = self.program();
        let CallElem::Function(f) = e else { return e };
        let fe = p.function(f);
        if self.name(fe.name) != "=="
            || fe.class.is_none()
            || fe.class != self.query.core.object_class
        {
            return e;
        }
        let ast::ExprKind::Binary { left, .. } = &cx.ast.expr(x).kind else {
            return e;
        };
        let Some(t) = cx.bodies.get_type(*left) else {
            return e;
        };
        let dartforge_types::Type::Interface { class, .. } = self.query.table.get(t) else {
            return e;
        };
        let mut queue = std::collections::VecDeque::from([*class]);
        let mut seen = std::collections::HashSet::new();
        while let Some(c) = queue.pop_front() {
            if !seen.insert(c) || Some(c) == self.query.core.object_class {
                continue;
            }
            let cl = p.class(c);
            if let Some(&g) = cl.instance_members.get(&fe.name) {
                return CallElem::Function(g);
            }
            queue.extend(cl.mixin_classes.iter().rev().copied());
            queue.extend(cl.supertype_class);
            queue.extend(cl.interface_classes.iter().copied());
            queue.extend(cl.on_classes.iter().copied());
        }
        e
    }

    /// `ElementLocator.locate(node)` for the kinds the hierarchy uses.
    fn locate_element(&self, cx: &Context<'_>, n: usize) -> Option<CallElem> {
        match cx.kind(n) {
            // `annotation.element`: the constructor of `@A(…)`/`@A.n(…)`.
            "Annotation" => match self.identify(cx.unit, cx.tree.nodes[n].start + 1).ok()?? {
                crate::project::Denoted {
                    target: Target::Constructor(f),
                    ..
                } => Some(CallElem::Function(f)),
                _ => None,
            },
            "SimpleIdentifier" => self.call_identifier_element(cx, n),
            "MethodInvocation" => self.call_identifier_element(cx, cx.method_name(n)?),
            "PrefixedIdentifier" => self.call_identifier_element(cx, *cx.children(n).get(1)?),
            "FunctionDeclaration" | "MethodDeclaration" => match cx.tree.nodes[n].mark {
                Mark::Function(fid) => {
                    if cx
                        .parent(n)
                        .is_some_and(|p| cx.kind(p) == "FunctionDeclarationStatement")
                    {
                        return Some(CallElem::Local(cx.unit, fid.0));
                    }
                    self.node_function(cx.unit, fid).map(CallElem::Function)
                }
                _ => None,
            },
            "ConstructorDeclaration" => {
                let s = cx.tree.span(n);
                let (mi, _) = cx.ast.members.iter().enumerate().find(|(_, m)| {
                    matches!(m.kind, ast::MemberKind::Constructor(_))
                        && m.span.start >= s.start
                        && m.span.end <= s.end
                })?;
                self.node_constructor(cx.unit, ast::MemberId(mi as u32))
                    .map(CallElem::Function)
            }
            "ClassDeclaration"
            | "MixinDeclaration"
            | "EnumDeclaration"
            | "ExtensionTypeDeclaration" => match cx.tree.nodes[n].mark {
                Mark::Decl(d) => cx.declaration_class(cx.unit, d).map(CallElem::Class),
                _ => None,
            },
            "ExtensionDeclaration" => match cx.tree.nodes[n].mark {
                Mark::Decl(d) => {
                    let p = self.program();
                    (0..p.extensions.len())
                        .map(|i| ExtensionId(i as u32))
                        .find(|&x| {
                            p.extension(x).decl.unit == cx.unit && p.extension(x).decl.decl == d
                        })
                        .map(CallElem::Extension)
                }
                _ => None,
            },
            "InstanceCreationExpression"
            | "BinaryExpression"
            | "PrefixExpression"
            | "PostfixExpression"
            | "IndexExpression"
            | "AssignmentExpression"
            | "FunctionExpressionInvocation" => {
                let x = match cx.tree.nodes[n].mark {
                    Mark::Operator(x) => x,
                    _ => cx.node_expr(n)?,
                };
                let e = self.resolution_element(cx.bodies.get_resolved(x)?)?;
                Some(self.specific_equality(cx, x, e))
            }
            _ => None,
        }
    }

    /// `_findTargetNode(offset)`.
    fn at_target(&self, cx: &Context<'_>, offset: usize) -> Option<usize> {
        let mut n = cx.tree.locate(offset, offset)?;
        if cx.kind(n) == "SimpleIdentifier"
            && let Some(p) = cx.parent(n)
            && !matches!(cx.kind(p), "VariableDeclaration" | "AssignmentExpression")
        {
            n = p;
        }
        match cx.kind(n) {
            "NamedType" => {
                if let Some(p) = cx.parent(n)
                    && cx.kind(p) == "ConstructorName"
                    && let Some(&name) = cx.children(p).get(1)
                    && offset < cx.tree.nodes[name].start
                {
                    return None;
                }
            }
            "ConstructorDeclaration" => {
                let s = cx.tree.span(n);
                let name = cx.ast.members.iter().find_map(|m| match &m.kind {
                    ast::MemberKind::Constructor(k)
                        if m.span.start >= s.start && m.span.end <= s.end =>
                    {
                        k.name
                    }
                    _ => None,
                });
                if name.is_some_and(|nm| offset < nm.span.start) {
                    return None;
                }
            }
            _ => {}
        }
        Some(n)
    }

    /// `_getElementOfNode(node)`: without the synthetic accessor.
    fn call_node_element(&self, cx: &Context<'_>, n: usize) -> Option<CallElem> {
        let parent = cx.parent(n);
        let e = match cx.kind(n) {
            "NamedType" if parent.is_some_and(|p| cx.kind(p) == "ConstructorName") => {
                let creation = cx.parent(parent?)?;
                self.locate_element(cx, creation)
            }
            "ConstructorName" => self.locate_element(cx, parent?),
            "PropertyAccess" => self.locate_element(cx, *cx.children(n).last()?),
            _ => self.locate_element(cx, n),
        }?;
        if let CallElem::Function(f) = e
            && self.program().function(f).kind == FunctionKind::ImplicitAccessor
        {
            return None;
        }
        Some(e)
    }

    /// `findTarget(offset)`: executables only.
    pub(crate) fn call_target(&self, unit: UnitId, offset: usize) -> Option<CallElem> {
        let cx = Context::new(self, unit);
        let n = self.at_target(&cx, offset)?;
        let e = self.call_node_element(&cx, n)?;
        matches!(e, CallElem::Function(_) | CallElem::Local(..)).then_some(e)
    }

    /// The target of the client's item (`toServerItem` + the node on the name +
    /// `_isMatchingElement` by displayed name); a class with constructor kind is
    /// the implicit unnamed constructor. With the node, for the outgoing calls.
    pub(crate) fn item_target(
        &self,
        unit: UnitId,
        offset: usize,
        name: &str,
        constructor: bool,
    ) -> Option<(CallElem, &'static str)> {
        let cx = Context::new(self, unit);
        let n = self.at_target(&cx, offset)?;
        let kind = cx.kind(n);
        let mut e = self.call_node_element(&cx, n)?;
        if self.displayed_name(e) != name {
            return None;
        }
        if let CallElem::Class(c) = e
            && constructor
        {
            let p = self.program();
            let unnamed = p
                .class(c)
                .constructors
                .iter()
                .find(|(s, _)| self.name(**s).is_empty())
                .map(|(_, f)| *f)?;
            e = CallElem::Function(unnamed);
        }
        Some((e, kind))
    }

    /// The index container for a reference at `offset` of `u`: the innermost
    /// named executable (function, method, constructor, local function), the
    /// class, the extension, or the file.
    fn container_at_offset(&self, u: UnitId, offset: usize, exclude: Option<u32>) -> CallElem {
        let p = self.program();
        let cx = Context::new(self, u);
        let Some(n) = cx.tree.locate_exclusive(offset) else {
            return CallElem::File(u);
        };
        for k in cx.tree.chain(n) {
            match (cx.kind(k), cx.tree.nodes[k].mark) {
                ("FunctionDeclaration" | "MethodDeclaration", Mark::Function(fid))
                    if Some(fid.0) != exclude =>
                {
                    if cx
                        .parent(k)
                        .is_some_and(|x| cx.kind(x) == "FunctionDeclarationStatement")
                    {
                        return CallElem::Local(u, fid.0);
                    }
                    if let Some(f) = self.node_function(u, fid) {
                        return CallElem::Function(f);
                    }
                }
                ("ConstructorDeclaration", _) => {
                    if let Some(CallElem::Function(f)) = self.locate_element(&cx, k) {
                        return CallElem::Function(f);
                    }
                }
                (
                    "ClassDeclaration"
                    | "MixinDeclaration"
                    | "EnumDeclaration"
                    | "ExtensionTypeDeclaration",
                    Mark::Decl(d),
                ) => {
                    if let Some(c) = cx.declaration_class(u, d) {
                        return CallElem::Class(c);
                    }
                }
                ("ExtensionDeclaration", Mark::Decl(d)) => {
                    if let Some(x) = (0..p.extensions.len())
                        .map(|i| ExtensionId(i as u32))
                        .find(|&x| p.extension(x).decl.unit == u && p.extension(x).decl.decl == d)
                    {
                        return CallElem::Extension(x);
                    }
                }
                _ => {}
            }
        }
        CallElem::File(u)
    }

    /// `findIncomingCalls`: the executable's references, grouped by container
    /// in the order of the first one, with each one's range
    /// (`_rangeForSearchMatch`).
    pub(crate) fn incoming_calls(&self, e: CallElem) -> Vec<(CallItem, Vec<Span>)> {
        let p = self.program();
        let (target, declarations) = match e {
            CallElem::Function(f) => {
                let target = match p.function(f).kind {
                    FunctionKind::Constructor | FunctionKind::SyntheticConstructor => {
                        Target::Constructor(f)
                    }
                    _ if p.function(f).class.is_none() && p.function(f).extension.is_none() => {
                        Target::Top(Element::Function(f))
                    }
                    _ => self.function_member(f),
                };
                let d = self.declarations(&target);
                (target, d)
            }
            CallElem::Local(u, fid) => {
                let Some(name) = p.unit(u).ast.function(ast::FunctionId(fid)).name else {
                    return Vec::new();
                };
                let target = Target::Local {
                    unit: u,
                    declaration: name.span.start,
                };
                let d = self.declarations(&target);
                (target, d)
            }
            _ => return Vec::new(),
        };
        let Ok(occurrences) = self.occurrences(&target, false) else {
            return Vec::new();
        };
        let mut groups: Vec<(CallElem, Vec<Span>)> = Vec::new();
        for (u, from, until) in occurrences {
            if declarations.contains(&(u, from, until)) {
                continue;
            }
            let container = self.container_at_offset(u, from, None);
            // The range: the method name in a `MethodInvocation` (a named constructor
            // comes with the dot in the index); that of the node for an empty range.
            let cx = Context::new(self, u);
            let mut s = Span {
                start: from,
                end: until,
            };
            if let Some(n) = cx.tree.locate(from, from) {
                if cx.kind(n) == "SimpleIdentifier"
                    && let Some(parent) = cx.parent(n)
                    && cx.kind(parent) == "MethodInvocation"
                    && let Some(m) = cx.method_name(parent)
                {
                    s = cx.tree.span(m);
                } else if from == until {
                    s = cx.tree.span(n);
                }
            }
            match groups.iter_mut().find(|(c, _)| *c == container) {
                Some((_, l)) => l.push(s),
                None => groups.push((container, vec![s])),
            }
        }
        groups
            .into_iter()
            .filter_map(|(c, spans)| Some((self.call_item(c)?, spans)))
            .collect()
    }

    /// `findOutgoingCalls`: the `_OutboundCallVisitor` on the target node (only
    /// a function, constructor or method), grouped by the called element in the
    /// order of the first one.
    pub(crate) fn outgoing_calls(&self, unit: UnitId, offset: usize) -> Vec<(CallItem, Vec<Span>)> {
        let cx = Context::new(self, unit);
        let Some(root) = self.at_target(&cx, offset) else {
            return Vec::new();
        };
        if !matches!(
            cx.kind(root),
            "FunctionDeclaration" | "ConstructorDeclaration" | "MethodDeclaration"
        ) {
            return Vec::new();
        }
        // The collected nodes, in visit order (pre-order).
        let mut collected: Vec<usize> = Vec::new();
        let mut stack = vec![root];
        while let Some(k) = stack.pop() {
            match cx.kind(k) {
                "FunctionDeclaration" if k != root => continue,
                "ConstructorName" => collected.push(cx.children(k).get(1).copied().unwrap_or(k)),
                "FunctionReference" => collected.push(k),
                "MethodInvocation" => {
                    if let Some(m) = cx.method_name(k) {
                        collected.push(m);
                    }
                }
                "PrefixedIdentifier" => {
                    if !cx.parent(k).is_some_and(|p| cx.kind(p) == "NamedType")
                        && let Some(&i) = cx.children(k).get(1)
                    {
                        collected.push(i);
                    }
                }
                "PropertyAccess" => {
                    if let Some(&i) = cx.children(k).last() {
                        collected.push(i);
                    }
                }
                "SimpleIdentifier" => {
                    // `staticElement is FunctionElement` (top-level or local) outside a
                    // declaration context.
                    let e = self.call_identifier_element(&cx, k);
                    let function = match e {
                        Some(CallElem::Local(..)) => true,
                        Some(CallElem::Function(f)) => {
                            let fe = self.program().function(f);
                            fe.kind == FunctionKind::Function
                                && fe.class.is_none()
                                && fe.extension.is_none()
                        }
                        _ => false,
                    };
                    if function && !matches!(cx.tree.nodes[k].mark, Mark::DeclarationContext) {
                        collected.push(k);
                    }
                }
                _ => {}
            }
            for &f in cx.children(k).iter().rev() {
                stack.push(f);
            }
        }
        // A set (`Set<AstNode>`): each node once, in insertion order.
        let mut seen = std::collections::HashSet::new();
        collected.retain(|n| seen.insert(*n));
        let mut groups: Vec<(CallElem, Vec<Span>)> = Vec::new();
        for n in collected {
            let Some(e) = self.call_node_element(&cx, n) else {
                continue;
            };
            let s = cx.tree.span(n);
            match groups.iter_mut().find(|(c, _)| *c == e) {
                Some((_, l)) => l.push(s),
                None => groups.push((e, vec![s])),
            }
        }
        groups
            .into_iter()
            .filter_map(|(c, spans)| Some((self.call_item(c)?, spans)))
            .collect()
    }
}
