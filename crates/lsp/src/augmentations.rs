//! Augmentation chains in the LSP (docs/LSP-ESPECIFICACAO.md §8.3 and §8.6):
//! the `AugmentationCodeLensProvider` ("Go to Augmented" and "Go to
//! Augmentation") and the 3.6.2 `dart/textDocument/augmented` and
//! `dart/textDocument/augmentation` methods
//! (`AS:src/lsp/handlers/code_lens/augmentations.dart`,
//! `custom/handler_augmented.dart`, `custom/handler_augmentation.dart`).
//!
//! The chain is the application order (docs/AUGMENTATIONS.md): the library's
//! units in pre-order of the part tree, the declarations in text order, and,
//! in a class, the members of the introductory declaration followed by those
//! of each `augment class`. An `augment` declaration links to the previous
//! one with the same key (name; setters apart; statics apart for a member;
//! the constructor name): that one is its `augmentationTarget`, and it is the
//! previous one's `augmentation`. An ordinary declaration starts a new
//! chain. Only in libraries with augmentations enabled.
//!
//! Written without compiling or running (2026-10-05).

use crate::project::Project;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, UnitId};
use dartforge_frontend::ast::{self, DeclKind, MemberKind};
use std::collections::HashMap;

/// A chain link: the declared name and the neighbors.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Link {
    pub unit: UnitId,
    pub name: Span,
    /// The span of the whole declaration (the cursor's "walk up the nodes").
    pub declaration: Span,
    /// `augmentationTarget`.
    pub previous: Option<usize>,
    /// `augmentation`.
    pub next: Option<usize>,
}

impl Project {
    /// The links of every chain of the libraries with augmentations.
    pub(crate) fn augmentation_links(&self) -> Vec<Link> {
        let p = self.program();
        let mut links: Vec<Link> = Vec::new();
        let link_ = |links: &mut Vec<Link>, current: &mut HashMap<String, usize>, key: String, new: Link, augment: bool| {
            let i = links.len();
            links.push(new);
            if augment
                && let Some(&previous) = current.get(&key)
            {
                links[previous].next = Some(i);
                links[i].previous = Some(previous);
            }
            current.insert(key, i);
        };
        for (li, lib) in p.libraries.iter().enumerate() {
            if !p.biblioteca_com_augmentations(dartforge_elements::model::LibraryId(li as u32)) {
                continue;
            }
            // The top level, in application order.
            let mut current: HashMap<String, usize> = HashMap::new();
            for &u in &lib.units {
                let a = &p.unit(u).ast;
                for &d in &p.unit(u).unit.declarations {
                    let decl = a.decl(d);
                    for (name, setter) in declaration_names(a, decl) {
                        let key = format!("{}{}", self.name(name.sym), if setter { "=" } else { "" });
                        let link = Link { unit: u, name: name.span, declaration: decl.span, previous: None, next: None };
                        link_(&mut links, &mut current, key, link, decl.augment);
                    }
                }
            }
        }
        // The members, along each class's chain.
        for ci in 0..p.classes.len() {
            let c = ClassId(ci as u32);
            if !p.biblioteca_com_augmentations(p.class(c).library) || p.class(c).decl.is_none() {
                continue;
            }
            let mut current: HashMap<String, usize> = HashMap::new();
            for (u, mid) in p.membros_da_classe(c) {
                let a = &p.unit(u).ast;
                let m = a.member(mid);
                for (name, key) in member_names(a, m, |s| self.name(s).to_string()) {
                    let link = Link { unit: u, name: name.span, declaration: m.span, previous: None, next: None };
                    link_(&mut links, &mut current, key, link, m.augment);
                }
            }
        }
        links
    }

    /// The lenses of `unit`: first the "Go to Augmented" ones (the target is the
    /// previous link), then the "Go to Augmentation" ones (the next), in text
    /// order. Each one: the name, the title and the target.
    pub(crate) fn augmentation_lenses(&self, unit: UnitId, augmented: bool, augmentations: bool) -> Vec<(Span, &'static str, UnitId, Span)> {
        let links = self.augmentation_links();
        let mut from_here: Vec<&Link> = links.iter().filter(|e| e.unit == unit).collect();
        from_here.sort_by_key(|e| e.name.start);
        let mut output = Vec::new();
        if augmented {
            for e in &from_here {
                if let Some(a) = e.previous {
                    output.push((e.name, "Go to Augmented", links[a].unit, links[a].name));
                }
            }
        }
        if augmentations {
            for e in &from_here {
                if let Some(a) = e.next {
                    output.push((e.name, "Go to Augmentation", links[a].unit, links[a].name));
                }
            }
        }
        output
    }

    /// `dart/textDocument/augmented` (`next` false) and
    /// `dart/textDocument/augmentation` (true): the link of the declaration that
    /// contains `offset` (the innermost) and its neighbor.
    pub(crate) fn augmentation_neighbor(&self, unit: UnitId, offset: usize, next: bool) -> Option<(UnitId, Span)> {
        let links = self.augmentation_links();
        let e = links
            .iter()
            .filter(|e| e.unit == unit && e.declaration.start <= offset && offset <= e.declaration.end)
            .min_by_key(|e| e.declaration.end - e.declaration.start)?;
        let target = if next { e.next } else { e.previous }?;
        Some((links[target].unit, links[target].name))
    }
}

/// The names declared by a top-level declaration: `(name, is setter)`.
fn declaration_names(a: &ast::Ast, d: &ast::Decl) -> Vec<(ast::Name, bool)> {
    match &d.kind {
        DeclKind::Class(x) => vec![(x.name, false)],
        DeclKind::Mixin(x) => vec![(x.name, false)],
        DeclKind::Enum(x) => vec![(x.name, false)],
        DeclKind::ExtensionType(x) => vec![(x.name, false)],
        DeclKind::Typedef(x) => vec![(x.name, false)],
        DeclKind::Extension(x) => x.name.map(|n| vec![(n, false)]).unwrap_or_default(),
        DeclKind::Function(f) => {
            let f = a.function(*f);
            f.name.map(|n| vec![(n, f.kind == ast::FunctionKind::Setter)]).unwrap_or_default()
        }
        DeclKind::Variables(l) => l.variables.iter().map(|v| (v.name, false)).collect(),
    }
}

/// The names declared by a member, with the chain key (statics apart; the
/// constructor by name).
fn member_names(a: &ast::Ast, m: &ast::Member, name: impl Fn(dartforge_intern::SymbolId) -> String) -> Vec<(ast::Name, String)> {
    match &m.kind {
        MemberKind::Method(f) => {
            let f = a.function(*f);
            let Some(n) = f.name else { return Vec::new() };
            let setter = if f.kind == ast::FunctionKind::Setter { "=" } else { "" };
            vec![(n, format!("{}{}{setter}", if f.static_ { "static " } else { "" }, name(n.sym)))]
        }
        MemberKind::Constructor(k) => {
            let n = k.name.unwrap_or(k.class_name);
            vec![(n, format!("new {}", k.name.map(|x| name(x.sym)).unwrap_or_default()))]
        }
        MemberKind::Field(l) => l
            .variables
            .iter()
            .map(|v| (v.name, format!("{}{}", if l.static_ { "static " } else { "" }, name(v.name.sym))))
            .collect(),
    }
}
