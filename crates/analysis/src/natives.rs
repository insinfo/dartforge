//! `native` outside the SDK, like the analyzer's `ErrorVerifier`:
//! `visitNativeClause` (`NATIVE_CLAUSE_IN_NON_SDK_CODE`, on the
//! `native 'name'?` clause of the class header) and
//! `_checkForNativeFunctionBodyInNonSdkCode`
//! (`NATIVE_FUNCTION_BODY_IN_NON_SDK_CODE`, on the `native 'name'?;` body).
//! Only applies to libraries outside the SDK: the caller does not pass the
//! SDK's.
//!
//! The tree does not keep the position of the word `native`; it is found in
//! the text, between the end of the header (or of the parameters) and the `{`
//! (or the `;`).

use crate::Unidade;
use dartforge_diagnostics::codigos::parser as p;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_frontend::ast::{DeclKind, FunctionBody};

/// Skips whitespace and comments from `i` on.
fn skip_blanks(b: &[u8], mut i: usize) -> usize {
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b[i..].starts_with(b"/*") {
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
        } else {
            return i;
        }
    }
}

fn is_name_part(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

/// The end of the string literal (simple, without nested interpolation) at `i`.
fn string_end(b: &[u8], mut i: usize) -> Option<usize> {
    let raw = b.get(i) == Some(&b'r');
    if raw {
        i += 1;
    }
    let quote = *b.get(i).filter(|c| matches!(c, b'\'' | b'"'))?;
    let triple = b.get(i + 1) == Some(&quote) && b.get(i + 2) == Some(&quote);
    let closer: &[u8] = if triple { &b[i..i + 3] } else { &b[i..i + 1] };
    i += closer.len();
    while i < b.len() {
        if !raw && b[i] == b'\\' {
            i += 2;
            continue;
        }
        if b[i..].starts_with(closer) {
            return Some(i + closer.len());
        }
        i += 1;
    }
    None
}

/// `native 'name'?` from `i` on (after blanks): its range.
fn clause(b: &[u8], i: usize) -> Option<Span> {
    let i = skip_blanks(b, i);
    if !b[i..].starts_with(b"native") || b.get(i + 6).is_some_and(|&c| is_name_part(c)) {
        return None;
    }
    let after = skip_blanks(b, i + 6);
    let end = string_end(b, after).unwrap_or(i + 6);
    Some(Span { start: i, end })
}

/// The unit's `native` diagnostics (outside the SDK).
pub fn outside_sdk(unit: Unidade<'_>) -> Vec<Diagnostic> {
    let ast = unit.ast;
    let b = unit.fonte.as_bytes();
    let mut out = Vec::new();
    for &id in &unit.unit.declarations {
        let DeclKind::Class(c) = &ast.decl(id).kind else { continue };
        if c.mixin_application {
            continue;
        }
        let mut end = c.name.span.end;
        end = end.max(c.type_params.iter().map(|t| t.span.end).max().unwrap_or(0));
        for t in c.extends.iter().chain(c.with.iter()).chain(c.implements.iter()) {
            end = end.max(ast.ty(*t).span.end);
        }
        // The `>` that closes the type parameters.
        let mut i = skip_blanks(b, end.min(b.len()));
        if !c.type_params.is_empty() && end == c.type_params.iter().map(|t| t.span.end).max().unwrap_or(0) && b.get(i) == Some(&b'>') {
            i += 1;
        }
        if let Some(span) = clause(b, i) {
            out.push(Diagnostic::com_codigo(p::NATIVE_CLAUSE_IN_NON_SDK_CODE, span, std::iter::empty::<&str>()));
        }
    }
    for f in &ast.functions {
        let FunctionBody::Native(name) = &f.body else { continue };
        let limit = name.as_ref().map_or(f.span.end, |s| s.span.start).min(b.len());
        let Some(start) = unit.fonte[f.span.start.min(limit)..limit].rfind("native").map(|k| k + f.span.start.min(limit)) else {
            continue;
        };
        out.push(Diagnostic::com_codigo(
            p::NATIVE_FUNCTION_BODY_IN_NON_SDK_CODE,
            Span { start, end: f.span.end },
            std::iter::empty::<&str>(),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_frontend::parser::parse;
    use dartforge_intern::Interner;

    fn findings(source: &str) -> Vec<(&'static str, usize, usize)> {
        let mut names = Interner::new();
        let out = parse(source, &mut names);
        assert!(out.diagnostics.is_empty(), "{source}: {:?}", out.diagnostics);
        let u = Unidade { ast: &out.ast, unit: &out.unit, fonte: source };
        outside_sdk(u).into_iter().map(|d| (d.code.unwrap().info().nome, d.span.start, d.span.end)).collect()
    }

    /// Positions from `dart analyze` 3.6.2 (`syntax/syntax_native_test.dart`
    /// and `executable_body/ExecutableBody__*_nativeBody.dart` of the corpus).
    #[test]
    fn clause_and_body_like_the_oracle() {
        assert_eq!(
            findings("class A {}\nclass W<T> extends A native \"*W\" {}\nclass N native 'x' {}\n"),
            [("native_clause_in_non_sdk_code", 32, 43), ("native_clause_in_non_sdk_code", 55, 65)]
        );
        assert_eq!(
            findings("int f() native 'f';\nclass C { void m() native; }\n"),
            [("native_function_body_in_non_sdk_code", 8, 19), ("native_function_body_in_non_sdk_code", 39, 46)]
        );
    }
}
