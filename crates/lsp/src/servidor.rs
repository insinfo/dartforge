//! Despacho do protocolo: um [`Servidor`] dono de tudo, passado por `&mut`.
//!
//! O desenho copia o do rust-analyzer (`main_loop.rs`, `op_queue.rs`,
//! `global_state.rs`, `mem_docs.rs`): os documentos em memória são versionados
//! e têm dono explícito ([`crate::DocumentStore`]); as mensagens que chegam
//! do `stdin` esperam numa fila; `$/cancelRequest` cancela a requisição ainda
//! na fila com `RequestCancelled`, e a que está em execução checa a
//! cancelamento em pontos seguros.
//!
//! A fila vive **dentro** do servidor (não num canal externo), para que o
//! cancelamento seja determinístico: cada [`Servidor::bombear`] primeiro
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
//! use dartforge_lsp::Servidor;
//! use serde_json::json;
//! let mut servidor = Servidor::new();
//! servidor.receber(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}));
//! let saidas = servidor.bombear();
//! assert_eq!(saidas.len(), 1);
//! assert!(saidas[0]["result"]["capabilities"].is_object());
//! ```

use crate::{Analisador, AnalisadorSintatico, DocumentStore, MudancaConteudo, Posicao};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};

/// Capacidade anunciada de sincronização: incremental (1 = full, 2 = incremental).
const SINCRONIZACAO_INCREMENTAL: u32 = 2;
/// `severity` LSP para todo diagnóstico sintático.
const SEVERIDADE_ERRO: u32 = 1;

/// `DiagnosticSeverity` do LSP para a severidade do analyzer (1 erro, 2 aviso, 3 informação).
fn severidade(s: dartforge_diagnostics::Severidade) -> u32 {
    match s {
        dartforge_diagnostics::Severidade::Error => SEVERIDADE_ERRO,
        dartforge_diagnostics::Severidade::Warning => 2,
        dartforge_diagnostics::Severidade::Info => 3,
    }
}

/// A mensagem como o servidor do Dart a mostra: o problema e, se houver, a correção.
fn mensagem(d: &dartforge_diagnostics::Diagnostic) -> String {
    match d.correcao() {
        Some(c) => format!("{}\n{c}", d.message),
        None => d.message.clone(),
    }
}
/// Origem publicada em cada diagnóstico.
const FONTE: &str = "dartforge";

