//! Hierarquia de chamadas (`textDocument/prepareCallHierarchy`,
//! `callHierarchy/incomingCalls`, `callHierarchy/outgoingCalls`) como o
//! `DartCallHierarchyComputer` do Dart 3.6.2
//! (`pkg/analysis_server/lib/src/computer/computer_call_hierarchy.dart`):
//!
//! * o alvo é um executável não sintético sob o cursor (função, método,
//!   operador, getter ou setter escritos, construtor; o construtor sem nome
//!   implícito fica na classe): o nome exibido é o nome (`get x`/`set x` nos
//!   acessores, `A.nome` ou `A` nos construtores), o contêiner a classe ou o
//!   arquivo, `range` a declaração e `selectionRange` o nome;
//! * chamadas recebidas: as referências ao elemento (a família de
//!   sobrescrita inclusa, como as `references`), agrupadas pelo executável
//!   que as contém (ou pela classe, num inicializador de campo, ou pelo
//!   arquivo, num de variável de topo), com o intervalo de cada uma;
//! * chamadas feitas: no corpo do alvo, cada chamada, criação e leitura de
//!   getter escrito, agrupadas pelo executável chamado, com o intervalo do
//!   nome.

use crate::projeto::{Alvo, Concreto, Projeto};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Element, FunctionElementId, FunctionKind, FunctionRef, UnitId};
use dartforge_frontend::ast::{DeclKind, ExprKind, MemberKind};
use dartforge_types::{MemberRef, Resolved};
use std::collections::BTreeMap;

/// Um item da hierarquia de chamadas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDeChamada {
    pub nome: String,
    /// `SymbolKind` do LSP (12 função, 6 método, 9 construtor, 7
    /// propriedade, 5 classe, 1 arquivo).
    pub especie: u8,
    pub detalhe: Option<String>,
    pub uri: String,
    pub intervalo: Span,
    pub selecao: Span,
}

/// O dono de uma referência: um executável, uma classe ou o arquivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Conteiner {
    Funcao(FunctionElementId),
    Classe(dartforge_elements::model::ClassId),
    Arquivo(UnitId),
}

impl Projeto {
    /// O executável sob `offset`.
    pub(crate) fn executavel_em(&self, unidade: UnitId, offset: usize) -> Option<FunctionElementId> {
        let d = self.identificar(unidade, offset).ok()??;
        let f = match (&d.alvo, d.concreto) {
            (_, Some(Concreto::Funcao(f))) => f,
            (Alvo::Construtor(f), _) => *f,
            _ => return None,
        };
        // Acessor sintético de campo não tem código.
        (self.programa().function(f).kind != FunctionKind::ImplicitAccessor).then_some(f)
    }

    /// O item de um executável.
    pub(crate) fn item_de_chamada(&self, f: FunctionElementId) -> Option<ItemDeChamada> {
        let p = self.programa();
        let fe = p.function(f);
        let nome = self.nome(fe.name);
        let base = crate::projeto::nome_base(nome).to_string();
        let (nome, especie) = match fe.kind {
            FunctionKind::Getter => (format!("get {base}"), 7),
            FunctionKind::Setter => (format!("set {base}"), 7),
            FunctionKind::Constructor | FunctionKind::SyntheticConstructor => {
                let classe = fe.class.map_or(String::new(), |c| self.nome(p.class(c).name).to_string());
                (if base.is_empty() { classe } else { format!("{classe}.{base}") }, 9)
            }
            FunctionKind::Function if fe.class.is_none() && fe.extension.is_none() => (base, 12),
            _ => (base, 6),
        };
        let (u, selecao) = self.nome_da_funcao(f)?;
        let intervalo = match fe.node {
            FunctionRef::Function { unit, function } => {
                let ast = &p.unit(unit).ast;
                ast.decls
                    .iter()
                    .find(|d| matches!(d.kind, DeclKind::Function(x) if x == function))
                    .map(|d| d.span)
                    .or_else(|| ast.members.iter().find(|m| matches!(m.kind, MemberKind::Method(x) if x == function)).map(|m| m.span))
                    .unwrap_or(ast.function(function).span)
            }
            FunctionRef::Constructor { unit, member } => p.unit(unit).ast.member(member).span,
            FunctionRef::None => self.intervalo_de_declaracao(u, selecao),
        };
        let detalhe = match (fe.class, fe.extension) {
            (Some(c), _) => Some(self.nome(p.class(c).name).to_string()),
            (None, Some(x)) => p.extension(x).name.map(|n| self.nome(n).to_string()),
            _ => p.unit(u).path.as_ref().and_then(|c| c.file_name()).map(|n| n.to_string_lossy().into_owned()),
        };
        Some(ItemDeChamada { nome, especie, detalhe, uri: self.uri_da_unidade(u)?, intervalo, selecao })
    }

    /// A declaração de topo ou membro que contém `nome`.
    fn intervalo_de_declaracao(&self, u: UnitId, nome: Span) -> Span {
        let ast = &self.programa().unit(u).ast;
        ast.members
            .iter()
            .map(|m| m.span)
            .chain(ast.decls.iter().map(|d| d.span))
            .filter(|s| s.start <= nome.start && nome.end <= s.end)
            .min_by_key(|s| s.end - s.start)
            .unwrap_or(nome)
    }

