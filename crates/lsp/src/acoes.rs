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
use crate::projeto::Projeto;
use crate::{DocumentStore, Edicao};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{Element, UnitId};
use dartforge_frontend::ast::{self, DeclKind, DirectiveKind, ExprKind};
use dartforge_types::{MemberRef, Resolved};
use std::collections::BTreeSet;
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
    /// Arquivo que a ação cria (URI e conteúdo): uma operação `create` no
    /// `WorkspaceEdit`, oferecida só ao cliente que a aceita.
    pub criar_arquivo: Option<(String, String)>,
}

/// A espécie do Dart pelo id (`dart.fix.…`, `dart.assist.…`): a
/// prioridade e a mensagem (docs/LSP-ESPECIFICACAO.md §13.7.2, gerada em
/// `especies_g.rs`).
pub(crate) fn especie_do_dart(id: &str) -> Option<(u16, &'static str)> {
    let t = crate::especies_g::ESPECIES;
    t.binary_search_by(|(i, _, _)| (*i).cmp(id)).ok().map(|k| (t[k].1, t[k].2))
}

/// O intervalo das linhas de um diagnóstico: do começo da linha do início
/// ao fim da linha do fim (o filtro por linhas do `dart.dart:160-171`).
pub(crate) fn linhas_do_diagnostico(texto: &str, d: &Diagnostic) -> (usize, usize) {
    let inicio = d.span.start.min(texto.len());
    let fim = d.span.end.min(texto.len());
    let a = texto[..inicio].rfind('\n').map_or(0, |i| i + 1);
    let b = texto[fim..].find('\n').map_or(texto.len(), |i| fim + i);
    (a, b)
}

/// As espécies cujos produtores (nas variantes que este servidor emite)
/// têm `applicability = singleLocation`: não entram no "Fix all in file"
/// mesmo com a espécie `.multi` (`canBeAppliedAcrossSingleFile` falso).
const SO_UM_LUGAR: &[&str] = &["quickfix.add.await", "quickfix.convert.bodyToBlock", "quickfix.remove.abstract", "quickfix.remove.initializer"];

/// O mesmo código de erro (`errorCode.name`).
fn mesmo_codigo(a: &Diagnostic, b: &Diagnostic) -> bool {
    match (a.code, b.code) {
        (Some(x), Some(y)) => x.info().nome == y.info().nome,
        _ => false,
    }
}

/// `FixInFileProcessor.compute` (docs/LSP-ESPECIFICACAO.md §13.7.1 item
/// 7): para cada correção isolada de um diagnóstico cuja espécie tem a
/// `*.multi`, com 2+ diagnósticos do mesmo código na unidade, reaplica o
/// produtor (`produzir(diagnóstico, linhas)`) a cada um dos outros na ordem
/// da lista, acumulando as edições; a edição que se sobrepõe a uma
/// acumulada descarta aquele diagnóstico (`ConflictingEditException`). Sai
/// a `*.multi` (mensagem sem formatar, prioridade da tabela) quando há
/// edições e o produtor rodou 2+ vezes sem conflito.
pub(crate) fn corrigir_em_todo_o_arquivo(
    isoladas: &[AcaoDeCodigo],
    diagnosticos: &[Diagnostic],
    texto: &str,
    mut produzir: impl FnMut(&Diagnostic, usize, usize) -> Vec<AcaoDeCodigo>,
) -> Vec<AcaoDeCodigo> {
    let mut saida = Vec::new();
    let mut feitos: Vec<(Span, String)> = Vec::new();
    for a in isoladas {
        let Some(d) = &a.diagnostico else { continue };
        let Some(resto) = a.especie.strip_prefix("quickfix.") else { continue };
        if SO_UM_LUGAR.contains(&a.especie.as_str()) {
            continue;
        }
        let multi = format!("dart.fix.{resto}.multi");
        let Some((_, mensagem)) = especie_do_dart(&multi) else { continue };
        if feitos.iter().any(|(s, e)| *s == d.span && *e == a.especie) {
            continue;
        }
        feitos.push((d.span, a.especie.clone()));
        let irmaos: Vec<&Diagnostic> = diagnosticos.iter().filter(|x| mesmo_codigo(x, d)).collect();
        if irmaos.len() < 2 {
            continue;
        }
        let mut edicoes: Vec<Edicao> = a.edicoes.clone();
        let mut vezes = 1usize;
        for o in irmaos {
            if o.span == d.span && o.message == d.message {
                continue;
            }
            let (la, lb) = linhas_do_diagnostico(texto, o);
            let produzidas = produzir(o, la, lb);
            let Some(p) = produzidas.into_iter().find(|p| p.especie == a.especie && p.diagnostico.as_ref().is_some_and(|x| x.span == o.span)) else {
                vezes += 1;
                continue;
            };
            let conflita = p.edicoes.iter().any(|e| {
                edicoes.iter().any(|x| x.uri == e.uri && e.span.start < x.span.end && x.span.start < e.span.end)
            });
            if conflita {
                continue;
            }
            vezes += 1;
            edicoes.extend(p.edicoes);
        }
        if vezes > 1 && !edicoes.is_empty() {
            saida.push(AcaoDeCodigo {
                titulo: mensagem.to_string(),
                especie: format!("quickfix.{resto}.multi"),
                edicoes,
                diagnostico: Some(d.clone()),
                criar_arquivo: None,
            });
        }
    }
    saida
}

