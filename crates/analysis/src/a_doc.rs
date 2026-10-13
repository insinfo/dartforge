//! The `{@name …}` directives of documentation comments
//! (docs/ANALYZER-ESPECIFICACAO.md §A, group 4, and A.R4-2): the argument
//! reader of the `DocCommentBuilder` (`_DirectiveParser`,
//! `analyzer/lib/src/fasta/doc_comment_builder.dart:1039-1245`) and the
//! validations of the `DocCommentVerifier`
//! (`analyzer/lib/src/error/doc_comment_verifier.dart:52-160`):
//! `doc_directive_missing_argument` (one, two or three missing),
//! `doc_directive_has_extra_arguments`,
//! `doc_directive_has_unexpected_named_argument` and
//! `doc_directive_argument_wrong_format`.
//!
//! Only `///` comments are read (not `/** */`); a directive counts when it
//! opens the comment line, outside a fenced (` ``` `) or indented code block.
//! Written without compiling or running (2026-10-04).

use crate::Unidade;
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Any,
    Integer,
    Uri,
    Youtube,
}

impl Format {
    fn display(self) -> &'static str {
        match self {
            Format::Any => "any",
            Format::Integer => "an integer",
            Format::Uri => "a URI",
            Format::Youtube => "a YouTube URL, starting with 'https://www.youtube.com/watch?v='",
        }
    }
}

const YOUTUBE_PREFIX: &str = "https://www.youtube.com/watch?v=";

/// A directive type (`DocDirectiveType`).
struct DirectiveType {
    nome: &'static str,
    positional: &'static [(&'static str, Format)],
    named: &'static [&'static str],
    rest: bool,
}

const DIRECTIVE_TYPES: &[DirectiveType] = &[
    DirectiveType {
        nome: "animation",
        positional: &[("width", Format::Integer), ("height", Format::Integer), ("url", Format::Uri)],
        named: &["id"],
        rest: false,
    },
    DirectiveType { nome: "canonicalFor", positional: &[("element", Format::Any)], named: &[], rest: false },
    DirectiveType { nome: "category", positional: &[], named: &[], rest: true },
    DirectiveType { nome: "end-inject-html", positional: &[], named: &[], rest: false },
    DirectiveType { nome: "end-tool", positional: &[], named: &[], rest: false },
    DirectiveType { nome: "endtemplate", positional: &[], named: &[], rest: false },
    DirectiveType { nome: "inject-html", positional: &[], named: &[], rest: false },
    DirectiveType { nome: "macro", positional: &[("name", Format::Any)], named: &[], rest: false },
    DirectiveType { nome: "subCategory", positional: &[], named: &[], rest: true },
    DirectiveType { nome: "template", positional: &[("name", Format::Any)], named: &[], rest: false },
    DirectiveType { nome: "tool", positional: &[("name", Format::Any)], named: &[], rest: true },
    DirectiveType {
        nome: "youtube",
        positional: &[("width", Format::Integer), ("height", Format::Integer), ("url", Format::Youtube)],
        named: &[],
        rest: false,
    },
];

/// A parsed argument: the absolute range and, if it is named, the name.
struct Argument<'a> {
    span: Span,
    nome: Option<&'a str>,
    value: &'a str,
}

fn is_blank(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 0x0b | 0x0c)
}

/// `int.tryParse`: decimal with an optional sign, or hexadecimal `0x…`.
fn is_integer(s: &str) -> bool {
    let t = s.strip_prefix(['-', '+']).unwrap_or(s);
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return !h.is_empty() && h.bytes().all(|b| b.is_ascii_hexdigit());
    }
    !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit())
}

/// `Uri.tryParse`, on the safe side: only rejects what Dart's URI parser
/// surely rejects (a `%` not followed by two hex digits, and an IPv6 bracket
/// without its closer).
fn is_uri(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && !(b.get(i + 1).is_some_and(u8::is_ascii_hexdigit) && b.get(i + 2).is_some_and(u8::is_ascii_hexdigit)) {
            return false;
        }
        i += 1;
    }
    !(s.contains("://[") && !s.contains(']'))
}

