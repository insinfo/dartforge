//! Os literais de string e os comentários como o texto os escreve, para as
//! regras de lint que olham aspas, escapes e linhas: a árvore guarda um
//! `StringLit` por expressão (as strings adjacentes juntas, o texto já
//! decodificado), e essas regras perguntam por cada literal
//! (`SimpleStringLiteral` ou `StringInterpolation` do analyzer) e pelo
//! lexema dele.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use dartforge_diagnostics::Span;

/// Um literal de string: `'…'`, `"…"`, `'''…'''`, `"""…"""`, com ou sem `r`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Literal {
    /// Do `r` ou da aspa de abertura até depois da aspa de fecho.
    pub span: Span,
    pub crua: bool,
    /// Três aspas.
    pub multilinha: bool,
    /// Aspas simples (`'` ou `'''`).
    pub aspas_simples: bool,
    /// Os trechos de texto, em ordem: o conteúdo entre as aspas, cortado
    /// pelas interpolações. Sempre há pelo menos um (pode ser vazio).
    pub trechos: Vec<Span>,
    /// As interpolações, do `$` ao fim do identificador ou ao `}`.
    pub interpolacoes: Vec<Span>,
}

impl Literal {
    /// Tem interpolação (o `StringInterpolation` do analyzer).
    pub fn interpolado(&self) -> bool {
        !self.interpolacoes.is_empty()
    }

    /// O texto dos trechos, como está escrito (sem decodificar escapes).
    pub fn texto(&self, fonte: &str) -> String {
        self.trechos.iter().filter_map(|t| fonte.get(t.start..t.end)).collect()
    }
}

fn de_identificador(c: u8) -> bool {
    c == b'_' || c == b'$' || c.is_ascii_alphanumeric()
}

/// A posição depois do comentário de bloco (aninhável) que começa em `i`.
fn fim_do_bloco(b: &[u8], mut i: usize, fim: usize) -> usize {
    let mut profundidade = 0usize;
    while i < fim {
        if i + 1 < fim && b[i] == b'/' && b[i + 1] == b'*' {
            profundidade += 1;
            i += 2;
        } else if i + 1 < fim && b[i] == b'*' && b[i + 1] == b'/' {
            profundidade -= 1;
            i += 2;
            if profundidade == 0 {
                return i;
            }
        } else {
            i += 1;
        }
    }
    fim
}

/// Pula brancos e comentários.
fn pular_espaco(b: &[u8], mut i: usize, fim: usize) -> usize {
    loop {
        while i < fim && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i + 1 < fim && b[i] == b'/' && b[i + 1] == b'/' {
            while i < fim && b[i] != b'\n' {
                i += 1;
            }
        } else if i + 1 < fim && b[i] == b'/' && b[i + 1] == b'*' {
            i = fim_do_bloco(b, i, fim);
        } else {
            return i;
        }
    }
}

/// A posição depois do `}` que fecha a interpolação cujo miolo começa em
/// `i`. Strings e comentários de dentro são pulados inteiros.
fn fecha_chave(b: &[u8], mut i: usize, fim: usize) -> usize {
    let mut profundidade = 1usize;
    while i < fim {
        match b[i] {
            b'{' => profundidade += 1,
            b'}' => {
                profundidade -= 1;
                if profundidade == 0 {
                    return i + 1;
                }
            }
            b'/' if i + 1 < fim && (b[i + 1] == b'/' || b[i + 1] == b'*') => {
                i = pular_espaco(b, i, fim);
                continue;
            }
            b'\'' | b'"' => {
                let crua = i > 0 && b[i - 1] == b'r' && (i < 2 || !de_identificador(b[i - 2]));
                if let Some((_, depois)) = literal(b, if crua { i - 1 } else { i }, fim) {
                    i = depois;
                    continue;
                }
            }
            _ => {}
        }
        i += 1;
    }
    fim
}

