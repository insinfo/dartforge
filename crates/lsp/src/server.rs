//! Despacho do protocolo: um [`Server`] dono de tudo, passado por `&mut`.
//!
//! O desenho copia o do rust-analyzer (`main_loop.rs`, `op_queue.rs`,
//! `global_state.rs`, `mem_docs.rs`): os documentos em memória são versionados
//! e têm dono explícito ([`crate::DocumentStore`]); as mensagens que chegam
//! do `stdin` esperam numa fila; `$/cancelRequest` cancela a requisição ainda
//! na fila com `RequestCancelled`, e a que está em execução checa a
//! cancelamento em pontos seguros.
//!
//! A fila vive **dentro** do servidor (não num canal externo), para que o
//! cancelamento seja determinístico: cada [`Server::pump`] primeiro
//! extrai e aplica *todos* os `$/cancelRequest` da fila — estejam eles antes
//! ou depois da requisição alvo — e só então executa uma requisição. Sem
//! isso, o cancelamento dependeria de a thread leitora ser mais rápida que o
//! despacho, que é condição de corrida, não protocolo.
//!
//! Nada aqui cresce com o número de mensagens: o conjunto de cancelamentos
//! só guarda ids de requisições ainda na fila (cancelar id desconhecido é
//! descartado), e é limpo ao responder; nenhum histórico de versões, nenhum
//! log em memória. Por documento, o único estado é o do [`crate::DocumentStore`].
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

/// Capacidade anunciada de sincronização: incremental (1 = full, 2 = incremental).
const SYNC_INCREMENTAL: u32 = 2;
/// `severity` LSP para todo diagnóstico sintático.
const SEVERITY_ERROR: u32 = 1;

/// `DiagnosticSeverity` do LSP para a severidade do analyzer (1 erro, 2 aviso, 3 informação).
fn severity(s: dartforge_diagnostics::Severidade) -> u32 {
    match s {
        dartforge_diagnostics::Severidade::Error => SEVERITY_ERROR,
        dartforge_diagnostics::Severidade::Warning => 2,
        dartforge_diagnostics::Severidade::Info => 3,
    }
}

/// A mensagem como o servidor do Dart a mostra: o problema e, se houver, a correção.
fn message(d: &dartforge_diagnostics::Diagnostic) -> String {
    match d.correcao() {
        Some(c) => format!("{}\n{c}", d.message),
        None => d.message.clone(),
    }
}
/// Origem publicada em cada diagnóstico.
const SOURCE: &str = "dartforge";

/// Códigos de erro JSON-RPC/LSP usados nas respostas.
const REQUEST_CANCELLED: i32 = -32800;
const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_REQUEST: i32 = -32600;
const SERVER_NOT_INITIALIZED: i32 = -32002;
/// Os erros do servidor do Dart usados pelos comandos
/// (`AS:src/lsp/constants.dart:273-312`).
const ERROR_NOT_HANDLED: i32 = -32001;
const UNKNOWN_COMMAND: i32 = -32005;
const INVALID_COMMAND_ARGUMENTS: i32 = -32006;
const FILE_NOT_ANALYZED: i32 = -32007;
const FILE_WITH_ERRORS: i32 = -32008;
const CLIENT_NOT_APPLIED: i32 = -32009;
const FEATURE_DISABLED: i32 = -32012;
/// Os comandos de `workspace/executeCommand`, na ordem do Dart
/// (`constants.dart:99-111` e `refactoring_processor.dart:20-28`).
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
/// Os `RefactoringKind` que o construtor do protocolo aceita
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
/// As ações de fonte de `codeAction`, na ordem do Dart: título, espécie e
/// comando.
const SOURCE_ACTIONS: &[(&str, &str, &str)] = &[
    ("Sort Members", "source.sortMembers", "dart.edit.sortMembers"),
    ("Organize Imports", "source.organizeImports", "dart.edit.organizeImports"),
    ("Fix All", "source.fixAll", "dart.edit.fixAll"),
];
/// Renomear recusado (nome inválido, elemento externo, conflito): o mesmo
/// código do servidor do Dart (`ServerErrorCodes.RenameNotValid`).
const RENAME_INVALID: i32 = -32010;

/// Teto do conjunto de cancelamentos: defesa contra cliente que cancela ids
/// inexistentes em volume. Acima do teto, o conjunto é esvaziado — o pior
/// caso é uma requisição cancelável executar mesmo assim, nunca retenção.
const MAX_CANCELLATIONS: usize = 4096;

/// Gancho de teste: `dartforge/sleep` com `{"ms": N}` dorme em fatias de
/// 10 ms checando cancelamento, para o teste de aceite exercitar as duas
/// formas de cancelamento (na fila e em execução) sem depender de análise
/// lenta. Clientes reais nunca enviam este método.
const SLEEP_METHOD: &str = "dartforge/dormir";

/// O texto de um arquivo do disco e a tabela de linhas dele.
type TextWithLines = std::sync::Arc<(String, crate::utf16::LineTable)>;

