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
//! Dos códigos semânticos publicados (`crates/analise/verificados.txt`), os
//! três de enum (`enum_constant_same_name_as_enclosing`,
//! `enum_with_name_values`, `values_declaration_in_enum`) não têm correção
//! no servidor do Dart 3.6.2 (conferido com o `dart language-server`). Os
//! publicados depois (2026-09-26: `unused_local_variable`,
//! `unused_element`, …) têm correções no servidor oficial que este ainda
//! não oferece; nenhuma é inventada.

use crate::consulta::Consulta;
use crate::indice::{IndiceProjeto, IndiceSdk, nome_do_pacote, uri_de_import};
use crate::projeto::{Projeto, raiz_do_projeto};
use crate::{DocumentStore, Edicao};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_frontend::ast::{self, DeclKind, DirectiveKind, ExprKind};
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
pub(crate) fn inserir_import(texto: &str, unit: &ast::CompilationUnit, uri_novo: &str) -> (Span, String) {
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
