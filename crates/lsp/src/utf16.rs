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
//! let tabela = dartforge_lsp::utf16::TabelaLinhas::construir("var s = '👭';\nint ;\n");
//! // Linha 0 tem 9 ASCII + 2 (emoji) + 2 = 13 unidades UTF-16 antes da quebra.
//! assert_eq!(tabela.largura_linha("var s = '👭';\nint ;\n", 0), 13);
//! ```

use std::cmp::min;

/// Índice de inícios de linha de um documento, em bytes UTF-8.
///
/// `inicios[0]` é sempre 0; cada entrada seguinte é o primeiro byte após uma
/// quebra. A tabela é recalculada **só do ponto editado em diante**
/// ([`TabelaLinhas::recalc_a_partir`]): prefixo intacto não é revisitado.
#[derive(Debug, Clone, Default)]
pub struct TabelaLinhas {
    inicios: Vec<usize>,
}

impl TabelaLinhas {
    /// Constroi a tabela para o texto integral (abertura de documento).
    pub fn construir(texto: &str) -> Self {
        let mut tabela = Self { inicios: vec![0] };
        tabela.varrer(texto, 0);
        tabela
    }

    /// Varre `texto` a partir de `base`, empilhando inícios de linha.
    fn varrer(&mut self, texto: &str, base: usize) {
        let bytes = texto.as_bytes();
        let mut i = base;
        while i < bytes.len() {
            match bytes[i] {
                b'\n' => {
                    self.inicios.push(i + 1);
                    i += 1;
                }
                b'\r' => {
                    if bytes.get(i + 1) == Some(&b'\n') {
                        self.inicios.push(i + 2);
                        i += 2;
                    } else {
                        self.inicios.push(i + 1);
                        i += 1;
                    }
                }
                _ => i += 1,
            }
        }
    }

    /// Quantas linhas o documento tem (a última pode ser vazia após `\n`).
    pub fn total_linhas(&self) -> usize {
        self.inicios.len()
    }

    /// Linha que contém o offset de bytes (saturado para o fim do texto).
    pub fn linha_de(&self, texto: &str, offset: usize) -> usize {
        let offset = min(offset, texto.len());
        self.inicios.partition_point(|inicio| *inicio <= offset) - 1
    }

    /// Byte onde a linha começa.
    pub fn inicio_da_linha(&self, linha: usize) -> usize {
        self.inicios[min(linha, self.inicios.len() - 1)]
    }

    /// Byte onde o conteúdo da linha termina, sem a quebra (`\r\n`, `\n`, `\r`).
    pub fn fim_da_linha(&self, texto: &str, linha: usize) -> usize {
        let inicio = self.inicio_da_linha(linha);
        let proximo = if linha + 1 < self.inicios.len() {
            self.inicios[linha + 1]
        } else {
            texto.len()
        };
        let mut fim = min(proximo, texto.len());
        if fim > inicio && texto.as_bytes()[fim - 1] == b'\n' {
            fim -= 1;
        }
        if fim > inicio && texto.as_bytes()[fim - 1] == b'\r' {
            fim -= 1;
        }
        fim
    }

    /// Recalcula os inícios da `linha` em diante, após edição já aplicada.
    ///
    /// Contratos: `texto` já contém a edição; `linha` é a linha do início da
    /// edição no texto novo; o prefixo `inicios[..linha]` está intacto.
    /// Custo proporcional ao sufixo, nunca ao prefixo.
    pub fn recalc_a_partir(&mut self, texto: &str, linha: usize) {
        let linha = min(linha, self.inicios.len().saturating_sub(1));
        let base = self.inicios[linha];
        self.inicios.truncate(linha + 1);
        self.varrer(texto, base);
    }

