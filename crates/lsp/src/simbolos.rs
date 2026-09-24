//! Símbolos de documento vindos da mesma AST usada pelos diagnósticos.
//! A árvore é temporária: nenhuma revisão anterior permanece no servidor.

use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{Ast, DeclKind, MemberKind, Name};
use dartforge_frontend::LibraryFeatures;
use dartforge_intern::Interner;
use serde_json::{Value, json};

use crate::utf16::TabelaLinhas;

pub(super) fn do_documento(texto: &str, features: LibraryFeatures) -> Vec<Value> {
    let mut nomes = Interner::new();
    let parsed = dartforge_frontend::parser::parse_com(texto, &mut nomes, features);
    let linhas = TabelaLinhas::construir(texto);
    let mut saida = Vec::new();
    for id in &parsed.unit.declarations {
        let decl = parsed.ast.decl(*id);
        let (nome, kind, membros) = match &decl.kind {
            DeclKind::Class(c) => (Some(c.name), 5, Some(c.members.as_slice())),
            DeclKind::Mixin(m) => (Some(m.name), 5, Some(m.members.as_slice())),
            DeclKind::Enum(e) => (Some(e.name), 10, Some(e.members.as_slice())),
            DeclKind::Extension(e) => (e.name, 5, Some(e.members.as_slice())),
            DeclKind::ExtensionType(e) => (Some(e.name), 5, Some(e.members.as_slice())),
            DeclKind::Typedef(t) => (Some(t.name), 5, None),
            DeclKind::Function(f) => {
                let func = parsed.ast.function(*f);
                (func.name, 12, None)
            }
            DeclKind::Variables(v) => {
                for variable in v.variables.iter() {
                    saida.push(simbolo(
                        texto, &linhas, &nomes, variable.name, decl.span,
                        if v.const_ { 14 } else { 13 }, Vec::new(),
                    ));
                }
                continue;
            }
        };
        let Some(nome) = nome else { continue };
        let filhos = membros.map_or_else(Vec::new, |ids| {
            simbolos_membros(texto, &linhas, &nomes, &parsed.ast, ids)
        });
        saida.push(simbolo(texto, &linhas, &nomes, nome, decl.span, kind, filhos));
    }
    saida
}

fn simbolos_membros(
    texto: &str,
    linhas: &TabelaLinhas,
    nomes: &Interner,
    ast: &Ast,
    membros: &[dartforge_frontend::ast::MemberId],
) -> Vec<Value> {
    let mut saida = Vec::new();
    for id in membros {
        let membro = ast.member(*id);
        match &membro.kind {
            MemberKind::Method(f) => {
                let func = ast.function(*f);
                if let Some(nome) = func.name {
                    let kind = match func.kind {
                        dartforge_frontend::ast::FunctionKind::Getter
                        | dartforge_frontend::ast::FunctionKind::Setter => 7,
                        _ => 6,
                    };
                    saida.push(simbolo(texto, linhas, nomes, nome, membro.span, kind, Vec::new()));
                }
            }
            MemberKind::Constructor(c) => {
                let nome = c.name.unwrap_or(c.class_name);
                saida.push(simbolo(texto, linhas, nomes, nome, membro.span, 9, Vec::new()));
            }
            MemberKind::Field(v) => {
                for variable in v.variables.iter() {
                    saida.push(simbolo(
                        texto, linhas, nomes, variable.name, membro.span,
                        if v.const_ { 14 } else { 8 }, Vec::new(),
                    ));
                }
            }
        }
    }
    saida
}

fn simbolo(
    texto: &str,
    linhas: &TabelaLinhas,
    nomes: &Interner,
    nome: Name,
    span: Span,
    kind: u32,
    children: Vec<Value>,
) -> Value {
    let range = intervalo(texto, linhas, span);
    let selection_range = intervalo(texto, linhas, nome.span);
    json!({
        "name": nomes.resolve(nome.sym),
        "kind": kind,
        "range": range,
        "selectionRange": selection_range,
        "children": children,
    })
}

fn intervalo(texto: &str, linhas: &TabelaLinhas, span: Span) -> Value {
    let inicio = span.start.min(texto.len());
    let fim = span.end.max(inicio).min(texto.len());
    let (l0, c0) = linhas.posicao_de_offset(texto, inicio);
    let (l1, c1) = linhas.posicao_de_offset(texto, fim);
    json!({
        "start": {"line": l0, "character": c0},
        "end": {"line": l1, "character": c1},
    })
}
