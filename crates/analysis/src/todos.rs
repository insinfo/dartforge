//! The analyzer's `TodoFinder` (`analyzer/lib/src/error/todo_finder.dart`,
//! `analyzer/lib/src/dart/error/todo_codes.dart`;
//! docs/ANALYZER-ESPECIFICACAO-INFRA.md §7.1): `todo`, `fixme`, `hack` and
//! `undone`, one per match of `Todo.TODO_REGEX` in the comments preceding a
//! token (those dangling at the end of the file are not examined). The
//! message is the comment's text itself.
//!
//! Written without compiling or running (2026-10-04).

use dartforge_diagnostics::codes::todo as t;
use dartforge_diagnostics::{Code, Diagnostic, Span};
use dartforge_frontend::token::Kind;

const WORDS: [(&str, Code); 4] = [("TODO", t::TODO), ("FIXME", t::FIXME), ("HACK", t::HACK), ("UNDONE", t::UNDONE)];

/// A comment of the source.
struct Comment {
    start: usize,
    end: usize,
    /// `/* */` (otherwise `//`).
    is_block: bool,
    line: usize,
    column: usize,
}

/// Dart's `\w` without the `unicode` option.
fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The end of `[^\r\n]*` from `i` on.
fn line_rest_end(s: &str, i: usize) -> usize {
    s[i..].find(['\r', '\n']).map_or(s.len(), |k| i + k)
}

/// The first match of `Todo.TODO_REGEX` in `lexeme` from `from` on: the
/// start and end of group 2 (the word and the text) and the code.
fn find_match(lexeme: &str, from: usize) -> Option<(usize, usize, Code)> {
    for (p, c) in lexeme[from..].char_indices() {
        // Group 1: a whitespace character, `/` or `*`.
        if !(c.is_whitespace() || c == '/' || c == '*') {
            continue;
        }
        let q = from + p + c.len_utf8();
        let rest = &lexeme[q..];
        for (word, code) in WORDS {
            if !rest.starts_with(word) {
                continue;
            }
            let after = q + word.len();
            match lexeme[after..].chars().next() {
                // Alternative A: a non-word character, the rest of the line
                // and the following `*` lines plus two spaces.
                Some(x) if !is_word_char(x) => {
                    let mut end = line_rest_end(lexeme, after + x.len_utf8());
                    loop {
                        let Some(after_break) = lexeme[end..].strip_prefix('\n') else { break };
                        let trimmed = after_break.trim_start();
                        let Some(after_mark) = trimmed.strip_prefix("*  ") else { break };
                        end = line_rest_end(lexeme, lexeme.len() - after_mark.len());
                    }
                    return Some((q, end, code));
                }
                // Alternative B: the word at the end of the lexeme.
                None => return Some((q, after, code)),
                Some(_) => {}
            }
        }
    }
    None
}

/// The comments of `source[from..until]`, a gap between two tokens.
fn gap_comments(source: &str, from: usize, until: usize, line_starts: &[usize], out: &mut Vec<Comment>) {
    let b = source.as_bytes();
    let mut i = from;
    while i + 1 < until {
        let (end, is_block) = if b[i] == b'/' && b[i + 1] == b'/' {
            let end = source[i..until].find('\n').map_or(until, |k| i + k);
            (if end > i && b[end - 1] == b'\r' { end - 1 } else { end }, false)
        } else if b[i] == b'/' && b[i + 1] == b'*' {
            let mut level = 1;
            let mut j = i + 2;
            while j < until && level > 0 {
                if j + 1 < until && b[j] == b'/' && b[j + 1] == b'*' {
                    level += 1;
                    j += 2;
                } else if j + 1 < until && b[j] == b'*' && b[j + 1] == b'/' {
                    level -= 1;
                    j += 2;
                } else {
                    j += 1;
                }
            }
            (j.min(until), true)
        } else if b[i].is_ascii_whitespace() {
            i += 1;
            continue;
        } else {
            // Neither blank nor comment: the gap is not what was expected.
            return;
        };
        let line = line_starts.partition_point(|&x| x <= i) - 1;
        out.push(Comment { start: i, end, is_block, line, column: i - line_starts[line] });
        i = end.max(i + 2);
    }
}

