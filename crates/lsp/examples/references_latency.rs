//! Measures the latency of `references` and `implementation` through the
//! protocol (`Server` in process, real SDK), as the oracle asks: all `lib/`
//! and `bin/` files of the project open, requests on identifiers spread over
//! one file, with the text idle and right after a body edit in another file.
//!
//! `cargo run -q --release -p dartforge-lsp --example references_latency -- <project> <file.dart> [positions]`
//!
//! Prints the first request (which pays the wide load), and the median and
//! maximum of the others.
use dartforge_elements::sdk::SdkLayout;
use dartforge_lsp::{SemanticAnalyzer, Server};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn summary(v: &[Duration]) -> String {
    let mut v = v.to_vec();
    v.sort();
    let m = v.get(v.len() / 2).copied().unwrap_or_default();
    let x = v.last().copied().unwrap_or_default();
    format!("{:.1} ms; {:.1} ms (n={})", m.as_secs_f64() * 1000.0, x.as_secs_f64() * 1000.0, v.len())
}

fn dart_in(d: &Path, output: &mut Vec<PathBuf>) {
    let Ok(r) = std::fs::read_dir(d) else { return };
    for e in r.flatten() {
        let p = e.path();
        if p.is_dir() {
            dart_in(&p, output);
        } else if p.extension().is_some_and(|x| x == "dart") {
            output.push(p);
        }
    }
}

fn uri_of(p: &Path) -> String {
    url::Url::from_file_path(dartforge_elements::config::sem_verbatim(p.to_path_buf())).unwrap().to_string()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(project), Some(file)) = (args.next(), args.next()) else {
        eprintln!("usage: references_latency <project> <file.dart> [positions]");
        std::process::exit(2);
    };
    let n: usize = args.next().and_then(|v| v.parse().ok()).unwrap_or(20);
    let root = std::fs::canonicalize(&project).expect("project");
    let path = std::fs::canonicalize(root.join(&file)).expect("file");
    let mut files = Vec::new();
    dart_in(&root.join("lib"), &mut files);
    dart_in(&root.join("bin"), &mut files);
    files.sort();
    let lib = SdkLayout::discover().expect("Dart SDK not found");
    let sdk = SdkLayout::load_como_analyzer(&lib).expect("SDK");
    let mut server = Server::with_analyzer(SemanticAnalyzer::new(Some(sdk)));
    let mut id = 0;
    let mut request = |server: &mut Server<SemanticAnalyzer>, method: &str, params: Value| {
        id += 1;
        server.receive(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let start = Instant::now();
        server.pump();
        start.elapsed()
    };
    server.receive(json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {
        "rootUri": uri_of(&root), "capabilities": {}}}));
    server.pump();
    for a in &files {
        let t = std::fs::read_to_string(a).expect("read");
        server.receive(json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
            "textDocument": {"uri": uri_of(a), "languageId": "dart", "version": 1, "text": t}}}));
    }
    server.pump();
    server.await_diagnostics(Duration::from_secs(600));

    let text = std::fs::read_to_string(&path).expect("read");
    let uri = uri_of(&path);
    let table = dartforge_lsp::utf16::LineTable::build(&text);
    let tokens = dartforge_frontend::lexer::lex(&text).unwrap_or_default();
    let idents: Vec<usize> =
        tokens.iter().filter(|t| t.kind == dartforge_frontend::token::Kind::Ident).map(|t| t.span.start).collect();
    let step = (idents.len() / n.max(1)).max(1);
    let sample: Vec<usize> = idents.iter().step_by(step).take(n).copied().collect();
    let pos = |o: usize| {
        let (l, c) = table.offset_position(&text, o);
        json!({"line": l, "character": c})
    };
    let doc = json!({"uri": uri});
    let (mut refs, mut imps) = (Vec::new(), Vec::new());
    for &a in &sample {
        refs.push(request(
            &mut server,
            "textDocument/references",
            json!({"textDocument": doc, "position": pos(a + 1), "context": {"includeDeclaration": true}}),
        ));
        imps.push(request(&mut server, "textDocument/implementation", json!({"textDocument": doc, "position": pos(a + 1)})));
    }
    // A body edit (a space at the end) in another file, followed by
    // `references`: what survives the edit.
    let other = files.iter().find(|a| **a != path).cloned().unwrap_or(path.clone());
    let other_text = std::fs::read_to_string(&other).expect("read");
    let last = other_text.lines().count() as u32;
    let mut after_edit = Vec::new();
    for v in 0..5u32 {
        server.receive(json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {
            "textDocument": {"uri": uri_of(&other), "version": 2 + v},
            "contentChanges": [{"range": {"start": {"line": last, "character": 0}, "end": {"line": last, "character": 0}}, "text": " "}]}}));
        server.pump();
        let a = sample[v as usize % sample.len().max(1)];
        after_edit.push(request(
            &mut server,
            "textDocument/references",
            json!({"textDocument": doc, "position": pos(a + 1), "context": {"includeDeclaration": true}}),
        ));
    }
    server.await_diagnostics(Duration::from_secs(600));
    println!("project: {} ({} files); file: {}", root.display(), files.len(), path.display());
    println!("| request | first | median; maximum of the others |");
    println!("|---|---|---|");
    println!("| references | {:.1} ms | {} |", refs[0].as_secs_f64() * 1000.0, summary(&refs[1..]));
    println!("| implementation | {:.1} ms | {} |", imps[0].as_secs_f64() * 1000.0, summary(&imps[1..]));
    println!("| references after editing another file | {:.1} ms | {} |", after_edit[0].as_secs_f64() * 1000.0, summary(&after_edit[1..]));
}
