//! `textDocument/codeAction`: correções rápidas.
//!
//! * **Inserir `;`** para o `expected_token` "Expected to find ';'." que o
//!   parser publica, no fim do intervalo do diagnóstico — a correção
//!   `dart.fix.insertSemicolon` do servidor do Dart, com o mesmo título.
//! * **Importar biblioteca** para um nome indefinido no intervalo pedido
//!   (identificador ou tipo que a inferência comum deixou sem resolução e que
//!   o escopo da biblioteca não conhece): as bibliotecas do SDK que o
//!   declaram (índice de nomes públicos do SDK, montado uma vez) e as do
//!   projeto (arquivos sob a raiz com `pubspec.yaml`), como
//!   `quickfix.import.librarySdk` e `quickfix.import.libraryProject1`.
//!
//! * **Correções dos códigos semânticos publicados** (os de
//!   `crates/analise/verificados.txt` que o servidor do Dart 3.6.2 corrige),
//!   sobre os diagnósticos imediatos e os tipados da versão vigente, com os
//!   títulos e espécies do Dart ([`corrigir_publicados`]).
//! * **Assistência** `Add type annotation` ([`assistencias`]).
//!
//! Dos códigos publicados, os três de enum
//! (`enum_constant_same_name_as_enclosing`, `enum_with_name_values`,
//! `values_declaration_in_enum`) não têm correção no servidor do Dart 3.6.2
//! (conferido com o `dart language-server`), e este também não oferece;
//! nenhuma correção é inventada.

use crate::consulta::Consulta;
use crate::indice::{IndiceProjeto, IndiceSdk, nome_do_pacote, uri_de_import};
use crate::projeto::{Projeto, raiz_do_projeto};
use crate::{DocumentStore, Edicao};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{Element, UnitId};
use dartforge_frontend::ast::{self, DeclKind, DirectiveKind, ExprKind};
use dartforge_types::{MemberRef, Resolved};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use url::Url;

/// Uma ação de código oferecida ao editor.
#[derive(Debug, Clone)]
pub struct AcaoDeCodigo {
    /// Título mostrado (`Insert ';'`, `Import library 'dart:math'`).
    pub titulo: String,
    /// `CodeActionKind` (`quickfix.insertSemicolon`, ...).
    pub especie: String,
    /// Edições do `WorkspaceEdit`.
    pub edicoes: Vec<Edicao>,
    /// Diagnóstico corrigido, quando a ação responde a um publicado.
    pub diagnostico: Option<Diagnostic>,
}

/// Correções dos diagnósticos sintáticos que tocam `inicio..fim`.
pub(crate) fn corrigir_sintaxe(
    uri: &str,
    diagnosticos: &[Diagnostic],
    inicio: usize,
    fim: usize,
) -> Vec<AcaoDeCodigo> {
    let mut saida = Vec::new();
    for d in diagnosticos {
        let toca = d.span.start <= fim && inicio <= d.span.end;
        let ponto_e_virgula =
            d.code.is_some_and(|c| c.info().nome == "expected_token") && d.message.contains("';'");
        if toca && ponto_e_virgula {
            saida.push(AcaoDeCodigo {
                titulo: "Insert ';'".into(),
                especie: "quickfix.insertSemicolon".into(),
                edicoes: vec![Edicao {
                    uri: uri.to_string(),
                    span: Span {
                        start: d.span.end,
                        end: d.span.end,
                    },
                    texto: ";".into(),
                }],
                diagnostico: Some(d.clone()),
            });
        }
    }
    saida
}

/// Grupo de ordenação das diretivas (`dart:`, `package:`, relativas).
fn grupo_de_uri(uri: &str) -> u8 {
    if uri.starts_with("dart:") {
        0
    } else if uri.starts_with("package:") {
        1
    } else {
        2
    }
}

