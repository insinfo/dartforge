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
pub(crate) enum Estilo {
    /// Palavra reservada: nunca é identificador.
    Reservada,
    /// Identificador embutido (`isBuiltIn`): identificador, mas não nome de
    /// tipo nem de declaração.
    Embutida,
    /// Pseudo-palavra (`isPseudo`): identificador em qualquer lugar.
    Pseudo,
}

/// O estilo da palavra `texto` pelas tabelas de `token.dart`, ou `None` se
/// não for palavra-chave do fasta.
pub(crate) fn estilo(texto: &str) -> Option<Estilo> {
    Some(match texto {
        "assert" | "break" | "case" | "catch" | "class" | "const" | "continue" | "default" | "do" | "else"
        | "enum" | "extends" | "false" | "final" | "finally" | "for" | "if" | "in" | "is" | "new" | "null"
        | "rethrow" | "return" | "super" | "switch" | "this" | "throw" | "true" | "try" | "var" | "void"
        | "while" | "with" => Estilo::Reservada,
        "abstract" | "as" | "augment" | "covariant" | "deferred" | "dynamic" | "export" | "extension"
        | "external" | "factory" | "Function" | "get" | "implements" | "import" | "interface" | "late"
        | "library" | "mixin" | "operator" | "part" | "required" | "set" | "static" | "typedef" => {
            Estilo::Embutida
        }
        "async" | "await" | "base" | "hide" | "inout" | "native" | "of" | "on" | "out" | "patch" | "sealed"
        | "show" | "source" | "sync" | "when" | "yield" => Estilo::Pseudo,
        _ => return None,
    })
}

/// `isModifier` de `token.dart`.
pub(crate) fn e_modificador(texto: &str) -> bool {
    matches!(
        texto,
        "abstract" | "augment" | "const" | "covariant" | "external" | "final" | "late" | "required" | "static" | "var"
    )
}

/// `isTopLevelKeyword` de `token.dart`.
pub(crate) fn e_palavra_de_topo(texto: &str) -> bool {
    matches!(texto, "class" | "enum" | "export" | "extension" | "import" | "library" | "mixin" | "part" | "typedef")
}

/// `isVariance` de `type_info_impl.dart`: `in`, `inout` ou `out`.
pub(crate) fn e_variancia(texto: &str) -> bool {
    matches!(texto, "in" | "inout" | "out")
}

