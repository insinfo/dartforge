//! Conversão entre offsets de bytes UTF-8 e posições LSP (linha, coluna UTF-16).
//!
//! O protocolo LSP conta colunas em **unidades UTF-16**, enquanto o texto em
//! memória é UTF-8 (`String`). Um emoji como `👭` ocupa 4 bytes em UTF-8 e 2
//! unidades em UTF-16; confundir as duas contagens edita o byte errado e
//! publica o sublinhado na coluna errada. Este módulo é a única fronteira
//! onde essa conversão acontece, nos dois sentidos:
//!
//! * cliente → servidor (`didChange`): posição UTF-16 vira offset de bytes;
//! * servidor → cliente (`publishDiagnostics`): span de bytes vira posição.
//!
//! Quebras de linha aceitas: `\n`, `\r\n` e `\r` solitário, como o VS Code.
//! Posições fora do documento são saturadas (nunca `panic`); coluna no meio
//! de um par substituto (entre as duas unidades de um astral) é truncada
//! para a borda do caractere, porque o protocolo exige fronteira de
//! caractere.
//!
//! ```
//! let table = dartforge_lsp::utf16::LineTable::build("var s = '👭';\nint ;\n");
//! // Linha 0 tem 9 ASCII + 2 (emoji) + 2 = 13 unidades UTF-16 antes da quebra.
//! assert_eq!(table.line_width("var s = '👭';\nint ;\n", 0), 13);
//! ```

use std::cmp::min;

/// Índice de inícios de linha de um documento, em bytes UTF-8.
///
/// `starts[0]` é sempre 0; cada entrada seguinte é o primeiro byte após uma
/// quebra. A tabela é recalculada **só do ponto editado em diante**
/// ([`LineTable::recompute_from`]): prefixo intacto não é revisitado.
#[derive(Debug, Clone, Default)]
pub struct LineTable {
    starts: Vec<usize>,
}

impl LineTable {
    /// Constroi a tabela para o texto integral (abertura de documento).
    pub fn build(text: &str) -> Self {
        let mut table = Self { starts: vec![0] };
        table.scan(text, 0);
        table
    }

    /// Varre `text` a partir de `base`, empilhando inícios de linha.
    fn scan(&mut self, text: &str, base: usize) {
        let bytes = text.as_bytes();
        let mut i = base;
        while i < bytes.len() {
            match bytes[i] {
                b'\n' => {
                    self.starts.push(i + 1);
                    i += 1;
                }
                b'\r' => {
                    if bytes.get(i + 1) == Some(&b'\n') {
                        self.starts.push(i + 2);
                        i += 2;
                    } else {
                        self.starts.push(i + 1);
                        i += 1;
                    }
                }
                _ => i += 1,
            }
        }
    }

    /// Quantas linhas o documento tem (a última pode ser vazia após `\n`).
    pub fn total_lines(&self) -> usize {
        self.starts.len()
    }

    /// Linha que contém o offset de bytes (saturado para o fim do texto).
    pub fn line_of(&self, text: &str, offset: usize) -> usize {
        let offset = min(offset, text.len());
        self.starts.partition_point(|start| *start <= offset) - 1
    }

    /// Byte onde a linha começa.
    pub fn line_start(&self, line: usize) -> usize {
        self.starts[min(line, self.starts.len() - 1)]
    }

    /// Byte onde o conteúdo da linha termina, sem a quebra (`\r\n`, `\n`, `\r`).
    pub fn line_end(&self, text: &str, line: usize) -> usize {
        let start = self.line_start(line);
        let next = if line + 1 < self.starts.len() {
            self.starts[line + 1]
        } else {
            text.len()
        };
        let mut end = min(next, text.len());
        if end > start && text.as_bytes()[end - 1] == b'\n' {
            end -= 1;
        }
        if end > start && text.as_bytes()[end - 1] == b'\r' {
            end -= 1;
        }
        end
    }

