//! Regressões do contrato de otimização opcional.
use dartforge_compiler::{Optimization, compile, compile_with_optimization};

/// A simplificação nunca deve ocultar erro semântico em um ramo não executável.
#[test]
fn validation_precedes_constant_folding() {
    let source = "void main() { print(1 ?? missing()); }";
    assert!(compile(source).is_err());
    assert!(compile_with_optimization(source, Optimization::Constants).is_err());
}

/// O passe reduz constantes, mas deixa resultados fora de i32 para o runtime.
#[test]
fn checked_constants_change_output_without_overflow() {
    let source = "void main() { print(2 + 3); print(2147483647 + 1); }";
    let direct = compile(source).unwrap();
    let optimized = compile_with_optimization(source, Optimization::Constants).unwrap();
    assert_ne!(direct, optimized);
    assert!(optimized.contains("console.log(5)"));
    assert!(optimized.contains("2147483647 + 1"));
}
