//! O registro dinâmico de capacidades e a configuração do cliente
//! (docs/LSP-ESPECIFICACAO.md §2.4 e §2.5): `server_capabilities_computer.dart`,
//! `feature_registration.dart` e `client_configuration.dart` do servidor do
//! Dart 3.6.2.
//!
//! Uma feature é dinâmica quando o cliente declara `dynamicRegistration`
//! para ela: o campo estático sai do `initialize` e o registro vem depois,
//! por `client/registerCapability`, aplicando a diferença contra o conjunto
//! vigente (primeiro o `client/unregisterCapability` dos que saíram, depois
//! o registro dos novos), comparando por método e opções. Os ids são um
//! contador decimal crescente.
//! Escrito sem compilar nem executar (2026-10-05).

use serde_json::{json, Value};

/// Uma feature: o nome da capacidade no cliente (`textDocument.<name>`), a
/// chave da capacidade estática do servidor e os métodos registrados.
pub struct Feature {
    pub client: &'static str,
    pub static_: &'static str,
    pub methods: &'static [&'static str],
    /// Só os arquivos Dart (o resto vale para `fullySupportedTypes`, que sem
    /// plugins é o mesmo seletor).
    pub so_dart: bool,
}

/// As features que o servidor anuncia e sabem ser dinâmicas.
pub const FEATURES: &[Feature] = &[
    Feature { client: "synchronization", static_: "textDocumentSync", methods: &["textDocument/didOpen", "textDocument/didClose", "textDocument/didChange"], so_dart: false },
    Feature { client: "completion", static_: "completionProvider", methods: &["textDocument/completion"], so_dart: false },
    Feature { client: "hover", static_: "hoverProvider", methods: &["textDocument/hover"], so_dart: false },
    Feature { client: "definition", static_: "definitionProvider", methods: &["textDocument/definition"], so_dart: false },
    Feature { client: "implementation", static_: "implementationProvider", methods: &["textDocument/implementation"], so_dart: false },
    Feature { client: "references", static_: "referencesProvider", methods: &["textDocument/references"], so_dart: false },
    Feature { client: "documentHighlight", static_: "documentHighlightProvider", methods: &["textDocument/documentHighlight"], so_dart: false },
    Feature { client: "documentSymbol", static_: "documentSymbolProvider", methods: &["textDocument/documentSymbol"], so_dart: false },
    Feature { client: "foldingRange", static_: "foldingRangeProvider", methods: &["textDocument/foldingRange"], so_dart: false },
    // A formatação (`handler_format_on_type.dart`, `handler_format_range.dart`,
    // `handler_formatting.dart`), na ordem do `allFeatures`.
    Feature { client: "onTypeFormatting", static_: "documentOnTypeFormattingProvider", methods: &["textDocument/onTypeFormatting"], so_dart: true },
    Feature { client: "rangeFormatting", static_: "documentRangeFormattingProvider", methods: &["textDocument/rangeFormatting"], so_dart: true },
    Feature { client: "formatting", static_: "documentFormattingProvider", methods: &["textDocument/formatting"], so_dart: false },
    Feature { client: "signatureHelp", static_: "signatureHelpProvider", methods: &["textDocument/signatureHelp"], so_dart: false },
    Feature { client: "codeAction", static_: "codeActionProvider", methods: &["textDocument/codeAction"], so_dart: false },
    Feature { client: "rename", static_: "renameProvider", methods: &["textDocument/rename"], so_dart: false },
    Feature { client: "semanticTokens", static_: "semanticTokensProvider", methods: &["textDocument/semanticTokens"], so_dart: false },
    Feature { client: "typeDefinition", static_: "typeDefinitionProvider", methods: &["textDocument/typeDefinition"], so_dart: true },
    Feature { client: "selectionRange", static_: "selectionRangeProvider", methods: &["textDocument/selectionRange"], so_dart: true },
    Feature { client: "callHierarchy", static_: "callHierarchyProvider", methods: &["textDocument/prepareCallHierarchy"], so_dart: true },
    Feature { client: "typeHierarchy", static_: "typeHierarchyProvider", methods: &["textDocument/prepareTypeHierarchy"], so_dart: true },
    Feature { client: "codeLens", static_: "codeLensProvider", methods: &["textDocument/codeLens"], so_dart: true },
    Feature { client: "documentLink", static_: "documentLinkProvider", methods: &["textDocument/documentLink"], so_dart: true },
    Feature { client: "inlayHint", static_: "inlayHintProvider", methods: &["textDocument/inlayHint"], so_dart: true },
    Feature { client: "colorProvider", static_: "colorProvider", methods: &["textDocument/documentColor"], so_dart: true },
];

/// `dartFiles` (`constants.dart`): os arquivos Dart.
fn dart_selector() -> Value {
    json!([{"language": "dart", "scheme": "file"}])
}

/// O seletor da sincronização: Dart, `pubspec.yaml`, `analysis_options.yaml`
/// e os `fix_data` (`handler_text_document_changes.dart:141-179`).
fn sync_selector() -> Value {
    json!([
        {"language": "dart", "scheme": "file"},
        {"language": "yaml", "scheme": "file", "pattern": "**/pubspec.yaml"},
        {"language": "yaml", "scheme": "file", "pattern": "**/analysis_options.yaml"},
        {"language": "yaml", "scheme": "file", "pattern": "**/lib/{fix_data.yaml,fix_data/**.yaml}"},
    ])
}

