//! Aceite do protocolo LSP por bytes no `stdin`/`stdout` do servidor.
//!
//! Nenhum teste aqui chama o servidor por função: cada sessão gera um
//! processo filho (`CARGO_BIN_EXE_dartforge-lsp`), escreve quadros
//! `Content-Length` no `stdin` e lê quadros do `stdout`. O que vale é o fio,
//! não a API.
//!
//! O teste de platô (`plateau_through_protocol`) vive em
//! `tests/plateau_protocol.rs`, num binário separado: o alocador contador é
//! global ao processo e os testes de um mesmo binário rodam em threads
//! paralelas, então a medição de memória precisa do processo só para ela
//! (a razão documentada em `plateau_documents.rs`).

use dartforge_lsp::transport::{frame, read_message};
use serde_json::{Value, json};
use std::io::{BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

/// Tempo máximo por espera: binário debug arranca devagar no Windows.
const TIME_LIMIT: Duration = Duration::from_secs(20);

/// Leitura da thread que drena o `stdout` do filho continuamente.
///
/// Drenar sem parar importa: se o filho encher o pipe de saída ele bloqueia
/// e a sessão trava — o teste travaria junto, em vez de medir.
enum Read {
    Frame(Value),
    Error(String),
    End,
}

/// Sessão com o servidor em processo filho.
struct Session {
    child: Child,
    entry: ChildStdin,
    reads: Receiver<Read>,
    /// Notificações e respostas que chegaram mas ainda não foram consumidas.
    pending: Vec<Value>,
    /// Quadros com erro de enquadramento (o teste `clean_stdout` os proíbe).
    errors: Vec<String>,
}

impl Session {
    /// Gera o servidor com as variáveis de ambiente dadas.
    fn new(variables: &[(&str, &str)]) -> Self {
        let path = env!("CARGO_BIN_EXE_dartforge-lsp");
        let mut command = Command::new(path);
        command
            .args(["--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in variables {
            command.env(key, value);
        }
        let mut child = command.spawn().expect("gerar dartforge-lsp");
        let entry = child.stdin.take().expect("stdin do filho");
        let output = child.stdout.take().expect("stdout do filho");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                match read_message(&mut reader) {
                    Ok(Some(frame)) => {
                        if tx.send(Read::Frame(frame)).is_err() {
                            return;
                        }
                    }
                    Ok(None) => {
                        let _ = tx.send(Read::End);
                        return;
                    }
                    Err(e) => {
                        let _ = tx.send(Read::Error(e.to_string()));
                        return;
                    }
                }
            }
        });
        Self {
            child,
            entry,
            reads: rx,
            pending: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Envia uma mensagem JSON como um quadro.
    fn send(&mut self, message: &Value) {
        let framed = frame(message);
        self.entry
            .write_all(&framed)
            .expect("escrever no stdin do filho");
        self.entry.flush().expect("descarregar stdin do filho");
    }

    /// Bombeia leituras até `condition` achar a mensagem, ou estoura o limite.
    fn wait_for(&mut self, what: &str, mut condition: impl FnMut(&Value) -> bool) -> Value {
        let start = Instant::now();
        loop {
            if let Some(pos) = self.pending.iter().position(&mut condition) {
                return self.pending.remove(pos);
            }
            let remaining = TIME_LIMIT.saturating_sub(start.elapsed());
            assert!(
                !remaining.is_zero(),
                "tempo esgotado esperando {what}; pendentes: {:?}",
                self.pending
            );
            match self.reads.recv_timeout(remaining) {
                Ok(Read::Frame(frame)) => self.pending.push(frame),
                Ok(Read::Error(e)) => self.errors.push(e),
                Ok(Read::End) => {
                    panic!(
                        "stdout fechou esperando {what}; pendentes: {:?}",
                        self.pending
                    )
                }
                Err(_) => panic!(
                    "tempo esgotado esperando {what}; pendentes: {:?}",
                    self.pending
                ),
            }
        }
    }

    /// Resposta com o `id` dado.
    fn response(&mut self, id: i64) -> Value {
        self.wait_for(&format!("resposta {id}"), |m| {
            m.get("id").is_some_and(|v| v.as_i64() == Some(id))
                && (m.get("result").is_some() || m.get("error").is_some())
        })
    }

    /// Última publicação de diagnósticos para a `uri` (descarta as anteriores).
    fn diagnostics(&mut self, uri: &str) -> Value {
        // Dá ao servidor uma chance de publicar após a edição: espera até
        // haver ao menos uma publicação nova para a uri.
        self.wait_for(&format!("diagnósticos de {uri}"), |m| {
            m.get("method")
                .is_some_and(|v| v.as_str() == Some("textDocument/publishDiagnostics"))
                && m.get("params")
                    .and_then(|p| p.get("uri"))
                    .and_then(Value::as_str)
                    == Some(uri)
        })
    }

    /// Encerra o protocolo (`shutdown` + `exit`) e devolve o código de saída.
    fn shutdown(mut self, id: i64) -> i32 {
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": "shutdown"}));
        let response = self.response(id);
        assert_eq!(response["result"], Value::Null, "shutdown responde nulo");
        self.send(&json!({"jsonrpc": "2.0", "method": "exit"}));
        drop(self.entry);
        let output = self.child.wait().expect("esperar o filho");
        // Drena o que restou para não mascarar erro de enquadramento.
        while let Ok(read) = self.reads.try_recv() {
            match read {
                Read::Frame(frame) => self.pending.push(frame),
                Read::Error(e) => self.errors.push(e),
                Read::End => break,
            }
        }
        assert!(
            self.errors.is_empty(),
            "stdout com bytes fora do enquadramento: {:?}",
            self.errors
        );
        output.code().expect("código de saída")
    }
}

/// Aperto de mãos mínimo de toda sessão.
fn start(session: &mut Session) {
    session.send(&json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"processId": null, "rootUri": "file:///tmp", "capabilities": {}},
    }));
    let response = session.response(1);
    assert_eq!(
        response["result"]["capabilities"]["textDocumentSync"], 2,
        "capacidade anunciada é sincronização incremental"
    );
    session.send(&json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}));
}

