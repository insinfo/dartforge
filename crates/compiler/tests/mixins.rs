//! Regressões de expansão e execução de mixins; oracle Dart VM 3.6.2.
use dartforge_compiler::{CompileOptions, Optimization, compile_llvm, compile_with_options};
const SOURCE: &str = include_str!("../../../tests/native/cases/mixins.dart");
const EXPECTED: &str = include_str!("../../../tests/native/cases/mixins.stdout");

/// O backend LLVM usa o layout da aplicação, mesmo com this tipado pelo mixin original.
#[test]
fn mixin_native_ir_preserves_physical_field_owners() {
    let ir = compile_llvm(SOURCE).unwrap();
    assert!(ir.contains("call i64 @dartforge_object_get(i64 %this, i64 1)"));
    assert!(ir.contains("call i64 @dartforge_object_get(i64 %this, i64 2)"));
    assert!(ir.contains("@df_dispatch_"));
}

/// Cada aplicação tem armazenamento próprio; o último mixin determina os overrides.
#[test]
#[ignore = "requer Node.js no PATH"]
fn mixins_match_dart_oracle_in_all_modes() {
    for optimization in [Optimization::None, Optimization::Constants] {
        for merge_identical_functions in [false, true] {
            let javascript = compile_with_options(
                SOURCE,
                CompileOptions {
                    optimization,
                    merge_identical_functions,
                },
            )
            .unwrap();
            let result = std::process::Command::new("node")
                .args(["--input-type=module", "--eval", &javascript])
                .output()
                .expect("Node.js necessário");
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                String::from_utf8(result.stdout)
                    .unwrap()
                    .replace("\r\n", "\n"),
                EXPECTED.replace("\r\n", "\n")
            );
        }
    }
}
