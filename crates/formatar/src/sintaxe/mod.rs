//! Leitura do código Dart no formato que o `dart_style` consome: tokens com
//! comentários ([`token`]) e a árvore do analyzer ([`arvore`]), montada pelo
//! [`parser`].
pub mod arvore;
pub mod parser;
pub mod token;
