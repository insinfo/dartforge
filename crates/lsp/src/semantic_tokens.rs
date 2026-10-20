//! Semantic tokens (`textDocument/semanticTokens/full` and `/range`) in the
//! Dart 3.6.2 server's format: the `DartUnitHighlightsComputer`
//! (`pkg/analysis_server/lib/src/computer/computer_highlights.dart`) with
//! the map of `lsp/semantic_tokens/mapping.dart` and the
//! `SemanticTokenEncoder` (`lsp/semantic_tokens/encoder.dart`).
//!
//! * reserved and built-in words (`keyword`; `control` for the flow ones:
//!   `if`, `for`, `return`, `await`, `async`…), `true`/`false` (`boolean`),
//!   `void` (`keyword` + `void`), numbers, strings (each piece of an
//!   interpolated one; `${…}` as `source` + `interpolation`; escapes as
//!   `string` + `escape`), comments (`documentation` for `///`/`/**`);
//! * each name by the identity from the common inference (`get_resolved`,
//!   namespaces): class, enum, extension and extension type
//!   (`class`/`enum`; `constructor` when it is the name of a creation),
//!   typedef (`type`), named constructor (`method` + `constructor`), method
//!   (`method`), getter, setter and reference to a field or top-level
//!   variable (`property`), field or variable declaration (`variable` +
//!   `declaration`), local, parameter (`parameter`; a `name:` label with
//!   `label`), type parameter, enum constant, import prefix (`variable` +
//!   `importPrefix`), function; `instance`/`static` and `declaration` as in
//!   Dart;
//! * annotations (`annotation` from the `@` to the `(`, and the `)`), with
//!   `annotation` on the names inside them;
//! * overlaps split by the one on top (the innermost), like
//!   `splitOverlappingTokens`, and multi-line tokens broken per line when
//!   the client does not accept `multilineTokenSupport`.

use crate::project::{Project, is_prefix, type_parameter_declaration};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Element, FunctionKind, UnitId};
use dartforge_frontend::ast::{self, DeclKind, ExprKind, MemberKind, StmtKind, TypeKind};
use dartforge_frontend::token::{Kind, Op};
use dartforge_types::{MemberRef, Resolved, Type};
use std::collections::HashMap;

/// Token types in the order of the Dart 3.6.2 legend (the index is the
/// number sent to the client).
pub(crate) const TYPES: &[&str] = &[
    "annotation",
    "keyword",
    "class",
    "comment",
    "method",
    "variable",
    "parameter",
    "enum",
    "enumMember",
    "type",
    "source",
    "property",
    "namespace",
    "boolean",
    "number",
    "string",
    "function",
    "typeParameter",
];

/// Modifiers in the order of the Dart 3.6.2 legend (the index is the bit).
pub(crate) const MODIFIERS: &[&str] = &[
    "documentation",
    "constructor",
    "declaration",
    "importPrefix",
    "instance",
    "static",
    "escape",
    "annotation",
    "control",
    "label",
    "interpolation",
    "void",
    "wildcard",
];

/// A token before encoding: offset and end in bytes, type and modifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Highlight {
    pub start: usize,
    pub end: usize,
    pub ty: &'static str,
    pub modifiers: Vec<&'static str>,
}

/// Built-in and contextual words that, outside a tree name, are keywords
/// (the analyzer's `BUILT_IN`).
const INLINE: &[&str] = &[
    "abstract",
    "as",
    "augment",
    "async",
    "await",
    "base",
    "covariant",
    "deferred",
    "export",
    "extension",
    "external",
    "factory",
    "Function",
    "get",
    "hide",
    "implements",
    "import",
    "interface",
    "late",
    "library",
    "macro",
    "mixin",
    "native",
    "of",
    "on",
    "operator",
    "part",
    "required",
    "sealed",
    "set",
    "show",
    "static",
    "sync",
    "type",
    "typedef",
    "when",
    "yield",
];

/// Control flow words (the `control` modifier).
const CONTROL: &[&str] = &[
    "assert", "await", "break", "case", "catch", "continue", "default", "do", "else", "finally",
    "for", "if", "in", "rethrow", "return", "switch", "throw", "try", "when", "while", "yield",
    "async", "sync",
];

struct Collector<'a> {
    project: &'a Project,
    unit: UnitId,
    /// Names classified by the tree, by start.
    names: HashMap<usize, Highlight>,
    others: Vec<Highlight>,
}

