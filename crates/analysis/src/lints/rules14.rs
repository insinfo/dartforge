//! The fourteenth batch of lint rules (docs/ANALYZER-ESPECIFICACAO-INFRA.md
//! §8), written directly from the 3.6.2 emitters
//! (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`): those of the
//! `core` set that do not need subtyping.
//!
//! * `dangling_library_doc_comments` and `unintended_html_in_doc_comment`
//!   read the documentation comments as the `AstBuilder` attaches them
//!   (`_findComment`: `findDartDoc` before the token after the annotations,
//!   otherwise before each annotation, from the last to the first; the
//!   `Comment`'s tokens are the initial one and, in a run of `///`, all the
//!   following `///` of the chain) on the nodes that receive it: directives,
//!   top-level declarations, members, enum constants, type parameters, formal
//!   parameters (before the `this`, the type or the name), local variable
//!   lists (before the name of the first one) and pattern declarations.
//!   Local functions receive no comment. The code blocks are those of the
//!   `DocCommentBuilder` (fenced and indented), and the
//!   `_markdownTokenPattern` pattern is matched by hand, alternative by
//!   alternative, with the semantics of the VM's `RegExp`
//!   (`caseSensitive: false`, without `unicode`).
//! * `file_names` uses the file name of the unit that defines the library
//!   (`library2.firstFragment.source.shortName`), also in parts.
//! * `valid_regexps` validates the literal with the port of the VM's parser
//!   (`super::regexp_vm`).
//! * `library_annotations` reads `targetKinds` (`crate::meta`) and
//!   `@pragma('dart2js:late:trust')`.
//! * `no_wildcard_variable_uses`: the identifier whose element is a local
//!   variable or a parameter; the left side of an assignment and the operand
//!   of `++`/`--` have no element (`readElement`/`writeElement` stay on the
//!   parent).
//! * `type_literal_in_constant_pattern`: the static type `Type` of
//!   `dart:core`.
//! * `avoid_types_as_parameter_names`: `resolveNameInScope` from the first
//!   ancestor with a scope of the `ScopeResolverVisitor`, by regions: each
//!   function's type parameters hold in the whole function; the parameters,
//!   in the body; a block's locals (variables, pattern declarations and local
//!   functions), in the whole block; those of a `case`, in the `case`; those
//!   of a `for`, in the statement; those of a `catch`, in the body; a class's
//!   declared members (the `InstanceScope`), in the methods and constructor
//!   bodies (a constructor's parameter list and the field initializers stop
//!   at the class's type parameter scope); outside everything, the unit's
//!   scope. Known deviation: the variables of collection-element `for` and
//!   `if case` and those of switch expression cases do not enter the regions.
//!
//! Written without compiling or running (2026-10-05).

use super::codes_g as c;
use super::rules::LintReport;
use super::LintCode;
use crate::Unit;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassKind, Element};
use dartforge_frontend::ast::{
    self, Annotation, Ast, DeclKind, DirectiveKind, ExprId, ExprKind, ForInit, MemberKind, Parameter, PatternKind, StmtId, StmtKind, TypeKind,
    TypedefKind, UnaryOp,
};
use dartforge_frontend::comments::Comments;
use dartforge_frontend::source::skip_whitespace;
use dartforge_intern::{Interner, SymbolId};

/// `LineInfo.fromContent`: the line starts (after `\n`, `\r\n` and a lone
/// `\r`).
fn line_starts(source: &str) -> Vec<usize> {
    let b = source.as_bytes();
    let mut v = vec![0usize];
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\r' => {
                if b.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
                v.push(i + 1);
            }
            b'\n' => v.push(i + 1),
            _ => {}
        }
        i += 1;
    }
    v
}

fn line_of(starts: &[usize], pos: usize) -> usize {
    starts.partition_point(|&x| x <= pos)
}

/// `DocumentationCommentToken`: `///…` or `/**…`.
fn is_doc(source: &str, s: Span) -> bool {
    let t = &source[s.start..s.end];
    t.starts_with("///") || t.starts_with("/**")
}

/// A documentation `Comment` attached to a node.
pub(super) struct Doc {
    /// The `Comment`'s tokens.
    pub(super) tokens: Vec<Span>,
    /// The comment chain from the initial token on.
    pub(super) chain: Vec<Span>,
    /// The token before which the chain is (the comments' `parent`).
    pub(super) anchor: usize,
}

/// A code block of a documentation comment (`MdCodeBlock`).
pub(super) struct CodeBlock {
    pub(super) fenced: bool,
    pub(super) has_info: bool,
    /// The lines: (position, length).
    pub(super) lines: Vec<(usize, usize)>,
}

