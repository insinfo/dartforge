//! Port of csslib 1.0.2 (`package:csslib`) limited to what the ngcompiler style
//! shim uses: `parse(css, errors: …)` with the default options, the tree, the
//! `Visitor` and the compact `CssPrinter`.
//!
//! - [`tokenizer`] — `src/tokenizer.dart`, `src/tokenizer_base.dart`,
//!   `src/token.dart`;
//! - [`token_kind`] — `src/token_kind.dart`;
//! - [`parser`] — the `_Parser` of `parser.dart`;
//! - [`tree`] — `src/tree.dart`, `src/tree_base.dart`;
//! - [`visitor`] — `visitor.dart`;
//! - [`printer`] — `src/css_printer.dart`.
//!
//! Left out is whatever the shim path does not go through: `analyzer.dart`
//! (`compile`, not `parse`), `polyfill.dart`, `tree_printer.dart`,
//! `validate.dart` and, from `property.dart`, the style values for Dart (nobody
//! reads them; see [`parser`]). The options are the `parse` defaults: `checked`
//! false (the warnings conditioned on it do not exist) and `lessSupport` true.
pub mod parser;
pub mod printer;
pub mod token_kind;
pub mod tokenizer;
pub mod tree;
pub mod visitor;

/// An exception that the official code throws (the name of the Dart type, or
/// the description of an endless loop): the builder would fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exception(pub &'static str);

impl std::fmt::Display for Exception {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// `parse(input, errors: errors)`: the stylesheet, read from the text as Dart
/// sees it (UTF-16 code units). Parse errors do not stop the reading.
pub fn parse(css: &str) -> Result<tree::StyleSheet, Exception> {
    let source: Vec<u16> = css.encode_utf16().collect();
    let mut p = parser::Parser::new(&source)?;
    p.parse()
}
