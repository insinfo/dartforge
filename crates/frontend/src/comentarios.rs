//! Os comentários de uma unidade (o lexer daqui os descarta), achados por
//! um varredor que conhece strings (simples, triplas, cruas) e
//! interpolações `${…}` aninhadas, para que `//` e `/*` dentro de uma string
//! não contem. Com eles: o `precedingComments` de um token (a sequência de
//! comentários colados antes dele, separados só por brancos) e o
//! `findDartDoc` do parser do analyzer 3.6.2
//! (`_fe_analyzer_shared/lib/src/parser/parser_impl.dart:9564`).
//! Escrito sem compilar nem executar (2026-10-05).

use crate::Span;

fn identifier_part(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

/// Os comentários de uma unidade, em ordem.
pub struct Comentarios {
    spans: Vec<Span>,
}

impl Comentarios {
    pub fn de(fonte: &str) -> Comentarios {
        let mut spans = Vec::new();
        sweep(fonte.as_bytes(), 0, false, &mut spans);
        Comentarios { spans }
    }

    pub fn todos(&self) -> &[Span] {
        &self.spans
    }

    /// `token.precedingComments`: os comentários logo antes de `pos`, só
    /// com brancos entre eles e até `pos`, em ordem.
    pub fn antes_de(&self, fonte: &str, pos: usize) -> Vec<Span> {
        let b = fonte.as_bytes();
        let only_whitespace = |de: usize, until: usize| b.get(de..until).is_some_and(|t| t.iter().all(u8::is_ascii_whitespace));
        let mut k = self.spans.partition_point(|s| s.end <= pos);
        let mut limit = pos;
        let mut v = Vec::new();
        while k > 0 {
            let s = self.spans[k - 1];
            if !only_whitespace(s.end, limit) {
                break;
            }
            v.push(s);
            limit = s.start;
            k -= 1;
        }
        v.reverse();
        v
    }

    /// `findDartDoc(token)`: entre os comentários antes de `pos`, o último
    /// `/**`, ou o primeiro `///` da última sequência de `///` que vem
    /// depois dele.
    pub fn dart_doc(&self, fonte: &str, pos: usize) -> Option<Span> {
        let mut doc: Option<Span> = None;
        let mut multiline = false;
        for s in self.antes_de(fonte, pos) {
            let lexeme = &fonte[s.start..s.end];
            if lexeme.starts_with("///") {
                if !multiline {
                    doc = Some(s);
                    multiline = true;
                }
            } else if lexeme.starts_with("/**") {
                doc = Some(s);
                multiline = false;
            }
        }
        doc
    }
}

/// Varre código a partir de `i`; com `until_brace`, para depois do `}` que
/// fecha a interpolação (devolve a posição depois dele).
fn sweep(b: &[u8], mut i: usize, until_brace: bool, output: &mut Vec<Span>) -> usize {
    let mut level = 0usize;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            let start_ = i;
            while i < b.len() && b[i] != b'\n' && b[i] != b'\r' {
                i += 1;
            }
            output.push(Span { start: start_, end: i });
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            // Os comentários de bloco aninham.
            let start_ = i;
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
            output.push(Span { start: start_, end: i });
            continue;
        }
        if until_brace {
            if c == b'{' {
                level += 1;
            } else if c == b'}' {
                if level == 0 {
                    return i + 1;
                }
                level -= 1;
            }
        }
        if c == b'\'' || c == b'"' {
            i = string(b, i, false, output);
            continue;
        }
        if identifier_part(c) {
            let start_ = i;
            while i < b.len() && identifier_part(b[i]) {
                i += 1;
            }
            // `r'…'`: a string crua.
            if &b[start_..i] == b"r" && matches!(b.get(i), Some(b'\'') | Some(b'"')) {
                i = string(b, i, true, output);
            }
            continue;
        }
        i += 1;
    }
    i
}

/// Varre a string que começa na aspa em `i`; devolve a posição depois dela.
fn string(b: &[u8], mut i: usize, raw: bool, output: &mut Vec<Span>) -> usize {
    let q = b[i];
    let triple = b[i..].starts_with(&[q, q, q]);
    let n = if triple { 3 } else { 1 };
    i += n;
    while i < b.len() {
        if b[i] == q && (!triple || b[i..].starts_with(&[q, q, q])) {
            return i + n;
        }
        // A string simples não passa da linha.
        if !triple && (b[i] == b'\n' || b[i] == b'\r') {
            return i;
        }
        if !raw && b[i] == b'\\' {
            i += 2;
            continue;
        }
        if !raw && b[i] == b'$' && b.get(i + 1) == Some(&b'{') {
            i = sweep(b, i + 2, true, output);
            continue;
        }
        i += 1;
    }
    i
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn strings_have_no_comments() {
        let f = "var a = '// nao'; /* sim */ var b = \"${x /* sim2 */}\"; // fim\n";
        let c = Comentarios::de(f);
        let texts: Vec<&str> = c.todos().iter().map(|s| &f[s.start..s.end]).collect();
        assert_eq!(texts, vec!["/* sim */", "/* sim2 */", "// fim"]);
    }

    #[test]
    fn dart_doc_like_the_parser() {
        let f = "/** a */\n// b\n/// c\n/// d\nclass A {}\n";
        let c = Comentarios::de(f);
        let pos = f.find("class").unwrap();
        let d = c.dart_doc(f, pos).unwrap();
        assert_eq!(&f[d.start..d.end], "/// c");
    }
}
