//! Memory plateau of the whole server, through real JSON messages.
//!
//! It is `plateau_documents.rs` raised to the protocol: opens K real
//! documents, applies K incremental edits to each and closes everything,
//! with each message serialized into a `Content-Length` frame, decoded and
//! dispatched by the [`dartforge_lsp::Server`] — never by a direct API call.
//! What this measurement proves is what PLANO.md requires: N successive
//! edits stabilize `live_bytes` on a plateau instead of growing with N (the
//! failure mode of Dart's LSP, which only grows until the process has to be
//! killed).
//!
//! A separate binary on purpose: the counting allocator is global to the
//! process and the harness runs a binary's `#[test]`s in parallel threads,
//! freeing structures of finished tests at a nondeterministic time. Any
//! other test in this binary would be measured along — hence there is a
//! single `#[test]` here (the reason is documented in
//! `plateau_documents.rs`).
#[global_allocator]
static ALLOCATOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_lsp::Server;
use dartforge_lsp::transport::{frame, read_message};
use serde_json::{Value, json};
use std::io::BufReader;

/// Documents per round, as in the document owner's test.
const K: usize = 24;

/// Default corpus: the owner's reference project.
const DEFAULT_CORPUS: &str = "C:/MyDartProjects/new_sali";

/// Sends `message` through the real transport: serializes it into a frame,
/// reads the frame back and dispatches it; the outputs are discarded as
/// transient.
fn round_trip(server: &mut Server, message: &Value) {
    let framed = frame(message);
    let mut reader = BufReader::new(&framed[..]);
    let read = read_message(&mut reader)
        .expect("quadro gerado aqui mesmo decodifica")
        .expect("quadro não vazio");
    server.receive(read);
    let outputs = server.pump();
    // Touches the outputs so nothing is optimized out of the measurement.
    let mut bytes = 0usize;
    for output in &outputs {
        bytes += frame(output).len();
    }
    std::hint::black_box(bytes);
}

/// Collects up to K `.dart` files from the corpus, sorted for repeatability.
fn collect_corpus() -> Vec<(String, String)> {
    let root = std::env::var("DARTFORGE_CORPUS").unwrap_or_else(|_| DEFAULT_CORPUS.to_string());
    let mut files = Vec::new();
    collect_dir(std::path::Path::new(&root), &mut files);
    files.sort();
    files.truncate(K);
    let mut docs = Vec::new();
    for path in files {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let uri = format!("file:///plato/{}", docs.len());
            docs.push((uri, text));
        }
    }
    docs
}

/// Walks the directory ignoring build outputs, like the measurement example.
fn collect_dir(dir: &std::path::Path, output: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|n| n == ".dart_tool" || n == "build")
            {
                continue;
            }
            collect_dir(&path, output);
        } else if path.extension().is_some_and(|e| e == "dart") {
            output.push(path);
        }
    }
}

/// Fallback when the corpus is not reachable (CI without the reference
/// project): synthetic texts of fixed size per document and revision.
fn synthetic() -> Vec<(String, String)> {
    (0..K)
        .map(|d| {
            (
                format!("file:///plato/doc{d}.dart"),
                format!(
                    "int f{d}(int n) {{ var soma = 0; for (var i = 0; i < n; i++) {{ soma += i; }} return soma; }}\nvoid main() {{ print(f{d}(3)); }}\n",
                ),
            )
        })
        .collect()
}

/// `initialize`/`initialized`: the session state (capabilities,
/// configuration) belongs to the server, not to the documents, and stays in
/// the baseline.
fn initialize(server: &mut Server) {
    round_trip(
        server,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );
    round_trip(
        server,
        &json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    );
}

/// Opens the documents through the protocol and applies K rounds of
/// incremental edits to each one, returning the live bytes after each round.
fn edit(server: &mut Server, docs: &[(String, String)]) -> Vec<usize> {
    for (uri, text) in docs {
        round_trip(
            server,
            &json!({
                "jsonrpc": "2.0", "method": "textDocument/didOpen",
                "params": {"textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": text}},
            }),
        );
    }
    assert_eq!(server.open_documents(), docs.len());
    let mut live = Vec::with_capacity(K);
    for round in 1..=K {
        for (uri, _) in docs {
            // Alternates inserting and removing a space at the start of line 0: the
            // retained text has a constant size and the edit is incremental (UTF-16
            // range), like an editor keystroke.
            let change = if round % 2 == 1 {
                json!([{
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                    "text": " ",
                }])
            } else {
                json!([{
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}},
                    "text": "",
                }])
            };
            round_trip(
                server,
                &json!({
                    "jsonrpc": "2.0", "method": "textDocument/didChange",
                    "params": {"textDocument": {"uri": uri, "version": round as i32 + 1}, "contentChanges": change},
                }),
            );
        }
        live.push(dartforge_instrument::live_bytes());
    }
    live
}

/// Closes every document through the protocol.
fn close(server: &mut Server, docs: &[(String, String)]) {
    for (uri, _) in docs {
        round_trip(
            server,
            &json!({
                "jsonrpc": "2.0", "method": "textDocument/didClose",
                "params": {"textDocument": {"uri": uri}},
            }),
        );
    }
    assert_eq!(server.open_documents(), 0);
}

/// `live_bytes` stabilizes over the rounds, and closing everything returns to
/// the level right after `initialize`: nothing of the edits, versions or
/// diagnostics survives.
///
/// In a single `#[test]`: see the comment at the top of the file.
#[test]
fn plateau_through_protocol() {
    let docs = collect_corpus();
    let real_docs = docs.len() == K;
    let docs = if real_docs { docs } else { synthetic() };
    // Warm-up: a throwaway server runs the whole cycle once, so that one-time
    // process allocations (lazy state of the standard library and of the
    // harness, such as the test output capture buffer) stay out of the
    // baseline. What is measured is retention per document and per edit, not
    // process initialization.
    {
        let mut warmup = Server::new();
        initialize(&mut warmup);
        edit(&mut warmup, &docs[..1]);
        close(&mut warmup, &docs[..1]);
    }
    let mut server = Server::new();
    let empty = dartforge_instrument::live_bytes();
    initialize(&mut server);
    let before = dartforge_instrument::live_bytes();

    let live = edit(&mut server, &docs);
    assert_eq!(live.len(), K);
    let reference = live[4..8].iter().max().copied().unwrap();
    let after = live[8..].iter().max().copied().unwrap();
    assert!(
        after <= reference.saturating_add(8 * 1024),
        "memória cresceu com as edições: platô {reference} bytes, cauda {after} bytes"
    );

    close(&mut server, &docs);
    let closed = dartforge_instrument::live_bytes();
    assert!(
        closed <= before.saturating_add(K * 192),
        "fechar não liberou os documentos: antes {before}, depois {closed}"
    );
    // The harness texts also retain memory: drops them together with the
    // server before asserting the return to the initial level.
    drop(docs);
    drop(server);
    assert!(dartforge_instrument::live_bytes() <= empty.saturating_add(64));

    if !real_docs {
        eprintln!("aviso: corpus indisponível, platô medido com documentos sintéticos");
    }
}
