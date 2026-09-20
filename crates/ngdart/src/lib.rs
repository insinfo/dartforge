//! Reserved boundary. AngularDart template compilation is NOT implemented.
use dartforge_diagnostics::{Diagnostic, Span};
pub const TARGET_VERSION: &str = "8.0.0-dev.4";
pub fn compile_component(_source: &str) -> Result<String, Diagnostic> {
    Err(Diagnostic::new(
        "ngdart compilation is not implemented",
        Span { start: 0, end: 0 },
    ))
}
