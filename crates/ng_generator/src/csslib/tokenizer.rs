//! Porte do tokenizador do csslib 1.0.2: `src/tokenizer_base.dart`
//! (`TokenizerBase`, `TokenizerState`), `src/tokenizer.dart` (`Tokenizer`,
//! `TokenizerHelpers`) e `src/token.dart` (`Token`, `IdentifierToken`).
//!
//! O texto é lido como o Dart o vê: unidades UTF-16 (`codeUnitAt`), e a
//! posição dos tokens é o deslocamento nessas unidades (`FileSpan`). Só o
//! que o `Tokenizer` usa está aqui: o `TokenizerBase` tem também leitura de
//! strings com escapes e de números com expoente que a subclasse sobrepõe ou
//! nunca chama.
use std::rc::Rc;

use super::Excecao;
use super::token_kind as tk;

/// `TokenChar`: os códigos dos caracteres que o tokenizador distingue.
mod ch {
    pub const END_OF_FILE: u16 = 0;
    pub const LPAREN: u16 = 0x28;
    pub const RPAREN: u16 = 0x29;
    pub const LBRACK: u16 = 0x5b;
    pub const RBRACK: u16 = 0x5d;
    pub const LBRACE: u16 = 0x7b;
    pub const RBRACE: u16 = 0x7d;
    pub const DOT: u16 = 0x2e;
    pub const SEMICOLON: u16 = 0x3b;
    pub const AT: u16 = 0x40;
    pub const HASH: u16 = 0x23;
    pub const PLUS: u16 = 0x2b;
    pub const GREATER: u16 = 0x3e;
    pub const TILDE: u16 = 0x7e;
    pub const ASTERISK: u16 = 0x2a;
    pub const NAMESPACE: u16 = 0x7c;
    pub const COLON: u16 = 0x3a;
    pub const COMMA: u16 = 0x2c;
    pub const SPACE: u16 = 0x20;
    pub const TAB: u16 = 0x9;
    pub const NEWLINE: u16 = 0xa;
    pub const RETURN: u16 = 0xd;
    pub const PERCENT: u16 = 0x25;
    pub const SINGLE_QUOTE: u16 = 0x27;
    pub const DOUBLE_QUOTE: u16 = 0x22;
    pub const SLASH: u16 = 0x2f;
    pub const EQUALS: u16 = 0x3d;
    pub const CARET: u16 = 0x5e;
    pub const DOLLAR: u16 = 0x24;
    pub const LESS: u16 = 0x3c;
    pub const BANG: u16 = 0x21;
    pub const MINUS: u16 = 0x2d;
    pub const BACKSLASH: u16 = 0x5c;
    pub const AMPERSAND: u16 = 0x26;
}

/// `Token` (e `IdentifierToken`, quando `ident` existe): o tipo, o trecho
/// `[ini, fim)` do texto e, num identificador, o texto já sem escapes.
#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub kind: i32,
    pub ini: usize,
    pub fim: usize,
    pub ident: Option<Rc<str>>,
}

impl Token {
    /// `Token.text`: o do `IdentifierToken`, ou o trecho do arquivo.
    pub fn texto(&self, fonte: &[u16]) -> String {
        match &self.ident {
            Some(t) => t.to_string(),
            None => String::from_utf16_lossy(&fonte[self.ini..self.fim]),
        }
    }
}

/// `String.trim` do Dart: tira o espaço Unicode das pontas, incluindo o
/// BOM (`U+FEFF`), que o `char::is_whitespace` do Rust não conta.
pub(crate) fn dart_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || c == '\u{FEFF}')
}

/// `TokenizerState`: o que a leitura antecipada do parser guarda e restaura.
#[derive(Clone, Copy)]
pub(crate) struct TokenizerState {
    index: usize,
    start_index: usize,
    in_selector_expression: bool,
    in_selector: bool,
}

