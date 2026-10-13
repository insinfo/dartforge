use dartforge_lsp::Server;
use serde_json::{Value, json};

#[test]
fn symbols_update_with_text_and_utf16_columns() {
    let mut server = Server::new();
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "capabilities":{"textDocument":{"documentSymbol":{
            "hierarchicalDocumentSymbolSupport":true
        }}}
    }}));
    let start = server.pump();
    assert_eq!(start[0]["result"]["capabilities"]["documentSymbolProvider"], true);

    let uri = "file:///simbolos.dart";
    let source = "/*👭*/ class Caixa { int valor = 1; int dobro() => valor * 2; }\nvoid main() {}";
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":2,"text":source}
    }}));
    server.pump();
    server.receive(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/documentSymbol",
        "params":{"textDocument":{"uri":uri}}}));
    let response = server.pump();
    let symbols = response[0]["result"].as_array().unwrap();
    assert_eq!(symbols.len(), 2);
    assert_eq!(symbols[0]["name"], "Caixa");
    assert_eq!(symbols[0]["kind"], 5);
    assert_eq!(symbols[0]["selectionRange"]["start"], json!({"line":0,"character":13}));
    assert_eq!(symbols[0]["children"][0]["name"], "valor");
    assert_eq!(symbols[0]["children"][1]["name"], "dobro");
    assert_eq!(symbols[1]["name"], "main");

    // A late notification changes neither the revision nor publishes diagnostics.
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":uri,"version":1},"contentChanges":[{"text":"void antigo() {}"}]
    }}));
    assert!(server.pump().is_empty());
    server.receive(json!({"jsonrpc":"2.0","id":3,"method":"textDocument/documentSymbol",
        "params":{"textDocument":{"uri":uri}}}));
    let response = server.pump();
    assert_eq!(response[0]["result"][0]["name"], "Caixa");

    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":uri,"version":3},"contentChanges":[{"text":"void novo() {}"}]
    }}));
    server.pump();
    server.receive(json!({"jsonrpc":"2.0","id":4,"method":"textDocument/documentSymbol",
        "params":{"textDocument":{"uri":uri}}}));
    let response = server.pump();
    assert_eq!(response[0]["result"][0]["name"], "novo");
    assert_eq!(response[0]["result"].as_array().unwrap().len(), 1);

    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{
        "textDocument":{"uri":uri}
    }}));
    server.pump();
    server.receive(json!({"jsonrpc":"2.0","id":5,"method":"textDocument/documentSymbol",
        "params":{"textDocument":{"uri":uri}}}));
    let response = server.pump();
    assert_eq!(response[0]["result"], Value::Array(Vec::new()));
}

#[test]
fn client_without_hierarchical_support_receives_flat_symbols() {
    let mut server = Server::new();
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
    server.pump();
    let uri = "file:///plano.dart";
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"class C { void m() {} }"}
    }}));
    server.pump();
    server.receive(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/documentSymbol",
        "params":{"textDocument":{"uri":uri}}}));
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
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"enum Cor { azul, verde; String get texto => name; }"}
    }}));
    server.pump();
    server.receive(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/documentSymbol",
        "params":{"textDocument":{"uri":uri}}}));
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
    let mut server = Server::new();
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
    let start = server.pump();
    assert_eq!(start[0]["result"]["capabilities"]["workspaceSymbolProvider"], true);
    for (uri, text) in [
        ("file:///z.dart", "class Caixa { void carregar() {} }"),
        ("file:///a.dart", "void carregarDados() {}"),
    ] {
        server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":text}
        }}));
        server.pump();
    }
    let search = |id: i32| json!({"jsonrpc":"2.0","id":id,"method":"workspace/symbol",
        "params":{"query":"ZZZ"}});
    // Query without results and case-insensitive.
    // Only those of the open documents (the SDK, when found, comes after).
    let open = |r: &serde_json::Value| -> Vec<serde_json::Value> {
        r["result"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| matches!(s["location"]["uri"].as_str(), Some("file:///a.dart" | "file:///z.dart")))
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
    assert_eq!(items[0]["location"]["uri"], "file:///a.dart");
    assert_eq!(items[1]["name"], "carregar()");
    assert_eq!(items[1]["containerName"], "Caixa");

    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{
        "textDocument":{"uri":"file:///a.dart"}
    }}));
    server.pump();
    server.receive(json!({"jsonrpc":"2.0","id":4,"method":"workspace/symbol",
        "params":{"query":"car"}}));
    let response = server.pump();
    assert_eq!(open(&response[0]).len(), 1);
}
