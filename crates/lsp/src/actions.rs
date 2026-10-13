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
//!   `crates/analysis/verificados.txt` que o servidor do Dart 3.6.2 corrige),
//!   sobre os diagnósticos imediatos e os tipados da versão vigente, com os
//!   títulos e espécies do Dart ([`fix_published`]).
//! * **Assistência** `Add type annotation` (`crate::assists`).
//!
//! Dos códigos publicados, os três de enum
//! (`enum_constant_same_name_as_enclosing`, `enum_with_name_values`,
//! `values_declaration_in_enum`) não têm correção no servidor do Dart 3.6.2
//! (conferido com o `dart language-server`), e este também não oferece;
//! nenhuma correção é inventada.

use crate::query::Query;
use crate::project::Project;
use crate::{DocumentStore, Edit};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{Element, UnitId};
use dartforge_frontend::ast::{self, DeclKind, DirectiveKind, ExprKind};
use dartforge_types::{MemberRef, Resolved};
use std::collections::BTreeSet;
use std::path::PathBuf;
use url::Url;

/// Uma ação de código oferecida ao editor.
#[derive(Debug, Clone)]
pub struct CodeAction {
    /// Título mostrado (`Insert ';'`, `Import library 'dart:math'`).
    pub title: String,
    /// `CodeActionKind` (`quickfix.insertSemicolon`, ...).
    pub kind: String,
    /// Edições do `WorkspaceEdit`.
    pub edits: Vec<Edit>,
    /// Diagnóstico corrigido, quando a ação responde a um publicado.
    pub diagnostic: Option<Diagnostic>,
    /// Arquivo que a ação cria (URI e conteúdo): uma operação `create` no
    /// `WorkspaceEdit`, oferecida só ao cliente que a aceita.
    pub create_file: Option<(String, String)>,
    /// A seleção e os grupos ligados da mudança (`SourceChange.selection` e
    /// `linkedEditGroups`), que viram `SnippetTextEdit` para o cliente com
    /// `experimental.snippetTextEdit` (docs/LSP-ESPECIFICACAO.md §13.8.1-G).
    pub linked: Linked,
}

/// A seleção e os grupos ligados de uma ação, em offsets (bytes) do
/// documento **depois** de aplicadas as edições, como no `SourceChange` do
/// Dart.
///
/// ```
/// use dartforge_lsp::{LinkedGroup, Linked};
/// let l = Linked { selection: Some((10, 0)), groups: vec![LinkedGroup { positions: vec![4], length: 1, suggestions: vec![] }] };
/// assert!(!l.empty());
/// assert!(Linked::default().empty());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Linked {
    /// Offset e comprimento da seleção depois da mudança.
    pub selection: Option<(usize, usize)>,
    /// Os grupos ligados, na ordem em que foram criados.
    pub groups: Vec<LinkedGroup>,
}

impl Linked {
    /// Sem seleção nem grupos: a edição fica simples mesmo com a capacidade.
    pub fn empty(&self) -> bool {
        self.selection.is_none() && self.groups.is_empty()
    }
}

impl CodeAction {
    /// O offset, no documento final, do byte `relative` do texto da edição
    /// `i`: o início dela mais o saldo das edições que vêm antes.
    fn in_final_document(&self, i: usize, relative: usize) -> Option<usize> {
        let e = self.edits.get(i)?;
        let before: isize = self
            .edits
            .iter()
            .filter(|o| o.uri == e.uri && o.span.start < e.span.start)
            .map(|o| o.text.len() as isize - (o.span.end - o.span.start) as isize)
            .sum();
        usize::try_from(e.span.start as isize + before + relative as isize).ok()
    }

    /// A seleção (`selectHere`/`selectAll`) no byte `relative` do texto da
    /// edição `i`, com `length` bytes.
    pub(crate) fn with_selection(mut self, i: usize, relative: usize, length: usize) -> Self {
        if let Some(s) = self.in_final_document(i, relative) {
            self.linked.selection = Some((s, length));
        }
        self
    }

    /// A seleção e os grupos de uma [`crate::snippets::Write`] que é o
    /// texto da edição `i`.
    pub(crate) fn with_write(self, i: usize, e: &crate::snippets::Write) -> Self {
        self.with_write_at(i, 0, e)
    }

    /// [`Self::with_write`] com a escrita começando no byte `base` do texto
    /// da edição `i`.
    pub(crate) fn with_write_at(mut self, i: usize, base: usize, e: &crate::snippets::Write) -> Self {
        if let Some((k, n)) = e.selection {
            self = self.with_selection(i, base + k, n);
        }
        for (_, positions, n, suggestions) in &e.groups {
            let positions: Vec<usize> = positions.iter().map(|p| base + p).collect();
            self = self.with_group(i, &positions, *n, suggestions.clone());
        }
        self
    }

    /// Um grupo ligado (`addLinkedEdit`) com as posições dadas em bytes do
    /// texto da edição `i`.
    pub(crate) fn with_group(mut self, i: usize, relative: &[usize], length: usize, suggestions: Vec<String>) -> Self {
        let positions: Vec<usize> = relative.iter().filter_map(|&r| self.in_final_document(i, r)).collect();
        if !positions.is_empty() {
            self.linked.groups.push(LinkedGroup { positions, length, suggestions });
        }
        self
    }
}

/// Um grupo ligado (`LinkedEditGroup`): as posições que mudam juntas.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkedGroup {
    /// Offsets (bytes) das posições no documento final.
    pub positions: Vec<usize>,
    /// Comprimento comum das posições (o da última acrescentada).
    pub length: usize,
    /// Os valores sugeridos, na ordem do produtor.
    pub suggestions: Vec<String>,
}

