//! The seventh batch of lint rules (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8),
//! the ones that look at the strings' lexemes and at the lines, written
//! directly from the 3.6.2 emitters
//! (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `prefer_single_quotes`, `prefer_double_quotes`,
//! `avoid_escaping_inner_quotes`, `unnecessary_string_escapes`,
//! `use_raw_strings`, `leading_newlines_in_multiline_strings` and
//! `lines_longer_than_80_chars`.
//!
//! The literals come from [`super::strings`], which rereads the text of each
//! string expression (the tree joins adjacent ones); the directives' URIs
//! come in too.
//!
//! Where the original asks about the string's value ("contains a quote",
//! "looks like a URI"), the question is asked of the value (`literal_value`:
//! the parts with the escapes decoded, like `SimpleStringLiteral.value` and
//! `InterpolationString.value`); where it asks about the lexeme
//! (`unnecessary_string_escapes`, `use_raw_strings`), of the written text.
//! The lines are those of `LineInfo` (`\n`, `\r\n` and a lone `\r` end a
//! line).
//! Written without compiling or running (2026-10-05).

use super::codes_g as c;
use super::strings::{comments, decode, literals, Literal};
use super::rules::LintReport;
use super::LintCode;
use crate::Unit;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{DirectiveKind, ExprKind, StringLit};
use dartforge_intern::Interner;

/// The characters that may follow `\` (`allowedEscapedChars`).
const ESCAPABLE: [char; 12] = ['"', '\'', '$', '\\', 'n', 'r', 'f', 'b', 't', 'v', 'x', 'u'];

/// `_looksLikeUriOrPath`: the value has `/` or `\`.
fn looks_like_uri(text: &str) -> bool {
    text.contains('/') || text.contains('\\')
}

/// The value of the literal's text parts (without the interpolations).
fn literal_value(source: &str, l: &Literal) -> String {
    let text = l.text(source);
    if l.raw { text } else { decode(&text) }
}

/// `unnecessary_string_escapes` in a text part (`visitLexeme`): the places of
/// the unnecessary backslashes. `last`: the part touches the closing quote.
fn unnecessary_escapes(source: &str, l: &Literal, part: Span, last: bool, out: &mut Vec<usize>) {
    let Some(text) = source.get(part.start..part.end) else { return };
    let (own, other) = if l.single_quoted { ('\'', '"') } else { ('"', '\'') };
    // The string's own quotes in a row: (place, escaped). In a triple-quoted
    // string, up to two in a row do not need the escape.
    let mut pending: Vec<(usize, bool)> = Vec::new();
    let flush = |pending: &[(usize, bool)], out: &mut Vec<usize>| {
        if l.multiline && pending.len() < 3 {
            for &(place, escaped) in pending {
                // `'''…\''''`: without the backslash, the quotes merge with the closing ones.
                if escaped && !(last && place + 2 == part.end) {
                    out.push(place);
                }
            }
        }
    };
    let cs: Vec<(usize, char)> = text.char_indices().collect();
    let mut k = 0;
    while k < cs.len() {
        let (mut place, mut current) = cs[k];
        let mut escaped = false;
        if current == '\\' && k + 1 < cs.len() {
            escaped = true;
            let backslash = place;
            k += 1;
            (place, current) = cs[k];
            if current == other || !ESCAPABLE.contains(&current) {
                out.push(part.start + backslash);
            }
        }
        if current == own {
            pending.push((part.start + place - usize::from(escaped), escaped));
        } else {
            flush(&pending[..], &mut *out);
            pending.clear();
        }
        k += 1;
    }
    flush(&pending[..], &mut *out);
}

