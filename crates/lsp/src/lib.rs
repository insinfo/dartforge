//! DartForge's LSP server: documents, incremental synchronization and dispatch.
//!
//! The design copies rust-analyzer's (`references/rust-analyzer`), not its
//! code: a [`Server`] that owns everything (documents, queue,
//! cancellations), passed by `&mut` to the dispatcher; in-memory versioned
//! documents as in `mem_docs.rs`; a queue with cancellation as in
//! `op_queue.rs`; UTF-16 conversion as in `lsp/utils.rs` (here in
//! [`utf16`]).
//!
//! The server diagnoses in two stages: right away, through the new parser
//! and the local checkers of `crates/analysis` via the [`trait Analyzer`];
//! in the background, when the analyzer has an SDK
//! ([`Analyzer::sdk_for_diagnostics`]), through the same typed analysis as
//! `dartforge analyze` (`typed`), published only if the version is still
//! the current one (`docs/LSP.md`, "Diagnósticos tipados").

pub mod formatting;
pub mod server;
mod actions;
pub(crate) use dartforge_analysis::arvore_analyzer;
mod signature;
mod assist_conditionals;
mod assist_destructure;
mod assist_enum;
mod assist_surround;
mod assist_expressions;
mod assist_flutter;
mod assist_flutter_widget;
mod assist_functions;
mod assist_imports;
mod assist_loops;
mod assist_super;
mod assist_switch;
mod assist_switch_expression;
mod assist_members;
mod assist_strings;
mod assist_type;
mod assists;
mod assists2;
mod assists3;
mod fuzzy_matcher;
mod call_hierarchy;
mod inlay_hints;
mod write_type;
mod especies_g;
mod structure;
mod fix_all;
mod organize_imports;
mod sort_members;
mod hierarchy;
mod workspace_symbols;
mod referenced_names;
mod ignore_diagnostic;
mod completion;
mod known_libraries;
mod query;
mod outline;
mod fixes;
mod dart_fixes;
mod create;
mod dartdoc;
mod document_highlight;
mod description;
mod index;
mod links;
mod colors;
mod augmentations;
mod move_file;
mod insert;
mod completion_item;
mod navigation;
mod project;
mod semantic_tokens;
mod refactorings;
mod refactorings_inline;
mod refactorings_exec;
mod refactorings_method;
mod refactorings_move;
mod registration;
mod relevance;
mod relevance_tables;
mod rename;
mod closing_labels;
mod semantic;
mod session;
mod symbols;
mod snippets;
mod typed;
pub mod transport;
pub mod utf16;

use dartforge_diagnostics::Diagnostic;
use std::collections::HashMap;
use utf16::LineTable;

pub use server::Server;
pub use semantic::SemanticAnalyzer;
pub use completion::{Call, Completion, AutoImport, CompletionItem};
pub use rename::{Edit, RenameFile, Rename};

pub use actions::{CodeAction, LinkedGroup, Linked};
pub use refactorings::{RefactoringCommand, RefactoringRequest, Refactoring, RefactoringResult};
pub use signature::Signature;
pub use call_hierarchy::{CallKind, CallItem};
pub use inlay_hints::Hint;
pub use structure::Fold;
pub use session::SessionStats;


/// LSP position: line and column in **UTF-16 units** (both 0-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// 0-based line.
    pub line: u32,
    /// 0-based column in UTF-16 units.
    pub column: u32,
}

/// One entry of the `contentChanges` of `textDocument/didChange`.
///
/// A missing `range` means a full replacement (`full` sync, which the server
/// accepts but never asks for: the announced capability is incremental).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentChange {
    /// Range in UTF-16 positions in the current text, or `None` for everything.
    pub range: Option<(Position, Position)>,
    /// Replacement text.
    pub text: String,
}

/// Response of `textDocument/hover`: the range (bytes) of the reference under
/// the cursor, the element's description (`int sum(int a, int b)`), the
/// static type when it is a variable or getter, and the documentation
/// comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hover {
    /// Range of the name under the cursor, in bytes of the current text.
    pub range: dartforge_diagnostics::Span,
    /// Description of the element, as the Dart server writes it.
    pub description: String,
    /// Static type (variables and getters), already formatted.
    pub ty: Option<String>,
    /// Documentation (`///` or `/** */`), already without the markers.
    pub documentation: Option<String>,
    /// The library that declares the element (not local), as Dart shows it:
    /// `package:x/y.dart`, `dart:core`, or the path relative to the project root
    /// for a file outside `lib/`.
    pub library: Option<String>,
}

