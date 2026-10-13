//! `textDocument/completion`: members by the receiver's static type, scope
//! names, keywords and named arguments.
//!
//! The typing point almost never parses: `a.` without a name, `a.ca`
//! without `;`. The parser recovers per statement (skips to the `;` or the
//! block's `}`), so the incomplete statement disappears from the tree.
//! Before parsing, the name under the cursor is replaced by a sentinel
//! identifier and, if needed, a short closing (`;`, `);` …) is appended
//! after it; the variant that puts the sentinel in an expression with the
//! fewest diagnostics wins. The program is then loaded with that text and
//! the library's bodies are inferred by the common inference
//! (`crates/types`): the receiver's type is the target's `get_type`, and
//! the lexical scope is captured by the inference's probe on the sentinel
//! identifier.
//!
//! In a type position (`Str▮ x`, `List<▮>`, `void f(p.▮ a)`), only types:
//! classes, mixins, enums, extension types, typedefs, type parameters in
//! scope, prefixes and `dynamic`/`void` — never values.
//!
//! Public names of libraries not imported yet (SDK and project, through the
//! indexes of [`crate::index`]) come with the edit that adds the `import`
//! (`additionalTextEdits`), like Dart's auto-import; not in parts (the
//! `import` would go to another file).
//!
//! The filter is Dart's `FuzzyMatcher` ([`crate::fuzzy_matcher`]): prefix,
//! contains, word initials and a subsequence that starts like the name. The
//! order is by relevance and stable: what starts with the typed text comes
//! before what only matches fuzzily; then the group (named arguments,
//! locals, class members — those inherited from `Object` last —, library
//! declarations, imported ones, prefixes, not imported ones, keywords) and
//! the name.

use crate::DocumentStore;
use crate::query::Query;
use crate::index::{ProjectIndex, SdkIndex};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{
    ClassId, ClassKind, Element, FunctionElementId, FunctionKind, LibraryId,
};
use dartforge_elements::sdk::SdkLayout;
use dartforge_frontend::LibraryFeatures;
use dartforge_frontend::ast::{self, ExprId, ExprKind};
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::{MemberRef, Resolved, Type, TypeId};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

mod target;
mod context;
mod isp;
mod isp_declarations;
mod isp_not_imported;
mod isp_helpers;
mod isp_keywords;
mod isp_visits;

/// Identifier that takes the place of the name under the cursor in the analysis.
const SENTINEL: &str = "dartforge__completar";

/// Closings tried after the sentinel, in order of preference.
const CLOSURES: &[&str] = &["", ";", ")", ");", "))", "));", ")));", "]", "];", "});"];

/// The LSP `CompletionItemKind`.
pub(crate) mod kind {
    pub const METHOD: u32 = 2;
    pub const FUNCTION: u32 = 3;
    pub const CONSTRUCTOR: u32 = 4;
    pub const FIELD: u32 = 5;
    pub const VARIABLE: u32 = 6;
    pub const CLASS: u32 = 7;
    pub const MODULE: u32 = 9;
    pub const PROPERTY: u32 = 10;
    pub const ENUM: u32 = 13;
    pub const WORD_KEY: u32 = 14;
    pub const ENUM_MEMBER: u32 = 20;
    pub const TYPE_PARAMETER: u32 = 25;
}

/// How to complete a call, when the client accepts snippets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    /// The required parameters, in order (`x`, and `name: ` for named ones):
    /// they become the snippet's `${1:x}` placeholders.
    Parameters(Vec<String>),
    /// Function with parameters of unknown names (name index): the cursor stays
    /// between the parentheses.
    Unknown,
}

/// An `import` to add together with the item (auto-import).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoImport {
    /// The imported URI (`dart:math`, `util.dart`).
    pub uri: String,
    /// Where the directive goes in the document (bytes) and its text.
    pub span: Span,
    pub text: String,
}

/// An item offered to the editor.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionItem {
    /// What the list shows (`met(…)`, `name: `).
    pub label: String,
    /// `CompletionItemKind`.
    pub kind: u32,
    /// Type or signature.
    pub detail: Option<String>,
    /// Text that replaces the typed prefix.
    pub insert: String,
    /// Call shape, for functions, methods and constructors.
    pub call: Option<Call>,
    /// File and start of the declaration, for the documentation of
    /// `completionItem/resolve`.
    pub origin: Option<(PathBuf, usize)>,
    /// The directive the item adds (a name not imported yet).
    pub import: Option<AutoImport>,
    /// Sorting group (smaller comes first).
    group: u8,
    /// Quality of the match with the typed text (0: prefix; 1: fuzzy).
    quality: u8,
    /// What goes into the relevance computation (`crate::relevance`).
    rel: crate::relevance::Rel,
    /// Class of the item (top-level element), for the context type.
    class: Option<ClassId>,
    /// The computed relevance (`0..1000`).
    relevance: i32,
    /// `sortText` = `9999 − relevance` (`MAP:59`, `:735-736`).
    pub sort_text: String,
    /// The score of the handler's `FuzzyMatcher` (`fuzzy.suggestionScore`), for
    /// truncation.
    pub score: f64,
    /// `selectionOffset`/`selectionLength` (UTF-16 units of the inserted text),
    /// when the suggestion puts the cursor before the end.
    pub selection: Option<(usize, usize)>,
    /// `displayText` (the label, when it differs from the inserted text).
    pub display: Option<String>,
    /// The item came from the `InScopeCompletionPass` (the order and the
    /// `matcherScore` are its own).
    pub(crate) of_pass: bool,
    /// `CompletionSuggestionKind.IDENTIFIER` on an executable (tear-off,
    /// redirection): the label shows the parameters, the insertion adds no
    /// parentheses.
    pub(crate) identifier: bool,
    /// The text the handler's filter matches (`textToMatchOverride`:
    /// `override_x`, `setState`), when it differs from the inserted text.
    pub(crate) match_: Option<String>,
    /// `replacementLength: 0`: the insertion does not replace the prefix.
    pub(crate) replace_empty: bool,
}

