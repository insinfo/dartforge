//! Tokenização do subconjunto Dart 3.6.2 com intervalos medidos em bytes.
//!
//! Literais de string interpolados viram uma sequência de tokens em vez de um
//! token único: o trecho literal de abertura, os tokens da expressão e o trecho
//! literal seguinte. A varredura continua em passagem única porque o lexer
//! guarda numa pilha o literal suspenso por `${`; a chave correspondente
//! devolve o controle ao mesmo literal sem reler nada.
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_syntax::{Token, TokenKind};

/// Literal de string suspenso por `${`, aguardando a chave correspondente.
#[derive(Debug, Clone, Copy)]
struct Interpolation {
    /// Chaves `{` ainda abertas na expressão; a interpolação fecha em zero.
    depth: usize,
    /// Aspa que delimita o literal suspenso.
    quote: u8,
    /// Literal de aspas triplas, que aceita quebras de linha no corpo.
    triple: bool,
    /// Início do literal, usado no diagnóstico de string não terminada.
    start: usize,
}

/// Consome o expoente `e[+-]dígitos` de um literal double quando bem formado.
///
/// `1e3`, `1E+3` e `1.5e-3` são doubles; um `e` sem dígitos (`1e`, `1e+`)
/// permanece para os tokens seguintes e falha adiante com o diagnóstico
/// da posição de uso, como no oráculo Dart.
fn scan_exponent(bytes: &[u8], i: &mut usize) {
    let mut end = *i;
    if !matches!(bytes.get(end), Some(b'e' | b'E')) {
        return;
    }
    end += 1;
    if matches!(bytes.get(end), Some(b'+' | b'-')) {
        end += 1;
    }
    if !matches!(bytes.get(end), Some(next) if next.is_ascii_digit()) {
        return;
    }
    while matches!(bytes.get(end), Some(next) if next.is_ascii_digit()) {
        end += 1;
    }
    *i = end;
}

