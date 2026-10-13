//! The `[…]` references of a doc comment, like the analyzer 3.6.2
//! `DocCommentBuilder` (`analyzer/lib/src/fasta/doc_comment_builder.dart`):
//! the comment is read line by line (`_CharacterSequence`: after each line's
//! `///`, or the lines of a `/** */` without the `* ` margin); an indented
//! block (four spaces after an empty line), a backtick-fenced block, a known
//! `{@…}` directive, a `@docImport` and a `@nodoc` have no references; in the
//! other lines, `_parseReferences` finds each `[…]` that is not link text,
//! and `_parseOneCommentReference` reads it with the Dart scanner: optional
//! `new`, up to two dotted prefixes and an identifier or a user-definable
//! operator (`[int.+]`, `[operator ==]`).

use crate::Span;

/// A reference: the parts (the range and text of each one, in order:
/// prefixes and the name) and the `new` written before them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocReference {
    pub has_new: bool,
    pub parts: Vec<(Span, String)>,
}

/// The user-definable operators (`TokenType.isUserDefinableOperator`).
const OPERATORS: &[&str] = &["==", "~", "[]", "[]=", "*", "/", "%", "~/", "+", "-", "<<", ">>", ">>>", ">=", ">", "<=", "<", "&", "^", "|"];

/// The reserved words (`KeywordStyle.reserved`): they are not identifiers.
const RESERVED_WORDS: &[&str] = &[
    "assert", "break", "case", "catch", "class", "const", "continue", "default", "do", "else", "enum", "extends", "false", "final", "finally", "for", "if", "in", "is", "new", "null", "rethrow", "return", "super", "switch", "this", "throw", "true", "try", "var", "void", "while", "with",
];

/// The `{@…}` directives the builder knows (`_parseDocDirectiveTag`).
const DIRECTIVES: &[&str] = &["animation", "canonicalFor", "category", "end-inject-html", "end-tool", "endtemplate", "inject-html", "macro", "subCategory", "template", "tool", "youtube"];

/// The scanner's `isWhitespace`: space, tab, LF and CR.
fn whitespace(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r')
}

/// `_readWhitespace`.
fn read_whitespace(content: &[u8], mut i: usize) -> usize {
    while i < content.len() && whitespace(content[i]) {
        i += 1;
    }
    i
}

/// The lines (`_CharacterSequence`): the source offset of each content and
/// the content. `tokens` is the comment chain starting at the `findDartDoc`
/// token.
fn lines_<'a>(source: &'a str, tokens: &[Span]) -> Vec<(usize, &'a str)> {
    let mut v = Vec::new();
    let Some(first) = tokens.first() else { return v };
    let lexeme_of = |s: &Span| {
        // The `//` token ends before the CR or the LF.
        let t = &source[s.start..s.end];
        let end_offset = t.find(['\r', '\n']).unwrap_or(t.len());
        (s.start, &t[..end_offset])
    };
    if source[first.start..].starts_with("///") {
        for s in tokens {
            let (start_offset, lexeme) = lexeme_of(s);
            if lexeme.starts_with("///") {
                v.push((start_offset + 3, &lexeme[3..]));
            }
        }
        return v;
    }
    // `_CharacterSequenceFromMultiLineComment`.
    let lexeme = &source[first.start..first.end];
    let b = lexeme.as_bytes();
    let base = first.start;
    let first_break = lexeme.find('\n').unwrap_or(lexeme.len());
    v.push((base, &lexeme[..first_break]));
    let mut end_offset = first_break;
    loop {
        let mut i = end_offset + 1;
        if i >= b.len() {
            break;
        }
        while whitespace(b[i]) {
            i += 1;
            if i >= b.len() {
                return v;
            }
        }
        let break_ = lexeme[i..].find('\n').map_or(b.len(), |k| i + k);
        end_offset = break_;
        if lexeme[i..].starts_with("* ") {
            i += 2;
        } else if break_ == i + 1 && b[i] == b'*' {
            i += 1;
        }
        v.push((base + i, &lexeme[i..break_]));
    }
    v
}