/// The lines of a comment's `_CharacterSequence`: (position, content).
pub(super) fn doc_lines<'f>(source: &'f str, doc: &Doc) -> Vec<(usize, &'f str)> {
    let first = doc.tokens[0];
    let lexeme = &source[first.start..first.end];
    if lexeme.starts_with("///") {
        return doc.tokens.iter().map(|s| (s.start + 3, &source[s.start + 3..s.end])).collect();
    }
    // `_CharacterSequenceFromMultiLineComment`.
    let b = lexeme.as_bytes();
    let is_blank = |c: u8| matches!(c, b' ' | b'\n' | b'\r' | b'\t');
    let mut v = Vec::new();
    let end0 = lexeme.find('\n').unwrap_or(lexeme.len());
    v.push((first.start, &lexeme[..end0]));
    let mut end = end0;
    loop {
        let mut o = end + 1;
        if o >= b.len() {
            break;
        }
        while is_blank(b[o]) {
            o += 1;
            if o >= b.len() {
                return v;
            }
        }
        let line_end = lexeme[o..].find('\n').map_or(lexeme.len(), |k| o + k);
        end = line_end;
        if lexeme[o..].starts_with("* ") {
            o += 2;
        } else if end == o + 1 && b[o] == b'*' {
            o += 1;
        }
        v.push((first.start + o, &lexeme[o.min(line_end)..line_end]));
    }
    v
}

/// The code blocks of `_parseDocComment` (indented and fenced).
pub(super) fn code_blocks(lines: &[(usize, &str)]) -> Vec<CodeBlock> {
    let mut blocks = Vec::new();
    let mut previous_blank = true;
    let mut i = 0;
    while i < lines.len() {
        let (pos, content) = lines[i];
        if previous_blank && leading_whitespace(content) >= 4 {
            let mut b = CodeBlock { fenced: false, has_info: false, lines: vec![(pos, content.len())] };
            i += 1;
            while i < lines.len() && leading_whitespace(lines[i].1) >= 4 {
                b.lines.push((lines[i].0, lines[i].1.len()));
                i += 1;
            }
            blocks.push(b);
            if i < lines.len() {
                previous_blank = lines[i].1.is_empty();
            }
            continue;
        }
        if let Some(mut k) = fence_delimiter(content, 3) {
            let bs = content.as_bytes();
            let mut backticks = 0;
            while k < bs.len() && bs[k] == b'`' {
                backticks += 1;
                k += 1;
            }
            let has_info = k < bs.len() && !content[k..].trim().is_empty();
            let mut b = CodeBlock { fenced: true, has_info, lines: vec![(pos, content.len())] };
            i += 1;
            while i < lines.len() {
                b.lines.push((lines[i].0, lines[i].1.len()));
                if fence_delimiter(lines[i].1, backticks).is_some() {
                    break;
                }
                i += 1;
            }
            blocks.push(b);
            previous_blank = false;
        } else {
            previous_blank = content.is_empty();
        }
        i += 1;
    }
    blocks
}

/// `_findComment(metadata, tokenAfterMetadata)`.
pub(super) fn doc_for(source: &str, comments: &Comments, metadata: &[Annotation], after: usize) -> Option<Doc> {
    let build = |anchor: usize| -> Option<Doc> {
        let start = comments.dart_doc(source, anchor)?;
        let chain: Vec<Span> = comments.before(source, anchor).into_iter().filter(|s| s.start >= start.start).collect();
        let mut tokens = vec![start];
        if source[start.start..start.end].starts_with("///") {
            tokens.extend(chain.iter().skip(1).filter(|s| source[s.start..s.end].starts_with("///")).copied());
        }
        Some(Doc { tokens, chain, anchor })
    };
    build(after).or_else(|| metadata.iter().rev().find_map(|m| build(m.span.start)))
}

/// The token after the annotations.
pub(super) fn after_annotations(source: &str, metadata: &[Annotation], start: usize) -> usize {
    match metadata.last() {
        Some(m) => skip_whitespace(source.as_bytes(), m.span.end),
        None => start,
    }
}

/// Skips the word `augment` (`_findComment` for `enum`, `extension`,
/// `extension type`, `typedef` and `mixin` starts from the keyword).
pub(super) fn skip_augment(source: &str, pos: usize) -> usize {
    if source[pos..].starts_with("augment") && !source.as_bytes().get(pos + 7).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'$') {
        return skip_whitespace(source.as_bytes(), pos + 7);
    }
    pos
}

/// The token that `endFormalParameter` uses: the `this`, the start of the
/// type, or the name.
fn parameter_token(source: &str, a: &Ast, p: &Parameter) -> Option<usize> {
    if p.this_
        && let Some(n) = p.name
    {
        return source[..n.span.start].rfind("this");
    }
    if let Some(t) = p.ty {
        return Some(a.ty(t).span.start);
    }
    p.name.map(|n| n.span.start)
}