/// Divide o código em tokens sem copiar seus lexemas.
///
/// # Exemplos
///
///     let tokens = dartforge_lexer::lex("var x = 1;").unwrap();
///     assert_eq!(tokens.len(), 5);
///
/// # Erros
///
/// Retorna diagnóstico para caracteres não suportados, comentários abertos,
/// strings incompletas, quebra de linha em string de aspas simples e `$` sem
/// identificador nem `{` em seguida.
pub fn lex(source: &str) -> Result<Vec<Token<'_>>, Diagnostic> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut interpolations: Vec<Interpolation> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if bytes.get(i..i + 2) == Some(b"//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if bytes.get(i..i + 2) == Some(b"/*") {
            i += 2;
            let mut depth = 1usize;
            while i < bytes.len() && depth != 0 {
                if bytes.get(i..i + 2) == Some(b"/*") {
                    depth += 1;
                    i += 2;
                } else if bytes.get(i..i + 2) == Some(b"*/") {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if depth != 0 {
                return Err(Diagnostic::new(
                    "unterminated block comment",
                    Span { start, end: i },
                ));
            }
            continue;
        }
        if b == b'r' && matches!(bytes.get(i + 1), Some(b'\'' | b'"')) {
            i += 1;
            scan_string(
                source,
                &mut i,
                true,
                start,
                &mut tokens,
                &mut interpolations,
            )?;
            continue;
        }
        if b == b'\'' || b == b'"' {
            scan_string(
                source,
                &mut i,
                false,
                start,
                &mut tokens,
                &mut interpolations,
            )?;
            continue;
        }
        // A chave que fecha a interpolação devolve o controle ao literal suspenso;
        // as demais apenas acompanham a profundidade e seguem como símbolo comum.
        if !interpolations.is_empty() && matches!(b, b'{' | b'}') {
            let top = interpolations.len() - 1;
            if b == b'{' {
                interpolations[top].depth += 1;
            } else if interpolations[top].depth == 0 {
                let context = interpolations.pop().expect("pilha consultada acima");
                i += 1;
                scan_body(
                    source,
                    &mut i,
                    &context,
                    false,
                    false,
                    start,
                    &mut tokens,
                    &mut interpolations,
                )?;
                continue;
            } else {
                interpolations[top].depth -= 1;
            }
        }
        let kind = if b.is_ascii_alphabetic() || matches!(b, b'_' | b'$') {
            i += 1;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'$'))
            {
                i += 1;
            }
            TokenKind::Word(&source[start..i])
        } else if b.is_ascii_digit() {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            // Fração decimal: `1.0` e `1.5e-3` são um único literal double.
            // `1..campo` continua sendo cascata e `1.nome`, acesso a membro:
            // só há fração quando o ponto é seguido de dígito.
            if bytes.get(i) == Some(&b'.')
                && matches!(bytes.get(i + 1), Some(next) if next.is_ascii_digit())
            {
                i += 2;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            } else if bytes.get(i) == Some(&b'.')
                && !matches!(
                    bytes.get(i + 1),
                    Some(b'.') | Some(b'a'..=b'z') | Some(b'A'..=b'Z') | Some(b'_' | b'$')
                )
            {
                // Oráculo Dart 3.6.2/3.13.4: `1.` é inválido (`1.nome` e `1..`
                // continuam válidos e seguem como inteiro + símbolo).
                return Err(Diagnostic::new(
                    "invalid double literal: '.' must be followed by a digit",
                    Span { start, end: i + 1 },
                ));
            }
            scan_exponent(bytes, &mut i);
            TokenKind::Number(&source[start..i])
        } else if b == b'.' && matches!(bytes.get(i + 1), Some(next) if next.is_ascii_digit()) {
            // Oráculo Dart 3.6.2/3.13.4: `.5` vale 0.5 e chega aqui como
            // literal double (o ponto nunca é símbolo isolado antes de dígito).
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            scan_exponent(bytes, &mut i);
            TokenKind::Number(&source[start..i])
        } else if bytes.get(i..i + 4) == Some(b"...?") {
            // Espalhamento null-aware: precisa vir antes de `...` e de `..`.
            i += 4;
            TokenKind::Operator(&source[start..i])
        } else if bytes.get(i..i + 3) == Some(b"...") {
            i += 3;
            TokenKind::Operator(&source[start..i])
        } else if bytes.get(i..i + 3) == Some(b"?..") {
            i += 3;
            TokenKind::Operator(&source[start..i])
        } else if bytes.get(i..i + 2) == Some(b"..") {
            i += 2;
            TokenKind::Operator(&source[start..i])
        } else if b"(){};,.[]:@".contains(&b) {
            i += 1;
            TokenKind::Symbol(b as char)
        } else if matches!(
            bytes.get(i..i + 2),
            Some(
                b"??"
                    | b"=>"
                    | b"=="
                    | b"!="
                    | b"<="
                    | b">="
                    | b"&&"
                    | b"||"
                    | b"++"
                    | b"--"
                    | b"+="
                    | b"-="
                    | b"*="
                    | b"~/"
                    | b"<<"
            )
        ) {
            i += 2;
            TokenKind::Operator(&source[start..i])
        } else if b"=+-*!<>?%/&|^~".contains(&b) {
            i += 1;
            TokenKind::Operator(&source[start..i])
        } else {
            return Err(Diagnostic::new(
                "unsupported token",
                Span {
                    start,
                    end: start + source[start..].chars().next().unwrap().len_utf8(),
                },
            ));
        };
        tokens.push(Token {
            kind,
            span: Span { start, end: i },
        });
    }
    if let Some(open) = interpolations.last() {
        return Err(Diagnostic::new(
            "unterminated string interpolation",
            Span {
                start: open.start,
                end: bytes.len(),
            },
        ));
    }
    Ok(tokens)
}

