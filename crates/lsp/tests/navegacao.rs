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

#[test]
fn uri_de_pacote_mapeado_navega_so_para_arquivo_existente() {
    let raiz = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/tmp-agent/lsp-pacote-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(raiz.join(".dart_tool")).unwrap();
    std::fs::create_dir_all(raiz.join("lib")).unwrap();
    std::fs::write(
        raiz.join(".dart_tool/package_config.json"),
        r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":"lib/"}]}"#,
    ).unwrap();
    let origem = raiz.join("lib/origem.dart");
    let alvo = raiz.join("lib/alvo.dart");
    let fonte = "import 'package:app/alvo.dart';";
    std::fs::write(&origem, fonte).unwrap();
    std::fs::write(&alvo, "class Alvo {}").unwrap();
    let uri = url::Url::from_file_path(&origem).unwrap().to_string();
    let alvo_uri = url::Url::from_file_path(std::fs::canonicalize(&alvo).unwrap()).unwrap().to_string();
    let mut servidor = Servidor::new();
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":fonte}
    }}));
    servidor.bombear();
    let definir = |servidor: &mut Servidor, id, coluna| {
        servidor.receber(json!({"jsonrpc":"2.0","id":id,"method":"textDocument/definition",
            "params":{"textDocument":{"uri":uri},"position":{"line":0,"character":coluna}}}));
        servidor.bombear()[0]["result"].clone()
    };
    assert_eq!(definir(&mut servidor, 1, 16)["uri"], alvo_uri);
    assert_eq!(definir(&mut servidor, 2, 6), Value::Null);
    std::fs::remove_file(&alvo).unwrap();
    assert_eq!(definir(&mut servidor, 3, 16), Value::Null);
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn tipo_local_unico_navega_para_o_nome_declarado() {
    let mut servidor = Servidor::new();
    let uri = "file:///tipo-local.dart";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"class Caixa {}\nCaixa criar(Caixa valor) => valor;"}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":2}}}));
    let resposta = servidor.bombear();
    assert_eq!(resposta[0]["result"]["uri"], uri);
    assert_eq!(resposta[0]["result"]["range"]["start"], json!({"line":0,"character":6}));
    assert_eq!(resposta[0]["result"]["range"]["end"], json!({"line":0,"character":11}));
}

#[test]
fn tipo_homonimo_de_parametro_generico_nao_navega_para_classe() {
    let mut servidor = Servidor::new();
    let uri = "file:///sombra.dart";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"class Caixa {}\nvoid f<Caixa>(Caixa x) {}"}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":15}}}));
    assert_eq!(servidor.bombear()[0]["result"], Value::Null);
}

#[test]
fn referencia_a_variavel_de_topo_unica_navega_para_declaracao() {
    let mut servidor = Servidor::new();
    let uri = "file:///var-topo.dart";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"int resposta = 42;\nvoid main() { print(resposta); }"}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":22}}}));
    let resposta = servidor.bombear();
    assert_eq!(resposta[0]["result"]["uri"], uri);
    assert_eq!(resposta[0]["result"]["range"]["start"], json!({"line":0,"character":4}));
    assert_eq!(resposta[0]["result"]["range"]["end"], json!({"line":0,"character":12}));
}

#[test]
fn chamada_de_funcao_de_topo_unica_navega_para_declaracao() {
    let mut servidor = Servidor::new();
    let uri = "file:///funcao-topo.dart";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"int resposta() => 42;\nvoid main() { print(resposta()); }"}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":22}}}));
    let resposta = servidor.bombear();
    assert_eq!(resposta[0]["result"]["uri"], uri);
    assert_eq!(resposta[0]["result"]["range"]["start"], json!({"line":0,"character":4}));
    assert_eq!(resposta[0]["result"]["range"]["end"], json!({"line":0,"character":12}));
}

#[test]
fn leitura_de_getter_de_topo_navega_para_declaracao() {
    let mut servidor = Servidor::new();
    let uri = "file:///getter-topo.dart";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"String get resposta => 'ok';\nvoid main() { print(resposta); }"}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":22}}}));
    let resposta = servidor.bombear();
    assert_eq!(resposta[0]["result"]["uri"], uri);
    assert_eq!(resposta[0]["result"]["range"]["start"], json!({"line":0,"character":11}));
    assert_eq!(resposta[0]["result"]["range"]["end"], json!({"line":0,"character":19}));
}

#[test]
fn uso_importado_navega_para_declaracao_em_outro_aberto() {
    let mut servidor = Servidor::new();
    let dono = "file:///nav-dono.dart";
    let usa = "file:///nav-usa.dart";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":dono,"languageId":"dart","version":1,
            "text":"int resposta = 42;"}
    }}));
    servidor.bombear();
    let texto_usa = "import 'nav-dono.dart';\nvoid f() { print(resposta); }";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":usa,"languageId":"dart","version":1,"text":texto_usa}
    }}));
    servidor.bombear();
    let linha = texto_usa.lines().nth(1).unwrap();
    let coluna = linha.find("resposta").unwrap() as u32;
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":usa},"position":{"line":1,"character":coluna}}}));
    let resposta = servidor.bombear();
    assert_eq!(resposta[0]["result"]["uri"], dono);
    assert_eq!(resposta[0]["result"]["range"]["start"], json!({"line":0,"character":4}));
    assert_eq!(resposta[0]["result"]["range"]["end"], json!({"line":0,"character":12}));
}

#[test]
fn uso_com_prefixo_nao_inventa_destino_em_outro_aberto() {
    let mut servidor = Servidor::new();
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":"file:///nav-dono.dart","languageId":"dart","version":1,
            "text":"int resposta = 42;"}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":"file:///nav-usa.dart","languageId":"dart","version":1,
            "text":"import 'nav-dono.dart' as d;\nvoid f() { print(d.resposta); }"}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"textDocument/definition",
        "params":{"textDocument":{"uri":"file:///nav-usa.dart"},"position":{"line":1,"character":24}}}));
    assert_eq!(servidor.bombear()[0]["result"], Value::Null);
}
