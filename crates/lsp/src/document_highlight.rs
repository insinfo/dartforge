//! `textDocument/documentHighlight` like the Dart 3.6.2 server's
//! `DartUnitOccurrencesComputer` (docs/LSP-ESPECIFICACAO.md §11.6): a visitor
//! of the (resolved) analyzer-shaped tree collects, per **canonical element**
//! (the field of a `this.x`, the variable of a getter/setter, the
//! declaration of a member), the offsets where it appears; the response is
//! the groups with some offset covering the cursor (inclusive end), without
//! override families, without repeated ranges.
//! Written without compiling or running (2026-10-05).

use crate::analyzer_tree::Mark;
use crate::project::{Concrete, Project, Target};
use crate::refactorings::Context;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{
    ClassId, Element, ExtensionId, FunctionElementId, FunctionKind, LibraryId, UnitId, VariableId,
};
use dartforge_frontend::ast;
use dartforge_intern::SymbolId;
use dartforge_types::{MemberRef, Resolved};

/// The canonical element (`_canonicalizeElement`).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Key {
    /// Local, parameter, local function, pattern variable (by the offset of the
    /// declared name).
    Local(usize),
    Variable(VariableId),
    /// The synthetic field of a declared getter/setter (the owner and the name).
    Accessors(Owner2, String),
    Function(FunctionElementId),
    Class(ClassId),
    Extension(ExtensionId),
    Typedef(dartforge_elements::model::TypedefId),
    TypeParameter(usize),
    Label(usize),
    Prefix(LibraryId, SymbolId),
}

/// The owner of a pair of accessors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner2 {
    Class(ClassId),
    Extension(ExtensionId),
    Library(LibraryId),
}

impl Project {
    /// The highlights of `offset` in `unit`, in the order of the groups and of
    /// the offsets.
    pub(crate) fn dart_highlights(&self, unit: UnitId, offset: usize) -> Vec<Span> {
        let cx = Context::new(self, unit);
        let mut groups: Vec<(Key, usize, Vec<usize>)> = Vec::new();
        let mut register =
            |key: Key, length: usize, o: usize| match groups.iter_mut().find(|(k, _, _)| *k == key)
            {
                Some((_, _, v)) => v.push(o),
                None => groups.push((key, length, vec![o])),
            };
        // Pre-order: the declaration records the name before the children.
        let mut stack = vec![0usize];
        while let Some(n) = stack.pop() {
            for (key, length, o) in records(self, &cx, n) {
                register(key, length, o);
            }
            stack.extend(cx.children(n).iter().rev().copied());
        }
        let mut output: Vec<Span> = Vec::new();
        for (_, length, offsets) in &groups {
            if !offsets.iter().any(|&o| o <= offset && offset <= o + length) {
                continue;
            }
            for &o in offsets {
                let s = Span {
                    start: o,
                    end: o + length,
                };
                if !output.contains(&s) {
                    output.push(s);
                }
            }
        }
        output
    }
}