/// All the unit's parameter lists (the nested ones too).
fn parameter_lists(a: &Ast) -> Vec<&[Parameter]> {
    fn with_nested<'x>(ps: &'x [Parameter], v: &mut Vec<&'x [Parameter]>) {
        v.push(ps);
        for p in ps {
            if let Some(inner) = &p.function_parameters {
                with_nested(inner, v);
            }
        }
    }
    let mut v: Vec<&[Parameter]> = Vec::new();
    for f in a.functions.iter() {
        if let Some(ps) = &f.parameters {
            with_nested(ps, &mut v);
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind {
            with_nested(&k.parameters, &mut v);
        }
    }
    for t in a.types.iter() {
        if let TypeKind::Function { parameters, .. } = &t.kind {
            with_nested(parameters, &mut v);
        }
    }
    for d in a.decls.iter() {
        if let DeclKind::Typedef(x) = &d.kind
            && let TypedefKind::Legacy { parameters, .. } = &x.kind
        {
            with_nested(parameters, &mut v);
        }
    }
    v
}

/// All the unit's type parameters.
fn type_parameters(a: &Ast) -> Vec<&ast::TypeParameter> {
    let mut v: Vec<&ast::TypeParameter> = Vec::new();
    for d in a.decls.iter() {
        match &d.kind {
            DeclKind::Class(x) => v.extend(x.type_params.iter()),
            DeclKind::Mixin(x) => v.extend(x.type_params.iter()),
            DeclKind::Enum(x) => v.extend(x.type_params.iter()),
            DeclKind::Extension(x) => v.extend(x.type_params.iter()),
            DeclKind::ExtensionType(x) => v.extend(x.type_params.iter()),
            DeclKind::Typedef(x) => v.extend(x.type_params.iter()),
            _ => {}
        }
    }
    for f in a.functions.iter() {
        v.extend(f.type_params.iter());
    }
    for t in a.types.iter() {
        if let TypeKind::Function { type_params, .. } = &t.kind {
            v.extend(type_params.iter());
        }
    }
    for ps in parameter_lists(a) {
        for p in ps {
            v.extend(p.function_type_params.iter());
        }
    }
    v
}

/// The documentation comments of all the nodes the `AstBuilder` attaches
/// them to, without repetition.
pub(super) fn unit_docs(u: Unit<'_>, comments: &Comments) -> Vec<Doc> {
    let a = u.ast;
    let source = u.source;
    let mut docs: Vec<Doc> = Vec::new();
    let add = |d: Option<Doc>, docs: &mut Vec<Doc>| {
        if let Some(d) = d
            && !docs.iter().any(|x| x.tokens[0] == d.tokens[0])
        {
            docs.push(d);
        }
    };
    for d in &u.unit.directives {
        let pos = after_annotations(source, &d.metadata, d.span.start);
        add(doc_for(source, comments, &d.metadata, pos), &mut docs);
    }
    for d in a.decls.iter() {
        let mut pos = after_annotations(source, &d.metadata, d.span.start);
        if d.augment && matches!(d.kind, DeclKind::Enum(_) | DeclKind::Extension(_) | DeclKind::ExtensionType(_) | DeclKind::Typedef(_) | DeclKind::Mixin(_)) {
            pos = skip_augment(source, pos);
        }
        add(doc_for(source, comments, &d.metadata, pos), &mut docs);
        if let DeclKind::Enum(x) = &d.kind {
            for k in x.constants.iter() {
                add(doc_for(source, comments, &k.metadata, k.name.span.start), &mut docs);
            }
        }
    }
    for m in a.members.iter() {
        let pos = after_annotations(source, &m.metadata, m.span.start);
        add(doc_for(source, comments, &m.metadata, pos), &mut docs);
    }
    for tp in type_parameters(a) {
        add(doc_for(source, comments, &tp.metadata, tp.name.span.start), &mut docs);
    }
    for ps in parameter_lists(a) {
        for p in ps {
            if let Some(pos) = parameter_token(source, a, p) {
                add(doc_for(source, comments, &p.metadata, pos), &mut docs);
            }
        }
    }
    let metadata_of = |s: StmtId| -> &[Annotation] { a.local_metadata.iter().find(|(x, _)| *x == s).map_or(&[], |(_, m)| &m[..]) };
    for (k, s) in a.stmts.iter().enumerate() {
        let id = StmtId(k as u32);
        match &s.kind {
            StmtKind::Variables(l) => {
                if let Some(v) = l.variables.first() {
                    add(doc_for(source, comments, metadata_of(id), v.name.span.start), &mut docs);
                }
            }
            StmtKind::For { init: Some(ForInit::Variables(l)), .. } => {
                if let Some(v) = l.variables.first() {
                    add(doc_for(source, comments, &[], v.name.span.start), &mut docs);
                }
            }
            StmtKind::PatternVariables { .. } => {
                let meta = metadata_of(id);
                let pos = after_annotations(source, meta, s.span.start);
                add(doc_for(source, comments, meta, pos), &mut docs);
            }
            _ => {}
        }
    }
    docs
}

