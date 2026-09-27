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
use crate::projeto::{arquivos_do_projeto, eh_parte, raiz_do_projeto};
use crate::semantica::AnalisadorSemantico;
use crate::{DocumentStore, Edicao};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::sdk::SdkLayout;
use dartforge_frontend::ast::{self, DeclKind, DirectiveKind, ExprKind};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};
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

/// Índice dos nomes públicos de topo das bibliotecas do SDK: nome →
/// URIs `dart:` que o declaram. Montado uma vez por SDK; o tamanho é o do
/// SDK, não cresce com as edições.
pub(crate) type IndiceSdk = HashMap<String, BTreeSet<String>>;

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

/// Nomes públicos de topo declarados numa unidade analisada.
fn nomes_de_topo(
    ast: &ast::Ast,
    unit: &ast::CompilationUnit,
    nomes: &dartforge_intern::Interner,
) -> Vec<String> {
    let mut saida = Vec::new();
    for &d in &unit.declarations {
        let decl = ast.decl(d);
        let mut empurrar = |n: Option<ast::Name>| {
            if let Some(n) = n {
                let s = nomes.resolve(n.sym);
                if !s.starts_with('_') {
                    saida.push(s.to_string());
                }
            }
        };
        match &decl.kind {
            DeclKind::Class(c) => empurrar(Some(c.name)),
            DeclKind::Mixin(m) => empurrar(Some(m.name)),
            DeclKind::Enum(e) => empurrar(Some(e.name)),
            DeclKind::ExtensionType(e) => empurrar(Some(e.name)),
            DeclKind::Typedef(t) => empurrar(Some(t.name)),
            DeclKind::Extension(x) => empurrar(x.name),
            DeclKind::Function(f) => empurrar(ast.function(*f).name),
            DeclKind::Variables(vl) => {
                for v in vl.variables.iter() {
                    empurrar(Some(v.name));
                }
            }
        }
    }
    saida
}

/// Monta o índice de nomes do SDK: cada biblioteca pública (sem `_`) com as
/// suas partes.
pub(crate) fn indexar_sdk(sdk: &SdkLayout) -> IndiceSdk {
    let mut indice: IndiceSdk = HashMap::new();
    let mut bibliotecas: Vec<_> = sdk
        .libraries
        .values()
        .filter(|l| !l.name.starts_with('_') && l.supported)
        .collect();
    bibliotecas.sort_by(|a, b| a.name.cmp(&b.name));
    for lib in bibliotecas {
        let uri = format!("dart:{}", lib.name);
        let mut pendentes = vec![lib.path.clone()];
        let mut vistos = BTreeSet::new();
        while let Some(caminho) = pendentes.pop() {
            if !vistos.insert(caminho.clone()) {
                continue;
            }
            let Ok(texto) = std::fs::read_to_string(&caminho) else {
                continue;
            };
            let mut nomes = dartforge_intern::Interner::new();
            let analisado = dartforge_frontend::parser::parse(&texto, &mut nomes);
            for nome in nomes_de_topo(&analisado.ast, &analisado.unit, &nomes) {
                indice.entry(nome).or_default().insert(uri.clone());
            }
            for d in &analisado.unit.directives {
                if let DirectiveKind::Part { uri } = &d.kind
                    && let Some(relativo) = dartforge_elements::load::string_lit_value(uri)
                    && let Some(dir) = caminho.parent()
                {
                    pendentes.push(dir.join(relativo));
                }
            }
        }
    }
    indice
}

/// Caminho de `destino` relativo ao diretório `base`, com `/`.
fn relativo(destino: &Path, base: &Path) -> Option<String> {
    let d: Vec<Component> = destino.components().collect();
    let b: Vec<Component> = base.components().collect();
    let comum = d.iter().zip(&b).take_while(|(x, y)| x == y).count();
    if comum == 0 {
        return None;
    }
    let mut partes: Vec<String> = b[comum..].iter().map(|_| "..".to_string()).collect();
    partes.extend(
        d[comum..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    Some(partes.join("/"))
}

/// Nome do pacote no `pubspec.yaml` da raiz.
fn nome_do_pacote(raiz: &Path) -> Option<String> {
    let texto = std::fs::read_to_string(raiz.join("pubspec.yaml")).ok()?;
    texto.lines().find_map(|l| {
        l.strip_prefix("name:")
            .map(|n| n.trim().trim_matches(['\'', '"']).to_string())
    })
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
fn inserir_import(texto: &str, unit: &ast::CompilationUnit, uri_novo: &str) -> (Span, String) {
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

/// Ações de importar biblioteca para os nomes indefinidos em `inicio..fim`.
pub(crate) fn importar(
    semantico: &mut AnalisadorSemantico,
    documentos: &DocumentStore,
    uri: &str,
    inicio: usize,
    fim: usize,
) -> Vec<AcaoDeCodigo> {
    let Some(texto) = documentos.get(uri) else {
        return Vec::new();
    };
    let Some((programa, nomes, unidade)) = semantico.carregar(uri, texto, Some(documentos)) else {
        return Vec::new();
    };
    let lib = programa.unit(unidade).library;
    let consulta = Consulta::inferir(programa, nomes, &[lib], false, None);
    let faltando = indefinidos(&consulta, unidade, inicio, fim);
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
    let Some(arquivo) = Url::parse(uri).ok().and_then(|u| u.to_file_path().ok()) else {
        return Vec::new();
    };
    let arquivo = dartforge_elements::config::sem_verbatim(
        std::fs::canonicalize(&arquivo).unwrap_or(arquivo),
    );
    let mut saida = Vec::new();

    // SDK.
    let indice = semantico.indice_sdk();
    let mut do_sdk: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for nome in &faltando {
        for l in indice.get(nome).into_iter().flatten() {
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
    let lib_dir = raiz.join("lib");
    let mut do_projeto: BTreeSet<(String, PathBuf)> = BTreeSet::new();
    for caminho in arquivos_do_projeto(&raiz) {
        if caminho == arquivo {
            continue;
        }
        let aberto = Url::from_file_path(&caminho)
            .ok()
            .and_then(|u| documentos.get(u.as_str()).map(str::to_string));
        let Some(fonte) = aberto.or_else(|| std::fs::read_to_string(&caminho).ok()) else {
            continue;
        };
        if eh_parte(&fonte) {
            continue;
        }
        let mut nomes = dartforge_intern::Interner::new();
        let analisado = dartforge_frontend::parser::parse(&fonte, &mut nomes);
        for nome in nomes_de_topo(&analisado.ast, &analisado.unit, &nomes) {
            if faltando.contains(&nome) {
                do_projeto.insert((nome, caminho.clone()));
            }
        }
    }
    for (_, caminho) in do_projeto {
        let em_lib = caminho.starts_with(&lib_dir);
        let uri_import = match (&pacote, em_lib && !arquivo.starts_with(&lib_dir)) {
            (Some(p), true) => relativo(&caminho, &lib_dir).map(|r| format!("package:{p}/{r}")),
            _ => arquivo.parent().and_then(|dir| relativo(&caminho, dir)),
        };
        let Some(uri_import) = uri_import else {
            continue;
        };
        let absoluto = Url::from_file_path(&caminho)
            .map(|u| u.to_string())
            .unwrap_or_default();
        let ja = importadas.contains(&uri_import) || importadas.contains(&absoluto);
        if ja {
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
