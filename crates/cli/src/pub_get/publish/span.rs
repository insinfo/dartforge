//! `SourceFile.fromString(text, url:).span(start, end).message(message)` of
//! `package:source_span` (the revision of the 3.6.2 SDK) for one span with
//! no color: the `line L, column C of <url>: <message>` line and the
//! highlighter's drawing with Unicode glyphs. Offsets are UTF-16 code units,
//! as in Dart.
use super::paths as p;

const DOWN_END: &str = "╷";
const UP_END: &str = "╵";
const VERTICAL: &str = "│";
const TOP_LEFT: &str = "┌";
const BOTTOM_LEFT: &str = "└";
const HORIZONTAL: &str = "─";
const SPACES_PER_TAB: usize = 4;
const LF: u16 = b'\n' as u16;
const CR: u16 = b'\r' as u16;
const TAB: u16 = b'\t' as u16;

/// A text split into lines like `SourceFile` (`\n`, `\r\n` and a lone `\r`).
pub(crate) struct SourceFile {
    chars: Vec<u16>,
    line_starts: Vec<usize>,
    url: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Location {
    offset: usize,
    line: usize,
    column: usize,
}

#[derive(Clone, Debug)]
struct Span {
    start: Location,
    end: Location,
    text: Vec<u16>,
    context: Vec<u16>,
}

fn index_of(haystack: &[u16], needle: &[u16], from: usize) -> Option<usize> {
    if needle.is_empty() {
        return (from <= haystack.len()).then_some(from);
    }
    if needle.len() > haystack.len() {
        return None;
    }
    (from..=haystack.len() - needle.len()).find(|&i| haystack[i..i + needle.len()] == *needle)
}

fn last_index_of(haystack: &[u16], c: u16, before_or_at: usize) -> Option<usize> {
    if haystack.is_empty() {
        return None;
    }
    let end = before_or_at.min(haystack.len() - 1);
    (0..=end).rev().find(|&i| haystack[i] == c)
}

fn count(text: &[u16], c: u16) -> usize {
    text.iter().filter(|&&x| x == c).count()
}

/// `findLineStart` of `utils.dart`.
fn find_line_start(context: &[u16], text: &[u16], column: usize) -> Option<usize> {
    if text.is_empty() {
        let mut beginning = 0;
        loop {
            match index_of(context, &[LF], beginning) {
                None => {
                    return (context.len() - beginning >= column).then_some(beginning);
                }
                Some(index) => {
                    if index - beginning >= column {
                        return Some(beginning);
                    }
                    beginning = index + 1;
                }
            }
        }
    }
    let mut index = index_of(context, text, 0);
    while let Some(i) = index {
        let line_start = if i == 0 {
            0
        } else {
            last_index_of(context, LF, i - 1).map_or(0, |x| x + 1)
        };
        if column == i - line_start {
            return Some(line_start);
        }
        index = index_of(context, text, i + 1);
    }
    None
}

fn last_line_length(text: &[u16]) -> usize {
    if text.is_empty() {
        0
    } else if text[text.len() - 1] == LF {
        if text.len() == 1 {
            0
        } else {
            text.len()
                - last_index_of(text, LF, text.len() - 2).map_or(-1i64, |x| x as i64) as usize
                - 1
        }
    } else {
        (text.len() as i64 - last_index_of(text, LF, text.len()).map_or(-1i64, |x| x as i64) - 1)
            as usize
    }
}

impl SourceFile {
    pub fn new(text: &str, url: &str) -> Self {
        let chars: Vec<u16> = text.encode_utf16().collect();
        let mut line_starts = vec![0];
        for i in 0..chars.len() {
            let mut c = chars[i];
            if c == CR && (i + 1 >= chars.len() || chars[i + 1] != LF) {
                c = LF;
            }
            if c == LF {
                line_starts.push(i + 1);
            }
        }
        Self {
            chars,
            line_starts,
            url: url.to_owned(),
        }
    }