/// Estado global do servidor: dono dos documentos, da fila e dos cancelamentos.
///
/// Parametrizado pelo [`Analyzer`] para que a semântica entre sem tocar no
/// transporte. Nada de `Rc<RefCell<…>>`: o despacho recebe `&mut self`.
#[derive(Debug)]
pub struct Server<A = SyntacticAnalyzer> {
    documents: DocumentStore,
    /// Texto e tabela de linhas dos arquivos do disco (não abertos) que a
    /// requisição em curso converteu para posições LSP: uma busca que
    /// devolve milhares de locais no SDK lê cada arquivo uma vez. Esvaziado
    /// a cada requisição (o disco pode mudar entre elas).
    disk_lines: std::cell::RefCell<HashMap<String, Option<TextWithLines>>>,
    analyzer: A,
    /// Mensagens recebidas ainda não despachadas, em ordem de chegada.
    queue: VecDeque<Value>,
    /// Ids cancelados de requisições ainda não respondidas, em forma canônica.
    cancelled: HashSet<String>,
    /// `shutdown` já foi respondido; `exit` sai com 0, sem ele sai com 1.
    disabling: bool,
    /// O `initialize` já foi respondido: um segundo é recusado (§2.3).
    initialized: bool,
    /// `exit` chegou (ou o fluxo acabou): o laço principal deve terminar.
    shutdown: bool,
    /// Código de saída correspondente.
    code: i32,
    /// O cliente aceita a árvore `DocumentSymbol` (LSP 3.10+).
    hierarchical_symbols: bool,
    hover_markdown: bool,
    /// O cliente aceita `prepareRename` (`rename.prepareSupport`).
    prepare_rename: bool,
    /// O cliente aceita `WorkspaceEdit.documentChanges` (edições com a
    /// versão do documento): as edições saem nessa forma.
    versioned_changes: bool,
    /// O cliente anunciou `experimental.snippetTextEdit`: as ações de código
    /// com seleção ou grupos ligados saem como `SnippetTextEdit`.
    edits_snippet: bool,
    /// O cliente aceita a operação de recurso `rename` e pediu
    /// `renameFilesWithClasses: "always"` nas opções de inicialização.
    rename_files: bool,
    /// O cliente aceita `documentChanges` com a operação `create`: as
    /// correções que criam arquivo (`Create file`) são oferecidas.
    create_files: bool,
    /// O cliente aceita snippets no completar e não pediu
    /// `completeFunctionCalls: false`: chamadas saem com os parênteses e os
    /// parâmetros obrigatórios como marcadores.
    completion_calls: bool,
    /// `suggestFromUnimportedLibraries` (padrão `true`) e o cliente aceita
    /// `workspace/applyEdit`: o completar traz os não importados.
    completion_not_imported: bool,
    /// O cliente aceita Markdown na documentação dos itens.
    documentation_markdown: bool,
    /// As capacidades do completar do cliente (§14.8.2).
    completion_capabilities: crate::completion_item::Capabilities,
    /// Os itens não importados da última resposta, pela chave do `data`
    /// (`ref`): a origem (para a documentação) e o import (o resolve).
    not_imported: HashMap<String, (Option<(std::path::PathBuf, usize)>, Option<crate::completion::AutoImport>, Option<String>)>,
    /// Raízes do workspace anunciadas no `initialize` (`rootUri`,
    /// `workspaceFolders`), para o `workspace/symbol` varrer o disco.
    roots: Vec<std::path::PathBuf>,
    /// `_filesWithClientDiagnostics`: os URIs cujos diagnósticos no
    /// cliente não estão vazios.
    with_client_diagnostics: std::collections::HashSet<String>,
    /// O documento do último `prepareTypeHierarchy` com resposta: o projeto
    /// dos supertipos e subtipos de itens fora do workspace.
    hierarchy_origin: Option<String>,
    /// Trabalhador dos diagnósticos tipados (`crate::typed`), iniciado no
    /// primeiro documento aberto quando o analisador tem SDK.
    typed: Option<crate::typed::Typed>,
    /// O trabalhador já foi tentado (iniciado ou recusado).
    typed_tried: bool,
    /// Chamado pelo trabalhador quando há resultado (acorda o laço do `main`).
    wake: Option<crate::typed::Wake>,
    /// Resultados tipados descartados por versão velha ou documento fechado.
    typed_discarded: usize,
    /// A última publicação tipada de cada documento aberto, com a versão:
    /// as ações de código corrigem o que o editor mostra. Uma entrada por
    /// documento, substituída a cada publicação e removida no `didClose`.
    typed_published: HashMap<String, (i32, Vec<dartforge_diagnostics::Diagnostic>)>,
    /// O cliente puxa os diagnósticos (`textDocument/diagnostic`, LSP 3.17:
    /// anunciou `textDocument.diagnostic`): o servidor anuncia o
    /// `diagnosticProvider` e deixa de empurrar `publishDiagnostics`.
    pulled_diagnostics: bool,
    /// Com os diagnósticos puxados, o cliente aceita
    /// `workspace/diagnostic/refresh`: pedido quando chega um resultado
    /// tipado, para ele puxar de novo.
    update_pulled: bool,
    /// Número do próximo pedido do servidor ao cliente (o id do `refresh`).
    next_request: u64,
    /// O cliente aceita `workspace/applyEdit` (`workspace.applyEdit`): sem
    /// isso não há ações de fonte (docs/LSP-ESPECIFICACAO.md §13.12.1).
    apply_edits: bool,
    /// A opção de inicialização `closingLabels`: o servidor envia
    /// `dart/textDocument/publishClosingLabels` dos documentos abertos.
    closing_labels: bool,
    /// O cliente aceita progresso de trabalho (`window.workDoneProgress`):
    /// o estado da análise sai por `$/progress` com o token `ANALYZING`;
    /// sem isso, por `$/analyzerStatus` (§2.6).
    work_progress: bool,
    /// A análise tipada está em curso e o cliente já foi avisado.
    analyzing: bool,
    /// Mensagens ao cliente produzidas fora de `pump` (o começo da
    /// análise), entregues na próxima passada.
    pending_outputs: Vec<Value>,
    /// A opção de inicialização `outline`: o servidor envia
    /// `dart/textDocument/publishOutline` dos documentos abertos.
    outline: bool,
    /// O cliente aceita anotações de mudança
    /// (`workspace.workspaceEdit.changeAnnotationSupport`).
    change_annotations: bool,
    /// O cliente aceita `CodeAction` literal
    /// (`textDocument.codeAction.codeActionLiteralSupport`); sem isso as
    /// ações de fonte saem como `Command` puro.
    actions_literals: bool,
    /// O `codeActionKind.valueSet` do cliente, quando ele o anuncia.
    action_kinds: Option<Vec<String>>,
    /// Os `workspace/applyEdit` enviados e ainda sem resposta do cliente:
    /// pelo id do pedido, o id do `executeCommand` que espera, o rótulo do
    /// comando e o `WorkspaceEdit` enviado.
    pending_edits: HashMap<String, (Value, &'static str, Value)>,
    /// O cliente aceita Markdown na documentação da ajuda de assinatura.
    signature_markdown: bool,
    /// O cliente aceita `activeParameter: null` (`noActiveParameterSupport`).
    signature_without_active: bool,
    /// O cliente só dobra linhas inteiras (`foldingRange.lineFoldingOnly`).
    folds_line_only: bool,
    /// O cliente aceita tokens semânticos de várias linhas.
    multiline_tokens: bool,
    /// `SymbolKind`s que o cliente anuncia (`documentSymbol.symbolKind`);
    /// sem a lista, só os da primeira versão do protocolo (1 a 18), e
    /// `EnumMember` vira `Enum` como no servidor do Dart.
    symbol_kinds: Option<Vec<u64>>,
    /// `workspace.symbol.symbolKind.valueSet` do cliente.
    workspace_symbol_kinds: Option<Vec<u64>>,
    /// O índice sintático das bibliotecas para o `workspace/symbol`.
    symbol_index: crate::workspace_symbols::Index,
    /// O cliente aceita `labelDetails` nos itens do completar: o rótulo é
    /// só o nome, e a assinatura curta e a biblioteca a importar vão nos
    /// detalhes (como o servidor do Dart faz com esse cliente).
    label_details: bool,
    /// O cliente responde `workspace/configuration` (§2.5).
    requestable_configuration: bool,
    /// As features que o cliente registra dinamicamente (§2.4) e se ele
    /// quer o `workspace/didChangeConfiguration` registrado.
    dynamic: Vec<&'static str>,
    dynamic_configuration: bool,
    /// O cliente registra dinamicamente o `workspace/didChangeWatchedFiles`.
    observe_files: bool,
    /// O cliente registra as operações de arquivo dinamicamente
    /// (`workspace.fileOperations.dynamicRegistration`).
    dynamic_file_operations: bool,
    /// As capacidades estáticas completas (antes de tirar as dinâmicas): a
    /// fonte das opções de cada registro.
    static_capabilities: Value,
    /// Os registros dinâmicos vigentes e o contador dos ids.
    records: crate::registration::Records,
    /// O `workspace/configuration` enviado e ainda sem resposta.
    configuration_request: Option<String>,
    /// As pastas do workspace (`scopeUri` dos itens da configuração).
    folders: Vec<String>,
    /// A configuração vigente do cliente (a global).
    configuration: crate::registration::Configuration,
    /// A configuração de cada pasta do workspace (`forResource`), pelo
    /// caminho da pasta.
    configuration_by_folder: Vec<(std::path::PathBuf, crate::registration::Configuration)>,
    /// O cliente aceita renomear arquivo numa edição (para o
    /// `renameFilesWithClasses: always` da configuração).
    possible_file_rename: bool,
    /// Os comandos que o cliente declara (`experimental.commands`, o
    /// `supportedCommands`): `dart.goToLocation` liga as lentes.
    client_commands: Vec<String>,
    /// O cliente declarou `experimental.supportsWindowShowMessageRequest`
    /// (o `userPromptSender` do Dart): o rename pergunta ao usuário.
    user_questions: bool,
    /// Os `window/showMessageRequest` do rename ainda sem resposta, pelo id
    /// do pedido (§12.1 passos 13 e 17).
    pending_renames: HashMap<String, PendingRename>,
}

/// O `rename` à espera da resposta do usuário.
#[derive(Debug)]
struct PendingRename {
    /// O id do `textDocument/rename`.
    id: Value,
    /// O documento da requisição e a versão dele no começo (`null` se
    /// fechado), para o `fileHasBeenModified`.
    uri: String,
    version: Option<i32>,
    edits: Vec<crate::Edit>,
    file: Option<crate::RenameFile>,
    /// A pergunta feita: `Rename Anyway`/`Cancel` (falso) ou
    /// `Yes`/`No` do arquivo (verdadeiro).
    file_question: bool,
}

impl Server<SyntacticAnalyzer> {
    /// Cria o servidor com o analisador sintático e fila vazia.
    pub fn new() -> Self {
        Self::with_analyzer(SyntacticAnalyzer::new())
    }
}

impl Default for Server<SyntacticAnalyzer> {
    /// Cria o servidor com o analisador sintático e fila vazia.
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Analyzer> Server<A> {
    /// Cria o servidor com um analisador arbitrário (a costura da semântica).
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

    /// Pede a configuração ao cliente (`workspace/configuration`, §2.5):
    /// uma entrada por pasta do workspace, na ordem, e a global por último.
    fn request_configuration(&mut self) {
        self.next_request += 1;
        let id = format!("dartforge/configuration/{}", self.next_request);
        let mut items: Vec<Value> = self.folders.iter().map(|p| json!({"scopeUri": p, "section": "dart"})).collect();
        items.push(json!({"section": "dart"}));
        self.configuration_request = Some(id.clone());
        self.pending_outputs.push(json!({"jsonrpc": "2.0", "id": id, "method": "workspace/configuration", "params": {"items": items}}));
    }

    /// `performDynamicRegistration` (§2.4): a diferença contra os registros
    /// vigentes, primeiro o que sai, depois o que entra; nada sem diferença.
    fn register_dynamic(&mut self) {
        let rename_files = self.dynamic_file_operations && self.configuration.update_imports_on_rename;
        let new = crate::registration::records(
            &self.static_capabilities,
            &self.dynamic,
            self.dynamic_configuration,
            rename_files,
            self.observe_files,
        );
        // `supportsDynamic => enableFormatter && ...` das três features de
        // formatação: com `enableSdkFormatter: false`, elas saem.
        let mut new = new;
        if !self.configuration.enable_sdk_formatter {
            new.retain(|(m, _)| !m.ends_with("ormatting"));
        }
        let (exit, enter) = self.records.difference(new);
        if !exit.is_empty() {
            self.next_request += 1;
            let id = format!("dartforge/unregisterCapability/{}", self.next_request);
            // A grafia do protocolo: `unregisterations`.
            self.pending_outputs.push(json!({"jsonrpc": "2.0", "id": id, "method": "client/unregisterCapability", "params": {"unregisterations": exit}}));
        }
        if !enter.is_empty() {
            self.next_request += 1;
            let id = format!("dartforge/registerCapability/{}", self.next_request);
            self.pending_outputs.push(json!({"jsonrpc": "2.0", "id": id, "method": "client/registerCapability", "params": {"registrations": enter}}));
        }
    }

    /// A resposta ao `workspace/configuration`: só uma lista com uma entrada
    /// por pasta e a global vale (item `null` é `{}`); fora isso, fica a
    /// configuração anterior. Depois, sempre, o registro dinâmico.
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
            // `affectsAnalysisResults` e `affectsAnalysisRoots`: os TODOs ou
            // as pastas excluídas mudaram, a análise refaz.
            let redo = new.show_all != self.configuration.show_all
                || new.show_all_types != self.configuration.show_all_types
                || new.analysis_excluded_folders != self.configuration.analysis_excluded_folders;
            // As das pastas (a lista vem na ordem de `folders`, a global por
            // último).
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

    /// A configuração de `uri` (`forResource`): a da pasta do workspace mais
    /// funda que o contém, senão a global.
    fn configuration_of(&self, uri: &str) -> &crate::registration::Configuration {
        let Some(path) = url::Url::parse(uri).ok().and_then(|u| u.to_file_path().ok()) else { return &self.configuration };
        self.configuration_by_folder
            .iter()
            .filter(|(p, _)| path.starts_with(p))
            .max_by_key(|(p, _)| p.as_os_str().len())
            .map_or(&self.configuration, |(_, c)| c)
    }

    /// As pastas excluídas da análise (`analysisExcludedFolders`): os
    /// caminhos absolutos como estão; os relativos, sob cada pasta do
    /// workspace (`_refreshAnalysisRoots`).
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

    /// O documento está numa pasta excluída da análise.
    fn excluded(&self, uri: &str) -> bool {
        let excluded = self.excluded_folders();
        !excluded.is_empty()
            && url::Url::parse(uri).ok().and_then(|u| u.to_file_path().ok()).is_some_and(|c| excluded.iter().any(|e| c.starts_with(e)))
    }

    /// Os TODOs mostrados ou as pastas excluídas mudaram: o trabalhador
    /// tipado recebe a configuração nova e os documentos abertos são
    /// analisados e publicados de novo.
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

    /// Registra o gancho chamado (de outra thread) quando a análise tipada
    /// tem resultado: o laço do `main` o usa para acordar e chamar
    /// [`Server::pump`], que publica o que ainda for da versão vigente.
    pub fn on_diagnostics(&mut self, hook: impl Fn() + Send + Sync + 'static) {
        self.wake = Some(crate::typed::Wake(std::sync::Arc::new(hook)));
    }

    /// Espera a análise tipada ficar ociosa (até `limit`) e devolve as
    /// publicações que ela produziu, já filtradas pela versão vigente. Para
    /// testes e medições; o editor recebe as mesmas pelo [`Server::pump`].
    pub fn await_diagnostics(&mut self, limit: std::time::Duration) -> Vec<Value> {
        if let Some(t) = &self.typed {
            t.wait_idle(limit);
        }
        self.drain_typed()
    }

    /// Espera a análise tipada ficar ociosa sem publicar o resultado (que
    /// fica para o próximo [`Server::pump`]). Gancho de teste.
    #[doc(hidden)]
    pub fn wait_idle_analysis(&self, limit: std::time::Duration) -> bool {
        self.typed.as_ref().is_none_or(|t| t.wait_idle(limit))
    }

    /// Suspende (ou retoma) o início de análises tipadas. Gancho de teste
    /// para exercitar a coalescência de edições rápidas.
    #[doc(hidden)]
    pub fn pause_typed_analysis(&mut self, pause: bool) {
        self.ensure_typed();
        if let Some(t) = &self.typed {
            t.pause(pause);
        }
    }

    /// Resultados tipados descartados até agora (versão velha ou documento
    /// fechado antes de publicar).
    pub fn typed_discarded(&self) -> usize {
        self.typed_discarded
    }

    /// Inicia o trabalhador tipado uma vez, se o analisador tem SDK.
    fn ensure_typed(&mut self) {
        if self.typed_tried {
            return;
        }
        self.typed_tried = true;
        if let Some(sdk) = crate::typed::analyzer_sdk(&self.analyzer) {
            self.typed = crate::typed::Typed::start(sdk, self.wake.clone());
        }
        // A configuração vigente da análise (os TODOs, as pastas excluídas).
        let excluded = self.excluded_folders();
        if let Some(t) = &self.typed
            && (self.configuration.show_all || !self.configuration.show_all_types.is_empty() || !excluded.is_empty())
        {
            t.configure(self.configuration.show_all, self.configuration.show_all_types.clone(), excluded);
        }
    }

    /// As pastas do workspace para o trabalhador tipado: todo `.dart` delas
    /// é analisado e publicado (§3.2, I7).
    fn analyze_folders(&mut self) {
        self.ensure_typed();
        let Some(t) = &self.typed else { return };
        t.folders(self.roots.clone());
        if !self.roots.is_empty() {
            self.mark_analyzing();
        }
    }

    /// Pede a análise tipada do texto vigente de `uri`.
    fn request_typed(&mut self, uri: &str) {
        self.ensure_typed();
        let (Some(t), Some(version), Some(text)) = (&self.typed, self.documents.version(uri), self.documents.get(uri)) else {
            return;
        };
        t.document(uri, version, text);
        self.mark_analyzing();
    }

    /// O estado da análise (§2.6): ociosa → trabalhando.
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

    /// Publica os resultados tipados prontos que ainda são da versão vigente;
    /// os demais são descartados.
    fn drain_typed(&mut self) -> Vec<Value> {
        let Some(results) = self.typed.as_ref().map(|t| t.receive()) else { return Vec::new() };
        let mut outputs = Vec::new();
        let mut new = false;
        let mut received = false;
        for r in results {
            received = true;
            // Um arquivo das raízes que não está aberto (I7): publica com o
            // texto analisado (do disco); se abriu nesse meio tempo, vale a
            // análise do documento.
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
            // A unidade acabou de ser resolvida para a versão vigente: é o
            // ponto em que o servidor do Dart manda os rótulos de fechamento
            // de um arquivo aberto (§3.4).
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
            // O contorno no formato do Dart, no mesmo ponto (§3.4).
            if self.outline
                && let Some(text) = self.documents.get(&r.uri)
            {
                outputs.push(json!({
                    "jsonrpc": "2.0",
                    "method": "dart/textDocument/publishOutline",
                    "params": {"uri": r.uri, "outline": crate::outline::of_document(text)},
                }));
            }
            self.typed_published.insert(r.uri, (version, r.diagnostics));
            new = true;
        }
        // Puxados: um pedido de `refresh` por lote, e o cliente puxa de novo
        // os documentos visíveis (o `resultId` muda com o resultado tipado).
        if new && self.pulled_diagnostics && self.update_pulled {
            self.next_request += 1;
            outputs.push(json!({
                "jsonrpc": "2.0",
                "id": format!("dartforge/refresh/{}", self.next_request),
                "method": "workspace/diagnostic/refresh",
            }));
        }
        // O estado da análise (§2.6): trabalhando → ociosa. O resultado
        // chega um instante antes de o trabalhador se declarar ocioso; a
        // espera curta cobre essa janela.
        let idle = self.typed.as_ref().is_some_and(|t| t.idle() || (received && t.wait_idle(std::time::Duration::from_millis(20))));
        if self.analyzing && idle {
            self.analyzing = false;
            // Na frente das publicações desta passada: quem lê a última
            // mensagem continua achando a publicação.
            let end = if self.work_progress {
                json!({"jsonrpc": "2.0", "method": "$/progress", "params": {"token": "ANALYZING", "value": {"kind": "end"}}})
            } else {
                json!({"jsonrpc": "2.0", "method": "$/analyzerStatus", "params": {"isAnalyzing": false}})
            };
            outputs.insert(0, end);
        }
        outputs
    }

    /// Enfileira uma mensagem decodificada do transporte, sem despachar.
    ///
    /// Enfileirar nunca responde: as saídas saem de [`Server::pump`].
    pub fn receive(&mut self, message: Value) {
        self.queue.push_back(message);
    }

    /// Há mensagens esperando despacho.
    pub fn has_pending(&self) -> bool {
        !self.queue.is_empty()
    }

    /// O laço principal deve terminar após escrever as saídas devolvidas.
    pub fn must_shutdown(&self) -> bool {
        self.shutdown
    }

    /// Código de saída: 0 após `shutdown`, 1 sem.
    pub fn exit_code(&self) -> i32 {
        self.code
    }

    /// O analisador (medição e testes: estatísticas da sessão semântica).
    pub fn analyzer(&self) -> &A {
        &self.analyzer
    }

    /// Documentos abertos retidos (uso em testes e medição).
    pub fn open_documents(&self) -> usize {
        self.documents.len()
    }

    /// Despacha o próximo passo e devolve as mensagens de saída, em ordem.
    ///
    /// Um passo é: aplicar todos os cancelamentos da fila; processar as
    /// notificações líderes (cada `didOpen`/`didChange` publica
    /// diagnósticos); executar **uma** requisição (ou responder
    /// `RequestCancelled` se ela foi cancelada). Mensagens que não são
    /// requisições nem notificações válidas são ignoradas em silêncio: entre
    /// elas as respostas do cliente ao único pedido do servidor
    /// (`workspace/diagnostic/refresh`), que não levam dado.
    pub fn pump(&mut self) -> Vec<Value> {
        self.apply_cancellations();
        let mut outputs = Vec::new();
        while let Some(next) = self.queue.front() {
            if is_request(next) {
                break;
            }
            let message = self.queue.pop_front().expect("fila não vazia");
            // Um pânico numa notificação: o `showMessage` e o `logMessage` do
            // servidor do Dart (`lsp_analysis_server.dart`, `sendErrorResponse`
            // e `logException`), e a sessão semântica cai.
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.handle_notification(&message))) {
                Ok(Some(output)) => outputs.push(output),
                Ok(None) => {}
                Err(panic) => {
                    self.analyzer.changed_document("");
                    let method = message.get("method").and_then(Value::as_str).unwrap_or("?").to_string();
                    let text = format!("An error occurred while handling {method} notification");
                    register(format!("pânico ao tratar {method}"));
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
            // Um pânico numa consulta não derruba o servidor (o do Dart
            // nunca cai por um pedido): a resposta é um erro interno e a
            // sessão semântica, que pode ter ficado pela metade, cai.
            let response = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.handle_request(&request)));
            match response {
                Ok(r) => outputs.push(r),
                Err(panic) => {
                    // O `UnhandledError` (-32001) do servidor do Dart, com a
                    // mensagem dele, e o `logException` (`window/logMessage`
                    // do tipo erro, com a causa).
                    self.analyzer.changed_document("");
                    let id = request.get("id").cloned().unwrap_or(Value::Null);
                    let method = request.get("method").and_then(Value::as_str).unwrap_or("?").to_string();
                    register(format!("pânico ao tratar {method}"));
                    let text = format!("An error occurred while handling {method} request");
                    outputs.push(error(&id, ERROR_NOT_HANDLED, text.clone()));
                    outputs.push(message_notification("window/logMessage", 1, &format!("{text}: {}", panic_text(&*panic))));
                }
            }
        }
        // Resultados tipados depois das notificações: o que uma mudança já
        // recebida tornou velho é descartado aqui, antes de chegar ao editor.
        // (Uma requisição não muda documentos; a resposta sai primeiro.)
        // O começo da análise vai na frente das saídas da passada.
        if !self.pending_outputs.is_empty() {
            let mut with_pending = std::mem::take(&mut self.pending_outputs);
            with_pending.append(&mut outputs);
            outputs = with_pending;
        }
        outputs.extend(self.drain_typed());
        outputs
    }

