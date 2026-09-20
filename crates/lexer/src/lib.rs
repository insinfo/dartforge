//! Tokenização do subconjunto Dart 3.6.2 com intervalos medidos em bytes.
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_syntax::{Token, TokenKind};

/// Divide o código em tokens sem copiar seus lexemas.
///
/// # Exemplos
///
///     let tokens = dartforge_lexer::lex("var x = 1;").unwrap();
///     assert_eq!(tokens.len(), 5);
///
/// # Erros
///
/// Retorna diagnóstico para caracteres não suportados, comentários abertos
/// ou strings incompletas, interpoladas ou com múltiplas linhas.
pub fn lex(source: &str) -> Result<Vec<Token<'_>>, Diagnostic> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
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
        let kind = if b == b'r' && matches!(bytes.get(i + 1), Some(b'\'' | b'"')) {
            i += 1;
            scan_string(source, &mut i, true, start)?
        } else if b.is_ascii_alphabetic() || b == b'_' {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            TokenKind::Word(&source[start..i])
        } else if b.is_ascii_digit() {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            TokenKind::Number(&source[start..i])
        } else if b == b'\'' || b == b'"' {
            scan_string(source, &mut i, false, start)?
        } else if b"(){};,.[]".contains(&b) {
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
            )
        ) {
            i += 2;
            TokenKind::Operator(&source[start..i])
        } else if b"=+-*!<>?%".contains(&b) {
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
    Ok(tokens)
}

/// Localiza o fim de uma string simples, preservando escapes para o parser.
fn scan_string<'a>(
    source: &'a str,
    i: &mut usize,
    raw: bool,
    start: usize,
) -> Result<TokenKind<'a>, Diagnostic> {
    let bytes = source.as_bytes();
    let quote = bytes[*i];
    if bytes.get(*i..*i + 3) == Some(&[quote, quote, quote]) {
        return Err(Diagnostic::new(
            "triple-quoted strings are not supported",
            Span { start, end: *i + 3 },
        ));
    }
    *i += 1;
    let content_start = *i;
    while *i < bytes.len() && bytes[*i] != quote {
        if matches!(bytes[*i], b'\n' | b'\r') {
            return Err(Diagnostic::new(
                "multiline strings are not supported",
                Span {
                    start: *i,
                    end: *i + 1,
                },
            ));
        }
        if !raw && bytes[*i] == b'$' {
            return Err(Diagnostic::new(
                "string interpolation is not supported",
                Span {
                    start: *i,
                    end: *i + 1,
                },
            ));
        }
        if !raw && bytes[*i] == b'\\' {
            *i += 1;
            if *i == bytes.len() {
                break;
            }
            if matches!(bytes[*i], b'\n' | b'\r') {
                return Err(Diagnostic::new(
                    "multiline strings are not supported",
                    Span {
                        start: *i,
                        end: *i + 1,
                    },
                ));
            }
        }
        *i += source[*i..].chars().next().unwrap().len_utf8();
    }
    if *i == bytes.len() {
        return Err(Diagnostic::new(
            "unterminated string",
            Span { start, end: *i },
        ));
    }
    let content = &source[content_start..*i];
    *i += 1;
    Ok(if raw {
        TokenKind::RawString(content)
    } else {
        TokenKind::String(content)
    })
}
#[cfg(test)]
mod tests {
    use super::*;
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
        for source in [
            "'''triple'''",
            "r'''triple'''",
            "'a\nb'",
            "r'a\nb'",
            "'$x'",
            "'a\\",
            "'a\\'",
        ] {
            assert!(lex(source).is_err(), "{source}");
        }
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
        for source in ["1 / 2", "'$x'", "'a\nb'", "a & b", "a | b"] {
            assert!(lex(source).is_err(), "{source}");
        }
    }
}