/// Códigos de erro JSON-RPC/LSP usados nas respostas.
const REQUEST_CANCELLED: i32 = -32800;
const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_REQUEST: i32 = -32600;
const SERVER_NOT_INITIALIZED: i32 = -32002;
/// Os erros do servidor do Dart usados pelos comandos
/// (`AS:src/lsp/constants.dart:273-312`).
const ERRO_NAO_TRATADO: i32 = -32001;
const COMANDO_DESCONHECIDO: i32 = -32005;
const ARGUMENTOS_DE_COMANDO_INVALIDOS: i32 = -32006;
const ARQUIVO_NAO_ANALISADO: i32 = -32007;
const ARQUIVO_COM_ERROS: i32 = -32008;
const CLIENTE_NAO_APLICOU: i32 = -32009;
const RECURSO_DESLIGADO: i32 = -32012;
/// Os comandos de `workspace/executeCommand`, na ordem do Dart
/// (`constants.dart:99-111` e `refactoring_processor.dart:20-28`).
const COMANDOS: &[&str] = &[
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
const TIPOS_DE_REFATORACAO: &[&str] = &[
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
const FALHA_DE_CALCULO: i32 = -32014;
/// `ContentModified`.
const CONTEUDO_MODIFICADO: i32 = -32801;
/// As ações de fonte de `codeAction`, na ordem do Dart: título, espécie e
/// comando.
const ACOES_DE_FONTE: &[(&str, &str, &str)] = &[
    ("Sort Members", "source.sortMembers", "dart.edit.sortMembers"),
    ("Organize Imports", "source.organizeImports", "dart.edit.organizeImports"),
    ("Fix All", "source.fixAll", "dart.edit.fixAll"),
];
/// Renomear recusado (nome inválido, elemento externo, conflito): o mesmo
/// código do servidor do Dart (`ServerErrorCodes.RenameNotValid`).
const RENOMEAR_INVALIDO: i32 = -32010;

/// Teto do conjunto de cancelamentos: defesa contra cliente que cancela ids
/// inexistentes em volume. Acima do teto, o conjunto é esvaziado — o pior
/// caso é uma requisição cancelável executar mesmo assim, nunca retenção.
const TETO_CANCELAMENTOS: usize = 4096;

/// Gancho de teste: `dartforge/dormir` com `{"ms": N}` dorme em fatias de
/// 10 ms checando cancelamento, para o teste de aceite exercitar as duas
/// formas de cancelamento (na fila e em execução) sem depender de análise
/// lenta. Clientes reais nunca enviam este método.
const METODO_DORMIR: &str = "dartforge/dormir";

/// Estado global do servidor: dono dos documentos, da fila e dos cancelamentos.
///
/// Parametrizado pelo [`Analisador`] para que a semântica entre sem tocar no
/// transporte. Nada de `Rc<RefCell<…>>`: o despacho recebe `&mut self`.
#[derive(Debug)]
pub struct Servidor<A = AnalisadorSintatico> {
    documentos: DocumentStore,
    analisador: A,
    /// Mensagens recebidas ainda não despachadas, em ordem de chegada.
    fila: VecDeque<Value>,
    /// Ids cancelados de requisições ainda não respondidas, em forma canônica.
    cancelados: HashSet<String>,
    /// `shutdown` já foi respondido; `exit` sai com 0, sem ele sai com 1.
    desligando: bool,
    /// O `initialize` já foi respondido: um segundo é recusado (§2.3).
    inicializado: bool,
    /// `exit` chegou (ou o fluxo acabou): o laço principal deve terminar.
    encerrar: bool,
    /// Código de saída correspondente.
    codigo: i32,
    /// O cliente aceita a árvore `DocumentSymbol` (LSP 3.10+).
    simbolos_hierarquicos: bool,
    hover_markdown: bool,
    /// O cliente aceita `prepareRename` (`rename.prepareSupport`).
    preparar_renomeacao: bool,
    /// O cliente aceita `WorkspaceEdit.documentChanges` (edições com a
    /// versão do documento): as edições saem nessa forma.
    mudancas_versionadas: bool,
    /// O cliente aceita a operação de recurso `rename` e pediu
    /// `renameFilesWithClasses: "always"` nas opções de inicialização.
    renomear_arquivos: bool,
    /// O cliente aceita `documentChanges` com a operação `create`: as
    /// correções que criam arquivo (`Create file`) são oferecidas.
    criar_arquivos: bool,
    /// O cliente aceita snippets no completar e não pediu
    /// `completeFunctionCalls: false`: chamadas saem com os parênteses e os
    /// parâmetros obrigatórios como marcadores.
    completar_chamadas: bool,
    /// O cliente aceita Markdown na documentação dos itens.
    documentacao_markdown: bool,
    /// As capacidades do completar do cliente (§14.8.2).
    capacidades_de_completar: crate::item_completar::Capacidades,
    /// Os itens não importados da última resposta, pela chave do `data`
    /// (`ref`): a origem (para a documentação) e o import (o resolve).
    nao_importados: HashMap<String, (Option<(std::path::PathBuf, usize)>, Option<crate::completar::ImportAutomatico>, Option<String>)>,
    /// Raízes do workspace anunciadas no `initialize` (`rootUri`,
    /// `workspaceFolders`), para o `workspace/symbol` varrer o disco.
    raizes: Vec<std::path::PathBuf>,
    /// O documento do último `prepareTypeHierarchy` com resposta: o projeto
    /// dos supertipos e subtipos de itens fora do workspace.
    origem_da_hierarquia: Option<String>,
    /// Trabalhador dos diagnósticos tipados (`crate::tipado`), iniciado no
    /// primeiro documento aberto quando o analisador tem SDK.
    tipado: Option<crate::tipado::Tipado>,
    /// O trabalhador já foi tentado (iniciado ou recusado).
    tipado_tentado: bool,
    /// Chamado pelo trabalhador quando há resultado (acorda o laço do `main`).
    despertar: Option<crate::tipado::Despertar>,
    /// Resultados tipados descartados por versão velha ou documento fechado.
    tipados_descartados: usize,
    /// A última publicação tipada de cada documento aberto, com a versão:
    /// as ações de código corrigem o que o editor mostra. Uma entrada por
    /// documento, substituída a cada publicação e removida no `didClose`.
    tipados_publicados: HashMap<String, (i32, Vec<dartforge_diagnostics::Diagnostic>)>,
    /// O cliente puxa os diagnósticos (`textDocument/diagnostic`, LSP 3.17:
    /// anunciou `textDocument.diagnostic`): o servidor anuncia o
    /// `diagnosticProvider` e deixa de empurrar `publishDiagnostics`.
    diagnosticos_puxados: bool,
    /// Com os diagnósticos puxados, o cliente aceita
    /// `workspace/diagnostic/refresh`: pedido quando chega um resultado
    /// tipado, para ele puxar de novo.
    atualizar_puxados: bool,
    /// Número do próximo pedido do servidor ao cliente (o id do `refresh`).
    proximo_pedido: u64,
    /// O cliente aceita `workspace/applyEdit` (`workspace.applyEdit`): sem
    /// isso não há ações de fonte (docs/LSP-ESPECIFICACAO.md §13.12.1).
    aplicar_edicoes: bool,
    /// A opção de inicialização `closingLabels`: o servidor envia
    /// `dart/textDocument/publishClosingLabels` dos documentos abertos.
    rotulos_de_fechamento: bool,
    /// O cliente aceita progresso de trabalho (`window.workDoneProgress`):
    /// o estado da análise sai por `$/progress` com o token `ANALYZING`;
    /// sem isso, por `$/analyzerStatus` (§2.6).
    progresso_de_trabalho: bool,
    /// A análise tipada está em curso e o cliente já foi avisado.
    analisando: bool,
    /// Mensagens ao cliente produzidas fora de `bombear` (o começo da
    /// análise), entregues na próxima passada.
    saidas_pendentes: Vec<Value>,
    /// A opção de inicialização `outline`: o servidor envia
    /// `dart/textDocument/publishOutline` dos documentos abertos.
    contorno: bool,
    /// O cliente aceita anotações de mudança
    /// (`workspace.workspaceEdit.changeAnnotationSupport`).
    anotacoes_de_mudanca: bool,
    /// O cliente aceita `CodeAction` literal
    /// (`textDocument.codeAction.codeActionLiteralSupport`); sem isso as
    /// ações de fonte saem como `Command` puro.
    acoes_literais: bool,
    /// O `codeActionKind.valueSet` do cliente, quando ele o anuncia.
    especies_de_acao: Option<Vec<String>>,
    /// Os `workspace/applyEdit` enviados e ainda sem resposta do cliente:
    /// pelo id do pedido, o id do `executeCommand` que espera, o rótulo do
    /// comando e o `WorkspaceEdit` enviado.
    edicoes_pendentes: HashMap<String, (Value, &'static str, Value)>,
    /// O cliente aceita Markdown na documentação da ajuda de assinatura.
    assinatura_markdown: bool,
    /// O cliente aceita `activeParameter: null` (`noActiveParameterSupport`).
    assinatura_sem_ativo: bool,
    /// O cliente só dobra linhas inteiras (`foldingRange.lineFoldingOnly`).
    dobras_so_linhas: bool,
    /// O cliente aceita tokens semânticos de várias linhas.
    tokens_multilinha: bool,
    /// `SymbolKind`s que o cliente anuncia (`documentSymbol.symbolKind`);
    /// sem a lista, só os da primeira versão do protocolo (1 a 18), e
    /// `EnumMember` vira `Enum` como no servidor do Dart.
    tipos_de_simbolo: Option<Vec<u64>>,
    /// `workspace.symbol.symbolKind.valueSet` do cliente.
    tipos_de_simbolo_do_workspace: Option<Vec<u64>>,
    /// O índice sintático das bibliotecas para o `workspace/symbol`.
    indice_de_simbolos: crate::simbolos_workspace::Indice,
    /// O cliente aceita `labelDetails` nos itens do completar: o rótulo é
    /// só o nome, e a assinatura curta e a biblioteca a importar vão nos
    /// detalhes (como o servidor do Dart faz com esse cliente).
    rotulo_detalhes: bool,
    /// O cliente responde `workspace/configuration` (§2.5).
    configuracao_pedivel: bool,
    /// As features que o cliente registra dinamicamente (§2.4) e se ele
    /// quer o `workspace/didChangeConfiguration` registrado.
    dinamicas: Vec<&'static str>,
    configuracao_dinamica: bool,
    /// As capacidades estáticas completas (antes de tirar as dinâmicas): a
    /// fonte das opções de cada registro.
    capacidades_estaticas: Value,
    /// Os registros dinâmicos vigentes e o contador dos ids.
    registros: crate::registro::Registros,
    /// O `workspace/configuration` enviado e ainda sem resposta.
    pedido_de_configuracao: Option<String>,
    /// As pastas do workspace (`scopeUri` dos itens da configuração).
    pastas: Vec<String>,
    /// A configuração vigente do cliente (a global).
    configuracao: crate::registro::Configuracao,
    /// O cliente aceita renomear arquivo numa edição (para o
    /// `renameFilesWithClasses: always` da configuração).
    renomeacao_de_arquivo_possivel: bool,
    /// O cliente declarou `experimental.supportsWindowShowMessageRequest`
    /// (o `userPromptSender` do Dart): o rename pergunta ao usuário.
    perguntas_ao_usuario: bool,
    /// Os `window/showMessageRequest` do rename ainda sem resposta, pelo id
    /// do pedido (§12.1 passos 13 e 17).
    renomeacoes_pendentes: HashMap<String, RenomeacaoPendente>,
}

/// O `rename` à espera da resposta do usuário.
struct RenomeacaoPendente {
    /// O id do `textDocument/rename`.
    id: Value,
    /// O documento da requisição e a versão dele no começo (`null` se
    /// fechado), para o `fileHasBeenModified`.
    uri: String,
    versao: Option<i32>,
    edicoes: Vec<crate::Edicao>,
    arquivo: Option<crate::RenomearArquivo>,
    /// A pergunta feita: `Rename Anyway`/`Cancel` (falso) ou
    /// `Yes`/`No` do arquivo (verdadeiro).
    pergunta_do_arquivo: bool,
}

impl Servidor<AnalisadorSintatico> {
    /// Cria o servidor com o analisador sintático e fila vazia.
    pub fn new() -> Self {
        Self::com_analisador(AnalisadorSintatico::new())
    }
}

impl Default for Servidor<AnalisadorSintatico> {
    /// Cria o servidor com o analisador sintático e fila vazia.
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Analisador> Servidor<A> {
    /// Cria o servidor com um analisador arbitrário (a costura da semântica).
    pub fn com_analisador(analisador: A) -> Self {
        Self {
            documentos: DocumentStore::new(),
            analisador,
            fila: VecDeque::new(),
            cancelados: HashSet::new(),
            desligando: false,
            inicializado: false,
            encerrar: false,
            codigo: 1,
            simbolos_hierarquicos: false,
            hover_markdown: false,
            preparar_renomeacao: false,
            mudancas_versionadas: false,
            renomear_arquivos: false,
            criar_arquivos: false,
            completar_chamadas: false,
            capacidades_de_completar: Default::default(),
            nao_importados: HashMap::new(),
            documentacao_markdown: false,
            raizes: Vec::new(),
            origem_da_hierarquia: None,
            tipado: None,
            tipado_tentado: false,
            despertar: None,
            tipados_descartados: 0,
            tipados_publicados: HashMap::new(),
            diagnosticos_puxados: false,
            atualizar_puxados: false,
            proximo_pedido: 0,
            aplicar_edicoes: false,
            rotulos_de_fechamento: false,
            progresso_de_trabalho: false,
            analisando: false,
            saidas_pendentes: Vec::new(),
            contorno: false,
            anotacoes_de_mudanca: false,
            acoes_literais: false,
            especies_de_acao: None,
            edicoes_pendentes: HashMap::new(),
            assinatura_markdown: false,
            assinatura_sem_ativo: false,
            dobras_so_linhas: false,
            tokens_multilinha: false,
            rotulo_detalhes: false,
            tipos_de_simbolo: None,
            tipos_de_simbolo_do_workspace: None,
            indice_de_simbolos: Default::default(),
            configuracao_pedivel: false,
            dinamicas: Vec::new(),
            configuracao_dinamica: false,
            capacidades_estaticas: Value::Null,
            registros: crate::registro::Registros::default(),
            pedido_de_configuracao: None,
            pastas: Vec::new(),
            configuracao: crate::registro::Configuracao::default(),
            renomeacao_de_arquivo_possivel: false,
            perguntas_ao_usuario: false,
            renomeacoes_pendentes: HashMap::new(),
        }
    }

    /// Pede a configuração ao cliente (`workspace/configuration`, §2.5):
    /// uma entrada por pasta do workspace, na ordem, e a global por último.
    fn pedir_configuracao(&mut self) {
        self.proximo_pedido += 1;
        let id = format!("dartforge/configuration/{}", self.proximo_pedido);
        let mut itens: Vec<Value> = self.pastas.iter().map(|p| json!({"scopeUri": p, "section": "dart"})).collect();
        itens.push(json!({"section": "dart"}));
        self.pedido_de_configuracao = Some(id.clone());
        self.saidas_pendentes.push(json!({"jsonrpc": "2.0", "id": id, "method": "workspace/configuration", "params": {"items": itens}}));
    }

    /// `performDynamicRegistration` (§2.4): a diferença contra os registros
    /// vigentes, primeiro o que sai, depois o que entra; nada sem diferença.
    fn registrar_dinamicas(&mut self) {
        let novos = crate::registro::registros(&self.capacidades_estaticas, &self.dinamicas, self.configuracao_dinamica);
        let (sair, entrar) = self.registros.diferenca(novos);
        if !sair.is_empty() {
            self.proximo_pedido += 1;
            let id = format!("dartforge/unregisterCapability/{}", self.proximo_pedido);
            // A grafia do protocolo: `unregisterations`.
            self.saidas_pendentes.push(json!({"jsonrpc": "2.0", "id": id, "method": "client/unregisterCapability", "params": {"unregisterations": sair}}));
        }
        if !entrar.is_empty() {
            self.proximo_pedido += 1;
            let id = format!("dartforge/registerCapability/{}", self.proximo_pedido);
            self.saidas_pendentes.push(json!({"jsonrpc": "2.0", "id": id, "method": "client/registerCapability", "params": {"registrations": entrar}}));
        }
    }

    /// A resposta ao `workspace/configuration`: só uma lista com uma entrada
    /// por pasta e a global vale (item `null` é `{}`); fora isso, fica a
    /// configuração anterior. Depois, sempre, o registro dinâmico.
    fn configuracao_recebida(&mut self, mensagem: &Value) {
        self.pedido_de_configuracao = None;
        if let Some(lista) = mensagem.get("result").and_then(Value::as_array)
            && lista.len() == self.pastas.len() + 1
        {
            let global = lista.last().cloned().unwrap_or(Value::Null);
            let nova = crate::registro::ler_configuracao(&global);
            if nova.rename_files_with_classes == "always" {
                self.renomear_arquivos = self.renomeacao_de_arquivo_possivel;
            } else if nova.rename_files_with_classes == "never" && self.configuracao.rename_files_with_classes != "never" {
                self.renomear_arquivos = false;
            }
            self.configuracao = nova;
        }
        self.registrar_dinamicas();
    }

    /// Registra o gancho chamado (de outra thread) quando a análise tipada
    /// tem resultado: o laço do `main` o usa para acordar e chamar
    /// [`Servidor::bombear`], que publica o que ainda for da versão vigente.
    pub fn ao_ter_diagnosticos(&mut self, gancho: impl Fn() + Send + Sync + 'static) {
        self.despertar = Some(crate::tipado::Despertar(std::sync::Arc::new(gancho)));
    }

    /// Espera a análise tipada ficar ociosa (até `limite`) e devolve as
    /// publicações que ela produziu, já filtradas pela versão vigente. Para
    /// testes e medições; o editor recebe as mesmas pelo [`Servidor::bombear`].
    pub fn aguardar_diagnosticos(&mut self, limite: std::time::Duration) -> Vec<Value> {
        if let Some(t) = &self.tipado {
            t.esperar_ocioso(limite);
        }
        self.drenar_tipados()
    }

    /// Espera a análise tipada ficar ociosa sem publicar o resultado (que
    /// fica para o próximo [`Servidor::bombear`]). Gancho de teste.
    #[doc(hidden)]
    pub fn esperar_analise_ociosa(&self, limite: std::time::Duration) -> bool {
        self.tipado.as_ref().is_none_or(|t| t.esperar_ocioso(limite))
    }

    /// Suspende (ou retoma) o início de análises tipadas. Gancho de teste
    /// para exercitar a coalescência de edições rápidas.
    #[doc(hidden)]
    pub fn pausar_analise_tipada(&mut self, pausar: bool) {
        self.garantir_tipado();
        if let Some(t) = &self.tipado {
            t.pausar(pausar);
        }
    }

    /// Resultados tipados descartados até agora (versão velha ou documento
    /// fechado antes de publicar).
    pub fn tipados_descartados(&self) -> usize {
        self.tipados_descartados
    }

    /// Inicia o trabalhador tipado uma vez, se o analisador tem SDK.
    fn garantir_tipado(&mut self) {
        if self.tipado_tentado {
            return;
        }
        self.tipado_tentado = true;
        if let Some(sdk) = crate::tipado::sdk_do_analisador(&self.analisador) {
            self.tipado = crate::tipado::Tipado::iniciar(sdk, self.despertar.clone());
        }
    }

    /// Pede a análise tipada do texto vigente de `uri`.
    fn pedir_tipado(&mut self, uri: &str) {
        self.garantir_tipado();
        let (Some(t), Some(versao), Some(texto)) = (&self.tipado, self.documentos.version(uri), self.documentos.get(uri)) else {
            return;
        };
        t.documento(uri, versao, texto);
        // O estado da análise (§2.6): ociosa → trabalhando.
        if !self.analisando {
            self.analisando = true;
            if self.progresso_de_trabalho {
                self.proximo_pedido += 1;
                self.saidas_pendentes.push(json!({
                    "jsonrpc": "2.0",
                    "id": format!("dartforge/progress/{}", self.proximo_pedido),
                    "method": "window/workDoneProgress/create",
                    "params": {"token": "ANALYZING"},
                }));
                self.saidas_pendentes.push(json!({
                    "jsonrpc": "2.0",
                    "method": "$/progress",
                    "params": {"token": "ANALYZING", "value": {"kind": "begin", "title": "Analyzing\u{2026}"}},
                }));
            } else {
                self.saidas_pendentes.push(json!({"jsonrpc": "2.0", "method": "$/analyzerStatus", "params": {"isAnalyzing": true}}));
            }
        }
    }

    /// Publica os resultados tipados prontos que ainda são da versão vigente;
    /// os demais são descartados.
    fn drenar_tipados(&mut self) -> Vec<Value> {
        let Some(t) = &self.tipado else { return Vec::new() };
        let mut saidas = Vec::new();
        let mut novos = false;
        let mut recebeu = false;
        for r in t.receber() {
            recebeu = true;
            if self.documentos.version(&r.uri) != Some(r.versao) {
                self.tipados_descartados += 1;
                continue;
            }
            if !self.diagnosticos_puxados {
                saidas.push(self.publicacao(&r.uri, &r.diagnosticos));
            }
            // A unidade acabou de ser resolvida para a versão vigente: é o
            // ponto em que o servidor do Dart manda os rótulos de fechamento
            // de um arquivo aberto (§3.4).
            if self.rotulos_de_fechamento {
                let rotulos = self.analisador.rotulos_de_fechamento(&self.documentos, &r.uri);
                let lista: Vec<Value> = rotulos
                    .iter()
                    .filter_map(|(span, texto)| Some(json!({"range": self.faixa(&r.uri, *span)?, "label": texto})))
                    .collect();
                saidas.push(json!({
                    "jsonrpc": "2.0",
                    "method": "dart/textDocument/publishClosingLabels",
                    "params": {"uri": r.uri, "labels": lista},
                }));
            }
            // O contorno no formato do Dart, no mesmo ponto (§3.4).
            if self.contorno
                && let Some(texto) = self.documentos.get(&r.uri)
            {
                saidas.push(json!({
                    "jsonrpc": "2.0",
                    "method": "dart/textDocument/publishOutline",
                    "params": {"uri": r.uri, "outline": crate::contorno::do_documento(texto)},
                }));
            }
            self.tipados_publicados.insert(r.uri, (r.versao, r.diagnosticos));
            novos = true;
        }
        // Puxados: um pedido de `refresh` por lote, e o cliente puxa de novo
        // os documentos visíveis (o `resultId` muda com o resultado tipado).
        if novos && self.diagnosticos_puxados && self.atualizar_puxados {
            self.proximo_pedido += 1;
            saidas.push(json!({
                "jsonrpc": "2.0",
                "id": format!("dartforge/refresh/{}", self.proximo_pedido),
                "method": "workspace/diagnostic/refresh",
            }));
        }
        // O estado da análise (§2.6): trabalhando → ociosa. O resultado
        // chega um instante antes de o trabalhador se declarar ocioso; a
        // espera curta cobre essa janela.
        if self.analisando && (t.ocioso() || (recebeu && t.esperar_ocioso(std::time::Duration::from_millis(20)))) {
            self.analisando = false;
            // Na frente das publicações desta passada: quem lê a última
            // mensagem continua achando a publicação.
            let fim = if self.progresso_de_trabalho {
                json!({"jsonrpc": "2.0", "method": "$/progress", "params": {"token": "ANALYZING", "value": {"kind": "end"}}})
            } else {
                json!({"jsonrpc": "2.0", "method": "$/analyzerStatus", "params": {"isAnalyzing": false}})
            };
            saidas.insert(0, fim);
        }
        saidas
    }

    /// Enfileira uma mensagem decodificada do transporte, sem despachar.
    ///
    /// Enfileirar nunca responde: as saídas saem de [`Servidor::bombear`].
    pub fn receber(&mut self, mensagem: Value) {
        self.fila.push_back(mensagem);
    }

    /// Há mensagens esperando despacho.
    pub fn tem_pendente(&self) -> bool {
        !self.fila.is_empty()
    }

    /// O laço principal deve terminar após escrever as saídas devolvidas.
    pub fn deve_encerrar(&self) -> bool {
        self.encerrar
    }

    /// Código de saída: 0 após `shutdown`, 1 sem.
    pub fn codigo_saida(&self) -> i32 {
        self.codigo
    }

    /// O analisador (medição e testes: estatísticas da sessão semântica).
    pub fn analisador(&self) -> &A {
        &self.analisador
    }

    /// Documentos abertos retidos (uso em testes e medição).
    pub fn documentos_abertos(&self) -> usize {
        self.documentos.len()
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
    pub fn bombear(&mut self) -> Vec<Value> {
        self.aplicar_cancelamentos();
        let mut saidas = Vec::new();
        while let Some(proxima) = self.fila.front() {
            if eh_requisicao(proxima) {
                break;
            }
            let mensagem = self.fila.pop_front().expect("fila não vazia");
            // Um pânico numa notificação: o `showMessage` e o `logMessage` do
            // servidor do Dart (`lsp_analysis_server.dart`, `sendErrorResponse`
            // e `logException`), e a sessão semântica cai.
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.tratar_notificacao(&mensagem))) {
                Ok(Some(saida)) => saidas.push(saida),
                Ok(None) => {}
                Err(panico) => {
                    self.analisador.documento_alterado("");
                    let metodo = mensagem.get("method").and_then(Value::as_str).unwrap_or("?").to_string();
                    let texto = format!("An error occurred while handling {metodo} notification");
                    registrar(format!("pânico ao tratar {metodo}"));
                    saidas.push(notificacao_de_mensagem("window/showMessage", 1, &texto));
                    saidas.push(notificacao_de_mensagem("window/logMessage", 1, &format!("{texto}: {}", texto_do_panico(&*panico))));
                }
            }
            if self.encerrar {
                return saidas;
            }
        }
        if let Some(requisicao) = self.fila.pop_front() {
            debug_assert!(eh_requisicao(&requisicao));
            // Um pânico numa consulta não derruba o servidor (o do Dart
            // nunca cai por um pedido): a resposta é um erro interno e a
            // sessão semântica, que pode ter ficado pela metade, cai.
            let resposta = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.tratar_requisicao(&requisicao)));
            match resposta {
                Ok(r) => saidas.push(r),
                Err(panico) => {
                    // O `UnhandledError` (-32001) do servidor do Dart, com a
                    // mensagem dele, e o `logException` (`window/logMessage`
                    // do tipo erro, com a causa).
                    self.analisador.documento_alterado("");
                    let id = requisicao.get("id").cloned().unwrap_or(Value::Null);
                    let metodo = requisicao.get("method").and_then(Value::as_str).unwrap_or("?").to_string();
                    registrar(format!("pânico ao tratar {metodo}"));
                    let texto = format!("An error occurred while handling {metodo} request");
                    saidas.push(erro(&id, ERRO_NAO_TRATADO, texto.clone()));
                    saidas.push(notificacao_de_mensagem("window/logMessage", 1, &format!("{texto}: {}", texto_do_panico(&*panico))));
                }
            }
        }
        // Resultados tipados depois das notificações: o que uma mudança já
        // recebida tornou velho é descartado aqui, antes de chegar ao editor.
        // (Uma requisição não muda documentos; a resposta sai primeiro.)
        // O começo da análise vai na frente das saídas da passada.
        if !self.saidas_pendentes.is_empty() {
            let mut com_pendentes = std::mem::take(&mut self.saidas_pendentes);
            com_pendentes.append(&mut saidas);
            saidas = com_pendentes;
        }
        saidas.extend(self.drenar_tipados());
        saidas
    }

    /// Extrai e aplica todo `$/cancelRequest` da fila, seja qual for a posição.
    ///
    /// Só guarda o id quando há requisição com esse id ainda na fila; fora
    /// isso o cancelamento é descartado, para que ids desconhecidos não
    /// façam o conjunto crescer com o número de mensagens.
    fn aplicar_cancelamentos(&mut self) {
        let mut varredura = 0;
        while varredura < self.fila.len() {
            let eh_cancel = self.fila[varredura]
                .get("method")
                .is_some_and(|m| m.as_str() == Some("$/cancelRequest"));
            if !eh_cancel {
                varredura += 1;
                continue;
            }
            let cancelada = self.fila.remove(varredura).expect("índice varrido");
            let alvo = cancelada.get("params").and_then(|p| p.get("id")).cloned();
            if let Some(id) = alvo {
                let chave = chave_id(&id);
                let ha_requisicao = self.fila.iter().any(|m| {
                    eh_requisicao(m) && m.get("id").is_some_and(|outro| chave_id(outro) == chave)
                });
                if ha_requisicao {
                    if self.cancelados.len() >= TETO_CANCELAMENTOS {
                        self.cancelados.clear();
                    }
                    self.cancelados.insert(chave);
                }
            }
        }
    }

    /// Trata uma notificação; devolve a publicação de diagnósticos quando há.
    fn tratar_notificacao(&mut self, mensagem: &Value) -> Option<Value> {
        if mensagem.get("method").is_none() {
            return self.resposta_do_cliente(mensagem);
        }
        let metodo = mensagem.get("method")?.as_str()?;
        match metodo {
            // Com a capacidade, a configuração e depois o registro
            // dinâmico; sem ela, só o registro (§2.2).
            "initialized" => {
                if self.configuracao_pedivel {
                    self.pedir_configuracao();
                } else {
                    self.registrar_dinamicas();
                }
                None
            }
            "$/cancelRequest" => None,
            // O `settings` recebido é ignorado: a configuração é pedida de
            // novo (`handler_workspace_configuration.dart:22-31`).
            "workspace/didChangeConfiguration" => {
                if self.configuracao_pedivel {
                    self.pedir_configuracao();
                }
                None
            }
            "exit" => {
                self.encerrar = true;
                self.codigo = if self.desligando { 0 } else { 1 };
                None
            }
            "textDocument/didOpen" => {
                let params = mensagem.get("params")?;
                let doc = params.get("textDocument")?;
                let uri = doc.get("uri")?.as_str()?;
                let versao = doc.get("version")?.as_i64()? as i32;
                let texto = doc.get("text")?.as_str()?;
                if self.documentos.get(uri).is_some() {
                    self.analisador.documento_fechado(uri);
                }
                self.documentos
                    .open(uri.to_string(), versao, texto.to_string());
                self.analisador.documento_alterado(uri);
                self.pedir_tipado(uri);
                if self.diagnosticos_puxados {
                    return None;
                }
                Some(self.publicar(uri))
            }
            "textDocument/didChange" => {
                let params = mensagem.get("params")?;
                let doc = params.get("textDocument")?;
                let uri = doc.get("uri")?.as_str()?;
                let versao = doc.get("version")?.as_i64()? as i32;
                let mudancas = ler_mudancas(params.get("contentChanges")?);
                if !self.documentos.apply(uri, versao, &mudancas) {
                    return None;
                }
                self.analisador.documento_editado(&self.documentos, uri);
                self.pedir_tipado(uri);
                if self.diagnosticos_puxados {
                    return None;
                }
                Some(self.publicar(uri))
            }
            "textDocument/didClose" => {
                let params = mensagem.get("params")?;
                let uri = params.get("textDocument")?.get("uri")?.as_str()?;
                if self.documentos.close(uri) {
                    self.tipados_publicados.remove(uri);
                    self.analisador.documento_fechado(uri);
                    if let Some(t) = &self.tipado {
                        t.fechado(uri);
                    }
                }
                if self.diagnosticos_puxados {
                    return None;
                }
                Some(publicacao_vazia(uri))
            }
            // Notificações comuns dos editores que este servidor não usa:
            // aceitas em silêncio (o do Dart trata as duas primeiras e não
            // registra a terceira).
            "workspace/didChangeWorkspaceFolders" | "workspace/didChangeWatchedFiles" => None,
            _ => {
                registrar(format!("notificação desconhecida ignorada: {metodo}"));
                // §2.3: `$/…` é ignorada; qualquer outra vira o erro
                // `Unknown method`, que para uma notificação é um
                // `window/showMessage` de erro. Depois do `shutdown`, é
                // descartada.
                if metodo.starts_with("$/") || self.desligando {
                    None
                } else {
                    Some(json!({
                        "jsonrpc": "2.0",
                        "method": "window/showMessage",
                        "params": {"type": 1, "message": format!("Unknown method {metodo}")},
                    }))
                }
            }
        }
    }

    /// Executa uma requisição e devolve exatamente uma resposta.
    fn tratar_requisicao(&mut self, mensagem: &Value) -> Value {
        let id = mensagem.get("id").cloned().unwrap_or(Value::Null);
        let chave = chave_id(&id);
        if self.cancelados.remove(&chave) {
            return erro(&id, REQUEST_CANCELLED, "requisição cancelada");
        }
        let metodo = mensagem.get("method").and_then(Value::as_str).unwrap_or("");
        // Os estados do servidor do Dart (§2.3) que não dependem da
        // notificação `initialized`: depois do `shutdown` só o `exit` vale, e
        // o `initialize` só vale uma vez.
        if self.desligando {
            return erro(&id, INVALID_REQUEST, format!("Unable to handle {metodo} after shutdown request"));
        }
        if metodo == "initialize" && self.inicializado {
            return erro(&id, SERVER_NOT_INITIALIZED, "Server already initialized");
        }
        match metodo {
            "initialize" => {
                self.inicializado = true;
                self.simbolos_hierarquicos = mensagem
                    .pointer("/params/capabilities/textDocument/documentSymbol/hierarchicalDocumentSymbolSupport")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.hover_markdown = mensagem
                    .pointer("/params/capabilities/textDocument/hover/contentFormat")
                    .and_then(Value::as_array)
                    .is_some_and(|formatos| formatos.iter().any(|f| f.as_str() == Some("markdown")));
                self.preparar_renomeacao = mensagem
                    .pointer("/params/capabilities/textDocument/rename/prepareSupport")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.mudancas_versionadas = mensagem
                    .pointer("/params/capabilities/workspace/workspaceEdit/documentChanges")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.aplicar_edicoes =
                    mensagem.pointer("/params/capabilities/workspace/applyEdit").and_then(Value::as_bool).unwrap_or(false);
                self.rotulos_de_fechamento =
                    mensagem.pointer("/params/initializationOptions/closingLabels").and_then(Value::as_bool).unwrap_or(false);
                self.contorno = mensagem.pointer("/params/initializationOptions/outline").and_then(Value::as_bool).unwrap_or(false);
                self.progresso_de_trabalho =
                    mensagem.pointer("/params/capabilities/window/workDoneProgress").and_then(Value::as_bool).unwrap_or(false);
                self.anotacoes_de_mudanca = mensagem
                    .pointer("/params/capabilities/workspace/workspaceEdit/changeAnnotationSupport")
                    .is_some_and(|x| !x.is_null());
                let literais = mensagem.pointer("/params/capabilities/textDocument/codeAction/codeActionLiteralSupport");
                self.acoes_literais = literais.is_some_and(|l| !l.is_null());
                self.especies_de_acao = literais
                    .and_then(|l| l.pointer("/codeActionKind/valueSet"))
                    .and_then(Value::as_array)
                    .map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect());
                let aceita_operacao = |op: &str| {
                    mensagem
                        .pointer("/params/capabilities/workspace/workspaceEdit/resourceOperations")
                        .and_then(Value::as_array)
                        .is_some_and(|l| l.iter().any(|o| o.as_str() == Some(op)))
                };
                let aceita_renomear_arquivo = aceita_operacao("rename");
                self.renomeacao_de_arquivo_possivel = self.mudancas_versionadas && aceita_renomear_arquivo;
                self.perguntas_ao_usuario = mensagem
                    .pointer("/params/capabilities/experimental/supportsWindowShowMessageRequest")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.configuracao_pedivel =
                    mensagem.pointer("/params/capabilities/workspace/configuration").and_then(Value::as_bool).unwrap_or(false);
                let (dinamicas, configuracao_dinamica) =
                    crate::registro::dinamicas(mensagem.pointer("/params/capabilities").unwrap_or(&Value::Null));
                self.dinamicas = dinamicas;
                self.configuracao_dinamica = configuracao_dinamica;
                // As pastas do workspace (ou a raiz), para o `scopeUri`.
                self.pastas = match mensagem.pointer("/params/workspaceFolders").and_then(Value::as_array) {
                    Some(l) => l.iter().filter_map(|p| p.get("uri").and_then(Value::as_str)).map(str::to_string).collect(),
                    None => mensagem.pointer("/params/rootUri").and_then(Value::as_str).map(|u| vec![u.to_string()]).unwrap_or_default(),
                };
                self.criar_arquivos = self.mudancas_versionadas
                    && aceita_operacao("create");
                self.renomear_arquivos = self.mudancas_versionadas
                    && aceita_renomear_arquivo
                    && mensagem.pointer("/params/initializationOptions/renameFilesWithClasses").and_then(Value::as_str) == Some("always");
                self.raizes = raizes_do_initialize(mensagem.get("params"));
                self.completar_chamadas = mensagem
                    .pointer("/params/capabilities/textDocument/completion/completionItem/snippetSupport")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    && mensagem.pointer("/params/initializationOptions/completeFunctionCalls").and_then(Value::as_bool) != Some(false);
                self.capacidades_de_completar = crate::item_completar::Capacidades::do_initialize(mensagem.pointer("/params/capabilities").unwrap_or(&Value::Null));
                self.documentacao_markdown = mensagem
                    .pointer("/params/capabilities/textDocument/completion/completionItem/documentationFormat")
                    .and_then(Value::as_array)
                    .is_some_and(|f| f.iter().any(|x| x.as_str() == Some("markdown")));
                self.diagnosticos_puxados = mensagem.pointer("/params/capabilities/textDocument/diagnostic").is_some_and(Value::is_object);
                self.atualizar_puxados = mensagem
                    .pointer("/params/capabilities/workspace/diagnostics/refreshSupport")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.assinatura_markdown = mensagem
                    .pointer("/params/capabilities/textDocument/signatureHelp/signatureInformation/documentationFormat")
                    .and_then(Value::as_array)
                    .is_some_and(|f| f.iter().any(|x| x.as_str() == Some("markdown")));
                self.assinatura_sem_ativo = mensagem
                    .pointer("/params/capabilities/textDocument/signatureHelp/signatureInformation/noActiveParameterSupport")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.tipos_de_simbolo = mensagem
                    .pointer("/params/capabilities/textDocument/documentSymbol/symbolKind/valueSet")
                    .and_then(Value::as_array)
                    .map(|l| l.iter().filter_map(Value::as_u64).collect());
                self.tipos_de_simbolo_do_workspace = mensagem
                    .pointer("/params/capabilities/workspace/symbol/symbolKind/valueSet")
                    .and_then(Value::as_array)
                    .map(|l| l.iter().filter_map(Value::as_u64).collect());
                self.rotulo_detalhes = mensagem
                    .pointer("/params/capabilities/textDocument/completion/completionItem/labelDetailsSupport")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.tokens_multilinha = mensagem
                    .pointer("/params/capabilities/textDocument/semanticTokens/multilineTokenSupport")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.dobras_so_linhas = mensagem
                    .pointer("/params/capabilities/textDocument/foldingRange/lineFoldingOnly")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let renomear = if self.preparar_renomeacao { json!({"prepareProvider": true}) } else { json!(true) };
                // `handler_code_actions.dart:252-280`: a lista de espécies só
                // para o cliente com literais; senão, `true`.
                let acoes_de_codigo = if self.acoes_literais {
                    json!({"codeActionKinds": ["source", "source.organizeImports", "source.fixAll", "source.sortMembers", "quickfix", "refactor"]})
                } else {
                    json!(true)
                };
                let mut resultado = json!({
                    "capabilities": {
                        "textDocumentSync": SINCRONIZACAO_INCREMENTAL,
                        "positionEncoding": "utf-16",
                        "documentSymbolProvider": true,
                        "workspaceSymbolProvider": true,
                        "definitionProvider": true,
                        "referencesProvider": true,
                        "hoverProvider": true,
                        "renameProvider": renomear,
                        "codeActionProvider": acoes_de_codigo,
                        // Só os comandos que este servidor executa (o do
                        // Dart anuncia treze, §13.12.4).
                        "executeCommandProvider": {"commands": COMANDOS, "workDoneProgress": true},
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
                        "semanticTokensProvider": {
                            "legend": {"tokenTypes": crate::realce::TIPOS, "tokenModifiers": crate::realce::MODIFICADORES},
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
                if self.diagnosticos_puxados {
                    // Um documento depende de outros (imports): editar um
                    // muda os diagnósticos de quem o importa. Sem diagnóstico
                    // do workspace inteiro (só dos documentos pedidos).
                    resultado["capabilities"]["diagnosticProvider"] = json!({
                        "identifier": FONTE,
                        "interFileDependencies": true,
                        "workspaceDiagnostics": false,
                    });
                }
                // §2.4: as features dinâmicas saem do estático e são
                // registradas depois do `initialized`.
                self.capacidades_estaticas = resultado["capabilities"].clone();
                crate::registro::tirar_dinamicas(&mut resultado["capabilities"], &self.dinamicas);
                resposta(&id, resultado)
            }
            "textDocument/diagnostic" => {
                let uri = mensagem.pointer("/params/textDocument/uri").and_then(Value::as_str).map(str::to_string);
                let anterior = mensagem.pointer("/params/previousResultId").and_then(Value::as_str);
                resposta(&id, uri.map_or_else(|| json!({"kind": "full", "items": []}), |u| self.relatorio_puxado(&u, anterior)))
            }
            "shutdown" => {
                self.desligando = true;
                resposta(&id, Value::Null)
            }
            "textDocument/documentSymbol" => {
                let uri = mensagem.get("params")
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|d| d.get("uri"))
                    .and_then(Value::as_str);
                let mut simbolos = uri
                    .and_then(|u| self.documentos.get(u).map(|t| (u, t.to_string())))
                    .map_or_else(Vec::new, |(u, t)| self.analisador.simbolos(u, &t));
                for s in &mut simbolos {
                    ajustar_especies(s, self.tipos_de_simbolo.as_deref());
                }
                let resultado = if self.simbolos_hierarquicos {
                    simbolos
                } else {
                    let mut planos = Vec::new();
                    for simbolo in &simbolos {
                        achatar_simbolos(simbolo, uri.unwrap_or(""), None, &mut planos);
                    }
                    planos
                };
                resposta(&id, json!(resultado))
            }
            "workspace/symbol" => {
                let consulta = mensagem.get("params")
                    .and_then(|p| p.get("query"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                resposta(&id, json!(self.simbolos_do_workspace(&consulta)))
            }
            "textDocument/definition" => {
                let params = mensagem.get("params");
                let uri = params
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|d| d.get("uri"))
                    .and_then(Value::as_str);
                let posicao = params.and_then(|p| p.get("position")).and_then(ler_posicao);
                let resultado = uri.zip(posicao).and_then(|(u, p)| {
                    let texto = self.documentos.get(u)?.to_string();
                    let offset = self.documentos.linhas(u)?
                        .offset_de_posicao(&texto, p.linha, p.coluna);
                    let (destino, selecao) = self.analisador.definicao_no_workspace(u, &texto, offset, &self.documentos)?;
                    let range = if let Some(s) = selecao {
                        let (l0, c0, l1, c1) = if let Some(fonte) = self.documentos.get(&destino) {
                            let tabela = self.documentos.linhas(&destino)?;
                            let (l0, c0) = tabela.posicao_de_offset(fonte, s.start);
                            let (l1, c1) = tabela.posicao_de_offset(fonte, s.end);
                            (l0, c0, l1, c1)
                        } else {
                            let caminho = url::Url::parse(&destino).ok()?.to_file_path().ok()?;
                            let fonte = std::fs::read_to_string(caminho).ok()?;
                            let tabela = crate::utf16::TabelaLinhas::construir(&fonte);
                            let (l0, c0) = tabela.posicao_de_offset(&fonte, s.start);
                            let (l1, c1) = tabela.posicao_de_offset(&fonte, s.end);
                            (l0, c0, l1, c1)
                        };
                        json!({"start": {"line": l0, "character": c0}, "end": {"line": l1, "character": c1}})
                    } else {
                        json!({"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}})
                    };
                    Some(json!({
                        "uri": destino,
                        "range": range,
                    }))
                });
                resposta(&id, resultado.unwrap_or(Value::Null))
            }
            "textDocument/references" => {
                let params = mensagem.get("params");
                let uri = params
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|d| d.get("uri"))
                    .and_then(Value::as_str);
                let posicao = params.and_then(|p| p.get("position")).and_then(ler_posicao);
                let incluir_declaracao = params
                    .and_then(|p| p.get("context"))
                    .and_then(|c| c.get("includeDeclaration"))
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                let resultado = uri.zip(posicao).and_then(|(u, p)| {
                    let texto = self.documentos.get(u)?.to_string();
                    let tabela = self.documentos.linhas(u)?;
                    let offset = tabela.offset_de_posicao(&texto, p.linha, p.coluna);
                    let achados = self.analisador.referencias_em(&self.documentos, u, offset)?;
                    // A declaração vai no fim (`_getDeclarations`, §11.5).
                    let declaracao = achados.declaracao.filter(|_| incluir_declaracao);
                    let locais: Vec<Value> = achados
                        .usos
                        .iter()
                        .chain(declaracao.iter())
                        .filter_map(|(alvo, s)| {
                            Some(json!({"uri": alvo, "range": self.faixa(alvo, *s)?}))
                        })
                        .collect();
                    Some(json!(locais))
                });
                resposta(&id, resultado.unwrap_or(Value::Null))
            }
            "textDocument/hover" => {
                let params = mensagem.get("params");
                let uri = params
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|d| d.get("uri"))
                    .and_then(Value::as_str);
                let posicao = params.and_then(|p| p.get("position")).and_then(ler_posicao);
                let resultado = uri.zip(posicao).and_then(|(u, p)| {
                    let texto = self.documentos.get(u)?.to_string();
                    let tabela = self.documentos.linhas(u)?;
                    let offset = tabela.offset_de_posicao(&texto, p.linha, p.coluna);
                    let hover = self.analisador.hover_no_workspace(u, &texto, offset, &self.documentos)?;
                    let (l0, c0) = tabela.posicao_de_offset(&texto, hover.intervalo.start);
                    let (l1, c1) = tabela.posicao_de_offset(&texto, hover.intervalo.end);
                    // O formato do servidor do Dart: a descrição em bloco de
                    // código, o tipo e, separada por `---`, a documentação.
                    let conteudo = if self.hover_markdown {
                        // Como o `toHover` do Dart: descrição, tipo, biblioteca
                        // e, depois de `---`, a documentação.
                        let mut valor = format!("```dart\n{}\n```\n", hover.descricao);
                        if let Some(t) = &hover.tipo {
                            valor.push_str(&format!("Type: `{t}`\n\n"));
                        }
                        if let Some(b) = &hover.biblioteca {
                            valor.push_str(&format!("*{b}*\n\n"));
                        }
                        if let Some(d) = &hover.documentacao {
                            valor.push_str(&format!("---\n{d}\n"));
                        }
                        json!({"kind": "markdown", "value": valor.trim_end()})
                    } else {
                        let mut valor = hover.descricao.clone();
                        if let Some(t) = &hover.tipo {
                            valor.push_str(&format!("\nType: {t}"));
                        }
                        if let Some(d) = &hover.documentacao {
                            valor.push_str(&format!("\n\n{d}"));
                        }
                        json!(valor)
                    };
                    Some(json!({
                        "contents": conteudo,
                        "range": {
                            "start": {"line": l0, "character": c0},
                            "end": {"line": l1, "character": c1},
                        },
                    }))
                });
                resposta(&id, resultado.unwrap_or(Value::Null))
            }
            "textDocument/completion" => {
                let maximo = self.configuracao.max_completion_items.map_or(crate::completar::MAXIMO_PADRAO, |m| m as usize);
                self.analisador.definir_maximo_de_completar(maximo);
                let resultado = self.posicao_da_requisicao(mensagem).and_then(|(u, offset)| {
                    let completar = self.analisador.completar(&self.documentos, &u, offset)?;
                    let texto = self.documentos.get(&u)?.to_string();
                    let tabela = self.documentos.linhas(&u)?;
                    // `computeReplacementRange`: a palavra inteira; `insert`
                    // até o cursor.
                    let fim_da_palavra = texto[completar.fim.min(texto.len())..]
                        .char_indices()
                        .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_' || *c == '$'))
                        .map_or(texto.len(), |(i, _)| completar.fim + i);
                    let substituir = intervalo_lsp(&texto, tabela, completar.inicio, fim_da_palavra);
                    let inserir = intervalo_lsp(&texto, tabela, completar.inicio, completar.fim);
                    let iguais = fim_da_palavra == completar.fim;
                    let cap = &self.capacidades_de_completar;
                    // `itemDefaults` (`HC:250-291`).
                    let mut padroes = serde_json::Map::new();
                    if cap.padrao_modo && cap.modos_de_insercao.contains(&1) {
                        padroes.insert("insertTextMode".into(), json!(1));
                    }
                    if cap.padrao_intervalo {
                        padroes.insert(
                            "editRange".into(),
                            if !cap.inserir_substituir || iguais { substituir.clone() } else { json!({"insert": inserir, "replace": substituir}) },
                        );
                    }
                    // `_hasExistingArgList`: já há parênteses depois do nome.
                    let com_parenteses = texto[fim_da_palavra..].trim_start_matches([' ', '\t']).starts_with('(');
                    let arquivo = url::Url::parse(&u).ok().and_then(|x| x.to_file_path().ok()).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
                    let pedido = crate::item_completar::Pedido {
                        substituir: substituir.clone(),
                        inserir: inserir.clone(),
                        iguais,
                        padrao: cap.padrao_intervalo,
                        chamadas: self.configuracao.complete_function_calls && !com_parenteses,
                        arquivo: &arquivo,
                        preferencia_de_doc: &self.configuracao.documentacao,
                    };
                    // A documentação de cada item (do arquivo da declaração).
                    let mut textos: HashMap<std::path::PathBuf, Option<String>> = HashMap::new();
                    self.nao_importados.clear();
                    let mut itens: Vec<Value> = Vec::new();
                    for item in &completar.itens {
                        let doc = match (&item.origem, &item.importar) {
                            (Some((caminho, inicio)), None) => {
                                let fonte = textos.entry(caminho.clone()).or_insert_with(|| {
                                    url::Url::from_file_path(caminho)
                                        .ok()
                                        .and_then(|x| self.documentos.get(x.as_str()).map(str::to_string))
                                        .or_else(|| std::fs::read_to_string(caminho).ok())
                                });
                                fonte.as_deref().filter(|f| *inicio <= f.len() && f.is_char_boundary(*inicio)).and_then(|f| crate::dartdoc::documentacao(f, *inicio))
                            }
                            _ => None,
                        };
                        let v = crate::item_completar::item(cap, &pedido, item, doc);
                        if let Some(r) = v.pointer("/data/ref").and_then(Value::as_str) {
                            self.nao_importados.insert(r.to_string(), (item.origem.clone(), item.importar.clone(), item.detalhe.clone()));
                        }
                        itens.push(v);
                    }
                    let mut incompleta = completar.incompleta;
                    // Os snippets (sem ranqueamento, no fim).
                    if cap.snippet && self.configuracao.enable_snippets {
                        let prefixo = &texto[completar.inicio..completar.fim];
                        if let Some(contexto) = crate::item_completar::contexto_de_snippet(&texto, offset) {
                            let recuo: String = {
                                let ini = texto[..offset].rfind('\n').map_or(0, |i| i + 1);
                                texto[ini..].chars().take_while(|c| *c == ' ' || *c == '\t').collect()
                            };
                            let eol = if texto.contains("\r\n") { "\r\n" } else { "\n" };
                            let em_testes = std::path::Path::new(&arquivo).components().any(|c| c.as_os_str() == "test");
                            let finais = std::path::Path::new(&arquivo).parent().is_some_and(|_| {
                                let caminho = std::path::Path::new(&arquivo);
                                let raiz = crate::projeto::raiz_do_projeto(caminho);
                                let opcoes = dartforge_paridade::filtros::Opcoes::de_subpasta(caminho, &raiz).unwrap_or_else(|| raiz.join("analysis_options.yaml"));
                                dartforge_paridade::filtros::Opcoes::ler_arquivo(&opcoes).regras.get("prefer_final_locals").copied().unwrap_or(false)
                            });
                            let mut casador = crate::casador::Casador::novo(prefixo, crate::casador::Estilo::Texto);
                            for (pre, rotulo, doc, corpo) in crate::item_completar::snippets(contexto, &recuo, eol, em_testes, finais) {
                                if casador.score(pre) <= 0.0 {
                                    continue;
                                }
                                let v = crate::item_completar::item_de_snippet(cap, &pedido, pre, rotulo, doc, &corpo);
                                let filtro = v.get("filterText").or_else(|| v.get("label")).and_then(Value::as_str).unwrap_or("").to_string();
                                if casador.score(&filtro) <= 0.0 {
                                    continue;
                                }
                                itens.push(v);
                            }
                        } else {
                            incompleta |= false;
                        }
                    }
                    let mut lista = json!({"isIncomplete": incompleta, "items": itens});
                    if !padroes.is_empty() {
                        lista["itemDefaults"] = Value::Object(padroes);
                    }
                    Some(lista)
                });
                resposta(&id, resultado.unwrap_or(Value::Null))
            }
            "completionItem/resolve" => {
                let mut item = mensagem.get("params").cloned().unwrap_or(Value::Null);
                // `resolveDartCompletion`: só o item com `data` (não
                // importado): o import, a documentação e o `detail`.
                let chave = item.pointer("/data/ref").and_then(Value::as_str).map(str::to_string);
                if let Some(chave) = chave
                    && let Some((origem, importar, detalhe)) = self.nao_importados.get(&chave).cloned()
                {
                    let arquivo = item.pointer("/data/file").and_then(Value::as_str).unwrap_or("").to_string();
                    let uri_doc = url::Url::from_file_path(&arquivo).map(|x| x.to_string()).unwrap_or_default();
                    if let Some(imp) = &importar
                        && let (Some(texto), Some(tabela)) = (self.documentos.get(&uri_doc), self.documentos.linhas(&uri_doc))
                    {
                        item["additionalTextEdits"] = json!([{"range": intervalo_lsp(texto, tabela, imp.span.start, imp.span.end), "newText": imp.texto}]);
                        let exibida = crate::item_completar::uri_de_exibicao(&imp.uri, &arquivo);
                        let d = format!("Auto import from '{exibida}'\n\n{}", detalhe.unwrap_or_default());
                        item["detail"] = json!(d.trim());
                    }
                    if let Some((caminho, inicio)) = origem {
                        let fonte = url::Url::from_file_path(&caminho)
                            .ok()
                            .and_then(|x| self.documentos.get(x.as_str()).map(str::to_string))
                            .or_else(|| std::fs::read_to_string(&caminho).ok());
                        if let Some(doc) = fonte.as_deref().filter(|f| inicio <= f.len() && f.is_char_boundary(inicio)).and_then(|f| crate::dartdoc::documentacao(f, inicio)) {
                            item["documentation"] = if self.documentacao_markdown { json!({"kind": "markdown", "value": doc}) } else { json!(doc) };
                        }
                    }
                }
                resposta(&id, item)
            }
            "textDocument/codeAction" => {
                let params = mensagem.get("params");
                let uri = params.and_then(|p| p.pointer("/textDocument/uri")).and_then(Value::as_str).map(str::to_string);
                let intervalo = params.and_then(|p| p.get("range")).and_then(ler_intervalo);
                let (Some(u), Some((de, ate))) = (uri, intervalo) else {
                    return resposta(&id, Value::Null);
                };
                let (Some(texto), Some(tabela)) = (self.documentos.get(&u), self.documentos.linhas(&u)) else {
                    return resposta(&id, Value::Null);
                };
                let inicio = tabela.offset_de_posicao(texto, de.linha, de.coluna);
                let fim = tabela.offset_de_posicao(texto, ate.linha, ate.coluna);
                let (inicio, fim) = (inicio.min(fim), inicio.max(fim));
                let apenas: Option<Vec<String>> = params
                    .and_then(|p| p.pointer("/context/only"))
                    .and_then(Value::as_array)
                    .map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect());
                // Só os tipados publicados para a versão vigente.
                let vazio = Vec::new();
                let publicados = match self.tipados_publicados.get(&u) {
                    Some((v, d)) if Some(*v) == self.documentos.version(&u) => d,
                    _ => &vazio,
                };
                let acoes = self.analisador.acoes(&self.documentos, &u, inicio, fim, publicados);
                let mut saida = Vec::new();
                // As ações de fonte (§13.12.1): antes de todas as outras, na
                // forma de comando, sempre oferecidas (o cálculo fica para o
                // `executeCommand`), e só a quem aceita `workspace/applyEdit`.
                let fontes_pedidas = apenas.as_ref().is_none_or(|l| l.iter().any(|w| w == "source" || w.starts_with("source.")));
                let caminho = url::Url::parse(&u).ok().filter(|x| x.scheme() == "file").and_then(|x| x.to_file_path().ok());
                if let (true, true, true, Some(caminho)) = (fontes_pedidas, self.aplicar_edicoes, u.ends_with(".dart"), caminho) {
                    let mut argumento = json!({"path": caminho.to_string_lossy()});
                    if params.and_then(|p| p.pointer("/context/triggerKind")).and_then(Value::as_u64) == Some(2) {
                        argumento["autoTriggered"] = json!(true);
                    }
                    for (titulo, especie, comando) in ACOES_DE_FONTE {
                        let casa = |w: &String| *especie == w.as_str() || especie.starts_with(&format!("{w}."));
                        let incluida = match (&apenas, &self.especies_de_acao) {
                            (Some(l), _) => l.iter().any(casa),
                            (None, Some(l)) if self.acoes_literais => l.iter().any(casa),
                            _ => true,
                        };
                        if !incluida {
                            continue;
                        }
                        let c = json!({"title": titulo, "command": comando, "arguments": [argumento.clone()]});
                        saida.push(if self.acoes_literais { json!({"title": titulo, "kind": especie, "command": c}) } else { c });
                    }
                }
                let (mut correcoes, mut assistencias, mut refatoracoes): (Vec<Value>, Vec<Value>, Vec<Value>) =
                    (Vec::new(), Vec::new(), Vec::new());
                for acao in acoes {
                    // As de fonte do analisador (a edição pronta do
                    // `Organize Imports`) dão lugar aos comandos acima.
                    if acao.especie.starts_with("source") {
                        continue;
                    }
                    let permitida = apenas.as_ref().is_none_or(|l| {
                        l.iter().any(|k| acao.especie == *k || acao.especie.starts_with(&format!("{k}.")))
                    });
                    if !permitida {
                        continue;
                    }
                    let edicao = match &acao.criar_arquivo {
                        // Só a quem aceita criar arquivo: a capacidade é a
                        // edição inteira ou nada.
                        Some(_) if !self.criar_arquivos => continue,
                        Some((novo, conteudo)) => {
                            let mut e = self.edicao_de_workspace(&acao.edicoes, Some(json!({"kind": "create", "uri": novo, "options": {"ignoreIfExists": true}})));
                            if !conteudo.is_empty()
                                && let Some(l) = e["documentChanges"].as_array_mut()
                            {
                                l.push(json!({
                                    "textDocument": {"uri": novo, "version": null},
                                    "edits": [{"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}, "newText": conteudo}],
                                }));
                            }
                            e
                        }
                        None => self.edicao_de_workspace(&acao.edicoes, None),
                    };
                    // A forma do servidor do Dart (§13.7.1 item 8 e §13.8.1):
                    // `diagnostics` (o do erro numa correção, vazia numa
                    // assistência), o comando de registro com o id original
                    // da mudança, e nunca `isPreferred`. As duas refatorações
                    // legadas (`Inline`/`Extract Local Variable`) não são
                    // assistências e ficam como estavam.
                    let refatoracao = matches!(acao.especie.as_str(), "refactor.inline" | "refactor.extract");
                    let mut valor = json!({"title": acao.titulo, "kind": acao.especie, "edit": edicao});
                    if let (Some(d), Some(texto), Some(tabela)) =
                        (&acao.diagnostico, self.documentos.get(&u), self.documentos.linhas(&u))
                    {
                        valor["diagnostics"] = json!([converter_diagnostico(texto, tabela, d)]);
                    } else if !refatoracao {
                        valor["diagnostics"] = json!([]);
                    }
                    if !refatoracao && let Some(acao_id) = id_da_acao(&acao.especie) {
                        valor["command"] = json!({"title": "Log Action", "command": "dart.logAction", "arguments": [{"action": acao_id}]});
                    }
                    if refatoracao {
                        refatoracoes.push(valor);
                    } else if acao.especie.starts_with("quickfix") {
                        correcoes.push(valor);
                    } else {
                        assistencias.push(valor);
                    }
                }
                // As ações de fonte, as correções, as assistências e as
                // refatorações, nesta ordem; correções e assistências passam
                // cada grupo pelo `_CodeActionSorter`.
                let coluna_do_pedido = i64::from(de.coluna);
                saida.extend(ordenar_acoes(correcoes, coluna_do_pedido));
                saida.extend(ordenar_acoes(assistencias, coluna_do_pedido));
                saida.extend(refatoracoes);
                saida.extend(self.refatoracoes_por_comando(&u, inicio, fim, apenas.as_deref()));
                resposta(&id, json!(saida))
            }
            "workspace/executeCommand" => self.executar_comando(&id, mensagem),
            "textDocument/prepareRename" => {
                let Some((u, offset)) = self.posicao_da_requisicao(mensagem) else {
                    return resposta(&id, Value::Null);
                };
                match self.analisador.preparar_renomeacao(&self.documentos, &u, offset) {
                    Ok(Some((span, texto))) => {
                        let range = self.faixa(&u, span);
                        resposta(&id, range.map_or(Value::Null, |r| json!({"range": r, "placeholder": texto})))
                    }
                    Ok(None) => resposta(&id, Value::Null),
                    Err(motivo) => erro(&id, RENOMEAR_INVALIDO, motivo),
                }
            }
            "textDocument/rename" => {
                let novo = mensagem.pointer("/params/newName").and_then(Value::as_str).unwrap_or("").to_string();
                let Some((u, offset)) = self.posicao_da_requisicao(mensagem) else {
                    return resposta(&id, Value::Null);
                };
                // A versão no começo da requisição (`extractDocumentVersion`).
                let versao = self.documentos.version(&u);
                match self.analisador.renomear(&self.documentos, &u, offset, &novo) {
                    Ok(renomeacao) if renomeacao.nulo => resposta(&id, Value::Null),
                    Ok(renomeacao) => {
                        let pendente = RenomeacaoPendente {
                            id: id.clone(),
                            uri: u,
                            versao,
                            edicoes: renomeacao.edicoes,
                            arquivo: renomeacao.arquivo,
                            pergunta_do_arquivo: false,
                        };
                        match renomeacao.aviso {
                            // ERROR/WARNING do `checkFinalConditions`: sem
                            // `userPromptSender`, `-32010`; com, a pergunta.
                            Some(aviso) if !self.perguntas_ao_usuario => erro(&id, RENOMEAR_INVALIDO, aviso),
                            Some(aviso) => self.perguntar_no_rename(
                                pendente,
                                json!({"type": 2, "message": aviso, "actions": [{"title": "Rename Anyway"}, {"title": "Cancel"}]}),
                            ),
                            None => self.concluir_renomeacao(pendente),
                        }
                    }
                    Err(motivo) => erro(&id, RENOMEAR_INVALIDO, motivo),
                }
            }
            "textDocument/signatureHelp" => {
                let Some((u, offset)) = self.posicao_da_requisicao(mensagem) else {
                    return resposta(&id, Value::Null);
                };
                // Disparo automático (`(` digitado, ajuda fechada): só se o
                // `(` abre a lista, como o servidor do Dart.
                let automatica = mensagem.pointer("/params/context/triggerKind").and_then(Value::as_u64) == Some(2)
                    && mensagem.pointer("/params/context/isRetrigger").and_then(Value::as_bool) == Some(false);
                let Some(a) = self.analisador.assinatura(&self.documentos, &u, offset, automatica) else {
                    return resposta(&id, Value::Null);
                };
                let mut assinatura = json!({
                    "label": a.rotulo,
                    "parameters": a.parametros.iter().map(|p| json!({"label": p})).collect::<Vec<_>>(),
                });
                if let Some(d) = &a.documentacao {
                    assinatura["documentation"] =
                        if self.assinatura_markdown { json!({"kind": "markdown", "value": d}) } else { json!(d) };
                }
                // Sem parâmetro ativo e sem `null` no cliente: um índice fora
                // da lista (o VS Code não destaca nenhum).
                let ativo = match a.ativo {
                    Some(i) => json!(i),
                    None if self.assinatura_sem_ativo => Value::Null,
                    None => json!(a.parametros.len()),
                };
                resposta(&id, json!({"signatures": [assinatura], "activeSignature": 0, "activeParameter": ativo}))
            }
            "textDocument/documentHighlight" => {
                let Some((u, offset)) = self.posicao_da_requisicao(mensagem) else {
                    return resposta(&id, Value::Null);
                };
                let destaques = self.analisador.destaques(&self.documentos, &u, offset).unwrap_or_default();
                let itens: Vec<Value> = destaques.into_iter().filter_map(|s| Some(json!({"range": self.faixa(&u, s)?}))).collect();
                // O Dart responde `[]`, nunca `null` (§11.6).
                resposta(&id, json!(itens))
            }
            "textDocument/implementation" => {
                let Some((u, offset)) = self.posicao_da_requisicao(mensagem) else {
                    return resposta(&id, json!([]));
                };
                let locais: Vec<Value> = self
                    .analisador
                    .implementacoes(&self.documentos, &u, offset)
                    .into_iter()
                    .filter_map(|(alvo, s)| Some(json!({"uri": alvo, "range": self.faixa(&alvo, s)?})))
                    .collect();
                resposta(&id, json!(locais))
            }
            "textDocument/typeDefinition" => {
                let Some((u, offset)) = self.posicao_da_requisicao(mensagem) else {
                    return resposta(&id, json!([]));
                };
                let local = self
                    .analisador
                    .definicao_de_tipo(&self.documentos, &u, offset)
                    .and_then(|(alvo, s)| Some(json!({"uri": alvo, "range": self.faixa(&alvo, s)?})));
                resposta(&id, local.unwrap_or_else(|| json!([])))
            }
            // `handler_document_link.dart:26-53` (docs/LSP-ESPECIFICACAO.md
            // §8.4): no 3.6.2 o visitante só liga os comentários "See code
            // in examples/api/…" do Flutter; as URIs das diretivas não viram
            // link (o oráculo devolve `[]` nos 36 arquivos, todos com
            // imports). Fora do Flutter a resposta é sempre a lista vazia.
            "textDocument/documentLink" => {
                // Os blocos `{@tool}` que citam o código de exemplo
                // (`crate::links`, o visitor do 3.6.2).
                let uri = mensagem.pointer("/params/textDocument/uri").and_then(Value::as_str).unwrap_or_default().to_string();
                let caminho = url::Url::parse(&uri).ok().and_then(|u| u.to_file_path().ok());
                let (Some(texto), Some(caminho)) = (self.documentos.get(&uri).map(str::to_string), caminho) else {
                    return resposta(&id, json!([]));
                };
                if caminho.extension().is_none_or(|e| e != "dart") {
                    return resposta(&id, json!([]));
                }
                let links: Vec<Value> = crate::links::links(&texto, &caminho)
                    .into_iter()
                    .filter_map(|(s, alvo)| {
                        let destino = url::Url::from_file_path(&alvo).ok()?;
                        Some(json!({"range": self.faixa(&uri, s)?, "target": destino.as_str()}))
                    })
                    .collect();
                resposta(&id, json!(links))
            }
            // `handler_code_lens.dart:32-61` (§8.3): só há lentes de
            // augmentations, e só para o cliente que declara o comando
            // `dart.goToLocation`; sem augmentations, lista vazia.
            "textDocument/codeLens" => resposta(&id, json!([])),
            "textDocument/foldingRange" => {
                let uri = mensagem.pointer("/params/textDocument/uri").and_then(Value::as_str).map(str::to_string);
                let Some((u, texto)) = uri.and_then(|u| Some((u.clone(), self.documentos.get(&u)?.to_string()))) else {
                    return resposta(&id, Value::Null);
                };
                let so_linhas = self.dobras_so_linhas;
                let dobras: Vec<Value> = self
                    .analisador
                    .dobras(&u, &texto, so_linhas)
                    .into_iter()
                    .map(|d| {
                        let mut v = json!({"startLine": d.linha_inicio, "endLine": d.linha_fim});
                        if !so_linhas {
                            v["startCharacter"] = json!(d.coluna_inicio);
                            v["endCharacter"] = json!(d.coluna_fim);
                        }
                        if let Some(k) = d.especie {
                            v["kind"] = json!(k);
                        }
                        v
                    })
                    .collect();
                resposta(&id, json!(dobras))
            }
            "textDocument/selectionRange" => {
                let uri = mensagem.pointer("/params/textDocument/uri").and_then(Value::as_str).map(str::to_string);
                let Some((u, texto)) = uri.and_then(|u| Some((u.clone(), self.documentos.get(&u)?.to_string()))) else {
                    return resposta(&id, Value::Null);
                };
                let posicoes: Vec<Posicao> = mensagem
                    .pointer("/params/positions")
                    .and_then(Value::as_array)
                    .map(|l| l.iter().filter_map(ler_posicao).collect())
                    .unwrap_or_default();
                let mut saida = Vec::new();
                for p in posicoes {
                    let Some(offset) = self.documentos.linhas(&u).map(|t| t.offset_de_posicao(&texto, p.linha, p.coluna)) else {
                        break;
                    };
                    let spans = self.analisador.selecoes(&u, &texto, offset);
                    let Some(tabela) = self.documentos.linhas(&u) else { break };
                    // Do mais externo ao mais interno, cada um pai do seguinte.
                    let mut atual: Option<Value> = None;
                    for s in spans.iter().rev() {
                        let mut v = json!({"range": intervalo_lsp(&texto, tabela, s.start, s.end)});
                        if let Some(pai) = atual.take() {
                            v["parent"] = pai;
                        }
                        atual = Some(v);
                    }
                    saida.push(atual.unwrap_or_else(|| json!({"range": intervalo_lsp(&texto, tabela, offset, offset)})));
                }
                resposta(&id, json!(saida))
            }
            "textDocument/semanticTokens/full" | "textDocument/semanticTokens/range" => {
                let uri = mensagem.pointer("/params/textDocument/uri").and_then(Value::as_str).map(str::to_string);
                let Some(u) = uri.filter(|u| self.documentos.get(u).is_some()) else {
                    return resposta(&id, Value::Null);
                };
                let faixa = mensagem.pointer("/params/range").and_then(ler_intervalo).and_then(|(de, ate)| {
                    let texto = self.documentos.get(&u)?;
                    let tabela = self.documentos.linhas(&u)?;
                    Some((tabela.offset_de_posicao(texto, de.linha, de.coluna), tabela.offset_de_posicao(texto, ate.linha, ate.coluna)))
                });
                let multilinha = self.tokens_multilinha;
                match self.analisador.tokens_semanticos(&self.documentos, &u, multilinha, faixa) {
                    Some(dados) => resposta(&id, json!({"data": dados})),
                    None => resposta(&id, Value::Null),
                }
            }
            "textDocument/inlayHint" => {
                let uri = mensagem.pointer("/params/textDocument/uri").and_then(Value::as_str).map(str::to_string);
                let Some(u) = uri.filter(|u| self.documentos.get(u).is_some()) else {
                    return resposta(&id, json!([]));
                };
                let dicas = self.analisador.dicas(&self.documentos, &u);
                let (Some(texto), Some(tabela)) = (self.documentos.get(&u), self.documentos.linhas(&u)) else {
                    return resposta(&id, json!([]));
                };
                let itens: Vec<Value> = dicas
                    .into_iter()
                    .map(|d| {
                        let (l, c) = tabela.posicao_de_offset(texto, d.offset);
                        let mut v = json!({
                            "position": {"line": l, "character": c},
                            "label": [{"value": d.rotulo}],
                            "kind": d.especie,
                        });
                        if d.espaco_depois {
                            v["paddingRight"] = json!(true);
                        }
                        v
                    })
                    .collect();
                resposta(&id, json!(itens))
            }
            "textDocument/prepareCallHierarchy" => {
                let Some((u, offset)) = self.posicao_da_requisicao(mensagem) else {
                    return resposta(&id, Value::Null);
                };
                let item = self.analisador.preparar_chamadas(&self.documentos, &u, offset);
                let valor = item.and_then(|i| self.item_de_chamada(&i));
                resposta(&id, valor.map_or(Value::Null, |v| json!([v])))
            }
            "callHierarchy/incomingCalls" | "callHierarchy/outgoingCalls" => {
                // `toServerItem`: o arquivo, o nome em `selectionRange` e o
                // nome exibido do item do cliente.
                let item = mensagem.pointer("/params/item");
                let uri = item.and_then(|i| i.get("uri")).and_then(Value::as_str).map(str::to_string);
                let nome = item.and_then(|i| i.get("name")).and_then(Value::as_str).unwrap_or("").to_string();
                let construtor = item.and_then(|i| i.get("kind")).and_then(Value::as_u64) == Some(9);
                let inicio = item.and_then(|i| i.pointer("/selectionRange/start")).cloned();
                let Some(uri) = uri else { return resposta(&id, json!([])) };
                if !uri.ends_with(".dart") {
                    return resposta(&id, json!([]));
                }
                let offset = inicio.and_then(|p| {
                    let linha = p.get("line")?.as_u64()? as u32;
                    let coluna = p.get("character")?.as_u64()? as u32;
                    self.offset_no_arquivo(&uri, linha, coluna)
                });
                let Some(offset) = offset else {
                    return erro(&id, CONTEUDO_MODIFICADO, "Content was modified since Call Hierarchy node was produced");
                };
                let recebidas = metodo == "callHierarchy/incomingCalls";
                let chamadas = self.analisador.chamadas(&self.documentos, &uri, offset, &nome, construtor, recebidas);
                let mut saida = Vec::new();
                for (item, spans) in chamadas {
                    // Recebidas: intervalos no arquivo de quem chama; feitas:
                    // no arquivo do item pedido.
                    let arquivo = if recebidas { item.uri.clone() } else { uri.clone() };
                    let faixas: Vec<Value> = spans.iter().filter_map(|s| self.faixa(&arquivo, *s)).collect();
                    let Some(v) = self.item_de_chamada(&item) else { continue };
                    saida.push(if recebidas { json!({"from": v, "fromRanges": faixas}) } else { json!({"to": v, "fromRanges": faixas}) });
                }
                resposta(&id, json!(saida))
            }
            "textDocument/prepareTypeHierarchy" => {
                // `isDartDocument`: outro arquivo responde lista vazia.
                let pedido = mensagem.pointer("/params/textDocument/uri").and_then(Value::as_str).unwrap_or("");
                if !pedido.ends_with(".dart") {
                    return resposta(&id, json!([]));
                }
                let Some((u, offset)) = self.posicao_da_requisicao(mensagem) else {
                    return resposta(&id, Value::Null);
                };
                let item = self.analisador.preparar_hierarquia(&self.documentos, &u, offset);
                if item.is_some() {
                    self.origem_da_hierarquia = Some(u.clone());
                }
                // `toLspItem(target, unit.lineInfo)`: as posições do arquivo
                // da classe convertidas com as linhas do documento atual.
                let valor = item.and_then(|i| self.item_de_hierarquia(&i, Some(&u)));
                resposta(&id, valor.map_or(Value::Null, |v| json!([v])))
            }
            "typeHierarchy/supertypes" | "typeHierarchy/subtypes" => {
                let item = mensagem.pointer("/params/item");
                let uri = item.and_then(|i| i.get("uri")).and_then(Value::as_str).unwrap_or("").to_string();
                let dados = item.and_then(|i| i.get("data")).filter(|d| !d.is_null());
                let Some(referencia) = dados.and_then(|d| d.get("ref")).and_then(Value::as_str).map(str::to_string) else {
                    return erro(&id, -32602, "TypeHierarchyItem is missing the data field");
                };
                let ancora: Option<(String, Vec<usize>)> = dados.and_then(|d| d.get("anchor")).filter(|a| !a.is_null()).and_then(|a| {
                    let r = a.get("ref")?.as_str()?.to_string();
                    let caminho = a.get("path")?.as_array()?.iter().filter_map(Value::as_u64).map(|x| x as usize).collect();
                    Some((r, caminho))
                });
                let projeto = self.documento_da_hierarquia(&uri);
                let supertipos = metodo == "typeHierarchy/supertypes";
                let itens = self.analisador.hierarquia(&self.documentos, &projeto, &referencia, ancora.as_ref().map(|(r, c)| (r.as_str(), c.as_slice())), supertipos);
                match itens {
                    None => resposta(&id, Value::Null),
                    // `_convertItems`: cada item com as linhas do próprio
                    // arquivo; o que não se lê fica de fora.
                    Some(lista) => resposta(&id, json!(lista.iter().filter_map(|i| self.item_de_hierarquia(i, None)).collect::<Vec<_>>())),
                }
            }
            METODO_DORMIR => {
                let ms = mensagem
                    .get("params")
                    .and_then(|p| p.get("ms"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                if dormir_cancelavel(ms, &self.cancelados, &chave) {
                    self.cancelados.remove(&chave);
                    erro(&id, REQUEST_CANCELLED, "requisição cancelada")
                } else {
                    resposta(&id, Value::Null)
                }
            }
            _ => {
                registrar(format!("requisição desconhecida: {metodo}"));
                erro(
                    &id,
                    METHOD_NOT_FOUND,
                    format!("Unknown method {metodo}"),
                )
            }
        }
    }

    /// Um `CallHierarchyItem` (`toLspItem`): a espécie pelo
    /// `toSymbolKindMapping` com o recuo (`File` → `Module`; outra não
    /// anunciada ou desconhecida → `Object`), sem `data`.
    fn item_de_chamada(&self, item: &crate::ItemDeChamada) -> Option<Value> {
        let aceitas: Vec<u64> = self.tipos_de_simbolo.clone().unwrap_or_else(|| (1..=18).collect());
        let especie = match item.especie.simbolo() {
            Some(k) if aceitas.contains(&u64::from(k)) => u64::from(k),
            Some(1) => 2,
            _ => 19,
        };
        let mut v = json!({
            "name": item.nome,
            "kind": especie,
            "uri": item.uri,
            "range": self.faixa(&item.uri, item.intervalo)?,
            "selectionRange": self.faixa(&item.uri, item.selecao)?,
        });
        if let Some(d) = &item.detalhe {
            v["detail"] = json!(d);
        }
        Some(v)
    }

    /// O offset (bytes) de uma posição LSP de `uri` (aberto ou no disco).
    fn offset_no_arquivo(&self, uri: &str, linha: u32, coluna: u32) -> Option<usize> {
        if let (Some(texto), Some(tabela)) = (self.documentos.get(uri), self.documentos.linhas(uri)) {
            return Some(tabela.offset_de_posicao(texto, linha, coluna));
        }
        let caminho = url::Url::parse(uri).ok()?.to_file_path().ok()?;
        let fonte = std::fs::read_to_string(caminho).ok()?;
        let tabela = crate::utf16::TabelaLinhas::construir(&fonte);
        Some(tabela.offset_de_posicao(&fonte, linha, coluna))
    }

    /// Um `TypeHierarchyItem` (`toLspItem`): espécie 5 (classe), `data` com
    /// o `ElementLocation` (`ref`) e a âncora. Com `linhas_de`, os
    /// intervalos são convertidos com as linhas desse documento.
    fn item_de_hierarquia(&self, item: &crate::ItemDeTipo, linhas_de: Option<&str>) -> Option<Value> {
        let (intervalo, selecao) = match linhas_de {
            Some(origem) => (self.faixa_com_linhas_de(&item.uri, origem, item.intervalo)?, self.faixa_com_linhas_de(&item.uri, origem, item.selecao)?),
            None => (self.faixa(&item.uri, item.intervalo)?, self.faixa(&item.uri, item.selecao)?),
        };
        let mut dados = json!({"ref": item.referencia});
        if let Some((r, caminho)) = &item.ancora {
            dados["anchor"] = json!({"ref": r, "path": caminho});
        }
        Some(json!({
            "name": item.nome,
            "kind": 5,
            "uri": item.uri,
            "range": intervalo,
            "selectionRange": selecao,
            "data": dados,
        }))
    }

    /// O documento cujo projeto atende `typeHierarchy/supertypes` e
    /// `subtypes` do item em `uri`: o próprio arquivo quando está numa raiz
    /// do workspace; senão (SDK, pacote) o do último `prepareTypeHierarchy`.
    fn documento_da_hierarquia(&self, uri: &str) -> String {
        let no_workspace = url::Url::parse(uri)
            .ok()
            .and_then(|u| u.to_file_path().ok())
            .is_some_and(|c| self.raizes.iter().any(|r| c.starts_with(r)));
        match &self.origem_da_hierarquia {
            Some(origem) if !no_workspace => origem.clone(),
            _ => uri.to_string(),
        }
    }

    /// O texto de `uri`: o aberto, senão o do disco.
    fn texto_do_arquivo(&self, uri: &str) -> Option<std::borrow::Cow<'_, str>> {
        if let Some(texto) = self.documentos.get(uri) {
            return Some(std::borrow::Cow::Borrowed(texto));
        }
        let caminho = url::Url::parse(uri).ok()?.to_file_path().ok()?;
        std::fs::read_to_string(caminho).ok().map(std::borrow::Cow::Owned)
    }

    /// Um intervalo de `arquivo` convertido com o `LineInfo` de `origem`
    /// (`sourceRangeToRange(unit.lineInfo, …)` do `prepareTypeHierarchy`):
    /// os offsets UTF-16 do arquivo da classe, a linha pelo último início de
    /// linha de `origem` que não passa deles e a coluna sem limite.
    fn faixa_com_linhas_de(&self, arquivo: &str, origem: &str, span: dartforge_diagnostics::Span) -> Option<Value> {
        if arquivo == origem {
            return self.faixa(arquivo, span);
        }
        let texto = self.texto_do_arquivo(arquivo)?;
        let alvo = self.texto_do_arquivo(origem)?;
        let inicios = inicios_utf16(&alvo);
        let posicao = |byte: usize| {
            let mut b = byte.min(texto.len());
            while !texto.is_char_boundary(b) {
                b -= 1;
            }
            let o = texto[..b].encode_utf16().count();
            let linha = inicios.partition_point(|&s| s <= o).saturating_sub(1);
            json!({"line": linha, "character": o - inicios[linha]})
        };
        Some(json!({"start": posicao(span.start), "end": posicao(span.end)}))
    }

    /// `workspace/executeCommand` (docs/LSP-ESPECIFICACAO.md §13.12.2 a
    /// §13.12.6). Devolve a resposta, ou, quando o comando tem edição a
    /// aplicar, o pedido `workspace/applyEdit`: a resposta do
    /// `executeCommand` sai quando o cliente responder
    /// ([`Servidor::resposta_do_cliente`]).
    /// As refatorações de `codeAction` (§13.11.1): depois dos assists, na
    /// ordem do Dart, na forma de comando; só com `workspace/applyEdit` e
    /// com `refactor` pedido. As legadas passam pelo `shouldIncludeKind`; a
    /// do `RefactoringProcessor` (Move) não.
    fn refatoracoes_por_comando(&mut self, u: &str, inicio: usize, fim: usize, apenas: Option<&[String]>) -> Vec<Value> {
        let pedidas = apenas.is_none_or(|l| l.iter().any(|w| w == "refactor" || w.starts_with("refactor.")));
        if !pedidas || !self.aplicar_edicoes || !u.ends_with(".dart") {
            return Vec::new();
        }
        let Some(caminho) = url::Url::parse(u).ok().filter(|x| x.scheme() == "file").and_then(|x| x.to_file_path().ok()) else {
            return Vec::new();
        };
        let caminho = caminho.to_string_lossy().into_owned();
        let lista = self.analisador.refatoracoes(&self.documentos, u, inicio, fim - inicio, self.criar_arquivos);
        let Some(texto) = self.documentos.get(u) else { return Vec::new() };
        let o16 = texto[..inicio.min(texto.len())].encode_utf16().count();
        let l16 = texto[inicio.min(texto.len())..fim.min(texto.len())].encode_utf16().count();
        let versao = self.documentos.version(u);
        let mut saida = Vec::new();
        for r in lista {
            match r.comando {
                crate::ComandoDeRefatoracao::Mover { caminho_padrao } => {
                    let padrao = url::Url::from_file_path(&caminho_padrao).map(|x| x.to_string()).unwrap_or_default();
                    let c = json!({
                        "title": r.titulo,
                        "command": "dart.refactor.move_top_level_to_file",
                        "arguments": [{"filePath": caminho, "selectionOffset": o16, "selectionLength": l16, "arguments": [padrao]}],
                    });
                    saida.push(json!({
                        "title": r.titulo,
                        "kind": r.especie,
                        "command": c,
                        "data": {"parameters": [{
                            "actionLabel": "Move",
                            "defaultValue": padrao,
                            "filters": {"Dart": ["dart"]},
                            "kind": "saveUri",
                            "parameterLabel": "Move to:",
                            "parameterTitle": "Select a file to move to",
                        }]},
                    }));
                }
                crate::ComandoDeRefatoracao::Legado(kind) => {
                    let casa = |w: &String| r.especie == w.as_str() || r.especie.starts_with(&format!("{w}."));
                    let incluida = match (apenas, &self.especies_de_acao) {
                        (Some(l), _) => l.iter().any(casa),
                        (None, Some(l)) if self.acoes_literais => l.iter().any(casa),
                        _ => true,
                    };
                    if !incluida {
                        continue;
                    }
                    let c = json!({
                        "title": r.titulo,
                        "command": "refactor.perform",
                        "arguments": [kind, caminho, versao, o16, l16, null],
                    });
                    saida.push(if self.acoes_literais { json!({"title": r.titulo, "kind": r.especie, "command": c}) } else { c });
                }
            }
        }
        saida
    }

    /// O texto vigente do arquivo (o aberto, senão o do disco).
    fn texto_do_arquivo(&self, uri: &str, caminho: &str) -> Option<String> {
        self.documentos.get(uri).map(str::to_string).or_else(|| std::fs::read_to_string(caminho).ok())
    }

    /// `refactor.perform`/`refactor.validate` (§13.11.2).
    fn executar_refatoracao_legada(&mut self, id: &Value, mensagem: &Value, so_validar: bool) -> Value {
        let nome = if so_validar { "Validate Refactor" } else { "Perform Refactor" };
        let invalidos = || {
            erro(
                id,
                ARGUMENTOS_DE_COMANDO_INVALIDOS,
                format!(
                    "{nome} requires 6 parameters: kind: String (RefactoringKind), filePath: String, docVersion: int?, offset: int, length: int, options: Map<String, Object?>"
                ),
            )
        };
        // `parseArgList`: posicional, 6 itens; outra forma vira `{}`.
        let argumentos = mensagem.pointer("/params/arguments").and_then(Value::as_array).cloned().unwrap_or_default();
        if argumentos.len() != 6 {
            return invalidos();
        }
        let (Some(kind), Some(caminho)) = (argumentos[0].as_str(), argumentos[1].as_str()) else { return invalidos() };
        let versao = match &argumentos[2] {
            Value::Null => None,
            v => match v.as_i64() {
                Some(n) => Some(n),
                None => return invalidos(),
            },
        };
        let (Some(offset), Some(comprimento)) = (argumentos[3].as_i64(), argumentos[4].as_i64()) else { return invalidos() };
        let opcoes = match &argumentos[5] {
            Value::Null => None,
            Value::Object(m) => Some(m.clone()),
            _ => return invalidos(),
        };
        // `requireResolvedUnit(path)`.
        let uri = url::Url::from_file_path(caminho).ok().map(|u| u.to_string());
        let texto = uri.as_deref().and_then(|u| self.texto_do_arquivo(u, caminho));
        let (Some(uri), Some(texto)) = (uri, texto) else {
            return erro(id, ARQUIVO_NAO_ANALISADO, "File is not being analyzed");
        };
        if !caminho.ends_with(".dart") {
            return erro(id, ARQUIVO_NAO_ANALISADO, "File is not being analyzed");
        }
        if !TIPOS_DE_REFATORACAO.contains(&kind) {
            return erro(id, ERRO_NAO_TRATADO, format!("Exception: Illegal enum value: {kind}"));
        }
        if matches!(kind, "MOVE_FILE" | "RENAME") {
            return erro(id, ARGUMENTOS_DE_COMANDO_INVALIDOS, format!("Unknown RefactoringKind RefactoringKind.{kind} was supplied to {nome}"));
        }
        if offset < 0 || comprimento < 0 {
            return erro(id, ERRO_NAO_TRATADO, format!("RangeError: offset {offset}, length {comprimento}"));
        }
        let ini = byte_de_utf16(&texto, offset as usize);
        let fim = byte_de_utf16(&texto, (offset + comprimento) as usize);
        let pedido = crate::PedidoDeRefatoracao { kind: kind.to_string(), offset: ini, comprimento: fim - ini, opcoes, so_validar };
        let resultado = self.analisador.executar_refatoracao(&self.documentos, &uri, &pedido);
        self.resposta_de_refatoracao(id, nome, resultado, if so_validar { None } else { Some((uri, versao)) })
    }

    /// A resposta de uma refatoração calculada. `versionada`: o arquivo e a
    /// versão pedida, para o `ContentModified` do `refactor.perform`.
    fn resposta_de_refatoracao(
        &mut self,
        id: &Value,
        rotulo: &'static str,
        resultado: crate::ResultadoDeRefatoracao,
        versionada: Option<(String, Option<i64>)>,
    ) -> Value {
        use crate::ResultadoDeRefatoracao as R;
        let validar = rotulo == "Validate Refactor";
        match resultado {
            R::NaoAnalisado => erro(id, ARQUIVO_NAO_ANALISADO, "File is not being analyzed"),
            R::ArgumentosInvalidos(m) => erro(id, ARGUMENTOS_DE_COMANDO_INVALIDOS, m),
            R::ErroInterno(m) => erro(id, ERRO_NAO_TRATADO, m),
            R::Falha(motivo) => erro(id, FALHA_DE_CALCULO, motivo.unwrap_or_else(|| "Cannot compute the change. No details.".to_string())),
            R::Erro(m) if validar => resposta(id, json!({"valid": false, "message": m})),
            R::Erro(m) => {
                // `showErrorMessageToUser` e sucesso `null`.
                self.saidas_pendentes.push(json!({
                    "jsonrpc": "2.0",
                    "method": "window/showMessage",
                    "params": {"type": 1, "message": m},
                }));
                resposta(id, Value::Null)
            }
            R::Valido => resposta(id, if validar { json!({"valid": true}) } else { Value::Null }),
            R::Mudanca { .. } if validar => resposta(id, json!({"valid": true})),
            R::Mudanca { edicoes, criar } => {
                if edicoes.is_empty() && criar.is_none() {
                    return resposta(id, Value::Null);
                }
                if let Some((uri, Some(v))) = &versionada
                    && Some(*v) != self.documentos.version(uri).map(i64::from)
                {
                    return erro(id, CONTEUDO_MODIFICADO, "Document was modified before operation completed");
                }
                let edicao = match criar {
                    Some((novo, conteudo)) => {
                        let mut e = self.edicao_de_workspace(&edicoes, Some(json!({"kind": "create", "uri": novo, "options": {"ignoreIfExists": true}})));
                        if !conteudo.is_empty()
                            && let Some(l) = e["documentChanges"].as_array_mut()
                        {
                            l.push(json!({
                                "textDocument": {"uri": novo, "version": null},
                                "edits": [{"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}, "newText": conteudo}],
                            }));
                        }
                        e
                    }
                    None => self.edicao_de_workspace(&edicoes, None),
                };
                self.pedir_aplicacao(id, rotulo, edicao)
            }
        }
    }

    /// Os comandos do `RefactoringProcessor` (§13.11.2, "Comandos do
    /// framework novo").
    fn executar_refatoracao_nova(&mut self, id: &Value, mensagem: &Value, comando: &'static str) -> Value {
        let invalidos = || {
            erro(
                id,
                ARGUMENTOS_DE_COMANDO_INVALIDOS,
                "Refactoring operations require 4 parameters: filePath: String, offset: int, length: int, arguments: List",
            )
        };
        let argumentos = mensagem.pointer("/params/arguments").and_then(Value::as_array).cloned().unwrap_or_default();
        let parametros = match argumentos.as_slice() {
            [] => json!({}),
            [unico] if unico.is_object() => unico.clone(),
            _ => return erro(id, ARGUMENTOS_DE_COMANDO_INVALIDOS, format!("{comando} requires a single Map argument")),
        };
        let (Some(caminho), Some(offset), Some(comprimento), Some(lista)) = (
            parametros.get("filePath").and_then(Value::as_str),
            parametros.get("selectionOffset").and_then(Value::as_i64),
            parametros.get("selectionLength").and_then(Value::as_i64),
            parametros.get("arguments").and_then(Value::as_array),
        ) else {
            return invalidos();
        };
        let uri = url::Url::from_file_path(caminho).ok().map(|u| u.to_string());
        let texto = uri.as_deref().and_then(|u| self.texto_do_arquivo(u, caminho));
        let (Some(uri), Some(texto)) = (uri, texto) else {
            return erro(id, ARQUIVO_NAO_ANALISADO, "File is not being analyzed");
        };
        if !caminho.ends_with(".dart") {
            return erro(id, ARQUIVO_NAO_ANALISADO, "File is not being analyzed");
        }
        if offset < 0 || comprimento < 0 {
            return erro(id, ERRO_NAO_TRATADO, format!("RangeError: offset {offset}, length {comprimento}"));
        }
        let ini = byte_de_utf16(&texto, offset as usize);
        let fim = byte_de_utf16(&texto, (offset + comprimento) as usize);
        let resultado = if comando == "dart.refactor.move_top_level_to_file" {
            // `arguments[0] as String`: outro tipo lança.
            match lista.first().and_then(Value::as_str) {
                Some(destino) => self.analisador.mover_para_arquivo(&self.documentos, &uri, ini, fim - ini, destino),
                None => crate::ResultadoDeRefatoracao::ErroInterno("type 'Null' is not a subtype of type 'String' in type cast".to_string()),
            }
        } else {
            // As experimentais (`analyzeAvailability`): sem a mudança.
            crate::ResultadoDeRefatoracao::Falha(None)
        };
        self.resposta_de_refatoracao(id, comando, resultado, None)
    }

    fn executar_comando(&mut self, id: &Value, mensagem: &Value) -> Value {
        let comando = mensagem.pointer("/params/command").and_then(Value::as_str).unwrap_or("").to_string();
        match comando.as_str() {
            "refactor.perform" => return self.executar_refatoracao_legada(id, mensagem, false),
            "refactor.validate" => return self.executar_refatoracao_legada(id, mensagem, true),
            _ => {}
        }
        if let Some(&c) = COMANDOS.iter().find(|c| c.starts_with("dart.refactor.") && **c == comando) {
            return self.executar_refatoracao_nova(id, mensagem, c);
        }
        let rotulo: &'static str = match comando.as_str() {
            "dart.edit.sortMembers" => "Sort Members",
            "dart.edit.organizeImports" => "Organize Imports",
            "dart.edit.fixAll" => "Fix All",
            "dart.edit.fixAllInWorkspace.preview" => "Preview All Fixes in Workspace",
            "dart.edit.fixAllInWorkspace" => "Apply All Fixes in Workspace",
            "dart.edit.sendWorkspaceEdit" => "Send Workspace Edit",
            "dart.logAction" => "Log Action",
            _ => return erro(id, COMANDO_DESCONHECIDO, format!("{comando} is not a valid command identifier")),
        };
        let argumentos = mensagem.pointer("/params/arguments").and_then(Value::as_array).cloned().unwrap_or_default();
        let parametros = match argumentos.as_slice() {
            [] => json!({}),
            [unico] if unico.is_object() => unico.clone(),
            _ => return erro(id, ARGUMENTOS_DE_COMANDO_INVALIDOS, format!("{comando} requires a single Map argument")),
        };
        match comando.as_str() {
            // Só telemetria no servidor do Dart; sem `action`, a conversão
            // dele lança (`log_action.dart:23`).
            "dart.logAction" => match parametros.get("action").and_then(Value::as_str) {
                Some(_) => resposta(id, Value::Null),
                None => erro(id, ERRO_NAO_TRATADO, "dart.logAction sem o campo action"),
            },
            "dart.edit.fixAllInWorkspace" | "dart.edit.fixAllInWorkspace.preview" => {
                self.corrigir_workspace(id, rotulo, comando.ends_with(".preview"))
            }
            "dart.edit.sendWorkspaceEdit" => match parametros.get("edit").filter(|e| e.is_object()).cloned() {
                Some(edicao) => self.pedir_aplicacao(id, rotulo, edicao),
                None => erro(
                    id,
                    ARGUMENTOS_DE_COMANDO_INVALIDOS,
                    "Send Workspace Edit requires a Map argument containing \"edit\" (WorkspaceEdit)",
                ),
            },
            _ => {
                let Some(caminho) = parametros.get("path").and_then(Value::as_str).map(str::to_string) else {
                    return erro(id, ARGUMENTOS_DE_COMANDO_INVALIDOS, format!("{rotulo} requires a Map argument containing a \"path\""));
                };
                let automatico = parametros.get("autoTriggered").and_then(Value::as_bool).unwrap_or(false);
                let uri = url::Url::from_file_path(&caminho).ok().map(|u| u.to_string());
                let texto = uri
                    .as_deref()
                    .and_then(|u| self.documentos.get(u).map(str::to_string))
                    .or_else(|| std::fs::read_to_string(&caminho).ok());
                let (Some(uri), Some(texto)) = (uri, texto) else {
                    return if automatico && comando != "dart.edit.fixAll" {
                        resposta(id, Value::Null)
                    } else {
                        erro(id, ARQUIVO_NAO_ANALISADO, format!("{rotulo} is only available for analyzed files"))
                    };
                };
                if comando == "dart.edit.fixAll" {
                    return self.corrigir_tudo(id, rotulo, &uri, &texto, automatico);
                }
                let mut nomes = dartforge_intern::Interner::new();
                let analisado = dartforge_frontend::parser::parse(&texto, &mut nomes);
                // `hasScanParseErrors`: algum erro do scanner ou do parser.
                let com_erros = analisado.diagnostics.iter().any(|d| {
                    d.code.is_none_or(|c| {
                        let unico = c.info().unico;
                        unico.starts_with("ParserErrorCode.") || unico.starts_with("ScannerErrorCode.")
                    })
                });
                if com_erros {
                    return if automatico {
                        resposta(id, Value::Null)
                    } else {
                        erro_com_dado(
                            id,
                            ARQUIVO_COM_ERROS,
                            format!("Unable to {rotulo} because the file contains parse errors"),
                            json!(caminho),
                        )
                    };
                }
                let edicoes: Vec<crate::Edicao> = if comando == "dart.edit.sortMembers" {
                    crate::fonte_ordenar::ordenar_membros(&texto, &analisado.unit, &analisado.ast, &nomes)
                        .map(|(offset, comprimento, novo)| {
                            vec![crate::Edicao {
                                uri: uri.clone(),
                                span: dartforge_diagnostics::Span { start: offset, end: offset + comprimento },
                                texto: novo,
                            }]
                        })
                        .unwrap_or_default()
                } else {
                    // Os erros semânticos são os da última publicação tipada,
                    // se ela é da versão vigente; sem ela, nenhum import sai
                    // por erro (só as duplicatas textuais).
                    let tipados: &[dartforge_diagnostics::Diagnostic] = self
                        .tipados_publicados
                        .get(&uri)
                        .filter(|(versao, _)| Some(*versao) == self.documentos.version(&uri))
                        .map(|(_, d)| d.as_slice())
                        .unwrap_or_default();
                    let nome_de =
                        |d: &dartforge_diagnostics::Diagnostic| d.code.map_or("", |c| c.info().unico.rsplit('.').next().unwrap_or(""));
                    let removiveis: Vec<usize> = tipados
                        .iter()
                        .filter(|d| matches!(nome_de(d), "DUPLICATE_IMPORT" | "UNUSED_IMPORT" | "UNNECESSARY_IMPORT"))
                        .map(|d| d.span.start)
                        .collect();
                    // Os nove códigos com `isUnresolvedIdentifier`.
                    let nao_resolvido = tipados.iter().any(|d| {
                        matches!(
                            nome_de(d),
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
                    crate::fonte_imports::organizar(&texto, &analisado.unit, &removiveis, nao_resolvido, true)
                        .and_then(|novo| crate::fonte_imports::edicao(&texto, &novo))
                        .map(|(offset, comprimento, novo)| {
                            vec![crate::Edicao {
                                uri: uri.clone(),
                                span: dartforge_diagnostics::Span { start: offset, end: offset + comprimento },
                                texto: novo,
                            }]
                        })
                        .unwrap_or_default()
                };
                // Nada a mudar: `null`, sem falar com o cliente.
                if edicoes.is_empty() {
                    return resposta(id, Value::Null);
                }
                let edicao = self.edicao_de_workspace(&edicoes, None);
                self.pedir_aplicacao(id, rotulo, edicao)
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
    fn corrigir_tudo(&mut self, id: &Value, rotulo: &'static str, uri: &str, texto: &str, automatico: bool) -> Value {
        use crate::fonte_corrigir::{Construtor, Ed};
        if !uri.ends_with(".dart") || crate::fonte_corrigir::gerado(uri) {
            return resposta(id, Value::Null);
        }
        let versao = self.documentos.version(uri);
        let publicados: Vec<dartforge_diagnostics::Diagnostic> = self
            .tipados_publicados
            .get(uri)
            .filter(|(v, _)| Some(*v) == versao)
            .map(|(_, d)| d.clone())
            .unwrap_or_default();
        let mut passadas: Vec<Vec<Ed>> = Vec::new();
        let mut atual = texto.to_string();
        let mut sobreposto: Option<DocumentStore> = None;
        for passada in 0..4 {
            let documentos = sobreposto.as_ref().unwrap_or(&self.documentos);
            let vistos: &[dartforge_diagnostics::Diagnostic] = if passada == 0 { &publicados } else { &[] };
            let acoes = self.analisador.acoes(documentos, uri, 0, atual.len(), vistos);
            // Fase A: por diagnóstico, em ordem de offset.
            let mut candidatas: Vec<&crate::AcaoDeCodigo> = acoes
                .iter()
                .filter(|a| {
                    a.diagnostico.is_some()
                        && a.criar_arquivo.is_none()
                        && crate::fonte_corrigir::em_lote(&a.especie, automatico)
                        && a.edicoes.iter().all(|e| e.uri == uri && e.span.start <= e.span.end && e.span.end <= atual.len())
                })
                .collect();
            candidatas.sort_by_key(|a| a.diagnostico.as_ref().map_or(0, |d| d.span.start));
            let mut construtor = Construtor::default();
            for a in candidatas {
                construtor.aplicar_produtor(a.edicoes.iter().map(|e| (e.span.start, e.span.end - e.span.start, e.texto.clone())));
            }
            // Fase B: a limpeza de imports, só no pedido manual e numa
            // passada sem edição da fase A. Os erros de import são os
            // tipados, que só valem para o texto original.
            if construtor.edicoes.is_empty() && !automatico && passada == 0 {
                let mut nomes = dartforge_intern::Interner::new();
                let analisado = dartforge_frontend::parser::parse(&atual, &mut nomes);
                for remocao in crate::fonte_corrigir::remover_imports(&atual, &analisado.unit, &publicados) {
                    construtor.aplicar_produtor([remocao]);
                }
            }
            if construtor.edicoes.is_empty() {
                break;
            }
            atual = construtor.aplicar(&atual);
            passadas.push(construtor.edicoes);
            // O texto corrigido como documento aberto, para a passada
            // seguinte; os outros documentos abertos seguem como estão.
            let mut copia = DocumentStore::new();
            for outro in self.documentos.uris() {
                if outro != uri
                    && let (Some(v), Some(t)) = (self.documentos.version(outro), self.documentos.get(outro))
                {
                    copia.open(outro.to_string(), v, t.to_string());
                }
            }
            copia.open(uri.to_string(), versao.unwrap_or(0), atual.clone());
            sobreposto = Some(copia);
            // O que o analisador retém era do texto anterior.
            self.analisador.documento_alterado(uri);
        }
        if sobreposto.is_some() {
            // O analisador volta a ver os documentos de verdade.
            self.analisador.documento_alterado(uri);
        }
        let edicoes: Vec<Ed> = match passadas.len() {
            0 => return resposta(id, Value::Null),
            1 => passadas.pop().unwrap_or_default(),
            _ => crate::fonte_corrigir::fundir(passadas.concat()),
        };
        // Em LSP, ordem crescente de offset; todas relativas ao original.
        let edicoes: Vec<crate::Edicao> = edicoes
            .into_iter()
            .rev()
            .map(|(offset, comprimento, novo)| crate::Edicao {
                uri: uri.to_string(),
                span: dartforge_diagnostics::Span { start: offset, end: offset + comprimento },
                texto: novo,
            })
            .collect();
        let edicao = self.edicao_de_workspace(&edicoes, None);
        self.pedir_aplicacao(id, rotulo, edicao)
    }

    /// `Fix All in Workspace` e a prévia (docs/LSP-ESPECIFICACAO.md
    /// §13.12.8): uma única passada, manual, por todos os arquivos `.dart`
    /// não gerados das raízes do workspace e dos documentos abertos, com as
    /// fases A e B do `Fix All`; cada edição leva a anotação com o modelo
    /// da mensagem da correção. Um documento aberto usa os diagnósticos
    /// tipados publicados para a versão vigente; um arquivo só do disco, os
    /// imediatos do analisador.
    fn corrigir_workspace(&mut self, id: &Value, rotulo: &'static str, confirmar: bool) -> Value {
        use crate::fonte_corrigir::{Construtor, Ed};
        if !self.aplicar_edicoes {
            return erro(id, RECURSO_DESLIGADO, format!("\"{rotulo}\" is only available for clients that support workspace/applyEdit"));
        }
        if !self.anotacoes_de_mudanca {
            return erro(id, RECURSO_DESLIGADO, format!("\"{rotulo}\" is only available for clients that support change annotations"));
        }
        // Os alvos: os abertos e, do disco, os que não estão abertos.
        let mut alvos: Vec<(String, bool)> = self.documentos.uris().map(|u| (u.to_string(), true)).collect();
        alvos.sort();
        let abertos: HashSet<std::path::PathBuf> =
            alvos.iter().filter_map(|(u, _)| url::Url::parse(u).ok().and_then(|x| x.to_file_path().ok())).collect();
        let mut vistos: HashSet<std::path::PathBuf> = HashSet::new();
        for raiz in &self.raizes {
            for arquivo in dartforge_paridade::corpus::arquivos_dart(raiz) {
                if abertos.contains(&arquivo) || !vistos.insert(arquivo.clone()) {
                    continue;
                }
                if let Ok(u) = url::Url::from_file_path(&arquivo) {
                    alvos.push((u.to_string(), false));
                }
            }
        }
        // Uma cópia dos abertos, em que cada arquivo do disco entra só
        // enquanto é examinado.
        let mut copia = DocumentStore::new();
        for u in self.documentos.uris() {
            if let (Some(v), Some(t)) = (self.documentos.version(u), self.documentos.get(u)) {
                copia.open(u.to_string(), v, t.to_string());
            }
        }
        let mut mudancas: Vec<(String, Vec<(Ed, &'static str)>)> = Vec::new();
        let mut mexeu_na_copia = false;
        for (u, aberto) in &alvos {
            if !u.ends_with(".dart") || crate::fonte_corrigir::gerado(u) {
                continue;
            }
            let (texto, tipados): (String, Vec<dartforge_diagnostics::Diagnostic>) = if *aberto {
                let Some(t) = self.documentos.get(u) else { continue };
                let versao = self.documentos.version(u);
                let d = self.tipados_publicados.get(u).filter(|(v, _)| Some(*v) == versao).map(|(_, d)| d.clone()).unwrap_or_default();
                (t.to_string(), d)
            } else {
                let Some(t) = url::Url::parse(u).ok().and_then(|x| x.to_file_path().ok()).and_then(|p| std::fs::read_to_string(p).ok()) else {
                    continue;
                };
                copia.open(u.clone(), 0, t.clone());
                self.analisador.documento_alterado(u);
                mexeu_na_copia = true;
                (t, Vec::new())
            };
            let acoes = self.analisador.acoes(&copia, u, 0, texto.len(), &tipados);
            let mut candidatas: Vec<&crate::AcaoDeCodigo> = acoes
                .iter()
                .filter(|a| {
                    a.diagnostico.is_some()
                        && a.criar_arquivo.is_none()
                        && crate::fonte_corrigir::em_lote(&a.especie, false)
                        && a.edicoes.iter().all(|e| e.uri == *u && e.span.start <= e.span.end && e.span.end <= texto.len())
                })
                .collect();
            candidatas.sort_by_key(|a| a.diagnostico.as_ref().map_or(0, |d| d.span.start));
            let mut construtor = Construtor::default();
            let mut descritas: Vec<(Ed, &'static str)> = Vec::new();
            for a in candidatas {
                let edicoes: Vec<Ed> = a.edicoes.iter().map(|e| (e.span.start, e.span.end - e.span.start, e.texto.clone())).collect();
                if construtor.aplicar_produtor(edicoes.clone()) {
                    let descricao = crate::fonte_corrigir::descricao(&a.especie);
                    descritas.extend(edicoes.into_iter().map(|e| (e, descricao)));
                }
            }
            if construtor.edicoes.is_empty() {
                let mut nomes = dartforge_intern::Interner::new();
                let analisado = dartforge_frontend::parser::parse(&texto, &mut nomes);
                for remocao in crate::fonte_corrigir::remover_imports(&texto, &analisado.unit, &tipados) {
                    if construtor.aplicar_produtor([remocao.clone()]) {
                        descritas.push((remocao, "Remove unused import"));
                    }
                }
            }
            if !*aberto {
                let _ = copia.close(u);
            }
            if !construtor.edicoes.is_empty() {
                let com_descricao = construtor
                    .edicoes
                    .into_iter()
                    .map(|e| {
                        let d = descritas.iter().find(|(x, _)| *x == e).map_or("", |(_, d)| *d);
                        (e, d)
                    })
                    .collect();
                mudancas.push((u.clone(), com_descricao));
            }
        }
        if mexeu_na_copia {
            // O analisador volta a ver os documentos de verdade.
            self.analisador.documento_alterado("");
        }
        if mudancas.is_empty() {
            return resposta(id, Value::Null);
        }
        let mut anotacoes = serde_json::Map::new();
        let mut por_arquivo: Vec<(String, Vec<Value>)> = Vec::new();
        for (u, edicoes) in &mudancas {
            let mut lista = Vec::new();
            // Em LSP, ordem crescente de offset.
            for ((offset, comprimento, novo), descricao) in edicoes.iter().rev() {
                let Some(range) = self.faixa(u, dartforge_diagnostics::Span { start: *offset, end: offset + comprimento }) else { continue };
                // Sem descrição, o último segmento da URI.
                let rotulo_da_edicao: &str = if descricao.is_empty() { u.rsplit('/').next().unwrap_or(u.as_str()) } else { *descricao };
                anotacoes
                    .entry(rotulo_da_edicao.to_string())
                    .or_insert_with(|| json!({"label": rotulo_da_edicao, "needsConfirmation": confirmar}));
                lista.push(json!({"range": range, "newText": novo, "annotationId": rotulo_da_edicao}));
            }
            por_arquivo.push((u.clone(), lista));
        }
        let mut edicao = if self.mudancas_versionadas {
            let documentos: Vec<Value> = por_arquivo
                .into_iter()
                .map(|(u, edits)| {
                    let versao = self.documentos.version(&u).map_or(Value::Null, |v| json!(v));
                    json!({"textDocument": {"uri": u, "version": versao}, "edits": edits})
                })
                .collect();
            json!({"documentChanges": documentos})
        } else {
            let mut mapa = serde_json::Map::new();
            for (u, edits) in por_arquivo {
                mapa.insert(u, json!(edits));
            }
            json!({"changes": mapa})
        };
        edicao["changeAnnotations"] = Value::Object(anotacoes);
        self.pedir_aplicacao(id, rotulo, edicao)
    }

    /// Um `window/showMessageRequest` do rename; o `rename` fica pendente
    /// até a resposta.
    fn perguntar_no_rename(&mut self, pendente: RenomeacaoPendente, params: Value) -> Value {
        self.proximo_pedido += 1;
        let pedido = json!(format!("dartforge/prompt/{}", self.proximo_pedido));
        self.renomeacoes_pendentes.insert(chave_id(&pedido), pendente);
        json!({"jsonrpc": "2.0", "id": pedido, "method": "window/showMessageRequest", "params": params})
    }

    /// `handler_rename.dart:205-269` depois das condições: a versão do
    /// documento (`-32801`), o arquivo da classe (`renameFilesWithClasses`:
    /// `always` renomeia, `prompt` pergunta `Yes`/`No`) e o `WorkspaceEdit`
    /// com o `RenameFile` no fim.
    fn concluir_renomeacao(&mut self, mut pendente: RenomeacaoPendente) -> Value {
        if !pendente.pergunta_do_arquivo
            && pendente.versao.is_some()
            && self.documentos.version(&pendente.uri) != pendente.versao
        {
            return erro(&pendente.id, CONTEUDO_MODIFICADO, "Document was modified before operation completed");
        }
        let mut recurso = None;
        if let Some(arquivo) = pendente.arquivo.take()
            && self.renomeacao_de_arquivo_possivel
        {
            let renomeia = if pendente.pergunta_do_arquivo || self.renomear_arquivos {
                true
            } else if self.configuracao.rename_files_with_classes == "prompt" && self.perguntas_ao_usuario {
                let base = |u: &str| u.rsplit('/').next().unwrap_or(u).to_string();
                let mensagem = format!("Rename '{}' to '{}'?", base(&arquivo.de), base(&arquivo.para));
                pendente.pergunta_do_arquivo = true;
                pendente.arquivo = Some(arquivo);
                return self.perguntar_no_rename(
                    pendente,
                    json!({"type": 3, "message": mensagem, "actions": [{"title": "Yes"}, {"title": "No"}]}),
                );
            } else {
                false
            };
            if renomeia {
                pendente.edicoes.extend(arquivo.diretivas);
                recurso = Some(json!({"kind": "rename", "oldUri": arquivo.de, "newUri": arquivo.para}));
            }
        }
        resposta(&pendente.id, self.edicao_de_workspace(&pendente.edicoes, recurso))
    }

    /// A resposta do usuário a uma pergunta do rename: `Rename Anyway`
    /// segue (qualquer outra, inclusive `null` ou erro, dá `{}`); `Yes`
    /// renomeia o arquivo (outra resposta, só as edições).
    fn resposta_do_rename(&mut self, mut pendente: RenomeacaoPendente, mensagem: &Value) -> Value {
        let escolha = mensagem.pointer("/result/title").and_then(Value::as_str);
        if pendente.pergunta_do_arquivo {
            if escolha != Some("Yes") {
                pendente.arquivo = None;
            }
            return self.concluir_renomeacao(pendente);
        }
        if escolha != Some("Rename Anyway") {
            return resposta(&pendente.id, json!({}));
        }
        self.concluir_renomeacao(pendente)
    }

    /// O pedido `workspace/applyEdit` de um comando (§13.12.3); o
    /// `executeCommand` de id `id` fica pendente até a resposta do cliente.
    fn pedir_aplicacao(&mut self, id: &Value, rotulo: &'static str, edicao: Value) -> Value {
        self.proximo_pedido += 1;
        let pedido = json!(format!("dartforge/applyEdit/{}", self.proximo_pedido));
        self.edicoes_pendentes.insert(chave_id(&pedido), (id.clone(), rotulo, edicao.clone()));
        json!({
            "jsonrpc": "2.0",
            "id": pedido,
            "method": "workspace/applyEdit",
            "params": {"label": rotulo, "edit": edicao},
        })
    }

    /// Uma resposta do cliente a um pedido do servidor. A de um
    /// `workspace/applyEdit` pendente vira a resposta do `executeCommand`
    /// que o enviou (tabela da §13.12.3); as outras não levam dado.
    fn resposta_do_cliente(&mut self, mensagem: &Value) -> Option<Value> {
        let chave = chave_id(mensagem.get("id")?);
        if self.pedido_de_configuracao.as_ref().is_some_and(|p| chave_id(&json!(p)) == chave) {
            self.configuracao_recebida(mensagem);
            return None;
        }
        if let Some(pendente) = self.renomeacoes_pendentes.remove(&chave) {
            return Some(self.resposta_do_rename(pendente, mensagem));
        }
        let (id, rotulo, edicao) = self.edicoes_pendentes.remove(&chave)?;
        if let Some(e) = mensagem.get("error").filter(|e| !e.is_null()) {
            return Some(erro_com_dado(
                &id,
                CLIENTE_NAO_APLICOU,
                format!("Client failed to apply workspace edit for {rotulo}"),
                json!(e.to_string()),
            ));
        }
        let Some(resultado) = mensagem.get("result").filter(|r| r.is_object()) else {
            return Some(erro(&id, ERRO_NAO_TRATADO, "resposta do workspace/applyEdit sem resultado"));
        };
        if resultado.get("applied").and_then(Value::as_bool) == Some(true) {
            return Some(resposta(&id, Value::Null));
        }
        Some(match resultado.get("failureReason").and_then(Value::as_str) {
            // Sem motivo: o usuário recusou uma prévia; não é erro.
            None => resposta(&id, Value::Null),
            Some(motivo) => erro_com_dado(
                &id,
                CLIENTE_NAO_APLICOU,
                format!("Client failed to apply workspace edit for {rotulo} (reason: {motivo})"),
                edicao,
            ),
        })
    }

    /// `WorkspaceEdit` das edições: `documentChanges` (cada documento com a
    /// versão vigente, `null` se fechado) quando o cliente aceita, senão
    /// `changes`. Uma operação de recurso (renomear arquivo) vai depois das
    /// edições de texto, que valem para o texto antes dela.
    fn edicao_de_workspace(&self, edicoes: &[crate::Edicao], recurso: Option<Value>) -> Value {
        let mut por_uri: Vec<(String, Vec<Value>)> = Vec::new();
        for e in edicoes {
            let Some(range) = self.faixa(&e.uri, e.span) else { continue };
            let item = json!({"range": range, "newText": e.texto});
            match por_uri.iter_mut().find(|(u, _)| *u == e.uri) {
                Some((_, l)) => l.push(item),
                None => por_uri.push((e.uri.clone(), vec![item])),
            }
        }
        if self.mudancas_versionadas || recurso.is_some() {
            let mut mudancas: Vec<Value> = por_uri
                .into_iter()
                .map(|(uri, edits)| {
                    let versao = self.documentos.version(&uri).map_or(Value::Null, |v| json!(v));
                    json!({"textDocument": {"uri": uri, "version": versao}, "edits": edits})
                })
                .collect();
            mudancas.extend(recurso);
            return json!({"documentChanges": mudancas});
        }
        let mut mudancas = serde_json::Map::new();
        for (uri, edits) in por_uri {
            mudancas.insert(uri, json!(edits));
        }
        json!({"changes": mudancas})
    }

    /// `workspace/symbol`: os símbolos dos documentos abertos e dos arquivos
    /// `.dart` dos projetos do workspace (raízes do `initialize` e o projeto,
    /// com `pubspec.yaml`, de cada documento aberto). O texto aberto vale
    /// mais que o disco; cada árvore é descartada antes da próxima. Casa por
    /// aproximação (`crate::aproximado`): os que contêm a consulta primeiro,
    /// depois as subsequências; entre iguais, a ordem de (URI, posição).
    fn simbolos_do_workspace(&mut self, consulta: &str) -> Vec<Value> {
        // `handler_workspace_symbols.dart:28-123` (docs/LSP-ESPECIFICACAO.md
        // §5): consulta vazia não devolve nada.
        if consulta.is_empty() {
            return Vec::new();
        }
        let mut raizes = self.raizes.clone();
        // O caminho de cada documento aberto (o da URI `file:`; senão o
        // caminho da URI, só para indexar o texto aberto).
        let caminho_aberto = |u: &str| -> Option<std::path::PathBuf> {
            let x = url::Url::parse(u).ok()?;
            Some(x.to_file_path().unwrap_or_else(|_| std::path::PathBuf::from(x.path())))
        };
        let mut textos_abertos: HashMap<std::path::PathBuf, (String, String)> = HashMap::new();
        let mut soltos: Vec<std::path::PathBuf> = Vec::new();
        for aberto in self.documentos.uris() {
            let Some(arquivo) = caminho_aberto(aberto) else { continue };
            let raiz = crate::projeto::raiz_do_projeto(&arquivo);
            if raiz.join("pubspec.yaml").is_file() && !raizes.contains(&raiz) {
                raizes.push(raiz);
            }
            if let Some(texto) = self.documentos.get(aberto) {
                textos_abertos.insert(dartforge_elements::gerado::chave(&arquivo), (aberto.to_string(), texto.to_string()));
            }
            if arquivo.extension().is_some_and(|x| x == "dart") {
                soltos.push(arquivo);
            }
        }
        // `addedFiles`: os arquivos das raízes analisadas; os conhecidos:
        // o SDK e os `lib/` dos pacotes de cada raiz.
        let mut adicionados: Vec<std::path::PathBuf> = raizes.iter().flat_map(|r| crate::projeto::arquivos_do_projeto(r)).collect();
        // Um documento aberto fora das raízes também é analisado (o
        // servidor cria um contexto para ele).
        soltos.retain(|c| !raizes.iter().any(|r| c.starts_with(r)));
        soltos.sort();
        adicionados.extend(soltos);
        let pacotes: Vec<std::path::PathBuf> = raizes.iter().flat_map(|r| crate::simbolos_workspace::pastas_dos_pacotes(r)).collect();
        let sdk = dartforge_elements::sdk::SdkLayout::discover();
        let especies = self.tipos_de_simbolo_do_workspace.clone();
        self.indice_de_simbolos.buscar(consulta, &adicionados, sdk.as_deref(), &pacotes, &textos_abertos, especies.as_deref())
    }

    /// URI e offset (bytes) de `params.textDocument` + `params.position`,
    /// quando o documento está aberto.
    fn posicao_da_requisicao(&self, mensagem: &Value) -> Option<(String, usize)> {
        let params = mensagem.get("params")?;
        let uri = params.get("textDocument")?.get("uri")?.as_str()?;
        let posicao = ler_posicao(params.get("position")?)?;
        let texto = self.documentos.get(uri)?;
        let offset = self.documentos.linhas(uri)?.offset_de_posicao(texto, posicao.linha, posicao.coluna);
        Some((uri.to_string(), offset))
    }

    /// Intervalo LSP de `span` (bytes) em `uri`: pelo texto aberto, senão
    /// pelo arquivo no disco (renomear toca arquivos fechados do projeto).
    fn faixa(&self, uri: &str, span: dartforge_diagnostics::Span) -> Option<Value> {
        if let (Some(texto), Some(tabela)) = (self.documentos.get(uri), self.documentos.linhas(uri)) {
            return Some(intervalo_lsp(texto, tabela, span.start, span.end));
        }
        let caminho = url::Url::parse(uri).ok()?.to_file_path().ok()?;
        let fonte = std::fs::read_to_string(caminho).ok()?;
        let tabela = crate::utf16::TabelaLinhas::construir(&fonte);
        Some(intervalo_lsp(&fonte, &tabela, span.start, span.end))
    }

    /// Diagnostica o documento e monta `textDocument/publishDiagnostics`.
    fn publicar(&mut self, uri: &str) -> Value {
        let Some(texto) = self.documentos.get(uri).map(str::to_string) else {
            return publicacao_vazia(uri);
        };
        let diagnosticos = self.analisador.diagnosticar(uri, &texto);
        self.publicacao(uri, &diagnosticos)
    }

    /// O relatório de `textDocument/diagnostic` de `uri`: o resultado tipado
    /// da versão vigente, senão o do analisador (o mesmo que o
    /// `publishDiagnostics` levaria). O `resultId` é a versão e se o
    /// resultado é o tipado; igual ao `previousResultId`, a resposta é
    /// `unchanged` sem recalcular. Documento que não está aberto não tem
    /// diagnóstico (como o `didClose` do modo empurrado).
    fn relatorio_puxado(&mut self, uri: &str, anterior: Option<&str>) -> Value {
        let Some(versao) = self.documentos.version(uri) else {
            return json!({"kind": "full", "items": []});
        };
        let tipado = self.tipados_publicados.get(uri).is_some_and(|(v, _)| *v == versao);
        let id_do_resultado = format!("{versao}{}", if tipado { ".t" } else { "" });
        if anterior == Some(id_do_resultado.as_str()) {
            return json!({"kind": "unchanged", "resultId": id_do_resultado});
        }
        let diagnosticos = if tipado {
            self.tipados_publicados[uri].1.clone()
        } else {
            let texto = self.documentos.get(uri).map(str::to_string).unwrap_or_default();
            self.analisador.diagnosticar(uri, &texto)
        };
        let (Some(texto), Some(tabela)) = (self.documentos.get(uri), self.documentos.linhas(uri)) else {
            return json!({"kind": "full", "items": []});
        };
        let itens: Vec<Value> = diagnosticos.iter().map(|d| converter_diagnostico(texto, tabela, d)).collect();
        json!({"kind": "full", "resultId": id_do_resultado, "items": itens})
    }

    /// `publishDiagnostics` de `diagnosticos` (spans do texto vigente de `uri`).
    fn publicacao(&self, uri: &str, diagnosticos: &[dartforge_diagnostics::Diagnostic]) -> Value {
        let (Some(texto), Some(tabela)) = (self.documentos.get(uri), self.documentos.linhas(uri)) else {
            return publicacao_vazia(uri);
        };
        let versao = self.documentos.version(uri).unwrap_or(0);
        let itens: Vec<Value> = diagnosticos
            .iter()
            .map(|d| converter_diagnostico(texto, tabela, d))
            .collect();
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {"uri": uri, "version": versao, "diagnostics": itens},
        })
    }
}

/// Raízes do workspace no `initialize`: `workspaceFolders`, senão
/// `rootUri`, senão `rootPath`; só diretórios existentes.
/// Os inícios de linha (`LineInfo.lineStarts`) de `texto` em unidades
/// UTF-16: depois de `\n`, de `\r\n` e de `\r` isolado.
fn inicios_utf16(texto: &str) -> Vec<usize> {
    let mut inicios = vec![0];
    let mut o = 0usize;
    let mut anterior_cr = false;
    for ch in texto.chars() {
        let largura = ch.len_utf16();
        if anterior_cr && ch != '\n' {
            inicios.push(o);
        }
        o += largura;
        anterior_cr = ch == '\r';
        if ch == '\n' {
            inicios.push(o);
        }
    }
    if anterior_cr {
        inicios.push(o);
    }
    inicios
}

fn raizes_do_initialize(params: Option<&Value>) -> Vec<std::path::PathBuf> {
    let Some(params) = params else { return Vec::new() };
    let de_uri = |v: &Value| {
        v.as_str()
            .and_then(|u| url::Url::parse(u).ok())
            .and_then(|u| u.to_file_path().ok())
    };
    let mut raizes: Vec<std::path::PathBuf> = params
        .get("workspaceFolders")
        .and_then(Value::as_array)
        .map(|l| l.iter().filter_map(|f| de_uri(&f["uri"])).collect())
        .unwrap_or_default();
    if raizes.is_empty() {
        raizes.extend(params.get("rootUri").and_then(de_uri));
    }
    if raizes.is_empty() {
        raizes.extend(params.get("rootPath").and_then(Value::as_str).map(std::path::PathBuf::from));
    }
    raizes.retain(|r| r.is_dir());
    raizes
}

/// Dorme até `ms` em fatias de 10 ms; verdadeiro quando cancelado no meio.
///
/// O ponto seguro de cancelamento em execução: entre fatias, sem estado
/// parcial, porque dormir não produz nada. A análise real checará a mesma
/// condição entre documentos, nunca no meio de uma frase do parser.
fn dormir_cancelavel(ms: u64, cancelados: &HashSet<String>, chave: &str) -> bool {
    let fatias = ms / 10 + u64::from(!ms.is_multiple_of(10));
    for _ in 0..fatias {
        std::thread::sleep(std::time::Duration::from_millis(10.min(ms)));
        if cancelados.contains(chave) {
            return true;
        }
    }
    false
}

/// Verdadeiro quando a mensagem é requisição (tem `id` e `method` textual).
fn eh_requisicao(mensagem: &Value) -> bool {
    mensagem.get("id").is_some() && mensagem.get("method").is_some_and(|m| m.is_string())
}

/// Chave canônica do id (número e texto com o mesmo dígito são ids distintos
/// no JSON-RPC; a serialização canônica preserva a distinção).
fn chave_id(id: &Value) -> String {
    id.to_string()
}

/// Resposta de sucesso JSON-RPC.
fn resposta(id: &Value, resultado: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": resultado})
}

/// Resposta de erro JSON-RPC.
fn erro(id: &Value, codigo: i32, mensagem: impl Into<String>) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": codigo, "message": mensagem.into()}})
}

/// `window/showMessage` ou `window/logMessage` do `tipo` (1 = erro).
fn notificacao_de_mensagem(metodo: &str, tipo: i32, texto: &str) -> Value {
    json!({"jsonrpc": "2.0", "method": metodo, "params": {"type": tipo, "message": texto}})
}

/// A mensagem de um pânico (o `&str` ou a `String` do `panic!`).
fn texto_do_panico(panico: &(dyn std::any::Any + Send)) -> String {
    panico
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| panico.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "pânico sem mensagem".to_string())
}

/// A prioridade de uma correção ou assistência pela espécie
/// (docs/LSP-ESPECIFICACAO.md §13.7.2 e §13.3): só as espécies que este
/// servidor emite têm valor próprio; as demais ficam no padrão do grupo (50
/// para correções, 40 para as `*.multi`, 30 para assistências).
fn prioridade_da_acao(especie: &str) -> i32 {
    // A tabela gerada das espécies do Dart (§13.7.2 e `DartAssistKind`).
    if let Some(id) = id_da_acao(especie)
        && let Some((p, _)) = crate::acoes::especie_do_dart(&id)
    {
        return i32::from(p);
    }
    match especie {
        outra if outra.starts_with("quickfix") && outra.ends_with(".multi") => 40,
        outra if outra.starts_with("quickfix") => 50,
        _ => 30,
    }
}

/// O id original da mudança (`dart.fix.…`, `dart.assist.…`) de uma espécie
/// LSP: o inverso de `toCodeActionKind`.
fn id_da_acao(especie: &str) -> Option<String> {
    if let Some(resto) = especie.strip_prefix("quickfix.") {
        return Some(format!("dart.fix.{resto}"));
    }
    especie.strip_prefix("refactor.").map(|resto| format!("dart.assist.{resto}"))
}

/// `_CodeActionSorter.sort` (`handler_code_actions.dart:284-406`;
/// docs/LSP-ESPECIFICACAO.md §13.7.1, item 9): agrupa por título na ordem
/// da primeira ocorrência; num grupo, fica a ação cujo primeiro diagnóstico
/// começa na coluna mais próxima da do pedido, com os diagnósticos das
/// outras de mesma edição fundidos (as de edição diferente saem); depois,
/// prioridade decrescente e ordem de chegada.
/// O offset em bytes do offset UTF-16 `u` (o fim do texto se passar dele).
fn byte_de_utf16(texto: &str, u: usize) -> usize {
    let mut contados = 0usize;
    for (i, c) in texto.char_indices() {
        if contados >= u {
            return i;
        }
        contados += c.len_utf16();
    }
    texto.len()
}

fn ordenar_acoes(acoes: Vec<Value>, coluna_do_pedido: i64) -> Vec<Value> {
    let mut grupos: Vec<(String, Vec<Value>)> = Vec::new();
    for a in acoes {
        let titulo = a["title"].as_str().unwrap_or("").to_string();
        match grupos.iter_mut().find(|(t, _)| *t == titulo) {
            Some((_, g)) => g.push(a),
            None => grupos.push((titulo, vec![a])),
        }
    }
    let distancia = |a: &Value| a.pointer("/diagnostics/0/range/start/character").and_then(Value::as_i64).map_or(0, |c| (c - coluna_do_pedido).abs());
    let mut unicas: Vec<Value> = Vec::new();
    for (_, mut grupo) in grupos {
        if grupo.len() > 1 {
            grupo.sort_by_key(distancia);
            let mut primeira = grupo.remove(0);
            let mut diagnosticos: Vec<Value> = primeira["diagnostics"].as_array().cloned().unwrap_or_default();
            for outra in grupo {
                if outra["edit"] == primeira["edit"] {
                    diagnosticos.extend(outra["diagnostics"].as_array().cloned().unwrap_or_default());
                }
            }
            primeira["diagnostics"] = Value::Array(diagnosticos);
            unicas.push(primeira);
        } else {
            unicas.extend(grupo);
        }
    }
    // Estável: em empate vale a ordem de chegada.
    unicas.sort_by_key(|a| -prioridade_da_acao(a["kind"].as_str().unwrap_or("")));
    unicas
}

/// Resposta de erro JSON-RPC com o campo `data`.
fn erro_com_dado(id: &Value, codigo: i32, mensagem: impl Into<String>, dado: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": codigo, "message": mensagem.into(), "data": dado}})
}

/// Publicação vazia: documento fechado ou desconhecido não tem diagnósticos.
fn publicacao_vazia(uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": {"uri": uri, "diagnostics": []},
    })
}

/// O LSP exige `SymbolInformation[]` para clientes que não anunciaram suporte
/// a `DocumentSymbol[]` hierárquico. A localização plana é a seleção do nome.
fn achatar_simbolos(simbolo: &Value, uri: &str, pai: Option<&str>, saida: &mut Vec<Value>) {
    let nome = simbolo.get("name").and_then(Value::as_str).unwrap_or("");
    let mut plano = json!({
        "name": nome,
        "kind": simbolo["kind"],
        "location": {"uri": uri, "range": simbolo["selectionRange"]},
    });
    if let Some(pai) = pai {
        plano["containerName"] = json!(pai);
    }
    saida.push(plano);
    if let Some(filhos) = simbolo.get("children").and_then(Value::as_array) {
        for filho in filhos {
            achatar_simbolos(filho, uri, Some(nome), saida);
        }
    }
}

/// Converte `contentChanges` do protocolo em mudanças internas.
///
/// Intervalo ausente ou malformado vira substituição integral (nunca `panic`
/// por mensagem malformada do cliente: o pior caso é reanalisar tudo).
fn ler_mudancas(valor: &Value) -> Vec<MudancaConteudo> {
    let Some(lista) = valor.as_array() else {
        return Vec::new();
    };
    lista
        .iter()
        .map(|m| {
            let intervalo = m.get("range").and_then(ler_intervalo);
            let texto = m
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            MudancaConteudo { intervalo, texto }
        })
        .collect()
}

/// Lê `{start: {line, character}, end: {...}}` em posições internas.
fn ler_intervalo(valor: &Value) -> Option<(Posicao, Posicao)> {
    let inicio = ler_posicao(valor.get("start")?)?;
    let fim = ler_posicao(valor.get("end")?)?;
    Some((inicio, fim))
}

/// Lê `{line, character}`; número ausente ou negativo vira 0 (satura).
fn ler_posicao(valor: &Value) -> Option<Posicao> {
    let linha = valor.get("line")?.as_u64()? as u32;
    let coluna = valor.get("character")?.as_u64()? as u32;
    Some(Posicao { linha, coluna })
}

/// Intervalo LSP (UTF-16) de `inicio..fim` em bytes, saturado no texto.
fn intervalo_lsp(texto: &str, tabela: &crate::utf16::TabelaLinhas, inicio: usize, fim: usize) -> Value {
    let inicio = inicio.min(texto.len());
    let fim = fim.clamp(inicio, texto.len());
    let (l0, c0) = tabela.posicao_de_offset(texto, inicio);
    let (l1, c1) = tabela.posicao_de_offset(texto, fim);
    json!({"start": {"line": l0, "character": c0}, "end": {"line": l1, "character": c1}})
}

/// Converte um diagnóstico (span em bytes UTF-8) em diagnóstico LSP.
///
/// Offsets saturados para o texto; fim antes do início colapsa no início.
fn converter_diagnostico(
    texto: &str,
    tabela: &crate::utf16::TabelaLinhas,
    diagnostico: &dartforge_diagnostics::Diagnostic,
) -> Value {
    let inicio = diagnostico.span.start.min(texto.len());
    let mut fim = diagnostico.span.end.min(texto.len());
    if fim < inicio {
        fim = inicio;
    }
    let (l0, c0) = tabela.posicao_de_offset(texto, inicio);
    let (l1, c1) = tabela.posicao_de_offset(texto, fim);
    json!({
        "range": {
            "start": {"line": l0, "character": c0},
            "end": {"line": l1, "character": c1},
        },
        "severity": severidade(diagnostico.severity),
        "source": FONTE,
        "message": mensagem(diagnostico),
        "code": diagnostico.code.map(|c| c.info().nome),
    })
}

/// Log do servidor: sempre `stderr`, nunca `stdout` (que é só protocolo).
///
/// Um `println!` esquecido corrompe a sessão; esta função existe para que o
/// caminho certo seja o mais curto.
fn registrar(mensagem: String) {
    eprintln!("[dartforge-lsp] {mensagem}");
}

/// Mapa de teste não usado fora de depuração: registra método de cada mensagem
/// quando `DARTFORGE_LSP_DEBUG` ou `RUST_LOG` indicam verbosidade.
pub(crate) fn nivel_detalhado() -> bool {
    std::env::var("DARTFORGE_LSP_DEBUG").is_ok()
        || std::env::var("RUST_LOG")
            .is_ok_and(|v| v.eq_ignore_ascii_case("debug") || v.eq_ignore_ascii_case("trace"))
}

impl<A: Analisador> Servidor<A> {
    /// Registra a mensagem recebida em `stderr` quando o nível é detalhado.
    pub fn registrar_recebida(&self, mensagem: &Value) {
        if nivel_detalhado() {
            let metodo = mensagem
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or("<resposta>");
            registrar(format!("recebida: {metodo}"));
        }
    }
}

/// Documenta os métodos aceitos, para `docs/LSP.md` e para o teste de aceite.
pub fn metodos_suportados() -> HashMap<&'static str, &'static str> {
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
            METODO_DORMIR,
            "requisição: gancho de teste do cancelamento em execução",
        ),
    ])
}

/// Troca, na árvore de símbolos, as espécies que o cliente não anunciou
/// pela alternativa do servidor do Dart (`elementKindToSymbolKind`):
/// `EnumMember` (22) → `Enum` (10), `TypeParameter` (26) → `Variable` (13).
fn ajustar_especies(simbolo: &mut Value, suportadas: Option<&[u64]>) {
    let aceita = |k: u64| suportadas.map_or(k <= 18, |l| l.contains(&k));
    if let Some(k) = simbolo["kind"].as_u64()
        && !aceita(k)
    {
        let alternativa = match k {
            22 => 10,
            26 => 13,
            _ => k,
        };
        simbolo["kind"] = json!(alternativa);
    }
    if let Some(filhos) = simbolo.get_mut("children").and_then(Value::as_array_mut) {
        for f in filhos {
            ajustar_especies(f, suportadas);
        }
    }
}
