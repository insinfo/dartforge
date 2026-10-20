//! `textDocument/codeAction`: quick fixes.
//!
//! * **Insert `;`** for the `expected_token` "Expected to find ';'." that the
//!   parser publishes, at the end of the diagnostic's range — the Dart
//!   server's `dart.fix.insertSemicolon` fix, with the same title.
//! * **Import library** for an undefined name in the requested range (an
//!   identifier or type that the common inference left unresolved and that
//!   the library scope does not know): the SDK libraries that declare it
//!   (index of the SDK's public names, built once) and the project's (files
//!   under the root with `pubspec.yaml`), as `quickfix.import.librarySdk` and
//!   `quickfix.import.libraryProject1`.
//!
//! * **Fixes for the published semantic codes** (those in
//!   `crates/analysis/verified.txt` that the Dart 3.6.2 server fixes),
//!   over the immediate diagnostics and the typed ones of the current
//!   version, with Dart's titles and kinds ([`fix_published`]).
//! * **Assist** `Add type annotation` (`crate::assists`).
//!
//! Of the published codes, the three enum ones
//! (`enum_constant_same_name_as_enclosing`, `enum_with_name_values`,
//! `values_declaration_in_enum`) have no fix in the Dart 3.6.2 server
//! (checked against `dart language-server`), and this one offers none
//! either; no fix is made up.

use crate::project::Project;
use crate::query::Query;
use crate::{DocumentStore, Edit};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{Element, UnitId};
use dartforge_frontend::ast::{self, DeclKind, DirectiveKind, ExprKind};
use dartforge_types::{MemberRef, Resolved};
use std::collections::BTreeSet;
use std::path::PathBuf;
use url::Url;

/// A code action offered to the editor.
#[derive(Debug, Clone)]
pub struct CodeAction {
    /// Displayed title (`Insert ';'`, `Import library 'dart:math'`).
    pub title: String,
    /// `CodeActionKind` (`quickfix.insertSemicolon`, ...).
    pub kind: String,
    /// Edits of the `WorkspaceEdit`.
    pub edits: Vec<Edit>,
    /// Fixed diagnostic, when the action answers a published one.
    pub diagnostic: Option<Diagnostic>,
    /// File the action creates (URI and content): a `create` operation in the
    /// `WorkspaceEdit`, offered only to a client that accepts it.
    pub create_file: Option<(String, String)>,
    /// The selection and the linked groups of the change
    /// (`SourceChange.selection` and `linkedEditGroups`), which become a
    /// `SnippetTextEdit` for a client with `experimental.snippetTextEdit`
    /// (docs/LSP-ESPECIFICACAO.md §13.8.1-G).
    pub linked: Linked,
}

/// The selection and the linked groups of an action, in offsets (bytes) of
/// the document **after** the edits are applied, as in Dart's
/// `SourceChange`.
///
/// ```
/// use dartforge_lsp::{LinkedGroup, Linked};
/// let l = Linked { selection: Some((10, 0)), groups: vec![LinkedGroup { positions: vec![4], length: 1, suggestions: vec![] }] };
/// assert!(!l.empty());
/// assert!(Linked::default().empty());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Linked {
    /// Offset and length of the selection after the change.
    pub selection: Option<(usize, usize)>,
    /// The linked groups, in the order they were created.
    pub groups: Vec<LinkedGroup>,
}

impl Linked {
    /// Without selection or groups: the edit stays plain even with the capability.
    pub fn empty(&self) -> bool {
        self.selection.is_none() && self.groups.is_empty()
    }
}

impl CodeAction {
    /// The offset, in the final document, of byte `relative` of the text of
    /// edit `i`: its start plus the balance of the edits that come before it.
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

    /// The selection (`selectHere`/`selectAll`) at byte `relative` of the text
    /// of edit `i`, with `length` bytes.
    pub(crate) fn with_selection(mut self, i: usize, relative: usize, length: usize) -> Self {
        if let Some(s) = self.in_final_document(i, relative) {
            self.linked.selection = Some((s, length));
        }
        self
    }

    /// The selection and the groups of a [`crate::snippets::Write`] that is the
    /// text of edit `i`.
    pub(crate) fn with_write(self, i: usize, e: &crate::snippets::Write) -> Self {
        self.with_write_at(i, 0, e)
    }

