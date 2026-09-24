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
        }
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
                self.documentos
                    .open(uri.to_string(), versao, texto.to_string());
                Some(self.publicar(uri))
            }
            "textDocument/didChange" => {
                let params = mensagem.get("params")?;
                let doc = params.get("textDocument")?;
                let uri = doc.get("uri")?.as_str()?;
                let versao = doc.get("version")?.as_i64()? as i32;
                let mudancas = ler_mudancas(params.get("contentChanges")?);
                self.documentos
                    .apply(uri, versao, &mudancas)
                    .then(|| self.publicar(uri))
            }
            "textDocument/didClose" => {
                let params = mensagem.get("params")?;
                let uri = params.get("textDocument")?.get("uri")?.as_str()?;
                self.documentos.close(uri);
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
                resposta(&id, json!({
                    "capabilities": {
                        "textDocumentSync": SINCRONIZACAO_INCREMENTAL,
                        "positionEncoding": "utf-16",
                        "documentSymbolProvider": true,
                        "workspaceSymbolProvider": true,
                        "definitionProvider": true,
                        "hoverProvider": true,
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
                    .to_lowercase();
                let mut uris: Vec<String> = self.documentos.uris().map(str::to_string).collect();
                uris.sort_unstable();
                let mut resultado = Vec::new();
                for uri in uris {
                    let Some(texto) = self.documentos.get(&uri).map(str::to_string) else {
                        continue;
                    };
                    for simbolo in self.analisador.simbolos(&uri, &texto) {
                        achatar_simbolos(&simbolo, &uri, None, &mut resultado);
                    }
                }
                resultado.retain(|s| s["name"].as_str().is_some_and(|n| n.to_lowercase().contains(&consulta)));
                resposta(&id, json!(resultado))
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
                    let (destino, selecao) = self.analisador.definicao(u, &texto, offset)?;
                    let range = selecao.map_or_else(
                        || json!({
                            "start": {"line": 0, "character": 0},
                            "end": {"line": 0, "character": 0},
                        }),
                        |s| {
                            let tabela = self.documentos.linhas(u).expect("documento aberto");
                            let (l0, c0) = tabela.posicao_de_offset(&texto, s.start);
                            let (l1, c1) = tabela.posicao_de_offset(&texto, s.end);
                            json!({
                                "start": {"line": l0, "character": c0},
                                "end": {"line": l1, "character": c1},
                            })
                        },
                    );
                    Some(json!({
                        "uri": destino,
                        "range": range,
                    }))
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
                    let (span, descricao, tipo) = self.analisador.hover(u, &texto, offset)?;
                    let (l0, c0) = tabela.posicao_de_offset(&texto, span.start);
                    let (l1, c1) = tabela.posicao_de_offset(&texto, span.end);
                    let conteudo = if self.hover_markdown {
                        let mut valor = format!("```dart\n{descricao}\n```");
                        if let Some(ref t) = tipo {
                            valor.push_str(&format!("\nType: `{t}`"));
                        }
                        json!({"kind": "markdown", "value": valor})
                    } else {
                        let valor = tipo.map_or(descricao.clone(), |t| format!("{descricao}\nType: {t}"));
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

    /// Diagnostica o documento e monta `textDocument/publishDiagnostics`.
    fn publicar(&mut self, uri: &str) -> Value {
        let Some(texto) = self.documentos.get(uri).map(str::to_string) else {
            return publicacao_vazia(uri);
        };
        let versao = self.documentos.version(uri).unwrap_or(0);
        let diagnosticos = self.analisador.diagnosticar(uri, &texto);
        let Some(tabela) = self.documentos.linhas(uri) else {
            return publicacao_vazia(uri);
        };
        let itens: Vec<Value> = diagnosticos
            .iter()
            .map(|d| converter_diagnostico(&texto, tabela, d))
            .collect();
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {"uri": uri, "version": versao, "diagnostics": itens},
        })
    }
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
            METODO_DORMIR,
            "requisição: gancho de teste do cancelamento em execução",
        ),
    ])
}
