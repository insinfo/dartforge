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
pub struct DocReference {
    pub novo: bool,
    pub partes: Vec<(Span, String)>,
}

/// Os operadores definíveis (`TokenType.isUserDefinableOperator`).
const OPERATORS: &[&str] = &["==", "~", "[]", "[]=", "*", "/", "%", "~/", "+", "-", "<<", ">>", ">>>", ">=", ">", "<=", "<", "&", "^", "|"];

/// As palavras reservadas (`KeywordStyle.reserved`): não são identificador.
const RESERVED_WORDS: &[&str] = &[
    "assert", "break", "case", "catch", "class", "const", "continue", "default", "do", "else", "enum", "extends", "false", "final", "finally", "for", "if", "in", "is", "new", "null", "rethrow", "return", "super", "switch", "this", "throw", "true", "try", "var", "void", "while", "with",
];

/// As diretivas `{@…}` que o builder conhece (`_parseDocDirectiveTag`).
const DIRECTIVES: &[&str] = &["animation", "canonicalFor", "category", "end-inject-html", "end-tool", "endtemplate", "inject-html", "macro", "subCategory", "template", "tool", "youtube"];

/// `isWhitespace` do scanner: espaço, tabulação, LF e CR.
fn whitespace(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r')
}

/// `_readWhitespace`.
fn read_whitespace(content: &[u8], mut i: usize) -> usize {
    while i < content.len() && whitespace(content[i]) {
        i += 1;
    }
    i
}

/// As linhas (`_CharacterSequence`): o deslocamento de cada conteúdo na
/// fonte e o conteúdo. `tokens` é a cadeia de comentários a partir do token
/// do `findDartDoc`.
fn lines_<'a>(fonte: &'a str, tokens: &[Span]) -> Vec<(usize, &'a str)> {
    let mut v = Vec::new();
    let Some(first) = tokens.first() else { return v };
    let lexeme_of = |s: &Span| {
        // O token de `//` termina antes do CR ou do LF.
        let t = &fonte[s.start..s.end];
        let end_offset = t.find(['\r', '\n']).unwrap_or(t.len());
        (s.start, &t[..end_offset])
    };
    if fonte[first.start..].starts_with("///") {
        for s in tokens {
            let (start_offset, lexeme) = lexeme_of(s);
            if lexeme.starts_with("///") {
                v.push((start_offset + 3, &lexeme[3..]));
            }
        }
        return v;
    }
    // `_CharacterSequenceFromMultiLineComment`.
    let lexeme = &fonte[first.start..first.end];
    let b = lexeme.as_bytes();
    let base = first.start;
    let first_break = lexeme.find('\n').unwrap_or(lexeme.len());
    v.push((base, &lexeme[..first_break]));
    let mut end_offset = first_break;
    loop {
        let mut i = end_offset + 1;
        if i >= b.len() {
            break;
        }
        while whitespace(b[i]) {
            i += 1;
            if i >= b.len() {
                return v;
            }
        }
        let break_ = lexeme[i..].find('\n').map_or(b.len(), |k| i + k);
        end_offset = break_;
        if lexeme[i..].starts_with("* ") {
            i += 2;
        } else if break_ == i + 1 && b[i] == b'*' {
            i += 1;
        }
        v.push((base + i, &lexeme[i..break_]));
    }
    v
}

/// `_fencedCodeBlockDelimiter`: o índice das crases de abertura, ou `None`.
/// A comparação é com três caracteres, e só casa quando o mínimo é três.
fn fence(content: &[u8], minimum: usize) -> Option<usize> {
    if content.is_empty() {
        return None;
    }
    let i = read_whitespace(content, 0);
    if i + 3 > content.len() {
        return None;
    }
    (minimum == 3 && &content[i..i + 3] == b"```").then_some(i)
}

