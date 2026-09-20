//! Regressões independentes do linker com arquivos reais e diagnósticos locais.
use dartforge_compiler::{Optimization, compile_path};
use std::path::PathBuf;

/// Grava uma biblioteca de teste isolada no diretório target do workspace.
fn fixture(case: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/linker-review-tests")
        .join(format!("{}-{case}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    for (name, source) in files {
        std::fs::write(root.join(name), source).unwrap();
    }
    root.join("main.dart")
}

/// Verifica que erros em dependências mantêm arquivo e intervalo UTF-8 locais.
#[test]
fn dependency_error_keeps_its_original_location() {
    let bad = "int broken(){ print('olá'); return true; }";
    let entry = fixture(
        "location",
        &[
            (
                "main.dart",
                "import 'dep.dart'; void main(){print(broken());}",
            ),
            ("dep.dart", bad),
        ],
    );
    let error = compile_path(&entry, Optimization::None).unwrap_err();
    assert_eq!(error.path.file_name().unwrap(), "dep.dart");
    let span = error.span.unwrap();
    assert!(bad.get(span.start..span.end).is_some());
    assert!(span.end <= bad.len());
}

/// Mantém a restrição final e a privacidade dos membros importados.
#[test]
fn imported_fields_remain_private_and_final() {
    for (case, body) in [
        ("private", "print(C()._secret);"),
        ("final", "var c=C(); c.value=2;"),
    ] {
        let entry = fixture(
            case,
            &[
                (
                    "main.dart",
                    &format!("import 'dep.dart'; void main(){{{body}}}"),
                ),
                ("dep.dart", "class C { int _secret=1; final int value=1; }"),
            ],
        );
        let error = compile_path(&entry, Optimization::Constants).unwrap_err();
        assert_eq!(error.path.file_name().unwrap(), "main.dart");
    }
}

/// Rejeita ciclos nominais entre bibliotecas antes da emissão.
#[test]
fn cross_library_inheritance_cycle_is_diagnostic() {
    let entry = fixture(
        "cycle",
        &[
            (
                "main.dart",
                "import 'dep.dart'; class A extends B{} void main(){}",
            ),
            ("dep.dart", "import 'main.dart'; class B extends A{}"),
        ],
    );
    let error = compile_path(&entry, Optimization::None).unwrap_err();
    assert!(error.message.to_lowercase().contains("cycle"));
}

/// Privados homônimos em subclasses de outra biblioteca são campos distintos em Dart.
#[test]
#[ignore = "requer Node.js no PATH"]
fn inherited_private_names_are_library_scoped() {
    let entry = fixture(
        "private-inheritance",
        &[
            (
                "main.dart",
                "import 'base.dart'; class Child extends Base { int _value=20; int child(){return this._value;} } void main(){var item=Child();print(item.readBase());print(item.child());}",
            ),
            (
                "base.dart",
                "class Base { final int _value=10; int readBase(){return this._value;} }",
            ),
        ],
    );
    for optimization in [Optimization::None, Optimization::Constants] {
        let js = compile_path(&entry, optimization).unwrap();
        let output = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &js])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"10\n20\n");
    }
}
