//! Records estruturais, efeitos e desestruturação entre bibliotecas Dart 3.6.2.
use dartforge_compiler::{
    CompileOptions, Optimization, compile, compile_llvm, compile_path_with_options,
};

/// Executa cada combinação de otimizações contra a saída conferida no SDK fixado.
#[test]
#[ignore = "requer Node.js no PATH"]
fn records_match_dart_in_all_modes() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/modules/records19/main.dart");
    let expected = include_str!("../../../tests/conformance/modules/records19/main.stdout")
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
}

/// Campos de records são imutáveis; variáveis declaradas por final também são.
#[test]
fn invalid_record_programs_fail_before_emission() {
    for source in [
        "void main(){var r=(1,2);r.$1=3;}",
        "void main(){final (a,b)=(1,2);a=3;}",
        "void main(){var (a,b)=(1,);}",
        "void main(){var (a,a)=(1,2);}",
        "void main(){var (a,b)=(a,2);}",
        "void main(){var r=(x:1);print(r.y);}",
        "void main(){var r=(1,);print(r.$0);}",
        "void main(){const r=(1,2);}",
    ] {
        assert!(compile(source).is_err(), "{source}");
    }
}

/// Sem lowering nativo de records, uma função não chamada também precisa de diagnóstico.
#[test]
fn llvm_rejects_records_in_dead_code() {
    for source in [
        "void main(){if(false){var r=(1,2);print(r.$1);}}",
        "(int,int) unused()=>(1,2);void main(){}",
    ] {
        let error = compile_llvm(source).unwrap_err();
        assert!(error.message.contains("LLVM"), "{error:?}");
    }
}

/// A fusão não pode redirecionar uma função para o nome ocultado por um padrão local.
#[test]
#[ignore = "requer Node.js no PATH"]
fn record_bindings_do_not_capture_merged_functions() {
    for optimization in [Optimization::None, Optimization::Constants] {
        for (source, expected) in [
            (
                "int first(int x)=>x+1;int other(int x)=>x+1;void main(){var(first,n)=(10,2);print(other(n));}",
                "3\n",
            ),
            (
                "void main(){var(first,n)=(10,2);var r=(callback:(int x)=>x+first);print(r.callback(1));}",
                "11\n",
            ),
        ] {
            let js = dartforge_compiler::compile_with_options(
                source,
                CompileOptions {
                    optimization,
                    merge_identical_functions: true,
                    tree_shaking: false,
                },
            )
            .unwrap();
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
}
