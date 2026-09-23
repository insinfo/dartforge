//! Runtime nativo com heap preciso, strings, objetos, células, ambientes e listas.
//! Consulte CONTRACT.md na raiz da crate.
//!
//! # Uma fonte, duas compilações
//!
//! `runtime_main.rs` é o runtime que o código gerado chama. O AOT o compila
//! como texto ([`RUNTIME_MAIN`]) com `rustc` avulso, e liga a `.lib` ao
//! executável. O JIT (`crates/jit`) usa o **mesmo arquivo** compilado aqui,
//! como o módulo [`abi`], e publica os endereços de [`simbolos::tabela`]. As
//! flags semânticas são as mesmas nos dois (opt-level 2, sem overflow-checks,
//! sem debug-assertions): o `Cargo.toml` da raiz fixa o perfil deste crate no
//! que o `rustc -O` do AOT dá. O teste `tests/fonte_unica.rs` confere.
pub mod heap;

/// O harness do runtime, compilado como módulo: tudo o que o executável AOT
/// liga, menos o `main` C (cfg `dartforge_runtime_embutido`, do `build.rs`).
///
/// Os `allow` são os mesmos que o AOT aplica (`#![allow(warnings)]` na
/// compilação avulsa): o arquivo é o mesmo código nos dois perfis, não uma
/// versão ajustada para as lints do workspace.
#[allow(warnings, unsafe_code, unsafe_op_in_unsafe_fn, clippy::all)]
#[path = "runtime_main.rs"]
pub mod abi;

/// A tabela `(nome, endereço)` dos símbolos do runtime, gerada pelo `build.rs`
/// a partir dos `#[unsafe(no_mangle)]` de `runtime_main.rs`.
pub mod simbolos {
    include!(concat!(env!("OUT_DIR"), "/simbolos.rs"));
}

/// Programa Rust 2024 completo: heap e harness ligados ao objeto LLVM.
/// O compilador mantém referências vivas por frames e tags de campos explícitas.
pub const RUNTIME_MAIN: &str = concat!(
    "mod heap {\n",
    include_str!("heap.rs"),
    "\n}\n",
    include_str!("runtime_main.rs")
);