/// A espécie do Dart pelo id (`dart.fix.…`, `dart.assist.…`): a
/// prioridade e a mensagem (docs/LSP-ESPECIFICACAO.md §13.7.2, gerada em
/// `especies_g.rs`).
pub(crate) fn dart_kind(id: &str) -> Option<(u16, &'static str)> {
    let t = crate::especies_g::ESPECIES;
    t.binary_search_by(|(i, _, _)| (*i).cmp(id)).ok().map(|k| (t[k].1, t[k].2))
}

/// O intervalo das linhas de um diagnóstico: do começo da linha do início
/// ao fim da linha do fim (o filtro por linhas do `dart.dart:160-171`).
pub(crate) fn diagnostic_lines(text: &str, d: &Diagnostic) -> (usize, usize) {
    let start = d.span.start.min(text.len());
    let end = d.span.end.min(text.len());
    let a = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let b = text[end..].find('\n').map_or(text.len(), |i| end + i);
    (a, b)
}

/// As espécies cujos produtores (nas variantes que este servidor emite)
/// têm `applicability = singleLocation`: não entram no "Fix all in file"
/// mesmo com a espécie `.multi` (`canBeAppliedAcrossSingleFile` falso).
const ONLY_ONE_PLACE: &[&str] = &["quickfix.add.await", "quickfix.convert.bodyToBlock", "quickfix.remove.abstract", "quickfix.remove.initializer"];

/// O mesmo código de erro (`errorCode.name`).
fn same_code(a: &Diagnostic, b: &Diagnostic) -> bool {
    match (a.code, b.code) {
        (Some(x), Some(y)) => x.info().nome == y.info().nome,
        _ => false,
    }
}

/// `FixInFileProcessor.compute` (docs/LSP-ESPECIFICACAO.md §13.7.1 item
/// 7): para cada correção isolada de um diagnóstico cuja espécie tem a
/// `*.multi`, com 2+ diagnósticos do mesmo código na unidade, reaplica o
/// produtor (`produce(diagnóstico, lines)`) a cada um dos outros na ordem
/// da lista, acumulando as edições; a edição que se sobrepõe a uma
/// acumulada descarta aquele diagnóstico (`ConflictingEditException`). Sai
/// a `*.multi` (mensagem sem formatar, prioridade da tabela) quando há
/// edições e o produtor rodou 2+ vezes sem conflito.
pub(crate) fn fix_in_whole_file(
    isolated: &[CodeAction],
    diagnostics: &[Diagnostic],
    text: &str,
    mut produce: impl FnMut(&Diagnostic, usize, usize) -> Vec<CodeAction>,
) -> Vec<CodeAction> {
    let mut output = Vec::new();
    let mut done: Vec<(Span, String)> = Vec::new();
    for a in isolated {
        let Some(d) = &a.diagnostic else { continue };
        let Some(rest) = a.kind.strip_prefix("quickfix.") else { continue };
        if ONLY_ONE_PLACE.contains(&a.kind.as_str()) {
            continue;
        }
        let multi = format!("dart.fix.{rest}.multi");
        let Some((_, message)) = dart_kind(&multi) else { continue };
        if done.iter().any(|(s, e)| *s == d.span && *e == a.kind) {
            continue;
        }
        done.push((d.span, a.kind.clone()));
        let siblings: Vec<&Diagnostic> = diagnostics.iter().filter(|x| same_code(x, d)).collect();
        if siblings.len() < 2 {
            continue;
        }
        let mut edits: Vec<Edit> = a.edits.clone();
        let mut times = 1usize;
        for o in siblings {
            if o.span == d.span && o.message == d.message {
                continue;
            }
            let (la, lb) = diagnostic_lines(text, o);
            let produced = produce(o, la, lb);
            let Some(p) = produced.into_iter().find(|p| p.kind == a.kind && p.diagnostic.as_ref().is_some_and(|x| x.span == o.span)) else {
                times += 1;
                continue;
            };
            let conflicts = p.edits.iter().any(|e| {
                edits.iter().any(|x| x.uri == e.uri && e.span.start < x.span.end && x.span.start < e.span.end)
            });
            if conflicts {
                continue;
            }
            times += 1;
            edits.extend(p.edits);
        }
        if times > 1 && !edits.is_empty() {
            output.push(CodeAction {
                title: message.to_string(),
                kind: format!("quickfix.{rest}.multi"),
                edits,
                diagnostic: Some(d.clone()),
                create_file: None, linked: Default::default(),
            });
        }
    }
    output
}

/// Correções dos diagnósticos sintáticos que tocam `start..end`.
pub(crate) fn fix_syntax(
    uri: &str,
    text: &str,
    diagnostics: &[Diagnostic],
    start: usize,
    end: usize,
) -> Vec<CodeAction> {
    let mut output = Vec::new();
    for d in diagnostics {
        let touches = d.span.start <= end && start <= d.span.end;
        // `InsertSemicolon`: a mensagem cita `';'` e o `node` não é o
        // identificador `await`.
        let semicolon = d.code.is_some_and(|c| c.info().nome == "expected_token")
            && d.message.contains("';'")
            && text.get(d.span.start..d.span.end) != Some("await");
        if touches && semicolon {
            output.push(CodeAction {
                title: "Insert ';'".into(),
                kind: "quickfix.insertSemicolon".into(),
                edits: vec![Edit {
                    uri: uri.to_string(),
                    span: Span {
                        start: d.span.end,
                        end: d.span.end,
                    },
                    text: ";".into(),
                }],
                diagnostic: Some(d.clone()),
                create_file: None, linked: Default::default(),
            });
        }
    }
    output
}

