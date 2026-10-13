//! Record field names (docs/ANALYZER-ESPECIFICACAO.md, §C):
//! `RecordTypeAnnotationResolver` (`record_type_annotation_resolver.dart:40-102`)
//! on every written record type and `RecordLiteralResolver`
//! (`record_literal_resolver.dart:78-131`) on every literal:
//!
//! * `duplicate_field_name`: the repeated name (in the type, positional and
//!   named; in the literal, only the named ones), reported first;
//! * `invalid_field_name`: private (`_x`), `$n` that is the index of another
//!   positional field, or the name of an `Object` member.
//!
//! The AST does not keep the name of the positional fields of a record type:
//! it is read again from the source, after the field's type.
//!
//! Written without compiling or running (2026-10-05).

use crate::Unidade;
use dartforge_diagnostics::codes::compile_time_error as c;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_frontend::ast::{ExprKind, TypeKind};
use dartforge_frontend::features::Feature;

/// The identifier right after `end` (the name of a positional field).
fn name_after(source: &str, end: usize) -> Option<(String, Span)> {
    let b = source.as_bytes();
    let mut i = end;
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if source[i..].starts_with("//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if source[i..].starts_with("/*") {
            match source[i + 2..].find("*/") {
                Some(k) => i = i + 2 + k + 2,
                None => return None,
            }
            continue;
        }
        break;
    }
    let start = i;
    while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
        i += 1;
    }
    if i == start || b[start].is_ascii_digit() {
        return None;
    }
    Some((source[start..i].to_string(), Span { start, end: i }))
}

/// `positionalFieldIndex`: `$n` (n ≥ 1, no leading zero) gives `n - 1`.
fn positional_index(name: &str) -> Option<usize> {
    let r = name.strip_prefix('$')?;
    if r.is_empty() || r.starts_with('0') || !r.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    r.parse::<usize>().ok()?.checked_sub(1)
}

fn from_object(name: &str) -> bool {
    matches!(name, "hashCode" | "runtimeType" | "noSuchMethod" | "toString")
}

/// A named field: the name, the place and the positional index (`None` for
/// named ones).
struct Field {
    name: String,
    span: Span,
    positional: Option<usize>,
}

fn report(fields: &[Field], n_positional: usize, wildcard: bool, out: &mut Vec<Diagnostic>) {
    // `reportDuplicateFieldDefinitions`.
    let mut seen: Vec<(&str, Span)> = Vec::new();
    for f in fields {
        if wildcard && f.positional.is_some() && f.name == "_" {
            continue;
        }
        if let Some(&(_, first)) = seen.iter().find(|(n, _)| *n == f.name.as_str()) {
            // `duplicateFieldDefinitionIn{Literal,Type}`: the analyzer's text is
            // `'The first '`, on the first one's name, with the name's length.
            let context = Span { start: first.start, end: first.start + f.name.len() };
            out.push(Diagnostic::with_code(c::DUPLICATE_FIELD_NAME, f.span, [f.name.as_str()]).with_context(context, "The first "));
        } else {
            seen.push((&f.name, f.span));
        }
    }
    // `reportInvalidFieldNames`.
    for f in fields {
        if f.name.starts_with('_') {
            if !(wildcard && f.positional.is_some() && f.name == "_") {
                out.push(Diagnostic::with_code(c::INVALID_FIELD_NAME_PRIVATE, f.span, [] as [&str; 0]));
            }
        } else if let Some(i) = positional_index(&f.name)
            && i < n_positional
            && f.positional != Some(i)
        {
            out.push(Diagnostic::with_code(c::INVALID_FIELD_NAME_POSITIONAL, f.span, [] as [&str; 0]));
        } else if from_object(&f.name) {
            out.push(Diagnostic::with_code(c::INVALID_FIELD_NAME_FROM_OBJECT, f.span, [] as [&str; 0]));
        }
    }
}

pub fn check(u: &Unidade<'_>, names: &dartforge_intern::Interner, features: dartforge_frontend::features::LibraryFeatures) -> Vec<Diagnostic> {
    let a = u.ast;
    let wildcard = features.tem(Feature::WildcardVariables);
    let mut out = Vec::new();
    for t in &a.types {
        let TypeKind::Record { positional, named } = &t.kind else { continue };
        let mut fields = Vec::new();
        for (i, &p) in positional.iter().enumerate() {
            if let Some((name, span)) = name_after(u.fonte, a.ty(p).span.end) {
                fields.push(Field { name, span, positional: Some(i) });
            }
        }
        for (n, _) in named.iter() {
            fields.push(Field { name: names.resolve(n.sym).to_string(), span: n.span, positional: None });
        }
        report(&fields, positional.len(), wildcard, &mut out);
    }
    for e in &a.exprs {
        let ExprKind::Record { positional, named, .. } = &e.kind else { continue };
        let fields: Vec<Field> = named.iter().map(|(n, _)| Field { name: names.resolve(n.sym).to_string(), span: n.span, positional: None }).collect();
        report(&fields, positional.len(), false, &mut out);
    }
    out
}
