use dartforge_lsp::Server;
use serde_json::{Value, json};

#[test]
fn existing_relative_part_definition_uses_utf16_position() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/tmp-agent/lsp-nav-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let origin = root.join("origem.dart");
    let target = root.join("alvo.dart");
    let source = "/*👭*/ part 'alvo.dart';";
    std::fs::write(&origin, source).unwrap();
    std::fs::write(&target, "part of 'origem.dart';").unwrap();
    let uri = url::Url::from_file_path(&origin).unwrap().to_string();
    let target_uri = url::Url::from_file_path(&target).unwrap().to_string();
    let mut server = Server::new();
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
    let start = server.pump();
    assert_eq!(
        start[0]["result"]["capabilities"]["definitionProvider"],
        true
    );
    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":source}
        }}),
    );
    server.pump();
    server.receive(
        json!({"jsonrpc":"2.0","id":2,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":uri},"position":{"line":0,"character":15}}}),
    );
    let response = server.pump();
    assert_eq!(response[0]["result"]["uri"], target_uri);
    assert_eq!(
        response[0]["result"]["range"]["start"],
        json!({"line":0,"character":0})
    );

    server.receive(
        json!({"jsonrpc":"2.0","id":3,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":uri},"position":{"line":0,"character":9}}}),
    );
    assert_eq!(server.pump()[0]["result"], Value::Null);

    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn package_uri_does_not_invent_target() {
    let mut server = Server::new();
    let uri = "file:///no-project/origin.dart";
    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri,"languageId":"dart","version":1,
                "text":"import 'package:nao_existe/a.dart';"}
        }}),
    );
    server.pump();
    server.receive(
        json!({"jsonrpc":"2.0","id":1,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":uri},"position":{"line":0,"character":10}}}),
    );
    assert_eq!(server.pump()[0]["result"], Value::Null);
}

#[test]
fn mapped_package_uri_navigates_only_to_existing_file() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/tmp-agent/lsp-pacote-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join(".dart_tool")).unwrap();
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::write(
        root.join(".dart_tool/package_config.json"),
        r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":"lib/"}]}"#,
    )
    .unwrap();
    let origin = root.join("lib/origem.dart");
    let target = root.join("lib/alvo.dart");
    let source = "import 'package:app/alvo.dart';";
    std::fs::write(&origin, source).unwrap();
    std::fs::write(&target, "class Alvo {}").unwrap();
    let uri = url::Url::from_file_path(&origin).unwrap().to_string();
    let target_uri = url::Url::from_file_path(std::fs::canonicalize(&target).unwrap())
        .unwrap()
        .to_string();
    let mut server = Server::new();
    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":source}
        }}),
    );
    server.pump();
    let define = |server: &mut Server, id, column| {
        server.receive(
            json!({"jsonrpc":"2.0","id":id,"method":"textDocument/definition",
            "params":{"textDocument":{"uri":uri},"position":{"line":0,"character":column}}}),
        );
        server.pump()[0]["result"].clone()
    };
    assert_eq!(define(&mut server, 1, 16)["uri"], target_uri);
    assert_eq!(define(&mut server, 2, 6), Value::Null);
    std::fs::remove_file(&target).unwrap();
    assert_eq!(define(&mut server, 3, 16), Value::Null);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn single_local_type_navigates_to_declared_name() {
    let mut server = Server::new();
    let uri = "file:///tipo-local.dart";
    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri,"languageId":"dart","version":1,
                "text":"class Caixa {}\nCaixa criar(Caixa valor) => valor;"}
        }}),
    );
    server.pump();
    server.receive(
        json!({"jsonrpc":"2.0","id":1,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":2}}}),
    );
    let response = server.pump();
    assert_eq!(response[0]["result"]["uri"], uri);
    assert_eq!(
        response[0]["result"]["range"]["start"],
        json!({"line":0,"character":6})
    );
    assert_eq!(
        response[0]["result"]["range"]["end"],
        json!({"line":0,"character":11})
    );
}

#[test]
fn type_homonymous_with_generic_parameter_does_not_navigate_to_class() {
    let mut server = Server::new();
    let uri = "file:///sombra.dart";
    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri,"languageId":"dart","version":1,
                "text":"class Caixa {}\nvoid f<Caixa>(Caixa x) {}"}
        }}),
    );
    server.pump();
    server.receive(
        json!({"jsonrpc":"2.0","id":1,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":15}}}),
    );
    assert_eq!(server.pump()[0]["result"], Value::Null);
}

#[test]
fn reference_to_single_top_level_variable_navigates_to_declaration() {
    let mut server = Server::new();
    let uri = "file:///var-topo.dart";
    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri,"languageId":"dart","version":1,
                "text":"int resposta = 42;\nvoid main() { print(resposta); }"}
        }}),
    );
    server.pump();
    server.receive(
        json!({"jsonrpc":"2.0","id":1,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":22}}}),
    );
    let response = server.pump();
    assert_eq!(response[0]["result"]["uri"], uri);
    assert_eq!(
        response[0]["result"]["range"]["start"],
        json!({"line":0,"character":4})
    );
    assert_eq!(
        response[0]["result"]["range"]["end"],
        json!({"line":0,"character":12})
    );
}

#[test]
fn call_to_single_top_level_function_navigates_to_declaration() {
    let mut server = Server::new();
    let uri = "file:///funcao-topo.dart";
    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri,"languageId":"dart","version":1,
                "text":"int resposta() => 42;\nvoid main() { print(resposta()); }"}
        }}),
    );
    server.pump();
    server.receive(
        json!({"jsonrpc":"2.0","id":1,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":22}}}),
    );
    let response = server.pump();
    assert_eq!(response[0]["result"]["uri"], uri);
    assert_eq!(
        response[0]["result"]["range"]["start"],
        json!({"line":0,"character":4})
    );
    assert_eq!(
        response[0]["result"]["range"]["end"],
        json!({"line":0,"character":12})
    );
}

#[test]
fn top_level_getter_read_navigates_to_declaration() {
    let mut server = Server::new();
    let uri = "file:///getter-topo.dart";
    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri,"languageId":"dart","version":1,
                "text":"String get resposta => 'ok';\nvoid main() { print(resposta); }"}
        }}),
    );
    server.pump();
    server.receive(
        json!({"jsonrpc":"2.0","id":1,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":22}}}),
    );
    let response = server.pump();
    assert_eq!(response[0]["result"]["uri"], uri);
    assert_eq!(
        response[0]["result"]["range"]["start"],
        json!({"line":0,"character":11})
    );
    assert_eq!(
        response[0]["result"]["range"]["end"],
        json!({"line":0,"character":19})
    );
}

#[test]
fn imported_use_navigates_to_declaration_in_other_open_file() {
    let mut server = Server::new();
    let owner = "file:///nav-dono.dart";
    let uses = "file:///nav-usa.dart";
    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":owner,"languageId":"dart","version":1,
                "text":"int resposta = 42;"}
        }}),
    );
    server.pump();
    let text_uses = "import 'nav-dono.dart';\nvoid f() { print(resposta); }";
    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uses,"languageId":"dart","version":1,"text":text_uses}
        }}),
    );
    server.pump();
    let line = text_uses.lines().nth(1).unwrap();
    let column = line.find("resposta").unwrap() as u32;
    server.receive(
        json!({"jsonrpc":"2.0","id":1,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":uses},"position":{"line":1,"character":column}}}),
    );
    let response = server.pump();
    assert_eq!(response[0]["result"]["uri"], owner);
    assert_eq!(
        response[0]["result"]["range"]["start"],
        json!({"line":0,"character":4})
    );
    assert_eq!(
        response[0]["result"]["range"]["end"],
        json!({"line":0,"character":12})
    );
}

#[test]
fn prefixed_use_does_not_invent_target_in_other_open_file() {
    let mut server = Server::new();
    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":"file:///nav-dono.dart","languageId":"dart","version":1,
                "text":"int resposta = 42;"}
        }}),
    );
    server.pump();
    server.receive(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":"file:///nav-usa.dart","languageId":"dart","version":1,
                "text":"import 'nav-dono.dart' as d;\nvoid f() { print(d.resposta); }"}
        }}),
    );
    server.pump();
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":"file:///nav-usa.dart"},"position":{"line":1,"character":24}}}));
    assert_eq!(server.pump()[0]["result"], Value::Null);
}