/// Response of `textDocument/references`: the declaration (when its location
/// is known) and the uses, each with the file's URI and the span in bytes of
/// the text the server sees (open, or the disk's for a closed file).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct References {
    /// Where the element is declared; it may be outside the project (SDK).
    pub declaration: Option<(String, dartforge_diagnostics::Span)>,
    /// Uses, in (URI, offset) order.
    pub uses: Vec<(String, dartforge_diagnostics::Span)>,
}

impl References {
    /// The syntactic form (declaration first, then the uses, all in the same
    /// document) as [`References`].
    fn from_list(uri: &str, spans: Vec<dartforge_diagnostics::Span>) -> Self {
        let mut spans = spans.into_iter().map(|s| (uri.to_string(), s));
        let declaration = spans.next();
        Self { declaration, uses: spans.collect() }
    }
}

/// Current text of a document open in the editor.
#[derive(Debug, Clone)]
struct OpenDocument {
    /// Most recent LSP version received; grows with each `didChange`.
    version: i32,
    /// Whole current text. It is the only state retained per document.
    text: String,
    /// Line starts of the current text, to convert UTF-16 without rescanning.
    lines: LineTable,
}

impl OpenDocument {
    /// Opens the document with the line table already built.
    fn new(version: i32, text: String) -> Self {
        let lines = LineTable::build(&text);
        Self {
            version,
            text,
            lines,
        }
    }

    /// Applies the changes in order and recomputes the table from the edited
    /// point on, once per change. Returns the first touched line, for whoever
    /// wants to measure the cost of the update.
    fn apply(&mut self, changes: &[ContentChange]) {
        for change in changes {
            match change.range {
                None => {
                    self.text = change.text.clone();
                    self.lines = LineTable::build(&self.text);
                }
                Some((start, end)) => {
                    let from =
                        self.lines
                            .position_offset(&self.text, start.line, start.column);
                    let until = self
                        .lines
                        .position_offset(&self.text, end.line, end.column);
                    let (from, until) = if from <= until { (from, until) } else { (until, from) };
                    self.text.replace_range(from..until, &change.text);
                    let line = self.lines.line_of(&self.text, from);
                    self.lines.recompute_from(&self.text, line);
                }
            }
        }
    }
}

/// Explicit owner of the open documents: the one that discards version N−1.
///
/// Each document has **one** entry (text + table + version), and the
/// arrival of version N replaces N−1 in place — the `drop` of the old
/// `String` is deterministic, not future eligibility for a collector.
/// Closing the document removes the entry, and nothing of the document
/// survives.
///
/// ```
/// let mut docs = dartforge_lsp::DocumentStore::new();
/// docs.open("file:///a.dart".into(), 1, "void main() {}".into());
/// docs.update("file:///a.dart", 2, "void main() { print(1); }".into());
/// assert_eq!(docs.version("file:///a.dart"), Some(2));
/// assert!(docs.close("file:///a.dart"));
/// assert!(docs.is_empty());
/// ```
#[derive(Debug, Default)]
pub struct DocumentStore {
    documents: HashMap<String, OpenDocument>,
}

impl DocumentStore {
    /// Creates an empty server, with no retained documents.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an open document; reopening replaces it, without retaining the
    /// previous one.
    pub fn open(&mut self, uri: String, version: i32, text: String) {
        self.documents
            .insert(uri, OpenDocument::new(version, text));
    }

    /// Applies version N and discards N−1 in place; false when it does not exist.
    ///
    /// A version lower than or equal to the current one is ignored and returns
    /// false: out-of-order changes cannot resurrect old text.
    pub fn update(&mut self, uri: &str, version: i32, text: String) -> bool {
        match self.documents.get_mut(uri) {
            Some(current) if version > current.version => {
                *current = OpenDocument::new(version, text);
                true
            }
            _ => false,
        }
    }

