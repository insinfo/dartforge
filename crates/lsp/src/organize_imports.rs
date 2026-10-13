//! `Organize Imports` (`dart.edit.organizeImports`): the Dart 3.6.2 server's
//! `ImportOrganizer` (`AS:src/services/correction/organize_imports.dart:22-345`
//! and `AP:src/utilities/directive_sort.dart`; docs/LSP-ESPECIFICACAO.md
//! §13.12.6).
//!
//! The `import`, `export` and `part` directives are sorted by group (nine
//! priorities) and by URI; each one carries the comment above it and the
//! first end-of-line comment; the whole block, from the first to the last,
//! is replaced by the concatenation of the texts, with a blank line between
//! groups. With `remove`, the imports flagged by the errors
//! (`unused_import`, `duplicate_import`, `unnecessary_import`) and the
//! textual duplicates go away, unless there is an unresolved identifier.
//!
//! Library annotations (`TargetKind.library`) on the first directive are not
//! recognized: they need the resolved unit. That is the original's behavior
//! when called by `Sort Members`.
//! Written without compiling or running (2026-10-04).

use crate::sort_members::{Comment, comments, line_of};
use dartforge_frontend::ast::{CompilationUnit, DirectiveKind};
use std::cmp::Ordering;

struct Info {
    /// The ordinal of `DirectiveSortPriority`.
    priority: u8,
    uri: String,
    start: usize,
    end: usize,
    /// There is an unused, duplicate or unnecessary import error on the URI
    /// literal.
    removable: bool,
}

/// The order of Dart's `String.compareTo`: UTF-16 units.
fn utf16(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `compareDirectiveUri` (`directive_sort.dart:10-22`).
fn compare_uri(a: &str, b: &str) -> Ordering {
    if (!a.starts_with("package:") || !b.starts_with("package:")) && !a.starts_with('/') && !b.starts_with('/') {
        return utf16(a, b);
    }
    match (a.find('/'), b.find('/')) {
        (Some(i), Some(j)) => utf16(&a[..i], &b[..j]).then_with(|| utf16(&a[i + 1..], &b[j + 1..])),
        _ => utf16(a, b),
    }
}

/// The start of the line after the offset's line (`getOffsetOfLineAfter`);
/// `None` when there is none (the original throws).
fn next_line(code: &str, offset: usize) -> Option<usize> {
    code.get(offset..)?.find('\n').map(|k| offset + k + 1)
}

/// `// @dart = 2.12`: the language version comment (`LanguageVersionToken`).
fn language_version(comment: &str) -> bool {
    let Some(rest) = comment.strip_prefix("//") else { return false };
    let Some(rest) = rest.trim_start().strip_prefix("@dart") else { return false };
    let Some(rest) = rest.trim_start().strip_prefix('=') else { return false };
    let mut parts = rest.trim().splitn(2, '.');
    let number = |p: Option<&str>| p.is_some_and(|x| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit()));
    number(parts.next()) && number(parts.next())
}

/// `IgnoreInfo.ignoreMatcher`: `^//+[ ]*ignore:`.
fn is_ignore_comment(comment: &str) -> bool {
    comment.starts_with("//") && comment.trim_start_matches('/').trim_start_matches(' ').starts_with("ignore:")
}

/// The documentation block of a gap: the last sequence of consecutive
/// documentation comments (start and end).
fn documentation_block(cs: &[Comment]) -> Option<(usize, usize)> {
    let last = cs.iter().rposition(|c| c.is_documentation)?;
    let mut first = last;
    while first > 0 && cs[first - 1].is_documentation {
        first -= 1;
    }
    Some((cs[first].start, cs[last].end))
}

