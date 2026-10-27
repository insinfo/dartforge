//! Conversion between UTF-8 byte offsets and LSP positions (line, UTF-16 column).
//!
//! The LSP protocol counts columns in **UTF-16 units**, while the in-memory
//! text is UTF-8 (`String`). An emoji such as `👭` takes 4 bytes in UTF-8
//! and 2 units in UTF-16; mixing up the two counts edits the wrong byte and
//! publishes the squiggle at the wrong column. This module is the only
//! boundary where that conversion happens, in both directions:
//!
//! * client → server (`didChange`): a UTF-16 position becomes a byte offset;
//! * server → client (`publishDiagnostics`): a byte span becomes a position.
//!
//! Accepted line breaks: `\n`, `\r\n` and a lone `\r`, like VS Code.
//! Positions outside the document are saturated (never a `panic`); a column
//! in the middle of a surrogate pair (between the two units of an astral
//! character) is truncated to the character boundary, because the protocol
//! requires a character boundary.
//!
//! ```
//! let table = dartforge_lsp::utf16::LineTable::build("var s = '👭';\nint ;\n");
//! // Line 0 has 9 ASCII + 2 (emoji) + 2 = 13 UTF-16 units before the break.
//! assert_eq!(table.line_width("var s = '👭';\nint ;\n", 0), 13);
//! ```

use std::cmp::min;

/// Index of the line starts of a document, in UTF-8 bytes.
///
/// `starts[0]` is always 0; each following entry is the first byte after a
/// line break. The table is recomputed **only from the edited point on**
/// ([`LineTable::recompute_from`]): an intact prefix is not revisited.
#[derive(Debug, Clone, Default)]
pub struct LineTable {
    starts: Vec<usize>,
}

impl LineTable {
    /// Builds the table for the whole text (document opening).
    pub fn build(text: &str) -> Self {
        let mut table = Self { starts: vec![0] };
        table.scan(text, 0);
        table
    }

    /// Scans `text` from `base`, pushing line starts.
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

    /// How many lines the document has (the last one may be empty after `\n`).
    pub fn total_lines(&self) -> usize {
        self.starts.len()
    }

    /// Line that contains the byte offset (saturated to the end of the text).
    pub fn line_of(&self, text: &str, offset: usize) -> usize {
        let offset = min(offset, text.len());
        self.starts.partition_point(|start| *start <= offset) - 1
    }

    /// Byte where the line starts.
    pub fn line_start(&self, line: usize) -> usize {
        self.starts[min(line, self.starts.len() - 1)]
    }

    /// Byte where the line content ends, without the break (`\r\n`, `\n`, `\r`).
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

    /// Recomputes the starts from `line` on, after an edit already applied.
    ///
    /// Contracts: `text` already contains the edit; `line` is the line where the
    /// edit starts in the new text; the prefix `starts[..line]` is intact.
    /// Cost proportional to the suffix, never to the prefix.
    pub fn recompute_from(&mut self, text: &str, line: usize) {
        let line = min(line, self.starts.len().saturating_sub(1));
        let base = self.starts[line];
        self.starts.truncate(line + 1);
        self.scan(text, base);
    }

    /// Converts a byte offset into (line, column in UTF-16 units).
    ///
    /// An offset outside the text saturates to the end; an offset in the middle
    /// of a character goes down to the previous boundary.
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

    /// Converts (line, column in UTF-16 units) into a byte offset.
    ///
    /// A line past the end saturates to the end of the text; a column past the
    /// end of the line saturates to the end of the content (before the break); a
    /// column in the middle of a surrogate pair truncates to the character
    /// boundary.
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
                break; // Middle of an astral character: stop at the character boundary.
            }
            consumed += width;
            offset += c.len_utf8();
        }
        offset
    }

    /// A converter for many offsets visited in increasing order (see
    /// [`PositionCursor`]).
    pub fn cursor<'a>(&'a self, text: &'a str) -> PositionCursor<'a> {
        PositionCursor {
            table: self,
            text,
            offset: 0,
            line: 0,
            column: 0,
        }
    }

    /// Width of the line in UTF-16 units, without the break (used in tests).
    pub fn line_width(&self, text: &str, line: usize) -> usize {
        let line = min(line, self.starts.len() - 1);
        text[self.starts[line]..self.line_end(text, line)]
            .chars()
            .map(|c| c.len_utf16())
            .sum()
    }
}

/// Sequential [`LineTable::offset_position`]: remembers the last converted
/// offset and, when the next one is on the same line and not before it, only
/// counts the UTF-16 units in between.
///
/// [`LineTable::offset_position`] costs the column of the offset, so
/// converting every edit of a long single-line document (a formatted
/// 80 000-term expression) would be quadratic; with the offsets in increasing
/// order the whole pass is linear in the text. An offset before the previous
/// one is still correct, at the cost of [`LineTable::offset_position`].
///
/// ```
/// let text = "a👭b\ncd";
/// let table = dartforge_lsp::utf16::LineTable::build(text);
/// let mut cursor = table.cursor(text);
/// assert_eq!(cursor.position(5), (0, 3));
/// assert_eq!(cursor.position(6), (0, 4));
/// assert_eq!(cursor.position(8), (1, 1));
/// assert_eq!(cursor.position(1), (0, 1));
/// ```
#[derive(Debug, Clone)]
pub struct PositionCursor<'a> {
    table: &'a LineTable,
    text: &'a str,
    offset: usize,
    line: usize,
    column: usize,
}

impl PositionCursor<'_> {
    /// The (line, UTF-16 column) of `offset`, with the same saturation and
    /// boundary rules as [`LineTable::offset_position`].
    pub fn position(&mut self, offset: usize) -> (u32, u32) {
        let text = self.text;
        let mut offset = min(offset, text.len());
        while !text.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = self.table.line_of(text, offset);
        let (from, column) = if line == self.line && offset >= self.offset {
            (self.offset, self.column)
        } else {
            (self.table.line_start(line), 0)
        };
        let column = column
            + text[from..offset]
                .chars()
                .map(char::len_utf16)
                .sum::<usize>();
        self.offset = offset;
        self.line = line;
        self.column = column;
        (line as u32, column as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_agrees_with_offset_position() {
        let text = "x👭 y\r\nz\rw é\n";
        let table = LineTable::build(text);
        let mut cursor = table.cursor(text);
        for offset in (0..=text.len() + 2).chain([3, 0, 9, 2]) {
            assert_eq!(
                cursor.position(offset),
                table.offset_position(text, offset),
                "{offset}"
            );
        }
    }

    #[test]
    fn emoji_counts_two_units_both_ways() {
        let text = "var s = '👭';\nint ;\n";
        let table = LineTable::build(text);
        assert_eq!(table.total_lines(), 3);
        assert_eq!(table.line_width(text, 0), 13);
        // The `;` after the emoji: byte 13, UTF-16 unit 11.
        assert_eq!(table.offset_position(text, 13), (0, 11));
        assert_eq!(table.position_offset(text, 0, 11), 13);
        // Column 10 falls in the middle of the surrogate pair: truncates to the boundary (byte 9).
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
        // Edit on line 2: only the suffix changes.
        let mut new = text.to_string();
        new.replace_range(6..8, "LONGA");
        table.recompute_from(&new, 2);
        assert_eq!(&table.starts[..3], &before[..3]);
        assert_eq!(table.line_start(3), 12);
    }
}
