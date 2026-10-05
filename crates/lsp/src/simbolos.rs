//! Símbolos de documento vindos da mesma AST usada pelos diagnósticos.
//! A árvore é temporária: nenhuma revisão anterior permanece no servidor.

use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{Ast, DeclKind, DirectiveKind, ExprKind, MemberKind, Name, StmtKind, TypeKind, TypedefKind};
use dartforge_frontend::LibraryFeatures;
use dartforge_intern::Interner;
use serde_json::{Value, json};

use crate::utf16::TabelaLinhas;

pub(super) fn do_documento(texto: &str, features: LibraryFeatures) -> Vec<Value> {
    let mut nomes = Interner::new();
    let parsed = dartforge_frontend::parser::parse_com(texto, &mut nomes, features);
    let linhas = TabelaLinhas::construir(texto);
    let mut saida = Vec::new();
    // Os símbolos de `group`/`test`: só na unidade que importa um arquivo
    // `…test.dart` e não declara função de topo com um desses nomes.
    let importa_testes = parsed.unit.directives.iter().any(|d| match &d.kind {
        DirectiveKind::Import { uri, .. } => dartforge_elements::load::string_lit_value(uri).is_some_and(|u| u.ends_with("test.dart")),
        _ => false,
    });
    let declara_homonimo = parsed.unit.declarations.iter().any(|d| match &parsed.ast.decl(*d).kind {
        DeclKind::Function(f) => parsed.ast.function(*f).name.is_some_and(|n| matches!(nomes.resolve(n.sym), "group" | "test")),
        _ => false,
    });
    let testes = importa_testes && !declara_homonimo;
    for id in &parsed.unit.declarations {
        let decl = parsed.ast.decl(*id);
        let (nome, kind, membros) = match &decl.kind {
            DeclKind::Class(c) => (Some(c.name), 5, Some(c.members.as_slice())),
            DeclKind::Mixin(m) => (Some(m.name), 5, Some(m.members.as_slice())),
            DeclKind::Enum(e) => {
                let mut filhos = e.constants.iter().map(|c| {
                    simbolo(texto, &linhas, &nomes, c.name, c.span, 22, Vec::new())
                }).collect::<Vec<_>>();
                filhos.extend(simbolos_membros(
                    texto, &linhas, &nomes, &parsed.ast, &e.members, testes,
                ));
                saida.push(simbolo(texto, &linhas, &nomes, e.name, decl.span, 10, filhos));
                continue;
            }
            // Extensões e extension types são `Namespace` no servidor do Dart.
            DeclKind::Extension(e) => (e.name, 3, Some(e.members.as_slice())),
            DeclKind::ExtensionType(e) => (Some(e.name), 3, Some(e.members.as_slice())),
            // `typedef F = void Function();` e a forma antiga são
            // FUNCTION_TYPE_ALIAS (5); `typedef A = int;` é TYPE_ALIAS, que
            // o `elementKindToSymbolKind` do Dart não trata e sai 19.
            DeclKind::Typedef(t) => {
                let de_funcao = match &t.kind {
                    TypedefKind::Legacy { .. } => true,
                    TypedefKind::Alias(ty) => matches!(parsed.ast.ty(*ty).kind, TypeKind::Function { .. }),
                };
                (Some(t.name), if de_funcao { 5 } else { 19 }, None)
            }
            DeclKind::Function(f) => {
                let func = parsed.ast.function(*f);
                let kind = match func.kind {
                    dartforge_frontend::ast::FunctionKind::Getter | dartforge_frontend::ast::FunctionKind::Setter => 7,
                    _ => 12,
                };
                (func.name, kind, None)
            }
            DeclKind::Variables(v) => {
                for variable in v.variables.iter() {
                    saida.push(simbolo(
                        texto, &linhas, &nomes, variable.name, decl.span,
                        13, Vec::new(),
                    ));
                }
                continue;
            }
        };
        let Some(nome) = nome else { continue };
        let filhos = match membros {
            Some(ids) => simbolos_membros(texto, &linhas, &nomes, &parsed.ast, ids, testes),
            // Função de topo: as funções locais do corpo, recursivamente.
            None if matches!(decl.kind, DeclKind::Function(_)) => funcoes_locais(texto, &linhas, &nomes, &parsed.ast, decl.span, testes),
            None => Vec::new(),
        };
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
    testes: bool,
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
                    let locais = funcoes_locais(texto, linhas, nomes, ast, membro.span, testes);
                    saida.push(simbolo(texto, linhas, nomes, nome, membro.span, kind, locais));
                }
            }
            MemberKind::Constructor(c) => {
                // Como o outline do Dart: `Classe.nome` (ou `Classe`).
                let nome = c.name.unwrap_or(c.class_name);
                let locais = funcoes_locais(texto, linhas, nomes, ast, membro.span, testes);
                let mut s = simbolo(texto, linhas, nomes, nome, membro.span, 9, locais);
                if let Some(n) = c.name {
                    s["name"] = json!(format!("{}.{}", nomes.resolve(c.class_name.sym), nomes.resolve(n.sym)));
                }
                saida.push(s);
            }
            MemberKind::Field(v) => {
                for variable in v.variables.iter() {
                    saida.push(simbolo(
                        texto, linhas, nomes, variable.name, membro.span,
                        8, Vec::new(),
                    ));
                }
            }
        }
    }
    saida
}

