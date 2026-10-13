//! Warning verifiers that the analyzer runs per file and DartForge did not
//! have (docs/ANALYZER-ESPECIFICACAO-INFRA.md, Part II, batch II.8, and
//! Part III, step 6):
//!
//! * from the `ImportsVerifier` (`analyzer/lib/src/error/imports_verifier.dart`):
//!   `duplicate_import`, `duplicate_export`, `duplicate_shown_name`,
//!   `duplicate_hidden_name`; and, from `DeadCodeVerifier._checkCombinator`,
//!   `undefined_hidden_name`;
//! * the `UnicodeTextVerifier` (`unicode_text_verifier.dart`):
//!   `text_direction_code_point_in_comment` and `…_in_literal`.
//!
//! Written without compiling or running (2026-10-04).

use crate::Unidade;
use dartforge_diagnostics::codes::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, Combinator, DirectiveKind, ExprKind};
use dartforge_intern::{Interner, SymbolId};

/// An import or export directive for the duplicate comparison.
struct Directive<'a> {
    index: usize,
    library_uri: &'a str,
    prefix: Option<SymbolId>,
    combinators: &'a [Combinator],
}

/// `areSyntacticallyIdenticalExceptUri` (`ast.dart:10005-10055`): the same
/// prefix and the same combinators, in the same order, with the same names in
/// the same order. `deferred` and the configurations do not count.
fn identical(a: &Directive<'_>, b: &Directive<'_>) -> bool {
    if a.prefix != b.prefix || a.combinators.len() != b.combinators.len() {
        return false;
    }
    a.combinators.iter().zip(b.combinators.iter()).all(|(x, y)| match (x, y) {
        (Combinator::Show(p), Combinator::Show(q)) | (Combinator::Hide(p), Combinator::Hide(q)) => {
            p.len() == q.len() && p.iter().zip(q.iter()).all(|(m, n)| m.sym == n.sym)
        }
        _ => false,
    })
}

/// `_duplicates` (`imports_verifier.dart:368-397`): the list sorted by the
/// library's absolute URI; of each equal CONSECUTIVE pair, the directive with
/// the larger offset is the duplicate.
fn duplicates(mut list: Vec<Directive<'_>>) -> Vec<usize> {
    let mut out = Vec::new();
    if list.len() <= 1 {
        return out;
    }
    list.sort_by(|a, b| a.library_uri.cmp(b.library_uri).then(a.index.cmp(&b.index)));
    for pair in list.windows(2) {
        let (current, next) = (&pair[0], &pair[1]);
        if current.library_uri == next.library_uri && identical(current, next) {
            out.push(current.index.max(next.index));
        }
    }
    out
}

/// The warnings for repeated directives and for names repeated in the
/// combinators of library `lib`, unit by unit.
pub fn repeated_directives(program: &Program, lib: LibraryId, interner: &Interner) -> Vec<(UnitId, Diagnostic)> {
    let mut out = Vec::new();
    let library = program.library(lib);
    for &u in &library.units {
        let directives = &program.unit(u).unit.directives;
        let uri_of = |index: usize| -> Option<Span> {
            match &directives.get(index)?.kind {
                DirectiveKind::Import { uri, .. } | DirectiveKind::Export { uri, .. } => Some(uri.span),
                _ => None,
            }
        };
        let imports: Vec<Directive<'_>> = library
            .imports
            .iter()
            .filter(|i| i.unit == u)
            .map(|i| Directive {
                index: i.directive,
                library_uri: program.library(i.library).uri.as_str(),
                prefix: i.prefix,
                combinators: &i.combinators,
            })
            .collect();
        let exports: Vec<Directive<'_>> = library
            .exports
            .iter()
            .filter(|e| e.unit == u)
            .map(|e| Directive {
                index: e.directive,
                library_uri: program.library(e.library).uri.as_str(),
                prefix: None,
                combinators: &e.combinators,
            })
            .collect();
        // The official fixed order: exports, imports, hidden and shown
        // names.
        for index in duplicates(exports) {
            if let Some(sp) = uri_of(index) {
                out.push((u, Diagnostic::with_code(w::DUPLICATE_EXPORT, sp, std::iter::empty::<&str>())));
            }
        }
        for index in duplicates(imports) {
            if let Some(sp) = uri_of(index) {
                out.push((u, Diagnostic::with_code(w::DUPLICATE_IMPORT, sp, std::iter::empty::<&str>())));
            }
        }
        // Combinator names: repeated in the SAME combinator (only those that
        // resolve: what the target library exports), and the `hide` ones it
        // does not export.
        let targets = library
            .imports
            .iter()
            .filter(|i| i.unit == u)
            .map(|i| (i.library, &i.combinators))
            .chain(library.exports.iter().filter(|e| e.unit == u).map(|e| (e.library, &e.combinators)));
        for (target, combinators) in targets {
            let exported = &program.library(target).exported;
            let uri = program.library(target).uri.as_str();
            for k in combinators.iter() {
                let (names, show) = match k {
                    Combinator::Show(n) => (n, true),
                    Combinator::Hide(n) => (n, false),
                };
                let mut seen: Vec<SymbolId> = Vec::new();
                for n in names.iter() {
                    let resolves = exported.contains_key(&n.sym);
                    if !resolves {
                        if !show {
                            out.push((u, Diagnostic::with_code(w::UNDEFINED_HIDDEN_NAME, n.span, [uri, interner.resolve(n.sym)])));
                        }
                        continue;
                    }
                    if seen.contains(&n.sym) {
                        let code = if show { w::DUPLICATE_SHOWN_NAME } else { w::DUPLICATE_HIDDEN_NAME };
                        out.push((u, Diagnostic::with_code(code, n.span, [interner.resolve(n.sym)])));
                    } else {
                        seen.push(n.sym);
                    }
                }
            }
        }
    }
    out
}

