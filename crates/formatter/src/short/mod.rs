//! The "short" style of `dart_style` 2.3.8 (`lib/src/short/`), the default of
//! `dart format` in SDK 3.6.2.
//!
//! * [`arena`]: rules, chunks, spans and nesting levels.
//! * [`builder`]: the `ChunkBuilder`.
//! * [`writer`] and [`splitter`]: the `LineWriter` and the `LineSplitter`.
//! * [`visitor`]: the `SourceVisitor`, with [`arguments`]
//!   (`ArgumentListVisitor`) and [`chain`] (`CallChainVisitor`).
//!
//! The visitor's methods keep the original names (`visit_binary_expression`
//! for `visitBinaryExpression`), so that every rule can be checked against
//! the `dart_style` source.
pub mod arena;
pub mod arguments;
pub mod builder;
pub mod chain;
pub mod splitter;
pub mod visitor;
pub mod writer;
