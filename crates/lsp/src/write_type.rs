//! `DartEditBuilderImpl.writeType`/`_writeType` from the Dart 3.6.2
//! `analyzer_plugin` (`change_builder_dart.dart:986-1011`, `:1396-1503`,
//! `:1553-1675`, `:1811-1850`; docs/LSP-ESPECIFICACAO.md §13.7.4.0,
//! "Tipos"): the text of a type at the insertion point, with the import
//! prefix and the libraries to import.
//! Written without compiling or running (2026-10-05).

use crate::refactorings::Context;
use dartforge_elements::model::{ClassId, Element, FunctionElementId, LibraryId};
use dartforge_types::table::DisplayDecoration;
use dartforge_types::{Type, TypeId, TypeParamOwner};
use std::collections::BTreeSet;

/// The type writer for a point of the unit.
pub(crate) struct Writer<'a, 'p> {
    query: &'a crate::query::Query,
    _p: std::marker::PhantomData<&'p ()>,
    library: LibraryId,
    /// `_EnclosingElementFinder`: the outermost `ClassDeclaration` and
    /// executable that contain the point.
    class: Option<ClassId>,
    executable: Option<FunctionElementId>,
    /// `methodBeingCopied`: the executable whose type parameters are copied
    /// along (completion's override).
    pub(crate) copied: Option<FunctionElementId>,
    /// The libraries that `_writeLibraryReference` scheduled for import.
    pub(crate) import: BTreeSet<LibraryId>,
}

impl<'a, 'p> Writer<'a, 'p> {
    pub(crate) fn new(cx: &'a Context<'p>, offset: usize) -> Writer<'a, 'p> {
        let prog = cx.p.program();
        let library = prog.unit(cx.unit).library;
        let mut class = None;
        let mut executable = None;
        if let Some(n) = cx.tree.localizar_exclusivo(offset) {
            let mut current = Some(n);
            while let Some(k) = current {
                match (cx.kind(k), cx.tree.nos[k].marca) {
                    ("ClassDeclaration", crate::arvore_analyzer::Marca::Decl(d)) => class = cx.declaration_class(cx.unit, d),
                    ("MethodDeclaration" | "FunctionDeclaration", crate::arvore_analyzer::Marca::Funcao(fid)) => {
                        executable = cx.p.node_function(cx.unit, fid);
                    }
                    ("ConstructorDeclaration", _) => {
                        let s = cx.tree.span(k);
                        executable = cx
                            .ast
                            .members
                            .iter()
                            .enumerate()
                            .find(|(_, m)| matches!(m.kind, dartforge_frontend::ast::MemberKind::Constructor(_)) && m.span.start >= s.start && m.span.end <= s.end)
                            .and_then(|(mi, _)| cx.p.node_constructor(cx.unit, dartforge_frontend::ast::MemberId(mi as u32)));
                    }
                    _ => {}
                }
                current = cx.parent(k);
            }
        }
        Writer { query: &cx.p.query, _p: std::marker::PhantomData, library, class, executable, copied: None, import: BTreeSet::new() }
    }

    /// The writer over the completion query: the request's library, and the
    /// class and executable that contain the point (`_EnclosingElementFinder`).
    pub(crate) fn from_query(
        query: &'a crate::query::Query,
        library: LibraryId,
        class: Option<ClassId>,
        executable: Option<FunctionElementId>,
    ) -> Writer<'a, 'p> {
        Writer { query, _p: std::marker::PhantomData, library, class, executable, copied: None, import: BTreeSet::new() }
    }

    fn table(&self) -> &dartforge_types::TypeTable {
        &self.query.table
    }

    /// `_getVisibleType(type) != null`.
    fn visible(&self, t: TypeId) -> bool {
        let prog = &self.query.program;
        match self.table().get(t) {
            Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => {
                let c = prog.class(*class);
                !(self.query.name(c.name).starts_with('_') && c.library != self.library)
            }
            Type::TypeParameter { param, .. } | Type::Intersection { param, .. } => match self.table().param(*param).owner {
                TypeParamOwner::Class(c) => self.class == Some(c),
                TypeParamOwner::Function(f) => self.executable == Some(f) || self.copied == Some(f),
                TypeParamOwner::GenericFunctionType => true,
                _ => false,
            },
            _ => true,
        }
    }

    /// `writeType(type, required: required)`: `None` when nothing is written
    /// (with `required`, `var`).
    pub(crate) fn write_type(&mut self, t: Option<TypeId>, required: bool) -> Option<String> {
        let written = match t {
            Some(t) if !matches!(self.table().get(t), Type::Dynamic) => self.write(t, false),
            _ => None,
        };
        match written {
            Some(s) => Some(s),
            None if required => Some("var".to_string()),
            None => None,
        }
    }