    /// Recalcula os inícios da `line` em diante, após edição já aplicada.
    ///
    /// Contratos: `text` já contém a edição; `line` é a linha do início da
    /// edição no texto novo; o prefixo `starts[..line]` está intacto.
    /// Custo proporcional ao sufixo, nunca ao prefixo.
    pub fn recompute_from(&mut self, text: &str, line: usize) {
        let line = min(line, self.starts.len().saturating_sub(1));
        let base = self.starts[line];
        self.starts.truncate(line + 1);
        self.scan(text, base);
    }

    /// Converte offset de bytes em (linha, coluna em unidades UTF-16).
    ///
    /// Offset fora do texto satura para o fim; offset no meio de um
    /// caractere desce para a fronteira anterior.
    pub fn offset_position(&self, text: &str, offset: usize) -> (u32, u32) {
        let mut offset = min(offset, text.len());
        while !text.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = self.line_of(text, offset);
        let column = text[self.line_start(line)..offset]
            .chars()
            .map(|c| c.len_utf16())
            .sum::<usize>();
        (line as u32, column as u32)
    }

    /// Converte (linha, coluna em unidades UTF-16) em offset de bytes.
    ///
    /// Linha além do fim satura para o fim do texto; coluna além do fim da
    /// linha satura para o fim do conteúdo (antes da quebra); coluna no meio
    /// de um par substituto trunca para a borda do caractere.
    pub fn position_offset(&self, text: &str, line: u32, column: u32) -> usize {
        let line = min(line as usize, self.starts.len() - 1);
        let start = self.starts[line];
        let end = self.line_end(text, line);
        let mut consumed = 0u32;
        let mut offset = start;
        for c in text[start..end].chars() {
            if consumed >= column {
                break;
            }
            let width = c.len_utf16() as u32;
            if consumed + width > column {
                break; // Meio de um astral: para na borda do caractere.
            }
            consumed += width;
            offset += c.len_utf8();
        }
        offset
    }

    /// Largura da linha em unidades UTF-16, sem a quebra (uso em testes).
    pub fn line_width(&self, text: &str, line: usize) -> usize {
        let line = min(line, self.starts.len() - 1);
        text[self.starts[line]..self.line_end(text, line)]
            .chars()
            .map(|c| c.len_utf16())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emoji_counts_two_units_both_ways() {
        let text = "var s = '👭';\nint ;\n";
        let table = LineTable::build(text);
        assert_eq!(table.total_lines(), 3);
        assert_eq!(table.line_width(text, 0), 13);
        // O `;` após o emoji: byte 13, unidade UTF-16 11.
        assert_eq!(table.offset_position(text, 13), (0, 11));
        assert_eq!(table.position_offset(text, 0, 11), 13);
        // Coluna 10 cai no meio do par substituto: trunca para a borda (byte 9).
        assert_eq!(table.position_offset(text, 0, 10), 9);
    }

    #[test]
    fn crlf_and_lone_cr_break_lines() {
        let text = "a\r\nb\rc\n";
        let table = LineTable::build(text);
        assert_eq!(table.total_lines(), 4);
        assert_eq!(table.offset_position(text, 3), (1, 0));
        assert_eq!(table.position_offset(text, 2, 0), 5);
        assert_eq!(table.line_width(text, 0), 1);
    }

    #[test]
    fn outside_document_saturates_without_panic() {
        let text = "ab\ncd";
        let table = LineTable::build(text);
        assert_eq!(table.offset_position(text, 999), (1, 2));
        assert_eq!(table.position_offset(text, 99, 99), text.len());
        assert_eq!(table.position_offset(text, 0, 99), 2);
    }

    #[test]
    fn partial_recompute_preserves_prefix() {
        let text = "l0\nl1\nl2\nl3\n";
        let mut table = LineTable::build(text);
        let before = table.starts.clone();
        // Edição na linha 2: só o sufixo muda.
        let mut new = text.to_string();
        new.replace_range(6..8, "LONGA");
        table.recompute_from(&new, 2);
        assert_eq!(&table.starts[..3], &before[..3]);
        assert_eq!(table.line_start(3), 12);
    }
}
