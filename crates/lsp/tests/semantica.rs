use dartforge_elements::sdk::SdkLayout;
use dartforge_lsp::{Analisador, AnalisadorSemantico, Servidor};
use serde_json::{Value, json};
use std::{fs, path::Path};
use std::time::Instant;

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
    // Vários buffers abertos exercitam a geração conjunta sem arquivo em
    // disco. O custo transitório pode subir, mas o estado vivo deve estabilizar.
    let corpo = format!("// {}\n", ".".repeat(16 * 1024));
    for n in 0..24 {
        let outro = url::Url::from_file_path(raiz.join(format!("aberto_{n}.dart"))).unwrap().to_string();
        servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":outro,"languageId":"dart","version":1,"text":corpo}
        }}));
        servidor.bombear();
    }
    for id in 10..20 {
        assert_ne!(requisitar(&mut servidor, id, "textDocument/hover", &uri, 10), Value::Null);
    }
    let vivos_antes = dartforge_instrument::live_bytes();
    let inicio = Instant::now();
    for id in 20..60 {
        assert_ne!(requisitar(&mut servidor, id, "textDocument/hover", &uri, 10), Value::Null);
    }
    let latencia_media = inicio.elapsed() / 40;
    let crescimento = dartforge_instrument::live_bytes().saturating_sub(vivos_antes);
    println!("LSP semântico: 25 buffers (~384 KiB extra), 40 hovers, média {latencia_media:?}, crescimento vivo {crescimento} bytes");
    assert!(crescimento < 256 * 1024, "consultas retiveram {crescimento} bytes");
    assert!(latencia_media.as_millis() < 200, "hover médio em 25 buffers: {latencia_media:?}");
    // O importado também está aberto e ainda não foi salvo. A posição e o
    // tipo vêm da versão em memória, não da versão antiga no disco.
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":destino,"languageId":"dart","version":1,
            "text":"// 👭\n// nova linha\nString answer = 'x';\n"}
    }}));
    servidor.bombear();
    let definicao_aberta = requisitar(&mut servidor, 60, "textDocument/definition", &uri, 10);
    assert_eq!(definicao_aberta["range"]["start"], json!({"line":2,"character":7}));
    assert_eq!(requisitar(&mut servidor, 61, "textDocument/hover", &uri, 10)["contents"], "String answer\nType: String");
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":destino}}}));
    servidor.bombear();
    assert_eq!(requisitar(&mut servidor, 62, "textDocument/definition", &uri, 10)["range"]["start"], json!({"line":1,"character":4}));

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

    struct DestinoAusente(String);
    impl Analisador for DestinoAusente {
        fn diagnosticar(&mut self, _: &str, _: &str) -> Vec<dartforge_diagnostics::Diagnostic> { Vec::new() }
        fn definicao(&mut self, _: &str, _: &str, _: usize) -> Option<(String, Option<dartforge_diagnostics::Span>)> {
            Some((self.0.clone(), Some(dartforge_diagnostics::Span { start: 10, end: 16 })))
        }
    }
    let ausente = url::Url::from_file_path(raiz.join("fantasma.dart")).unwrap().to_string();
    let mut sem_arquivo = Servidor::com_analisador(DestinoAusente(ausente));
    sem_arquivo.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":texto}
    }}));
    sem_arquivo.bombear();
    sem_arquivo.receber(json!({"jsonrpc":"2.0","id":80,"method":"textDocument/definition","params":{
        "textDocument":{"uri":uri},"position":{"line":1,"character":10}
    }}));
    assert_eq!(sem_arquivo.bombear()[0]["result"], Value::Null);

    let biblioteca_fn = raiz.join("funcao.dart");
    let entrada_fn = raiz.join("main_func.dart");
    fs::write(&biblioteca_fn, "int soma(int a, int b) => a + b;\n").unwrap();
    fs::write(&entrada_fn, "import 'funcao.dart';\nvar y = 0;\n").unwrap();
    let uri_fn = url::Url::from_file_path(&entrada_fn).unwrap().to_string();
    let destino_fn = url::Url::from_file_path(&biblioteca_fn).unwrap().to_string();
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri_fn,"languageId":"dart","version":1,
            "text":"import 'funcao.dart';\nvar y = soma(1, 2);\n"}
    }}));
    servidor.bombear();
    assert_eq!(requisitar(&mut servidor, 81, "textDocument/definition", &uri_fn, 9)["range"]["start"], json!({"line":0,"character":4}));
    assert_eq!(requisitar(&mut servidor, 82, "textDocument/hover", &uri_fn, 9)["contents"], "int soma(int a, int b)");
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":destino_fn,"languageId":"dart","version":1,
            "text":"// nova linha\nString soma(String a) => a;\n"}
    }}));
    servidor.bombear();
    assert_eq!(requisitar(&mut servidor, 83, "textDocument/definition", &uri_fn, 9)["range"]["start"], json!({"line":1,"character":7}));
    assert_eq!(requisitar(&mut servidor, 84, "textDocument/hover", &uri_fn, 9)["contents"], "String soma(String a)");
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":destino_fn,"version":2},
        "contentChanges":[{"text":"int soma([int a = 0]) => a;\n"}]
    }}));
    servidor.bombear();
    assert_eq!(requisitar(&mut servidor, 85, "textDocument/definition", &uri_fn, 9)["range"]["start"], json!({"line":0,"character":4}));
    assert_eq!(requisitar(&mut servidor, 86, "textDocument/hover", &uri_fn, 9), Value::Null);

    let biblioteca_getter = raiz.join("getter.dart");
    let entrada_getter = raiz.join("main_getter.dart");
    fs::write(&biblioteca_getter, "int get resposta => 42;\n").unwrap();
    fs::write(&entrada_getter, "import 'getter.dart';\nvar y = resposta;\n").unwrap();
    let uri_getter = url::Url::from_file_path(&entrada_getter).unwrap().to_string();
    assert_eq!(requisitar(&mut servidor, 87, "textDocument/definition", &uri_getter, 9), Value::Null);
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri_getter,"languageId":"dart","version":1,
            "text":"import 'getter.dart';\nvar y = resposta;\n"}
    }}));
    servidor.bombear();
    assert_eq!(requisitar(&mut servidor, 88, "textDocument/definition", &uri_getter, 9)["range"]["start"], json!({"line":0,"character":8}));
    assert_eq!(requisitar(&mut servidor, 89, "textDocument/hover", &uri_getter, 9)["contents"], "int get resposta\nType: int");
    fs::remove_dir_all(&raiz).unwrap();
}
