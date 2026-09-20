//! Adaptador de diagnósticos para o futuro servidor LSP.
//!
//! Esta crate ainda não implementa transporte, JSON-RPC ou sincronização de documentos.
use dartforge_diagnostics::Diagnostic;

/// Analisa o texto e retorna o primeiro erro do pipeline, quando houver.
///
/// A lista vazia indica que o programa pertence ao subconjunto aceito pelo compilador.
/// Isso não equivale a uma análise completa de toda a linguagem Dart.
///
/// # Exemplos
///
/// ```
/// assert!(dartforge_lsp::diagnose("void main() {}").is_empty());
/// assert_eq!(dartforge_lsp::diagnose("void main() { print(desconhecido); }").len(), 1);
/// ```
pub fn diagnose(source: &str) -> Vec<Diagnostic> {
    match dartforge_compiler::compile(source) {
        Ok(_) => vec![],
        Err(error) => vec![error],
    }
}