/// Edição que acrescenta `import 'uri';` na ordem das diretivas existentes.
pub(crate) fn inserir_import(
    texto: &str,
    unit: &ast::CompilationUnit,
    uri_novo: &str,
) -> (Span, String) {
    let chave_nova = (grupo_de_uri(uri_novo), uri_novo.to_string());
    let imports: Vec<(&ast::Directive, String)> = unit
        .directives
        .iter()
        .filter_map(|d| match &d.kind {
            DirectiveKind::Import { uri, .. } => {
                Some((d, dartforge_elements::load::string_lit_value(uri)?))
            }
            _ => None,
        })
        .collect();
    if let Some((d, _)) = imports
        .iter()
        .find(|(_, u)| (grupo_de_uri(u), u.clone()) > chave_nova)
    {
        let inicio = texto[..d.span.start].rfind('\n').map_or(0, |i| i + 1);
        return (
            Span {
                start: inicio,
                end: inicio,
            },
            format!("import '{uri_novo}';\n"),
        );
    }
    if let Some((d, _)) = imports.last() {
        return (
            Span {
                start: d.span.end,
                end: d.span.end,
            },
            format!("\nimport '{uri_novo}';"),
        );
    }
    if let Some(d) = unit
        .directives
        .iter()
        .find(|d| matches!(d.kind, DirectiveKind::Library { .. }))
    {
        return (
            Span {
                start: d.span.end,
                end: d.span.end,
            },
            format!("\n\nimport '{uri_novo}';"),
        );
    }
    let inicio = unit.script_tag.map_or(0, |s| {
        texto[s.end..]
            .find('\n')
            .map_or(texto.len(), |i| s.end + i + 1)
    });
    (
        Span {
            start: inicio,
            end: inicio,
        },
        format!("import '{uri_novo}';\n\n"),
    )
}

/// Nomes indefinidos que tocam `inicio..fim`: identificadores sem resolução
/// que o escopo da biblioteca não conhece e nomes de tipo não encontrados.
fn indefinidos(
    consulta: &Consulta,
    unidade: dartforge_elements::model::UnitId,
    inicio: usize,
    fim: usize,
) -> Vec<String> {
    let u = consulta.programa.unit(unidade);
    let corpos = &consulta.corpos.units[unidade.0 as usize];
    let lib = u.library;
    let toca = |s: Span| s.start <= fim && inicio <= s.end;
    let mut saida = BTreeSet::new();
    for (i, e) in u.ast.exprs.iter().enumerate() {
        if let ExprKind::Identifier(n) = &e.kind
            && toca(n.span)
            && corpos.get_resolved(ast::ExprId(i as u32)).is_none()
            && consulta.programa.lookup(lib, n.sym).is_none()
        {
            saida.insert(consulta.nome(n.sym).to_string());
        }
    }
    for t in &u.ast.types {
        if let ast::TypeKind::Named { name, .. } = &t.kind
            && let [n] = &name[..]
            && toca(n.span)
            && consulta.programa.lookup(lib, n.sym).is_none()
        {
            let texto = consulta.nome(n.sym);
            if !["dynamic", "Never", "void", "Function", "Record"].contains(&texto)
                && !tipo_param(&u.ast, n)
            {
                saida.insert(texto.to_string());
            }
        }
    }
    saida.into_iter().filter(|n| !n.starts_with('_')).collect()
}

/// `n` é um parâmetro de tipo de alguma declaração que o contém.
fn tipo_param(ast: &ast::Ast, n: &ast::Name) -> bool {
    let tem = |ps: &[ast::TypeParameter]| ps.iter().any(|t| t.name.sym == n.sym);
    let dentro = |s: Span| s.start <= n.span.start && n.span.start < s.end;
    ast.decls.iter().any(|d| {
        dentro(d.span)
            && match &d.kind {
                DeclKind::Class(c) => tem(&c.type_params),
                DeclKind::Mixin(m) => tem(&m.type_params),
                DeclKind::Enum(e) => tem(&e.type_params),
                DeclKind::Extension(x) => tem(&x.type_params),
                DeclKind::ExtensionType(x) => tem(&x.type_params),
                DeclKind::Typedef(t) => tem(&t.type_params),
                _ => false,
            }
    }) || ast
        .functions
        .iter()
        .any(|f| dentro(f.span) && tem(&f.type_params))
}

