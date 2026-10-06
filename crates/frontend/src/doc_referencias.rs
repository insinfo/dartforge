//! As referências `[…]` de um comentário de documentação como o
//! `DocCommentBuilder` do analyzer 3.6.2
//! (`analyzer/lib/src/fasta/doc_comment_builder.dart`): o comentário é lido
//! linha a linha (`_CharacterSequence`: depois do `///` de cada linha, ou as
//! linhas de um `/** */` sem o `* ` da margem); um bloco indentado (quatro
//! brancos depois de uma linha vazia), um bloco cercado por crases, uma
//! diretiva `{@…}` conhecida, um `@docImport` e um `@nodoc` não têm
//! referências; nas outras linhas, `_parseReferences` acha cada `[…]` que
//! não é texto de link, e `_parseOneCommentReference` o lê com o scanner do
//! Dart: `new` opcional, até dois prefixos com ponto e um identificador ou
//! um operador definível (`[int.+]`, `[operator ==]`).

use crate::Span;

/// Uma referência: as partes (o intervalo e o texto de cada uma, na ordem:
/// prefixos e o nome) e o `new` escrito antes delas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenciaDeDoc {
    pub novo: bool,
    pub partes: Vec<(Span, String)>,
}

/// Os operadores definíveis (`TokenType.isUserDefinableOperator`).
const OPERADORES: &[&str] = &["==", "~", "[]", "[]=", "*", "/", "%", "~/", "+", "-", "<<", ">>", ">>>", ">=", ">", "<=", "<", "&", "^", "|"];

/// As palavras reservadas (`KeywordStyle.reserved`): não são identificador.
const RESERVADAS: &[&str] = &[
    "assert", "break", "case", "catch", "class", "const", "continue", "default", "do", "else", "enum", "extends", "false", "final", "finally", "for", "if", "in", "is", "new", "null", "rethrow", "return", "super", "switch", "this", "throw", "true", "try", "var", "void", "while", "with",
];

/// As diretivas `{@…}` que o builder conhece (`_parseDocDirectiveTag`).
const DIRETIVAS: &[&str] = &["animation", "canonicalFor", "category", "end-inject-html", "end-tool", "endtemplate", "inject-html", "macro", "subCategory", "template", "tool", "youtube"];

/// `isWhitespace` do scanner: espaço, tabulação, LF e CR.
fn branco(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r')
}

/// `_readWhitespace`.
fn ler_brancos(conteudo: &[u8], mut i: usize) -> usize {
    while i < conteudo.len() && branco(conteudo[i]) {
        i += 1;
    }
    i
}

/// As linhas (`_CharacterSequence`): o deslocamento de cada conteúdo na
/// fonte e o conteúdo. `tokens` é a cadeia de comentários a partir do token
/// do `findDartDoc`.
fn linhas<'a>(fonte: &'a str, tokens: &[Span]) -> Vec<(usize, &'a str)> {
    let mut v = Vec::new();
    let Some(primeiro) = tokens.first() else { return v };
    let lexema_de = |s: &Span| {
        // O token de `//` termina antes do CR ou do LF.
        let t = &fonte[s.start..s.end];
        let fim = t.find(['\r', '\n']).unwrap_or(t.len());
        (s.start, &t[..fim])
    };
    if fonte[primeiro.start..].starts_with("///") {
        for s in tokens {
            let (inicio, lexema) = lexema_de(s);
            if lexema.starts_with("///") {
                v.push((inicio + 3, &lexema[3..]));
            }
        }
        return v;
    }
    // `_CharacterSequenceFromMultiLineComment`.
    let lexema = &fonte[primeiro.start..primeiro.end];
    let b = lexema.as_bytes();
    let base = primeiro.start;
    let primeira_quebra = lexema.find('\n').unwrap_or(lexema.len());
    v.push((base, &lexema[..primeira_quebra]));
    let mut fim = primeira_quebra;
    loop {
        let mut i = fim + 1;
        if i >= b.len() {
            break;
        }
        while branco(b[i]) {
            i += 1;
            if i >= b.len() {
                return v;
            }
        }
        let quebra = lexema[i..].find('\n').map_or(b.len(), |k| i + k);
        fim = quebra;
        if lexema[i..].starts_with("* ") {
            i += 2;
        } else if quebra == i + 1 && b[i] == b'*' {
            i += 1;
        }
        v.push((base + i, &lexema[i..quebra]));
    }
    v
}