/// `use_raw_strings`: the content has an escape, and all are `\\` or `\$`.
fn only_escapes_backslash_and_dollar(content: &str) -> bool {
    let cs: Vec<char> = content.chars().collect();
    let mut has = false;
    let mut i = 0;
    while i + 1 < cs.len() {
        if cs[i] == '\\' {
            has = true;
            i += 1;
            if cs[i] != '\\' && cs[i] != '$' {
                return false;
            }
        }
        i += 1;
    }
    has
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unit<'_>, _interner: &Interner, enabled: &dyn Fn(&str) -> bool, _sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let a = u.ast;
    let source = u.source;
    let mut report = |code: &'static LintCode, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    let rules = [
        "prefer_single_quotes",
        "prefer_double_quotes",
        "avoid_escaping_inner_quotes",
        "unnecessary_string_escapes",
        "use_raw_strings",
        "leading_newlines_in_multiline_strings",
        "lines_longer_than_80_chars",
    ];
    if !rules.iter().any(|r| enabled(r)) {
        return out;
    }

    // All the unit's literals: those of the expressions and of the directives.
    let mut all: Vec<Literal> = Vec::new();
    for e in a.exprs.iter() {
        if matches!(e.kind, ExprKind::String(_)) {
            all.extend(literals(source, e.span));
        }
    }
    let mut add_directive = |lit: &StringLit| all.extend(literals(source, lit.span));
    for d in u.unit.directives.iter() {
        match &d.kind {
            DirectiveKind::Import { uri, configurations, .. } | DirectiveKind::Export { uri, configurations, .. } => {
                add_directive(uri);
                for k in configurations.iter() {
                    add_directive(&k.uri);
                    if let Some(v) = &k.value {
                        add_directive(v);
                    }
                }
            }
            DirectiveKind::Part { uri } | DirectiveKind::ImportAugment { uri } | DirectiveKind::AugmentLibrary { uri } => add_directive(uri),
            DirectiveKind::PartOf { uri: Some(uri), .. } => add_directive(uri),
            DirectiveKind::PartOf { uri: None, .. } | DirectiveKind::Library { .. } => {}
        }
    }
    all.sort_by_key(|l| (l.span.start, l.span.end));
    all.dedup();
    let interpolations: Vec<Span> = all.iter().flat_map(|l| l.interpolations.iter().copied()).collect();
    let inside = |outer: Span, x: Span| outer.start < x.start && x.end <= outer.end;
    // Inside another string's interpolation.
    let nested = |l: &Literal| interpolations.iter().any(|s| inside(*s, l.span));
    // Has another string in one of its interpolations.
    let contains_string = |l: &Literal| all.iter().any(|o| l.interpolations.iter().any(|s| inside(*s, o.span)));

    // `prefer_single_quotes` and `prefer_double_quotes` (the `QuoteVisitor`).
    for (rule, single, code) in
        [("prefer_single_quotes", true, &c::PREFER_SINGLE_QUOTES), ("prefer_double_quotes", false, &c::PREFER_DOUBLE_QUOTES)]
    {
        if !enabled(rule) {
            continue;
        }
        let wanted = if single { '\'' } else { '"' };
        for l in all.iter() {
            if l.single_quoted == single || literal_value(source, l).contains(wanted) || nested(l) {
                continue;
            }
            if l.is_interpolated() && contains_string(l) {
                continue;
            }
            report(code, l.span, &[]);
        }
    }
    // `avoid_escaping_inner_quotes`: a plain single-line string that escapes
    // its own quote and does not have the other one.
    if enabled("avoid_escaping_inner_quotes") {
        for l in all.iter().filter(|l| !l.raw && !l.multiline) {
            let text = literal_value(source, l);
            let (own, other) = if l.single_quoted { ("'", "\"") } else { ("\"", "'") };
            if text.contains(own) && !text.contains(other) {
                report(&c::AVOID_ESCAPING_INNER_QUOTES, l.span, &[own, other]);
            }
        }
    }
    // `unnecessary_string_escapes`.
    if enabled("unnecessary_string_escapes") {
        let mut places: Vec<usize> = Vec::new();
        for l in all.iter().filter(|l| !l.raw) {
            let n = l.parts.len();
            for (i, t) in l.parts.iter().enumerate() {
                unnecessary_escapes(source, l, *t, i + 1 == n, &mut places);
            }
        }
        for place in places {
            report(&c::UNNECESSARY_STRING_ESCAPES, Span { start: place, end: place + 1 }, &[]);
        }
    }
    // `use_raw_strings`: only a literal without interpolation.
    if enabled("use_raw_strings") {
        for l in all.iter().filter(|l| !l.raw && !l.is_interpolated()) {
            if only_escapes_backslash_and_dollar(&l.text(source)) {
                report(&c::USE_RAW_STRINGS, l.span, &[]);
            }
        }
    }
    // `leading_newlines_in_multiline_strings`: a triple-quoted string that
    // spans more than one line starts with a line break.
    if enabled("leading_newlines_in_multiline_strings") {
        for l in all.iter().filter(|l| l.multiline) {
            let Some(lexeme) = source.get(l.span.start..l.span.end) else { continue };
            let Some(first) = l.parts.first() else { continue };
            let head = source.get(first.start..).unwrap_or("");
            if lexeme.contains('\n') && !head.starts_with('\n') && !head.starts_with('\r') {
                report(&c::LEADING_NEWLINES_IN_MULTILINE_STRINGS, l.span, &[]);
            }
        }
    }
    // `lines_longer_than_80_chars`.
    if enabled("lines_longer_than_80_chars") {
        // The line starts, for the (1-based) number of a place.
        // `LineInfo`: `\n`, `\r\n` and a lone `\r` end a line.
        let bs = source.as_bytes();
        let mut line_starts: Vec<usize> = vec![0];
        line_starts.extend(
            bs.iter()
                .enumerate()
                .filter(|(i, b)| **b == b'\n' || (**b == b'\r' && bs.get(i + 1) != Some(&b'\n')))
                .map(|(i, _)| i + 1),
        );
        let line_of = |place: usize| line_starts.partition_point(|&x| x <= place);
        let mut long_lines: Vec<(usize, Span)> = Vec::new();
        for (i, &start) in line_starts.iter().enumerate() {
            let mut end = match line_starts.get(i + 1) {
                Some(&next) => next - 1,
                None => source.len(),
            };
            let Some(text) = source.get(start..end) else { continue };
            if text.encode_utf16().count() <= 80 {
                continue;
            }
            // The `\r\n` break does not count (only the last line stays as is).
            if i + 1 < line_starts.len() && text.ends_with('\r') {
                end -= 1;
            }
            let Some(text) = source.get(start..end) else { continue };
            if text.encode_utf16().count() <= 80 {
                continue;
            }
            // The report starts at column 80 (in UTF-16 units).
            let mut units = 0;
            let mut cut = end;
            for (k, ch) in text.char_indices() {
                if units >= 80 {
                    cut = start + k;
                    break;
                }
                units += ch.len_utf16();
            }
            long_lines.push((i + 1, Span { start: cut, end }));
        }
        if !long_lines.is_empty() {
            let mut allowed: Vec<usize> = Vec::new();
            // Strings: a triple-quoted one allows all its lines; a single-line
            // one, its own, if it looks like a URI or a path.
            for l in all.iter() {
                if l.multiline {
                    allowed.extend(line_of(l.span.start)..=line_of(l.span.end));
                } else if looks_like_uri(&literal_value(source, l)) {
                    allowed.push(line_of(l.span.start));
                }
            }
            // Comments: `// ignore:` allows the line; any comment line that
            // looks like a URI or a path allows its own.
            for s in comments(source) {
                let Some(text) = source.get(s.start..s.end) else { continue };
                let first = line_of(s.start);
                let mut lines: Vec<&str> = Vec::new();
                if let Some(rest) = text.strip_prefix("///") {
                    lines.push(rest);
                } else if let Some(rest) = text.strip_prefix("//") {
                    if rest.trim_start().starts_with("ignore:") {
                        allowed.push(first);
                    } else {
                        lines.push(rest);
                    }
                } else if text.len() >= 4 {
                    let inner = &text[2..text.len() - 2];
                    // Split at `\r\n`, then at `\r`, then at `\n`.
                    for a in inner.split("\r\n") {
                        for b in a.split('\r') {
                            lines.extend(b.split('\n'));
                        }
                    }
                }
                for (i, line_text) in lines.iter().enumerate() {
                    if line_text.contains('/') || line_text.contains('\\') {
                        allowed.push(first + i);
                    }
                }
            }
            for (line, span) in long_lines {
                if !allowed.contains(&line) {
                    report(&c::LINES_LONGER_THAN_80_CHARS, span, &[]);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn only(rule: &str, source: &str) -> Vec<String> {
        let mut names = Interner::new();
        let p = dartforge_frontend::parser::parse(source, &mut names);
        let u = Unit { ast: &p.ast, unit: &p.unit, source };
        let mut reports = run(u, &names, &|r| r == rule, None);
        reports.sort_by_key(|r| (r.span.start, r.span.end));
        reports.into_iter().map(|r| source[r.span.start..r.span.end].to_string()).collect()
    }

    #[test]
    fn quotes() {
        let source = "import \"dart:io\";\nvar a = \"x\";\nvar b = \"it's\";\nvar c = 'y';\nvar d = \"p$a\";\nvar e = \"q${a == 'x' ? 'y' : \"z\"}\";\n";
        assert_eq!(only("prefer_single_quotes", source), vec!["\"dart:io\"".to_string(), "\"x\"".to_string(), "\"p$a\"".to_string()]);
        assert_eq!(only("prefer_double_quotes", "var a = 'x';\nvar b = 'say \"hi\"';\nvar c = \"y\";\n"), vec!["'x'".to_string()]);
        assert_eq!(
            only("avoid_escaping_inner_quotes", "var a = 'it\\'s';\nvar b = 'it\\'s \"x\"';\nvar c = \"it's\";\n"),
            vec!["'it\\'s'".to_string()]
        );
    }

    #[test]
    fn escapes() {
        // `\"` in single quotes, `\a`; `\n`, `\'` and `\$` stay.
        assert_eq!(
            only("unnecessary_string_escapes", "var a = 'x\\\"y\\az\\n\\'\\$';\n"),
            vec!["\\".to_string(), "\\".to_string()]
        );
        // In triple quotes, the string's own quote alone needs no escape.
        assert_eq!(only("unnecessary_string_escapes", "var a = '''x\\'y''';\n").len(), 1);
        assert!(only("unnecessary_string_escapes", "var a = r'x\\ay';\n").is_empty());
        assert_eq!(only("use_raw_strings", "var a = 'a\\\\b\\$c';\nvar b = 'a\\\\b\\n';\nvar c = 'abc';\n"), vec!["'a\\\\b\\$c'".to_string()]);
    }

    #[test]
    fn lines() {
        assert_eq!(
            only("leading_newlines_in_multiline_strings", "var a = '''x\ny''';\nvar b = '''\nx''';\nvar c = '''x''';\n"),
            vec!["'''x\ny'''".to_string()]
        );
        let long_line = format!("var a = {};\n", "1 + ".repeat(20) + "1");
        let reports = only("lines_longer_than_80_chars", &long_line);
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].len(), long_line.len() - 1 - 80);
        // A line with a URI in a string or a comment is allowed.
        let with_uri = format!("var a = 'https://example/{}';\n// see http://{}\n", "x".repeat(80), "y".repeat(80));
        assert!(only("lines_longer_than_80_chars", &with_uri).is_empty());
    }
}
