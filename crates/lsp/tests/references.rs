use dartforge_lsp::Server;
use serde_json::{Value, json};

fn open(server: &mut Server, uri: &str, text: &str) {
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":text}
    }}));
    server.pump();
}

fn request(server: &mut Server, id: i64, uri: &str, line: u32, column: u32, include: bool) -> Value {
    server.receive(json!({"jsonrpc":"2.0","id":id,"method":"textDocument/references",
        "params":{"textDocument":{"uri":uri},"position":{"line":line,"character":column},
            "context":{"includeDeclaration":include}}}));
    server.pump()[0]["result"].clone()
}

#[test]
fn references_announces_provider() {
    let mut server = Server::new();
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
    let start = server.pump();
    assert_eq!(start[0]["result"]["capabilities"]["referencesProvider"], true);
}

#[test]
fn top_level_variable_lists_declaration_and_use() {
    let mut server = Server::new();
    let uri = "file:///refs-var.dart";
    let text = "int resposta = 42;\nvoid main() { print(resposta); }";
    open(&mut server, uri, text);
    let result = request(&mut server, 1, uri, 1, 22, true);
    let list = result.as_array().expect("lista de locais");
    // Como o `dart language-server` 3.6.2: os usos e, por último, a
    // declaração.
    assert_eq!(list.len(), 2, "{list:?}");
    assert_eq!(list[1]["uri"], uri);
    assert_eq!(list[1]["range"]["start"], json!({"line":0,"character":4}));
    assert_eq!(list[1]["range"]["end"], json!({"line":0,"character":12}));
    assert_eq!(list[0]["range"]["start"], json!({"line":1,"character":20}));
    assert_eq!(list[0]["range"]["end"], json!({"line":1,"character":28}));
    // A partir da declaração também resolve.
    let of_decl = request(&mut server, 2, uri, 0, 5, true);
    assert_eq!(of_decl.as_array().expect("lista").len(), 2);
    // Sem a declaração, só o uso.
    let without_decl = request(&mut server, 3, uri, 1, 22, false);
    let list = without_decl.as_array().expect("lista");
    assert_eq!(list.len(), 1, "{list:?}");
    assert_eq!(list[0]["range"]["start"], json!({"line":1,"character":20}));
}

#[test]
fn top_level_function_lists_declaration_and_call() {
    let mut server = Server::new();
    let uri = "file:///refs-func.dart";
    open(&mut server, uri, "int resposta() => 42;\nvoid main() { print(resposta()); }");
    let result = request(&mut server, 1, uri, 1, 22, true);
    let list = result.as_array().expect("lista");
    assert_eq!(list.len(), 2, "{list:?}");
    assert_eq!(list[1]["range"]["start"], json!({"line":0,"character":4}));
}

#[test]
fn single_local_type_lists_declaration_and_uses() {
    let mut server = Server::new();
    let uri = "file:///refs-tipo.dart";
    open(&mut server, uri, "class Caixa {}\nCaixa a;\nCaixa b;");
    let result = request(&mut server, 1, uri, 1, 2, true);
    let list = result.as_array().expect("lista");
    assert_eq!(list.len(), 3, "{list:?}");
    assert_eq!(list[2]["range"]["start"], json!({"line":0,"character":6}));
    assert_eq!(list[2]["range"]["end"], json!({"line":0,"character":11}));
}

#[test]
fn local_shadow_refuses_references() {
    let mut server = Server::new();
    let uri = "file:///refs-sombra.dart";
    let source = "int resposta = 42;\nvoid f() { int resposta = 1; print(resposta); }";
    open(&mut server, uri, source);
    let column = source.lines().nth(1).unwrap().find("print(resposta)").unwrap() + 8;
    let result = request(&mut server, 1, uri, 1, column as u32, true);
    assert_eq!(result, Value::Null);
}

#[test]
fn present_import_refuses_references() {
    let mut server = Server::new();
    let uri = "file:///refs-import.dart";
    open(&mut server, uri, "import 'dart:math';\nint resposta = 1;\nvoid f() { print(resposta); }");
    let result = request(&mut server, 1, uri, 2, 20, true);
    assert_eq!(result, Value::Null);
}

const OWNER_A: &str = "file:///cross-a.dart";
const USES_B: &str = "file:///cross-b.dart";

fn column_in(line: &str, target: &str) -> u32 {
    line.find(target).expect("alvo na linha") as u32
}