/// As referências do comentário cujos tokens (a cadeia a partir do do
/// `findDartDoc`) são `tokens`.
pub fn referencias(fonte: &str, tokens: &[Span]) -> Vec<DocReference> {
    let ls = lines_(fonte, tokens);
    let mut output = Vec::new();
    let mut blank_line_before = true;
    let mut k = 0;
    while k < ls.len() {
        let (shift, content) = ls[k];
        let b = content.as_bytes();
        let ws = read_whitespace(b, 0);
        if blank_line_before && ws >= 4 {
            // `_parseIndentedCodeBlock`: até a primeira linha com menos de
            // quatro brancos, que é lida em seguida.
            k += 1;
            while k < ls.len() && read_whitespace(ls[k].1.as_bytes(), 0) >= 4 {
                k += 1;
            }
            if k < ls.len() {
                blank_line_before = ls[k].1.is_empty();
            }
            continue;
        }
        if let Some(i) = fence(b, 3) {
            // `_parseFencedCodeBlock`: até a linha que fecha (com o mínimo
            // de crases da abertura), consumida.
            let mut backticks = 0;
            let mut j = i;
            while j < b.len() && b[j] == b'`' {
                backticks += 1;
                j += 1;
            }
            k += 1;
            while k < ls.len() {
                if fence(ls[k].1.as_bytes(), backticks).is_some() {
                    break;
                }
                k += 1;
            }
            blank_line_before = false;
            k += 1;
            continue;
        }
        if known_directive(b, ws) || doc_import(content, ws) || nodoc(b, ws) {
            blank_line_before = false;
            k += 1;
            continue;
        }
        line_references(content, shift, &mut output);
        blank_line_before = content.is_empty();
        k += 1;
    }
    output
}

/// `_parseDocDirectiveTag`: `{@nome` com um nome conhecido.
fn known_directive(b: &[u8], i: usize) -> bool {
    if !b[i..].starts_with(b"{@") {
        return false;
    }
    let mut j = i + 2;
    if j >= b.len() {
        return false;
    }
    let start_offset = j;
    while j < b.len() && !whitespace(b[j]) && b[j] != b'}' {
        j += 1;
    }
    let nome = std::str::from_utf8(&b[start_offset..j]).unwrap_or("");
    DIRECTIVES.contains(&nome)
}

/// `_parseDocImport`: `@docImport ` seguido de um import (a URI entre
/// aspas).
fn doc_import(content: &str, i: usize) -> bool {
    let Some(rest) = content[i..].strip_prefix("@docImport ") else { return false };
    rest.trim_start_matches([' ', '\t', '\n', '\r']).starts_with(['\'', '"'])
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
fn reference_end(b: &[u8], mut i: usize, end_offset: usize) -> usize {
    let letter = |c: u8| c.is_ascii_alphabetic();
    let letter_or_digit = |c: u8| c.is_ascii_alphanumeric();
    if i >= end_offset || !letter(b[i]) {
        return i;
    }
    while i < end_offset && letter_or_digit(b[i]) {
        i += 1;
    }
    if i >= end_offset || b[i] != b'.' {
        return i;
    }
    i += 1;
    if i >= end_offset || !letter(b[i]) {
        return i;
    }
    i += 1;
    while i < end_offset && letter_or_digit(b[i]) {
        i += 1;
    }
    i
}

/// `_isLinkText`.
fn link_text(b: &[u8], right: usize, can_be_link_reference: bool) -> bool {
    let mut i = right + 1;
    if i >= b.len() {
        return false;
    }
    let mut c = b[i];
    if c == b'(' {
        return true;
    }
    if can_be_link_reference && c == b':' {
        return true;
    }
    while whitespace(c) {
        i += 1;
        if i >= b.len() {
            return false;
        }
        c = b[i];
    }
    c == b'['
}

/// `_parseReferences` numa linha.
fn line_references(content: &str, shift: usize, output: &mut Vec<DocReference>) {
    let b = content.as_bytes();
    let end_offset = b.len();
    let mut i = 0;
    let mut only_whitespace = true;
    while i < end_offset {
        let c = b[i];
        if c == b'[' {
            i += 1;
            if i < end_offset && b[i] == b':' {
                // `[:código:]` antigo.
                match content[i + 1..].find(":]") {
                    Some(k) => i = i + 1 + k + 1,
                    None => break,
                }
                if i > end_offset {
                    break;
                }
            } else {
                let start_offset = i;
                let right = match content[i..].find(']') {
                    Some(k) => i + k,
                    None => reference_end(b, start_offset, end_offset),
                };
                i = right;
                if !link_text(b, right, only_whitespace)
                    && let Some(r) = one_reference(&content[start_offset..right], shift + start_offset)
                {
                    output.push(r);
                }
            }
            only_whitespace = false;
        } else if c == b'`' {
            if let Some(k) = content[i + 1..].find('`') {
                i = i + 1 + k;
            }
            only_whitespace = false;
        } else if !whitespace(c) {
            only_whitespace = false;
        }
        i += 1;
    }
}

/// Um token do scanner do Dart, para o que a referência precisa.
#[derive(Debug, Clone)]
enum Tk {
    /// Identificador ou palavra-chave (o texto decide).
    Word(Span, String),
    Dot,
    Operator(Span, String),
    Other,
}

/// O scanner do Dart sobre o texto de uma referência: `None` quando ele
/// acusa erro (caractere inválido, string sem fechar).
fn scan(texto: &str, shift: usize) -> Option<Vec<Tk>> {
    let b = texto.as_bytes();
    let mut v = Vec::new();
    let mut i = 0;
    // Os operadores, do maior ao menor (o scanner consome o mais longo).
    const SIGNS: &[&str] = &[
        ">>>=", "...?", "<<=", ">>=", ">>>", "~/=", "??=", "...", "&&=", "||=", "==", "!=", "<=", ">=", "<<", ">>", "~/", "[]=", "[]", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "&&", "||", "++", "--", "??", "?.", "=>", "+", "-", "*",
        "/", "%", "~", "<", ">", "&", "|", "^", "!", "?", "=", ":", ";", ",", "(", ")", "[", "]", "{", "}", "@", "#",
    ];
    while i < b.len() {
        let c = b[i];
        if whitespace(c) {
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
            let start_ = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
            v.push(Tk::Word(Span { start: shift + start_, end: shift + i }, texto[start_..i].to_string()));
            continue;
        }
        if c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'.' || b[i] == b'_') {
                i += 1;
            }
            v.push(Tk::Other);
            continue;
        }
        if c == b'\'' || c == b'"' {
            let closes = texto[i + 1..].find(c as char)?;
            i = i + 1 + closes + 1;
            v.push(Tk::Other);
            continue;
        }
        if c == b'.' {
            v.push(Tk::Dot);
            i += 1;
            continue;
        }
        let sinal = SIGNS.iter().find(|s| b[i..].starts_with(s.as_bytes()))?;
        v.push(Tk::Operator(Span { start: shift + i, end: shift + i + sinal.len() }, sinal.to_string()));
        i += sinal.len();
    }
    Some(v)
}