    /// [`Self::with_write`] with the write starting at byte `base` of the text
    /// of edit `i`.
    pub(crate) fn with_write_at(
        mut self,
        i: usize,
        base: usize,
        e: &crate::snippets::Write,
    ) -> Self {
        if let Some((k, n)) = e.selection {
            self = self.with_selection(i, base + k, n);
        }
        for (_, positions, n, suggestions) in &e.groups {
            let positions: Vec<usize> = positions.iter().map(|p| base + p).collect();
            self = self.with_group(i, &positions, *n, suggestions.clone());
        }
        self
    }

    /// A linked group (`addLinkedEdit`) with the positions given in bytes of the
    /// text of edit `i`.
    pub(crate) fn with_group(
        mut self,
        i: usize,
        relative: &[usize],
        length: usize,
        suggestions: Vec<String>,
    ) -> Self {
        let positions: Vec<usize> = relative
            .iter()
            .filter_map(|&r| self.in_final_document(i, r))
            .collect();
        if !positions.is_empty() {
            self.linked.groups.push(LinkedGroup {
                positions,
                length,
                suggestions,
            });
        }
        self
    }
}

/// A linked group (`LinkedEditGroup`): the positions that change together.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkedGroup {
    /// Offsets (bytes) of the positions in the final document.
    pub positions: Vec<usize>,
    /// Common length of the positions (that of the last one added).
    pub length: usize,
    /// The suggested values, in the producer's order.
    pub suggestions: Vec<String>,
}

/// The Dart kind by id (`dart.fix.…`, `dart.assist.…`): the priority and the
/// message (docs/LSP-ESPECIFICACAO.md §13.7.2, generated in `action_kinds_g.rs`).
pub(crate) fn dart_kind(id: &str) -> Option<(u16, &'static str)> {
    let t = crate::action_kinds_g::KINDS;
    t.binary_search_by(|(i, _, _)| (*i).cmp(id))
        .ok()
        .map(|k| (t[k].1, t[k].2))
}

/// The line range of a diagnostic: from the start of the start's line to the
/// end of the end's line (the line filter of `dart.dart:160-171`).
pub(crate) fn diagnostic_lines(text: &str, d: &Diagnostic) -> (usize, usize) {
    let start = d.span.start.min(text.len());
    let end = d.span.end.min(text.len());
    let a = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let b = text[end..].find('\n').map_or(text.len(), |i| end + i);
    (a, b)
}

/// The kinds whose producers (in the variants this server emits) have
/// `applicability = singleLocation`: they are not part of "Fix all in file"
/// even with the `.multi` kind (`canBeAppliedAcrossSingleFile` false).
const ONLY_ONE_PLACE: &[&str] = &[
    "quickfix.add.await",
    "quickfix.convert.bodyToBlock",
    "quickfix.remove.abstract",
    "quickfix.remove.initializer",
];

/// The same error code (`errorCode.name`).
fn same_code(a: &Diagnostic, b: &Diagnostic) -> bool {
    match (a.code, b.code) {
        (Some(x), Some(y)) => x.info().name == y.info().name,
        _ => false,
    }
}

/// `FixInFileProcessor.compute` (docs/LSP-ESPECIFICACAO.md §13.7.1 item 7):
/// for each isolated fix of a diagnostic whose kind has a `*.multi`, with 2+
/// diagnostics of the same code in the unit, reapplies the producer
/// (`produce(diagnostic, lines)`) to each of the others in list order,
/// accumulating the edits; an edit that overlaps an accumulated one discards
/// that diagnostic (`ConflictingEditException`). The `*.multi` comes out
/// (unformatted message, priority from the table) when there are edits and
/// the producer ran 2+ times without conflict.
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
        let Some(rest) = a.kind.strip_prefix("quickfix.") else {
            continue;
        };
        if ONLY_ONE_PLACE.contains(&a.kind.as_str()) {
            continue;
        }
        let multi = format!("dart.fix.{rest}.multi");
        let Some((_, message)) = dart_kind(&multi) else {
            continue;
        };
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
            let Some(p) = produced.into_iter().find(|p| {
                p.kind == a.kind && p.diagnostic.as_ref().is_some_and(|x| x.span == o.span)
            }) else {
                times += 1;
                continue;
            };
            let conflicts = p.edits.iter().any(|e| {
                edits.iter().any(|x| {
                    x.uri == e.uri && e.span.start < x.span.end && x.span.start < e.span.end
                })
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
                create_file: None,
                linked: Default::default(),
            });
        }
    }
    output
}

