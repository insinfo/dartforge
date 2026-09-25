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

#[test]
fn hover_de_funcao_de_topo_sem_parametros() {
    let mut servidor = Servidor::new();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}
    }}));
    servidor.bombear();
    let uri = "file:///funcao.dart";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"int resposta() => 42;\nvoid main() { print(resposta()); }"}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":22}}}));
    let resposta = servidor.bombear();
    assert_eq!(resposta[0]["result"]["contents"], json!({
        "kind":"markdown","value":"```dart\nint resposta()\n```"
    }));
}

#[test]
fn funcao_local_homonima_impede_hover_de_topo() {
    let mut servidor = Servidor::new();
    let uri = "file:///sombra-funcao.dart";
    let fonte = "int resposta() => 42;\nvoid main() { int resposta() => 1; print(resposta()); }";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":fonte}
    }}));
    servidor.bombear();
    let coluna = fonte.lines().nth(1).unwrap().find("print(resposta())").unwrap() + 8;
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":coluna}}}));
    assert_eq!(servidor.bombear()[0]["result"], Value::Null);
}

#[test]
fn hover_de_funcao_com_dois_posicionais_tipados() {
    let mut servidor = Servidor::new();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}
    }}));
    servidor.bombear();
    let uri = "file:///funcao-parametros.dart";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"int soma(int a, int b) => a + b;\nvoid main() { print(soma(1, 2)); }"}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":21}}}));
    let resposta = servidor.bombear();
    assert_eq!(resposta[0]["result"]["contents"], json!({
        "kind":"markdown","value":"```dart\nint soma(int a, int b)\n```"
    }));
}

#[test]
fn hover_recusa_assinatura_opcional_que_precisa_formatacao_completa() {
    let mut servidor = Servidor::new();
    let uri = "file:///funcao-opcional.dart";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"int soma([int a = 1]) => a;\nvoid main() { print(soma()); }"}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":21}}}));
    assert_eq!(servidor.bombear()[0]["result"], Value::Null);
}

#[test]
fn hover_getter_de_topo_tipado_exibe_assinatura_e_tipo() {
    let mut servidor = Servidor::new();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}
    }}));
    servidor.bombear();
    let uri = "file:///getter-topo.dart";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"String get resposta => 'ok';\nvoid main() { print(resposta); }"}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":22}}}));
    let resposta = servidor.bombear();
    assert_eq!(resposta[0]["result"]["contents"], json!({
        "kind":"markdown","value":"```dart\nString get resposta\n```\nType: `String`"
    }));
    assert_eq!(resposta[0]["result"]["range"]["start"], json!({"line":1,"character":20}));
    assert_eq!(resposta[0]["result"]["range"]["end"], json!({"line":1,"character":28}));
}

#[test]
fn getter_homonimo_em_membro_impede_hover_de_topo() {
    let mut servidor = Servidor::new();
    let uri = "file:///getter-sombra.dart";
    let fonte = "String get resposta => 'topo';\nclass A { String get resposta => 'membro'; void f() { print(resposta); } }";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":fonte}
    }}));
    servidor.bombear();
    let coluna = fonte.lines().nth(1).unwrap().find("print(resposta)").unwrap() + 8;
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":1,"character":coluna}}}));
    assert_eq!(servidor.bombear()[0]["result"], Value::Null);
}

fn abrir(servidor: &mut Servidor, uri: &str, texto: &str) {
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":texto}
    }}));
    servidor.bombear();
}

fn pedir_hover(servidor: &mut Servidor, id: i64, uri: &str, linha: u32, coluna: u32) -> Value {
    servidor.receber(json!({"jsonrpc":"2.0","id":id,"method":"textDocument/hover",
        "params":{"textDocument":{"uri":uri},"position":{"line":linha,"character":coluna}}}));
    servidor.bombear()[0]["result"].clone()
}

fn com_markdown(servidor: &mut Servidor) {
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}
    }}));
    servidor.bombear();
}

