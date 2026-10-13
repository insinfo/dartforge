//! Protocol dispatch: a [`Server`] that owns everything, passed by `&mut`.
//!
//! The design copies rust-analyzer's (`main_loop.rs`, `op_queue.rs`,
//! `global_state.rs`, `mem_docs.rs`): in-memory documents are versioned and
//! have an explicit owner ([`crate::DocumentStore`]); messages arriving from
//! `stdin` wait in a queue; `$/cancelRequest` cancels a request still in the
//! queue with `RequestCancelled`, and the one being executed checks for
//! cancellation at safe points.
//!
//! The queue lives **inside** the server (not in an external channel), so
//! that cancellation is deterministic: each [`Server::pump`] first extracts
//! and applies *all* the `$/cancelRequest`s of the queue — whether they come
//! before or after the target request — and only then executes a request.
//! Without that, cancellation would depend on the reader thread being faster
//! than the dispatcher, which is a race condition, not protocol.
//!
//! Nothing here grows with the number of messages: the cancellation set only
//! keeps ids of requests still in the queue (cancelling an unknown id is
//! discarded), and is cleared on responding; no version history, no
//! in-memory log. Per document, the only state is that of the
//! [`crate::DocumentStore`].
//!
//! ```
//! use dartforge_lsp::Server;
//! use serde_json::json;
//! let mut server = Server::new();
//! server.receive(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}));
//! let outputs = server.pump();
//! assert_eq!(outputs.len(), 1);
//! assert!(outputs[0]["result"]["capabilities"].is_object());
//! ```

use crate::{Analyzer, SyntacticAnalyzer, DocumentStore, ContentChange, Position};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};

/// Announced synchronization capability: incremental (1 = full, 2 = incremental).
const SYNC_INCREMENTAL: u32 = 2;
/// LSP `severity` for every syntactic diagnostic.
const SEVERITY_ERROR: u32 = 1;

/// LSP `DiagnosticSeverity` for the analyzer's severity (1 error, 2 warning, 3 information).
fn severity(s: dartforge_diagnostics::Severity) -> u32 {
    match s {
        dartforge_diagnostics::Severity::Error => SEVERITY_ERROR,
        dartforge_diagnostics::Severity::Warning => 2,
        dartforge_diagnostics::Severity::Info => 3,
    }
}

/// The message as the Dart server shows it: the problem and, if any, the correction.
fn message(d: &dartforge_diagnostics::Diagnostic) -> String {
    match d.correction() {
        Some(c) => format!("{}\n{c}", d.message),
        None => d.message.clone(),
    }
}
/// Source published on every diagnostic.
const SOURCE: &str = "dartforge";

/// JSON-RPC/LSP error codes used in the responses.
const REQUEST_CANCELLED: i32 = -32800;
const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_REQUEST: i32 = -32600;
const SERVER_NOT_INITIALIZED: i32 = -32002;
/// The Dart server errors used by the commands
/// (`AS:src/lsp/constants.dart:273-312`).
const ERROR_NOT_HANDLED: i32 = -32001;
const UNKNOWN_COMMAND: i32 = -32005;
const INVALID_COMMAND_ARGUMENTS: i32 = -32006;
const FILE_NOT_ANALYZED: i32 = -32007;
const FILE_WITH_ERRORS: i32 = -32008;
const CLIENT_NOT_APPLIED: i32 = -32009;
const FEATURE_DISABLED: i32 = -32012;
/// The `workspace/executeCommand` commands, in Dart's order
/// (`constants.dart:99-111` and `refactoring_processor.dart:20-28`).
const COMMANDS: &[&str] = &[
    "dart.edit.sortMembers",
    "dart.edit.organizeImports",
    "dart.edit.fixAll",
    "dart.edit.fixAllInWorkspace.preview",
    "dart.edit.fixAllInWorkspace",
    "dart.edit.sendWorkspaceEdit",
    "refactor.perform",
    "refactor.validate",
    "dart.logAction",
    "dart.refactor.convert_all_formal_parameters_to_named",
    "dart.refactor.convert_selected_formal_parameters_to_named",
    "dart.refactor.move_selected_formal_parameters_left",
    "dart.refactor.move_top_level_to_file",
];
/// The `RefactoringKind`s the protocol constructor accepts
/// (`protocol_common.dart:3702-3724`).
const REFACTORING_TYPES: &[&str] = &[
    "CONVERT_GETTER_TO_METHOD",
    "CONVERT_METHOD_TO_GETTER",
    "EXTRACT_LOCAL_VARIABLE",
    "EXTRACT_METHOD",
    "EXTRACT_WIDGET",
    "INLINE_LOCAL_VARIABLE",
    "INLINE_METHOD",
    "MOVE_FILE",
    "RENAME",
];
/// `RefactoringComputeStatusFailure`.
const COMPUTATION_FAILURE: i32 = -32014;
/// `ContentModified`.
const MODIFIED_CONTENT: i32 = -32801;
/// The `codeAction` source actions, in Dart's order: title, kind and command.
const SOURCE_ACTIONS: &[(&str, &str, &str)] = &[
    ("Sort Members", "source.sortMembers", "dart.edit.sortMembers"),
    ("Organize Imports", "source.organizeImports", "dart.edit.organizeImports"),
    ("Fix All", "source.fixAll", "dart.edit.fixAll"),
];
/// Rename refused (invalid name, external element, conflict): the same code
/// as the Dart server (`ServerErrorCodes.RenameNotValid`).
const RENAME_INVALID: i32 = -32010;

/// Ceiling of the cancellation set: a defense against a client that cancels
/// nonexistent ids in bulk. Above the ceiling, the set is emptied — the
/// worst case is a cancellable request running anyway, never retention.
const MAX_CANCELLATIONS: usize = 4096;

/// Test hook: `dartforge/sleep` with `{"ms": N}` sleeps in 10 ms slices
/// checking for cancellation, so the acceptance test can exercise both forms
/// of cancellation (queued and running) without depending on a slow
/// analysis. Real clients never send this method.
const SLEEP_METHOD: &str = "dartforge/dormir";

/// The text of a file on disk and its line table.
type TextWithLines = std::sync::Arc<(String, crate::utf16::LineTable)>;

