//! Token classification and recovery helpers ported from the
//! `_fe_analyzer_shared` parser (the "fasta" the analyzer uses).
//!
//! DartForge's lexer only marks reserved words as [`Kind::Keyword`]; the
//! built-in identifiers (`abstract`, `get`…) and the pseudo-identifiers
//! (`async`, `out`…) arrive as [`Kind::Ident`]. fasta's scanner marks all of
//! them as keywords, with a style (`KeywordStyle` in `scanner/token.dart`),
//! and the parser's error recovery decides by that style and by the
//! `isModifier` and `isTopLevelKeyword` flags. The functions here redo that
//! classification from the text, with the tables of `token.dart` (version
//! 76.0.0, Dart 3.6's).
//!
//! Also here is the matching of `<` with `>` that fasta's scanner does
//! (`BeginToken.endGroup` of `<`): the recovery of type parameter lists skips
//! to it.

use super::Parser;
use crate::token::{Interp, Kind, Op};

/// Style of a fasta keyword (`KeywordStyle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WordStyle {
    /// Reserved word: never an identifier.
    Reserved,
    /// Built-in identifier (`isBuiltIn`): an identifier, but not a type or
    /// declaration name.
    BuiltIn,
    /// Pseudo-keyword (`isPseudo`): an identifier anywhere.
    Pseudo,
}

/// The style of the word `text` according to the `token.dart` tables, or
/// `None` if it is not a fasta keyword.
pub(crate) fn word_style(text: &str) -> Option<WordStyle> {
    Some(match text {
        "assert" | "break" | "case" | "catch" | "class" | "const" | "continue" | "default"
        | "do" | "else" | "enum" | "extends" | "false" | "final" | "finally" | "for" | "if"
        | "in" | "is" | "new" | "null" | "rethrow" | "return" | "super" | "switch" | "this"
        | "throw" | "true" | "try" | "var" | "void" | "while" | "with" => WordStyle::Reserved,
        "abstract" | "as" | "augment" | "covariant" | "deferred" | "dynamic" | "export"
        | "extension" | "external" | "factory" | "Function" | "get" | "implements" | "import"
        | "interface" | "late" | "library" | "mixin" | "operator" | "part" | "required" | "set"
        | "static" | "typedef" => WordStyle::BuiltIn,
        "async" | "await" | "base" | "hide" | "inout" | "native" | "of" | "on" | "out"
        | "patch" | "sealed" | "show" | "source" | "sync" | "when" | "yield" => WordStyle::Pseudo,
        _ => return None,
    })
}

/// `isModifier` from `token.dart`.
pub(crate) fn is_modifier(text: &str) -> bool {
    matches!(
        text,
        "abstract"
            | "augment"
            | "const"
            | "covariant"
            | "external"
            | "final"
            | "late"
            | "required"
            | "static"
            | "var"
    )
}

/// `isTopLevelKeyword` from `token.dart`.
pub(crate) fn is_top_level_word(text: &str) -> bool {
    matches!(
        text,
        "class"
            | "enum"
            | "export"
            | "extension"
            | "import"
            | "library"
            | "mixin"
            | "part"
            | "typedef"
    )
}

/// `isVariance` from `type_info_impl.dart`: `in`, `inout` or `out`.
pub(crate) fn is_variance(text: &str) -> bool {
    matches!(text, "in" | "inout" | "out")
}

