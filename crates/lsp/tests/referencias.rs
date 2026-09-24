use dartforge_lsp::Servidor;
use serde_json::{Value, json};

fn abrir(servidor: &mut Servidor, uri: &str, texto: &str) {
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":texto}
    }}));
    servidor.bombear();
}

fn pedir(servidor: &mut Servidor, id: i64, uri: &str, linha: u32, coluna: u32, incluir: bool) -> Value {
    servidor.receber(json!({"jsonrpc":"2.0","id":id,"method":"textDocument/references",
        "params":{"textDocument":{"uri":uri},"position":{"line":linha,"character":coluna},
            "context":{"includeDeclaration":incluir}}}));
    servidor.bombear()[0]["result"].clone()
}

#[test]
fn referencias_anuncia_provedor() {
    let mut servidor = Servidor::new();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
    let inicio = servidor.bombear();
    assert_eq!(inicio[0]["result"]["capabilities"]["referencesProvider"], true);
}

#[test]
fn variavel_de_topo_lista_declaracao_e_uso() {
    let mut servidor = Servidor::new();
    let uri = "file:///refs-var.dart";
    let texto = "int resposta = 42;\nvoid main() { print(resposta); }";
    abrir(&mut servidor, uri, texto);
    let resultado = pedir(&mut servidor, 1, uri, 1, 22, true);
    let lista = resultado.as_array().expect("lista de locais");
    assert_eq!(lista.len(), 2, "{lista:?}");
    assert_eq!(lista[0]["uri"], uri);
    assert_eq!(lista[0]["range"]["start"], json!({"line":0,"character":4}));
    assert_eq!(lista[0]["range"]["end"], json!({"line":0,"character":12}));
    assert_eq!(lista[1]["range"]["start"], json!({"line":1,"character":20}));
    assert_eq!(lista[1]["range"]["end"], json!({"line":1,"character":28}));
    // A partir da declaração também resolve.
    let da_decl = pedir(&mut servidor, 2, uri, 0, 5, true);
    assert_eq!(da_decl.as_array().expect("lista").len(), 2);
    // Sem a declaração, só o uso.
    let sem_decl = pedir(&mut servidor, 3, uri, 1, 22, false);
    let lista = sem_decl.as_array().expect("lista");
    assert_eq!(lista.len(), 1, "{lista:?}");
    assert_eq!(lista[0]["range"]["start"], json!({"line":1,"character":20}));
}

#[test]
fn funcao_de_topo_lista_declaracao_e_chamada() {
    let mut servidor = Servidor::new();
    let uri = "file:///refs-func.dart";
    abrir(&mut servidor, uri, "int resposta() => 42;\nvoid main() { print(resposta()); }");
    let resultado = pedir(&mut servidor, 1, uri, 1, 22, true);
    let lista = resultado.as_array().expect("lista");
    assert_eq!(lista.len(), 2, "{lista:?}");
    assert_eq!(lista[0]["range"]["start"], json!({"line":0,"character":4}));
}

#[test]
fn tipo_local_unico_lista_declaracao_e_usos() {
    let mut servidor = Servidor::new();
    let uri = "file:///refs-tipo.dart";
    abrir(&mut servidor, uri, "class Caixa {}\nCaixa a;\nCaixa b;");
    let resultado = pedir(&mut servidor, 1, uri, 1, 2, true);
    let lista = resultado.as_array().expect("lista");
    assert_eq!(lista.len(), 3, "{lista:?}");
    assert_eq!(lista[0]["range"]["start"], json!({"line":0,"character":6}));
    assert_eq!(lista[0]["range"]["end"], json!({"line":0,"character":11}));
}

#[test]
fn sombra_local_recusa_referencias() {
    let mut servidor = Servidor::new();
    let uri = "file:///refs-sombra.dart";
    let fonte = "int resposta = 42;\nvoid f() { int resposta = 1; print(resposta); }";
    abrir(&mut servidor, uri, fonte);
    let coluna = fonte.lines().nth(1).unwrap().find("print(resposta)").unwrap() + 8;
    let resultado = pedir(&mut servidor, 1, uri, 1, coluna as u32, true);
    assert_eq!(resultado, Value::Null);
}

#[test]
fn import_presente_recusa_referencias() {
    let mut servidor = Servidor::new();
    let uri = "file:///refs-import.dart";
    abrir(&mut servidor, uri, "import 'dart:math';\nint resposta = 1;\nvoid f() { print(resposta); }");
    let resultado = pedir(&mut servidor, 1, uri, 2, 20, true);
    assert_eq!(resultado, Value::Null);
}