/// The records (`key`, `nameLength`, offset) that node `n` makes.
fn records(p: &Project, cx: &Context<'_>, n: usize) -> Vec<(Key, usize, usize)> {
    let node = &cx.tree.nodes[n];
    let mut v = Vec::new();
    match node.kind {
        "ClassDeclaration"
        | "EnumDeclaration"
        | "ExtensionTypeDeclaration"
        | "MixinDeclaration" => {
            if let Mark::Decl(d) = node.mark
                && let Some(c) = cx.declaration_class(cx.unit, d)
                && let Some(name) = cx.declaration_name(d)
            {
                v.push((
                    Key::Class(c),
                    name.span.end - name.span.start,
                    name.span.start,
                ));
            }
        }
        "FunctionDeclaration" | "MethodDeclaration" => {
            if let Mark::Function(fid) = node.mark
                && let Some(name) = cx.ast.function(fid).name
            {
                let local = node.kind == "FunctionDeclaration"
                    && cx
                        .parent(n)
                        .is_some_and(|x| cx.kind(x) == "FunctionDeclarationStatement");
                let key = if local {
                    Some(Key::Local(name.span.start))
                } else {
                    p.node_function(cx.unit, fid)
                        .and_then(|f| function_key(p, f))
                };
                if let Some(k) = key {
                    v.push((k, name.span.end - name.span.start, name.span.start));
                }
            }
        }
        "VariableDeclaration" => {
            let name = crate::refactorings_exec::name_at_start_pub(cx.node_text(n));
            let key = if node.mark == Mark::LocalVariable {
                Some(Key::Local(node.start))
            } else {
                variable_declared_in(p, cx.unit, node.start).map(Key::Variable)
            };
            if let Some(k) = key {
                v.push((k, name.len(), node.start));
            }
        }
        "DeclaredIdentifier"
        | "DeclaredVariablePattern"
        | "SuperFormalParameter"
        | "SimpleFormalParameter"
        | "FieldFormalParameter"
        | "EnumConstantDeclaration" => {
            // The name: the last identifier before `=`/`:`/`(` (in an enum constant,
            // the first).
            let declared = if node.kind == "EnumConstantDeclaration" {
                first_identifier(cx, n)
            } else {
                declared_name(cx, n)
            };
            if let Some((o, name)) = declared {
                let key = match node.kind {
                    "EnumConstantDeclaration" => {
                        variable_declared_in(p, cx.unit, o).map(Key::Variable)
                    }
                    "FieldFormalParameter" => this_field(p, cx, n, &name).map(Key::Variable),
                    _ => Some(Key::Local(o)),
                };
                if let Some(k) = key {
                    v.push((k, name.len(), o));
                }
            }
        }
        "ConstructorDeclaration" => {
            // Only a named constructor (the name written after the dot).
            if let Some(f) = declared_constructor_in(p, cx.unit, node.start, node.end)
                && !p.name(p.program().function(f).name).is_empty()
                && let Some((_, s)) = p.function_name(f)
            {
                v.push((Key::Function(f), s.end - s.start, s.start));
            }
        }
        "NamedType" => {
            if let Some((k, length)) = type_element(p, cx, n) {
                v.push((k, length, node.start));
            }
        }
        "SimpleIdentifier" => {
            if let Some(k) = identifier_key(p, cx, n) {
                let length = name_length(p, &k, node.end - node.start);
                v.push((k, length, node.start));
            }
        }
        _ => {}
    }
    v
}

/// The length of the element's name (`element.nameLength`).
fn name_length(p: &Project, k: &Key, pattern: usize) -> usize {
    match k {
        Key::Function(f) => {
            let fe = p.program().function(*f);
            if matches!(
                fe.kind,
                FunctionKind::Constructor | FunctionKind::SyntheticConstructor
            ) {
                let name = p.name(fe.name);
                if name.is_empty() {
                    // The unnamed constructor: the written class name, or 0.
                    return match (fe.kind, fe.class) {
                        (FunctionKind::Constructor, Some(c)) => {
                            p.name(p.program().class(c).name).len()
                        }
                        _ => 0,
                    };
                }
                return name.len();
            }
            p.name(fe.name).trim_end_matches('=').len()
        }
        _ => pattern,
    }
}

/// The key of a function element (implicit accessor → the variable; a
/// declared getter or setter → the pair; anything else, the function
/// itself).
fn function_key(p: &Project, f: FunctionElementId) -> Option<Key> {
    let prog = p.program();
    let fe = prog.function(f);
    match fe.kind {
        FunctionKind::ImplicitAccessor => fe.variable.map(Key::Variable),
        FunctionKind::Getter | FunctionKind::Setter => {
            let owner = match (fe.class, fe.extension) {
                (_, Some(x)) => Owner2::Extension(x),
                (Some(c), _) => Owner2::Class(c),
                _ => Owner2::Library(fe.library),
            };
            Some(Key::Accessors(
                owner,
                p.name(fe.name).trim_end_matches('=').to_string(),
            ))
        }
        _ => Some(Key::Function(f)),
    }
}