/// The HTML tags that `rule.reportLintForOffset` accepts (`_validHtmlTags`),
/// in the order of the alternation.
const HTML_TAGS: [&str; 98] = [
    "a", "abbr", "address", "area", "article", "aside", "audio", "b", "bdi", "bdo", "blockquote", "br", "button", "canvas", "caption",
    "cite", "code", "col", "colgroup", "data", "datalist", "dd", "del", "dfn", "div", "dl", "dt", "em", "fieldset", "figcaption",
    "figure", "footer", "form", "h1", "h2", "h3", "h4", "h5", "h6", "header", "hr", "i", "iframe", "img", "input", "ins", "kbd", "keygen",
    "label", "legend", "li", "link", "main", "map", "mark", "meta", "meter", "nav", "noscript", "object", "ol", "optgroup", "option",
    "output", "p", "param", "pre", "progress", "q", "s", "samp", "script", "section", "select", "small", "source", "span", "strong",
    "style", "sub", "sup", "table", "tbody", "td", "template", "textarea", "tfoot", "th", "thead", "time", "title", "tr", "track", "u",
    "ul", "var", "video", "wbr",
];

fn eq_ignore_case(b: &[u8], i: usize, s: &[u8]) -> bool {
    b.len() >= i + s.len() && b[i..i + s.len()].eq_ignore_ascii_case(s)
}

fn find(b: &[u8], from: usize, s: &[u8]) -> Option<usize> {
    if from > b.len() {
        return None;
    }
    b[from..].windows(s.len()).position(|w| w == s).map(|k| from + k)
}

/// `_markdownTokenPattern` matched at `i` (the alternation in order):
/// `Some((end, nh))`, with `nh` when the alternative of the unintended tags
/// matched.
fn match_markdown(t: &str, i: usize) -> Option<(usize, bool)> {
    let b = t.as_bytes();
    let n = b.len();
    match b[i] {
        // `\\.`: `.` without `dotAll` does not match line terminators.
        b'\\' => {
            let c = t[i + 1..].chars().next()?;
            if matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
                return None;
            }
            Some((i + 1 + c.len_utf8(), false))
        }
        // `(?<cq>`+)(?:[^]+?\k<cq>)?`.
        b'`' => {
            let mut k = i;
            while k < n && b[k] == b'`' {
                k += 1;
            }
            let q = k - i;
            let mut j = k + 1;
            while j + q <= n {
                if b[j..j + q].iter().all(|&x| x == b'`') {
                    return Some((j + q, false));
                }
                j += 1;
            }
            Some((k, false))
        }
        // `(?<!\])\[[^\]]*\](?![(\[])`.
        b'[' => {
            if i > 0 && b[i - 1] == b']' {
                return None;
            }
            let close = find(b, i + 1, b"]")?;
            if matches!(b.get(close + 1), Some(b'(' | b'[')) {
                return None;
            }
            Some((close + 1, false))
        }
        b'<' => {
            let is_letter = |k: usize| b.get(k).is_some_and(u8::is_ascii_alphabetic);
            // Autolink: `<[a-z][a-z\d\-+.]+:[^\x00-\x20\x7f<>]*>`.
            if is_letter(i + 1) {
                let mut j = i + 2;
                while j < n && (b[j].is_ascii_alphanumeric() || matches!(b[j], b'-' | b'+' | b'.')) {
                    j += 1;
                }
                if j > i + 2 && b.get(j) == Some(&b':') {
                    let mut k = j + 1;
                    while k < n && !(b[k] <= 0x20 || b[k] == 0x7F || b[k] == b'<' || b[k] == b'>') {
                        k += 1;
                    }
                    if b.get(k) == Some(&b'>') {
                        return Some((k + 1, false));
                    }
                }
            }
            // HTML comment: `<!--(?:-?>|[^]*?-->)`.
            if b[i..].starts_with(b"<!--") {
                let j = i + 4;
                if b[j..].starts_with(b"->") {
                    return Some((j + 2, false));
                }
                if b.get(j) == Some(&b'>') {
                    return Some((j + 1, false));
                }
                if let Some(k) = find(b, j, b"-->") {
                    return Some((k + 3, false));
                }
            }
            // Declaration: `<![a-z][^]*?!>`.
            if b[i..].starts_with(b"<!")
                && is_letter(i + 2)
                && let Some(k) = find(b, i + 3, b"!>")
            {
                return Some((k + 2, false));
            }
            // Processing instruction: `<\?[^]*?\?>`.
            if b[i..].starts_with(b"<?")
                && let Some(k) = find(b, i + 2, b"?>")
            {
                return Some((k + 2, false));
            }
            // CDATA: `<\[CDATA[^]*\]>` (greedy: the last `]>`).
            if b[i..].starts_with(b"<[") && eq_ignore_case(b, i + 2, b"cdata") {
                let from = i + 7;
                if from <= n
                    && let Some(k) = b[from..].windows(2).rposition(|w| w == b"]>")
                {
                    return Some((from + k + 2, false));
                }
            }
            // Valid tag: `<(?<et>/?)(?:tags)(?:/(?=\k<et>)>|>|[\x20\r\n\t][^]*?>)`.
            let closing_tag = b.get(i + 1) == Some(&b'/');
            let p = if closing_tag { i + 2 } else { i + 1 };
            for tag in HTML_TAGS.iter() {
                if !eq_ignore_case(b, p, tag.as_bytes()) {
                    continue;
                }
                let q = p + tag.len();
                if !closing_tag && b.get(q) == Some(&b'/') && b.get(q + 1) == Some(&b'>') {
                    return Some((q + 2, false));
                }
                if b.get(q) == Some(&b'>') {
                    return Some((q + 1, false));
                }
                if matches!(b.get(q), Some(b' ' | b'\r' | b'\n' | b'\t'))
                    && let Some(k) = find(b, q + 1, b">")
                {
                    return Some((k + 1, false));
                }
            }
            // The unintended ones: `</?[a-z][^]*?>`.
            let q = if b.get(i + 1) == Some(&b'/') && is_letter(i + 2) {
                i + 3
            } else if is_letter(i + 1) {
                i + 2
            } else {
                return None;
            };
            let k = find(b, q, b">")?;
            Some((k + 1, true))
        }
        _ => None,
    }
}