/// Reads and validates the directive that opens at `index` (the `{@`) of
/// `contents`, whose first byte is at `base` in the source. `before` is how
/// many bytes the analyzer's line contents (everything after the `///`) have
/// before `contents` (the removed space).
///
/// The `_DirectiveParser` (3.6.2, `doc_comment_builder.dart:1084-1200`) adds
/// the index in the line contents to the offset of the `{@`
/// (`_offset + index`) for the end of the directive and the position of
/// `DOC_DIRECTIVE_MISSING_CLOSING_BRACE`: both are shifted by what precedes
/// the `{@` on the line, and we reproduce that. A directive with nothing after
/// the name (`{@youtube`) ends at the end of the line without arguments and
/// without the brace warning (`directive`, `:1086-1088`, and the early return
/// of `_parseArguments`).
fn directive(contents: &str, base: usize, before: usize, index: usize, out: &mut Vec<Diagnostic>) {
    let b = contents.as_bytes();
    let n = b.len();
    let start = index;
    let mut i = index + 2;
    if i >= n {
        return;
    }
    let name_start = i;
    while i < n && !is_blank(b[i]) && b[i] != b'}' {
        i += 1;
    }
    let name = &contents[name_start..i];
    while i < n && is_blank(b[i]) {
        i += 1;
    }
    let Some(ty) = DIRECTIVE_TYPES.iter().find(|t| t.nome == name) else { return };
    // `_offset + index`: the offset of the `{@` plus the index in the line contents.
    let in_analyzer = |index_in_contents: usize| base + start + before + index_in_contents;
    // `_parseArguments`.
    let mut positional: Vec<Argument<'_>> = Vec::new();
    let mut named: Vec<Argument<'_>> = Vec::new();
    let mut end = n;
    let empty = i >= n;
    let mut closed = empty;
    while i < n {
        if b[i] == b'}' {
            i += 1;
            end = i;
            closed = true;
            break;
        }
        // `_parseArgument`.
        let arg_start = i;
        let mut only_alnum = true;
        let mut named_arg: Option<(usize, usize)> = None;
        while i < n {
            let ch = b[i];
            if is_blank(ch) || ch == b'}' {
                break;
            }
            if ch == b'=' && only_alnum {
                let name_end = i;
                i += 1;
                let value_start = i;
                while i < n && !is_blank(b[i]) && b[i] != b'}' {
                    i += 1;
                }
                named_arg = Some((name_end, value_start));
                break;
            }
            if !ch.is_ascii_alphanumeric() {
                only_alnum = false;
            }
            i += 1;
        }
        let span = Span { start: base + arg_start, end: base + i };
        match named_arg {
            Some((name_end, value_start)) => {
                named.push(Argument { span, nome: Some(&contents[arg_start..name_end]), value: &contents[value_start..i] })
            }
            None => positional.push(Argument { span, nome: None, value: &contents[arg_start..i] }),
        }
        while i < n && is_blank(b[i]) {
            i += 1;
        }
        end = i;
    }
    // `_parseArguments`: the end of the line without `}`, on the last character.
    if !closed && n > 0 {
        let p = in_analyzer(n) - 1;
        out.push(Diagnostic::com_codigo(w::DOC_DIRECTIVE_MISSING_CLOSING_BRACE, Span { start: p, end: p + 1 }, Vec::<&str>::new()));
    }
    let tag = Span { start: base + start, end: in_analyzer(end) };
    // `validateArgumentCount`.
    let required = ty.positional.len();
    if positional.len() < required {
        let missing = required - positional.len();
        let names: Vec<&str> = ty.positional[required - missing.min(3)..].iter().map(|p| p.0).collect();
        let code = match missing {
            1 => Some(w::DOC_DIRECTIVE_MISSING_ONE_ARGUMENT),
            2 => Some(w::DOC_DIRECTIVE_MISSING_TWO_ARGUMENTS),
            3 => Some(w::DOC_DIRECTIVE_MISSING_THREE_ARGUMENTS),
            _ => None,
        };
        if let Some(code) = code {
            let mut args: Vec<&str> = vec![ty.nome];
            args.extend(names);
            out.push(Diagnostic::com_codigo(code, tag, args));
        }
    }
    if !ty.rest {
        if positional.len() > required {
            let span = Span { start: positional[required].span.start, end: positional[positional.len() - 1].span.end };
            let (given, expected) = (positional.len().to_string(), required.to_string());
            out.push(Diagnostic::com_codigo(w::DOC_DIRECTIVE_HAS_EXTRA_ARGUMENTS, span, [ty.nome, given.as_str(), expected.as_str()]));
        }
        for a in &named {
            let arg_name = a.nome.unwrap_or("");
            if !ty.named.contains(&arg_name) {
                out.push(Diagnostic::com_codigo(w::DOC_DIRECTIVE_HAS_UNEXPECTED_NAMED_ARGUMENT, a.span, [ty.nome, arg_name]));
            }
        }
    }
    // `validateArgumentFormat`: roda sempre.
    for (a, (param_name, format)) in positional.iter().zip(ty.positional.iter()) {
        let ok = match format {
            Format::Any => true,
            Format::Integer => is_integer(a.value),
            Format::Uri => is_uri(a.value),
            Format::Youtube => is_uri(a.value) && a.value.starts_with(YOUTUBE_PREFIX),
        };
        if !ok {
            out.push(Diagnostic::com_codigo(w::DOC_DIRECTIVE_ARGUMENT_WRONG_FORMAT, a.span, [*param_name, format.display()]));
        }
    }
}

