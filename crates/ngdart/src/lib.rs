//! Fronteira reservada para compilação de componentes e templates ngdart.
//!
//! A implementação de AngularDart ainda não está disponível.
use dartforge_diagnostics::{Diagnostic, Span};

/// Versão inicial de referência do pacote ngdart.
pub const TARGET_VERSION: &str = "8.0.0-dev.4";

/// Informa que a compilação de componentes ainda não foi implementada.
///
/// # Erros
///
/// Sempre retorna um diagnóstico de funcionalidade indisponível. Nenhum JavaScript
/// parcial é produzido ou apresentado como compilação válida.
///
/// # Exemplos
///
/// ```
/// assert!(dartforge_ngdart::compile_component("@Component() class App {}").is_err());
/// ```
pub fn compile_component(_source: &str) -> Result<String, Diagnostic> {
    Err(Diagnostic::new(
        "Compilação ngdart ainda não implementada",
        Span { start: 0, end: 0 },
    ))
}