#[test]
fn hover_de_classe_importada_formata_a_partir_do_dono() {
    let mut servidor = Servidor::new();
    com_markdown(&mut servidor);
    abrir(&mut servidor, "file:///hover-dono-classe.dart", "class Caixa {}");
    let texto = "import 'hover-dono-classe.dart';\nCaixa b;";
    abrir(&mut servidor, "file:///hover-usa-classe.dart", texto);
    let resultado = pedir_hover(&mut servidor, 2, "file:///hover-usa-classe.dart", 1, 2);
    assert_eq!(resultado["contents"],
        json!({"kind":"markdown","value":"```dart\nclass Caixa\n```"}));
    assert_eq!(resultado["range"]["start"], json!({"line":1,"character":0}));
    assert_eq!(resultado["range"]["end"], json!({"line":1,"character":5}));
}

#[test]
fn hover_de_variavel_importada_mostra_tipo_do_dono() {
    let mut servidor = Servidor::new();
    com_markdown(&mut servidor);
    abrir(&mut servidor, "file:///hover-dono-var.dart", "int resposta = 42;");
    let texto = "import 'hover-dono-var.dart';\nvoid f() { print(resposta); }";
    abrir(&mut servidor, "file:///hover-usa-var.dart", texto);
    let linha = texto.lines().nth(1).unwrap();
    let coluna = linha.find("resposta").unwrap() as u32;
    let resultado = pedir_hover(&mut servidor, 2, "file:///hover-usa-var.dart", 1, coluna);
    assert_eq!(resultado["contents"], json!({
        "kind":"markdown","value":"```dart\nint resposta\n```\nType: `int`"
    }));
    assert_eq!(resultado["range"]["start"], json!({"line":1,"character":coluna}));
    assert_eq!(resultado["range"]["end"], json!({"line":1,"character":coluna + 8}));
}

#[test]
fn hover_de_funcao_importada_mostra_assinatura_do_dono() {
    let mut servidor = Servidor::new();
    com_markdown(&mut servidor);
    abrir(&mut servidor, "file:///hover-dono-func.dart", "int soma(int a, int b) => a + b;");
    let texto = "import 'hover-dono-func.dart';\nvoid f() { print(soma(1, 2)); }";
    abrir(&mut servidor, "file:///hover-usa-func.dart", texto);
    let linha = texto.lines().nth(1).unwrap();
    let coluna = linha.find("soma").unwrap() as u32;
    let resultado = pedir_hover(&mut servidor, 2, "file:///hover-usa-func.dart", 1, coluna);
    assert_eq!(resultado["contents"], json!({
        "kind":"markdown","value":"```dart\nint soma(int a, int b)\n```"
    }));
}

#[test]
fn hover_com_prefixo_nao_inventa_descricao() {
    let mut servidor = Servidor::new();
    abrir(&mut servidor, "file:///hover-dono-prefixo.dart", "int resposta = 42;");
    let texto = "import 'hover-dono-prefixo.dart' as d;\nvoid f() { print(d.resposta); }";
    abrir(&mut servidor, "file:///hover-usa-prefixo.dart", texto);
    let linha = texto.lines().nth(1).unwrap();
    let coluna = (linha.find("resposta").unwrap() as u32) + 1;
    assert_eq!(
        pedir_hover(&mut servidor, 1, "file:///hover-usa-prefixo.dart", 1, coluna),
        Value::Null
    );
}

#[test]
fn hover_com_show_nao_inventa_descricao() {
    let mut servidor = Servidor::new();
    abrir(&mut servidor, "file:///hover-dono-show.dart", "int resposta = 42;");
    let texto = "import 'hover-dono-show.dart' show resposta;\nvoid f() { print(resposta); }";
    abrir(&mut servidor, "file:///hover-usa-show.dart", texto);
    let linha = texto.lines().nth(1).unwrap();
    let coluna = linha.find("resposta").unwrap() as u32;
    assert_eq!(
        pedir_hover(&mut servidor, 1, "file:///hover-usa-show.dart", 1, coluna),
        Value::Null
    );
}

#[test]
fn hover_de_simbolo_ausente_no_dono_devolve_vazio() {
    let mut servidor = Servidor::new();
    abrir(&mut servidor, "file:///hover-dono-ausente.dart", "int outra = 1;");
    let texto = "import 'hover-dono-ausente.dart';\nvoid f() { print(resposta); }";
    abrir(&mut servidor, "file:///hover-usa-ausente.dart", texto);
    let linha = texto.lines().nth(1).unwrap();
    let coluna = linha.find("resposta").unwrap() as u32;
    assert_eq!(
        pedir_hover(&mut servidor, 1, "file:///hover-usa-ausente.dart", 1, coluna),
        Value::Null
    );
}
