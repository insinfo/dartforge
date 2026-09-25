//! Parser de Dart 3.6 completo, descendente recursivo, sem resolução.
//!
//! O parser está dividido em módulos por área da gramática, todos como
//! `impl Parser` sobre o mesmo cursor:
//!
//! * [`types`] — anotações de tipo, argumentos e parâmetros de tipo, listas de
//!   parâmetros formais e os *lookaheads* especulativos (`skip_type`);
//! * [`declarations`] — unidade, diretivas, metadata, classes, mixins, enums,
//!   extensions, extension types, typedefs, membros, construtores e corpos de
//!   função;
//! * [`expressions`] — expressões, literais (strings com interpolação e
//!   decodificação de escapes, coleções, records), argumentos, cascatas,
//!   expressões de função e `switch` como expressão;
//! * [`statements`] — statements, declarações locais e cabeçalhos de `for`;
//! * [`patterns`] — padrões (Dart 3).
//!
//! Regras comuns a todos os módulos:
//!
//! * `>` chega sempre isolado do lexer; em contexto de expressão use
//!   [`Parser::composed_gt`] para ler `>>`, `>>>`, `>=`, `>>=` e `>>>=`.
//! * Identificadores embutidos e contextuais (`abstract`, `async`, `show`,
//!   `when`, `on`…) são [`Kind::Ident`]; compare o texto com
//!   [`Parser::at_ident`]. Só palavras reservadas são [`Kind::Keyword`].
//! * Um erro produz um [`Diagnostic`] em [`Parser::diagnostics`] e devolve
//!   [`ParseError`]; o chamador propaga com `?`. A recuperação, quando houver,
//!   fica nas fronteiras de declaração de topo e de membro.
//! * O parser aceita um **superconjunto** de programas válidos onde a
//!   distinção exigiria resolução (por exemplo, qualquer expressão unária
//!   como padrão constante): rejeitar programas inválidos é papel das fases
//!   seguintes, e aceitar todo programa válido é o critério de aceite aqui.
pub mod declarations;
pub mod expressions;
pub mod patterns;
pub mod statements;
pub mod types;

use crate::ast::{Ast, CompilationUnit, ExprId, ForInTarget, ForInit, Name};
use crate::features::{Feature, LibraryFeatures};
use crate::lexer;
use crate::token::{Keyword, Kind, Op, Token};
use dartforge_diagnostics::{Codigo, Diagnostic, Span, codigos};
use dartforge_intern::Interner;

/// Marcador de falha: o diagnóstico já foi registrado em [`Parser::diagnostics`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseError;

/// Resultado das funções do parser.
pub type PResult<T> = Result<T, ParseError>;

/// Composição de `>` isolados em contexto de expressão.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposedGt {
    /// `>`
    Gt,
    /// `>=`
    GtEq,
    /// `>>`
    Shr,
    /// `>>>`
    UShr,
    /// `>>=`
    ShrAssign,
    /// `>>>=`
    UShrAssign,
}

impl ComposedGt {
    /// Quantos tokens a composição consome (nunca zero: `>` sozinho é 1).
    #[allow(clippy::len_without_is_empty)]
    pub fn len(self) -> usize {
        match self {
            ComposedGt::Gt => 1,
            ComposedGt::GtEq | ComposedGt::Shr => 2,
            ComposedGt::UShr | ComposedGt::ShrAssign => 3,
            ComposedGt::UShrAssign => 4,
        }
    }
}

/// Cabeçalho de `for (...)` já consumido, incluindo os parênteses.
#[derive(Debug)]
pub enum ForHeader {
    Classic {
        init: Option<ForInit>,
        condition: Option<ExprId>,
        updates: Vec<ExprId>,
    },
    In {
        target: ForInTarget,
        iterable: ExprId,
    },
}

/// Resultado de [`parse`].
#[derive(Debug)]
pub struct Parsed {
    pub unit: CompilationUnit,
    pub ast: Ast,
    pub diagnostics: Vec<Diagnostic>,
}

/// Analisa uma unidade de compilação inteira na versão de linguagem corrente
/// ([`LibraryFeatures::atual`]); ver [`parse_com`].
///
/// Diagnósticos léxicos interrompem a análise; diagnósticos sintáticos ficam
/// em [`Parsed::diagnostics`], e a unidade devolvida contém o que foi lido
/// até o ponto de recuperação mais recente. Lista vazia significa que a
/// unidade inteira foi aceita pela gramática.
///
/// ```
/// let mut nomes = dartforge_intern::Interner::new();
/// let saida = dartforge_frontend::parser::parse("void main() { print('oi'); }", &mut nomes);
/// assert!(saida.diagnostics.is_empty(), "{:?}", saida.diagnostics);
/// assert_eq!(saida.unit.declarations.len(), 1);
/// ```
pub fn parse(source: &str, interner: &mut Interner) -> Parsed {
    parse_com(source, interner, LibraryFeatures::atual())
}

