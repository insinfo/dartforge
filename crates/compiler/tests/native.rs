//! Regressões da API LLVM e execução real de programas nativos nos dois modos.
use dartforge_compiler::{compile_llvm, compile_path_llvm};
use std::path::{Path, PathBuf};

/// Diretório exclusivo; o teste não remove caminhos fornecidos externamente.
struct OutputDir(PathBuf);
impl OutputDir {
    /// Reserva um diretório único mesmo em execuções simultâneas.
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dartforge-native-integration-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for OutputDir {
    /// Limpa somente o diretório criado pelo próprio teste.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Nenhuma otimização pode esconder recursos ainda sem runtime nativo.
#[test]
fn rejects_unsupported_native_features_even_when_unused() {
    for source in [
        "class Box { int x = 1; } void main() {}",
        "void main() { if (false) { print('unreachable'); } }",
        "void main() { int? n = null; print(n); }",
        "String unused() { return 'unused'; } void main() {}",
    ] {
        let error = compile_llvm(source).unwrap_err();
        assert!(error.message.contains("LLVM"), "{error:?}");
        assert!(error.span.end <= source.len());
    }
}

/// O erro de backend deve preservar o caminho e o intervalo da biblioteca importada.
#[test]
fn imported_backend_error_is_localized() {
    let dir = OutputDir::new();
    std::fs::write(
        dir.0.join("main.dart"),
        "import 'lib.dart'; void main() { print(value()); }",
    )
    .unwrap();
    let source = "int value() { print('unsupported'); return 1; }";
    std::fs::write(dir.0.join("lib.dart"), source).unwrap();
    let error = compile_path_llvm(&dir.0.join("main.dart")).unwrap_err();
    assert_eq!(error.path.file_name().unwrap(), "lib.dart");
    let span = error.span.unwrap();
    assert!(span.start <= span.end && span.end <= source.len());
    assert!(error.message.contains("LLVM"));
}

/// Confere resultados obtidos previamente com Dart 3.6.2; exige ferramentas reais.
#[test]
#[ignore = "requer clang LLVM 17+ e rustc/linker nativo no PATH ou DARTFORGE_CLANG/RUSTC"]
fn executes_native_corpus_at_o0_and_o2() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = OutputDir::new();
    for (index, fixture) in [
        "cases/arithmetic.dart",
        "cases/calls.dart",
        "cases/control_flow.dart",
        "cases/edge_cfg.dart",
        "modules/packages/main.dart",
    ]
    .iter()
    .enumerate()
    {
        let input = root.join("tests/native").join(fixture);
        let expected = std::fs::read_to_string(input.with_extension("stdout")).unwrap();
        let ir = compile_path_llvm(&input).unwrap();
        for optimize in [false, true] {
            let output = dir.0.join(format!(
                "case-{index}-{optimize}{}",
                std::env::consts::EXE_SUFFIX
            ));
            let options = dartforge_native::NativeOptions {
                optimize,
                ..Default::default()
            };
            dartforge_native::build_executable(&ir, &output, &options).unwrap();
            let actual = std::process::Command::new(&output).output().unwrap();
            assert!(actual.status.success(), "{fixture}: {:?}", actual);
            assert_eq!(
                String::from_utf8(actual.stdout)
                    .unwrap()
                    .replace("\r\n", "\n"),
                expected.replace("\r\n", "\n"),
                "{fixture}, optimize={optimize}"
            );
        }
    }
}
