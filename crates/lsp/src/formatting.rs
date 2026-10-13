//! Document formatting: `textDocument/formatting`,
//! `textDocument/rangeFormatting` and `textDocument/onTypeFormatting`, like
//! the Dart 3.6.2 server's `FormattingHandler`, `FormatRangeHandler` and
//! `FormatOnTypeHandler` (`generateEditsForFormatting`,
//! `lsp/source_edits.dart`).
//!
//! The text is formatted by the `dart_style` port ([`dartforge_formatter`]);
//! the response is the minimal whitespace edits between tokens
//! ([`dartforge_formatter::edicoes::edicoes_minimas`]). Without changes, or
//! with a syntax error, the response is `null`.

use dartforge_formatter::edicoes::edicoes_minimas;
use dartforge_formatter::{Opcoes, formatar};
use serde_json::{Value, json};

use crate::utf16::LineTable;

/// The first character that triggers formatting while typing, and the others
/// (`dartTypeFormattingCharacters`, `constants.dart:46`).
pub const TRIGGER_CHARACTERS: [&str; 2] = ["}", ";"];

/// The formatting edits of `text`, as JSON (`List<TextEdit>?`).
///
/// `width` is the client's `dart.lineLength` (80 without it). `range`, in
/// bytes, restricts the result to the edits entirely inside it
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

/// The Dart server's `toOffset(lineInfo, position)`, in bytes: `None` for a
/// line past the last one; the column (UTF-16) is added to the line start
/// without stopping at the line break, saturating at the end of the text.
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
