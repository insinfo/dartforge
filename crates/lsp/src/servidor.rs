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
    /// O cliente aceita snippets no completar e não pediu
    /// `completeFunctionCalls: false`: chamadas saem com os parênteses e os
    /// parâmetros obrigatórios como marcadores.
    completar_chamadas: bool,
    /// O cliente aceita Markdown na documentação dos itens.
    documentacao_markdown: bool,
    /// Raízes do workspace anunciadas no `initialize` (`rootUri`,
    /// `workspaceFolders`), para o `workspace/symbol` varrer o disco.
    raizes: Vec<std::path::PathBuf>,
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
            encerrar: false,
            codigo: 1,
            simbolos_hierarquicos: false,
            hover_markdown: false,
            preparar_renomeacao: false,
            mudancas_versionadas: false,
            renomear_arquivos: false,
            completar_chamadas: false,
            documentacao_markdown: false,
            raizes: Vec::new(),
            tipado: None,
            tipado_tentado: false,
            despertar: None,
            tipados_descartados: 0,
            tipados_publicados: HashMap::new(),
        }
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
    }

    /// Publica os resultados tipados prontos que ainda são da versão vigente;
    /// os demais são descartados.
    fn drenar_tipados(&mut self) -> Vec<Value> {
        let Some(t) = &self.tipado else { return Vec::new() };
        let mut saidas = Vec::new();
        for r in t.receber() {
            if self.documentos.version(&r.uri) != Some(r.versao) {
                self.tipados_descartados += 1;
                continue;
            }
            saidas.push(self.publicacao(&r.uri, &r.diagnosticos));
            self.tipados_publicados.insert(r.uri, (r.versao, r.diagnosticos));
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
    /// `RequestCancelled` se ela foi cancelada). Respostas a mensagens que
    /// não são requisições nem notificações válidas são ignoradas em
    /// silêncio: o servidor nunca envia requisições, então não há resposta
    /// a tratar.
    pub fn bombear(&mut self) -> Vec<Value> {
        self.aplicar_cancelamentos();
        let mut saidas = Vec::new();
        while let Some(proxima) = self.fila.front() {
            if eh_requisicao(proxima) {
                break;
            }
            let mensagem = self.fila.pop_front().expect("fila não vazia");
            if let Some(saida) = self.tratar_notificacao(&mensagem) {
                saidas.push(saida);
            }
            if self.encerrar {
                return saidas;
            }
        }
        if let Some(requisicao) = self.fila.pop_front() {
            debug_assert!(eh_requisicao(&requisicao));
            saidas.push(self.tratar_requisicao(&requisicao));
        }
        // Resultados tipados depois das notificações: o que uma mudança já
        // recebida tornou velho é descartado aqui, antes de chegar ao editor.
        // (Uma requisição não muda documentos; a resposta sai primeiro.)
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
        let metodo = mensagem.get("method")?.as_str()?;
        match metodo {
            "initialized" | "$/cancelRequest" => None,
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
                self.analisador.documento_alterado(uri);
                self.pedir_tipado(uri);
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
                Some(publicacao_vazia(uri))
            }
            _ => {
                registrar(format!("notificação desconhecida ignorada: {metodo}"));
                None
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
        match metodo {
            "initialize" => {
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
                let aceita_renomear_arquivo = mensagem
                    .pointer("/params/capabilities/workspace/workspaceEdit/resourceOperations")
                    .and_then(Value::as_array)
                    .is_some_and(|l| l.iter().any(|o| o.as_str() == Some("rename")));
                self.renomear_arquivos = self.mudancas_versionadas
                    && aceita_renomear_arquivo
                    && mensagem.pointer("/params/initializationOptions/renameFilesWithClasses").and_then(Value::as_str) == Some("always");
                self.raizes = raizes_do_initialize(mensagem.get("params"));
                self.completar_chamadas = mensagem
                    .pointer("/params/capabilities/textDocument/completion/completionItem/snippetSupport")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    && mensagem.pointer("/params/initializationOptions/completeFunctionCalls").and_then(Value::as_bool) != Some(false);
                self.documentacao_markdown = mensagem
                    .pointer("/params/capabilities/textDocument/completion/completionItem/documentationFormat")
                    .and_then(Value::as_array)
                    .is_some_and(|f| f.iter().any(|x| x.as_str() == Some("markdown")));
                let renomear = if self.preparar_renomeacao { json!({"prepareProvider": true}) } else { json!(true) };
                resposta(&id, json!({
                    "capabilities": {
                        "textDocumentSync": SINCRONIZACAO_INCREMENTAL,
                        "positionEncoding": "utf-16",
                        "documentSymbolProvider": true,
                        "workspaceSymbolProvider": true,
                        "definitionProvider": true,
                        "referencesProvider": true,
                        "hoverProvider": true,
                        "renameProvider": renomear,
                        "codeActionProvider": {"codeActionKinds": ["quickfix", "refactor"]},
                        "completionProvider": {
                            "triggerCharacters": ["."],
                            "resolveProvider": true,
                        },
                    },
                    "serverInfo": {
                        "name": "dartforge-lsp",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                }))
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
                let simbolos = uri
                    .and_then(|u| self.documentos.get(u).map(|t| (u, t.to_string())))
                    .map_or_else(Vec::new, |(u, t)| self.analisador.simbolos(u, &t));
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
                    let declaracao = achados.declaracao.filter(|_| incluir_declaracao);
                    let locais: Vec<Value> = declaracao
                        .iter()
                        .chain(achados.usos.iter())
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
                        let mut valor = format!("```dart\n{}\n```", hover.descricao);
                        if let Some(t) = &hover.tipo {
                            valor.push_str(&format!("\nType: `{t}`"));
                        }
                        if let Some(d) = &hover.documentacao {
                            valor.push_str(&format!("\n\n---\n{d}"));
                        }
                        json!({"kind": "markdown", "value": valor})
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
                let resultado = self.posicao_da_requisicao(mensagem).and_then(|(u, offset)| {
                    let completar = self.analisador.completar(&self.documentos, &u, offset)?;
                    let texto = self.documentos.get(&u)?;
                    let tabela = self.documentos.linhas(&u)?;
                    let range = intervalo_lsp(texto, tabela, completar.inicio, completar.fim);
                    // Já há parênteses depois do nome: só o nome entra.
                    let depois = texto[completar.fim.min(texto.len())..]
                        .trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$');
                    let com_parenteses = depois.starts_with('(');
                    let itens: Vec<Value> = completar
                        .itens
                        .iter()
                        .enumerate()
                        .map(|(i, item)| {
                            let snippet = match (&item.chamada, self.completar_chamadas && !com_parenteses) {
                                (Some(chamada), true) => Some(snippet_de_chamada(&item.inserir, chamada)),
                                _ => None,
                            };
                            let mut valor = json!({
                                "label": item.rotulo,
                                "kind": item.especie,
                                "sortText": format!("{i:05}"),
                                "filterText": item.inserir.trim_end(),
                                "textEdit": {"range": range, "newText": snippet.as_deref().unwrap_or(&item.inserir)},
                            });
                            if snippet.is_some() {
                                valor["insertTextFormat"] = json!(2);
                            }
                            if let Some(detalhe) = &item.detalhe {
                                valor["detail"] = json!(detalhe);
                            }
                            if let Some(imp) = &item.importar {
                                valor["additionalTextEdits"] = json!([{
                                    "range": intervalo_lsp(texto, tabela, imp.span.start, imp.span.end),
                                    "newText": imp.texto,
                                }]);
                            }
                            if let Some((arquivo, inicio)) = &item.origem {
                                valor["data"] = json!({"arquivo": arquivo, "inicio": inicio});
                            }
                            valor
                        })
                        .collect();
                    Some(json!({"isIncomplete": completar.incompleta, "items": itens}))
                });
                resposta(&id, resultado.unwrap_or(Value::Null))
            }
            "completionItem/resolve" => {
                let mut item = mensagem.get("params").cloned().unwrap_or(Value::Null);
                if let Some(doc) = self.documentacao_do_item(&item) {
                    item["documentation"] = if self.documentacao_markdown {
                        json!({"kind": "markdown", "value": doc})
                    } else {
                        json!(doc)
                    };
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
                for acao in acoes {
                    let permitida = apenas.as_ref().is_none_or(|l| {
                        l.iter().any(|k| acao.especie == *k || acao.especie.starts_with(&format!("{k}.")))
                    });
                    if !permitida {
                        continue;
                    }
                    let edicao = self.edicao_de_workspace(&acao.edicoes, None);
                    let mut valor = json!({"title": acao.titulo, "kind": acao.especie, "edit": edicao});
                    if let (Some(d), Some(texto), Some(tabela)) =
                        (&acao.diagnostico, self.documentos.get(&u), self.documentos.linhas(&u))
                    {
                        valor["diagnostics"] = json!([converter_diagnostico(texto, tabela, d)]);
                        valor["isPreferred"] = json!(true);
                    }
                    saida.push(valor);
                }
                resposta(&id, json!(saida))
            }
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
                match self.analisador.renomear(&self.documentos, &u, offset, &novo) {
                    Ok(renomeacao) => {
                        let mut edicoes = renomeacao.edicoes;
                        let mut recurso = None;
                        if self.renomear_arquivos
                            && let Some(arquivo) = renomeacao.arquivo
                        {
                            edicoes.extend(arquivo.diretivas);
                            recurso = Some(json!({"kind": "rename", "oldUri": arquivo.de, "newUri": arquivo.para}));
                        }
                        resposta(&id, self.edicao_de_workspace(&edicoes, recurso))
                    }
                    Err(motivo) => erro(&id, RENOMEAR_INVALIDO, motivo),
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
                    format!("método desconhecido: {metodo}"),
                )
            }
        }
    }

    /// A documentação da declaração que o item aponta (`data.arquivo`,
    /// `data.inicio`), lida do texto aberto ou do disco.
    fn documentacao_do_item(&self, item: &Value) -> Option<String> {
        let arquivo = std::path::PathBuf::from(item.pointer("/data/arquivo")?.as_str()?);
        let inicio = item.pointer("/data/inicio")?.as_u64()? as usize;
        let aberto = url::Url::from_file_path(&arquivo)
            .ok()
            .and_then(|u| self.documentos.get(u.as_str()).map(str::to_string));
        let texto = aberto.or_else(|| std::fs::read_to_string(&arquivo).ok())?;
        if inicio > texto.len() || !texto.is_char_boundary(inicio) {
            return None;
        }
        crate::dartdoc::documentacao(&texto, inicio)
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
        let mut uris: std::collections::BTreeSet<String> = self.documentos.uris().map(str::to_string).collect();
        let mut raizes = self.raizes.clone();
        for aberto in self.documentos.uris() {
            if let Some(arquivo) = url::Url::parse(aberto).ok().and_then(|u| u.to_file_path().ok()) {
                let raiz = crate::projeto::raiz_do_projeto(&arquivo);
                if raiz.join("pubspec.yaml").is_file() && !raizes.contains(&raiz) {
                    raizes.push(raiz);
                }
            }
        }
        for raiz in &raizes {
            for arquivo in crate::projeto::arquivos_do_projeto(raiz) {
                if let Ok(u) = url::Url::from_file_path(&arquivo) {
                    uris.insert(u.to_string());
                }
            }
        }
        let mut achados: Vec<(u8, usize, Value)> = Vec::new();
        for uri in uris {
            let texto = match self.documentos.get(&uri) {
                Some(t) => t.to_string(),
                None => {
                    let Some(caminho) = url::Url::parse(&uri).ok().and_then(|u| u.to_file_path().ok()) else { continue };
                    let Ok(t) = std::fs::read_to_string(caminho) else { continue };
                    t
                }
            };
            let mut planos = Vec::new();
            for simbolo in self.analisador.simbolos(&uri, &texto) {
                achatar_simbolos(&simbolo, &uri, None, &mut planos);
            }
            for s in planos {
                // Subsequência solta (qualidade 4) é ruído numa busca global.
                let Some(q) = s["name"].as_str().and_then(|n| crate::aproximado::pontuar(consulta, n)).filter(|q| *q <= 3) else {
                    continue;
                };
                let ordem = achados.len();
                achados.push((if q <= 2 { 0 } else { 1 }, ordem, s));
            }
        }
        achados.sort_by_key(|(q, ordem, _)| (*q, *ordem));
        achados.into_iter().map(|(_, _, s)| s).collect()
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

/// O snippet de uma chamada: `nome(${1:a}, ${2:b})$0`, com os nomeados
/// como `nome: ${n:nome}`; sem parâmetros, `nome()$0`; desconhecidos,
/// `nome($0)`. `$`, `}` e `\` do texto são escapados.
fn snippet_de_chamada(nome: &str, chamada: &crate::completar::Chamada) -> String {
    let escapar = |t: &str| t.replace('\\', "\\\\").replace('$', "\\$").replace('}', "\\}");
    match chamada {
        crate::completar::Chamada::Desconhecida => format!("{}($0)", escapar(nome)),
        crate::completar::Chamada::Parametros(ps) => {
            let marcadores: Vec<String> = ps
                .iter()
                .enumerate()
                .map(|(i, p)| match p.strip_suffix(": ") {
                    Some(n) => format!("{}: ${{{}:{}}}", escapar(n), i + 1, escapar(n)),
                    None => format!("${{{}:{}}}", i + 1, escapar(p)),
                })
                .collect();
            format!("{}({})$0", escapar(nome), marcadores.join(", "))
        }
    }
}

/// Raízes do workspace no `initialize`: `workspaceFolders`, senão
/// `rootUri`, senão `rootPath`; só diretórios existentes.
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
