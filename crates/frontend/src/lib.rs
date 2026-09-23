//! Front-end completo de Dart 3.6: léxico, árvore sintática e parser.
//!
//! Esta trilha existe para a meta governante do PLANO.md: compilar qualquer
//! projeto Dart 3.6 válido no dart2js/DDC. O parser daqui **não resolve
//! nada**: `Foo<Bar>` é sintaxe válida sem saber o que `Foo` é. Resolução,
//! tipos e emissão vêm em fases posteriores sobre a árvore produzida aqui.
//!
//! O parser antigo (`crates/parser`/`crates/syntax`), que resolvia tipos
//! enquanto analisava, saiu do repositório quando a emissão migrou para esta
//! trilha; está preservado na branch `exploracao-inicial`.
//!
//! Desenho de memória, conforme a meta de cadeia de ferramentas própria:
//! nós vivem em arenas indexadas por `u32` ([`ast::Ast`]), identificadores são
//! [`dartforge_intern::SymbolId`], e nenhum nó empresta da fonte.
pub mod ast;
pub mod lexer;
pub mod parser;
pub mod text;
pub mod token;

pub use dartforge_diagnostics::{Diagnostic, Span};
pub use dartforge_intern::{Interner, SymbolId};
pub use text::DartStr;