/// Completion result: the prefix range (bytes) and the items in order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Completion {
    /// The `replacementOffset` (bytes).
    pub start: usize,
    /// The end of `insert` (`replacementOffset + min(offset -
    /// replacementOffset, replacementLength)`).
    pub end: usize,
    /// The end of `replacementRange`, when the pass computed it (otherwise the
    /// handler extends the word).
    pub replacement_end: Option<usize>,
    pub items: Vec<CompletionItem>,
    /// The list was cut (not-imported names above the ceiling).
    pub incomplete: bool,
}

/// Where the sentinel landed in the analysis.
#[derive(Clone, Copy)]
enum Sentinel {
    /// In an expression; with the target when it is `target.▮`.
    Expr(ExprId, Option<ExprId>),
    /// In a type name; with the prefix when it is `p.▮`, and whether the type
    /// opens the declaration (top-level, member or statement), where keywords
    /// also fit.
    Type {
        prefix: Option<SymbolId>,
        leader: Option<&'static [&'static str]>,
    },
}

/// What completion queries besides the program.
pub(crate) struct Indices<'a> {
    pub sdk: &'a SdkIndex,
    pub project: &'a mut ProjectIndex,
    /// The summaries of the known libraries (`FileStateFilter` and
    /// `exportNamespace`) for the not-imported items.
    pub known: &'a mut crate::known_libraries::LibraryIndex,
}

mod group {
    pub const NAMED: u8 = 0;
    pub const LOCAL: u8 = 1;
    pub const MEMBER: u8 = 2;
    /// Members inherited from `Object` (`toString`, `hashCode`…).
    pub const OBJECT_MEMBER: u8 = 3;
    pub const LIBRARY: u8 = 4;
    pub const IMPORTED: u8 = 5;
    pub const PREFIX: u8 = 6;
    pub const NOT_IMPORTED: u8 = 7;
    pub const WORD: u8 = 8;
}

/// Ceiling of items from not-imported libraries in one response; above it
/// the list comes out with `isIncomplete` and the editor asks again at each
/// keystroke.

const EXPRESSION_WORDS: &[&str] = &["const", "false", "new", "null", "true"];
const COMMAND_WORDS: &[&str] = &[
    "assert", "break", "const", "continue", "do", "false", "final", "for", "if", "late", "new",
    "null", "return", "switch", "throw", "true", "try", "var", "void", "while",
];
const TOP_WORDS: &[&str] = &[
    "abstract",
    "base",
    "class",
    "const",
    "enum",
    "export",
    "extension",
    "external",
    "final",
    "import",
    "interface",
    "late",
    "library",
    "mixin",
    "part",
    "sealed",
    "typedef",
    "var",
    "void",
];
const MEMBER_WORDS: &[&str] = &[
    "abstract",
    "const",
    "covariant",
    "external",
    "factory",
    "final",
    "get",
    "late",
    "operator",
    "set",
    "static",
    "var",
    "void",
];

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// Completes at position `offset` (bytes) of `text`, the current content of
/// `uri`.
///
/// `None` when the file cannot be loaded (a URI that is not a file). Inside
/// a comment, string text or a number, the list is empty.
/// The `maxSuggestions` (`dart.maxCompletionItems`, default
/// `defaultMaxCompletions = 2000`).
pub(crate) const MAX_PATTERN: usize = 2000;

/// `SuggestionCollector.addSuggestion` (`SC:45-81`): stable insertion by
/// descending `matcherScore` and pruning above `max`.
fn collect(list: &mut Vec<(f64, CompletionItem)>, score: f64, item: CompletionItem, max: usize) {
    let mut position = 0;
    for k in (0..list.len()).rev() {
        if list[k].0 >= score {
            position = k + 1;
            break;
        }
    }
    list.insert(position, (score, item));
    if list.len() > max {
        let min = list[max].0;
        while list.len() > max && list.last().is_some_and(|(s, _)| *s < min) {
            list.pop();
        }
    }
}

/// The scored text (`displayName`): the insertion label without the `: ` of
/// a named argument.
fn scored_text(i: &CompletionItem) -> &str {
    match &i.match_ {
        Some(c) => c,
        None => i.insert.trim_end_matches([':', ' ']),
    }
}

