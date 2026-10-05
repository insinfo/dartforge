//! `LanguageVersionOverrideVerifier`
//! (`analyzer/lib/src/error/language_version_override_verifier.dart`,
//! docs/ANALYZER-ESPECIFICACAO.md, `invalid_language_version_override`): os
//! comentários antes do primeiro token que parecem um `// @dart = X.Y` mal
//! escrito, e os `// @dart = X.Y` depois da primeira diretiva ou declaração.
//! O `_GREATER` vem do scanner (`crates/elements/src/load.rs`).
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::Unidade;
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Codigo, Diagnostic, Span};
use dartforge_frontend::comentarios::Comentarios;

fn branco(c: u8) -> bool {
    c == 0x09 || c == 0x20
}

fn digito(c: u8) -> bool {
    c.is_ascii_digit()
}

fn letra(c: u8) -> bool {
    c.is_ascii_alphabetic()
}

/// `_findLanguageVersionOverrideComment`: `Ok(true)` num override válido
/// (a busca para), `Ok(false)` quando não é tentativa, `Err(código)` quando
/// é uma tentativa inválida (a busca segue).
fn conferir(comentario: &[u8]) -> Result<bool, Codigo> {
    let n = comentario.len();
    let mut i = 0usize;
    let pular_brancos = |i: &mut usize| {
        while *i < n && branco(comentario[*i]) {
            *i += 1;
        }
    };
    while i < n && comentario[i] == b'/' {
        i += 1;
    }
    let barras = i;
    pular_brancos(&mut i);
    if i == n {
        return Ok(false);
    }
    let arroba = comentario[i] == b'@';
    if arroba {
        i += 1;
    }
    if n - i < 4 {
        return Ok(false);
    }
    let dart = &comentario[i..i + 4];
    if !dart.eq_ignore_ascii_case(b"dart") {
        return Ok(false);
    }
    i += 4;
    pular_brancos(&mut i);
    if i == n {
        return Ok(false);
    }
    let inicio_separador = i;
    while i < n {
        let c = comentario[i];
        if digito(c) || letra(c) || branco(c) {
            break;
        }
        i += 1;
    }
    if i == n {
        return Ok(false);
    }
    let separador = i - inicio_separador;
    pular_brancos(&mut i);
    if i == n {
        return Ok(false);
    }
    let mut prefixo = false;
    if letra(comentario[i]) {
        prefixo = true;
        i += 1;
        if i == n {
            return Ok(false);
        }
    }
    if !digito(comentario[i]) {
        return Ok(false);
    }
    if i + 1 < n && letra(comentario[i + 1]) {
        return Ok(false);
    }
    if !arroba && separador == 0 {
        return Ok(false);
    }
    if barras > 2 {
        return Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_TWO_SLASHES);
    }
    if !arroba {
        return Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_AT_SIGN);
    }
    if dart != b"dart" {
        return Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_LOWER_CASE);
    }
    if separador != 1 || comentario[inicio_separador] != b'=' {
        return Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_EQUALS);
    }
    if prefixo {
        return Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_PREFIX);
    }
    while i < n && digito(comentario[i]) {
        i += 1;
    }
    if i == n || comentario[i] != b'.' {
        return Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_NUMBER);
    }
    i += 1;
    while i < n && digito(comentario[i]) {
        i += 1;
    }
    pular_brancos(&mut i);
    if i == n {
        return Ok(true);
    }
    Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_TRAILING_CHARACTERS)
}