    /// Applies incremental changes (UTF-16 ranges) as version N.
    ///
    /// Same version rule as [`DocumentStore::update`]: a nonexistent document or
    /// an out-of-order version returns false and touches nothing.
    ///
    /// ```
    /// use dartforge_lsp::{DocumentStore, ContentChange, Position};
    /// let mut docs = DocumentStore::new();
    /// docs.open("file:///a.dart".into(), 1, "void main() {}".into());
    /// let swap = ContentChange {
    ///     range: Some((Position { line: 0, column: 13 }, Position { line: 0, column: 13 })),
    ///     text: " print(1);".into(),
    /// };
    /// assert!(docs.apply("file:///a.dart", 2, &[swap]));
    /// assert_eq!(docs.get("file:///a.dart"), Some("void main() { print(1);}"));
    /// ```
    pub fn apply(&mut self, uri: &str, version: i32, changes: &[ContentChange]) -> bool {
        match self.documents.get_mut(uri) {
            Some(current) if version > current.version => {
                current.apply(changes);
                current.version = version;
                true
            }
            _ => false,
        }
    }

    /// Closes the document and discards its text; false when it does not exist.
    pub fn close(&mut self, uri: &str) -> bool {
        self.documents.remove(uri).is_some()
    }

    /// Current text of an open document, if it exists.
    pub fn get(&self, uri: &str) -> Option<&str> {
        self.documents.get(uri).map(|open| open.text.as_str())
    }

    /// Current version of an open document, if it exists.
    pub fn version(&self, uri: &str) -> Option<i32> {
        self.documents.get(uri).map(|open| open.version)
    }

    /// Line table of the document, to convert spans without rescanning.
    pub(crate) fn lines(&self, uri: &str) -> Option<&LineTable> {
        self.documents.get(uri).map(|open| &open.lines)
    }

    /// Documents currently retained; each one costs exactly one text.
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    /// URIs of the open documents, without retaining a copy of the texts.
    pub(crate) fn uris(&self) -> impl Iterator<Item = &str> {
        self.documents.keys().map(String::as_str)
    }

    /// True when no document is open.
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// Diagnoses the current text through the new parser.
    ///
    /// An empty list when the document is closed; never retains the result,
    /// which is transient to the caller.
    pub fn diagnose_open(&self, uri: &str) -> Vec<Diagnostic> {
        match self.get(uri) {
            Some(text) => SyntacticAnalyzer::new().diagnose(uri, text),
            None => Vec::new(),
        }
    }
}

/// Analysis that produces the diagnostics published after each change.
///
/// The [`Server`] calls this trait, never the parser directly. The signature
/// takes `uri` and `text` (not the document) so that the implementation can
/// consult other files. Typed diagnostics do not go through
/// [`Analyzer::diagnose`] (which is synchronous): the server requests them
/// from the background worker with the SDK from
/// [`Analyzer::sdk_for_diagnostics`].
pub trait Analyzer {
    /// Diagnoses the current text and returns spans in UTF-8 bytes.
    ///
    /// The result is transient to the caller: nothing is retained between
    /// calls, so that N edits do not retain N analyses (the failure mode of
    /// Dart's LSP, measured in PLANO.md).
    fn diagnose(&mut self, uri: &str, text: &str) -> Vec<Diagnostic>;

    /// Syntactic symbols of the document, without keeping the tree between edits.
    fn symbols(&mut self, _uri: &str, _text: &str) -> Vec<serde_json::Value> {
        Vec::new()
    }

    /// Conservative definition of the position in the text: URI and selection
    /// at the target.
    fn definition(&mut self, _uri: &str, _text: &str, _offset: usize) -> Option<(String, Option<dartforge_diagnostics::Span>)> {
        None
    }

    /// Variant with the open buffers, to resolve imports not saved yet without
    /// duplicating documents in the resident analyzer.
    fn definition_in_workspace(&mut self, uri: &str, text: &str, offset: usize, _documents: &DocumentStore) -> Option<(String, Option<dartforge_diagnostics::Span>)> {
        self.definition(uri, text, offset)
    }

    /// Description, type and documentation of what is under the cursor.
    fn hover(&mut self, _uri: &str, _text: &str, _offset: usize) -> Option<Hover> {
        None
    }

    /// Variant with the open buffers (texts not saved yet).
    fn hover_in_workspace(&mut self, uri: &str, text: &str, offset: usize, _documents: &DocumentStore) -> Option<Hover> {
        self.hover(uri, text, offset)
    }