fn publish(uri: &str, version: i32, text: &str) -> Value {
    json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": {"textDocument": {"uri": uri, "languageId": "dart", "version": version, "text": text}},
    })
}

fn edit(uri: &str, version: i32, changes: Value) -> Value {
    json!({
        "jsonrpc": "2.0", "method": "textDocument/didChange",
        "params": {"textDocument": {"uri": uri, "version": version}, "contentChanges": changes},
    })
}

fn close(uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0", "method": "textDocument/didClose",
        "params": {"textDocument": {"uri": uri}},
    })
}

/// Lista de diagnósticos da publicação.
fn items(publication: &Value) -> &Vec<Value> {
    publication["params"]["diagnostics"]
        .as_array()
        .expect("publishDiagnostics traz lista")
}

/// Transcrição completa pelo fio: dois erros, correção incremental de um,
/// fechamento com lista vazia e saída com código 0.
#[test]
fn complete_session() {
    let mut session = Session::new(&[]);
    start(&mut session);

    let uri = "file:///sessao/teste.dart";
    // As variáveis são lidas (`x;`): `unused_local_variable` é publicado, e
    // o teste conta só os erros de sintaxe.
    session.send(&publish(
        uri,
        1,
        "void main() { int x = ; x; }\nvoid outra() { int y = ; y; }\n",
    ));
    let first = session.diagnostics(uri);
    assert_eq!(
        items(&first).len(),
        2,
        "dois erros de sintaxe: {first}"
    );
    assert_eq!(first["params"]["diagnostics"][0]["source"], "dartforge");
    assert_eq!(
        first["params"]["diagnostics"][0]["range"]["start"],
        json!({"line": 0, "character": 22}),
        "span de bytes vira posição UTF-16: {first}"
    );
    assert_eq!(
        first["params"]["diagnostics"][0]["severity"], 1,
        "sintaxe quebrada é erro: {first}"
    );

    // Correção incremental: troca o `;` da linha 0 por `1;` (intervalo UTF-16).
    session.send(&edit(
        uri,
        2,
        json!([{
            "range": {"start": {"line": 0, "character": 22}, "end": {"line": 0, "character": 23}},
            "text": "1;",
        }]),
    ));
    let second = session.diagnostics(uri);
    assert_eq!(items(&second).len(), 1, "um erro restante: {second}");
    assert_eq!(
        second["params"]["diagnostics"][0]["range"]["start"],
        json!({"line": 1, "character": 23}),
        "o erro que sobrou é o da segunda declaração: {second}"
    );

    session.send(&close(uri));
    let third = session.diagnostics(uri);
    assert!(
        items(&third).is_empty(),
        "documento fechado publica lista vazia: {third}"
    );

    assert_eq!(session.shutdown(10), 0, "exit após shutdown sai com 0");
}