pub(crate) fn completion(
    sdk: &SdkLayout,
    indices: Indices<'_>,
    documents: &DocumentStore,
    uri: &str,
    text: &str,
    offset: usize,
    features: LibraryFeatures,
    max: usize,
    not_imported: bool,
    retained: Option<&mut crate::project::Project>,
) -> Option<Completion> {
    let bytes = text.as_bytes();
    let offset = offset.min(text.len());
    let mut start = offset;
    while start > 0 && is_ident(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = offset;
    while end < bytes.len() && is_ident(bytes[end]) {
        end += 1;
    }
    let empty = Completion {
        start: offset,
        end: offset,
        replacement_end: None,
        items: Vec::new(),
        incomplete: false,
    };
    // The analyzed text replaces the word under the cursor with the sentinel
    // (only for semantics: the context is that of the real text,
    // `completion/isp.rs`).
    let typed = &text[start..offset];
    let prepared = prepare(text, start, end, features);
    let analyzed_text = prepared.as_deref().unwrap_or(text);
    let delta = analyzed_text.len() as isize - text.len() as isize;
    let request = Request { sdk, indices, documents, text, start, end, offset, typed, speculative: prepared.is_some(), max, not_imported, delta };
    // The incremental path (docs/LSP-ESPECIFICACAO.md §16.7): the retained
    // state is that of the document's text; the text with the sentinel swaps
    // the unit in place, only the sentinel's body is inferred (with the
    // probe), and the swap is undone after the response. Outside a body, or
    // with the signature, the directives or the shape changed, the full load.
    if let Some(project) = retained
        && let Some(unit) = project.uri_unit(uri)
        && project.program().unit(unit).source == text
        && project.libraries.contains(&project.program().unit(unit).library)
    {
        let query = &mut project.query;
        match prepared.as_deref() {
            None => return respond(query, unit, request, empty),
            Some(speculative) => {
                if let Some(swap) = query.swap_unit(unit, speculative, crate::query::SwapMode::Speculative { offset: start }) {
                    let response = respond(query, unit, request, empty);
                    query.undo(swap);
                    return response;
                }
            }
        }
    }
    let (program, names, unit) =
        crate::semantic::load(sdk, uri, analyzed_text, Some(documents))?;
    let library = program.unit(unit).library;
    let probe = prepared.as_ref().map(|_| (unit, start));
    let mut query = Query::infer(program, names, &[library], false, probe);
    respond(&mut query, unit, request, empty)
}

/// The completion request, already with the word's range.
struct Request<'a, 'i> {
    sdk: &'a SdkLayout,
    indices: Indices<'i>,
    documents: &'a DocumentStore,
    text: &'a str,
    start: usize,
    end: usize,
    offset: usize,
    typed: &'a str,
    /// The analyzed text has the sentinel.
    speculative: bool,
    max: usize,
    /// Suggest not-imported items (`suggestFromUnimportedLibraries` with
    /// `workspace/applyEdit`).
    not_imported: bool,
    /// The shift of the offsets after the word in the analyzed text.
    delta: isize,
}

/// The suggestions over the prepared query (the retained one, with the unit
/// swapped for the speculative text, or a loaded one): the
/// `InScopeCompletionPass` over the real text (`completion/isp.rs`), the
/// `NotImportedCompletionPass`, the relevance of each candidate, the
/// collector (`matcherScore`, pruning), the suggestion map and the handler's
/// filter and truncation.
fn respond(query: &mut Query, unit: dartforge_elements::model::UnitId, request: Request<'_, '_>, empty: Completion) -> Option<Completion> {
    let Request { sdk, indices, documents, text, start, end, offset, typed: _, speculative, max, not_imported, delta } = request;
    let features = query.program.unit(unit).features;
    let before_3 = features.versao() < dartforge_frontend::LanguageVersion::new(3, 0);
    let swap = speculative.then_some((start, end, SENTINEL.len(), delta));
    // In a part, the `import` would go to the owning library (another file).
    let part = query.program.unit(unit).unit.directives.iter().any(|d| matches!(d.kind, ast::DirectiveKind::PartOf { .. }));
    // The candidate libraries for the not-imported items (`FileStateFilter`,
    // minus the request's own and those imported without combinators by the
    // defining unit, `_ImportSummary`), with the `import` URI.
    let filter = candidates_filter(query, unit);
    let Indices { sdk: _, project: _, known } = indices;
    let candidates = |c: &mut crate::known_libraries::LibraryIndex| -> Vec<(PathBuf, String)> {
        let Some((file, own, imported)) = &filter else { return Vec::new() };
        let root = crate::project::project_root(file);
        let package = crate::index::package_name(&root);
        let key = |x: &std::path::Path| dartforge_elements::gerado::chave(x);
        let mut v = Vec::new();
        for candidate in crate::known_libraries::candidates(file, Some(sdk), c, documents) {
            let k = key(&candidate.path);
            if own.contains(&k) || imported.contains(&k) {
                continue;
            }
            let uri = if candidate.sdk || candidate.uri.starts_with("package:") {
                candidate.uri.clone()
            } else {
                match crate::index::import_uri(file, &candidate.path, &root, package.as_deref()) {
                    Some(u) => u,
                    None => continue,
                }
            };
            v.push((candidate.path, uri));
        }
        v
    };
    let r = {
        let mut list = || candidates(known);
        let not_imported_: Option<&mut dyn FnMut() -> Vec<(PathBuf, String)>> = if part || !not_imported { None } else { Some(&mut list) };
        isp::run(query, unit, text, features, offset, swap, before_3, not_imported_)
    };
    if r.comment {
        return Some(empty);
    }
    let library = query.program.unit(unit).library;
    // The sentinel belongs to the analyzed text, not to the user.
    let mut arrival: Vec<(f64, CompletionItem)> = r.items.into_iter().filter(|(_, i)| !i.insert.contains(SENTINEL)).collect();
    // `StaticMembersOperation` over the libraries the query did not load: the
    // names of the `exportNamespace` summary.
    if r.operations.iter().any(|o| matches!(o, isp::Operation::StaticMembers)) && !r.summarized.is_empty() {
        let only_types = r.flags.as_ref().is_some_and(|f| f.ty);
        let mut collector = Collector { items: Vec::new(), library, receiver: None };
        let mut names = Interner::new();
        let real = dartforge_frontend::parser::parse_com(text, &mut names, features);
        if let Some((file, _, _)) = &filter {
            collector.not_imported_summary(text, &real.unit, only_types, sdk, file, known, documents, &r.summarized, &r.visible);
        }
        let mut matcher = (!r.matcher_prefix.is_empty()).then(|| crate::fuzzy_matcher::Matcher::new(&r.matcher_prefix, crate::fuzzy_matcher::Style::Text));
        for i in collector.items {
            let s = match matcher.as_mut() {
                Some(c) => c.score(i.insert.as_str()),
                None => 0.0,
            };
            if s != -1.0 {
                arrival.push((s, i));
            }
        }
    }
    // The relevance (`relevanceComputer.computeRelevance`): the location
    // recorded by the pass, the request's context type and `preferConstants`.
    let local = r.local.clone();
    let expected = r.context_type;
    let prefer_constants = r.prefers_constants || r.in_constant_context;
    let type_bool = query.core.bool_;
    let null_type = query.core.null;
    for (_, i) in &mut arrival {
        if i.rel.ty.is_none() {
            i.rel.ty = match i.rel.word {
                Some("true" | "false") => Some(type_bool),
                Some("null") => Some(null_type),
                _ => None,
            };
        }
        if i.rel.ty.is_none()
            && let Some(c) = i.class
        {
            i.rel.ty = class_type(query, c);
        }
        let context = match (expected, i.rel.ty) {
            (Some(e), Some(t)) => context_feature(query, e, t),
            _ => 0.0,
        };
        i.relevance = crate::relevance::relevance(&i.rel, local.as_deref(), context, prefer_constants);
        i.sort_text = (9999 - i.relevance).to_string();
    }
    // The collector: stable insertion by `matcherScore`, and pruning.
    let mut list: Vec<(f64, CompletionItem)> = Vec::new();
    for (s, i) in arrival {
        collect(&mut list, s, i, max);
    }
    // The `_suggestionMap` (`SB:989-1026`): the key is the text (a constructor
    // with `()`, a not-imported item with `::uri`); the last one wins, at the
    // position of the first; a constructor whose class name is already in the
    // map through an element that is not a class is left out.
    let mut order: Vec<CompletionItem> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for (_, i) in list {
        if i.kind == kind::CONSTRUCTOR
            && let Some(c) = i.class
        {
            let class_name = query.name(query.program.class(c).name).to_string();
            if let Some(&k) = index.get(&class_name) {
                let existing = &order[k];
                let is_class = existing.class.is_some_and(|x| {
                    let cl = query.program.class(x);
                    matches!(cl.kind, ClassKind::Class | ClassKind::MixinApplication)
                }) && existing.kind == kind::CLASS;
                if !is_class {
                    continue;
                }
            }
        }
        let mut key = i.insert.clone();
        if i.kind == kind::CONSTRUCTOR {
            key.push_str("()");
        }
        if let Some(imp) = &i.import
            && i.rel.not_imported
        {
            key.push_str("::");
            key.push_str(&imp.uri);
        }
        match index.get(&key) {
            Some(&k) => order[k] = i,
            None => {
                index.insert(key, order.len());
                order.push(i);
            }
        }
    }
    // The handler: `fuzzy.suggestionScore(item) > 0` by the request's prefix
    // (empty → 1.0).
    let mut fuzzy = crate::fuzzy_matcher::Matcher::new(&r.request_prefix, crate::fuzzy_matcher::Style::Text);
    for i in &mut order {
        i.score = fuzzy.score(scored_text(i));
    }
    order.retain(|i| i.score > 0.0);
    let mut incomplete = r.incomplete;
    // `_truncateResults` above the maximum.
    if order.len() > max {
        let prefix = r.request_prefix.to_lowercase();
        order.sort_by(|a, b| {
            if a.score != b.score {
                return b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal);
            }
            if a.sort_text == b.sort_text {
                return a.label.len().cmp(&b.label.len());
            }
            a.sort_text.cmp(&b.sort_text)
        });
        let mut k = 0usize;
        order.retain(|i| {
            let keep = k < max || (!prefix.is_empty() && scored_text(i).to_lowercase() == prefix);
            k += 1;
            keep
        });
        incomplete = true;
    }
    // The handler's `replacementRange` and `insert` (`min(offset -
    // replacementOffset, replacementLength)`).
    let (ri, rl) = r.range;
    let insertion = ri + offset.saturating_sub(ri).min(rl);
    Some(Completion { start: ri, end: insertion, replacement_end: Some(ri + rl), items: order, incomplete })
}

