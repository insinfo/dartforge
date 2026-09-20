//! Entry compilation API only; no HTTP server, bundler or watcher yet.
use dartforge_diagnostics::Diagnostic;
pub fn build_entry(source: &str) -> Result<String, Diagnostic> {
    dartforge_compiler::compile(source)
}