/// Global server state: owner of the documents, the queue and the cancellations.
///
/// Parameterized by the [`Analyzer`] so that semantics come in without
/// touching the transport. No `Rc<RefCell<…>>`: the dispatcher receives
/// `&mut self`.
#[derive(Debug)]
pub struct Server<A = SyntacticAnalyzer> {
    documents: DocumentStore,
    /// Text and line table of the disk files (not open) that the current request
    /// converted to LSP positions: a search that returns thousands of locations
    /// in the SDK reads each file once. Emptied at each request (the disk may
    /// change between them).
    disk_lines: std::cell::RefCell<HashMap<String, Option<TextWithLines>>>,
    analyzer: A,
    /// Messages received and not dispatched yet, in arrival order.
    queue: VecDeque<Value>,
    /// Cancelled ids of requests not answered yet, in canonical form.
    cancelled: HashSet<String>,
    /// `shutdown` has already been answered; `exit` exits with 0, without it with 1.
    disabling: bool,
    /// `initialize` has already been answered: a second one is refused (§2.3).
    initialized: bool,
    /// `exit` arrived (or the stream ended): the main loop must end.
    shutdown: bool,
    /// Corresponding exit code.
    code: i32,
    /// The client accepts the `DocumentSymbol` tree (LSP 3.10+).
    hierarchical_symbols: bool,
    hover_markdown: bool,
    /// The client accepts `prepareRename` (`rename.prepareSupport`).
    prepare_rename: bool,
    /// The client accepts `WorkspaceEdit.documentChanges` (edits with the
    /// document version): edits go out in that form.
    versioned_changes: bool,
    /// The client announced `experimental.snippetTextEdit`: code actions with a
    /// selection or linked groups go out as `SnippetTextEdit`.
    edits_snippet: bool,
    /// The client accepts the `rename` resource operation and asked for
    /// `renameFilesWithClasses: "always"` in the initialization options.
    rename_files: bool,
    /// The client accepts `documentChanges` with the `create` operation: the
    /// fixes that create a file (`Create file`) are offered.
    create_files: bool,
    /// The client accepts snippets in completion and did not ask for
    /// `completeFunctionCalls: false`: calls go out with the parentheses and
    /// the required parameters as placeholders.
    completion_calls: bool,
    /// `suggestFromUnimportedLibraries` (default `true`) and the client accepts
    /// `workspace/applyEdit`: completion brings the not-imported items.
    completion_not_imported: bool,
    /// The client accepts Markdown in the items' documentation.
    documentation_markdown: bool,
    /// The client's completion capabilities (§14.8.2).
    completion_capabilities: crate::completion_item::Capabilities,
    /// The not-imported items of the last response, by the `data` key (`ref`):
    /// the origin (for the documentation) and the import (for resolve).
    not_imported: HashMap<String, (Option<(std::path::PathBuf, usize)>, Option<crate::completion::AutoImport>, Option<String>)>,
    /// Workspace roots announced in `initialize` (`rootUri`, `workspaceFolders`),
    /// for `workspace/symbol` to scan the disk.
    roots: Vec<std::path::PathBuf>,
    /// `_filesWithClientDiagnostics`: the URIs whose diagnostics in the client
    /// are not empty.
    with_client_diagnostics: std::collections::HashSet<String>,
    /// The document of the last answered `prepareTypeHierarchy`: the project of
    /// the supertypes and subtypes of items outside the workspace.
    hierarchy_origin: Option<String>,
    /// Worker of the typed diagnostics (`crate::typed`), started with the first
    /// open document when the analyzer has an SDK.
    typed: Option<crate::typed::Typed>,
    /// The worker has already been tried (started or refused).
    typed_tried: bool,
    /// Called by the worker when there is a result (wakes up the `main` loop).
    wake: Option<crate::typed::Wake>,
    /// Typed results discarded for an old version or a closed document.
    typed_discarded: usize,
    /// The last typed publication of each open document, with the version: code
    /// actions fix what the editor shows. One entry per document, replaced at
    /// each publication and removed on `didClose`.
    typed_published: HashMap<String, (i32, Vec<dartforge_diagnostics::Diagnostic>)>,
    /// The client pulls diagnostics (`textDocument/diagnostic`, LSP 3.17: it
    /// announced `textDocument.diagnostic`): the server announces the
    /// `diagnosticProvider` and stops pushing `publishDiagnostics`.
    pulled_diagnostics: bool,
    /// With pulled diagnostics, the client accepts
    /// `workspace/diagnostic/refresh`: requested when a typed result arrives, so
    /// that it pulls again.
    update_pulled: bool,
    /// Number of the server's next request to the client (the `refresh` id).
    next_request: u64,
    /// The client accepts `workspace/applyEdit` (`workspace.applyEdit`): without
    /// it there are no source actions (docs/LSP-ESPECIFICACAO.md §13.12.1).
    apply_edits: bool,
    /// The `closingLabels` initialization option: the server sends
    /// `dart/textDocument/publishClosingLabels` for the open documents.
    closing_labels: bool,
    /// The client accepts work done progress (`window.workDoneProgress`): the
    /// analysis state goes out through `$/progress` with the `ANALYZING` token;
    /// without it, through `$/analyzerStatus` (§2.6).
    work_progress: bool,
    /// The typed analysis is running and the client has already been notified.
    analyzing: bool,
    /// Messages to the client produced outside `pump` (the start of the
    /// analysis), delivered on the next pass.
    pending_outputs: Vec<Value>,
    /// The `outline` initialization option: the server sends
    /// `dart/textDocument/publishOutline` for the open documents.
    outline: bool,
    /// The client accepts change annotations
    /// (`workspace.workspaceEdit.changeAnnotationSupport`).
    change_annotations: bool,
    /// The client accepts literal `CodeAction`s
    /// (`textDocument.codeAction.codeActionLiteralSupport`); without it the
    /// source actions go out as plain `Command`s.
    actions_literals: bool,
    /// The client's `codeActionKind.valueSet`, when it announces it.
    action_kinds: Option<Vec<String>>,
    /// The `workspace/applyEdit` requests sent and not answered by the client
    /// yet: by request id, the id of the waiting `executeCommand`, the command's
    /// label and the sent `WorkspaceEdit`.
    pending_edits: HashMap<String, (Value, &'static str, Value)>,
    /// The client accepts Markdown in the signature help documentation.
    signature_markdown: bool,
    /// The client accepts `activeParameter: null` (`noActiveParameterSupport`).
    signature_without_active: bool,
    /// The client only folds whole lines (`foldingRange.lineFoldingOnly`).
    folds_line_only: bool,
    /// The client accepts multi-line semantic tokens.
    multiline_tokens: bool,
    /// `SymbolKind`s the client announces (`documentSymbol.symbolKind`); without
    /// the list, only those of the first protocol version (1 to 18), and
    /// `EnumMember` becomes `Enum` as in the Dart server.
    symbol_kinds: Option<Vec<u64>>,
    /// The client's `workspace.symbol.symbolKind.valueSet`.
    workspace_symbol_kinds: Option<Vec<u64>>,
    /// The syntactic index of the libraries for `workspace/symbol`.
    symbol_index: crate::workspace_symbols::Index,
    /// The client accepts `labelDetails` on completion items: the label is only
    /// the name, and the short signature and the library to import go in the
    /// details (as the Dart server does with such a client).
    label_details: bool,
    /// The client answers `workspace/configuration` (§2.5).
    requestable_configuration: bool,
    /// The features the client registers dynamically (§2.4) and whether it wants
    /// `workspace/didChangeConfiguration` registered.
    dynamic: Vec<&'static str>,
    dynamic_configuration: bool,
    /// The client registers `workspace/didChangeWatchedFiles` dynamically.
    observe_files: bool,
    /// The client registers the file operations dynamically
    /// (`workspace.fileOperations.dynamicRegistration`).
    dynamic_file_operations: bool,
    /// The full static capabilities (before removing the dynamic ones): the
    /// source of each registration's options.
    static_capabilities: Value,
    /// The current dynamic registrations and the id counter.
    records: crate::registration::Records,
    /// The `workspace/configuration` request sent and not answered yet.
    configuration_request: Option<String>,
    /// The workspace folders (`scopeUri` of the configuration items).
    folders: Vec<String>,
    /// The client's current configuration (the global one).
    configuration: crate::registration::Configuration,
    /// The configuration of each workspace folder (`forResource`), by folder path.
    configuration_by_folder: Vec<(std::path::PathBuf, crate::registration::Configuration)>,
    /// The client accepts renaming a file in an edit (for the configuration's
    /// `renameFilesWithClasses: always`).
    possible_file_rename: bool,
    /// The commands the client declares (`experimental.commands`, the
    /// `supportedCommands`): `dart.goToLocation` enables the lenses.
    client_commands: Vec<String>,
    /// The client declared `experimental.supportsWindowShowMessageRequest`
    /// (Dart's `userPromptSender`): rename asks the user.
    user_questions: bool,
    /// The rename `window/showMessageRequest`s not answered yet, by request id
    /// (§12.1 steps 13 and 17).
    pending_renames: HashMap<String, PendingRename>,
}

/// The `rename` waiting for the user's answer.
#[derive(Debug)]
struct PendingRename {
    /// The id of the `textDocument/rename`.
    id: Value,
    /// The request's document and its version at the start (`null` if closed),
    /// for `fileHasBeenModified`.
    uri: String,
    version: Option<i32>,
    edits: Vec<crate::Edit>,
    file: Option<crate::RenameFile>,
    /// The question asked: `Rename Anyway`/`Cancel` (false) or the file's
    /// `Yes`/`No` (true).
    file_question: bool,
}

impl Server<SyntacticAnalyzer> {
    /// Creates the server with the syntactic analyzer and an empty queue.
    pub fn new() -> Self {
        Self::with_analyzer(SyntacticAnalyzer::new())
    }
}

impl Default for Server<SyntacticAnalyzer> {
    /// Creates the server with the syntactic analyzer and an empty queue.
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Analyzer> Server<A> {
    /// Creates the server with an arbitrary analyzer (the semantic seam).
    pub fn with_analyzer(analyzer: A) -> Self {
        Self {
            documents: DocumentStore::new(),
            analyzer,
            queue: VecDeque::new(),
            cancelled: HashSet::new(),
            disk_lines: Default::default(),
            disabling: false,
            initialized: false,
            shutdown: false,
            code: 1,
            hierarchical_symbols: false,
            hover_markdown: false,
            prepare_rename: false,
            versioned_changes: false,
            edits_snippet: false,
            rename_files: false,
            create_files: false,
            completion_calls: false,
            completion_not_imported: false,
            completion_capabilities: Default::default(),
            not_imported: HashMap::new(),
            documentation_markdown: false,
            roots: Vec::new(),
            with_client_diagnostics: std::collections::HashSet::new(),
            hierarchy_origin: None,
            typed: None,
            typed_tried: false,
            wake: None,
            typed_discarded: 0,
            typed_published: HashMap::new(),
            pulled_diagnostics: false,
            update_pulled: false,
            next_request: 0,
            apply_edits: false,
            closing_labels: false,
            work_progress: false,
            analyzing: false,
            pending_outputs: Vec::new(),
            outline: false,
            change_annotations: false,
            actions_literals: false,
            action_kinds: None,
            pending_edits: HashMap::new(),
            signature_markdown: false,
            signature_without_active: false,
            folds_line_only: false,
            multiline_tokens: false,
            label_details: false,
            symbol_kinds: None,
            workspace_symbol_kinds: None,
            symbol_index: Default::default(),
            requestable_configuration: false,
            dynamic: Vec::new(),
            dynamic_configuration: false,
            observe_files: false,
            dynamic_file_operations: false,
            static_capabilities: Value::Null,
            records: crate::registration::Records::default(),
            configuration_request: None,
            folders: Vec::new(),
            configuration: crate::registration::Configuration::default(),
            configuration_by_folder: Vec::new(),
            possible_file_rename: false,
            user_questions: false,
            client_commands: Vec::new(),
            pending_renames: HashMap::new(),
        }
    }

    /// Asks the client for the configuration (`workspace/configuration`, §2.5):
    /// one entry per workspace folder, in order, and the global one last.
    fn request_configuration(&mut self) {
        self.next_request += 1;
        let id = format!("dartforge/configuration/{}", self.next_request);
        let mut items: Vec<Value> = self.folders.iter().map(|p| json!({"scopeUri": p, "section": "dart"})).collect();
        items.push(json!({"section": "dart"}));
        self.configuration_request = Some(id.clone());
        self.pending_outputs.push(json!({"jsonrpc": "2.0", "id": id, "method": "workspace/configuration", "params": {"items": items}}));
    }

    /// `performDynamicRegistration` (§2.4): the difference against the current
    /// registrations, first what leaves, then what comes in; nothing without a
    /// difference.
    fn register_dynamic(&mut self) {
        let rename_files = self.dynamic_file_operations && self.configuration.update_imports_on_rename;
        let new = crate::registration::records(
            &self.static_capabilities,
            &self.dynamic,
            self.dynamic_configuration,
            rename_files,
            self.observe_files,
        );
        // `supportsDynamic => enableFormatter && ...` of the three formatting
        // features: with `enableSdkFormatter: false`, they go away.
        let mut new = new;
        if !self.configuration.enable_sdk_formatter {
            new.retain(|(m, _)| !m.ends_with("ormatting"));
        }
        let (exit, enter) = self.records.difference(new);
        if !exit.is_empty() {
            self.next_request += 1;
            let id = format!("dartforge/unregisterCapability/{}", self.next_request);
            // The protocol's spelling: `unregisterations`.
            self.pending_outputs.push(json!({"jsonrpc": "2.0", "id": id, "method": "client/unregisterCapability", "params": {"unregisterations": exit}}));
        }
        if !enter.is_empty() {
            self.next_request += 1;
            let id = format!("dartforge/registerCapability/{}", self.next_request);
            self.pending_outputs.push(json!({"jsonrpc": "2.0", "id": id, "method": "client/registerCapability", "params": {"registrations": enter}}));
        }
    }

    /// The response to `workspace/configuration`: only a list with one entry per
    /// folder and the global one counts (a `null` item is `{}`); otherwise, the
    /// previous configuration stays. Then, always, the dynamic registration.
    fn received_configuration(&mut self, message: &Value) {
        self.configuration_request = None;
        if let Some(list) = message.get("result").and_then(Value::as_array)
            && list.len() == self.folders.len() + 1
        {
            let global = list.last().cloned().unwrap_or(Value::Null);
            let new = crate::registration::read_configuration(&global);
            if new.rename_files_with_classes == "always" {
                self.rename_files = self.possible_file_rename;
            } else if new.rename_files_with_classes == "never" && self.configuration.rename_files_with_classes != "never" {
                self.rename_files = false;
            }
            // `affectsAnalysisResults` and `affectsAnalysisRoots`: the TODOs or the
            // excluded folders changed, the analysis runs again.
            let redo = new.show_all != self.configuration.show_all
                || new.show_all_types != self.configuration.show_all_types
                || new.analysis_excluded_folders != self.configuration.analysis_excluded_folders;
            // Those of the folders (the list comes in `folders` order, the global one last).
            self.configuration_by_folder = self
                .folders
                .iter()
                .zip(list.iter())
                .filter_map(|(u, v)| {
                    let path = url::Url::parse(u).ok()?.to_file_path().ok()?;
                    v.is_object().then(|| (path, crate::registration::feature_configuration(v, &new)))
                })
                .collect();
            self.configuration = new;
            if redo {
                self.reconfigure_analysis();
            }
        }
        self.register_dynamic();
    }

    /// The configuration of `uri` (`forResource`): that of the deepest workspace
    /// folder that contains it, otherwise the global one.
    fn configuration_of(&self, uri: &str) -> &crate::registration::Configuration {
        let Some(path) = url::Url::parse(uri).ok().and_then(|u| u.to_file_path().ok()) else { return &self.configuration };
        self.configuration_by_folder
            .iter()
            .filter(|(p, _)| path.starts_with(p))
            .max_by_key(|(p, _)| p.as_os_str().len())
            .map_or(&self.configuration, |(_, c)| c)
    }

    /// The folders excluded from analysis (`analysisExcludedFolders`): absolute
    /// paths as they are; relative ones, under each workspace folder
    /// (`_refreshAnalysisRoots`).
    fn excluded_folders(&self) -> Vec<std::path::PathBuf> {
        let mut v = Vec::new();
        for e in &self.configuration.analysis_excluded_folders {
            let p = std::path::Path::new(e);
            if p.is_absolute() {
                v.push(p.to_path_buf());
            } else {
                v.extend(self.roots.iter().map(|r| r.join(p)));
            }
        }
        v
    }

    /// The document is in a folder excluded from analysis.
    fn excluded(&self, uri: &str) -> bool {
        let excluded = self.excluded_folders();
        !excluded.is_empty()
            && url::Url::parse(uri).ok().and_then(|u| u.to_file_path().ok()).is_some_and(|c| excluded.iter().any(|e| c.starts_with(e)))
    }

    /// The shown TODOs or the excluded folders changed: the typed worker gets the
    /// new configuration and the open documents are analyzed and published again.
    fn reconfigure_analysis(&mut self) {
        let excluded = self.excluded_folders();
        if let Some(t) = &self.typed {
            t.configure(self.configuration.show_all, self.configuration.show_all_types.clone(), excluded);
        }
        let uris: Vec<String> = self.documents.uris().map(str::to_string).collect();
        for u in uris {
            if !self.pulled_diagnostics {
                let p = self.publish(&u);
                self.pending_outputs.push(p);
            }
            self.request_typed(&u);
        }
    }

    /// Registers the hook called (from another thread) when the typed analysis
    /// has a result: the `main` loop uses it to wake up and call
    /// [`Server::pump`], which publishes what is still of the current version.
    pub fn on_diagnostics(&mut self, hook: impl Fn() + Send + Sync + 'static) {
        self.wake = Some(crate::typed::Wake(std::sync::Arc::new(hook)));
    }

    /// Waits for the typed analysis to go idle (up to `limit`) and returns the
    /// publications it produced, already filtered by the current version. For
    /// tests and measurements; the editor gets the same ones through
    /// [`Server::pump`].
    pub fn await_diagnostics(&mut self, limit: std::time::Duration) -> Vec<Value> {
        if let Some(t) = &self.typed {
            t.wait_idle(limit);
        }
        self.drain_typed()
    }

    /// Waits for the typed analysis to go idle without publishing the result
    /// (which is left for the next [`Server::pump`]). Test hook.
    #[doc(hidden)]
    pub fn wait_idle_analysis(&self, limit: std::time::Duration) -> bool {
        self.typed.as_ref().is_none_or(|t| t.wait_idle(limit))
    }

    /// Suspends (or resumes) the start of typed analyses. Test hook to exercise
    /// the coalescing of fast edits.
    #[doc(hidden)]
    pub fn pause_typed_analysis(&mut self, pause: bool) {
        self.ensure_typed();
        if let Some(t) = &self.typed {
            t.pause(pause);
        }
    }

    /// Typed results discarded so far (old version or document closed before
    /// publishing).
    pub fn typed_discarded(&self) -> usize {
        self.typed_discarded
    }

    /// Starts the typed worker once, if the analyzer has an SDK.
    fn ensure_typed(&mut self) {
        if self.typed_tried {
            return;
        }
        self.typed_tried = true;
        if let Some(sdk) = crate::typed::analyzer_sdk(&self.analyzer) {
            self.typed = crate::typed::Typed::start(sdk, self.wake.clone());
        }
        // The current analysis configuration (the TODOs, the excluded folders).
        let excluded = self.excluded_folders();
        if let Some(t) = &self.typed
            && (self.configuration.show_all || !self.configuration.show_all_types.is_empty() || !excluded.is_empty())
        {
            t.configure(self.configuration.show_all, self.configuration.show_all_types.clone(), excluded);
        }
    }

    /// The workspace folders for the typed worker: every `.dart` in them is
    /// analyzed and published (§3.2, I7).
    fn analyze_folders(&mut self) {
        self.ensure_typed();
        let Some(t) = &self.typed else { return };
        t.folders(self.roots.clone());
        if !self.roots.is_empty() {
            self.mark_analyzing();
        }
    }

    /// Requests the typed analysis of the current text of `uri`.
    fn request_typed(&mut self, uri: &str) {
        self.ensure_typed();
        let (Some(t), Some(version), Some(text)) = (&self.typed, self.documents.version(uri), self.documents.get(uri)) else {
            return;
        };
        t.document(uri, version, text);
        self.mark_analyzing();
    }

    /// The analysis state (§2.6): idle → working.
    fn mark_analyzing(&mut self) {
        if !self.analyzing {
            self.analyzing = true;
            if self.work_progress {
                self.next_request += 1;
                self.pending_outputs.push(json!({
                    "jsonrpc": "2.0",
                    "id": format!("dartforge/progress/{}", self.next_request),
                    "method": "window/workDoneProgress/create",
                    "params": {"token": "ANALYZING"},
                }));
                self.pending_outputs.push(json!({
                    "jsonrpc": "2.0",
                    "method": "$/progress",
                    "params": {"token": "ANALYZING", "value": {"kind": "begin", "title": "Analyzing\u{2026}"}},
                }));
            } else {
                self.pending_outputs.push(json!({"jsonrpc": "2.0", "method": "$/analyzerStatus", "params": {"isAnalyzing": true}}));
            }
        }
    }

    /// Publishes the ready typed results that are still of the current version;
    /// the others are discarded.
    fn drain_typed(&mut self) -> Vec<Value> {
        let Some(results) = self.typed.as_ref().map(|t| t.receive()) else { return Vec::new() };
        let mut outputs = Vec::new();
        let mut new = false;
        let mut received = false;
        for r in results {
            received = true;
            // A file of the roots that is not open (I7): publishes with the analyzed
            // text (from disk); if it was opened in the meantime, the document's
            // analysis counts.
            let Some(version) = r.version else {
                if self.documents.version(&r.uri).is_some() {
                    self.typed_discarded += 1;
                    continue;
                }
                if !self.pulled_diagnostics
                    && let Some(text) = &r.text
                {
                    let p = if r.uri.ends_with(".yaml") {
                        self.yaml_publication(&r.uri, text, &r.yaml)
                    } else {
                        self.text_publication(&r.uri, text, &r.diagnostics)
                    };
                    if let Some(p) = p {
                        outputs.push(p);
                    }
                }
                continue;
            };
            if self.documents.version(&r.uri) != Some(version) {
                self.typed_discarded += 1;
                continue;
            }
            if !self.pulled_diagnostics {
                let p = self.publication(&r.uri, &r.diagnostics);
                if let Some(p) = self.filter_repeated_empty(&r.uri, p) {
                    outputs.push(p);
                }
            }
            // The unit has just been resolved for the current version: it is the point
            // at which the Dart server sends the closing labels of an open file (§3.4).
            if self.closing_labels {
                let labels = self.analyzer.closing_labels(&self.documents, &r.uri);
                let list: Vec<Value> = labels
                    .iter()
                    .filter_map(|(span, text)| Some(json!({"range": self.range(&r.uri, *span)?, "label": text})))
                    .collect();
                outputs.push(json!({
                    "jsonrpc": "2.0",
                    "method": "dart/textDocument/publishClosingLabels",
                    "params": {"uri": r.uri, "labels": list},
                }));
            }
            // The outline in Dart's format, at the same point (§3.4).
            if self.outline
                && let Some(text) = self.documents.get(&r.uri)
            {
                outputs.push(json!({
                    "jsonrpc": "2.0",
                    "method": "dart/textDocument/publishOutline",
                    "params": {"uri": r.uri, "outline": crate::outline::for_document(text)},
                }));
            }
            self.typed_published.insert(r.uri, (version, r.diagnostics));
            new = true;
        }
        // Pulled: one `refresh` request per batch, and the client pulls the visible
        // documents again (the `resultId` changes with the typed result).
        if new && self.pulled_diagnostics && self.update_pulled {
            self.next_request += 1;
            outputs.push(json!({
                "jsonrpc": "2.0",
                "id": format!("dartforge/refresh/{}", self.next_request),
                "method": "workspace/diagnostic/refresh",
            }));
        }
        // The analysis state (§2.6): working → idle. The result arrives a moment
        // before the worker declares itself idle; the short wait covers that window.
        let idle = self.typed.as_ref().is_some_and(|t| t.idle() || (received && t.wait_idle(std::time::Duration::from_millis(20))));
        if self.analyzing && idle {
            self.analyzing = false;
            // Ahead of this pass's publications: whoever reads the last message still
            // finds the publication.
            let end = if self.work_progress {
                json!({"jsonrpc": "2.0", "method": "$/progress", "params": {"token": "ANALYZING", "value": {"kind": "end"}}})
            } else {
                json!({"jsonrpc": "2.0", "method": "$/analyzerStatus", "params": {"isAnalyzing": false}})
            };
            outputs.insert(0, end);
        }
        outputs
    }

    /// Enqueues a message decoded from the transport, without dispatching it.
    ///
    /// Enqueuing never responds: the outputs come out of [`Server::pump`].
    pub fn receive(&mut self, message: Value) {
        self.queue.push_back(message);
    }

    /// There are messages waiting for dispatch.
    pub fn has_pending(&self) -> bool {
        !self.queue.is_empty()
    }

    /// The main loop must end after writing the returned outputs.
    pub fn must_shutdown(&self) -> bool {
        self.shutdown
    }

    /// Exit code: 0 after `shutdown`, 1 without it.
    pub fn exit_code(&self) -> i32 {
        self.code
    }

    /// The analyzer (measurement and tests: semantic session stats).
    pub fn analyzer(&self) -> &A {
        &self.analyzer
    }

    /// Retained open documents (used in tests and measurement).
    pub fn open_documents(&self) -> usize {
        self.documents.len()
    }

    /// Dispatches the next step and returns the outgoing messages, in order.
    ///
    /// A step is: apply all the cancellations in the queue; process the leading
    /// notifications (each `didOpen`/`didChange` publishes diagnostics); execute
    /// **one** request (or answer `RequestCancelled` if it was cancelled).
    /// Messages that are neither requests nor valid notifications are silently
    /// ignored: among them the client's responses to the server's only request
    /// (`workspace/diagnostic/refresh`), which carry no data.
    pub fn pump(&mut self) -> Vec<Value> {
        self.apply_cancellations();
        let mut outputs = Vec::new();
        while let Some(next) = self.queue.front() {
            if is_request(next) {
                break;
            }
            let message = self.queue.pop_front().expect("non-empty queue");
            // A panic in a notification: the Dart server's `showMessage` and
            // `logMessage` (`lsp_analysis_server.dart`, `sendErrorResponse` and
            // `logException`), and the semantic session drops.
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.handle_notification(&message))) {
                Ok(Some(output)) => outputs.push(output),
                Ok(None) => {}
                Err(panic) => {
                    self.analyzer.changed_document("");
                    let method = message.get("method").and_then(Value::as_str).unwrap_or("?").to_string();
                    let text = format!("An error occurred while handling {method} notification");
                    register(format!("panic while handling {method}"));
                    outputs.push(message_notification("window/showMessage", 1, &text));
                    outputs.push(message_notification("window/logMessage", 1, &format!("{text}: {}", panic_text(&*panic))));
                }
            }
            if self.shutdown {
                return outputs;
            }
        }
        if let Some(request) = self.queue.pop_front() {
            debug_assert!(is_request(&request));
            // A panic in a query does not bring the server down (Dart's never goes down
            // because of a request): the response is an internal error and the semantic
            // session, which may have been left half-done, drops.
            let response = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.handle_request(&request)));
            match response {
                Ok(r) => outputs.push(r),
                Err(panic) => {
                    // The Dart server's `UnhandledError` (-32001), with its message, and the
                    // `logException` (an error-type `window/logMessage`, with the cause).
                    self.analyzer.changed_document("");
                    let id = request.get("id").cloned().unwrap_or(Value::Null);
                    let method = request.get("method").and_then(Value::as_str).unwrap_or("?").to_string();
                    register(format!("panic while handling {method}"));
                    let text = format!("An error occurred while handling {method} request");
                    outputs.push(error(&id, ERROR_NOT_HANDLED, text.clone()));
                    outputs.push(message_notification("window/logMessage", 1, &format!("{text}: {}", panic_text(&*panic))));
                }
            }
        }
        // Typed results after the notifications: what an already received change
        // made stale is discarded here, before reaching the editor. (A request does
        // not change documents; the response goes out first.) The start of the
        // analysis goes ahead of the pass's outputs.
        if !self.pending_outputs.is_empty() {
            let mut with_pending = std::mem::take(&mut self.pending_outputs);
            with_pending.append(&mut outputs);
            outputs = with_pending;
        }
        outputs.extend(self.drain_typed());
        outputs
    }

    /// Extracts and applies every `$/cancelRequest` in the queue, whatever its
    /// position.
    ///
    /// Only keeps the id when there is a request with that id still in the
    /// queue; otherwise the cancellation is discarded, so that unknown ids do
    /// not make the set grow with the number of messages.
    fn apply_cancellations(&mut self) {
        let mut scan = 0;
        while scan < self.queue.len() {
            let is_cancel = self.queue[scan]
                .get("method")
                .is_some_and(|m| m.as_str() == Some("$/cancelRequest"));
            if !is_cancel {
                scan += 1;
                continue;
            }
            let cancelled = self.queue.remove(scan).expect("scanned index");
            let target = cancelled.get("params").and_then(|p| p.get("id")).cloned();
            if let Some(id) = target {
                let key = key_id(&id);
                let has_request = self.queue.iter().any(|m| {
                    is_request(m) && m.get("id").is_some_and(|other| key_id(other) == key)
                });
                if has_request {
                    if self.cancelled.len() >= MAX_CANCELLATIONS {
                        self.cancelled.clear();
                    }
                    self.cancelled.insert(key);
                }
            }
        }
    }

    /// Handles a notification; returns the diagnostics publication when there is one.
    fn handle_notification(&mut self, message: &Value) -> Option<Value> {
        if message.get("method").is_none() {
            return self.client_response(message);
        }
        let method = message.get("method")?.as_str()?;
        match method {
            // With the capability, the configuration and then the dynamic registration;
            // without it, only the registration (§2.2).
            "initialized" => {
                if self.requestable_configuration {
                    self.request_configuration();
                } else {
                    self.register_dynamic();
                }
                // The files of the roots are analyzed and published from the start (Dart's
                // driver adds them when creating the contexts, §3.2).
                self.analyze_folders();
                None
            }
            "$/cancelRequest" => None,
            // The received `settings` are ignored: the configuration is requested again
            // (`handler_workspace_configuration.dart:22-31`).
            "workspace/didChangeConfiguration" => {
                if self.requestable_configuration {
                    self.request_configuration();
                }
                None
            }
            "exit" => {
                self.shutdown = true;
                self.code = if self.disabling { 0 } else { 1 };
                None
            }
            "textDocument/didOpen" => {
                let params = message.get("params")?;
                let doc = params.get("textDocument")?;
                let uri = doc.get("uri")?.as_str()?;
                let version = doc.get("version")?.as_i64()? as i32;
                let text = doc.get("text")?.as_str()?;
                if self.documents.get(uri).is_some() {
                    self.analyzer.closed_document(uri);
                }
                self.documents
                    .open(uri.to_string(), version, text.to_string());
                self.analyzer.open_document(&self.documents, uri);
                self.request_typed(uri);
                if self.pulled_diagnostics {
                    return None;
                }
                let p = self.publish(uri);
                self.filter_repeated_empty(uri, p)
            }
            "textDocument/didChange" => {
                let params = message.get("params")?;
                let doc = params.get("textDocument")?;
                let uri = doc.get("uri")?.as_str()?;
                let version = doc.get("version")?.as_i64()? as i32;
                let changes = read_changes(params.get("contentChanges")?);
                if !self.documents.apply(uri, version, &changes) {
                    return None;
                }
                self.analyzer.edited_document(&self.documents, uri);
                self.request_typed(uri);
                if self.pulled_diagnostics {
                    return None;
                }
                let p = self.publish(uri);
                self.filter_repeated_empty(uri, p)
            }
            "textDocument/didClose" => {
                let params = message.get("params")?;
                let uri = params.get("textDocument")?.get("uri")?.as_str()?;
                if self.documents.close(uri) {
                    self.typed_published.remove(uri);
                    self.analyzer.document_closed_at(&self.documents, uri);
                    if let Some(t) = &self.typed {
                        t.closed(uri);
                    }
                }
                if self.pulled_diagnostics {
                    return None;
                }
                // Inside the workspace folders `didClose` does not clear: the file is
                // analyzed again from disk and republished (§3.2). Outside them, the file's
                // root goes away and its results are cleared (`flushResults`).
                let inside = url::Url::parse(uri).ok().and_then(|u| u.to_file_path().ok()).is_some_and(|c| self.roots.iter().any(|r| c.starts_with(r)));
                if inside && self.typed.is_some() {
                    return None;
                }
                self.filter_repeated_empty(uri, empty_publication(uri))
            }
            // Common editor notifications this server does not use: accepted silently
            // (Dart's handles the first two and does not register the third).
            // The workspace folders changed: the analysis roots and those of the
            // configuration request follow (the `changeNotifications`).
            "workspace/didChangeWorkspaceFolders" => {
                let file_path_of = |v: &Value| v.as_str().and_then(|u| url::Url::parse(u).ok()).and_then(|u| u.to_file_path().ok());
                let list = |k: &str| message.pointer(&format!("/params/event/{k}")).and_then(Value::as_array).cloned().unwrap_or_default();
                for f in list("removed") {
                    if let Some(u) = f["uri"].as_str() {
                        self.folders.retain(|p| p != u);
                    }
                    if let Some(p) = file_path_of(&f["uri"]) {
                        self.roots.retain(|r| *r != p);
                    }
                }
                for f in list("added") {
                    if let Some(u) = f["uri"].as_str()
                        && !self.folders.iter().any(|p| p == u)
                    {
                        self.folders.push(u.to_string());
                    }
                    if let Some(p) = file_path_of(&f["uri"])
                        && p.is_dir()
                        && !self.roots.contains(&p)
                    {
                        self.roots.push(p);
                    }
                }
                // What the session retains may depend on the roots.
                self.analyzer.changed_document("");
                // The new folders join the worker's analysis (I7).
                if self.typed_tried {
                    self.analyze_folders();
                }
                None
            }
            "workspace/didChangeWatchedFiles" => {
                // The file watcher (§3.3): what changed on disk.
                if let Some(t) = &self.typed
                    && let Some(changes) = message.pointer("/params/changes").and_then(Value::as_array)
                {
                    for m in changes {
                        let Some(path) = m
                            .get("uri")
                            .and_then(Value::as_str)
                            .and_then(|u| url::Url::parse(u).ok())
                            .filter(|u| u.scheme() == "file")
                            .and_then(|u| u.to_file_path().ok())
                        else {
                            continue;
                        };
                        t.file_on_disk(&path);
                    }
                }
                None
            }
            _ => {
                register(format!("unknown notification ignored: {method}"));
                // §2.3: `$/…` is ignored; any other becomes the `Unknown method` error,
                // which for a notification is an error `window/showMessage`. After
                // `shutdown`, it is discarded.
                if method.starts_with("$/") || self.disabling {
                    None
                } else {
                    Some(json!({
                        "jsonrpc": "2.0",
                        "method": "window/showMessage",
                        "params": {"type": 1, "message": format!("Unknown method {method}")},
                    }))
                }
            }
        }
    }

    /// Executes a request and returns exactly one response.
    fn handle_request(&mut self, message: &Value) -> Value {
        self.disk_lines.get_mut().clear();
        let id = message.get("id").cloned().unwrap_or(Value::Null);
        let key = key_id(&id);
        if self.cancelled.remove(&key) {
            return error(&id, REQUEST_CANCELLED, "request cancelled");
        }
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        // The Dart server's states (§2.3) that do not depend on the `initialized`
        // notification: after `shutdown` only `exit` counts, and `initialize` only
        // counts once.
        if self.disabling {
            return error(&id, INVALID_REQUEST, format!("Unable to handle {method} after shutdown request"));
        }
        if method == "initialize" && self.initialized {
            return error(&id, SERVER_NOT_INITIALIZED, "Server already initialized");
        }
        match method {
            "initialize" => {
                self.initialized = true;
                self.hierarchical_symbols = message
                    .pointer("/params/capabilities/textDocument/documentSymbol/hierarchicalDocumentSymbolSupport")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.hover_markdown = message
                    .pointer("/params/capabilities/textDocument/hover/contentFormat")
                    .and_then(Value::as_array)
                    .is_some_and(|formats| formats.iter().any(|f| f.as_str() == Some("markdown")));
                self.prepare_rename = message
                    .pointer("/params/capabilities/textDocument/rename/prepareSupport")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.versioned_changes = message
                    .pointer("/params/capabilities/workspace/workspaceEdit/documentChanges")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                // `experimental.snippetTextEdit == true` (`client_capabilities.dart:368-371`).
                self.edits_snippet =
                    message.pointer("/params/capabilities/experimental/snippetTextEdit").and_then(Value::as_bool).unwrap_or(false);
                self.apply_edits =
                    message.pointer("/params/capabilities/workspace/applyEdit").and_then(Value::as_bool).unwrap_or(false);
                self.closing_labels =
                    message.pointer("/params/initializationOptions/closingLabels").and_then(Value::as_bool).unwrap_or(false);
                self.outline = message.pointer("/params/initializationOptions/outline").and_then(Value::as_bool).unwrap_or(false);
                self.work_progress =
                    message.pointer("/params/capabilities/window/workDoneProgress").and_then(Value::as_bool).unwrap_or(false);
                self.change_annotations = message
                    .pointer("/params/capabilities/workspace/workspaceEdit/changeAnnotationSupport")
                    .is_some_and(|x| !x.is_null());
                let literals = message.pointer("/params/capabilities/textDocument/codeAction/codeActionLiteralSupport");
                self.actions_literals = literals.is_some_and(|l| !l.is_null());
                self.action_kinds = literals
                    .and_then(|l| l.pointer("/codeActionKind/valueSet"))
                    .and_then(Value::as_array)
                    .map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect());
                let accepts_operation = |op: &str| {
                    message
                        .pointer("/params/capabilities/workspace/workspaceEdit/resourceOperations")
                        .and_then(Value::as_array)
                        .is_some_and(|l| l.iter().any(|o| o.as_str() == Some(op)))
                };
                let accepts_rename_file = accepts_operation("rename");
                self.possible_file_rename = self.versioned_changes && accepts_rename_file;
                self.user_questions = message
                    .pointer("/params/capabilities/experimental/supportsWindowShowMessageRequest")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.client_commands = message
                    .pointer("/params/capabilities/experimental/commands")
                    .and_then(Value::as_array)
                    .map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect())
                    .unwrap_or_default();
                self.requestable_configuration =
                    message.pointer("/params/capabilities/workspace/configuration").and_then(Value::as_bool).unwrap_or(false);
                let (dynamic, dynamic_configuration) =
                    crate::registration::dynamic(message.pointer("/params/capabilities").unwrap_or(&Value::Null));
                self.dynamic = dynamic;
                self.dynamic_configuration = dynamic_configuration;
                self.observe_files = message
                    .pointer("/params/capabilities/workspace/didChangeWatchedFiles/dynamicRegistration")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.dynamic_file_operations = message
                    .pointer("/params/capabilities/workspace/fileOperations/dynamicRegistration")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                // The workspace folders (or the root), for the `scopeUri`.
                self.folders = match message.pointer("/params/workspaceFolders").and_then(Value::as_array) {
                    Some(l) => l.iter().filter_map(|p| p.get("uri").and_then(Value::as_str)).map(str::to_string).collect(),
                    None => message.pointer("/params/rootUri").and_then(Value::as_str).map(|u| vec![u.to_string()]).unwrap_or_default(),
                };
                self.create_files = self.versioned_changes
                    && accepts_operation("create");
                self.rename_files = self.versioned_changes
                    && accepts_rename_file
                    && message.pointer("/params/initializationOptions/renameFilesWithClasses").and_then(Value::as_str) == Some("always");
                self.roots = initialize_roots(message.get("params"));
                self.completion_calls = message
                    .pointer("/params/capabilities/textDocument/completion/completionItem/snippetSupport")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    && message.pointer("/params/initializationOptions/completeFunctionCalls").and_then(Value::as_bool) != Some(false);
                self.completion_not_imported = message.pointer("/params/initializationOptions/suggestFromUnimportedLibraries").and_then(Value::as_bool) != Some(false)
                    && message.pointer("/params/capabilities/workspace/applyEdit").and_then(Value::as_bool).unwrap_or(false);
                self.completion_capabilities = crate::completion_item::Capabilities::from_initialize(message.pointer("/params/capabilities").unwrap_or(&Value::Null));
                self.documentation_markdown = message
                    .pointer("/params/capabilities/textDocument/completion/completionItem/documentationFormat")
                    .and_then(Value::as_array)
                    .is_some_and(|f| f.iter().any(|x| x.as_str() == Some("markdown")));
                self.pulled_diagnostics = message.pointer("/params/capabilities/textDocument/diagnostic").is_some_and(Value::is_object);
                self.update_pulled = message
                    .pointer("/params/capabilities/workspace/diagnostics/refreshSupport")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.signature_markdown = message
                    .pointer("/params/capabilities/textDocument/signatureHelp/signatureInformation/documentationFormat")
                    .and_then(Value::as_array)
                    .is_some_and(|f| f.iter().any(|x| x.as_str() == Some("markdown")));
                self.signature_without_active = message
                    .pointer("/params/capabilities/textDocument/signatureHelp/signatureInformation/noActiveParameterSupport")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.symbol_kinds = message
                    .pointer("/params/capabilities/textDocument/documentSymbol/symbolKind/valueSet")
                    .and_then(Value::as_array)
                    .map(|l| l.iter().filter_map(Value::as_u64).collect());
                self.workspace_symbol_kinds = message
                    .pointer("/params/capabilities/workspace/symbol/symbolKind/valueSet")
                    .and_then(Value::as_array)
                    .map(|l| l.iter().filter_map(Value::as_u64).collect());
                self.label_details = message
                    .pointer("/params/capabilities/textDocument/completion/completionItem/labelDetailsSupport")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.multiline_tokens = message
                    .pointer("/params/capabilities/textDocument/semanticTokens/multilineTokenSupport")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.folds_line_only = message
                    .pointer("/params/capabilities/textDocument/foldingRange/lineFoldingOnly")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let rename_ = if self.prepare_rename { json!({"prepareProvider": true}) } else { json!(true) };
                // `handler_code_actions.dart:252-280`: the list of kinds only for a client
                // with literals; otherwise, `true`.
                let code_actions = if self.actions_literals {
                    json!({"codeActionKinds": ["source", "source.organizeImports", "source.fixAll", "source.sortMembers", "quickfix", "refactor"]})
                } else {
                    json!(true)
                };
                let mut result = json!({
                    "capabilities": {
                        "textDocumentSync": SYNC_INCREMENTAL,
                        "positionEncoding": "utf-16",
                        "documentSymbolProvider": true,
                        "workspaceSymbolProvider": true,
                        "definitionProvider": true,
                        "referencesProvider": true,
                        "hoverProvider": true,
                        "renameProvider": rename_,
                        "codeActionProvider": code_actions,
                        // Only the commands this server executes (Dart's announces thirteen, §13.12.4).
                        "executeCommandProvider": {"commands": COMMANDS, "workDoneProgress": true},
                        // The Dart server's characters (`dartSignatureHelpTriggerCharacters`).
                        "signatureHelpProvider": {"triggerCharacters": ["("], "retriggerCharacters": [","]},
                        "documentHighlightProvider": true,
                        "implementationProvider": true,
                        "typeDefinitionProvider": true,
                        "documentLinkProvider": {"resolveProvider": false},
                        "codeLensProvider": {"resolveProvider": false},
                        "foldingRangeProvider": true,
                        "selectionRangeProvider": true,
                        "typeHierarchyProvider": true,
                        "callHierarchyProvider": true,
                        "inlayHintProvider": {"resolveProvider": false},
                        // `DocumentColorOptions()` (§8.2).
                        "colorProvider": {},
                        "semanticTokensProvider": {
                            "legend": {"tokenTypes": crate::semantic_tokens::TYPES, "tokenModifiers": crate::semantic_tokens::MODIFIERS},
                            "full": true,
                            "range": true,
                        },
                        "completionProvider": {
                            "triggerCharacters": ["."],
                            "resolveProvider": true,
                        },
                    },
                    "serverInfo": {
                        "name": "dartforge-lsp",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                });
                // `workspace`: the folders and, without dynamic registration of the file
                // operations, the static `willRename` (with `updateImportsOnRename`,
                // `server_capabilities_computer.dart:197-205`).
                // Formatting (`FormattingRegistrations` and the like): static when
                // `enableSdkFormatter` (the default).
                if self.configuration.enable_sdk_formatter {
                    result["capabilities"]["documentFormattingProvider"] = json!(true);
                    result["capabilities"]["documentOnTypeFormattingProvider"] = json!({
                        "firstTriggerCharacter": crate::formatting::TRIGGER_CHARACTERS[0],
                        "moreTriggerCharacter": &crate::formatting::TRIGGER_CHARACTERS[1..],
                    });
                    result["capabilities"]["documentRangeFormattingProvider"] = json!(true);
                }
                result["capabilities"]["workspace"] = json!({"workspaceFolders": {"supported": true, "changeNotifications": true}});
                if !self.dynamic_file_operations && self.configuration.update_imports_on_rename {
                    result["capabilities"]["workspace"]["fileOperations"] = json!({"willRename": crate::registration::file_operation_options()});
                }
                if self.pulled_diagnostics {
                    // A document depends on others (imports): editing one changes the
                    // diagnostics of whoever imports it. No whole-workspace diagnostics (only
                    // for the requested documents).
                    result["capabilities"]["diagnosticProvider"] = json!({
                        "identifier": SOURCE,
                        "interFileDependencies": true,
                        "workspaceDiagnostics": false,
                    });
                }
                // §2.4: the dynamic features leave the static ones and are registered after
                // `initialized`.
                self.static_capabilities = result["capabilities"].clone();
                crate::registration::remove_dynamic(&mut result["capabilities"], &self.dynamic);
                response(&id, result)
            }
            "textDocument/diagnostic" => {
                let uri = message.pointer("/params/textDocument/uri").and_then(Value::as_str).map(str::to_string);
                let previous = message.pointer("/params/previousResultId").and_then(Value::as_str);
                response(&id, uri.map_or_else(|| json!({"kind": "full", "items": []}), |u| self.pulled_report(&u, previous)))
            }
            "shutdown" => {
                self.disabling = true;
                response(&id, Value::Null)
            }
            "textDocument/documentSymbol" => {
                let uri = message.get("params")
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|d| d.get("uri"))
                    .and_then(Value::as_str);
                let mut symbols = uri
                    .and_then(|u| self.documents.get(u).map(|t| (u, t.to_string())))
                    .map_or_else(Vec::new, |(u, t)| self.analyzer.symbols(u, &t));
                for s in &mut symbols {
                    adjust_kinds(s, self.symbol_kinds.as_deref());
                }
                let result = if self.hierarchical_symbols {
                    symbols
                } else {
                    let mut plans = Vec::new();
                    for symbol in &symbols {
                        flatten_symbols(symbol, uri.unwrap_or(""), None, &mut plans);
                    }
                    plans
                };
                response(&id, json!(result))
            }
            "workspace/symbol" => {
                let query = message.get("params")
                    .and_then(|p| p.get("query"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                response(&id, json!(self.workspace_symbols(&query)))
            }
            "textDocument/definition" => {
                let params = message.get("params");
                let uri = params
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|d| d.get("uri"))
                    .and_then(Value::as_str);
                let position = params.and_then(|p| p.get("position")).and_then(read_position);
                let result = uri.zip(position).and_then(|(u, p)| {
                    let text = self.documents.get(u)?.to_string();
                    let offset = self.documents.lines(u)?
                        .position_offset(&text, p.line, p.column);
                    let (target_, selection) = self.analyzer.definition_in_workspace(u, &text, offset, &self.documents)?;
                    let range = if let Some(s) = selection {
                        let (l0, c0, l1, c1) = if let Some(source) = self.documents.get(&target_) {
                            let table = self.documents.lines(&target_)?;
                            let (l0, c0) = table.offset_position(source, s.start);
                            let (l1, c1) = table.offset_position(source, s.end);
                            (l0, c0, l1, c1)
                        } else {
                            let path = url::Url::parse(&target_).ok()?.to_file_path().ok()?;
                            let source = std::fs::read_to_string(path).ok()?;
                            let table = crate::utf16::LineTable::build(&source);
                            let (l0, c0) = table.offset_position(&source, s.start);
                            let (l1, c1) = table.offset_position(&source, s.end);
                            (l0, c0, l1, c1)
                        };
                        json!({"start": {"line": l0, "character": c0}, "end": {"line": l1, "character": c1}})
                    } else {
                        json!({"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}})
                    };
                    Some(json!({
                        "uri": target_,
                        "range": range,
                    }))
                });
                response(&id, result.unwrap_or(Value::Null))
            }
            "textDocument/references" => {
                let params = message.get("params");
                let uri = params
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|d| d.get("uri"))
                    .and_then(Value::as_str);
                let position = params.and_then(|p| p.get("position")).and_then(read_position);
                let include_declaration = params
                    .and_then(|p| p.get("context"))
                    .and_then(|c| c.get("includeDeclaration"))
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                let result = uri.zip(position).and_then(|(u, p)| {
                    let text = self.documents.get(u)?.to_string();
                    let table = self.documents.lines(u)?;
                    let offset = table.position_offset(&text, p.line, p.column);
                    let found = self.analyzer.references_in(&self.documents, u, offset)?;
                    // The order of the Dart server's `SearchEngine` (probed on 3.6.2): the
                    // other files by path, then the declaring one; within each one, by
                    // position. The declaration goes at the end (`_getDeclarations`, §11.5).
                    let owner = found.declaration.as_ref().map(|(target, _)| target.clone());
                    let mut uses = found.uses.clone();
                    uses.sort_by(|(a, sa), (b, sb)| {
                        (Some(a) == owner.as_ref(), a, sa.start).cmp(&(Some(b) == owner.as_ref(), b, sb.start))
                    });
                    let declaration = found.declaration.filter(|_| include_declaration);
                    let locals: Vec<Value> = uses
                        .iter()
                        .chain(declaration.iter())
                        .filter_map(|(target, s)| {
                            Some(json!({"uri": target, "range": self.range(target, *s)?}))
                        })
                        .collect();
                    Some(json!(locals))
                });
                response(&id, result.unwrap_or(Value::Null))
            }
            "textDocument/hover" => {
                let params = message.get("params");
                let uri = params
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|d| d.get("uri"))
                    .and_then(Value::as_str);
                let position = params.and_then(|p| p.get("position")).and_then(read_position);
                let result = uri.zip(position).and_then(|(u, p)| {
                    let text = self.documents.get(u)?.to_string();
                    let table = self.documents.lines(u)?;
                    let offset = table.position_offset(&text, p.line, p.column);
                    let hover = self.analyzer.hover_in_workspace(u, &text, offset, &self.documents)?;
                    let (l0, c0) = table.offset_position(&text, hover.range.start);
                    let (l1, c1) = table.offset_position(&text, hover.range.end);
                    // The Dart server's format: the description in a code block, the type and,
                    // separated by `---`, the documentation.
                    let content = if self.hover_markdown {
                        // Like Dart's `toHover`: description, type, library and, after `---`, the
                        // documentation.
                        let mut value = format!("```dart\n{}\n```\n", hover.description);
                        if let Some(t) = &hover.ty {
                            value.push_str(&format!("Type: `{t}`\n\n"));
                        }
                        if let Some(b) = &hover.library {
                            value.push_str(&format!("*{b}*\n\n"));
                        }
                        if let Some(d) = &hover.documentation {
                            value.push_str(&format!("---\n{d}\n"));
                        }
                        json!({"kind": "markdown", "value": value.trim_end()})
                    } else {
                        let mut value = hover.description.clone();
                        if let Some(t) = &hover.ty {
                            value.push_str(&format!("\nType: {t}"));
                        }
                        if let Some(d) = &hover.documentation {
                            value.push_str(&format!("\n\n{d}"));
                        }
                        json!(value)
                    };
                    Some(json!({
                        "contents": content,
                        "range": {
                            "start": {"line": l0, "character": c0},
                            "end": {"line": l1, "character": c1},
                        },
                    }))
                });
                response(&id, result.unwrap_or(Value::Null))
            }
            "textDocument/completion" => {
                // `maxCompletionItems` and `enableSnippets` come from the document's folder
                // (`forResource`).
                let request_uri = message.pointer("/params/textDocument/uri").and_then(Value::as_str).unwrap_or_default().to_string();
                let max = self.configuration_of(&request_uri).max_completion_items.map_or(crate::completion::MAX_PATTERN, |m| m as usize);
                self.analyzer.set_completion_max(max);
                self.analyzer.set_not_imported_in_completion(self.completion_not_imported);
                let result = self.request_position(message).and_then(|(u, offset)| {
                    let completion = self.analyzer.completion(&self.documents, &u, offset)?;
                    let text = self.documents.get(&u)?.to_string();
                    let table = self.documents.lines(&u)?;
                    // `computeReplacementRange`: the whole word; `insert` up to the cursor.
                    let word_end = completion.replacement_end.unwrap_or_else(|| {
                        text[completion.end.min(text.len())..]
                            .char_indices()
                            .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_' || *c == '$'))
                            .map_or(text.len(), |(i, _)| completion.end + i)
                    });
                    let replace = lsp_range(&text, table, completion.start, word_end);
                    let insert = lsp_range(&text, table, completion.start, completion.end);
                    let equal = word_end == completion.end;
                    let cap = &self.completion_capabilities;
                    // `itemDefaults` (`HC:250-291`).
                    let mut patterns = serde_json::Map::new();
                    if cap.default_mode && cap.insertion_modes.contains(&1) {
                        patterns.insert("insertTextMode".into(), json!(1));
                    }
                    if cap.default_range {
                        patterns.insert(
                            "editRange".into(),
                            if !cap.insert_replace || equal { replace.clone() } else { json!({"insert": insert, "replace": replace}) },
                        );
                    }
                    // `_hasExistingArgList`: there are already parentheses after the name.
                    let with_parens = text[word_end..].trim_start_matches([' ', '\t']).starts_with('(');
                    let file = url::Url::parse(&u).ok().and_then(|x| x.to_file_path().ok()).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
                    let request = crate::completion_item::Request {
                        replace: replace.clone(),
                        insert: insert.clone(),
                        equal,
                        pattern: cap.default_range,
                        calls: self.configuration.complete_function_calls && !with_parens,
                        file: &file,
                        doc_preference: &self.configuration.documentation,
                    };
                    // The documentation of each item (from the declaration's file).
                    let mut texts: HashMap<std::path::PathBuf, Option<String>> = HashMap::new();
                    self.not_imported.clear();
                    let mut items: Vec<Value> = Vec::new();
                    for item in &completion.items {
                        let doc = match (&item.origin, &item.import) {
                            (Some((path, start)), None) => {
                                let source = texts.entry(path.clone()).or_insert_with(|| {
                                    url::Url::from_file_path(path)
                                        .ok()
                                        .and_then(|x| self.documents.get(x.as_str()).map(str::to_string))
                                        .or_else(|| std::fs::read_to_string(path).ok())
                                });
                                source.as_deref().filter(|f| *start <= f.len() && f.is_char_boundary(*start)).and_then(|f| crate::dartdoc::documentation(f, *start))
                            }
                            _ => None,
                        };
                        let v = crate::completion_item::item(cap, &request, item, doc);
                        if let Some(r) = v.pointer("/data/ref").and_then(Value::as_str) {
                            self.not_imported.insert(r.to_string(), (item.origin.clone(), item.import.clone(), item.detail.clone()));
                        }
                        items.push(v);
                    }
                    let mut incomplete = completion.incomplete;
                    // The snippets (without ranking, at the end).
                    if cap.snippet && self.configuration_of(&u).enable_snippets {
                        let prefix = &text[completion.start..completion.end];
                        if let Some(context) = crate::completion_item::snippet_context(&text, offset) {
                            let indent: String = {
                                let start_ = text[..offset].rfind('\n').map_or(0, |i| i + 1);
                                text[start_..].chars().take_while(|c| *c == ' ' || *c == '\t').collect()
                            };
                            let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
                            let in_tests = std::path::Path::new(&file).components().any(|c| c.as_os_str() == "test");
                            let finals = std::path::Path::new(&file).parent().is_some_and(|_| {
                                let path = std::path::Path::new(&file);
                                let root = crate::project::project_root(path);
                                let options = dartforge_parity::filtros::Opcoes::de_subpasta(path, &root).unwrap_or_else(|| root.join("analysis_options.yaml"));
                                dartforge_parity::filtros::Opcoes::ler_arquivo(&options).regras.get("prefer_final_locals").copied().unwrap_or(false)
                            });
                            let mut matcher = crate::fuzzy_matcher::Matcher::new(prefix, crate::fuzzy_matcher::Style::Text);
                            for (pre, label, doc, body) in crate::completion_item::snippets(context, &indent, eol, in_tests, finals) {
                                if matcher.score(pre) <= 0.0 {
                                    continue;
                                }
                                let v = crate::completion_item::snippet_item(cap, &request, pre, label, doc, &body);
                                let filter = v.get("filterText").or_else(|| v.get("label")).and_then(Value::as_str).unwrap_or("").to_string();
                                if matcher.score(&filter) <= 0.0 {
                                    continue;
                                }
                                items.push(v);
                            }
                        } else {
                            incomplete |= false;
                        }
                    }
                    let mut list = json!({"isIncomplete": incomplete, "items": items});
                    if !patterns.is_empty() {
                        list["itemDefaults"] = Value::Object(patterns);
                    }
                    Some(list)
                });
                response(&id, result.unwrap_or(Value::Null))
            }
            "completionItem/resolve" => {
                let mut item = message.get("params").cloned().unwrap_or(Value::Null);
                // `resolveDartCompletion`: only an item with `data` (not imported): the
                // import, the documentation and the `detail`.
                let key = item.pointer("/data/ref").and_then(Value::as_str).map(str::to_string);
                if let Some(key) = key
                    && let Some((origin, import, detail)) = self.not_imported.get(&key).cloned()
                {
                    let file = item.pointer("/data/file").and_then(Value::as_str).unwrap_or("").to_string();
                    let uri_doc = url::Url::from_file_path(&file).map(|x| x.to_string()).unwrap_or_default();
                    if let Some(imp) = &import
                        && let (Some(text), Some(table)) = (self.documents.get(&uri_doc), self.documents.lines(&uri_doc))
                    {
                        item["additionalTextEdits"] = json!([{"range": lsp_range(text, table, imp.span.start, imp.span.end), "newText": imp.text}]);
                        let displayed = crate::completion_item::display_uri(&imp.uri, &file);
                        let d = format!("Auto import from '{displayed}'\n\n{}", detail.unwrap_or_default());
                        item["detail"] = json!(d.trim());
                    }
                    if let Some((path, start)) = origin {
                        let source = url::Url::from_file_path(&path)
                            .ok()
                            .and_then(|x| self.documents.get(x.as_str()).map(str::to_string))
                            .or_else(|| std::fs::read_to_string(&path).ok());
                        if let Some(doc) = source.as_deref().filter(|f| start <= f.len() && f.is_char_boundary(start)).and_then(|f| crate::dartdoc::documentation(f, start)) {
                            item["documentation"] = if self.documentation_markdown { json!({"kind": "markdown", "value": doc}) } else { json!(doc) };
                        }
                    }
                }
                response(&id, item)
            }
            "textDocument/codeAction" => {
                let params = message.get("params");
                let uri = params.and_then(|p| p.pointer("/textDocument/uri")).and_then(Value::as_str).map(str::to_string);
                let range2 = params.and_then(|p| p.get("range")).and_then(read_range);
                let (Some(u), Some((from, until))) = (uri, range2) else {
                    return response(&id, Value::Null);
                };
                let (Some(text), Some(table)) = (self.documents.get(&u), self.documents.lines(&u)) else {
                    return response(&id, Value::Null);
                };
                let start = table.position_offset(text, from.line, from.column);
                let end = table.position_offset(text, until.line, until.column);
                let (start, end) = (start.min(end), start.max(end));
                let only: Option<Vec<String>> = params
                    .and_then(|p| p.pointer("/context/only"))
                    .and_then(Value::as_array)
                    .map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect());
                // Only the typed ones published for the current version.
                let empty = Vec::new();
                let published = match self.typed_published.get(&u) {
                    Some((v, d)) if Some(*v) == self.documents.version(&u) => d,
                    _ => &empty,
                };
                let actions = self.analyzer.actions(&self.documents, &u, start, end, published);
                let mut output = Vec::new();
                // The source actions (§13.12.1): before all the others, as commands, always
                // offered (the computation is left to `executeCommand`), and only to a
                // client that accepts `workspace/applyEdit`.
                let requested_sources = only.as_ref().is_none_or(|l| l.iter().any(|w| w == "source" || w.starts_with("source.")));
                let path = url::Url::parse(&u).ok().filter(|x| x.scheme() == "file").and_then(|x| x.to_file_path().ok());
                if let (true, true, true, Some(path)) = (requested_sources, self.apply_edits, u.ends_with(".dart"), path) {
                    let mut argument = json!({"path": path.to_string_lossy()});
                    if params.and_then(|p| p.pointer("/context/triggerKind")).and_then(Value::as_u64) == Some(2) {
                        argument["autoTriggered"] = json!(true);
                    }
                    for (title, kind, command) in SOURCE_ACTIONS {
                        let matches_ = |w: &String| *kind == w.as_str() || kind.starts_with(&format!("{w}."));
                        let included = match (&only, &self.action_kinds) {
                            (Some(l), _) => l.iter().any(matches_),
                            (None, Some(l)) if self.actions_literals => l.iter().any(matches_),
                            _ => true,
                        };
                        if !included {
                            continue;
                        }
                        let c = json!({"title": title, "command": command, "arguments": [argument.clone()]});
                        output.push(if self.actions_literals { json!({"title": title, "kind": kind, "command": c}) } else { c });
                    }
                }
                let (mut fixes, mut assists, mut refactorings): (Vec<Value>, Vec<Value>, Vec<Value>) =
                    (Vec::new(), Vec::new(), Vec::new());
                for action in actions {
                    // The analyzer's source actions (the ready edit of `Organize Imports`) give
                    // way to the commands above.
                    if action.kind.starts_with("source") {
                        continue;
                    }
                    let allowed = only.as_ref().is_none_or(|l| {
                        l.iter().any(|k| action.kind == *k || action.kind.starts_with(&format!("{k}.")))
                    });
                    if !allowed {
                        continue;
                    }
                    let edit = match &action.create_file {
                        // Only to a client that accepts creating files: the capability is the whole
                        // edit or nothing.
                        Some(_) if !self.create_files => continue,
                        Some((new, content)) => {
                            let mut e = self.workspace_edit(&action.edits, Some(json!({"kind": "create", "uri": new})));
                            if !content.is_empty()
                                && let Some(l) = e["documentChanges"].as_array_mut()
                            {
                                l.push(json!({
                                    "textDocument": {"uri": new, "version": null},
                                    "edits": [{"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}, "newText": content}],
                                }));
                            }
                            e
                        }
                        None => match self.edit_snippet(&u, &action) {
                            Some(e) => e,
                            None => self.workspace_edit(&action.edits, None),
                        },
                    };
                    // The Dart server's form (§13.7.1 item 8 and §13.8.1): `diagnostics` (the
                    // error's for a fix, empty for an assist), the logging command with the
                    // change's original id, and never `isPreferred`. The two legacy
                    // refactorings (`Inline`/`Extract Local Variable`) are not assists and stay
                    // as they were.
                    // The `Inline invocation of '…'` assist (`dart.assist.inline`) has the same
                    // kind and is an assist.
                    let refactoring = matches!(action.kind.as_str(), "refactor.inline" | "refactor.extract")
                        && !action.title.starts_with("Inline invocation of");
                    let mut value = json!({"title": action.title, "kind": action.kind, "edit": edit});
                    if let (Some(d), Some(text), Some(table)) =
                        (&action.diagnostic, self.documents.get(&u), self.documents.lines(&u))
                    {
                        value["diagnostics"] = json!([convert_diagnostic(text, table, d)]);
                    } else if !refactoring {
                        value["diagnostics"] = json!([]);
                    }
                    if !refactoring && let Some(action_id) = action_id(&action.kind) {
                        value["command"] = json!({"title": "Log Action", "command": "dart.logAction", "arguments": [{"action": action_id}]});
                    }
                    if refactoring {
                        refactorings.push(value);
                    } else if action.kind.starts_with("quickfix") {
                        fixes.push(value);
                    } else {
                        assists.push(value);
                    }
                }
                // The source actions, the fixes, the assists and the refactorings, in this
                // order; fixes and assists pass each group through `_CodeActionSorter`.
                let request_column = i64::from(from.column);
                // `getFixActions`/`getAssistActions`: only as literal `CodeAction`s; without
                // `codeActionLiteralSupport`, none.
                if !self.actions_literals {
                    fixes.clear();
                    assists.clear();
                }
                output.extend(sort_actions(fixes, request_column));
                output.extend(sort_actions(assists, request_column));
                output.extend(refactorings);
                output.extend(self.refactorings_by_command(&u, start, end, only.as_deref()));
                response(&id, json!(output))
            }
            "workspace/executeCommand" => self.run_command(&id, message),
            "textDocument/prepareRename" => {
                let Some((u, offset)) = self.request_position(message) else {
                    return response(&id, Value::Null);
                };
                match self.analyzer.prepare_rename(&self.documents, &u, offset) {
                    Ok(Some((span, text))) => {
                        let range = self.range(&u, span);
                        response(&id, range.map_or(Value::Null, |r| json!({"range": r, "placeholder": text})))
                    }
                    Ok(None) => response(&id, Value::Null),
                    Err(reason) => error(&id, RENAME_INVALID, reason),
                }
            }
            "textDocument/rename" => {
                let new = message.pointer("/params/newName").and_then(Value::as_str).unwrap_or("").to_string();
                let Some((u, offset)) = self.request_position(message) else {
                    return response(&id, Value::Null);
                };
                // The version at the start of the request (`extractDocumentVersion`).
                let version = self.documents.version(&u);
                match self.analyzer.rename(&self.documents, &u, offset, &new) {
                    Ok(rename) if rename.null => response(&id, Value::Null),
                    Ok(rename) => {
                        let pending = PendingRename {
                            id: id.clone(),
                            uri: u,
                            version,
                            edits: rename.edits,
                            file: rename.file,
                            file_question: false,
                        };
                        match rename.warning {
                            // ERROR/WARNING from `checkFinalConditions`: without `userPromptSender`,
                            // `-32010`; with it, the question.
                            Some(warning) if !self.user_questions => error(&id, RENAME_INVALID, warning),
                            Some(warning) => self.ask_on_rename(
                                pending,
                                json!({"type": 2, "message": warning, "actions": [{"title": "Rename Anyway"}, {"title": "Cancel"}]}),
                            ),
                            None => self.finish_rename(pending),
                        }
                    }
                    Err(reason) => error(&id, RENAME_INVALID, reason),
                }
            }
            "textDocument/signatureHelp" => {
                let Some((u, offset)) = self.request_position(message) else {
                    return response(&id, Value::Null);
                };
                // Automatic trigger (`(` typed, help closed): only if the `(` opens the
                // list, like the Dart server.
                let auto = message.pointer("/params/context/triggerKind").and_then(Value::as_u64) == Some(2)
                    && message.pointer("/params/context/isRetrigger").and_then(Value::as_bool) == Some(false);
                let Some(a) = self.analyzer.signature(&self.documents, &u, offset, auto) else {
                    return response(&id, Value::Null);
                };
                let mut signature = json!({
                    "label": a.label,
                    "parameters": a.parameters.iter().map(|p| json!({"label": p})).collect::<Vec<_>>(),
                });
                if let Some(d) = &a.documentation {
                    signature["documentation"] =
                        if self.signature_markdown { json!({"kind": "markdown", "value": d}) } else { json!(d) };
                }
                // No active parameter and no `null` in the client: an index outside the
                // list (VS Code highlights none).
                let active = match a.active {
                    Some(i) => json!(i),
                    None if self.signature_without_active => Value::Null,
                    None => json!(a.parameters.len()),
                };
                response(&id, json!({"signatures": [signature], "activeSignature": 0, "activeParameter": active}))
            }
            "textDocument/documentHighlight" => {
                let Some((u, offset)) = self.request_position(message) else {
                    return response(&id, Value::Null);
                };
                let document_highlights = self.analyzer.document_highlights(&self.documents, &u, offset).unwrap_or_default();
                let items: Vec<Value> = document_highlights.into_iter().filter_map(|s| Some(json!({"range": self.range(&u, s)?}))).collect();
                // Dart answers `[]`, never `null` (§11.6).
                response(&id, json!(items))
            }
            "textDocument/implementation" => {
                let Some((u, offset)) = self.request_position(message) else {
                    return response(&id, json!([]));
                };
                let locals: Vec<Value> = self
                    .analyzer
                    .implementations(&self.documents, &u, offset)
                    .into_iter()
                    .filter_map(|(target, s)| Some(json!({"uri": target, "range": self.range(&target, s)?})))
                    .collect();
                response(&id, json!(locals))
            }
            // `workspace/willRenameFiles` (`MoveFileRefactoringImpl.multi`,
            // `crate::move_file`): the URIs of the directives that reach the moved
            // files and the relative ones that leave them; `null` on a fatal condition.
            "workspace/willRenameFiles" => {
                let mut map = Vec::new();
                for f in message.pointer("/params/files").and_then(Value::as_array).cloned().unwrap_or_default() {
                    let path = |k: &str| f.get(k).and_then(Value::as_str).and_then(|u| url::Url::parse(u).ok()).and_then(|u| u.to_file_path().ok());
                    let (Some(old), Some(new)) = (path("oldUri"), path("newUri")) else {
                        return error(&id, -32602, "invalid file URI in willRenameFiles");
                    };
                    map.push((old, new));
                }
                let mut roots = self.roots.clone();
                for u in self.documents.uris() {
                    if let Some(p) = url::Url::parse(u).ok().and_then(|x| x.to_file_path().ok()) {
                        let r = crate::project::project_root(&p);
                        if r.join("pubspec.yaml").is_file() && !roots.contains(&r) {
                            roots.push(r);
                        }
                    }
                }
                match crate::move_file::edits(&roots, &self.documents, &map) {
                    Some(edits) => response(&id, self.workspace_edit(&edits, None)),
                    None => response(&id, Value::Null),
                }
            }
            // `dart/textDocument/super` (Dart-Code's "go to super").
            "dart/textDocument/super" => {
                let Some((u, offset)) = self.request_position(message) else {
                    return response(&id, Value::Null);
                };
                let local = self
                    .analyzer
                    .upper(&self.documents, &u, offset)
                    .and_then(|(target, s)| Some(json!({"uri": target, "range": self.range(&target, s)?})));
                response(&id, local.unwrap_or(Value::Null))
            }
            "textDocument/typeDefinition" => {
                let Some((u, offset)) = self.request_position(message) else {
                    return response(&id, json!([]));
                };
                let local = self
                    .analyzer
                    .type_definition(&self.documents, &u, offset)
                    .and_then(|(target, s)| Some(json!({"uri": target, "range": self.range(&target, s)?})));
                response(&id, local.unwrap_or_else(|| json!([])))
            }
            // `handler_document_link.dart:26-53` (docs/LSP-ESPECIFICACAO.md §8.4): in
            // 3.6.2 the visitor only links Flutter's "See code in examples/api/…"
            // comments; directive URIs do not become links (the oracle returns `[]` on
            // the 36 files, all with imports). Outside Flutter the response is always
            // the empty list.
            "textDocument/formatting" | "textDocument/rangeFormatting" | "textDocument/onTypeFormatting" => {
                // `FormattingHandler`, `FormatRangeHandler` and `FormatOnTypeHandler`: only
                // Dart files, nothing with `enableSdkFormatter: false`, the resource's
                // `lineLength`; `onTypeFormatting` formats the whole document.
                let uri = message.pointer("/params/textDocument/uri").and_then(Value::as_str).unwrap_or_default().to_string();
                if !uri.ends_with(".dart") {
                    return response(&id, Value::Null);
                }
                let configuration = self.configuration_of(&uri);
                if !configuration.enable_sdk_formatter {
                    return response(&id, Value::Null);
                }
                let width = configuration.line_length;
                let Some(text) = self.file_text(&uri).map(|t| t.into_owned()) else {
                    return response(&id, Value::Null);
                };
                let range_ = if method == "textDocument/rangeFormatting" {
                    // `toOffset`: a line past the end is an error; the column is added to the
                    // line start without a limit (it may go past the line break).
                    let mut points = [0usize; 2];
                    for (i, p) in ["start", "end"].into_iter().enumerate() {
                        let l = message.pointer(&format!("/params/range/{p}/line")).and_then(Value::as_u64).unwrap_or(0);
                        let c = message.pointer(&format!("/params/range/{p}/character")).and_then(Value::as_u64).unwrap_or(0);
                        match crate::formatting::offset_without_limit(&text, l, c) {
                            Some(o) => points[i] = o,
                            None => return error_with_data(&id, -32004, "Invalid line number", json!(l.to_string())),
                        }
                    }
                    Some((points[0], points[1]))
                } else {
                    None
                };
                response(&id, crate::formatting::edits(&text, width, range_))
            }
            "textDocument/documentLink" => {
                // The `{@tool}` blocks that cite the example code (`crate::links`, the 3.6.2
                // visitor).
                let uri = message.pointer("/params/textDocument/uri").and_then(Value::as_str).unwrap_or_default().to_string();
                let path = url::Url::parse(&uri).ok().and_then(|u| u.to_file_path().ok());
                let (Some(text), Some(path)) = (self.documents.get(&uri).map(str::to_string), path) else {
                    return response(&id, json!([]));
                };
                if path.extension().is_none_or(|e| e != "dart") {
                    return response(&id, json!([]));
                }
                let links: Vec<Value> = crate::links::links(&text, &path)
                    .into_iter()
                    .filter_map(|(s, target)| {
                        let target_ = url::Url::from_file_path(&target).ok()?;
                        Some(json!({"range": self.range(&uri, s)?, "target": target_.as_str()}))
                    })
                    .collect();
                response(&id, json!(links))
            }
            // `handler_code_lens.dart:32-61` (§8.3): there are only augmentation
            // lenses, and only for a client that declares the `dart.goToLocation`
            // command; without augmentations, an empty list.
            "textDocument/codeLens" => {
                let uri = message.pointer("/params/textDocument/uri").and_then(Value::as_str).unwrap_or_default().to_string();
                let (augmentations, augmented) = (self.configuration.code_lens_augmentation, self.configuration.code_lens_augmented);
                if !uri.ends_with(".dart") || !(augmentations || augmented) || !self.client_commands.iter().any(|c| c == "dart.goToLocation") {
                    return response(&id, json!([]));
                }
                let lenses: Vec<Value> = self
                    .analyzer
                    .augmentation_lenses(&self.documents, &uri, augmented, augmentations)
                    .into_iter()
                    .filter_map(|(s, title, target, s_target)| {
                        let local = json!({"uri": target, "range": self.range(&target, s_target)?});
                        Some(json!({"range": self.range(&uri, s)?, "command": {"title": title, "command": "dart.goToLocation", "arguments": [local]}}))
                    })
                    .collect();
                response(&id, json!(lenses))
            }
            // `textDocument/documentColor` (the `ColorComputer`, §8.2): the colors from
            // 0 to 1.
            "textDocument/documentColor" => {
                let uri = message.pointer("/params/textDocument/uri").and_then(Value::as_str).unwrap_or_default().to_string();
                if !uri.ends_with(".dart") {
                    return response(&id, json!([]));
                }
                let colors: Vec<Value> = self
                    .analyzer
                    .colors(&self.documents, &uri)
                    .into_iter()
                    .filter_map(|(s, [a, r, g, b])| {
                        let f = |x: u8| f64::from(x) / 255.0;
                        Some(json!({"range": self.range(&uri, s)?, "color": {"alpha": f(a), "red": f(r), "green": f(g), "blue": f(b)}}))
                    })
                    .collect();
                response(&id, json!(colors))
            }
            // `textDocument/colorPresentation`: the four forms, with `const` when the
            // expression is constant outside a constant context; nothing outside the
            // analysis roots or without Flutter's `Color`.
            "textDocument/colorPresentation" => {
                let uri = message.pointer("/params/textDocument/uri").and_then(Value::as_str).unwrap_or_default().to_string();
                let color = message.pointer("/params/color").cloned().unwrap_or(Value::Null);
                let reach = message.pointer("/params/range").cloned().unwrap_or(Value::Null);
                let n = |k: &str| color.get(k).and_then(Value::as_f64).unwrap_or(0.0);
                let inside = url::Url::parse(&uri).ok().and_then(|u| u.to_file_path().ok()).is_some_and(|c| self.roots.iter().any(|r| c.starts_with(r)));
                if !uri.ends_with(".dart") || !inside || self.excluded(&uri) {
                    return response(&id, json!([]));
                }
                let start = reach
                    .pointer("/start")
                    .and_then(|p| Some((p.get("line")?.as_u64()? as u32, p.get("character")?.as_u64()? as u32)))
                    .and_then(|(l, c)| Some(self.documents.lines(&uri)?.position_offset(self.documents.get(&uri)?, l, c)));
                let Some(start) = start else { return response(&id, json!([])) };
                let Some(with_const) = self.analyzer.color_presentation(&self.documents, &uri, start) else {
                    return response(&id, json!([]));
                };
                let list: Vec<Value> = crate::colors::presentations(n("alpha"), n("red"), n("green"), n("blue"), with_const)
                    .into_iter()
                    .map(|(label, text)| json!({"label": label, "textEdit": {"range": reach, "newText": text}}))
                    .collect();
                response(&id, json!(list))
            }
            // `dart/textDocument/augmented` and `augmentation` (§8.6).
            "dart/textDocument/augmented" | "dart/textDocument/augmentation" => {
                let Some((u, offset)) = self.request_position(message) else {
                    return response(&id, Value::Null);
                };
                let next = method == "dart/textDocument/augmentation";
                let local = self
                    .analyzer
                    .augmentation_neighbor(&self.documents, &u, offset, next)
                    .and_then(|(target, s)| Some(json!({"uri": target, "range": self.range(&target, s)?})));
                response(&id, local.unwrap_or(Value::Null))
            }
            "textDocument/foldingRange" => {
                let uri = message.pointer("/params/textDocument/uri").and_then(Value::as_str).map(str::to_string);
                let Some((u, text)) = uri.and_then(|u| Some((u.clone(), self.documents.get(&u)?.to_string()))) else {
                    return response(&id, Value::Null);
                };
                let lines_only = self.folds_line_only;
                let folds: Vec<Value> = self
                    .analyzer
                    .folds(&u, &text, lines_only)
                    .into_iter()
                    .map(|d| {
                        let mut v = json!({"startLine": d.line_begin, "endLine": d.line_end});
                        if !lines_only {
                            v["startCharacter"] = json!(d.column_start);
                            v["endCharacter"] = json!(d.column_end);
                        }
                        if let Some(k) = d.kind {
                            v["kind"] = json!(k);
                        }
                        v
                    })
                    .collect();
                response(&id, json!(folds))
            }
            "textDocument/selectionRange" => {
                let uri = message.pointer("/params/textDocument/uri").and_then(Value::as_str).map(str::to_string);
                let Some((u, text)) = uri.and_then(|u| Some((u.clone(), self.documents.get(&u)?.to_string()))) else {
                    return response(&id, Value::Null);
                };
                let positions: Vec<Position> = message
                    .pointer("/params/positions")
                    .and_then(Value::as_array)
                    .map(|l| l.iter().filter_map(read_position).collect())
                    .unwrap_or_default();
                let mut output = Vec::new();
                for p in positions {
                    let Some(offset) = self.documents.lines(&u).map(|t| t.position_offset(&text, p.line, p.column)) else {
                        break;
                    };
                    let spans = self.analyzer.selections(&u, &text, offset);
                    let Some(table) = self.documents.lines(&u) else { break };
                    // From the outermost to the innermost, each one the parent of the next.
                    let mut current: Option<Value> = None;
                    for s in spans.iter().rev() {
                        let mut v = json!({"range": lsp_range(&text, table, s.start, s.end)});
                        if let Some(parent) = current.take() {
                            v["parent"] = parent;
                        }
                        current = Some(v);
                    }
                    output.push(current.unwrap_or_else(|| json!({"range": lsp_range(&text, table, offset, offset)})));
                }
                response(&id, json!(output))
            }
            "textDocument/semanticTokens/full" | "textDocument/semanticTokens/range" => {
                let uri = message.pointer("/params/textDocument/uri").and_then(Value::as_str).map(str::to_string);
                let Some(u) = uri.filter(|u| self.documents.get(u).is_some()) else {
                    return response(&id, Value::Null);
                };
                let range_ = message.pointer("/params/range").and_then(read_range).and_then(|(from, until)| {
                    let text = self.documents.get(&u)?;
                    let table = self.documents.lines(&u)?;
                    Some((table.position_offset(text, from.line, from.column), table.position_offset(text, until.line, until.column)))
                });
                let multiline = self.multiline_tokens;
                match self.analyzer.semantic_tokens(&self.documents, &u, multiline, range_) {
                    Some(data) => response(&id, json!({"data": data})),
                    None => response(&id, Value::Null),
                }
            }
            "textDocument/inlayHint" => {
                let uri = message.pointer("/params/textDocument/uri").and_then(Value::as_str).map(str::to_string);
                let Some(u) = uri.filter(|u| self.documents.get(u).is_some()) else {
                    return response(&id, json!([]));
                };
                let hints = self.analyzer.hints(&self.documents, &u);
                let (Some(text), Some(table)) = (self.documents.get(&u), self.documents.lines(&u)) else {
                    return response(&id, json!([]));
                };
                let items: Vec<Value> = hints
                    .into_iter()
                    .map(|d| {
                        let (l, c) = table.offset_position(text, d.offset);
                        let mut v = json!({
                            "position": {"line": l, "character": c},
                            "label": [{"value": d.label}],
                            "kind": d.kind,
                        });
                        if d.space_after {
                            v["paddingRight"] = json!(true);
                        }
                        v
                    })
                    .collect();
                response(&id, json!(items))
            }
            "textDocument/prepareCallHierarchy" => {
                let Some((u, offset)) = self.request_position(message) else {
                    return response(&id, Value::Null);
                };
                let item = self.analyzer.prepare_calls(&self.documents, &u, offset);
                let value = item.and_then(|i| self.call_item(&i));
                response(&id, value.map_or(Value::Null, |v| json!([v])))
            }
            "callHierarchy/incomingCalls" | "callHierarchy/outgoingCalls" => {
                // `toServerItem`: the file, the name in `selectionRange` and the displayed
                // name of the client's item.
                let item = message.pointer("/params/item");
                let uri = item.and_then(|i| i.get("uri")).and_then(Value::as_str).map(str::to_string);
                let name = item.and_then(|i| i.get("name")).and_then(Value::as_str).unwrap_or("").to_string();
                let constructor = item.and_then(|i| i.get("kind")).and_then(Value::as_u64) == Some(9);
                let start = item.and_then(|i| i.pointer("/selectionRange/start")).cloned();
                let Some(uri) = uri else { return response(&id, json!([])) };
                if !uri.ends_with(".dart") {
                    return response(&id, json!([]));
                }
                let offset = start.and_then(|p| {
                    let line = p.get("line")?.as_u64()? as u32;
                    let column = p.get("character")?.as_u64()? as u32;
                    self.offset_in_file(&uri, line, column)
                });
                let Some(offset) = offset else {
                    return error(&id, MODIFIED_CONTENT, "Content was modified since Call Hierarchy node was produced");
                };
                let received = method == "callHierarchy/incomingCalls";
                let calls = self.analyzer.calls(&self.documents, &uri, offset, &name, constructor, received);
                let mut output = Vec::new();
                for (item, spans) in calls {
                    // Incoming: ranges in the caller's file; outgoing: in the requested item's file.
                    let file = if received { item.uri.clone() } else { uri.clone() };
                    let ranges: Vec<Value> = spans.iter().filter_map(|s| self.range(&file, *s)).collect();
                    let Some(v) = self.call_item(&item) else { continue };
                    output.push(if received { json!({"from": v, "fromRanges": ranges}) } else { json!({"to": v, "fromRanges": ranges}) });
                }
                response(&id, json!(output))
            }
            "textDocument/prepareTypeHierarchy" => {
                // `isDartDocument`: another file answers an empty list.
                let request = message.pointer("/params/textDocument/uri").and_then(Value::as_str).unwrap_or("");
                if !request.ends_with(".dart") {
                    return response(&id, json!([]));
                }
                let Some((u, offset)) = self.request_position(message) else {
                    return response(&id, Value::Null);
                };
                let item = self.analyzer.prepare_hierarchy(&self.documents, &u, offset);
                if item.is_some() {
                    self.hierarchy_origin = Some(u.clone());
                }
                // `toLspItem(target, unit.lineInfo)`: the positions of the class's file
                // converted with the current document's lines.
                let value = item.and_then(|i| self.hierarchy_item(&i, Some(&u)));
                response(&id, value.map_or(Value::Null, |v| json!([v])))
            }
            "typeHierarchy/supertypes" | "typeHierarchy/subtypes" => {
                let item = message.pointer("/params/item");
                let uri = item.and_then(|i| i.get("uri")).and_then(Value::as_str).unwrap_or("").to_string();
                let data = item.and_then(|i| i.get("data")).filter(|d| !d.is_null());
                let Some(reference) = data.and_then(|d| d.get("ref")).and_then(Value::as_str).map(str::to_string) else {
                    return error(&id, -32602, "TypeHierarchyItem is missing the data field");
                };
                let anchor: Option<(String, Vec<usize>)> = data.and_then(|d| d.get("anchor")).filter(|a| !a.is_null()).and_then(|a| {
                    let r = a.get("ref")?.as_str()?.to_string();
                    let path = a.get("path")?.as_array()?.iter().filter_map(Value::as_u64).map(|x| x as usize).collect();
                    Some((r, path))
                });
                let project = self.hierarchy_document(&uri);
                let supertypes = method == "typeHierarchy/supertypes";
                let items = self.analyzer.hierarchy(&self.documents, &project, &reference, anchor.as_ref().map(|(r, c)| (r.as_str(), c.as_slice())), supertypes);
                match items {
                    None => response(&id, Value::Null),
                    // `_convertItems`: each item with its own file's lines; what cannot be read
                    // is left out.
                    Some(list) => response(&id, json!(list.iter().filter_map(|i| self.hierarchy_item(i, None)).collect::<Vec<_>>())),
                }
            }
            SLEEP_METHOD => {
                let ms = message
                    .get("params")
                    .and_then(|p| p.get("ms"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                if sleep_cancellable(ms, &self.cancelled, &key) {
                    self.cancelled.remove(&key);
                    error(&id, REQUEST_CANCELLED, "request cancelled")
                } else {
                    response(&id, Value::Null)
                }
            }
            _ => {
                register(format!("unknown request: {method}"));
                error(
                    &id,
                    METHOD_NOT_FOUND,
                    format!("Unknown method {method}"),
                )
            }
        }
    }

    /// A `CallHierarchyItem` (`toLspItem`): the kind through
    /// `toSymbolKindMapping` with the fallback (`File` → `Module`; another one
    /// not announced or unknown → `Object`), without `data`.
    fn call_item(&self, item: &crate::CallItem) -> Option<Value> {
        let accepted: Vec<u64> = self.symbol_kinds.clone().unwrap_or_else(|| (1..=18).collect());
        let kind = match item.kind.symbol() {
            Some(k) if accepted.contains(&u64::from(k)) => u64::from(k),
            Some(1) => 2,
            _ => 19,
        };
        let mut v = json!({
            "name": item.name,
            "kind": kind,
            "uri": item.uri,
            "range": self.range(&item.uri, item.range)?,
            "selectionRange": self.range(&item.uri, item.selection)?,
        });
        if let Some(d) = &item.detail {
            v["detail"] = json!(d);
        }
        Some(v)
    }

    /// The offset (bytes) of an LSP position of `uri` (open or on disk).
    fn offset_in_file(&self, uri: &str, line: u32, column: u32) -> Option<usize> {
        if let (Some(text), Some(table)) = (self.documents.get(uri), self.documents.lines(uri)) {
            return Some(table.position_offset(text, line, column));
        }
        let path = url::Url::parse(uri).ok()?.to_file_path().ok()?;
        let source = std::fs::read_to_string(path).ok()?;
        let table = crate::utf16::LineTable::build(&source);
        Some(table.position_offset(&source, line, column))
    }

    /// A `TypeHierarchyItem` (`toLspItem`): kind 5 (class), `data` with the
    /// `ElementLocation` (`ref`) and the anchor. With `lines_of`, the ranges are
    /// converted with that document's lines.
    fn hierarchy_item(&self, item: &crate::TypeItem, lines_of: Option<&str>) -> Option<Value> {
        let (range, selection) = match lines_of {
            Some(origin) => (self.range_with_lines_of(&item.uri, origin, item.range)?, self.range_with_lines_of(&item.uri, origin, item.selection)?),
            None => (self.range(&item.uri, item.range)?, self.range(&item.uri, item.selection)?),
        };
        let mut data = json!({"ref": item.reference});
        if let Some((r, path)) = &item.anchor {
            data["anchor"] = json!({"ref": r, "path": path});
        }
        Some(json!({
            "name": item.name,
            "kind": 5,
            "uri": item.uri,
            "range": range,
            "selectionRange": selection,
            "data": data,
        }))
    }

    /// The document whose project serves `typeHierarchy/supertypes` and
    /// `subtypes` of the item at `uri`: the file itself when it is in a
    /// workspace root; otherwise (SDK, package) that of the last
    /// `prepareTypeHierarchy`.
    fn hierarchy_document(&self, uri: &str) -> String {
        let in_workspace = url::Url::parse(uri)
            .ok()
            .and_then(|u| u.to_file_path().ok())
            .is_some_and(|c| self.roots.iter().any(|r| c.starts_with(r)));
        match &self.hierarchy_origin {
            Some(origin) if !in_workspace => origin.clone(),
            _ => uri.to_string(),
        }
    }

    /// The text of `uri`: the open one, otherwise the disk's.
    fn file_text(&self, uri: &str) -> Option<std::borrow::Cow<'_, str>> {
        if let Some(text) = self.documents.get(uri) {
            return Some(std::borrow::Cow::Borrowed(text));
        }
        let path = url::Url::parse(uri).ok()?.to_file_path().ok()?;
        std::fs::read_to_string(path).ok().map(std::borrow::Cow::Owned)
    }

    /// A range of `file` converted with the `LineInfo` of `origin`
    /// (`sourceRangeToRange(unit.lineInfo, …)` of `prepareTypeHierarchy`): the
    /// UTF-16 offsets of the class's file, the line from the last line start of
    /// `origin` that does not exceed them, and the column without a limit.
    fn range_with_lines_of(&self, file: &str, origin: &str, span: dartforge_diagnostics::Span) -> Option<Value> {
        if file == origin {
            return self.range(file, span);
        }
        let text = self.file_text(file)?;
        let target = self.file_text(origin)?;
        let starts = utf16_starts(&target);
        let position = |byte: usize| {
            let mut b = byte.min(text.len());
            while !text.is_char_boundary(b) {
                b -= 1;
            }
            let o = text[..b].encode_utf16().count();
            let line = starts.partition_point(|&s| s <= o).saturating_sub(1);
            json!({"line": line, "character": o - starts[line]})
        };
        Some(json!({"start": position(span.start), "end": position(span.end)}))
    }

    /// `workspace/executeCommand` (docs/LSP-ESPECIFICACAO.md §13.12.2 to
    /// §13.12.6). Returns the response, or, when the command has an edit to
    /// apply, the `workspace/applyEdit` request: the `executeCommand` response
    /// goes out when the client answers ([`Server::client_response`]).
    /// The `codeAction` refactorings (§13.11.1): after the assists, in Dart's
    /// order, as commands; only with `workspace/applyEdit` and with `refactor`
    /// requested. The legacy ones go through `shouldIncludeKind`; that of the
    /// `RefactoringProcessor` (Move) does not.
    fn refactorings_by_command(&mut self, u: &str, start: usize, end: usize, only: Option<&[String]>) -> Vec<Value> {
        let requested = only.is_none_or(|l| l.iter().any(|w| w == "refactor" || w.starts_with("refactor.")));
        if !requested || !self.apply_edits || !u.ends_with(".dart") {
            return Vec::new();
        }
        let Some(path) = url::Url::parse(u).ok().filter(|x| x.scheme() == "file").and_then(|x| x.to_file_path().ok()) else {
            return Vec::new();
        };
        let path = path.to_string_lossy().into_owned();
        let list = self.analyzer.refactorings(&self.documents, u, start, end - start, self.create_files);
        let Some(text) = self.documents.get(u) else { return Vec::new() };
        let o16 = text[..start.min(text.len())].encode_utf16().count();
        let l16 = text[start.min(text.len())..end.min(text.len())].encode_utf16().count();
        let version = self.documents.version(u);
        let mut output = Vec::new();
        for r in list {
            match r.command {
                crate::RefactoringCommand::Move { path_pattern } => {
                    let pattern = url::Url::from_file_path(&path_pattern).map(|x| x.to_string()).unwrap_or_default();
                    let c = json!({
                        "title": r.title,
                        "command": "dart.refactor.move_top_level_to_file",
                        "arguments": [{"filePath": path, "selectionOffset": o16, "selectionLength": l16, "arguments": [pattern]}],
                    });
                    output.push(json!({
                        "title": r.title,
                        "kind": r.kind,
                        "command": c,
                        "data": {"parameters": [{
                            "actionLabel": "Move",
                            "defaultValue": pattern,
                            "filters": {"Dart": ["dart"]},
                            "kind": "saveUri",
                            "parameterLabel": "Move to:",
                            "parameterTitle": "Select a file to move to",
                        }]},
                    }));
                }
                crate::RefactoringCommand::Legacy(kind) => {
                    let matches = |w: &String| r.kind == w.as_str() || r.kind.starts_with(&format!("{w}."));
                    let included = match (only, &self.action_kinds) {
                        (Some(l), _) => l.iter().any(matches),
                        (None, Some(l)) if self.actions_literals => l.iter().any(matches),
                        _ => true,
                    };
                    if !included {
                        continue;
                    }
                    let c = json!({
                        "title": r.title,
                        "command": "refactor.perform",
                        "arguments": [kind, path, version, o16, l16, null],
                    });
                    output.push(if self.actions_literals { json!({"title": r.title, "kind": r.kind, "command": c}) } else { c });
                }
            }
        }
        output
    }

    /// The current text of the file (the open one, otherwise the disk's).
    fn current_text(&self, uri: &str, path: &str) -> Option<String> {
        self.documents.get(uri).map(str::to_string).or_else(|| std::fs::read_to_string(path).ok())
    }

    /// `refactor.perform`/`refactor.validate` (§13.11.2).
    fn run_legacy_refactoring(&mut self, id: &Value, message: &Value, only_validate: bool) -> Value {
        let name = if only_validate { "Validate Refactor" } else { "Perform Refactor" };
        let invalid = || {
            error(
                id,
                INVALID_COMMAND_ARGUMENTS,
                format!(
                    "{name} requires 6 parameters: kind: String (RefactoringKind), filePath: String, docVersion: int?, offset: int, length: int, options: Map<String, Object?>"
                ),
            )
        };
        // `parseArgList`: positional, 6 items; any other shape becomes `{}`.
        let arguments = message.pointer("/params/arguments").and_then(Value::as_array).cloned().unwrap_or_default();
        if arguments.len() != 6 {
            return invalid();
        }
        let (Some(kind), Some(path)) = (arguments[0].as_str(), arguments[1].as_str()) else { return invalid() };
        let version = match &arguments[2] {
            Value::Null => None,
            v => match v.as_i64() {
                Some(n) => Some(n),
                None => return invalid(),
            },
        };
        let (Some(offset), Some(length)) = (arguments[3].as_i64(), arguments[4].as_i64()) else { return invalid() };
        let options = match &arguments[5] {
            Value::Null => None,
            Value::Object(m) => Some(m.clone()),
            _ => return invalid(),
        };
        // `requireResolvedUnit(path)`.
        let uri = url::Url::from_file_path(path).ok().map(|u| u.to_string());
        let text = uri.as_deref().and_then(|u| self.current_text(u, path));
        let (Some(uri), Some(text)) = (uri, text) else {
            return error(id, FILE_NOT_ANALYZED, "File is not being analyzed");
        };
        if !path.ends_with(".dart") {
            return error(id, FILE_NOT_ANALYZED, "File is not being analyzed");
        }
        if !REFACTORING_TYPES.contains(&kind) {
            return error(id, ERROR_NOT_HANDLED, format!("Exception: Illegal enum value: {kind}"));
        }
        if matches!(kind, "MOVE_FILE" | "RENAME") {
            return error(id, INVALID_COMMAND_ARGUMENTS, format!("Unknown RefactoringKind RefactoringKind.{kind} was supplied to {name}"));
        }
        if offset < 0 || length < 0 {
            return error(id, ERROR_NOT_HANDLED, format!("RangeError: offset {offset}, length {length}"));
        }
        let start = byte_from_utf16(&text, offset as usize);
        let end = byte_from_utf16(&text, (offset + length) as usize);
        let request = crate::RefactoringRequest { kind: kind.to_string(), offset: start, length: end - start, options, only_validate };
        let result = self.analyzer.run_refactoring(&self.documents, &uri, &request);
        self.refactoring_response(id, name, result, if only_validate { None } else { Some((uri, version)) })
    }

    /// The response of a computed refactoring. `versioned`: the file and the
    /// requested version, for the `ContentModified` of `refactor.perform`.
    fn refactoring_response(
        &mut self,
        id: &Value,
        label: &'static str,
        result: crate::RefactoringResult,
        versioned: Option<(String, Option<i64>)>,
    ) -> Value {
        use crate::RefactoringResult as R;
        let validate = label == "Validate Refactor";
        match result {
            R::NotAnalyzed => error(id, FILE_NOT_ANALYZED, "File is not being analyzed"),
            R::InvalidArguments(m) => error(id, INVALID_COMMAND_ARGUMENTS, m),
            R::InternalError(m) => error(id, ERROR_NOT_HANDLED, m),
            R::Failure(reason) => error(id, COMPUTATION_FAILURE, reason.unwrap_or_else(|| "Cannot compute the change. No details.".to_string())),
            R::Error(m) if validate => response(id, json!({"valid": false, "message": m})),
            R::Error(m) => {
                // `showErrorMessageToUser` and a `null` success.
                self.pending_outputs.push(json!({
                    "jsonrpc": "2.0",
                    "method": "window/showMessage",
                    "params": {"type": 1, "message": m},
                }));
                response(id, Value::Null)
            }
            R::Valid => response(id, if validate { json!({"valid": true}) } else { Value::Null }),
            R::Change { .. } if validate => response(id, json!({"valid": true})),
            R::Change { edits, create } => {
                if edits.is_empty() && create.is_none() {
                    return response(id, Value::Null);
                }
                if let Some((uri, Some(v))) = &versioned
                    && Some(*v) != self.documents.version(uri).map(i64::from)
                {
                    return error(id, MODIFIED_CONTENT, "Document was modified before operation completed");
                }
                let edit = match create {
                    Some((new, content)) => {
                        let mut e = self.workspace_edit(&edits, Some(json!({"kind": "create", "uri": new})));
                        if !content.is_empty()
                            && let Some(l) = e["documentChanges"].as_array_mut()
                        {
                            l.push(json!({
                                "textDocument": {"uri": new, "version": null},
                                "edits": [{"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}, "newText": content}],
                            }));
                        }
                        e
                    }
                    None => self.workspace_edit(&edits, None),
                };
                self.request_application(id, label, edit)
            }
        }
    }

    /// The commands of the `RefactoringProcessor` (§13.11.2, "Comandos do
    /// framework novo").
    fn run_new_refactoring(&mut self, id: &Value, message: &Value, command: &'static str) -> Value {
        let invalid = || {
            error(
                id,
                INVALID_COMMAND_ARGUMENTS,
                "Refactoring operations require 4 parameters: filePath: String, offset: int, length: int, arguments: List",
            )
        };
        let arguments = message.pointer("/params/arguments").and_then(Value::as_array).cloned().unwrap_or_default();
        let parameters = match arguments.as_slice() {
            [] => json!({}),
            [single] if single.is_object() => single.clone(),
            _ => return error(id, INVALID_COMMAND_ARGUMENTS, format!("{command} requires a single Map argument")),
        };
        let (Some(path), Some(offset), Some(length), Some(list)) = (
            parameters.get("filePath").and_then(Value::as_str),
            parameters.get("selectionOffset").and_then(Value::as_i64),
            parameters.get("selectionLength").and_then(Value::as_i64),
            parameters.get("arguments").and_then(Value::as_array),
        ) else {
            return invalid();
        };
        let uri = url::Url::from_file_path(path).ok().map(|u| u.to_string());
        let text = uri.as_deref().and_then(|u| self.current_text(u, path));
        let (Some(uri), Some(text)) = (uri, text) else {
            return error(id, FILE_NOT_ANALYZED, "File is not being analyzed");
        };
        if !path.ends_with(".dart") {
            return error(id, FILE_NOT_ANALYZED, "File is not being analyzed");
        }
        if offset < 0 || length < 0 {
            return error(id, ERROR_NOT_HANDLED, format!("RangeError: offset {offset}, length {length}"));
        }
        let start = byte_from_utf16(&text, offset as usize);
        let end = byte_from_utf16(&text, (offset + length) as usize);
        let result = if command == "dart.refactor.move_top_level_to_file" {
            // `arguments[0] as String`: another type throws.
            match list.first().and_then(Value::as_str) {
                Some(target) => self.analyzer.move_to_file(&self.documents, &uri, start, end - start, target),
                None => crate::RefactoringResult::InternalError("type 'Null' is not a subtype of type 'String' in type cast".to_string()),
            }
        } else {
            // The experimental ones (`analyzeAvailability`): without the change.
            crate::RefactoringResult::Failure(None)
        };
        self.refactoring_response(id, command, result, None)
    }

    fn run_command(&mut self, id: &Value, message: &Value) -> Value {
        let command = message.pointer("/params/command").and_then(Value::as_str).unwrap_or("").to_string();
        match command.as_str() {
            "refactor.perform" => return self.run_legacy_refactoring(id, message, false),
            "refactor.validate" => return self.run_legacy_refactoring(id, message, true),
            _ => {}
        }
        if let Some(&c) = COMMANDS.iter().find(|c| c.starts_with("dart.refactor.") && **c == command) {
            return self.run_new_refactoring(id, message, c);
        }
        let label: &'static str = match command.as_str() {
            "dart.edit.sortMembers" => "Sort Members",
            "dart.edit.organizeImports" => "Organize Imports",
            "dart.edit.fixAll" => "Fix All",
            "dart.edit.fixAllInWorkspace.preview" => "Preview All Fixes in Workspace",
            "dart.edit.fixAllInWorkspace" => "Apply All Fixes in Workspace",
            "dart.edit.sendWorkspaceEdit" => "Send Workspace Edit",
            "dart.logAction" => "Log Action",
            _ => return error(id, UNKNOWN_COMMAND, format!("{command} is not a valid command identifier")),
        };
        let arguments = message.pointer("/params/arguments").and_then(Value::as_array).cloned().unwrap_or_default();
        let parameters = match arguments.as_slice() {
            [] => json!({}),
            [single] if single.is_object() => single.clone(),
            _ => return error(id, INVALID_COMMAND_ARGUMENTS, format!("{command} requires a single Map argument")),
        };
        match command.as_str() {
            // Only telemetry in the Dart server; without `action`, its conversion
            // throws (`log_action.dart:23`).
            "dart.logAction" => match parameters.get("action").and_then(Value::as_str) {
                Some(_) => response(id, Value::Null),
                None => error(id, ERROR_NOT_HANDLED, "dart.logAction without the action field"),
            },
            "dart.edit.fixAllInWorkspace" | "dart.edit.fixAllInWorkspace.preview" => {
                self.fix_workspace(id, label, command.ends_with(".preview"))
            }
            "dart.edit.sendWorkspaceEdit" => match parameters.get("edit").filter(|e| e.is_object()).cloned() {
                Some(edit) => self.request_application(id, label, edit),
                None => error(
                    id,
                    INVALID_COMMAND_ARGUMENTS,
                    "Send Workspace Edit requires a Map argument containing \"edit\" (WorkspaceEdit)",
                ),
            },
            _ => {
                let Some(path) = parameters.get("path").and_then(Value::as_str).map(str::to_string) else {
                    return error(id, INVALID_COMMAND_ARGUMENTS, format!("{label} requires a Map argument containing a \"path\""));
                };
                let auto = parameters.get("autoTriggered").and_then(Value::as_bool).unwrap_or(false);
                let uri = url::Url::from_file_path(&path).ok().map(|u| u.to_string());
                let text = uri
                    .as_deref()
                    .and_then(|u| self.documents.get(u).map(str::to_string))
                    .or_else(|| std::fs::read_to_string(&path).ok());
                let (Some(uri), Some(text)) = (uri, text) else {
                    return if auto && command != "dart.edit.fixAll" {
                        response(id, Value::Null)
                    } else {
                        error(id, FILE_NOT_ANALYZED, format!("{label} is only available for analyzed files"))
                    };
                };
                if command == "dart.edit.fixAll" {
                    return self.fix_all(id, label, &uri, &text, auto);
                }
                let mut names = dartforge_intern::Interner::new();
                let analyzed = dartforge_frontend::parser::parse(&text, &mut names);
                // `hasScanParseErrors`: some scanner or parser error.
                let with_errors = analyzed.diagnostics.iter().any(|d| {
                    d.code.is_none_or(|c| {
                        let single = c.info().unique;
                        single.starts_with("ParserErrorCode.") || single.starts_with("ScannerErrorCode.")
                    })
                });
                if with_errors {
                    return if auto {
                        response(id, Value::Null)
                    } else {
                        error_with_data(
                            id,
                            FILE_WITH_ERRORS,
                            format!("Unable to {label} because the file contains parse errors"),
                            json!(path),
                        )
                    };
                }
                let edits: Vec<crate::Edit> = if command == "dart.edit.sortMembers" {
                    crate::sort_members::sort_members(&text, &analyzed.unit, &analyzed.ast, &names)
                        .map(|(offset, length, new)| {
                            vec![crate::Edit {
                                uri: uri.clone(),
                                span: dartforge_diagnostics::Span { start: offset, end: offset + length },
                                text: new,
                            }]
                        })
                        .unwrap_or_default()
                } else {
                    // The semantic errors are those of the last typed publication, if it is of
                    // the current version; without it, no import is removed because of an
                    // error (only the textual duplicates).
                    let typed: &[dartforge_diagnostics::Diagnostic] = self
                        .typed_published
                        .get(&uri)
                        .filter(|(version, _)| Some(*version) == self.documents.version(&uri))
                        .map(|(_, d)| d.as_slice())
                        .unwrap_or_default();
                    let name_of =
                        |d: &dartforge_diagnostics::Diagnostic| d.code.map_or("", |c| c.info().unique.rsplit('.').next().unwrap_or(""));
                    let removable: Vec<usize> = typed
                        .iter()
                        .filter(|d| matches!(name_of(d), "DUPLICATE_IMPORT" | "UNUSED_IMPORT" | "UNNECESSARY_IMPORT"))
                        .map(|d| d.span.start)
                        .collect();
                    // The nine codes with `isUnresolvedIdentifier`.
                    let not_resolved = typed.iter().any(|d| {
                        matches!(
                            name_of(d),
                            "CONST_WITH_NON_TYPE"
                                | "EXTENDS_NON_CLASS"
                                | "NEW_WITH_NON_TYPE"
                                | "NON_TYPE_AS_TYPE_ARGUMENT"
                                | "UNDEFINED_ANNOTATION"
                                | "UNDEFINED_CLASS"
                                | "UNDEFINED_CLASS_BOOLEAN"
                                | "UNDEFINED_FUNCTION"
                                | "UNDEFINED_IDENTIFIER"
                        )
                    });
                    crate::organize_imports::organize(&text, &analyzed.unit, &removable, not_resolved, true)
                        .and_then(|new| crate::organize_imports::edit(&text, &new))
                        .map(|(offset, length, new)| {
                            vec![crate::Edit {
                                uri: uri.clone(),
                                span: dartforge_diagnostics::Span { start: offset, end: offset + length },
                                text: new,
                            }]
                        })
                        .unwrap_or_default()
                };
                // Nothing to change: `null`, without talking to the client.
                if edits.is_empty() {
                    return response(id, Value::Null);
                }
                let edit = self.workspace_edit(&edits, None);
                self.request_application(id, label, edit)
            }
        }
    }

    /// `Fix All` (docs/LSP-ESPECIFICACAO.md §13.12.7): up to four passes; in
    /// each one, the bulk-applicable fixes of the file's diagnostics, in offset
    /// order, each producer atomically and discarded if it conflicts. The first
    /// pass sees the typed diagnostics published for the current version; the
    /// following ones run over the already fixed text (a copy of the open
    /// documents) and only see the analyzer's immediate diagnostics. No refusal
    /// because of syntax errors.
    fn fix_all(&mut self, id: &Value, label: &'static str, uri: &str, text: &str, auto: bool) -> Value {
        use crate::fix_all::{Constructor, Ed};
        if !uri.ends_with(".dart") || crate::fix_all::generated(uri) {
            return response(id, Value::Null);
        }
        let version = self.documents.version(uri);
        let published: Vec<dartforge_diagnostics::Diagnostic> = self
            .typed_published
            .get(uri)
            .filter(|(v, _)| Some(*v) == version)
            .map(|(_, d)| d.clone())
            .unwrap_or_default();
        let mut passed: Vec<Vec<Ed>> = Vec::new();
        let mut current = text.to_string();
        let mut overlapping: Option<DocumentStore> = None;
        for passed_ in 0..4 {
            let documents = overlapping.as_ref().unwrap_or(&self.documents);
            let seen: &[dartforge_diagnostics::Diagnostic] = if passed_ == 0 { &published } else { &[] };
            let actions = self.analyzer.actions(documents, uri, 0, current.len(), seen);
            // Phase A: per diagnostic, in offset order.
            let mut candidates: Vec<&crate::CodeAction> = actions
                .iter()
                .filter(|a| {
                    a.diagnostic.is_some()
                        && a.create_file.is_none()
                        && crate::fix_all::in_batch(&a.kind, auto)
                        && a.edits.iter().all(|e| e.uri == uri && e.span.start <= e.span.end && e.span.end <= current.len())
                })
                .collect();
            candidates.sort_by_key(|a| a.diagnostic.as_ref().map_or(0, |d| d.span.start));
            let mut constructor = Constructor::default();
            for a in candidates {
                constructor.apply_producer(a.edits.iter().map(|e| (e.span.start, e.span.end - e.span.start, e.text.clone())));
            }
            // Phase B: the import cleanup, only on a manual request and in a pass
            // without phase A edits. The import errors are the typed ones, which only
            // apply to the original text.
            if constructor.edits.is_empty() && !auto && passed_ == 0 {
                let mut names = dartforge_intern::Interner::new();
                let analyzed = dartforge_frontend::parser::parse(&current, &mut names);
                for removal in crate::fix_all::remove_imports(&current, &analyzed.unit, &published) {
                    constructor.apply_producer([removal]);
                }
            }
            if constructor.edits.is_empty() {
                break;
            }
            current = constructor.apply(&current);
            passed.push(constructor.edits);
            // The fixed text as an open document, for the next pass; the other open
            // documents stay as they are.
            let mut copy = DocumentStore::new();
            for other in self.documents.uris() {
                if other != uri
                    && let (Some(v), Some(t)) = (self.documents.version(other), self.documents.get(other))
                {
                    copy.open(other.to_string(), v, t.to_string());
                }
            }
            copy.open(uri.to_string(), version.unwrap_or(0), current.clone());
            overlapping = Some(copy);
            // What the analyzer retains belonged to the previous text.
            self.analyzer.changed_document(uri);
        }
        if overlapping.is_some() {
            // The analyzer sees the real documents again.
            self.analyzer.changed_document(uri);
        }
        let edits: Vec<Ed> = match passed.len() {
            0 => return response(id, Value::Null),
            1 => passed.pop().unwrap_or_default(),
            _ => crate::fix_all::merge(passed.concat()),
        };
        // In LSP, ascending offset order; all relative to the original.
        let edits: Vec<crate::Edit> = edits
            .into_iter()
            .rev()
            .map(|(offset, length, new)| crate::Edit {
                uri: uri.to_string(),
                span: dartforge_diagnostics::Span { start: offset, end: offset + length },
                text: new,
            })
            .collect();
        let edit = self.workspace_edit(&edits, None);
        self.request_application(id, label, edit)
    }

    /// `Fix All in Workspace` and its preview (docs/LSP-ESPECIFICACAO.md
    /// §13.12.8): a single manual pass over every non-generated `.dart` file of
    /// the workspace roots and of the open documents, with phases A and B of
    /// `Fix All`; each edit carries the annotation with the fix's message
    /// template. An open document uses the typed diagnostics published for the
    /// current version; a disk-only file, the analyzer's immediate ones.
    fn fix_workspace(&mut self, id: &Value, label: &'static str, confirm: bool) -> Value {
        use crate::fix_all::{Constructor, Ed};
        if !self.apply_edits {
            return error(id, FEATURE_DISABLED, format!("\"{label}\" is only available for clients that support workspace/applyEdit"));
        }
        if !self.change_annotations {
            return error(id, FEATURE_DISABLED, format!("\"{label}\" is only available for clients that support change annotations"));
        }
        // The targets: the open ones and, from disk, those not open.
        let mut targets: Vec<(String, bool)> = self.documents.uris().map(|u| (u.to_string(), true)).collect();
        targets.sort();
        let open: HashSet<std::path::PathBuf> =
            targets.iter().filter_map(|(u, _)| url::Url::parse(u).ok().and_then(|x| x.to_file_path().ok())).collect();
        let mut seen: HashSet<std::path::PathBuf> = HashSet::new();
        for root in &self.roots {
            for file in dartforge_parity::corpus::arquivos_dart(root) {
                if open.contains(&file) || !seen.insert(file.clone()) {
                    continue;
                }
                if let Ok(u) = url::Url::from_file_path(&file) {
                    targets.push((u.to_string(), false));
                }
            }
        }
        // A copy of the open documents, in which each disk file only enters while
        // it is being examined.
        let mut copy = DocumentStore::new();
        for u in self.documents.uris() {
            if let (Some(v), Some(t)) = (self.documents.version(u), self.documents.get(u)) {
                copy.open(u.to_string(), v, t.to_string());
            }
        }
        let mut changes: Vec<(String, Vec<(Ed, &'static str)>)> = Vec::new();
        let mut touched_copy = false;
        for (u, open) in &targets {
            if !u.ends_with(".dart") || crate::fix_all::generated(u) {
                continue;
            }
            let (text, typed): (String, Vec<dartforge_diagnostics::Diagnostic>) = if *open {
                let Some(t) = self.documents.get(u) else { continue };
                let version = self.documents.version(u);
                let d = self.typed_published.get(u).filter(|(v, _)| Some(*v) == version).map(|(_, d)| d.clone()).unwrap_or_default();
                (t.to_string(), d)
            } else {
                let Some(t) = url::Url::parse(u).ok().and_then(|x| x.to_file_path().ok()).and_then(|p| std::fs::read_to_string(p).ok()) else {
                    continue;
                };
                copy.open(u.clone(), 0, t.clone());
                self.analyzer.changed_document(u);
                touched_copy = true;
                (t, Vec::new())
            };
            let actions = self.analyzer.actions(&copy, u, 0, text.len(), &typed);
            let mut candidates: Vec<&crate::CodeAction> = actions
                .iter()
                .filter(|a| {
                    a.diagnostic.is_some()
                        && a.create_file.is_none()
                        && crate::fix_all::in_batch(&a.kind, false)
                        && a.edits.iter().all(|e| e.uri == *u && e.span.start <= e.span.end && e.span.end <= text.len())
                })
                .collect();
            candidates.sort_by_key(|a| a.diagnostic.as_ref().map_or(0, |d| d.span.start));
            let mut constructor = Constructor::default();
            let mut described: Vec<(Ed, &'static str)> = Vec::new();
            for a in candidates {
                let edits_: Vec<Ed> = a.edits.iter().map(|e| (e.span.start, e.span.end - e.span.start, e.text.clone())).collect();
                if constructor.apply_producer(edits_.clone()) {
                    let description = crate::fix_all::description(&a.kind);
                    described.extend(edits_.into_iter().map(|e| (e, description)));
                }
            }
            if constructor.edits.is_empty() {
                let mut names = dartforge_intern::Interner::new();
                let analyzed = dartforge_frontend::parser::parse(&text, &mut names);
                for removal in crate::fix_all::remove_imports(&text, &analyzed.unit, &typed) {
                    if constructor.apply_producer([removal.clone()]) {
                        described.push((removal, "Remove unused import"));
                    }
                }
            }
            if !*open {
                let _ = copy.close(u);
            }
            if !constructor.edits.is_empty() {
                let with_description = constructor
                    .edits
                    .into_iter()
                    .map(|e| {
                        let d = described.iter().find(|(x, _)| *x == e).map_or("", |(_, d)| *d);
                        (e, d)
                    })
                    .collect();
                changes.push((u.clone(), with_description));
            }
        }
        if touched_copy {
            // The analyzer sees the real documents again.
            self.analyzer.changed_document("");
        }
        if changes.is_empty() {
            return response(id, Value::Null);
        }
        let mut annotations = serde_json::Map::new();
        let mut by_file: Vec<(String, Vec<Value>)> = Vec::new();
        for (u, edits_) in &changes {
            let mut list = Vec::new();
            // In LSP, ascending offset order.
            for ((offset, length, new), description) in edits_.iter().rev() {
                let Some(range) = self.range(u, dartforge_diagnostics::Span { start: *offset, end: offset + length }) else { continue };
                // Without a description, the last segment of the URI.
                let edit_label: &str = if description.is_empty() { u.rsplit('/').next().unwrap_or(u.as_str()) } else { *description };
                annotations
                    .entry(edit_label.to_string())
                    .or_insert_with(|| json!({"label": edit_label, "needsConfirmation": confirm}));
                list.push(json!({"range": range, "newText": new, "annotationId": edit_label}));
            }
            by_file.push((u.clone(), list));
        }
        let mut edit = if self.versioned_changes {
            let documents: Vec<Value> = by_file
                .into_iter()
                .map(|(u, edits)| {
                    let version = self.documents.version(&u).map_or(Value::Null, |v| json!(v));
                    json!({"textDocument": {"uri": u, "version": version}, "edits": edits})
                })
                .collect();
            json!({"documentChanges": documents})
        } else {
            let mut map = serde_json::Map::new();
            for (u, edits) in by_file {
                map.insert(u, json!(edits));
            }
            json!({"changes": map})
        };
        edit["changeAnnotations"] = Value::Object(annotations);
        self.request_application(id, label, edit)
    }

    /// A rename `window/showMessageRequest`; the `rename` stays pending until
    /// the answer.
    fn ask_on_rename(&mut self, pending: PendingRename, params: Value) -> Value {
        self.next_request += 1;
        let request = json!(format!("dartforge/prompt/{}", self.next_request));
        self.pending_renames.insert(key_id(&request), pending);
        json!({"jsonrpc": "2.0", "id": request, "method": "window/showMessageRequest", "params": params})
    }

    /// `handler_rename.dart:205-269` after the conditions: the document version
    /// (`-32801`), the class's file (`renameFilesWithClasses`: `always` renames,
    /// `prompt` asks `Yes`/`No`) and the `WorkspaceEdit` with the `RenameFile`
    /// at the end.
    fn finish_rename(&mut self, mut pending: PendingRename) -> Value {
        if !pending.file_question
            && pending.version.is_some()
            && self.documents.version(&pending.uri) != pending.version
        {
            return error(&pending.id, MODIFIED_CONTENT, "Document was modified before operation completed");
        }
        let mut feature = None;
        if let Some(file) = pending.file.take()
            && self.possible_file_rename
        {
            // `renameFilesWithClasses` of the file's folder (`forResource`).
            let mode = self.configuration_of(&file.from).rename_files_with_classes.clone();
            let renames = if pending.file_question || mode == "always" || (self.rename_files && mode != "never" && mode != "prompt") {
                true
            } else if mode == "prompt" && self.user_questions {
                let base = |u: &str| u.rsplit('/').next().unwrap_or(u).to_string();
                let message = format!("Rename '{}' to '{}'?", base(&file.from), base(&file.to));
                pending.file_question = true;
                pending.file = Some(file);
                return self.ask_on_rename(
                    pending,
                    json!({"type": 3, "message": message, "actions": [{"title": "Yes"}, {"title": "No"}]}),
                );
            } else {
                false
            };
            // Only the file operation: the Dart server does not rewrite the directives
            // in `rename` (the client asks for `willRenameFiles`).
            if renames {
                feature = Some(json!({"kind": "rename", "oldUri": file.from, "newUri": file.to}));
            }
        }
        // The `SourceChange` order: the declaration's file (the first edit) and the
        // others in search order (by path).
        if let Some(first) = pending.edits.first().map(|e| e.uri.clone()) {
            pending.edits.sort_by(|a, b| (a.uri != first, &a.uri).cmp(&(b.uri != first, &b.uri)));
        }
        response(&pending.id, self.workspace_edit(&pending.edits, feature))
    }

    /// The user's answer to a rename question: `Rename Anyway` proceeds (any
    /// other, including `null` or an error, gives `{}`); `Yes` renames the file
    /// (another answer, only the edits).
    fn rename_response(&mut self, mut pending: PendingRename, message: &Value) -> Value {
        let choice = message.pointer("/result/title").and_then(Value::as_str);
        if pending.file_question {
            if choice != Some("Yes") {
                pending.file = None;
            }
            return self.finish_rename(pending);
        }
        if choice != Some("Rename Anyway") {
            return response(&pending.id, json!({}));
        }
        self.finish_rename(pending)
    }

    /// The `workspace/applyEdit` request of a command (§13.12.3); the
    /// `executeCommand` with id `id` stays pending until the client answers.
    fn request_application(&mut self, id: &Value, label: &'static str, edit: Value) -> Value {
        self.next_request += 1;
        let request = json!(format!("dartforge/applyEdit/{}", self.next_request));
        self.pending_edits.insert(key_id(&request), (id.clone(), label, edit.clone()));
        json!({
            "jsonrpc": "2.0",
            "id": request,
            "method": "workspace/applyEdit",
            "params": {"label": label, "edit": edit},
        })
    }

    /// A client response to a server request. That of a pending
    /// `workspace/applyEdit` becomes the response of the `executeCommand` that
    /// sent it (the table of §13.12.3); the others carry no data.
    fn client_response(&mut self, message: &Value) -> Option<Value> {
        let key = key_id(message.get("id")?);
        if self.configuration_request.as_ref().is_some_and(|p| key_id(&json!(p)) == key) {
            self.received_configuration(message);
            return None;
        }
        if let Some(pending) = self.pending_renames.remove(&key) {
            return Some(self.rename_response(pending, message));
        }
        // The client refused the dynamic registration (`_applyRegistrations`): the
        // error goes to the client's log.
        if message.get("id").and_then(Value::as_str).is_some_and(|i| i.starts_with("dartforge/registerCapability/"))
            && let Some(e) = message.get("error").filter(|e| !e.is_null())
        {
            let code = e.get("code").map(|c| c.to_string()).unwrap_or_default();
            let text = e.get("message").and_then(Value::as_str).unwrap_or_default();
            return Some(message_notification("window/logMessage", 1, &format!("Failed to register capabilities with client: ({code}) {text}")));
        }
        let (id, label, edit) = self.pending_edits.remove(&key)?;
        if let Some(e) = message.get("error").filter(|e| !e.is_null()) {
            return Some(error_with_data(
                &id,
                CLIENT_NOT_APPLIED,
                format!("Client failed to apply workspace edit for {label}"),
                json!(e.to_string()),
            ));
        }
        let Some(result) = message.get("result").filter(|r| r.is_object()) else {
            return Some(error(&id, ERROR_NOT_HANDLED, "workspace/applyEdit response without a result"));
        };
        if result.get("applied").and_then(Value::as_bool) == Some(true) {
            return Some(response(&id, Value::Null));
        }
        Some(match result.get("failureReason").and_then(Value::as_str) {
            // Without a reason: the user declined a preview; not an error.
            None => response(&id, Value::Null),
            Some(reason) => error_with_data(
                &id,
                CLIENT_NOT_APPLIED,
                format!("Client failed to apply workspace edit for {label} (reason: {reason})"),
                edit,
            ),
        })
    }

    /// `WorkspaceEdit` of the edits: `documentChanges` (each document with the
    /// current version, `null` if closed) when the client accepts it, otherwise
    /// `changes`. A resource operation (renaming a file) goes after the text
    /// edits, which apply to the text before it.
    /// `createWorkspaceEdit(..., allowSnippets: true)` (`mapping.dart:156-213`,
    /// docs/LSP-ESPECIFICACAO.md §13.8.1-G): the action becomes a
    /// `TextDocumentEdit` with a `SnippetTextEdit` only when the client
    /// announced `experimental.snippetTextEdit` and `documentChanges`, the
    /// change is **one** edit in the request's file (which already exists) and
    /// there is a selection or a linked group. Otherwise `None`: the plain
    /// edit, without selection or groups.
    fn edit_snippet(&self, uri: &str, action: &crate::CodeAction) -> Option<Value> {
        if !self.edits_snippet || !self.versioned_changes || action.create_file.is_some() || action.linked.empty() {
            return None;
        }
        let [e] = action.edits.as_slice() else { return None };
        if e.uri != uri {
            return None;
        }
        let range = self.range(&e.uri, e.span)?;
        // A single edit: its text starts at the same offset in the final document
        // (`toSnippetTextEdits`, `offsetDelta` = 0).
        let new = crate::snippets::snippet_text(&e.text, e.span.start, &action.linked);
        let version = self.documents.version(&e.uri).map_or(Value::Null, |v| json!(v));
        Some(json!({"documentChanges": [{
            "textDocument": {"uri": e.uri, "version": version},
            "edits": [{"insertTextFormat": 2, "range": range, "newText": new}],
        }]}))
    }

    fn workspace_edit(&self, edits_: &[crate::Edit], feature: Option<Value>) -> Value {
        let mut by_uri: Vec<(String, Vec<Value>)> = Vec::new();
        for e in edits_ {
            let Some(range) = self.range(&e.uri, e.span) else { continue };
            let item = json!({"range": range, "newText": e.text});
            match by_uri.iter_mut().find(|(u, _)| *u == e.uri) {
                Some((_, l)) => l.push(item),
                None => by_uri.push((e.uri.clone(), vec![item])),
            }
        }
        if self.versioned_changes || feature.is_some() {
            let mut changes: Vec<Value> = by_uri
                .into_iter()
                .map(|(uri, edits)| {
                    let version = self.documents.version(&uri).map_or(Value::Null, |v| json!(v));
                    json!({"textDocument": {"uri": uri, "version": version}, "edits": edits})
                })
                .collect();
            changes.extend(feature);
            return json!({"documentChanges": changes});
        }
        let mut changes = serde_json::Map::new();
        for (uri, edits) in by_uri {
            changes.insert(uri, json!(edits));
        }
        json!({"changes": changes})
    }

    /// `workspace/symbol`: the symbols of the open documents and of the `.dart`
    /// files of the workspace projects (the `initialize` roots and the project,
    /// with `pubspec.yaml`, of each open document). The open text wins over the
    /// disk; each tree is discarded before the next one. Matches fuzzily
    /// (`crate::fuzzy_matcher`): those that contain the query first, then the
    /// subsequences; among equals, (URI, position) order.
    fn workspace_symbols(&mut self, query: &str) -> Vec<Value> {
        // `handler_workspace_symbols.dart:28-123` (docs/LSP-ESPECIFICACAO.md §5):
        // an empty query returns nothing.
        if query.is_empty() {
            return Vec::new();
        }
        let mut roots = self.roots.clone();
        // The path of each open document (that of the `file:` URI; otherwise the
        // URI's path, only to index the open text).
        let open_path = |u: &str| -> Option<std::path::PathBuf> {
            let x = url::Url::parse(u).ok()?;
            Some(x.to_file_path().unwrap_or_else(|_| std::path::PathBuf::from(x.path())))
        };
        let mut open_texts: HashMap<std::path::PathBuf, (String, String)> = HashMap::new();
        let mut loose: Vec<std::path::PathBuf> = Vec::new();
        for open in self.documents.uris() {
            let Some(file) = open_path(open) else { continue };
            let root = crate::project::project_root(&file);
            if root.join("pubspec.yaml").is_file() && !roots.contains(&root) {
                roots.push(root);
            }
            if let Some(text) = self.documents.get(open) {
                open_texts.insert(dartforge_elements::generated::key(&file), (open.to_string(), text.to_string()));
            }
            if file.extension().is_some_and(|x| x == "dart") {
                loose.push(file);
            }
        }
        // `addedFiles`: the files of the analyzed roots; the known ones: the SDK
        // and the `lib/` of the packages of each root.
        let mut added: Vec<std::path::PathBuf> = roots.iter().flat_map(|r| crate::project::project_files(r)).collect();
        // An open document outside the roots is also analyzed (the server creates a
        // context for it).
        loose.retain(|c| !roots.iter().any(|r| c.starts_with(r)));
        loose.sort();
        added.extend(loose);
        let packages: Vec<std::path::PathBuf> = roots.iter().flat_map(|r| crate::workspace_symbols::package_folders(r)).collect();
        let sdk = dartforge_elements::sdk::SdkLayout::discover();
        let kinds = self.workspace_symbol_kinds.clone();
        self.symbol_index.search(query, &added, sdk.as_deref(), &packages, &open_texts, kinds.as_deref())
    }

    /// URI and offset (bytes) of `params.textDocument` + `params.position`, when
    /// the document is open.
    fn request_position(&self, message: &Value) -> Option<(String, usize)> {
        let params = message.get("params")?;
        let uri = params.get("textDocument")?.get("uri")?.as_str()?;
        let position = read_position(params.get("position")?)?;
        let text = self.documents.get(uri)?;
        let offset = self.documents.lines(uri)?.position_offset(text, position.line, position.column);
        Some((uri.to_string(), offset))
    }

    /// LSP range of `span` (bytes) in `uri`: through the open text, otherwise
    /// through the file on disk (rename touches closed files of the project).
    fn range(&self, uri: &str, span: dartforge_diagnostics::Span) -> Option<Value> {
        if let (Some(text), Some(table)) = (self.documents.get(uri), self.documents.lines(uri)) {
            return Some(lsp_range(text, table, span.start, span.end));
        }
        let read = self
            .disk_lines
            .borrow_mut()
            .entry(uri.to_string())
            .or_insert_with(|| {
                let path = url::Url::parse(uri).ok()?.to_file_path().ok()?;
                let source = std::fs::read_to_string(path).ok()?;
                let table = crate::utf16::LineTable::build(&source);
                Some(std::sync::Arc::new((source, table)))
            })
            .clone()?;
        Some(lsp_range(&read.0, &read.1, span.start, span.end))
    }

    /// Diagnoses the document and builds `textDocument/publishDiagnostics`.
    fn publish(&mut self, uri: &str) -> Value {
        let Some(text) = self.documents.get(uri).map(str::to_string) else {
            return empty_publication(uri);
        };
        // Outside the analysis roots (`analysisExcludedFolders`): nothing.
        if self.excluded(uri) {
            return empty_publication(uri);
        }
        let diagnostics = self.analyzer.diagnose(uri, &text);
        self.publication(uri, &diagnostics)
    }

    /// `_filesWithClientDiagnostics` (`lsp_analysis_server.dart:709-729`): an
    /// empty list for a file the client already has empty is not sent; the
    /// others update the record.
    fn filter_repeated_empty(&mut self, uri: &str, publication: Value) -> Option<Value> {
        let empty = publication.pointer("/params/diagnostics").and_then(Value::as_array).is_none_or(|l| l.is_empty());
        if empty {
            if !self.with_client_diagnostics.remove(uri) {
                return None;
            }
        } else {
            self.with_client_diagnostics.insert(uri.to_string());
        }
        Some(publication)
    }

    /// `publishDiagnostics` of a file that is not open, with the analyzed text.
    /// The publication of the diagnostics of a YAML file (in the analyzer's
    /// JSON: 1-based line and column, UTF-16 offset): the ranges through the
    /// read text.
    fn yaml_publication(&mut self, uri: &str, text: &str, diagnostics: &[dartforge_parity::json::DiagJson]) -> Option<Value> {
        let items: Vec<Value> = diagnostics
            .iter()
            .map(|d| {
                let dot = |p: &dartforge_parity::json::Ponto| json!({"line": p.line.saturating_sub(1), "character": p.column.saturating_sub(1)});
                let severity = match d.severity.as_str() {
                    "ERROR" => SEVERITY_ERROR,
                    "WARNING" => 2,
                    _ => 3,
                };
                let message = match &d.correction_message {
                    Some(c) => format!("{}\n{c}", d.problem_message),
                    None => d.problem_message.clone(),
                };
                json!({
                    "range": {"start": dot(&d.location.range.start), "end": dot(&d.location.range.end)},
                    "severity": severity,
                    "source": SOURCE,
                    "message": message,
                    "code": d.code,
                })
            })
            .collect();
        let _ = text;
        let p = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {"uri": uri, "diagnostics": items},
        });
        self.filter_repeated_empty(uri, p)
    }

    fn text_publication(&mut self, uri: &str, text: &str, diagnostics: &[dartforge_diagnostics::Diagnostic]) -> Option<Value> {
        let table = crate::utf16::LineTable::build(text);
        let items: Vec<Value> = diagnostics.iter().map(|d| convert_diagnostic(text, &table, d)).collect();
        let p = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {"uri": uri, "diagnostics": items},
        });
        self.filter_repeated_empty(uri, p)
    }

    /// The `textDocument/diagnostic` report of `uri`: the typed result of the
    /// current version, otherwise the analyzer's (the same that
    /// `publishDiagnostics` would carry). The `resultId` is the version and
    /// whether the result is the typed one; when equal to `previousResultId`, the
    /// response is `unchanged` without recomputing. A document that is not open
    /// has no diagnostics (like `didClose` in push mode).
    fn pulled_report(&mut self, uri: &str, previous: Option<&str>) -> Value {
        let Some(version) = self.documents.version(uri) else {
            return json!({"kind": "full", "items": []});
        };
        if self.excluded(uri) {
            return json!({"kind": "full", "items": []});
        }
        let typed = self.typed_published.get(uri).is_some_and(|(v, _)| *v == version);
        let result_id = format!("{version}{}", if typed { ".t" } else { "" });
        if previous == Some(result_id.as_str()) {
            return json!({"kind": "unchanged", "resultId": result_id});
        }
        let diagnostics = if typed {
            self.typed_published[uri].1.clone()
        } else {
            let text = self.documents.get(uri).map(str::to_string).unwrap_or_default();
            self.analyzer.diagnose(uri, &text)
        };
        let (Some(text), Some(table)) = (self.documents.get(uri), self.documents.lines(uri)) else {
            return json!({"kind": "full", "items": []});
        };
        let items: Vec<Value> = diagnostics.iter().map(|d| convert_diagnostic(text, table, d)).collect();
        json!({"kind": "full", "resultId": result_id, "items": items})
    }

    /// `publishDiagnostics` of `diagnostics` (spans of the current text of `uri`).
    fn publication(&self, uri: &str, diagnostics: &[dartforge_diagnostics::Diagnostic]) -> Value {
        let (Some(text), Some(table)) = (self.documents.get(uri), self.documents.lines(uri)) else {
            return empty_publication(uri);
        };
        let version = self.documents.version(uri).unwrap_or(0);
        let items: Vec<Value> = diagnostics
            .iter()
            .map(|d| convert_diagnostic(text, table, d))
            .collect();
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {"uri": uri, "version": version, "diagnostics": items},
        })
    }
}

