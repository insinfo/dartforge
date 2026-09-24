use dartforge_lsp::Servidor;
use serde_json::{Value, json};

#[test]
fn definicao_de_part_relativo_existente_usa_posicao_utf16() {
    let raiz = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/tmp-agent/lsp-nav-{}", std::process::id()));
    std::fs::create_dir_all(&raiz).unwrap();
    let raiz = std::fs::canonicalize(raiz).unwrap();
    let origem = raiz.join("origem.dart");
    let destino = raiz.join("alvo.dart");
    let fonte = "/*👭*/ part 'alvo.dart';";
    std::fs::write(&origem, fonte).unwrap();
    std::fs::write(&destino, "part of 'origem.dart';").unwrap();
    let uri = url::Url::from_file_path(&origem).unwrap().to_string();
    let destino_uri = url::Url::from_file_path(&destino).unwrap().to_string();
    let mut servidor = Servidor::new();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
    let inicio = servidor.bombear();
    assert_eq!(inicio[0]["result"]["capabilities"]["definitionProvider"], true);
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":fonte}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":uri},"position":{"line":0,"character":15}}}));
    let resposta = servidor.bombear();
    assert_eq!(resposta[0]["result"]["uri"], destino_uri);
    assert_eq!(resposta[0]["result"]["range"]["start"], json!({"line":0,"character":0}));

    servidor.receber(json!({"jsonrpc":"2.0","id":3,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":uri},"position":{"line":0,"character":9}}}));
    assert_eq!(servidor.bombear()[0]["result"], Value::Null);

    std::fs::remove_dir_all(&raiz).unwrap();
}

#[test]
fn uri_de_pacote_nao_produz_destino_inventado() {
    let mut servidor = Servidor::new();
    let uri = "file:///sem-projeto/origem.dart";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"import 'package:nao_existe/a.dart';"}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":uri},"position":{"line":0,"character":10}}}));
    assert_eq!(servidor.bombear()[0]["result"], Value::Null);
}
