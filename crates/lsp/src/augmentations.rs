//! As cadeias de augmentation no LSP (docs/LSP-ESPECIFICACAO.md §8.3 e §8.6):
//! o `AugmentationCodeLensProvider` ("Go to Augmented" e "Go to
//! Augmentation") e os métodos `dart/textDocument/augmented` e
//! `dart/textDocument/augmentation` do 3.6.2
//! (`AS:src/lsp/handlers/code_lens/augmentations.dart`,
//! `custom/handler_augmented.dart`, `custom/handler_augmentation.dart`).
//!
//! A cadeia é a ordem de aplicação (docs/AUGMENTATIONS.md): as unidades da
//! biblioteca na pré-ordem da árvore de partes, as declarações na ordem do
//! texto, e, numa classe, os membros da declaração introdutória seguidos dos
//! de cada `augment class`. Uma declaração `augment` liga-se à anterior de
//! mesma chave (nome; setter à parte; estático à parte num membro; o nome do
//! construtor): é o `augmentationTarget` dela, e ela é o `augmentation` da
//! anterior. Uma declaração comum começa cadeia nova. Só nas bibliotecas com
//! augmentations ligadas.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::project::Project;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, UnitId};
use dartforge_frontend::ast::{self, DeclKind, MemberKind};
use std::collections::HashMap;

/// Um elo da cadeia: o nome declarado e os vizinhos.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Link {
    pub unit: UnitId,
    pub name: Span,
    /// O span da declaração inteira (o "subir pelos nós" do cursor).
    pub declaration: Span,
    /// `augmentationTarget`.
    pub previous: Option<usize>,
    /// `augmentation`.
    pub next: Option<usize>,
}

impl Project {
    /// Os elos de todas as cadeias das bibliotecas com augmentations.
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
            // O topo, na ordem de aplicação.
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
        // Os membros, pela cadeia de cada classe.
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

    /// As lentes de `unit`: primeiro os "Go to Augmented" (o alvo é o
    /// anterior), depois os "Go to Augmentation" (o próximo), na ordem do
    /// texto. Cada uma: o nome, o título e o alvo.
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

    /// `dart/textDocument/augmented` (`next` falso) e
    /// `dart/textDocument/augmentation` (verdadeiro): o elo da declaração que
    /// contém `offset` (a mais interna) e o vizinho dele.
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

/// Os nomes declarados por uma declaração de topo: `(name, é setter)`.
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

/// Os nomes declarados por um membro, com a chave da cadeia (estático à
/// parte; o construtor pelo nome).
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