/// Workspace roots in `initialize`: `workspaceFolders`, else `rootUri`, else
/// `rootPath`; only existing directories.
/// The line starts (`LineInfo.lineStarts`) of `text` in UTF-16 units: after
/// `\n`, `\r\n` and a lone `\r`.
fn utf16_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    let mut o = 0usize;
    let mut previous_cr = false;
    for ch in text.chars() {
        let width = ch.len_utf16();
        if previous_cr && ch != '\n' {
            starts.push(o);
        }
        o += width;
        previous_cr = ch == '\r';
        if ch == '\n' {
            starts.push(o);
        }
    }
    if previous_cr {
        starts.push(o);
    }
    starts
}

fn initialize_roots(params: Option<&Value>) -> Vec<std::path::PathBuf> {
    let Some(params) = params else { return Vec::new() };
    let file_path_of = |v: &Value| {
        v.as_str()
            .and_then(|u| url::Url::parse(u).ok())
            .and_then(|u| u.to_file_path().ok())
    };
    let mut roots: Vec<std::path::PathBuf> = params
        .get("workspaceFolders")
        .and_then(Value::as_array)
        .map(|l| l.iter().filter_map(|f| file_path_of(&f["uri"])).collect())
        .unwrap_or_default();
    if roots.is_empty() {
        roots.extend(params.get("rootUri").and_then(file_path_of));
    }
    if roots.is_empty() {
        roots.extend(params.get("rootPath").and_then(Value::as_str).map(std::path::PathBuf::from));
    }
    roots.retain(|r| r.is_dir());
    roots
}