    /// Extrai e aplica todo `$/cancelRequest` da fila, seja qual for a posição.
    ///
    /// Só guarda o id quando há requisição com esse id ainda na fila; fora
    /// isso o cancelamento é descartado, para que ids desconhecidos não
    /// façam o conjunto crescer com o número de mensagens.
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
            let cancelled = self.queue.remove(scan).expect("índice varrido");
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

    /// Trata uma notificação; devolve a publicação de diagnósticos quando há.
    fn handle_notification(&mut self, message: &Value) -> Option<Value> {
        if message.get("method").is_none() {
            return self.client_response(message);
        }
        let method = message.get("method")?.as_str()?;
        match method {
            // Com a capacidade, a configuração e depois o registro
            // dinâmico; sem ela, só o registro (§2.2).
            "initialized" => {
                if self.requestable_configuration {
                    self.request_configuration();
                } else {
                    self.register_dynamic();
                }
                // Os arquivos das raízes são analisados e publicados desde o
                // começo (o driver do Dart os adiciona ao criar os
                // contextos, §3.2).
                self.analyze_folders();
                None
            }
            "$/cancelRequest" => None,
            // O `settings` recebido é ignorado: a configuração é pedida de
            // novo (`handler_workspace_configuration.dart:22-31`).
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
                // Dentro das pastas do workspace o `didClose` não limpa: o
                // arquivo volta a ser analisado pelo disco e é republicado
                // (§3.2). Fora delas, a raiz do arquivo some e os resultados
                // dele são limpos (`flushResults`).
                let inside = url::Url::parse(uri).ok().and_then(|u| u.to_file_path().ok()).is_some_and(|c| self.roots.iter().any(|r| c.starts_with(r)));
                if inside && self.typed.is_some() {
                    return None;
                }
                self.filter_repeated_empty(uri, empty_publication(uri))
            }
            // Notificações comuns dos editores que este servidor não usa:
            // aceitas em silêncio (o do Dart trata as duas primeiras e não
            // registra a terceira).
            // As pastas do workspace mudaram: as raízes de análise e as do
            // pedido de configuração acompanham (o `changeNotifications`).
            "workspace/didChangeWorkspaceFolders" => {
                let of_uri = |v: &Value| v.as_str().and_then(|u| url::Url::parse(u).ok()).and_then(|u| u.to_file_path().ok());
                let list = |k: &str| message.pointer(&format!("/params/event/{k}")).and_then(Value::as_array).cloned().unwrap_or_default();
                for f in list("removed") {
                    if let Some(u) = f["uri"].as_str() {
                        self.folders.retain(|p| p != u);
                    }
                    if let Some(p) = of_uri(&f["uri"]) {
                        self.roots.retain(|r| *r != p);
                    }
                }
                for f in list("added") {
                    if let Some(u) = f["uri"].as_str()
                        && !self.folders.iter().any(|p| p == u)
                    {
                        self.folders.push(u.to_string());
                    }
                    if let Some(p) = of_uri(&f["uri"])
                        && p.is_dir()
                        && !self.roots.contains(&p)
                    {
                        self.roots.push(p);
                    }
                }
                // O que a sessão retém pode depender das raízes.
                self.analyzer.changed_document("");
                // As pastas novas entram na análise do trabalhador (I7).
                if self.typed_tried {
                    self.analyze_folders();
                }
                None
            }
            "workspace/didChangeWatchedFiles" => {
                // O observador de arquivos (§3.3): o que mudou no disco.
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
                register(format!("notificação desconhecida ignorada: {method}"));
                // §2.3: `$/…` é ignorada; qualquer outra vira o erro
                // `Unknown method`, que para uma notificação é um
                // `window/showMessage` de erro. Depois do `shutdown`, é
                // descartada.
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

    /// Executa uma requisição e devolve exatamente uma resposta.
    fn handle_request(&mut self, message: &Value) -> Value {
        self.disk_lines.get_mut().clear();
        let id = message.get("id").cloned().unwrap_or(Value::Null);
        let key = key_id(&id);
        if self.cancelled.remove(&key) {
            return error(&id, REQUEST_CANCELLED, "requisição cancelada");
        }
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        // Os estados do servidor do Dart (§2.3) que não dependem da
        // notificação `initialized`: depois do `shutdown` só o `exit` vale, e
        // o `initialize` só vale uma vez.
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
                // `experimental.snippetTextEdit == true`
                // (`client_capabilities.dart:368-371`).
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
                // As pastas do workspace (ou a raiz), para o `scopeUri`.
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
                self.completion_capabilities = crate::completion_item::Capabilities::of_initialize(message.pointer("/params/capabilities").unwrap_or(&Value::Null));
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
                // `handler_code_actions.dart:252-280`: a lista de espécies só
                // para o cliente com literais; senão, `true`.
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
                        // Só os comandos que este servidor executa (o do
                        // Dart anuncia treze, §13.12.4).
                        "executeCommandProvider": {"commands": COMMANDS, "workDoneProgress": true},
                        // Os caracteres do servidor do Dart
                        // (`dartSignatureHelpTriggerCharacters`).
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
                // `workspace`: as pastas e, sem o registro dinâmico das
                // operações de arquivo, o `willRename` estático (com
                // `updateImportsOnRename`, `server_capabilities_computer.dart:197-205`).
                // A formatação (`FormattingRegistrations` e afins): estática
                // quando `enableSdkFormatter` (o padrão).
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
                    // Um documento depende de outros (imports): editar um
                    // muda os diagnósticos de quem o importa. Sem diagnóstico
                    // do workspace inteiro (só dos documentos pedidos).
                    result["capabilities"]["diagnosticProvider"] = json!({
                        "identifier": SOURCE,
                        "interFileDependencies": true,
                        "workspaceDiagnostics": false,
                    });
                }
                // §2.4: as features dinâmicas saem do estático e são
                // registradas depois do `initialized`.
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
                    // A ordem do `SearchEngine` do servidor do Dart (sondado no
                    // 3.6.2): os outros arquivos pelo caminho, depois o que
                    // declara; em cada um, pela posição. A declaração vai no
                    // fim (`_getDeclarations`, §11.5).
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
                    // O formato do servidor do Dart: a descrição em bloco de
                    // código, o tipo e, separada por `---`, a documentação.
                    let content = if self.hover_markdown {
                        // Como o `toHover` do Dart: descrição, tipo, biblioteca
                        // e, depois de `---`, a documentação.
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
                // `maxCompletionItems` e `enableSnippets` são da pasta do
                // documento (`forResource`).
                let request_uri = message.pointer("/params/textDocument/uri").and_then(Value::as_str).unwrap_or_default().to_string();
                let max = self.configuration_of(&request_uri).max_completion_items.map_or(crate::completion::MAX_PATTERN, |m| m as usize);
                self.analyzer.set_completion_max(max);
                self.analyzer.set_not_imported_in_completion(self.completion_not_imported);
                let result = self.request_position(message).and_then(|(u, offset)| {
                    let completion = self.analyzer.completion(&self.documents, &u, offset)?;
                    let text = self.documents.get(&u)?.to_string();
                    let table = self.documents.lines(&u)?;
                    // `computeReplacementRange`: a palavra inteira; `insert`
                    // até o cursor.
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
                    // `_hasExistingArgList`: já há parênteses depois do nome.
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
                    // A documentação de cada item (do arquivo da declaração).
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
                    // Os snippets (sem ranqueamento, no fim).
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
                // `resolveDartCompletion`: só o item com `data` (não
                // importado): o import, a documentação e o `detail`.
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
                let (Some(u), Some((de, until))) = (uri, range2) else {
                    return response(&id, Value::Null);
                };
                let (Some(text), Some(table)) = (self.documents.get(&u), self.documents.lines(&u)) else {
                    return response(&id, Value::Null);
                };
                let start = table.position_offset(text, de.line, de.column);
                let end = table.position_offset(text, until.line, until.column);
                let (start, end) = (start.min(end), start.max(end));
                let only: Option<Vec<String>> = params
                    .and_then(|p| p.pointer("/context/only"))
                    .and_then(Value::as_array)
                    .map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect());
                // Só os tipados publicados para a versão vigente.
                let empty = Vec::new();
                let published = match self.typed_published.get(&u) {
                    Some((v, d)) if Some(*v) == self.documents.version(&u) => d,
                    _ => &empty,
                };
                let actions = self.analyzer.actions(&self.documents, &u, start, end, published);
                let mut output = Vec::new();
                // As ações de fonte (§13.12.1): antes de todas as outras, na
                // forma de comando, sempre oferecidas (o cálculo fica para o
                // `executeCommand`), e só a quem aceita `workspace/applyEdit`.
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
                    // As de fonte do analisador (a edição pronta do
                    // `Organize Imports`) dão lugar aos comandos acima.
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
                        // Só a quem aceita criar arquivo: a capacidade é a
                        // edição inteira ou nada.
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
                    // A forma do servidor do Dart (§13.7.1 item 8 e §13.8.1):
                    // `diagnostics` (o do erro numa correção, vazia numa
                    // assistência), o comando de registro com o id original
                    // da mudança, e nunca `isPreferred`. As duas refatorações
                    // legadas (`Inline`/`Extract Local Variable`) não são
                    // assistências e ficam como estavam.
                    // A assistência `Inline invocation of '…'` (`dart.assist.inline`)
                    // tem a mesma espécie e é assistência.
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
                // As ações de fonte, as correções, as assistências e as
                // refatorações, nesta ordem; correções e assistências passam
                // cada grupo pelo `_CodeActionSorter`.
                let request_column = i64::from(de.column);
                // `getFixActions`/`getAssistActions`: só como `CodeAction`
                // literal; sem `codeActionLiteralSupport`, nenhuma.
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
                // A versão no começo da requisição (`extractDocumentVersion`).
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
                            // ERROR/WARNING do `checkFinalConditions`: sem
                            // `userPromptSender`, `-32010`; com, a pergunta.
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
                // Disparo automático (`(` digitado, ajuda fechada): só se o
                // `(` abre a lista, como o servidor do Dart.
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
                // Sem parâmetro ativo e sem `null` no cliente: um índice fora
                // da lista (o VS Code não destaca nenhum).
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
                // O Dart responde `[]`, nunca `null` (§11.6).
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
            // `crate::move_file`): as URIs das diretivas que chegam aos
            // arquivos movidos e as relativas que saem deles; `null` numa
            // condição fatal.
            "workspace/willRenameFiles" => {
                let mut map = Vec::new();
                for f in message.pointer("/params/files").and_then(Value::as_array).cloned().unwrap_or_default() {
                    let path = |k: &str| f.get(k).and_then(Value::as_str).and_then(|u| url::Url::parse(u).ok()).and_then(|u| u.to_file_path().ok());
                    let (Some(old), Some(new)) = (path("oldUri"), path("newUri")) else {
                        return error(&id, -32602, "URI de arquivo inválida no willRenameFiles");
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
            // `dart/textDocument/super` (o "ir para o super" do Dart-Code).
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
            // `handler_document_link.dart:26-53` (docs/LSP-ESPECIFICACAO.md
            // §8.4): no 3.6.2 o visitante só liga os comentários "See code
            // in examples/api/…" do Flutter; as URIs das diretivas não viram
            // link (o oráculo devolve `[]` nos 36 arquivos, todos com
            // imports). Fora do Flutter a resposta é sempre a lista vazia.
            "textDocument/formatting" | "textDocument/rangeFormatting" | "textDocument/onTypeFormatting" => {
                // `FormattingHandler`, `FormatRangeHandler` e
                // `FormatOnTypeHandler`: só arquivos Dart, nada com
                // `enableSdkFormatter: false`, o `lineLength` do recurso; o
                // `onTypeFormatting` formata o documento inteiro.
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
                    // `toOffset`: linha além do fim é erro; a coluna soma ao
                    // início da linha sem limite (pode passar da quebra).
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
                // Os blocos `{@tool}` que citam o código de exemplo
                // (`crate::links`, o visitor do 3.6.2).
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
            // `handler_code_lens.dart:32-61` (§8.3): só há lentes de
            // augmentations, e só para o cliente que declara o comando
            // `dart.goToLocation`; sem augmentations, lista vazia.
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
            // `textDocument/documentColor` (o `ColorComputer`, §8.2): as
            // cores de 0 a 1.
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
            // `textDocument/colorPresentation`: as quatro formas, com `const`
            // quando a expressão é constante fora de contexto constante; nada
            // fora das raízes de análise ou sem o `Color` do Flutter.
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
            // `dart/textDocument/augmented` e `augmentation` (§8.6).
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
                    // Do mais externo ao mais interno, cada um pai do seguinte.
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
                let range_ = message.pointer("/params/range").and_then(read_range).and_then(|(de, until)| {
                    let text = self.documents.get(&u)?;
                    let table = self.documents.lines(&u)?;
                    Some((table.position_offset(text, de.line, de.column), table.position_offset(text, until.line, until.column)))
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
                // `toServerItem`: o arquivo, o nome em `selectionRange` e o
                // nome exibido do item do cliente.
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
                    // Recebidas: intervalos no arquivo de quem chama; feitas:
                    // no arquivo do item pedido.
                    let file = if received { item.uri.clone() } else { uri.clone() };
                    let ranges: Vec<Value> = spans.iter().filter_map(|s| self.range(&file, *s)).collect();
                    let Some(v) = self.call_item(&item) else { continue };
                    output.push(if received { json!({"from": v, "fromRanges": ranges}) } else { json!({"to": v, "fromRanges": ranges}) });
                }
                response(&id, json!(output))
            }
            "textDocument/prepareTypeHierarchy" => {
                // `isDartDocument`: outro arquivo responde lista vazia.
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
                // `toLspItem(target, unit.lineInfo)`: as posições do arquivo
                // da classe convertidas com as linhas do documento atual.
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
                    // `_convertItems`: cada item com as linhas do próprio
                    // arquivo; o que não se lê fica de fora.
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
                    error(&id, REQUEST_CANCELLED, "requisição cancelada")
                } else {
                    response(&id, Value::Null)
                }
            }
            _ => {
                register(format!("requisição desconhecida: {method}"));
                error(
                    &id,
                    METHOD_NOT_FOUND,
                    format!("Unknown method {method}"),
                )
            }
        }
    }

    /// Um `CallHierarchyItem` (`toLspItem`): a espécie pelo
    /// `toSymbolKindMapping` com o recuo (`File` → `Module`; outra não
    /// anunciada ou desconhecida → `Object`), sem `data`.
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

    /// O offset (bytes) de uma posição LSP de `uri` (aberto ou no disco).
    fn offset_in_file(&self, uri: &str, line: u32, column: u32) -> Option<usize> {
        if let (Some(text), Some(table)) = (self.documents.get(uri), self.documents.lines(uri)) {
            return Some(table.position_offset(text, line, column));
        }
        let path = url::Url::parse(uri).ok()?.to_file_path().ok()?;
        let source = std::fs::read_to_string(path).ok()?;
        let table = crate::utf16::LineTable::build(&source);
        Some(table.position_offset(&source, line, column))
    }

    /// Um `TypeHierarchyItem` (`toLspItem`): espécie 5 (classe), `data` com
    /// o `ElementLocation` (`ref`) e a âncora. Com `lines_of`, os
    /// intervalos são convertidos com as linhas desse documento.
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

    /// O documento cujo projeto atende `typeHierarchy/supertypes` e
    /// `subtypes` do item em `uri`: o próprio arquivo quando está numa raiz
    /// do workspace; senão (SDK, pacote) o do último `prepareTypeHierarchy`.
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

    /// O texto de `uri`: o aberto, senão o do disco.
    fn file_text(&self, uri: &str) -> Option<std::borrow::Cow<'_, str>> {
        if let Some(text) = self.documents.get(uri) {
            return Some(std::borrow::Cow::Borrowed(text));
        }
        let path = url::Url::parse(uri).ok()?.to_file_path().ok()?;
        std::fs::read_to_string(path).ok().map(std::borrow::Cow::Owned)
    }

    /// Um intervalo de `file` convertido com o `LineInfo` de `origin`
    /// (`sourceRangeToRange(unit.lineInfo, …)` do `prepareTypeHierarchy`):
    /// os offsets UTF-16 do arquivo da classe, a linha pelo último início de
    /// linha de `origin` que não passa deles e a coluna sem limite.
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

    /// `workspace/executeCommand` (docs/LSP-ESPECIFICACAO.md §13.12.2 a
    /// §13.12.6). Devolve a resposta, ou, quando o comando tem edição a
    /// aplicar, o pedido `workspace/applyEdit`: a resposta do
    /// `executeCommand` sai quando o cliente responder
    /// ([`Server::client_response`]).
    /// As refatorações de `codeAction` (§13.11.1): depois dos assists, na
    /// ordem do Dart, na forma de comando; só com `workspace/applyEdit` e
    /// com `refactor` pedido. As legadas passam pelo `shouldIncludeKind`; a
    /// do `RefactoringProcessor` (Move) não.
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

    /// O texto vigente do arquivo (o aberto, senão o do disco).
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
        // `parseArgList`: posicional, 6 itens; outra forma vira `{}`.
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

    /// A resposta de uma refatoração calculada. `versioned`: o arquivo e a
    /// versão pedida, para o `ContentModified` do `refactor.perform`.
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
                // `showErrorMessageToUser` e sucesso `null`.
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

    /// Os comandos do `RefactoringProcessor` (§13.11.2, "Comandos do
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
            // `arguments[0] as String`: outro tipo lança.
            match list.first().and_then(Value::as_str) {
                Some(target) => self.analyzer.move_to_file(&self.documents, &uri, start, end - start, target),
                None => crate::RefactoringResult::InternalError("type 'Null' is not a subtype of type 'String' in type cast".to_string()),
            }
        } else {
            // As experimentais (`analyzeAvailability`): sem a mudança.
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
            // Só telemetria no servidor do Dart; sem `action`, a conversão
            // dele lança (`log_action.dart:23`).
            "dart.logAction" => match parameters.get("action").and_then(Value::as_str) {
                Some(_) => response(id, Value::Null),
                None => error(id, ERROR_NOT_HANDLED, "dart.logAction sem o campo action"),
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
                // `hasScanParseErrors`: algum erro do scanner ou do parser.
                let with_errors = analyzed.diagnostics.iter().any(|d| {
                    d.code.is_none_or(|c| {
                        let single = c.info().unico;
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
                    // Os erros semânticos são os da última publicação tipada,
                    // se ela é da versão vigente; sem ela, nenhum import sai
                    // por erro (só as duplicatas textuais).
                    let typed: &[dartforge_diagnostics::Diagnostic] = self
                        .typed_published
                        .get(&uri)
                        .filter(|(version, _)| Some(*version) == self.documents.version(&uri))
                        .map(|(_, d)| d.as_slice())
                        .unwrap_or_default();
                    let name_of =
                        |d: &dartforge_diagnostics::Diagnostic| d.code.map_or("", |c| c.info().unico.rsplit('.').next().unwrap_or(""));
                    let removable: Vec<usize> = typed
                        .iter()
                        .filter(|d| matches!(name_of(d), "DUPLICATE_IMPORT" | "UNUSED_IMPORT" | "UNNECESSARY_IMPORT"))
                        .map(|d| d.span.start)
                        .collect();
                    // Os nove códigos com `isUnresolvedIdentifier`.
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
                // Nada a mudar: `null`, sem falar com o cliente.
                if edits.is_empty() {
                    return response(id, Value::Null);
                }
                let edit = self.workspace_edit(&edits, None);
                self.request_application(id, label, edit)
            }
        }
    }

    /// `Fix All` (docs/LSP-ESPECIFICACAO.md §13.12.7): até quatro passadas;
    /// em cada uma, as correções aplicáveis em lote dos diagnósticos do
    /// arquivo, em ordem de offset, cada produtor de forma atômica e
    /// descartado se conflita. A primeira passada vê os diagnósticos tipados
    /// publicados para a versão vigente; as seguintes rodam sobre o texto já
    /// corrigido (uma cópia dos documentos abertos) e só veem os
    /// diagnósticos imediatos do analisador. Sem recusa por erro de sintaxe.
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
            // Fase A: por diagnóstico, em ordem de offset.
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
            // Fase B: a limpeza de imports, só no pedido manual e numa
            // passada sem edição da fase A. Os erros de import são os
            // tipados, que só valem para o texto original.
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
            // O texto corrigido como documento aberto, para a passada
            // seguinte; os outros documentos abertos seguem como estão.
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
            // O que o analisador retém era do texto anterior.
            self.analyzer.changed_document(uri);
        }
        if overlapping.is_some() {
            // O analisador volta a ver os documentos de verdade.
            self.analyzer.changed_document(uri);
        }
        let edits: Vec<Ed> = match passed.len() {
            0 => return response(id, Value::Null),
            1 => passed.pop().unwrap_or_default(),
            _ => crate::fix_all::merge(passed.concat()),
        };
        // Em LSP, ordem crescente de offset; todas relativas ao original.
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

    /// `Fix All in Workspace` e a prévia (docs/LSP-ESPECIFICACAO.md
    /// §13.12.8): uma única passada, manual, por todos os arquivos `.dart`
    /// não gerados das raízes do workspace e dos documentos abertos, com as
    /// fases A e B do `Fix All`; cada edição leva a anotação com o modelo
    /// da mensagem da correção. Um documento aberto usa os diagnósticos
    /// tipados publicados para a versão vigente; um arquivo só do disco, os
    /// imediatos do analisador.
    fn fix_workspace(&mut self, id: &Value, label: &'static str, confirm: bool) -> Value {
        use crate::fix_all::{Constructor, Ed};
        if !self.apply_edits {
            return error(id, FEATURE_DISABLED, format!("\"{label}\" is only available for clients that support workspace/applyEdit"));
        }
        if !self.change_annotations {
            return error(id, FEATURE_DISABLED, format!("\"{label}\" is only available for clients that support change annotations"));
        }
        // Os alvos: os abertos e, do disco, os que não estão abertos.
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
        // Uma cópia dos abertos, em que cada arquivo do disco entra só
        // enquanto é examinado.
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
            // O analisador volta a ver os documentos de verdade.
            self.analyzer.changed_document("");
        }
        if changes.is_empty() {
            return response(id, Value::Null);
        }
        let mut annotations = serde_json::Map::new();
        let mut by_file: Vec<(String, Vec<Value>)> = Vec::new();
        for (u, edits_) in &changes {
            let mut list = Vec::new();
            // Em LSP, ordem crescente de offset.
            for ((offset, length, new), description) in edits_.iter().rev() {
                let Some(range) = self.range(u, dartforge_diagnostics::Span { start: *offset, end: offset + length }) else { continue };
                // Sem descrição, o último segmento da URI.
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

    /// Um `window/showMessageRequest` do rename; o `rename` fica pendente
    /// até a resposta.
    fn ask_on_rename(&mut self, pending: PendingRename, params: Value) -> Value {
        self.next_request += 1;
        let request = json!(format!("dartforge/prompt/{}", self.next_request));
        self.pending_renames.insert(key_id(&request), pending);
        json!({"jsonrpc": "2.0", "id": request, "method": "window/showMessageRequest", "params": params})
    }

    /// `handler_rename.dart:205-269` depois das condições: a versão do
    /// documento (`-32801`), o arquivo da classe (`renameFilesWithClasses`:
    /// `always` renomeia, `prompt` pergunta `Yes`/`No`) e o `WorkspaceEdit`
    /// com o `RenameFile` no fim.
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
            // `renameFilesWithClasses` da pasta do arquivo (`forResource`).
            let mode = self.configuration_of(&file.de).rename_files_with_classes.clone();
            let renames = if pending.file_question || mode == "always" || (self.rename_files && mode != "never" && mode != "prompt") {
                true
            } else if mode == "prompt" && self.user_questions {
                let base = |u: &str| u.rsplit('/').next().unwrap_or(u).to_string();
                let message = format!("Rename '{}' to '{}'?", base(&file.de), base(&file.para));
                pending.file_question = true;
                pending.file = Some(file);
                return self.ask_on_rename(
                    pending,
                    json!({"type": 3, "message": message, "actions": [{"title": "Yes"}, {"title": "No"}]}),
                );
            } else {
                false
            };
            // Só a operação de arquivo: o servidor do Dart não reescreve as
            // diretivas no `rename` (o cliente pede `willRenameFiles`).
            if renames {
                feature = Some(json!({"kind": "rename", "oldUri": file.de, "newUri": file.para}));
            }
        }
        // A ordem do `SourceChange`: o arquivo da declaração (a primeira
        // edição) e os outros na ordem da busca (pelo caminho).
        if let Some(first) = pending.edits.first().map(|e| e.uri.clone()) {
            pending.edits.sort_by(|a, b| (a.uri != first, &a.uri).cmp(&(b.uri != first, &b.uri)));
        }
        response(&pending.id, self.workspace_edit(&pending.edits, feature))
    }

    /// A resposta do usuário a uma pergunta do rename: `Rename Anyway`
    /// segue (qualquer outra, inclusive `null` ou erro, dá `{}`); `Yes`
    /// renomeia o arquivo (outra resposta, só as edições).
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

    /// O pedido `workspace/applyEdit` de um comando (§13.12.3); o
    /// `executeCommand` de id `id` fica pendente até a resposta do cliente.
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

    /// Uma resposta do cliente a um pedido do servidor. A de um
    /// `workspace/applyEdit` pendente vira a resposta do `executeCommand`
    /// que o enviou (tabela da §13.12.3); as outras não levam dado.
    fn client_response(&mut self, message: &Value) -> Option<Value> {
        let key = key_id(message.get("id")?);
        if self.configuration_request.as_ref().is_some_and(|p| key_id(&json!(p)) == key) {
            self.received_configuration(message);
            return None;
        }
        if let Some(pending) = self.pending_renames.remove(&key) {
            return Some(self.rename_response(pending, message));
        }
        // O cliente recusou o registro dinâmico (`_applyRegistrations`): o
        // erro vai ao log do cliente.
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
            return Some(error(&id, ERROR_NOT_HANDLED, "resposta do workspace/applyEdit sem resultado"));
        };
        if result.get("applied").and_then(Value::as_bool) == Some(true) {
            return Some(response(&id, Value::Null));
        }
        Some(match result.get("failureReason").and_then(Value::as_str) {
            // Sem motivo: o usuário recusou uma prévia; não é erro.
            None => response(&id, Value::Null),
            Some(reason) => error_with_data(
                &id,
                CLIENT_NOT_APPLIED,
                format!("Client failed to apply workspace edit for {label} (reason: {reason})"),
                edit,
            ),
        })
    }

    /// `WorkspaceEdit` das edições: `documentChanges` (cada documento com a
    /// versão vigente, `null` se fechado) quando o cliente aceita, senão
    /// `changes`. Uma operação de recurso (renomear arquivo) vai depois das
    /// edições de texto, que valem para o texto antes dela.
    /// `createWorkspaceEdit(..., allowSnippets: true)` (`mapping.dart:156-213`,
    /// docs/LSP-ESPECIFICACAO.md §13.8.1-G): a ação vira um `TextDocumentEdit`
    /// com um `SnippetTextEdit` só quando o cliente anunciou
    /// `experimental.snippetTextEdit` e `documentChanges`, a mudança é **uma**
    /// edição no arquivo do pedido (que já existe) e há seleção ou grupo
    /// ligado. Senão `None`: a edição simples, sem seleção nem grupos.
    fn edit_snippet(&self, uri: &str, action: &crate::CodeAction) -> Option<Value> {
        if !self.edits_snippet || !self.versioned_changes || action.create_file.is_some() || action.linked.empty() {
            return None;
        }
        let [e] = action.edits.as_slice() else { return None };
        if e.uri != uri {
            return None;
        }
        let range = self.range(&e.uri, e.span)?;
        // Uma só edição: o texto dela começa no mesmo offset no documento
        // final (`toSnippetTextEdits`, `offsetDelta` = 0).
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

    /// `workspace/symbol`: os símbolos dos documentos abertos e dos arquivos
    /// `.dart` dos projetos do workspace (raízes do `initialize` e o projeto,
    /// com `pubspec.yaml`, de cada documento aberto). O texto aberto vale
    /// mais que o disco; cada árvore é descartada antes da próxima. Casa por
    /// aproximação (`crate::fuzzy_matcher`): os que contêm a consulta primeiro,
    /// depois as subsequências; entre iguais, a ordem de (URI, posição).
    fn workspace_symbols(&mut self, query: &str) -> Vec<Value> {
        // `handler_workspace_symbols.dart:28-123` (docs/LSP-ESPECIFICACAO.md
        // §5): consulta vazia não devolve nada.
        if query.is_empty() {
            return Vec::new();
        }
        let mut roots = self.roots.clone();
        // O caminho de cada documento aberto (o da URI `file:`; senão o
        // caminho da URI, só para indexar o texto aberto).
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
                open_texts.insert(dartforge_elements::gerado::chave(&file), (open.to_string(), text.to_string()));
            }
            if file.extension().is_some_and(|x| x == "dart") {
                loose.push(file);
            }
        }
        // `addedFiles`: os arquivos das raízes analisadas; os conhecidos:
        // o SDK e os `lib/` dos pacotes de cada raiz.
        let mut added: Vec<std::path::PathBuf> = roots.iter().flat_map(|r| crate::project::project_files(r)).collect();
        // Um documento aberto fora das raízes também é analisado (o
        // servidor cria um contexto para ele).
        loose.retain(|c| !roots.iter().any(|r| c.starts_with(r)));
        loose.sort();
        added.extend(loose);
        let packages: Vec<std::path::PathBuf> = roots.iter().flat_map(|r| crate::workspace_symbols::package_folders(r)).collect();
        let sdk = dartforge_elements::sdk::SdkLayout::discover();
        let kinds = self.workspace_symbol_kinds.clone();
        self.symbol_index.search(query, &added, sdk.as_deref(), &packages, &open_texts, kinds.as_deref())
    }

    /// URI e offset (bytes) de `params.textDocument` + `params.position`,
    /// quando o documento está aberto.
    fn request_position(&self, message: &Value) -> Option<(String, usize)> {
        let params = message.get("params")?;
        let uri = params.get("textDocument")?.get("uri")?.as_str()?;
        let position = read_position(params.get("position")?)?;
        let text = self.documents.get(uri)?;
        let offset = self.documents.lines(uri)?.position_offset(text, position.line, position.column);
        Some((uri.to_string(), offset))
    }

    /// Intervalo LSP de `span` (bytes) em `uri`: pelo texto aberto, senão
    /// pelo arquivo no disco (renomear toca arquivos fechados do projeto).
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

    /// Diagnostica o documento e monta `textDocument/publishDiagnostics`.
    fn publish(&mut self, uri: &str) -> Value {
        let Some(text) = self.documents.get(uri).map(str::to_string) else {
            return empty_publication(uri);
        };
        // Fora das raízes de análise (`analysisExcludedFolders`): nada.
        if self.excluded(uri) {
            return empty_publication(uri);
        }
        let diagnostics = self.analyzer.diagnose(uri, &text);
        self.publication(uri, &diagnostics)
    }

    /// `_filesWithClientDiagnostics` (`lsp_analysis_server.dart:709-729`):
    /// uma lista vazia para um arquivo que o cliente já tem vazio não é
    /// enviada; as outras atualizam o registro.
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

    /// `publishDiagnostics` de um arquivo não aberto, com o texto analisado.
    /// A publicação dos diagnósticos de um YAML (no JSON do analyzer: linha e
    /// coluna a partir de 1, deslocamento UTF-16): as faixas pelo texto lido.
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

    /// O relatório de `textDocument/diagnostic` de `uri`: o resultado tipado
    /// da versão vigente, senão o do analisador (o mesmo que o
    /// `publishDiagnostics` levaria). O `resultId` é a versão e se o
    /// resultado é o tipado; igual ao `previousResultId`, a resposta é
    /// `unchanged` sem recalcular. Documento que não está aberto não tem
    /// diagnóstico (como o `didClose` do modo empurrado).
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

    /// `publishDiagnostics` de `diagnostics` (spans do texto vigente de `uri`).
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

/// Raízes do workspace no `initialize`: `workspaceFolders`, senão
/// `rootUri`, senão `rootPath`; só diretórios existentes.
/// Os inícios de linha (`LineInfo.lineStarts`) de `text` em unidades
/// UTF-16: depois de `\n`, de `\r\n` e de `\r` isolado.
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
    let of_uri = |v: &Value| {
        v.as_str()
            .and_then(|u| url::Url::parse(u).ok())
            .and_then(|u| u.to_file_path().ok())
    };
    let mut roots: Vec<std::path::PathBuf> = params
        .get("workspaceFolders")
        .and_then(Value::as_array)
        .map(|l| l.iter().filter_map(|f| of_uri(&f["uri"])).collect())
        .unwrap_or_default();
    if roots.is_empty() {
        roots.extend(params.get("rootUri").and_then(of_uri));
    }
    if roots.is_empty() {
        roots.extend(params.get("rootPath").and_then(Value::as_str).map(std::path::PathBuf::from));
    }
    roots.retain(|r| r.is_dir());
    roots
}

/// Dorme até `ms` em fatias de 10 ms; verdadeiro quando cancelado no meio.
///
/// O ponto seguro de cancelamento em execução: entre fatias, sem estado
/// parcial, porque dormir não produz nada. A análise real checará a mesma
/// condição entre documentos, nunca no meio de uma frase do parser.
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

/// Verdadeiro quando a mensagem é requisição (tem `id` e `method` textual).
fn is_request(message: &Value) -> bool {
    message.get("id").is_some() && message.get("method").is_some_and(|m| m.is_string())
}

/// Chave canônica do id (número e texto com o mesmo dígito são ids distintos
/// no JSON-RPC; a serialização canônica preserva a distinção).
fn key_id(id: &Value) -> String {
    id.to_string()
}

/// Resposta de sucesso JSON-RPC.
fn response(id: &Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// Resposta de erro JSON-RPC.
fn error(id: &Value, code: i32, message: impl Into<String>) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message.into()}})
}

