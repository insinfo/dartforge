use dartforge_elements::sdk::SdkLayout;
use dartforge_lsp::{AnalisadorSemantico, Servidor};
use serde_json::{Value, json};
use std::{fs, path::Path};

#[global_allocator]
static ALOCADOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

fn sdk(raiz: &Path) -> SdkLayout {
    let lib = raiz.join("sdk/lib");
    fs::create_dir_all(lib.join("core")).unwrap();
    fs::write(lib.join("libraries.json"), r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#).unwrap();
    fs::write(lib.join("core/core.dart"), "library dart.core; class Object {} class num extends Object {} class int extends num {} class String extends Object {} class bool extends Object {} class Null extends Object {}").unwrap();
    SdkLayout::load(&lib, "dartdevc").unwrap()
}

fn requisitar(servidor: &mut Servidor<AnalisadorSemantico>, id: i32, metodo: &str, uri: &str, coluna: u32) -> Value {
    servidor.receber(json!({"jsonrpc":"2.0","id":id,"method":metodo,"params":{
        "textDocument":{"uri":uri},"position":{"line":1,"character":coluna}
    }}));
    servidor.bombear()[0]["result"].clone()
}

#[test]
fn importado_usa_tipos_e_texto_vigente_sem_reter_versoes() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/tmp-agent/lsp-semantica-{}", std::process::id()));
    let _ = fs::remove_dir_all(&raiz);
    fs::create_dir_all(&raiz).unwrap();
    let raiz = fs::canonicalize(raiz).unwrap();
    let sdk = sdk(&raiz);
    let biblioteca = raiz.join("lib.dart");
    let entrada = raiz.join("main.dart");
    fs::write(&biblioteca, "// 👭\nint answer = 42;\n").unwrap();
    fs::write(&entrada, "import 'lib.dart';\nvar x = 0;\n").unwrap();
    let uri = url::Url::from_file_path(&entrada).unwrap().to_string();
    let destino = url::Url::from_file_path(&biblioteca).unwrap().to_string();
    let mut servidor = Servidor::com_analisador(AnalisadorSemantico::novo(Some(sdk)));
    servidor.receber(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
    servidor.bombear();
    let texto = "import 'lib.dart';\nvar x = answer;\n";
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":texto}
    }}));
    servidor.bombear();

    let definicao = requisitar(&mut servidor, 2, "textDocument/definition", &uri, 10);
    assert_eq!(definicao["uri"], destino);
    assert_eq!(definicao["range"]["start"], json!({"line":1,"character":4}));
    assert_eq!(definicao["range"]["end"], json!({"line":1,"character":10}));
    let hover = requisitar(&mut servidor, 3, "textDocument/hover", &uri, 10);
    assert_eq!(hover["contents"], "int answer\nType: int");
    for id in 10..20 {
        assert_ne!(requisitar(&mut servidor, id, "textDocument/hover", &uri, 10), Value::Null);
    }
    let vivos_antes = dartforge_instrument::live_bytes();
    for id in 20..60 {
        assert_ne!(requisitar(&mut servidor, id, "textDocument/hover", &uri, 10), Value::Null);
    }
    let crescimento = dartforge_instrument::live_bytes().saturating_sub(vivos_antes);
    assert!(crescimento < 256 * 1024, "consultas retiveram {crescimento} bytes");

    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":"import 'lib.dart';\nvar x = missing;\n"}]
    }}));
    servidor.bombear();
    assert_eq!(requisitar(&mut servidor, 4, "textDocument/hover", &uri, 10), Value::Null);
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":uri,"version":1},"contentChanges":[{"text":texto}]
    }}));
    assert!(servidor.bombear().is_empty());
    assert_eq!(requisitar(&mut servidor, 5, "textDocument/hover", &uri, 10), Value::Null);
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":uri,"version":3},"contentChanges":[{"text":"import 'lib.dart';\nvoid f(int answer) => answer;\n"}]
    }}));
    servidor.bombear();
    assert_eq!(requisitar(&mut servidor, 7, "textDocument/hover", &uri, 24), Value::Null);
    assert_eq!(requisitar(&mut servidor, 8, "textDocument/definition", &uri, 24), Value::Null);

    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":uri}}}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":texto}
    }}));
    servidor.bombear();
    assert_ne!(requisitar(&mut servidor, 6, "textDocument/hover", &uri, 10), Value::Null);
    fs::remove_dir_all(&raiz).unwrap();
}