/// `TokenizerHelpers`.
pub(crate) mod helpers {
    /// `isIdentifierStart`.
    pub fn is_identifier_start(c: u16) -> bool {
        is_identifier_start_expr(c) || c == 45
    }
    /// `isDigit`.
    pub fn is_digit(c: u16) -> bool {
        (48..=57).contains(&c)
    }
    /// `isHexDigit`.
    pub fn is_hex_digit(c: u16) -> bool {
        is_digit(c) || (97..=102).contains(&c) || (65..=70).contains(&c)
    }
    /// `isIdentifierPart`.
    pub fn is_identifier_part(c: u16) -> bool {
        is_identifier_part_expr(c) || c == 45
    }
    /// `isIdentifierStartExpr`.
    pub fn is_identifier_start_expr(c: u16) -> bool {
        (97..=122).contains(&c) || (65..=90).contains(&c) || c == 95 || c >= 0xA0 || c == 92
    }
    /// `isIdentifierPartExpr`.
    pub fn is_identifier_part_expr(c: u16) -> bool {
        is_identifier_start_expr(c) || is_digit(c)
    }
}

use helpers::*;

/// `Tokenizer` (com o que ele herda de `TokenizerBase`).
pub(crate) struct Tokenizer<'a> {
    text: &'a [u16],
    /// `_inString`: apesar do nome, verdadeiro quando espaço e comentário
    /// são pulados; o parser o desliga para ler o conteúdo de strings.
    pub in_string: bool,
    /// `inSelectorExpression`: dentro de `:pseudo(…)` o `-` é operador.
    pub in_selector_expression: bool,
    /// `inSelector`: em seletor não há unidades nem `\` solto.
    pub in_selector: bool,
    index: usize,
    start_index: usize,
}

impl<'a> Tokenizer<'a> {
    /// `Tokenizer(file, text, skipWhitespace, [index])`.
    pub fn new(text: &'a [u16], in_string: bool) -> Self {
        Tokenizer {
            text,
            in_string,
            in_selector_expression: false,
            in_selector: false,
            index: 0,
            start_index: 0,
        }
    }

    /// `mark`.
    pub fn mark(&self) -> TokenizerState {
        TokenizerState {
            index: self.index,
            start_index: self.start_index,
            in_selector_expression: self.in_selector_expression,
            in_selector: self.in_selector,
        }
    }

    /// `restore`.
    pub fn restore(&mut self, m: TokenizerState) {
        self.index = m.index;
        self.start_index = m.start_index;
        self.in_selector_expression = m.in_selector_expression;
        self.in_selector = m.in_selector;
    }

    /// `_nextChar`: `0` no fim do texto.
    fn next_char(&mut self) -> u16 {
        if self.index < self.text.len() {
            let c = self.text[self.index];
            self.index += 1;
            c
        } else {
            0
        }
    }

    /// `_peekChar`.
    fn peek_char(&self, offset: usize) -> u16 {
        self.text.get(self.index + offset).copied().unwrap_or(0)
    }

    /// `_maybeEatChar`.
    fn maybe_eat_char(&mut self, c: u16) -> bool {
        if self.index < self.text.len() && self.text[self.index] == c {
            self.index += 1;
            true
        } else {
            false
        }
    }

    /// `_nextCharsAreNumber`.
    fn next_chars_are_number(&self, first: u16) -> bool {
        if is_digit(first) {
            return true;
        }
        let second = self.peek_char(0);
        if first == ch::DOT {
            return is_digit(second);
        }
        if first == ch::PLUS || first == ch::MINUS {
            return is_digit(second) || (second == ch::DOT && is_digit(self.peek_char(1)));
        }
        false
    }

    /// `_finishToken`.
    fn finish_token(&self, kind: i32) -> Token {
        Token {
            kind,
            ini: self.start_index,
            fim: self.index,
            ident: None,
        }
    }

    /// `Tokenizer._errorToken`: um token `ERROR`.
    fn error_token(&self) -> Token {
        self.finish_token(tk::ERROR)
    }

