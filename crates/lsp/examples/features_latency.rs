//! Measures the latency of editing features through the protocol (`Server`
//! in process, real SDK): `hover`, `completion`, `signatureHelp`,
//! `documentHighlight` and `foldingRange`, with the text idle and right after
//! each edit (the typing case: the semantic session is discarded).
//!
//! `cargo run -q --release -p dartforge-lsp --example features_latency -- <file.dart> [positions]`
//!
//! For each of the first `positions` (default 20) identifier positions:
//! `hover`; `completion` with a prefix of up to two characters of the
//! identifier; `signatureHelp` after each call `(`; `documentHighlight`.
//! Then, 10 times: an incremental edit (a space at the end of the file)
//! followed by `hover` and `completion` (the first query pays the library
//! load). Prints median and maximum.
use dartforge_elements::sdk::SdkLayout;
use dartforge_lsp::{SemanticAnalyzer, Server};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

fn summary(v: &mut [Duration]) -> String {
    v.sort();
    let m = v.get(v.len() / 2).copied().unwrap_or_default();
    let x = v.last().copied().unwrap_or_default();
    format!("{:.1} ms; {:.1} ms (n={})", m.as_secs_f64() * 1000.0, x.as_secs_f64() * 1000.0, v.len())
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(file) = args.next() else {
        eprintln!("usage: features_latency <file.dart> [positions]");
        std::process::exit(2);
    };
    let n: usize = args.next().and_then(|v| v.parse().ok()).unwrap_or(20);
    let path = std::fs::canonicalize(&file).expect("file");
    let text = std::fs::read_to_string(&path).expect("read");
    let uri = url::Url::from_file_path(dartforge_elements::config::sem_verbatim(path.clone())).unwrap().to_string();
    let lib = SdkLayout::discover().expect("Dart SDK not found");
    let sdk = SdkLayout::load(&lib, "dartdevc").expect("SDK");
    let mut server = Server::with_analyzer(SemanticAnalyzer::new(Some(sdk)));
    let mut id = 0;
    let mut request = |server: &mut Server<SemanticAnalyzer>, method: &str, params: Value| {
        id += 1;
        server.receive(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let start = Instant::now();
        server.pump();
        start.elapsed()
    };
    server.receive(json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"capabilities": {
        "textDocument": {"completion": {"completionItem": {"snippetSupport": true}}, "foldingRange": {"lineFoldingOnly": true}}}}}));
    server.pump();
    server.receive(json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
        "textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": text}}}));
    server.pump();
    server.await_diagnostics(Duration::from_secs(600));

    let table = dartforge_lsp::utf16::LineTable::build(&text);
    let tokens = dartforge_frontend::lexer::lex(&text).unwrap_or_default();
    let idents: Vec<(usize, usize)> = tokens
        .iter()
        .filter(|t| t.kind == dartforge_frontend::token::Kind::Ident)
        .map(|t| (t.span.start, t.span.end))
        .collect();
    let step = (idents.len() / n.max(1)).max(1);
    let sample: Vec<(usize, usize)> = idents.iter().step_by(step).take(n).copied().collect();
    let pos = |o: usize| {
        let (l, c) = table.offset_position(&text, o);
        json!({"line": l, "character": c})
    };
    let doc = json!({"uri": uri});
    let (mut hover, mut completion, mut signature, mut highlight) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for &(a, b) in &sample {
        hover.push(request(&mut server, "textDocument/hover", json!({"textDocument": doc, "position": pos(a + 1)})));
        completion.push(request(
            &mut server,
            "textDocument/completion",
            json!({"textDocument": doc, "position": pos(a + (b - a).min(2))}),
        ));
        highlight.push(request(&mut server, "textDocument/documentHighlight", json!({"textDocument": doc, "position": pos(a + 1)})));
    }
    for (i, _) in text.match_indices('(').filter(|(i, _)| *i > 0 && text.as_bytes()[i - 1].is_ascii_alphanumeric()).take(n) {
        signature.push(request(
            &mut server,
            "textDocument/signatureHelp",
            json!({"textDocument": doc, "position": pos(i + 1), "context": {"triggerKind": 2, "triggerCharacter": "(", "isRetrigger": false}}),
        ));
    }
    let mut folds = vec![request(&mut server, "textDocument/foldingRange", json!({"textDocument": doc}))];
    let (mut hover_ed, mut completion_ed) = (Vec::new(), Vec::new());
    let last = table.total_lines().saturating_sub(1) as u32;
    for v in 0..10 {
        server.receive(json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {
            "textDocument": {"uri": uri, "version": 2 + v},
            "contentChanges": [{"range": {"start": {"line": last, "character": 0}, "end": {"line": last, "character": 0}}, "text": " "}]}}));
        server.pump();
        let (a, b) = sample[v as usize % sample.len().max(1)];
        hover_ed.push(request(&mut server, "textDocument/hover", json!({"textDocument": doc, "position": pos(a + 1)})));
        server.receive(json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {
            "textDocument": {"uri": uri, "version": 100 + v},
            "contentChanges": [{"range": {"start": {"line": last, "character": 0}, "end": {"line": last, "character": 0}}, "text": " "}]}}));
        server.pump();
        completion_ed.push(request(&mut server, "textDocument/completion", json!({"textDocument": doc, "position": pos(a + (b - a).min(2))})));
        folds.push(request(&mut server, "textDocument/foldingRange", json!({"textDocument": doc})));
    }
    // Edits inside a body (a space before the last `return`): `didChange` swaps
    // the retained unit and re-infers its bodies (§16.10 I4); `didChange` and
    // the following `hover` are measured.
    let (mut swap, mut hover_body) = (Vec::new(), Vec::new());
    if let Some(r) = text.rfind("return ") {
        let (l, c) = table.offset_position(&text, r);
        for v in 0..10 {
            server.receive(json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {
                "textDocument": {"uri": uri, "version": 1000 + v},
                "contentChanges": [{"range": {"start": {"line": l, "character": c}, "end": {"line": l, "character": c}}, "text": " "}]}}));
            let start = Instant::now();
            server.pump();
            swap.push(start.elapsed());
            let (a, _) = sample[v as usize % sample.len().max(1)];
            hover_body.push(request(&mut server, "textDocument/hover", json!({"textDocument": doc, "position": pos(a + 1)})));
        }
    }
    server.await_diagnostics(Duration::from_secs(600));
    println!("file: {} ({} bytes)", path.display(), text.len());
    println!("| feature | median; maximum |");
    println!("|---|---|");
    println!("| hover (idle text) | {} |", summary(&mut hover));
    println!("| completion (idle text) | {} |", summary(&mut completion));
    println!("| signatureHelp at `(` | {} |", summary(&mut signature));
    println!("| documentHighlight | {} |", summary(&mut highlight));
    println!("| foldingRange | {} |", summary(&mut folds));
    println!("| hover right after an edit | {} |", summary(&mut hover_ed));
    println!("| completion right after an edit | {} |", summary(&mut completion_ed));
    println!("| didChange inside a body | {} |", summary(&mut swap));
    println!("| hover after the body edit | {} |", summary(&mut hover_body));
}