/// `_fencedCodeBlockDelimiter`: o índice das crases de abertura, ou `None`.
/// A comparação é com três caracteres, e só casa quando o mínimo é três.
fn cerca(conteudo: &[u8], minimo: usize) -> Option<usize> {
    if conteudo.is_empty() {
        return None;
    }
    let i = ler_brancos(conteudo, 0);
    if i + 3 > conteudo.len() {
        return None;
    }
    (minimo == 3 && &conteudo[i..i + 3] == b"```").then_some(i)
}

/// As referências do comentário cujos tokens (a cadeia a partir do do
/// `findDartDoc`) são `tokens`.
pub fn referencias(fonte: &str, tokens: &[Span]) -> Vec<ReferenciaDeDoc> {
    let ls = linhas(fonte, tokens);
    let mut saida = Vec::new();
    let mut linha_vazia_antes = true;
    let mut k = 0;
    while k < ls.len() {
        let (desloc, conteudo) = ls[k];
        let b = conteudo.as_bytes();
        let ws = ler_brancos(b, 0);
        if linha_vazia_antes && ws >= 4 {
            // `_parseIndentedCodeBlock`: até a primeira linha com menos de
            // quatro brancos, que é lida em seguida.
            k += 1;
            while k < ls.len() && ler_brancos(ls[k].1.as_bytes(), 0) >= 4 {
                k += 1;
            }
            if k < ls.len() {
                linha_vazia_antes = ls[k].1.is_empty();
            }
            continue;
        }
        if let Some(i) = cerca(b, 3) {
            // `_parseFencedCodeBlock`: até a linha que fecha (com o mínimo
            // de crases da abertura), consumida.
            let mut crases = 0;
            let mut j = i;
            while j < b.len() && b[j] == b'`' {
                crases += 1;
                j += 1;
            }
            k += 1;
            while k < ls.len() {
                if cerca(ls[k].1.as_bytes(), crases).is_some() {
                    break;
                }
                k += 1;
            }
            linha_vazia_antes = false;
            k += 1;
            continue;
        }
        if diretiva_conhecida(b, ws) || doc_import(conteudo, ws) || nodoc(b, ws) {
            linha_vazia_antes = false;
            k += 1;
            continue;
        }
        referencias_da_linha(conteudo, desloc, &mut saida);
        linha_vazia_antes = conteudo.is_empty();
        k += 1;
    }
    saida
}

/// `_parseDocDirectiveTag`: `{@nome` com um nome conhecido.
fn diretiva_conhecida(b: &[u8], i: usize) -> bool {
    if !b[i..].starts_with(b"{@") {
        return false;
    }
    let mut j = i + 2;
    if j >= b.len() {
        return false;
    }
    let inicio = j;
    while j < b.len() && !branco(b[j]) && b[j] != b'}' {
        j += 1;
    }
    let nome = std::str::from_utf8(&b[inicio..j]).unwrap_or("");
    DIRETIVAS.contains(&nome)
}

/// `_parseDocImport`: `@docImport ` seguido de um import (a URI entre
/// aspas).
fn doc_import(conteudo: &str, i: usize) -> bool {
    let Some(resto) = conteudo[i..].strip_prefix("@docImport ") else { return false };
    resto.trim_start_matches([' ', '\t', '\n', '\r']).starts_with(['\'', '"'])
}

/// `_parseNodoc`.
fn nodoc(b: &[u8], i: usize) -> bool {
    if !b[i..].starts_with(b"@nodoc") {
        return false;
    }
    b.len() == i + 6 || b[i + 6] == b' '
}

/// `_findCommentReferenceEnd`: o fim de `ident` ou `ident.ident` (letras e
/// dígitos ASCII), para o `[` sem `]` na linha.
fn fim_da_referencia(b: &[u8], mut i: usize, fim: usize) -> usize {
    let letra = |c: u8| c.is_ascii_alphabetic();
    let letra_ou_digito = |c: u8| c.is_ascii_alphanumeric();
    if i >= fim || !letra(b[i]) {
        return i;
    }
    while i < fim && letra_ou_digito(b[i]) {
        i += 1;
    }
    if i >= fim || b[i] != b'.' {
        return i;
    }
    i += 1;
    if i >= fim || !letra(b[i]) {
        return i;
    }
    i += 1;
    while i < fim && letra_ou_digito(b[i]) {
        i += 1;
    }
    i
}