    /// `TokenizerBase.finishWhitespace`.
    fn finish_whitespace(&mut self) -> Result<Option<Token>, Excecao> {
        self.index -= 1;
        while self.index < self.text.len() {
            let c = self.text[self.index];
            self.index += 1;
            if c == ch::SPACE || c == ch::TAB || c == ch::RETURN {
                // nada
            } else if c == ch::NEWLINE {
                if !self.in_string {
                    return Ok(Some(self.finish_token(tk::WHITESPACE)));
                }
            } else {
                self.index -= 1;
                if self.in_string {
                    return Ok(None);
                } else {
                    return Ok(Some(self.finish_token(tk::WHITESPACE)));
                }
            }
        }
        Ok(Some(self.finish_token(tk::END_OF_FILE)))
    }

    /// `TokenizerBase.eatDigits`.
    fn eat_digits(&mut self) {
        while self.index < self.text.len() && is_digit(self.text[self.index]) {
            self.index += 1;
        }
    }

    /// `Tokenizer.next`. As chamadas `return next()` do oficial (depois de
    /// espaço, comentário, `]]>` e `<![CDATA[`) são de cauda; aqui viram
    /// laço, sem crescer a pilha numa sequência longa de comentários.
    pub fn next(&mut self, unicode_range: bool) -> Result<Token, Excecao> {
        let mut unicode_range = unicode_range;
        loop {
            if let Some(t) = self.proximo(unicode_range)? {
                return Ok(t);
            }
            unicode_range = false;
        }
    }

