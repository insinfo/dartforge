//! Genéricos reificados e limites anuláveis comparados com o SDK Dart 3.6.2.
use dartforge_compiler::{
    CompileOptions, Optimization, compile_llvm, compile_path_with_options, compile_with_options,
};

/// Exercita cada combinação dos passes sem compartilhar estado entre compilações.
fn modes() -> impl Iterator<Item = CompileOptions> {
    [Optimization::None, Optimization::Constants]
        .into_iter()
        .flat_map(|optimization| {
            [false, true]
                .into_iter()
                .map(move |merge_identical_functions| CompileOptions {
                    optimization,
                    merge_identical_functions,
                })
        })
}

/// Verifica remapeamento de limites, argumentos simbólicos e capturas entre bibliotecas.
#[test]
#[ignore = "requer Node.js no PATH"]
fn reified_generics_match_dart_in_all_modes() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/modules/reified18/main.dart");
    let expected = include_str!("../../../tests/conformance/modules/reified18/main.stdout")
        .replace("\r\n", "\n");
    for options in modes() {
        let js = compile_path_with_options(&path, options).unwrap();
        let output = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &js])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout)
                .unwrap()
                .replace("\r\n", "\n"),
            expected
        );
    }
}

/// Casts inválidos e escritas covariantes devem falhar antes do efeito seguinte.
#[test]
#[ignore = "requer Node.js no PATH"]
fn runtime_checks_preserve_list_element_types() {
    for options in modes() {
        for source in [
            "T checked<T>(Object? x) => x as T; void main(){print('before');checked<int>('bad');print('after');}",
            "void main(){List<Object> values=<int>[1];print('before');values.add('bad');print('after');}",
            "void main(){List<Object> values=<int>[1];print('before');values[0]='bad';print('after');}",
        ] {
            let js = compile_with_options(source, options).unwrap();
            let output = std::process::Command::new("node")
                .args(["--input-type=module", "--eval", &js])
                .output()
                .unwrap();
            assert!(!output.status.success(), "{source}");
            assert_eq!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .replace("\r\n", "\n"),
                "before\n"
            );
        }
    }
}

/// LLVM mantém diagnóstico explícito, inclusive para testes em código não executado.
#[test]
fn native_reified_operations_are_explicitly_unsupported() {
    for source in [
        "void main(){print(1 is int);}",
        "void main(){if(false){print(1 as int);}}",
        "T id<T extends Object>(T x)=>x;void main(){print(id<int>(1));}",
    ] {
        let error = compile_llvm(source).expect_err("LLVM não deve apagar testes de tipo");
        assert!(error.message.contains("LLVM"), "{error:?}");
    }
}