/// Ações de importar biblioteca para os nomes indefinidos em `inicio..fim`,
/// sobre a biblioteca do documento já carregada (`projeto`, da sessão):
/// as bibliotecas do SDK (`indice`) e as do projeto (`indice_projeto`) que
/// declaram o nome e ainda não são importadas.
pub(crate) fn importar(
    projeto: &Projeto,
    indice: &IndiceSdk,
    indice_projeto: &mut IndiceProjeto,
    documentos: &DocumentStore,
    uri: &str,
    inicio: usize,
    fim: usize,
) -> Vec<AcaoDeCodigo> {
    let Some(texto) = documentos.get(uri) else {
        return Vec::new();
    };
    let Some(unidade) = projeto.unidade_do_uri(uri) else {
        return Vec::new();
    };
    let consulta = &projeto.consulta;
    let lib = consulta.programa.unit(unidade).library;
    let faltando = indefinidos(consulta, unidade, inicio, fim);
    if faltando.is_empty() {
        return Vec::new();
    }
    let importadas: BTreeSet<String> = consulta
        .programa
        .library(lib)
        .imports
        .iter()
        .map(|i| consulta.programa.library(i.library).uri.clone())
        .collect();
    let unit = &consulta.programa.unit(unidade).unit;
    let Some(arquivo) = crate::projeto::arquivo_da_uri(uri) else {
        return Vec::new();
    };
    let mut saida = Vec::new();

    // SDK.
    let mut do_sdk: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for nome in &faltando {
        for l in indice.por_nome.get(nome).into_iter().flat_map(|m| m.keys()) {
            if l != "dart:core" && !importadas.contains(l) {
                do_sdk.entry(nome.clone()).or_default().insert(l.clone());
            }
        }
    }
    for bibliotecas in do_sdk.values() {
        for l in bibliotecas {
            let (span, novo) = inserir_import(texto, unit, l);
            saida.push(AcaoDeCodigo {
                titulo: format!("Import library '{l}'"),
                especie: "quickfix.import.librarySdk".into(),
                edicoes: vec![Edicao {
                    uri: uri.to_string(),
                    span,
                    texto: novo,
                }],
                diagnostico: None,
            });
        }
    }

    // Projeto.
    let raiz = raiz_do_projeto(&arquivo);
    let pacote = nome_do_pacote(&raiz);
    let mut do_projeto: BTreeSet<PathBuf> = BTreeSet::new();
    for (caminho, nomes) in indice_projeto.atualizar(&raiz, documentos) {
        if caminho != arquivo && nomes.iter().any(|d| faltando.contains(&d.nome)) {
            do_projeto.insert(caminho.to_path_buf());
        }
    }
    for caminho in do_projeto {
        let Some(uri_import) = uri_de_import(&arquivo, &caminho, &raiz, pacote.as_deref()) else {
            continue;
        };
        let absoluto = Url::from_file_path(&caminho)
            .map(|u| u.to_string())
            .unwrap_or_default();
        if importadas.contains(&uri_import) || importadas.contains(&absoluto) {
            continue;
        }
        let (span, novo) = inserir_import(texto, unit, &uri_import);
        saida.push(AcaoDeCodigo {
            titulo: format!("Import library '{uri_import}'"),
            especie: "quickfix.import.libraryProject1".into(),
            edicoes: vec![Edicao {
                uri: uri.to_string(),
                span,
                texto: novo,
            }],
            diagnostico: None,
        });
    }
    saida
}