/// Correções dos diagnósticos sintáticos que tocam `inicio..fim`.
pub(crate) fn corrigir_sintaxe(
    uri: &str,
    texto: &str,
    diagnosticos: &[Diagnostic],
    inicio: usize,
    fim: usize,
) -> Vec<AcaoDeCodigo> {
    let mut saida = Vec::new();
    for d in diagnosticos {
        let toca = d.span.start <= fim && inicio <= d.span.end;
        // `InsertSemicolon`: a mensagem cita `';'` e o `node` não é o
        // identificador `await`.
        let ponto_e_virgula = d.code.is_some_and(|c| c.info().nome == "expected_token")
            && d.message.contains("';'")
            && texto.get(d.span.start..d.span.end) != Some("await");
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
                criar_arquivo: None,
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

/// Um nome sem resolução no intervalo, com o código que o analyzer daria
/// (o que escolhe as variantes do `ImportLibrary`) e o offset do nó.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Indefinido {
    /// `UNDEFINED_CLASS` (anotação de tipo): `forType`.
    Tipo(String, usize),
    /// `UNDEFINED_FUNCTION` (`f()` sem alvo): `forExtension`,
    /// `forExtensionType`, `forFunction`, `forType`.
    Funcao(String, usize),
    /// `UNDEFINED_IDENTIFIER`: `forExtension`, `forFunction`,
    /// `forTopLevelVariable`, `forType`.
    Identificador(String, usize),
}

/// Os nomes sem resolução em `inicio..fim` (sem os privados).
fn indefinidos_com_contexto(consulta: &Consulta, unidade: UnitId, inicio: usize, fim: usize) -> Vec<Indefinido> {
    let u = consulta.programa.unit(unidade);
    let corpos = &consulta.corpos.units[unidade.0 as usize];
    let lib = u.library;
    let toca = |s: Span| s.start <= fim && inicio <= s.end;
    let alvos_de_chamada: BTreeSet<u32> = u
        .ast
        .exprs
        .iter()
        .filter_map(|e| match &e.kind {
            ExprKind::Call { target, .. } => Some(target.0),
            _ => None,
        })
        .collect();
    let mut saida = Vec::new();
    for (i, e) in u.ast.exprs.iter().enumerate() {
        if let ExprKind::Identifier(n) = &e.kind
            && toca(n.span)
            && corpos.get_resolved(ast::ExprId(i as u32)).is_none()
            && consulta.programa.lookup(lib, n.sym).is_none()
        {
            let nome = consulta.nome(n.sym).to_string();
            if nome.starts_with('_') {
                continue;
            }
            if alvos_de_chamada.contains(&(i as u32)) {
                saida.push(Indefinido::Funcao(nome, n.span.start));
            } else {
                saida.push(Indefinido::Identificador(nome, n.span.start));
            }
        }
    }
    for t in &u.ast.types {
        if let ast::TypeKind::Named { name, .. } = &t.kind
            && let [n] = &name[..]
            && toca(n.span)
            && consulta.programa.lookup(lib, n.sym).is_none()
        {
            let texto = consulta.nome(n.sym);
            if !["dynamic", "Never", "void", "Function", "Record"].contains(&texto) && !tipo_param(&u.ast, n) && !texto.starts_with('_') {
                saida.push(Indefinido::Tipo(texto.to_string(), n.span.start));
            }
        }
    }
    saida
}

/// A espécie (`ElementKind`) de um elemento do programa.
fn especie_do_elemento(p: &dartforge_elements::model::Program, el: Element) -> Option<crate::conhecidas::Especie> {
    use crate::conhecidas::Especie as E;
    use dartforge_elements::model::ClassKind;
    Some(match el {
        Element::Class(c) => match p.class(c).kind {
            ClassKind::Enum => E::Enum,
            ClassKind::Mixin => E::Mixin,
            ClassKind::ExtensionType => E::TipoDeExtensao,
            _ => E::Classe,
        },
        Element::Typedef(_) => E::AliasDeTipo,
        Element::Function(f) => match p.function(f).kind {
            dartforge_elements::model::FunctionKind::Getter
            | dartforge_elements::model::FunctionKind::Setter
            | dartforge_elements::model::FunctionKind::ImplicitAccessor => E::Variavel,
            _ => E::Funcao,
        },
        Element::Variable(_) => E::Variavel,
        Element::Extension(_) => E::Extensao,
        Element::Prefix(..) => return None,
    })
}

/// `ImportLibrary` (`import_library.dart`) para os nomes sem resolução
/// das linhas pedidas: as variantes de cada código, o
/// `_importLibraryForElement` (prefixo e `show` dos imports existentes;
/// depois as bibliotecas conhecidas, com `SDK`/`PROJECT1`/`2`/`3` e as
/// variantes absoluta e relativa) e as edições do `DartFileEditBuilder`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn importar(
    projeto: &Projeto,
    sdk: Option<&dartforge_elements::sdk::SdkLayout>,
    conhecidas: &mut crate::conhecidas::IndiceDeBibliotecas,
    documentos: &DocumentStore,
    uri: &str,
    inicio: usize,
    fim: usize,
) -> Vec<AcaoDeCodigo> {
    use crate::conhecidas::{DE_TIPO, Especie as E};
    let Some(unidade) = projeto.unidade_do_uri(uri) else { return Vec::new() };
    let consulta = &projeto.consulta;
    let p = &consulta.programa;
    let lib = p.unit(unidade).library;
    let faltando = indefinidos_com_contexto(consulta, unidade, inicio, fim);
    if faltando.is_empty() {
        return Vec::new();
    }
    let Some(arquivo) = crate::projeto::arquivo_da_uri(uri) else { return Vec::new() };
    let cx = crate::refatoracoes::Contexto::novo(projeto, unidade);
    let resolvedor = crate::conhecidas::Resolvedor { sdk, pacotes: dartforge_elements::config::PackageConfig::discover(&arquivo).and_then(|c| dartforge_elements::config::PackageConfig::load(&c).ok()) };
    let candidatas = crate::conhecidas::candidatas(&arquivo, sdk, conhecidas, documentos);
    let uri_da_biblioteca = p.library(lib).uri.clone();
    let regra = |r: &str| crate::refatoracoes_exec::regra_ligada(projeto, unidade, r);
    let (pacote_sempre, relativo_sempre) = (regra("always_use_package_imports"), regra("prefer_relative_imports"));
    let definidora = p.library(lib).units.first().copied();
    let mut saida = Vec::new();
    for ind in &faltando {
        let (nome, offset, variantes): (&str, usize, Vec<Vec<E>>) = match ind {
            Indefinido::Tipo(n, o) => (n, *o, vec![DE_TIPO.to_vec()]),
            Indefinido::Funcao(n, o) => (n, *o, vec![vec![E::Extensao], vec![E::TipoDeExtensao], vec![E::Funcao, E::Variavel], DE_TIPO.to_vec()]),
            Indefinido::Identificador(n, o) => (n, *o, vec![vec![E::Extensao], vec![E::Funcao, E::Variavel], vec![E::Variavel], DE_TIPO.to_vec()]),
        };
        let simbolo = consulta.nomes.lookup(nome);
        for especies in variantes {
            // Os imports existentes (`libraryImports` da unidade definidora).
            let mut por_show: BTreeSet<String> = BTreeSet::new();
            for imp in p.library(lib).imports.iter().filter(|i| Some(i.unit) == definidora) {
                let Some(s) = simbolo else { break };
                let ligacao = p.library(imp.library).exported.get(&s).and_then(|b| b.getter.or(b.setter));
                let Some(el) = ligacao else { continue };
                if !especie_do_elemento(p, el).is_some_and(|e| especies.contains(&e)) {
                    continue;
                }
                let uri_importada = p.library(imp.library).uri.clone();
                if let Some(prefixo) = imp.prefix {
                    let pr = consulta.nome(prefixo).to_string();
                    saida.push(AcaoDeCodigo {
                        titulo: format!("Use imported library '{uri_importada}' with prefix '{pr}'"),
                        especie: "quickfix.import.libraryPrefix".into(),
                        edicoes: vec![Edicao { uri: uri.to_string(), span: Span { start: offset, end: offset }, texto: format!("{pr}.") }],
                        diagnostico: None,
                        criar_arquivo: None,
                    });
                    continue;
                }
                if let [ast::Combinator::Show(ns)] = &imp.combinators[..] {
                    let sdk_lib = p.library(imp.library).is_sdk;
                    let nome_da_biblioteca = if sdk_lib {
                        p.library(imp.library).units.first().and_then(|&x| p.unit(x).path.as_deref()).and_then(|c| c.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
                    } else {
                        p.library(imp.library).units.first().and_then(|&x| projeto.uri_da_unidade(x)).unwrap_or(uri_importada.clone())
                    };
                    por_show.insert(uri_importada.clone());
                    let fonte_imp = &p.unit(imp.unit).source;
                    let mut nomes: BTreeSet<String> = ns.iter().map(|n| fonte_imp[n.span.start..n.span.end].to_string()).collect();
                    nomes.insert(nome.to_string());
                    let (Some(a), Some(b)) = (ns.first(), ns.last()) else { continue };
                    let comeco = fonte_imp[..a.span.start].rfind("show").unwrap_or(a.span.start);
                    if let Some(uri_def) = projeto.uri_da_unidade(imp.unit) {
                        saida.push(AcaoDeCodigo {
                            titulo: format!("Update library '{nome_da_biblioteca}' import"),
                            especie: "quickfix.import.libraryShow".into(),
                            edicoes: vec![Edicao { uri: uri_def, span: Span { start: comeco, end: b.span.end }, texto: format!("show {}", nomes.into_iter().collect::<Vec<_>>().join(", ")) }],
                            diagnostico: None,
                            criar_arquivo: None,
                        });
                    }
                }
            }
            // As bibliotecas conhecidas.
            for c in &candidatas {
                if c.caminho == arquivo {
                    continue;
                }
                let Some((declarante, especie)) = conhecidas.exportado(&c.caminho, nome, &resolvedor, documentos) else { continue };
                if !especies.contains(&especie) || por_show.contains(&c.uri) || c.uri.ends_with(".template.dart") {
                    continue;
                }
                let tipo = if c.sdk {
                    "quickfix.import.librarySdk"
                } else if crate::conhecidas::caminho_lib_src(&c.caminho) {
                    "quickfix.import.libraryProject3"
                } else if declarante != c.caminho {
                    "quickfix.import.libraryProject2"
                } else {
                    "quickfix.import.libraryProject1"
                };
                // `canBeRelativeImport`: as duas `package:` do mesmo pacote.
                let pacote = |s: &str| s.strip_prefix("package:").and_then(|r| r.split('/').next()).map(str::to_string);
                let pode_relativo = pacote(&c.uri).is_some() && pacote(&c.uri) == pacote(&uri_da_biblioteca);
                let modos: Vec<bool> = if !pode_relativo || pacote_sempre {
                    vec![false]
                } else if relativo_sempre {
                    vec![true]
                } else {
                    vec![false, true]
                };
                for relativo in modos {
                    if let Some((texto_uri, edicoes)) = crate::refatoracoes_mover::importar_uma(&cx, &c.uri, relativo) {
                        saida.push(AcaoDeCodigo {
                            titulo: format!("Import library '{texto_uri}'"),
                            especie: tipo.into(),
                            edicoes,
                            diagnostico: None,
                            criar_arquivo: None,
                        });
                    }
                }
            }
        }
    }
    saida
}

/// Código publicado de um diagnóstico (`unused_local_variable`, …).
fn codigo(d: &Diagnostic) -> Option<&'static str> {
    d.code.map(|c| c.info().nome)
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
        criar_arquivo: None,
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
    // A árvore do analyzer (o `node`/`coveringNode` do contexto do fix), só
    // quando algum produtor precisa dela.
    let arvore = std::cell::OnceCell::new();
    let cx = || arvore.get_or_init(|| crate::refatoracoes::Contexto::novo(projeto, unidade));
    let mut saida = Vec::new();
    for d in diagnosticos {
        if !(d.span.start <= fim && inicio <= d.span.end) || d.span.end > texto.len() {
            continue;
        }
        match codigo(d) {
            Some("unused_local_variable") => {
                // `RemoveUnusedLocalVariable` (singleLocation).
                let Some(edicoes) = cx().remover_variavel_local(d.span) else { continue };
                saida.push(correcao(uri, "Remove unused local variable".into(), "quickfix.remove.unusedLocalVariable", edicoes, d));
            }
            Some("unused_element") if d.code.is_some_and(|c| c.info().unico.ends_with("UNUSED_ELEMENT_PARAMETER")) => {
                // `RemoveUnusedParameter`.
                let Some(s) = cx().remover_parametro(d.span) else { continue };
                saida.push(correcao(uri, "Remove the unused parameter".into(), "quickfix.remove.unusedParameter", vec![(s, String::new())], d));
            }
            Some("unused_element") => {
                // `RemoveUnusedElement`: sem referências na unidade.
                let Some(faixas) = cx().remover_elemento(d.span) else { continue };
                let edicoes = faixas.into_iter().map(|s| (s, String::new())).collect();
                saida.push(correcao(uri, "Remove unused element".into(), "quickfix.remove.unusedElement", edicoes, d));
            }
            Some("unnecessary_cast") => {
                // `RemoveUnnecessaryCast`: o `coveringNode` é a `AsExpression`;
                // apaga ` as T` e os parênteses em volta enquanto a
                // precedência do pai do parêntese não passa da do `as`.
                let cx = cx();
                let Some(n) = cx.arvore.localizar2(d.span.start, d.span.end.saturating_sub(1)) else { continue };
                if cx.especie(n) != "AsExpression" {
                    continue;
                }
                let Some(&expressao) = cx.filhos(n).first() else { continue };
                let mut edicoes = vec![(Span { start: cx.arvore.nos[expressao].fim, end: cx.arvore.nos[n].fim }, String::new())];
                let mut atual = n;
                while let Some(p) = cx.pai(atual)
                    && cx.especie(p) == "ParenthesizedExpression"
                {
                    if cx.precedencia_do_pai(p) > cx.precedencia(n) {
                        break;
                    }
                    let s = cx.arvore.span(p);
                    edicoes.push((Span { start: s.start, end: s.start + 1 }, String::new()));
                    edicoes.push((Span { start: s.end - 1, end: s.end }, String::new()));
                    atual = p;
                }
                saida.push(correcao(uri, "Remove unnecessary cast".into(), "quickfix.remove.unnecessaryCast", edicoes, d));
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
                // `ReplaceWithNotNullAware` sobre o `coveringNode`.
                let cx = cx();
                let Some(n) = cx.arvore.localizar2(d.span.start, d.span.end.saturating_sub(1)) else { continue };
                if let Some((s, novo, titulo)) = cx.trocar_operador_null_aware(n) {
                    saida.push(correcao(uri, format!("Replace with '{titulo}'"), "quickfix.replace.withNotNullAware", vec![(s, novo)], d));
                }
            }
            Some("instance_access_to_static_member") => {
                // `ChangeToStaticAccess`: o `node` é o nome de uma
                // `MethodInvocation` ou o identificador de um
                // `PrefixedIdentifier`.
                let cx = cx();
                if let Some((nome, s, texto_novo, importar)) = cx.acesso_estatico(d.span) {
                    let mut m = crate::refatoracoes_exec::Mudanca::default();
                    m.adicionar(uri, s, texto_novo);
                    crate::refatoracoes_mover::imports_do_builder(cx, &mut m, &importar);
                    if m.conflito.is_none() {
                        saida.push(AcaoDeCodigo {
                            titulo: format!("Change access to static using '{nome}'"),
                            especie: "quickfix.change.toStaticAccess".into(),
                            edicoes: m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
                            diagnostico: Some(d.clone()),
                            criar_arquivo: None,
                        });
                    }
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
            Some("assignment_to_final") => {
                // `MakeFieldNotFinal` e `AddLate`, nesta ordem.
                if let Some(acao) = campo_nao_final(projeto, unidade, d) {
                    saida.push(acao);
                }
                if let Some(acao) = campo_late(projeto, unidade, d) {
                    saida.push(acao);
                }
            }
            Some("abstract_field_initializer") => {
                saida.extend(campo_abstrato_inicializado(projeto, unidade, uri, ast, texto, d));
            }
            Some("non_bool_condition") => {
                // `AddNeNull` (tipo estático sem `?` → nada) e
                // `AddAwait.nonBool` (`Future<bool>` → `await `), nesta ordem.
                let cx = cx();
                let Some(n) = cx.arvore.localizar(d.span.start, d.span.end) else { continue };
                if !cx.e_expressao(n) {
                    continue;
                }
                let tipo = cx.tipo_do_no(n).map(|t| consulta.tabela.get(t).clone());
                if tipo.as_ref().is_none_or(|t| t.is_declared_nullable()) {
                    saida.push(correcao(
                        uri,
                        "Add != null".into(),
                        "quickfix.add.neNull",
                        vec![(Span { start: d.span.end, end: d.span.end }, " != null".into())],
                        d,
                    ));
                }
                let futuro_de_bool = match &tipo {
                    Some(dartforge_types::Type::Interface { class, args, .. }) => {
                        Some(*class) == consulta.core.future_class
                            && args.len() == 1
                            && matches!(consulta.tabela.get(args[0]), dartforge_types::Type::Interface { class: b, .. } if Some(*b) == consulta.core.bool_class)
                    }
                    _ => false,
                };
                if futuro_de_bool {
                    saida.push(correcao(
                        uri,
                        "Add 'await' keyword".into(),
                        "quickfix.add.await",
                        vec![(Span { start: d.span.start, end: d.span.start }, "await ".into())],
                        d,
                    ));
                }
            }
            Some("uri_does_not_exist") => {
                if let Some(acao) = criar_arquivo(projeto, unidade, uri, d) {
                    saida.push(acao);
                }
            }
            _ => {}
        }
    }
    saida
}

/// Os tokens de `texto[inicio..fim]`, com spans no texto inteiro.
fn tokens(texto: &str, inicio: usize, fim: usize) -> Vec<dartforge_frontend::token::Token> {
    dartforge_frontend::lexer::lex(&texto[inicio..fim])
        .map(|ts| {
            ts.into_iter()
                .map(|mut t| {
                    t.span.start += inicio;
                    t.span.end += inicio;
                    t
                })
                .collect()
        })
        .unwrap_or_default()
}

/// O trecho de um token e do espaço em branco que o segue (apagar `final `).
fn com_espaco_depois(texto: &str, span: Span) -> Span {
    let resto = &texto[span.end..];
    let n = resto.len() - resto.trim_start_matches([' ', '\t']).len();
    Span { start: span.start, end: span.end + n }
}

/// O campo escrito por `assignment_to_final`: o `writeOrReadElement` do
/// identificador é o getter sintético (o acessor implícito) de um campo não
/// sintético sem setter; o campo, a unidade e o nome dele, a lista e o
/// membro que o declara.
fn campo_escrito<'p>(projeto: &'p Projeto, unidade: UnitId, d: &Diagnostic) -> Option<(dartforge_elements::model::VariableId, UnitId, Span, &'p ast::VariableList, &'p ast::Member)> {
    let consulta = &projeto.consulta;
    let p = &consulta.programa;
    let u = p.unit(unidade);
    let corpos = &consulta.corpos.units[unidade.0 as usize];
    let id = u.ast.exprs.iter().enumerate().find_map(|(i, e)| {
        let casa = match &e.kind {
            ExprKind::Identifier(n) => n.span == d.span,
            ExprKind::Property { name, .. } => name.span == d.span,
            _ => false,
        };
        casa.then_some(ast::ExprId(i as u32))
    })?;
    let f = match corpos.get_resolved(id)? {
        Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::Element(Element::Function(f)) => *f,
        Resolved::Member { member: MemberRef::Variable(v), .. } | Resolved::Element(Element::Variable(v)) => p.variable(*v).getter?,
        _ => return None,
    };
    let fe = p.function(f);
    if fe.kind != dartforge_elements::model::FunctionKind::ImplicitAccessor || consulta.nome(fe.name).ends_with('=') {
        return None;
    }
    let v = fe.variable?;
    if p.variable(v).setter.is_some() {
        return None;
    }
    let (u_decl, nome) = projeto.nome_da_variavel(v)?;
    let ud = p.unit(u_decl);
    let membro = ud.ast.members.iter().find(|m| match &m.kind {
        ast::MemberKind::Field(l) => l.variables.iter().any(|x| x.name.span == nome),
        _ => false,
    })?;
    let ast::MemberKind::Field(l) = &membro.kind else { return None };
    Some((v, u_decl, nome, l, membro))
}

/// `MakeFieldNotFinal` (`make_field_not_final.dart`): campo de **classe**,
/// lista de uma variável com `final`; com tipo, apaga `final `; sem tipo,
/// troca por `var `. Edita o arquivo da declaração (o Dart registra o edit
/// no arquivo do uso, defeito que só aparece entre arquivos e não é
/// reproduzido).
fn campo_nao_final(projeto: &Projeto, unidade: UnitId, d: &Diagnostic) -> Option<AcaoDeCodigo> {
    let p = &projeto.consulta.programa;
    let (v, u_decl, nome, l, membro) = campo_escrito(projeto, unidade, d)?;
    let classe = p.variable(v).class?;
    if !matches!(p.class(classe).kind, dartforge_elements::model::ClassKind::Class | dartforge_elements::model::ClassKind::MixinApplication) {
        return None;
    }
    if l.variables.len() != 1 || !l.final_ || l.const_ {
        return None;
    }
    let texto = p.unit(u_decl).source.as_str();
    let final_ = tokens(texto, membro.span.start, nome.start)
        .into_iter()
        .find(|t| t.kind == dartforge_frontend::token::Kind::Keyword(dartforge_frontend::token::Keyword::Final))?;
    let (span, novo) = match l.ty {
        // `range.startStart(final, type)`.
        Some(t) => (Span { start: final_.span.start, end: p.unit(u_decl).ast.ty(t).span.start }, String::new()),
        // `range.startStart(final, variável)` por `var `.
        None => (Span { start: final_.span.start, end: nome.start }, "var ".to_string()),
    };
    let uri_decl = projeto.uri_da_unidade(u_decl)?;
    Some(AcaoDeCodigo {
        titulo: format!("Make field '{}' not final", &texto[nome.start..nome.end]),
        especie: "quickfix.makeFieldNotFinal".into(),
        edicoes: vec![Edicao { uri: uri_decl, span, texto: novo }],
        diagnostico: Some(d.clone()),
        criar_arquivo: None,
    })
}

/// `AddLate` (`add_late.dart:60-91`): o campo de classe, mixin, enum ou
/// tipo de extensão, não `late`, numa `FieldDeclaration` de uma variável
/// com `final`: `late ` antes do `final`, no arquivo do campo.
fn campo_late(projeto: &Projeto, unidade: UnitId, d: &Diagnostic) -> Option<AcaoDeCodigo> {
    let p = &projeto.consulta.programa;
    let (v, u_decl, nome, l, membro) = campo_escrito(projeto, unidade, d)?;
    p.variable(v).class?;
    if p.variable(v).late || l.late || l.variables.len() != 1 || !l.final_ {
        return None;
    }
    let texto = p.unit(u_decl).source.as_str();
    let final_ = tokens(texto, membro.span.start, nome.start)
        .into_iter()
        .find(|t| t.kind == dartforge_frontend::token::Kind::Keyword(dartforge_frontend::token::Keyword::Final))?;
    let uri_decl = projeto.uri_da_unidade(u_decl)?;
    Some(AcaoDeCodigo {
        titulo: "Add 'late' modifier".into(),
        especie: "quickfix.add.late".into(),
        edicoes: vec![Edicao { uri: uri_decl, span: Span { start: final_.span.start, end: final_.span.start }, texto: "late ".into() }],
        diagnostico: Some(d.clone()),
        criar_arquivo: None,
    })
}

/// `abstract_field_initializer` → `Remove the 'abstract' keyword` e
/// `Remove initializer`, nesta ordem (a do `RemoveAbstract` e do
/// `RemoveInitializer` na lista de produtores do Dart). O `RemoveAbstract`
/// pula a lista de várias variáveis cujo tipo não é anulável.
fn campo_abstrato_inicializado(projeto: &Projeto, unidade: UnitId, uri: &str, ast: &ast::Ast, texto: &str, d: &Diagnostic) -> Vec<AcaoDeCodigo> {
    let mut saida = Vec::new();
    let Some((membro, lista, var)) = ast.members.iter().find_map(|m| match &m.kind {
        ast::MemberKind::Field(l) if l.abstract_ => {
            l.variables.iter().find(|x| x.name.span == d.span).map(|x| (m, l, x))
        }
        _ => None,
    }) else {
        return saida;
    };
    let anulavel = || {
        lista.ty.is_some()
            && crate::destaques::variavel_declarada_em(projeto, unidade, var.name.span.start)
                .and_then(|v| projeto.consulta.tipo_da_variavel(v))
                .is_some_and(|t| projeto.consulta.tabela.get(t).is_declared_nullable())
    };
    if lista.variables.len() <= 1 || anulavel() {
        let abstrato = tokens(texto, membro.span.start, var.name.span.start)
            .into_iter()
            .find(|t| t.kind == dartforge_frontend::token::Kind::Ident && &texto[t.span.start..t.span.end] == "abstract");
        if let Some(t) = abstrato {
            // `[abstract.offset, próximoToken.offset)`.
            let proximo = tokens(texto, t.span.end, var.name.span.end).into_iter().find(|x| x.kind != dartforge_frontend::token::Kind::Eof).map_or(var.name.span.start, |x| x.span.start);
            saida.push(correcao(uri, "Remove the 'abstract' keyword".into(), "quickfix.remove.abstract", vec![(Span { start: t.span.start, end: proximo }, String::new())], d));
        }
    }
    if let Some(init) = var.initializer {
        let fim = ast.expr(init).span.end;
        saida.push(correcao(
            uri,
            "Remove initializer".into(),
            "quickfix.remove.initializer",
            vec![(Span { start: var.name.span.end, end: fim }, String::new())],
            d,
        ));
    }
    saida
}

/// `uri_does_not_exist` → `Create file 'x.dart'` (`CreateFile`,
/// `create_file.dart`): o `node` é o `SimpleStringLiteral` da URI; num
/// `import`/`export` com fonte referida absoluta `.dart`, o arquivo com
/// `// TODO Implement this library.`; numa `part`, `part of '<biblioteca
/// relativa à pasta da parte>';` e duas quebras.
fn criar_arquivo(projeto: &Projeto, unidade: UnitId, _uri: &str, d: &Diagnostic) -> Option<AcaoDeCodigo> {
    let p = projeto.programa();
    let u = p.unit(unidade);
    let (literal, parte) = u.unit.directives.iter().find_map(|dir| match &dir.kind {
        DirectiveKind::Import { uri, .. } | DirectiveKind::Export { uri, .. } if uri.span == d.span => Some((uri, false)),
        DirectiveKind::Part { uri } if uri.span == d.span => Some((uri, true)),
        _ => None,
    })?;
    // Só o literal simples (sem interpolação nem adjacência).
    let simples = tokens(&u.source, literal.span.start, literal.span.end)
        .iter()
        .filter(|t| t.kind != dartforge_frontend::token::Kind::Eof)
        .count()
        == 1;
    if !simples {
        return None;
    }
    let texto = literal.constant_value()?.as_str()?.to_string();
    let atual = u.path.clone()?;
    // `referencedSource`: `package:` pelo `package_config`, `file:`, ou
    // relativo à unidade.
    let destino: PathBuf = if texto.starts_with("package:") {
        let configuracao = dartforge_elements::config::PackageConfig::discover(&atual)
            .and_then(|c| dartforge_elements::config::PackageConfig::load(&c).ok())?;
        configuracao.resolve_package_uri(&texto).ok()?
    } else if texto.starts_with("file:") {
        Url::parse(&texto).ok()?.to_file_path().ok()?
    } else if texto.contains(':') {
        return None;
    } else {
        Url::from_file_path(&atual).ok()?.join(&texto).ok()?.to_file_path().ok()?
    };
    let nome = destino.file_name()?.to_string_lossy().into_owned();
    let conteudo = if parte {
        let biblioteca = p.library(u.library).units.first().and_then(|&x| p.unit(x).path.clone())?;
        let pasta = destino.parent()?;
        let relativo = crate::refatoracoes_metodo::caminho_relativo(&biblioteca, pasta);
        let eol = crate::refatoracoes_exec::Texto::novo(&u.source).eol();
        format!("part of '{relativo}';{eol}{eol}")
    } else {
        if !destino.is_absolute() || destino.extension().and_then(|e| e.to_str()) != Some("dart") {
            return None;
        }
        "// TODO Implement this library.".to_string()
    };
    Some(AcaoDeCodigo {
        titulo: format!("Create file '{nome}'"),
        especie: "quickfix.create.file".into(),
        edicoes: Vec::new(),
        diagnostico: Some(d.clone()),
        criar_arquivo: Some((Url::from_file_path(&destino).ok()?.to_string(), conteudo)),
    })
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
            criar_arquivo: None,
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

/// `Organize Imports` (`source.organizeImports`) sobre a unidade só
/// parseada: o `ImportOrganizer` de `crate::fonte_imports`
/// (docs/LSP-ESPECIFICACAO.md §13.12.6), que ordena, agrupa e tira as
/// duplicatas textuais. Sem erros semânticos, nenhum import sai por não ser
/// usado. Sem nada a mudar, a ação vem sem edição.
pub(crate) fn organizar_imports(uri: &str, texto: &str) -> AcaoDeCodigo {
    let mut nomes = dartforge_intern::Interner::new();
    let analisado = dartforge_frontend::parser::parse(texto, &mut nomes);
    let edicoes = crate::fonte_imports::organizar(texto, &analisado.unit, &[], false, true)
        .and_then(|novo| crate::fonte_imports::edicao(texto, &novo))
        .map(|(offset, comprimento, novo)| {
            vec![Edicao { uri: uri.to_string(), span: Span { start: offset, end: offset + comprimento }, texto: novo }]
        })
        .unwrap_or_default();
    AcaoDeCodigo {
        titulo: "Organize Imports".into(),
        especie: "source.organizeImports".into(),
        edicoes,
        diagnostico: None,
        criar_arquivo: None,
    }
}
