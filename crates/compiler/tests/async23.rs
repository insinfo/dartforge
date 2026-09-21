//! Corpus diferencial async baseado no Dart SDK 3.6.2, sem then/periodic/try-catch.
use dartforge_compiler::{
    CompileOptions, Optimization, compile_path_with_options, compile_with_options,
};

/// Exercita independentemente folding, fusão de funções e remoção de declarações mortas.
fn modes() -> impl Iterator<Item = CompileOptions> {
    [Optimization::None, Optimization::Constants]
        .into_iter()
        .flat_map(|optimization| {
            [false, true]
                .into_iter()
                .flat_map(move |merge_identical_functions| {
                    [false, true]
                        .into_iter()
                        .map(move |tree_shaking| CompileOptions {
                            optimization,
                            merge_identical_functions,
                            tree_shaking,
                        })
                })
        })
}

/// Localiza o módulo que importa dart:async e mantém a saída do oráculo ao lado da fonte.
fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/modules/async23/main.dart")
}

/// A análise completa deve funcionar em todos os modos antes de executar o JavaScript.
#[test]
fn async_corpus_compiles_in_all_modes() {
    for options in modes() {
        compile_path_with_options(&fixture(), options).unwrap();
    }
}

/// Compara microtasks, timers, adoção, reificação e capturas com o SDK instalado.
#[test]
#[ignore = "requer Node.js no PATH"]
fn async_order_and_nested_futures_match_dart() {
    let expected = include_str!("../../../tests/conformance/modules/async23/main.stdout")
        .replace("\r\n", "\n");
    for options in modes() {
        let javascript = compile_path_with_options(&fixture(), options).unwrap();
        let output = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &javascript])
            .output()
            .unwrap();
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

/// Await precisa do contexto async; o tipo de retorno e imports continuam sendo validados.
#[test]
fn invalid_async_contexts_and_missing_imports_are_diagnosed() {
    for options in modes() {
        for source in [
            "void main(){await Future<int>.value(1);}",
            "void main(){var f=()=>await Future<int>.value(1);}",
            "Future<int> bad() async{return 'wrong';}void main(){}",
            "Future<int> bad() async{return;}void main(){}",
            "Future<int> bad(){return 1;}void main(){}",
            "void main(){Timer(Duration.zero,()=>print(1));}",
            "void main(){scheduleMicrotask(()=>print(1));}",
            "void main(){Timer? t=null;}",
            "Future<void> main() async*{}",
            "void main(){Future<int>.error(1);}",
        ] {
            let error = compile_with_options(source, options).unwrap_err();
            assert!(
                error.span.start <= error.span.end && error.span.end <= source.len(),
                "{source}: {error:?}"
            );
        }
    }
}

/// LLVM precisa recusar async mesmo em funções sem chamadas ou ramos não executados.
#[test]
fn llvm_rejects_async_explicitly() {
    for source in [
        "void main() async{}",
        "Future<int> unused() async=>1;void main(){}",
        "void main(){if(false){Future<int>.value(1);}}",
    ] {
        let error = dartforge_compiler::compile_llvm(source).unwrap_err();
        assert!(error.message.contains("LLVM"), "{source}: {error:?}");
    }
}

/// Erros de uma função async chegam à entrada e impedem instruções posteriores ao await.
#[test]
#[ignore = "requer Node.js no PATH"]
fn async_failure_rejects_entry_future() {
    let source = "Future<int> fail() async{print('before');return 'wrong' as int;}Future<void> main() async{await fail();print('after');}";
    for options in modes() {
        let javascript = compile_with_options(source, options).unwrap();
        let output = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &javascript])
            .output()
            .unwrap();
        assert!(!output.status.success(), "{options:?}");
        assert_eq!(
            String::from_utf8(output.stdout)
                .unwrap()
                .replace("\r\n", "\n"),
            "before\n"
        );
    }
}
