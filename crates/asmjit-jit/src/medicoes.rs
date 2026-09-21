//! Medições da compilação em memória, separadas por fase.
//!
//! Só existe aqui o que o JIT realmente faz: nada de front-end, nada de I/O.
//! Cada duração é cronometrada no próprio trecho, nunca por subtração, como
//! exige `docs/DESEMPENHO.md`.
//!
//! As fases de um montador não são as de um compilador com IR. Um montador
//! **codifica cada instrução no ato**: não há IR intermediária para otimizar
//! depois, e no dynasm-rs a codificação já foi resolvida quando o próprio Rust
//! compilou. Por isso a primeira fase mede HIR → bytes de código já
//! codificados, e a segunda mede apenas a resolução de saltos pendentes e a
//! publicação das páginas executáveis. Fingir três fases aqui daria uma tabela
//! comparável com a do Cranelift e falsa.
//!
//! Os dois contadores de trabalho — instruções emitidas e bytes de código — não
//! dependem da carga da máquina e servem para detectar regressão mesmo quando o
//! relógio está ruidoso.
use std::time::Duration;

/// Tempos e contadores de uma compilação da HIR até código executável.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Medicoes {
    /// HIR do DartForge até instruções x86-64 codificadas, com a validação da fatia.
    pub traducao: Duration,
    /// Resolução dos saltos pendentes e publicação do bloco executável.
    pub geracao: Duration,
    /// Soma cronometrada da chamada inteira, incluindo a criação do alocador.
    pub total: Duration,
    /// Instruções x86-64 emitidas; contador de trabalho independente da máquina.
    pub instrucoes_emitidas: usize,
    /// Bytes de código de máquina emitidos, somando todas as funções.
    ///
    /// É o deslocamento final do montador, não o tamanho do mapeamento: o bloco
    /// executável é arredondado para páginas inteiras e daria 4096 para qualquer
    /// programa desta fatia. O tamanho mapeado está em
    /// [`crate::ProgramaCompilado::bytes`].
    pub bytes_codigo: usize,
}

impl Medicoes {
    /// Linha pronta para tabela, com as durações em milissegundos.
    ///
    /// # Exemplos
    /// ```
    /// use dartforge_asmjit_jit::Medicoes;
    /// use std::time::Duration;
    /// let m = Medicoes {
    ///     traducao: Duration::from_micros(500),
    ///     geracao: Duration::from_micros(1_500),
    ///     total: Duration::from_micros(2_000),
    ///     instrucoes_emitidas: 42,
    ///     bytes_codigo: 256,
    /// };
    /// assert_eq!(m.linha(), "0,500 ms | 1,500 ms | 2,000 ms | 42 instruções | 256 bytes");
    /// ```
    #[must_use]
    pub fn linha(&self) -> String {
        format!(
            "{} | {} | {} | {} instruções | {} bytes",
            milissegundos(self.traducao),
            milissegundos(self.geracao),
            milissegundos(self.total),
            self.instrucoes_emitidas,
            self.bytes_codigo
        )
    }
}

/// Formata uma duração em milissegundos com três casas e vírgula decimal.
fn milissegundos(duracao: Duration) -> String {
    let micros = duracao.as_micros();
    format!("{},{:03} ms", micros / 1000, micros % 1000)
}