/// `_findUnintendedHtmlTags`: `(start, length)` relative to the text.
fn unintended_tags(t: &str) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    let mut i = 0;
    while i < t.len() {
        match match_markdown(t, i) {
            Some((end, nh)) => {
                if nh {
                    v.push((i, end - i));
                }
                i = end.max(i + 1);
            }
            None => i += 1,
        }
    }
    v
}

/// `_readWhitespace` with the parser's `isWhitespace` (space, `\n`, `\r`,
/// `\t`).
fn leading_whitespace(s: &str) -> usize {
    s.bytes().take_while(|c| matches!(c, b' ' | b'\n' | b'\r' | b'\t')).count()
}

/// `_fencedCodeBlockDelimiter`: the index of the backticks, or nothing.
fn fence_delimiter(s: &str, min_len: usize) -> Option<usize> {
    if s.is_empty() {
        return None;
    }
    let i = leading_whitespace(s);
    if i + 3 > s.len() {
        return None;
    }
    if s.as_bytes()[i..i + 3] == *"`".repeat(min_len).as_bytes() { Some(i) } else { None }
}

/// The lines of a run of `///` (the contents after the slashes) that are in
/// a code block (`_parseDocComment`).
fn lines_in_code_block(lines: &[&str]) -> Vec<bool> {
    let mut marked = vec![false; lines.len()];
    let mut previous_blank = true;
    let mut i = 0;
    while i < lines.len() {
        let content = lines[i];
        if previous_blank && leading_whitespace(content) >= 4 {
            // `_parseIndentedCodeBlock`.
            marked[i] = true;
            i += 1;
            while i < lines.len() && leading_whitespace(lines[i]) >= 4 {
                marked[i] = true;
                i += 1;
            }
            if i < lines.len() {
                previous_blank = lines[i].is_empty();
            }
            continue;
        }
        if let Some(mut k) = fence_delimiter(content, 3) {
            // `_parseFencedCodeBlock`.
            let b = content.as_bytes();
            let mut backticks = 0;
            while k < b.len() && b[k] == b'`' {
                backticks += 1;
                k += 1;
            }
            marked[i] = true;
            i += 1;
            while i < lines.len() {
                marked[i] = true;
                if fence_delimiter(lines[i], backticks).is_some() {
                    break;
                }
                i += 1;
            }
            previous_blank = false;
        } else {
            previous_blank = content.is_empty();
        }
        i += 1;
    }
    marked
}

/// `isValidDartFileName`.
fn is_valid_file_name(name: &str) -> bool {
    let b = name.as_bytes();
    if b.len() < 6 || !name.ends_with(".dart") {
        return true;
    }
    let len = b.len() - 5;
    if (1..len.saturating_sub(1)).any(|i| b[i] == b'.') {
        return true;
    }
    for (i, &c) in b[..len].iter().enumerate() {
        if !c.is_ascii_lowercase() && c != b'_' {
            if c.is_ascii_digit() {
                if i == 0 {
                    return false;
                }
                continue;
            }
            return false;
        }
    }
    true
}