/// `built_in_identifier_as_type`: a named type whose name is a built-in
/// identifier (`abstract`, `import`, `get`…), except `dynamic` and
/// `Function`, which are types. The analyzer's parser reports it while
/// reading the type (`TypeReferenceIdentifierContext`,
/// `identifier_context_impl.dart`), and the `NamedTypeResolver` on the name
/// after a prefix (`p.import`). Here the report comes from the tree: on the
/// name without a prefix, or on the name after the prefix. A built-in word
/// followed by `.` is a prefix, not a type. In a two-part name, only after an
/// import prefix (`A.factory` of `new A.factory()` is the class and the
/// constructor, whose name accepts built-ins). `augment` is only built-in
/// with `augmentations` enabled: without it the scanner
/// reads it as an ordinary identifier (`abstract_scanner.dart`).
pub fn built_in_as_type(u: Unidade<'_>, interner: &Interner, augmentations: bool) -> Vec<Diagnostic> {
    const BUILT_INS: [&str; 22] = [
        "abstract", "as", "augment", "covariant", "deferred", "export", "extension", "external", "factory", "get", "implements",
        "import", "interface", "late", "library", "mixin", "operator", "part", "required", "set", "static", "typedef",
    ];
    let prefixes: Vec<dartforge_intern::SymbolId> = u
        .unit
        .directives
        .iter()
        .filter_map(|d| match &d.kind {
            ast::DirectiveKind::Import { prefix: Some(p), .. } => Some(p.sym),
            _ => None,
        })
        .collect();
    let mut out = Vec::new();
    for t in u.ast.types.iter() {
        let ast::TypeKind::Named { name, .. } = &t.kind else { continue };
        let Some(last) = name.last() else { continue };
        if name.len() >= 2 && !prefixes.contains(&name[0].sym) {
            continue;
        }
        let text = interner.resolve(last.sym);
        if text == "augment" && !augmentations {
            continue;
        }
        if BUILT_INS.contains(&text) {
            out.push(Diagnostic::with_code(
                dartforge_diagnostics::codes::compile_time_error::BUILT_IN_IDENTIFIER_AS_TYPE,
                last.span,
                [text],
            ));
        }
    }
    out
}

/// `obsolete_colon_for_default_value`
/// (`BestPracticesVerifier.visitDefaultFormalParameter`,
/// `best_practices_verifier.dart:288-306`): the named parameter whose default
/// value comes after `:` instead of `=`; on the `:`. For a language version
/// before 3.0 the original gives the hint `deprecated_colon_for_default_value`,
/// which does not come out here.
///
/// `before_3_0`: the library's language version is before 3.0; the `:`
/// separator is then only `DEPRECATED_COLON_FOR_DEFAULT_VALUE` (a hint), and
/// not the error (`best_practices_verifier.dart:288-306` of analyzer 3.6.2).
pub fn colon_for_default_value(u: Unidade<'_>, before_3_0: bool) -> Vec<Diagnostic> {
    let code = if before_3_0 {
        dartforge_diagnostics::codes::hint::DEPRECATED_COLON_FOR_DEFAULT_VALUE
    } else {
        dartforge_diagnostics::codes::compile_time_error::OBSOLETE_COLON_FOR_DEFAULT_VALUE
    };
    fn visit(u: Unidade<'_>, list: &[ast::Parameter], code: dartforge_diagnostics::Code, out: &mut Vec<Diagnostic>) {
        for p in list {
            if let Some(inner) = &p.function_parameters {
                visit(u, inner, code, out);
            }
            let (ast::ParameterKind::Named, Some(default)) = (p.kind, p.default_value) else { continue };
            // The separator is the last `:` or `=` before the value, after the
            // start of the parameter.
            let until = u.ast.expr(default).span.start;
            let Some(before) = u.fonte.get(p.span.start..until) else { continue };
            let trimmed = before.trim_end();
            if trimmed.ends_with(':') {
                let pos = p.span.start + trimmed.len() - 1;
                out.push(Diagnostic::with_code(code, Span { start: pos, end: pos + 1 }, std::iter::empty::<&str>()));
            }
        }
    }
    let mut out = Vec::new();
    for f in u.ast.functions.iter() {
        if let Some(ps) = &f.parameters {
            visit(u, ps, code, &mut out);
        }
    }
    for m in u.ast.members.iter() {
        if let ast::MemberKind::Constructor(k) = &m.kind {
            visit(u, &k.parameters, code, &mut out);
        }
    }
    out
}

