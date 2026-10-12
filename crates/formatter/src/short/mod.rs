//! O estilo "curto" do `dart_style` 2.3.8 (`lib/src/short/`), o padrão do
//! `dart format` do SDK 3.6.2.
//!
//! * [`arena`]: regras, chunks, spans e níveis de aninhamento.
//! * [`construtor`]: o `ChunkBuilder`.
//! * [`escritor`] e [`divisor`]: o `LineWriter` e o `LineSplitter`.
//! * [`visitante`]: o `SourceVisitor`, com [`argumentos`]
//!   (`ArgumentListVisitor`) e [`cadeia`] (`CallChainVisitor`).
//!
//! Os métodos do visitante mantêm os nomes do original (`visit_binary_expression`
//! para `visitBinaryExpression`), para que cada regra possa ser conferida
//! contra o fonte do `dart_style`.
pub mod arena;
pub mod arguments;
pub mod chain;
pub mod builder;
pub mod splitter;
pub mod writer;
pub mod visitor;
