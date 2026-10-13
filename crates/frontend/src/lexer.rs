//! Complete Dart 3.6 lexer, single pass, without copying lexemes.
//!
//! Contract in [`crate::token`]. Points that are not an obvious translation:
//!
//! * `>` always comes out alone; the parser composes `>>`, `>=` etc. using
//!   the [`Token::glued`] flag. That removes backtracking in
//!   `List<List<int>>`.
//! * Interpolated strings become `StrBegin … StrMid … StrEnd` sequences. The
//!   lexer keeps a stack of literals suspended by `${`; the `}` that closes
//!   the interpolation hands control back to the literal, counting inner
//!   braces of expressions like `${ {'a': 1}['a'] }`.
//! * `$name` inside a string produces a single [`Kind::Ident`] token between
//!   the pieces, without the stack: the identifier ends where the name ends.
//! * Numbers: `1`, `0x1F`, `1.5`, `1e3`, `1.5e-3`. In Dart `1.` is not a
//!   double (it is `1` followed by `.`), and `.5` is not a number.
//! * Block comments nest; doc comments are discarded like any comment (the
//!   tree does not keep them for now).
use crate::text::{DartStr, DartStrBuilder};
use crate::token::{Interp, Keyword, Kind, Op, StrFlags, Token};
use dartforge_diagnostics::{Diagnostic, Span};

/// A literal suspended by `${`, waiting for the matching brace.
#[derive(Debug, Clone, Copy)]
struct Suspended {
    flags: StrFlags,
    /// Braces opened inside the expression; the interpolation closes at zero.
    depth: usize,
}

struct Lexer<'s> {
    source: &'s str,
    bytes: &'s [u8],
    pos: usize,
    tokens: Vec<Token>,
    suspended: Vec<Suspended>,
}

