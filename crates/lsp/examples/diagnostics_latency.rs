//! Measures the latency of LSP diagnostics (immediate and typed) per file.
//!
//! `cargo run -q --release -p dartforge-lsp --example diagnostics_latency -- <file.dart>...`
//!
//! For each file, through the protocol (`didOpen`/`didChange` in JSON, like
//! an editor): opens it, waits for the typed publication, applies 10
//! incremental edits waiting for each one, and a burst of 20 edits without
//! waiting. The SDK is the one from `SdkLayout::discover`
//! (`DARTFORGE_SDK_LIB`, `DART_SDK`, `PATH`). Prints:
//!
//! * `immediate`: time of the `didChange` `pump` (parser + local checkers),
//!   the publication that goes out right away;
//! * `typed`: from `didChange` to the typed publication (the `dartforge
//!   analyze` analysis in the background), median and maximum;
//! * the burst: how many typed publications went out (coalescing wants 1)
//!   and how long until the last one;
//! * live memory above the baseline after closing everything (nothing
//!   retained).
#[global_allocator]
static ALLOCATOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_lsp::{SemanticAnalyzer, Server};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

const LIMIT: Duration = Duration::from_secs(600);

fn edit(uri: &str, version: i64) -> Value {
    json!({
        "jsonrpc": "2.0", "method": "textDocument/didChange",
        "params": {
            "textDocument": {"uri": uri, "version": version},
            "contentChanges": [{
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                "text": " ",
            }],
        },
    })
}

/// Waits for the typed publication of `uri` (the immediate ones go out in `pump`).
fn wait_typed(server: &mut Server<SemanticAnalyzer>, uri: &str) -> Option<Value> {
    server
        .await_diagnostics(LIMIT)
        .into_iter()
        .rev()
        .find(|m| m["params"]["uri"] == uri)
}

/// Compares the typed publication with what `dartforge analyze` publishes
/// for the same file (same code path: `Engine` + `diagnosticos_json`), by
/// (code, line, column).
fn check_with_analyze(file: &std::path::Path, publication: &Value) -> String {
    let mut root = file.parent().unwrap().to_path_buf();
    while !root.join("pubspec.yaml").is_file() {
        match root.parent() {
            Some(p) => root = p.to_path_buf(),
            None => {
                root = file.parent().unwrap().to_path_buf();
                break;
            }
        }
    }
    let engine = dartforge_parity::analise::Motor::descobrir().expect("SDK");
    let config = root.join(".dart_tool/package_config.json");
    let config = config.is_file().then_some(config);
    let options = dartforge_parity::filtros::Opcoes::ler(&root);
    let analysis = engine.analisar(&root, &[file.to_path_buf()], config.as_deref());
    let mut cli: Vec<(String, u64, u64)> =
        dartforge_parity::diagnosticos_json(&analysis, &root, &options, true)
            .iter()
            .map(|d| {
                (
                    d.code.to_lowercase(),
                    d.location.range.start.line as u64 - 1,
                    d.location.range.start.column as u64 - 1,
                )
            })
            .collect();
    let mut lsp: Vec<(String, u64, u64)> = publication["params"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["code"]
                    .as_str()
                    .unwrap_or("dartforge_no_code")
                    .to_string(),
                d["range"]["start"]["line"].as_u64().unwrap(),
                d["range"]["start"]["character"].as_u64().unwrap(),
            )
        })
        .collect();
    cli.sort();
    lsp.sort();
    let semantic = lsp
        .iter()
        .filter(|(c, _, _)| dartforge_analysis::publicacao::verificados().contains(&c.as_str()))
        .count();
    let cli_only: Vec<_> = cli.iter().filter(|x| !lsp.contains(x)).take(5).collect();
    let so_lsp: Vec<_> = lsp.iter().filter(|x| !cli.contains(x)).take(5).collect();
    format!(
        "parity with `dartforge analyze`: analyze {} / LSP {} ({} semantic ones published); only in analyze {:?}; only in the LSP {:?}",
        cli.len(),
        lsp.len(),
        semantic,
        cli_only,
        so_lsp
    )
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn main() {
    let files: Vec<std::path::PathBuf> = std::env::args().skip(1).map(Into::into).collect();
    let base = dartforge_instrument::live_bytes();
    let mut server = Server::with_analyzer(SemanticAnalyzer::discover());
    server.receive(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}));
    server.pump();
    for file in &files {
        let file = std::fs::canonicalize(file).expect("file exists");
        let text = std::fs::read_to_string(&file).expect("text");
        let uri = url::Url::from_file_path(&file).unwrap().to_string();
        let lines = text.lines().count();

        let t = Instant::now();
        server.receive(json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": {"textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": text}},
        }));
        server.pump();
        let open_immediate = t.elapsed();
        let typed_ = wait_typed(&mut server, &uri);
        let open_typed = t.elapsed();
        let n = typed_
            .as_ref()
            .and_then(|p| p["params"]["diagnostics"].as_array().map(Vec::len));
        let parity = typed_.as_ref().map(|p| check_with_analyze(&file, p));

        let mut immediate = Vec::new();
        let mut typed = Vec::new();
        for v in 2..12 {
            let t = Instant::now();
            server.receive(edit(&uri, v));
            server.pump();
            immediate.push(t.elapsed());
            // With no change in the diagnostics, the repeated list is not published
            // (`_filesWithClientDiagnostics`): the time is measured until the worker
            // goes idle, with or without a publication.
            let _ = wait_typed(&mut server, &uri);
            typed.push(t.elapsed());
        }
        immediate.sort();
        typed.sort();

        // Burst: 20 consecutive edits, without waiting for the analysis.
        let t = Instant::now();
        let mut published = 0;
        for v in 12..32 {
            server.receive(edit(&uri, v));
            published += server
                .pump()
                .iter()
                .filter(|m| m["params"]["uri"] == uri)
                .count()
                .saturating_sub(1);
        }
        let finals = server.await_diagnostics(LIMIT);
        published += finals.iter().filter(|m| m["params"]["uri"] == uri).count();
        let burst = t.elapsed();
        let last_version = finals
            .last()
            .map(|m| m["params"]["version"].clone())
            .unwrap_or(Value::Null);

        println!(
            "{} ({} bytes, {lines} linhas)",
            file.display(),
            text.len()
        );
        println!(
            "  open: immediate {:.1} ms; typed {:.1} ms ({} diagnostics published)",
            ms(open_immediate),
            ms(open_typed),
            n.map_or("?".to_string(), |n| n.to_string())
        );
        if let Some(p) = parity {
            println!("  {p}");
        }
        println!(
            "  10 edits: immediate median {:.1} ms (max {:.1}); typed median {:.1} ms (min {:.1}, max {:.1})",
            ms(immediate[immediate.len() / 2]),
            ms(immediate[immediate.len() - 1]),
            ms(typed[typed.len() / 2]),
            ms(typed[0]),
            ms(typed[typed.len() - 1])
        );
        println!(
            "  burst of 20 edits: {published} typed publication(s), the last one of version {last_version}, {:.1} ms until it; {} stale results discarded so far",
            ms(burst),
            server.typed_discarded()
        );
        server.receive(json!({
            "jsonrpc": "2.0", "method": "textDocument/didClose",
            "params": {"textDocument": {"uri": uri}},
        }));
        server.pump();
        server.await_diagnostics(LIMIT);
    }
    println!(
        "live above the baseline with the server still open and the documents closed: {:.2} MiB",
        dartforge_instrument::live_bytes().saturating_sub(base) as f64 / (1024.0 * 1024.0)
    );
}