/// The key of a `SimpleIdentifier`: through the resolution of the marked
/// expression, or through `identify` (combinators, annotations,
/// documentation, argument labels, the constructor's `returnType`), or the
/// label of a statement.
fn identifier_key(p: &Project, cx: &Context<'_>, n: usize) -> Option<Key> {
    let node = &cx.tree.nodes[n];
    // The `returnType` of a constructor declaration (the `A` of `A(…)` and of
    // `A.n(…)`): its `staticElement` is the class.
    if let Some(parent) = cx.parent(n)
        && cx.kind(parent) == "ConstructorDeclaration"
    {
        let f = declared_constructor_in(
            p,
            cx.unit,
            cx.tree.nodes[parent].start,
            cx.tree.nodes[parent].end,
        )?;
        return p.program().function(f).class.map(Key::Class);
    }
    // Statement labels and `break`/`continue`.
    if let Some(parent) = cx.parent(n) {
        if cx.kind(parent) == "Label"
            && cx.parent(parent).is_some_and(|g| {
                matches!(
                    cx.kind(g),
                    "LabeledStatement" | "SwitchCase" | "SwitchDefault" | "SwitchPatternCase"
                )
            })
        {
            return Some(Key::Label(node.start));
        }
        if matches!(cx.kind(parent), "BreakStatement" | "ContinueStatement") {
            let name = cx.node_text(n);
            for k in cx.with_parents(parent) {
                for &l in cx.children(k) {
                    if cx.kind(l) == "Label"
                        && let Some(&i) = cx.children(l).first()
                        && cx.node_text(i) == name
                    {
                        return Some(Key::Label(cx.tree.nodes[i].start));
                    }
                }
            }
            return None;
        }
    }
    if let Mark::Expr(x) = node.mark {
        return match cx.bodies.get_resolved(x) {
            Some(Resolved::Local(_)) | Some(Resolved::Parameter { .. }) => {
                let d = cx.bodies.local_declaration(x)?;
                // `this.x` → the field.
                if let Some((q, _)) = crate::project::parameter_in(cx.ast, d)
                    && q.this_
                {
                    let name = p.name(q.name?.sym).to_string();
                    return class_field_in(p, cx, d, &name).map(Key::Variable);
                }
                Some(Key::Local(d))
            }
            Some(Resolved::Element(Element::Function(f)))
            | Some(Resolved::Member {
                member: MemberRef::Function(f),
                ..
            })
            | Some(Resolved::ExtensionMember { member: f, .. }) => {
                // In a write, the setter (the same variable/pair).
                function_key(p, *f)
            }
            Some(Resolved::Element(Element::Variable(v)))
            | Some(Resolved::Member {
                member: MemberRef::Variable(v),
                ..
            }) => Some(Key::Variable(*v)),
            Some(Resolved::Element(Element::Class(c))) => Some(Key::Class(*c)),
            Some(Resolved::Element(Element::Extension(e))) => Some(Key::Extension(*e)),
            Some(Resolved::Element(Element::Typedef(t))) => Some(Key::Typedef(*t)),
            Some(Resolved::Element(Element::Prefix(l, s))) => Some(Key::Prefix(*l, *s)),
            Some(Resolved::Prefix(_)) => {
                let ast::ExprKind::Identifier(name) = &cx.ast.expr(x).kind else {
                    return None;
                };
                Some(Key::Prefix(p.program().unit(cx.unit).library, name.sym))
            }
            Some(Resolved::TypeParameter(_)) => {
                let ast::ExprKind::Identifier(name) = &cx.ast.expr(x).kind else {
                    return None;
                };
                crate::project::type_parameter_declaration(cx.ast, name.span.start, name.sym)
                    .map(Key::TypeParameter)
            }
            Some(Resolved::Constructor(f)) => Some(Key::Function(*f)),
            None if cx.constructors_by_target.contains_key(&x) => {
                let f = cx.constructors_by_target[&x];
                Some(Key::Function(f))
            }
            _ => None,
        };
    }
    // Through `identify`.
    let d = p.identify(cx.unit, node.start).ok()??;
    if d.name.start != node.start {
        return None;
    }
    match &d.target {
        Target::Local { declaration, .. } => Some(Key::Local(*declaration)),
        Target::TypeParameter { declaration, .. } => Some(Key::TypeParameter(*declaration)),
        Target::Prefix { library, name } => Some(Key::Prefix(*library, *name)),
        Target::Top(Element::Class(c)) => Some(Key::Class(*c)),
        Target::Top(Element::Extension(e)) => Some(Key::Extension(*e)),
        Target::Top(Element::Typedef(t)) => Some(Key::Typedef(*t)),
        Target::Top(Element::Variable(v)) => Some(Key::Variable(*v)),
        Target::Top(Element::Function(f)) => function_key(p, *f),
        Target::Top(Element::Prefix(l, s)) => Some(Key::Prefix(*l, *s)),
        Target::Constructor(f) => Some(Key::Function(*f)),
        Target::Member { .. } => match d.concrete {
            Some(Concrete::Function(f)) => function_key(p, f),
            Some(Concrete::Variable(v)) => Some(Key::Variable(v)),
            None => None,
        },
    }
}