/// `_overrideCommentLine` (`^\s*//\s*@dart\s*=\s*\d+\.\d+`): o fim do
/// casamento, a partir do começo do comentário.
fn casa_override(texto: &[u8]) -> Option<usize> {
    let n = texto.len();
    let mut i = 0usize;
    let espacos = |i: &mut usize| {
        while *i < n && texto[*i].is_ascii_whitespace() {
            *i += 1;
        }
    };
    espacos(&mut i);
    if !texto[i..].starts_with(b"//") {
        return None;
    }
    i += 2;
    espacos(&mut i);
    if !texto[i..].starts_with(b"@dart") {
        return None;
    }
    i += 5;
    espacos(&mut i);
    if texto.get(i) != Some(&b'=') {
        return None;
    }
    i += 1;
    espacos(&mut i);
    let d = i;
    while i < n && digito(texto[i]) {
        i += 1;
    }
    if i == d || texto.get(i) != Some(&b'.') {
        return None;
    }
    i += 1;
    let d = i;
    while i < n && digito(texto[i]) {
        i += 1;
    }
    (i > d).then_some(i)
}

/// O primeiro token depois do `#!` (o começo do código), pulando brancos e
/// comentários.
fn primeiro_token(fonte: &[u8], comentarios: &[Span], mut pos: usize) -> usize {
    loop {
        while pos < fonte.len() && fonte[pos].is_ascii_whitespace() {
            pos += 1;
        }
        match comentarios.iter().find(|c| c.start == pos) {
            Some(c) => pos = c.end,
            None => return pos,
        }
    }
}

pub fn verificar(u: Unidade<'_>) -> Vec<Diagnostic> {
    let fonte = u.fonte.as_bytes();
    let comentarios = Comentarios::de(u.fonte);
    let todos = comentarios.todos();
    let mut saida = Vec::new();
    // `_verifyMisplaced`: depois do primeiro token significativo.
    let primeiro_significativo = u
        .unit
        .directives
        .first()
        .map(|d| d.span.start)
        .or_else(|| u.unit.declarations.first().map(|&d| u.ast.decl(d).span.start));
    if let Some(p) = primeiro_significativo {
        for c in todos.iter().filter(|c| c.start > p) {
            let lexema = &fonte[c.start..c.end];
            if let Some(fim) = casa_override(lexema)
                && let Some(arroba) = u.fonte[c.start..c.end].find("@dart")
            {
                saida.push(Diagnostic::com_codigo(
                    w::INVALID_LANGUAGE_VERSION_OVERRIDE_LOCATION,
                    Span { start: c.start + arroba, end: c.start + fim },
                    Vec::<&str>::new(),
                ));
            }
        }
    }
    // Os comentários antes do primeiro token (depois do `#!`).
    let depois_do_script = if fonte.starts_with(b"#!") { fonte.iter().position(|&b| b == b'\n').map_or(fonte.len(), |i| i + 1) } else { 0 };
    let primeiro = primeiro_token(fonte, todos, depois_do_script);
    for c in todos.iter().filter(|c| c.start >= depois_do_script && c.end <= primeiro) {
        match conferir(&fonte[c.start..c.end]) {
            Ok(true) => break,
            Ok(false) => {}
            Err(codigo) => saida.push(Diagnostic::com_codigo(codigo, *c, Vec::<&str>::new())),
        }
    }
    saida
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatos() {
        assert_eq!(conferir(b"// @dart = 2.12"), Ok(true));
        assert_eq!(conferir(b"// dart = 2.0"), Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_AT_SIGN));
        assert_eq!(conferir(b"// @dart 2.0"), Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_EQUALS));
        assert_eq!(conferir(b"// @dart >= 2.0"), Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_EQUALS));
        assert_eq!(conferir(b"// @Dart = 2.0"), Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_LOWER_CASE));
        assert_eq!(conferir(b"/// @dart = 2.0"), Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_TWO_SLASHES));
        assert_eq!(conferir(b"// @dart = v2.0"), Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_PREFIX));
        assert_eq!(conferir(b"// @dart = 2"), Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_NUMBER));
        assert_eq!(conferir(b"// @dart = 2.0 x"), Err(w::INVALID_LANGUAGE_VERSION_OVERRIDE_TRAILING_CHARACTERS));
        assert_eq!(conferir(b"/// dart2 is great"), Ok(false));
        assert_eq!(casa_override(b"// @dart = 3.0"), Some(14));
    }
}