    /// Um passo do `Tokenizer.next`: `None` onde o oficial chama `next()`
    /// de novo.
    fn proximo(&mut self, unicode_range: bool) -> Result<Option<Token>, Excecao> {
        self.start_index = self.index;
        let c = self.next_char();
        match c {
            ch::NEWLINE | ch::RETURN | ch::SPACE | ch::TAB => self.finish_whitespace(),
            ch::END_OF_FILE => Ok(Some(self.finish_token(tk::END_OF_FILE))),
            ch::AT => {
                let peek = self.peek_char(0);
                if is_identifier_start(peek) {
                    let old_index = self.index;
                    let old_start_index = self.start_index;
                    self.start_index = self.index;
                    self.next_char();
                    self.finish_identifier()?;
                    // É uma diretiva?
                    let trecho = &self.text[self.start_index..self.index];
                    let mut tok_id = tk::match_directives(trecho);
                    if tok_id == -1 {
                        tok_id = tk::match_margin_directives(trecho);
                    }
                    if tok_id != -1 {
                        return Ok(Some(self.finish_token(tok_id)));
                    }
                    // Não: é o `@nome` das definições do Less.
                    self.start_index = old_start_index;
                    self.index = old_index;
                }
                Ok(Some(self.finish_token(tk::AT)))
            }
            ch::DOT => {
                let start = self.start_index;
                if self.maybe_eat_digit() {
                    let number = self.finish_number();
                    if number.kind == tk::INTEGER {
                        self.start_index = start;
                        return Ok(Some(self.finish_token(tk::DOUBLE)));
                    }
                    return Ok(Some(self.error_token()));
                }
                Ok(Some(self.finish_token(tk::DOT)))
            }
            ch::LPAREN => Ok(Some(self.finish_token(tk::LPAREN))),
            ch::RPAREN => Ok(Some(self.finish_token(tk::RPAREN))),
            ch::LBRACE => Ok(Some(self.finish_token(tk::LBRACE))),
            ch::RBRACE => Ok(Some(self.finish_token(tk::RBRACE))),
            ch::LBRACK => Ok(Some(self.finish_token(tk::LBRACK))),
            ch::RBRACK => {
                if self.maybe_eat_char(ch::RBRACK) && self.maybe_eat_char(ch::GREATER) {
                    // ]]>
                    return Ok(None);
                }
                Ok(Some(self.finish_token(tk::RBRACK)))
            }
            ch::HASH => Ok(Some(self.finish_token(tk::HASH))),
            ch::PLUS => {
                if self.next_chars_are_number(c) {
                    return Ok(Some(self.finish_number()));
                }
                Ok(Some(self.finish_token(tk::PLUS)))
            }
            ch::MINUS => {
                if self.in_selector_expression || unicode_range {
                    Ok(Some(self.finish_token(tk::MINUS)))
                } else if self.next_chars_are_number(c) {
                    Ok(Some(self.finish_number()))
                } else if is_identifier_start(c) {
                    self.finish_identifier().map(Some)
                } else {
                    Ok(Some(self.finish_token(tk::MINUS)))
                }
            }
            ch::GREATER => Ok(Some(self.finish_token(tk::GREATER))),
            ch::TILDE => {
                if self.maybe_eat_char(ch::EQUALS) {
                    return Ok(Some(self.finish_token(tk::INCLUDES)));
                }
                Ok(Some(self.finish_token(tk::TILDE)))
            }
            ch::ASTERISK => {
                if self.maybe_eat_char(ch::EQUALS) {
                    return Ok(Some(self.finish_token(tk::SUBSTRING_MATCH)));
                }
                Ok(Some(self.finish_token(tk::ASTERISK)))
            }
            ch::AMPERSAND => Ok(Some(self.finish_token(tk::AMPERSAND))),
            ch::NAMESPACE => {
                if self.maybe_eat_char(ch::EQUALS) {
                    return Ok(Some(self.finish_token(tk::DASH_MATCH)));
                }
                Ok(Some(self.finish_token(tk::NAMESPACE)))
            }
            ch::COLON => Ok(Some(self.finish_token(tk::COLON))),
            ch::COMMA => Ok(Some(self.finish_token(tk::COMMA))),
            ch::SEMICOLON => Ok(Some(self.finish_token(tk::SEMICOLON))),
            ch::PERCENT => Ok(Some(self.finish_token(tk::PERCENT))),
            ch::SINGLE_QUOTE => Ok(Some(self.finish_token(tk::SINGLE_QUOTE))),
            ch::DOUBLE_QUOTE => Ok(Some(self.finish_token(tk::DOUBLE_QUOTE))),
            ch::SLASH => {
                if self.maybe_eat_char(ch::ASTERISK) {
                    return self.finish_multi_line_comment();
                }
                Ok(Some(self.finish_token(tk::SLASH)))
            }
            ch::LESS => {
                // <!--
                if self.maybe_eat_char(ch::BANG) {
                    if self.maybe_eat_char(ch::MINUS) && self.maybe_eat_char(ch::MINUS) {
                        return self.finish_html_comment();
                    } else if self.maybe_eat_char(ch::LBRACK)
                        && self.maybe_eat_char(u16::from(b'C'))
                        && self.maybe_eat_char(u16::from(b'D'))
                        && self.maybe_eat_char(u16::from(b'A'))
                        && self.maybe_eat_char(u16::from(b'T'))
                        && self.maybe_eat_char(u16::from(b'A'))
                        && self.maybe_eat_char(ch::LBRACK)
                    {
                        // <![CDATA[
                        return Ok(None);
                    }
                }
                Ok(Some(self.finish_token(tk::LESS)))
            }
            ch::EQUALS => Ok(Some(self.finish_token(tk::EQUALS))),
            ch::CARET => {
                if self.maybe_eat_char(ch::EQUALS) {
                    return Ok(Some(self.finish_token(tk::PREFIX_MATCH)));
                }
                Ok(Some(self.finish_token(tk::CARET)))
            }
            ch::DOLLAR => {
                if self.maybe_eat_char(ch::EQUALS) {
                    return Ok(Some(self.finish_token(tk::SUFFIX_MATCH)));
                }
                Ok(Some(self.finish_token(tk::DOLLAR)))
            }
            ch::BANG => self.finish_identifier().map(Some),
            _ => {
                if !self.in_selector && c == ch::BACKSLASH {
                    return Ok(Some(self.finish_token(tk::BACKSLASH)));
                }
                if unicode_range {
                    // U+416, U+400-4ff, U+4??
                    if self.maybe_eat_hex_digit() {
                        let t = self.finish_hex_number();
                        if self.maybe_eat_question_mark() {
                            self.finish_unicode_range();
                        }
                        Ok(Some(t))
                    } else if self.maybe_eat_question_mark() {
                        Ok(Some(self.finish_unicode_range()))
                    } else {
                        Ok(Some(self.error_token()))
                    }
                } else if self.in_string
                    && (c == u16::from(b'U') || c == u16::from(b'u'))
                    && self.peek_char(0) == u16::from(b'+')
                {
                    // `U+número`: o token é o número, depois do `+`.
                    self.next_char();
                    self.start_index = self.index;
                    Ok(Some(self.finish_token(tk::UNICODE_RANGE)))
                } else if self.var_def(c) {
                    Ok(Some(self.finish_token(tk::VAR_DEFINITION)))
                } else if self.var_usage(c) {
                    Ok(Some(self.finish_token(tk::VAR_USAGE)))
                } else if is_identifier_start(c) {
                    self.finish_identifier().map(Some)
                } else if is_digit(c) {
                    Ok(Some(self.finish_number()))
                } else {
                    Ok(Some(self.error_token()))
                }
            }
        }
    }

