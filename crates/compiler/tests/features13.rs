//! Regressões originais de enums, switches, const e genericos, com stdout conferido no Dart VM 3.6.2.
use dartforge_compiler::{
    CompileOptions, Optimization, compile_path_with_options, compile_with_options,
};
use std::path::PathBuf;

const SOURCE: &str = include_str!("../../../tests/conformance/cases/enhanced_enums_switch.dart");
const MAIN: &str = include_str!("../../../tests/conformance/modules/features13/main.dart");
const HELPER: &str = include_str!("../../../tests/conformance/modules/features13/helper.dart");
const GENERICS: &str = include_str!("../../../tests/conformance/cases/generics_constants.dart");
const EXPECTED: &str =
    "fast!\ninterface\nclass\nfast\n1\nsmall\nfast!\n2\nmany\nfast case\n7\n8\n31\n100\n32\n100\n";
const GENERICS_EXPECTED: &str =
    "7\ninferred\n9\nnested\nconstant\ntrue\ntrue\nfalse\nfalse\ntrue\n2\nfalse\n1\n";
const MODULE_EXPECTED: &str = "done!\ndone\nD\nimported\ntrue\n";

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
            "dartforge-features13-{}-{stamp}",
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
fn features13_compile_in_all_modes() {
    let fixture = Fixture::new();
    for options in options() {
        compile_with_options(GENERICS, options).unwrap();
        compile_with_options(SOURCE, options)
            .unwrap_or_else(|error| panic!("{options:?}: {error:?}"));
        compile_path_with_options(&fixture.0.join("main.dart"), options)
            .unwrap_or_else(|error| panic!("{options:?}: {error:?}"));
    }
}

/// Node deve reproduzir efeitos de guards, despacho de enum, inferencia generica e identidade const.
#[test]
#[ignore = "requer Node.js no PATH"]
fn features13_match_dart_3_6_2_in_all_modes() {
    let fixture = Fixture::new();
    for options in options() {
        for (source, expected) in [(SOURCE, EXPECTED), (GENERICS, GENERICS_EXPECTED)] {
            check_output(
                &compile_with_options(source, options).unwrap(),
                expected,
                options,
            );
        }
        let module = compile_path_with_options(&fixture.0.join("main.dart"), options).unwrap();
        check_output(&module, MODULE_EXPECTED, options);
    }
}

/// Const congela a lista, não apenas o binding; operações mutáveis devem falhar.
#[test]
#[ignore = "requer Node.js no PATH"]
fn const_lists_reject_runtime_mutation() {
    for options in options() {
        for mutation in ["values[0] = 9;", "values.add(9);"] {
            let source = format!(
                "void main(){{const values=<int>[1];print('before');{mutation}print('after');}}"
            );
            let javascript = compile_with_options(&source, options).unwrap();
            let output = std::process::Command::new("node")
                .args(["--input-type=module", "--eval", &javascript])
                .output()
                .expect("Node.js necessário");
            assert!(!output.status.success(), "{options:?}: {mutation}");
            assert_eq!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .replace("\r\n", "\n"),
                "before\n"
            );
        }
    }
}

/// Executa um resultado isolado para que erros de imports nao ocultem regressões locais.
fn check_output(javascript: &str, expected: &str, options: CompileOptions) {
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", javascript])
        .output()
        .expect("Node.js necessario");
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
