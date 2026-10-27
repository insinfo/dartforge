//! Formatting at scale and off the dispatcher.
//!
//! * `long_chains_format_in_linear_time`: the edits of a document whose
//!   formatting splits a single 40 000-term (debug) or 50 000-term (release)
//!   binary chain over thousands of lines are computed within a time bound,
//!   and applying them gives exactly the formatter's output. Each edit used
//!   to be converted to a position by scanning its line from the start, which
//!   was quadratic on a one-line document (80 000 terms: 13 s).
//! * `slow_formatting_is_answered_later`, `cancelled_formatting_answers_at_once`
//!   and `formatting_of_a_changed_document_is_content_modified`: with a wake
//!   hook, a formatting that exceeds the budget is answered after the
//!   requests that came after it, can be cancelled, and reports
//!   `ContentModified` when the document changed meanwhile.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use dartforge_lsp::Server;
use serde_json::{Value, json};

/// Runs `f` on a thread with the server's dispatcher stack: the parser and
/// the formatter recurse over the 50 000-level binary tree.
fn with_large_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(f)
        .expect("thread")
        .join()
        .expect("no panic")
}

/// Applies LSP edits (sorted, non-overlapping, ASCII text) to `text`.
fn apply(text: &str, edits: &Value) -> String {
    let starts: Vec<usize> = std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    let offset = |p: &Value| {
        starts[p["line"].as_u64().unwrap() as usize] + p["character"].as_u64().unwrap() as usize
    };
    let mut out = String::new();
    let mut at = 0;
    for e in edits.as_array().expect("edits") {
        let (s, t) = (offset(&e["range"]["start"]), offset(&e["range"]["end"]));
        out.push_str(&text[at..s]);
        out.push_str(e["newText"].as_str().unwrap());
        at = t;
    }
    out.push_str(&text[at..]);
    out
}

#[test]
fn long_chains_format_in_linear_time() {
    // Generous bounds: the quadratic conversion took 0.8 s (release) for
    // 20 000 terms and grew fourfold per doubling (about 5 s for 50 000);
    // now 50 000 terms take about 0.2 s in release.
    let (terms, limit) = if cfg!(debug_assertions) {
        (40_000, Duration::from_secs(5))
    } else {
        (50_000, Duration::from_secs(2))
    };
    with_large_stack(move || {
        for term in ["1", "1 * 2"] {
            let text = format!(
                "void main() {{ var x = {}; }}\n",
                vec![term; terms].join(" + ")
            );
            let started = Instant::now();
            let edits = dartforge_lsp::formatting::edits(&text, None, None);
            let elapsed = started.elapsed();
            eprintln!("{terms} terms `{term}`: {elapsed:?}");
            assert!(
                elapsed < limit,
                "{terms} terms `{term}`: {elapsed:?} (limit {limit:?})"
            );
            let expected = dartforge_formatter::format(&text, &Default::default()).unwrap();
            assert_eq!(edits.as_array().unwrap().len(), terms + 1);
            assert!(apply(&text, &edits) == expected, "edits of `{term}` differ");
        }
    });
}

/// A server with a wake hook and the given formatting budget, with `text`
/// open as version 1 of `file:///a.dart`; the receiver gets one `()` per wake.
fn server(text: &str, budget: Duration) -> (Server, mpsc::Receiver<()>) {
    let mut s = Server::new();
    let (tx, rx) = mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    s.on_diagnostics(move || {
        let _ = tx.lock().unwrap().send(());
    });
    s.set_formatting_budget(budget);
    s.receive(json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {}}));
    s.receive(json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}));
    s.receive(json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": {"textDocument": {"uri": "file:///a.dart", "languageId": "dart", "version": 1, "text": text}},
    }));
    while s.has_pending() {
        s.pump();
    }
    (s, rx)
}

/// A document whose formatting takes a while (the line splitter explores many
/// states for an argument list with a thousand splittable arguments, as
/// `dart format` does: hundreds of milliseconds even in release).
fn slow_document() -> String {
    format!("void main() {{ f({}); }}\n", vec!["a + b"; 1500].join(", "))
}

fn formatting(id: i64) -> Value {
    json!({
        "jsonrpc": "2.0", "id": id, "method": "textDocument/formatting",
        "params": {"textDocument": {"uri": "file:///a.dart"}, "options": {"tabSize": 2, "insertSpaces": true}},
    })
}

fn find(outputs: &[Value], id: i64) -> Option<&Value> {
    outputs.iter().find(|m| m["id"] == json!(id))
}

/// Pumps after each wake until the response with `id` comes out.
fn wait_response(s: &mut Server, wakes: &mpsc::Receiver<()>, id: i64) -> Value {
    let started = Instant::now();
    loop {
        if let Some(r) = find(&s.pump(), id) {
            return r.clone();
        }
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "no response {id}"
        );
        let _ = wakes.recv_timeout(Duration::from_millis(100));
    }
}

#[test]
fn slow_formatting_is_answered_later() {
    let text = slow_document();
    let (mut s, wakes) = server(&text, Duration::ZERO);
    s.receive(formatting(1));
    s.receive(json!({"jsonrpc": "2.0", "id": 2, "method": "dartforge/sleep", "params": {"ms": 0}}));
    let started = Instant::now();
    let mut outputs = s.pump();
    outputs.extend(s.pump());
    assert!(find(&outputs, 2).is_some(), "the next request is answered");
    assert!(
        find(&outputs, 1).is_none(),
        "while the formatting still runs ({:?})",
        started.elapsed()
    );
    let r = wait_response(&mut s, &wakes, 1);
    let edits = r["result"].as_array().expect("edits");
    assert!(edits.len() > 1000, "{}", edits.len());
}

#[test]
fn cancelled_formatting_answers_at_once() {
    let (mut s, wakes) = server(&slow_document(), Duration::ZERO);
    s.receive(formatting(1));
    assert!(find(&s.pump(), 1).is_none());
    s.receive(json!({"jsonrpc": "2.0", "method": "$/cancelRequest", "params": {"id": 1}}));
    let outputs = s.pump();
    let r = find(&outputs, 1).expect("cancelled at once");
    assert_eq!(r["error"]["code"], -32800, "{r}");
    // The thread's result, when it arrives, is dropped.
    let _ = wakes.recv_timeout(Duration::from_secs(30));
    assert!(find(&s.pump(), 1).is_none(), "answered once");
}

#[test]
fn formatting_of_a_changed_document_is_content_modified() {
    let (mut s, wakes) = server(&slow_document(), Duration::ZERO);
    s.receive(formatting(1));
    s.receive(json!({
        "jsonrpc": "2.0", "method": "textDocument/didChange",
        "params": {"textDocument": {"uri": "file:///a.dart", "version": 2},
                   "contentChanges": [{"text": "main( ) {}"}]},
    }));
    s.pump();
    let r = wait_response(&mut s, &wakes, 1);
    assert_eq!(r["error"]["code"], -32801, "{r}");
}

#[test]
fn quick_formatting_is_answered_in_place() {
    let (mut s, _wakes) = server("main( ){}", Duration::from_secs(60));
    s.receive(formatting(1));
    let outputs = s.pump();
    let r = find(&outputs, 1).expect("answered within the budget");
    assert_eq!(r["result"].as_array().map(Vec::len), Some(3), "{r}");
}
