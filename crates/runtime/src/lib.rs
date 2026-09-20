//! Fonte Rust embarcada do runtime nativo mínimo de inteiros e booleanos.
//! O harness é compilado separadamente: unsafe fica restrito à fronteira FFI.

/// Programa standalone Rust 2024 ligado ao objeto produzido pelo LLVM.
///
/// Os símbolos usam ABI C: entrada sem argumentos, impressão i64 e impressão u8.
/// Inclui impressão de null e falha de asserção. Não implementa strings, classes ou GC.
pub const RUNTIME_MAIN: &str = include_str!("runtime_main.rs");
