//! Dynamic capability registration and client configuration
//! (docs/LSP-ESPECIFICACAO.md §2.4 and §2.5):
//! `server_capabilities_computer.dart`, `feature_registration.dart` and
//! `client_configuration.dart` of the Dart 3.6.2 server.
//!
//! A feature is dynamic when the client declares `dynamicRegistration` for
//! it: the static field leaves `initialize` and the registration comes
//! later, through `client/registerCapability`, applying the difference
//! against the current set (first the `client/unregisterCapability` of those
//! that left, then the registration of the new ones), comparing by method
//! and options. The ids are an increasing decimal counter.
//! Written without compiling or running (2026-10-05).

use serde_json::{json, Value};

/// A feature: the capability name in the client (`textDocument.<name>`),
/// the key of the server's static capability and the registered methods.
pub struct Feature {
    pub client: &'static str,
    pub static_: &'static str,
    pub methods: &'static [&'static str],
    /// Only Dart files (the rest applies to `fullySupportedTypes`, which without
    /// plugins is the same selector).
    pub dart_only: bool,
}

/// The features the server announces that can be dynamic.
pub const FEATURES: &[Feature] = &[
    Feature { client: "synchronization", static_: "textDocumentSync", methods: &["textDocument/didOpen", "textDocument/didClose", "textDocument/didChange"], dart_only: false },
    Feature { client: "completion", static_: "completionProvider", methods: &["textDocument/completion"], dart_only: false },
    Feature { client: "hover", static_: "hoverProvider", methods: &["textDocument/hover"], dart_only: false },
    Feature { client: "definition", static_: "definitionProvider", methods: &["textDocument/definition"], dart_only: false },
    Feature { client: "implementation", static_: "implementationProvider", methods: &["textDocument/implementation"], dart_only: false },
    Feature { client: "references", static_: "referencesProvider", methods: &["textDocument/references"], dart_only: false },
    Feature { client: "documentHighlight", static_: "documentHighlightProvider", methods: &["textDocument/documentHighlight"], dart_only: false },
    Feature { client: "documentSymbol", static_: "documentSymbolProvider", methods: &["textDocument/documentSymbol"], dart_only: false },
    Feature { client: "foldingRange", static_: "foldingRangeProvider", methods: &["textDocument/foldingRange"], dart_only: false },
    // Formatting (`handler_format_on_type.dart`, `handler_format_range.dart`,
    // `handler_formatting.dart`), in the order of `allFeatures`.
    Feature { client: "onTypeFormatting", static_: "documentOnTypeFormattingProvider", methods: &["textDocument/onTypeFormatting"], dart_only: true },
    Feature { client: "rangeFormatting", static_: "documentRangeFormattingProvider", methods: &["textDocument/rangeFormatting"], dart_only: true },
    Feature { client: "formatting", static_: "documentFormattingProvider", methods: &["textDocument/formatting"], dart_only: false },
    Feature { client: "signatureHelp", static_: "signatureHelpProvider", methods: &["textDocument/signatureHelp"], dart_only: false },
    Feature { client: "codeAction", static_: "codeActionProvider", methods: &["textDocument/codeAction"], dart_only: false },
    Feature { client: "rename", static_: "renameProvider", methods: &["textDocument/rename"], dart_only: false },
    Feature { client: "semanticTokens", static_: "semanticTokensProvider", methods: &["textDocument/semanticTokens"], dart_only: false },
    Feature { client: "typeDefinition", static_: "typeDefinitionProvider", methods: &["textDocument/typeDefinition"], dart_only: true },
    Feature { client: "selectionRange", static_: "selectionRangeProvider", methods: &["textDocument/selectionRange"], dart_only: true },
    Feature { client: "callHierarchy", static_: "callHierarchyProvider", methods: &["textDocument/prepareCallHierarchy"], dart_only: true },
    Feature { client: "typeHierarchy", static_: "typeHierarchyProvider", methods: &["textDocument/prepareTypeHierarchy"], dart_only: true },
    Feature { client: "codeLens", static_: "codeLensProvider", methods: &["textDocument/codeLens"], dart_only: true },
    Feature { client: "documentLink", static_: "documentLinkProvider", methods: &["textDocument/documentLink"], dart_only: true },
    Feature { client: "inlayHint", static_: "inlayHintProvider", methods: &["textDocument/inlayHint"], dart_only: true },
    Feature { client: "colorProvider", static_: "colorProvider", methods: &["textDocument/documentColor"], dart_only: true },
];

