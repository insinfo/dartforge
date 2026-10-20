//! Complete Dart 3.6 front end: lexer, syntax tree and parser.
//!
//! This track exists for the governing goal of PLANO.md: compiling any valid
//! Dart 3.6 project the way dart2js/DDC do. The parser here **resolves
//! nothing**: `Foo<Bar>` is valid syntax without knowing what `Foo` is.
//! Resolution, types and emission come in later phases on top of the tree
//! produced here.
//!
//! The old parser (`crates/parser`/`crates/syntax`), which resolved types
//! while parsing, left the repository when emission moved to this track; it
//! is preserved in the `exploracao-inicial` branch.
//!
//! Memory design, following the goal of an in-house toolchain: nodes live in
//! arenas indexed by `u32` ([`ast::Ast`]), identifiers are
//! [`dartforge_intern::SymbolId`], and no node borrows from the source.
pub mod ast;
pub mod comments;
pub mod doc_references;
pub mod features;
pub mod grouping;
pub mod lexer;
pub mod parents;
pub mod parser;
pub mod source;
pub mod text;
pub mod token;

pub use dartforge_diagnostics::{Diagnostic, Span};
pub use dartforge_intern::{Interner, SymbolId};
pub use features::{Feature, LanguageVersion, LibraryFeatures};
pub use text::DartStr;
