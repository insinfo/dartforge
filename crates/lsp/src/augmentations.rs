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

use crate::projeto::Projeto;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, UnitId};
use dartforge_frontend::ast::{self, DeclKind, MemberKind};
use std::collections::HashMap;

/// Um elo da cadeia: o nome declarado e os vizinhos.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Elo {
    pub unidade: UnitId,
    pub nome: Span,
    /// O span da declaração inteira (o "subir pelos nós" do cursor).
    pub declaracao: Span,
    /// `augmentationTarget`.
    pub anterior: Option<usize>,
    /// `augmentation`.
    pub proximo: Option<usize>,
}

impl Projeto {
    /// Os elos de todas as cadeias das bibliotecas com augmentations.
    pub(crate) fn elos_de_augmentation(&self) -> Vec<Elo> {
        let p = self.programa();
        let mut elos: Vec<Elo> = Vec::new();
        let ligar = |elos: &mut Vec<Elo>, atual: &mut HashMap<String, usize>, chave: String, novo: Elo, augment: bool| {
            let i = elos.len();
            elos.push(novo);
            if augment
                && let Some(&anterior) = atual.get(&chave)
            {
                elos[anterior].proximo = Some(i);
                elos[i].anterior = Some(anterior);
            }
            atual.insert(chave, i);
        };
        for (li, lib) in p.libraries.iter().enumerate() {
            if !p.biblioteca_com_augmentations(dartforge_elements::model::LibraryId(li as u32)) {
                continue;
            }
            // O topo, na ordem de aplicação.
            let mut atual: HashMap<String, usize> = HashMap::new();
            for &u in &lib.units {
                let a = &p.unit(u).ast;
                for &d in &p.unit(u).unit.declarations {
                    let decl = a.decl(d);
                    for (nome, setter) in nomes_da_declaracao(a, decl) {
                        let chave = format!("{}{}", self.nome(nome.sym), if setter { "=" } else { "" });
                        let elo = Elo { unidade: u, nome: nome.span, declaracao: decl.span, anterior: None, proximo: None };
                        ligar(&mut elos, &mut atual, chave, elo, decl.augment);
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
            let mut atual: HashMap<String, usize> = HashMap::new();
            for (u, mid) in p.membros_da_classe(c) {
                let a = &p.unit(u).ast;
                let m = a.member(mid);
                for (nome, chave) in nomes_do_membro(a, m, |s| self.nome(s).to_string()) {
                    let elo = Elo { unidade: u, nome: nome.span, declaracao: m.span, anterior: None, proximo: None };
                    ligar(&mut elos, &mut atual, chave, elo, m.augment);
                }
            }
        }
        elos
    }

    /// As lentes de `unidade`: primeiro os "Go to Augmented" (o alvo é o
    /// anterior), depois os "Go to Augmentation" (o próximo), na ordem do
    /// texto. Cada uma: o nome, o título e o alvo.
    pub(crate) fn lentes_de_augmentation(&self, unidade: UnitId, aumentados: bool, aumentacoes: bool) -> Vec<(Span, &'static str, UnitId, Span)> {
        let elos = self.elos_de_augmentation();
        let mut daqui: Vec<&Elo> = elos.iter().filter(|e| e.unidade == unidade).collect();
        daqui.sort_by_key(|e| e.nome.start);
        let mut saida = Vec::new();
        if aumentados {
            for e in &daqui {
                if let Some(a) = e.anterior {
                    saida.push((e.nome, "Go to Augmented", elos[a].unidade, elos[a].nome));
                }
            }
        }
        if aumentacoes {
            for e in &daqui {
                if let Some(a) = e.proximo {
                    saida.push((e.nome, "Go to Augmentation", elos[a].unidade, elos[a].nome));
                }
            }
        }
        saida
    }

    /// `dart/textDocument/augmented` (`proximo` falso) e
    /// `dart/textDocument/augmentation` (verdadeiro): o elo da declaração que
    /// contém `offset` (a mais interna) e o vizinho dele.
    pub(crate) fn vizinho_de_augmentation(&self, unidade: UnitId, offset: usize, proximo: bool) -> Option<(UnitId, Span)> {
        let elos = self.elos_de_augmentation();
        let e = elos
            .iter()
            .filter(|e| e.unidade == unidade && e.declaracao.start <= offset && offset <= e.declaracao.end)
            .min_by_key(|e| e.declaracao.end - e.declaracao.start)?;
        let alvo = if proximo { e.proximo } else { e.anterior }?;
        Some((elos[alvo].unidade, elos[alvo].nome))
    }
}

/// Os nomes declarados por uma declaração de topo: `(nome, é setter)`.
fn nomes_da_declaracao(a: &ast::Ast, d: &ast::Decl) -> Vec<(ast::Name, bool)> {
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
fn nomes_do_membro(a: &ast::Ast, m: &ast::Member, nome: impl Fn(dartforge_intern::SymbolId) -> String) -> Vec<(ast::Name, String)> {
    match &m.kind {
        MemberKind::Method(f) => {
            let f = a.function(*f);
            let Some(n) = f.name else { return Vec::new() };
            let setter = if f.kind == ast::FunctionKind::Setter { "=" } else { "" };
            vec![(n, format!("{}{}{setter}", if f.static_ { "static " } else { "" }, nome(n.sym)))]
        }
        MemberKind::Constructor(k) => {
            let n = k.name.unwrap_or(k.class_name);
            vec![(n, format!("new {}", k.name.map(|x| nome(x.sym)).unwrap_or_default()))]
        }
        MemberKind::Field(l) => l
            .variables
            .iter()
            .map(|v| (v.name, format!("{}{}", if l.static_ { "static " } else { "" }, nome(v.name.sym))))
            .collect(),
    }
}