/// A `@docImport` of a `///` line (`DocCommentBuilder._parseDocImport`): the
/// URI (the value and the literal), the `deferred` and the configurations, at
/// the unit's offsets.
pub struct DocImport {
    pub uri: Option<String>,
    pub literal: Span,
    pub deferred: Option<Span>,
    pub configurations: Option<Span>,
    /// The URI has interpolation (`'${'foo'}.dart'`).
    pub interpolated: bool,
}

/// `_parseDocImport`: `@docImport ` at `index` of the line contents (which
/// start at `base`); the rest of the line is read as `import …`.
fn doc_import(contents: &str, base: usize, index: usize) -> Option<DocImport> {
    const DOC_IMPORT: &str = "@docImport ";
    let after = contents.get(index..)?.strip_prefix(DOC_IMPORT)?;
    let blanks = after.len() - after.trim_start_matches([' ', '\t']).len();
    let idx = index + DOC_IMPORT.len() + blanks;
    let synthetic = format!("import {}", &contents[idx..]);
    // The `sourceMap`: the start of the synthetic text is 7 before the rest.
    let shift = (base + idx) as isize - 7;
    let map = |s: Span| Span { start: (s.start as isize + shift) as usize, end: (s.end as isize + shift) as usize };
    let mut names = dartforge_intern::Interner::new();
    let parsed = dartforge_frontend::parser::parse(&synthetic, &mut names);
    let d = parsed.unit.directives.first()?;
    let dartforge_frontend::ast::DirectiveKind::Import { uri, configurations, deferred, .. } = &d.kind else { return None };
    let after_uris = configurations.last().map_or(uri.span.end, |c| c.span.end);
    let deferred = if *deferred {
        synthetic.get(after_uris..).and_then(|s| s.find("deferred")).map(|k| map(Span { start: after_uris + k, end: after_uris + k + 8 }))
    } else {
        None
    };
    let configurations = match (configurations.first(), configurations.last()) {
        (Some(a), Some(b)) => Some(map(Span { start: a.span.start, end: b.span.end })),
        _ => None,
    };
    let interpolated = uri.parts.iter().any(|p| matches!(p, dartforge_frontend::ast::StringPart::Interpolation(_)));
    Some(DocImport {
        uri: dartforge_elements::load::string_lit_value(uri).map(|s| s.to_string()),
        literal: map(uri.span),
        deferred,
        configurations,
        interpolated,
    })
}

