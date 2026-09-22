//! Lexer de Dart 3.6 completo, em passagem única, sem copiar lexemas.
//!
//! Contrato em [`crate::token`]. Pontos que não são tradução óbvia:
//!
//! * `>` sai sempre sozinho; o parser compõe `>>`, `>=` etc. pela marca
//!   [`Token::glued`]. Isso elimina o retrocesso em `List<List<int>>`.
//! * Strings interpoladas viram sequências `StrBegin … StrMid … StrEnd`. O
//!   lexer guarda uma pilha de literais suspensos por `${`; a `}` que fecha a
//!   interpolação devolve o controle ao literal, contando chaves internas de
//!   expressões como `${ {'a': 1}['a'] }`.
//! * `$nome` dentro de string produz um único token [`Kind::Ident`] entre os
//!   trechos, sem pilha: o identificador termina onde termina o nome.
//! * Números: `1`, `0x1F`, `1.5`, `1e3`, `1.5e-3`. Em Dart `1.` não é double
//!   (é `1` seguido de `.`), e `.5` não é número.
//! * Comentários de bloco aninham; comentários de documentação são descartados
//!   como qualquer comentário (a árvore não os guarda por enquanto).
use crate::text::{DartStr, DartStrBuilder};
use crate::token::{Interp, Keyword, Kind, Op, StrFlags, Token};
use dartforge_diagnostics::{Diagnostic, Span};

/// Literal suspenso por `${`, aguardando a chave correspondente.
#[derive(Debug, Clone, Copy)]
struct Suspended {
    flags: StrFlags,
    /// Chaves abertas dentro da expressão; a interpolação fecha em zero.
    depth: usize,
}

struct Lexer<'s> {
    source: &'s str,
    bytes: &'s [u8],
    pos: usize,
    tokens: Vec<Token>,
    suspended: Vec<Suspended>,
}

/// Divide a fonte em tokens; o último é sempre [`Kind::Eof`].
///
/// ```
/// use dartforge_frontend::token::{Kind, Op};
/// let tokens = dartforge_frontend::lexer::lex("a >>= 1;").unwrap();
/// let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
/// assert_eq!(kinds, [Kind::Ident, Kind::Op(Op::Gt), Kind::Op(Op::Gt), Kind::Op(Op::Assign), Kind::Int, Kind::Op(Op::Semicolon), Kind::Eof]);
/// assert!(tokens[1].glued && tokens[2].glued && !tokens[3].glued);
/// ```
///
/// # Erros
/// Devolve o primeiro diagnóstico léxico: caractere inesperado, comentário de
/// bloco ou string não terminados, quebra de linha em string simples.
pub fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> {
    let mut lexer = Lexer {
        source,
        bytes: source.as_bytes(),
        pos: 0,
        tokens: Vec::with_capacity(source.len() / 4 + 8),
        suspended: Vec::new(),
    };
    lexer.run()?;
    Ok(lexer.tokens)
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b'$'
}