/// `_fencedCodeBlockDelimiter`: the index of the opening backticks, or
/// `None`. The comparison is against three characters, and it only matches
/// when the minimum is three.
fn fence(content: &[u8], minimum: usize) -> Option<usize> {
    if content.is_empty() {
        return None;
    }
    let i = read_whitespace(content, 0);
    if i + 3 > content.len() {
        return None;
    }
    (minimum == 3 && &content[i..i + 3] == b"```").then_some(i)
}

/// The references of the comment whose tokens (the chain starting at the
/// `findDartDoc` one) are `tokens`.
pub fn references(source: &str, tokens: &[Span]) -> Vec<DocReference> {
    let ls = lines_(source, tokens);
    let mut output = Vec::new();
    let mut blank_line_before = true;
    let mut k = 0;
    while k < ls.len() {
        let (shift, content) = ls[k];
        let b = content.as_bytes();
        let ws = read_whitespace(b, 0);
        if blank_line_before && ws >= 4 {
            // `_parseIndentedCodeBlock`: up to the first line with fewer than four
            // spaces, which is read next.
            k += 1;
            while k < ls.len() && read_whitespace(ls[k].1.as_bytes(), 0) >= 4 {
                k += 1;
            }
            if k < ls.len() {
                blank_line_before = ls[k].1.is_empty();
            }
            continue;
        }
        if let Some(i) = fence(b, 3) {
            // `_parseFencedCodeBlock`: up to the closing line (with at least as many
            // backticks as the opening), consumed.
            let mut backticks = 0;
            let mut j = i;
            while j < b.len() && b[j] == b'`' {
                backticks += 1;
                j += 1;
            }
            k += 1;
            while k < ls.len() {
                if fence(ls[k].1.as_bytes(), backticks).is_some() {
                    break;
                }
                k += 1;
            }
            blank_line_before = false;
            k += 1;
            continue;
        }
        if known_directive(b, ws) || doc_import(content, ws) || nodoc(b, ws) {
            blank_line_before = false;
            k += 1;
            continue;
        }
        line_references(content, shift, &mut output);
        blank_line_before = content.is_empty();
        k += 1;
    }
    output
}

/// `_parseDocDirectiveTag`: `{@name` with a known name.
fn known_directive(b: &[u8], i: usize) -> bool {
    if !b[i..].starts_with(b"{@") {
        return false;
    }
    let mut j = i + 2;
    if j >= b.len() {
        return false;
    }
    let start_offset = j;
    while j < b.len() && !whitespace(b[j]) && b[j] != b'}' {
        j += 1;
    }
    let name = std::str::from_utf8(&b[start_offset..j]).unwrap_or("");
    DIRECTIVES.contains(&name)
}

/// `_parseDocImport`: `@docImport ` followed by an import (the URI in
/// quotes).
fn doc_import(content: &str, i: usize) -> bool {
    let Some(rest) = content[i..].strip_prefix("@docImport ") else { return false };
    rest.trim_start_matches([' ', '\t', '\n', '\r']).starts_with(['\'', '"'])
}

/// `_parseNodoc`.
fn nodoc(b: &[u8], i: usize) -> bool {
    if !b[i..].starts_with(b"@nodoc") {
        return false;
    }
    b.len() == i + 6 || b[i + 6] == b' '
}

/// `_findCommentReferenceEnd`: the end of `ident` or `ident.ident` (ASCII
/// letters and digits), for a `[` without `]` on the line.
fn reference_end(b: &[u8], mut i: usize, end_offset: usize) -> usize {
    let letter = |c: u8| c.is_ascii_alphabetic();
    let letter_or_digit = |c: u8| c.is_ascii_alphanumeric();
    if i >= end_offset || !letter(b[i]) {
        return i;
    }
    while i < end_offset && letter_or_digit(b[i]) {
        i += 1;
    }
    if i >= end_offset || b[i] != b'.' {
        return i;
    }
    i += 1;
    if i >= end_offset || !letter(b[i]) {
        return i;
    }
    i += 1;
    while i < end_offset && letter_or_digit(b[i]) {
        i += 1;
    }
    i
}

/// `_isLinkText`.
fn link_text(b: &[u8], right: usize, can_be_link_reference: bool) -> bool {
    let mut i = right + 1;
    if i >= b.len() {
        return false;
    }
    let mut c = b[i];
    if c == b'(' {
        return true;
    }
    if can_be_link_reference && c == b':' {
        return true;
    }
    while whitespace(c) {
        i += 1;
        if i >= b.len() {
            return false;
        }
        c = b[i];
    }
    c == b'['
}