    /// Completion items at position `offset` (bytes) of the open document
    /// `uri`. `None` when the analysis cannot answer (no SDK, for example); an
    /// empty list when there is nothing to offer.
    ///
    /// ```
    /// use dartforge_lsp::{Analyzer, SemanticAnalyzer, SyntacticAnalyzer, DocumentStore};
    /// let mut docs = DocumentStore::new();
    /// docs.open("file:///a.dart".into(), 1, "void f(int x) { x. }".into());
    /// // Without types there is nothing to complete; the semantic one without an SDK does not know either.
    /// assert!(SyntacticAnalyzer::new().completion(&docs, "file:///a.dart", 18).is_none());
    /// assert!(SemanticAnalyzer::new(None).completion(&docs, "file:///a.dart", 18).is_none());
    /// ```
    fn completion(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize) -> Option<Completion> {
        None
    }

    /// The client configuration's `maxCompletionItems` (the `maxSuggestions` of
    /// the collector and of the truncation).
    fn set_completion_max(&mut self, _max: usize) {}

    /// Whether completion suggests declarations from not-imported libraries
    /// (the `suggestFromUnimportedLibraries && capabilities.applyEdit` of
    /// `handler_completion.dart:367`).
    fn set_not_imported_in_completion(&mut self, _yes: bool) {}

    /// `prepareRename`: range (bytes) and text of the renameable name under the
    /// cursor; `Ok(None)` when there is no name there.
    ///
    /// # Errors
    ///
    /// A message for the user when the element exists but cannot be renamed
    /// (SDK, external package, shape not supported yet).
    fn prepare_rename(
        &mut self,
        _documents: &DocumentStore,
        _uri: &str,
        _offset: usize,
    ) -> Result<Option<(dartforge_diagnostics::Span, String)>, String> {
        Ok(None)
    }

    /// `rename`: the edits in all files of the project.
    ///
    /// ```
    /// use dartforge_lsp::{Analyzer, SyntacticAnalyzer, DocumentStore};
    /// let mut docs = DocumentStore::new();
    /// docs.open("file:///a.dart".into(), 1, "var x = 1;".into());
    /// let mut a = SyntacticAnalyzer::new();
    /// assert_eq!(a.prepare_rename(&docs, "file:///a.dart", 4), Ok(None));
    /// assert!(a.rename(&docs, "file:///a.dart", 4, "y").is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Invalid name, element that cannot be renamed, or a conflict, with the
    /// message for the user.
    fn rename(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize, _new: &str) -> Result<Rename, String> {
        Err("Rename requires the semantic analysis (Dart SDK).".into())
    }

    /// `codeAction`: fixes and assists for the range `start..end` (bytes) of the
    /// open document. `published` are the typed diagnostics already published
    /// for the current version (the server does not pass those of an old
    /// version). The default offers the fixes of the syntactic diagnostics
    /// (insert `;`).
    ///
    /// ```
    /// use dartforge_lsp::{Analyzer, SyntacticAnalyzer, DocumentStore};
    /// let mut docs = DocumentStore::new();
    /// docs.open("file:///a.dart".into(), 1, "void f() { var x = 1 }".into());
    /// let actions = SyntacticAnalyzer::new().actions(&docs, "file:///a.dart", 19, 19, &[]);
    /// assert_eq!(actions[0].title, "Insert ';'");
    /// assert_eq!(actions[0].edits[0].text, ";");
    /// ```
    fn actions(&mut self, documents: &DocumentStore, uri: &str, start: usize, end: usize, _published: &[Diagnostic]) -> Vec<CodeAction> {
        let Some(text) = documents.get(uri) else { return Vec::new() };
        let diagnostics = self.diagnose(uri, text);
        let mut output = actions::fix_syntax(uri, text, &diagnostics, start, end);
        output.push(actions::organize_imports(uri, text));
        output
    }

    /// The refactorings listed in `offset..offset + length`
    /// (docs/LSP-ESPECIFICACAO.md §13.11.1); `create_files` is the client's
    /// `supportsFileCreation`.
    fn refactorings(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize, _length: usize, _create_files: bool) -> Vec<Refactoring> {
        Vec::new()
    }

    /// `refactor.perform`/`refactor.validate` of a legacy refactoring (§13.11.2).
    fn run_refactoring(&mut self, _documents: &DocumentStore, _uri: &str, _request: &RefactoringRequest) -> RefactoringResult {
        RefactoringResult::NotAnalyzed
    }

