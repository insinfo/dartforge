//! The comments of a unit (the lexer here discards them), found by a scanner
//! that knows strings (simple, triple, raw) and nested `${…}`
//! interpolations, so that `//` and `/*` inside a string do not count. With
//! them: a token's `precedingComments` (the run of comments right before it,
//! separated only by whitespace) and the `findDartDoc` of the analyzer 3.6.2
//! parser (`_fe_analyzer_shared/lib/src/parser/parser_impl.dart:9564`).
//! Written without compiling or running (2026-10-05).

use crate::Span;

fn identifier_part(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

/// The comments of a unit, in order.
pub struct Comments {
    spans: Vec<Span>,
}

impl Comments {
    pub fn scan(source: &str) -> Comments {
        let mut spans = Vec::new();
        sweep(source.as_bytes(), 0, false, &mut spans);
        Comments { spans }
    }

    pub fn all(&self) -> &[Span] {
        &self.spans
    }

    /// `token.precedingComments`: the comments right before `pos`, with only
    /// whitespace between them and up to `pos`, in order.
    pub fn before(&self, source: &str, pos: usize) -> Vec<Span> {
        let b = source.as_bytes();
        let only_whitespace = |from: usize, until: usize| {
            b.get(from..until)
                .is_some_and(|t| t.iter().all(u8::is_ascii_whitespace))
        };
        let mut k = self.spans.partition_point(|s| s.end <= pos);
        let mut limit = pos;
        let mut v = Vec::new();
        while k > 0 {
            let s = self.spans[k - 1];
            if !only_whitespace(s.end, limit) {
                break;
            }
            v.push(s);
            limit = s.start;
            k -= 1;
        }
        v.reverse();
        v
    }

    /// `findDartDoc(token)`: among the comments before `pos`, the last `/**`,
    /// or the first `///` of the last run of `///` that comes after it.
    pub fn dart_doc(&self, source: &str, pos: usize) -> Option<Span> {
        let mut doc: Option<Span> = None;
        let mut multiline = false;
        for s in self.before(source, pos) {
            let lexeme = &source[s.start..s.end];
            if lexeme.starts_with("///") {
                if !multiline {
                    doc = Some(s);
                    multiline = true;
                }
            } else if lexeme.starts_with("/**") {
                doc = Some(s);
                multiline = false;
            }
        }
        doc
    }
}

/// Scans code from `i`; with `until_brace`, stops after the `}` that closes
/// the interpolation (returns the position after it).
fn sweep(b: &[u8], mut i: usize, until_brace: bool, output: &mut Vec<Span>) -> usize {
    let mut level = 0usize;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            let start_ = i;
            while i < b.len() && b[i] != b'\n' && b[i] != b'\r' {
                i += 1;
            }
            output.push(Span {
                start: start_,
                end: i,
            });
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            // Block comments nest.
            let start_ = i;
            let mut depth = 0usize;
            while i < b.len() {
                if b[i..].starts_with(b"/*") {
                    depth += 1;
                    i += 2;
                } else if b[i..].starts_with(b"*/") {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            output.push(Span {
                start: start_,
                end: i,
            });
            continue;
        }
        if until_brace {
            if c == b'{' {
                level += 1;
            } else if c == b'}' {
                if level == 0 {
                    return i + 1;
                }
                level -= 1;
            }
        }
        if c == b'\'' || c == b'"' {
            i = string(b, i, false, output);
            continue;
        }
        if identifier_part(c) {
            let start_ = i;
            while i < b.len() && identifier_part(b[i]) {
                i += 1;
            }
            // `r'…'`: the raw string.
            if &b[start_..i] == b"r" && matches!(b.get(i), Some(b'\'') | Some(b'"')) {
                i = string(b, i, true, output);
            }
            continue;
        }
        i += 1;
    }
    i
}

/// Scans the string that starts at the quote at `i`; returns the position
/// after it.
fn string(b: &[u8], mut i: usize, raw: bool, output: &mut Vec<Span>) -> usize {
    let q = b[i];
    let triple = b[i..].starts_with(&[q, q, q]);
    let n = if triple { 3 } else { 1 };
    i += n;
    while i < b.len() {
        if b[i] == q && (!triple || b[i..].starts_with(&[q, q, q])) {
            return i + n;
        }
        // A simple string does not go past the end of the line.
        if !triple && (b[i] == b'\n' || b[i] == b'\r') {
            return i;
        }
        if !raw && b[i] == b'\\' {
            i += 2;
            continue;
        }
        if !raw && b[i] == b'$' && b.get(i + 1) == Some(&b'{') {
            i = sweep(b, i + 2, true, output);
            continue;
        }
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_have_no_comments() {
        let f = "var a = '// nao'; /* sim */ var b = \"${x /* sim2 */}\"; // fim\n";
        let c = Comments::scan(f);
        let texts: Vec<&str> = c.all().iter().map(|s| &f[s.start..s.end]).collect();
        assert_eq!(texts, vec!["/* sim */", "/* sim2 */", "// fim"]);
    }

    #[test]
    fn dart_doc_like_the_parser() {
        let f = "/** a */\n// b\n/// c\n/// d\nclass A {}\n";
        let c = Comments::scan(f);
        let pos = f.find("class").unwrap();
        let d = c.dart_doc(f, pos).unwrap();
        assert_eq!(&f[d.start..d.end], "/// c");
    }
}