impl Collector<'_> {
    fn name(&mut self, s: Span, ty: &'static str, modifiers: &[&'static str]) {
        if s.end > s.start {
            self.names.entry(s.start).or_insert(Highlight {
                start: s.start,
                end: s.end,
                ty,
                modifiers: modifiers.to_vec(),
            });
        }
    }

    fn region(&mut self, start: usize, end: usize, ty: &'static str, modifiers: &[&'static str]) {
        if end > start {
            self.others.push(Highlight {
                start,
                end,
                ty,
                modifiers: modifiers.to_vec(),
            });
        }
    }

    /// Written type name (`A`, `p.A`, `T`, `dynamic`).
    fn written_type(&mut self, ty: ast::TypeId, extra: &[&'static str]) {
        let p = self.project.program();
        let u = p.unit(self.unit);
        let t = u.ast.ty(ty);
        let TypeKind::Named { name, .. } = &t.kind else {
            return;
        };
        let lib = u.library;
        let text = |n: &ast::Name| &u.source[n.span.start..n.span.end];
        match &name[..] {
            [n] => {
                if text(n) == "dynamic" || text(n) == "Never" {
                    self.name(n.span, "type", &[]);
                    return;
                }
                if type_parameter_declaration(&u.ast, n.span.start, n.sym).is_some() {
                    self.name(n.span, "typeParameter", &[]);
                    return;
                }
                if let Some(el) = p.lookup(lib, n.sym).and_then(|b| b.getter) {
                    self.element(n.span, el, extra);
                }
            }
            [pf, n] if is_prefix(p, lib, pf.sym) => {
                self.name(pf.span, "variable", &["importPrefix"]);
                if let Some(el) = p.lookup_prefixed(lib, pf.sym, n.sym).and_then(|b| b.getter) {
                    self.element(n.span, el, extra);
                }
            }
            [c, k] => {
                // `A.name` as a type: the class and the constructor.
                if let Some(el) = p.lookup(lib, c.sym).and_then(|b| b.getter) {
                    self.element(c.span, el, extra);
                }
                let mut m = vec!["constructor"];
                m.extend_from_slice(extra);
                self.name(k.span, "method", &m);
            }
            _ => {}
        }
    }

    /// Reference to a top-level element.
    fn element(&mut self, s: Span, el: Element, extra: &[&'static str]) {
        let p = self.project.program();
        let (ty, mods): (&'static str, Vec<&'static str>) = match el {
            Element::Class(c) => {
                let enum_ = p.class(c).decl.is_some_and(|d| {
                    matches!(p.unit(d.unit).ast.decl(d.decl).kind, DeclKind::Enum(_))
                });
                (if enum_ { "enum" } else { "class" }, Vec::new())
            }
            Element::Extension(_) => ("class", Vec::new()),
            Element::Typedef(_) => ("type", Vec::new()),
            Element::Function(f) => match p.function(f).kind {
                FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => {
                    ("property", Vec::new())
                }
                _ => ("function", Vec::new()),
            },
            Element::Variable(_) => ("property", Vec::new()),
            Element::Prefix(..) => ("variable", vec!["importPrefix"]),
        };
        let mut m = mods;
        m.extend_from_slice(extra);
        self.name(s, ty, &m);
    }

    /// Resolved reference in an expression.
    fn resolved(
        &mut self,
        s: Span,
        r: Option<&Resolved>,
        constructor: bool,
        extra: &[&'static str],
        dynamic: bool,
    ) {
        let p = self.project.program();
        let combine = |base: &[&'static str]| {
            let mut m = base.to_vec();
            m.extend_from_slice(extra);
            m
        };
        match r {
            Some(Resolved::Local(_)) => self.name(s, "variable", &combine(&[])),
            Some(Resolved::Parameter { .. }) => self.name(s, "parameter", &combine(&[])),
            Some(Resolved::TypeParameter(_)) => self.name(s, "typeParameter", &combine(&[])),
            Some(Resolved::Prefix(_)) => self.name(s, "variable", &combine(&["importPrefix"])),
            Some(Resolved::Element(Element::Class(c))) if constructor => {
                let _ = c;
                self.name(s, "class", &combine(&["constructor"]))
            }
            Some(Resolved::Element(el)) => self.element(s, *el, extra),
            Some(Resolved::Constructor(f)) => {
                let class = p.function(*f).class.map(|c| p.class(c).name);
                let text = &p.unit(self.unit).source[s.start..s.end];
                if class.is_some_and(|c| self.project.name(c) == text) {
                    self.name(s, "class", &combine(&["constructor"]));
                } else {
                    self.name(s, "method", &combine(&["constructor"]));
                }
            }
            Some(Resolved::Member {
                member: MemberRef::Function(f),
                ..
            })
            | Some(Resolved::ExtensionMember { member: f, .. }) => {
                let fe = p.function(*f);
                let scope = if fe.static_ { "static" } else { "instance" };
                let is_constant = fe.variable.is_some_and(|v| {
                    let ve = p.variable(v);
                    ve.class
                        .is_some_and(|c| p.class(c).enum_constants.contains(&v))
                });
                match fe.kind {
                    _ if is_constant => self.name(s, "enumMember", &combine(&[])),
                    FunctionKind::Getter
                    | FunctionKind::Setter
                    | FunctionKind::ImplicitAccessor => {
                        self.name(s, "property", &combine(&[scope]))
                    }
                    _ => self.name(s, "method", &combine(&[scope])),
                }
            }
            Some(Resolved::Member {
                member: MemberRef::Variable(v),
                ..
            }) => {
                let ve = p.variable(*v);
                if ve
                    .class
                    .is_some_and(|c| p.class(c).enum_constants.contains(v))
                {
                    self.name(s, "enumMember", &combine(&[]));
                } else {
                    self.name(
                        s,
                        "property",
                        &combine(&[if ve.static_ { "static" } else { "instance" }]),
                    );
                }
            }
            Some(Resolved::Dynamic) | None if dynamic => self.name(s, "source", &combine(&[])),
            _ => {}
        }
    }

    fn annotations(&mut self, annotations: &[ast::Annotation]) {
        let p = self.project.program();
        let u = p.unit(self.unit);
        for a in annotations {
            match &a.arguments {
                None => self.region(a.span.start, a.span.end, "annotation", &[]),
                Some(args) => {
                    self.region(a.span.start, args.span.start + 1, "annotation", &[]);
                    if args.span.end > args.span.start
                        && u.source.as_bytes().get(args.span.end - 1) == Some(&b')')
                    {
                        self.region(args.span.end - 1, args.span.end, "annotation", &[]);
                    }
                }
            }
            let lib = u.library;
            match &a.name[..] {
                [n] => {
                    if let Some(el) = p.lookup(lib, n.sym).and_then(|b| b.getter) {
                        self.element(n.span, el, &["annotation"]);
                    }
                }
                [pf, n, ..] if is_prefix(p, lib, pf.sym) => {
                    self.name(pf.span, "variable", &["importPrefix"]);
                    if let Some(el) = p.lookup_prefixed(lib, pf.sym, n.sym).and_then(|b| b.getter) {
                        self.element(n.span, el, &["annotation"]);
                    }
                    if let Some(k) = a.name.get(2) {
                        self.name(k.span, "method", &["constructor", "annotation"]);
                    }
                }
                [c, k] => {
                    if let Some(el) = p.lookup(lib, c.sym).and_then(|b| b.getter) {
                        self.element(c.span, el, &["annotation"]);
                    }
                    self.name(k.span, "method", &["constructor", "annotation"]);
                }
                _ => {}
            }
        }
    }

    fn parameters(&mut self, ps: &[ast::Parameter], bodies: &dartforge_types::UnitBodyTypes) {
        for prm in ps {
            self.annotations(&prm.metadata);
            if let Some(n) = prm.name {
                if prm.this_ {
                    self.name(n.span, "variable", &["instance"]);
                } else {
                    let _ = bodies;
                    self.name(n.span, "parameter", &["declaration"]);
                }
            }
            if let Some(t) = prm.ty {
                self.written_type(t, &[]);
            }
            if let Some(fps) = &prm.function_parameters {
                self.parameters(fps, bodies);
            }
        }
    }
}

impl Project {
    /// The semantic tokens of `unit`, already without overlaps, in order.
    pub(crate) fn highlights(&self, unit: UnitId) -> Vec<Highlight> {
        let p = self.program();
        let u = p.unit(unit);
        let ast = &u.ast;
        let source = u.source.as_str();
        let bodies = &self.query.bodies.units[unit.0 as usize];
        let mut c = Collector {
            project: self,
            unit,
            names: HashMap::new(),
            others: Vec::new(),
        };

        // Top-level declarations.
        for d in &ast.decls {
            c.annotations(&d.metadata);
            match &d.kind {
                DeclKind::Class(k) => {
                    c.name(k.name.span, "class", &["declaration"]);
                    for t in k
                        .extends
                        .iter()
                        .chain(k.with.iter())
                        .chain(k.implements.iter())
                    {
                        c.written_type(*t, &[]);
                    }
                    for tp in k.type_params.iter() {
                        c.name(tp.name.span, "typeParameter", &[]);
                    }
                }
                DeclKind::Mixin(k) => {
                    c.name(k.name.span, "class", &[]);
                    for t in k.on.iter().chain(k.implements.iter()) {
                        c.written_type(*t, &[]);
                    }
                    for tp in k.type_params.iter() {
                        c.name(tp.name.span, "typeParameter", &[]);
                    }
                }
                DeclKind::Enum(k) => {
                    c.name(k.name.span, "enum", &[]);
                    for k2 in &k.constants {
                        c.annotations(&k2.metadata);
                        c.name(k2.name.span, "enumMember", &[]);
                    }
                    for t in k.with.iter().chain(k.implements.iter()) {
                        c.written_type(*t, &[]);
                    }
                    for tp in k.type_params.iter() {
                        c.name(tp.name.span, "typeParameter", &[]);
                    }
                }
                DeclKind::Extension(k) => {
                    if let Some(n) = k.name {
                        c.name(n.span, "class", &[]);
                    }
                    c.written_type(k.on, &[]);
                    for tp in k.type_params.iter() {
                        c.name(tp.name.span, "typeParameter", &[]);
                    }
                }
                DeclKind::ExtensionType(k) => {
                    c.name(k.name.span, "class", &["declaration"]);
                    if let Some(n) = k.constructor {
                        c.name(n.span, "method", &["constructor", "declaration"]);
                    }
                    c.name(
                        k.representation_name.span,
                        "variable",
                        &["declaration", "instance"],
                    );
                    c.written_type(k.representation_type, &[]);
                    for t in k.implements.iter() {
                        c.written_type(*t, &[]);
                    }
                    for tp in k.type_params.iter() {
                        c.name(tp.name.span, "typeParameter", &[]);
                    }
                }
                DeclKind::Typedef(k) => {
                    c.name(k.name.span, "type", &[]);
                    for tp in k.type_params.iter() {
                        c.name(tp.name.span, "typeParameter", &[]);
                    }
                    match &k.kind {
                        ast::TypedefKind::Alias(t) => c.written_type(*t, &[]),
                        ast::TypedefKind::Legacy {
                            return_type,
                            parameters,
                        } => {
                            if let Some(t) = return_type {
                                c.written_type(*t, &[]);
                            }
                            c.parameters(parameters, bodies);
                        }
                    }
                }
                DeclKind::Function(f) => {
                    let func = ast.function(*f);
                    if let Some(n) = func.name {
                        match func.kind {
                            ast::FunctionKind::Getter | ast::FunctionKind::Setter => {
                                c.name(n.span, "property", &["declaration"])
                            }
                            _ => c.name(n.span, "function", &["declaration", "static"]),
                        }
                    }
                }
                DeclKind::Variables(v) => {
                    for var in v.variables.iter() {
                        c.name(var.name.span, "variable", &["declaration"]);
                    }
                    if let Some(t) = v.ty {
                        c.written_type(t, &[]);
                    }
                }
            }
        }
        // Members.
        for m in &ast.members {
            c.annotations(&m.metadata);
            match &m.kind {
                MemberKind::Field(v) => {
                    let scope = if v.static_ { "static" } else { "instance" };
                    for var in v.variables.iter() {
                        c.name(var.name.span, "variable", &["declaration", scope]);
                    }
                    if let Some(t) = v.ty {
                        c.written_type(t, &[]);
                    }
                }
                MemberKind::Method(f) => {
                    let func = ast.function(*f);
                    let scope = if func.static_ { "static" } else { "instance" };
                    if let Some(n) = func.name {
                        match func.kind {
                            ast::FunctionKind::Getter | ast::FunctionKind::Setter => {
                                c.name(n.span, "property", &["declaration", scope])
                            }
                            _ => c.name(n.span, "method", &["declaration", scope]),
                        }
                    }
                }
                MemberKind::Constructor(k) => {
                    c.name(k.class_name.span, "class", &["constructor", "declaration"]);
                    if let Some(n) = k.name {
                        c.name(n.span, "method", &["constructor", "declaration"]);
                    }
                    c.parameters(&k.parameters, bodies);
                    for start in k.initializers.iter() {
                        match start {
                            ast::Initializer::Field { name, .. } => {
                                c.name(name.span, "variable", &["instance"])
                            }
                            ast::Initializer::Super {
                                constructor: Some(n),
                                ..
                            }
                            | ast::Initializer::Redirect {
                                constructor: Some(n),
                                ..
                            } => c.name(n.span, "method", &["constructor"]),
                            _ => {}
                        }
                    }
                    if let Some(r) = &k.redirect {
                        c.written_type(r.ty, &["constructor"]);
                        if let Some(n) = r.constructor {
                            c.name(n.span, "method", &["constructor"]);
                        }
                    }
                }
            }
        }
        // Functions: parameters, type parameters and return; locals.
        for f in &ast.functions {
            c.parameters(f.parameters.as_deref().unwrap_or(&[]), bodies);
            for tp in f.type_params.iter() {
                c.name(tp.name.span, "typeParameter", &[]);
            }
            if let Some(t) = f.return_type {
                c.written_type(t, &[]);
            }
        }
        for s in &ast.stmts {
            match &s.kind {
                StmtKind::Variables(v) => {
                    for var in v.variables.iter() {
                        c.name(var.name.span, "variable", &["declaration"]);
                    }
                }
                // `for (var i = 0; …)`: the header variable is a declaration like any local
                // (docs/LSP-ESPECIFICACAO.md §15.2).
                StmtKind::For {
                    init: Some(ast::ForInit::Variables(v)),
                    ..
                } => {
                    for var in v.variables.iter() {
                        c.name(var.name.span, "variable", &["declaration"]);
                    }
                }
                StmtKind::Function(f) => {
                    if let Some(n) = ast.function(*f).name {
                        c.name(n.span, "function", &["declaration"]);
                    }
                }
                StmtKind::ForIn {
                    target: ast::ForInTarget::Declared { name, metadata, .. },
                    ..
                } => {
                    c.annotations(metadata);
                    c.name(name.span, "variable", &["declaration"]);
                }
                StmtKind::Try { catches, .. } => {
                    for k in catches.iter() {
                        for n in k.exception.iter().chain(k.stack_trace.iter()) {
                            c.name(n.span, "variable", &["declaration"]);
                        }
                    }
                }
                _ => {}
            }
        }
        for pt in &ast.patterns {
            if let ast::PatternKind::Variable { name, .. } = &pt.kind {
                c.name(name.span, "variable", &["declaration"]);
            }
        }
        // Type annotations (those that did not come through the declarations above).
        for i in 0..ast.types.len() {
            c.written_type(ast::TypeId(i as u32), &[]);
        }
        // Directive prefixes.
        for d in &u.unit.directives {
            c.annotations(&d.metadata);
            if let ast::DirectiveKind::Import {
                prefix: Some(n), ..
            } = &d.kind
            {
                c.name(n.span, "variable", &["importPrefix"]);
            }
            if let ast::DirectiveKind::Library { name } | ast::DirectiveKind::PartOf { name, .. } =
                &d.kind
                && let (Some(a), Some(b)) = (name.first(), name.last())
            {
                c.region(a.span.start, b.span.end, "namespace", &[]);
            }
        }
        // Expressions: references, creations and argument labels.
        let constructor2: std::collections::HashSet<u32> = ast
            .exprs
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match &e.kind {
                ExprKind::Call { target, .. }
                    if matches!(
                        bodies.get_resolved(ast::ExprId(i as u32)),
                        Some(Resolved::Constructor(_))
                    ) =>
                {
                    Some(target.0)
                }
                _ => None,
            })
            .collect();
        // The names of declared parameters (inference resolves a use of a parameter
        // as a local).
        let parameters_: std::collections::HashSet<usize> = ast
            .functions
            .iter()
            .flat_map(|f| f.parameters.iter().flatten())
            .chain(ast.members.iter().flat_map(|m| match &m.kind {
                MemberKind::Constructor(k) => k.parameters.iter(),
                _ => [].iter(),
            }))
            .filter_map(|p| p.name.map(|n| n.span.start))
            .collect();
        for (i, e) in ast.exprs.iter().enumerate() {
            let id = ast::ExprId(i as u32);
            match &e.kind {
                ExprKind::Identifier(n) => {
                    let constructor_ = constructor2.contains(&(i as u32));
                    let r = bodies.get_resolved(id);
                    if matches!(r, Some(Resolved::Local(_)))
                        && bodies
                            .local_declaration(id)
                            .is_some_and(|d| parameters_.contains(&d))
                    {
                        c.name(n.span, "parameter", &[]);
                    } else {
                        c.resolved(n.span, r, constructor_, &[], true);
                    }
                }
                ExprKind::Property { target, name, .. } => {
                    let constructor_ = constructor2.contains(&(i as u32));
                    // `A.name(…)`: the target is the creation's class.
                    if constructor_ && let ExprKind::Identifier(cn) = &ast.expr(*target).kind {
                        c.name(cn.span, "class", &["constructor"]);
                    }
                    let dynamic = bodies
                        .get_type(*target)
                        .is_some_and(|t| matches!(self.query.table.get(t), Type::Dynamic));
                    let r = bodies.get_resolved(id);
                    if constructor_ && !matches!(r, Some(Resolved::Constructor(_))) {
                        c.name(name.span, "method", &["constructor"]);
                    } else if r.is_none()
                        && let Some(t) = bodies.get_type(*target)
                        && matches!(self.query.table.get(t), Type::Record { .. })
                    {
                        c.name(name.span, "property", &["instance"]);
                    } else {
                        c.resolved(name.span, r, false, &[], dynamic || r.is_none());
                    }
                }
                ExprKind::InstanceCreation {
                    ty,
                    constructor,
                    arguments,
                    ..
                } => {
                    c.written_type(*ty, &["constructor"]);
                    if let Some(n) = constructor {
                        c.name(n.span, "method", &["constructor"]);
                    }
                    let _ = arguments;
                }
                _ => {}
            }
            if let ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } =
                &e.kind
            {
                for a in arguments.args.iter() {
                    if let Some(n) = a.name {
                        c.name(n.span, "parameter", &["label"]);
                    }
                }
            }
            if let ExprKind::Record { named, .. } = &e.kind {
                for (n, _) in named.iter() {
                    c.name(n.span, "parameter", &[]);
                }
            }
        }

        // `show`/`hide` names: the element the target library exports.
        for d in &u.unit.directives {
            if let ast::DirectiveKind::Import { combinators, .. }
            | ast::DirectiveKind::Export { combinators, .. } = &d.kind
            {
                for comb in combinators.iter() {
                    let (ast::Combinator::Show(names) | ast::Combinator::Hide(names)) = comb;
                    for n in names.iter() {
                        if let Ok(Some(den)) = self.identify(unit, n.span.start)
                            && let Some((ty_, mods)) =
                                classify(self, &den.target, den.concrete, &parameters_)
                        {
                            c.name(n.span, ty_, &mods);
                        }
                    }
                }
            }
        }
        // `[name]` references of documentation comments, on top of the comment
        // (which is split around them).
        let mut references: Vec<Highlight> = Vec::new();
        for combine in crate::dartdoc::comments(source) {
            for r in &combine.references {
                for pos in 0..r.len() {
                    let Some((target_, concrete)) =
                        self.resolve_doc_reference(unit, combine.span, &r[..=pos])
                    else {
                        continue;
                    };
                    if let Some((ty_, mods)) = classify(self, &target_, concrete, &parameters_) {
                        references.push(Highlight {
                            start: r[pos].0.start,
                            end: r[pos].0.end,
                            ty: ty_,
                            modifiers: mods,
                        });
                    }
                }
            }
        }

        // Text tokens: words, literals, strings and comments.
        let mut regions = std::mem::take(&mut c.others);
        regions.extend(references);
        let names = std::mem::take(&mut c.names);
        let Ok(tokens) = dartforge_frontend::lexer::lex(source) else {
            return Vec::new();
        };
        let mut previous = 0usize;
        // Interpolation opened by `${` (start of the `$`).
        let mut interpolations: Vec<usize> = Vec::new();
        let mut last_interp_ident: Option<usize> = None;
        for (k, t) in tokens.iter().enumerate() {
            // Comments in the gap.
            for (start, end) in comments(source, previous, t.span.start) {
                let text = &source[start..end];
                let doc = text.starts_with("///") || text.starts_with("/**");
                regions.push(Highlight {
                    start,
                    end,
                    ty: "comment",
                    modifiers: if doc {
                        vec!["documentation"]
                    } else {
                        Vec::new()
                    },
                });
            }
            previous = t.span.end;
            let text = &source[t.span.start..t.span.end];
            let r = |ty_: &'static str, m: &[&'static str]| Highlight {
                start: t.span.start,
                end: t.span.end,
                ty: ty_,
                modifiers: m.to_vec(),
            };
            match t.kind {
                Kind::Keyword(_) => {
                    let highlight = match text {
                        "true" | "false" => r("boolean", &[]),
                        "void" => r("keyword", &["void"]),
                        "switch" if !is_command_switch(ast, t.span.start) => r("keyword", &[]),
                        _ if CONTROL.contains(&text) => r("keyword", &["control"]),
                        _ => r("keyword", &[]),
                    };
                    regions.push(highlight);
                }
                Kind::Ident => {
                    if let Some(n) = names.get(&t.span.start) {
                        regions.push(n.clone());
                        if last_interp_ident == Some(k) {
                            regions.push(Highlight {
                                start: t.span.start - 1,
                                end: t.span.end,
                                ty: "source",
                                modifiers: vec!["interpolation"],
                            });
                        }
                        continue;
                    }
                    if last_interp_ident == Some(k) {
                        regions.push(Highlight {
                            start: t.span.start - 1,
                            end: t.span.end,
                            ty: "source",
                            modifiers: vec!["interpolation"],
                        });
                        continue;
                    }
                    if text == "dynamic" {
                        regions.push(r("type", &[]));
                    } else if INLINE.contains(&text) {
                        // `async*`/`sync*`/`yield*`: a single token with the `*`.
                        let mut end = t.span.end;
                        if matches!(text, "async" | "sync" | "yield")
                            && let Some(next) = tokens.get(k + 1)
                            && next.kind == Kind::Op(Op::Star)
                        {
                            end = next.span.end;
                        }
                        // `part of`: a single token.
                        if text == "part"
                            && let Some(next) = tokens.get(k + 1)
                            && next.text(source) == "of"
                        {
                            end = next.span.end;
                        }
                        let m: &[&'static str] = if CONTROL.contains(&text)
                            || (text == "on" && is_catch_on(source, t.span.start))
                        {
                            &["control"]
                        } else {
                            &[]
                        };
                        regions.push(Highlight {
                            start: t.span.start,
                            end,
                            ty: "keyword",
                            modifiers: m.to_vec(),
                        });
                    } else {
                        regions.push(r("source", &[]));
                    }
                }
                Kind::Int | Kind::Double => regions.push(r("number", &[])),
                Kind::Str(flags) => {
                    regions.push(r("string", &[]));
                    if !flags.raw {
                        escapes(source, t.span, &mut regions);
                    }
                }
                Kind::StrBegin(_, interp) | Kind::StrMid(_, interp) => {
                    // The text piece goes up to before the `$`/`${`.
                    let mut start = t.span.start;
                    if matches!(t.kind, Kind::StrMid(..))
                        && source.as_bytes().get(start) == Some(&b'}')
                    {
                        if let Some(opens) = interpolations.pop() {
                            regions.push(Highlight {
                                start: opens,
                                end: start + 1,
                                ty: "source",
                                modifiers: vec!["interpolation"],
                            });
                        }
                        start += 1;
                    }
                    let cut = match interp {
                        dartforge_frontend::token::Interp::Brace => t.span.end - 2,
                        dartforge_frontend::token::Interp::Ident => t.span.end - 1,
                    };
                    regions.push(Highlight {
                        start,
                        end: cut,
                        ty: "string",
                        modifiers: Vec::new(),
                    });
                    match interp {
                        dartforge_frontend::token::Interp::Brace => interpolations.push(cut),
                        dartforge_frontend::token::Interp::Ident => last_interp_ident = Some(k + 1),
                    }
                }
                Kind::StrEnd(_) => {
                    let mut start = t.span.start;
                    if source.as_bytes().get(start) == Some(&b'}') {
                        if let Some(opens) = interpolations.pop() {
                            regions.push(Highlight {
                                start: opens,
                                end: start + 1,
                                ty: "source",
                                modifiers: vec!["interpolation"],
                            });
                        }
                        start += 1;
                    }
                    regions.push(Highlight {
                        start,
                        end: t.span.end,
                        ty: "string",
                        modifiers: Vec::new(),
                    });
                }
                _ => {}
            }
        }
        split_overlaps(regions)
    }
}