    /// `dart.refactor.move_top_level_to_file` to file `target` (§13.11.9 e).
    fn move_to_file(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize, _length: usize, _target: &str) -> RefactoringResult {
        RefactoringResult::NotAnalyzed
    }

    /// Discards state associated with the document when it leaves the editor.
    fn closed_document(&mut self, _uri: &str) {}

    /// The text of `uri` changed (an accepted `didOpen` or `didChange`): what the
    /// analyzer retains from previous queries stops being valid and drops here.
    fn changed_document(&mut self, _uri: &str) {}

    /// `didOpen`: `uri` is now open in `documents`. The default is
    /// [`Analyzer::changed_document`]; whoever retains state may keep it when
    /// the open text is what it already read from disk.
    fn open_document(&mut self, _documents: &DocumentStore, uri: &str) {
        self.changed_document(uri);
    }

    /// `didClose`: `uri` has already left `documents`. The default is
    /// [`Analyzer::closed_document`]; whoever retains state may keep it when the
    /// disk text is what it already had.
    fn document_closed_at(&mut self, _documents: &DocumentStore, uri: &str) {
        self.closed_document(uri);
    }

    /// Accepted `didChange`: the text of `uri` in `documents` is the new one.
    /// The default is [`Analyzer::changed_document`]; whoever retains state may
    /// swap it in place when only bodies changed.
    fn edited_document(&mut self, _documents: &DocumentStore, uri: &str) {
        self.changed_document(uri);
    }

    /// The SDK `lib/` with which the server runs, in the background, the
    /// `dartforge analyze` typed analysis over the open documents
    /// (`docs/LSP.md`, "Diagnósticos tipados"). `None` (the default): only the
    /// diagnostics of [`Analyzer::diagnose`].
    fn sdk_for_diagnostics(&self) -> Option<std::path::PathBuf> {
        None
    }

    /// Conservative references in the document itself: the declaration first,
    /// then the uses, all as spans in UTF-8 bytes.
    ///
    /// Only answers in the same safe cases as [`Analyzer::definition`] (a single
    /// top-level type, variable, function or getter, without imports or
    /// shadows): `None` means "I don't know", never "there are none". The
    /// result is transient to the caller, as in the other methods.
    fn references(&mut self, _uri: &str, _text: &str, _offset: usize) -> Option<Vec<dartforge_diagnostics::Span>> {
        None
    }

