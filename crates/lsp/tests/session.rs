//! Sessão semântica limitada (`crates/lsp/src/session.rs`) pelo protocolo:
//! consultas sobre a mesma versão reaproveitam o programa carregado; uma
//! edição, um arquivo do disco alterado ou criado o descartam; o orçamento
//! zero nunca retém. A memória é medida à parte, em `session_memory.rs`.

mod common;

use common::Project;
use dartforge_elements::sdk::SdkLayout;
use dartforge_lsp::{SemanticAnalyzer, SessionStats, Server};
use serde_json::{Value, json};

fn est(p: &Project) -> SessionStats {
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
    assert_eq!(est(&p), SessionStats::default());
    // Biblioteca: carrega uma vez, reaproveita na definição.
    assert_eq!(
        hover(&mut p, "lib/a.dart", 1, 11),
        json!({"contents": "int f()\nType: int Function()", "range": {"start": {"line": 1, "character": 11}, "end": {"line": 1, "character": 12}}})
    );
    p.in_position("textDocument/definition", "lib/a.dart", 1, 11, json!({}));
    let e = est(&p);
    assert_eq!((e.loaded, e.reused), (1, 1), "{e:?}");
    // Projeto inteiro (outro escopo): carrega; prepareRename, rename, hover
    // e codeAction na mesma versão reaproveitam.
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
    let e = est(&p);
    assert_eq!((e.loaded, e.reused), (2, 5), "{e:?}");
    assert!(e.retained_source > 0);
    // A edição descarta na hora; a consulta seguinte vê o texto novo.
    let before = est(&p).invalidated;
    p.change("lib/a.dart", 2, "double f() => 1;\nint g() => 2;\n");
    let e = est(&p);
    assert_eq!((e.invalidated, e.retained_source), (before + 1, 0), "{e:?}");
    assert_eq!(hover(&mut p, "lib/a.dart", 0, 7)["contents"], "double f()");
    assert_eq!(est(&p).loaded, 3);
}

#[test]
fn changed_disk_or_new_file_reloads() {
    let mut p = Project::new("sessao-disco");
    let a = "int f() => 1;\n";
    p.write("lib/b.dart", "import 'a.dart';\nint h() => f();\n");
    p.open("lib/a.dart", a);
    // A ordem do servidor do Dart: os outros arquivos, e o da declaração
    // (com ela por último) no fim.
    assert_eq!(
        references(&mut p, "lib/a.dart", 0, 4),
        vec!["lib/b.dart:1:11", "lib/a.dart:0:4"]
    );
    // O mesmo pedido reaproveita.
    references(&mut p, "lib/a.dart", 0, 4);
    assert_eq!(est(&p).reused, 1);
    // Arquivo fechado alterado no disco (outra ferramenta, `git checkout`).
    p.write("lib/b.dart", "import 'a.dart';\nint h() => f() + f();\n");
    assert_eq!(
        references(&mut p, "lib/a.dart", 0, 4),
        vec!["lib/b.dart:1:11", "lib/b.dart:1:17", "lib/a.dart:0:4"]
    );
    // Arquivo novo no projeto.
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
    let e = est(&p);
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
    let e = est(&p);
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

/// Abrir no editor um arquivo com o mesmo texto do disco não derruba o
/// programa retido; fechar um arquivo cujo texto aberto é o do disco
/// também não. Abrir com outro texto derruba.
#[test]
fn open_and_close_with_disk_text_keeps_session() {
    let mut p = Project::new("sessao-abrir");
    let b = "import 'a.dart';\nint h() => f();\n";
    p.write("lib/b.dart", b);
    p.open("lib/a.dart", "int f() => 1;\n");
    assert_eq!(references(&mut p, "lib/a.dart", 0, 4), vec!["lib/b.dart:1:11", "lib/a.dart:0:4"]);
    let loaded = est(&p).loaded;
    // `b.dart` aberto com o texto do disco (sem regravar): a entrada fica.
    let uri_b = p.uri("lib/b.dart");
    p.server.receive(json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": uri_b, "languageId": "dart", "version": 1, "text": b}}}));
    p.server.pump();
    assert_eq!(references(&mut p, "lib/a.dart", 0, 4), vec!["lib/b.dart:1:11", "lib/a.dart:0:4"]);
    assert_eq!(est(&p).loaded, loaded, "{:?}", est(&p));
    // Fechado sem mudança: continua.
    let uri = p.uri("lib/b.dart");
    p.server.receive(json!({"jsonrpc": "2.0", "method": "textDocument/didClose", "params": {"textDocument": {"uri": uri}}}));
    p.server.pump();
    assert_eq!(references(&mut p, "lib/a.dart", 0, 4), vec!["lib/b.dart:1:11", "lib/a.dart:0:4"]);
    assert_eq!(est(&p).loaded, loaded, "{:?}", est(&p));
    // Aberto com outro texto (um uso a mais): recarrega e vê o uso.
    p.server.receive(json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": uri_b, "languageId": "dart", "version": 1, "text": "import 'a.dart';\nint h() => f() + f();\n"}}}));
    p.server.pump();
    assert_eq!(references(&mut p, "lib/a.dart", 0, 4), vec!["lib/b.dart:1:11", "lib/b.dart:1:17", "lib/a.dart:0:4"]);
    assert!(est(&p).loaded > loaded);
}
