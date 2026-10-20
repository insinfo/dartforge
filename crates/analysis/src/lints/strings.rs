//! String literals and comments as the text writes them, for the lint rules
//! that look at quotes, escapes and lines: the tree keeps one `StringLit` per
//! expression (adjacent strings joined, the text already decoded), and these
//! rules ask about each literal (the analyzer's `SimpleStringLiteral` or
//! `StringInterpolation`) and its lexeme.
//!
//! Written without compiling or running (2026-10-05).

use dartforge_diagnostics::Span;

/// A string literal: `'…'`, `"…"`, `'''…'''`, `"""…"""`, with or without `r`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Literal {
    /// From the `r` or the opening quote to after the closing quote.
    pub span: Span,
    pub raw: bool,
    /// Three quotes.
    pub multiline: bool,
    /// Single quotes (`'` or `'''`).
    pub single_quoted: bool,
    /// The text parts, in order: the content between the quotes, cut by the
    /// interpolations. There is always at least one (it may be empty).
    pub parts: Vec<Span>,
    /// The interpolations, from the `$` to the end of the identifier or the `}`.
    pub interpolations: Vec<Span>,
}

impl Literal {
    /// Has an interpolation (the analyzer's `StringInterpolation`).
    pub fn is_interpolated(&self) -> bool {
        !self.interpolations.is_empty()
    }

    /// The text of the parts, as written (escapes not decoded).
    pub fn text(&self, source: &str) -> String {
        self.parts
            .iter()
            .filter_map(|t| source.get(t.start..t.end))
            .collect()
    }
}

fn is_ident_char(c: u8) -> bool {
    c == b'_' || c == b'$' || c.is_ascii_alphanumeric()
}

/// The position after the (nestable) block comment that starts at `i`.
fn block_end(b: &[u8], mut i: usize, end: usize) -> usize {
    let mut depth = 0usize;
    while i < end {
        if i + 1 < end && b[i] == b'/' && b[i + 1] == b'*' {
            depth += 1;
            i += 2;
        } else if i + 1 < end && b[i] == b'*' && b[i + 1] == b'/' {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return i;
            }
        } else {
            i += 1;
        }
    }
    end
}

/// Skips whitespace and comments.
fn skip_blanks(b: &[u8], mut i: usize, end: usize) -> usize {
    loop {
        while i < end && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i + 1 < end && b[i] == b'/' && b[i + 1] == b'/' {
            while i < end && b[i] != b'\n' {
                i += 1;
            }
        } else if i + 1 < end && b[i] == b'/' && b[i + 1] == b'*' {
            i = block_end(b, i, end);
        } else {
            return i;
        }
    }
}

/// The position after the `}` that closes the interpolation whose body starts
/// at `i`. Strings and comments inside are skipped whole.
fn closing_brace(b: &[u8], mut i: usize, end: usize) -> usize {
    let mut depth = 1usize;
    while i < end {
        match b[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            b'/' if i + 1 < end && (b[i + 1] == b'/' || b[i + 1] == b'*') => {
                i = skip_blanks(b, i, end);
                continue;
            }
            b'\'' | b'"' => {
                let raw = i > 0 && b[i - 1] == b'r' && (i < 2 || !is_ident_char(b[i - 2]));
                if let Some((_, after)) = literal(b, if raw { i - 1 } else { i }, end) {
                    i = after;
                    continue;
                }
            }
            _ => {}
        }
        i += 1;
    }
    end
}

/// Reads the literal that starts at `i` (at the `r` or the quote); returns it
/// with the next position. A literal that does not close runs to `end`.
fn literal(b: &[u8], start: usize, end: usize) -> Option<(Literal, usize)> {
    let mut i = start;
    let raw = b.get(i) == Some(&b'r');
    if raw {
        i += 1;
    }
    let quote = *b.get(i).filter(|_| i < end)?;
    if quote != b'\'' && quote != b'"' {
        return None;
    }
    let triple = i + 2 < end && b[i + 1] == quote && b[i + 2] == quote;
    let n = if triple { 3 } else { 1 };
    i += n;
    let mut parts = Vec::new();
    let mut interpolations = Vec::new();
    let mut part_start = i;
    let build = |parts: Vec<Span>, interpolations: Vec<Span>, until: usize| Literal {
        span: Span { start, end: until },
        raw,
        multiline: triple,
        single_quoted: quote == b'\'',
        parts,
        interpolations,
    };
    while i < end {
        let ch = b[i];
        if ch == quote && (!triple || (i + 2 < end && b[i + 1] == quote && b[i + 2] == quote)) {
            parts.push(Span {
                start: part_start,
                end: i,
            });
            return Some((build(parts, interpolations, i + n), i + n));
        }
        if !raw && ch == b'\\' {
            i += 2;
            continue;
        }
        if !raw && ch == b'$' {
            let next = b.get(i + 1).copied().filter(|_| i + 1 < end);
            if next == Some(b'{') {
                parts.push(Span {
                    start: part_start,
                    end: i,
                });
                let after = closing_brace(b, i + 2, end);
                interpolations.push(Span {
                    start: i,
                    end: after,
                });
                i = after;
                part_start = i;
                continue;
            }
            if next.is_some_and(|c| c == b'_' || c.is_ascii_alphabetic()) {
                parts.push(Span {
                    start: part_start,
                    end: i,
                });
                let mut j = i + 1;
                while j < end && (b[j] == b'_' || b[j].is_ascii_alphanumeric()) {
                    j += 1;
                }
                interpolations.push(Span { start: i, end: j });
                i = j;
                part_start = i;
                continue;
            }
        }
        i += 1;
    }
    parts.push(Span {
        start: part_start.min(end),
        end,
    });
    Some((build(parts, interpolations, end), end))
}