/// Como [`parse`], com os recursos da biblioteca a que a unidade pertence
/// (`docs/VERSOES-LINGUAGEM.md`). A gramática aceita é sempre o
/// superconjunto; um recurso desligado vira diagnóstico, e onde a gramática
/// **diverge** entre versões (`factory() {}`, `var`/`final` em parâmetro) a
/// versão decide.
pub fn parse_com(source: &str, interner: &mut Interner, features: LibraryFeatures) -> Parsed {
    parse_lexed_com(source, lexer::lex(source), interner, features)
}

/// Como [`parse`], com a fonte já lexada. O lexer é puro (não interna nomes),
/// então pode correr noutra thread; só a análise sintática precisa do
/// `Interner` e fica sequencial.
pub fn parse_lexed(
    source: &str,
    tokens: Result<Vec<Token>, Diagnostic>,
    interner: &mut Interner,
) -> Parsed {
    parse_lexed_com(source, tokens, interner, LibraryFeatures::atual())
}

/// [`parse_lexed`] com os recursos da biblioteca; ver [`parse_com`].
pub fn parse_lexed_com(
    source: &str,
    tokens: Result<Vec<Token>, Diagnostic>,
    interner: &mut Interner,
    features: LibraryFeatures,
) -> Parsed {
    let tokens = match tokens {
        Ok(tokens) => tokens,
        Err(diagnostic) => {
            return Parsed {
                unit: CompilationUnit::default(),
                ast: Ast::default(),
                diagnostics: vec![diagnostic],
            };
        }
    };
    let mut parser = Parser::new(source, tokens, interner);
    parser.features = features;
    let unit = parser.parse_compilation_unit();
    let mut ast = parser.ast;
    // A árvore devolvida vive muito (um editor a retém por arquivo aberto) e
    // as arenas cresceram em potências de dois: no `new_sali` a folga era
    // 29 MiB de 132 MiB. Uma realocação por arena aqui é mais barata do que
    // reter a folga pela vida inteira da unidade.
    ast.shrink_to_fit();
    Parsed {
        unit,
        ast,
        diagnostics: parser.diagnostics,
    }
}

/// Cursor sobre os tokens de uma unidade mais as arenas em construção.
pub struct Parser<'s, 'i> {
    pub(crate) source: &'s str,
    pub(crate) tokens: Vec<Token>,
    pub(crate) pos: usize,
    pub(crate) ast: Ast,
    pub(crate) interner: &'i mut Interner,
    pub(crate) diagnostics: Vec<Diagnostic>,
    /// Profundidade de aninhamento de expressões/statements, para recusar
    /// entradas patológicas antes de estourar a pilha.
    pub(crate) depth: u32,
    /// Dentro de um corpo `async`/`async*`: `await` e `yield` são palavras-chave.
    pub(crate) in_async: bool,
    /// Dentro de um corpo `sync*`/`async*`: `yield` é palavra-chave.
    pub(crate) in_generator: bool,
    /// Dentro de `<...>` de tipo: `>` nunca se compõe.
    pub(crate) in_type_args: u32,
    /// Rascunho de argumentos de chamada, compartilhado por todas as
    /// chamadas (aninhadas empilham acima da anterior); ver
    /// [`Parser::parse_arguments`].
    pub(crate) scratch_args: Vec<crate::ast::Argument>,
    /// Rascunho de trechos de string literal; ver
    /// [`Parser::parse_string_literal`].
    pub(crate) scratch_parts: Vec<crate::ast::StringPart>,
    /// Recursos da biblioteca (versão de linguagem e experimentos).
    pub(crate) features: LibraryFeatures,
    /// Lendo a lista de parâmetros de um construtor primário (Dart 3.13):
    /// `var`/`final` declaram campo e nomeado privado declarante é permitido.
    pub(crate) em_construtor_primario: bool,
}

/// Limite de aninhamento antes de um diagnóstico de profundidade.
pub const MAX_DEPTH: u32 = 512;