#[test]
fn variable_across_open_lists_declaration_and_uses() {
    let mut server = Server::new();
    let text_a = "int resposta = 42;\nvoid g() { print(resposta); }";
    let text_b = "import 'cross-a.dart';\nvoid f() { print(resposta); }";
    open(&mut server, OWNER_A, text_a);
    open(&mut server, USES_B, text_b);
    let line_b = text_b.lines().nth(1).unwrap();
    let result = request(&mut server, 1, USES_B, 1, column_in(line_b, "resposta"), true);
    let list = result.as_array().expect("lista de locais");
    // O uso no arquivo do pedido, o do dono e a declaração por último
    // (`dart language-server` 3.6.2).
    assert_eq!(list.len(), 3, "{list:?}");
    assert_eq!(list[2]["uri"], OWNER_A);
    assert_eq!(list[2]["range"]["start"], json!({"line":0,"character":4}));
    assert_eq!(list[2]["range"]["end"], json!({"line":0,"character":12}));
    assert_eq!(list[1]["uri"], OWNER_A);
    let line_a = text_a.lines().nth(1).unwrap();
    assert_eq!(list[1]["range"]["start"], json!({"line":1,"character":column_in(line_a, "resposta")}));
    assert_eq!(list[0]["uri"], USES_B);
    assert_eq!(list[0]["range"]["start"], json!({"line":1,"character":column_in(line_b, "resposta")}));
    // A partir da declaração no dono agrega os abertos.
    let of_decl = request(&mut server, 2, OWNER_A, 0, 5, true);
    assert_eq!(of_decl.as_array().expect("lista").len(), 3);
    // Sem a declaração, só os usos dos dois arquivos.
    let without_decl = request(&mut server, 3, USES_B, 1, column_in(line_b, "resposta"), false);
    let uses = without_decl.as_array().expect("lista");
    assert_eq!(uses.len(), 2, "{uses:?}");
    assert_eq!(uses[0]["uri"], USES_B);
    assert_eq!(uses[1]["uri"], OWNER_A);
}

#[test]
fn type_across_open_lists_declaration_and_uses() {
    let mut server = Server::new();
    open(&mut server, OWNER_A, "class Caixa {}\nCaixa a;");
    open(&mut server, USES_B, "import 'cross-a.dart';\nCaixa b;");
    let result = request(&mut server, 1, USES_B, 1, 2, true);
    let list = result.as_array().expect("lista");
    assert_eq!(list.len(), 3, "{list:?}");
    assert_eq!(list[2]["uri"], OWNER_A);
    assert_eq!(list[2]["range"]["start"], json!({"line":0,"character":6}));
    assert_eq!(list[2]["range"]["end"], json!({"line":0,"character":11}));
    assert_eq!(list[1], json!({"uri": OWNER_A, "range": {"start": {"line":1,"character":0}, "end": {"line":1,"character":5}}}));
    assert_eq!(list[0], json!({"uri": USES_B, "range": {"start": {"line":1,"character":0}, "end": {"line":1,"character":5}}}));
}

#[test]
fn function_across_open_lists_declaration_and_calls() {
    let mut server = Server::new();
    open(&mut server, OWNER_A, "int somar() => 1;\nvoid g() { print(somar()); }");
    open(&mut server, USES_B, "import 'cross-a.dart';\nvoid f() { print(somar()); }");
    let line_b = "void f() { print(somar()); }";
    let result = request(&mut server, 1, USES_B, 1, column_in(line_b, "somar"), true);
    let list = result.as_array().expect("lista");
    assert_eq!(list.len(), 3, "{list:?}");
    assert_eq!(list[2]["uri"], OWNER_A);
    assert_eq!(list[2]["range"]["start"], json!({"line":0,"character":4}));
}

#[test]
fn prefixed_import_refuses_references() {
    let mut server = Server::new();
    open(&mut server, OWNER_A, "int resposta = 42;");
    open(&mut server, USES_B, "import 'cross-a.dart' as a;\nvoid f() { print(a.resposta); }");
    let line_b = "void f() { print(a.resposta); }";
    let result = request(&mut server, 1, USES_B, 1, column_in(line_b, "resposta"), true);
    assert_eq!(result, Value::Null);
}

#[test]
fn show_import_refuses_references() {
    let mut server = Server::new();
    open(&mut server, OWNER_A, "int resposta = 42;");
    open(&mut server, USES_B, "import 'cross-a.dart' show resposta;\nvoid f() { print(resposta); }");
    let line_b = "void f() { print(resposta); }";
    let result = request(&mut server, 1, USES_B, 1, column_in(line_b, "resposta"), true);
    assert_eq!(result, Value::Null);
}

#[test]
fn closed_owner_refuses_references() {
    let mut server = Server::new();
    let text_b = "import 'cross-a.dart';\nvoid f() { print(resposta); }";
    open(&mut server, USES_B, text_b);
    let line_b = text_b.lines().nth(1).unwrap();
    let result = request(&mut server, 1, USES_B, 1, column_in(line_b, "resposta"), true);
    assert_eq!(result, Value::Null);
}

#[test]
fn shadow_in_importer_skips_file() {
    let mut server = Server::new();
    let text_a = "int resposta = 42;\nvoid g() { print(resposta); }";
    open(&mut server, OWNER_A, text_a);
    open(
        &mut server,
        USES_B,
        "import 'cross-a.dart';\nint resposta = 1;\nvoid f() { print(resposta); }",
    );
    let result = request(&mut server, 1, OWNER_A, 0, 5, true);
    let list = result.as_array().expect("lista");
    assert_eq!(list.len(), 2, "{list:?}");
    assert!(list.iter().all(|l| l["uri"] == OWNER_A), "{list:?}");
}

#[test]
fn two_owners_refuses_references() {
    let mut server = Server::new();
    open(&mut server, OWNER_A, "int resposta = 1;");
    open(&mut server, "file:///cross-c.dart", "int resposta = 2;");
    let text_b = "import 'cross-a.dart';\nimport 'cross-c.dart';\nvoid f() { print(resposta); }";
    open(&mut server, USES_B, text_b);
    let line_b = text_b.lines().nth(2).unwrap();
    let result = request(&mut server, 1, USES_B, 2, column_in(line_b, "resposta"), true);
    assert_eq!(result, Value::Null);
}
