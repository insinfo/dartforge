//! Reading Dart code in the shape `dart_style` consumes: tokens with
//! comments ([`token`]) and the analyzer tree ([`tree`]), built by the
//! [`parser`].
pub mod parser;
pub mod token;
pub mod tree;