/// The token of a name by what it denotes (documentation references and
/// `show`/`hide` names, like the analyzer's `_addIdentifierRegion`).
fn classify(
    project: &Project,
    target: &crate::project::Target,
    concrete: Option<crate::project::Concrete>,
    parameters: &std::collections::HashSet<usize>,
) -> Option<(&'static str, Vec<&'static str>)> {
    use crate::project::{Concrete, Target};
    let p = project.program();
    Some(match (target, concrete) {
        (Target::Local { declaration, .. }, _) => (
            if parameters.contains(declaration) {
                "parameter"
            } else {
                "variable"
            },
            Vec::new(),
        ),
        (Target::TypeParameter { .. }, _) => ("typeParameter", Vec::new()),
        (Target::Prefix { .. }, _) => ("variable", vec!["importPrefix"]),
        (Target::Constructor(_), _) => ("method", vec!["constructor"]),
        (Target::Top(Element::Class(c)), _) => {
            let enum_ = p
                .class(*c)
                .decl
                .is_some_and(|d| matches!(p.unit(d.unit).ast.decl(d.decl).kind, DeclKind::Enum(_)));
            (if enum_ { "enum" } else { "class" }, Vec::new())
        }
        (Target::Top(Element::Typedef(_)), _) => ("type", Vec::new()),
        (Target::Top(Element::Extension(_)), _) => ("class", Vec::new()),
        (_, Some(Concrete::Function(f))) => {
            let fe = p.function(f);
            let member = fe.class.is_some() || fe.extension.is_some();
            let scope: Vec<&'static str> = if !member {
                Vec::new()
            } else if fe.static_ {
                vec!["static"]
            } else {
                vec!["instance"]
            };
            match fe.kind {
                FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => {
                    ("property", scope)
                }
                _ if member => ("method", scope),
                _ => ("function", Vec::new()),
            }
        }
        (_, Some(Concrete::Variable(v))) => {
            let ve = p.variable(v);
            match ve.class {
                Some(c) if p.class(c).enum_constants.contains(&v) => ("enumMember", Vec::new()),
                Some(_) => (
                    "property",
                    vec![if ve.static_ { "static" } else { "instance" }],
                ),
                None => ("property", Vec::new()),
            }
        }
        _ => return None,
    })
}

