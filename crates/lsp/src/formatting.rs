//! Formatação de documentos: `textDocument/formatting`,
//! `textDocument/rangeFormatting` e `textDocument/onTypeFormatting`, como os
//! `FormattingHandler`, `FormatRangeHandler` e `FormatOnTypeHandler` do
//! servidor do Dart 3.6.2 (`generateEditsForFormatting`,
//! `lsp/source_edits.dart`).
//!
//! O texto é formatado pelo porte do `dart_style` ([`dartforge_formatter`]); a
//! resposta são as edições mínimas de brancos entre os tokens
//! ([`dartforge_formatter::edits::edicoes_minimas`]). Sem mudança, ou com
//! erro de sintaxe, a resposta é `null`.

use dartforge_formatter::edicoes::edicoes_minimas;
use dartforge_formatter::{Opcoes, formatar};
use serde_json::{Value, json};

use crate::utf16::LineTable;

/// O primeiro caractere que dispara a formatação ao digitar e os demais
/// (`dartTypeFormattingCharacters`, `constants.dart:46`).
pub const TRIGGER_CHARACTERS: [&str; 2] = ["}", ";"];

/// As edições de formatação de `text`, em JSON (`List<TextEdit>?`).
///
/// `width` é o `dart.lineLength` do cliente (80 sem ele). `range`, em
/// bytes, restringe às edições inteiramente dentro dela
/// (`rangeFormatting`).
///
/// ```
/// let e = dartforge_lsp::formatting::edits("main( ){}", None, None);
/// assert_eq!(e.as_array().unwrap().len(), 3);
/// assert!(dartforge_lsp::formatting::edits("main() {}\n", None, None).is_null());
/// ```
pub fn edits(text: &str, width: Option<u64>, range: Option<(usize, usize)>) -> Value {
    let options = Opcoes {
        largura: width.map_or(80, |l| l as usize),
        ..Opcoes::default()
    };
    let Ok(Ok(formatted)) = std::panic::catch_unwind(|| formatar(text, &options)) else {
        return Value::Null;
    };
    if formatted == text {
        return Value::Null;
    }
    let table = LineTable::build(text);
    let position = |o: usize| {
        let (l, c) = table.offset_position(text, o);
        json!({"line": l, "character": c})
    };
    let list: Vec<Value> = edicoes_minimas(text, &formatted, range)
        .into_iter()
        .map(|e| json!({"range": {"start": position(e.inicio), "end": position(e.fim)}, "newText": e.texto}))
        .collect();
    json!(list)
}

/// `toOffset(lineInfo, position)` do servidor do Dart, em bytes: `None` para
/// linha além da última; a coluna (UTF-16) soma ao início da linha sem
/// parar na quebra, saturando no fim do texto.
///
/// ```
/// use dartforge_lsp::formatting::offset_without_limit;
/// assert_eq!(offset_without_limit("ab\ncd\n", 0, 4), Some(4));
/// assert_eq!(offset_without_limit("ab\ncd\n", 3, 0), None);
/// ```
pub fn offset_without_limit(text: &str, line: u64, column: u64) -> Option<usize> {
    let table = LineTable::build(text);
    if line as usize >= table.total_lines() {
        return None;
    }
    let mut o = table.line_start(line as usize);
    let mut remaining = column as usize;
    for c in text[o..].chars() {
        if remaining == 0 {
            break;
        }
        remaining = remaining.saturating_sub(c.len_utf16());
        o += c.len_utf8();
    }
    Some(o)
}
