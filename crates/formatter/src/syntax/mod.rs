//! Leitura do código Dart no formato que o `dart_style` consome: tokens com
//! comentários ([`token`]) e a árvore do analyzer ([`tree`]), montada pelo
//! [`parser`].
pub mod tree;
pub mod parser;
pub mod token;
