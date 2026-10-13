//! Formatter tokens, in the shape `dart_style` sees them: the analyzer
//! scanner's stream, with comments hanging off the following token
//! (`precedingComments`).
//!
//! The base is the `dartforge-frontend` lexer ([`dartforge_frontend::lexer`]).
//! Two contract differences are resolved here:
//!
//! * The frontend lexer delivers interpolated strings as
//!   `StrBegin`/`StrMid`/`StrEnd` pieces that include the `${`, the `$` and
//!   the `}`. The analyzer splits those delimiters into tokens of their own
//!   (`STRING_INTERPOLATION_EXPRESSION`, `STRING_INTERPOLATION_IDENTIFIER`,
//!   `CLOSE_CURLY_BRACKET`) and the text piece becomes a separate `STRING`;
//!   the `SourceVisitor` writes each of those tokens, so they are split.
//! * The frontend lexer never combines `>`; the parser composes `>>`, `>=`,
//!   `>>>`, `>>=` and `>>>=` (see [`Tokens::merge_greater`]), only in
//!   expression context, just as fasta's parser splits the scanner's
//!   composite tokens in type context.
//!
//! Comments are read right here, in the gap between two tokens (which only
//! contains whitespace and comments), with the same rule as fasta's scanner:
//! `//` runs up to the `\n` or `\r`, `/*` nests.

use dartforge_frontend::token::{Keyword, Kind, Op};

/// Index of a token in [`Tokens::list`].
pub type Tk = u32;

/// Token class, at the granularity the formatter needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// Identifier, including built-in and contextual ones (`abstract`,
    /// `async`, `show`…).
    Identifier,
    /// Reserved word.
    Reserved(Keyword),
    /// Integer literal.
    Integer,
    /// Literal `double`.
    Double,
    /// A string piece (the analyzer's `STRING`): the whole string when there is
    /// no interpolation, or a piece between interpolations.
    Text,
    /// `${` that opens an interpolation.
    ExpressionInterpolation,
    /// `$` before an interpolated identifier.
    IdentifierInterpolation,
    /// Punctuation or a simple operator.
    Op(Op),
    /// `>` composed by the parser with the following ones (`>>`, `>=`, `>>>`,
    /// `>>=`, `>>>=`); the text comes from the range.
    ComposedGreater,
    /// `#!...` on the first line.
    ScriptTag,
    /// End of file.
    Eof,
}

/// A comment between two tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Comment {
    pub start: usize,
    pub end: usize,
    /// `//` (o `SINGLE_LINE_COMMENT` do analyzer).
    pub is_line: bool,
}

/// Token with a byte position and the comments that precede it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind_: TokenKind,
    pub start: usize,
    pub end: usize,
    /// The next token starts at `end` (no whitespace or comment in between).
    pub glued: bool,
    /// Range in [`Tokens::comments`] with the comments before this token.
    pub comments: (u32, u32),
}

/// Syntax error detected while reading the tokens or the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroSintaxe {
    /// Byte position in the source.
    pub posicao: usize,
    /// Length in bytes of the highlighted excerpt (at least 1).
    pub comprimento: usize,
    pub mensagem: String,
}

/// The token stream of a unit and its comments.
#[derive(Debug, Clone)]
pub struct Tokens {
    pub list: Vec<Token>,
    pub comments: Vec<Comment>,
}