    /// References in the workspace: the declaration and the uses, each with the
    /// file's URI. The default delegates to the document itself. Nothing is
    /// retained between requests beyond what the analyzer's session policy
    /// allows.
    fn references_in(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<References> {
        let text = documents.get(uri)?;
        let spans = self.references(uri, text, offset)?;
        Some(References::from_list(uri, spans))
    }

    /// Signature help in the argument list containing `offset`
    /// (`textDocument/signatureHelp`). `auto`: the editor asked on typing `(`;
    /// it only answers if that `(` opens the list. The default: none.
    fn signature(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize, _auto: bool) -> Option<Signature> {
        None
    }

    /// The occurrences, in the document itself, of what `offset` denotes
    /// (`textDocument/documentHighlight`). The default uses the document's
    /// conservative references.
    fn document_highlights(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<Vec<dartforge_diagnostics::Span>> {
        let text = documents.get(uri)?.to_string();
        self.references(uri, &text, offset)
    }

    /// Implementations (`textDocument/implementation`): subtypes of a class or
    /// the overrides of a member, with each one's URI.
    fn implementations(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize) -> Vec<(String, dartforge_diagnostics::Span)> {
        Vec::new()
    }

    /// The colors of `uri` (`textDocument/documentColor`): the range and the
    /// ARGB from 0 to 255.
    fn colors(&mut self, _documents: &DocumentStore, _uri: &str) -> Vec<(dartforge_diagnostics::Span, [u8; 4])> {
        Vec::new()
    }

    /// `textDocument/colorPresentation`: Flutter's `Color` exists and the
    /// expression at `offset` requires `const` (`None`: without the `Color`).
    fn color_presentation(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize) -> Option<bool> {
        None
    }

    /// The augmentation lenses of `uri` (`AugmentationCodeLensProvider`): the
    /// name, the title and the target (URI and span).
    fn augmentation_lenses(&mut self, _documents: &DocumentStore, _uri: &str, _augmented: bool, _augmentations: bool) -> Vec<(dartforge_diagnostics::Span, &'static str, String, dartforge_diagnostics::Span)> {
        Vec::new()
    }

    /// `dart/textDocument/augmented` (`next` false) and
    /// `dart/textDocument/augmentation`: the neighbor in the augmentation chain.
    fn augmentation_neighbor(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize, _next: bool) -> Option<(String, dartforge_diagnostics::Span)> {
        None
    }

    /// `dart/textDocument/super`: the constructor, class or member that what
    /// `offset` denotes overrides or extends.
    fn upper(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize) -> Option<(String, dartforge_diagnostics::Span)> {
        None
    }

    /// The declaration of the static type of what `offset` denotes
    /// (`textDocument/typeDefinition`).
    fn type_definition(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize) -> Option<(String, dartforge_diagnostics::Span)> {
        None
    }

    /// Folding regions (`textDocument/foldingRange`); `lines_only`: the client
    /// announced `lineFoldingOnly`.
    fn folds(&mut self, _uri: &str, _text: &str, _only_lines: bool) -> Vec<Fold> {
        Vec::new()
    }

    /// Selection ranges at `offset`, from the innermost to the outermost
    /// (`textDocument/selectionRange`).
    fn selections(&mut self, _uri: &str, _text: &str, _offset: usize) -> Vec<dartforge_diagnostics::Span> {
        Vec::new()
    }

    /// The document's closing labels (`dart/textDocument/publishClosingLabels`):
    /// the node's range (bytes) and the text. The default has none.
    fn closing_labels(&mut self, _documents: &DocumentStore, _uri: &str) -> Vec<(dartforge_diagnostics::Span, String)> {
        Vec::new()
    }

    /// Inlay hints of the whole document (`textDocument/inlayHint`).
    fn hints(&mut self, _documents: &DocumentStore, _uri: &str) -> Vec<Hint> {
        Vec::new()
    }

    /// Semantic tokens of the document already encoded for the protocol (groups
    /// of 5 numbers); `multiline`: the client accepts multi-line tokens; `range`
    /// (bytes): only those that touch it.
    fn semantic_tokens(&mut self, _documents: &DocumentStore, _uri: &str, _multiline: bool, _range: Option<(usize, usize)>) -> Option<Vec<u32>> {
        None
    }

    /// The executable at `offset` as a call hierarchy item
    /// (`textDocument/prepareCallHierarchy`).
    fn prepare_calls(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize) -> Option<CallItem> {
        None
    }

    /// Incoming (`true`) or outgoing calls of the client's item: the name at
    /// `offset` of `uri`, the displayed name (`_isMatchingElement`) and whether
    /// the kind is constructor (the class's implicit unnamed constructor); the
    /// ranges of each one (in the caller's file, for incoming calls).
    fn calls(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize, _name: &str, _constructor: bool, _received: bool) -> Vec<(CallItem, Vec<dartforge_diagnostics::Span>)> {
        Vec::new()
    }

    /// The type hierarchy item at `offset` (`textDocument/prepareTypeHierarchy`).
    fn prepare_hierarchy(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize) -> Option<TypeItem> {
        None
    }

    /// Supertypes (`true`) or direct subtypes of the class whose
    /// `ElementLocation` is `reference` (the item's `data.ref`), in the project
    /// of `uri`; `anchor` is the `data.anchor` (the reference and the path).
    /// `None` when the reference does not locate a class (`null` response).
    fn hierarchy(&mut self, _documents: &DocumentStore, _uri: &str, _reference: &str, _anchor: Option<(&str, &[usize])>, _supertypes: bool) -> Option<Vec<TypeItem>> {
        None
    }
}

/// A type hierarchy item: the displayed name (`Base<int>`), the file, the
/// whole declaration (with the documentation) and the name, the class's
/// `ElementLocation` and the anchor (reference and path) of the supertypes
/// with type arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeItem {
    pub name: String,
    pub uri: String,
    pub range: dartforge_diagnostics::Span,
    pub selection: dartforge_diagnostics::Span,
    pub reference: String,
    pub anchor: Option<(String, Vec<usize>)>,
}

/// Syntactic analysis: the new parser, without resolution (names and types
/// come later).
///
/// Each call interns into a new [`dartforge_intern::Interner`] and discards
/// it together with the tree: only the diagnostics (message + span) cross
/// the call. Accepts 100% of the SDK, the pub corpus and `new_sali`; valid
/// files produce no diagnostics.
///
/// ```
/// use dartforge_lsp::{Analyzer, SyntacticAnalyzer};
/// let mut a = SyntacticAnalyzer::new();
/// assert!(a.diagnose("file:///a.dart", "void main() {}").is_empty());
/// // The missing identifier and, as in `dart analyze`, the unused local.
/// assert_eq!(a.diagnose("file:///b.dart", "void a() { int x = ; }").len(), 2);
/// ```
#[derive(Debug, Default)]
pub struct SyntacticAnalyzer {
    /// `package_config.json` files already read, by path (one per open
    /// project): each file's default language version comes from it.
    configs: std::collections::HashMap<std::path::PathBuf, dartforge_elements::config::PackageConfig>,
    /// Default version (the package's) of each document already seen.
    patterns: std::collections::HashMap<String, dartforge_frontend::LanguageVersion>,
}

impl SyntacticAnalyzer {
    /// Creates the syntactic analyzer.
    pub fn new() -> Self {
        Self::default()
    }

