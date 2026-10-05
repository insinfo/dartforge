//! Runtime nativo com heap preciso, strings, objetos, células, ambientes e listas.
//! Consulte CONTRACT.md na raiz da crate.
//!
//! # Uma fonte, duas compilações
//!
//! O runtime que o código gerado chama é a concatenação dos fragmentos
//! `src/<fragmento>.rs` na ordem de `FRAGMENTOS` do `build.rs` ([`simbolos::FRAGMENTOS`]):
//! `nucleo` (entrada, estado por thread, objetos, caixas), `gc_raizes`,
//! `excecoes`, `saida`, `strings`, `colecoes` e `closures`. O AOT compila o
//! texto concatenado ([`RUNTIME_MAIN`]) com `rustc` avulso, e liga a `.lib` ao
//! executável. O JIT (`crates/jit`) usa os **mesmos arquivos** compilados aqui,
//! como o módulo [`abi`], e publica os endereços de [`simbolos::tabela`]. As
//! flags semânticas são as mesmas nos dois (opt-level 2, sem overflow-checks,
//! sem debug-assertions): o `Cargo.toml` da raiz fixa o perfil deste crate no
//! que o `rustc -O` do AOT dá. O teste `tests/fonte_unica.rs` confere.
pub mod hash;
pub mod heap;
/// O contrato de layout dos blocos (docs/NATIVO-ESPACO-UNIFICADO.md §3.1).
pub mod layout;
/// O espaço de objetos (páginas, classes de tamanho, regiões grandes, varredura).
pub mod espaco;
/// As vistas por pacote do espaço unificado (§3.3): strings, caixas, listas e
/// listas tipadas, cada uma um `impl Heap` sobre a API de `heap`.
pub mod textos;
pub mod caixas;
pub mod listas;
pub mod tipadas;

/// O harness do runtime, compilado como módulo: tudo o que o executável AOT
/// liga, menos o `main` C (cfg `dartforge_runtime_embutido`, do `build.rs`).
/// O corpo é um `include!` por fragmento, na ordem de `FRAGMENTOS`.
///
/// Os `allow` são os mesmos que o AOT aplica (`#![allow(warnings)]` na
/// compilação avulsa): o arquivo é o mesmo código nos dois perfis, não uma
/// versão ajustada para as lints do workspace.
#[allow(warnings, unsafe_code, unsafe_op_in_unsafe_fn, clippy::all)]
pub mod abi {
    include!(concat!(env!("OUT_DIR"), "/abi.rs"));
}

/// A tabela `(nome, endereço)` dos símbolos do runtime e a lista dos
/// fragmentos, gerados pelo `build.rs` a partir dos `#[unsafe(no_mangle)]` de
/// todos os fragmentos.
pub mod simbolos {
    include!(concat!(env!("OUT_DIR"), "/simbolos.rs"));
}

/// A tabela de efeitos das externs — o que cada função do runtime pode fazer
/// (coletar, deixar exceção pendente, rodar código Dart) —, gerada pelo
/// `build.rs` de `efeitos.tsv` (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
/// §13.8). O emissor decide por ela onde conferir a exceção pendente e o que
/// é ponto de coleta.
pub mod efeitos {
    include!(concat!(env!("OUT_DIR"), "/efeitos.rs"));
}

/// Programa Rust 2024 completo: heap e harness ligados ao objeto LLVM.
/// O compilador mantém referências vivas por frames e tags de campos explícitas.
pub const RUNTIME_MAIN: &str = concat!(
    "mod hash {\n",
    include_str!("hash.rs"),
    "\n}\n",
    "mod heap {\n",
    include_str!("heap.rs"),
    "\n}\n",
    "mod layout {\n",
    include_str!("layout.rs"),
    "\n}\n",
    "mod espaco {\n",
    include_str!("espaco.rs"),
    "\n}\n",
    "mod textos {\n",
    include_str!("textos.rs"),
    "\n}\n",
    "mod caixas {\n",
    include_str!("caixas.rs"),
    "\n}\n",
    "mod listas {\n",
    include_str!("listas.rs"),
    "\n}\n",
    "mod tipadas {\n",
    include_str!("tipadas.rs"),
    "\n}\n",
    include_str!(concat!(env!("OUT_DIR"), "/runtime_main.rs"))
);