/// Código publicado de um diagnóstico (`unused_local_variable`, …).
fn codigo(d: &Diagnostic) -> Option<&'static str> {
    d.code.map(|c| c.info().nome)
}

/// O trecho de `span` estendido à linha inteira quando ele é tudo o que há
/// nela (só espaço antes e depois): apagar um comando não deixa linha vazia.
fn linha_inteira(texto: &str, span: Span) -> Span {
    let inicio_linha = texto[..span.start].rfind('\n').map_or(0, |i| i + 1);
    let fim_linha = texto[span.end..]
        .find('\n')
        .map_or(texto.len(), |i| span.end + i);
    let antes_vazio = texto[inicio_linha..span.start].trim().is_empty();
    let depois_vazio = texto[span.end..fim_linha].trim().is_empty();
    if antes_vazio && depois_vazio {
        let fim = (fim_linha + 1).min(texto.len());
        Span {
            start: inicio_linha,
            end: fim,
        }
    } else {
        span
    }
}

/// Uma ação de correção com uma edição só.
fn correcao(
    uri: &str,
    titulo: String,
    especie: &str,
    edicoes: Vec<(Span, String)>,
    d: &Diagnostic,
) -> AcaoDeCodigo {
    AcaoDeCodigo {
        titulo,
        especie: especie.into(),
        edicoes: edicoes
            .into_iter()
            .map(|(span, texto)| Edicao {
                uri: uri.to_string(),
                span,
                texto,
            })
            .collect(),
        diagnostico: Some(d.clone()),
    }
}