/// Sleeps up to `ms` in 10 ms slices; true when cancelled midway.
///
/// The safe cancellation point while running: between slices, without
/// partial state, because sleeping produces nothing. Real analysis will
/// check the same condition between documents, never in the middle of a
/// parser sentence.
fn sleep_cancellable(ms: u64, cancelled: &HashSet<String>, key: &str) -> bool {
    let slices = ms / 10 + u64::from(!ms.is_multiple_of(10));
    for _ in 0..slices {
        std::thread::sleep(std::time::Duration::from_millis(10.min(ms)));
        if cancelled.contains(key) {
            return true;
        }
    }
    false
}

/// True when the message is a request (it has an `id` and a textual `method`).
fn is_request(message: &Value) -> bool {
    message.get("id").is_some() && message.get("method").is_some_and(|m| m.is_string())
}

/// Canonical key of the id (a number and a text with the same digit are
/// distinct ids in JSON-RPC; canonical serialization preserves the
/// distinction).
fn key_id(id: &Value) -> String {
    id.to_string()
}

/// JSON-RPC success response.
fn response(id: &Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// JSON-RPC error response.
fn error(id: &Value, code: i32, message: impl Into<String>) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message.into()}})
}

/// `window/showMessage` or `window/logMessage` of type `ty` (1 = error).
fn message_notification(method: &str, ty: i32, text: &str) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": {"type": ty, "message": text}})
}

