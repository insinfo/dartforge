//! Diagnósticos compartilhados entre compilador e futuras ferramentas de edição.

/// Intervalo semiaberto de bytes UTF-8 no arquivo de origem.
///
/// Adaptadores de editor devem converter esses offsets para a codificação do protocolo,
/// por exemplo unidades UTF-16 no LSP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Span {
    /// Primeiro byte incluído.
    pub start: usize,
    /// Primeiro byte depois do intervalo.
    pub end: usize,
}

/// Erro acompanhado da localização correspondente no código de origem.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Diagnostic {
    /// Explicação legível do problema.
    pub message: String,
    /// Região do código associada ao problema.
    pub span: Span,
}
impl Diagnostic {
    /// Cria um diagnóstico sem alterar a mensagem ou o intervalo informado.
    ///
    /// # Exemplos
    ///
    /// ```
    /// use dartforge_diagnostics::{Diagnostic, Span};
    /// let erro = Diagnostic::new("Nome não encontrado", Span { start: 2, end: 5 });
    /// assert_eq!(erro.span.start, 2);
    /// ```
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}
impl std::fmt::Display for Diagnostic {
    /// Formata a mensagem com seu intervalo de bytes.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at bytes {}..{}",
            self.message, self.span.start, self.span.end
        )
    }
}
impl std::error::Error for Diagnostic {}