    /// The language features of file `uri` (`docs/VERSOES-LINGUAGEM.md` §2): the
    /// `// @dart = x.y` marker, else the package's `languageVersion` in the
    /// `package_config.json` that contains it, else the current version. Without
    /// this, a 3.6 project would see an error on a `final` parameter (forbidden
    /// in 3.13) and a 3.13 one would see an error on a primary constructor.
    pub(crate) fn features(&mut self, uri: &str, text: &str) -> dartforge_frontend::LibraryFeatures {
        use dartforge_frontend::{LanguageVersion, LibraryFeatures};
        if let Some((v, _)) = dartforge_frontend::features::marcador_versao(text) {
            return LibraryFeatures::new(v, &[]);
        }
        if let Some(v) = self.patterns.get(uri) {
            return LibraryFeatures::new(*v, &[]);
        }
        // Once per document: the canonical path (what the `package_config` stores)
        // and the package that contains it.
        let path = url::Url::parse(uri)
            .ok()
            .filter(|u| u.scheme() == "file")
            .and_then(|u| u.to_file_path().ok())
            .map(|p| dartforge_elements::config::sem_verbatim(std::fs::canonicalize(&p).unwrap_or(p)));
        let pattern = path
            .as_deref()
            .and_then(|p| {
                let cfg = dartforge_elements::config::PackageConfig::discover(p)?;
                if !self.configs.contains_key(&cfg) {
                    let read = dartforge_elements::config::PackageConfig::load(&cfg).ok()?;
                    self.configs.insert(cfg.clone(), read);
                }
                self.configs.get(&cfg)?.pacote_da_biblioteca(uri, Some(p))?.language_version
            })
            .unwrap_or(LanguageVersion::ATUAL);
        self.patterns.insert(uri.to_string(), pattern);
        LibraryFeatures::new(pattern, &[])
    }