    /// `_writeType(type, required)`.
    pub(crate) fn write(&mut self, t: TypeId, required: bool) -> Option<String> {
        if !self.visible(t) {
            return None;
        }
        let display = self.table().decoration(t).cloned();
        if matches!(display, Some(DisplayDecoration::Invalid)) || matches!(self.table().get(t), Type::Dynamic) {
            return required.then(|| "dynamic".to_string());
        }
        if matches!(self.table().get(t), Type::Never) && !matches!(display, Some(DisplayDecoration::NeverNullable)) {
            return Some("Never".to_string());
        }
        let nullable_ = self.table().get(t).is_declared_nullable();
        let suffix = if nullable_ { "?" } else { "" };
        if let Some(DisplayDecoration::Alias { typedef, args }) = display {
            let s = self.element_with_arguments(Element::Typedef(typedef), &args);
            return Some(format!("{s}{suffix}"));
        }
        if matches!(display, Some(DisplayDecoration::NeverNullable)) {
            return Some("Never?".to_string());
        }
        let p = self.query;
        match self.table().get(t).clone() {
            Type::Function { type_params, ret, positional, optional, named, nullable } => {
                let mut s = String::new();
                if let Some(r) = self.write(ret, false) {
                    s.push_str(&r);
                    s.push(' ');
                }
                s.push_str("Function");
                if !type_params.is_empty() {
                    s.push('<');
                    for (i, tp) in type_params.iter().enumerate() {
                        if i > 0 {
                            s.push_str(", ");
                        }
                        let data = self.table().param(*tp);
                        let (name, limit, explicit) = (data.name, data.bound, data.explicit);
                        s.push_str(p.name(name));
                        if explicit && let Some(b) = self.write(limit, false) {
                            s.push_str(" extends ");
                            s.push_str(&b);
                        }
                    }
                    s.push('>');
                }
                // `writeParameters(includeDefaultValues: false, requiredTypes: true)`;
                // unnamed positionals become `p1`…
                let mut used: BTreeSet<String> = named.iter().map(|(n, _, _)| p.name(*n).to_string()).collect();
                s.push('(');
                let mut i = 0usize;
                let mut opened_optional = false;
                let mut opened_named = false;
                let generate = |used: &mut BTreeSet<String>| {
                    let mut k = 1;
                    while used.contains(&format!("p{k}")) {
                        k += 1;
                    }
                    let n = format!("p{k}");
                    used.insert(n.clone());
                    n
                };
                for &q in positional.iter() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    let name = generate(&mut used);
                    let ty = self.write(q, true);
                    s.push_str(&parameter(ty, &name, false));
                    i += 1;
                }
                for &q in optional.iter() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    if !opened_optional {
                        s.push('[');
                        opened_optional = true;
                    }
                    let name = generate(&mut used);
                    let ty = self.write(q, true);
                    s.push_str(&parameter(ty, &name, false));
                    i += 1;
                }
                for (n, q, required_) in named.iter() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    if !opened_named {
                        s.push('{');
                        opened_named = true;
                    }
                    let ty = self.write(*q, true);
                    s.push_str(&parameter(ty, p.name(*n), *required_));
                    i += 1;
                }
                if opened_named {
                    s.push('}');
                }
                if opened_optional {
                    s.push(']');
                }
                s.push(')');
                if nullable {
                    s.push('?');
                }
                Some(s)
            }
            Type::Interface { class, args, .. } | Type::ExtensionType { decl: class, args, .. } => {
                Some(format!("{}{suffix}", self.element_with_arguments(Element::Class(class), &args)))
            }
            Type::FutureOr { arg, .. } => match self.future_or_class() {
                Some(c) => Some(format!("{}{suffix}", self.element_with_arguments(Element::Class(c), &[arg]))),
                None => Some(format!("FutureOr{suffix}")),
            },
            Type::Null => match p.core.null_class {
                Some(c) => Some(self.element_with_arguments(Element::Class(c), &[])),
                None => Some("Null".to_string()),
            },
            Type::Never => Some(format!("Never{suffix}")),
            Type::TypeParameter { param, .. } | Type::Intersection { param, .. } => {
                Some(format!("{}{suffix}", p.name(self.table().param(param).name)))
            }
            Type::Void => Some("void".to_string()),
            Type::Record { positional, named, .. } => {
                let mut s = String::from("(");
                let mut first = true;
                for &q in positional.iter() {
                    if !first {
                        s.push_str(", ");
                    }
                    first = false;
                    s.push_str(&self.write(q, false).unwrap_or_default());
                }
                if !named.is_empty() {
                    s.push_str(if first { "{" } else { ", {" });
                    let mut first_named = true;
                    let mut named_: Vec<(String, TypeId)> = named.iter().map(|(n, q)| (p.name(*n).to_string(), *q)).collect();
                    named_.sort_by(|a, b| a.0.cmp(&b.0));
                    for (n, q) in named_ {
                        if !first_named {
                            s.push_str(", ");
                        }
                        first_named = false;
                        s.push_str(&self.write(q, false).unwrap_or_default());
                        s.push(' ');
                        s.push_str(&n);
                    }
                    s.push('}');
                }
                s.push(')');
                s.push_str(suffix);
                Some(s)
            }
            Type::Dynamic => required.then(|| "dynamic".to_string()),
        }
    }

    /// `writeType(typeProvider.futureType(x))` with `x` already flattened
    /// (`nullable`: the `flatten` of a `FutureOr<X>?`/`Future<X>?` is `X?`).
    pub(crate) fn write_future(&mut self, x: TypeId, nullable: bool) -> Option<String> {
        let future = self.query.core.future_class?;
        let mut s = self.reference(Element::Class(future));
        s.push_str("Future");
        let dynamic = matches!(self.table().get(x), Type::Dynamic);
        if !dynamic && self.visible(x) {
            let mut internal = self.write(x, true).unwrap_or_default();
            let already = self.table().get(x).is_declared_nullable() || matches!(self.table().get(x), Type::Void | Type::Null);
            if nullable && !already {
                internal.push('?');
            }
            s.push('<');
            s.push_str(&internal);
            s.push('>');
        }
        Some(s)
    }

    /// `_writeTypeElementArguments`: the prefix, the name and the arguments
    /// (only if some is not `dynamic` and all are visible).
    fn element_with_arguments(&mut self, el: Element, args: &[TypeId]) -> String {
        let mut s = self.reference(el);
        let p = self.query;
        let prog = &p.program;
        s.push_str(match el {
            Element::Class(c) => p.name(prog.class(c).name),
            Element::Typedef(t) => p.name(prog.typedef(t).name),
            _ => "",
        });
        if !args.is_empty() {
            let some = args.iter().any(|a| !matches!(self.table().get(*a), Type::Dynamic));
            let visible_all = args.iter().all(|a| self.visible(*a));
            if some && visible_all {
                s.push('<');
                for (i, &a) in args.iter().enumerate() {
                    if i != 0 {
                        s.push_str(", ");
                    }
                    s.push_str(&self.write(a, true).unwrap_or_default());
                }
                s.push('>');
            }
        }
        s
    }

    /// `_writeLibraryReference`: the prefix of the first import (the implicit
    /// `dart:core` one last) whose namespace has the element; without an
    /// import, the element's library is scheduled (without a prefix).
    fn reference(&mut self, el: Element) -> String {
        let p = self.query;
        let prog = &p.program;
        let (lib, name) = match el {
            Element::Class(c) => (prog.class(c).library, prog.class(c).name),
            Element::Typedef(t) => (prog.typedef(t).library, prog.typedef(t).name),
            Element::Extension(x) => match prog.extension(x).name {
                Some(n) => (prog.extension(x).library, n),
                None => return String::new(),
            },
            _ => return String::new(),
        };
        if lib == self.library {
            return String::new();
        }
        let current = prog.library(self.library);
        let defining = current.units.first().copied();
        let node_namespace = |target: LibraryId, combinators: &[dartforge_frontend::ast::Combinator]| {
            prog.library(target).exported.get(&name).and_then(|b| b.getter) == Some(el)
                && combinators.iter().all(|c| match c {
                    dartforge_frontend::ast::Combinator::Show(ns) => ns.iter().any(|n| n.sym == name),
                    dartforge_frontend::ast::Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == name),
                })
        };
        let mut explicit_core = false;
        for i in current.imports.iter().filter(|i| Some(i.unit) == defining) {
            if Some(i.library) == p.core.core_library {
                explicit_core = true;
            }
            if node_namespace(i.library, &i.combinators) {
                return i.prefix.map(|x| format!("{}.", p.name(x))).unwrap_or_default();
            }
        }
        if !explicit_core
            && let Some(core) = p.core.core_library
            && node_namespace(core, &[])
        {
            return String::new();
        }
        self.import.insert(lib);
        String::new()
    }

    /// `writeReference(element)`: the prefix and the name.
    pub(crate) fn reference_of(&mut self, el: Element, name: &str) -> String {
        format!("{}{name}", self.reference(el))
    }

    /// The `FutureOr` class of `dart:async`.
    fn future_or_class(&self) -> Option<ClassId> {
        let p = self.query;
        let prog = &p.program;
        let lib = p.core.async_library?;
        prog.library(lib).exported.iter().find(|(s, _)| p.name(**s) == "FutureOr").and_then(|(_, b)| match b.getter {
            Some(Element::Class(c)) => Some(c),
            _ => None,
        })
    }
}

/// `writeParameter(name, type, isRequiredType: true)` without a default.
fn parameter(ty: Option<String>, name: &str, required: bool) -> String {
    let mut s = String::new();
    if required {
        s.push_str("required ");
    }
    if let Some(t) = &ty {
        s.push_str(t);
        if !name.is_empty() {
            s.push(' ');
        }
    }
    s.push_str(name);
    s
}