/// Edição que acrescenta `import 'uri';` na ordem das diretivas existentes.
pub(crate) fn insert_import(
    text: &str,
    unit: &ast::CompilationUnit,
    new_uri: &str,
) -> (Span, String) {
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
    // `_addLibraryImports` (`change_builder_dart.dart`), para um import sem
    // prefixo nem combinadores: entre os existentes, pelo grupo (`dart:`,
    // `package:`, relativo) e pela ordem do texto da URI.
    if !imports.is_empty() {
        // `preferredQuoteForUris`: a aspa das diretivas existentes.
        let quote = imports
            .iter()
            .find_map(|(d, _)| text.get(d.span.start..d.span.end).and_then(|s| s.find(['\'', '"']).map(|i| &s[i..=i])))
            .unwrap_or("'");
        let line = format!("import {quote}{new_uri}{quote};");
        let is_dart = new_uri.starts_with("dart:");
        let is_package = new_uri.starts_with("package:");
        // `insert(prev:)`: depois do anterior (e de um comentário na mesma
        // linha); `insert(next:)`: antes do seguinte.
        let after = |d: &ast::Directive| -> (Span, String) {
            let mut end = d.span.end;
            let rest = &text[end..];
            let spaces = rest.len() - rest.trim_start_matches([' ', '\t']).len();
            let r = &rest[spaces..];
            if r.starts_with("//") {
                end += spaces + r.find('\n').unwrap_or(r.len());
            } else if r.starts_with("/*")
                && let Some(f) = r.find("*/")
                && !r[..f].contains('\n')
            {
                end += spaces + f + 2;
            }
            (Span { start: end, end: end }, format!("\n{line}"))
        };
        let before = |d: &ast::Directive, empty_line: bool| -> (Span, String) {
            let start = d.span.start;
            (Span { start: start, end: start }, format!("{line}\n{}", if empty_line { "\n" } else { "" }))
        };
        let mut last: Option<&ast::Directive> = None;
        let mut last_dart: Option<&ast::Directive> = None;
        let mut last_package: Option<&ast::Directive> = None;
        let mut last_is_dart = false;
        let mut last_is_package = false;
        for (d, u) in &imports {
            let existing_dart = u.starts_with("dart:");
            let existing_package = u.starts_with("package:");
            let existing_relative = !u.contains(':');
            let new_before = new_uri < u.as_str();
            if is_dart {
                if !existing_dart || new_before {
                    return match last_dart {
                        Some(p) => after(p),
                        None => before(d, !existing_dart),
                    };
                }
            } else if is_package {
                if existing_relative || new_before {
                    return match last_package {
                        Some(p) => after(p),
                        None => before(d, existing_relative),
                    };
                }
            } else if !existing_dart && !existing_package && new_before {
                return before(d, false);
            }
            last = Some(d);
            if existing_dart {
                last_dart = Some(d);
            } else if existing_package {
                last_package = Some(d);
            }
            last_is_dart = existing_dart;
            last_is_package = existing_package;
        }
        let d = last.expect("há imports");
        let empty_line = if is_package { last_is_dart } else { !is_dart && (last_is_dart || last_is_package) };
        return (
            Span { start: d.span.end, end: d.span.end },
            format!("{}\n{line}", if empty_line { "\n" } else { "" }),
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
            format!("\n\nimport '{new_uri}';"),
        );
    }
    let start = unit.script_tag.map_or(0, |s| {
        text[s.end..]
            .find('\n')
            .map_or(text.len(), |i| s.end + i + 1)
    });
    (
        Span {
            start: start,
            end: start,
        },
        format!("import '{new_uri}';\n\n"),
    )
}