/// Fixes for the syntactic diagnostics touching `start..end`.
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
        // `InsertSemicolon`: the message cites `';'` and the `node` is not the
        // `await` identifier.
        let semicolon = d.code.is_some_and(|c| c.info().name == "expected_token")
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
                create_file: None,
                linked: Default::default(),
            });
        }
    }
    output
}

/// Edit that adds `import 'uri';` in the order of the existing directives.
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
    // `_addLibraryImports` (`change_builder_dart.dart`), for an import without
    // a prefix or combinators: among the existing ones, by group (`dart:`,
    // `package:`, relative) and by the order of the URI text.
    if !imports.is_empty() {
        // `preferredQuoteForUris`: the quote of the existing directives.
        let quote = imports
            .iter()
            .find_map(|(d, _)| {
                text.get(d.span.start..d.span.end)
                    .and_then(|s| s.find(['\'', '"']).map(|i| &s[i..=i]))
            })
            .unwrap_or("'");
        let line = format!("import {quote}{new_uri}{quote};");
        let is_dart = new_uri.starts_with("dart:");
        let is_package = new_uri.starts_with("package:");
        // `insert(prev:)`: after the previous one (and after a comment on the same
        // line); `insert(next:)`: before the next one.
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
            (Span { start: end, end }, format!("\n{line}"))
        };
        let before = |d: &ast::Directive, empty_line: bool| -> (Span, String) {
            let start = d.span.start;
            (
                Span { start, end: start },
                format!("{line}\n{}", if empty_line { "\n" } else { "" }),
            )
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
        let d = last.expect("there are imports");
        let empty_line = if is_package {
            last_is_dart
        } else {
            !is_dart && (last_is_dart || last_is_package)
        };
        return (
            Span {
                start: d.span.end,
                end: d.span.end,
            },
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
        Span { start, end: start },
        format!("import '{new_uri}';\n\n"),
    )
}

/// `n` is a type parameter of some declaration that contains it.
fn type_param(ast: &ast::Ast, n: &ast::Name) -> bool {
    let has = |ps: &[ast::TypeParameter]| ps.iter().any(|t| t.name.sym == n.sym);
    let inside = |s: Span| s.start <= n.span.start && n.span.start < s.end;
    ast.decls.iter().any(|d| {
        inside(d.span)
            && match &d.kind {
                DeclKind::Class(c) => has(&c.type_params),
                DeclKind::Mixin(m) => has(&m.type_params),
                DeclKind::Enum(e) => has(&e.type_params),
                DeclKind::Extension(x) => has(&x.type_params),
                DeclKind::ExtensionType(x) => has(&x.type_params),
                DeclKind::Typedef(t) => has(&t.type_params),
                _ => false,
            }
    }) || ast
        .functions
        .iter()
        .any(|f| inside(f.span) && has(&f.type_params))
}

/// An unresolved name in the range, with the code the analyzer would give
/// (which selects the `ImportLibrary` variants) and the node's offset.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Undefined {
    /// `UNDEFINED_CLASS` (type annotation): `forType`.
    Type(String, usize),
    /// `UNDEFINED_FUNCTION` (`f()` without a target): `forExtension`,
    /// `forExtensionType`, `forFunction`, `forType`.
    Function(String, usize),
    /// `UNDEFINED_IDENTIFIER`: `forExtension`, `forFunction`,
    /// `forTopLevelVariable`, `forType`.
    Identifier(String, usize),
}

/// The unresolved names in `start..end` (without the private ones).
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
            if !["dynamic", "Never", "void", "Function", "Record"].contains(&text)
                && !type_param(&u.ast, n)
                && !text.starts_with('_')
            {
                output.push(Undefined::Type(text.to_string(), n.span.start));
            }
        }
    }
    output
}

