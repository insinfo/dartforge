//! Diagnostics adapter only; no LSP protocol/transport server yet.
use dartforge_diagnostics::Diagnostic;
pub fn diagnose(source: &str) -> Vec<Diagnostic> {
    match dartforge_compiler::compile(source) {
        Ok(_) => vec![],
        Err(error) => vec![error],
    }
}
