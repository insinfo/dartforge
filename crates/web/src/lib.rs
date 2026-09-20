//! API de compilação para a futura ferramenta de desenvolvimento web.
//!
//! Ainda não há servidor HTTP, empacotador, observação de arquivos ou HMR.
use dartforge_diagnostics::Diagnostic;

/// Compila o conteúdo de uma entrada Dart para um módulo JavaScript.
///
/// # Erros
///
/// Retorna o primeiro erro léxico, sintático ou semântico do subconjunto suportado.
///
/// # Exemplos
///
/// ```
/// let javascript = dartforge_web::build_entry("void main() { print('web'); }")?;
/// assert!(javascript.contains("console.log"));
/// # Ok::<(), dartforge_diagnostics::Diagnostic>(())
/// ```
pub fn build_entry(source: &str) -> Result<String, Diagnostic> {
    dartforge_compiler::compile(source)
}