    /// Only the parser's diagnostics (always published), in the file's language
    /// version.
    pub(crate) fn syntax(&mut self, uri: &str, text: &str) -> Vec<Diagnostic> {
        let features = self.features(uri, text);
        let mut names = dartforge_intern::Interner::new();
        let parsed = dartforge_frontend::parser::parse_com(text, &mut names, features);
        // Name and text of the analyzer that is the file's reference (T2).
        let reference = parsed.referencia;
        parsed.diagnostics.into_iter().filter_map(|d| d.for_reference(reference)).collect()
    }
}

impl Analyzer for SyntacticAnalyzer {
    /// Analyzes with the full parser, in the file's language version, with
    /// per-declaration recovery.
    ///
    /// After the syntax, the `crates/analysis` checkers that do not depend on
    /// types (duplicate names, unused locals) over the file itself; of these,
    /// only what the publication rule lets through comes out
    /// (`dartforge_analysis::publicacao`): codes verified against the oracle,
    /// and not suppressed by `// ignore:` (the `dartforge analyze` filter).
    fn diagnose(&mut self, uri: &str, text: &str) -> Vec<Diagnostic> {
        let features = self.features(uri, text);
        let mut names = dartforge_intern::Interner::new();
        let parsed = dartforge_frontend::parser::parse_com(text, &mut names, features);
        let mut output = parsed.diagnostics;
        let unit = dartforge_analysis::Unidade { ast: &parsed.ast, unit: &parsed.unit, fonte: text };
        // The unused locals by the skipped-spans rule (T5), as in `dartforge
        // analyze`: no gate on syntax errors.
        let skipped = dartforge_analysis::Pulados { fonte: text, trechos: &parsed.pulados };
        let wildcard = features.tem(dartforge_frontend::features::Feature::WildcardVariables);
        let reference = parsed.referencia;
        let join = reference == dartforge_diagnostics::AnalyzerReference::V3_6;
        let semantic = dartforge_analysis::duplicatas::duplicatas(&[unit], &names, wildcard, join)
            .into_iter()
            .map(|(_, d)| d)
            .chain(dartforge_analysis::enums::sem_constantes(&[unit]).into_iter().map(|(_, d)| d))
            .chain(dartforge_analysis::inicializacao::finais_nao_inicializados(&[unit], &names).into_iter().map(|(_, d)| d))
            .chain(dartforge_analysis::locais::nao_usados_com_pulados(unit, &names, wildcard, skipped))
            .chain(dartforge_analysis::externos::inicializadores(unit))
            .chain(dartforge_analysis::operadores::aridade(unit, &names));
        // `// ignore:` as in `dartforge analyze` (the same filter).
        let ignored = dartforge_parity::filtros::Ignorados::de_texto(text);
        let lines = dartforge_parity::json::Linhas::new(text);
        output.extend(
            semantic
                .filter(|d| dartforge_analysis::publicacao::publicado(d, false))
                .filter(|d| !ignored.ignora(d, lines.ponto(d.span.start).line)),
        );
        // Last, the name and text of the reference analyzer (T2): the filters above
        // know the codes by their 3.6 names.
        output.into_iter().filter_map(|d| d.for_reference(reference)).collect()
    }

    fn symbols(&mut self, uri: &str, text: &str) -> Vec<serde_json::Value> {
        let features = self.features(uri, text);
        symbols::for_document(text, features)
    }

    fn definition(&mut self, uri: &str, text: &str, offset: usize) -> Option<(String, Option<dartforge_diagnostics::Span>)> {
        let features = self.features(uri, text);
        match navigation::target(uri, text, features, offset)? {
            navigation::Target::File(target) => Some((target, None)),
            navigation::Target::LocalName(ty) => Some((uri.to_string(), Some(ty.declaration))),
        }
    }

    fn hover(&mut self, uri: &str, text: &str, offset: usize) -> Option<Hover> {
        let features = self.features(uri, text);
        match navigation::target(uri, text, features, offset)? {
            navigation::Target::File(_) => None,
            navigation::Target::LocalName(ty) => Some(Hover {
                range: ty.reference,
                description: ty.description?,
                ty: ty.static_type,
                documentation: None,
                library: None,
            }),
        }
    }

    fn closed_document(&mut self, uri: &str) {
        self.patterns.remove(uri);
        // The configuration may have changed while the file was closed. The next
        // analysis loads it again; nothing of the closed project stays retained
        // indefinitely in the server session.
        self.configs.clear();
    }

    fn references(&mut self, uri: &str, text: &str, offset: usize) -> Option<Vec<dartforge_diagnostics::Span>> {
        let features = self.features(uri, text);
        navigation::references(uri, text, features, offset)
    }

    fn definition_in_workspace(&mut self, uri: &str, _text: &str, offset: usize, documents: &DocumentStore) -> Option<(String, Option<dartforge_diagnostics::Span>)> {
        navigation::definition_in(documents, uri, offset, self)
    }

    fn hover_in_workspace(&mut self, uri: &str, _text: &str, offset: usize, documents: &DocumentStore) -> Option<Hover> {
        let (range, description, ty) = navigation::hover_in(documents, uri, offset, self)?;
        Some(Hover { range, description, ty, documentation: None, library: None })
    }

    fn folds(&mut self, uri: &str, text: &str, lines_only: bool) -> Vec<Fold> {
        let features = self.features(uri, text);
        structure::folds(text, features, lines_only)
    }

    fn selections(&mut self, uri: &str, text: &str, offset: usize) -> Vec<dartforge_diagnostics::Span> {
        let features = self.features(uri, text);
        structure::selections(text, features, offset)
    }

    fn references_in(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<References> {
        let mut found = navigation::references_in(documents, uri, offset, self)?.into_iter();
        let declaration = found.next();
        Some(References { declaration, uses: found.collect() })
    }
}
