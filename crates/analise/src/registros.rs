//! Os nomes dos campos de registro (docs/ANALYZER-ESPECIFICACAO.md, §C):
//! `RecordTypeAnnotationResolver` (`record_type_annotation_resolver.dart:40-102`)
//! em todo tipo de registro escrito e `RecordLiteralResolver`
//! (`record_literal_resolver.dart:78-131`) em todo literal:
//!
//! * `duplicate_field_name`: o nome repetido (no tipo, posicionais e
//!   nomeados; no literal, só os nomeados), relatado primeiro;
//! * `invalid_field_name`: privado (`_x`), `$n` que é o índice de outro
//!   campo posicional, ou nome de membro de `Object`.
//!
//! O AST não guarda o nome dos campos posicionais de um tipo de registro: ele
//! é relido da fonte, depois do tipo do campo.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::Unidade;
use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_frontend::ast::{ExprKind, TypeKind};
use dartforge_frontend::features::Feature;

/// O identificador logo depois de `fim` (o nome de um campo posicional).
fn nome_depois(fonte: &str, fim: usize) -> Option<(String, Span)> {
    let b = fonte.as_bytes();
    let mut i = fim;
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if fonte[i..].starts_with("//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if fonte[i..].starts_with("/*") {
            match fonte[i + 2..].find("*/") {
                Some(k) => i = i + 2 + k + 2,
                None => return None,
            }
            continue;
        }
        break;
    }
    let ini = i;
    while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
        i += 1;
    }
    if i == ini || b[ini].is_ascii_digit() {
        return None;
    }
    Some((fonte[ini..i].to_string(), Span { start: ini, end: i }))
}

/// `positionalFieldIndex`: `$n` (n ≥ 1, sem zero à esquerda) dá `n - 1`.
fn indice_posicional(nome: &str) -> Option<usize> {
    let r = nome.strip_prefix('$')?;
    if r.is_empty() || r.starts_with('0') || !r.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    r.parse::<usize>().ok()?.checked_sub(1)
}

fn de_object(nome: &str) -> bool {
    matches!(nome, "hashCode" | "runtimeType" | "noSuchMethod" | "toString")
}

/// Um campo com nome: o nome, o lugar e o índice posicional (`None` nos
/// nomeados).
struct Campo {
    nome: String,
    span: Span,
    posicional: Option<usize>,
}

fn relatar(campos: &[Campo], n_posicionais: usize, curinga: bool, saida: &mut Vec<Diagnostic>) {
    // `reportDuplicateFieldDefinitions`.
    let mut vistos: Vec<(&str, Span)> = Vec::new();
    for f in campos {
        if curinga && f.posicional.is_some() && f.nome == "_" {
            continue;
        }
        if let Some(&(_, primeiro)) = vistos.iter().find(|(n, _)| *n == f.nome.as_str()) {
            // `duplicateFieldDefinitionIn{Literal,Type}`: o texto do analyzer é
            // `'The first '`, no nome do primeiro, com o comprimento do nome.
            let contexto = Span { start: primeiro.start, end: primeiro.start + f.nome.len() };
            saida.push(Diagnostic::com_codigo(c::DUPLICATE_FIELD_NAME, f.span, [f.nome.as_str()]).com_contexto(contexto, "The first "));
        } else {
            vistos.push((&f.nome, f.span));
        }
    }
    // `reportInvalidFieldNames`.
    for f in campos {
        if f.nome.starts_with('_') {
            if !(curinga && f.posicional.is_some() && f.nome == "_") {
                saida.push(Diagnostic::com_codigo(c::INVALID_FIELD_NAME_PRIVATE, f.span, [] as [&str; 0]));
            }
        } else if let Some(i) = indice_posicional(&f.nome)
            && i < n_posicionais
            && f.posicional != Some(i)
        {
            saida.push(Diagnostic::com_codigo(c::INVALID_FIELD_NAME_POSITIONAL, f.span, [] as [&str; 0]));
        } else if de_object(&f.nome) {
            saida.push(Diagnostic::com_codigo(c::INVALID_FIELD_NAME_FROM_OBJECT, f.span, [] as [&str; 0]));
        }
    }
}

pub fn verificar(u: &Unidade<'_>, nomes: &dartforge_intern::Interner, recursos: dartforge_frontend::features::LibraryFeatures) -> Vec<Diagnostic> {
    let a = u.ast;
    let curinga = recursos.tem(Feature::WildcardVariables);
    let mut saida = Vec::new();
    for t in &a.types {
        let TypeKind::Record { positional, named } = &t.kind else { continue };
        let mut campos = Vec::new();
        for (i, &p) in positional.iter().enumerate() {
            if let Some((nome, span)) = nome_depois(u.fonte, a.ty(p).span.end) {
                campos.push(Campo { nome, span, posicional: Some(i) });
            }
        }
        for (n, _) in named.iter() {
            campos.push(Campo { nome: nomes.resolve(n.sym).to_string(), span: n.span, posicional: None });
        }
        relatar(&campos, positional.len(), curinga, &mut saida);
    }
    for e in &a.exprs {
        let ExprKind::Record { positional, named, .. } = &e.kind else { continue };
        let campos: Vec<Campo> = named.iter().map(|(n, _)| Campo { nome: nomes.resolve(n.sym).to_string(), span: n.span, posicional: None }).collect();
        relatar(&campos, positional.len(), false, &mut saida);
    }
    saida
}
