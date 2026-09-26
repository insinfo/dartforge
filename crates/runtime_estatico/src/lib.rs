//! A `staticlib` do runtime do AOT: tudo vem de `dartforge-runtime` (o módulo
//! `abi`, no perfil da feature `aot` ou `dll`). Os símbolos são os
//! `#[unsafe(no_mangle)]` dele; a referência abaixo só garante que a
//! biblioteca entra no arquivo.
pub use dartforge_runtime::abi;