/// As funções locais e os testes declarados dentro de `dentro` (o corpo de
/// uma função, método ou construtor), como filhos: o outline do Dart os
/// lista recursivamente, inclusive os de dentro de closures
/// (`computer_outline.dart:450-548`). A árvore sai da contenção dos
/// intervalos: um item é filho do menor que o contém.
///
/// Com `testes`, uma chamada `group(...)` ou `test(...)` vira o símbolo
/// `group("descrição")` (espécie 6), com o intervalo da chamada e a seleção
/// no nome; o que está dentro de um `group` é filho dele, e nada de dentro de
/// um `test` é listado. O original decide pelo elemento resolvido (a função
/// de topo de um arquivo `…test.dart`, ou uma função com `@isTest` ou
/// `@isTestGroup`); aqui vale a chamada sem prefixo numa unidade que importa
/// um `…test.dart` e não declara função de topo com esse nome.
fn funcoes_locais(texto: &str, linhas: &TabelaLinhas, nomes: &Interner, ast: &Ast, dentro: Span, testes: bool) -> Vec<Value> {
    /// Um item do outline de um corpo.
    struct Item {
        span: Span,
        nome: Name,
        /// O nome mostrado de um teste (`group("x")`); `None` numa função.
        de_teste: Option<String>,
        /// Um `test`: nada de dentro dele entra.
        folha: bool,
    }
    let mut achadas: Vec<Item> = Vec::new();
    for s in ast.stmts.iter() {
        let StmtKind::Function(f) = &s.kind else { continue };
        if s.span.start < dentro.start || s.span.end > dentro.end {
            continue;
        }
        if let Some(nome) = ast.function(*f).name {
            achadas.push(Item { span: s.span, nome, de_teste: None, folha: false });
        }
    }
    if testes {
        for e in ast.exprs.iter() {
            let ExprKind::Call { target, arguments } = &e.kind else { continue };
            if e.span.start < dentro.start || e.span.end > dentro.end {
                continue;
            }
            let ExprKind::Identifier(nome) = &ast.expr(*target).kind else { continue };
            let chamada = nomes.resolve(nome.sym);
            if chamada != "group" && chamada != "test" {
                continue;
            }
            // `extractString`: o valor do literal de string, senão o texto do
            // primeiro argumento, senão `unnamed`.
            let descricao = match arguments.args.first() {
                None => "unnamed".to_string(),
                Some(primeiro) => {
                    let expr = ast.expr(primeiro.value);
                    let valor = match &expr.kind {
                        ExprKind::String(lit) => dartforge_elements::load::string_lit_value(lit),
                        _ => None,
                    };
                    valor.unwrap_or_else(|| texto.get(expr.span.start..expr.span.end).unwrap_or("").to_string())
                }
            };
            achadas.push(Item { span: e.span, nome: *nome, de_teste: Some(format!("{chamada}(\"{descricao}\")")), folha: chamada == "test" });
        }
    }
    // Pré-ordem: pelo começo e, no empate, a maior primeiro.
    achadas.sort_by(|a, b| a.span.start.cmp(&b.span.start).then(b.span.end.cmp(&a.span.end)));
    fn montar(texto: &str, linhas: &TabelaLinhas, nomes: &Interner, achadas: &[Item], i: &mut usize, limite: usize) -> Vec<Value> {
        let mut saida = Vec::new();
        while *i < achadas.len() && achadas[*i].span.start < limite {
            let item = &achadas[*i];
            *i += 1;
            let filhos = if item.folha {
                // Pula tudo o que está dentro do teste.
                while *i < achadas.len() && achadas[*i].span.start < item.span.end {
                    *i += 1;
                }
                Vec::new()
            } else {
                montar(texto, linhas, nomes, achadas, i, item.span.end)
            };
            let mut s = simbolo(texto, linhas, nomes, item.nome, item.span, if item.de_teste.is_some() { 6 } else { 12 }, filhos);
            if let Some(mostrado) = &item.de_teste {
                s["name"] = json!(mostrado);
            }
            saida.push(s);
        }
        saida
    }
    let mut i = 0;
    montar(texto, linhas, nomes, &achadas, &mut i, dentro.end)
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
