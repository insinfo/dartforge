use dartforge_lsp::Server;
use serde_json::{Value, json};

#[test]
fn local_type_hover_uses_markdown_and_reference_range() {
    let mut server = Server::new();
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "capabilities":{"textDocument":{"hover":{"contentFormat":["markdown","plaintext"]}}}
    }}));
    let start = server.pump();
    assert_eq!(start[0]["result"]["capabilities"]["hoverProvider"], true);
    let uri = "file:///hover.dart";
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"class Caixa {}\nCaixa item;"}
    }}));
    server.pump();
    server.receive(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":2}}}));
    let response = server.pump();
    assert_eq!(response[0]["result"]["contents"],
        json!({"kind":"markdown","value":"```dart\nclass Caixa\n```"}));
    assert_eq!(response[0]["result"]["range"]["start"],
        json!({"line":1,"character":0}));
    assert_eq!(response[0]["result"]["range"]["end"],
        json!({"line":1,"character":5}));
}

#[test]
fn plain_hover_and_refuses_generic_type_without_faithful_signature() {
    let mut server = Server::new();
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
    server.pump();
    let uri = "file:///hover-plano.dart";
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"enum Cor { azul }\nCor cor;\nclass Caixa<T> {}\nCaixa<int> caixa;"}
    }}));
    server.pump();
    server.receive(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":1}}}));
    assert_eq!(server.pump()[0]["result"]["contents"], "enum Cor");
    server.receive(json!({"jsonrpc":"2.0","id":3,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":3,"character":2}}}));
    assert_eq!(server.pump()[0]["result"], Value::Null);
}

#[test]
fn top_level_variable_hover_with_written_type() {
    let mut server = Server::new();
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}
    }}));
    server.pump();
    let uri = "file:///variavel.dart";
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"int resposta = 42;\nvoid main() { print(resposta); }"}
    }}));
    server.pump();
    server.receive(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":22}}}));
    let response = server.pump();
    assert_eq!(response[0]["result"]["contents"], json!({
        "kind":"markdown","value":"```dart\nint resposta\n```\nType: `int`"
    }));
}

#[test]
fn homonymous_local_variable_prevents_top_level_hover() {
    let mut server = Server::new();
    let uri = "file:///sombra-variavel.dart";
    let source = "int resposta = 42;\nvoid f() { int resposta = 1; print(resposta); }";
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":source}
    }}));
    server.pump();
    let column = source.lines().nth(1).unwrap().find("print(resposta)").unwrap() + 8;
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":column}}}));
    assert_eq!(server.pump()[0]["result"], Value::Null);
}

#[test]
fn top_level_function_hover_without_parameters() {
    let mut server = Server::new();
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}
    }}));
    server.pump();
    let uri = "file:///funcao.dart";
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"int resposta() => 42;\nvoid main() { print(resposta()); }"}
    }}));
    server.pump();
    server.receive(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":22}}}));
    let response = server.pump();
    assert_eq!(response[0]["result"]["contents"], json!({
        "kind":"markdown","value":"```dart\nint resposta()\n```"
    }));
}

#[test]
fn homonymous_local_function_prevents_top_level_hover() {
    let mut server = Server::new();
    let uri = "file:///sombra-funcao.dart";
    let source = "int resposta() => 42;\nvoid main() { int resposta() => 1; print(resposta()); }";
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":source}
    }}));
    server.pump();
    let column = source.lines().nth(1).unwrap().find("print(resposta())").unwrap() + 8;
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":column}}}));
    assert_eq!(server.pump()[0]["result"], Value::Null);
}

#[test]
fn function_hover_with_two_typed_positionals() {
    let mut server = Server::new();
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}
    }}));
    server.pump();
    let uri = "file:///funcao-parametros.dart";
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"int soma(int a, int b) => a + b;\nvoid main() { print(soma(1, 2)); }"}
    }}));
    server.pump();
    server.receive(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":21}}}));
    let response = server.pump();
    assert_eq!(response[0]["result"]["contents"], json!({
        "kind":"markdown","value":"```dart\nint soma(int a, int b)\n```"
    }));
}

#[test]
fn hover_refuses_optional_signature_needing_full_formatting() {
    let mut server = Server::new();
    let uri = "file:///funcao-opcional.dart";
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"int soma([int a = 1]) => a;\nvoid main() { print(soma()); }"}
    }}));
    server.pump();
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":21}}}));
    assert_eq!(server.pump()[0]["result"], Value::Null);
}

#[test]
fn typed_top_level_getter_hover_shows_signature_and_type() {
    let mut server = Server::new();
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}
    }}));
    server.pump();
    let uri = "file:///getter-topo.dart";
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"String get resposta => 'ok';\nvoid main() { print(resposta); }"}
    }}));
    server.pump();
    server.receive(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":22}}}));
    let response = server.pump();
    assert_eq!(response[0]["result"]["contents"], json!({
        "kind":"markdown","value":"```dart\nString get resposta\n```\nType: `String`"
    }));
    assert_eq!(response[0]["result"]["range"]["start"], json!({"line":1,"character":20}));
    assert_eq!(response[0]["result"]["range"]["end"], json!({"line":1,"character":28}));
}

