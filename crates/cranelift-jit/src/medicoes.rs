//! Medições da compilação em memória, separadas por fase.
//!
//! Só existe aqui o que o JIT realmente faz: nada de front-end, nada de I/O.
//! Cada duração é cronometrada no próprio trecho, nunca por subtração, como
//! exige `docs/DESEMPENHO.md`. Os dois contadores de trabalho — instruções CLIF
//! e bytes de código de máquina — não dependem da carga da máquina e servem
//! para detectar regressão mesmo quando o relógio está ruidoso.
use std::time::Duration;

/// Tempos e contadores de uma compilação da HIR até código executável.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Medicoes {
    /// HIR do DartForge até a IR do Cranelift, incluindo a validação da fatia.
    pub traducao: Duration,
    /// IR do Cranelift até código de máquina, incluindo relocações e finalização.
    pub geracao: Duration,
    /// Soma cronometrada da chamada inteira, incluindo a criação do `JITModule`.
    pub total: Duration,
    /// Instruções CLIF emitidas; contador de trabalho independente da máquina.
    pub instrucoes_clif: usize,
    /// Bytes de código de máquina produzidos, somando todas as funções.
    pub bytes_codigo: usize,
}

impl Medicoes {
    /// Linha pronta para tabela, com as durações em milissegundos.
    ///
    /// # Exemplos
    /// ```
    /// use dartforge_cranelift_jit::Medicoes;
    /// use std::time::Duration;
    /// let m = Medicoes {
    ///     traducao: Duration::from_micros(500),
    ///     geracao: Duration::from_micros(1_500),
    ///     total: Duration::from_micros(2_000),
    ///     instrucoes_clif: 42,
    ///     bytes_codigo: 256,
    /// };
    /// assert_eq!(m.linha(), "0,500 ms | 1,500 ms | 2,000 ms | 42 instruções CLIF | 256 bytes");
    /// ```
    #[must_use]
    pub fn linha(&self) -> String {
        format!(
            "{} | {} | {} | {} instruções CLIF | {} bytes",
            milissegundos(self.traducao),
            milissegundos(self.geracao),
            milissegundos(self.total),
            self.instrucoes_clif,
            self.bytes_codigo
        )
    }
}

/// Formata uma duração em milissegundos com três casas e vírgula decimal.
fn milissegundos(duracao: Duration) -> String {
    let micros = duracao.as_micros();
    format!("{},{:03} ms", micros / 1000, micros % 1000)
}
