//! `native` fora do SDK, como o `ErrorVerifier` do analyzer:
//! `visitNativeClause` (`NATIVE_CLAUSE_IN_NON_SDK_CODE`, na cláusula
//! `native 'nome'?` do cabeçalho de classe) e
//! `_checkForNativeFunctionBodyInNonSdkCode`
//! (`NATIVE_FUNCTION_BODY_IN_NON_SDK_CODE`, no corpo `native 'nome'?;`).
//! Só vale para bibliotecas fora do SDK: quem chama não passa as do SDK.
//!
//! A árvore não guarda a posição da palavra `native`; ela é achada no texto,
//! entre o fim do cabeçalho (ou dos parâmetros) e o `{` (ou o `;`).

use crate::Unidade;
use dartforge_diagnostics::codigos::parser as p;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_frontend::ast::{DeclKind, FunctionBody};

/// Pula espaço e comentários a partir de `i`.
fn pular_brancos(b: &[u8], mut i: usize) -> usize {
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b[i..].starts_with(b"/*") {
            let mut prof = 0usize;
            while i < b.len() {
                if b[i..].starts_with(b"/*") {
                    prof += 1;
                    i += 2;
                } else if b[i..].starts_with(b"*/") {
                    prof -= 1;
                    i += 2;
                    if prof == 0 {
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

fn parte_de_nome(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

/// Fim do literal de string (simples, sem interpolação aninhada) em `i`.
fn fim_da_string(b: &[u8], mut i: usize) -> Option<usize> {
    let cru = b.get(i) == Some(&b'r');
    if cru {
        i += 1;
    }
    let aspa = *b.get(i).filter(|c| matches!(c, b'\'' | b'"'))?;
    let tripla = b.get(i + 1) == Some(&aspa) && b.get(i + 2) == Some(&aspa);
    let fecho: &[u8] = if tripla { &b[i..i + 3] } else { &b[i..i + 1] };
    i += fecho.len();
    while i < b.len() {
        if !cru && b[i] == b'\\' {
            i += 2;
            continue;
        }
        if b[i..].starts_with(fecho) {
            return Some(i + fecho.len());
        }
        i += 1;
    }
    None
}

/// `native 'nome'?` a partir de `i` (depois de brancos): o intervalo.
fn clausula(b: &[u8], i: usize) -> Option<Span> {
    let i = pular_brancos(b, i);
    if !b[i..].starts_with(b"native") || b.get(i + 6).is_some_and(|&c| parte_de_nome(c)) {
        return None;
    }
    let depois = pular_brancos(b, i + 6);
    let fim = fim_da_string(b, depois).unwrap_or(i + 6);
    Some(Span { start: i, end: fim })
}

/// Os diagnósticos de `native` da unidade (fora do SDK).
pub fn fora_do_sdk(unidade: Unidade<'_>) -> Vec<Diagnostic> {
    let ast = unidade.ast;
    let b = unidade.fonte.as_bytes();
    let mut out = Vec::new();
    for &id in &unidade.unit.declarations {
        let DeclKind::Class(c) = &ast.decl(id).kind else { continue };
        if c.mixin_application {
            continue;
        }
        let mut fim = c.name.span.end;
        fim = fim.max(c.type_params.iter().map(|t| t.span.end).max().unwrap_or(0));
        for t in c.extends.iter().chain(c.with.iter()).chain(c.implements.iter()) {
            fim = fim.max(ast.ty(*t).span.end);
        }
        // `>` que fecha os parâmetros de tipo.
        let mut i = pular_brancos(b, fim.min(b.len()));
        if !c.type_params.is_empty() && fim == c.type_params.iter().map(|t| t.span.end).max().unwrap_or(0) && b.get(i) == Some(&b'>') {
            i += 1;
        }
        if let Some(span) = clausula(b, i) {
            out.push(Diagnostic::com_codigo(p::NATIVE_CLAUSE_IN_NON_SDK_CODE, span, std::iter::empty::<&str>()));
        }
    }
    for f in &ast.functions {
        let FunctionBody::Native(nome) = &f.body else { continue };
        let limite = nome.as_ref().map_or(f.span.end, |s| s.span.start).min(b.len());
        let Some(inicio) = unidade.fonte[f.span.start.min(limite)..limite].rfind("native").map(|k| k + f.span.start.min(limite)) else {
            continue;
        };
        out.push(Diagnostic::com_codigo(
            p::NATIVE_FUNCTION_BODY_IN_NON_SDK_CODE,
            Span { start: inicio, end: f.span.end },
            std::iter::empty::<&str>(),
        ));
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;
    use dartforge_frontend::parser::parse;
    use dartforge_intern::Interner;

    fn achados(fonte: &str) -> Vec<(&'static str, usize, usize)> {
        let mut nomes = Interner::new();
        let out = parse(fonte, &mut nomes);
        assert!(out.diagnostics.is_empty(), "{fonte}: {:?}", out.diagnostics);
        let u = Unidade { ast: &out.ast, unit: &out.unit, fonte };
        fora_do_sdk(u).into_iter().map(|d| (d.code.unwrap().info().nome, d.span.start, d.span.end)).collect()
    }

    /// Posições do `dart analyze` 3.6.2 (`syntax/syntax_native_test.dart` e
    /// `executable_body/ExecutableBody__*_nativeBody.dart` do corpus).
    #[test]
    fn clausula_e_corpo_como_o_oraculo() {
        assert_eq!(
            achados("class A {}\nclass W<T> extends A native \"*W\" {}\nclass N native 'x' {}\n"),
            [("native_clause_in_non_sdk_code", 32, 43), ("native_clause_in_non_sdk_code", 55, 65)]
        );
        assert_eq!(
            achados("int f() native 'f';\nclass C { void m() native; }\n"),
            [("native_function_body_in_non_sdk_code", 8, 19), ("native_function_body_in_non_sdk_code", 39, 46)]
        );
    }
}