    fn line(&self, offset: usize) -> usize {
        match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i - 1,
        }
    }

    fn location(&self, offset: usize) -> Location {
        let line = self.line(offset);
        Location {
            offset,
            line,
            column: offset - self.line_starts[line],
        }
    }

    /// `_FileSpan.context`.
    fn context(&self, start: usize, end: usize) -> Vec<u16> {
        let end_line = self.line(end);
        let end_column = end - self.line_starts[end_line];
        let lines = self.line_starts.len();
        let end_offset = if end_column == 0 && end_line != 0 {
            if start == end {
                return if end_line == lines - 1 {
                    Vec::new()
                } else {
                    self.chars[self.line_starts[end_line]..self.line_starts[end_line + 1]].to_vec()
                };
            }
            end
        } else if end_line == lines - 1 {
            self.chars.len()
        } else {
            self.line_starts[end_line + 1]
        };
        self.chars[self.line_starts[self.line(start)]..end_offset].to_vec()
    }

    /// `span(start, end).message(message)`.
    pub fn message(&self, start: usize, end: usize, message: &str) -> String {
        let s = self.location(start);
        let span = Span {
            start: s,
            end: self.location(end),
            text: self.chars[start..end].to_vec(),
            context: self.context(start, end),
        };
        format!(
            "line {}, column {} of {}: {message}\n{}",
            s.line + 1,
            s.column + 1,
            pretty_uri(&self.url),
            highlight(span)
        )
    }
}

