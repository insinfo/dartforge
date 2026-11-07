use dartforge_lsp::Server;
use serde_json::{Value, json};

#[test]
fn symbols_update_with_text_and_utf16_columns() {
    let mut server = Server::new();
    server.receive(
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "capabilities":{"textDocument":{"documentSymbol":{
                "hierarchicalDocumentSymbolSupport":true
            }}}
        }}),
    );
    let start = server.pump();
    assert_eq!(
        start[0]["result"]["capabilities"]["documentSymbolProvider"],
        true
    );

    let uri = "file:///simbolos.dart";
    let source = "/*👭*/ class Caixa { int valor = 1; int dobro() => valor * 2; }\nvoid main() {}";
    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri,"languageId":"dart","version":2,"text":source}
        }}),
    );
    server.pump();
    server.receive(
        json!({"jsonrpc":"2.0","id":2,"method":"textDocument/documentSymbol",
        "params":{"textDocument":{"uri":uri}}}),
    );
    let response = server.pump();
    let symbols = response[0]["result"].as_array().unwrap();
    assert_eq!(symbols.len(), 2);
    assert_eq!(symbols[0]["name"], "Caixa");
    assert_eq!(symbols[0]["kind"], 5);
    assert_eq!(
        symbols[0]["selectionRange"]["start"],
        json!({"line":0,"character":13})
    );
    assert_eq!(symbols[0]["children"][0]["name"], "valor");
    assert_eq!(symbols[0]["children"][1]["name"], "dobro");
    assert_eq!(symbols[1]["name"], "main");

    // A late notification changes neither the revision nor publishes diagnostics.
    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":uri,"version":1},"contentChanges":[{"text":"void antigo() {}"}]
        }}),
    );
    assert!(server.pump().is_empty());
    server.receive(
        json!({"jsonrpc":"2.0","id":3,"method":"textDocument/documentSymbol",
        "params":{"textDocument":{"uri":uri}}}),
    );
    let response = server.pump();
    assert_eq!(response[0]["result"][0]["name"], "Caixa");

    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":uri,"version":3},"contentChanges":[{"text":"void novo() {}"}]
        }}),
    );
    server.pump();
    server.receive(
        json!({"jsonrpc":"2.0","id":4,"method":"textDocument/documentSymbol",
        "params":{"textDocument":{"uri":uri}}}),
    );
    let response = server.pump();
    assert_eq!(response[0]["result"][0]["name"], "novo");
    assert_eq!(response[0]["result"].as_array().unwrap().len(), 1);

    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{
            "textDocument":{"uri":uri}
        }}),
    );
    server.pump();
    server.receive(
        json!({"jsonrpc":"2.0","id":5,"method":"textDocument/documentSymbol",
        "params":{"textDocument":{"uri":uri}}}),
    );
    let response = server.pump();
    assert_eq!(response[0]["result"], Value::Array(Vec::new()));
}

#[test]
fn client_without_hierarchical_support_receives_flat_symbols() {
    let mut server = Server::new();
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
    server.pump();
    let uri = "file:///plano.dart";
    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri,"languageId":"dart","version":1,
                "text":"class C { void m() {} }"}
        }}),
    );
    server.pump();
    server.receive(
        json!({"jsonrpc":"2.0","id":2,"method":"textDocument/documentSymbol",
        "params":{"textDocument":{"uri":uri}}}),
    );
    let response = server.pump();
    let symbols = response[0]["result"].as_array().unwrap();
    assert_eq!(symbols.len(), 2);
    assert_eq!(symbols[0]["location"]["uri"], uri);
    assert_eq!(symbols[1]["name"], "m");
    assert_eq!(symbols[1]["containerName"], "C");
    assert!(symbols[1].get("children").is_none());
}

#[test]
fn enum_includes_constants_and_members() {
    let mut server = Server::new();
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "capabilities":{"textDocument":{"documentSymbol":{
            "hierarchicalDocumentSymbolSupport":true,
            "symbolKind":{"valueSet":[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26]}
        }}}
    }}));
    server.pump();
    let uri = "file:///enum.dart";
    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri,"languageId":"dart","version":1,
                "text":"enum Cor { azul, verde; String get texto => name; }"}
        }}),
    );
    server.pump();
    server.receive(
        json!({"jsonrpc":"2.0","id":2,"method":"textDocument/documentSymbol",
        "params":{"textDocument":{"uri":uri}}}),
    );
    let response = server.pump();
    let children = response[0]["result"][0]["children"].as_array().unwrap();
    assert_eq!(children.len(), 3);
    assert_eq!(children[0]["name"], "azul");
    assert_eq!(children[0]["kind"], 22);
    assert_eq!(children[1]["name"], "verde");
    assert_eq!(children[2]["name"], "texto");
}