impl<'s, 'i> Parser<'s, 'i> {
    /// O texto do token na posição absoluta `pos`, se ele é palavra ou
    /// identificador (senão vazio).
    fn palavra_de(&self, pos: usize) -> &'s str {
        match self.kind_of(pos) {
            Kind::Ident | Kind::Keyword(_) => self.text_of(pos),
            _ => "",
        }
    }

    /// O estilo fasta do token na posição absoluta `pos`.
    pub(crate) fn estilo_de(&self, pos: usize) -> Option<Estilo> {
        match self.kind_of(pos) {
            Kind::Ident | Kind::Keyword(_) => estilo(self.text_of(pos)),
            _ => None,
        }
    }

    /// `token.kind == IDENTIFIER_TOKEN` do fasta: identificador que não é
    /// palavra-chave de estilo nenhum.
    pub(crate) fn e_identificador_puro(&self, pos: usize) -> bool {
        self.kind_of(pos) == Kind::Ident && estilo(self.text_of(pos)).is_none()
    }

    /// `isKeywordOrIdentifier` do fasta.
    pub(crate) fn e_palavra_ou_identificador(&self, pos: usize) -> bool {
        matches!(self.kind_of(pos), Kind::Ident | Kind::Keyword(_))
    }

    /// `looksLikeStartOfNextTopLevelDeclaration` (`identifier_context_impl.dart`).
    pub(crate) fn parece_inicio_de_topo(&self, pos: usize) -> bool {
        let t = self.palavra_de(pos);
        e_palavra_de_topo(t)
            || matches!(t, "const" | "get" | "final" | "set" | "var" | "void")
            || self.kind_of(pos) == Kind::Eof
    }

    /// `looksLikeStartOfNextClassMember` (`identifier_context_impl.dart`).
    pub(crate) fn parece_inicio_de_membro(&self, pos: usize) -> bool {
        let t = self.palavra_de(pos);
        e_modificador(t)
            || self.kind_of(pos) == Kind::Op(Op::At)
            || matches!(t, "get" | "set" | "void")
            || self.kind_of(pos) == Kind::Eof
    }

    /// `looksLikeStatementStart` (`identifier_context.dart`).
    pub(crate) fn parece_inicio_de_comando(&self, pos: usize) -> bool {
        let t = if self.kind_of(pos) == Kind::Ident { "" } else { self.palavra_de(pos) };
        matches!(
            t,
            "assert" | "break" | "continue" | "do" | "else" | "final" | "for" | "if" | "return" | "switch" | "try"
                | "var" | "void" | "while"
        ) || matches!(self.kind_of(pos), Kind::Op(Op::At) | Kind::Eof)
    }

    /// `looksLikeTypeParamOrArg` (`type_info_impl.dart`): em declaração, um
    /// identificador puro seguido de identificador puro, `,` ou `>` parece
    /// mais um parâmetro de tipo depois de uma vírgula que falta.
    pub(crate) fn parece_param_ou_arg_de_tipo(&self, em_declaracao: bool, pos: usize) -> bool {
        em_declaracao
            && self.e_identificador_puro(pos)
            && (self.e_identificador_puro(pos + 1) || matches!(self.kind_of(pos + 1), Kind::Op(Op::Comma | Op::Gt)))
    }

    /// `isValidNonRecordTypeReference` (`type_info.dart`).
    pub(crate) fn e_referencia_de_tipo_valida(&self, pos: usize) -> bool {
        match self.kind_of(pos) {
            Kind::Ident => match estilo(self.text_of(pos)) {
                None | Some(Estilo::Pseudo) => true,
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
    pub(crate) fn fim_do_grupo_lt(&self, lt: usize) -> Option<usize> {
        // Pilha de posições: `<` e os outros abridores (marcados pelo tipo).
        #[derive(Clone, Copy, PartialEq)]
        enum Abre {
            Lt,
            Paren,
            Outro,
        }
        let mut pilha: Vec<(Abre, usize)> = Vec::new();
        let descartar_lt = |p: &mut Vec<(Abre, usize)>| {
            while p.last().is_some_and(|(a, _)| *a == Abre::Lt) {
                p.pop();
            }
        };
        for i in 0..self.tokens.len() {
            let t = self.tokens[i];
            match t.kind {
                Kind::Op(Op::Lt) => pilha.push((Abre::Lt, i)),
                Kind::Op(Op::LParen) => pilha.push((Abre::Paren, i)),
                Kind::Op(Op::LBrace | Op::LBracket) | Kind::StrBegin(_, Interp::Brace) | Kind::StrMid(_, Interp::Brace) => {
                    descartar_lt(&mut pilha);
                    pilha.push((Abre::Outro, i));
                }
                Kind::Op(Op::Gt) => {
                    let ge = t.glued && self.kind_of(i + 1) == Kind::Op(Op::Assign);
                    if !ge {
                        if let Some(&(Abre::Lt, abre)) = pilha.last() {
                            pilha.pop();
                            if abre == lt {
                                return Some(i);
                            }
                        }
                    }
                }
                Kind::Op(Op::RParen | Op::RBracket | Op::RBrace) | Kind::StrEnd(_) | Kind::StrMid(_, _) => {
                    descartar_lt(&mut pilha);
                    let quer = if t.kind == Kind::Op(Op::RParen) { Abre::Paren } else { Abre::Outro };
                    if let Some(k) = pilha.iter().rposition(|(a, _)| *a == quer || (quer == Abre::Outro && *a == Abre::Outro)) {
                        pilha.truncate(k);
                    }
                    if let Kind::StrMid(_, Interp::Brace) = t.kind {
                        pilha.push((Abre::Outro, i));
                    }
                }
                Kind::Op(Op::Semicolon | Op::Assign | Op::EqEq | Op::Arrow) => {
                    // `>=` chega como `>` colado a `=`: o `=` ali não é
                    // token do fasta.
                    let parte_de_ge = t.kind == Kind::Op(Op::Assign)
                        && i > 0
                        && self.tokens[i - 1].glued
                        && self.tokens[i - 1].kind == Kind::Op(Op::Gt);
                    if !parte_de_ge {
                        descartar_lt(&mut pilha);
                    }
                }
                Kind::Keyword(crate::token::Keyword::This) => descartar_lt(&mut pilha),
                Kind::Eof => break,
                _ => {}
            }
            if pilha.iter().all(|(_, p)| *p != lt) && i > lt {
                return None;
            }
        }
        None
    }
}