/// The message of a panic (the `&str` or the `String` of the `panic!`).
fn panic_text(panic: &(dyn std::any::Any + Send)) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic without a message".to_string())
}

/// The priority of a fix or assist by kind (docs/LSP-ESPECIFICACAO.md
/// §13.7.2 and §13.3): only the kinds this server emits have their own
/// value; the others get the group's default (50 for fixes, 40 for the
/// `*.multi` ones, 30 for assists).
fn action_priority(kind: &str) -> i32 {
    // The generated table of Dart's kinds (§13.7.2 and `DartAssistKind`).
    if let Some(id) = action_id(kind)
        && let Some((p, _)) = crate::actions::dart_kind(&id)
    {
        return i32::from(p);
    }
    match kind {
        other if other.starts_with("quickfix") && other.ends_with(".multi") => 40,
        other if other.starts_with("quickfix") => 50,
        _ => 30,
    }
}

/// The original id of the change (`dart.fix.…`, `dart.assist.…`) of an LSP
/// kind: the inverse of `toCodeActionKind`.
fn action_id(kind: &str) -> Option<String> {
    if let Some(rest) = kind.strip_prefix("quickfix.") {
        return Some(format!("dart.fix.{rest}"));
    }
    kind.strip_prefix("refactor.").map(|rest| format!("dart.assist.{rest}"))
}