/// Correções dos diagnósticos semânticos publicados que tocam `inicio..fim`,
/// sobre a biblioteca carregada (`projeto`), como as do servidor do Dart:
///
/// * `unused_local_variable` → `Remove unused local variable`: a declaração
///   (ou só a variável, numa lista) e os comandos que só atribuem a ela;
/// * `unused_element` (função local) → `Remove unused element`;
/// * `unnecessary_cast` → `Remove unnecessary cast` (e o parêntese que
///   sobraria em volta de uma expressão primária);
/// * `unnecessary_non_null_assertion` → `Remove the '!'`;
/// * `invalid_null_aware_operator` → `Replace with '.'` (ou `'['`);
/// * `instance_access_to_static_member` → `Change access to static using
///   'C'`, com o nome da classe como a biblioteca o vê (com prefixo, se é
///   por prefixo);
/// * `record_literal_one_positional_no_trailing_comma` → `Add trailing comma`.
///
/// Cada edição é conferida contra a árvore do texto vigente: diagnóstico
/// que não corresponde a um nó esperado (texto já mudou, forma diferente)
/// não gera ação.
pub(crate) fn corrigir_publicados(
    projeto: &Projeto,
    uri: &str,
    diagnosticos: &[Diagnostic],
    inicio: usize,
    fim: usize,
) -> Vec<AcaoDeCodigo> {
    let Some(unidade) = projeto.unidade_do_uri(uri) else {
        return Vec::new();
    };
    let consulta = &projeto.consulta;
    let u = consulta.programa.unit(unidade);
    let ast = &u.ast;
    let texto = u.source.as_str();
    let corpos = &consulta.corpos.units[unidade.0 as usize];
    let mut saida = Vec::new();
    for d in diagnosticos {
        if !(d.span.start <= fim && inicio <= d.span.end) || d.span.end > texto.len() {
            continue;
        }
        match codigo(d) {
            Some("unused_local_variable") => {
                let Some(edicoes) = remover_local(ast, corpos, texto, d.span) else {
                    continue;
                };
                saida.push(correcao(
                    uri,
                    "Remove unused local variable".into(),
                    "quickfix.remove.unusedLocalVariable",
                    edicoes,
                    d,
                ));
            }
            Some("unused_element") => {
                let funcao_local = ast.stmts.iter().find(|s| {
                    matches!(&s.kind, ast::StmtKind::Function(f) if ast.function(*f).name.is_some_and(|n| n.span == d.span))
                });
                if let Some(s) = funcao_local {
                    let span = linha_inteira(texto, s.span);
                    saida.push(correcao(
                        uri,
                        "Remove unused element".into(),
                        "quickfix.remove.unusedElement",
                        vec![(span, String::new())],
                        d,
                    ));
                }
            }
            Some("unnecessary_cast") => {
                let Some((id, valor)) =
                    ast.exprs
                        .iter()
                        .enumerate()
                        .find_map(|(i, e)| match &e.kind {
                            ExprKind::As { value, .. } if e.span == d.span => {
                                Some((ast::ExprId(i as u32), *value))
                            }
                            _ => None,
                        })
                else {
                    continue;
                };
                let v = ast.expr(valor).span;
                // `(x as T).m()` → `x.m()` quando `x` é primária.
                let primaria = matches!(
                    ast.expr(valor).kind,
                    ExprKind::Identifier(_)
                        | ExprKind::Property { .. }
                        | ExprKind::Call { .. }
                        | ExprKind::Index { .. }
                        | ExprKind::Parenthesized(_)
                        | ExprKind::This
                        | ExprKind::Int(_)
                        | ExprKind::Double(_)
                        | ExprKind::String(_)
                        | ExprKind::Bool(_)
                        | ExprKind::Null
                );
                let pai = ast
                    .exprs
                    .iter()
                    .find(|p| matches!(p.kind, ExprKind::Parenthesized(x) if x == id));
                let alvo = match pai {
                    Some(p) if primaria => p.span,
                    _ => d.span,
                };
                saida.push(correcao(
                    uri,
                    "Remove unnecessary cast".into(),
                    "quickfix.remove.unnecessaryCast",
                    vec![(alvo, texto[v.start..v.end].to_string())],
                    d,
                ));
            }
            Some("unnecessary_non_null_assertion") => {
                if &texto[d.span.start..d.span.end] == "!" {
                    saida.push(correcao(
                        uri,
                        "Remove the '!'".into(),
                        "quickfix.remove.nonNullAssertion",
                        vec![(d.span, String::new())],
                        d,
                    ));
                }
            }
            Some("invalid_null_aware_operator") => {
                let trecho = &texto[d.span.start..d.span.end];
                let novo = if trecho == "?." {
                    "."
                } else if trecho.starts_with('?') && trecho.ends_with('[') {
                    "["
                } else {
                    continue;
                };
                saida.push(correcao(
                    uri,
                    format!("Replace with '{novo}'"),
                    "quickfix.replace.withNotNullAware",
                    vec![(d.span, novo.to_string())],
                    d,
                ));
            }
            Some("instance_access_to_static_member") => {
                if let Some(acao) = acesso_estatico(projeto, unidade, d) {
                    saida.push(correcao(
                        uri,
                        acao.0,
                        "quickfix.change.toStaticAccess",
                        vec![acao.1],
                        d,
                    ));
                }
            }
            Some("record_literal_one_positional_no_trailing_comma") => {
                let parentese = ast
                    .exprs
                    .iter()
                    .find(|e| e.span == d.span && matches!(e.kind, ExprKind::Parenthesized(_)));
                if let Some(ExprKind::Parenthesized(interno)) = parentese.map(|e| &e.kind) {
                    let fim_interno = ast.expr(*interno).span.end;
                    saida.push(correcao(
                        uri,
                        "Add trailing comma".into(),
                        "quickfix.add.trailingComma",
                        vec![(
                            Span {
                                start: fim_interno,
                                end: fim_interno,
                            },
                            ",".into(),
                        )],
                        d,
                    ));
                }
            }
            _ => {}
        }
    }
    saida
}