/// As features que o cliente quer registrar dinamicamente (pelo nome do
/// cliente), e se ele quer o `workspace/didChangeConfiguration`.
pub fn dynamic(capabilities: &Value) -> (Vec<&'static str>, bool) {
    let yes = |c: &str| capabilities.pointer(&format!("/textDocument/{c}/dynamicRegistration")).and_then(Value::as_bool) == Some(true);
    let features = FEATURES.iter().filter(|f| yes(f.client)).map(|f| f.client).collect();
    let configuration = capabilities.pointer("/workspace/didChangeConfiguration/dynamicRegistration").and_then(Value::as_bool) == Some(true);
    (features, configuration)
}

/// Tira das capacidades estáticas as das features dinâmicas
/// (`staticRegistration`, `feature_registration.dart:246-247`).
pub fn remove_dynamic(capabilities: &mut Value, dynamic: &[&str]) {
    let Some(obj) = capabilities.as_object_mut() else { return };
    for f in FEATURES.iter().filter(|f| dynamic.contains(&f.client)) {
        obj.remove(f.static_);
    }
}

/// `fileOperationRegistrationOptions` (`constants.dart:57-74`): os arquivos
/// `.dart` e as pastas.
pub fn file_operation_options() -> Value {
    json!({"filters": [
        {"scheme": "file", "pattern": {"glob": "**/*.dart", "matches": "file"}},
        {"scheme": "file", "pattern": {"glob": "**/", "matches": "folder"}},
    ]})
}

/// Os registros `(método, opções)` das features dinâmicas, a partir das
/// capacidades estáticas que o servidor anunciaria (`registrations`,
/// `server_capabilities_computer.dart:236-255`). `rename_files`: o
/// `workspace/willRenameFiles` dinâmico (`WillRenameFilesRegistrations`: o
/// cliente registra as operações de arquivo e `updateImportsOnRename` está
/// ligado).
///
/// `observe_files`: o `workspace/didChangeWatchedFiles` dos `.dart`, dos
/// YAML do pacote e do `package_config.json` (o servidor do Dart observa o
/// disco ele mesmo; aqui é o cliente quem observa, §3.3: mesmo efeito).
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
        let _ = f.so_dart;
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

/// Os registros vigentes e o contador dos ids.
#[derive(Default, Debug)]
pub struct Records {
    current: Vec<(String, String, Value)>,
    next: u64,
}

impl Records {
    /// A diferença contra os vigentes (`_applyRegistrations`,
    /// `server_capabilities_computer.dart:257-287`): os que saem, por
    /// `{id, method}`, e os que entram, como `Registration`. Os vigentes
    /// passam a ser os novos.
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

/// A configuração do cliente (`client_configuration.dart`), a global; só as
/// chaves que este servidor usa ou guarda.
#[derive(Debug, Clone, PartialEq)]
pub struct Configuration {
    pub complete_function_calls: bool,
    pub enable_snippets: bool,
    /// `showTodos: true` (`showAllTodos`).
    pub show_all: bool,
    /// `showTodos: [...]` (`showTodoTypes`), em maiúsculas.
    pub show_all_types: Vec<String>,
    pub max_completion_items: Option<u64>,
    pub line_length: Option<u64>,
    pub rename_files_with_classes: String,
    pub update_imports_on_rename: bool,
    pub enable_sdk_formatter: bool,
    pub analysis_excluded_folders: Vec<String>,
    /// `documentation`: `none`, `summary` ou `full`.
    pub documentation: String,
    /// `codeLens` (`LspClientCodeLensConfiguration`): as lentes "Go to
    /// Augmentation" e "Go to Augmented" (um `bool` vale para as duas; um
    /// mapa as liga por nome, padrão ligadas).
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

/// Lê a seção `dart` global (o último item da resposta; `null` vale `{}`).
pub fn read_configuration(v: &Value) -> Configuration {
    let mut c = Configuration::default();
    let b = |k: &str| v.get(k).and_then(Value::as_bool);
    if let Some(x) = b("completeFunctionCalls") {
        c.complete_function_calls = x;
    }
    // `enableServerSnippets == false` (o nome antigo) também desliga.
    c.enable_snippets = b("enableSnippets").unwrap_or(true) && b("enableServerSnippets") != Some(false);
    // `codeLens`: um `bool` para as duas, ou o mapa (padrão ligadas: só o
    // `false` explícito desliga).
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
    // `showTodos`: `true` (todos), ou a lista dos tipos (em maiúsculas).
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

/// A configuração de uma pasta do workspace (`LspResourceClientConfiguration`
/// com a global de reserva, `client_configuration.dart`): só as chaves de
/// recurso (`enableSdkFormatter`, `enableSnippets`, `lineLength`,
/// `maxCompletionItems`, `renameFilesWithClasses`, `updateImportsOnRename`)
/// valem da pasta; as outras são da global.
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
        // De novo, nada muda.
        let (exit, enter) = r.difference(regs);
        assert!(exit.is_empty() && enter.is_empty());
    }

    #[test]
    fn reads_configuration_with_defaults() {
        let c = read_configuration(&json!({"showTodos": ["fixme"], "enableServerSnippets": false, "analysisExcludedFolders": "gen"}));
        assert!(!c.show_all && !c.enable_snippets);
        assert_eq!(c.show_all_types, vec!["FIXME"]);
        // A de uma pasta: só as chaves de recurso, o resto da global.
        let folder = feature_configuration(&json!({"maxCompletionItems": 7, "completeFunctionCalls": true}), &c);
        assert_eq!(folder.max_completion_items, Some(7));
        assert!(!folder.complete_function_calls && !folder.enable_snippets);
        assert_eq!(c.analysis_excluded_folders, vec!["gen"]);
        assert_eq!(read_configuration(&Value::Null), Configuration::default());
    }
}
