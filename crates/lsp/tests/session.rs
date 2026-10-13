//! Bounded semantic session (`crates/lsp/src/session.rs`) through the
//! protocol: queries on the same version reuse the loaded program; an edit,
//! or a file changed or created on disk, discards it; a zero budget never
//! retains. Memory is measured separately, in `session_memory.rs`.

mod common;

use common::Project;
use dartforge_elements::sdk::SdkLayout;
use dartforge_lsp::{SemanticAnalyzer, SessionStats, Server};
use serde_json::{Value, json};

fn stats(p: &Project) -> SessionStats {
    p.server.analyzer().session_stats()
}

fn hover(p: &mut Project, rel: &str, l: u32, c: u32) -> Value {
    p.in_position("textDocument/hover", rel, l, c, json!({}))["result"].clone()
}

fn references(p: &mut Project, rel: &str, l: u32, c: u32) -> Vec<String> {
    let r = p.in_position(
        "textDocument/references",
        rel,
        l,
        c,
        json!({"context": {"includeDeclaration": true}}),
    );
    let base = format!("{}/", p.uri(""));
    r["result"]
        .as_array()
        .unwrap_or_else(|| panic!("{r}"))
        .iter()
        .map(|l| {
            format!(
                "{}:{}:{}",
                l["uri"].as_str().unwrap().trim_start_matches(&base),
                l["range"]["start"]["line"],
                l["range"]["start"]["character"]
            )
        })
        .collect()
}