#[test]
fn workspace_search_uses_only_open_revisions_and_stable_order() {
    // File URIs need a drive on Windows; these documents stay in memory.
    let root = std::env::temp_dir().join(format!("dartforge-open-symbols-{}", std::process::id()));
    let a = url::Url::from_file_path(root.join("a.dart"))
        .unwrap()
        .to_string();
    let z = url::Url::from_file_path(root.join("z.dart"))
        .unwrap()
        .to_string();
    let mut server = Server::new();
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
    let start = server.pump();
    assert_eq!(
        start[0]["result"]["capabilities"]["workspaceSymbolProvider"],
        true
    );
    for (uri, text) in [
        (z.as_str(), "class Caixa { void carregar() {} }"),
        (a.as_str(), "void carregarDados() {}"),
    ] {
        server.receive(
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
                "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":text}
            }}),
        );
        server.pump();
    }
    let search = |id: i32| {
        json!({"jsonrpc":"2.0","id":id,"method":"workspace/symbol",
        "params":{"query":"ZZZ"}})
    };
    // Query without results and case-insensitive.
    // Only those of the open documents (the SDK, when found, comes after).
    let open = |r: &serde_json::Value| -> Vec<serde_json::Value> {
        r["result"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| {
                s["location"]["uri"]
                    .as_str()
                    .is_some_and(|uri| uri == a || uri == z)
            })
            .cloned()
            .collect()
    };
    server.receive(search(2));
    assert!(open(&server.pump()[0]).is_empty());
    server.receive(json!({"jsonrpc":"2.0","id":3,"method":"workspace/symbol",
        "params":{"query":"CAR"}}));
    let response = server.pump();
    let items = open(&response[0]);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["name"], "carregarDados()");
    assert_eq!(items[0]["location"]["uri"], a);
    assert_eq!(items[1]["name"], "carregar()");
    assert_eq!(items[1]["containerName"], "Caixa");

    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{
            "textDocument":{"uri":a}
        }}),
    );
    server.pump();
    server.receive(json!({"jsonrpc":"2.0","id":4,"method":"workspace/symbol",
        "params":{"query":"car"}}));
    let response = server.pump();
    assert_eq!(open(&response[0]).len(), 1);
}

#[test]
fn symbol_details_use_ast_parameters_and_follow_edits() {
    let source = "/* 🧭 */ class Box<T> {\r\n  Box();\r\n  Box.named(int value, {required String title});\r\n  int get getter => 1;\r\n  set setter(int value) {}\r\n  void method<U>(int first, [String second = 'x)y']) {}\r\n}\r\nvoid outside<T>(\r\n  List<T> /* comment */ values, {\r\n  String text = \"a(b)\", required int count,\r\n}) { void local(int value) {} }\r\n";
    let uri = "file:///symbol_details.dart";
    let mut server = Server::new();
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "capabilities":{"textDocument":{"documentSymbol":{"hierarchicalDocumentSymbolSupport":true}}}
    }}));
    server.pump();
    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":source}
        }}),
    );
    server.pump();
    for (id, text) in [
        (2, source.to_string()),
        (3, source.replace("'x)y'", "'updated)'")),
    ] {
        if id == 3 {
            server.receive(
                json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
                    "textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":text}]
                }}),
            );
            server.pump();
        }
        server.receive(json!({"jsonrpc":"2.0","id":id,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":uri}}}));
        let response = server.pump();
        let symbols = response[0]["result"].as_array().unwrap();
        let members = symbols[0]["children"].as_array().unwrap();
        assert!(symbols[0].get("detail").is_none());
        assert_eq!(members[0]["detail"], "()");
        assert_eq!(members[1]["detail"], "(int value, {required String title})");
        assert!(members[2].get("detail").is_none());
        assert_eq!(members[3]["detail"], "(int value)");
        assert_eq!(
            members[4]["detail"],
            if id == 2 {
                "(int first, [String second = 'x)y'])"
            } else {
                "(int first, [String second = 'updated)'])"
            }
        );
        assert_eq!(
            symbols[1]["detail"],
            "(List<T> values, {String text = \"a(b)\", required int count})"
        );
        assert_eq!(symbols[1]["children"][0]["detail"], "(int value)");
    }
}
