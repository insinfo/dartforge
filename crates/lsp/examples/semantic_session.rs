//! Measures latency and memory of semantic queries with and without the
//! bounded session (`crates/lsp/src/session.rs`), together, through the
//! protocol.
//!
//! `cargo run -q --release -p dartforge-lsp --example semantic_session -- <file.dart> [queries]`
//!
//! With the SDK from `SdkLayout::discover`, opens the file (the project is the
//! directory with `pubspec.yaml`) and requests, at the first `queries`
//! identifier positions (default 20), `hover` and `definition` (library
//! scope), and three `references` (whole project). Measures each request,
//! then applies an edit and measures the first query again. Runs twice:
//! session off (budget 0) and on (default). Prints, for each mode, the median
//! and maximum latency, the live memory retained during the queries (counting
//! allocator) and the live memory after the edit.
#[global_allocator]
static ALLOCATOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_elements::sdk::SdkLayout;
use dartforge_lsp::{SemanticAnalyzer, Server};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

fn median(v: &mut [Duration]) -> Duration {
    v.sort();
    v.get(v.len() / 2).copied().unwrap_or_default()
}

fn ms(d: Duration) -> String {
    format!("{:.1} ms", d.as_secs_f64() * 1000.0)
}

fn mib(b: usize) -> String {
    format!("{:.1} MiB", b as f64 / (1024.0 * 1024.0))
}

/// Positions (line, UTF-16 column) of the first identifiers in the text.
fn positions(text: &str, n: usize) -> Vec<(u32, u32)> {
    let Ok(tokens) = dartforge_frontend::lexer::lex(text) else {
        return Vec::new();
    };
    let table = dartforge_lsp::utf16::LineTable::build(text);
    tokens
        .iter()
        .filter(|t| t.kind == dartforge_frontend::token::Kind::Ident)
        .take(n)
        .map(|t| table.offset_position(text, t.span.start))
        .collect()
}

struct Measured {
    hover: Vec<Duration>,
    definition: Vec<Duration>,
    references: Vec<Duration>,
    first_after_edit: Duration,
    retained: usize,
    after_edit: usize,
    reused: u64,
    loaded: u64,
    retained_source: usize,
}

fn measure(sdk: SdkLayout, uri: &str, text: &str, n: usize, budget: Option<usize>) -> Measured {
    let analyzer = SemanticAnalyzer::new(Some(sdk));
    let analyzer = match budget {
        Some(mib) => analyzer.with_session_budget(mib),
        None => analyzer,
    };
    let mut server = Server::with_analyzer(analyzer);
    let mut id = 0;
    let mut request = |server: &mut Server<SemanticAnalyzer>,
                     method: &str,
                     (l, c): (u32, u32),
                     extra: Value| {
        id += 1;
        let mut params =
            json!({"textDocument": {"uri": uri}, "position": {"line": l, "character": c}});
        if let Value::Object(m) = extra {
            for (k, v) in m {
                params[k] = v;
            }
        }
        server.receive(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let start = Instant::now();
        server.pump();
        start.elapsed()
    };
    server.receive(
        json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"capabilities": {}}}),
    );
    server.pump();
    server.receive(
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
        "textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": text}}}),
    );
    server.pump();
    server.await_diagnostics(Duration::from_secs(600));
    let base = dartforge_instrument::live_bytes();
    let pos = positions(text, n);
    let mut m = Measured {
        hover: Vec::new(),
        definition: Vec::new(),
        references: Vec::new(),
        first_after_edit: Duration::ZERO,
        retained: 0,
        after_edit: 0,
        reused: 0,
        loaded: 0,
        retained_source: 0,
    };
    for &p in &pos {
        m.hover
            .push(request(&mut server, "textDocument/hover", p, json!({})));
        m.definition.push(request(
            &mut server,
            "textDocument/definition",
            p,
            json!({}),
        ));
        m.retained = m
            .retained
            .max(dartforge_instrument::live_bytes().saturating_sub(base));
    }
    for &p in pos.iter().take(3) {
        let extra = json!({"context": {"includeDeclaration": true}});
        m.references
            .push(request(&mut server, "textDocument/references", p, extra));
        m.retained = m
            .retained
            .max(dartforge_instrument::live_bytes().saturating_sub(base));
    }
    m.retained_source = server.analyzer().session_stats().retained_source;
    server.receive(json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {
        "textDocument": {"uri": uri, "version": 2},
        "contentChanges": [{"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}, "text": " "}]}}));
    server.pump();
    server.await_diagnostics(Duration::from_secs(600));
    m.after_edit = dartforge_instrument::live_bytes().saturating_sub(base);
    if let Some(&(l, c)) = pos.first() {
        m.first_after_edit = request(
            &mut server,
            "textDocument/hover",
            (l, c + u32::from(l == 0)),
            json!({}),
        );
    }
    let e = server.analyzer().session_stats();
    m.reused = e.reused;
    m.loaded = e.loaded;
    m
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(file) = args.next() else {
        eprintln!("usage: semantic_session <file.dart> [queries]");
        std::process::exit(2);
    };
    let n: usize = args.next().and_then(|v| v.parse().ok()).unwrap_or(20);
    let path = std::fs::canonicalize(&file).expect("file");
    let text = std::fs::read_to_string(&path).expect("read");
    let uri = url::Url::from_file_path(&path).unwrap().to_string();
    let lib = SdkLayout::discover().expect("Dart SDK not found");
    let load = || SdkLayout::load(&lib, "dartdevc").expect("SDK");
    println!(
        "file: {} ({} bytes), {n} positions",
        path.display(),
        text.len()
    );
    println!(
        "| mode | hover (median; max) | definition (median; max) | references (median; max) | 1st query after an edit | retained source | live retained during queries | live after the edit | loads / reuses |"
    );
    println!("|---|---|---|---|---|---|---|---|---|");
    for (name, budget) in [
        ("no session (budget 0)", Some(0)),
        ("with session (default)", None),
    ] {
        let mut m = measure(load(), &uri, &text, n, budget);
        let (hm, hx) = (
            median(&mut m.hover),
            m.hover.iter().max().copied().unwrap_or_default(),
        );
        let (dm, dx) = (
            median(&mut m.definition),
            m.definition.iter().max().copied().unwrap_or_default(),
        );
        let (rm, rx) = (
            median(&mut m.references),
            m.references.iter().max().copied().unwrap_or_default(),
        );
        println!(
            "| {name} | {}; {} | {}; {} | {}; {} | {} | {} | {} | {} | {} / {} |",
            ms(hm),
            ms(hx),
            ms(dm),
            ms(dx),
            ms(rm),
            ms(rx),
            ms(m.first_after_edit),
            mib(m.retained_source),
            mib(m.retained),
            mib(m.after_edit),
            m.loaded,
            m.reused
        );
    }
}