/// What the filter of the not-imported candidates needs: the request's file,
/// the units of its own library and the libraries imported without
/// combinators by the defining unit (`_ImportSummary`).
#[allow(clippy::type_complexity)]
fn candidates_filter(query: &Query, unit: dartforge_elements::model::UnitId) -> Option<(PathBuf, HashSet<PathBuf>, HashSet<PathBuf>)> {
    let program = &query.program;
    let lib = program.library(program.unit(unit).library);
    let file = program.unit(unit).path.clone()?;
    let defining = lib.units.first().copied();
    let key = |c: &std::path::Path| dartforge_elements::gerado::chave(c);
    let imported: HashSet<PathBuf> = lib
        .imports
        .iter()
        .filter(|i| Some(i.unit) == defining && i.combinators.is_empty())
        .filter_map(|i| program.library(i.library).units.first())
        .filter_map(|u| program.unit(*u).path.as_deref().map(key))
        .collect();
    let own: HashSet<PathBuf> = lib.units.iter().filter_map(|u| program.unit(*u).path.as_deref().map(key)).collect();
    Some((file, own, imported))
}

/// Text with the name under the cursor replaced by the sentinel (and the
/// closing that makes it parse); `None` when no variant puts the sentinel in
/// an expression or a type name. The variant with the fewest diagnostics
/// wins (the first one, among equals).
fn prepare(text: &str, start: usize, end: usize, features: LibraryFeatures) -> Option<String> {
    let mut best: Option<(usize, String)> = None;
    for closure in CLOSURES {
        let candidate = format!("{}{SENTINEL}{closure}{}", &text[..start], &text[end..]);
        let mut names = Interner::new();
        let analyzed = dartforge_frontend::parser::parse_com(&candidate, &mut names, features);
        if find_sentinel(&analyzed.ast, start).is_none() {
            continue;
        }
        // Without a closing, the text has the real text's tree (the parser inserts
        // the synthetic closings like fasta): it is the choice whenever the
        // sentinel lands in an expression or a type.
        if closure.is_empty() {
            return Some(candidate);
        }
        let errors = analyzed.diagnostics.len();
        if best.as_ref().is_none_or(|(e, _)| errors < *e) {
            let zero = errors == 0;
            best = Some((errors, candidate));
            if zero {
                break;
            }
        }
    }
    best.map(|(_, t)| t)
}