/// The `switch` at `offset` is a statement (the expression one gets no
/// `control`).
fn is_command_switch(ast: &ast::Ast, offset: usize) -> bool {
    ast.stmts
        .iter()
        .any(|s| s.span.start == offset && matches!(s.kind, StmtKind::Switch { .. }))
        || !ast
            .exprs
            .iter()
            .any(|e| e.span.start == offset && matches!(e.kind, ExprKind::Switch { .. }))
}

/// The `on` at `offset` is that of a `catch` clause (`} on T catch`).
fn is_catch_on(source: &str, offset: usize) -> bool {
    source[..offset].trim_end().ends_with('}')
}

/// The valid escapes of a simple string (`\n`, `\x41`, `\u{1F600}`).
fn escapes(source: &str, s: Span, output: &mut Vec<Highlight>) {
    let b = source.as_bytes();
    let hex = |i: usize, min: usize, max: usize| {
        let mut n = 0;
        while n < max && b.get(i + n).is_some_and(u8::is_ascii_hexdigit) {
            n += 1;
        }
        (n >= min).then_some(n)
    };
    let mut i = s.start;
    while i < s.end {
        if b[i] != b'\\' {
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        let Some(&c) = b.get(i) else { break };
        let end = match c {
            b'x' => hex(i + 1, 2, 2).map(|n| i + 1 + n),
            b'u' if b.get(i + 1) == Some(&b'{') => hex(i + 2, 1, 6)
                .filter(|n| b.get(i + 2 + n) == Some(&b'}'))
                .map(|n| i + 3 + n),
            b'u' => hex(i + 1, 4, 4).map(|n| i + 1 + n),
            _ => Some(i + source[i..].chars().next().map_or(1, char::len_utf8)),
        };
        match end {
            Some(f) if f <= s.end => {
                output.push(Highlight {
                    start,
                    end: f,
                    ty: "string",
                    modifiers: vec!["escape"],
                });
                i = f;
            }
            _ => i += 1,
        }
    }
}

/// The comments (start, end) of the gap `[from, until)`.
fn comments(source: &str, from: usize, until: usize) -> Vec<(usize, usize)> {
    let b = source.as_bytes();
    let until = until.min(b.len());
    let mut output = Vec::new();
    let mut i = from;
    while i < until {
        if b[i] == b'/' && b.get(i + 1) == Some(&b'/') {
            let end = source[i..until].find(['\n', '\r']).map_or(until, |k| i + k);
            output.push((i, end));
            i = end;
        } else if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
            let mut level = 0usize;
            let mut j = i;
            while j + 1 < until {
                if b[j] == b'/' && b[j + 1] == b'*' {
                    level += 1;
                    j += 2;
                } else if b[j] == b'*' && b[j + 1] == b'/' {
                    level -= 1;
                    j += 2;
                    if level == 0 {
                        break;
                    }
                } else {
                    j += 1;
                }
            }
            output.push((i, j.min(until)));
            i = j.max(i + 2);
        } else {
            i += 1;
        }
    }
    output
}

