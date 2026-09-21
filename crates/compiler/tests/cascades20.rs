//! Cascatas entre bibliotecas com efeitos e identidades preservados em quatro modos.
use dartforge_compiler::{
    CompileOptions, Optimization, compile_path_with_options, compile_with_options,
};

/// Exercita independentemente folding e fusão opcional de funções.
fn modes() -> impl Iterator<Item = CompileOptions> {
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

/// Compara o corpus importado à saída produzida pelo Dart SDK 3.6.2.
#[test]
#[ignore = "requer Node.js no PATH"]
fn cascades_match_dart_in_all_modes() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/modules/cascades20/main.dart");
    let expected = include_str!("../../../tests/conformance/modules/cascades20/main.stdout")
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

/// O receptor sintético mantém a privacidade da biblioteca que escreveu a seção.
#[test]
fn imported_private_fields_are_inaccessible_in_cascades() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/modules/cascades20/private_error.dart");
    for options in modes() {
        assert!(compile_path_with_options(&path, options).is_err());
    }
}

/// Mesmo seções ignoradas por null precisam ser estaticamente válidas.
#[test]
fn invalid_sections_fail_in_all_modes() {
    for options in modes() {
        for source in [
            "class C{int x=0;}void main(){C? c=null;c..x=1;}",
            "class C{int x=0;}void main(){C? c=null;c?..x='bad';}",
            "class C{int x=0;}void main(){C? c=null;c?..missing();}",
            "class C{int x=0;}void main(){C? c=null;c?..x=1;print(c.x);}",
            "class C{final int x=0;}void main(){C()..x=1;}",
            "void main(){(1,2)..$1=3;}",
        ] {
            assert!(compile_with_options(source, options).is_err(), "{source}");
        }
    }
}

/// Escritas covariantes em cascata conservam o tipo real da lista antes de efeitos posteriores.
#[test]
#[ignore = "requer Node.js no PATH"]
fn covariant_cascade_writes_keep_runtime_checks() {
    for options in modes() {
        let js=compile_with_options("void main(){List<Object> xs=<int>[1];print('before');xs..add('bad')..add(2);print('after');}",options).unwrap();
        let output = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &js])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout)
                .unwrap()
                .replace("\r\n", "\n"),
            "before\n"
        );
    }
}

/// O backend nativo não deve ignorar cascatas em código morto ou funções não chamadas.
#[test]
fn llvm_rejects_cascades_before_emission() {
    for source in [
        "class C{int x=0;}void main(){if(false){C()..x=1;}}",
        "class C{int x=0;}C unused()=>C()..x=1;void main(){}",
    ] {
        let error = dartforge_compiler::compile_llvm(source).unwrap_err();
        assert!(error.message.contains("LLVM"), "{error:?}");
        assert!(error.message.contains("cascatas"), "{error:?}");
    }
}

/// Recursos modernos ainda pendentes conservam diagnósticos até terem implementação própria.
#[test]
fn pending_language_features_remain_diagnosed() {
    // A lista primária exige tipo: `this.campo` precisa do campo no corpo.
    assert!(
        dartforge_compiler::compile("class Cliente(this.nome, this.email);void main(){}").is_err()
    );
    // Parâmetros nomeados privados (Dart 3.12) passaram a ser aceitos como
    // initializing formals; ver docs/PARAMETROS.md.
    assert!(
        dartforge_compiler::compile("class C{final int _x;C({required this._x});}void main(){}")
            .is_ok()
    );
    // Implementados no incremento 25; ver docs/IMPLEMENTACAO-25.md.
    assert!(
        dartforge_compiler::compile("enum E{one}void use(E e){}void main(){use(.one);}").is_ok()
    );
    assert!(dartforge_compiler::compile("void main(){var _=1;var _=2;}").is_ok());
    assert!(
        dartforge_compiler::compile("void main(){int? x=null;var xs=[?x];print(xs.length);}")
            .is_ok()
    );
    // A partir de 3.7 `_` não declara nome algum: lê-lo é erro.
    assert!(dartforge_compiler::compile("void main(){var _=7;print(_);}").is_err());
    assert!(dartforge_compiler::compile("void main(){var (_,_,x)=(1,2,3);print(x);}").is_ok());
}