/// `token.isIdentifier`: identificador, palavra embutida ou pseudo.
fn identifier(t: Option<&Tk>) -> bool {
    matches!(t, Some(Tk::Word(_, p)) if !RESERVED_WORDS.contains(&p.as_str()))
}

/// `_parseOneCommentReference`.
fn one_reference(texto: &str, shift: usize) -> Option<DocReference> {
    let tks = scan(texto, shift)?;
    let t = |i: usize| tks.get(i);
    let mut i = 0;
    let mut novo = false;
    if matches!(t(i), Some(Tk::Word(_, p)) if p == "new") {
        novo = true;
        i += 1;
    }
    let mut partes: Vec<(Span, String)> = Vec::new();
    if identifier(t(i)) && matches!(t(i + 1), Some(Tk::Dot)) {
        let mut second = i;
        let mut dot = i + 1;
        if identifier(t(dot + 1)) && matches!(t(dot + 2), Some(Tk::Dot)) {
            if let Some(Tk::Word(s, p)) = t(second) {
                partes.push((*s, p.clone()));
            }
            second = dot + 1;
            dot += 2;
        }
        if let Some(Tk::Word(s, p)) = t(second) {
            partes.push((*s, p.clone()));
        }
        i = dot + 1;
        // `new` depois do `.` vira identificador (o construtor sem nome).
    }
    // Sem nada depois: o identificador sintético da recuperação (vazio, não
    // resolve); os prefixos antes dele são referências.
    if t(i).is_none() {
        let end_offset = partes.last()?.0.end;
        partes.push((Span { start: end_offset + 1, end: end_offset + 1 }, String::new()));
        return Some(DocReference { novo, partes });
    }
    let mut operator_word = None;
    if matches!(t(i), Some(Tk::Word(_, p)) if p == "operator") {
        operator_word = Some(i);
        i += 1;
    }
    if let Some(Tk::Operator(s, op)) = t(i)
        && OPERATORS.contains(&op.as_str())
    {
        if t(i + 1).is_none() {
            partes.push((*s, op.clone()));
            return Some(DocReference { novo, partes });
        }
        return None;
    }
    let j = operator_word.unwrap_or(i);
    if t(j + 1).is_some() {
        return None;
    }
    let after_dot = !partes.is_empty();
    match t(j) {
        Some(Tk::Word(s, p)) if identifier(t(j)) || (after_dot && p == "new") => {
            partes.push((*s, p.clone()));
            Some(DocReference { novo, partes })
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
        let start_offset = c.dart_doc(fonte, pos).unwrap();
        let tokens: Vec<Span> = c.antes_de(fonte, pos).into_iter().filter(|s| s.start >= start_offset.start).collect();
        referencias(fonte, &tokens).into_iter().map(|r| r.partes.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join(".")).collect()
    }

    #[test]
    fn like_the_doc_comment_builder() {
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