    /// `varDef`: `var-` (consome o que casar, mesmo quando falha).
    fn var_def(&mut self, c: u16) -> bool {
        c == u16::from(b'v')
            && self.maybe_eat_char(u16::from(b'a'))
            && self.maybe_eat_char(u16::from(b'r'))
            && self.maybe_eat_char(u16::from(b'-'))
    }

    /// `varUsage`.
    fn var_usage(&mut self, c: u16) -> bool {
        c == u16::from(b'v')
            && self.maybe_eat_char(u16::from(b'a'))
            && self.maybe_eat_char(u16::from(b'r'))
            && self.peek_char(0) == u16::from(b'-')
    }

    /// `getIdentifierKind`: unidade (fora de seletor), `!important` ou
    /// identificador.
    fn get_identifier_kind(&self) -> i32 {
        let trecho = &self.text[self.start_index..self.index];
        let mut tok_id = -1;
        if !self.in_selector_expression && !self.in_selector {
            tok_id = tk::match_units(trecho);
        }
        if tok_id == -1 {
            let important: Vec<u16> = "!important".encode_utf16().collect();
            tok_id = if trecho == important.as_slice() {
                tk::IMPORTANT
            } else {
                -1
            };
        }
        if tok_id >= 0 { tok_id } else { tk::IDENTIFIER }
    }

    /// `finishIdentifier`: com os escapes `\hex` desfeitos no texto do
    /// token (quando se pulam espaços) e o trecho cru como posição.
    fn finish_identifier(&mut self) -> Result<Token, Excecao> {
        let mut chars: Vec<u32> = Vec::new();
        let validate_from = self.index;
        self.index = self.start_index;
        while self.index < self.text.len() {
            let mut c = self.text[self.index];
            if c == 92 && self.in_string {
                self.index += 1;
                let start_hex = self.index;
                self.eat_hex_digits(start_hex + 6);
                if self.index != start_hex {
                    let hex = String::from_utf16_lossy(&self.text[start_hex..self.index]);
                    chars.push(u32::from_str_radix(&hex, 16).unwrap_or(0));
                    if self.index == self.text.len() {
                        break;
                    }
                    c = self.text[self.index];
                    if self.index - start_hex != 6
                        && (c == ch::SPACE || c == ch::TAB || c == ch::RETURN || c == ch::NEWLINE)
                    {
                        self.index += 1;
                    }
                } else {
                    if self.index == self.text.len() {
                        break;
                    }
                    chars.push(u32::from(self.text[self.index]));
                    self.index += 1;
                }
            } else if self.index < validate_from
                || (if self.in_selector_expression {
                    is_identifier_part_expr(c)
                } else {
                    is_identifier_part(c)
                })
            {
                chars.push(u32::from(c));
                self.index += 1;
            } else {
                break;
            }
        }
        let texto = de_char_codes(&chars)?;
        Ok(Token {
            kind: self.get_identifier_kind(),
            ini: self.start_index,
            fim: self.index,
            ident: Some(Rc::from(texto)),
        })
    }

    /// `Tokenizer.finishNumber`.
    fn finish_number(&mut self) -> Token {
        self.eat_digits();
        if self.peek_char(0) == 46 {
            self.next_char();
            if is_digit(self.peek_char(0)) {
                self.eat_digits();
                return self.finish_token(tk::DOUBLE);
            } else {
                self.index -= 1;
            }
        }
        self.finish_token(tk::INTEGER)
    }