/// The element of a `NamedType` (by the written name, in the scope of the
/// unit or of a type parameter), with the length of the element's name.
fn type_element(p: &Project, cx: &Context<'_>, n: usize) -> Option<(Key, usize)> {
    let prog = p.program();
    let node = &cx.tree.nodes[n];
    let prefix = cx
        .children(n)
        .iter()
        .copied()
        .find(|&k| cx.kind(k) == "ImportPrefixReference");
    let name_start = prefix.map_or(node.start, |k| cx.tree.nodes[k].end);
    let rest = cx.source[name_start..node.end].trim_start();
    let end = rest
        .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
        .unwrap_or(rest.len());
    let name = &rest[..end];
    if name.is_empty() || name == "dynamic" || name == "void" {
        return None;
    }
    let s = p.query.names.lookup(name)?;
    let binding = match prefix {
        Some(k) => {
            let t = cx.node_text(k);
            let pf = t.trim_end_matches('.').trim();
            let ps = p.query.names.lookup(pf)?;
            prog.lookup_prefixed_in_unit(cx.unit, ps, s)
        }
        None => {
            // A type parameter in scope comes before the unit scope.
            if let Some(d) = crate::project::type_parameter_declaration(cx.ast, node.start, s) {
                return Some((Key::TypeParameter(d), name.len()));
            }
            prog.lookup_in_unit(cx.unit, s)
        }
    }?;
    let key = match binding.getter? {
        Element::Class(c) => Key::Class(c),
        Element::Typedef(t) => Key::Typedef(t),
        Element::Extension(e) => Key::Extension(e),
        _ => return None,
    };
    Some((key, name.len()))
}

/// The declared name of a parameter or of a simple declaration: the offset
/// and the text.
fn declared_name(cx: &Context<'_>, n: usize) -> Option<(usize, String)> {
    let node = &cx.tree.nodes[n];
    let t = cx.node_text(n);
    // Without the children (type, annotations, default value): the first
    // identifier stretch outside them, the last one before `=`, `:`, `(`.
    let mut best: Option<(usize, String)> = None;
    let mut i = 0;
    let b = t.as_bytes();
    while i < b.len() {
        let c = b[i];
        let inside_child = cx.children(n).iter().any(|&f| {
            let s = &cx.tree.nodes[f];
            s.start <= node.start + i && node.start + i < s.end
        });
        if inside_child {
            i += 1;
            continue;
        }
        if c == b'=' || c == b':' || c == b'(' {
            break;
        }
        if c.is_ascii_alphabetic() || c == b'_' || c == b'$' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
            let word = &t[start..i];
            if !matches!(
                word,
                "final" | "var" | "const" | "late" | "covariant" | "required" | "this" | "super"
            ) {
                best = Some((node.start + start, word.to_string()));
            }
            continue;
        }
        i += 1;
    }
    best
}

/// The first identifier outside the children (the name of an enum constant,
/// after the annotations).
fn first_identifier(cx: &Context<'_>, n: usize) -> Option<(usize, String)> {
    let node = &cx.tree.nodes[n];
    let t = cx.node_text(n);
    let b = t.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let inside = cx.children(n).iter().any(|&f| {
            let s = &cx.tree.nodes[f];
            s.start <= node.start + i && node.start + i < s.end
        });
        if !inside && (b[i].is_ascii_alphabetic() || b[i] == b'_' || b[i] == b'$') {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
            return Some((node.start + start, t[start..i].to_string()));
        }
        i += 1;
    }
    None
}

/// The field of `this.x` (the `FieldFormalParameter` at `n`).
fn this_field(p: &Project, cx: &Context<'_>, n: usize, name: &str) -> Option<VariableId> {
    class_field_in(p, cx, cx.tree.nodes[n].start, name)
}

/// The field `name` of the class containing the offset.
fn class_field_in(p: &Project, cx: &Context<'_>, offset: usize, name: &str) -> Option<VariableId> {
    let prog = p.program();
    let c = prog.classes.iter().position(|c| {
        c.decl.is_some_and(|d| {
            d.unit == cx.unit && {
                let s = cx.ast.decl(d.decl).span;
                s.start <= offset && offset <= s.end
            }
        })
    })?;
    prog.class(ClassId(c as u32))
        .fields
        .iter()
        .copied()
        .find(|v| p.name(prog.variable(*v).name) == name)
}

/// The variable (top-level, field or enum constant) whose name starts at
/// `offset` in the unit.
pub(crate) fn variable_declared_in(p: &Project, u: UnitId, offset: usize) -> Option<VariableId> {
    let prog = p.program();
    (0..prog.variables.len())
        .map(|i| VariableId(i as u32))
        .find(|&v| {
            p.variable_name(v)
                .is_some_and(|(uu, s)| uu == u && s.start == offset)
        })
}

/// The constructor declared in the range.
fn declared_constructor_in(
    p: &Project,
    u: UnitId,
    start: usize,
    end: usize,
) -> Option<FunctionElementId> {
    let prog = p.program();
    let unit = prog.unit(u);
    let (mi, _) = unit.ast.members.iter().enumerate().find(|(_, m)| {
        matches!(m.kind, ast::MemberKind::Constructor(_))
            && m.span.start >= start
            && m.span.end <= end
    })?;
    p.node_constructor(u, ast::MemberId(mi as u32))
}
