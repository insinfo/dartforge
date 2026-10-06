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

/// Uma feature: o nome da capacidade no cliente (`textDocument.<nome>`), a
/// chave da capacidade estática do servidor e os métodos registrados.
pub struct Feature {
    pub cliente: &'static str,
    pub estatica: &'static str,
    pub metodos: &'static [&'static str],
    /// Só os arquivos Dart (o resto vale para `fullySupportedTypes`, que sem
    /// plugins é o mesmo seletor).
    pub so_dart: bool,
}

/// As features que o servidor anuncia e sabem ser dinâmicas.
pub const FEATURES: &[Feature] = &[
    Feature { cliente: "synchronization", estatica: "textDocumentSync", metodos: &["textDocument/didOpen", "textDocument/didClose", "textDocument/didChange"], so_dart: false },
    Feature { cliente: "completion", estatica: "completionProvider", metodos: &["textDocument/completion"], so_dart: false },
    Feature { cliente: "hover", estatica: "hoverProvider", metodos: &["textDocument/hover"], so_dart: false },
    Feature { cliente: "definition", estatica: "definitionProvider", metodos: &["textDocument/definition"], so_dart: false },
    Feature { cliente: "implementation", estatica: "implementationProvider", metodos: &["textDocument/implementation"], so_dart: false },
    Feature { cliente: "references", estatica: "referencesProvider", metodos: &["textDocument/references"], so_dart: false },
    Feature { cliente: "documentHighlight", estatica: "documentHighlightProvider", metodos: &["textDocument/documentHighlight"], so_dart: false },
    Feature { cliente: "documentSymbol", estatica: "documentSymbolProvider", metodos: &["textDocument/documentSymbol"], so_dart: false },
    Feature { cliente: "foldingRange", estatica: "foldingRangeProvider", metodos: &["textDocument/foldingRange"], so_dart: false },
    Feature { cliente: "signatureHelp", estatica: "signatureHelpProvider", metodos: &["textDocument/signatureHelp"], so_dart: false },
    Feature { cliente: "codeAction", estatica: "codeActionProvider", metodos: &["textDocument/codeAction"], so_dart: false },
    Feature { cliente: "rename", estatica: "renameProvider", metodos: &["textDocument/rename"], so_dart: false },
    Feature { cliente: "semanticTokens", estatica: "semanticTokensProvider", metodos: &["textDocument/semanticTokens"], so_dart: false },
    Feature { cliente: "typeDefinition", estatica: "typeDefinitionProvider", metodos: &["textDocument/typeDefinition"], so_dart: true },
    Feature { cliente: "selectionRange", estatica: "selectionRangeProvider", metodos: &["textDocument/selectionRange"], so_dart: true },
    Feature { cliente: "callHierarchy", estatica: "callHierarchyProvider", metodos: &["textDocument/prepareCallHierarchy"], so_dart: true },
    Feature { cliente: "typeHierarchy", estatica: "typeHierarchyProvider", metodos: &["textDocument/prepareTypeHierarchy"], so_dart: true },
    Feature { cliente: "codeLens", estatica: "codeLensProvider", metodos: &["textDocument/codeLens"], so_dart: true },
    Feature { cliente: "documentLink", estatica: "documentLinkProvider", metodos: &["textDocument/documentLink"], so_dart: true },
    Feature { cliente: "inlayHint", estatica: "inlayHintProvider", metodos: &["textDocument/inlayHint"], so_dart: true },
    Feature { cliente: "colorProvider", estatica: "colorProvider", metodos: &["textDocument/documentColor"], so_dart: true },
];

/// `dartFiles` (`constants.dart`): os arquivos Dart.
fn seletor_dart() -> Value {
    json!([{"language": "dart", "scheme": "file"}])
}

/// O seletor da sincronização: Dart, `pubspec.yaml`, `analysis_options.yaml`
/// e os `fix_data` (`handler_text_document_changes.dart:141-179`).
fn seletor_da_sincronizacao() -> Value {
    json!([
        {"language": "dart", "scheme": "file"},
        {"language": "yaml", "scheme": "file", "pattern": "**/pubspec.yaml"},
        {"language": "yaml", "scheme": "file", "pattern": "**/analysis_options.yaml"},
        {"language": "yaml", "scheme": "file", "pattern": "**/lib/{fix_data.yaml,fix_data/**.yaml}"},
    ])
}