/// `_CodeActionSorter.sort` (`handler_code_actions.dart:284-406`;
/// docs/LSP-ESPECIFICACAO.md §13.7.1, item 9): groups by title in the order
/// of the first occurrence; within a group, keeps the action whose first
/// diagnostic starts at the column closest to the request's, with the
/// diagnostics of the others with the same edit merged (those with a
/// different edit go away); then, descending priority and arrival order.
/// The byte offset of UTF-16 offset `u` (the end of the text if it goes past it).
fn byte_from_utf16(text: &str, u: usize) -> usize {
    let mut counted = 0usize;
    for (i, c) in text.char_indices() {
        if counted >= u {
            return i;
        }
        counted += c.len_utf16();
    }
    text.len()
}

fn sort_actions(actions: Vec<Value>, request_column: i64) -> Vec<Value> {
    let mut groups: Vec<(String, Vec<Value>)> = Vec::new();
    for a in actions {
        let title = a["title"].as_str().unwrap_or("").to_string();
        match groups.iter_mut().find(|(t, _)| *t == title) {
            Some((_, g)) => g.push(a),
            None => groups.push((title, vec![a])),
        }
    }
    let distance = |a: &Value| a.pointer("/diagnostics/0/range/start/character").and_then(Value::as_i64).map_or(0, |c| (c - request_column).abs());
    let mut unique: Vec<Value> = Vec::new();
    for (_, mut group) in groups {
        if group.len() > 1 {
            group.sort_by_key(distance);
            let mut first = group.remove(0);
            let mut diagnostics: Vec<Value> = first["diagnostics"].as_array().cloned().unwrap_or_default();
            for other in group {
                if other["edit"] == first["edit"] {
                    diagnostics.extend(other["diagnostics"].as_array().cloned().unwrap_or_default());
                }
            }
            first["diagnostics"] = Value::Array(diagnostics);
            unique.push(first);
        } else {
            unique.extend(group);
        }
    }
    // Stable: on a tie, arrival order counts.
    unique.sort_by_key(|a| -action_priority(a["kind"].as_str().unwrap_or("")));
    unique
}