    fn item_de_conteiner(&self, c: Conteiner) -> Option<ItemDeChamada> {
        let p = self.programa();
        match c {
            Conteiner::Funcao(f) => self.item_de_chamada(f),
            Conteiner::Classe(k) => {
                let item = self.item_de_tipo(k, Some(self.nome(p.class(k).name).to_string()))?;
                let detalhe = p.class(k).decl.and_then(|d| p.unit(d.unit).path.as_ref()?.file_name().map(|n| n.to_string_lossy().into_owned()));
                Some(ItemDeChamada { nome: item.nome, especie: 5, detalhe, uri: item.uri, intervalo: item.intervalo, selecao: item.selecao })
            }
            Conteiner::Arquivo(u) => {
                let nome = p.unit(u).path.as_ref()?.file_name()?.to_string_lossy().into_owned();
                let fim = p.unit(u).source.len();
                Some(ItemDeChamada { nome, especie: 1, detalhe: None, uri: self.uri_da_unidade(u)?, intervalo: Span { start: 0, end: fim }, selecao: Span { start: 0, end: 0 } })
            }
        }
    }

    /// O executável (ou classe, ou arquivo) que contém `offset` de `u`.
    fn conteiner_em(&self, u: UnitId, offset: usize) -> Conteiner {
        let p = self.programa();
        let ast = &p.unit(u).ast;
        // A função declarada (não expressão de função) mais interna.
        let mut melhor: Option<(usize, FunctionElementId)> = None;
        for (i, fe) in p.functions.iter().enumerate() {
            let span = match fe.node {
                FunctionRef::Function { unit, function } if unit == u => {
                    let f = ast.function(function);
                    if f.name.is_none() {
                        continue;
                    }
                    f.span
                }
                FunctionRef::Constructor { unit, member } if unit == u => ast.member(member).span,
                _ => continue,
            };
            if span.start <= offset && offset < span.end && melhor.is_none_or(|(t, _)| span.end - span.start < t) {
                melhor = Some((span.end - span.start, FunctionElementId(i as u32)));
            }
        }
        if let Some((_, f)) = melhor {
            return Conteiner::Funcao(f);
        }
        let classe = (0..p.classes.len()).map(|i| dartforge_elements::model::ClassId(i as u32)).find(|c| {
            p.class(*c).decl.is_some_and(|d| d.unit == u && {
                let s = ast.decl(d.decl).span;
                s.start <= offset && offset < s.end
            })
        });
        match classe {
            Some(c) => Conteiner::Classe(c),
            None => Conteiner::Arquivo(u),
        }
    }

    /// Chamadas recebidas por `f`: (quem chama, intervalos na unidade dele).
    pub(crate) fn chamadas_recebidas(&self, f: FunctionElementId) -> Vec<(ItemDeChamada, Vec<Span>)> {
        let alvo = match self.programa().function(f).kind {
            FunctionKind::Constructor | FunctionKind::SyntheticConstructor => Alvo::Construtor(f),
            _ => self.membro_de_funcao(f),
        };
        let declaracoes = self.declaracoes(&alvo);
        let Ok(ocorrencias) = self.ocorrencias(&alvo, false) else { return Vec::new() };
        let mut grupos: BTreeMap<Conteiner, Vec<Span>> = BTreeMap::new();
        for (u, de, ate) in ocorrencias {
            if declaracoes.contains(&(u, de, ate)) {
                continue;
            }
            grupos.entry(self.conteiner_em(u, de)).or_default().push(Span { start: de, end: ate });
        }
        grupos.into_iter().filter_map(|(c, spans)| Some((self.item_de_conteiner(c)?, spans))).collect()
    }

    /// Chamadas feitas no corpo de `f`: (o chamado, intervalos no corpo).
    pub(crate) fn chamadas_feitas(&self, f: FunctionElementId) -> Vec<(ItemDeChamada, Vec<Span>)> {
        let p = self.programa();
        let (u, corpo) = match p.function(f).node {
            FunctionRef::Function { unit, function } => (unit, p.unit(unit).ast.function(function).span),
            FunctionRef::Constructor { unit, member } => (unit, p.unit(unit).ast.member(member).span),
            FunctionRef::None => return Vec::new(),
        };
        let ast = &p.unit(u).ast;
        let corpos = &self.consulta.corpos.units[u.0 as usize];
        let mut grupos: BTreeMap<FunctionElementId, Vec<Span>> = BTreeMap::new();
        for (i, e) in ast.exprs.iter().enumerate() {
            if !(corpo.start <= e.span.start && e.span.end <= corpo.end) {
                continue;
            }
            let id = dartforge_frontend::ast::ExprId(i as u32);
            let nome = match &e.kind {
                ExprKind::Identifier(n) => n.span,
                ExprKind::Property { name, .. } => name.span,
                ExprKind::InstanceCreation { ty, constructor, .. } => constructor.map_or(ast.ty(*ty).span, |c| c.span),
                ExprKind::Call { .. } => match corpos.get_resolved(id) {
                    // `A(…)` sem `new`: o construtor fica na chamada.
                    Some(Resolved::Constructor(_)) => e.span,
                    _ => continue,
                },
                _ => continue,
            };
            let chamado = match corpos.get_resolved(id) {
                Some(Resolved::Constructor(c)) => *c,
                Some(Resolved::Member { member: MemberRef::Function(c), .. }) | Some(Resolved::ExtensionMember { member: c, .. }) => *c,
                Some(Resolved::Element(Element::Function(c))) => *c,
                _ => continue,
            };
            if p.function(chamado).kind == FunctionKind::ImplicitAccessor {
                continue;
            }
            let nome = if let ExprKind::Call { target, .. } = &e.kind {
                ast.expr(*target).span
            } else {
                nome
            };
            grupos.entry(chamado).or_default().push(nome);
        }
        grupos.into_iter().filter_map(|(c, spans)| Some((self.item_de_chamada(c)?, spans))).collect()
    }
}
