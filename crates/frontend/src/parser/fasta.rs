//! Classificação de tokens e utilidades de recuperação portadas do parser do
//! `_fe_analyzer_shared` (o "fasta", que o analyzer usa).
//!
//! O lexer do DartForge só marca como [`Kind::Keyword`] as palavras
//! reservadas; os identificadores embutidos (`abstract`, `get`…) e os
//! pseudo-identificadores (`async`, `out`…) chegam como [`Kind::Ident`]. O
//! scanner do fasta os marca todos como palavra-chave, com um estilo
//! (`KeywordStyle` em `scanner/token.dart`), e a recuperação de erros do
//! parser decide por esse estilo e pelas marcas `isModifier` e
//! `isTopLevelKeyword`. As funções daqui refazem essa classificação pelo
//! texto, com as tabelas de `token.dart` (versão 76.0.0, a do Dart 3.6).
//!
//! Também fica aqui o casamento de `<` com `>` que o scanner do fasta faz
//! (`BeginToken.endGroup` de `<`): a recuperação de listas de parâmetros de
//! tipo pula até ele.

use super::Parser;
use crate::token::{Interp, Kind, Op};

/// Estilo de uma palavra-chave do fasta (`KeywordStyle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WordStyle {
    /// Palavra reservada: nunca é identificador.
    Reserved,
    /// Identificador embutido (`isBuiltIn`): identificador, mas não nome de
    /// tipo nem de declaração.
    BuiltIn,
    /// Pseudo-palavra (`isPseudo`): identificador em qualquer lugar.
    Pseudo,
}

/// O estilo da palavra `texto` pelas tabelas de `token.dart`, ou `None` se
/// não for palavra-chave do fasta.
pub(crate) fn word_style(texto: &str) -> Option<WordStyle> {
    Some(match texto {
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

/// `isModifier` de `token.dart`.
pub(crate) fn is_modifier(texto: &str) -> bool {
    matches!(
        texto,
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

/// `isTopLevelKeyword` de `token.dart`.
pub(crate) fn is_top_level_word(texto: &str) -> bool {
    matches!(
        texto,
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

/// `isVariance` de `type_info_impl.dart`: `in`, `inout` ou `out`.
pub(crate) fn is_variance(texto: &str) -> bool {
    matches!(texto, "in" | "inout" | "out")
}

impl<'s, 'i> Parser<'s, 'i> {
    /// O texto do token na posição absoluta `pos`, se ele é palavra ou
    /// identificador (senão vazio).
    fn word_of(&self, pos: usize) -> &'s str {
        match self.kind_of(pos) {
            Kind::Ident | Kind::Keyword(_) => self.text_of(pos),
            _ => "",
        }
    }

    /// O estilo fasta do token na posição absoluta `pos`.
    pub(crate) fn word_style_of(&self, pos: usize) -> Option<WordStyle> {
        match self.kind_of(pos) {
            Kind::Ident | Kind::Keyword(_) => word_style(self.text_of(pos)),
            _ => None,
        }
    }

    /// `token.kind == IDENTIFIER_TOKEN` do fasta: identificador que não é
    /// palavra-chave de estilo nenhum.
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

    /// `looksLikeTypeParamOrArg` (`type_info_impl.dart`): em declaração, um
    /// identificador puro seguido de identificador puro, `,` ou `>` parece
    /// mais um parâmetro de tipo depois de uma vírgula que falta.
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

    /// O `>` que o scanner do fasta casa com o `<` na posição absoluta
    /// `lt` (`BeginToken.endGroup`), se houver.
    ///
    /// Porte de `appendBeginGroup`/`appendGt`/`discardOpenLt` de
    /// `abstract_scanner.dart`: `<` abre grupo; `>` fecha o `<` do topo da
    /// pilha; `;`, `{`, `[`, `${`, `this`, qualquer token que começa com `=`
    /// e os fechos `)`/`]`/`}` descartam os `<` abertos no topo. O lexer do
    /// DartForge separa `>=`, `>>` e `>>>`; o fasta os lê inteiros, e `>=`
    /// não fecha grupo — por isso um `>` colado a `=` não fecha aqui.
    pub(crate) fn lt_group_end(&self, lt: usize) -> Option<usize> {
        // Pilha de abridores: `<`, `(`, `[`, `{`, com a posição.
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
                // `${`: a interpolação fecha dentro do próprio token de string.
                Kind::StrBegin(_, Interp::Brace) | Kind::StrMid(_, Interp::Brace) => {
                    discard_lt(&mut stack)
                }
                Kind::Op(Op::Gt) => {
                    let ge = t.glued && self.kind_of(i + 1) == Kind::Op(Op::Assign);
                    if !ge && let Some(&(Op::Lt, abre)) = stack.last() {
                        stack.pop();
                        if abre == lt {
                            return Some(i);
                        }
                    }
                }
                Kind::Op(o @ (Op::RParen | Op::RBracket | Op::RBrace)) => {
                    // `discardBeginGroupUntil`: sem abridor correspondente, o
                    // fecho é ignorado e a pilha fica como estava.
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
                    // `>=` chega como `>` colado a `=`: o `=` ali não é
                    // token do fasta.
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
