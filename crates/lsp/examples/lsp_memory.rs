//! Measures memory and time of the LSP server over a whole project, through the protocol.
//!
//! `cargo run -q --release -p dartforge-lsp --example lsp_memory -- C:/MyDartProjects/new_sali`
//!
//! Opens every `.dart` of the project with real `didOpen` frames, applies one
//! incremental edit to each and closes everything, printing source bytes,
//! live bytes with everything retained, peak, allocations and time. It is the
//! counterpart of the front-end measurement (`dartforge-frontend --example
//! memory_frontend`), except that here the cost includes transport, documents
//! and published diagnostics — what an editor does with the project open.
#[global_allocator]
static ALLOCATOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_lsp::Server;
use dartforge_lsp::transport::{frame, read_message};
use serde_json::{Value, json};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn collect(dir: &Path, output: &mut Vec<PathBuf>) {
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
            collect(&path, output);
        } else if path.extension().is_some_and(|e| e == "dart") {
            output.push(path);
        }
    }
}

/// One trip through the transport: serialize, decode and dispatch.
fn round_trip(server: &mut Server, message: &Value) -> usize {
    let framed = frame(message);
    let mut reader = BufReader::new(&framed[..]);
    let read: Value = read_message(&mut reader)
        .expect("valid frame")
        .expect("non-empty frame");
    server.receive(read);
    server
        .pump()
        .iter()
        .map(frame)
        .map(|q| q.len())
        .sum()
}

fn main() {
    let mut files = Vec::new();
    for root in std::env::args().skip(1) {
        collect(Path::new(&root), &mut files);
    }
    files.sort();
    let start = Instant::now();
    let base = dartforge_instrument::live_bytes();
    dartforge_instrument::reset_peak();
    let base_allocations = dartforge_instrument::allocation_count();
    let mut server = Server::new();
    round_trip(
        &mut server,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );
    let mut source_bytes = 0usize;
    let mut output_bytes = 0usize;
    let mut uris = Vec::with_capacity(files.len());
    for (i, file) in files.iter().enumerate() {
        let Ok(source) = std::fs::read_to_string(file) else {
            continue;
        };
        source_bytes += source.len();
        let uri = format!("file:///memoria/{i}.dart");
        output_bytes += round_trip(
            &mut server,
            &json!({
                "jsonrpc": "2.0", "method": "textDocument/didOpen",
                "params": {"textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": source}},
            }),
        );
        uris.push(uri);
    }
    // One incremental edit per document: inserts a space on line 0.
    for uri in &uris {
        output_bytes += round_trip(
            &mut server,
            &json!({
                "jsonrpc": "2.0", "method": "textDocument/didChange",
                "params": {
                    "textDocument": {"uri": uri, "version": 2},
                    "contentChanges": [{
                        "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                        "text": " ",
                    }],
                },
            }),
        );
    }
    let time = start.elapsed();
    let live = dartforge_instrument::live_bytes() - base;
    let peak = dartforge_instrument::peak_bytes() - base;
    let allocations = dartforge_instrument::allocation_count() - base_allocations;
    let mib = |b: usize| b as f64 / (1024.0 * 1024.0);
    println!("open files: {}", uris.len());
    println!("source: {:.2} MiB", mib(source_bytes));
    println!(
        "live with everything retained: {:.2} MiB ({:.2}x the source)",
        mib(live),
        live as f64 / source_bytes.max(1) as f64
    );
    println!("peak: {:.2} MiB", mib(peak));
    println!("bytes published in diagnostics: {output_bytes}");
    println!(
        "allocations: {allocations}; time: {:.0} ms ({:.1} MiB/s)",
        time.as_secs_f64() * 1e3,
        mib(source_bytes) / time.as_secs_f64().max(1e-9)
    );
    for uri in &uris {
        round_trip(
            &mut server,
            &json!({
                "jsonrpc": "2.0", "method": "textDocument/didClose",
                "params": {"textDocument": {"uri": uri}},
            }),
        );
    }
    drop(uris);
    drop(server);
    println!(
        "after closing everything: {} bytes above the baseline",
        dartforge_instrument::live_bytes().saturating_sub(base)
    );
}