/// `_isLinkText`.
fn texto_de_link(b: &[u8], direita: usize, pode_ser_referencia_de_link: bool) -> bool {
    let mut i = direita + 1;
    if i >= b.len() {
        return false;
    }
    let mut c = b[i];
    if c == b'(' {
        return true;
    }
    if pode_ser_referencia_de_link && c == b':' {
        return true;
    }
    while branco(c) {
        i += 1;
        if i >= b.len() {
            return false;
        }
        c = b[i];
    }
    c == b'['
}

/// `_parseReferences` numa linha.
fn referencias_da_linha(conteudo: &str, desloc: usize, saida: &mut Vec<ReferenciaDeDoc>) {
    let b = conteudo.as_bytes();
    let fim = b.len();
    let mut i = 0;
    let mut so_brancos = true;
    while i < fim {
        let c = b[i];
        if c == b'[' {
            i += 1;
            if i < fim && b[i] == b':' {
                // `[:código:]` antigo.
                match conteudo[i + 1..].find(":]") {
                    Some(k) => i = i + 1 + k + 1,
                    None => break,
                }
                if i > fim {
                    break;
                }
            } else {
                let inicio = i;
                let direita = match conteudo[i..].find(']') {
                    Some(k) => i + k,
                    None => fim_da_referencia(b, inicio, fim),
                };
                i = direita;
                if !texto_de_link(b, direita, so_brancos)
                    && let Some(r) = uma_referencia(&conteudo[inicio..direita], desloc + inicio)
                {
                    saida.push(r);
                }
            }
            so_brancos = false;
        } else if c == b'`' {
            if let Some(k) = conteudo[i + 1..].find('`') {
                i = i + 1 + k;
            }
            so_brancos = false;
        } else if !branco(c) {
            so_brancos = false;
        }
        i += 1;
    }
}

/// Um token do scanner do Dart, para o que a referência precisa.
#[derive(Debug, Clone)]
enum Tk {
    /// Identificador ou palavra-chave (o texto decide).
    Palavra(Span, String),
    Ponto,
    Operador(Span, String),
    Outro,
}

/// O scanner do Dart sobre o texto de uma referência: `None` quando ele
/// acusa erro (caractere inválido, string sem fechar).
fn escanear(texto: &str, desloc: usize) -> Option<Vec<Tk>> {
    let b = texto.as_bytes();
    let mut v = Vec::new();
    let mut i = 0;
    // Os operadores, do maior ao menor (o scanner consome o mais longo).
    const SINAIS: &[&str] = &[
        ">>>=", "...?", "<<=", ">>=", ">>>", "~/=", "??=", "...", "&&=", "||=", "==", "!=", "<=", ">=", "<<", ">>", "~/", "[]=", "[]", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "&&", "||", "++", "--", "??", "?.", "=>", "+", "-", "*",
        "/", "%", "~", "<", ">", "&", "|", "^", "!", "?", "=", ":", ";", ",", "(", ")", "[", "]", "{", "}", "@", "#",
    ];
    while i < b.len() {
        let c = b[i];
        if branco(c) {
            i += 1;
            continue;
        }
        if b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' && b[i] != b'\r' {
                i += 1;
            }
            continue;
        }
        if b[i..].starts_with(b"/*") {
            let mut prof = 0usize;
            while i + 1 < b.len() {
                if b[i] == b'/' && b[i + 1] == b'*' {
                    prof += 1;
                    i += 2;
                } else if b[i] == b'*' && b[i + 1] == b'/' {
                    prof -= 1;
                    i += 2;
                    if prof == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            if prof != 0 {
                return None;
            }
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' || c == b'$' {
            let ini = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
            v.push(Tk::Palavra(Span { start: desloc + ini, end: desloc + i }, texto[ini..i].to_string()));
            continue;
        }
        if c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'.' || b[i] == b'_') {
                i += 1;
            }
            v.push(Tk::Outro);
            continue;
        }
        if c == b'\'' || c == b'"' {
            let fecha = texto[i + 1..].find(c as char)?;
            i = i + 1 + fecha + 1;
            v.push(Tk::Outro);
            continue;
        }
        if c == b'.' {
            v.push(Tk::Ponto);
            i += 1;
            continue;
        }
        let sinal = SINAIS.iter().find(|s| b[i..].starts_with(s.as_bytes()))?;
        v.push(Tk::Operador(Span { start: desloc + i, end: desloc + i + sinal.len() }, sinal.to_string()));
        i += sinal.len();
    }
    Some(v)
}

/// `token.isIdentifier`: identificador, palavra embutida ou pseudo.
fn identificador(t: Option<&Tk>) -> bool {
    matches!(t, Some(Tk::Palavra(_, p)) if !RESERVADAS.contains(&p.as_str()))
}