/// `window/showMessage` ou `window/logMessage` do `ty` (1 = erro).
fn message_notification(method: &str, ty: i32, text: &str) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": {"type": ty, "message": text}})
}

/// A mensagem de um pânico (o `&str` ou a `String` do `panic!`).
fn panic_text(panic: &(dyn std::any::Any + Send)) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "pânico sem mensagem".to_string())
}

/// A prioridade de uma correção ou assistência pela espécie
/// (docs/LSP-ESPECIFICACAO.md §13.7.2 e §13.3): só as espécies que este
/// servidor emite têm valor próprio; as demais ficam no padrão do grupo (50
/// para correções, 40 para as `*.multi`, 30 para assistências).
fn action_priority(kind: &str) -> i32 {
    // A tabela gerada das espécies do Dart (§13.7.2 e `DartAssistKind`).
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

/// O id original da mudança (`dart.fix.…`, `dart.assist.…`) de uma espécie
/// LSP: o inverso de `toCodeActionKind`.
fn action_id(kind: &str) -> Option<String> {
    if let Some(rest) = kind.strip_prefix("quickfix.") {
        return Some(format!("dart.fix.{rest}"));
    }
    kind.strip_prefix("refactor.").map(|rest| format!("dart.assist.{rest}"))
}

/// `_CodeActionSorter.sort` (`handler_code_actions.dart:284-406`;
/// docs/LSP-ESPECIFICACAO.md §13.7.1, item 9): agrupa por título na ordem
/// da primeira ocorrência; num grupo, fica a ação cujo primeiro diagnóstico
/// começa na coluna mais próxima da do pedido, com os diagnósticos das
/// outras de mesma edição fundidos (as de edição diferente saem); depois,
/// prioridade decrescente e ordem de chegada.
/// O offset em bytes do offset UTF-16 `u` (o fim do texto se passar dele).
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
    // Estável: em empate vale a ordem de chegada.
    unique.sort_by_key(|a| -action_priority(a["kind"].as_str().unwrap_or("")));
    unique
}

