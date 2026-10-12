//! Formatação de documentos: `textDocument/formatting`,
//! `textDocument/rangeFormatting` e `textDocument/onTypeFormatting`, como os
//! `FormattingHandler`, `FormatRangeHandler` e `FormatOnTypeHandler` do
//! servidor do Dart 3.6.2 (`generateEditsForFormatting`,
//! `lsp/source_edits.dart`).
//!
//! O texto é formatado pelo porte do `dart_style` ([`dartforge_formatter`]); a
//! resposta são as edições mínimas de brancos entre os tokens
//! ([`dartforge_formatter::edicoes::edicoes_minimas`]). Sem mudança, ou com
//! erro de sintaxe, a resposta é `null`.

use dartforge_formatter::edicoes::edicoes_minimas;
use dartforge_formatter::{Opcoes, formatar};
use serde_json::{Value, json};

use crate::utf16::TabelaLinhas;

/// O primeiro caractere que dispara a formatação ao digitar e os demais
/// (`dartTypeFormattingCharacters`, `constants.dart:46`).
pub const CARACTERES_AO_DIGITAR: [&str; 2] = ["}", ";"];

/// As edições de formatação de `texto`, em JSON (`List<TextEdit>?`).
///
/// `largura` é o `dart.lineLength` do cliente (80 sem ele). `faixa`, em
/// bytes, restringe às edições inteiramente dentro dela
/// (`rangeFormatting`).
///
/// ```
/// let e = dartforge_lsp::formatting::edicoes("main( ){}", None, None);
/// assert_eq!(e.as_array().unwrap().len(), 3);
/// assert!(dartforge_lsp::formatting::edicoes("main() {}\n", None, None).is_null());
/// ```
pub fn edicoes(texto: &str, largura: Option<u64>, faixa: Option<(usize, usize)>) -> Value {
    let opcoes = Opcoes {
        largura: largura.map_or(80, |l| l as usize),
        ..Opcoes::default()
    };
    let Ok(Ok(formatado)) = std::panic::catch_unwind(|| formatar(texto, &opcoes)) else {
        return Value::Null;
    };
    if formatado == texto {
        return Value::Null;
    }
    let tabela = TabelaLinhas::construir(texto);
    let posicao = |o: usize| {
        let (l, c) = tabela.posicao_de_offset(texto, o);
        json!({"line": l, "character": c})
    };
    let lista: Vec<Value> = edicoes_minimas(texto, &formatado, faixa)
        .into_iter()
        .map(|e| json!({"range": {"start": posicao(e.inicio), "end": posicao(e.fim)}, "newText": e.texto}))
        .collect();
    json!(lista)
}

/// `toOffset(lineInfo, position)` do servidor do Dart, em bytes: `None` para
/// linha além da última; a coluna (UTF-16) soma ao início da linha sem
/// parar na quebra, saturando no fim do texto.
///
/// ```
/// use dartforge_lsp::formatting::offset_sem_limite;
/// assert_eq!(offset_sem_limite("ab\ncd\n", 0, 4), Some(4));
/// assert_eq!(offset_sem_limite("ab\ncd\n", 3, 0), None);
/// ```
pub fn offset_sem_limite(texto: &str, linha: u64, coluna: u64) -> Option<usize> {
    let tabela = TabelaLinhas::construir(texto);
    if linha as usize >= tabela.total_linhas() {
        return None;
    }
    let mut o = tabela.inicio_da_linha(linha as usize);
    let mut restante = coluna as usize;
    for c in texto[o..].chars() {
        if restante == 0 {
            break;
        }
        restante = restante.saturating_sub(c.len_utf16());
        o += c.len_utf8();
    }
    Some(o)
}