/// As features que o cliente quer registrar dinamicamente (pelo nome do
/// cliente), e se ele quer o `workspace/didChangeConfiguration`.
pub fn dinamicas(capacidades: &Value) -> (Vec<&'static str>, bool) {
    let sim = |c: &str| capacidades.pointer(&format!("/textDocument/{c}/dynamicRegistration")).and_then(Value::as_bool) == Some(true);
    let features = FEATURES.iter().filter(|f| sim(f.cliente)).map(|f| f.cliente).collect();
    let configuracao = capacidades.pointer("/workspace/didChangeConfiguration/dynamicRegistration").and_then(Value::as_bool) == Some(true);
    (features, configuracao)
}

/// Tira das capacidades estáticas as das features dinâmicas
/// (`staticRegistration`, `feature_registration.dart:246-247`).
pub fn tirar_dinamicas(capacidades: &mut Value, dinamicas: &[&str]) {
    let Some(obj) = capacidades.as_object_mut() else { return };
    for f in FEATURES.iter().filter(|f| dinamicas.contains(&f.cliente)) {
        obj.remove(f.estatica);
    }
}

/// `fileOperationRegistrationOptions` (`constants.dart:57-74`): os arquivos
/// `.dart` e as pastas.
pub fn opcoes_das_operacoes_de_arquivo() -> Value {
    json!({"filters": [
        {"scheme": "file", "pattern": {"glob": "**/*.dart", "matches": "file"}},
        {"scheme": "file", "pattern": {"glob": "**/", "matches": "folder"}},
    ]})
}

