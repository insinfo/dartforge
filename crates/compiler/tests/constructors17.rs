//! Construtores, this e escopos através de bibliotecas; resultado conferido no Dart 3.6.2.
use dartforge_compiler::{CompileOptions, Optimization, compile_path_with_options};

/// Compila o fixture com e sem passes, incluindo remapeamento de campos privados.
#[test]
fn constructors_and_scopes_compile_in_all_modes() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/modules/constructors17/main.dart");
    for optimization in [Optimization::None, Optimization::Constants] {
        for merge_identical_functions in [false, true] {
            compile_path_with_options(
                &path,
                CompileOptions {
                    optimization,
                    merge_identical_functions,
                    tree_shaking: false,
                },
            )
            .unwrap();
        }
    }
}

/// A execução JS precisa conservar avaliação, despacho durante a construção e sombreamento.
#[test]
#[ignore = "requer Node.js no PATH"]
fn constructors_and_scopes_match_dart_in_all_modes() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/modules/constructors17/main.dart");
    let expected = include_str!("../../../tests/native/modules/constructors17/main.stdout")
        .replace("\r\n", "\n");
    for optimization in [Optimization::None, Optimization::Constants] {
        for merge_identical_functions in [false, true] {
            let js = compile_path_with_options(
                &path,
                CompileOptions {
                    optimization,
                    merge_identical_functions,
                    tree_shaking: false,
                },
            )
            .unwrap();
            let result = std::process::Command::new("node")
                .args(["--input-type=module", "--eval", &js])
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                String::from_utf8(result.stdout)
                    .unwrap()
                    .replace("\r\n", "\n"),
                expected
            );
        }
    }
}