/// The kind (`ElementKind`) of a program element.
fn element_kind(
    p: &dartforge_elements::model::Program,
    el: Element,
) -> Option<crate::known_libraries::Kind> {
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

/// `ImportLibrary` (`import_library.dart`) for the unresolved names of the
/// requested lines: the variants of each code, `_importLibraryForElement`
/// (prefix and `show` of the existing imports; then the known libraries,
/// with `SDK`/`PROJECT1`/`2`/`3` and the absolute and relative variants) and
/// the edits of the `DartFileEditBuilder`.
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
    use crate::known_libraries::{Kind as E, OF_TYPE};
    let Some(unit) = project.uri_unit(uri) else {
        return Vec::new();
    };
    let query = &project.query;
    let p = &query.program;
    let lib = p.unit(unit).library;
    let missing = undefined_with_context(query, unit, start, end);
    if missing.is_empty() {
        return Vec::new();
    }
    let Some(file) = crate::project::uri_file(uri) else {
        return Vec::new();
    };
    let cx = crate::refactorings::Context::new(project, unit);
    let resolver = crate::known_libraries::Resolver {
        sdk,
        packages: dartforge_elements::config::PackageConfig::discover(&file)
            .and_then(|c| dartforge_elements::config::PackageConfig::load(&c).ok()),
    };
    let candidates = crate::known_libraries::candidates(&file, sdk, known, documents);
    let library_uri = p.library(lib).uri.clone();
    let rule = |r: &str| crate::refactorings_exec::enabled_rule(project, unit, r);
    let (always_package, always_relative) = (
        rule("always_use_package_imports"),
        rule("prefer_relative_imports"),
    );
    let defining = p.library(lib).units.first().copied();
    let mut output = Vec::new();
    for ind in &missing {
        let (name, offset, variants): (&str, usize, Vec<Vec<E>>) = match ind {
            Undefined::Type(n, o) => (n, *o, vec![OF_TYPE.to_vec()]),
            Undefined::Function(n, o) => (
                n,
                *o,
                vec![
                    vec![E::Extension],
                    vec![E::ExtensionType],
                    vec![E::Function, E::Variable],
                    OF_TYPE.to_vec(),
                ],
            ),
            Undefined::Identifier(n, o) => (
                n,
                *o,
                vec![
                    vec![E::Extension],
                    vec![E::Function, E::Variable],
                    vec![E::Variable],
                    OF_TYPE.to_vec(),
                ],
            ),
        };
        let symbol = query.names.lookup(name);
        for kinds in variants {
            // The existing imports (`libraryImports` of the defining unit).
            let mut by_show: BTreeSet<String> = BTreeSet::new();
            for imp in p
                .library(lib)
                .imports
                .iter()
                .filter(|i| Some(i.unit) == defining)
            {
                let Some(s) = symbol else { break };
                let binding = p
                    .library(imp.library)
                    .exported
                    .get(&s)
                    .and_then(|b| b.getter.or(b.setter));
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
                        edits: vec![Edit {
                            uri: uri.to_string(),
                            span: Span {
                                start: offset,
                                end: offset,
                            },
                            text: format!("{pr}."),
                        }],
                        diagnostic: None,
                        create_file: None,
                        linked: Default::default(),
                    });
                    continue;
                }
                if let [ast::Combinator::Show(ns)] = &imp.combinators[..] {
                    let sdk_lib = p.library(imp.library).is_sdk;
                    let library_name = if sdk_lib {
                        p.library(imp.library)
                            .units
                            .first()
                            .and_then(|&x| p.unit(x).path.as_deref())
                            .and_then(|c| c.file_name())
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    } else {
                        p.library(imp.library)
                            .units
                            .first()
                            .and_then(|&x| project.unit_uri(x))
                            .unwrap_or(imported_uri.clone())
                    };
                    by_show.insert(imported_uri.clone());
                    let imp_source = &p.unit(imp.unit).source;
                    let mut names: BTreeSet<String> = ns
                        .iter()
                        .map(|n| imp_source[n.span.start..n.span.end].to_string())
                        .collect();
                    names.insert(name.to_string());
                    let (Some(a), Some(b)) = (ns.first(), ns.last()) else {
                        continue;
                    };
                    let start_ = imp_source[..a.span.start]
                        .rfind("show")
                        .unwrap_or(a.span.start);
                    if let Some(uri_def) = project.unit_uri(imp.unit) {
                        output.push(CodeAction {
                            title: format!("Update library '{library_name}' import"),
                            kind: "quickfix.import.libraryShow".into(),
                            edits: vec![Edit {
                                uri: uri_def,
                                span: Span {
                                    start: start_,
                                    end: b.span.end,
                                },
                                text: format!(
                                    "show {}",
                                    names.into_iter().collect::<Vec<_>>().join(", ")
                                ),
                            }],
                            diagnostic: None,
                            create_file: None,
                            linked: Default::default(),
                        });
                    }
                }
            }
            // The known libraries.
            for c in &candidates {
                if c.path == file {
                    continue;
                }
                let Some((declarer, kind)) =
                    known.exported_entry(&c.path, name, &resolver, documents)
                else {
                    continue;
                };
                if !kinds.contains(&kind)
                    || by_show.contains(&c.uri)
                    || c.uri.ends_with(".template.dart")
                {
                    continue;
                }
                let ty = if c.sdk {
                    "quickfix.import.librarySdk"
                } else if crate::known_libraries::path_lib_src(&c.path) {
                    "quickfix.import.libraryProject3"
                } else if declarer != c.path {
                    "quickfix.import.libraryProject2"
                } else {
                    "quickfix.import.libraryProject1"
                };
                // `canBeRelativeImport`: both `package:` of the same package.
                let package = |s: &str| {
                    s.strip_prefix("package:")
                        .and_then(|r| r.split('/').next())
                        .map(str::to_string)
                };
                let can_relative =
                    package(&c.uri).is_some() && package(&c.uri) == package(&library_uri);
                let modes: Vec<bool> = if !can_relative || always_package {
                    vec![false]
                } else if always_relative {
                    vec![true]
                } else {
                    vec![false, true]
                };
                for relative in modes {
                    if let Some((text_uri, edits)) =
                        crate::refactorings_move::import_one(&cx, &c.uri, relative)
                    {
                        output.push(CodeAction {
                            title: format!("Import library '{text_uri}'"),
                            kind: ty.into(),
                            edits,
                            diagnostic: None,
                            create_file: None,
                            linked: Default::default(),
                        });
                    }
                }
            }
        }
    }
    output
}