/// Where the sentinel at `start` is: in an expression (with the target, if
/// it is a member access) or in a type name.
fn find_sentinel(ast: &ast::Ast, start: usize) -> Option<Sentinel> {
    let eh = |n: &ast::Name| n.span.start == start && n.span.end == start + SENTINEL.len();
    let expr = ast
        .exprs
        .iter()
        .enumerate()
        .find_map(|(i, e)| match &e.kind {
            ExprKind::Identifier(n) if eh(n) => Some(Sentinel::Expr(ExprId(i as u32), None)),
            ExprKind::Property { target, name, .. } if eh(name) => {
                Some(Sentinel::Expr(ExprId(i as u32), Some(*target)))
            }
            _ => None,
        });
    if expr.is_some() {
        return expr;
    }
    let (ty, name_) = ast.types.iter().find_map(|t| match &t.kind {
        ast::TypeKind::Named { name, .. } if name.last().is_some_and(eh) => Some((t, name)),
        _ => None,
    })?;
    let prefix = (name_.len() == 2).then(|| name_[0].sym);
    // The type opens a top-level declaration, a member or a statement: there
    // the declaration keywords also fit.
    let opens = |s: dartforge_diagnostics::Span| s.start == ty.span.start;
    let leader: Option<&'static [&'static str]> = if prefix.is_some() {
        None
    } else if ast.decls.iter().any(|d| opens(d.span)) {
        Some(TOP_WORDS)
    } else if ast.members.iter().any(|m| opens(m.span)) {
        Some(MEMBER_WORDS)
    } else if ast.stmts.iter().any(|s| opens(s.span)) {
        Some(COMMAND_WORDS)
    } else {
        None
    };
    Some(Sentinel::Type { prefix, leader })
}

struct Collector {
    items: Vec<CompletionItem>,
    library: LibraryId,
    /// Class of the receiver while instance members are collected (the
    /// inheritance distance of each member).
    receiver: Option<ClassId>,
}

impl Collector {
    fn push(
        &mut self,
        group: u8,
        kind: u32,
        label: String,
        insert: String,
        detail: Option<String>,
    ) -> &mut CompletionItem {
        self.items.push(CompletionItem {
            label,
            kind,
            detail,
            insert,
            call: None,
            origin: None,
            import: None,
            group,
            quality: 0,
            rel: crate::relevance::Rel::default(),
            class: None,
            relevance: 0,
            sort_text: String::new(),
            score: 0.0,
            selection: None,
            display: None,
            of_pass: false,
            identifier: false,
            match_: None,
            replace_empty: false,
        });
        self.items.last_mut().expect("item just pushed")
    }

    /// Fills the relevance of the last item from function `f` (with type `ty`
    /// as seen through the receiver).
    fn function_rel(&mut self, query: &Query, f: FunctionElementId, ty: TypeId) {
        use crate::relevance::Kind;
        let fe = query.program.function(f);
        let member = fe.class.is_some() || fe.extension.is_some();
        let name = query.name(fe.name).to_string();
        let enum_constant = fe.variable.is_some_and(|v| {
            let ve = query.program.variable(v);
            ve.class.is_some_and(|c| query.program.class(c).enum_constants.contains(&v))
        });
        let (kind, item_type) = match fe.kind {
            _ if enum_constant => (Kind::NoTable, Some(ty)),
            FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => {
                (if member { Kind::Field } else { Kind::TopLevelVariable }, Some(ty))
            }
            FunctionKind::Constructor | FunctionKind::SyntheticConstructor => (Kind::Constructor, return_(query, ty)),
            FunctionKind::Function | FunctionKind::Operator => {
                (if member { Kind::Method } else { Kind::Function }, return_(query, ty))
            }
        };
        let distance = match (self.receiver, fe.class) {
            (Some(r), Some(c)) if member => inheritance_distance(query, r, c).map(|d| d as u32),
            _ => None,
        };
        // `isConstantFeature`: a `const` constructor; a synthetic accessor of a
        // `const` static (or top-level) variable.
        let constant = match fe.kind {
            FunctionKind::Constructor | FunctionKind::SyntheticConstructor => fe.const_,
            FunctionKind::ImplicitAccessor => fe.variable.is_some_and(|v| {
                let ve = query.program.variable(v);
                ve.const_ && (ve.static_ || (ve.class.is_none() && ve.extension.is_none()))
            }),
            _ => false,
        };
        let deprecated = function_deprecated(query, f);
        if let Some(item) = self.items.last_mut() {
            item.rel.kind = Some(kind);
            item.rel.ty = item_type;
            item.rel.distance = distance;
            item.rel.starts_with_dollar = name.starts_with('$');
            item.rel.no_such_method = name == "noSuchMethod";
            item.rel.constant = constant;
            item.rel.deprecated = deprecated;
        }
    }

    /// A private name from another library: invisible here.
    fn invisible(&self, query: &Query, name: &str, owning: LibraryId) -> bool {
        let _ = query;
        name.starts_with('_') && owning != self.library
    }