/// `isJustUnderscores`.
fn only_underscores(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| c == b'_')
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unit<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let a = u.ast;
    let source = u.source;
    let mut report = |code: &'static LintCode, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    let needs_comments = enabled("dangling_library_doc_comments") || enabled("unintended_html_in_doc_comment");
    let comments = if needs_comments { Some(Comments::scan(source)) } else { None };

    // `dangling_library_doc_comments`.
    if enabled("dangling_library_doc_comments")
        && let Some(comments) = &comments
    {
        let starts = line_starts(source);
        let l = |pos: usize| line_of(&starts, pos);
        'rule: {
            if let Some(first_item) = u.unit.directives.first() {
                if matches!(first_item.kind, DirectiveKind::Library { .. } | DirectiveKind::PartOf { .. }) {
                    break 'rule;
                }
                let pos = after_annotations(source, &first_item.metadata, first_item.span.start);
                if let Some(doc) = doc_for(source, comments, &first_item.metadata, pos) {
                    report(&c::DANGLING_LIBRARY_DOC_COMMENTS, doc.tokens[0], &[]);
                }
                break 'rule;
            }
            let Some(&first_item) = u.unit.declarations.first() else {
                // No declarations: the documentation comments before the end
                // of the file.
                for s in comments.before(source, source.len()) {
                    if is_doc(source, s) {
                        report(&c::DANGLING_LIBRARY_DOC_COMMENTS, s, &[]);
                    }
                }
                break 'rule;
            };
            let d = a.decl(first_item);
            let mut pos = after_annotations(source, &d.metadata, d.span.start);
            if d.augment && matches!(d.kind, DeclKind::Enum(_) | DeclKind::Extension(_) | DeclKind::ExtensionType(_) | DeclKind::Typedef(_) | DeclKind::Mixin(_)) {
                pos = skip_augment(source, pos);
            }
            let Some(doc) = doc_for(source, comments, &d.metadata, pos) else { break 'rule };
            let node = Span { start: doc.tokens[0].start, end: doc.tokens[doc.tokens.len() - 1].end };
            for pair in doc.tokens.windows(2) {
                if l(pair[1].start) > l(pair[0].end) + 1 {
                    report(&c::DANGLING_LIBRARY_DOC_COMMENTS, pair[0], &[]);
                    break 'rule;
                }
            }
            // The comments after the last token of the `Comment`.
            let last = doc.tokens[doc.tokens.len() - 1];
            let mut current = last;
            for &next in doc.chain.iter().filter(|s| s.start > last.start) {
                if l(next.start) > l(current.end) + 1 {
                    report(&c::DANGLING_LIBRARY_DOC_COMMENTS, node, &[]);
                    break 'rule;
                }
                current = next;
            }
            if l(doc.anchor) > l(current.end) + 1 {
                report(&c::DANGLING_LIBRARY_DOC_COMMENTS, node, &[]);
            }
        }
    }

    // `unintended_html_in_doc_comment`.
    if enabled("unintended_html_in_doc_comment")
        && let Some(comments) = &comments
    {
        for doc in unit_docs(u, comments) {
            let first = doc.tokens[0];
            let is_line_doc = source[first.start..first.end].starts_with("///");
            // In a `/** … */` the first line (with the `/**`) is never part of
            // a code block; in a run of `///`, the lines are the tokens.
            let in_block = if is_line_doc {
                let lines: Vec<&str> = doc.tokens.iter().map(|s| &source[s.start + 3..s.end]).collect();
                lines_in_code_block(&lines)
            } else {
                vec![false]
            };
            for (k, s) in doc.tokens.iter().enumerate() {
                if in_block.get(k).copied().unwrap_or(false) {
                    continue;
                }
                for (start_rel, len_rel) in unintended_tags(&source[s.start..s.end]) {
                    report(&c::UNINTENDED_HTML_IN_DOC_COMMENT, Span { start: s.start + start_rel, end: s.start + start_rel + len_rel }, &[]);
                }
            }
        }
    }

    let Some(s) = sem else { return out };
    let program = s.program;

    // `file_names`: the name of the file that defines the library.
    if enabled("file_names") {
        let lib = program.library(program.unit(s.unit).library);
        if let Some(&defining) = lib.units.first() {
            let un = program.unit(defining);
            let name = match &un.path {
                Some(p) => p.file_name().map(|x| x.to_string_lossy().into_owned()),
                None => un.uri.rsplit('/').next().map(str::to_string),
            };
            if let Some(name) = name
                && !is_valid_file_name(&name)
            {
                report(&c::FILE_NAMES, Span { start: 0, end: 0 }, &[name.as_str()]);
            }
        }
    }

    // `depend_on_referenced_packages`: the package (the `pubspec.yaml`
    // nearest to the library's file), its name, its dependencies and, outside
    // `lib/`, `bin/` and the `hook/build.dart` and `hook/link.dart` hooks,
    // its dev dependencies.
    let imports_packages = u.unit.directives.iter().any(|d| match &d.kind {
        DirectiveKind::Import { uri, .. } | DirectiveKind::Export { uri, .. } => {
            uri.constant_value().is_some_and(|v| v.to_string_lossy().starts_with("package:"))
        }
        _ => false,
    });
    if enabled("depend_on_referenced_packages") && imports_packages {
        let lib = program.library(program.unit(s.unit).library);
        let lib_path = lib.units.first().and_then(|x| program.unit(*x).path.clone());
        let root = lib_path.as_ref().and_then(|c| c.ancestors().skip(1).find(|d| d.join("pubspec.yaml").is_file()).map(std::path::Path::to_path_buf));
        if let (Some(file), Some(root)) = (lib_path, root)
            && let Ok(text) = std::fs::read_to_string(root.join("pubspec.yaml"))
        {
            let inside = |d: &std::path::Path| file.starts_with(d) && file != d;
            let public = inside(&root.join("lib"))
                || inside(&root.join("bin"))
                || file == root.join("hook").join("build.dart")
                || file == root.join("hook").join("link.dart");
            if let Some(available) = super::pubspec::available_dependencies(&text, !public) {
                for d in &u.unit.directives {
                    let uri = match &d.kind {
                        DirectiveKind::Import { uri, .. } | DirectiveKind::Export { uri, .. } => uri,
                        _ => continue,
                    };
                    let Some(value) = uri.constant_value().map(|v| v.to_string_lossy()) else { continue };
                    if !value.starts_with("package:") {
                        continue;
                    }
                    let Some(slash) = value.find('/') else { continue };
                    let package = &value[8..slash.max(8)];
                    if package == "flutter_gen" || available.iter().any(|x| x == package) {
                        continue;
                    }
                    report(&c::DEPEND_ON_REFERENCED_PACKAGES, uri.span, &[package]);
                }
            }
        }
    }

    // `valid_regexps`.
    if enabled("valid_regexps") {
        for (k, e) in a.exprs.iter().enumerate() {
            let arguments = match &e.kind {
                ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => arguments,
                _ => continue,
            };
            let Some(dartforge_types::resolved::Resolved::Constructor(f)) = s.body.get_resolved(ExprId(k as u32)) else { continue };
            let Some(class_id) = program.function(*f).class else { continue };
            let cl = program.class(class_id);
            if interner.resolve(cl.name) != "RegExp" || program.library(cl.library).uri != "dart:core" {
                continue;
            }
            let Some(first) = arguments.args.first() else { continue };
            let unicode = arguments.args.iter().any(|x| {
                x.name.is_some_and(|n| interner.resolve(n.sym) == "unicode") && matches!(a.expr(x.value).kind, ExprKind::Bool(true))
            });
            if first.name.is_some() {
                continue;
            }
            let ExprKind::String(lit) = &a.expr(first.value).kind else { continue };
            let Some(value) = lit.constant_value() else { continue };
            let units: Vec<u16> = value.code_units().collect();
            if !super::regexp_vm::is_valid(&units, unicode) {
                report(&c::VALID_REGEXPS, a.expr(first.value).span, &[]);
            }
        }
    }

    // `library_annotations`.
    if enabled("library_annotations") {
        let first_item = u.unit.directives.first().map(|d| d.span);
        // `@pragma('dart2js:late:trust')`: the `pragma` constructor of
        // `dart:core` with the constant name.
        let pragma_late_trust = |m: &Annotation| -> bool {
            let simple = |n: ast::Name| program.lookup_in_unit(s.unit, n.sym).and_then(|b| b.getter);
            let element = match &m.name[..] {
                [n] => simple(*n),
                [p, n] => program.lookup_prefixed_in_unit(s.unit, p.sym, n.sym).and_then(|b| b.getter),
                _ => None,
            };
            let Some(Element::Class(cl)) = element else { return false };
            let class_id = program.class(cl);
            if interner.resolve(class_id.name) != "pragma" || program.library(class_id.library).uri != "dart:core" {
                return false;
            }
            let Some(arg) = m.arguments.as_ref().and_then(|x| x.args.iter().find(|y| y.name.is_none())) else { return false };
            constant_string_value(program, s.unit, a, arg.value).as_deref() == Some("dart2js:late:trust")
        };
        let mut check = |metadata: &[Annotation], is_first: bool| {
            for m in metadata {
                let kinds = crate::meta::annotation_target_kinds(program, interner, s.unit, m);
                if (kinds.len() == 1 && kinds[0] == "library" && is_first) || pragma_late_trust(m) {
                    report(&c::LIBRARY_ANNOTATIONS, m.span, &[]);
                }
            }
        };
        'rule: {
            for d in &u.unit.directives {
                if matches!(d.kind, DirectiveKind::PartOf { .. }) {
                    break 'rule;
                }
                if !matches!(d.kind, DirectiveKind::Library { .. }) {
                    check(&d.metadata, Some(d.span) == first_item);
                }
            }
            for &did in &u.unit.declarations {
                let d = a.decl(did);
                check(&d.metadata, false);
            }
        }
    }

    // `no_wildcard_variable_uses`.
    if enabled("no_wildcard_variable_uses") {
        let mut without_element: std::collections::HashSet<ExprId> = std::collections::HashSet::new();
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::Assign { target, .. } => {
                    without_element.insert(*target);
                }
                ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } => {
                    without_element.insert(*operand);
                }
                _ => {}
            }
        }
        // The local functions (the element is a `LocalFunctionElement`).
        let local_functions: std::collections::HashSet<usize> = a
            .stmts
            .iter()
            .filter_map(|x| match &x.kind {
                StmtKind::Function(f) => a.function(*f).name.map(|n| n.span.start),
                _ => None,
            })
            .collect();
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            let ExprKind::Identifier(n) = &e.kind else { continue };
            if without_element.contains(&id) || !only_underscores(interner.resolve(n.sym)) {
                continue;
            }
            let local_or_param = match s.body.get_resolved(id) {
                Some(dartforge_types::resolved::Resolved::Local(_)) => {
                    !s.body.local_declaration(id).is_some_and(|d| local_functions.contains(&d))
                }
                Some(dartforge_types::resolved::Resolved::Parameter { .. }) => true,
                _ => false,
            };
            if local_or_param {
                report(&c::NO_WILDCARD_VARIABLE_USES, e.span, &[]);
            }
        }
    }

    // `type_literal_in_constant_pattern`.
    if enabled("type_literal_in_constant_pattern") {
        let type_class = match s.table.get(s.core.type_) {
            dartforge_types::table::Type::Interface { class, .. } => Some(*class),
            _ => None,
        };
        for p in a.patterns.iter() {
            let PatternKind::Constant(e) = &p.kind else { continue };
            let text = &source[p.span.start..p.span.end];
            if text.starts_with("const") && !text.as_bytes().get(5).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'$') {
                continue;
            }
            let Some(t) = s.body.get_type(*e) else { continue };
            if let dartforge_types::table::Type::Interface { class, .. } = s.table.get(t)
                && Some(*class) == type_class
            {
                report(&c::TYPE_LITERAL_IN_CONSTANT_PATTERN, p.span, &[]);
            }
        }
    }

    // `avoid_types_as_parameter_names`.
    if enabled("avoid_types_as_parameter_names") {
        let scopes = super::scope::Scopes::new(a);
        let is_type = |name: SymbolId, pos: usize| -> bool {
            let found = scopes.lookup(name, pos);
            if !found.is_empty() {
                // The scope's getter: a type parameter is a type; the rest
                // (local, member, or setter only) is not.
                return found.iter().any(|r| r.kind == super::scope::EntryKind::TypeParameter);
            }
            let Some(b) = program.lookup_in_unit(s.unit, name) else { return false };
            if b.ambiguous {
                return false;
            }
            match b.getter {
                Some(Element::Class(cl)) => matches!(program.class(cl).kind, ClassKind::Class | ClassKind::MixinApplication | ClassKind::ExtensionType),
                Some(Element::Typedef(_)) => true,
                _ => false,
            }
        };
        for st in a.stmts.iter() {
            let StmtKind::Try { catches, .. } = &st.kind else { continue };
            for k in catches.iter() {
                if let Some(n) = k.exception
                    && is_type(n.sym, k.span.start)
                {
                    report(&c::AVOID_TYPES_AS_PARAMETER_NAMES, n.span, &[interner.resolve(n.sym)]);
                }
            }
        }
        for ps in parameter_lists(a) {
            for p in ps {
                let Some(n) = p.name else { continue };
                // `hasImplicitType`, except `this.x`.
                if p.this_ || p.ty.is_some() || p.function_parameters.is_some() {
                    continue;
                }
                if is_type(n.sym, p.span.start) {
                    report(&c::AVOID_TYPES_AS_PARAMETER_NAMES, n.span, &[interner.resolve(n.sym)]);
                }
            }
        }
    }

    out
}