    /// `maybeEatDigit`.
    fn maybe_eat_digit(&mut self) -> bool {
        if self.index < self.text.len() && is_digit(self.text[self.index]) {
            self.index += 1;
            return true;
        }
        false
    }

    /// `finishHexNumber`.
    fn finish_hex_number(&mut self) -> Token {
        self.eat_hex_digits(self.text.len());
        self.finish_token(tk::HEX_INTEGER)
    }

    /// `eatHexDigits`.
    fn eat_hex_digits(&mut self, end: usize) {
        let end = end.min(self.text.len());
        while self.index < end {
            if is_hex_digit(self.text[self.index]) {
                self.index += 1;
            } else {
                return;
            }
        }
    }

    /// `maybeEatHexDigit`.
    fn maybe_eat_hex_digit(&mut self) -> bool {
        if self.index < self.text.len() && is_hex_digit(self.text[self.index]) {
            self.index += 1;
            return true;
        }
        false
    }

    /// `maybeEatQuestionMark`.
    fn maybe_eat_question_mark(&mut self) -> bool {
        if self.index < self.text.len() && self.text[self.index] == u16::from(b'?') {
            self.index += 1;
            return true;
        }
        false
    }

    /// `eatQuestionMarks`.
    fn eat_question_marks(&mut self) {
        while self.index < self.text.len() && self.text[self.index] == u16::from(b'?') {
            self.index += 1;
        }
    }

    /// `finishUnicodeRange`.
    fn finish_unicode_range(&mut self) -> Token {
        self.eat_question_marks();
        self.finish_token(tk::HEX_RANGE)
    }

    /// `finishHtmlComment`.
    fn finish_html_comment(&mut self) -> Result<Option<Token>, Excecao> {
        loop {
            let c = self.next_char();
            if c == 0 {
                return Ok(Some(self.finish_token(tk::INCOMPLETE_COMMENT)));
            } else if c == ch::MINUS
                && self.maybe_eat_char(ch::MINUS)
                && self.maybe_eat_char(ch::GREATER)
            {
                if self.in_string {
                    return Ok(None);
                } else {
                    return Ok(Some(self.finish_token(tk::HTML_COMMENT)));
                }
            }
        }
    }

    /// `Tokenizer.finishMultiLineComment`.
    fn finish_multi_line_comment(&mut self) -> Result<Option<Token>, Excecao> {
        loop {
            let c = self.next_char();
            if c == 0 {
                return Ok(Some(self.finish_token(tk::INCOMPLETE_COMMENT)));
            } else if c == 42 && self.maybe_eat_char(47) {
                if self.in_string {
                    return Ok(None);
                } else {
                    return Ok(Some(self.finish_token(tk::COMMENT)));
                }
            }
        }
    }

    /// A posição de leitura (`_index`), para o parser reconhecer um laço
    /// que não consome nada.
    pub fn posicao(&self) -> usize {
        self.index
    }

    /// `makeIEFilter`: o trecho `[start, end)` como texto.
    pub fn texto_entre(&self, start: usize, end: usize) -> String {
        String::from_utf16_lossy(&self.text[start..end])
    }
}

/// `String.fromCharCodes`: códigos até `0xFFFF` são unidades UTF-16 (um par
/// de substitutos se junta), acima disso viram um par; acima de `0x10FFFF` o
/// Dart lança `RangeError`.
fn de_char_codes(codigos: &[u32]) -> Result<String, Excecao> {
    let mut unidades: Vec<u16> = Vec::with_capacity(codigos.len());
    for &c in codigos {
        if c > 0x10FFFF {
            return Err(Excecao("RangeError"));
        }
        if c > 0xFFFF {
            let v = c - 0x10000;
            unidades.push(0xD800 + (v >> 10) as u16);
            unidades.push(0xDC00 + (v & 0x3FF) as u16);
        } else {
            unidades.push(c as u16);
        }
    }
    Ok(String::from_utf16_lossy(&unidades))
}