#[test]
fn reused_across_queries_and_dropped_on_edit() {
    let mut p = Project::new("sessao-reuso");
    let a = "int f() => 1;\nint g() => f() + f();\n";
    p.write("lib/b.dart", "import 'a.dart';\nint h() => f();\n");
    p.open("lib/a.dart", a);
    assert_eq!(stats(&p), SessionStats::default());
    // Library: loads once, reuses it for the definition.
    assert_eq!(
        hover(&mut p, "lib/a.dart", 1, 11),
        json!({"contents": "int f()\nType: int Function()", "range": {"start": {"line": 1, "character": 11}, "end": {"line": 1, "character": 12}}})
    );
    p.in_position("textDocument/definition", "lib/a.dart", 1, 11, json!({}));
    let e = stats(&p);
    assert_eq!((e.loaded, e.reused), (1, 1), "{e:?}");
    // Whole project (another scope): loads; prepareRename, rename, hover and
    // codeAction on the same version reuse it.
    assert_eq!(references(&mut p, "lib/a.dart", 1, 11).len(), 4);
    p.in_position("textDocument/prepareRename", "lib/a.dart", 1, 11, json!({}));
    p.in_position(
        "textDocument/rename",
        "lib/a.dart",
        1,
        11,
        json!({"newName": "um"}),
    );
    hover(&mut p, "lib/a.dart", 1, 11);
    p.request(
        "textDocument/codeAction",
        json!({"textDocument": {"uri": p.uri("lib/a.dart")}, "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 0}}, "context": {"diagnostics": []}}),
    );
    let e = stats(&p);
    assert_eq!((e.loaded, e.reused), (2, 5), "{e:?}");
    assert!(e.retained_source > 0);
    // The edit discards right away; the next query sees the new text.
    let before = stats(&p).invalidated;
    p.change("lib/a.dart", 2, "double f() => 1;\nint g() => 2;\n");
    let e = stats(&p);
    assert_eq!((e.invalidated, e.retained_source), (before + 1, 0), "{e:?}");
    assert_eq!(hover(&mut p, "lib/a.dart", 0, 7)["contents"], "double f()");
    assert_eq!(stats(&p).loaded, 3);
}

#[test]
fn changed_disk_or_new_file_reloads() {
    let mut p = Project::new("sessao-disco");
    let a = "int f() => 1;\n";
    p.write("lib/b.dart", "import 'a.dart';\nint h() => f();\n");
    p.open("lib/a.dart", a);
    // The Dart server's order: the other files, and the declaration's file
    // (with the declaration last) at the end.
    assert_eq!(
        references(&mut p, "lib/a.dart", 0, 4),
        vec!["lib/b.dart:1:11", "lib/a.dart:0:4"]
    );
    // The same request reuses it.
    references(&mut p, "lib/a.dart", 0, 4);
    assert_eq!(stats(&p).reused, 1);
    // Closed file changed on disk (another tool, `git checkout`).
    p.write("lib/b.dart", "import 'a.dart';\nint h() => f() + f();\n");
    assert_eq!(
        references(&mut p, "lib/a.dart", 0, 4),
        vec!["lib/b.dart:1:11", "lib/b.dart:1:17", "lib/a.dart:0:4"]
    );
    // New file in the project.
    p.write("lib/c.dart", "import 'a.dart';\nvar x = f();\n");
    assert_eq!(
        references(&mut p, "lib/a.dart", 0, 4),
        vec![
            "lib/b.dart:1:11",
            "lib/b.dart:1:17",
            "lib/c.dart:1:8",
            "lib/a.dart:0:4"
        ]
    );
    let e = stats(&p);
    assert_eq!((e.loaded, e.reused), (3, 1), "{e:?}");
}

#[test]
fn zero_budget_never_retains() {
    let mut p = Project::new("sessao-orcamento");
    let sdk = SdkLayout::load(&p.root.join("sdk/lib"), "dartdevc").unwrap();
    p.server =
        Server::with_analyzer(SemanticAnalyzer::new(Some(sdk)).with_session_budget(0));
    p.request("initialize", json!({"capabilities": {}}));
    p.open("lib/a.dart", "int f() => 1;\nint g() => f();\n");
    for _ in 0..3 {
        assert_eq!(hover(&mut p, "lib/a.dart", 1, 11)["contents"], "int f()\nType: int Function()");
    }
    let e = stats(&p);
    assert_eq!(
        (
            e.loaded,
            e.reused,
            e.above_budget,
            e.retained_source
        ),
        (3, 0, 3, 0),
        "{e:?}"
    );
}

/// Opening in the editor a file with the same text as on disk does not drop
/// the retained program; closing a file whose open text is the disk's does
/// not either. Opening with a different text drops it.
#[test]
fn open_and_close_with_disk_text_keeps_session() {
    let mut p = Project::new("sessao-abrir");
    let b = "import 'a.dart';\nint h() => f();\n";
    p.write("lib/b.dart", b);
    p.open("lib/a.dart", "int f() => 1;\n");
    assert_eq!(references(&mut p, "lib/a.dart", 0, 4), vec!["lib/b.dart:1:11", "lib/a.dart:0:4"]);
    let loaded = stats(&p).loaded;
    // `b.dart` opened with the disk text (without rewriting it): the entry stays.
    let uri_b = p.uri("lib/b.dart");
    p.server.receive(json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": uri_b, "languageId": "dart", "version": 1, "text": b}}}));
    p.server.pump();
    assert_eq!(references(&mut p, "lib/a.dart", 0, 4), vec!["lib/b.dart:1:11", "lib/a.dart:0:4"]);
    assert_eq!(stats(&p).loaded, loaded, "{:?}", stats(&p));
    // Closed without changes: still there.
    let uri = p.uri("lib/b.dart");
    p.server.receive(json!({"jsonrpc": "2.0", "method": "textDocument/didClose", "params": {"textDocument": {"uri": uri}}}));
    p.server.pump();
    assert_eq!(references(&mut p, "lib/a.dart", 0, 4), vec!["lib/b.dart:1:11", "lib/a.dart:0:4"]);
    assert_eq!(stats(&p).loaded, loaded, "{:?}", stats(&p));
    // Opened with a different text (one more use): reloads and sees the use.
    p.server.receive(json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": uri_b, "languageId": "dart", "version": 1, "text": "import 'a.dart';\nint h() => f() + f();\n"}}}));
    p.server.pump();
    assert_eq!(references(&mut p, "lib/a.dart", 0, 4), vec!["lib/b.dart:1:11", "lib/b.dart:1:17", "lib/a.dart:0:4"]);
    assert!(stats(&p).loaded > loaded);
}
