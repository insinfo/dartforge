//! The AOT runtime `staticlib`: everything comes from `dartforge-runtime` (the
//! `abi` module, built with the `aot` or `dll` feature profile). The symbols are
//! its `#[unsafe(no_mangle)]` functions; the reference below only makes sure the
//! library ends up in the archive.
pub use dartforge_runtime::abi;