    /// Item of a function (method, getter, accessor) with type `ty` already seen
    /// through the receiver.
    fn function(&mut self, query: &Query, group: u8, f: FunctionElementId, ty: TypeId) {
        let fe = query.program.function(f);
        let name = query.name(fe.name).trim_end_matches('=').to_string();
        if name.is_empty()
            || !name.as_bytes()[0].is_ascii_alphabetic() && !name.starts_with(['_', '$'])
        {
            return;
        }
        if self.invisible(query, &name, fe.library) {
            return;
        }
        match fe.kind {
            FunctionKind::ImplicitAccessor => {
                let enum_constant = fe.variable.is_some_and(|v| {
                    let ve = query.program.variable(v);
                    ve.class
                        .is_some_and(|c| query.program.class(c).enum_constants.contains(&v))
                });
                let kind = if enum_constant {
                    kind::ENUM_MEMBER
                } else if fe.class.is_some() || fe.extension.is_some() {
                    kind::FIELD
                } else {
                    kind::VARIABLE
                };
                let origin = query.origin(query.function_start(f));
                self.push(
                    group,
                    kind,
                    name.clone(),
                    name,
                    Some(query.format(ty)),
                )
                .origin = origin;
                self.function_rel(query, f, ty);
            }
            FunctionKind::Getter | FunctionKind::Setter => {
                let kind = if fe.class.is_some() || fe.extension.is_some() {
                    kind::PROPERTY
                } else {
                    kind::VARIABLE
                };
                let origin = query.origin(query.function_start(f));
                self.push(
                    group,
                    kind,
                    name.clone(),
                    name,
                    Some(query.format(ty)),
                )
                .origin = origin;
                self.function_rel(query, f, ty);
            }
            FunctionKind::Function
            | FunctionKind::Constructor
            | FunctionKind::SyntheticConstructor => {
                let kind = match fe.kind {
                    FunctionKind::Function if fe.class.is_some() || fe.extension.is_some() => {
                        kind::METHOD
                    }
                    FunctionKind::Function => kind::FUNCTION,
                    _ => kind::CONSTRUCTOR,
                };
                let without_parameters = matches!(query.table.get(ty), Type::Function { positional, optional, named, .. }
                    if positional.is_empty() && optional.is_empty() && named.is_empty());
                let label = if without_parameters {
                    format!("{name}()")
                } else {
                    format!("{name}(…)")
                };
                // The required ones become snippet placeholders: positionals by name and
                // named ones as `name: `.
                let parameters: Vec<String> = query.outline.functions[f.0 as usize]
                    .parameters
                    .iter()
                    .filter_map(|p| {
                        let n = query.name(p.externo.or(p.name)?).to_string();
                        match p.kind {
                            ast::ParameterKind::Required => Some(n),
                            ast::ParameterKind::Named if p.required => Some(format!("{n}: ")),
                            _ => None,
                        }
                    })
                    .collect();
                let origin = query.origin(query.function_start(f));
                let item = self.push(
                    group,
                    kind,
                    label,
                    name,
                    Some(query.function_detail(f, ty)),
                );
                item.call = Some(Call::Parameters(parameters));
                item.origin = origin;
                self.function_rel(query, f, ty);
            }
            FunctionKind::Operator => {}
        }
    }

    /// Item of a top-level element (of the library or imported).
    fn element(&mut self, query: &Query, group: u8, element: Element) {
        let program = &query.program;
        match element {
            Element::Class(c) => {
                let class = program.class(c);
                let name = query.name(class.name).to_string();
                if self.invisible(query, &name, class.library)
                    || class.kind == ClassKind::MixinApplication && class.decl.is_none()
                {
                    return;
                }
                let kind = if class.kind == ClassKind::Enum {
                    kind::ENUM
                } else {
                    kind::CLASS
                };
                let kind_rel = match class.decl.map(|d| &program.unit(d.unit).ast.decl(d.decl).kind) {
                    Some(ast::DeclKind::Enum(_)) => crate::relevance::Kind::Enum,
                    Some(ast::DeclKind::Mixin(_)) => crate::relevance::Kind::Mixin,
                    Some(ast::DeclKind::ExtensionType(_)) => crate::relevance::Kind::NoTable,
                    _ => crate::relevance::Kind::Class,
                };
                let origin = query.origin(query.element_start(element));
                let deprecated = element_deprecated(query, element);
                let item = self.push(group, kind, name.clone(), name, None);
                item.origin = origin;
                item.rel.kind = Some(kind_rel);
                item.rel.deprecated = deprecated;
                item.class = Some(c);
            }
            Element::Typedef(t) => {
                let name = query.name(program.typedef(t).name).to_string();
                if !self.invisible(query, &name, program.typedef(t).library) {
                    let origin = query.origin(query.element_start(element));
                    let target_function = matches!(query.table.get(query.outline.typedefs[t.0 as usize].target_type), Type::Function { .. });
                    let item = self.push(group, kind::CLASS, name.clone(), name, None);
                    item.origin = origin;
                    item.rel.kind = Some(if target_function {
                        crate::relevance::Kind::FunctionAlias
                    } else {
                        crate::relevance::Kind::NoTable
                    });
                }
            }
            Element::Extension(x) => {
                let Some(n) = program.extension(x).name else {
                    return;
                };
                let name = query.name(n).to_string();
                if !self.invisible(query, &name, program.extension(x).library) {
                    let origin = query.origin(query.element_start(element));
                    let item = self.push(group, kind::CLASS, name.clone(), name, None);
                    item.origin = origin;
                    item.rel.kind = Some(crate::relevance::Kind::NoTable);
                }
            }
            Element::Function(f) => {
                let data = &query.outline.functions[f.0 as usize];
                let ty = match program.function(f).kind {
                    FunctionKind::Getter | FunctionKind::ImplicitAccessor => data.return_type,
                    FunctionKind::Setter => data
                        .parameters
                        .first()
                        .map_or(query.core.dynamic_, |p| p.ty),
                    _ => data.signature,
                };
                let ty = match program
                    .function(f)
                    .variable
                    .and_then(|v| query.variable_type(v))
                {
                    Some(t) if program.function(f).kind == FunctionKind::ImplicitAccessor => t,
                    _ => ty,
                };
                self.function(query, group, f, ty);
            }
            Element::Variable(v) => {
                let ve = program.variable(v);
                let name = query.name(ve.name).to_string();
                if !self.invisible(query, &name, ve.library) {
                    let detail = query.variable_type(v).map(|t| query.format(t));
                    let origin = query.origin(query.variable_start(v));
                    let deprecated = element_deprecated(query, element);
                    let item = self.push(group, kind::VARIABLE, name.clone(), name, detail);
                    item.origin = origin;
                    item.rel.kind = Some(crate::relevance::Kind::TopLevelVariable);
                    item.rel.ty = query.variable_type(v);
                    item.rel.constant = ve.const_;
                    item.rel.deprecated = deprecated;
                }
            }
            Element::Prefix(_, p) => {
                let name = query.name(p).to_string();
                self.push(group::PREFIX, kind::MODULE, name.clone(), name, None)
                    .rel
                    .kind = Some(crate::relevance::Kind::Prefix);
            }
        }
    }

