//! O analisador do LSP usa a versão de linguagem do arquivo
//! (`docs/VERSOES-LINGUAGEM.md` §2): marcador, senão o pacote, senão a
//! corrente. Um projeto 3.6 não pode ver erro em `final` de parâmetro, que
//! só é proibido na 3.13.
use dartforge_lsp::{Analisador, AnalisadorSintatico};

fn uri(p: &std::path::Path) -> String {
    format!("file:///{}", p.to_string_lossy().replace('\\', "/"))
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
    assert!(a.diagnosticar(&uri(&arquivo), fonte).is_empty(), "o pacote está na 3.6");
    let com_marcador = format!("// @dart=3.13\n{fonte}");
    assert_eq!(a.diagnosticar(&uri(&arquivo), &com_marcador).len(), 1, "o marcador vale mais");
    // Fora de pacote: a versão corrente (3.13).
    assert_eq!(a.diagnosticar("file:///nao/existe/b.dart", fonte).len(), 1);
    let _ = std::fs::remove_dir_all(&raiz);
}
