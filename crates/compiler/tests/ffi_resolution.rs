//! Resolução de metadados FFI por biblioteca e proteção dos backends.
use dartforge_compiler::{
    CompileOptions, Optimization, compile_path, compile_path_llvm_with_options,
};
use std::path::PathBuf;

/// Isola fontes de cada cenário e conserva os arquivos durante o diagnóstico.
struct Fixture(PathBuf);
impl Fixture {
    /// Cria um diretório exclusivo para imports e declarações de teste.
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dartforge-ffi-resolution-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    /// Grava a entrada e devolve seu caminho.
    fn source(&self, source: &str) -> PathBuf {
        let path = self.0.join("main.dart");
        std::fs::write(&path, source).unwrap();
        path
    }
}
impl Drop for Fixture {
    /// Remove exclusivamente o diretório temporário desta instância.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Bindings devem usar o prefixo importado, sem reconhecer classes locais como FFI.
#[test]
fn native_names_require_the_matching_library() {
    let fixture = Fixture::new();
    for source in [
        "@Native<Int32 Function()>() external int value(); void main(){}",
        "import 'dart:ffi' as ffi; @Native<Int32 Function()>() external int value(); void main(){}",
        "import 'dart:ffi' as ffi; @other.Native<other.Int32 Function()>() external int value(); void main(){}",
        "import 'dart:ffi'; class Native {} @Native<Int32 Function()>() external int value(); void main(){}",
        "import 'dart:ffi' as ffi; class ffi {} @ffi.Native<ffi.Int32 Function()>() external int value(); void main(){}",
        "import 'dart:ffi'; @Native<Void Function()>() external void main();",
    ] {
        let entry = fixture.source(source);
        let error = compile_path_llvm_with_options(&entry, CompileOptions::default()).unwrap_err();
        assert!(error.span.is_some(), "{source}: {error}");
    }
}

/// Dois corpos external vazios não podem ser fundidos quando os símbolos C diferem.
#[test]
fn native_bindings_survive_merge_and_keep_default_symbol() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("bindings.dart"), "import 'dart:ffi'; @Native<Int32 Function()>() external int first(); @Native<Int32 Function()>() external int second();").unwrap();
    let entry =
        fixture.source("import 'bindings.dart'; void main(){print(first());print(second());}");
    let ir = compile_path_llvm_with_options(
        &entry,
        CompileOptions {
            merge_identical_functions: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(ir.contains("declare i32 @first()"));
    assert!(ir.contains("declare i32 @second()"));
    assert!(compile_path(&entry, Optimization::None).is_err());
}

/// O perfil JS ignora completamente a alternativa que importa FFI nativo.
#[test]
fn inactive_native_library_does_not_block_javascript() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("stub.dart"), "int value()=>42;").unwrap();
    std::fs::write(
        fixture.0.join("native.dart"),
        "import 'dart:ffi'; @Native<Int32 Function()>() external int value();",
    )
    .unwrap();
    let entry = fixture.source(
        "import 'stub.dart' if (dart.library.ffi) 'native.dart'; void main(){print(value());}",
    );
    assert!(compile_path(&entry, Optimization::None).is_ok());
    assert!(
        compile_path_llvm_with_options(&entry, CompileOptions::default())
            .unwrap()
            .contains("declare i32 @value()")
    );
}

/// A resolução entre bibliotecas não deve confundir membros e funções com metadata core.
#[test]
fn imported_metadata_shadowing_is_diagnosed() {
    let fixture = Fixture::new();
    for (library, entry_source) in [
        (
            "int Deprecated(int n)=>n;",
            "import 'other.dart'; @Deprecated('old') int value()=>1; void main(){}",
        ),
        (
            "class Base { int override=1; }",
            "import 'other.dart'; class Child extends Base { @override int value()=>1; } void main(){}",
        ),
    ] {
        std::fs::write(fixture.0.join("other.dart"), library).unwrap();
        let entry = fixture.source(entry_source);
        let error = compile_path(&entry, Optimization::None).unwrap_err();
        assert!(error.message.contains("anotação sombreada"), "{error}");
        assert!(error.span.is_some());
    }
}