/// The literals written in `span` (the range of a string expression: one
/// literal, or several adjacent ones separated by whitespace and comments).
pub fn literals(source: &str, span: Span) -> Vec<Literal> {
    let b = source.as_bytes();
    let end = span.end.min(b.len());
    let mut i = span.start;
    let mut v = Vec::new();
    loop {
        i = skip_blanks(b, i, end);
        if i >= end {
            break;
        }
        match literal(b, i, end) {
            Some((l, after)) => {
                v.push(l);
                i = after;
            }
            None => break,
        }
    }
    v
}

/// The source's comments (line and block), outside strings.
pub fn comments(source: &str) -> Vec<Span> {
    let b = source.as_bytes();
    let end = b.len();
    let mut i = 0;
    let mut v = Vec::new();
    while i < end {
        let ch = b[i];
        if ch == b'/' && b.get(i + 1) == Some(&b'/') {
            let start = i;
            while i < end && b[i] != b'\n' && b[i] != b'\r' {
                i += 1;
            }
            v.push(Span { start, end: i });
            continue;
        }
        if ch == b'/' && b.get(i + 1) == Some(&b'*') {
            let start = i;
            i = block_end(b, i, end);
            v.push(Span { start, end: i });
            continue;
        }
        if ch == b'\'' || ch == b'"' {
            let raw = i > 0 && b[i - 1] == b'r' && (i < 2 || !is_ident_char(b[i - 2]));
            if let Some((_, after)) = literal(b, if raw { i - 1 } else { i }, end) {
                i = after;
                continue;
            }
        }
        i += 1;
    }
    v
}

/// The value of a plain string part (Dart's escapes decoded: `\n`, `\r`, `\f`,
/// `\b`, `\t`, `\v`, `\xHH`, `\uHHHH`, `\u{H…}`; any other `\c` is `c`). A lone
/// surrogate escape produces no character (it is neither a quote nor a
/// backslash).
pub fn decode(text: &str) -> String {
    let cs: Vec<char> = text.chars().collect();
    let mut s = String::with_capacity(text.len());
    let hex = |from: usize, until: usize| -> Option<u32> {
        let h: String = cs.get(from..until)?.iter().collect();
        u32::from_str_radix(&h, 16).ok()
    };
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c != '\\' || i + 1 >= cs.len() {
            s.push(c);
            i += 1;
            continue;
        }
        let d = cs[i + 1];
        i += 2;
        match d {
            'n' => s.push('\n'),
            'r' => s.push('\r'),
            'f' => s.push('\u{c}'),
            'b' => s.push('\u{8}'),
            't' => s.push('\t'),
            'v' => s.push('\u{b}'),
            'x' => {
                if let Some(ch) = hex(i, i + 2).and_then(char::from_u32) {
                    s.push(ch);
                }
                i += 2;
            }
            'u' => {
                if cs.get(i) == Some(&'{') {
                    let close = cs[i..].iter().position(|x| *x == '}').map(|k| i + k);
                    if let Some(f) = close {
                        if let Some(ch) = hex(i + 1, f).and_then(char::from_u32) {
                            s.push(ch);
                        }
                        i = f + 1;
                    } else {
                        i = cs.len();
                    }
                } else {
                    if let Some(ch) = hex(i, i + 4).and_then(char::from_u32) {
                        s.push(ch);
                    }
                    i += 4;
                }
            }
            other => s.push(other),
        }
    }
    s
}

/// `StringLiteral.stringValue`: the value of the adjacent literals joined, or
/// `None` if any has an interpolation.
pub fn string_value(source: &str, span: Span) -> Option<String> {
    let mut v = String::new();
    for l in literals(source, span) {
        if l.is_interpolated() {
            return None;
        }
        let t = l.text(source);
        v.push_str(&if l.raw { t } else { decode(&t) });
    }
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(source: &str) -> Vec<Literal> {
        literals(
            source,
            Span {
                start: 0,
                end: source.len(),
            },
        )
    }

    #[test]
    fn adjacent_literals_and_interpolation() {
        let source = "'a' \"b$c d${e ? 'x}' : \"y\"}f\" // z\n r'''g'h'''";
        let v = read(source);
        assert_eq!(v.len(), 3);
        assert_eq!(&source[v[0].span.start..v[0].span.end], "'a'");
        assert!(v[0].single_quoted && !v[0].is_interpolated() && !v[0].raw && !v[0].multiline);
        assert_eq!(v[1].text(source), "b df");
        let interpolations: Vec<&str> = v[1]
            .interpolations
            .iter()
            .map(|s| &source[s.start..s.end])
            .collect();
        assert_eq!(interpolations, vec!["$c", "${e ? 'x}' : \"y\"}"]);
        assert!(v[2].raw && v[2].multiline && v[2].single_quoted);
        assert_eq!(v[2].text(source), "g'h");
    }

    #[test]
    fn comments_outside_strings() {
        let source = "var a = '// no'; // yes\n/* b /* c */ d */ var e = \"/* no */\";\n";
        let v: Vec<&str> = comments(source)
            .iter()
            .map(|s| &source[s.start..s.end])
            .collect();
        assert_eq!(v, vec!["// yes", "/* b /* c */ d */"]);
    }
}