/// Splits the source into tokens; the last one is always [`Kind::Eof`].
///
/// ```
/// use dartforge_frontend::token::{Kind, Op};
/// let tokens = dartforge_frontend::lexer::lex("a >>= 1;").unwrap();
/// let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
/// assert_eq!(kinds, [Kind::Ident, Kind::Op(Op::Gt), Kind::Op(Op::Gt), Kind::Op(Op::Assign), Kind::Int, Kind::Op(Op::Semicolon), Kind::Eof]);
/// assert!(tokens[1].glued && tokens[2].glued && !tokens[3].glued);
/// ```
///
/// # Errors
/// It does not fail today: like fasta's scanner, the lexer goes on after
/// each error (an unterminated string comes out with [`StrFlags::unterminated`],
/// an unexpected character joins an identifier or is skipped, an open block
/// comment runs to the end), and the errors come out in the parser
/// (`parser::recoverable_scanner_errors`).
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
            // Whitespace and comments break the gluing of the previous token.
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
            // The previous token is glued to this one if there was no whitespace
            // between them.
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
            // `.5` is a `double` literal in the grammar (`'.' DIGIT+ EXPONENT?`);
            // `a.5` does not exist in Dart, so a `.` followed by a digit is always a
            // number. `..5` does not occur: `..` was already read as a cascade.
            if b.is_ascii_digit() || (b == b'.' && self.at(1).is_ascii_digit()) {
                self.number(start);
                continue;
            }
            if b == b'}' && !self.suspended.is_empty() {
                let top = self.suspended.len() - 1;
                if self.suspended[top].depth == 0 {
                    // Closes the interpolation: the literal continues from here.
                    let flags = self.suspended.pop().expect("checked stack").flags;
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
                // Dart only accepts ASCII identifiers; any other character outside a
                // string or comment is fasta scanner's `unexpected`, which does not stop.
                let ch = self.source[start..].chars().next().unwrap_or('\u{FFFD}');
                self.unexpected(start, ch);
                continue;
            }
            self.operator(start);
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
        // Without the closer, the comment runs to the end of the file and the
        // `UNTERMINATED_MULTI_LINE_COMMENT` comes out in the parser
        // (`parser::recoverable_scanner_errors`).
        let _ = (start, depth);
        Ok(())
    }

    /// Numeric literal with the token boundaries of fasta's scanner
    /// (`tokenizeNumber`, `tokenizeHex`, `tokenizeFractionPart`), including the
    /// `_` digit separators (Dart 3.6): the `_` stays inside the token where the
    /// scanner puts it, even when misplaced (`100_`, `0x_1`, `3_.14`, `1e_3`) —
    /// the parser reports `UNEXPECTED_SEPARATOR_IN_NUMBER`. `3._14` is `3`, `.`
    /// and the identifier `_14`, as there.
    fn number(&mut self, start: usize) {
        let digit = |b: u8| b.is_ascii_digit();
        // `tokenizeHex`: `0x`/`0X` always opens a hexadecimal; without a digit, the
        // synthetic token is worth 0 and `MISSING_HEX_DIGIT` comes out at the last
        // character (`parser::recoverable_scanner_errors`).
        if self.at(0) == b'0' && matches!(self.at(1), b'x' | b'X') {
            self.pos += 2;
            while self.at(0).is_ascii_hexdigit() || self.at(0) == b'_' {
                self.pos += 1;
            }
            self.push(Kind::Int, start);
            return;
        }
        if self.at(0) == b'.' {
            // `.5`: fraction part directly.
            self.pos += 1;
            self.fraction(start);
            return;
        }
        while digit(self.at(0)) || self.at(0) == b'_' {
            self.pos += 1;
        }
        if matches!(self.at(0), b'e' | b'E') {
            // `tokenizeFractionPart` from the `e`: without a digit in the exponent, the
            // `double` token runs up to the `e` (or the sign) and `MISSING_DIGIT` comes
            // out at its last character.
            self.fraction(start);
            return;
        } else if self.at(0) == b'.' && digit(self.at(1)) {
            self.pos += 1;
            self.fraction(start);
            return;
        }
        self.push(Kind::Int, start);
    }

    /// `tokenizeFractionPart`: digits and `_`, optional exponent; the token is
    /// always `double`.
    fn fraction(&mut self, start: usize) {
        while self.at(0).is_ascii_digit() || self.at(0) == b'_' {
            self.pos += 1;
        }
        if matches!(self.at(0), b'e' | b'E') {
            self.pos += 1;
            while self.at(0) == b'_' {
                self.pos += 1;
            }
            if matches!(self.at(0), b'+' | b'-') {
                self.pos += 1;
            }
            while self.at(0).is_ascii_digit() || self.at(0) == b'_' {
                self.pos += 1;
            }
        }
        self.push(Kind::Double, start);
    }

    /// Reads a string literal starting at the opening quote.
    fn string(&mut self, start: usize, raw: bool) -> Result<(), Diagnostic> {
        let quote = self.bytes[self.pos];
        let triple = self.at(1) == quote && self.at(2) == quote;
        self.pos += if triple { 3 } else { 1 };
        let flags = StrFlags { raw, triple, quote, unterminated: false };
        self.string_body(start, flags, true)
    }

    /// Continues a literal after an interpolation (`}` or end of `$name`).
    fn string_tail(&mut self, start: usize, flags: StrFlags) -> Result<(), Diagnostic> {
        self.string_body(start, flags, false)
    }

    /// Reads the body up to the closing quote or the next interpolation.
    ///
    /// `first` says this piece starts at the opening quote (then the token is
    /// `Str`/`StrBegin`; otherwise `StrEnd`/`StrMid`).
    fn string_body(
        &mut self,
        start: usize,
        flags: StrFlags,
        first: bool,
    ) -> Result<(), Diagnostic> {
        // `unterminatedString` (`abstract_scanner.dart:2055-2067`): at end of
        // file, or at the line break (LF or CR) of a simple string, the piece ends
        // there with the synthetic closer and reading continues at the line break.
        let mut unterminated = false;
        loop {
            let Some(&b) = self.bytes.get(self.pos) else {
                unterminated = true;
                break;
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
            if (b == b'\n' || b == b'\r') && !flags.triple {
                unterminated = true;
                break;
            }
            if b == b'\\' && !flags.raw {
                // An escape never ends the string: skip the whole pair. `\u{...}` escapes
                // are validated when decoding, not here. A line break after the backslash
                // still ends a simple string (`tokenizeSingleLineString` checks the next
                // character).
                self.pos += 1;
                if self.pos < self.bytes.len() && (flags.triple || !matches!(self.bytes[self.pos], b'\n' | b'\r')) {
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
                    // The next piece restarts at the byte after the name.
                    let tail_start = self.pos;
                    return self.string_tail(tail_start, flags);
                }
                // `$` without a name (or `{`): fasta's scanner does not stop; the parser
                // reports `MISSING_IDENTIFIER` at what follows the `$` and goes on (`"$"`,
                // `"$ "`, `"$$x"`). The next piece restarts right after the `$`.
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
                let tail_start = self.pos;
                return self.string_tail(tail_start, flags);
            }
            self.pos += 1;
        }
        let flags = StrFlags { unterminated, ..flags };
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

    fn operator(&mut self, start: usize) {
        // Longest first; `>` never combines.
        const TABLE: &[(&str, Op)] = &[
            ("...?", Op::EllipsisQuestion),
            ("~/=", Op::TildeSlashAssign),
            ("<<=", Op::LtLtAssign),
            ("??=", Op::QuestionQuestionAssign),
            ("...", Op::Ellipsis),
            // `tokenizeEquals`/`tokenizeExclamation`: `===` and `!==` are operators
            // with equality precedence and the `UNSUPPORTED_OPERATOR` error
            // (`parser::recoverable_scanner_errors`).
            ("===", Op::EqEq),
            ("!==", Op::BangEq),
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
                return;
            }
        }
        let ch = self.source[start..].chars().next().unwrap_or('\u{FFFD}');
        self.unexpected(start, ch);
    }

    /// `unexpected` (`abstract_scanner.dart:2020-2048`) with
    /// `buildUnexpectedCharacterToken` (`error_token.dart:25-60`). A control
    /// character and non-ASCII whitespace are just skipped; any other becomes
    /// part of an identifier, together with the identifier glued before it (an
    /// `Ident`, not a keyword) and the ASCII identifier characters after it.
    /// The `ILLEGAL_CHARACTER` of each one comes out in the parser
    /// (`parser::recoverable_scanner_errors`).
    fn unexpected(&mut self, start: usize, ch: char) {
        self.pos += ch.len_utf8();
        if skipped_char(ch) {
            self.unglue();
            return;
        }
        let mut start_offset = start;
        if let Some(t) = self.tokens.last()
            && t.kind == Kind::Ident
            && t.span.end == start
        {
            start_offset = t.span.start;
            self.tokens.pop();
        }
        while self.pos < self.bytes.len() && is_ident_part(self.bytes[self.pos]) {
            self.pos += 1;
        }
        self.push(Kind::Ident, start_offset);
    }
}