/// A text-direction control character: U+202A..U+202E or U+2066..U+2069.
fn is_direction_char(c: char) -> bool {
    matches!(c as u32, 0x202A..=0x202E | 0x2066..=0x2069)
}

/// The string-literal text ranges of a unit: the literal fragments of each
/// string (the interpolated expressions are left out) and the directives'
/// URIs.
fn literal_fragments(u: Unidade<'_>) -> Vec<Span> {
    let mut literals: Vec<Span> = Vec::new();
    let mut interpolated: Vec<Span> = Vec::new();
    for e in u.ast.exprs.iter() {
        if let ExprKind::String(lit) = &e.kind {
            literals.push(lit.span);
            for p in lit.parts.iter() {
                if let ast::StringPart::Interpolation(x) = p {
                    interpolated.push(u.ast.expr(*x).span);
                }
            }
        }
    }
    for d in u.unit.directives.iter() {
        match &d.kind {
            DirectiveKind::Import { uri, .. } | DirectiveKind::Export { uri, .. } | DirectiveKind::Part { uri } => literals.push(uri.span),
            _ => {}
        }
    }
    // An offset is a literal's if it is in some string and outside every
    // interpolated expression that does not itself contain another string there.
    literals.sort_by_key(|s| (s.start, s.end));
    interpolated.sort_by_key(|s| (s.start, s.end));
    let mut out = literals;
    out.extend(interpolated.into_iter().map(|s| Span { start: s.end, end: s.start }));
    out
}

/// `UnicodeTextVerifier.verify`: one report per direction character, of
/// length 1, with the code in upper-case hexadecimal; the code depends on
/// whether the character is in the text of a string literal or anywhere else
/// (comment, code).
pub fn bidi_text(u: Unidade<'_>) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if !u.fonte.chars().any(is_direction_char) {
        return out;
    }
    // The inverted ranges (`start > end`) mark the interpolated expressions;
    // see `literal_fragments`.
    let fragments = literal_fragments(u);
    for (offset, c) in u.fonte.char_indices() {
        if !is_direction_char(c) {
            continue;
        }
        // The innermost node: the string or the interpolation with the
        // smallest range covering the offset.
        let mut best: Option<(usize, bool)> = None;
        for t in &fragments {
            let (start, end, literal) = if t.start <= t.end { (t.start, t.end, true) } else { (t.end, t.start, false) };
            if start <= offset && offset < end {
                let size = end - start;
                if best.is_none_or(|(m, _)| size < m) {
                    best = Some((size, literal));
                }
            }
        }
        let in_literal = best.is_some_and(|(_, literal)| literal);
        let code = if in_literal { w::TEXT_DIRECTION_CODE_POINT_IN_LITERAL } else { w::TEXT_DIRECTION_CODE_POINT_IN_COMMENT };
        let hex = format!("{:04X}", c as u32);
        out.push(Diagnostic::with_code(code, Span { start: offset, end: offset + c.len_utf8() }, [hex.as_str()]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bidi(source: &str) -> Vec<&'static str> {
        let mut names = Interner::new();
        let p = dartforge_frontend::parser::parse(source, &mut names);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte: source };
        bidi_text(u).into_iter().map(|d| d.code.map_or("", |c| c.info().name)).collect()
    }

    #[test]
    fn direction_in_literal_and_in_comment() {
        assert_eq!(bidi("var s = 'a\u{202E}b';\n"), vec!["text_direction_code_point_in_literal"]);
        assert_eq!(bidi("// a\u{2066}b\nvar s = 1;\n"), vec!["text_direction_code_point_in_comment"]);
        assert!(bidi("var s = 'ab';\n").is_empty());
    }

    #[test]
    fn built_in_word_as_type() {
        let source = "void f(List<int> a, dynamic b, Function c) {}
";
        let mut names = Interner::new();
        let p = dartforge_frontend::parser::parse(source, &mut names);
        assert!(built_in_as_type(Unidade { ast: &p.ast, unit: &p.unit, fonte: source }, &names, false).is_empty());
    }

    #[test]
    fn colon_as_default_separator() {
        let source = "void f({int a: 1, int b = 2}) {}
";
        let mut names = Interner::new();
        let p = dartforge_frontend::parser::parse(source, &mut names);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte: source };
        let found = colon_for_default_value(u, false);
        assert_eq!(found.len(), 1);
        assert_eq!(&source[found[0].span.start..found[0].span.end], ":");
        assert_eq!(found[0].code.unwrap().info().name, "obsolete_colon_for_default_value");
        let old = colon_for_default_value(u, true);
        assert_eq!(old[0].code.unwrap().info().name, "deprecated_colon_for_default_value");
    }

    #[test]
    fn identical_consecutive_pair_is_duplicate() {
        let uri = "package:a/a.dart";
        let d = |index: usize| Directive { index, library_uri: uri, prefix: None, combinators: &[] };
        assert_eq!(duplicates(vec![d(0), d(2)]), vec![2]);
        assert!(duplicates(vec![d(0)]).is_empty());
    }
}
