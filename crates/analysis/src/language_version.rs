//! `LanguageVersionOverrideVerifier`
//! (`analyzer/lib/src/error/language_version_override_verifier.dart`,
//! docs/ANALYZER-ESPECIFICACAO.md, `invalid_language_version_override`): the
//! comments before the first token that look like a mistyped `// @dart = X.Y`,
//! and the `// @dart = X.Y` after the first directive or declaration. The
//! `_GREATER` comes from the scanner (`crates/elements/src/load.rs`).
//!
//! Written without compiling or running (2026-10-05).

use crate::Unit;
use dartforge_diagnostics::codes::warning as w;
use dartforge_diagnostics::{Code, Diagnostic, Span};
use dartforge_frontend::comments::Comments;

fn is_blank(c: u8) -> bool {
    c == 0x09 || c == 0x20
}

fn is_digit(c: u8) -> bool {
    c.is_ascii_digit()
}

fn is_letter(c: u8) -> bool {
    c.is_ascii_alphabetic()
}

/// `_findLanguageVersionOverrideComment`: `Ok(true)` on a valid override (the
/// search stops), `Ok(false)` when it is not an attempt, `Err(code)` when it
/// is an invalid attempt (the search goes on).
fn check_comment(comment: &[u8]) -> Result<bool, Code> {
    let n = comment.len();
    let mut i = 0usize;
    let skip_blanks = |i: &mut usize| {
        while *i < n && is_blank(comment[*i]) {
            *i += 1;
        }
    };
    while i < n && comment[i] == b'/' {
        i += 1;
    }
    let slashes = i;
    skip_blanks(&mut i);
    if i == n {
        return Ok(false);
    }
    let at_sign = comment[i] == b'@';
    if at_sign {
        i += 1;
    }
    if n - i < 4 {
        return Ok(false);
    }
    let dart = &comment[i..i + 4];
    if !dart.eq_ignore_ascii_case(b"dart") {
        return Ok(false);
    }
    i += 4;
    skip_blanks(&mut i);
    if i == n {
        return Ok(false);
    }
    let separator_start = i;
    while i < n {
        let c = comment[i];
        if is_digit(c) || is_letter(c) || is_blank(c) {
            break;
        }
        i += 1;
    }
    if i == n {
        return Ok(false);
    }
    let separator = i - separator_start;
    skip_blanks(&mut i);
    if i == n {
        return Ok(false);
    }
    let mut prefix = false;
    if is_letter(comment[i]) {
        prefix = true;
        i += 1;
        if i == n {
            return Ok(false);
        }
    }
    if !is_digit(comment[i]) {
        return Ok(false);
    }
    if i + 1 < n && is_letter(comment[i + 1]) {
        return Ok(false);
    }
    if !at_sign && separator == 0 {
        return Ok(false);
    }
    if slashes > 2 {
        return Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_TWO_SLASHES);
    }
    if !at_sign {
        return Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_AT_SIGN);
    }
    if dart != b"dart" {
        return Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_LOWER_CASE);
    }
    if separator != 1 || comment[separator_start] != b'=' {
        return Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_EQUALS);
    }
    if prefix {
        return Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_PREFIX);
    }
    while i < n && is_digit(comment[i]) {
        i += 1;
    }
    if i == n || comment[i] != b'.' {
        return Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_NUMBER);
    }
    i += 1;
    while i < n && is_digit(comment[i]) {
        i += 1;
    }
    skip_blanks(&mut i);
    if i == n {
        return Ok(true);
    }
    Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_TRAILING_CHARACTERS)
}

/// `_overrideCommentLine` (`^\s*//\s*@dart\s*=\s*\d+\.\d+`): the end of the
/// match, from the start of the comment.
fn match_override(text: &[u8]) -> Option<usize> {
    let n = text.len();
    let mut i = 0usize;
    let skip_spaces = |i: &mut usize| {
        while *i < n && text[*i].is_ascii_whitespace() {
            *i += 1;
        }
    };
    skip_spaces(&mut i);
    if !text[i..].starts_with(b"//") {
        return None;
    }
    i += 2;
    skip_spaces(&mut i);
    if !text[i..].starts_with(b"@dart") {
        return None;
    }
    i += 5;
    skip_spaces(&mut i);
    if text.get(i) != Some(&b'=') {
        return None;
    }
    i += 1;
    skip_spaces(&mut i);
    let d = i;
    while i < n && is_digit(text[i]) {
        i += 1;
    }
    if i == d || text.get(i) != Some(&b'.') {
        return None;
    }
    i += 1;
    let d = i;
    while i < n && is_digit(text[i]) {
        i += 1;
    }
    (i > d).then_some(i)
}

/// The first token after the `#!` (the start of the code), skipping blanks
/// and comments.
fn first_token(source: &[u8], comments: &[Span], mut pos: usize) -> usize {
    loop {
        while pos < source.len() && source[pos].is_ascii_whitespace() {
            pos += 1;
        }
        match comments.iter().find(|c| c.start == pos) {
            Some(c) => pos = c.end,
            None => return pos,
        }
    }
}

pub fn check(u: Unit<'_>) -> Vec<Diagnostic> {
    let source = u.source.as_bytes();
    let comments = Comments::scan(u.source);
    let all = comments.all();
    let mut out = Vec::new();
    // `_verifyMisplaced`: after the first significant token.
    let first_significant = u
        .unit
        .directives
        .first()
        .map(|d| d.span.start)
        .or_else(|| u.unit.declarations.first().map(|&d| u.ast.decl(d).span.start));
    if let Some(p) = first_significant {
        for c in all.iter().filter(|c| c.start > p) {
            let lexeme = &source[c.start..c.end];
            if let Some(end) = match_override(lexeme)
                && let Some(at_sign) = u.source[c.start..c.end].find("@dart")
            {
                out.push(Diagnostic::with_code(
                    w::INVALID_LANGUAGE_VERSION_OVERRIDE_LOCATION,
                    Span { start: c.start + at_sign, end: c.start + end },
                    Vec::<&str>::new(),
                ));
            }
        }
    }
    // The comments before the first token (after the `#!`).
    let after_script = if source.starts_with(b"#!") { source.iter().position(|&b| b == b'\n').map_or(source.len(), |i| i + 1) } else { 0 };
    let first = first_token(source, all, after_script);
    for c in all.iter().filter(|c| c.start >= after_script && c.end <= first) {
        match check_comment(&source[c.start..c.end]) {
            Ok(true) => break,
            Ok(false) => {}
            Err(code) => out.push(Diagnostic::with_code(code, *c, Vec::<&str>::new())),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(check_comment(b"// @dart = 2.12"), Ok(true));
        assert_eq!(check_comment(b"// dart = 2.0"), Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_AT_SIGN));
        assert_eq!(check_comment(b"// @dart 2.0"), Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_EQUALS));
        assert_eq!(check_comment(b"// @dart >= 2.0"), Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_EQUALS));
        assert_eq!(check_comment(b"// @Dart = 2.0"), Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_LOWER_CASE));
        assert_eq!(check_comment(b"/// @dart = 2.0"), Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_TWO_SLASHES));
        assert_eq!(check_comment(b"// @dart = v2.0"), Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_PREFIX));
        assert_eq!(check_comment(b"// @dart = 2"), Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_NUMBER));
        assert_eq!(check_comment(b"// @dart = 2.0 x"), Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_TRAILING_CHARACTERS));
        assert_eq!(check_comment(b"/// dart2 is great"), Ok(false));
        assert_eq!(match_override(b"// @dart = 3.0"), Some(14));
    }
}