/// JSON-RPC error response with the `data` field.
fn error_with_data(id: &Value, code: i32, message: impl Into<String>, given: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message.into(), "data": given}})
}

/// Empty publication: a closed or unknown document has no diagnostics.
fn empty_publication(uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": {"uri": uri, "diagnostics": []},
    })
}

/// LSP requires `SymbolInformation[]` for clients that did not announce
/// support for hierarchical `DocumentSymbol[]`. The flat location is the
/// selection of the name.
fn flatten_symbols(symbol: &Value, uri: &str, parent: Option<&str>, output: &mut Vec<Value>) {
    let name = symbol.get("name").and_then(Value::as_str).unwrap_or("");
    let mut plan = json!({
        "name": name,
        "kind": symbol["kind"],
        "location": {"uri": uri, "range": symbol["selectionRange"]},
    });
    if let Some(parent) = parent {
        plan["containerName"] = json!(parent);
    }
    output.push(plan);
    if let Some(children) = symbol.get("children").and_then(Value::as_array) {
        for child in children {
            flatten_symbols(child, uri, Some(name), output);
        }
    }
}

/// Converts the protocol's `contentChanges` into internal changes.
///
/// A missing or malformed range becomes a full replacement (never a `panic`
/// because of a malformed client message: the worst case is reanalyzing
/// everything).
fn read_changes(value: &Value) -> Vec<ContentChange> {
    let Some(list) = value.as_array() else {
        return Vec::new();
    };
    list
        .iter()
        .map(|m| {
            let range = m.get("range").and_then(read_range);
            let text = m
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            ContentChange { range, text }
        })
        .collect()
}

