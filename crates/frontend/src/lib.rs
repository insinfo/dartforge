//! Front-end completo de Dart 3.6: léxico, árvore sintática e parser.
//!
//! Esta trilha existe para a meta governante do PLANO.md: compilar qualquer
//! projeto Dart 3.6 válido no dart2js/DDC. Diferente de `crates/parser`, o
//! parser daqui **não resolve nada**: `Foo<Bar>` é sintaxe válida sem saber o
//! que `Foo` é. Resolução, tipos e emissão vêm em fases posteriores sobre a
//! árvore produzida aqui.
//!
//! Por que este crate coexiste com `crates/parser` e `crates/syntax`: o parser
//! antigo resolve tipos enquanto analisa e é consumido por todo o pipeline de
//! emissão (semantic → hir → codegen → linker → llvm/jit). Trocá-lo por baixo
//! quebraria o que já compila. A trilha nova cresce até cobrir mais do que a
//! antiga, a emissão migra para ela, e só então `crates/parser` e
//! `crates/syntax` são removidos. Nada da capacidade existente é retirado antes.
//!
//! Desenho de memória, conforme a meta de cadeia de ferramentas própria:
//! nós vivem em arenas indexadas por `u32` ([`ast::Ast`]), identificadores são
//! [`dartforge_intern::SymbolId`], e nenhum nó empresta da fonte.
pub mod ast;
pub mod features;
pub mod lexer;
pub mod parser;
pub mod text;
pub mod token;

pub use dartforge_diagnostics::{Diagnostic, Span};
pub use dartforge_intern::{Interner, SymbolId};
pub use features::{Feature, LanguageVersion, LibraryFeatures};
pub use text::DartStr;