/// `getLeadingComment` (`:227-284`): the offset of the comment above that
/// goes with the directive. `cs`: the comments between the previous token
/// and the directive; `previous_end`: the end of the previous token (`None`
/// when the directive is the first token of the file).
fn comment_above(code: &str, cs: &[Comment], pseudo: bool, previous_end: Option<usize>, directive: usize) -> Option<usize> {
    if cs.is_empty() {
        return None;
    }
    let line = |offset: usize| line_of(code, offset);
    let mut k = 0;
    if pseudo {
        // The start of the last block without a blank line in the middle.
        for (i, pair) in cs.windows(2).enumerate() {
            if line(pair[1].start) > line(pair[0].start) + 1 {
                k = i + 1;
            }
        }
    }
    if language_version(&code[cs[k].start..cs[k].end]) {
        k += 1;
    }
    let c = cs.get(k)?;
    // The first comment of the file is a header, unless it is an `ignore:`.
    if previous_end.is_none() && k == 0 {
        return is_ignore_comment(&code[c.start..c.end]).then_some(c.start);
    }
    if pseudo {
        return (line(directive) == line(c.start) + 1).then_some(c.start);
    }
    // Those that start on the previous token's line are its end-of-line comment.
    let previous_line = line(previous_end.unwrap_or(0));
    cs[k..].iter().find(|x| line(x.start) != previous_line).map(|x| x.start)
}

/// The end of the comment that starts at `pos` or after whitespace on the
/// same line (`getTrailingComment`: only the first one).
fn line_end_comment(code: &str, pos: usize) -> Option<usize> {
    let rest = code.get(pos..)?;
    let blanks = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    let start = pos + blanks;
    let b = code.as_bytes();
    if code[start..].starts_with("//") {
        let end = code[start..].find('\n').map_or(code.len(), |k| start + k);
        return Some(if end > start && b[end - 1] == b'\r' { end - 1 } else { end });
    }
    if code[start..].starts_with("/*") {
        let mut level = 1;
        let mut j = start + 2;
        while j < b.len() && level > 0 {
            if j + 1 < b.len() && b[j] == b'/' && b[j + 1] == b'*' {
                level += 1;
                j += 2;
            } else if j + 1 < b.len() && b[j] == b'*' && b[j + 1] == b'/' {
                level -= 1;
                j += 2;
            } else {
                j += 1;
            }
        }
        return Some(j.min(b.len()));
    }
    None
}

/// `ImportOrganizer.organize` up to `code`: the code with the directive
/// block rewritten. `None` when there is no directive with a URI, or in a
/// case where the original throws (a documented first directive without a
/// following line).
///
/// `removable`: the offsets of the URI literals with an `unused_import`,
/// `duplicate_import` or `unnecessary_import` error; `not_resolved`: there
/// is an unresolved identifier error in the file; `remove`: `removeUnused`.
pub(crate) fn organize(code: &str, unit: &CompilationUnit, removable: &[usize], not_resolved: bool, remove: bool) -> Option<String> {
    let line_end = if code.contains("\r\n") { "\r\n" } else { "\n" };
    let mut has_library = false;
    let mut infos: Vec<Info> = Vec::new();
    // The script tag is a token: what comes after it is not the file's first
    // comment.
    let mut previous_end: Option<usize> = unit.script_tag.map(|s| s.end);
    for (i, d) in unit.directives.iter().enumerate() {
        let (ty, literal) = match &d.kind {
            DirectiveKind::Import { uri, .. } => (0u8, uri),
            DirectiveKind::Export { uri, .. } => (1u8, uri),
            DirectiveKind::Part { uri } => (2u8, uri),
            other => {
                if matches!(other, DirectiveKind::Library { .. }) {
                    has_library = true;
                }
                previous_end = Some(d.span.end);
                continue;
            }
        };
        let content = dartforge_elements::load::string_lit_value(literal).unwrap_or_default();
        let group = if content.starts_with("dart:") {
            0
        } else if content.starts_with("package:") {
            1
        } else if content.contains("://") {
            2
        } else {
            3
        };
        let priority = if ty == 2 { 8 } else { ty * 4 + group };
        let from = previous_end.unwrap_or(0).min(d.span.start);
        let cs = comments(code, from, d.span.start);
        // `directive.offset` includes the documentation comment.
        let doc = documentation_block(&cs);
        let directive_offset = doc.map_or(d.span.start, |(start, _)| start);
        let pseudo = !has_library && i == 0;
        // On the first directive without `library`, the documentation belongs to
        // the library and stays at the top, with a blank line after it.
        let mut library_end: Option<usize> = None;
        if pseudo && let Some((_, end_doc)) = doc {
            let next = next_line(code, end_doc)?;
            let line_end_ = code[next..].find('\n').map_or(code.len(), |k| next + k);
            library_end =
                Some(if code[next..line_end_].trim().is_empty() { next_line(code, next)? } else { next });
        }
        // With documentation, the directive's first token is the documentation
        // comment, which has no comments before it.
        let from_above = if doc.is_some() { None } else { comment_above(code, &cs, pseudo, previous_end, d.span.start) };
        let end = line_end_comment(code, d.span.end).unwrap_or(d.span.end);
        let start = library_end.or(from_above).unwrap_or(directive_offset);
        if start > end {
            return None;
        }
        infos.push(Info { priority, uri: content, start, end, removable: removable.contains(&literal.span.start) });
        previous_end = Some(d.span.end);
    }
    let (first, last) = (infos.first()?.start, infos.last()?.end);
    if last < first {
        return None;
    }
    let mut order: Vec<&Info> = infos.iter().collect();
    order.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| compare_uri(&a.uri, &b.uri))
            .then_with(|| utf16(&code[a.start..a.end], &code[b.start..b.end]))
    });
    let mut output = String::new();
    let mut current: Option<u8> = None;
    let mut previous: &str = "";
    for d in order {
        let text = &code[d.start..d.end];
        if !not_resolved && remove && (d.removable || previous == text) {
            continue;
        }
        if current != Some(d.priority) {
            if current.is_some() {
                output.push_str(line_end);
            }
            current = Some(d.priority);
        }
        output.push_str(text);
        output.push_str(line_end);
        previous = text;
    }
    Some(format!("{}{}{}", &code[..first], output.trim_end(), &code[last..]))
}