/// `_parseReferences` on one line.
fn line_references(content: &str, shift: usize, output: &mut Vec<DocReference>) {
    let b = content.as_bytes();
    let end_offset = b.len();
    let mut i = 0;
    let mut only_whitespace = true;
    while i < end_offset {
        let c = b[i];
        if c == b'[' {
            i += 1;
            if i < end_offset && b[i] == b':' {
                // Old `[:code:]`.
                match content[i + 1..].find(":]") {
                    Some(k) => i = i + 1 + k + 1,
                    None => break,
                }
                if i > end_offset {
                    break;
                }
            } else {
                let start_offset = i;
                let right = match content[i..].find(']') {
                    Some(k) => i + k,
                    None => reference_end(b, start_offset, end_offset),
                };
                i = right;
                if !link_text(b, right, only_whitespace)
                    && let Some(r) = one_reference(&content[start_offset..right], shift + start_offset)
                {
                    output.push(r);
                }
            }
            only_whitespace = false;
        } else if c == b'`' {
            if let Some(k) = content[i + 1..].find('`') {
                i = i + 1 + k;
            }
            only_whitespace = false;
        } else if !whitespace(c) {
            only_whitespace = false;
        }
        i += 1;
    }
}

/// A Dart scanner token, as far as the reference needs.
#[derive(Debug, Clone)]
enum Tk {
    /// Identifier or keyword (the text decides).
    Word(Span, String),
    Dot,
    Operator(Span, String),
    Other,
}

/// The Dart scanner over the text of a reference: `None` when it reports an
/// error (invalid character, unterminated string).
fn scan(text: &str, shift: usize) -> Option<Vec<Tk>> {
    let b = text.as_bytes();
    let mut v = Vec::new();
    let mut i = 0;
    // The operators, longest first (the scanner consumes the longest one).
    const SIGNS: &[&str] = &[
        ">>>=", "...?", "<<=", ">>=", ">>>", "~/=", "??=", "...", "&&=", "||=", "==", "!=", "<=", ">=", "<<", ">>", "~/", "[]=", "[]", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "&&", "||", "++", "--", "??", "?.", "=>", "+", "-", "*",
        "/", "%", "~", "<", ">", "&", "|", "^", "!", "?", "=", ":", ";", ",", "(", ")", "[", "]", "{", "}", "@", "#",
    ];
    while i < b.len() {
        let c = b[i];
        if whitespace(c) {
            i += 1;
            continue;
        }
        if b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' && b[i] != b'\r' {
                i += 1;
            }
            continue;
        }
        if b[i..].starts_with(b"/*") {
            let mut prof = 0usize;
            while i + 1 < b.len() {
                if b[i] == b'/' && b[i + 1] == b'*' {
                    prof += 1;
                    i += 2;
                } else if b[i] == b'*' && b[i + 1] == b'/' {
                    prof -= 1;
                    i += 2;
                    if prof == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            if prof != 0 {
                return None;
            }
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' || c == b'$' {
            let start_ = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
            v.push(Tk::Word(Span { start: shift + start_, end: shift + i }, text[start_..i].to_string()));
            continue;
        }
        if c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'.' || b[i] == b'_') {
                i += 1;
            }
            v.push(Tk::Other);
            continue;
        }
        if c == b'\'' || c == b'"' {
            let closes = text[i + 1..].find(c as char)?;
            i = i + 1 + closes + 1;
            v.push(Tk::Other);
            continue;
        }
        if c == b'.' {
            v.push(Tk::Dot);
            i += 1;
            continue;
        }
        let sign = SIGNS.iter().find(|s| b[i..].starts_with(s.as_bytes()))?;
        v.push(Tk::Operator(Span { start: shift + i, end: shift + i + sign.len() }, sign.to_string()));
        i += sign.len();
    }
    Some(v)
}

/// `token.isIdentifier`: identifier, built-in word or pseudo-keyword.
fn identifier(t: Option<&Tk>) -> bool {
    matches!(t, Some(Tk::Word(_, p)) if !RESERVED_WORDS.contains(&p.as_str()))
}