/// Resposta de erro JSON-RPC com o campo `data`.
fn error_with_data(id: &Value, code: i32, message: impl Into<String>, given: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message.into(), "data": given}})
}

/// Publicação vazia: documento fechado ou desconhecido não tem diagnósticos.
fn empty_publication(uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": {"uri": uri, "diagnostics": []},
    })
}

/// O LSP exige `SymbolInformation[]` para clientes que não anunciaram suporte
/// a `DocumentSymbol[]` hierárquico. A localização plana é a seleção do nome.
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

/// Converte `contentChanges` do protocolo em mudanças internas.
///
/// Intervalo ausente ou malformado vira substituição integral (nunca `panic`
/// por mensagem malformada do cliente: o pior caso é reanalisar tudo).
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

/// Lê `{start: {line, character}, end: {...}}` em posições internas.
fn read_range(value: &Value) -> Option<(Position, Position)> {
    let start = read_position(value.get("start")?)?;
    let end = read_position(value.get("end")?)?;
    Some((start, end))
}

/// Lê `{line, character}`; número ausente ou negativo vira 0 (satura).
fn read_position(value: &Value) -> Option<Position> {
    let line = value.get("line")?.as_u64()? as u32;
    let column = value.get("character")?.as_u64()? as u32;
    Some(Position { line, column })
}