impl Tokens {
    /// Reads `source` into tokens.
    ///
    /// ```
    /// use dartforge_formatter::syntax::token::{Tokens, TokenKind};
    /// let t = Tokens::read("a = 'x${b}y'; // end\n").unwrap();
    /// let texts: Vec<_> = t.list.iter().map(|k| &"a = 'x${b}y'; // end\n"[k.start..k.end]).collect();
    /// assert_eq!(texts, ["a", "=", "'x", "${", "b", "}", "y'", ";", ""]);
    /// assert_eq!(t.list.last().unwrap().kind_, TokenKind::Eof);
    /// assert_eq!(t.comments.len(), 1);
    /// ```
    ///
    /// # Errors
    /// An unexpected character, an unterminated string or block comment, and
    /// the `===`/`!==` operators, which fasta's scanner only accepts to report an
    /// error.
    pub fn read(source: &str) -> Result<Tokens, ErroSintaxe> {
        let raw_tokens = dartforge_frontend::lexer::lex(source).map_err(|d| ErroSintaxe {
            posicao: d.span.start,
            comprimento: (d.span.end - d.span.start).max(1),
            mensagem: d.message.clone(),
        })?;
        let mut list: Vec<Token> = Vec::with_capacity(raw_tokens.len() + 8);
        // Stack of open interpolations, per literal: `true` for `${…}`.
        // The piece that follows a `${…}` interpolation starts at the `}`.
        let mut open_groups: Vec<bool> = Vec::new();
        let new = |kind_, start, end| Token {
            kind_,
            start,
            end,
            glued: false,
            comments: (0, 0),
        };
        for b in &raw_tokens {
            let (start_, end) = (b.span.start, b.span.end);
            let text = &source[start_..end];
            match b.kind {
                Kind::Ident => {
                    if !text.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'$') {
                        return Err(ErroSintaxe {
                            posicao: start_,
                            comprimento: end - start_,
                            mensagem: format!("Unexpected character in '{text}'."),
                        });
                    }
                    list.push(new(TokenKind::Identifier, start_, end));
                }
                Kind::Keyword(k) => list.push(new(TokenKind::Reserved(k), start_, end)),
                Kind::Int => list.push(new(TokenKind::Integer, start_, end)),
                Kind::Double => list.push(new(TokenKind::Double, start_, end)),
                Kind::Str(f) => {
                    if f.aberta {
                        return Err(string_error(start_, end));
                    }
                    list.push(new(TokenKind::Text, start_, end));
                }
                Kind::StrBegin(_, interp) | Kind::StrMid(_, interp) => {
                    let mut i = start_;
                    let key = matches!(b.kind, Kind::StrMid(..)) && open_groups.pop().unwrap_or(false);
                    open_groups.push(interp == dartforge_frontend::token::Interp::Brace);
                    if key {
                        list.push(new(TokenKind::Op(Op::RBrace), i, i + 1));
                        i += 1;
                    }
                    let delim = match interp {
                        dartforge_frontend::token::Interp::Brace => 2,
                        dartforge_frontend::token::Interp::Ident => 1,
                    };
                    list.push(new(TokenKind::Text, i, end - delim));
                    let kind_ = if delim == 2 {
                        TokenKind::ExpressionInterpolation
                    } else {
                        TokenKind::IdentifierInterpolation
                    };
                    list.push(new(kind_, end - delim, end));
                }
                Kind::StrEnd(f) => {
                    if f.aberta {
                        return Err(string_error(start_, end));
                    }
                    let mut i = start_;
                    if open_groups.pop().unwrap_or(false) {
                        list.push(new(TokenKind::Op(Op::RBrace), i, i + 1));
                        i += 1;
                    }
                    list.push(new(TokenKind::Text, i, end));
                }
                Kind::Op(op) => {
                    if (op == Op::EqEq || op == Op::BangEq) && end - start_ == 3 {
                        return Err(ErroSintaxe {
                            posicao: start_,
                            comprimento: 3,
                            mensagem: format!("The '{text}' operator is not supported."),
                        });
                    }
                    list.push(new(TokenKind::Op(op), start_, end));
                }
                Kind::ScriptTag => list.push(new(TokenKind::ScriptTag, start_, end)),
                Kind::Eof => list.push(new(TokenKind::Eof, start_, end)),
            }
        }
        // Gluing and comments, from the end positions.
        let mut comments = Vec::new();
        let bytes = source.as_bytes();
        let mut previous_end = 0usize;
        for i in 0..list.len() {
            let start_ = list[i].start;
            let c0 = comments.len() as u32;
            let mut p = previous_end;
            while p < start_ {
                let c = bytes[p];
                if c == b'/' && bytes.get(p + 1) == Some(&b'/') {
                    let s = p;
                    while p < start_ && bytes[p] != b'\n' && bytes[p] != b'\r' {
                        p += 1;
                    }
                    comments.push(Comment {
                        start: s,
                        end: p,
                        is_line: true,
                    });
                } else if c == b'/' && bytes.get(p + 1) == Some(&b'*') {
                    let s = p;
                    p += 2;
                    let mut level = 1usize;
                    while p < bytes.len() && level > 0 {
                        if bytes[p] == b'/' && bytes.get(p + 1) == Some(&b'*') {
                            level += 1;
                            p += 2;
                        } else if bytes[p] == b'*' && bytes.get(p + 1) == Some(&b'/') {
                            level -= 1;
                            p += 2;
                        } else {
                            p += 1;
                        }
                    }
                    if level > 0 {
                        return Err(ErroSintaxe {
                            posicao: s,
                            comprimento: 2,
                            mensagem: "Unterminated multi-line comment.".into(),
                        });
                    }
                    comments.push(Comment {
                        start: s,
                        end: p,
                        is_line: false,
                    });
                } else {
                    p += 1;
                }
            }
            list[i].comments = (c0, comments.len() as u32);
            if i > 0 {
                list[i - 1].glued = list[i - 1].end == start_;
            }
            previous_end = list[i].end.max(previous_end);
        }
        Ok(Tokens { list, comments })
    }

    /// Text of token `t`.
    pub fn text<'a>(&self, source: &'a str, t: Tk) -> &'a str {
        let k = &self.list[t as usize];
        &source[k.start..k.end]
    }

    /// The comments that precede token `t`.
    pub fn comments_of(&self, t: Tk) -> &[Comment] {
        let (a, b) = self.list[t as usize].comments;
        &self.comments[a as usize..b as usize]
    }

    /// Joins the `>` at `t` with the glued `>`/`=` that follow it, up to the
    /// operator `target_` (`">>"`, `">="`, `">>>"`, `">>="` or `">>>="`). Only
    /// called by the parser in expression context, right after checking with
    /// [`Tokens::composed_greater`]. The absorbed tokens leave the list; no node
    /// points to them yet, because the parser is exactly at `t`.
    pub fn merge_greater(&mut self, t: Tk, target_: &str) {
        let i = t as usize;
        let n = target_.len();
        let end = self.list[i].start + n;
        let glued_last = self.list[i + n - 1].glued;
        self.list.drain(i + 1..i + n);
        self.list[i].end = end;
        self.list[i].kind_ = TokenKind::ComposedGreater;
        self.list[i].glued = glued_last;
    }

    /// If the `>` at `t` starts an operator made of glued tokens, returns the
    /// longest of them (`>>>=`, `>>>`, `>>=`, `>>`, `>=`).
    pub fn composed_greater(&self, t: Tk) -> Option<&'static str> {
        let i = t as usize;
        let e = |k: usize, op: Op| self.list.get(i + k).is_some_and(|x| x.kind_ == TokenKind::Op(op));
        let col = |k: usize| self.list.get(i + k).is_some_and(|x| x.glued);
        if !e(0, Op::Gt) || !col(0) {
            return None;
        }
        if e(1, Op::Gt) {
            if col(1) && e(2, Op::Gt) {
                if col(2) && e(3, Op::Assign) {
                    return Some(">>>=");
                }
                return Some(">>>");
            }
            if col(1) && e(2, Op::Assign) {
                return Some(">>=");
            }
            return Some(">>");
        }
        if e(1, Op::Assign) {
            return Some(">=");
        }
        // `>==`, `>=>` and the like do not exist; `>` followed by `==` is `>=` and
        // `=`.
        if self.list.get(i + 1).is_some_and(|x| x.kind_ == TokenKind::Op(Op::EqEq)) {
            return None;
        }
        None
    }
}

fn string_error(start_: usize, end: usize) -> ErroSintaxe {
    ErroSintaxe {
        posicao: start_,
        comprimento: (end - start_).max(1),
        mensagem: "Unterminated string literal.".into(),
    }
}