/// `_parseOneCommentReference`.
fn one_reference(text: &str, shift: usize) -> Option<DocReference> {
    let tks = scan(text, shift)?;
    let t = |i: usize| tks.get(i);
    let mut i = 0;
    let mut has_new = false;
    if matches!(t(i), Some(Tk::Word(_, p)) if p == "new") {
        has_new = true;
        i += 1;
    }
    let mut parts: Vec<(Span, String)> = Vec::new();
    if identifier(t(i)) && matches!(t(i + 1), Some(Tk::Dot)) {
        let mut second = i;
        let mut dot = i + 1;
        if identifier(t(dot + 1)) && matches!(t(dot + 2), Some(Tk::Dot)) {
            if let Some(Tk::Word(s, p)) = t(second) {
                parts.push((*s, p.clone()));
            }
            second = dot + 1;
            dot += 2;
        }
        if let Some(Tk::Word(s, p)) = t(second) {
            parts.push((*s, p.clone()));
        }
        i = dot + 1;
        // `new` after the `.` becomes an identifier (the unnamed constructor).
    }
    // Nothing after it: the recovery's synthetic identifier (empty, it does not
    // resolve); the prefixes before it are references.
    if t(i).is_none() {
        let end_offset = parts.last()?.0.end;
        parts.push((Span { start: end_offset + 1, end: end_offset + 1 }, String::new()));
        return Some(DocReference { has_new, parts });
    }
    let mut operator_word = None;
    if matches!(t(i), Some(Tk::Word(_, p)) if p == "operator") {
        operator_word = Some(i);
        i += 1;
    }
    if let Some(Tk::Operator(s, op)) = t(i)
        && OPERATORS.contains(&op.as_str())
    {
        if t(i + 1).is_none() {
            parts.push((*s, op.clone()));
            return Some(DocReference { has_new, parts });
        }
        return None;
    }
    let j = operator_word.unwrap_or(i);
    if t(j + 1).is_some() {
        return None;
    }
    let after_dot = !parts.is_empty();
    match t(j) {
        Some(Tk::Word(s, p)) if identifier(t(j)) || (after_dot && p == "new") => {
            parts.push((*s, p.clone()));
            Some(DocReference { has_new, parts })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs(source: &str) -> Vec<String> {
        let c = crate::comments::Comments::scan(source);
        let pos = source.find("class").unwrap();
        let start_offset = c.dart_doc(source, pos).unwrap();
        let tokens: Vec<Span> = c.before(source, pos).into_iter().filter(|s| s.start >= start_offset.start).collect();
        references(source, &tokens).into_iter().map(|r| r.parts.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join(".")).collect()
    }

    #[test]
    fn like_the_doc_comment_builder() {
        assert_eq!(refs("/// [a], [b.c], [new D], [E.new], [int.+], [operator ==].\nclass X {}\n"), vec!["a", "b.c", "D", "E.new", "int.+", "=="]);
        // Links: `[t](u)`, `[t] [r]` (the `[r]` is a reference), `[r]:` at the
        // start. `[r] [q]`: the `[r]` followed by `[` (after whitespace) is link
        // text.
        assert_eq!(refs("/// [t](u) [x][r] [q]: y\n/// [z]: link\n/// [x][s]\nclass X {}\n"), vec!["q", "s"]);
        // Backtick code, fenced block and indented block.
        assert_eq!(refs("/// `[a]` [b]\n/// ```\n/// [c]\n/// ```\n///\n///     [d]\n/// [e]\nclass X {}\n"), vec!["b", "e"]);
        // `[a [b]`: the content runs up to the first `]`.
        assert_eq!(refs("/// [a [b] [c d] [ f ]\nclass X {}\n"), vec!["f"]);
        // `//` in the middle of the run is skipped; `{@template}` has no reference.
        assert_eq!(refs("/// [a]\n// [b]\n/// {@template t} [c]\n/// [d\nclass X {}\n"), vec!["a", "d"]);
        // `/** */` block.
        assert_eq!(refs("/**\n * [a] e\n *\n * [b]\n */\nclass X {}\n"), vec!["a", "b"]);
    }
}
