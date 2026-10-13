//! The LSP analyzer uses the file's language version
//! (`docs/VERSOES-LINGUAGEM.md` §2): the marker, else the package, else the
//! current one. A 3.6 project must not see an error on a `final` parameter,
//! which is only forbidden in 3.13.
use dartforge_lsp::{Analyzer, SyntacticAnalyzer, Server};
use serde_json::json;

fn uri(p: &std::path::Path) -> String {
    url::Url::from_file_path(p).unwrap().to_string()
}

#[test]
fn package_version_with_percent_encoded_uri() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/tmp-agent/lsp-versao-{}-acentuação", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join(".dart_tool")).unwrap();
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::write(
        root.join(".dart_tool/package_config.json"),
        r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":"lib/","languageVersion":"3.6"}]}"#,
    ).unwrap();
    let file = root.join("lib/ação.dart");
    let source = "void f(final int x) {}";
    std::fs::write(&file, source).unwrap();
    let mut a = SyntacticAnalyzer::new();
    assert!(a.diagnose(&uri(&file), source).is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn package_and_marker_version() {
    let root = std::env::temp_dir().join(format!("dartforge-lsp-versao-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join(".dart_tool")).unwrap();
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::write(
        root.join(".dart_tool/package_config.json"),
        r#"{ "configVersion": 2, "packages": [
            { "name": "app", "rootUri": "../", "packageUri": "lib/", "languageVersion": "3.6" }
        ] }"#,
    )
    .unwrap();
    let file = root.join("lib/a.dart");
    let source = "void f(final int x) {}";
    std::fs::write(&file, source).unwrap();
    let mut a = SyntacticAnalyzer::new();
    assert!(
        a.diagnose(&uri(&file), source).is_empty(),
        "o pacote está na 3.6"
    );
    let with_marker = format!("// @dart=3.13\n{source}");
    assert_eq!(
        a.diagnose(&uri(&file), &with_marker).len(),
        1,
        "o marcador vale mais"
    );
    // Outside a package: the current version (3.13).
    assert_eq!(a.diagnose("file:///nao/existe/b.dart", source).len(), 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn close_and_reopen_rereads_package_config() {
    let root = std::env::temp_dir().join(format!("dartforge-lsp-reabrir-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join(".dart_tool")).unwrap();
    std::fs::create_dir_all(root.join("lib")).unwrap();
    let config = root.join(".dart_tool/package_config.json");
    let write = |version: &str| {
        std::fs::write(&config, format!(
            "{{\"configVersion\":2,\"packages\":[{{\"name\":\"app\",\"rootUri\":\"../\",\"packageUri\":\"lib/\",\"languageVersion\":\"{version}\"}}]}}"
        )).unwrap();
    };
    write("3.6");
    let file = root.join("lib/a.dart");
    let source = "void f(final int x) {}";
    std::fs::write(&file, source).unwrap();
    let uri = uri(&file);
    let mut server = Server::new();
    let open = |server: &mut Server, version: i32| {
        server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri.clone(),"version":version,"languageId":"dart","text":source}
        }}));
        server.pump()
    };
    // Without errors, nothing is published (as in the Dart server).
    assert!(open(&mut server, 1).iter().all(|m| m["method"] != "textDocument/publishDiagnostics"));
    server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{
        "textDocument":{"uri":uri.clone()}
    }}));
    server.pump();
    write("3.13");
    let outputs = open(&mut server, 2);
    let publication = outputs.iter().find(|m| m["method"] == "textDocument/publishDiagnostics").expect("publicação");
    assert_eq!(publication["params"]["diagnostics"].as_array().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&root);
}
