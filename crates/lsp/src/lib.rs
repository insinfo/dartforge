//! Adaptador de diagnósticos para o futuro servidor LSP.
//!
//! Esta crate ainda não implementa transporte, JSON-RPC ou sincronização de documentos.
use dartforge_diagnostics::Diagnostic;

/// Analisa o texto e retorna **todos** os diagnósticos que o pipeline encontra.
///
/// O parser recupera de erros em fronteiras de declaração, então um arquivo com
/// três declarações quebradas rende três diagnósticos, ordenados por span
/// (início, depois fim). Um editor que mostrasse um erro por arquivo obrigaria a
/// recompilar a cada correção para descobrir o próximo, o que é inútil.
///
/// As fases posteriores à sintaxe — macros, mixins, semântica, emissão — ainda
/// param no primeiro erro, e por isso a lista volta a ter no máximo um elemento
/// assim que a sintaxe do arquivo está correta.
///
/// A lista vazia indica que o programa pertence ao subconjunto aceito pelo compilador.
/// Isso não equivale a uma análise completa de toda a linguagem Dart.
///
/// # Exemplos
///
/// ```
/// assert!(dartforge_lsp::diagnose("void main() {}").is_empty());
/// assert_eq!(dartforge_lsp::diagnose("void main() { print(desconhecido); }").len(), 1);
/// let tres = "void a() { int ; } void b() { int ; } void main() { int ; }";
/// assert_eq!(dartforge_lsp::diagnose(tres).len(), 3);
/// ```
pub fn diagnose(source: &str) -> Vec<Diagnostic> {
    dartforge_compiler::compile_diagnostics(source)
}
