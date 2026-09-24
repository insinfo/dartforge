//! O analisador do LSP usa a versão de linguagem do arquivo
//! (`docs/VERSOES-LINGUAGEM.md` §2): marcador, senão o pacote, senão a
//! corrente. Um projeto 3.6 não pode ver erro em `final` de parâmetro, que
//! só é proibido na 3.13.
use dartforge_lsp::{Analisador, AnalisadorSintatico, Servidor};
use serde_json::json;

fn uri(p: &std::path::Path) -> String {
    url::Url::from_file_path(p).unwrap().to_string()
}

#[test]
fn versao_do_pacote_com_uri_percent_encodada() {
    let raiz = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/tmp-agent/lsp-versao-{}-acentuação", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(raiz.join(".dart_tool")).unwrap();
    std::fs::create_dir_all(raiz.join("lib")).unwrap();
    std::fs::write(
        raiz.join(".dart_tool/package_config.json"),
        r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":"lib/","languageVersion":"3.6"}]}"#,
    ).unwrap();
    let arquivo = raiz.join("lib/ação.dart");
    let fonte = "void f(final int x) {}";
    std::fs::write(&arquivo, fonte).unwrap();
    let mut a = AnalisadorSintatico::new();
    assert!(a.diagnosticar(&uri(&arquivo), fonte).is_empty());
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn versao_do_pacote_e_do_marcador() {
    let raiz = std::env::temp_dir().join(format!("dartforge-lsp-versao-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(raiz.join(".dart_tool")).unwrap();
    std::fs::create_dir_all(raiz.join("lib")).unwrap();
    std::fs::write(
        raiz.join(".dart_tool/package_config.json"),
        r#"{ "configVersion": 2, "packages": [
            { "name": "app", "rootUri": "../", "packageUri": "lib/", "languageVersion": "3.6" }
        ] }"#,
    )
    .unwrap();
    let arquivo = raiz.join("lib/a.dart");
    let fonte = "void f(final int x) {}";
    std::fs::write(&arquivo, fonte).unwrap();
    let mut a = AnalisadorSintatico::new();
    assert!(
        a.diagnosticar(&uri(&arquivo), fonte).is_empty(),
        "o pacote está na 3.6"
    );
    let com_marcador = format!("// @dart=3.13\n{fonte}");
    assert_eq!(
        a.diagnosticar(&uri(&arquivo), &com_marcador).len(),
        1,
        "o marcador vale mais"
    );
    // Fora de pacote: a versão corrente (3.13).
    assert_eq!(a.diagnosticar("file:///nao/existe/b.dart", fonte).len(), 1);
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn fechar_e_reabrir_rele_configuracao_de_pacotes() {
    let raiz = std::env::temp_dir().join(format!("dartforge-lsp-reabrir-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(raiz.join(".dart_tool")).unwrap();
    std::fs::create_dir_all(raiz.join("lib")).unwrap();
    let config = raiz.join(".dart_tool/package_config.json");
    let gravar = |versao: &str| {
        std::fs::write(&config, format!(
            "{{\"configVersion\":2,\"packages\":[{{\"name\":\"app\",\"rootUri\":\"../\",\"packageUri\":\"lib/\",\"languageVersion\":\"{versao}\"}}]}}"
        )).unwrap();
    };
    gravar("3.6");
    let arquivo = raiz.join("lib/a.dart");
    let fonte = "void f(final int x) {}";
    std::fs::write(&arquivo, fonte).unwrap();
    let uri = uri(&arquivo);
    let mut servidor = Servidor::new();
    let abrir = |servidor: &mut Servidor, versao: i32| {
        servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri.clone(),"version":versao,"languageId":"dart","text":fonte}
        }}));
        servidor.bombear()
    };
    assert_eq!(abrir(&mut servidor, 1)[0]["params"]["diagnostics"].as_array().unwrap().len(), 0);
    servidor.receber(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{
        "textDocument":{"uri":uri.clone()}
    }}));
    servidor.bombear();
    gravar("3.13");
    assert_eq!(abrir(&mut servidor, 2)[0]["params"]["diagnostics"].as_array().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&raiz);
}
