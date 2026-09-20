//! Integração original Dart → LLVM → objetos C: larguras, sinal, ordem e ligação explícita.
use dartforge_compiler::compile_path_llvm;
use dartforge_native::{NativeOptions, build_executable_with_report_and_objects};
use std::path::PathBuf;
use std::process::Command;

const SOURCE: &str = include_str!("../../../tests/ffi/main.dart");
const C_SOURCE: &str = include_str!("../../../tests/ffi/native.c");
const EXPECTED: &str = include_str!("../../../tests/ffi/main.stdout");

/// Diretório exclusivo cujo nome inclui espaços para verificar passagem literal dos caminhos.
struct Fixture(PathBuf);
impl Fixture {
    /// Reserva fontes de teste sem tocar nos exemplos compartilhados.
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("dartforge ffi {} {stamp}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("main.dart"), SOURCE).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    /// Remove somente o diretório criado nesta instância.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// O import prefixado deve chegar aos wrappers com nomes C preservados pelo linker Dart.
#[test]
fn ffi_module_emits_scalar_wrappers() {
    let fixture = Fixture::new();
    let ir = compile_path_llvm(&fixture.0.join("main.dart")).unwrap();
    assert!(ir.contains("declare i32 @fixture_echo32(i32)"));
    assert!(ir.contains("trunc i64"));
    assert!(ir.contains("sext i32"));
    assert!(ir.contains("declare void @fixture_store(i64)"));
}

/// Executa ambos os níveis de otimização com um objeto C real e confirma efeitos ordenados.
#[test]
#[ignore = "requer Clang e rustc/linker nativos ou DARTFORGE_CLANG/DARTFORGE_RUSTC"]
fn ffi_objects_execute_at_o0_and_o2() {
    let fixture = Fixture::new();
    let ir = compile_path_llvm(&fixture.0.join("main.dart")).unwrap();
    let source = fixture.0.join("external code.c");
    std::fs::write(&source, C_SOURCE).unwrap();
    for optimize in [false, true] {
        let options = NativeOptions {
            optimize,
            ..Default::default()
        };
        let object = fixture.0.join(if optimize {
            "external optimized.obj"
        } else {
            "external plain.obj"
        });
        let result = Command::new(&options.clang)
            .arg("-c")
            .arg(if optimize { "-O2" } else { "-O0" })
            .arg(&source)
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let executable = fixture.0.join(if optimize {
            "optimized.exe"
        } else {
            "plain.exe"
        });
        let report =
            build_executable_with_report_and_objects(&ir, &executable, &options, &[object])
                .unwrap();
        assert!(report.executable_bytes > 0);
        let result = Command::new(executable).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout)
                .unwrap()
                .replace("\r\n", "\n"),
            EXPECTED
        );
    }
    let invalid = fixture.0.join("invalid object.obj");
    std::fs::write(&invalid, b"este arquivo nao contem codigo objeto").unwrap();
    for (name, objects) in [
        ("missing-symbol.exe", vec![]),
        ("invalid-object.exe", vec![invalid]),
    ] {
        let output = fixture.0.join(name);
        let error = build_executable_with_report_and_objects(
            &ir,
            &output,
            &NativeOptions::default(),
            &objects,
        )
        .unwrap_err();
        assert_eq!(error.stage, "rustc");
        assert!(!output.exists());
    }
}
