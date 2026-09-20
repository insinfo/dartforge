use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_syntax::{Token, TokenKind};

/// Tokenizes the supported subset without copying lexemes.
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
        if matches!(bytes.get(i..i + 2), Some(b"++" | b"--")) {
            return Err(Diagnostic::new(
                "increment and decrement are not supported",
                Span {
                    start,
                    end: start + 2,
                },
            ));
        }
        let kind = if b.is_ascii_alphabetic() || b == b'_' {
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
            i += 1;
            let content_start = i;
            while i < bytes.len() && bytes[i] != b {
                if matches!(bytes[i], b'\\' | b'$' | b'\n' | b'\r') {
                    return Err(Diagnostic::new(
                        "escapes, interpolation and multiline strings are not supported yet",
                        Span {
                            start: i,
                            end: i + 1,
                        },
                    ));
                }
                i += 1;
            }
            if i == bytes.len() {
                return Err(Diagnostic::new(
                    "unterminated string",
                    Span { start, end: i },
                ));
            }
            let value = &source[content_start..i];
            i += 1;
            TokenKind::String(value)
        } else if b"(){};,".contains(&b) {
            i += 1;
            TokenKind::Symbol(b as char)
        } else if matches!(
            bytes.get(i..i + 2),
            Some(b"==" | b"!=" | b"<=" | b">=" | b"&&" | b"||")
        ) {
            i += 2;
            TokenKind::Operator(&source[start..i])
        } else if b"=+-*!<>".contains(&b) {
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

#[cfg(test)]
mod tests {
    use super::*;
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
        for source in ["1 / 2", "'$x'", "'\\n'", "'a\nb'", "a & b", "a | b", "1.5"] {
            assert!(lex(source).is_err(), "{source}");
        }
    }
}