fn is_ident_part(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

impl<'s> Lexer<'s> {
    fn at(&self, offset: usize) -> u8 {
        self.bytes.get(self.pos + offset).copied().unwrap_or(0)
    }

    fn starts_with(&self, text: &str) -> bool {
        self.bytes[self.pos..].starts_with(text.as_bytes())
    }

    fn push(&mut self, kind: Kind, start: usize) {
        self.tokens.push(Token {
            kind,
            span: Span {
                start,
                end: self.pos,
            },
            glued: false,
        });
    }

    fn error(&self, start: usize, message: &str) -> Diagnostic {
        Diagnostic::new(
            message,
            Span {
                start,
                end: self.pos.max(start + 1).min(self.bytes.len().max(start + 1)),
            },
        )
    }

    fn run(&mut self) -> Result<(), Diagnostic> {
        if self.starts_with("#!") {
            let start = 0;
            while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' {
                self.pos += 1;
            }
            self.push(Kind::ScriptTag, start);
        }
        while self.pos < self.bytes.len() {
            let start = self.pos;
            let b = self.bytes[self.pos];
            // Espaço e comentários quebram a colagem do token anterior.
            if b.is_ascii_whitespace() {
                self.pos += 1;
                self.unglue();
                continue;
            }
            if self.starts_with("//") {
                while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' {
                    self.pos += 1;
                }
                self.unglue();
                continue;
            }
            if self.starts_with("/*") {
                self.block_comment()?;
                self.unglue();
                continue;
            }
            // O token anterior é colado a este se não houve espaço entre eles.
            if let Some(last) = self.tokens.last_mut() {
                last.glued = last.span.end == start;
            }
            if b == b'r' && matches!(self.at(1), b'\'' | b'"') {
                self.pos += 1;
                self.string(start, true)?;
                continue;
            }
            if b == b'\'' || b == b'"' {
                self.string(start, false)?;
                continue;
            }
            if is_ident_start(b) {
                while self.pos < self.bytes.len() && is_ident_part(self.bytes[self.pos]) {
                    self.pos += 1;
                }
                let text = &self.source[start..self.pos];
                let kind = match Keyword::from_text(text) {
                    Some(keyword) => Kind::Keyword(keyword),
                    None => Kind::Ident,
                };
                self.push(kind, start);
                continue;
            }
            // `.5` é literal `double` na gramática (`'.' DIGIT+ EXPONENT?`);
            // `a.5` não existe em Dart, então o `.` seguido de dígito é sempre
            // número. `..5` não ocorre: `..` já foi lido como cascata.
            if b.is_ascii_digit() || (b == b'.' && self.at(1).is_ascii_digit()) {
                self.number(start);
                continue;
            }
            if b == b'}' && !self.suspended.is_empty() {
                let top = self.suspended.len() - 1;
                if self.suspended[top].depth == 0 {
                    // Fecha a interpolação: o literal continua a partir daqui.
                    let flags = self.suspended.pop().expect("pilha conferida").flags;
                    self.pos += 1;
                    self.string_tail(start, flags)?;
                    continue;
                }
                self.suspended[top].depth -= 1;
            } else if b == b'{' && !self.suspended.is_empty() {
                let top = self.suspended.len() - 1;
                self.suspended[top].depth += 1;
            }
            if b >= 0x80 {
                // Dart só aceita identificadores ASCII; qualquer outro byte
                // fora de string ou comentário é erro.
                let ch = self.source[start..].chars().next().unwrap_or('\u{FFFD}');
                self.pos += ch.len_utf8();
                return Err(self.error(start, &format!("caractere inesperado '{ch}'")));
            }
            self.operator(start)?;
        }
        if let Some(last) = self.tokens.last_mut() {
            last.glued = false;
        }
        self.tokens.push(Token {
            kind: Kind::Eof,
            span: Span {
                start: self.bytes.len(),
                end: self.bytes.len(),
            },
            glued: false,
        });
        Ok(())
    }

    fn unglue(&mut self) {
        if let Some(last) = self.tokens.last_mut() {
            last.glued = false;
        }
    }

    fn block_comment(&mut self) -> Result<(), Diagnostic> {
        let start = self.pos;
        self.pos += 2;
        let mut depth = 1usize;
        while self.pos < self.bytes.len() && depth != 0 {
            if self.starts_with("/*") {
                depth += 1;
                self.pos += 2;
            } else if self.starts_with("*/") {
                depth -= 1;
                self.pos += 2;
            } else {
                self.pos += 1;
            }
        }
        if depth != 0 {
            return Err(self.error(start, "comentário de bloco não terminado"));
        }
        Ok(())
    }

    fn number(&mut self, start: usize) {
        if self.at(0) == b'0' && matches!(self.at(1), b'x' | b'X') && self.at(2).is_ascii_hexdigit()
        {
            self.pos += 2;
            while self.at(0).is_ascii_hexdigit() {
                self.pos += 1;
            }
            self.push(Kind::Int, start);
            return;
        }
        while self.at(0).is_ascii_digit() {
            self.pos += 1;
        }
        let mut is_double = false;
        if self.at(0) == b'.' && self.at(1).is_ascii_digit() {
            is_double = true;
            self.pos += 1;
            while self.at(0).is_ascii_digit() {
                self.pos += 1;
            }
        }
        if matches!(self.at(0), b'e' | b'E') {
            let mut end = self.pos + 1;
            if matches!(self.bytes.get(end), Some(b'+' | b'-')) {
                end += 1;
            }
            if matches!(self.bytes.get(end), Some(d) if d.is_ascii_digit()) {
                while matches!(self.bytes.get(end), Some(d) if d.is_ascii_digit()) {
                    end += 1;
                }
                self.pos = end;
                is_double = true;
            }
        }
        self.push(if is_double { Kind::Double } else { Kind::Int }, start);
    }

    /// Lê um literal de string a partir da aspa de abertura.
    fn string(&mut self, start: usize, raw: bool) -> Result<(), Diagnostic> {
        let quote = self.bytes[self.pos];
        let triple = self.at(1) == quote && self.at(2) == quote;
        self.pos += if triple { 3 } else { 1 };
        let flags = StrFlags { raw, triple, quote };
        self.string_body(start, flags, true)
    }

    /// Continua um literal após uma interpolação (`}` ou fim de `$nome`).
    fn string_tail(&mut self, start: usize, flags: StrFlags) -> Result<(), Diagnostic> {
        self.string_body(start, flags, false)
    }

    /// Lê o corpo até a aspa de fechamento ou a próxima interpolação.
    ///
    /// `first` indica que este trecho começa na aspa de abertura (então o
    /// token é `Str`/`StrBegin`; senão `StrEnd`/`StrMid`).
    fn string_body(
        &mut self,
        start: usize,
        flags: StrFlags,
        first: bool,
    ) -> Result<(), Diagnostic> {
        loop {
            let Some(&b) = self.bytes.get(self.pos) else {
                return Err(self.error(start, "string não terminada"));
            };
            if b == flags.quote {
                if flags.triple {
                    if self.at(1) == flags.quote && self.at(2) == flags.quote {
                        self.pos += 3;
                        break;
                    }
                    self.pos += 1;
                    continue;
                }
                self.pos += 1;
                break;
            }
            if b == b'\n' && !flags.triple {
                return Err(self.error(start, "quebra de linha em string de aspas simples"));
            }
            if b == b'\\' && !flags.raw {
                // Um escape nunca termina a string: pula o par inteiro. Escapes
                // `\u{...}` são validados na decodificação, não aqui.
                self.pos += 1;
                if self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' {
                    self.pos += 1;
                }
                continue;
            }
            if b == b'$' && !flags.raw {
                if self.at(1) == b'{' {
                    let end = self.pos;
                    self.pos += 2;
                    self.tokens.push(Token {
                        kind: if first {
                            Kind::StrBegin(flags, Interp::Brace)
                        } else {
                            Kind::StrMid(flags, Interp::Brace)
                        },
                        span: Span {
                            start,
                            end: self.pos,
                        },
                        glued: false,
                    });
                    let _ = end;
                    self.suspended.push(Suspended { flags, depth: 0 });
                    return Ok(());
                }
                if is_ident_start(self.at(1)) && self.at(1) != b'$' {
                    self.pos += 1;
                    self.tokens.push(Token {
                        kind: if first {
                            Kind::StrBegin(flags, Interp::Ident)
                        } else {
                            Kind::StrMid(flags, Interp::Ident)
                        },
                        span: Span {
                            start,
                            end: self.pos,
                        },
                        glued: false,
                    });
                    let ident_start = self.pos;
                    while self.pos < self.bytes.len()
                        && (self.bytes[self.pos].is_ascii_alphanumeric()
                            || self.bytes[self.pos] == b'_')
                    {
                        self.pos += 1;
                    }
                    self.tokens.push(Token {
                        kind: Kind::Ident,
                        span: Span {
                            start: ident_start,
                            end: self.pos,
                        },
                        glued: false,
                    });
                    // O trecho seguinte recomeça no byte após o nome.
                    let tail_start = self.pos;
                    return self.string_tail(tail_start, flags);
                }
                return Err(self.error(self.pos, "'$' precisa de identificador ou '{' em string"));
            }
            self.pos += 1;
        }
        self.tokens.push(Token {
            kind: if first {
                Kind::Str(flags)
            } else {
                Kind::StrEnd(flags)
            },
            span: Span {
                start,
                end: self.pos,
            },
            glued: false,
        });
        Ok(())
    }

    fn operator(&mut self, start: usize) -> Result<(), Diagnostic> {
        // Do mais longo ao mais curto; `>` nunca se combina.
        const TABLE: &[(&str, Op)] = &[
            ("...?", Op::EllipsisQuestion),
            ("~/=", Op::TildeSlashAssign),
            ("<<=", Op::LtLtAssign),
            ("??=", Op::QuestionQuestionAssign),
            ("...", Op::Ellipsis),
            ("?..", Op::QuestionDotDot),
            ("==", Op::EqEq),
            ("!=", Op::BangEq),
            ("<=", Op::LtEq),
            ("<<", Op::LtLt),
            ("&&", Op::AmpAmp),
            ("||", Op::PipePipe),
            ("??", Op::QuestionQuestion),
            ("?.", Op::QuestionDot),
            ("..", Op::DotDot),
            ("=>", Op::Arrow),
            ("++", Op::PlusPlus),
            ("--", Op::MinusMinus),
            ("+=", Op::PlusAssign),
            ("-=", Op::MinusAssign),
            ("*=", Op::StarAssign),
            ("/=", Op::SlashAssign),
            ("%=", Op::PercentAssign),
            ("&=", Op::AmpAssign),
            ("|=", Op::PipeAssign),
            ("^=", Op::CaretAssign),
            ("~/", Op::TildeSlash),
            ("(", Op::LParen),
            (")", Op::RParen),
            ("[", Op::LBracket),
            ("]", Op::RBracket),
            ("{", Op::LBrace),
            ("}", Op::RBrace),
            (",", Op::Comma),
            (";", Op::Semicolon),
            (".", Op::Dot),
            (":", Op::Colon),
            ("?", Op::Question),
            ("!", Op::Bang),
            ("~", Op::Tilde),
            ("@", Op::At),
            ("#", Op::Hash),
            ("=", Op::Assign),
            ("<", Op::Lt),
            (">", Op::Gt),
            ("+", Op::Plus),
            ("-", Op::Minus),
            ("*", Op::Star),
            ("/", Op::Slash),
            ("%", Op::Percent),
            ("&", Op::Amp),
            ("|", Op::Pipe),
            ("^", Op::Caret),
        ];
        for (text, op) in TABLE {
            if self.starts_with(text) {
                self.pos += text.len();
                self.push(Kind::Op(*op), start);
                return Ok(());
            }
        }
        let ch = self.source[start..].chars().next().unwrap_or('\u{FFFD}');
        self.pos += ch.len_utf8();
        Err(self.error(start, &format!("caractere inesperado '{ch}'")))
    }
}