    /// The `StaticMembersOperation` of the `NotImportedCompletionPass`
    /// (docs/LSP-ESPECIFICACAO.md §14.9) over the libraries the query did not
    /// load: the `exportNamespace` declarations from the name summary
    /// (`_addExternalTopLevelDeclarations`: only types with `mustBeType`), each
    /// with the `import` that makes it visible; the pass's `VisibilityTracker`
    /// hides those whose name was already seen. The summary has neither the
    /// constructors nor the static fields of the classes.
    #[allow(clippy::too_many_arguments)]
    fn not_imported_summary(
        &mut self,
        text: &str,
        unit: &ast::CompilationUnit,
        only_types: bool,
        sdk: &SdkLayout,
        file: &std::path::Path,
        known: &mut crate::known_libraries::LibraryIndex,
        documents: &DocumentStore,
        candidates: &[(PathBuf, String)],
        visible: &HashSet<String>,
    ) {
        use crate::known_libraries::Kind as E;
        let resolver = crate::known_libraries::Resolver {
            sdk: Some(sdk),
            packages: dartforge_elements::config::PackageConfig::discover(file).and_then(|c| dartforge_elements::config::PackageConfig::load(&c).ok()),
        };
        for (path, uri) in candidates {
            for (name, origin, d) in known.exported(path, &resolver, documents) {
                let ty = crate::known_libraries::OF_TYPE.contains(&d.kind);
                if (only_types && !ty) || visible.contains(&name) {
                    continue;
                }
                let (lsp_kind, rel) = match d.kind {
                    E::Class | E::ExtensionType => (kind::CLASS, crate::relevance::Kind::Class),
                    E::Mixin => (kind::CLASS, crate::relevance::Kind::Mixin),
                    E::Enum => (kind::ENUM, crate::relevance::Kind::Enum),
                    E::FunctionAlias => (kind::CLASS, crate::relevance::Kind::FunctionAlias),
                    E::TypeAlias | E::Extension => (kind::CLASS, crate::relevance::Kind::NoTable),
                    E::Function => (kind::FUNCTION, crate::relevance::Kind::Function),
                    E::Variable => (kind::VARIABLE, crate::relevance::Kind::TopLevelVariable),
                };
                let function = d.kind == E::Function;
                let label = match (function, d.without_parameters) {
                    (true, true) => format!("{name}()"),
                    (true, false) => format!("{name}(…)"),
                    _ => name.clone(),
                };
                let (span, new) = crate::actions::insert_import(text, unit, uri);
                // The summary's written signature is the `detail` (the `Auto import from`
                // comes in `completionItem/resolve`).
                let item = self.push(group::NOT_IMPORTED, lsp_kind, label, name.clone(), d.signature.as_deref().map(str::to_string));
                item.import = Some(AutoImport { uri: uri.clone(), span, text: new });
                item.origin = Some((origin, d.start));
                item.rel.not_imported = true;
                item.rel.kind = Some(rel);
                item.of_pass = true;
                if function {
                    item.call = Some(if d.without_parameters { Call::Parameters(Vec::new()) } else { Call::Unknown });
                }
            }
        }
    }

}

/// The names of the annotations (the last segment) of a metadata list.
fn annotation_names(query: &Query, ms: &[ast::Annotation]) -> Vec<String> {
    ms.iter().filter_map(|m| m.name.last().map(|n| query.name(n.sym).to_string())).collect()
}

/// `@deprecated`/`@Deprecated(...)` among the metadata.
fn has_deprecated(query: &Query, ms: &[ast::Annotation]) -> bool {
    annotation_names(query, ms).iter().any(|n| n == "deprecated" || n == "Deprecated")
}

/// `hasDeprecated` of a top-level element.
fn element_deprecated(query: &Query, el: Element) -> bool {
    let p = &query.program;
    let decl = match el {
        Element::Class(c) => p.class(c).decl,
        Element::Extension(x) => Some(p.extension(x).decl),
        Element::Typedef(t) => Some(p.typedef(t).decl),
        Element::Function(f) => return function_deprecated(query, f),
        Element::Variable(v) => match p.variable(v).node {
            dartforge_elements::model::VariableRef::TopLevel { unit, decl, .. } => Some(dartforge_elements::model::DeclRef { unit, decl }),
            _ => None,
        },
        Element::Prefix(..) => None,
    };
    decl.is_some_and(|d| has_deprecated(query, &p.unit(d.unit).ast.decl(d.decl).metadata))
}