/// `dartFiles` (`constants.dart`): the Dart files.
fn dart_selector() -> Value {
    json!([{"language": "dart", "scheme": "file"}])
}

/// The synchronization selector: Dart, `pubspec.yaml`,
/// `analysis_options.yaml` and the `fix_data` files
/// (`handler_text_document_changes.dart:141-179`).
fn sync_selector() -> Value {
    json!([
        {"language": "dart", "scheme": "file"},
        {"language": "yaml", "scheme": "file", "pattern": "**/pubspec.yaml"},
        {"language": "yaml", "scheme": "file", "pattern": "**/analysis_options.yaml"},
        {"language": "yaml", "scheme": "file", "pattern": "**/lib/{fix_data.yaml,fix_data/**.yaml}"},
    ])
}

/// The features the client wants to register dynamically (by client name),
/// and whether it wants `workspace/didChangeConfiguration`.
pub fn dynamic(capabilities: &Value) -> (Vec<&'static str>, bool) {
    let yes = |c: &str| capabilities.pointer(&format!("/textDocument/{c}/dynamicRegistration")).and_then(Value::as_bool) == Some(true);
    let features = FEATURES.iter().filter(|f| yes(f.client)).map(|f| f.client).collect();
    let configuration = capabilities.pointer("/workspace/didChangeConfiguration/dynamicRegistration").and_then(Value::as_bool) == Some(true);
    (features, configuration)
}

/// Removes the capabilities of the dynamic features from the static ones
/// (`staticRegistration`, `feature_registration.dart:246-247`).
pub fn remove_dynamic(capabilities: &mut Value, dynamic: &[&str]) {
    let Some(obj) = capabilities.as_object_mut() else { return };
    for f in FEATURES.iter().filter(|f| dynamic.contains(&f.client)) {
        obj.remove(f.static_);
    }
}

/// `fileOperationRegistrationOptions` (`constants.dart:57-74`): the `.dart`
/// files and the folders.
pub fn file_operation_options() -> Value {
    json!({"filters": [
        {"scheme": "file", "pattern": {"glob": "**/*.dart", "matches": "file"}},
        {"scheme": "file", "pattern": {"glob": "**/", "matches": "folder"}},
    ]})
}

/// The `(method, options)` registrations of the dynamic features, from the
/// static capabilities the server would announce (`registrations`,
/// `server_capabilities_computer.dart:236-255`). `rename_files`: the dynamic
/// `workspace/willRenameFiles` (`WillRenameFilesRegistrations`: the client
/// registers file operations and `updateImportsOnRename` is on).
///
/// `observe_files`: the `workspace/didChangeWatchedFiles` of the `.dart`
/// files, the package YAML files and `package_config.json` (the Dart server
/// watches the disk itself; here the client watches, §3.3: same effect).
pub fn records(statics: &Value, dynamic: &[&str], configuration: bool, rename_files: bool, observe_files: bool) -> Vec<(String, Value)> {
    let mut v = Vec::new();
    for f in FEATURES.iter().filter(|f| dynamic.contains(&f.client)) {
        if f.client == "synchronization" {
            v.push(("textDocument/didOpen".to_string(), json!({"documentSelector": sync_selector()})));
            v.push(("textDocument/didClose".to_string(), json!({"documentSelector": sync_selector()})));
            v.push(("textDocument/didChange".to_string(), json!({"documentSelector": sync_selector(), "syncKind": 2})));
            continue;
        }
        let Some(static_) = statics.get(f.static_) else { continue };
        let mut options = if static_.is_object() { static_.clone() } else { json!({}) };
        options["documentSelector"] = dart_selector();
        let _ = f.dart_only;
        for m in f.methods {
            v.push((m.to_string(), options.clone()));
        }
    }
    if rename_files {
        v.push(("workspace/willRenameFiles".to_string(), file_operation_options()));
    }
    if configuration {
        v.push(("workspace/didChangeConfiguration".to_string(), Value::Null));
    }
    if observe_files {
        v.push((
            "workspace/didChangeWatchedFiles".to_string(),
            json!({"watchers": [
                {"globPattern": "**/*.dart"},
                {"globPattern": "**/pubspec.yaml"},
                {"globPattern": "**/analysis_options.yaml"},
                {"globPattern": "**/.dart_tool/package_config.json"},
            ]}),
        ));
    }
    v
}

