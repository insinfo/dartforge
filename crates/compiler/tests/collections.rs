//! Regressões originais de coleções/closures, com stdout conferido no Dart VM 3.6.2.
use dartforge_compiler::{
    CompileOptions, Optimization, compile_path_with_options, compile_with_options,
};
use std::path::PathBuf;

const SOURCE: &str = include_str!("../../../tests/conformance/cases/collections_closures.dart");
const MAIN: &str = include_str!("../../../tests/conformance/modules/closures/main.dart");
const HELPER: &str = include_str!("../../../tests/conformance/modules/closures/helper.dart");
const EXPECTED: &str = "5\n5\n11\n2\n0\n0\n5\n4\n4\n50\ntrue\n7\n5\n20\n5\n0\nfalse\n0\n12\n1\n11\n12\n21\nfalse\ntrue\n8\ntrue\nfalse\n0\n1\n2\n2\n6\n";
const MODULE_EXPECTED: &str = "5\n10\n7\nfalse\ntrue\nfalse\n12\n7\n9\n";

/// Reserva apenas os arquivos do fixture modular num diretório exclusivo do teste.
struct Fixture(PathBuf);
impl Fixture {
    /// Copia fontes incorporadas para exercitar o carregador real de imports.
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dartforge-collections-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("main.dart"), MAIN).unwrap();
        std::fs::write(path.join("helper.dart"), HELPER).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    /// Remove somente o diretório temporário criado por este teste.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Exercita as duas transformações independentemente, preservando identidade observável.
fn options() -> impl Iterator<Item = CompileOptions> {
    [Optimization::None, Optimization::Constants]
        .into_iter()
        .flat_map(|optimization| {
            [false, true]
                .into_iter()
                .map(move |merge_identical_functions| CompileOptions {
                    optimization,
                    merge_identical_functions,
                    tree_shaking: false,
                })
        })
}

/// Compila os fixtures mesmo quando Node não estiver disponível no ambiente de testes.
#[test]
fn collection_and_closure_fixtures_compile_in_all_modes() {
    let fixture = Fixture::new();
    for options in options() {
        compile_with_options(SOURCE, options)
            .unwrap_or_else(|error| panic!("{options:?}: {error:?}"));
        compile_path_with_options(&fixture.0.join("main.dart"), options)
            .unwrap_or_else(|error| panic!("{options:?}: {error:?}"));
    }
}

/// Node deve reproduzir efeitos lazy, escape, captura por iteração e identidade de funções.
#[test]
#[ignore = "requer Node.js no PATH"]
fn collections_and_closures_match_dart_3_6_2_in_all_modes() {
    let fixture = Fixture::new();
    for options in options() {
        let single = compile_with_options(SOURCE, options).unwrap();
        let module = compile_path_with_options(&fixture.0.join("main.dart"), options).unwrap();
        for (javascript, expected) in [(single, EXPECTED), (module, MODULE_EXPECTED)] {
            let output = std::process::Command::new("node")
                .args(["--input-type=module", "--eval", &javascript])
                .output()
                .expect("Node.js necessário");
            assert!(
                output.status.success(),
                "{options:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .replace("\r\n", "\n"),
                expected,
                "{options:?}"
            );
        }
    }
}
