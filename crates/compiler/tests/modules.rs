//! Contrato público da compilação de arquivos e dos diagnósticos por biblioteca.
use dartforge_compiler::{Optimization, compile_path};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Diretório exclusivo de teste, removido somente depois de todas as leituras.
struct Fixture(PathBuf);
impl Fixture {
    /// Cria arquivos mínimos sem depender do diretório atual ou do SDK instalado.
    fn new(files: &[(&str, &str)]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "dartforge-module-api-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        for (name, text) in files {
            std::fs::write(path.join(name), text).unwrap();
        }
        Self(path)
    }
    /// Devolve a entrada da fixture.
    fn entry(&self) -> PathBuf {
        self.0.join("main.dart")
    }
}
impl Drop for Fixture {
    /// Remove apenas o diretório temporário exclusivo criado pela própria fixture.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// O erro de uma biblioteca deve manter seu arquivo e bytes originais, inclusive UTF-8.
#[test]
fn imported_diagnostic_keeps_source_file_and_local_span() {
    let source = "int answer() { return 'ação'; }";
    let fixture = Fixture::new(&[
        (
            "main.dart",
            "import 'lib.dart'; void main() { print(answer()); }",
        ),
        ("lib.dart", source),
    ]);
    let error = compile_path(&fixture.entry(), Optimization::None).unwrap_err();
    assert_eq!(
        error.path.file_name(),
        Some(std::ffi::OsStr::new("lib.dart"))
    );
    let span = error.span.expect("erro semântico tem posição");
    assert!(span.start <= span.end && span.end <= source.len());
    assert!(source.is_char_boundary(span.start) && source.is_char_boundary(span.end));
}

/// Nomes importados não transitivos nunca devem vazar para uma unidade intermediária.
#[test]
fn transitive_import_does_not_leak_symbols() {
    let fixture = Fixture::new(&[
        (
            "main.dart",
            "import 'middle.dart'; void main() { print(hidden()); }",
        ),
        (
            "middle.dart",
            "import 'leaf.dart'; int publicValue() { return hidden(); }",
        ),
        ("leaf.dart", "int hidden() { return 42; }"),
    ]);
    assert!(compile_path(&fixture.entry(), Optimization::None).is_err());
}

/// O caminho sem imports também informa o arquivo em erros de compilação.
#[test]
fn standalone_error_is_located() {
    let fixture = Fixture::new(&[("main.dart", "void main() { print(unknown); }")]);
    let error = compile_path(&fixture.entry(), Optimization::Constants).unwrap_err();
    assert_eq!(error.path.file_name(), Path::new("main.dart").file_name());
    assert!(error.span.is_some());
}