/// `n` é um parâmetro de tipo de alguma declaração que o contém.
fn type_param(ast: &ast::Ast, n: &ast::Name) -> bool {
    let tem = |ps: &[ast::TypeParameter]| ps.iter().any(|t| t.name.sym == n.sym);
    let inside = |s: Span| s.start <= n.span.start && n.span.start < s.end;
    ast.decls.iter().any(|d| {
        inside(d.span)
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
        .any(|f| inside(f.span) && tem(&f.type_params))
}

/// Um nome sem resolução no intervalo, com o código que o analyzer daria
/// (o que escolhe as variantes do `ImportLibrary`) e o offset do nó.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Undefined {
    /// `UNDEFINED_CLASS` (anotação de tipo): `forType`.
    Type(String, usize),
    /// `UNDEFINED_FUNCTION` (`f()` sem alvo): `forExtension`,
    /// `forExtensionType`, `forFunction`, `forType`.
    Function(String, usize),
    /// `UNDEFINED_IDENTIFIER`: `forExtension`, `forFunction`,
    /// `forTopLevelVariable`, `forType`.
    Identifier(String, usize),
}

/// Os nomes sem resolução em `start..end` (sem os privados).
fn undefined_with_context(query: &Query, unit: UnitId, start: usize, end: usize) -> Vec<Undefined> {
    let u = query.program.unit(unit);
    let bodies = &query.bodies.units[unit.0 as usize];
    let lib = u.library;
    let touches = |s: Span| s.start <= end && start <= s.end;
    let call_targets: BTreeSet<u32> = u
        .ast
        .exprs
        .iter()
        .filter_map(|e| match &e.kind {
            ExprKind::Call { target, .. } => Some(target.0),
            _ => None,
        })
        .collect();
    let mut output = Vec::new();
    for (i, e) in u.ast.exprs.iter().enumerate() {
        if let ExprKind::Identifier(n) = &e.kind
            && touches(n.span)
            && bodies.get_resolved(ast::ExprId(i as u32)).is_none()
            && query.program.lookup(lib, n.sym).is_none()
        {
            let name_ = query.name(n.sym).to_string();
            if name_.starts_with('_') {
                continue;
            }
            if call_targets.contains(&(i as u32)) {
                output.push(Undefined::Function(name_, n.span.start));
            } else {
                output.push(Undefined::Identifier(name_, n.span.start));
            }
        }
    }
    for t in &u.ast.types {
        if let ast::TypeKind::Named { name, .. } = &t.kind
            && let [n] = &name[..]
            && touches(n.span)
            && query.program.lookup(lib, n.sym).is_none()
        {
            let text = query.name(n.sym);
            if !["dynamic", "Never", "void", "Function", "Record"].contains(&text) && !type_param(&u.ast, n) && !text.starts_with('_') {
                output.push(Undefined::Type(text.to_string(), n.span.start));
            }
        }
    }
    output
}

/// A espécie (`ElementKind`) de um elemento do programa.
fn element_kind(p: &dartforge_elements::model::Program, el: Element) -> Option<crate::known_libraries::Kind> {
    use crate::known_libraries::Kind as E;
    use dartforge_elements::model::ClassKind;
    Some(match el {
        Element::Class(c) => match p.class(c).kind {
            ClassKind::Enum => E::Enum,
            ClassKind::Mixin => E::Mixin,
            ClassKind::ExtensionType => E::ExtensionType,
            _ => E::Class,
        },
        Element::Typedef(_) => E::TypeAlias,
        Element::Function(f) => match p.function(f).kind {
            dartforge_elements::model::FunctionKind::Getter
            | dartforge_elements::model::FunctionKind::Setter
            | dartforge_elements::model::FunctionKind::ImplicitAccessor => E::Variable,
            _ => E::Function,
        },
        Element::Variable(_) => E::Variable,
        Element::Extension(_) => E::Extension,
        Element::Prefix(..) => return None,
    })
}

/// `ImportLibrary` (`import_library.dart`) para os nomes sem resolução
/// das linhas pedidas: as variantes de cada código, o
/// `_importLibraryForElement` (prefixo e `show` dos imports existentes;
/// depois as bibliotecas conhecidas, com `SDK`/`PROJECT1`/`2`/`3` e as
/// variantes absoluta e relativa) e as edições do `DartFileEditBuilder`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn import(
    project: &Project,
    sdk: Option<&dartforge_elements::sdk::SdkLayout>,
    known: &mut crate::known_libraries::LibraryIndex,
    documents: &DocumentStore,
    uri: &str,
    start: usize,
    end: usize,
) -> Vec<CodeAction> {
    use crate::known_libraries::{OF_TYPE, Kind as E};
    let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
    let query = &project.query;
    let p = &query.program;
    let lib = p.unit(unit).library;
    let missing = undefined_with_context(query, unit, start, end);
    if missing.is_empty() {
        return Vec::new();
    }
    let Some(file) = crate::project::uri_file(uri) else { return Vec::new() };
    let cx = crate::refactorings::Context::new(project, unit);
    let resolver = crate::known_libraries::Resolver { sdk, packages: dartforge_elements::config::PackageConfig::discover(&file).and_then(|c| dartforge_elements::config::PackageConfig::load(&c).ok()) };
    let candidates = crate::known_libraries::candidates(&file, sdk, known, documents);
    let library_uri = p.library(lib).uri.clone();
    let rule = |r: &str| crate::refactorings_exec::enabled_rule(project, unit, r);
    let (always_package, always_relative) = (rule("always_use_package_imports"), rule("prefer_relative_imports"));
    let defining = p.library(lib).units.first().copied();
    let mut output = Vec::new();
    for ind in &missing {
        let (name, offset, variants): (&str, usize, Vec<Vec<E>>) = match ind {
            Undefined::Type(n, o) => (n, *o, vec![OF_TYPE.to_vec()]),
            Undefined::Function(n, o) => (n, *o, vec![vec![E::Extension], vec![E::ExtensionType], vec![E::Function, E::Variable], OF_TYPE.to_vec()]),
            Undefined::Identifier(n, o) => (n, *o, vec![vec![E::Extension], vec![E::Function, E::Variable], vec![E::Variable], OF_TYPE.to_vec()]),
        };
        let symbol = query.names.lookup(name);
        for kinds in variants {
            // Os imports existentes (`libraryImports` da unidade definidora).
            let mut by_show: BTreeSet<String> = BTreeSet::new();
            for imp in p.library(lib).imports.iter().filter(|i| Some(i.unit) == defining) {
                let Some(s) = symbol else { break };
                let binding = p.library(imp.library).exported.get(&s).and_then(|b| b.getter.or(b.setter));
                let Some(el) = binding else { continue };
                if !element_kind(p, el).is_some_and(|e| kinds.contains(&e)) {
                    continue;
                }
                let imported_uri = p.library(imp.library).uri.clone();
                if let Some(prefix) = imp.prefix {
                    let pr = query.name(prefix).to_string();
                    output.push(CodeAction {
                        title: format!("Use imported library '{imported_uri}' with prefix '{pr}'"),
                        kind: "quickfix.import.libraryPrefix".into(),
                        edits: vec![Edit { uri: uri.to_string(), span: Span { start: offset, end: offset }, text: format!("{pr}.") }],
                        diagnostic: None,
                        create_file: None, linked: Default::default(),
                    });
                    continue;
                }
                if let [ast::Combinator::Show(ns)] = &imp.combinators[..] {
                    let sdk_lib = p.library(imp.library).is_sdk;
                    let library_name = if sdk_lib {
                        p.library(imp.library).units.first().and_then(|&x| p.unit(x).path.as_deref()).and_then(|c| c.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
                    } else {
                        p.library(imp.library).units.first().and_then(|&x| project.unit_uri(x)).unwrap_or(imported_uri.clone())
                    };
                    by_show.insert(imported_uri.clone());
                    let imp_source = &p.unit(imp.unit).source;
                    let mut names: BTreeSet<String> = ns.iter().map(|n| imp_source[n.span.start..n.span.end].to_string()).collect();
                    names.insert(name.to_string());
                    let (Some(a), Some(b)) = (ns.first(), ns.last()) else { continue };
                    let start_ = imp_source[..a.span.start].rfind("show").unwrap_or(a.span.start);
                    if let Some(uri_def) = project.unit_uri(imp.unit) {
                        output.push(CodeAction {
                            title: format!("Update library '{library_name}' import"),
                            kind: "quickfix.import.libraryShow".into(),
                            edits: vec![Edit { uri: uri_def, span: Span { start: start_, end: b.span.end }, text: format!("show {}", names.into_iter().collect::<Vec<_>>().join(", ")) }],
                            diagnostic: None,
                            create_file: None, linked: Default::default(),
                        });
                    }
                }
            }
            // As bibliotecas conhecidas.
            for c in &candidates {
                if c.path == file {
                    continue;
                }
                let Some((declarante, kind)) = known.exported_entry(&c.path, name, &resolver, documents) else { continue };
                if !kinds.contains(&kind) || by_show.contains(&c.uri) || c.uri.ends_with(".template.dart") {
                    continue;
                }
                let ty = if c.sdk {
                    "quickfix.import.librarySdk"
                } else if crate::known_libraries::path_lib_src(&c.path) {
                    "quickfix.import.libraryProject3"
                } else if declarante != c.path {
                    "quickfix.import.libraryProject2"
                } else {
                    "quickfix.import.libraryProject1"
                };
                // `canBeRelativeImport`: as duas `package:` do mesmo pacote.
                let package = |s: &str| s.strip_prefix("package:").and_then(|r| r.split('/').next()).map(str::to_string);
                let can_relative = package(&c.uri).is_some() && package(&c.uri) == package(&library_uri);
                let modes: Vec<bool> = if !can_relative || always_package {
                    vec![false]
                } else if always_relative {
                    vec![true]
                } else {
                    vec![false, true]
                };
                for relative in modes {
                    if let Some((text_uri, edits)) = crate::refactorings_move::import_one(&cx, &c.uri, relative) {
                        output.push(CodeAction {
                            title: format!("Import library '{text_uri}'"),
                            kind: ty.into(),
                            edits,
                            diagnostic: None,
                            create_file: None, linked: Default::default(),
                        });
                    }
                }
            }
        }
    }
    output
}

/// Código publicado de um diagnóstico (`unused_local_variable`, …).
fn code(d: &Diagnostic) -> Option<&'static str> {
    d.code.map(|c| c.info().nome)
}