/// The `@docImport`s of the documentation comment of the `library` directive
/// (the only ones the `LibraryAnalyzer` resolves).
pub fn library_doc_imports(u: Unidade<'_>) -> Vec<DocImport> {
    let Some(d) = u.unit.directives.iter().find(|d| matches!(d.kind, dartforge_frontend::ast::DirectiveKind::Library { .. })) else { return Vec::new() };
    let source = u.fonte;
    let comments = dartforge_frontend::comentarios::Comentarios::de(source);
    let before = d.metadata.first().map_or(d.span.start, |m| m.span.start.min(d.span.start));
    let Some(doc) = comments.dart_doc(source, before) else { return Vec::new() };
    let mut out = Vec::new();
    let mut pos = doc.start;
    for line in source[doc.start..before].split_inclusive('\n') {
        let start = pos;
        pos += line.len();
        let trimmed = line.trim_end_matches(['\n', '\r']);
        let indent = trimmed.len() - trimmed.trim_start().len();
        let Some(contents) = trimmed[indent..].strip_prefix("///") else { continue };
        let (contents, base) = match contents.strip_prefix(' ') {
            Some(c) => (c, start + indent + 4),
            None => (contents, start + indent + 3),
        };
        let blanks = contents.len() - contents.trim_start().len();
        if let Some(i) = doc_import(contents, base, blanks) {
            out.push(i);
        }
    }
    out
}

/// The `DocCommentBuilder`'s `_isLinkText`: the `]` at `end` is followed by
/// `(` or `[` (or by `:` in a reference definition).
fn is_link_text(contents: &[u8], end: usize, may_be_definition: bool) -> bool {
    match contents.get(end + 1) {
        Some(b'(') | Some(b'[') => true,
        Some(b':') => may_be_definition,
        _ => false,
    }
}

/// `_parseReferences` reduced to what the `BestPracticesVerifier` looks at:
/// the `new` that opens an `[new A]`/`[new A.b]` reference
/// (`DEPRECATED_NEW_IN_COMMENT_REFERENCE`, on the `new`).
fn references_with_new(contents: &str, base: usize, out: &mut Vec<Diagnostic>) {
    let b = contents.as_bytes();
    let n = b.len();
    let mut i = 0usize;
    let mut only_blanks = true;
    while i < n {
        match b[i] {
            b'[' => {
                i += 1;
                if i < n && b[i] == b':' {
                    match contents[i + 1..].find(":]") {
                        Some(k) => i = i + 1 + k + 1,
                        None => break,
                    }
                } else {
                    let start = i;
                    let end = contents[i..].find(']').map_or(n, |k| i + k);
                    if !(end < n && is_link_text(b, end, only_blanks)) {
                        let text = &contents[start..end];
                        let leading = text.len() - text.trim_start().len();
                        let rest = &text[leading..];
                        if let Some(after) = rest.strip_prefix("new")
                            && after.starts_with([' ', '\t'])
                            && after.trim_start().starts_with(|c: char| c.is_ascii_alphabetic() || c == '_' || c == '$')
                        {
                            let start = base + start + leading;
                            out.push(Diagnostic::com_codigo(w::DEPRECATED_NEW_IN_COMMENT_REFERENCE, Span { start: start, end: start + 3 }, Vec::<&str>::new()));
                        }
                    }
                    i = end;
                }
                only_blanks = false;
            }
            b'`' => {
                if let Some(k) = contents[i + 1..].find('`') {
                    i = i + 1 + k;
                }
                only_blanks = false;
            }
            c if !is_blank(c) => only_blanks = false,
            _ => {}
        }
        i += 1;
    }
}

