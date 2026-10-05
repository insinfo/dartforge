//! Troca do texto de uma unidade sem recarregar o programa (o completar do
//! LSP): reanalisa a unidade e, se as declarações continuam as mesmas (só o
//! interior dos corpos mudou), troca a árvore e o texto no lugar. Os
//! elementos guardam ids das arenas de declarações, membros e funções
//! (`DeclRef`, `FunctionRef`, `VariableRef`), então a troca só é aceita se
//! essas arenas mantêm o tamanho e cada declaração/membro/função nomeada
//! mantém o nome e a espécie; as arenas de expressões, comandos, tipos e
//! padrões podem mudar à vontade (as tabelas laterais dos corpos são
//! refeitas pela inferência).

use crate::model::{Program, UnitId};
use dartforge_frontend::ast::{Ast, CompilationUnit, DeclKind, MemberKind};
use dartforge_intern::Interner;

/// Por que a troca foi recusada (o chamador recarrega o programa).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recusa {
    /// A nova fonte tem erro léxico ou sintático novo que muda declarações.
    FormaMudou(&'static str),
}

/// O que a troca substituiu, para desfazer com [`restaurar_unidade`].
pub struct Anterior {
    unidade: UnitId,
    source: String,
    ast: Ast,
    unit: CompilationUnit,
    pulados: Vec<dartforge_diagnostics::Span>,
    referencia: dartforge_diagnostics::Referencia,
}

/// Troca o texto da unidade `u` por `texto` quando só o interior de corpos
/// mudou; devolve o anterior para desfazer.
pub fn substituir_unidade(programa: &mut Program, nomes: &mut Interner, u: UnitId, texto: &str) -> Result<Anterior, Recusa> {
    let features = programa.unit(u).features;
    let novo = dartforge_frontend::parser::parse_com(texto, nomes, features);
    {
        let velho = &programa.unit(u).ast;
        let a = &novo.ast;
        if velho.decls.len() != a.decls.len() {
            return Err(Recusa::FormaMudou("declarações"));
        }
        if velho.members.len() != a.members.len() {
            return Err(Recusa::FormaMudou("membros"));
        }
        if velho.functions.len() != a.functions.len() {
            return Err(Recusa::FormaMudou("funções"));
        }
        for (d0, d1) in velho.decls.iter().zip(a.decls.iter()) {
            if std::mem::discriminant(&d0.kind) != std::mem::discriminant(&d1.kind) || nome_da_decl(&d0.kind) != nome_da_decl(&d1.kind) {
                return Err(Recusa::FormaMudou("declaração"));
            }
        }
        for (m0, m1) in velho.members.iter().zip(a.members.iter()) {
            if std::mem::discriminant(&m0.kind) != std::mem::discriminant(&m1.kind) || nome_do_membro(&m0.kind) != nome_do_membro(&m1.kind) {
                return Err(Recusa::FormaMudou("membro"));
            }
        }
        for (f0, f1) in velho.functions.iter().zip(a.functions.iter()) {
            if f0.name.map(|n| n.sym) != f1.name.map(|n| n.sym) {
                return Err(Recusa::FormaMudou("função"));
            }
        }
        if programa.unit(u).unit.declarations.len() != novo.unit.declarations.len() {
            return Err(Recusa::FormaMudou("declarações da unidade"));
        }
    }
    let un = &mut programa.units[u.0 as usize];
    let anterior = Anterior {
        unidade: u,
        source: std::mem::replace(&mut un.source, texto.to_string()),
        ast: std::mem::replace(&mut un.ast, novo.ast),
        unit: std::mem::replace(&mut un.unit, novo.unit),
        pulados: std::mem::replace(&mut un.pulados, novo.pulados),
        referencia: std::mem::replace(&mut un.referencia, novo.referencia),
    };
    Ok(anterior)
}

/// Desfaz uma [`substituir_unidade`].
pub fn restaurar_unidade(programa: &mut Program, anterior: Anterior) {
    let un = &mut programa.units[anterior.unidade.0 as usize];
    un.source = anterior.source;
    un.ast = anterior.ast;
    un.unit = anterior.unit;
    un.pulados = anterior.pulados;
    un.referencia = anterior.referencia;
}

fn nome_da_decl(k: &DeclKind) -> Option<dartforge_intern::SymbolId> {
    match k {
        DeclKind::Class(x) => Some(x.name.sym),
        DeclKind::Mixin(x) => Some(x.name.sym),
        DeclKind::Enum(x) => Some(x.name.sym),
        DeclKind::Extension(x) => x.name.map(|n| n.sym),
        DeclKind::ExtensionType(x) => Some(x.name.sym),
        DeclKind::Typedef(x) => Some(x.name.sym),
        DeclKind::Variables(l) => l.variables.first().map(|v| v.name.sym),
        _ => None,
    }
}

fn nome_do_membro(k: &MemberKind) -> Option<dartforge_intern::SymbolId> {
    match k {
        MemberKind::Constructor(c) => c.name.map(|n| n.sym),
        MemberKind::Field(l) => l.variables.first().map(|v| v.name.sym),
        _ => None,
    }
}
