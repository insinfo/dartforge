//! Runtime nativo com heap preciso, strings e objetos gerenciados.
//! O harness FFI é compilado separadamente; o heap seguro participa dos testes.
pub mod heap;

/// Programa Rust 2024 completo: heap e harness ligados ao objeto LLVM.
/// O compilador mantém referências vivas por frames e tags de campos explícitas.
pub const RUNTIME_MAIN: &str = concat!(
    "mod heap {\n",
    include_str!("heap.rs"),
    "\n}\n",
    include_str!("runtime_main.rs")
);