/// Intervalo LSP (UTF-16) de `start..end` em bytes, saturado no texto.
fn lsp_range(text: &str, table: &crate::utf16::LineTable, start: usize, end: usize) -> Value {
    let start = start.min(text.len());
    let end = end.clamp(start, text.len());
    let (l0, c0) = table.offset_position(text, start);
    let (l1, c1) = table.offset_position(text, end);
    json!({"start": {"line": l0, "character": c0}, "end": {"line": l1, "character": c1}})
}

/// Converte um diagnóstico (span em bytes UTF-8) em diagnóstico LSP.
///
/// Offsets saturados para o texto; fim antes do início colapsa no início.
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
        "code": diagnostic.code.map(|c| c.info().nome),
    })
}

/// Log do servidor: sempre `stderr`, nunca `stdout` (que é só protocolo).
///
/// Um `println!` esquecido corrompe a sessão; esta função existe para que o
/// caminho certo seja o mais curto.
fn register(message: String) {
    eprintln!("[dartforge-lsp] {message}");
}

/// Mapa de teste não usado fora de depuração: registra método de cada mensagem
/// quando `DARTFORGE_LSP_DEBUG` ou `RUST_LOG` indicam verbosidade.
pub(crate) fn detail_level() -> bool {
    std::env::var("DARTFORGE_LSP_DEBUG").is_ok()
        || std::env::var("RUST_LOG")
            .is_ok_and(|v| v.eq_ignore_ascii_case("debug") || v.eq_ignore_ascii_case("trace"))
}