/// `replaceAll(RegExp(r'\s*\n\s*\*\s*'), ' ')`.
fn unfold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(k) = rest.find('\n') {
        let after = rest[k + 1..].trim_start();
        match after.strip_prefix('*') {
            Some(after) => {
                out.push_str(rest[..k].trim_end());
                out.push(' ');
                rest = after.trim_start();
            }
            None => {
                out.push_str(&rest[..=k]);
                rest = &rest[k + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// `TodoFinder.findIn`: the reports of a unit.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    // Without any of the words, nothing to do (the common case).
    if !WORDS.iter().any(|(p, _)| source.contains(p)) {
        return out;
    }
    let Ok(tokens) = dartforge_frontend::lexer::lex(source) else { return out };
    let mut line_starts = vec![0usize];
    line_starts.extend(source.bytes().enumerate().filter(|(_, c)| *c == b'\n').map(|(i, _)| i + 1));
    let mut previous = 0usize;
    for token in &tokens {
        // The comments dangling on the end-of-file token are left out.
        if matches!(token.kind, Kind::Eof) {
            break;
        }
        let mut cs = Vec::new();
        gap_comments(source, previous.min(token.span.start), token.span.start, &line_starts, &mut cs);
        previous = token.span.end;
        let mut k = 0;
        while k < cs.len() {
            let c = &cs[k];
            let lexeme = &source[c.start..c.end];
            // How many following comments became a continuation of this one.
            let mut consumed = 0;
            let mut from = 0;
            while let Some((start, text_end, code)) = find_match(lexeme, from) {
                from = text_end.max(start + 1);
                let offset = c.start + start;
                let column = c.column + start;
                let mut text = lexeme[start..text_end].to_string();
                let mut end = offset + text.len();
                if c.is_block {
                    if let Some(without) = text.strip_suffix("*/") {
                        text = without.trim_end().to_string();
                        end = offset + text.len();
                    }
                    text = unfold(&text);
                } else {
                    // The continuation lines: `//` on the next line, at the
                    // same column, with the text indented one space more than
                    // the word, and without another TODO.
                    let mut line = c.line;
                    let mut n = consumed;
                    while let Some(next) = cs.get(k + 1 + n) {
                        let lex = &source[next.start..next.end];
                        let indent = lex.len() - lex.trim_start_matches(['/', ' ']).len();
                        let continues = !next.is_block
                            && !lex.starts_with("///")
                            && next.line == line + 1
                            && next.column == c.column
                            && next.column + indent == column + 1
                            && find_match(lex, 0).is_none();
                        if !continues {
                            break;
                        }
                        end = next.end;
                        text.push(' ');
                        text.push_str(lex[indent..].trim_end());
                        line = next.line;
                        n += 1;
                    }
                    consumed = n;
                }
                out.push(Diagnostic::with_code(code, Span { start: offset, end }, [text.as_str()]));
            }
            k += 1 + consumed;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(code, start, length, message)` of each report.
    fn findings(source: &str) -> Vec<(&'static str, usize, usize, String)> {
        check(source)
            .into_iter()
            .map(|d| (d.code.map_or("", |c| c.info().name), d.span.start, d.span.end - d.span.start, d.message))
            .collect()
    }

    /// The table checked on the 3.6.2 binary (§7.1).
    #[test]
    fn oracle_table() {
        assert_eq!(findings("/// TODO: doc\nvoid a() {}\n"), vec![("todo", 4, 9, "TODO: doc".to_string())]);
        let block = "/** HACK block\n *  and more here\n */\nvoid b() {}\n";
        assert_eq!(findings(block), vec![("hack", 4, 28, "HACK block and more here".to_string())]);
        let lines = "// FIXME(x): line one\n//  and more\n// no more here\nvoid c() {}\n";
        assert_eq!(findings(lines), vec![("fixme", 3, 31, "FIXME(x): line one and more".to_string())]);
        assert_eq!(findings("void c() {} // UNDONE\nvoid d() {}\n"), vec![("undone", 15, 6, "UNDONE".to_string())]);
    }

    #[test]
    fn what_does_not_match() {
        assert!(findings("// todo: lowercase TODOS not\nvoid a() {}\n").is_empty());
        // Dangling at the end of the file.
        assert!(findings("void a() {}\n// TODO: at the end of the file\n").is_empty());
    }

    #[test]
    fn unfolds_the_block() {
        assert_eq!(unfold("HACK block\n *  and more here"), "HACK block and more here");
    }
}