/// As edições que removem o local declarado em `nome` (a declaração ou a
/// variável da lista) e os comandos que só atribuem a ele (`x = e;`,
/// `x += e;`). `None` quando o nome não é de uma declaração de comando.
fn remover_local(
    ast: &ast::Ast,
    corpos: &dartforge_types::UnitBodyTypes,
    texto: &str,
    nome: Span,
) -> Option<Vec<(Span, String)>> {
    let (comando, lista) = ast.stmts.iter().find_map(|s| match &s.kind {
        ast::StmtKind::Variables(vl) if vl.variables.iter().any(|v| v.name.span == nome) => {
            Some((s.span, vl))
        }
        _ => None,
    })?;
    let mut edicoes = Vec::new();
    if lista.variables.len() == 1 {
        edicoes.push((linha_inteira(texto, comando), String::new()));
    } else {
        let i = lista.variables.iter().position(|v| v.name.span == nome)?;
        let fim_de = |v: &ast::Variable| {
            v.initializer
                .map_or(v.name.span.end, |e| ast.expr(e).span.end)
        };
        let span = if i + 1 < lista.variables.len() {
            Span {
                start: nome.start,
                end: lista.variables[i + 1].name.span.start,
            }
        } else {
            Span {
                start: fim_de(&lista.variables[i - 1]),
                end: fim_de(&lista.variables[i]),
            }
        };
        edicoes.push((span, String::new()));
    }
    for s in &ast.stmts {
        let ast::StmtKind::Expression(e) = &s.kind else {
            continue;
        };
        let ExprKind::Assign { target, .. } = &ast.expr(*e).kind else {
            continue;
        };
        if matches!(ast.expr(*target).kind, ExprKind::Identifier(_))
            && corpos.declaracao_local(*target) == Some(nome.start)
        {
            edicoes.push((linha_inteira(texto, s.span), String::new()));
        }
    }
    Some(edicoes)
}

/// `a.estatico` → `C.estatico`: o título e a troca do alvo pelo nome da
/// classe como a biblioteca o enxerga (`C` ou `p.C`).
fn acesso_estatico(
    projeto: &Projeto,
    unidade: UnitId,
    d: &Diagnostic,
) -> Option<(String, (Span, String))> {
    let consulta = &projeto.consulta;
    let p = &consulta.programa;
    let u = p.unit(unidade);
    let corpos = &consulta.corpos.units[unidade.0 as usize];
    let (id, alvo) = u
        .ast
        .exprs
        .iter()
        .enumerate()
        .find_map(|(i, e)| match &e.kind {
            ExprKind::Property { target, name, .. } if name.span == d.span => {
                Some((ast::ExprId(i as u32), *target))
            }
            _ => None,
        })?;
    let membro = match corpos.get_resolved(id).or_else(|| {
        // Numa chamada, a resolução pode estar na chamada.
        u.ast
            .exprs
            .iter()
            .enumerate()
            .find_map(|(i, e)| match &e.kind {
                ExprKind::Call { target, .. } if *target == id => {
                    corpos.get_resolved(ast::ExprId(i as u32))
                }
                _ => None,
            })
    }) {
        Some(Resolved::Member {
            member: MemberRef::Function(f),
            ..
        }) => p.function(*f).class,
        Some(Resolved::Member {
            member: MemberRef::Variable(v),
            ..
        }) => p.variable(*v).class,
        _ => None,
    };
    // Sem resolução (o membro não existe na instância), pelo tipo do alvo.
    let classe = membro.or_else(|| match consulta.tabela.get(corpos.get_type(alvo)?) {
        dartforge_types::Type::Interface { class, .. } => Some(*class),
        _ => None,
    })?;
    let nome = p.class(classe).name;
    let lib = u.library;
    let texto_classe = if p.lookup(lib, nome).and_then(|b| b.getter) == Some(Element::Class(classe))
    {
        consulta.nome(nome).to_string()
    } else {
        let prefixo = p.library(lib).prefixes.iter().find_map(|(pr, espaco)| {
            (espaco.get(&nome).and_then(|b| b.getter) == Some(Element::Class(classe)))
                .then_some(*pr)
        })?;
        format!("{}.{}", consulta.nome(prefixo), consulta.nome(nome))
    };
    Some((
        format!("Change access to static using '{texto_classe}'"),
        (u.ast.expr(alvo).span, texto_classe),
    ))
}