/// Lê o literal que começa em `i` (no `r` ou na aspa); devolve-o com a
/// posição seguinte. Um literal que não fecha vai até `fim`.
fn literal(b: &[u8], inicio: usize, fim: usize) -> Option<(Literal, usize)> {
    let mut i = inicio;
    let crua = b.get(i) == Some(&b'r');
    if crua {
        i += 1;
    }
    let aspa = *b.get(i).filter(|_| i < fim)?;
    if aspa != b'\'' && aspa != b'"' {
        return None;
    }
    let tripla = i + 2 < fim && b[i + 1] == aspa && b[i + 2] == aspa;
    let n = if tripla { 3 } else { 1 };
    i += n;
    let mut trechos = Vec::new();
    let mut interpolacoes = Vec::new();
    let mut do_trecho = i;
    let pronto = |trechos: Vec<Span>, interpolacoes: Vec<Span>, ate: usize| Literal {
        span: Span { start: inicio, end: ate },
        crua,
        multilinha: tripla,
        aspas_simples: aspa == b'\'',
        trechos,
        interpolacoes,
    };
    while i < fim {
        let ch = b[i];
        if ch == aspa && (!tripla || (i + 2 < fim && b[i + 1] == aspa && b[i + 2] == aspa)) {
            trechos.push(Span { start: do_trecho, end: i });
            return Some((pronto(trechos, interpolacoes, i + n), i + n));
        }
        if !crua && ch == b'\\' {
            i += 2;
            continue;
        }
        if !crua && ch == b'$' {
            let seguinte = b.get(i + 1).copied().filter(|_| i + 1 < fim);
            if seguinte == Some(b'{') {
                trechos.push(Span { start: do_trecho, end: i });
                let depois = fecha_chave(b, i + 2, fim);
                interpolacoes.push(Span { start: i, end: depois });
                i = depois;
                do_trecho = i;
                continue;
            }
            if seguinte.is_some_and(|c| c == b'_' || c.is_ascii_alphabetic()) {
                trechos.push(Span { start: do_trecho, end: i });
                let mut j = i + 1;
                while j < fim && (b[j] == b'_' || b[j].is_ascii_alphanumeric()) {
                    j += 1;
                }
                interpolacoes.push(Span { start: i, end: j });
                i = j;
                do_trecho = i;
                continue;
            }
        }
        i += 1;
    }
    trechos.push(Span { start: do_trecho.min(fim), end: fim });
    Some((pronto(trechos, interpolacoes, fim), fim))
}

/// Os literais escritos em `span` (o intervalo de uma expressão de string:
/// um literal, ou vários adjacentes separados por brancos e comentários).
pub fn literais(fonte: &str, span: Span) -> Vec<Literal> {
    let b = fonte.as_bytes();
    let fim = span.end.min(b.len());
    let mut i = span.start;
    let mut v = Vec::new();
    loop {
        i = pular_espaco(b, i, fim);
        if i >= fim {
            break;
        }
        match literal(b, i, fim) {
            Some((l, depois)) => {
                v.push(l);
                i = depois;
            }
            None => break,
        }
    }
    v
}

/// Os comentários da fonte (de linha e de bloco), fora das strings.
pub fn comentarios(fonte: &str) -> Vec<Span> {
    let b = fonte.as_bytes();
    let fim = b.len();
    let mut i = 0;
    let mut v = Vec::new();
    while i < fim {
        let ch = b[i];
        if ch == b'/' && b.get(i + 1) == Some(&b'/') {
            let inicio = i;
            while i < fim && b[i] != b'\n' && b[i] != b'\r' {
                i += 1;
            }
            v.push(Span { start: inicio, end: i });
            continue;
        }
        if ch == b'/' && b.get(i + 1) == Some(&b'*') {
            let inicio = i;
            i = fim_do_bloco(b, i, fim);
            v.push(Span { start: inicio, end: i });
            continue;
        }
        if ch == b'\'' || ch == b'"' {
            let crua = i > 0 && b[i - 1] == b'r' && (i < 2 || !de_identificador(b[i - 2]));
            if let Some((_, depois)) = literal(b, if crua { i - 1 } else { i }, fim) {
                i = depois;
                continue;
            }
        }
        i += 1;
    }
    v
}

#[cfg(test)]
mod testes {
    use super::*;

    fn ler(fonte: &str) -> Vec<Literal> {
        literais(fonte, Span { start: 0, end: fonte.len() })
    }

    #[test]
    fn literais_adjacentes_e_interpolacao() {
        let fonte = "'a' \"b$c d${e ? 'x}' : \"y\"}f\" // z\n r'''g'h'''";
        let v = ler(fonte);
        assert_eq!(v.len(), 3);
        assert_eq!(&fonte[v[0].span.start..v[0].span.end], "'a'");
        assert!(v[0].aspas_simples && !v[0].interpolado() && !v[0].crua && !v[0].multilinha);
        assert_eq!(v[1].texto(fonte), "b df");
        let interpolacoes: Vec<&str> = v[1].interpolacoes.iter().map(|s| &fonte[s.start..s.end]).collect();
        assert_eq!(interpolacoes, vec!["$c", "${e ? 'x}' : \"y\"}"]);
        assert!(v[2].crua && v[2].multilinha && v[2].aspas_simples);
        assert_eq!(v[2].texto(fonte), "g'h");
    }

    #[test]
    fn comentarios_fora_de_strings() {
        let fonte = "var a = '// não'; // sim\n/* b /* c */ d */ var e = \"/* não */\";\n";
        let v: Vec<&str> = comentarios(fonte).iter().map(|s| &fonte[s.start..s.end]).collect();
        assert_eq!(v, vec!["// sim", "/* b /* c */ d */"]);
    }
}