/// The constant value of a string expression: the literal without
/// interpolation, or the identifier of a top-level constant with such an
/// initializer.
fn constant_string_value(
    program: &dartforge_elements::model::Program,
    u: dartforge_elements::model::UnitId,
    a: &Ast,
    e: ExprId,
) -> Option<String> {
    match &a.expr(e).kind {
        ExprKind::String(lit) => lit.constant_value().map(|v| v.to_string_lossy()),
        ExprKind::Parenthesized(x) => constant_string_value(program, u, a, *x),
        ExprKind::Identifier(n) => {
            let b = program.lookup_in_unit(u, n.sym)?;
            let v = match b.getter? {
                Element::Variable(v) => v,
                Element::Function(f) => program.function(f).variable?,
                _ => return None,
            };
            let dartforge_elements::model::VariableRef::TopLevel { unit, decl, index } = program.variable(v).node else { return None };
            let a2 = &program.unit(unit).ast;
            let DeclKind::Variables(l) = &a2.decl(decl).kind else { return None };
            if !l.const_ {
                return None;
            }
            let init = l.variables.get(index)?.initializer?;
            match &a2.expr(init).kind {
                ExprKind::String(lit) => lit.constant_value().map(|v| v.to_string_lossy()),
                _ => None,
            }
        }
        _ => None,
    }
}
