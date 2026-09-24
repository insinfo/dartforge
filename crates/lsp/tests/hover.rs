use dartforge_lsp::Servidor;
use serde_json::{Value, json};

#[test]
fn hover_de_tipo_local_usa_markdown_e_intervalo_da_referencia() {
    let mut servidor = Servidor::new();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "capabilities":{"textDocument":{"hover":{"contentFormat":["markdown","plaintext"]}}}
    }}));
    let inicio = servidor.bombear();
    assert_eq!(inicio[0]["result"]["capabilities"]["hoverProvider"], true);
    let uri = "file:///hover.dart";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"class Caixa {}\nCaixa item;"}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":2}}}));
    let resposta = servidor.bombear();
    assert_eq!(resposta[0]["result"]["contents"],
        json!({"kind":"markdown","value":"```dart\nclass Caixa\n```"}));
    assert_eq!(resposta[0]["result"]["range"]["start"],
        json!({"line":1,"character":0}));
    assert_eq!(resposta[0]["result"]["range"]["end"],
        json!({"line":1,"character":5}));
}

#[test]
fn hover_plano_e_recusa_tipo_generico_sem_assinatura_fiel() {
    let mut servidor = Servidor::new();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
    servidor.bombear();
    let uri = "file:///hover-plano.dart";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"enum Cor { azul }\nCor cor;\nclass Caixa<T> {}\nCaixa<int> caixa;"}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":1}}}));
    assert_eq!(servidor.bombear()[0]["result"]["contents"], "enum Cor");
    servidor.receber(json!({"jsonrpc":"2.0","id":3,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":3,"character":2}}}));
    assert_eq!(servidor.bombear()[0]["result"], Value::Null);
}

#[test]
fn hover_da_variavel_de_topo_com_tipo_escrito() {
    let mut servidor = Servidor::new();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}
    }}));
    servidor.bombear();
    let uri = "file:///variavel.dart";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"int resposta = 42;\nvoid main() { print(resposta); }"}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":22}}}));
    let resposta = servidor.bombear();
    assert_eq!(resposta[0]["result"]["contents"], json!({
        "kind":"markdown","value":"```dart\nint resposta\n```\nType: `int`"
    }));
}

#[test]
fn variavel_local_homonima_impede_hover_de_topo() {
    let mut servidor = Servidor::new();
    let uri = "file:///sombra-variavel.dart";
    let fonte = "int resposta = 42;\nvoid f() { int resposta = 1; print(resposta); }";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":fonte}
    }}));
    servidor.bombear();
    let coluna = fonte.lines().nth(1).unwrap().find("print(resposta)").unwrap() + 8;
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":coluna}}}));
    assert_eq!(servidor.bombear()[0]["result"], Value::Null);
}
