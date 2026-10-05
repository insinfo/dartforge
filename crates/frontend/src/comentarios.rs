//! Os comentários de uma unidade (o lexer daqui os descarta), achados por
//! um varredor que conhece strings (simples, triplas, cruas) e
//! interpolações `${…}` aninhadas, para que `//` e `/*` dentro de uma string
//! não contem. Com eles: o `precedingComments` de um token (a sequência de
//! comentários colados antes dele, separados só por brancos) e o
//! `findDartDoc` do parser do analyzer 3.6.2
//! (`_fe_analyzer_shared/lib/src/parser/parser_impl.dart:9564`).
//! Escrito sem compilar nem executar (2026-10-05).

use crate::Span;

fn parte_de_identificador(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

/// Os comentários de uma unidade, em ordem.
pub struct Comentarios {
    spans: Vec<Span>,
}

impl Comentarios {
    pub fn de(fonte: &str) -> Comentarios {
        let mut spans = Vec::new();
        varrer(fonte.as_bytes(), 0, false, &mut spans);
        Comentarios { spans }
    }

    pub fn todos(&self) -> &[Span] {
        &self.spans
    }

    /// `token.precedingComments`: os comentários logo antes de `pos`, só
    /// com brancos entre eles e até `pos`, em ordem.
    pub fn antes_de(&self, fonte: &str, pos: usize) -> Vec<Span> {
        let b = fonte.as_bytes();
        let so_brancos = |de: usize, ate: usize| b.get(de..ate).is_some_and(|t| t.iter().all(u8::is_ascii_whitespace));
        let mut k = self.spans.partition_point(|s| s.end <= pos);
        let mut limite = pos;
        let mut v = Vec::new();
        while k > 0 {
            let s = self.spans[k - 1];
            if !so_brancos(s.end, limite) {
                break;
            }
            v.push(s);
            limite = s.start;
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
        let mut multilinha = false;
        for s in self.antes_de(fonte, pos) {
            let lexema = &fonte[s.start..s.end];
            if lexema.starts_with("///") {
                if !multilinha {
                    doc = Some(s);
                    multilinha = true;
                }
            } else if lexema.starts_with("/**") {
                doc = Some(s);
                multilinha = false;
            }
        }
        doc
    }
}

/// Varre código a partir de `i`; com `ate_chave`, para depois do `}` que
/// fecha a interpolação (devolve a posição depois dele).
fn varrer(b: &[u8], mut i: usize, ate_chave: bool, saida: &mut Vec<Span>) -> usize {
    let mut nivel = 0usize;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            let ini = i;
            while i < b.len() && b[i] != b'\n' && b[i] != b'\r' {
                i += 1;
            }
            saida.push(Span { start: ini, end: i });
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            // Os comentários de bloco aninham.
            let ini = i;
            let mut profundidade = 0usize;
            while i < b.len() {
                if b[i..].starts_with(b"/*") {
                    profundidade += 1;
                    i += 2;
                } else if b[i..].starts_with(b"*/") {
                    profundidade -= 1;
                    i += 2;
                    if profundidade == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            saida.push(Span { start: ini, end: i });
            continue;
        }
        if ate_chave {
            if c == b'{' {
                nivel += 1;
            } else if c == b'}' {
                if nivel == 0 {
                    return i + 1;
                }
                nivel -= 1;
            }
        }
        if c == b'\'' || c == b'"' {
            i = string(b, i, false, saida);
            continue;
        }
        if parte_de_identificador(c) {
            let ini = i;
            while i < b.len() && parte_de_identificador(b[i]) {
                i += 1;
            }
            // `r'…'`: a string crua.
            if &b[ini..i] == b"r" && matches!(b.get(i), Some(b'\'') | Some(b'"')) {
                i = string(b, i, true, saida);
            }
            continue;
        }
        i += 1;
    }
    i
}

/// Varre a string que começa na aspa em `i`; devolve a posição depois dela.
fn string(b: &[u8], mut i: usize, cru: bool, saida: &mut Vec<Span>) -> usize {
    let q = b[i];
    let tripla = b[i..].starts_with(&[q, q, q]);
    let n = if tripla { 3 } else { 1 };
    i += n;
    while i < b.len() {
        if b[i] == q && (!tripla || b[i..].starts_with(&[q, q, q])) {
            return i + n;
        }
        // A string simples não passa da linha.
        if !tripla && (b[i] == b'\n' || b[i] == b'\r') {
            return i;
        }
        if !cru && b[i] == b'\\' {
            i += 2;
            continue;
        }
        if !cru && b[i] == b'$' && b.get(i + 1) == Some(&b'{') {
            i = varrer(b, i + 2, true, saida);
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
    fn strings_nao_tem_comentarios() {
        let f = "var a = '// nao'; /* sim */ var b = \"${x /* sim2 */}\"; // fim\n";
        let c = Comentarios::de(f);
        let textos: Vec<&str> = c.todos().iter().map(|s| &f[s.start..s.end]).collect();
        assert_eq!(textos, vec!["/* sim */", "/* sim2 */", "// fim"]);
    }

    #[test]
    fn dart_doc_como_o_parser() {
        let f = "/** a */\n// b\n/// c\n/// d\nclass A {}\n";
        let c = Comentarios::de(f);
        let pos = f.find("class").unwrap();
        let d = c.dart_doc(f, pos).unwrap();
        assert_eq!(&f[d.start..d.end], "/// c");
    }
}