/// Abre um literal de string e delega a varredura do corpo.
///
/// Reconhece as aspas triplas e aplica ali mesmo a regra do Dart que descarta a
/// primeira linha quando ela só tem espaços: é recorte de índice, sem cópia.
fn scan_string<'a>(
    source: &'a str,
    i: &mut usize,
    raw: bool,
    start: usize,
    tokens: &mut Vec<Token<'a>>,
    interpolations: &mut Vec<Interpolation>,
) -> Result<(), Diagnostic> {
    let bytes = source.as_bytes();
    let quote = bytes[*i];
    let triple = bytes.get(*i..*i + 3) == Some(&[quote, quote, quote]);
    *i += if triple { 3 } else { 1 };
    if triple {
        skip_leading_blank_line(bytes, i);
    }
    let context = Interpolation {
        depth: 0,
        quote,
        triple,
        start,
    };
    scan_body(
        source,
        i,
        &context,
        raw,
        true,
        start,
        tokens,
        interpolations,
    )
}

/// Descarta a primeira linha de uma string tripla quando ela só tem brancos.
///
/// A regra do Dart 3.6.2 é léxica: vale para espaço e tabulação escritos na
/// fonte, nunca para o resultado de um escape, e remove também o terminador,
/// que pode ser LF, CR ou CRLF.
fn skip_leading_blank_line(bytes: &[u8], i: &mut usize) {
    let mut end = *i;
    while matches!(bytes.get(end), Some(b' ' | b'\t')) {
        end += 1;
    }
    match bytes.get(end) {
        Some(b'\n') => *i = end + 1,
        Some(b'\r') => {
            *i = if bytes.get(end + 1) == Some(&b'\n') {
                end + 2
            } else {
                end + 1
            }
        }
        _ => {}
    }
}

/// Percorre o corpo de um literal emitindo trechos literais e interpolações.
///
/// `first` indica que o trecho corrente abre o literal, o que decide entre
/// `String`/`StringStart` e `StringEnd`/`StringMid`. A função devolve o controle
/// ao laço principal assim que encontra `${`, deixando na pilha o contexto que
/// permite retomar o mesmo literal depois da chave correspondente. Nenhum
/// trecho é copiado: todos são emprestados da fonte e só o parser decide se
/// precisa decodificar escapes.
#[allow(clippy::too_many_arguments)]
fn scan_body<'a>(
    source: &'a str,
    i: &mut usize,
    context: &Interpolation,
    raw: bool,
    mut first: bool,
    mut token_start: usize,
    tokens: &mut Vec<Token<'a>>,
    interpolations: &mut Vec<Interpolation>,
) -> Result<(), Diagnostic> {
    let bytes = source.as_bytes();
    let quote = context.quote;
    loop {
        let chunk_start = *i;
        loop {
            let Some(&b) = bytes.get(*i) else {
                return Err(Diagnostic::new(
                    "unterminated string",
                    Span {
                        start: context.start,
                        end: bytes.len(),
                    },
                ));
            };
            if b == quote
                && (!context.triple || bytes.get(*i..*i + 3) == Some(&[quote, quote, quote]))
            {
                let text = &source[chunk_start..*i];
                *i += if context.triple { 3 } else { 1 };
                tokens.push(Token {
                    kind: closing_kind(text, context.triple, raw, first),
                    span: Span {
                        start: token_start,
                        end: *i,
                    },
                });
                return Ok(());
            }
            if !context.triple && matches!(b, b'\n' | b'\r') {
                return Err(Diagnostic::new(
                    "single-quoted strings must end on the same line",
                    Span {
                        start: *i,
                        end: *i + 1,
                    },
                ));
            }
            if !raw && b == b'\\' {
                *i += 1;
                match bytes.get(*i) {
                    None => continue,
                    Some(b'\n' | b'\r') if !context.triple => {
                        return Err(Diagnostic::new(
                            "single-quoted strings must end on the same line",
                            Span {
                                start: *i,
                                end: *i + 1,
                            },
                        ));
                    }
                    Some(_) => {}
                }
            } else if !raw && b == b'$' {
                break;
            }
            *i += source[*i..]
                .chars()
                .next()
                .expect("índice em fronteira de caractere")
                .len_utf8();
        }
        let text = &source[chunk_start..*i];
        let dollar = *i;
        *i += 1;
        match bytes.get(*i) {
            Some(b'{') => {
                *i += 1;
                tokens.push(Token {
                    kind: opening_kind(text, context.triple, first),
                    span: Span {
                        start: token_start,
                        end: *i,
                    },
                });
                interpolations.push(Interpolation {
                    depth: 0,
                    ..*context
                });
                return Ok(());
            }
            // Dart usa IDENTIFIER_NO_DOLLAR aqui: `'$a$b'` são duas interpolações.
            Some(&next) if next.is_ascii_alphabetic() || next == b'_' => {
                tokens.push(Token {
                    kind: opening_kind(text, context.triple, first),
                    span: Span {
                        start: token_start,
                        end: *i,
                    },
                });
                let name_start = *i;
                while matches!(bytes.get(*i), Some(byte) if byte.is_ascii_alphanumeric() || *byte == b'_')
                {
                    *i += 1;
                }
                tokens.push(Token {
                    kind: TokenKind::InterpolatedName(&source[name_start..*i]),
                    span: Span {
                        start: name_start,
                        end: *i,
                    },
                });
                first = false;
                token_start = *i;
            }
            _ => {
                return Err(Diagnostic::new(
                    "a '$' inside a string must be followed by an identifier or '{'",
                    Span {
                        start: dollar,
                        end: dollar + 1,
                    },
                ));
            }
        }
    }
}