/// Assistências no intervalo: `Add type annotation` para um local
/// `var x = e;` ou `final x = e;` com o cursor no nome ou na palavra-chave,
/// com o tipo inferido pela inferência comum — só quando todo nome do tipo
/// é visível na biblioteca (senão a anotação não compilaria) e o tipo não é
/// `dynamic`.
pub(crate) fn assistencias(
    projeto: &Projeto,
    uri: &str,
    inicio: usize,
    fim: usize,
) -> Vec<AcaoDeCodigo> {
    let Some(unidade) = projeto.unidade_do_uri(uri) else {
        return Vec::new();
    };
    let consulta = &projeto.consulta;
    let u = consulta.programa.unit(unidade);
    let ast = &u.ast;
    let corpos = &consulta.corpos.units[unidade.0 as usize];
    let mut saida = Vec::new();
    for s in &ast.stmts {
        let ast::StmtKind::Variables(vl) = &s.kind else {
            continue;
        };
        if vl.ty.is_some()
            || vl.variables.len() != 1
            || !(s.span.start <= fim && inicio <= s.span.end)
        {
            continue;
        }
        let v = &vl.variables[0];
        // O cursor na palavra-chave ou no nome, não no inicializador.
        if inicio > v.name.span.end {
            continue;
        }
        let Some(tipo) = corpos.tipo_local(v.name.span.start) else {
            continue;
        };
        let texto_tipo = consulta.formatar(tipo);
        if texto_tipo == "dynamic"
            || !tipo_escrevivel(projeto, unidade, v.name.span.start, &texto_tipo)
        {
            continue;
        }
        let palavra = &u.source[s.span.start..v.name.span.start];
        let edicao = if vl.var_ && palavra.trim_start().starts_with("var") {
            let de = s.span.start + palavra.find("var").unwrap_or(0);
            (
                Span {
                    start: de,
                    end: de + 3,
                },
                texto_tipo,
            )
        } else if vl.final_ {
            (
                Span {
                    start: v.name.span.start,
                    end: v.name.span.start,
                },
                format!("{texto_tipo} "),
            )
        } else {
            continue;
        };
        saida.push(AcaoDeCodigo {
            titulo: "Add type annotation".into(),
            especie: "refactor.add.typeAnnotation".into(),
            edicoes: vec![Edicao {
                uri: uri.to_string(),
                span: edicao.0,
                texto: edicao.1,
            }],
            diagnostico: None,
        });
    }
    saida
}

/// Todo identificador do texto do tipo nomeia, no ponto `offset` da unidade,
/// uma classe ou typedef visível na biblioteca, um parâmetro de tipo em
/// escopo ou um tipo embutido.
fn tipo_escrevivel(projeto: &Projeto, unidade: UnitId, offset: usize, texto_tipo: &str) -> bool {
    let consulta = &projeto.consulta;
    let u = consulta.programa.unit(unidade);
    texto_tipo
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$'))
        .filter(|p| !p.is_empty() && !p.as_bytes()[0].is_ascii_digit())
        .all(|nome| {
            if ["dynamic", "void", "Never", "Function", "Null", "required"].contains(&nome) {
                return true;
            }
            let Some(simbolo) = consulta.nomes.lookup(nome) else {
                return false;
            };
            crate::projeto::declaracao_de_parametro_de_tipo(&u.ast, offset, simbolo).is_some()
                || matches!(
                    consulta
                        .programa
                        .lookup(u.library, simbolo)
                        .and_then(|b| b.getter),
                    Some(Element::Class(_) | Element::Typedef(_))
                )
        })
}
