use dartforge_lsp::Servidor;
use serde_json::{Value, json};

#[test]
fn simbolos_atualizam_com_texto_e_colunas_utf16() {
    let mut servidor = Servidor::new();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "capabilities":{"textDocument":{"documentSymbol":{
            "hierarchicalDocumentSymbolSupport":true
        }}}
    }}));
    let inicio = servidor.bombear();
    assert_eq!(inicio[0]["result"]["capabilities"]["documentSymbolProvider"], true);

    let uri = "file:///simbolos.dart";
    let fonte = "/*👭*/ class Caixa { int valor = 1; int dobro() => valor * 2; }\nvoid main() {}";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":2,"text":fonte}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/documentSymbol",
        "params":{"textDocument":{"uri":uri}}}));
    let resposta = servidor.bombear();
    let simbolos = resposta[0]["result"].as_array().unwrap();
    assert_eq!(simbolos.len(), 2);
    assert_eq!(simbolos[0]["name"], "Caixa");
    assert_eq!(simbolos[0]["kind"], 5);
    assert_eq!(simbolos[0]["selectionRange"]["start"], json!({"line":0,"character":13}));
    assert_eq!(simbolos[0]["children"][0]["name"], "valor");
    assert_eq!(simbolos[0]["children"][1]["name"], "dobro");
    assert_eq!(simbolos[1]["name"], "main");

    // Uma notificação atrasada não muda a revisão nem publica diagnósticos.
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":uri,"version":1},"contentChanges":[{"text":"void antigo() {}"}]
    }}));
    assert!(servidor.bombear().is_empty());
    servidor.receber(json!({"jsonrpc":"2.0","id":3,"method":"textDocument/documentSymbol",
        "params":{"textDocument":{"uri":uri}}}));
    let resposta = servidor.bombear();
    assert_eq!(resposta[0]["result"][0]["name"], "Caixa");

    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":uri,"version":3},"contentChanges":[{"text":"void novo() {}"}]
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":4,"method":"textDocument/documentSymbol",
        "params":{"textDocument":{"uri":uri}}}));
    let resposta = servidor.bombear();
    assert_eq!(resposta[0]["result"][0]["name"], "novo");
    assert_eq!(resposta[0]["result"].as_array().unwrap().len(), 1);

    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{
        "textDocument":{"uri":uri}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":5,"method":"textDocument/documentSymbol",
        "params":{"textDocument":{"uri":uri}}}));
    let resposta = servidor.bombear();
    assert_eq!(resposta[0]["result"], Value::Array(Vec::new()));
}

#[test]
fn cliente_sem_suporte_hierarquico_recebe_simbolos_planos() {
    let mut servidor = Servidor::new();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
    servidor.bombear();
    let uri = "file:///plano.dart";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"class C { void m() {} }"}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/documentSymbol",
        "params":{"textDocument":{"uri":uri}}}));
    let resposta = servidor.bombear();
    let simbolos = resposta[0]["result"].as_array().unwrap();
    assert_eq!(simbolos.len(), 2);
    assert_eq!(simbolos[0]["location"]["uri"], uri);
    assert_eq!(simbolos[1]["name"], "m");
    assert_eq!(simbolos[1]["containerName"], "C");
    assert!(simbolos[1].get("children").is_none());
}

#[test]
fn enum_inclui_constantes_e_membros() {
    let mut servidor = Servidor::new();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "capabilities":{"textDocument":{"documentSymbol":{
            "hierarchicalDocumentSymbolSupport":true
        }}}
    }}));
    servidor.bombear();
    let uri = "file:///enum.dart";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,
            "text":"enum Cor { azul, verde; String get texto => name; }"}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/documentSymbol",
        "params":{"textDocument":{"uri":uri}}}));
    let resposta = servidor.bombear();
    let filhos = resposta[0]["result"][0]["children"].as_array().unwrap();
    assert_eq!(filhos.len(), 3);
    assert_eq!(filhos[0]["name"], "azul");
    assert_eq!(filhos[0]["kind"], 22);
    assert_eq!(filhos[1]["name"], "verde");
    assert_eq!(filhos[2]["name"], "texto");
}

#[test]
fn busca_workspace_usa_apenas_revisoes_abertas_e_ordem_estavel() {
    let mut servidor = Servidor::new();
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
    let inicio = servidor.bombear();
    assert_eq!(inicio[0]["result"]["capabilities"]["workspaceSymbolProvider"], true);
    for (uri, texto) in [
        ("file:///z.dart", "class Caixa { void carregar() {} }"),
        ("file:///a.dart", "void carregarDados() {}"),
    ] {
        servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":texto}
        }}));
        servidor.bombear();
    }
    let buscar = |id: i32| json!({"jsonrpc":"2.0","id":id,"method":"workspace/symbol",
        "params":{"query":"ZZZ"}});
    // Consulta sem resultados e caixa indiferente.
    servidor.receber(buscar(2));
    assert!(servidor.bombear()[0]["result"].as_array().unwrap().is_empty());
    servidor.receber(json!({"jsonrpc":"2.0","id":3,"method":"workspace/symbol",
        "params":{"query":"CAR"}}));
    let resposta = servidor.bombear();
    let itens = resposta[0]["result"].as_array().unwrap();
    assert_eq!(itens.len(), 2);
    assert_eq!(itens[0]["name"], "carregarDados");
    assert_eq!(itens[0]["location"]["uri"], "file:///a.dart");
    assert_eq!(itens[1]["name"], "carregar");
    assert_eq!(itens[1]["containerName"], "Caixa");

    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{
        "textDocument":{"uri":"file:///a.dart"}
    }}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","id":4,"method":"workspace/symbol",
        "params":{"query":"car"}}));
    let resposta = servidor.bombear();
    assert_eq!(resposta[0]["result"].as_array().unwrap().len(), 1);
}