/// `_parseOneCommentReference`.
fn uma_referencia(texto: &str, desloc: usize) -> Option<ReferenciaDeDoc> {
    let tks = escanear(texto, desloc)?;
    let t = |i: usize| tks.get(i);
    let mut i = 0;
    let mut novo = false;
    if matches!(t(i), Some(Tk::Palavra(_, p)) if p == "new") {
        novo = true;
        i += 1;
    }
    let mut partes: Vec<(Span, String)> = Vec::new();
    if identificador(t(i)) && matches!(t(i + 1), Some(Tk::Ponto)) {
        let mut segundo = i;
        let mut ponto = i + 1;
        if identificador(t(ponto + 1)) && matches!(t(ponto + 2), Some(Tk::Ponto)) {
            if let Some(Tk::Palavra(s, p)) = t(segundo) {
                partes.push((*s, p.clone()));
            }
            segundo = ponto + 1;
            ponto += 2;
        }
        if let Some(Tk::Palavra(s, p)) = t(segundo) {
            partes.push((*s, p.clone()));
        }
        i = ponto + 1;
        // `new` depois do `.` vira identificador (o construtor sem nome).
    }
    // Sem nada depois: o identificador sintético da recuperação (vazio, não
    // resolve); os prefixos antes dele são referências.
    if t(i).is_none() {
        let fim = partes.last()?.0.end;
        partes.push((Span { start: fim + 1, end: fim + 1 }, String::new()));
        return Some(ReferenciaDeDoc { novo, partes });
    }
    let mut palavra_operator = None;
    if matches!(t(i), Some(Tk::Palavra(_, p)) if p == "operator") {
        palavra_operator = Some(i);
        i += 1;
    }
    if let Some(Tk::Operador(s, op)) = t(i)
        && OPERADORES.contains(&op.as_str())
    {
        if t(i + 1).is_none() {
            partes.push((*s, op.clone()));
            return Some(ReferenciaDeDoc { novo, partes });
        }
        return None;
    }
    let j = palavra_operator.unwrap_or(i);
    if t(j + 1).is_some() {
        return None;
    }
    let depois_do_ponto = !partes.is_empty();
    match t(j) {
        Some(Tk::Palavra(s, p)) if identificador(t(j)) || (depois_do_ponto && p == "new") => {
            partes.push((*s, p.clone()));
            Some(ReferenciaDeDoc { novo, partes })
        }
        _ => None,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn refs(fonte: &str) -> Vec<String> {
        let c = crate::comentarios::Comentarios::de(fonte);
        let pos = fonte.find("class").unwrap();
        let inicio = c.dart_doc(fonte, pos).unwrap();
        let tokens: Vec<Span> = c.antes_de(fonte, pos).into_iter().filter(|s| s.start >= inicio.start).collect();
        referencias(fonte, &tokens).into_iter().map(|r| r.partes.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join(".")).collect()
    }

    #[test]
    fn como_o_doc_comment_builder() {
        assert_eq!(refs("/// [a], [b.c], [new D], [E.new], [int.+], [operator ==].\nclass X {}\n"), vec!["a", "b.c", "D", "E.new", "int.+", "=="]);
        // Links: `[t](u)`, `[t] [r]` (o `[r]` é referência), `[r]:` no começo.
        // `[r] [q]`: o `[r]` seguido de `[` (depois de brancos) é texto de link.
        assert_eq!(refs("/// [t](u) [x][r] [q]: y\n/// [z]: link\n/// [x][s]\nclass X {}\n"), vec!["q", "s"]);
        // Código entre crases, bloco cercado e bloco indentado.
        assert_eq!(refs("/// `[a]` [b]\n/// ```\n/// [c]\n/// ```\n///\n///     [d]\n/// [e]\nclass X {}\n"), vec!["b", "e"]);
        // `[a [b]`: o conteúdo vai até o primeiro `]`.
        assert_eq!(refs("/// [a [b] [c d] [ f ]\nclass X {}\n"), vec!["f"]);
        // `//` no meio da sequência é pulado; `{@template}` não tem referência.
        assert_eq!(refs("/// [a]\n// [b]\n/// {@template t} [c]\n/// [d\nclass X {}\n"), vec!["a", "d"]);
        // Bloco `/** */`.
        assert_eq!(refs("/**\n * [a] e\n *\n * [b]\n */\nclass X {}\n"), vec!["a", "b"]);
    }
}