impl<'s, 'i> Parser<'s, 'i> {
    pub(crate) fn new(source: &'s str, tokens: Vec<Token>, interner: &'i mut Interner) -> Self {
        Parser {
            source,
            tokens,
            pos: 0,
            ast: Ast::default(),
            interner,
            diagnostics: Vec::new(),
            depth: 0,
            in_async: false,
            in_generator: false,
            in_type_args: 0,
            scratch_args: Vec::new(),
            scratch_parts: Vec::new(),
            features: LibraryFeatures::atual(),
            em_construtor_primario: false,
        }
    }

    // -- Cursor ---------------------------------------------------------

    /// Token corrente (o último é sempre [`Kind::Eof`]).
    pub(crate) fn peek(&self) -> Token {
        self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    /// Token `n` posições à frente, saturando em `Eof`.
    pub(crate) fn peek_at(&self, n: usize) -> Token {
        self.tokens[(self.pos + n).min(self.tokens.len() - 1)]
    }

    pub(crate) fn kind(&self) -> Kind {
        self.peek().kind
    }

    pub(crate) fn kind_at(&self, n: usize) -> Kind {
        self.peek_at(n).kind
    }

    pub(crate) fn span(&self) -> Span {
        self.peek().span
    }

    /// Texto do token corrente.
    pub(crate) fn text(&self) -> &'s str {
        let token = self.peek();
        &self.source[token.span.start..token.span.end]
    }