/// Published code of a diagnostic (`unused_local_variable`, …).
fn code(d: &Diagnostic) -> Option<&'static str> {
    d.code.map(|c| c.info().name)
}

/// A fix action with a single edit.
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
        create_file: None,
        linked: Default::default(),
    }
}

/// Fixes for the published semantic diagnostics touching `start..end`, over
/// the loaded library (`project`), like the Dart server's:
///
/// * `unused_local_variable` → `Remove unused local variable`: the
///   declaration (or only the variable, in a list) and the statements that
///   only assign to it;
/// * `unused_element` (local function) → `Remove unused element`;
/// * `unnecessary_cast` → `Remove unnecessary cast` (and the parenthesis
///   that would remain around a primary expression);
/// * `unnecessary_non_null_assertion` → `Remove the '!'`;
/// * `invalid_null_aware_operator` → `Replace with '.'` (or `'['`);
/// * `instance_access_to_static_member` → `Change access to static using
///   'C'`, with the class name as the library sees it (with a prefix, if it
///   is reached through one);
/// * `record_literal_one_positional_no_trailing_comma` → `Add trailing comma`.
///
/// Each edit is checked against the tree of the current text: a diagnostic
/// that does not match an expected node (text already changed, different
/// shape) produces no action.
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
    // The analyzer tree (the fix context's `node`/`coveringNode`), only when
    // some producer needs it.
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
                let Some(edits) = cx().remove_local_variable(d.span) else {
                    continue;
                };
                output.push(fix(
                    uri,
                    "Remove unused local variable".into(),
                    "quickfix.remove.unusedLocalVariable",
                    edits,
                    d,
                ));
            }
            Some("unused_element")
                if d.code
                    .is_some_and(|c| c.info().unique.ends_with("UNUSED_ELEMENT_PARAMETER")) =>
            {
                // `RemoveUnusedParameter`.
                let Some(s) = cx().remove_parameter(d.span) else {
                    continue;
                };
                output.push(fix(
                    uri,
                    "Remove the unused parameter".into(),
                    "quickfix.remove.unusedParameter",
                    vec![(s, String::new())],
                    d,
                ));
            }
            Some("unused_element") => {
                // `RemoveUnusedElement`: without references in the unit.
                let Some(ranges) = cx().remove_element(d.span) else {
                    continue;
                };
                let edits = ranges.into_iter().map(|s| (s, String::new())).collect();
                output.push(fix(
                    uri,
                    "Remove unused element".into(),
                    "quickfix.remove.unusedElement",
                    edits,
                    d,
                ));
            }
            Some("unnecessary_cast") => {
                // `RemoveUnnecessaryCast`: the `coveringNode` is the `AsExpression`;
                // deletes ` as T` and the surrounding parentheses while the precedence of
                // the parenthesis's parent does not exceed that of `as`.
                let cx = cx();
                let Some(n) = cx.tree.locate2(d.span.start, d.span.end.saturating_sub(1)) else {
                    continue;
                };
                if cx.kind(n) != "AsExpression" {
                    continue;
                }
                let Some(&expression) = cx.children(n).first() else {
                    continue;
                };
                let mut edits = vec![(
                    Span {
                        start: cx.tree.nodes[expression].end,
                        end: cx.tree.nodes[n].end,
                    },
                    String::new(),
                )];
                let mut current = n;
                while let Some(p) = cx.parent(current)
                    && cx.kind(p) == "ParenthesizedExpression"
                {
                    if cx.parent_precedence(p) > cx.precedence(n) {
                        break;
                    }
                    let s = cx.tree.span(p);
                    edits.push((
                        Span {
                            start: s.start,
                            end: s.start + 1,
                        },
                        String::new(),
                    ));
                    edits.push((
                        Span {
                            start: s.end - 1,
                            end: s.end,
                        },
                        String::new(),
                    ));
                    current = p;
                }
                output.push(fix(
                    uri,
                    "Remove unnecessary cast".into(),
                    "quickfix.remove.unnecessaryCast",
                    edits,
                    d,
                ));
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
                // `ReplaceWithNotNullAware` over the `coveringNode`.
                let cx = cx();
                let Some(n) = cx.tree.locate2(d.span.start, d.span.end.saturating_sub(1)) else {
                    continue;
                };
                if let Some((s, new, title)) = cx.swap_null_aware_operator(n) {
                    output.push(fix(
                        uri,
                        format!("Replace with '{title}'"),
                        "quickfix.replace.withNotNullAware",
                        vec![(s, new)],
                        d,
                    ));
                }
            }
            Some("instance_access_to_static_member") => {
                // `ChangeToStaticAccess`: the `node` is the name of a `MethodInvocation` or
                // the identifier of a `PrefixedIdentifier`.
                let cx = cx();
                if let Some((name, s, new_text, import)) = cx.static_access(d.span) {
                    let mut m = crate::refactorings_exec::Change::default();
                    m.add(uri, s, new_text);
                    crate::refactorings_move::builder_imports(cx, &mut m, &import);
                    if m.conflict.is_none() {
                        output.push(CodeAction {
                            title: format!("Change access to static using '{name}'"),
                            kind: "quickfix.change.toStaticAccess".into(),
                            edits: m
                                .files
                                .into_iter()
                                .flat_map(|(_, l)| l.into_iter().rev())
                                .collect(),
                            diagnostic: Some(d.clone()),
                            create_file: None,
                            linked: Default::default(),
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
                // `MakeFieldNotFinal` and `AddLate`, in this order.
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
                // `AddNeNull` (static type without `?` → nothing) and `AddAwait.nonBool`
                // (`Future<bool>` → `await `), in this order.
                let cx = cx();
                let Some(n) = cx.tree.locate(d.span.start, d.span.end) else {
                    continue;
                };
                if !cx.is_expression(n) {
                    continue;
                }
                let ty = cx.node_type(n).map(|t| query.table.get(t).clone());
                if ty.as_ref().is_none_or(|t| t.is_declared_nullable()) {
                    output.push(fix(
                        uri,
                        "Add != null".into(),
                        "quickfix.add.neNull",
                        vec![(
                            Span {
                                start: d.span.end,
                                end: d.span.end,
                            },
                            " != null".into(),
                        )],
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
                        vec![(
                            Span {
                                start: d.span.start,
                                end: d.span.start,
                            },
                            "await ".into(),
                        )],
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

/// The tokens of `text[start..end]`, with spans in the whole text.
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

/// The field written by `assignment_to_final`: the identifier's
/// `writeOrReadElement` is the synthetic getter (the implicit accessor) of a
/// non-synthetic field without a setter; the field, its unit and name, the
/// list and the member that declares it.
fn written_field<'p>(
    project: &'p Project,
    unit: UnitId,
    d: &Diagnostic,
) -> Option<(
    dartforge_elements::model::VariableId,
    UnitId,
    Span,
    &'p ast::VariableList,
    &'p ast::Member,
)> {
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
        Resolved::Member {
            member: MemberRef::Function(f),
            ..
        }
        | Resolved::Element(Element::Function(f)) => *f,
        Resolved::Member {
            member: MemberRef::Variable(v),
            ..
        }
        | Resolved::Element(Element::Variable(v)) => p.variable(*v).getter?,
        _ => return None,
    };
    let fe = p.function(f);
    if fe.kind != dartforge_elements::model::FunctionKind::ImplicitAccessor
        || query.name(fe.name).ends_with('=')
    {
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
    let ast::MemberKind::Field(l) = &member.kind else {
        return None;
    };
    Some((v, u_decl, name_, l, member))
}

/// `MakeFieldNotFinal` (`make_field_not_final.dart`): a **class** field, a
/// list of one variable with `final`; with a type, deletes `final `; without
/// one, replaces it with `var `. Edits the declaration's file (Dart records
/// the edit in the use's file, a defect that only shows up across files and
/// is not reproduced).
fn non_final_field(project: &Project, unit: UnitId, d: &Diagnostic) -> Option<CodeAction> {
    let p = &project.query.program;
    let (v, u_decl, name, l, member) = written_field(project, unit, d)?;
    let class = p.variable(v).class?;
    if !matches!(
        p.class(class).kind,
        dartforge_elements::model::ClassKind::Class
            | dartforge_elements::model::ClassKind::MixinApplication
    ) {
        return None;
    }
    if l.variables.len() != 1 || !l.final_ || l.const_ {
        return None;
    }
    let text = p.unit(u_decl).source.as_str();
    let final_ = tokens(text, member.span.start, name.start)
        .into_iter()
        .find(|t| {
            t.kind
                == dartforge_frontend::token::Kind::Keyword(
                    dartforge_frontend::token::Keyword::Final,
                )
        })?;
    let (span, new) = match l.ty {
        // `range.startStart(final, type)`.
        Some(t) => (
            Span {
                start: final_.span.start,
                end: p.unit(u_decl).ast.ty(t).span.start,
            },
            String::new(),
        ),
        // `range.startStart(final, variable)` with `var `.
        None => (
            Span {
                start: final_.span.start,
                end: name.start,
            },
            "var ".to_string(),
        ),
    };
    let uri_decl = project.unit_uri(u_decl)?;
    Some(CodeAction {
        title: format!("Make field '{}' not final", &text[name.start..name.end]),
        kind: "quickfix.makeFieldNotFinal".into(),
        edits: vec![Edit {
            uri: uri_decl,
            span,
            text: new,
        }],
        diagnostic: Some(d.clone()),
        create_file: None,
        linked: Default::default(),
    })
}

/// `AddLate` (`add_late.dart:60-91`): a field of a class, mixin, enum or
/// extension type, not `late`, in a `FieldDeclaration` of one variable with
/// `final`: `late ` before the `final`, in the field's file.
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
        .find(|t| {
            t.kind
                == dartforge_frontend::token::Kind::Keyword(
                    dartforge_frontend::token::Keyword::Final,
                )
        })?;
    let uri_decl = project.unit_uri(u_decl)?;
    Some(CodeAction {
        title: "Add 'late' modifier".into(),
        kind: "quickfix.add.late".into(),
        edits: vec![Edit {
            uri: uri_decl,
            span: Span {
                start: final_.span.start,
                end: final_.span.start,
            },
            text: "late ".into(),
        }],
        diagnostic: Some(d.clone()),
        create_file: None,
        linked: Default::default(),
    })
}

/// `abstract_field_initializer` → `Remove the 'abstract' keyword` and
/// `Remove initializer`, in this order (that of `RemoveAbstract` and
/// `RemoveInitializer` in Dart's producer list). `RemoveAbstract` skips a
/// list of several variables whose type is not nullable.
fn initialized_abstract_field(
    project: &Project,
    unit: UnitId,
    uri: &str,
    ast: &ast::Ast,
    text: &str,
    d: &Diagnostic,
) -> Vec<CodeAction> {
    let mut output = Vec::new();
    let Some((member, list, var)) = ast.members.iter().find_map(|m| match &m.kind {
        ast::MemberKind::Field(l) if l.abstract_ => l
            .variables
            .iter()
            .find(|x| x.name.span == d.span)
            .map(|x| (m, l, x)),
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
            .find(|t| {
                t.kind == dartforge_frontend::token::Kind::Ident
                    && &text[t.span.start..t.span.end] == "abstract"
            });
        if let Some(t) = abstract_ {
            // `[abstract.offset, nextToken.offset)`.
            let next = tokens(text, t.span.end, var.name.span.end)
                .into_iter()
                .find(|x| x.kind != dartforge_frontend::token::Kind::Eof)
                .map_or(var.name.span.start, |x| x.span.start);
            output.push(fix(
                uri,
                "Remove the 'abstract' keyword".into(),
                "quickfix.remove.abstract",
                vec![(
                    Span {
                        start: t.span.start,
                        end: next,
                    },
                    String::new(),
                )],
                d,
            ));
        }
    }
    if let Some(init) = var.initializer {
        let end = ast.expr(init).span.end;
        output.push(fix(
            uri,
            "Remove initializer".into(),
            "quickfix.remove.initializer",
            vec![(
                Span {
                    start: var.name.span.end,
                    end,
                },
                String::new(),
            )],
            d,
        ));
    }
    output
}

/// `uri_does_not_exist` → `Create file 'x.dart'` (`CreateFile`,
/// `create_file.dart`): the `node` is the URI's `SimpleStringLiteral`; for
/// an `import`/`export` with an absolute referenced `.dart` source, the file
/// with `// TODO Implement this library.`; for a `part`, `part of '<library
/// relative to the part's folder>';` and two line breaks.
fn create_file(project: &Project, unit: UnitId, _uri: &str, d: &Diagnostic) -> Option<CodeAction> {
    let p = project.program();
    let u = p.unit(unit);
    let (literal, part) = u.unit.directives.iter().find_map(|dir| match &dir.kind {
        DirectiveKind::Import { uri, .. } | DirectiveKind::Export { uri, .. }
            if uri.span == d.span =>
        {
            Some((uri, false))
        }
        DirectiveKind::Part { uri } if uri.span == d.span => Some((uri, true)),
        _ => None,
    })?;
    // Only a simple literal (no interpolation nor adjacency).
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
    // `referencedSource`: `package:` through the `package_config`, `file:`, or
    // relative to the unit.
    let target: PathBuf = if text.starts_with("package:") {
        let configuration = dartforge_elements::config::PackageConfig::discover(&current)
            .and_then(|c| dartforge_elements::config::PackageConfig::load(&c).ok())?;
        configuration.resolve_package_uri(&text).ok()?
    } else if text.starts_with("file:") {
        Url::parse(&text).ok()?.to_file_path().ok()?
    } else if text.contains(':') {
        return None;
    } else {
        Url::from_file_path(&current)
            .ok()?
            .join(&text)
            .ok()?
            .to_file_path()
            .ok()?
    };
    let name = target.file_name()?.to_string_lossy().into_owned();
    let content = if part {
        let library = p
            .library(u.library)
            .units
            .first()
            .and_then(|&x| p.unit(x).path.clone())?;
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

/// `Organize Imports` (`source.organizeImports`) over the merely parsed
/// unit: the `ImportOrganizer` of `crate::organize_imports`
/// (docs/LSP-ESPECIFICACAO.md §13.12.6), which sorts, groups and removes
/// textual duplicates. Without semantic errors, no import is removed for
/// being unused. With nothing to change, the action comes without an edit.
pub(crate) fn organize_imports(uri: &str, text: &str) -> CodeAction {
    let mut names = dartforge_intern::Interner::new();
    let analyzed = dartforge_frontend::parser::parse(text, &mut names);
    let edits = crate::organize_imports::organize(text, &analyzed.unit, &[], false, true)
        .and_then(|new| crate::organize_imports::edit(text, &new))
        .map(|(offset, length, new)| {
            vec![Edit {
                uri: uri.to_string(),
                span: Span {
                    start: offset,
                    end: offset + length,
                },
                text: new,
            }]
        })
        .unwrap_or_default();
    CodeAction {
        title: "Organize Imports".into(),
        kind: "source.organizeImports".into(),
        edits,
        diagnostic: None,
        create_file: None,
        linked: Default::default(),
    }
}