/// `hasOrInheritsDeprecated` of a function, method, accessor or constructor:
/// its own annotation, or that of the member with the same name that it
/// overrides.
fn function_deprecated(query: &Query, f: FunctionElementId) -> bool {
    let p = &query.program;
    let own = |f: FunctionElementId| -> bool {
        match p.function(f).node {
            dartforge_elements::model::FunctionRef::Function { unit, function } => {
                let a = &p.unit(unit).ast;
                a.members
                    .iter()
                    .find(|m| matches!(m.kind, ast::MemberKind::Method(x) if x == function))
                    .map(|m| has_deprecated(query, &m.metadata))
                    .or_else(|| a.decls.iter().find(|d| matches!(d.kind, ast::DeclKind::Function(x) if x == function)).map(|d| has_deprecated(query, &d.metadata)))
                    .unwrap_or(false)
            }
            dartforge_elements::model::FunctionRef::Constructor { unit, member } => has_deprecated(query, &p.unit(unit).ast.member(member).metadata),
            dartforge_elements::model::FunctionRef::None => match p.function(f).variable.map(|v| p.variable(v).node) {
                Some(dartforge_elements::model::VariableRef::Field { unit, member, .. }) => has_deprecated(query, &p.unit(unit).ast.member(member).metadata),
                Some(dartforge_elements::model::VariableRef::TopLevel { unit, decl, .. }) => has_deprecated(query, &p.unit(unit).ast.decl(decl).metadata),
                Some(dartforge_elements::model::VariableRef::EnumConstant { unit, decl, index }) => match &p.unit(unit).ast.decl(decl).kind {
                    ast::DeclKind::Enum(e) => e.constants.get(index).is_some_and(|k| has_deprecated(query, &k.metadata)),
                    _ => false,
                },
                _ => false,
            },
        }
    };
    if own(f) {
        return true;
    }
    // The overridden member (`inheritsDeprecated`): the one with the same name
    // in the supertypes.
    let fe = p.function(f);
    let Some(c) = fe.class else { return false };
    if fe.static_ || matches!(fe.kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor) {
        return false;
    }
    let mut stack: Vec<ClassId> = Vec::new();
    let cl = p.class(c);
    stack.extend(cl.supertype_class);
    stack.extend(cl.mixin_classes.iter().copied());
    stack.extend(cl.interface_classes.iter().copied());
    stack.extend(cl.on_classes.iter().copied());
    let mut seen = HashSet::new();
    while let Some(x) = stack.pop() {
        if !seen.insert(x) {
            continue;
        }
        let s = p.class(x);
        if let Some(&g) = s.instance_members.get(&fe.name)
            && own(g)
        {
            return true;
        }
        stack.extend(s.supertype_class);
        stack.extend(s.mixin_classes.iter().copied());
        stack.extend(s.interface_classes.iter().copied());
        stack.extend(s.on_classes.iter().copied());
    }
    false
}

/// Declared type of a member, without a receiver (statics and extensions).
fn declared_type(query: &Query, f: FunctionElementId) -> TypeId {
    let fe = query.program.function(f);
    let data = &query.outline.functions[f.0 as usize];
    match fe.kind {
        FunctionKind::ImplicitAccessor => fe
            .variable
            .and_then(|v| query.variable_type(v))
            .unwrap_or(data.return_type),
        FunctionKind::Getter => data.return_type,
        FunctionKind::Setter => data
            .parameters
            .first()
            .map_or(query.core.dynamic_, |p| p.ty),
        _ => data.signature,
    }
}

/// Library that declares a top-level element.
fn element_library(query: &Query, element: Element) -> LibraryId {
    let p = &query.program;
    match element {
        Element::Class(c) => p.class(c).library,
        Element::Extension(x) => p.extension(x).library,
        Element::Typedef(t) => p.typedef(t).library,
        Element::Function(f) => p.function(f).library,
        Element::Variable(v) => p.variable(v).library,
        Element::Prefix(l, _) => l,
    }
}

/// The return of a function type.
fn return_(query: &Query, ty: TypeId) -> Option<TypeId> {
    match query.table.get(ty) {
        Type::Function { ret, .. } => Some(*ret),
        _ => None,
    }
}

/// Edges from `sub` to `sup` in the hierarchy (superclass, mixins,
/// interfaces, `on`), like Dart's `_inheritanceDistance`.
fn inheritance_distance(query: &Query, sub: ClassId, sup: ClassId) -> Option<usize> {
    let mut queue = std::collections::VecDeque::from([(sub, 0usize)]);
    let mut seen = HashSet::new();
    while let Some((c, d)) = queue.pop_front() {
        if c == sup {
            return Some(d);
        }
        if !seen.insert(c) {
            continue;
        }
        let cl = query.program.class(c);
        for s in cl.supertype_class.iter().chain(&cl.mixin_classes).chain(&cl.interface_classes).chain(&cl.on_classes) {
            queue.push_back((*s, d + 1));
        }
    }
    None
}

/// The interface type of `c` with `Never` arguments, as Dart instantiates a
/// suggested class (`instantiateInstanceElement`).
fn class_type(query: &mut Query, c: ClassId) -> Option<TypeId> {
    let n = query.outline.classes.get(c.0 as usize)?.type_params.len();
    let never = query.core.never;
    let args: Box<[TypeId]> = std::iter::repeat_n(never, n).collect();
    Some(query.table.intern(Type::Interface { class: c, args, nullable: false }))
}

/// `contextTypeFeature`: equal 1.0; subtype 0.40; supertype 0.02; unrelated
/// 0.13.
fn context_feature(query: &mut Query, expected: TypeId, ty: TypeId) -> f64 {
    if expected == ty {
        return 1.0;
    }
    let Query { table, outline, core, .. } = query;
    let mut env = dartforge_types::SubtypeEnv::new(table, &outline.hierarchy, core);
    if dartforge_types::is_subtype(ty, expected, &mut env) {
        0.40
    } else if dartforge_types::is_subtype(expected, ty, &mut env) {
        0.02
    } else {
        0.13
    }
}