    pub(crate) fn text_at(&self, n: usize) -> &'s str {
        let token = self.peek_at(n);
        &self.source[token.span.start..token.span.end]
    }

    /// Texto de um token qualquer.
    pub(crate) fn token_text(&self, token: Token) -> &'s str {
        &self.source[token.span.start..token.span.end]
    }

    /// Avança e devolve o token consumido.
    pub(crate) fn advance(&mut self) -> Token {
        let token = self.peek();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        token
    }

    pub(crate) fn at_eof(&self) -> bool {
        self.kind() == Kind::Eof
    }

    pub(crate) fn at_op(&self, op: Op) -> bool {
        self.kind() == Kind::Op(op)
    }

    pub(crate) fn at_op_at(&self, n: usize, op: Op) -> bool {
        self.kind_at(n) == Kind::Op(op)
    }

    pub(crate) fn at_kw(&self, kw: Keyword) -> bool {
        self.kind() == Kind::Keyword(kw)
    }

    pub(crate) fn at_kw_at(&self, n: usize, kw: Keyword) -> bool {
        self.kind_at(n) == Kind::Keyword(kw)
    }

    /// O token corrente é um identificador (qualquer, inclusive embutido).
    pub(crate) fn at_identifier(&self) -> bool {
        self.kind() == Kind::Ident
    }

    pub(crate) fn at_identifier_at(&self, n: usize) -> bool {
        self.kind_at(n) == Kind::Ident
    }

    /// O token corrente é o identificador contextual `text`.
    pub(crate) fn at_ident(&self, text: &str) -> bool {
        self.kind() == Kind::Ident && self.text() == text
    }

    pub(crate) fn at_ident_at(&self, n: usize, text: &str) -> bool {
        self.kind_at(n) == Kind::Ident && self.text_at(n) == text
    }

    /// Consome o operador se for o corrente.
    pub(crate) fn eat_op(&mut self, op: Op) -> bool {
        if self.at_op(op) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub(crate) fn eat_kw(&mut self, kw: Keyword) -> bool {
        if self.at_kw(kw) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Consome o identificador contextual se for o corrente.
    pub(crate) fn eat_ident(&mut self, text: &str) -> bool {
        if self.at_ident(text) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub(crate) fn expect_op(&mut self, op: Op) -> PResult<Token> {
        if self.at_op(op) {
            Ok(self.advance())
        } else {
            Err(self.erro_esperado(op.text()))
        }
    }

    pub(crate) fn expect_kw(&mut self, kw: Keyword) -> PResult<Token> {
        if self.at_kw(kw) {
            Ok(self.advance())
        } else {
            Err(self.erro_esperado(kw.text()))
        }
    }

    /// Exige o identificador contextual `text`.
    pub(crate) fn expect_ident(&mut self, text: &str) -> PResult<Token> {
        if self.at_ident(text) {
            Ok(self.advance())
        } else {
            Err(self.erro_esperado(text))
        }
    }

    /// Exige um identificador qualquer e devolve seu [`Name`].
    pub(crate) fn expect_identifier(&mut self) -> PResult<Name> {
        if self.at_identifier() {
            Ok(self.identifier())
        } else {
            Err(self.erro_identificador())
        }
    }

    /// Consome o identificador corrente (precondição: [`Parser::at_identifier`]).
    pub(crate) fn identifier(&mut self) -> Name {
        let token = self.advance();
        let sym = self
            .interner
            .intern(&self.source[token.span.start..token.span.end]);
        Name {
            sym,
            span: token.span,
        }
    }

    /// Interna um texto arbitrário como nome (operadores, `unary-`, `[]=`).
    pub(crate) fn name_from(&mut self, text: &str, span: Span) -> Name {
        Name {
            sym: self.interner.intern(text),
            span,
        }
    }

    /// Span do início dado até o fim do último token consumido.
    pub(crate) fn span_from(&self, start: Span) -> Span {
        let end = if self.pos == 0 {
            start.end
        } else {
            self.tokens[self.pos - 1].span.end
        };
        Span {
            start: start.start,
            end: end.max(start.start),
        }
    }

    /// Fim do último token consumido.
    pub(crate) fn last_end(&self) -> usize {
        if self.pos == 0 {
            0
        } else {
            self.tokens[self.pos - 1].span.end
        }
    }

    // -- Diagnósticos ---------------------------------------------------

    /// Registra o diagnóstico `codigo` (do `ParserErrorCode`/`ScannerErrorCode`
    /// do analyzer, mensagem oficial em inglês) em `span` e devolve o marcador.
    pub(crate) fn erro_em(&mut self, codigo: Codigo, span: Span, args: &[&str]) -> ParseError {
        self.diagnostics.push(Diagnostic::com_codigo(codigo, span, args.iter().copied()));
        ParseError
    }

    /// `codigo` no token corrente — onde o fasta reporta o que falta ou sobra.
    pub(crate) fn erro(&mut self, codigo: Codigo, args: &[&str]) -> ParseError {
        let span = self.span();
        self.erro_em(codigo, span, args)
    }

    /// `EXPECTED_TOKEN`, no lugar em que o analyzer o põe: `;` que falta vai no
    /// token **anterior** (`ensureSemicolon` do fasta); fecho `)`/`]`/`}` que
    /// falta no fim do arquivo é erro do scanner, no fim, com comprimento 1
    /// (`translateErrorToken`); o resto, no token corrente.
    pub(crate) fn erro_esperado(&mut self, texto: &str) -> ParseError {
        if texto == ";" && self.pos > 0 {
            let span = self.tokens[self.pos - 1].span;
            return self.erro_em(codigos::parser::EXPECTED_TOKEN, span, &[";"]);
        }
        if matches!(texto, ")" | "]" | "}") && self.kind() == Kind::Eof {
            let s = self.span().start;
            return self.erro_em(codigos::scanner::EXPECTED_TOKEN, Span { start: s, end: s + 1 }, &[texto]);
        }
        self.erro(codigos::parser::EXPECTED_TOKEN, &[texto])
    }

    /// Faltou um comando: `MISSING_STATEMENT` no token corrente, precedido de
    /// `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD` se ele é uma palavra reservada
    /// (o fasta tenta o comando de expressão e tropeça no identificador).
    pub(crate) fn erro_statement(&mut self) -> ParseError {
        if let Kind::Keyword(_) = self.kind() {
            let texto = self.text().to_string();
            let span = self.span();
            self.erro_em(codigos::parser::EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD, span, &[&texto]);
        }
        self.erro(codigos::parser::MISSING_STATEMENT, &[])
    }

    /// Faltou um identificador: `MISSING_IDENTIFIER` no token corrente, ou
    /// `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD` se ele é uma palavra reservada.
    pub(crate) fn erro_identificador(&mut self) -> ParseError {
        if let Kind::Keyword(_) = self.kind() {
            let texto = self.text().to_string();
            return self.erro(codigos::parser::EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD, &[&texto]);
        }
        self.erro(codigos::parser::MISSING_IDENTIFIER, &[])
    }
    /// Registra que `span` usa o recurso `f`: diagnóstico se a versão da
    /// biblioteca não o liga. A análise continua (o superconjunto é aceito).
    pub(crate) fn exigir(&mut self, f: Feature, span: Span) {
        if !self.features.tem(f) {
            // `EXPERIMENT_NOT_ENABLED` (com a versão em que o recurso liga) ou
            // `EXPERIMENT_NOT_ENABLED_OFF_BY_DEFAULT` (experimento sem versão).
            match f.habilitado_em() {
                Some(v) => {
                    let versao = format!("{v}.0");
                    self.erro_em(codigos::parser::EXPERIMENT_NOT_ENABLED, span, &[f.nome(), &versao]);
                }
                None => {
                    self.erro_em(codigos::parser::EXPERIMENT_NOT_ENABLED_OFF_BY_DEFAULT, span, &[f.nome()]);
                }
            }
        }
    }

    /// Como [`Parser::exigir`], para os recursos que o analyzer confere ao
    /// montar a árvore (`AstBuilder`), não no parser do fasta: a correção
    /// leva a versão como `x.y` (`3.13`), não `x.y.0`. São o corpo `;`, os
    /// membros `this`/`new` e o `final` declarante (medido no oráculo 3.13.4).
    pub(crate) fn exigir_no_ast(&mut self, f: Feature, span: Span) {
        if self.features.tem(f) {
            return;
        }
        match f.habilitado_em() {
            Some(v) => {
                let versao = v.to_string();
                self.erro_em(codigos::parser::EXPERIMENT_NOT_ENABLED, span, &[f.nome(), &versao]);
            }
            None => self.exigir(f, span),
        }
    }

    /// Entra num nível de aninhamento; falha além de [`MAX_DEPTH`].
    pub(crate) fn enter(&mut self) -> PResult<()> {
        if self.depth >= MAX_DEPTH {
            return Err(self.erro(codigos::parser::STACK_OVERFLOW, &[]));
        }
        self.depth += 1;
        Ok(())
    }

    pub(crate) fn leave(&mut self) {
        self.depth -= 1;
    }

    // -- Composição de `>` ---------------------------------------------

    /// Lê, sem consumir, a composição de `>` que começa no token corrente.
    ///
    /// Só compõe tokens **colados** (`glued`), então `a > >b` nunca vira
    /// shift. Devolve `None` se o token corrente não é `>`.
    pub(crate) fn composed_gt(&self) -> Option<ComposedGt> {
        if !self.at_op(Op::Gt) {
            return None;
        }
        let t0 = self.peek();
        let t1 = self.peek_at(1);
        let t2 = self.peek_at(2);
        let t3 = self.peek_at(3);
        let g1 = t0.glued && t1.kind == Kind::Op(Op::Gt);
        let g1_eq = t0.glued && t1.kind == Kind::Op(Op::Assign);
        if !g1 {
            return Some(if g1_eq {
                ComposedGt::GtEq
            } else {
                ComposedGt::Gt
            });
        }
        let g2 = t1.glued && t2.kind == Kind::Op(Op::Gt);
        let g2_eq = t1.glued && t2.kind == Kind::Op(Op::Assign);
        if !g2 {
            return Some(if g2_eq {
                ComposedGt::ShrAssign
            } else {
                ComposedGt::Shr
            });
        }
        let g3_eq = t2.glued && t3.kind == Kind::Op(Op::Assign);
        Some(if g3_eq {
            ComposedGt::UShrAssign
        } else {
            ComposedGt::UShr
        })
    }

    /// Consome a composição devolvida por [`Parser::composed_gt`].
    pub(crate) fn eat_composed_gt(&mut self, composed: ComposedGt) {
        for _ in 0..composed.len() {
            self.advance();
        }
    }

    // -- Utilidades de lookahead ----------------------------------------

    /// Índice do token que fecha o delimitador aberto em `open_pos`, ou
    /// `None` se não houver. Considera `( ) [ ] { }` e trata `<`/`>` **não**
    /// como delimitadores. Usado por lookaheads que precisam pular um grupo.
    pub(crate) fn matching_close(&self, open_pos: usize) -> Option<usize> {
        let mut depth = 0usize;
        let mut i = open_pos;
        while i < self.tokens.len() {
            match self.tokens[i].kind {
                Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => depth += 1,
                Kind::Op(Op::RParen | Op::RBracket | Op::RBrace) => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                Kind::Eof => return None,
                _ => {}
            }
            i += 1;
        }
        None
    }

    /// Texto do token na posição absoluta `pos`.
    pub(crate) fn text_of(&self, pos: usize) -> &'s str {
        let token = self.tokens[pos.min(self.tokens.len() - 1)];
        &self.source[token.span.start..token.span.end]
    }

    /// Kind do token na posição absoluta `pos`.
    pub(crate) fn kind_of(&self, pos: usize) -> Kind {
        self.tokens[pos.min(self.tokens.len() - 1)].kind
    }
}