    /// Converte offset de bytes em (linha, coluna em unidades UTF-16).
    ///
    /// Offset fora do texto satura para o fim; offset no meio de um
    /// caractere desce para a fronteira anterior.
    pub fn posicao_de_offset(&self, texto: &str, offset: usize) -> (u32, u32) {
        let mut offset = min(offset, texto.len());
        while !texto.is_char_boundary(offset) {
            offset -= 1;
        }
        let linha = self.linha_de(texto, offset);
        let coluna = texto[self.inicio_da_linha(linha)..offset]
            .chars()
            .map(|c| c.len_utf16())
            .sum::<usize>();
        (linha as u32, coluna as u32)
    }

    /// Converte (linha, coluna em unidades UTF-16) em offset de bytes.
    ///
    /// Linha além do fim satura para o fim do texto; coluna além do fim da
    /// linha satura para o fim do conteúdo (antes da quebra); coluna no meio
    /// de um par substituto trunca para a borda do caractere.
    pub fn offset_de_posicao(&self, texto: &str, linha: u32, coluna: u32) -> usize {
        let linha = min(linha as usize, self.inicios.len() - 1);
        let inicio = self.inicios[linha];
        let fim = self.fim_da_linha(texto, linha);
        let mut consumidas = 0u32;
        let mut offset = inicio;
        for c in texto[inicio..fim].chars() {
            if consumidas >= coluna {
                break;
            }
            let largura = c.len_utf16() as u32;
            if consumidas + largura > coluna {
                break; // Meio de um astral: para na borda do caractere.
            }
            consumidas += largura;
            offset += c.len_utf8();
        }
        offset
    }

    /// Largura da linha em unidades UTF-16, sem a quebra (uso em testes).
    pub fn largura_linha(&self, texto: &str, linha: usize) -> usize {
        let linha = min(linha, self.inicios.len() - 1);
        texto[self.inicios[linha]..self.fim_da_linha(texto, linha)]
            .chars()
            .map(|c| c.len_utf16())
            .sum()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn emoji_conta_duas_unidades_nos_dois_sentidos() {
        let texto = "var s = '👭';\nint ;\n";
        let tabela = TabelaLinhas::construir(texto);
        assert_eq!(tabela.total_linhas(), 3);
        assert_eq!(tabela.largura_linha(texto, 0), 13);
        // O `;` após o emoji: byte 13, unidade UTF-16 11.
        assert_eq!(tabela.posicao_de_offset(texto, 13), (0, 11));
        assert_eq!(tabela.offset_de_posicao(texto, 0, 11), 13);
        // Coluna 10 cai no meio do par substituto: trunca para a borda (byte 9).
        assert_eq!(tabela.offset_de_posicao(texto, 0, 10), 9);
    }

    #[test]
    fn crlf_e_cr_solito_quebram_linha() {
        let texto = "a\r\nb\rc\n";
        let tabela = TabelaLinhas::construir(texto);
        assert_eq!(tabela.total_linhas(), 4);
        assert_eq!(tabela.posicao_de_offset(texto, 3), (1, 0));
        assert_eq!(tabela.offset_de_posicao(texto, 2, 0), 5);
        assert_eq!(tabela.largura_linha(texto, 0), 1);
    }

    #[test]
    fn fora_do_documento_satura_sem_panic() {
        let texto = "ab\ncd";
        let tabela = TabelaLinhas::construir(texto);
        assert_eq!(tabela.posicao_de_offset(texto, 999), (1, 2));
        assert_eq!(tabela.offset_de_posicao(texto, 99, 99), texto.len());
        assert_eq!(tabela.offset_de_posicao(texto, 0, 99), 2);
    }

    #[test]
    fn recalculo_parcial_preserva_o_prefixo() {
        let texto = "l0\nl1\nl2\nl3\n";
        let mut tabela = TabelaLinhas::construir(texto);
        let antes = tabela.inicios.clone();
        // Edição na linha 2: só o sufixo muda.
        let mut novo = texto.to_string();
        novo.replace_range(6..8, "LONGA");
        tabela.recalc_a_partir(&novo, 2);
        assert_eq!(&tabela.inicios[..3], &antes[..3]);
        assert_eq!(tabela.inicio_da_linha(3), 12);
    }
}