/// Sorts (start; the larger first; `boolean` above `keyword`) and splits the
/// overlaps by the token on top, like `splitOverlappingTokens`.
fn split_overlaps(mut regions: Vec<Highlight>) -> Vec<Highlight> {
    regions.sort_by(|a, b| {
        let priority = |r: &Highlight| u8::from(r.ty == "boolean");
        (a.start, std::cmp::Reverse(a.end - a.start), priority(a)).cmp(&(
            b.start,
            std::cmp::Reverse(b.end - b.start),
            priority(b),
        ))
    });
    regions.dedup_by(|b, a| a.start == b.start && a.end == b.end && a.ty == b.ty);
    let mut output = Vec::new();
    let mut stack: Vec<Highlight> = Vec::new();
    let process =
        |stack: &mut Vec<Highlight>, mut from: usize, until: usize, output: &mut Vec<Highlight>| {
            while let Some(last) = stack.last() {
                let end = last.end.min(until);
                if end > from {
                    output.push(Highlight {
                        start: from,
                        end,
                        ty: last.ty,
                        modifiers: last.modifiers.clone(),
                    });
                    from = end;
                }
                if last.end <= until {
                    stack.pop();
                } else {
                    return;
                }
            }
        };
    let mut pos = regions.first().map_or(0, |r| r.start);
    for r in regions {
        process(&mut stack, pos, r.start, &mut output);
        pos = r.start;
        stack.push(r);
    }
    if let Some(first) = stack.first() {
        let end = first.end;
        process(&mut stack, pos, end, &mut output);
    }
    output
}