#[test]
fn homonymous_member_getter_prevents_top_level_hover() {
    let mut server = Server::new();
    let uri = "file:///getter-sombra.dart";
    let source = "String get resposta => 'topo';\nclass A { String get resposta => 'membro'; void f() { print(resposta); } }";
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":source}
    }}));
    server.pump();
    let column = source.lines().nth(1).unwrap().find("print(resposta)").unwrap() + 8;
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":column}}}));
    assert_eq!(server.pump()[0]["result"], Value::Null);
}

fn open(server: &mut Server, uri: &str, text: &str) {
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":text}
    }}));
    server.pump();
}

fn request_hover(server: &mut Server, id: i64, uri: &str, line: u32, column: u32) -> Value {
    server.receive(json!({"jsonrpc":"2.0","id":id,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":line,"character":column}}}));
    server.pump()[0]["result"].clone()
}

fn with_markdown(server: &mut Server) {
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}
    }}));
    server.pump();
}

#[test]
fn imported_class_hover_formats_from_owner() {
    let mut server = Server::new();
    with_markdown(&mut server);
    open(&mut server, "file:///hover-dono-classe.dart", "class Caixa {}");
    let text = "import 'hover-dono-classe.dart';\nCaixa b;";
    open(&mut server, "file:///hover-usa-classe.dart", text);
    let result = request_hover(&mut server, 2, "file:///hover-usa-classe.dart", 1, 2);
    assert_eq!(result["contents"],
        json!({"kind":"markdown","value":"```dart\nclass Caixa\n```"}));
    assert_eq!(result["range"]["start"], json!({"line":1,"character":0}));
    assert_eq!(result["range"]["end"], json!({"line":1,"character":5}));
}

#[test]
fn imported_variable_hover_shows_owner_type() {
    let mut server = Server::new();
    with_markdown(&mut server);
    open(&mut server, "file:///hover-dono-var.dart", "int resposta = 42;");
    let text = "import 'hover-dono-var.dart';\nvoid f() { print(resposta); }";
    open(&mut server, "file:///hover-usa-var.dart", text);
    let line = text.lines().nth(1).unwrap();
    let column = line.find("resposta").unwrap() as u32;
    let result = request_hover(&mut server, 2, "file:///hover-usa-var.dart", 1, column);
    assert_eq!(result["contents"], json!({
        "kind":"markdown","value":"```dart\nint resposta\n```\nType: `int`"
    }));
    assert_eq!(result["range"]["start"], json!({"line":1,"character":column}));
    assert_eq!(result["range"]["end"], json!({"line":1,"character":column + 8}));
}

#[test]
fn imported_function_hover_shows_owner_signature() {
    let mut server = Server::new();
    with_markdown(&mut server);
    open(&mut server, "file:///hover-dono-func.dart", "int soma(int a, int b) => a + b;");
    let text = "import 'hover-dono-func.dart';\nvoid f() { print(soma(1, 2)); }";
    open(&mut server, "file:///hover-usa-func.dart", text);
    let line = text.lines().nth(1).unwrap();
    let column = line.find("soma").unwrap() as u32;
    let result = request_hover(&mut server, 2, "file:///hover-usa-func.dart", 1, column);
    assert_eq!(result["contents"], json!({
        "kind":"markdown","value":"```dart\nint soma(int a, int b)\n```"
    }));
}

#[test]
fn prefixed_hover_does_not_invent_description() {
    let mut server = Server::new();
    open(&mut server, "file:///hover-dono-prefixo.dart", "int resposta = 42;");
    let text = "import 'hover-dono-prefixo.dart' as d;\nvoid f() { print(d.resposta); }";
    open(&mut server, "file:///hover-usa-prefixo.dart", text);
    let line = text.lines().nth(1).unwrap();
    let column = (line.find("resposta").unwrap() as u32) + 1;
    assert_eq!(
        request_hover(&mut server, 1, "file:///hover-usa-prefixo.dart", 1, column),
        Value::Null
    );
}

#[test]
fn show_hover_does_not_invent_description() {
    let mut server = Server::new();
    open(&mut server, "file:///hover-dono-show.dart", "int resposta = 42;");
    let text = "import 'hover-dono-show.dart' show resposta;\nvoid f() { print(resposta); }";
    open(&mut server, "file:///hover-usa-show.dart", text);
    let line = text.lines().nth(1).unwrap();
    let column = line.find("resposta").unwrap() as u32;
    assert_eq!(
        request_hover(&mut server, 1, "file:///hover-usa-show.dart", 1, column),
        Value::Null
    );
}

#[test]
fn hover_of_symbol_missing_in_owner_returns_empty() {
    let mut server = Server::new();
    open(&mut server, "file:///hover-dono-ausente.dart", "int outra = 1;");
    let text = "import 'hover-dono-ausente.dart';\nvoid f() { print(resposta); }";
    open(&mut server, "file:///hover-usa-ausente.dart", text);
    let line = text.lines().nth(1).unwrap();
    let column = line.find("resposta").unwrap() as u32;
    assert_eq!(
        request_hover(&mut server, 1, "file:///hover-usa-ausente.dart", 1, column),
        Value::Null
    );
}
