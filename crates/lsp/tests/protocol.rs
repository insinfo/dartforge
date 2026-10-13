//! Acceptance of the LSP protocol through bytes on the server's
//! `stdin`/`stdout`.
//!
//! No test here calls the server through a function: each session spawns a
//! child process (`CARGO_BIN_EXE_dartforge-lsp`), writes `Content-Length`
//! frames to `stdin` and reads frames from `stdout`. What counts is the
//! wire, not the API.
//!
//! The plateau test (`plateau_through_protocol`) lives in
//! `tests/plateau_protocol.rs`, in a separate binary: the counting allocator
//! is global to the process and the tests of one binary run in parallel
//! threads, so the memory measurement needs the process to itself (the
//! reason is documented in `plateau_documents.rs`).

use dartforge_lsp::transport::{frame, read_message};
use serde_json::{Value, json};
use std::io::{BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

/// Maximum time per wait: a debug binary starts slowly on Windows.
const TIME_LIMIT: Duration = Duration::from_secs(20);

/// Read from the thread that continuously drains the child's `stdout`.
///
/// Draining nonstop matters: if the child fills the output pipe it blocks
/// and the session hangs — the test would hang along, instead of measuring.
enum Read {
    Frame(Value),
    Error(String),
    End,
}

/// Session with the server in a child process.
struct Session {
    child: Child,
    entry: ChildStdin,
    reads: Receiver<Read>,
    /// Notifications and responses that arrived but were not consumed yet.
    pending: Vec<Value>,
    /// Frames with framing errors (the `clean_stdout` test forbids them).
    errors: Vec<String>,
}

impl Session {
    /// Spawns the server with the given environment variables.
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

    /// Sends a JSON message as a frame.
    fn send(&mut self, message: &Value) {
        let framed = frame(message);
        self.entry
            .write_all(&framed)
            .expect("escrever no stdin do filho");
        self.entry.flush().expect("descarregar stdin do filho");
    }

    /// Pumps reads until `condition` finds the message, or the limit expires.
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

    /// Response with the given `id`.
    fn response(&mut self, id: i64) -> Value {
        self.wait_for(&format!("resposta {id}"), |m| {
            m.get("id").is_some_and(|v| v.as_i64() == Some(id))
                && (m.get("result").is_some() || m.get("error").is_some())
        })
    }

    /// Last diagnostics publication for `uri` (discards the earlier ones).
    fn diagnostics(&mut self, uri: &str) -> Value {
        // Gives the server a chance to publish after the edit: waits until there is
        // at least one new publication for the uri.
        self.wait_for(&format!("diagnósticos de {uri}"), |m| {
            m.get("method")
                .is_some_and(|v| v.as_str() == Some("textDocument/publishDiagnostics"))
                && m.get("params")
                    .and_then(|p| p.get("uri"))
                    .and_then(Value::as_str)
                    == Some(uri)
        })
    }

    /// Ends the protocol (`shutdown` + `exit`) and returns the exit code.
    fn shutdown(mut self, id: i64) -> i32 {
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": "shutdown"}));
        let response = self.response(id);
        assert_eq!(response["result"], Value::Null, "shutdown responde nulo");
        self.send(&json!({"jsonrpc": "2.0", "method": "exit"}));
        drop(self.entry);
        let output = self.child.wait().expect("esperar o filho");
        // Drains what is left so as not to mask a framing error.
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

/// Minimal handshake of every session.
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

/// Diagnostics list of the publication.
fn items(publication: &Value) -> &Vec<Value> {
    publication["params"]["diagnostics"]
        .as_array()
        .expect("publishDiagnostics traz lista")
}

/// Full transcript over the wire: two errors, incremental fix of one,
/// closing with an empty list and exit with code 0.
#[test]
fn complete_session() {
    let mut session = Session::new(&[]);
    start(&mut session);

    let uri = "file:///sessao/teste.dart";
    // The variables are read (`x;`): `unused_local_variable` is published, and
    // the test counts only the syntax errors.
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

    // Incremental fix: replaces the `;` on line 0 with `1;` (UTF-16 range).
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

/// Columns count UTF-16 units in both directions, with CRLF and a lone
/// surrogate escape in the same file.
#[test]
fn correct_utf16() {
    let mut session = Session::new(&[]);
    start(&mut session);

    // `👭` takes 4 bytes and 2 UTF-16 units; the error's `;` is at byte 38 of
    // line 0 and at UTF-16 unit 36. The `\uD800` is ASCII (an escape), and the
    // lexer accepts it as a valid literal: it produces no diagnostic.
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

    // Incremental edit addressed in UTF-16 after the emoji: replacing the `;`
    // (units 36..37) with `1;` fixes it; treating the column as a byte, the cut
    // would land on the `=` and the error would remain.
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

/// With verbose logging on, `stdout` is still protocol only.
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
    // Closing does not republish: the client already has the empty list (the
    // Dart server only publishes an empty list over previously published
    // errors).
    session.send(&close(uri));

    assert_eq!(session.shutdown(10), 0);
    // `shutdown` has already asserted it: no byte outside `Content-Length` on
    // stdout.
}

/// Two queued requests and the cancellation of the second before the first
/// finishes: its response is `RequestCancelled` (-32800).
#[test]
fn cancellation() {
    let mut session = Session::new(&[]);
    start(&mut session);

    // The three messages go out in a burst: the reader enqueues while the main
    // thread is still sleeping, and the cancellation holds either way because
    // `pump` applies the cancels of the whole queue before executing.
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