/// The current registrations and the id counter.
#[derive(Default, Debug)]
pub struct Records {
    current: Vec<(String, String, Value)>,
    next: u64,
}

impl Records {
    /// The difference against the current ones (`_applyRegistrations`,
    /// `server_capabilities_computer.dart:257-287`): those that leave, by
    /// `{id, method}`, and those that come in, as `Registration`s. The new ones
    /// become the current ones.
    pub fn difference(&mut self, new: Vec<(String, Value)>) -> (Vec<Value>, Vec<Value>) {
        let key = |m: &str, o: &Value| format!("{m}\u{0}{o}");
        let new_keys: Vec<String> = new.iter().map(|(m, o)| key(m, o)).collect();
        let mut exit = Vec::new();
        self.current.retain(|(id, m, o)| {
            let stays = new_keys.contains(&key(m, o));
            if !stays {
                exit.push(json!({"id": id, "method": m}));
            }
            stays
        });
        let mut enter = Vec::new();
        for (m, o) in new {
            if self.current.iter().any(|(_, mv, ov)| key(mv, ov) == key(&m, &o)) {
                continue;
            }
            let id = self.next.to_string();
            self.next += 1;
            enter.push(json!({"id": id, "method": m, "registerOptions": o}));
            self.current.push((id, m, o));
        }
        (exit, enter)
    }
}

/// The client configuration (`client_configuration.dart`), the global one;
/// only the keys this server uses or stores.
#[derive(Debug, Clone, PartialEq)]
pub struct Configuration {
    pub complete_function_calls: bool,
    pub enable_snippets: bool,
    /// `showTodos: true` (`showAllTodos`).
    pub show_all: bool,
    /// `showTodos: [...]` (`showTodoTypes`), in uppercase.
    pub show_all_types: Vec<String>,
    pub max_completion_items: Option<u64>,
    pub line_length: Option<u64>,
    pub rename_files_with_classes: String,
    pub update_imports_on_rename: bool,
    pub enable_sdk_formatter: bool,
    pub analysis_excluded_folders: Vec<String>,
    /// `documentation`: `none`, `summary` or `full`.
    pub documentation: String,
    /// `codeLens` (`LspClientCodeLensConfiguration`): the "Go to Augmentation"
    /// and "Go to Augmented" lenses (a `bool` applies to both; a map enables
    /// them by name, enabled by default).
    pub code_lens_augmentation: bool,
    pub code_lens_augmented: bool,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            complete_function_calls: false,
            enable_snippets: true,
            show_all: false,
            show_all_types: Vec::new(),
            code_lens_augmentation: true,
            code_lens_augmented: true,
            max_completion_items: None,
            line_length: None,
            rename_files_with_classes: "never".to_string(),
            update_imports_on_rename: true,
            enable_sdk_formatter: true,
            analysis_excluded_folders: Vec::new(),
            documentation: "full".to_string(),
        }
    }
}

/// Reads the global `dart` section (the last item of the response; `null`
/// counts as `{}`).
pub fn read_configuration(v: &Value) -> Configuration {
    let mut c = Configuration::default();
    let b = |k: &str| v.get(k).and_then(Value::as_bool);
    if let Some(x) = b("completeFunctionCalls") {
        c.complete_function_calls = x;
    }
    // `enableServerSnippets == false` (the old name) also disables.
    c.enable_snippets = b("enableSnippets").unwrap_or(true) && b("enableServerSnippets") != Some(false);
    // `codeLens`: a `bool` for both, or the map (enabled by default: only an
    // explicit `false` disables).
    match v.get("codeLens") {
        Some(Value::Bool(x)) => {
            c.code_lens_augmentation = *x;
            c.code_lens_augmented = *x;
        }
        Some(Value::Object(m)) => {
            c.code_lens_augmentation = m.get("augmentation") != Some(&Value::Bool(false));
            c.code_lens_augmented = m.get("augmented") != Some(&Value::Bool(false));
        }
        _ => {}
    }
    // `showTodos`: `true` (all), or the list of kinds (in uppercase).
    c.show_all = matches!(v.get("showTodos"), Some(Value::Bool(true)));
    c.show_all_types = match v.get("showTodos") {
        Some(Value::Array(l)) => l.iter().filter_map(Value::as_str).map(str::to_uppercase).collect(),
        _ => Vec::new(),
    };
    c.max_completion_items = v.get("maxCompletionItems").and_then(Value::as_u64);
    c.line_length = v.get("lineLength").and_then(Value::as_u64);
    if let Some(s) = v.get("renameFilesWithClasses").and_then(Value::as_str) {
        c.rename_files_with_classes = s.to_string();
    }
    if let Some(x) = b("updateImportsOnRename") {
        c.update_imports_on_rename = x;
    }
    if let Some(x) = b("enableSdkFormatter") {
        c.enable_sdk_formatter = x;
    }
    c.analysis_excluded_folders = match v.get("analysisExcludedFolders") {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(l)) => l.iter().filter_map(Value::as_str).map(str::to_string).collect(),
        _ => Vec::new(),
    };
    c.documentation = match v.get("documentation").and_then(Value::as_str) {
        Some("none") => "none".to_string(),
        Some("summary") => "summary".to_string(),
        _ => "full".to_string(),
    };
    c
}