/// Colunas contam unidades UTF-16 nos dois sentidos, com CRLF e escape de
/// surrogate solto no mesmo arquivo.
#[test]
fn correct_utf16() {
    let mut session = Session::new(&[]);
    start(&mut session);

    // `👭` ocupa 4 bytes e 2 unidades UTF-16; o `;` do erro está no byte 38
    // da linha 0 e na unidade UTF-16 36. O `\uD800` é ASCII (escape), e o
    // lexer o aceita como literal válido: não gera diagnóstico.
    let uri = "file:///utf16/emoji.dart";
    session.send(&publish(
        uri,
        1,
        "var s = '👭'; void main() { int x = ; x; }\r\nvar e = '\\uD800';\r\n",
    ));
    let first = session.diagnostics(uri);
    assert_eq!(items(&first).len(), 1, "só o `;`: {first}");
    assert_eq!(
        first["params"]["diagnostics"][0]["range"]["start"],
        json!({"line": 0, "character": 36}),
        "coluna publicada é UTF-16, não byte: {first}"
    );
    assert_eq!(
        first["params"]["diagnostics"][0]["range"]["end"],
        json!({"line": 0, "character": 37}),
        "fim do span também é UTF-16: {first}"
    );

    // Edição incremental endereçada em UTF-16 depois do emoji: trocar o `;`
    // (unidades 36..37) por `1;` conserta; tratando coluna como byte, o corte
    // cairia no `=` e o erro continuaria.
    session.send(&edit(
        uri,
        2,
        json!([{
            "range": {"start": {"line": 0, "character": 36}, "end": {"line": 0, "character": 37}},
            "text": "1;",
        }]),
    ));
    let second = session.diagnostics(uri);
    assert!(
        items(&second).is_empty(),
        "edição UTF-16 acertou o byte: {second}"
    );

    assert_eq!(session.shutdown(10), 0);
}

/// Com log detalhado ligado, o `stdout` continua sendo só protocolo.
#[test]
fn clean_stdout() {
    let mut session = Session::new(&[("RUST_LOG", "debug")]);
    start(&mut session);

    let uri = "file:///limpo/a.dart";
    session.send(&publish(uri, 1, "void main() { int x = ; x; }\n"));
    let first = session.diagnostics(uri);
    assert_eq!(items(&first).len(), 1);
    session.send(&edit(
        uri,
        2,
        json!([{"text": "void main() { int x = 1; x; }\n"}]),
    ));
    let second = session.diagnostics(uri);
    assert!(items(&second).is_empty());
    // O fechamento não republica: o cliente já tem a lista vazia (o
    // servidor do Dart só publica vazio sobre erros publicados antes).
    session.send(&close(uri));

    assert_eq!(session.shutdown(10), 0);
    // `shutdown` já afirmou: nenhum byte fora de `Content-Length` no stdout.
}

/// Duas requisições enfileiradas e cancelamento da segunda antes de a
/// primeira terminar: a resposta dela é `RequestCancelled` (-32800).
#[test]
fn cancellation() {
    let mut session = Session::new(&[]);
    start(&mut session);

    // As três mensagens saem em rajada: a leitora enfileira enquanto a
    // principal ainda executa o dormir, e o cancelamento vale de todo jeito
    // porque `pump` aplica os cancels da fila inteira antes de executar.
    session.send(&json!({
        "jsonrpc": "2.0", "id": 2, "method": "dartforge/dormir",
        "params": {"ms": 1500},
    }));
    session.send(&json!({"jsonrpc": "2.0", "id": 3, "method": "shutdown"}));
    session.send(&json!({
        "jsonrpc": "2.0", "method": "$/cancelRequest", "params": {"id": 3},
    }));

    let sleep = session.response(2);
    assert_eq!(
        sleep["result"],
        Value::Null,
        "a primeira termina: {sleep}"
    );
    let cancelled = session.response(3);
    assert_eq!(
        cancelled["error"]["code"], -32800,
        "a segunda foi cancelada na fila: {cancelled}"
    );

    assert_eq!(session.shutdown(4), 0);
}