impl<A: Analyzer> Server<A> {
    /// Registra a mensagem recebida em `stderr` quando o nível é detalhado.
    pub fn register_received(&self, message: &Value) {
        if detail_level() {
            let method = message
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or("<resposta>");
            register(format!("recebida: {method}"));
        }
    }
}

/// Documenta os métodos aceitos, para `docs/LSP.md` e para o teste de aceite.
pub fn supported_methods() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        (
            "initialize",
            "requisição: aperta mãos e anuncia capacidades",
        ),
        (
            "shutdown",
            "requisição: prepara o encerramento com código 0",
        ),
        (
            "initialized",
            "notificação: confirmação do cliente, sem efeito",
        ),
        ("exit", "notificação: encerra (0 após shutdown, 1 sem)"),
        (
            "$/cancelRequest",
            "notificação: cancela requisição na fila ou em execução",
        ),
        (
            "textDocument/didOpen",
            "notificação: abre e publica diagnósticos",
        ),
        (
            "textDocument/didChange",
            "notificação: aplica edição incremental e republica",
        ),
        (
            "textDocument/didClose",
            "notificação: fecha e publica lista vazia",
        ),
        (
            "textDocument/documentLink",
            "requisição: lista vazia (no 3.6.2 só há links de exemplos do Flutter)",
        ),
        (
            "textDocument/codeLens",
            "requisição: lista vazia (só há lentes de augmentations)",
        ),
        (
            "textDocument/completion",
            "requisição: membros pelo tipo do receptor, escopo, palavras-chave, nomeados",
        ),
        (
            "textDocument/prepareRename",
            "requisição: intervalo e nome renomeáveis sob o cursor",
        ),
        (
            "textDocument/rename",
            "requisição: edições em todos os arquivos do projeto",
        ),
        (
            "textDocument/codeAction",
            "requisição: inserir ';' e importar biblioteca de nome indefinido",
        ),
        (
            SLEEP_METHOD,
            "requisição: gancho de teste do cancelamento em execução",
        ),
    ])
}

/// Troca, na árvore de símbolos, as espécies que o cliente não anunciou
/// pela alternativa do servidor do Dart (`elementKindToSymbolKind`):
/// `EnumMember` (22) → `Enum` (10), `TypeParameter` (26) → `Variable` (13).
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