/// The `organize` edit (`:47-58`): from offset 0 to the end of the changed
/// block (only the common suffix is trimmed). `(offset, length, text)`.
pub(crate) fn edit(initial: &str, new: &str) -> Option<(usize, usize, String)> {
    if initial == new {
        return None;
    }
    let (a, n) = (initial.as_bytes(), new.as_bytes());
    let mut suffix = a.iter().rev().zip(n.iter().rev()).take_while(|(x, y)| x == y).count();
    while !initial.is_char_boundary(a.len() - suffix) || !new.is_char_boundary(n.len() - suffix) {
        suffix -= 1;
    }
    Some((0, a.len() - suffix, new[..n.len() - suffix].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn organized(source: &str, removable: &[&str]) -> String {
        let mut names = dartforge_intern::Interner::new();
        let p = dartforge_frontend::parser::parse(source, &mut names);
        // Each removable one is given by the literal's text; the 1st occurrence counts.
        let offsets: Vec<usize> = removable.iter().map(|r| source.find(r).expect("literal present")).collect();
        organize(source, &p.unit, &offsets, false, true).unwrap_or_else(|| source.to_string())
    }

    /// Example 1 of §13.12.6.
    #[test]
    fn spec_example_one() {
        let before = "// Copyright.\n\nimport 'package:b/b.dart';\nimport 'dart:io'; // io\nimport 'src/z.dart';\nimport 'dart:async';\n// sobre a\nimport 'package:a/a.dart';\nexport 'src/y.dart';\nimport 'dart:io'; // io\n\nvoid main() {}\n";
        let after = "// Copyright.\n\nimport 'dart:io'; // io\n\n// sobre a\nimport 'package:a/a.dart';\nimport 'package:b/b.dart';\n\nimport 'src/z.dart';\n\nexport 'src/y.dart';\n\nvoid main() {}\n";
        assert_eq!(organized(before, &["'dart:async'"]), after);
    }

    #[test]
    fn edit_starts_at_offset_zero() {
        let before = "import 'b.dart';\nimport 'a.dart';\n\nvoid main() {}\n";
        let after = organized(before, &[]);
        assert_eq!(after, "import 'a.dart';\nimport 'b.dart';\n\nvoid main() {}\n");
        let (offset, length, text) = edit(before, &after).expect("changed");
        assert_eq!(offset, 0);
        assert_eq!(format!("{}{}", text, &before[length..]), after);
    }

    #[test]
    fn package_prefix_of_another_comes_first() {
        assert_eq!(compare_uri("package:a/x.dart", "package:a.b/x.dart"), Ordering::Less);
        assert_eq!(compare_uri("dart:io", "dart:async"), Ordering::Greater);
    }

    #[test]
    fn special_comments() {
        assert!(language_version("// @dart = 2.12"));
        assert!(!language_version("// @dart"));
        assert!(is_ignore_comment("// ignore: unused_import"));
        assert!(!is_ignore_comment("// ignore_for_file: unused_import"));
    }
}
