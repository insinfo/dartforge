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

const DONO_A: &str = "file:///cross-a.dart";
const USA_B: &str = "file:///cross-b.dart";

fn coluna_em(linha: &str, alvo: &str) -> u32 {
    linha.find(alvo).expect("alvo na linha") as u32
}

#[test]
fn variavel_entre_abertos_lista_declaracao_e_usos() {
    let mut servidor = Servidor::new();
    let texto_a = "int resposta = 42;\nvoid g() { print(resposta); }";
    let texto_b = "import 'cross-a.dart';\nvoid f() { print(resposta); }";
    abrir(&mut servidor, DONO_A, texto_a);
    abrir(&mut servidor, USA_B, texto_b);
    let linha_b = texto_b.lines().nth(1).unwrap();
    let resultado = pedir(&mut servidor, 1, USA_B, 1, coluna_em(linha_b, "resposta"), true);
    let lista = resultado.as_array().expect("lista de locais");
    assert_eq!(lista.len(), 3, "{lista:?}");
    assert_eq!(lista[0]["uri"], DONO_A);
    assert_eq!(lista[0]["range"]["start"], json!({"line":0,"character":4}));
    assert_eq!(lista[0]["range"]["end"], json!({"line":0,"character":12}));
    assert_eq!(lista[1]["uri"], DONO_A);
    let linha_a = texto_a.lines().nth(1).unwrap();
    assert_eq!(lista[1]["range"]["start"], json!({"line":1,"character":coluna_em(linha_a, "resposta")}));
    assert_eq!(lista[2]["uri"], USA_B);
    assert_eq!(lista[2]["range"]["start"], json!({"line":1,"character":coluna_em(linha_b, "resposta")}));
    // A partir da declaração no dono agrega os abertos.
    let da_decl = pedir(&mut servidor, 2, DONO_A, 0, 5, true);
    assert_eq!(da_decl.as_array().expect("lista").len(), 3);
    // Sem a declaração, só os usos dos dois arquivos.
    let sem_decl = pedir(&mut servidor, 3, USA_B, 1, coluna_em(linha_b, "resposta"), false);
    let usos = sem_decl.as_array().expect("lista");
    assert_eq!(usos.len(), 2, "{usos:?}");
    assert_eq!(usos[0]["uri"], DONO_A);
    assert_eq!(usos[1]["uri"], USA_B);
}

#[test]
fn tipo_entre_abertos_lista_declaracao_e_usos() {
    let mut servidor = Servidor::new();
    abrir(&mut servidor, DONO_A, "class Caixa {}\nCaixa a;");
    abrir(&mut servidor, USA_B, "import 'cross-a.dart';\nCaixa b;");
    let resultado = pedir(&mut servidor, 1, USA_B, 1, 2, true);
    let lista = resultado.as_array().expect("lista");
    assert_eq!(lista.len(), 3, "{lista:?}");
    assert_eq!(lista[0]["uri"], DONO_A);
    assert_eq!(lista[0]["range"]["start"], json!({"line":0,"character":6}));
    assert_eq!(lista[0]["range"]["end"], json!({"line":0,"character":11}));
    assert_eq!(lista[1], json!({"uri": DONO_A, "range": {"start": {"line":1,"character":0}, "end": {"line":1,"character":5}}}));
    assert_eq!(lista[2], json!({"uri": USA_B, "range": {"start": {"line":1,"character":0}, "end": {"line":1,"character":5}}}));
}

#[test]
fn funcao_entre_abertos_lista_declaracao_e_chamadas() {
    let mut servidor = Servidor::new();
    abrir(&mut servidor, DONO_A, "int somar() => 1;\nvoid g() { print(somar()); }");
    abrir(&mut servidor, USA_B, "import 'cross-a.dart';\nvoid f() { print(somar()); }");
    let linha_b = "void f() { print(somar()); }";
    let resultado = pedir(&mut servidor, 1, USA_B, 1, coluna_em(linha_b, "somar"), true);
    let lista = resultado.as_array().expect("lista");
    assert_eq!(lista.len(), 3, "{lista:?}");
    assert_eq!(lista[0]["uri"], DONO_A);
    assert_eq!(lista[0]["range"]["start"], json!({"line":0,"character":4}));
}

#[test]
fn import_com_prefixo_recusa_referencias() {
    let mut servidor = Servidor::new();
    abrir(&mut servidor, DONO_A, "int resposta = 42;");
    abrir(&mut servidor, USA_B, "import 'cross-a.dart' as a;\nvoid f() { print(a.resposta); }");
    let linha_b = "void f() { print(a.resposta); }";
    let resultado = pedir(&mut servidor, 1, USA_B, 1, coluna_em(linha_b, "resposta"), true);
    assert_eq!(resultado, Value::Null);
}

#[test]
fn import_com_show_recusa_referencias() {
    let mut servidor = Servidor::new();
    abrir(&mut servidor, DONO_A, "int resposta = 42;");
    abrir(&mut servidor, USA_B, "import 'cross-a.dart' show resposta;\nvoid f() { print(resposta); }");
    let linha_b = "void f() { print(resposta); }";
    let resultado = pedir(&mut servidor, 1, USA_B, 1, coluna_em(linha_b, "resposta"), true);
    assert_eq!(resultado, Value::Null);
}

#[test]
fn dono_fechado_recusa_referencias() {
    let mut servidor = Servidor::new();
    let texto_b = "import 'cross-a.dart';\nvoid f() { print(resposta); }";
    abrir(&mut servidor, USA_B, texto_b);
    let linha_b = texto_b.lines().nth(1).unwrap();
    let resultado = pedir(&mut servidor, 1, USA_B, 1, coluna_em(linha_b, "resposta"), true);
    assert_eq!(resultado, Value::Null);
}

#[test]
fn sombra_no_importador_pula_o_arquivo() {
    let mut servidor = Servidor::new();
    let texto_a = "int resposta = 42;\nvoid g() { print(resposta); }";
    abrir(&mut servidor, DONO_A, texto_a);
    abrir(
        &mut servidor,
        USA_B,
        "import 'cross-a.dart';\nint resposta = 1;\nvoid f() { print(resposta); }",
    );
    let resultado = pedir(&mut servidor, 1, DONO_A, 0, 5, true);
    let lista = resultado.as_array().expect("lista");
    assert_eq!(lista.len(), 2, "{lista:?}");
    assert!(lista.iter().all(|l| l["uri"] == DONO_A), "{lista:?}");
}

#[test]
fn dois_donos_recusa_referencias() {
    let mut servidor = Servidor::new();
    abrir(&mut servidor, DONO_A, "int resposta = 1;");
    abrir(&mut servidor, "file:///cross-c.dart", "int resposta = 2;");
    let texto_b = "import 'cross-a.dart';\nimport 'cross-c.dart';\nvoid f() { print(resposta); }";
    abrir(&mut servidor, USA_B, texto_b);
    let linha_b = texto_b.lines().nth(2).unwrap();
    let resultado = pedir(&mut servidor, 1, USA_B, 2, coluna_em(linha_b, "resposta"), true);
    assert_eq!(resultado, Value::Null);
}