/// The unexpected character that fasta's scanner only skips (the error
/// without a token): ASCII control (`AsciiControlCharacterToken`) and
/// non-ASCII whitespace (`NonAsciiWhitespaceToken`). The others become a
/// `NonAsciiIdentifierToken`.
pub(crate) fn skipped_char(ch: char) -> bool {
    (ch as u32) < 0x1f
        || matches!(
            ch as u32,
            0x00A0
                | 0x1680
                | 0x180E
                | 0x2000..=0x200B
                | 0x2028
                | 0x2029
                | 0x202F
                | 0x205F
                | 0x3000
                | 0xFEFF
        )
}

/// The character that joins an identifier without error
/// (`_isIdentifierChar` with the `$`).
pub(crate) fn identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '$'
}

/// Decodes the content of a string piece (without the quotes), resolving
/// escapes when `raw` is false.
///
/// `strip_leading_newline` removes the first line break of a triple string,
/// as the specification requires (§17.7): only the first piece of a triple
/// literal gets it. The result is a [`DartStr`] because `\uD800` is a valid
/// literal (a UTF-16 unit without a scalar) and `String` cannot represent
/// it; a `👭` pair becomes the corresponding scalar.
///
/// ```
/// use dartforge_frontend::lexer::decode_string;
/// assert_eq!(decode_string(r"a\nb", false, false).unwrap(), "a\nb");
/// assert_eq!(decode_string(r"\u{1F600}", false, false).unwrap(), "😀");
/// assert_eq!(decode_string(r"a\nb", true, false).unwrap(), "a\\nb");
/// assert_eq!(decode_string(r"\uD800", false, false).unwrap().utf16_len(), 1);
/// ```
///
/// # Errors
/// A malformed `\u`/`\x` escape or a code point outside the Unicode range.
pub fn decode_string(
    content: &str,
    raw: bool,
    strip_leading_newline: bool,
) -> Result<DartStr, String> {
    let mut content = content;
    if strip_leading_newline {
        // §17.7, `lengthOfOptionalWhitespacePrefix` (`quote.dart:62-86`): the first
        // line is dropped if it only has spaces and tabs, each optionally after a
        // `\`, up to the line break (`\r`, `\r\n` or `\n`).
        let bytes = content.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let mut b = bytes[i];
            if b == b'\\' {
                i += 1;
                let Some(&d) = bytes.get(i) else { break };
                b = d;
            }
            if b == b' ' || b == b'\t' {
                i += 1;
                continue;
            }
            if b == b'\r' {
                if bytes.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
                content = &content[i + 1..];
            } else if b == b'\n' {
                content = &content[i + 1..];
            }
            break;
        }
    }
    // `unescapeCodeUnits` (`quote.dart:186-198`): `\r` and `\r\n` in the body
    // become `\n`, in raw strings too.
    if raw {
        if !content.contains('\r') {
            return Ok(DartStr::from(content));
        }
        return Ok(DartStr::from(content.replace("\r\n", "\n").replace('\r', "\n").as_str()));
    }
    let mut out = DartStrBuilder::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push_char('\n');
            continue;
        }
        if c != '\\' {
            out.push_char(c);
            continue;
        }
        let Some(e) = chars.next() else {
            return Err("escape at end of string".into());
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
                        .ok_or("\\x escape requires two hexadecimal digits")?;
                    value = value * 16 + d;
                }
                if !out.push_code_point(value) {
                    return Err("invalid code point".into());
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
                                    return Err("\\u{...} escape with more than six digits".into());
                                }
                                value = value * 16 + d.to_digit(16).expect("checked digit");
                            }
                            _ => return Err("escape \\u{...} malformado".into()),
                        }
                    }
                    if digits == 0 {
                        return Err("empty \\u{} escape".into());
                    }
                    value
                } else {
                    let mut value = 0u32;
                    for _ in 0..4 {
                        let d = chars
                            .next()
                            .and_then(|d| d.to_digit(16))
                            .ok_or("\\u escape requires four hexadecimal digits")?;
                        value = value * 16 + d;
                    }
                    value
                };
                if !out.push_code_point(value) {
                    return Err("invalid Unicode code point".into());
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
    fn reserved_words_and_identifiers() {
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
    fn numbers() {
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
    fn greater_never_combines() {
        let tokens = lex("List<List<int>> x; a >>> b; a > b").unwrap();
        let gts: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind == Kind::Op(Op::Gt))
            .map(|t| t.glued)
            .collect();
        assert_eq!(gts, [true, false, true, true, false, false]);
    }

    #[test]
    fn simple_and_raw_string() {
        let tokens = lex(r##"'a' r"b\n" '''c
d''' "e" "##)
        .unwrap();
        assert!(matches!(
            tokens[0].kind,
            Kind::Str(StrFlags {
                raw: false,
                triple: false,
                quote: b'\'',
                unterminated: false
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
    fn interpolation_by_name_and_by_braces() {
        let source = r#"'a $b c ${d + {1}.length} e'"#;
        let tokens = lex(source).unwrap();
        let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds[0],
            Kind::StrBegin(
                StrFlags {
                    raw: false,
                    triple: false,
                    quote: b'\'',
                    unterminated: false
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
    fn nested_interpolation_in_string() {
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
    fn lexical_errors_do_not_stop() {
        // An unterminated string ends at the line break or at end of file.
        let t = lex("'abc").unwrap();
        assert!(matches!(t[0].kind, Kind::Str(StrFlags { unterminated: true, .. })));
        assert_eq!(t[0].span, Span { start: 0, end: 4 });
        let t = lex("'a\r\nb'").unwrap();
        assert!(matches!(t[0].kind, Kind::Str(StrFlags { unterminated: true, .. })));
        assert_eq!(t[0].span, Span { start: 0, end: 2 });
        assert_eq!(t[1].kind, Kind::Ident);
        assert!(matches!(t[2].kind, Kind::Str(StrFlags { unterminated: true, .. })));
        // An open block comment runs to the end.
        assert_eq!(lex("a /* abc").unwrap().len(), 2);
        // The unexpected character joins the identifier; the control one vanishes.
        let s = "pisk\u{f8}de `x \u{1}y";
        let t = lex(s).unwrap();
        let texts: Vec<_> = t.iter().map(|k| k.text(s)).collect();
        assert_eq!(texts, ["pisk\u{f8}de", "`x", "y", ""]);
    }

    #[test]
    fn long_operators() {
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