/// `p.prettyUri(Uri.parse(url))` for a file path.
fn pretty_uri(url: &str) -> String {
    let path = p::normalize(&percent_decode(url));
    let relative = p::relative_to_current(&path);
    if p::split(&relative).len() > p::split(&path).len() {
        path
    } else {
        relative
    }
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(v) = u8::from_str_radix(&text[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn normalize(mut span: Span) -> Span {
    // _normalizeContext
    if find_line_start(&span.context, &span.text, span.start.column).is_none() {
        span = Span {
            start: Location {
                offset: span.start.offset,
                line: 0,
                column: 0,
            },
            end: Location {
                offset: span.end.offset,
                line: count(&span.text, LF),
                column: last_line_length(&span.text),
            },
            context: span.text.clone(),
            text: span.text,
        };
    }
    // _normalizeNewlines
    let crlf = span
        .text
        .windows(2)
        .filter(|w| w[0] == CR && w[1] == LF)
        .count();
    if crlf > 0 {
        let replace = |v: &[u16]| -> Vec<u16> {
            let mut out = Vec::with_capacity(v.len());
            let mut i = 0;
            while i < v.len() {
                if v[i] == CR && v.get(i + 1) == Some(&LF) {
                    i += 1;
                    continue;
                }
                out.push(v[i]);
                i += 1;
            }
            out
        };
        span.end.offset -= crlf;
        span.text = replace(&span.text);
        span.context = replace(&span.context);
    }
    // _normalizeTrailingNewline
    if span.context.last() == Some(&LF) && !span.text.ends_with(&[LF, LF]) {
        let at_end = find_line_start(&span.context, &span.text, span.start.column)
            .is_some_and(|ls| ls + span.start.column + span.text.len() == span.context.len());
        let context = span.context[..span.context.len() - 1].to_vec();
        let mut text = span.text.clone();
        let mut start = span.start;
        let mut end = span.end;
        if span.text.last() == Some(&LF) && at_end {
            text.pop();
            if text.is_empty() {
                end = start;
            } else {
                end = Location {
                    offset: span.end.offset - 1,
                    line: span.end.line - 1,
                    column: last_line_length(&context),
                };
                if span.start.offset == span.end.offset {
                    start = end;
                }
            }
        }
        span = Span {
            start,
            end,
            text,
            context,
        };
    }
    // _normalizeEndOfLine
    if span.end.column == 0 && span.end.line != span.start.line {
        let text = span.text[..span.text.len() - 1].to_vec();
        let column = (text.len() as i64
            - last_index_of(&text, LF, text.len()).map_or(-1, |x| x as i64)
            - 1) as usize;
        let context = if span.context.last() == Some(&LF) {
            span.context[..span.context.len() - 1].to_vec()
        } else {
            span.context.clone()
        };
        span = Span {
            start: span.start,
            end: Location {
                offset: span.end.offset - 1,
                line: span.end.line - 1,
                column,
            },
            text,
            context,
        };
    }
    span
}

struct Line {
    text: Vec<u16>,
    number: usize,
    highlighted: bool,
}

struct Writer {
    out: String,
    padding: usize,
}

impl Writer {
    fn sidebar(&mut self, text: &str, end: &str) {
        self.out.push_str(text);
        for _ in text.chars().count()..self.padding {
            self.out.push(' ');
        }
        self.out.push_str(end);
    }

    fn text(&mut self, text: &[u16]) {
        for &c in text {
            if c == TAB {
                self.out.push_str(&" ".repeat(SPACES_PER_TAB));
            } else {
                self.out.push_str(&String::from_utf16_lossy(&[c]));
            }
        }
    }
}

fn tabs(text: &[u16]) -> usize {
    count(text, TAB)
}

/// `Highlighter(span).highlight()` for one primary span without a label.
fn highlight(span: Span) -> String {
    let span = normalize(span);
    let multiline = span.start.line != span.end.line;
    let line_start = find_line_start(&span.context, &span.text, span.start.column).unwrap_or(0);
    let before = count(&span.context[..line_start], LF);
    let first = span.start.line as i64 - before as i64;
    let mut lines: Vec<Line> = Vec::new();
    for (i, text) in span.context.split(|&c| c == LF).enumerate() {
        let number = first + i as i64;
        if lines.last().is_none_or(|l| number > l.number as i64) {
            let n = number.max(0) as usize;
            lines.push(Line {
                text: text.to_vec(),
                number: n,
                highlighted: false,
            });
        }
    }
    for line in &mut lines {
        line.highlighted = span.start.line <= line.number && line.number <= span.end.line;
    }
    let last = lines.last().map_or(0, |l| l.number);
    let mut w = Writer {
        out: String::new(),
        padding: 1 + (last + 1).to_string().len(),
    };
    w.sidebar("", DOWN_END);
    w.out.push('\n');
    // `highlightsByColumn`: one column for a multi-line span, none otherwise.
    let columns = usize::from(multiline);
    let mut column: Option<()> = None;
    for line in &lines {
        let has = line.highlighted;
        if has
            && multiline
            && span.start.line == line.number
            && line.text[..span.start.column.min(line.text.len())]
                .iter()
                .all(|&c| c == b' ' as u16 || c == TAB)
        {
            column = Some(());
        }
        w.sidebar(&(line.number + 1).to_string(), VERTICAL);
        w.out.push(' ');
        multiline_highlights(&mut w, line, &span, column, false);
        if columns > 0 {
            w.out.push(' ');
        }
        w.text(&line.text);
        w.out.push('\n');
        if has {
            indicator(&mut w, line, &span, &mut column, columns);
        }
    }
    w.sidebar("", UP_END);
    w.out
}

/// `_writeMultilineHighlights` with at most the one span in its column.
fn multiline_highlights(
    w: &mut Writer,
    line: &Line,
    span: &Span,
    column: Option<()>,
    current: bool,
) {
    if !(span.start.line != span.end.line) {
        return;
    }
    match column {
        None => w.out.push(' '),
        Some(()) => {
            if current {
                w.out.push_str(if span.start.line == line.number {
                    TOP_LEFT
                } else {
                    BOTTOM_LEFT
                });
            } else if span.start.line == line.number {
                w.out.push_str(TOP_LEFT);
            } else if span.end.line == line.number && span.end.column == line.text.len() {
                w.out.push_str(BOTTOM_LEFT);
            } else {
                w.out.push_str(VERTICAL);
            }
        }
    }
}

fn arrow(w: &mut Writer, line: &Line, column: usize, beginning: bool) {
    let upto = (column + usize::from(!beginning)).min(line.text.len());
    let tabs = tabs(&line.text[..upto]);
    w.out
        .push_str(&HORIZONTAL.repeat(1 + column + tabs * (SPACES_PER_TAB - 1)));
    w.out.push('^');
}

fn indicator(w: &mut Writer, line: &Line, span: &Span, column: &mut Option<()>, columns: usize) {
    if span.start.line == span.end.line {
        w.sidebar("", VERTICAL);
        w.out.push(' ');
        if columns > 0 {
            w.out.push(' ');
        }
        let mut start = span.start.column;
        let mut end = span.end.column;
        let before = tabs(&line.text[..start.min(line.text.len())]);
        let inside = tabs(&line.text[start.min(line.text.len())..end.min(line.text.len())]);
        start += before * (SPACES_PER_TAB - 1);
        end += (before + inside) * (SPACES_PER_TAB - 1);
        w.out.push_str(&" ".repeat(start));
        w.out
            .push_str(&"^".repeat(end.saturating_sub(start).max(1)));
        w.out.push('\n');
    } else if span.start.line == line.number {
        if column.is_some() && line_has_column_from_start(span, line) {
            return;
        }
        *column = Some(());
        w.sidebar("", VERTICAL);
        w.out.push(' ');
        multiline_highlights(w, line, span, *column, true);
        arrow(w, line, span.start.column, true);
        w.out.push('\n');
    } else if span.end.line == line.number {
        let whole = span.end.column == line.text.len();
        if whole {
            *column = None;
            return;
        }
        w.sidebar("", VERTICAL);
        w.out.push(' ');
        multiline_highlights(w, line, span, *column, true);
        arrow(w, line, span.end.column.saturating_sub(1), false);
        w.out.push('\n');
        *column = None;
    }
}

/// Whether the column was taken on the span's first line before its
/// indicator (`highlightsByColumn.contains(highlight)`): it is when the text
/// before the span on that line is only whitespace.
fn line_has_column_from_start(span: &Span, line: &Line) -> bool {
    line.text[..span.start.column.min(line.text.len())]
        .iter()
        .all(|&c| c == b' ' as u16 || c == TAB)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_span() {
        let file = SourceFile::new(
            "import 'a.dart';\nimport 'package:x/y.dart';\n",
            "lib/a.dart",
        );
        assert_eq!(
            file.message(17, 43, "Problem."),
            "line 2, column 1 of lib/a.dart: Problem.\n  ╷\n2 │ import 'package:x/y.dart';\n  │ ^^^^^^^^^^^^^^^^^^^^^^^^^^\n  ╵"
        );
    }

    #[test]
    fn multi_line_span() {
        let text = "a\nimport 'package:x/y.dart'\n    show z;\n";
        let file = SourceFile::new(text, "lib/a.dart");
        let start = 2;
        let end = text.find(';').unwrap() + 1;
        assert_eq!(
            file.message(start, end, "M."),
            "line 2, column 1 of lib/a.dart: M.\n  ╷\n2 │ ┌ import 'package:x/y.dart'\n3 │ └     show z;\n  ╵"
        );
        // A span that starts after other text and ends inside a line.
        let text = "x; import 'a'\n  show b; // c\n";
        let file = SourceFile::new(text, "lib/a.dart");
        let start = 3;
        let end = text
            .find(';')
            .map(|_| text.rfind(';').unwrap() + 1)
            .unwrap();
        assert_eq!(
            file.message(start, end, "M."),
            "line 1, column 4 of lib/a.dart: M.\n  ╷\n1 │   x; import 'a'\n  │ ┌────^\n2 │ │   show b; // c\n  │ └─────────^\n  ╵"
        );
    }
}