/// The diagnostics of the documentation directives of a unit.
pub fn check(u: Unidade<'_>) -> Vec<Diagnostic> {
    let source = u.fonte;
    let mut out = Vec::new();
    let mut pos = 0;
    // The state of the current comment (a sequence of `///` lines).
    let mut in_comment = false;
    let mut fenced = false;
    let mut indented = false;
    let mut previous_empty = true;
    for line in source.split_inclusive('\n') {
        let line_start = pos;
        pos += line.len();
        let trimmed = line.trim_end_matches(['\n', '\r']);
        let indent = trimmed.len() - trimmed.trim_start().len();
        let rest = &trimmed[indent..];
        if !rest.starts_with("///") {
            in_comment = false;
            continue;
        }
        if !in_comment {
            in_comment = true;
            fenced = false;
            indented = false;
            previous_empty = true;
        }
        // The line contents: after the `///` and one space, if any.
        let mut base = line_start + indent + 3;
        let mut contents = &rest[3..];
        if let Some(c) = contents.strip_prefix(' ') {
            contents = c;
            base += 1;
        }
        let blanks = contents.len() - contents.trim_start().len();
        let text = &contents[blanks..];
        if fenced {
            if text.starts_with("```") {
                fenced = false;
            }
            previous_empty = false;
            continue;
        }
        // Indented code block: four spaces after an empty line, up to the
        // first line with less indentation.
        if (previous_empty || indented) && blanks >= 4 {
            indented = true;
            previous_empty = text.is_empty();
            continue;
        }
        indented = false;
        if text.starts_with("```") {
            fenced = true;
            previous_empty = false;
            continue;
        }
        if text.starts_with("{@") {
            directive(contents, base, base - (line_start + indent + 3), blanks, &mut out);
            previous_empty = false;
            continue;
        }
        // `DocCommentVerifier.docImport` (`doc_comment_verifier.dart:35-51`).
        if let Some(i) = doc_import(contents, base, blanks) {
            if let Some(s) = i.deferred {
                out.push(Diagnostic::com_codigo(w::DOC_IMPORT_CANNOT_BE_DEFERRED, s, Vec::<&str>::new()));
            }
            if let Some(s) = i.configurations {
                out.push(Diagnostic::com_codigo(w::DOC_IMPORT_CANNOT_HAVE_CONFIGURATIONS, s, Vec::<&str>::new()));
            }
            previous_empty = false;
            continue;
        }
        references_with_new(contents, base, &mut out);
        previous_empty = contents.is_empty();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reports(source: &str) -> Vec<(&'static str, usize, usize)> {
        let mut names = dartforge_intern::Interner::new();
        let p = dartforge_frontend::parser::parse(source, &mut names);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte: source };
        check(u).into_iter().map(|d| (d.code.map_or("", |c| c.info().nome), d.span.start, d.span.end - d.span.start)).collect()
    }

    /// The live-oracle example in A.R4-2 (the arity and named-argument
    /// reports; `doc_directive_missing_opening_tag` belongs to another
    /// verifier).
    #[test]
    fn oracle_example() {
        let source = "/// {@youtube 600 400 https://www.youtube.com/watch?v=x extra}\n\
/// {@macro name extra}\n\
/// {@category a b c}\n\
/// {@animation 600 400 http://x.mp4 foo=bar}\n\
/// {@macro name foo=bar}\n\
/// {@animation 600 400 http://x.mp4 id=a}\n\
/// {@end-inject-html extra}\n\
class A {}\n";
        let r = reports(source);
        assert_eq!(
            r,
            vec![
                ("doc_directive_has_extra_arguments", 56, 5),
                ("doc_directive_has_extra_arguments", 80, 5),
                ("doc_directive_has_unexpected_named_argument", 146, 7),
                ("doc_directive_has_unexpected_named_argument", 172, 7),
                ("doc_directive_has_extra_arguments", 246, 5),
            ]
        );
    }

    #[test]
    fn missing_and_format() {
        let r = reports("/// {@youtube 600}\nclass A {}\n");
        // The end of the tag is the offset of the `{@` plus the index in the
        // line contents (with the space after the `///`): length 15 in dart 3.6.2.
        assert_eq!(r, vec![("doc_directive_missing_argument", 4, 15)]);
        let r = reports("/// {@animation x 400 http://a}\nclass A {}\n");
        assert_eq!(r, vec![("doc_directive_argument_wrong_format", 16, 1)]);
    }

    #[test]
    fn inside_fenced_block_does_not_count() {
        assert!(reports("/// ```\n/// {@macro}\n/// ```\nclass A {}\n").is_empty());
    }
}