/// Decodifica o conteúdo de um trecho de string (sem as aspas), resolvendo
/// escapes quando `raw` é falso.
///
/// `strip_leading_newline` remove a primeira quebra de linha de uma string
/// tripla, como a especificação exige (§17.7): só o primeiro trecho de um
/// literal triplo a recebe. O resultado é [`DartStr`] porque `\uD800` é
/// literal válido (unidade UTF-16 sem escalar) e `String` não o representa;
/// um par `👭` vira o escalar correspondente.
///
/// ```
/// use dartforge_frontend::lexer::decode_string;
/// assert_eq!(decode_string(r"a\nb", false, false).unwrap(), "a\nb");
/// assert_eq!(decode_string(r"\u{1F600}", false, false).unwrap(), "😀");
/// assert_eq!(decode_string(r"a\nb", true, false).unwrap(), "a\\nb");
/// assert_eq!(decode_string(r"\uD800", false, false).unwrap().utf16_len(), 1);
/// ```
///
/// # Erros
/// Escape `\u`/`\x` malformado ou código fora do intervalo Unicode.
pub fn decode_string(
    content: &str,
    raw: bool,
    strip_leading_newline: bool,
) -> Result<DartStr, String> {
    let mut content = content;
    if strip_leading_newline {
        // §17.7: a primeira linha é removida se só tem espaços/tabs,
        // opcionalmente um `\`, e a quebra de linha.
        let bytes = content.as_bytes();
        let mut i = 0;
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'\\' {
            i += 1;
        }
        if content[i..].starts_with("\r\n") {
            content = &content[i + 2..];
        } else if content[i..].starts_with('\n') {
            content = &content[i + 1..];
        }
    }
    if raw {
        return Ok(DartStr::from(content));
    }
    let mut out = DartStrBuilder::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push_char(c);
            continue;
        }
        let Some(e) = chars.next() else {
            return Err("escape no fim da string".into());
        };
        match e {
            'n' => out.push_char('\n'),
            'r' => out.push_char('\r'),
            't' => out.push_char('\t'),
            'b' => out.push_char('\u{8}'),
            'f' => out.push_char('\u{c}'),
            'v' => out.push_char('\u{b}'),
            'x' => {
                let mut value = 0u32;
                for _ in 0..2 {
                    let d = chars
                        .next()
                        .and_then(|d| d.to_digit(16))
                        .ok_or("escape \\x exige dois dígitos hexadecimais")?;
                    value = value * 16 + d;
                }
                if !out.push_code_point(value) {
                    return Err("código inválido".into());
                }
            }
            'u' => {
                let value = if chars.peek() == Some(&'{') {
                    chars.next();
                    let mut value = 0u32;
                    let mut digits = 0;
                    loop {
                        match chars.next() {
                            Some('}') => break,
                            Some(d) if d.is_ascii_hexdigit() => {
                                digits += 1;
                                if digits > 6 {
                                    return Err("escape \\u{...} com mais de seis dígitos".into());
                                }
                                value = value * 16 + d.to_digit(16).expect("dígito conferido");
                            }
                            _ => return Err("escape \\u{...} malformado".into()),
                        }
                    }
                    if digits == 0 {
                        return Err("escape \\u{} vazio".into());
                    }
                    value
                } else {
                    let mut value = 0u32;
                    for _ in 0..4 {
                        let d = chars
                            .next()
                            .and_then(|d| d.to_digit(16))
                            .ok_or("escape \\u exige quatro dígitos hexadecimais")?;
                        value = value * 16 + d;
                    }
                    value
                };
                if !out.push_code_point(value) {
                    return Err("código Unicode inválido".into());
                }
            }
            other => out.push_char(other),
        }
    }
    Ok(out.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<Kind> {
        lex(source).unwrap().iter().map(|t| t.kind).collect()
    }

    #[test]
    fn palavras_reservadas_e_identificadores() {
        assert_eq!(
            kinds("class abstract _x $y"),
            [
                Kind::Keyword(Keyword::Class),
                Kind::Ident,
                Kind::Ident,
                Kind::Ident,
                Kind::Eof
            ]
        );
    }

    #[test]
    fn numeros() {
        assert_eq!(kinds("1 0x1F 1.5 1e3 1.5e-3 1.foo"), {
            let mut v = vec![
                Kind::Int,
                Kind::Int,
                Kind::Double,
                Kind::Double,
                Kind::Double,
            ];
            v.extend([Kind::Int, Kind::Op(Op::Dot), Kind::Ident, Kind::Eof]);
            v
        });
    }

    #[test]
    fn maior_nunca_se_combina() {
        let tokens = lex("List<List<int>> x; a >>> b; a > b").unwrap();
        let gts: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind == Kind::Op(Op::Gt))
            .map(|t| t.glued)
            .collect();
        assert_eq!(gts, [true, false, true, true, false, false]);
    }

    #[test]
    fn string_simples_e_crua() {
        let tokens = lex(r##"'a' r"b\n" '''c
d''' "e" "##)
        .unwrap();
        assert!(matches!(
            tokens[0].kind,
            Kind::Str(StrFlags {
                raw: false,
                triple: false,
                quote: b'\''
            })
        ));
        assert!(matches!(
            tokens[1].kind,
            Kind::Str(StrFlags { raw: true, .. })
        ));
        assert!(matches!(
            tokens[2].kind,
            Kind::Str(StrFlags { triple: true, .. })
        ));
        assert_eq!(
            &"'a' r\"b\\n\" '''c\nd''' \"e\""[tokens[2].span.start..tokens[2].span.end],
            "'''c\nd'''"
        );
    }

    #[test]
    fn interpolacao_por_nome_e_por_chaves() {
        let source = r#"'a $b c ${d + {1}.length} e'"#;
        let tokens = lex(source).unwrap();
        let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds[0],
            Kind::StrBegin(
                StrFlags {
                    raw: false,
                    triple: false,
                    quote: b'\''
                },
                Interp::Ident
            )
        );
        assert_eq!(kinds[1], Kind::Ident);
        assert_eq!(tokens[1].text(source), "b");
        assert!(matches!(kinds[2], Kind::StrMid(_, Interp::Brace)));
        assert_eq!(tokens[2].text(source), " c ${");
        assert_eq!(kinds[3], Kind::Ident); // d
        assert_eq!(kinds[4], Kind::Op(Op::Plus));
        assert_eq!(kinds[5], Kind::Op(Op::LBrace));
        assert_eq!(kinds[6], Kind::Int);
        assert_eq!(kinds[7], Kind::Op(Op::RBrace));
        assert_eq!(kinds[8], Kind::Op(Op::Dot));
        assert_eq!(kinds[9], Kind::Ident); // length
        assert!(matches!(kinds[10], Kind::StrEnd(_)));
        assert_eq!(tokens[10].text(source), "} e'");
        assert_eq!(kinds[11], Kind::Eof);
    }

    #[test]
    fn interpolacao_aninhada_em_string() {
        let source = r#""${'${x}'}""#;
        let tokens = lex(source).unwrap();
        let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
        assert!(matches!(kinds[0], Kind::StrBegin(_, Interp::Brace)));
        assert!(matches!(kinds[1], Kind::StrBegin(_, Interp::Brace)));
        assert_eq!(kinds[2], Kind::Ident);
        assert!(matches!(kinds[3], Kind::StrEnd(_)));
        assert!(matches!(kinds[4], Kind::StrEnd(_)));
    }

    #[test]
    fn erros_lexicos() {
        assert!(lex("'abc").is_err());
        assert!(lex("/* abc").is_err());
        assert!(lex("'a\nb'").is_err());
        assert!(lex("`").is_err());
    }

    #[test]
    fn operadores_longos() {
        assert_eq!(
            kinds("...? ~/= <<= ??= ... ?.. ?. .. => ~/"),
            [
                Kind::Op(Op::EllipsisQuestion),
                Kind::Op(Op::TildeSlashAssign),
                Kind::Op(Op::LtLtAssign),
                Kind::Op(Op::QuestionQuestionAssign),
                Kind::Op(Op::Ellipsis),
                Kind::Op(Op::QuestionDotDot),
                Kind::Op(Op::QuestionDot),
                Kind::Op(Op::DotDot),
                Kind::Op(Op::Arrow),
                Kind::Op(Op::TildeSlash),
                Kind::Eof
            ]
        );
    }

    #[test]
    fn script_tag() {
        assert_eq!(
            kinds("#!/usr/bin/env dart\nx"),
            [Kind::ScriptTag, Kind::Ident, Kind::Eof]
        );
    }
}