/// Reads `{start: {line, character}, end: {...}}` into internal positions.
fn read_range(value: &Value) -> Option<(Position, Position)> {
    let start = read_position(value.get("start")?)?;
    let end = read_position(value.get("end")?)?;
    Some((start, end))
}

/// Reads `{line, character}`; a missing or negative number becomes 0 (saturates).
fn read_position(value: &Value) -> Option<Position> {
    let line = value.get("line")?.as_u64()? as u32;
    let column = value.get("character")?.as_u64()? as u32;
    Some(Position { line, column })
}

/// LSP range (UTF-16) of `start..end` in bytes, saturated to the text.
fn lsp_range(text: &str, table: &crate::utf16::LineTable, start: usize, end: usize) -> Value {
    let start = start.min(text.len());
    let end = end.clamp(start, text.len());
    let (l0, c0) = table.offset_position(text, start);
    let (l1, c1) = table.offset_position(text, end);
    json!({"start": {"line": l0, "character": c0}, "end": {"line": l1, "character": c1}})
}

/// Converts a diagnostic (span in UTF-8 bytes) into an LSP diagnostic.
///
/// Offsets saturated to the text; an end before the start collapses onto the
/// start.
fn convert_diagnostic(
    text: &str,
    table: &crate::utf16::LineTable,
    diagnostic: &dartforge_diagnostics::Diagnostic,
) -> Value {
    let start = diagnostic.span.start.min(text.len());
    let mut end = diagnostic.span.end.min(text.len());
    if end < start {
        end = start;
    }
    let (l0, c0) = table.offset_position(text, start);
    let (l1, c1) = table.offset_position(text, end);
    json!({
        "range": {
            "start": {"line": l0, "character": c0},
            "end": {"line": l1, "character": c1},
        },
        "severity": severity(diagnostic.severity),
        "source": SOURCE,
        "message": message(diagnostic),
        "code": diagnostic.code.map(|c| c.info().name),
    })
}

/// Server log: always `stderr`, never `stdout` (which is protocol only).
///
/// A forgotten `println!` corrupts the session; this function exists so that
/// the right path is the shortest one.
fn register(message: String) {
    eprintln!("[dartforge-lsp] {message}");
}

/// Test map unused outside debugging: logs the method of each message when
/// `DARTFORGE_LSP_DEBUG` or `RUST_LOG` ask for verbosity.
pub(crate) fn detail_level() -> bool {
    std::env::var("DARTFORGE_LSP_DEBUG").is_ok()
        || std::env::var("RUST_LOG")
            .is_ok_and(|v| v.eq_ignore_ascii_case("debug") || v.eq_ignore_ascii_case("trace"))
}

impl<A: Analyzer> Server<A> {
    /// Logs the received message to `stderr` when the level is verbose.
    pub fn register_received(&self, message: &Value) {
        if detail_level() {
            let method = message
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or("<response>");
            register(format!("received: {method}"));
        }
    }
}

/// Documents the accepted methods, for `docs/LSP.md` and for the acceptance test.
pub fn supported_methods() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        (
            "initialize",
            "request: handshake and capability announcement",
        ),
        (
            "shutdown",
            "request: prepares the shutdown with code 0",
        ),
        (
            "initialized",
            "notification: the client's acknowledgement, no effect",
        ),
        ("exit", "notification: exits (0 after shutdown, 1 without it)"),
        (
            "$/cancelRequest",
            "notification: cancels a queued or running request",
        ),
        (
            "textDocument/didOpen",
            "notification: opens and publishes diagnostics",
        ),
        (
            "textDocument/didChange",
            "notification: applies an incremental edit and republishes",
        ),
        (
            "textDocument/didClose",
            "notification: closes and publishes an empty list",
        ),
        (
            "textDocument/documentLink",
            "request: empty list (in 3.6.2 there are only Flutter example links)",
        ),
        (
            "textDocument/codeLens",
            "request: empty list (there are only augmentation lenses)",
        ),
        (
            "textDocument/completion",
            "request: members by the receiver's type, scope, keywords, named arguments",
        ),
        (
            "textDocument/prepareRename",
            "request: renameable range and name under the cursor",
        ),
        (
            "textDocument/rename",
            "request: edits in all of the project's files",
        ),
        (
            "textDocument/codeAction",
            "request: insert ';' and import the library of an undefined name",
        ),
        (
            SLEEP_METHOD,
            "request: test hook for cancellation while running",
        ),
    ])
}

/// Replaces, in the symbol tree, the kinds the client did not announce with
/// the Dart server's alternative (`elementKindToSymbolKind`): `EnumMember`
/// (22) → `Enum` (10), `TypeParameter` (26) → `Variable` (13).
fn adjust_kinds(symbol: &mut Value, supported: Option<&[u64]>) {
    let accepts = |k: u64| supported.map_or(k <= 18, |l| l.contains(&k));
    if let Some(k) = symbol["kind"].as_u64()
        && !accepts(k)
    {
        let alternative = match k {
            22 => 10,
            26 => 13,
            _ => k,
        };
        symbol["kind"] = json!(alternative);
    }
    if let Some(children) = symbol.get_mut("children").and_then(Value::as_array_mut) {
        for f in children {
            adjust_kinds(f, supported);
        }
    }
}