/// Classifica o trecho literal que fecha o literal de string.
fn closing_kind(text: &str, triple: bool, raw: bool, first: bool) -> TokenKind<'_> {
    match (first, triple) {
        (false, multiline) => TokenKind::StringEnd { text, multiline },
        (true, true) => TokenKind::MultilineString { text, raw },
        (true, false) if raw => TokenKind::RawString(text),
        (true, false) => TokenKind::String(text),
    }
}

/// Classifica o trecho literal que antecede uma interpolação.
fn opening_kind(text: &str, multiline: bool, first: bool) -> TokenKind<'_> {
    if first {
        TokenKind::StringStart { text, multiline }
    } else {
        TokenKind::StringMid { text, multiline }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    /// Operadores de cascata são indivisíveis e seus spans usam bytes da fonte.
    #[test]
    fn cascade_operators_and_member_dots() {
        let source = "'é';a?..field..method().value;1..isEven";
        let tokens = lex(source).unwrap();
        let cascades: Vec<_> = tokens
            .iter()
            .filter(|token| matches!(token.kind, TokenKind::Operator(".." | "?..")))
            .collect();
        assert_eq!(cascades.len(), 3);
        for token in cascades {
            let TokenKind::Operator(operator) = token.kind else {
                unreachable!()
            };
            assert_eq!(&source[token.span.start..token.span.end], operator);
        }
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::Symbol('.'))
        );
    }
    #[test]
    fn raw_strings_and_escaped_delimiters() {
        let source = r#"'olá\'fim' r'\n$x\' "\$""#;
        let tokens = lex(source).unwrap();
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, TokenKind::String(r"olá\'fim"));
        assert_eq!(tokens[1].kind, TokenKind::RawString(r"\n$x\"));
        for token in tokens {
            assert!(source.get(token.span.start..token.span.end).is_some());
        }
        for source in ["'a\nb'", "r'a\nb'", "'a\\", "'a\\'", "'a$ b'", "'${x'"] {
            assert!(lex(source).is_err(), "{source}");
        }
    }
    /// Uma string interpolada vira trechos literais emprestados e tokens comuns.
    #[test]
    fn interpolation_splits_into_chunks() {
        let tokens = lex("'a $nome.campo ${obj.campo}!'").unwrap();
        assert_eq!(
            tokens.iter().map(|token| token.kind).collect::<Vec<_>>(),
            vec![
                TokenKind::StringStart {
                    text: "a ",
                    multiline: false
                },
                TokenKind::InterpolatedName("nome"),
                TokenKind::StringMid {
                    text: ".campo ",
                    multiline: false
                },
                TokenKind::Word("obj"),
                TokenKind::Symbol('.'),
                TokenKind::Word("campo"),
                TokenKind::StringEnd {
                    text: "!",
                    multiline: false
                },
            ]
        );
    }
    /// Chaves de literais internos não encerram a interpolação antes da hora.
    #[test]
    fn nested_braces_and_strings_close_in_order() {
        let tokens = lex("'${ {'k': '${x}'} }'").unwrap();
        assert!(matches!(
            tokens.first().map(|token| token.kind),
            Some(TokenKind::StringStart { text: "", .. })
        ));
        assert!(matches!(
            tokens.last().map(|token| token.kind),
            Some(TokenKind::StringEnd { text: "", .. })
        ));
        assert_eq!(
            tokens
                .iter()
                .filter(|token| matches!(token.kind, TokenKind::Symbol('{' | '}')))
                .count(),
            2
        );
    }
    /// A primeira linha só de brancos some e o escape `\$` não interpola.
    #[test]
    fn triple_quoted_strings_drop_the_first_blank_line() {
        let tokens = lex("'''  \nlinha\n''' r'''a\\n''' '\\$x'").unwrap();
        assert_eq!(
            tokens.iter().map(|token| token.kind).collect::<Vec<_>>(),
            vec![
                TokenKind::MultilineString {
                    text: "linha\n",
                    raw: false
                },
                TokenKind::MultilineString {
                    text: "a\\n",
                    raw: true
                },
                TokenKind::String("\\$x"),
            ]
        );
        assert_eq!(lex("'''a'''").unwrap().len(), 1);
        assert_eq!(lex("'''\n'''").unwrap()[0].span, Span { start: 0, end: 7 });
    }
    #[test]
    fn update_tokens_use_maximal_munch() {
        let tokens = lex("i++ --i i+=2 i-=1 i*=3").unwrap();
        let operators = tokens
            .iter()
            .filter_map(|token| match token.kind {
                TokenKind::Operator(op) => Some(op),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(operators, ["++", "--", "+=", "-=", "*="]);
    }
    #[test]
    fn operators_and_nested_comments() {
        let tokens = lex("12/* outer /* nested */ end */<=3 && true != false").unwrap();
        assert_eq!(
            tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![
                TokenKind::Number("12"),
                TokenKind::Operator("<="),
                TokenKind::Number("3"),
                TokenKind::Operator("&&"),
                TokenKind::Word("true"),
                TokenKind::Operator("!="),
                TokenKind::Word("false")
            ]
        );
        assert!(lex("/* never closed").is_err());
    }
    #[test]
    fn unicode_byte_spans_and_unsupported_features() {
        let tokens = lex("'olá' + 1").unwrap();
        assert_eq!(tokens[0].span, Span { start: 0, end: 6 });
        assert_eq!(tokens[1].span, Span { start: 7, end: 8 });
        let err = lex("é").unwrap_err();
        assert_eq!(err.span, Span { start: 0, end: 2 });
        for source in ["'a\nb'", "a & b", "a | b", "a ~ b"] {
            assert!(lex(source).is_err(), "{source}");
        }
    }
    #[test]
    fn doubles_division_and_truncating_division() {
        // `1.0`, `1e3` e `1.5e-3` são um único literal; `/` e `~/` viram operadores.
        let tokens = lex("1.0 1e3 1.5e-3 .5 7 ~/ 2 1 / 2").unwrap();
        assert_eq!(
            tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![
                TokenKind::Number("1.0"),
                TokenKind::Number("1e3"),
                TokenKind::Number("1.5e-3"),
                TokenKind::Number(".5"),
                TokenKind::Number("7"),
                TokenKind::Operator("~/"),
                TokenKind::Number("2"),
                TokenKind::Number("1"),
                TokenKind::Operator("/"),
                TokenKind::Number("2"),
            ]
        );
        // `1..campo` é cascata e `1.nome`, acesso a membro: o ponto não fecha double.
        let tokens = lex("1..isEven").unwrap();
        assert_eq!(
            tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![
                TokenKind::Number("1"),
                TokenKind::Operator(".."),
                TokenKind::Word("isEven"),
            ]
        );
        // Oráculo Dart 3.6.2/3.13.4: `1.` é inválido.
        for source in ["var x = 1.;", "var x = 1. ;", "print(1.)"] {
            let err = lex(source).expect_err(source);
            assert_eq!(
                err.message,
                "invalid double literal: '.' must be followed by a digit",
                "{source}"
            );
        }
    }
}
