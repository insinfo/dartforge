use dartforge_elements::sdk::SdkLayout;
use dartforge_lsp::{Analyzer, SemanticAnalyzer, Server};
use serde_json::{Value, json};
use std::{fs, path::Path};
use std::time::Instant;

#[global_allocator]
static ALLOCATOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

fn sdk(root: &Path) -> SdkLayout {
    let lib = root.join("sdk/lib");
    fs::create_dir_all(lib.join("core")).unwrap();
    fs::write(lib.join("libraries.json"), r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#).unwrap();
    fs::write(lib.join("core/core.dart"), "library dart.core; class Object {} class num extends Object {} class int extends num {} class String extends Object {} class bool extends Object {} class Null extends Object {}").unwrap();
    SdkLayout::load(&lib, "dartdevc").unwrap()
}

fn request(server: &mut Server<SemanticAnalyzer>, id: i32, method: &str, uri: &str, column: u32) -> Value {
    server.receive(json!({"jsonrpc":"2.0","id":id,"method":method,"params":{
        "textDocument":{"uri":uri},"position":{"line":1,"character":column}
    }}));
    server.pump()[0]["result"].clone()
}

#[test]
fn imported_uses_types_and_current_text_without_retaining_versions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/tmp-agent/lsp-semantica-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    let sdk = sdk(&root);
    let library = root.join("lib.dart");
    let entry = root.join("main.dart");
    fs::write(&library, "// 👭\nint answer = 42;\n").unwrap();
    fs::write(&entry, "import 'lib.dart';\nvar x = 0;\n").unwrap();
    let uri = url::Url::from_file_path(&entry).unwrap().to_string();
    let target = url::Url::from_file_path(&library).unwrap().to_string();
    let mut server = Server::with_analyzer(SemanticAnalyzer::new(Some(sdk)));
    server.receive(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
    server.pump();
    let text = "import 'lib.dart';\nvar x = answer;\n";
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":text}
    }}));
    server.pump();

    let definition = request(&mut server, 2, "textDocument/definition", &uri, 10);
    assert_eq!(definition["uri"], target);
    assert_eq!(definition["range"]["start"], json!({"line":1,"character":4}));
    assert_eq!(definition["range"]["end"], json!({"line":1,"character":10}));
    let hover = request(&mut server, 3, "textDocument/hover", &uri, 10);
    assert_eq!(hover["contents"], "int answer\nType: int");
    // Several open buffers exercise the joint generation without a file on disk.
    // The transient cost may rise, but the live state must stabilize.
    let body = format!("// {}\n", ".".repeat(16 * 1024));
    for n in 0..24 {
        let other = url::Url::from_file_path(root.join(format!("aberto_{n}.dart"))).unwrap().to_string();
        server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":other,"languageId":"dart","version":1,"text":body}
        }}));
        server.pump();
    }
    for id in 10..20 {
        assert_ne!(request(&mut server, id, "textDocument/hover", &uri, 10), Value::Null);
    }
    // The typed analysis of the 25 open files runs in the background: the
    // measurement starts and ends with it idle, to count only what the queries
    // retain (the semantic session is already built by the previous 10).
    server.await_diagnostics(std::time::Duration::from_secs(120));
    let live_before = dartforge_instrument::live_bytes();
    let start = Instant::now();
    for id in 20..60 {
        assert_ne!(request(&mut server, id, "textDocument/hover", &uri, 10), Value::Null);
    }
    let mean_latency = start.elapsed() / 40;
    server.await_diagnostics(std::time::Duration::from_secs(120));
    let growth = dartforge_instrument::live_bytes().saturating_sub(live_before);
    println!("semantic LSP: 25 buffers (~384 KiB extra), 40 hovers, mean {mean_latency:?}, live growth {growth} bytes");
    assert!(growth < 256 * 1024, "consultas retiveram {growth} bytes");
    assert!(mean_latency.as_millis() < 200, "mean hover over 25 buffers: {mean_latency:?}");
    // The imported file is also open and not saved yet. The position and the
    // type come from the in-memory version, not from the old version on disk.
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":target,"languageId":"dart","version":1,
            "text":"// 👭\n// nova linha\nString answer = 'x';\n"}
    }}));
    server.pump();
    let open_definition = request(&mut server, 60, "textDocument/definition", &uri, 10);
    assert_eq!(open_definition["range"]["start"], json!({"line":2,"character":7}));
    assert_eq!(request(&mut server, 61, "textDocument/hover", &uri, 10)["contents"], "String answer\nType: String");
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":target}}}));
    server.pump();
    assert_eq!(request(&mut server, 62, "textDocument/definition", &uri, 10)["range"]["start"], json!({"line":1,"character":4}));

    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":"import 'lib.dart';\nvar x = missing;\n"}]
    }}));
    server.pump();
    // An undefined name: only its `InvalidType`, like `dart language-server`
    // 3.6.2.
    assert_eq!(request(&mut server, 4, "textDocument/hover", &uri, 10)["contents"], "Type: InvalidType");
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":uri,"version":1},"contentChanges":[{"text":text}]
    }}));
    assert!(server.pump().is_empty());
    // The older version is ignored: the text is still version 2's.
    assert_eq!(request(&mut server, 5, "textDocument/hover", &uri, 10)["contents"], "Type: InvalidType");
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":uri,"version":3},"contentChanges":[{"text":"import 'lib.dart';\nvoid f(int answer) => answer;\n"}]
    }}));
    server.pump();
    // The homonymous parameter shadows the imported one: hover and definition
    // are its own (before scope resolution, the answer was empty).
    assert_eq!(request(&mut server, 7, "textDocument/hover", &uri, 24)["contents"], "int answer\nType: int");
    assert_eq!(request(&mut server, 8, "textDocument/definition", &uri, 24)["range"]["start"], json!({"line":1,"character":11}));

    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":uri}}}));
    server.pump();
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":text}
    }}));
    server.pump();
    assert_ne!(request(&mut server, 6, "textDocument/hover", &uri, 10), Value::Null);

    struct MissingTarget(String);
    impl Analyzer for MissingTarget {
        fn diagnose(&mut self, _: &str, _: &str) -> Vec<dartforge_diagnostics::Diagnostic> { Vec::new() }
        fn definition(&mut self, _: &str, _: &str, _: usize) -> Option<(String, Option<dartforge_diagnostics::Span>)> {
            Some((self.0.clone(), Some(dartforge_diagnostics::Span { start: 10, end: 16 })))
        }
    }
    let missing = url::Url::from_file_path(root.join("fantasma.dart")).unwrap().to_string();
    let mut without_file = Server::with_analyzer(MissingTarget(missing));
    without_file.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":text}
    }}));
    without_file.pump();
    without_file.receive(json!({"jsonrpc":"2.0","id":80,"method":"textDocument/definition","params":{
        "textDocument":{"uri":uri},"position":{"line":1,"character":10}
    }}));
    assert_eq!(without_file.pump()[0]["result"], Value::Null);

    let library_fn = root.join("funcao.dart");
    let entry_fn = root.join("main_func.dart");
    fs::write(&library_fn, "int soma(int a, int b) => a + b;\n").unwrap();
    fs::write(&entry_fn, "import 'funcao.dart';\nvar y = 0;\n").unwrap();
    let uri_fn = url::Url::from_file_path(&entry_fn).unwrap().to_string();
    let target_fn = url::Url::from_file_path(&library_fn).unwrap().to_string();
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri_fn,"languageId":"dart","version":1,
            "text":"import 'funcao.dart';\nvar y = soma(1, 2);\n"}
    }}));
    server.pump();
    assert_eq!(request(&mut server, 81, "textDocument/definition", &uri_fn, 9)["range"]["start"], json!({"line":0,"character":4}));
    assert_eq!(request(&mut server, 82, "textDocument/hover", &uri_fn, 9)["contents"], "int soma(int a, int b)\nType: int Function(int, int)");
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":target_fn,"languageId":"dart","version":1,
            "text":"// nova linha\nString soma(String a) => a;\n"}
    }}));
    server.pump();
    assert_eq!(request(&mut server, 83, "textDocument/definition", &uri_fn, 9)["range"]["start"], json!({"line":1,"character":7}));
    assert_eq!(request(&mut server, 84, "textDocument/hover", &uri_fn, 9)["contents"], "String soma(String a)\nType: String Function(String)");
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":target_fn,"version":2},
        "contentChanges":[{"text":"int soma([int a = 0]) => a;\n"}]
    }}));
    server.pump();
    assert_eq!(request(&mut server, 85, "textDocument/definition", &uri_fn, 9)["range"]["start"], json!({"line":0,"character":4}));
    // Optional parameter with a default value: the full signature.
    assert_eq!(request(&mut server, 86, "textDocument/hover", &uri_fn, 9)["contents"], "int soma([int a = 0])\nType: int Function([int])");

    let library_getter = root.join("getter.dart");
    let entry_getter = root.join("main_getter.dart");
    fs::write(&library_getter, "int get resposta => 42;\n").unwrap();
    fs::write(&entry_getter, "import 'getter.dart';\nvar y = resposta;\n").unwrap();
    let uri_getter = url::Url::from_file_path(&entry_getter).unwrap().to_string();
    assert_eq!(request(&mut server, 87, "textDocument/definition", &uri_getter, 9), Value::Null);
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri_getter,"languageId":"dart","version":1,
            "text":"import 'getter.dart';\nvar y = resposta;\n"}
    }}));
    server.pump();
    assert_eq!(request(&mut server, 88, "textDocument/definition", &uri_getter, 9)["range"]["start"], json!({"line":0,"character":8}));
    assert_eq!(request(&mut server, 89, "textDocument/hover", &uri_getter, 9)["contents"], "int get resposta\nType: int");
    let prefixed_entry = root.join("main_prefix.dart");
    fs::write(&prefixed_entry, "import 'getter.dart' as p;\nvar y = 0;\n").unwrap();
    let prefixed_uri = url::Url::from_file_path(&prefixed_entry).unwrap().to_string();
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":prefixed_uri,"languageId":"dart","version":1,
            "text":"import 'getter.dart' as p;\nvar y = p.resposta;\n"}
    }}));
    server.pump();
    assert_eq!(request(&mut server, 90, "textDocument/definition", &prefixed_uri, 12)["range"]["start"], json!({"line":0,"character":8}));
    assert_eq!(request(&mut server, 91, "textDocument/hover", &prefixed_uri, 12)["contents"], "int get resposta\nType: int");
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":prefixed_uri,"version":2},
        "contentChanges":[{"text":"import 'getter.dart' as p;\nvar p = 0; var y = p.resposta;\n"}]
    }}));
    server.pump();
    assert_eq!(request(&mut server, 92, "textDocument/definition", &prefixed_uri, 23), Value::Null);
    assert_eq!(request(&mut server, 93, "textDocument/hover", &prefixed_uri, 23), Value::Null);
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":target_fn,"version":3},
        "contentChanges":[{"text":"int soma(int a, int b) => a + b;\n"}]
    }}));
    server.pump();
    let prefixed_function_entry = root.join("main_func_prefix.dart");
    fs::write(&prefixed_function_entry, "import 'funcao.dart' as f;\nvar y = 0;\n").unwrap();
    let prefixed_function_uri = url::Url::from_file_path(&prefixed_function_entry).unwrap().to_string();
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":prefixed_function_uri,"languageId":"dart","version":1,
            "text":"import 'funcao.dart' as f;\nvar y = f.soma(1, 2);\n"}
    }}));
    server.pump();
    assert_eq!(request(&mut server, 94, "textDocument/definition", &prefixed_function_uri, 11)["range"]["start"], json!({"line":0,"character":4}));
    assert_eq!(request(&mut server, 95, "textDocument/hover", &prefixed_function_uri, 11)["contents"], "int soma(int a, int b)\nType: int Function(int, int)");
    // The background typed analysis wrote the cache of this minimal SDK.
    server.await_diagnostics(std::time::Duration::from_secs(120));
    let layout = SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap();
    let _ = fs::remove_file(dartforge_elements::SdkCache::path(&layout, "dartdevc"));
    fs::remove_dir_all(&root).unwrap();
}