/// Uma ação de correção com uma edição só.
fn fix(
    uri: &str,
    title: String,
    kind: &str,
    edits: Vec<(Span, String)>,
    d: &Diagnostic,
) -> CodeAction {
    CodeAction {
        title,
        kind: kind.into(),
        edits: edits
            .into_iter()
            .map(|(span, text)| Edit {
                uri: uri.to_string(),
                span,
                text,
            })
            .collect(),
        diagnostic: Some(d.clone()),
        create_file: None, linked: Default::default(),
    }
}

/// Correções dos diagnósticos semânticos publicados que tocam `start..end`,
/// sobre a biblioteca carregada (`project`), como as do servidor do Dart:
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
pub(crate) fn fix_published(
    project: &Project,
    uri: &str,
    diagnostics: &[Diagnostic],
    start: usize,
    end: usize,
) -> Vec<CodeAction> {
    let Some(unit) = project.uri_unit(uri) else {
        return Vec::new();
    };
    let query = &project.query;
    let u = query.program.unit(unit);
    let ast = &u.ast;
    let text = u.source.as_str();
    // A árvore do analyzer (o `node`/`coveringNode` do contexto do fix), só
    // quando algum produtor precisa dela.
    let tree = std::cell::OnceCell::new();
    let cx = || tree.get_or_init(|| crate::refactorings::Context::new(project, unit));
    let mut output = Vec::new();
    for d in diagnostics {
        if !(d.span.start <= end && start <= d.span.end) || d.span.end > text.len() {
            continue;
        }
        match code(d) {
            Some("unused_local_variable") => {
                // `RemoveUnusedLocalVariable` (singleLocation).
                let Some(edits) = cx().remove_local_variable(d.span) else { continue };
                output.push(fix(uri, "Remove unused local variable".into(), "quickfix.remove.unusedLocalVariable", edits, d));
            }
            Some("unused_element") if d.code.is_some_and(|c| c.info().unico.ends_with("UNUSED_ELEMENT_PARAMETER")) => {
                // `RemoveUnusedParameter`.
                let Some(s) = cx().remove_parameter(d.span) else { continue };
                output.push(fix(uri, "Remove the unused parameter".into(), "quickfix.remove.unusedParameter", vec![(s, String::new())], d));
            }
            Some("unused_element") => {
                // `RemoveUnusedElement`: sem referências na unidade.
                let Some(ranges) = cx().remove_element(d.span) else { continue };
                let edits = ranges.into_iter().map(|s| (s, String::new())).collect();
                output.push(fix(uri, "Remove unused element".into(), "quickfix.remove.unusedElement", edits, d));
            }
            Some("unnecessary_cast") => {
                // `RemoveUnnecessaryCast`: o `coveringNode` é a `AsExpression`;
                // apaga ` as T` e os parênteses em volta enquanto a
                // precedência do pai do parêntese não passa da do `as`.
                let cx = cx();
                let Some(n) = cx.tree.localizar2(d.span.start, d.span.end.saturating_sub(1)) else { continue };
                if cx.kind(n) != "AsExpression" {
                    continue;
                }
                let Some(&expression) = cx.children(n).first() else { continue };
                let mut edits = vec![(Span { start: cx.tree.nos[expression].fim, end: cx.tree.nos[n].fim }, String::new())];
                let mut current = n;
                while let Some(p) = cx.parent(current)
                    && cx.kind(p) == "ParenthesizedExpression"
                {
                    if cx.parent_precedence(p) > cx.precedence(n) {
                        break;
                    }
                    let s = cx.tree.span(p);
                    edits.push((Span { start: s.start, end: s.start + 1 }, String::new()));
                    edits.push((Span { start: s.end - 1, end: s.end }, String::new()));
                    current = p;
                }
                output.push(fix(uri, "Remove unnecessary cast".into(), "quickfix.remove.unnecessaryCast", edits, d));
            }
            Some("unnecessary_non_null_assertion") => {
                if &text[d.span.start..d.span.end] == "!" {
                    output.push(fix(
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
                let Some(n) = cx.tree.localizar2(d.span.start, d.span.end.saturating_sub(1)) else { continue };
                if let Some((s, new, title)) = cx.swap_null_aware_operator(n) {
                    output.push(fix(uri, format!("Replace with '{title}'"), "quickfix.replace.withNotNullAware", vec![(s, new)], d));
                }
            }
            Some("instance_access_to_static_member") => {
                // `ChangeToStaticAccess`: o `node` é o nome de uma
                // `MethodInvocation` ou o identificador de um
                // `PrefixedIdentifier`.
                let cx = cx();
                if let Some((name, s, new_text, import)) = cx.static_access(d.span) {
                    let mut m = crate::refactorings_exec::Change::default();
                    m.add(uri, s, new_text);
                    crate::refactorings_move::builder_imports(cx, &mut m, &import);
                    if m.conflict.is_none() {
                        output.push(CodeAction {
                            title: format!("Change access to static using '{name}'"),
                            kind: "quickfix.change.toStaticAccess".into(),
                            edits: m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
                            diagnostic: Some(d.clone()),
                            create_file: None, linked: Default::default(),
                        });
                    }
                }
            }
            Some("record_literal_one_positional_no_trailing_comma") => {
                let paren = ast
                    .exprs
                    .iter()
                    .find(|e| e.span == d.span && matches!(e.kind, ExprKind::Parenthesized(_)));
                if let Some(ExprKind::Parenthesized(internal)) = paren.map(|e| &e.kind) {
                    let internal_end = ast.expr(*internal).span.end;
                    output.push(fix(
                        uri,
                        "Add trailing comma".into(),
                        "quickfix.add.trailingComma",
                        vec![(
                            Span {
                                start: internal_end,
                                end: internal_end,
                            },
                            ",".into(),
                        )],
                        d,
                    ));
                }
            }
            Some("assignment_to_final") => {
                // `MakeFieldNotFinal` e `AddLate`, nesta ordem.
                if let Some(action) = non_final_field(project, unit, d) {
                    output.push(action);
                }
                if let Some(action) = late_field(project, unit, d) {
                    output.push(action);
                }
            }
            Some("abstract_field_initializer") => {
                output.extend(initialized_abstract_field(project, unit, uri, ast, text, d));
            }
            Some("non_bool_condition") => {
                // `AddNeNull` (tipo estático sem `?` → nada) e
                // `AddAwait.nonBool` (`Future<bool>` → `await `), nesta ordem.
                let cx = cx();
                let Some(n) = cx.tree.localizar(d.span.start, d.span.end) else { continue };
                if !cx.is_expression(n) {
                    continue;
                }
                let ty = cx.node_type(n).map(|t| query.table.get(t).clone());
                if ty.as_ref().is_none_or(|t| t.is_declared_nullable()) {
                    output.push(fix(
                        uri,
                        "Add != null".into(),
                        "quickfix.add.neNull",
                        vec![(Span { start: d.span.end, end: d.span.end }, " != null".into())],
                        d,
                    ));
                }
                let future_of_bool = match &ty {
                    Some(dartforge_types::Type::Interface { class, args, .. }) => {
                        Some(*class) == query.core.future_class
                            && args.len() == 1
                            && matches!(query.table.get(args[0]), dartforge_types::Type::Interface { class: b, .. } if Some(*b) == query.core.bool_class)
                    }
                    _ => false,
                };
                if future_of_bool {
                    output.push(fix(
                        uri,
                        "Add 'await' keyword".into(),
                        "quickfix.add.await",
                        vec![(Span { start: d.span.start, end: d.span.start }, "await ".into())],
                        d,
                    ));
                }
            }
            Some("uri_does_not_exist") => {
                if let Some(action) = create_file(project, unit, uri, d) {
                    output.push(action);
                }
            }
            _ => {}
        }
    }
    output
}

/// Os tokens de `text[start..end]`, com spans no texto inteiro.
fn tokens(text: &str, start: usize, end: usize) -> Vec<dartforge_frontend::token::Token> {
    dartforge_frontend::lexer::lex(&text[start..end])
        .map(|ts| {
            ts.into_iter()
                .map(|mut t| {
                    t.span.start += start;
                    t.span.end += start;
                    t
                })
                .collect()
        })
        .unwrap_or_default()
}

/// O campo escrito por `assignment_to_final`: o `writeOrReadElement` do
/// identificador é o getter sintético (o acessor implícito) de um campo não
/// sintético sem setter; o campo, a unidade e o nome dele, a lista e o
/// membro que o declara.
fn written_field<'p>(project: &'p Project, unit: UnitId, d: &Diagnostic) -> Option<(dartforge_elements::model::VariableId, UnitId, Span, &'p ast::VariableList, &'p ast::Member)> {
    let query = &project.query;
    let p = &query.program;
    let u = p.unit(unit);
    let bodies = &query.bodies.units[unit.0 as usize];
    let id = u.ast.exprs.iter().enumerate().find_map(|(i, e)| {
        let matches = match &e.kind {
            ExprKind::Identifier(n) => n.span == d.span,
            ExprKind::Property { name, .. } => name.span == d.span,
            _ => false,
        };
        matches.then_some(ast::ExprId(i as u32))
    })?;
    let f = match bodies.get_resolved(id)? {
        Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::Element(Element::Function(f)) => *f,
        Resolved::Member { member: MemberRef::Variable(v), .. } | Resolved::Element(Element::Variable(v)) => p.variable(*v).getter?,
        _ => return None,
    };
    let fe = p.function(f);
    if fe.kind != dartforge_elements::model::FunctionKind::ImplicitAccessor || query.name(fe.name).ends_with('=') {
        return None;
    }
    let v = fe.variable?;
    if p.variable(v).setter.is_some() {
        return None;
    }
    let (u_decl, name_) = project.variable_name(v)?;
    let ud = p.unit(u_decl);
    let member = ud.ast.members.iter().find(|m| match &m.kind {
        ast::MemberKind::Field(l) => l.variables.iter().any(|x| x.name.span == name_),
        _ => false,
    })?;
    let ast::MemberKind::Field(l) = &member.kind else { return None };
    Some((v, u_decl, name_, l, member))
}

/// `MakeFieldNotFinal` (`make_field_not_final.dart`): campo de **classe**,
/// lista de uma variável com `final`; com tipo, apaga `final `; sem tipo,
/// troca por `var `. Edita o arquivo da declaração (o Dart registra o edit
/// no arquivo do uso, defeito que só aparece entre arquivos e não é
/// reproduzido).
fn non_final_field(project: &Project, unit: UnitId, d: &Diagnostic) -> Option<CodeAction> {
    let p = &project.query.program;
    let (v, u_decl, name, l, member) = written_field(project, unit, d)?;
    let class = p.variable(v).class?;
    if !matches!(p.class(class).kind, dartforge_elements::model::ClassKind::Class | dartforge_elements::model::ClassKind::MixinApplication) {
        return None;
    }
    if l.variables.len() != 1 || !l.final_ || l.const_ {
        return None;
    }
    let text = p.unit(u_decl).source.as_str();
    let final_ = tokens(text, member.span.start, name.start)
        .into_iter()
        .find(|t| t.kind == dartforge_frontend::token::Kind::Keyword(dartforge_frontend::token::Keyword::Final))?;
    let (span, new) = match l.ty {
        // `range.startStart(final, type)`.
        Some(t) => (Span { start: final_.span.start, end: p.unit(u_decl).ast.ty(t).span.start }, String::new()),
        // `range.startStart(final, variável)` por `var `.
        None => (Span { start: final_.span.start, end: name.start }, "var ".to_string()),
    };
    let uri_decl = project.unit_uri(u_decl)?;
    Some(CodeAction {
        title: format!("Make field '{}' not final", &text[name.start..name.end]),
        kind: "quickfix.makeFieldNotFinal".into(),
        edits: vec![Edit { uri: uri_decl, span, text: new }],
        diagnostic: Some(d.clone()),
        create_file: None, linked: Default::default(),
    })
}

/// `AddLate` (`add_late.dart:60-91`): o campo de classe, mixin, enum ou
/// tipo de extensão, não `late`, numa `FieldDeclaration` de uma variável
/// com `final`: `late ` antes do `final`, no arquivo do campo.
fn late_field(project: &Project, unit: UnitId, d: &Diagnostic) -> Option<CodeAction> {
    let p = &project.query.program;
    let (v, u_decl, name, l, member) = written_field(project, unit, d)?;
    p.variable(v).class?;
    if p.variable(v).late || l.late || l.variables.len() != 1 || !l.final_ {
        return None;
    }
    let text = p.unit(u_decl).source.as_str();
    let final_ = tokens(text, member.span.start, name.start)
        .into_iter()
        .find(|t| t.kind == dartforge_frontend::token::Kind::Keyword(dartforge_frontend::token::Keyword::Final))?;
    let uri_decl = project.unit_uri(u_decl)?;
    Some(CodeAction {
        title: "Add 'late' modifier".into(),
        kind: "quickfix.add.late".into(),
        edits: vec![Edit { uri: uri_decl, span: Span { start: final_.span.start, end: final_.span.start }, text: "late ".into() }],
        diagnostic: Some(d.clone()),
        create_file: None, linked: Default::default(),
    })
}

/// `abstract_field_initializer` → `Remove the 'abstract' keyword` e
/// `Remove initializer`, nesta ordem (a do `RemoveAbstract` e do
/// `RemoveInitializer` na lista de produtores do Dart). O `RemoveAbstract`
/// pula a lista de várias variáveis cujo tipo não é anulável.
fn initialized_abstract_field(project: &Project, unit: UnitId, uri: &str, ast: &ast::Ast, text: &str, d: &Diagnostic) -> Vec<CodeAction> {
    let mut output = Vec::new();
    let Some((member, list, var)) = ast.members.iter().find_map(|m| match &m.kind {
        ast::MemberKind::Field(l) if l.abstract_ => {
            l.variables.iter().find(|x| x.name.span == d.span).map(|x| (m, l, x))
        }
        _ => None,
    }) else {
        return output;
    };
    let nullable = || {
        list.ty.is_some()
            && crate::document_highlight::variable_declared_in(project, unit, var.name.span.start)
                .and_then(|v| project.query.variable_type(v))
                .is_some_and(|t| project.query.table.get(t).is_declared_nullable())
    };
    if list.variables.len() <= 1 || nullable() {
        let abstract_ = tokens(text, member.span.start, var.name.span.start)
            .into_iter()
            .find(|t| t.kind == dartforge_frontend::token::Kind::Ident && &text[t.span.start..t.span.end] == "abstract");
        if let Some(t) = abstract_ {
            // `[abstract.offset, próximoToken.offset)`.
            let next = tokens(text, t.span.end, var.name.span.end).into_iter().find(|x| x.kind != dartforge_frontend::token::Kind::Eof).map_or(var.name.span.start, |x| x.span.start);
            output.push(fix(uri, "Remove the 'abstract' keyword".into(), "quickfix.remove.abstract", vec![(Span { start: t.span.start, end: next }, String::new())], d));
        }
    }
    if let Some(init) = var.initializer {
        let end = ast.expr(init).span.end;
        output.push(fix(
            uri,
            "Remove initializer".into(),
            "quickfix.remove.initializer",
            vec![(Span { start: var.name.span.end, end: end }, String::new())],
            d,
        ));
    }
    output
}

/// `uri_does_not_exist` → `Create file 'x.dart'` (`CreateFile`,
/// `create_file.dart`): o `node` é o `SimpleStringLiteral` da URI; num
/// `import`/`export` com fonte referida absoluta `.dart`, o arquivo com
/// `// TODO Implement this library.`; numa `part`, `part of '<biblioteca
/// relativa à pasta da parte>';` e duas quebras.
fn create_file(project: &Project, unit: UnitId, _uri: &str, d: &Diagnostic) -> Option<CodeAction> {
    let p = project.program();
    let u = p.unit(unit);
    let (literal, part) = u.unit.directives.iter().find_map(|dir| match &dir.kind {
        DirectiveKind::Import { uri, .. } | DirectiveKind::Export { uri, .. } if uri.span == d.span => Some((uri, false)),
        DirectiveKind::Part { uri } if uri.span == d.span => Some((uri, true)),
        _ => None,
    })?;
    // Só o literal simples (sem interpolação nem adjacência).
    let simple = tokens(&u.source, literal.span.start, literal.span.end)
        .iter()
        .filter(|t| t.kind != dartforge_frontend::token::Kind::Eof)
        .count()
        == 1;
    if !simple {
        return None;
    }
    let text = literal.constant_value()?.as_str()?.to_string();
    let current = u.path.clone()?;
    // `referencedSource`: `package:` pelo `package_config`, `file:`, ou
    // relativo à unidade.
    let target: PathBuf = if text.starts_with("package:") {
        let configuration = dartforge_elements::config::PackageConfig::discover(&current)
            .and_then(|c| dartforge_elements::config::PackageConfig::load(&c).ok())?;
        configuration.resolve_package_uri(&text).ok()?
    } else if text.starts_with("file:") {
        Url::parse(&text).ok()?.to_file_path().ok()?
    } else if text.contains(':') {
        return None;
    } else {
        Url::from_file_path(&current).ok()?.join(&text).ok()?.to_file_path().ok()?
    };
    let name = target.file_name()?.to_string_lossy().into_owned();
    let content = if part {
        let library = p.library(u.library).units.first().and_then(|&x| p.unit(x).path.clone())?;
        let folder = target.parent()?;
        let relative = crate::refactorings_method::relative_path(&library, folder);
        let eol = crate::refactorings_exec::Text::new(&u.source).eol();
        format!("part of '{relative}';{eol}{eol}")
    } else {
        if !target.is_absolute() || target.extension().and_then(|e| e.to_str()) != Some("dart") {
            return None;
        }
        "// TODO Implement this library.".to_string()
    };
    Some(CodeAction {
        title: format!("Create file '{name}'"),
        kind: "quickfix.create.file".into(),
        edits: Vec::new(),
        diagnostic: Some(d.clone()),
        create_file: Some((Url::from_file_path(&target).ok()?.to_string(), content)),
        linked: Default::default(),
    })
}

/// `Organize Imports` (`source.organizeImports`) sobre a unidade só
/// parseada: o `ImportOrganizer` de `crate::organize_imports`
/// (docs/LSP-ESPECIFICACAO.md §13.12.6), que ordena, agrupa e tira as
/// duplicatas textuais. Sem erros semânticos, nenhum import sai por não ser
/// usado. Sem nada a mudar, a ação vem sem edição.
pub(crate) fn organize_imports(uri: &str, text: &str) -> CodeAction {
    let mut names = dartforge_intern::Interner::new();
    let analyzed = dartforge_frontend::parser::parse(text, &mut names);
    let edits = crate::organize_imports::organize(text, &analyzed.unit, &[], false, true)
        .and_then(|new| crate::organize_imports::edit(text, &new))
        .map(|(offset, length, new)| {
            vec![Edit { uri: uri.to_string(), span: Span { start: offset, end: offset + length }, text: new }]
        })
        .unwrap_or_default();
    CodeAction {
        title: "Organize Imports".into(),
        kind: "source.organizeImports".into(),
        edits,
        diagnostic: None,
        create_file: None, linked: Default::default(),
    }
}