/// Os registros `(método, opções)` das features dinâmicas, a partir das
/// capacidades estáticas que o servidor anunciaria (`registrations`,
/// `server_capabilities_computer.dart:236-255`). `renomear_arquivos`: o
/// `workspace/willRenameFiles` dinâmico (`WillRenameFilesRegistrations`: o
/// cliente registra as operações de arquivo e `updateImportsOnRename` está
/// ligado).
///
/// `observar_arquivos`: o `workspace/didChangeWatchedFiles` dos `.dart`, dos
/// YAML do pacote e do `package_config.json` (o servidor do Dart observa o
/// disco ele mesmo; aqui é o cliente quem observa, §3.3: mesmo efeito).
pub fn registros(estaticas: &Value, dinamicas: &[&str], configuracao: bool, renomear_arquivos: bool, observar_arquivos: bool) -> Vec<(String, Value)> {
    let mut v = Vec::new();
    for f in FEATURES.iter().filter(|f| dinamicas.contains(&f.cliente)) {
        if f.cliente == "synchronization" {
            v.push(("textDocument/didOpen".to_string(), json!({"documentSelector": seletor_da_sincronizacao()})));
            v.push(("textDocument/didClose".to_string(), json!({"documentSelector": seletor_da_sincronizacao()})));
            v.push(("textDocument/didChange".to_string(), json!({"documentSelector": seletor_da_sincronizacao(), "syncKind": 2})));
            continue;
        }
        let Some(estatica) = estaticas.get(f.estatica) else { continue };
        let mut opcoes = if estatica.is_object() { estatica.clone() } else { json!({}) };
        opcoes["documentSelector"] = seletor_dart();
        let _ = f.so_dart;
        for m in f.metodos {
            v.push((m.to_string(), opcoes.clone()));
        }
    }
    if renomear_arquivos {
        v.push(("workspace/willRenameFiles".to_string(), opcoes_das_operacoes_de_arquivo()));
    }
    if configuracao {
        v.push(("workspace/didChangeConfiguration".to_string(), Value::Null));
    }
    if observar_arquivos {
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
pub struct Registros {
    vigentes: Vec<(String, String, Value)>,
    proximo: u64,
}

impl Registros {
    /// A diferença contra os vigentes (`_applyRegistrations`,
    /// `server_capabilities_computer.dart:257-287`): os que saem, por
    /// `{id, method}`, e os que entram, como `Registration`. Os vigentes
    /// passam a ser os novos.
    pub fn diferenca(&mut self, novos: Vec<(String, Value)>) -> (Vec<Value>, Vec<Value>) {
        let chave = |m: &str, o: &Value| format!("{m}\u{0}{o}");
        let novas_chaves: Vec<String> = novos.iter().map(|(m, o)| chave(m, o)).collect();
        let mut sair = Vec::new();
        self.vigentes.retain(|(id, m, o)| {
            let fica = novas_chaves.contains(&chave(m, o));
            if !fica {
                sair.push(json!({"id": id, "method": m}));
            }
            fica
        });
        let mut entrar = Vec::new();
        for (m, o) in novos {
            if self.vigentes.iter().any(|(_, mv, ov)| chave(mv, ov) == chave(&m, &o)) {
                continue;
            }
            let id = self.proximo.to_string();
            self.proximo += 1;
            entrar.push(json!({"id": id, "method": m, "registerOptions": o}));
            self.vigentes.push((id, m, o));
        }
        (sair, entrar)
    }
}

/// A configuração do cliente (`client_configuration.dart`), a global; só as
/// chaves que este servidor usa ou guarda.
#[derive(Debug, Clone, PartialEq)]
pub struct Configuracao {
    pub complete_function_calls: bool,
    pub enable_snippets: bool,
    /// `showTodos: true` (`showAllTodos`).
    pub show_todos: bool,
    /// `showTodos: [...]` (`showTodoTypes`), em maiúsculas.
    pub show_todo_types: Vec<String>,
    pub max_completion_items: Option<u64>,
    pub line_length: Option<u64>,
    pub rename_files_with_classes: String,
    pub update_imports_on_rename: bool,
    pub enable_sdk_formatter: bool,
    pub analysis_excluded_folders: Vec<String>,
    /// `documentation`: `none`, `summary` ou `full`.
    pub documentacao: String,
    /// `codeLens` (`LspClientCodeLensConfiguration`): as lentes "Go to
    /// Augmentation" e "Go to Augmented" (um `bool` vale para as duas; um
    /// mapa as liga por nome, padrão ligadas).
    pub code_lens_augmentation: bool,
    pub code_lens_augmented: bool,
}

impl Default for Configuracao {
    fn default() -> Self {
        Configuracao {
            complete_function_calls: false,
            enable_snippets: true,
            show_todos: false,
            show_todo_types: Vec::new(),
            code_lens_augmentation: true,
            code_lens_augmented: true,
            max_completion_items: None,
            line_length: None,
            rename_files_with_classes: "never".to_string(),
            update_imports_on_rename: true,
            enable_sdk_formatter: true,
            analysis_excluded_folders: Vec::new(),
            documentacao: "full".to_string(),
        }
    }
}

/// Lê a seção `dart` global (o último item da resposta; `null` vale `{}`).
pub fn ler_configuracao(v: &Value) -> Configuracao {
    let mut c = Configuracao::default();
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
    c.show_todos = matches!(v.get("showTodos"), Some(Value::Bool(true)));
    c.show_todo_types = match v.get("showTodos") {
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
    c.documentacao = match v.get("documentation").and_then(Value::as_str) {
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
pub fn configuracao_de_recurso(v: &Value, global: &Configuracao) -> Configuracao {
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
mod testes {
    use super::*;

    #[test]
    fn so_registra_o_que_o_cliente_pede_e_aplica_a_diferenca() {
        let caps = json!({"textDocument": {"hover": {"dynamicRegistration": true}, "synchronization": {"dynamicRegistration": true}}});
        let (d, conf) = dinamicas(&caps);
        assert_eq!(d, vec!["synchronization", "hover"]);
        assert!(!conf);
        let mut estaticas = json!({"hoverProvider": true, "textDocumentSync": 2, "definitionProvider": true});
        let regs = registros(&estaticas, &d, conf, false, false);
        tirar_dinamicas(&mut estaticas, &d);
        assert!(estaticas.get("hoverProvider").is_none() && estaticas.get("definitionProvider").is_some());
        assert_eq!(regs.len(), 4);
        let mut r = Registros::default();
        let (sair, entrar) = r.diferenca(regs.clone());
        assert!(sair.is_empty());
        assert_eq!(entrar.len(), 4);
        assert_eq!(entrar[0]["id"], "0");
        // De novo, nada muda.
        let (sair, entrar) = r.diferenca(regs);
        assert!(sair.is_empty() && entrar.is_empty());
    }

    #[test]
    fn le_a_configuracao_com_padroes() {
        let c = ler_configuracao(&json!({"showTodos": ["fixme"], "enableServerSnippets": false, "analysisExcludedFolders": "gen"}));
        assert!(!c.show_todos && !c.enable_snippets);
        assert_eq!(c.show_todo_types, vec!["FIXME"]);
        // A de uma pasta: só as chaves de recurso, o resto da global.
        let pasta = configuracao_de_recurso(&json!({"maxCompletionItems": 7, "completeFunctionCalls": true}), &c);
        assert_eq!(pasta.max_completion_items, Some(7));
        assert!(!pasta.complete_function_calls && !pasta.enable_snippets);
        assert_eq!(c.analysis_excluded_folders, vec!["gen"]);
        assert_eq!(ler_configuracao(&Value::Null), Configuracao::default());
    }
}