impl<'s, 'i> Parser<'s, 'i> {
    /// The text of the token at absolute position `pos`, if it is a word or an
    /// identifier (otherwise empty).
    fn word_of(&self, pos: usize) -> &'s str {
        match self.kind_of(pos) {
            Kind::Ident | Kind::Keyword(_) => self.text_of(pos),
            _ => "",
        }
    }

    /// The fasta style of the token at absolute position `pos`.
    pub(crate) fn word_style_of(&self, pos: usize) -> Option<WordStyle> {
        match self.kind_of(pos) {
            Kind::Ident | Kind::Keyword(_) => word_style(self.text_of(pos)),
            _ => None,
        }
    }

    /// fasta's `token.kind == IDENTIFIER_TOKEN`: an identifier that is not a
    /// keyword of any style.
    pub(crate) fn is_plain_identifier(&self, pos: usize) -> bool {
        self.kind_of(pos) == Kind::Ident && word_style(self.text_of(pos)).is_none()
    }

    /// `isKeywordOrIdentifier` do fasta.
    pub(crate) fn is_word_or_identifier(&self, pos: usize) -> bool {
        matches!(self.kind_of(pos), Kind::Ident | Kind::Keyword(_))
    }

    /// `looksLikeStartOfNextTopLevelDeclaration` (`identifier_context_impl.dart`).
    pub(crate) fn looks_like_top_level_start(&self, pos: usize) -> bool {
        let t = self.word_of(pos);
        is_top_level_word(t)
            || matches!(t, "const" | "get" | "final" | "set" | "var" | "void")
            || self.kind_of(pos) == Kind::Eof
    }

    /// `looksLikeStartOfNextClassMember` (`identifier_context_impl.dart`).
    pub(crate) fn looks_like_member_start(&self, pos: usize) -> bool {
        let t = self.word_of(pos);
        is_modifier(t)
            || self.kind_of(pos) == Kind::Op(Op::At)
            || matches!(t, "get" | "set" | "void")
            || self.kind_of(pos) == Kind::Eof
    }

    /// `looksLikeStatementStart` (`identifier_context.dart`).
    pub(crate) fn looks_like_statement_start(&self, pos: usize) -> bool {
        let t = if self.kind_of(pos) == Kind::Ident {
            ""
        } else {
            self.word_of(pos)
        };
        matches!(
            t,
            "assert"
                | "break"
                | "continue"
                | "do"
                | "else"
                | "final"
                | "for"
                | "if"
                | "return"
                | "switch"
                | "try"
                | "var"
                | "void"
                | "while"
        ) || matches!(self.kind_of(pos), Kind::Op(Op::At) | Kind::Eof)
    }

    /// `looksLikeExpressionStart` (`identifier_context.dart`).
    pub(crate) fn looks_like_expression_start(&self, pos: usize) -> bool {
        match self.kind_of(pos) {
            Kind::Ident | Kind::Int | Kind::Double | Kind::Str(_) | Kind::StrBegin(..) => true,
            Kind::Keyword(_) => !self.looks_like_statement_start(pos),
            Kind::Op(op) => matches!(
                op,
                Op::Hash
                    | Op::LBrace
                    | Op::LParen
                    | Op::LBracket
                    | Op::Lt
                    | Op::Bang
                    | Op::Minus
                    | Op::Tilde
                    | Op::PlusPlus
                    | Op::MinusMinus
            ),
            _ => false,
        }
    }

    /// `looksLikeTypeParamOrArg` (`type_info_impl.dart`): in a declaration, a
    /// plain identifier followed by a plain identifier, `,` or `>` looks like
    /// one more type parameter after a missing comma.
    pub(crate) fn looks_like_type_param_or_arg(&self, in_declaration: bool, pos: usize) -> bool {
        in_declaration
            && self.is_plain_identifier(pos)
            && (self.is_plain_identifier(pos + 1)
                || matches!(self.kind_of(pos + 1), Kind::Op(Op::Comma | Op::Gt)))
    }

    /// `isValidNonRecordTypeReference` (`type_info.dart`).
    pub(crate) fn is_valid_type_reference(&self, pos: usize) -> bool {
        match self.kind_of(pos) {
            Kind::Ident => match word_style(self.text_of(pos)) {
                None | Some(WordStyle::Pseudo) => true,
                Some(_) => {
                    matches!(self.text_of(pos), "dynamic" | "Function")
                        || self.kind_of(pos + 1) == Kind::Op(Op::Dot)
                }
            },
            Kind::Keyword(k) => k == crate::token::Keyword::Void,
            _ => false,
        }
    }

    /// The `>` that fasta's scanner matches with the `<` at absolute position
    /// `lt` (`BeginToken.endGroup`), if any.
    ///
    /// Port of `appendBeginGroup`/`appendGt`/`discardOpenLt` from
    /// `abstract_scanner.dart`: `<` opens a group; `>` closes the `<` on top of
    /// the stack; `;`, `{`, `[`, `${`, `this`, any token starting with `=` and
    /// the closers `)`/`]`/`}` discard the open `<` on top. DartForge's lexer
    /// splits `>=`, `>>` and `>>>`; fasta reads them whole, and `>=` does not
    /// close a group — so a `>` glued to `=` does not close here.
    pub(crate) fn lt_group_end(&self, lt: usize) -> Option<usize> {
        // Opener stack: `<`, `(`, `[`, `{`, with the position.
        let mut stack: Vec<(Op, usize)> = Vec::new();
        let discard_lt = |p: &mut Vec<(Op, usize)>| {
            while p.last().is_some_and(|(a, _)| *a == Op::Lt) {
                p.pop();
            }
        };
        for i in 0..self.tokens.len() {
            let t = self.tokens[i];
            match t.kind {
                Kind::Op(Op::Lt) | Kind::Op(Op::LParen) => stack.push((
                    if t.kind == Kind::Op(Op::Lt) {
                        Op::Lt
                    } else {
                        Op::LParen
                    },
                    i,
                )),
                Kind::Op(o @ (Op::LBrace | Op::LBracket)) => {
                    discard_lt(&mut stack);
                    stack.push((o, i));
                }
                // `${`: the interpolation closes inside the string token itself.
                Kind::StrBegin(_, Interp::Brace) | Kind::StrMid(_, Interp::Brace) => {
                    discard_lt(&mut stack)
                }
                Kind::Op(Op::Gt) => {
                    let ge = t.glued && self.kind_of(i + 1) == Kind::Op(Op::Assign);
                    if !ge && let Some(&(Op::Lt, opener)) = stack.last() {
                        stack.pop();
                        if opener == lt {
                            return Some(i);
                        }
                    }
                }
                Kind::Op(o @ (Op::RParen | Op::RBracket | Op::RBrace)) => {
                    // `discardBeginGroupUntil`: without a matching opener, the closer is
                    // ignored and the stack stays as it was.
                    let wants = match o {
                        Op::RParen => Op::LParen,
                        Op::RBracket => Op::LBracket,
                        _ => Op::LBrace,
                    };
                    if let Some(k) = stack.iter().rposition(|(a, _)| *a == wants) {
                        stack.truncate(k);
                    }
                }
                Kind::Op(Op::Semicolon | Op::Assign | Op::EqEq | Op::Arrow) => {
                    // `>=` arrives as `>` glued to `=`: that `=` is not a fasta token.
                    let part_of_ge = t.kind == Kind::Op(Op::Assign)
                        && i > 0
                        && self.tokens[i - 1].glued
                        && self.tokens[i - 1].kind == Kind::Op(Op::Gt);
                    if !part_of_ge {
                        discard_lt(&mut stack);
                    }
                }
                Kind::Keyword(crate::token::Keyword::This) => discard_lt(&mut stack),
                Kind::Eof => break,
                _ => {}
            }
            if i >= lt && stack.iter().all(|(_, p)| *p != lt) {
                return None;
            }
        }
        None
    }
}