/// Encodes for the protocol: relative line and column, length (UTF-16), type
/// index and modifier bits. Without `multilineTokenSupport`, each multi-line
/// token becomes one per line. `range` (bytes) filters out the tokens
/// outside it.
pub(crate) fn encode(
    text: &str,
    table: &crate::utf16::LineTable,
    highlights: &[Highlight],
    multiline: bool,
    range: Option<(usize, usize)>,
) -> Vec<u32> {
    let mut data = Vec::new();
    let (mut last_line, mut last_column) = (0u32, 0u32);
    let mut emit = |start: usize, end: usize, r: &Highlight, data: &mut Vec<u32>| {
        let (l0, c0) = table.offset_position(text, start);
        let (l1, c1) = table.offset_position(text, end);
        let length = if l1 == l0 {
            c1 - c0
        } else {
            text[start..end].encode_utf16().count() as u32
        };
        if length == 0 {
            return;
        }
        let ty = TYPES.iter().position(|t| *t == r.ty).unwrap_or(0) as u32;
        let bits = r
            .modifiers
            .iter()
            .filter_map(|m| MODIFIERS.iter().position(|x| x == m))
            .fold(0u32, |b, i| b | (1 << i));
        let dl = l0 - last_line;
        let dc = if dl == 0 { c0 - last_column } else { c0 };
        data.extend_from_slice(&[dl, dc, length, ty, bits]);
        last_line = l0;
        last_column = c0;
    };
    for r in highlights {
        if let Some((from, until)) = range
            && (r.end < from || r.start > until)
        {
            continue;
        }
        let l0 = table.line_of(text, r.start);
        let l1 = table.line_of(text, r.end);
        if multiline || l0 == l1 {
            emit(r.start, r.end, r, &mut data);
            continue;
        }
        // One per line, with the line break and the indentation included.
        for l in l0..=l1 {
            let start = if l == l0 {
                r.start
            } else {
                table.line_start(l)
            };
            let end = if l == l1 {
                r.end
            } else {
                table.line_start(l + 1)
            };
            let end_without_break = if l == l1 {
                end
            } else {
                table.line_end(text, l)
            };
            emit(start, end_without_break.max(start), r, &mut data);
            let _ = end;
        }
    }
    data
}