/// The configuration of a workspace folder
/// (`LspResourceClientConfiguration` with the global one as fallback,
/// `client_configuration.dart`): only the resource keys
/// (`enableSdkFormatter`, `enableSnippets`, `lineLength`,
/// `maxCompletionItems`, `renameFilesWithClasses`, `updateImportsOnRename`)
/// come from the folder; the others are the global ones.
pub fn feature_configuration(v: &Value, global: &Configuration) -> Configuration {
    let mut c = global.clone();
    let b = |k: &str| v.get(k).and_then(Value::as_bool);
    if let Some(x) = b("enableSdkFormatter") {
        c.enable_sdk_formatter = x;
    }
    if b("enableServerSnippets") == Some(false) {
        c.enable_snippets = false;
    } else if let Some(x) = b("enableSnippets") {
        c.enable_snippets = x;
    }
    if let Some(x) = v.get("lineLength").and_then(Value::as_u64) {
        c.line_length = Some(x);
    }
    if let Some(x) = v.get("maxCompletionItems").and_then(Value::as_u64) {
        c.max_completion_items = Some(x);
    }
    if let Some(s) = v.get("renameFilesWithClasses").and_then(Value::as_str) {
        c.rename_files_with_classes = s.to_string();
    }
    if let Some(x) = b("updateImportsOnRename") {
        c.update_imports_on_rename = x;
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_only_what_client_asks_and_applies_difference() {
        let caps = json!({"textDocument": {"hover": {"dynamicRegistration": true}, "synchronization": {"dynamicRegistration": true}}});
        let (d, conf) = dynamic(&caps);
        assert_eq!(d, vec!["synchronization", "hover"]);
        assert!(!conf);
        let mut statics = json!({"hoverProvider": true, "textDocumentSync": 2, "definitionProvider": true});
        let regs = records(&statics, &d, conf, false, false);
        remove_dynamic(&mut statics, &d);
        assert!(statics.get("hoverProvider").is_none() && statics.get("definitionProvider").is_some());
        assert_eq!(regs.len(), 4);
        let mut r = Records::default();
        let (exit, enter) = r.difference(regs.clone());
        assert!(exit.is_empty());
        assert_eq!(enter.len(), 4);
        assert_eq!(enter[0]["id"], "0");
        // Again, nothing changes.
        let (exit, enter) = r.difference(regs);
        assert!(exit.is_empty() && enter.is_empty());
    }

    #[test]
    fn reads_configuration_with_defaults() {
        let c = read_configuration(&json!({"showTodos": ["fixme"], "enableServerSnippets": false, "analysisExcludedFolders": "gen"}));
        assert!(!c.show_all && !c.enable_snippets);
        assert_eq!(c.show_all_types, vec!["FIXME"]);
        // That of a folder: only the resource keys, the rest from the global one.
        let folder = feature_configuration(&json!({"maxCompletionItems": 7, "completeFunctionCalls": true}), &c);
        assert_eq!(folder.max_completion_items, Some(7));
        assert!(!folder.complete_function_calls && !folder.enable_snippets);
        assert_eq!(c.analysis_excluded_folders, vec!["gen"]);
        assert_eq!(read_configuration(&Value::Null), Configuration::default());
    }
}
