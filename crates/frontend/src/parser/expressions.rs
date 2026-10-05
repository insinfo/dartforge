//! Expressões, literais, argumentos, cascatas e expressões de função.
//!
//! A gramática de expressões de Dart 3.6 (§17 da especificação) é lida por
//! descida recursiva com *precedence climbing* para os operadores binários:
//! uma única função ([`Parser::parse_binary`]) trata todos os níveis de `??`
//! até `*`, então a profundidade da pilha cresce com o aninhamento real da
//! expressão e não com o número de níveis da gramática.
//!
//! Decisões de desambiguação que seguem o parser do SDK
//! (`parser_impl.dart`), porque o corpus alvo foi validado por ele:
//!
//! * `>` chega isolado do lexer; [`Parser::composed_gt`] o recompõe em
//!   `>=`, `>>`, `>>>`, `>>=`, `>>>=` só entre tokens colados e só fora de
//!   listas de argumentos de tipo.
//! * `e<T>(...)`/`e<T>.x`: `<...>` é lido como argumentos de tipo quando a
//!   lista é bem formada **e** o token seguinte é um dos que podem seguir
//!   argumentos de tipo (`(`, `.`, `==`, `!=`, `)`, `]`, `}`, `;`, `:`, `,`,
//!   fim de interpolação, fim do arquivo). Assim `a < b > c` é relacional e
//!   `f<int>(1)` é chamada genérica.
//! * `e?[i]` contra `c ? [a] : b`: como no SDK, tenta-se ler o que segue o
//!   `?` como `expressão : expressão` de forma especulativa; se der certo é
//!   uma condicional, senão é índice null-aware. A especulação restaura o
//!   cursor, as arenas e os diagnósticos.
//! * `(` em posição primária é expressão de função se depois do `)`
//!   correspondente vem `{`, `=>`, `async` ou `sync`; senão é record ou
//!   expressão entre parênteses (record quando há vírgula, campo nomeado ou
//!   nada dentro).
//! * `new`/`const T.n()` guarda o nome como o SDK: `T.n` sem argumentos de
//!   tipo vira nome de duas partes e `constructor` fica `None`; só depois de
//!   argumentos de tipo (ou de um nome já qualificado) o `.n` vira
//!   `constructor`. Distinguir prefixo de biblioteca de nome de construtor
//!   exige resolução.
//! * `throw` e `rethrow` só são aceitos no nível de `expression`, como na
//!   gramática (`x ?? throw e` exige parênteses, e o corpus os tem).
//! * `await` é operador em corpo `async`; fora dele é identificador, salvo
//!   quando parece expressão (`looksLikeAwaitExpression`, com erro).
//!
//! Em padrões constantes (`parse_unary(true)`) o `!` final que
//! não é seguido de seletor fica para o parser de padrões (é um padrão de
//! asserção de nulo), exatamente como o SDK faz com `ConstantPatternContext`.
use super::{ComposedGt, ForHeader, PResult, Parser};
use crate::ast::{
    Argument, Arguments, AssignOp, BinaryOp, CollectionElement, CreationKeyword, Expr, ExprId,
    ExprKind, Function, FunctionKind, Name, StringLit, StringPart, SwitchExprCase, TypeAnnotation,
    TypeId, TypeKind, UnaryOp,
};
use crate::features::Feature;
use crate::token::{Interp, Keyword, Kind, Op, StrFlags, Token};
use dartforge_diagnostics::{Span, codigos};

/// Níveis de precedência dos operadores binários, do mais baixo ao mais alto.
/// Os valores são contíguos porque [`Parser::parse_binary`] usa `nível + 1`
/// para o operando direito (associatividade à esquerda).
const LEVEL_IF_NULL: u8 = 0;
const LEVEL_OR: u8 = 1;
const LEVEL_AND: u8 = 2;
pub(crate) const LEVEL_EQUALITY: u8 = 3;
const LEVEL_RELATIONAL: u8 = 4;
const LEVEL_BIT_OR: u8 = 5;
const LEVEL_BIT_XOR: u8 = 6;
const LEVEL_BIT_AND: u8 = 7;
const LEVEL_SHIFT: u8 = 8;
const LEVEL_ADDITIVE: u8 = 9;
const LEVEL_MULTIPLICATIVE: u8 = 10;

/// Operador binário encontrado no cursor, com seu nível e quantos tokens ocupa.
#[derive(Debug, Clone, Copy)]
enum BinaryHere {
    Op { level: u8, op: BinaryOp, len: usize },
    Is,
    As,
}

/// Estado salvo para análise especulativa (`?[` contra `? [...] :`).
///
/// Restaurar descarta os nós criados e os diagnósticos emitidos durante a
/// especulação; como as arenas são só anexadas e nada fora da especulação
/// referencia os nós novos, truncar é suficiente.
struct Checkpoint {
    pos: usize,
    exprs: usize,
    stmts: usize,
    types: usize,
    patterns: usize,
    decls: usize,
    members: usize,
    functions: usize,
    diagnostics: usize,
    depth: u32,
    in_async: bool,
    in_generator: bool,
    in_type_args: u32,
}

/// O primeiro erro de escape de `content` (o texto entre as aspas, não cru)
/// como `unescapeCodeUnits` do fasta (`quote.dart:186`) o relata: código,
/// início relativo (a `\`) e comprimento.
fn erro_de_escape(b: &[u8]) -> Option<(dartforge_diagnostics::Codigo, usize, usize)> {
    use codigos::parser as c;
    let n = b.len();
    let hex = |x: u8| x.is_ascii_hexdigit();
    let valor = |x: u8| (x as char).to_digit(16).unwrap_or(0);
    let mut i = 0;
    while i < n {
        if b[i] != b'\\' {
            i += 1;
            continue;
        }
        i += 1;
        if i == n {
            return Some((c::INVALID_UNICODE_ESCAPE_STARTED, i - 1, 1));
        }
        // `begin` do fasta é o índice do `x`/`u`; o erro começa na `\`.
        let begin = i;
        match b[i] {
            b'x' => {
                if n <= i + 2 {
                    return Some((c::INVALID_HEX_ESCAPE, begin - 1, n + 1 - begin));
                }
                for _ in 0..2 {
                    i += 1;
                    if !hex(b[i]) {
                        return Some((c::INVALID_HEX_ESCAPE, begin - 1, i + 1 - begin));
                    }
                }
            }
            b'u' => {
                if n == i + 1 {
                    return Some((c::INVALID_UNICODE_ESCAPE_U_STARTED, begin - 1, n + 1 - begin));
                }
                let mut code: u32 = 0;
                if b[i + 1] == b'{' {
                    i += 1;
                    let mut fechou = false;
                    for j in 0..7 {
                        i += 1;
                        if i == n {
                            return Some((c::INVALID_UNICODE_ESCAPE_U_BRACKET, begin - 1, i + 1 - begin));
                        }
                        let d = b[i];
                        if j != 0 && d == b'}' {
                            fechou = true;
                            break;
                        } else if j == 6 {
                            break;
                        }
                        if !hex(d) {
                            return Some((c::INVALID_UNICODE_ESCAPE_U_BRACKET, begin - 1, i + 2 - begin));
                        }
                        code = (code << 4) + valor(d);
                    }
                    if !fechou {
                        return Some((c::INVALID_UNICODE_ESCAPE_U_BRACKET, begin - 1, i + 1 - begin));
                    }
                } else {
                    if n <= i + 4 {
                        return Some((c::INVALID_UNICODE_ESCAPE_U_NO_BRACKET, begin - 1, n + 1 - begin));
                    }
                    for _ in 0..4 {
                        i += 1;
                        if !hex(b[i]) {
                            return Some((c::INVALID_UNICODE_ESCAPE_U_NO_BRACKET, begin - 1, i + 1 - begin));
                        }
                        code = (code << 4) + valor(b[i]);
                    }
                }
                if code > 0x10FFFF {
                    return Some((c::INVALID_CODE_POINT, begin - 1, i + 1 - begin));
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

impl<'s, 'i> Parser<'s, 'i> {
    // -- Entradas públicas do contrato ----------------------------------

    /// `expression` completa, inclusive atribuição, cascata e `throw`.
    pub(crate) fn parse_expression(&mut self) -> PResult<ExprId> {
        self.parse_expression_ex(true)
    }

    /// `expressionWithoutCascade` — usada em argumentos de cascata e em
    /// lugares onde `..` fecharia a expressão.
    pub(crate) fn parse_expression_without_cascade(&mut self) -> PResult<ExprId> {
        self.parse_expression_ex(false)
    }

    /// `bitwiseOrExpression` — operando de padrões relacionais.
    pub(crate) fn parse_bitwise_or_expression(&mut self) -> PResult<ExprId> {
        self.parse_binary(LEVEL_BIT_OR)
    }

    /// Separadores de dígito do literal numérico em `span`: fora de lugar
    /// (não entre dois dígitos), `UNEXPECTED_SEPARATOR_IN_NUMBER` no início
    /// do literal, com comprimento 1 (o token de erro do scanner); numa
    /// biblioteca antes da 3.6, `EXPERIMENT_NOT_ENABLED` no literal inteiro
    /// (`digit-separators` entrou na 3.6).
    fn conferir_separadores(&mut self, span: Span) {
        let texto = &self.source.as_bytes()[span.start..span.end];
        if !texto.contains(&b'_') {
            return;
        }
        let hex = texto.len() > 1 && matches!(texto[1], b'x' | b'X');
        let digito = |b: u8| if hex { b.is_ascii_hexdigit() } else { b.is_ascii_digit() };
        let inicio = if hex { 2 } else { 0 };
        let mut fora = false;
        for (i, &b) in texto.iter().enumerate().skip(inicio) {
            if b != b'_' {
                continue;
            }
            let antes = texto[inicio..i].iter().rev().find(|&&c| c != b'_');
            let depois = texto[i + 1..].iter().find(|&&c| c != b'_');
            if !antes.is_some_and(|&c| digito(c)) || !depois.is_some_and(|&c| digito(c)) {
                fora = true;
                break;
            }
        }
        if fora {
            let um = Span { start: span.start, end: span.start + 1 };
            self.erro_em(codigos::scanner::UNEXPECTED_SEPARATOR_IN_NUMBER, um, &[]);
        }
        if self.features.versao() < crate::features::LanguageVersion::new(3, 6) {
            self.erro_em(codigos::parser::EXPERIMENT_NOT_ENABLED, span, &["digit-separators", "3.6.0", "3.6.0"]);
        }
    }

    /// `(args)` com os parênteses; sem argumentos de tipo (o chamador os lê).
    ///
    /// Um argumento é nomeado quando é um identificador seguido de `:`.
    pub(crate) fn parse_arguments(&mut self) -> PResult<Arguments> {
        let abre = self.pos;
        let start = self.expect_op(Op::LParen)?.span;
        // Os argumentos vão para o rascunho compartilhado a partir de `base`
        // (chamadas aninhadas empilham acima) e saem numa única alocação de
        // tamanho exato. Um `Vec` local cresceria em dobro e realocaria ao
        // virar `Box<[T]>`: no `new_sali` eram 57 mil chamadas.
        let base = self.scratch_args.len();
        let result = (|| {
            while !self.at_op(Op::RParen) {
                let name = if self.at_identifier() && self.at_op_at(1, Op::Colon) {
                    let name = self.identifier();
                    self.advance();
                    Some(name)
                } else {
                    None
                };
                let value = self.parse_expression()?;
                self.scratch_args.push(Argument { name, value });
                if !self.eat_op(Op::Comma) {
                    // `parseArgumentsRest`: o que parece começar outra
                    // expressão é um argumento depois de uma vírgula que
                    // falta; o resto fecha em `ensureCloseParen`.
                    if !self.at_op(Op::RParen) && self.parece_inicio_de_expressao(self.pos) {
                        self.erro(codigos::parser::EXPECTED_TOKEN, &[","]);
                        continue;
                    }
                    break;
                }
            }
            self.garantir_fecha_parenteses(abre).map(|()| self.tokens[self.pos - 1])
        })();
        let args: Box<[Argument]> = self.scratch_args[base..].into();
        self.scratch_args.truncate(base);
        result?;
        Ok(Arguments {
            span: self.span_from(start),
            type_args: Box::default(),
            args,
        })
    }

    /// Literal de string (com adjacentes e interpolação) em posição de URI ou
    /// de `native`.
    ///
    /// Literais adjacentes (`'a' 'b'`) viram um só [`StringLit`] com os
    /// trechos concatenados em `parts`. Cada trecho de texto já vem
    /// decodificado ([`crate::lexer::decode_string`]).
    pub(crate) fn parse_string_literal(&mut self) -> PResult<StringLit> {
        let start = self.span();
        // Trechos vão para o rascunho compartilhado a partir de `base` (uma
        // string dentro de interpolação empilha acima) e saem numa alocação
        // exata: a maioria dos literais tem um trecho só, e um `Vec` local
        // reservava quatro.
        let base = self.scratch_parts.len();
        let result = (|| {
            let mut any = false;
            loop {
                match self.kind() {
                    Kind::Str(flags) => {
                        let token = self.advance();
                        self.push_string_text(token, flags, true, false)?;
                    }
                    Kind::StrBegin(flags, interp) => {
                        let token = self.advance();
                        self.push_string_text(token, flags, true, false)?;
                        self.parse_interpolations(interp)?;
                    }
                    _ => break,
                }
                any = true;
            }
            if !any {
                return Err(self.erro(codigos::parser::EXPECTED_STRING_LITERAL, &[]));
            }
            Ok(())
        })();
        let parts: Box<[StringPart]> = self.scratch_parts.drain(base..).collect();
        result?;
        Ok(StringLit {
            span: self.span_from(start),
            parts,
        })
    }

    // -- Níveis inferiores: throw, atribuição por padrão, atribuição -------

    /// `expression` ou `expressionWithoutCascade`, conforme `allow_cascade`.
    fn parse_expression_ex(&mut self, allow_cascade: bool) -> PResult<ExprId> {
        self.enter()?;
        let result = self.parse_expression_inner(allow_cascade);
        self.leave();
        result
    }

    fn parse_expression_inner(&mut self, allow_cascade: bool) -> PResult<ExprId> {
        let start = self.span();
        if self.eat_kw(Keyword::Throw) {
            let value = self.parse_expression_ex(allow_cascade)?;
            return Ok(self.push(start, ExprKind::Throw(value)));
        }
        if self.eat_kw(Keyword::Rethrow) {
            return Ok(self.push(start, ExprKind::Rethrow));
        }
        // `allowPatterns` do fasta: antes da 3.0 (padrões), `(a) = 0` é
        // atribuição a expressão entre parênteses.
        if self.features.versao() >= crate::features::LanguageVersion::new(3, 0)
            && self.pattern_assignment_ahead()
            && self.looks_like_pattern_assignment(self.pos)
        {
            let pattern = self.parse_pattern_em(super::patterns::ContextoDePadrao::Atribuicao)?;
            self.expect_op(Op::Assign)?;
            let value = self.parse_expression_ex(allow_cascade)?;
            return Ok(self.push(start, ExprKind::PatternAssign { pattern, value }));
        }
        let mut left = self.parse_conditional()?;
        if allow_cascade && matches!(self.kind(), Kind::Op(Op::DotDot | Op::QuestionDotDot)) {
            left = self.parse_cascade_rest(start, left)?;
        }
        if let Some((op, len)) = self.assignment_op_here() {
            for _ in 0..len {
                self.advance();
            }
            let value = self.parse_expression_ex(allow_cascade)?;
            self.conferir_atribuicao(left, value);
            left = self.push(
                start,
                ExprKind::Assign {
                    op,
                    target: left,
                    value,
                },
            );
        }
        Ok(left)
    }

    /// Pré-filtro barato de atribuição por padrão: um `outerPattern`
    /// (`(…)`, `[…]`, `{…}`, `<T>[…]`, `<T>{…}`, `Nome(…)`, `p.Nome<T>(…)`)
    /// cujo delimitador de fechamento é seguido de `=`. Só então vale a pena
    /// consultar `looks_like_pattern_assignment`, que confere o conteúdo.
    fn pattern_assignment_ahead(&self) -> bool {
        let mut i = self.pos;
        match self.kind_of(i) {
            Kind::Ident => {
                i += 1;
                if self.kind_of(i) == Kind::Op(Op::Dot) {
                    if self.kind_of(i + 1) != Kind::Ident {
                        return false;
                    }
                    i += 2;
                }
                if self.kind_of(i) == Kind::Op(Op::Lt) {
                    let Some(close) = self.angle_close(i) else {
                        return false;
                    };
                    i = close + 1;
                }
                if self.kind_of(i) != Kind::Op(Op::LParen) {
                    return false;
                }
            }
            Kind::Op(Op::Lt) => {
                let Some(close) = self.angle_close(i) else {
                    return false;
                };
                i = close + 1;
                if !matches!(self.kind_of(i), Kind::Op(Op::LBracket | Op::LBrace)) {
                    return false;
                }
            }
            Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => {}
            _ => return false,
        }
        let Some(close) = self.matching_close(i) else {
            return false;
        };
        self.kind_of(close + 1) == Kind::Op(Op::Assign)
    }

    /// Índice do `>` que fecha o `<` em `open`, contando aninhamento, ou
    /// `None` se antes disso aparecer `{`, `}`, `;` ou o fim do arquivo
    /// (nada disso cabe numa lista de parâmetros ou argumentos de tipo).
    fn angle_close(&self, open: usize) -> Option<usize> {
        let mut depth = 0usize;
        let mut i = open;
        loop {
            match self.kind_of(i) {
                Kind::Op(Op::Lt) => depth += 1,
                Kind::Op(Op::Gt) => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                Kind::Op(Op::LBrace | Op::RBrace | Op::Semicolon) | Kind::Eof => return None,
                _ => {}
            }
            i += 1;
        }
    }

    /// Operador de atribuição no cursor e quantos tokens ocupa (`>>=` e
    /// `>>>=` chegam como `>` isolados).
    fn assignment_op_here(&self) -> Option<(AssignOp, usize)> {
        let op = match self.kind() {
            Kind::Op(Op::Assign) => AssignOp::Assign,
            Kind::Op(Op::PlusAssign) => AssignOp::Compound(BinaryOp::Add),
            Kind::Op(Op::MinusAssign) => AssignOp::Compound(BinaryOp::Sub),
            Kind::Op(Op::StarAssign) => AssignOp::Compound(BinaryOp::Mul),
            Kind::Op(Op::SlashAssign) => AssignOp::Compound(BinaryOp::Div),
            Kind::Op(Op::TildeSlashAssign) => AssignOp::Compound(BinaryOp::TruncDiv),
            Kind::Op(Op::PercentAssign) => AssignOp::Compound(BinaryOp::Rem),
            Kind::Op(Op::LtLtAssign) => AssignOp::Compound(BinaryOp::Shl),
            Kind::Op(Op::AmpAssign) => AssignOp::Compound(BinaryOp::BitAnd),
            Kind::Op(Op::PipeAssign) => AssignOp::Compound(BinaryOp::BitOr),
            Kind::Op(Op::CaretAssign) => AssignOp::Compound(BinaryOp::BitXor),
            Kind::Op(Op::QuestionQuestionAssign) => AssignOp::Compound(BinaryOp::IfNull),
            Kind::Op(Op::Gt) => {
                return match self.gt_here()? {
                    ComposedGt::ShrAssign => Some((AssignOp::Compound(BinaryOp::Shr), 3)),
                    ComposedGt::UShrAssign => Some((AssignOp::Compound(BinaryOp::UShr), 4)),
                    _ => None,
                };
            }
            _ => return None,
        };
        Some((op, 1))
    }

    /// Composição de `>` válida no contexto corrente: dentro de argumentos de
    /// tipo `>` nunca vira shift (só `>` ou `>=`).
    fn gt_here(&self) -> Option<ComposedGt> {
        let composed = self.composed_gt()?;
        if self.in_type_args == 0 {
            return Some(composed);
        }
        Some(match composed {
            ComposedGt::GtEq => ComposedGt::GtEq,
            _ => ComposedGt::Gt,
        })
    }

    // -- Cascata ---------------------------------------------------------

    /// Seções `..x`, `?..x`, `..[i]`, cada uma uma expressão cujo receptor
    /// mais interno é [`ExprKind::CascadeTarget`]. Uma seção pode terminar em
    /// atribuição, cujo lado direito é `expressionWithoutCascade`.
    fn parse_cascade_rest(&mut self, start: Span, target: ExprId) -> PResult<ExprId> {
        let null_aware = self.at_op(Op::QuestionDotDot);
        let mut sections = Vec::new();
        while matches!(self.kind(), Kind::Op(Op::DotDot | Op::QuestionDotDot)) {
            let dots = self.advance().span;
            let section_start = Span {
                start: dots.end,
                end: dots.end,
            };
            let receiver = self.push(section_start, ExprKind::CascadeTarget);
            let mut section = if self.at_op(Op::LBracket) {
                self.advance();
                let index = self.parse_expression()?;
                self.expect_op(Op::RBracket)?;
                self.push(
                    section_start,
                    ExprKind::Index {
                        target: receiver,
                        index,
                        null_aware: false,
                    },
                )
            } else {
                let name = self.member_name()?;
                self.push(
                    section_start,
                    ExprKind::Property {
                        target: receiver,
                        name,
                        null_aware: false,
                    },
                )
            };
            section = self.parse_selectors(section_start, section, false)?;
            if let Some((op, len)) = self.assignment_op_here() {
                for _ in 0..len {
                    self.advance();
                }
                let value = self.parse_expression_without_cascade()?;
                self.conferir_atribuicao(section, value);
                section = self.push(
                    section_start,
                    ExprKind::Assign {
                        op,
                        target: section,
                        value,
                    },
                );
            }
            sections.push(section);
        }
        Ok(self.push(
            start,
            ExprKind::Cascade {
                target,
                sections: sections.into_boxed_slice(),
                null_aware,
            },
        ))
    }

    // -- Condicional e binários -------------------------------------------

    /// `c ? a : b`, associativa à direita; os ramos são
    /// `expressionWithoutCascade`.
    fn parse_conditional(&mut self) -> PResult<ExprId> {
        let start = self.span();
        let condition = self.parse_binary(LEVEL_IF_NULL)?;
        if !self.at_op(Op::Question) {
            return Ok(condition);
        }
        self.advance();
        let then = self.parse_expression_without_cascade()?;
        self.expect_op(Op::Colon)?;
        let else_ = self.parse_expression_without_cascade()?;
        Ok(self.push(
            start,
            ExprKind::Conditional {
                condition,
                then,
                else_,
            },
        ))
    }

    /// Operadores binários de `??` a `*` por *precedence climbing*: lê um
    /// operando unário e, enquanto o operador no cursor tem nível ≥
    /// `min_level`, lê o operando direito no nível seguinte. `is`/`as` entram
    /// no nível relacional com um tipo à direita.
    fn parse_binary(&mut self, min_level: u8) -> PResult<ExprId> {
        let start = self.span();
        let left = self.parse_unary(false)?;
        self.parse_binary_rest(start, left, min_level)
    }

    /// Continuação de [`Parser::parse_binary`] com o operando esquerdo já
    /// lido (o padrão constante lê o operando com as regras dele).
    pub(crate) fn parse_binary_rest(&mut self, start: Span, mut left: ExprId, min_level: u8) -> PResult<ExprId> {
        // `lastBinaryExpressionLevel` do fasta (`parser_impl.dart:6094`): dois
        // operadores seguidos de igualdade (`a == b == c`) ou relacionais
        // (`a < b < c`) no mesmo nível são
        // `EQUALITY_CANNOT_BE_EQUALITY_OPERAND` no segundo, e a leitura segue.
        let mut ultimo_nivel: Option<u8> = None;
        loop {
            let Some(here) = self.binary_here() else {
                match self.operador_por_extenso(min_level) {
                    Some((level, op, texto)) => {
                        let palavra = self.text().to_string();
                        self.erro(codigos::parser::BINARY_OPERATOR_WRITTEN_OUT, &[&palavra, texto]);
                        self.advance();
                        let right = self.parse_binary(level + 1)?;
                        left = self.push(start, ExprKind::Binary { op, left, right });
                        continue;
                    }
                    None => break,
                }
            };
            match here {
                BinaryHere::Op { level, op, len } => {
                    if level < min_level {
                        break;
                    }
                    if level == LEVEL_EQUALITY || level == LEVEL_RELATIONAL {
                        if ultimo_nivel == Some(level) {
                            let fim = self.peek_at(len - 1).span.end;
                            let span = Span { start: self.span().start, end: fim };
                            self.erro_em(codigos::parser::EQUALITY_CANNOT_BE_EQUALITY_OPERAND, span, &[]);
                        }
                        ultimo_nivel = Some(level);
                    } else {
                        ultimo_nivel = None;
                    }
                    for _ in 0..len {
                        self.advance();
                    }
                    let right = self.parse_binary(level + 1)?;
                    left = self.push(start, ExprKind::Binary { op, left, right });
                }
                BinaryHere::Is => {
                    if LEVEL_RELATIONAL < min_level {
                        break;
                    }
                    self.advance();
                    let negated = self.eat_op(Op::Bang);
                    let ty = self.tipo_depois_de_is_ou_as()?;
                    left = self.push(
                        start,
                        ExprKind::Is {
                            value: left,
                            ty,
                            negated,
                        },
                    );
                    self.pular_is_as_encadeados()?;
                }
                BinaryHere::As => {
                    if LEVEL_RELATIONAL < min_level {
                        break;
                    }
                    self.advance();
                    let ty = self.tipo_depois_de_is_ou_as()?;
                    left = self.push(start, ExprKind::As { value: left, ty });
                    self.pular_is_as_encadeados()?;
                }
            }
        }
        Ok(left)
    }

    /// `looksLikePatternStart` (`identifier_context.dart:372`).
    fn parece_inicio_de_padrao(&self, pos: usize) -> bool {
        match self.kind_of(pos) {
            Kind::Ident | Kind::Int | Kind::Double | Kind::Str(_) | Kind::StrBegin(..) => true,
            Kind::Keyword(k) => matches!(k, Keyword::Null | Keyword::False | Keyword::True | Keyword::Var | Keyword::Final | Keyword::Const),
            Kind::Op(op) => matches!(
                op,
                Op::Hash | Op::LBrace | Op::LParen | Op::LBracket | Op::Lt | Op::LtEq | Op::Gt | Op::BangEq | Op::EqEq
            ),
            _ => false,
        }
    }

    /// O tipo depois de `is`/`as` (`computeTypeAfterIsOrAs` +
    /// `ensureTypeNotVoid`): o que não pode começar um tipo é
    /// `EXPECTED_TYPE_NAME` nele, com um tipo sintético vazio, e a expressão
    /// continua (`a is "A"`).
    fn tipo_depois_de_is_ou_as(&mut self) -> PResult<TypeId> {
        if matches!(self.kind(), Kind::Ident | Kind::Keyword(Keyword::Void) | Kind::Op(Op::LParen)) {
            return self.parse_type_after_is_or_as();
        }
        let aqui = self.span();
        self.erro_em(codigos::parser::EXPECTED_TYPE_NAME, aqui, &[]);
        let s = Span { start: aqui.start, end: aqui.start };
        let nome = self.name_from("", s);
        Ok(self.ast.push_type(crate::ast::TypeAnnotation {
            span: s,
            nullable: false,
            kind: crate::ast::TypeKind::Named { name: vec![nome].into_boxed_slice(), args: Box::default() },
        }))
    }

    /// `skipChainedAsIsOperators` (`parser_impl.dart`): `is`/`as` logo
    /// depois de outro é `UNEXPECTED_TOKEN` nele, e o tipo dele é pulado.
    fn pular_is_as_encadeados(&mut self) -> PResult<()> {
        while self.at_kw(Keyword::Is) || self.at_ident("as") {
            let texto = self.text().to_string();
            self.erro(codigos::parser::UNEXPECTED_TOKEN, &[&texto]);
            self.advance();
            self.eat_op(Op::Bang);
            self.tipo_depois_de_is_ou_as()?;
        }
        Ok(())
    }

    /// `_attemptPrecedenceLevelRecovery` do fasta: um identificador `and`,
    /// `or`, `xor`, `shl` ou `shr` onde cabe um operador binário é lido, em
    /// tentativa, como cada operador que ele descreve (`and`: `&`, depois
    /// `&&`); a primeira leitura sem erro que termina num ponto plausível
    /// (`;`, `,`, `)`, `{`, `}`, `|`, `||`, `&`, `&&`, fim ou outra dessas
    /// palavras) é aceita, com `BINARY_OPERATOR_WRITTEN_OUT`.
    fn operador_por_extenso(&mut self, min_level: u8) -> Option<(u8, BinaryOp, &'static str)> {
        if self.recuperando_operador || !self.e_identificador_puro(self.pos) {
            return None;
        }
        let trocas: &[(u8, BinaryOp, &'static str)] = match self.text() {
            "xor" => &[(LEVEL_BIT_XOR, BinaryOp::BitXor, "^")],
            "and" => &[(LEVEL_BIT_AND, BinaryOp::BitAnd, "&"), (LEVEL_AND, BinaryOp::And, "&&")],
            "or" => &[(LEVEL_BIT_OR, BinaryOp::BitOr, "|"), (LEVEL_OR, BinaryOp::Or, "||")],
            "shl" => &[(LEVEL_SHIFT, BinaryOp::Shl, "<<")],
            "shr" => &[(LEVEL_SHIFT, BinaryOp::Shr, ">>")],
            _ => return None,
        };
        for &(level, op, texto) in trocas {
            if level < min_level {
                continue;
            }
            let cp = self.checkpoint();
            self.recuperando_operador = true;
            self.advance();
            let inicio = self.span();
            let ok = self
                .parse_binary(level + 1)
                .and_then(|r| self.parse_binary_rest(inicio, r, min_level))
                .is_ok();
            let aceita = ok
                && self.diagnostics.len() == cp.diagnostics
                && (matches!(
                    self.kind(),
                    Kind::Op(Op::Semicolon | Op::Comma | Op::RParen | Op::LBrace | Op::RBrace | Op::Pipe | Op::PipePipe | Op::Amp | Op::AmpAmp)
                        | Kind::Eof
                ) || (self.e_identificador_puro(self.pos) && matches!(self.text(), "xor" | "and" | "or" | "shl" | "shr")));
            self.recuperando_operador = false;
            self.restore(cp);
            if aceita {
                return Some((level, op, texto));
            }
        }
        None
    }

    /// Operador binário (ou `is`) no cursor com nível a partir da igualdade:
    /// o nível, o texto do token do fasta (`>>`, `>=`, `is`) e o intervalo
    /// dele. `as` fica de fora (é o padrão de cast).
    pub(crate) fn operador_binario_de_padrao(&self) -> Option<(u8, &'static str, Span)> {
        let inicio = self.span();
        let (level, texto, len) = match self.binary_here()? {
            BinaryHere::As => return None,
            BinaryHere::Is => (LEVEL_RELATIONAL, "is", 1),
            BinaryHere::Op { level, op, len } => {
                let texto = match op {
                    BinaryOp::Eq => "==",
                    BinaryOp::NotEq => "!=",
                    BinaryOp::Lt => "<",
                    BinaryOp::LtEq => "<=",
                    BinaryOp::Gt => ">",
                    BinaryOp::GtEq => ">=",
                    BinaryOp::Shr => ">>",
                    BinaryOp::UShr => ">>>",
                    BinaryOp::BitOr => "|",
                    BinaryOp::BitXor => "^",
                    BinaryOp::BitAnd => "&",
                    BinaryOp::Shl => "<<",
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    BinaryOp::Rem => "%",
                    BinaryOp::TruncDiv => "~/",
                    _ => return None,
                };
                (level, texto, len)
            }
        };
        if level < LEVEL_EQUALITY {
            return None;
        }
        let fim = self.peek_at(len - 1).span.end;
        Some((level, texto, Span { start: inicio.start, end: fim }))
    }

    /// Operador binário no cursor, se houver, com nível e tamanho em tokens.
    fn binary_here(&self) -> Option<BinaryHere> {
        let (level, op, len) = match self.kind() {
            Kind::Op(Op::QuestionQuestion) => (LEVEL_IF_NULL, BinaryOp::IfNull, 1),
            Kind::Op(Op::PipePipe) => (LEVEL_OR, BinaryOp::Or, 1),
            Kind::Op(Op::AmpAmp) => (LEVEL_AND, BinaryOp::And, 1),
            Kind::Op(Op::EqEq) => (LEVEL_EQUALITY, BinaryOp::Eq, 1),
            Kind::Op(Op::BangEq) => (LEVEL_EQUALITY, BinaryOp::NotEq, 1),
            Kind::Op(Op::Lt) => (LEVEL_RELATIONAL, BinaryOp::Lt, 1),
            Kind::Op(Op::LtEq) => (LEVEL_RELATIONAL, BinaryOp::LtEq, 1),
            Kind::Keyword(Keyword::Is) => return Some(BinaryHere::Is),
            // `as` é identificador embutido; depois de um operando só pode
            // ser o operador de cast.
            Kind::Ident if self.text() == "as" => return Some(BinaryHere::As),
            Kind::Op(Op::Gt) => match self.gt_here()? {
                ComposedGt::Gt => (LEVEL_RELATIONAL, BinaryOp::Gt, 1),
                ComposedGt::GtEq => (LEVEL_RELATIONAL, BinaryOp::GtEq, 2),
                ComposedGt::Shr => (LEVEL_SHIFT, BinaryOp::Shr, 2),
                ComposedGt::UShr => (LEVEL_SHIFT, BinaryOp::UShr, 3),
                ComposedGt::ShrAssign | ComposedGt::UShrAssign => return None,
            },
            Kind::Op(Op::Pipe) => (LEVEL_BIT_OR, BinaryOp::BitOr, 1),
            Kind::Op(Op::Caret) => (LEVEL_BIT_XOR, BinaryOp::BitXor, 1),
            Kind::Op(Op::Amp) => (LEVEL_BIT_AND, BinaryOp::BitAnd, 1),
            Kind::Op(Op::LtLt) => (LEVEL_SHIFT, BinaryOp::Shl, 1),
            Kind::Op(Op::Plus) => (LEVEL_ADDITIVE, BinaryOp::Add, 1),
            Kind::Op(Op::Minus) => (LEVEL_ADDITIVE, BinaryOp::Sub, 1),
            Kind::Op(Op::Star) => (LEVEL_MULTIPLICATIVE, BinaryOp::Mul, 1),
            Kind::Op(Op::Slash) => (LEVEL_MULTIPLICATIVE, BinaryOp::Div, 1),
            Kind::Op(Op::Percent) => (LEVEL_MULTIPLICATIVE, BinaryOp::Rem, 1),
            Kind::Op(Op::TildeSlash) => (LEVEL_MULTIPLICATIVE, BinaryOp::TruncDiv, 1),
            _ => return None,
        };
        Some(BinaryHere::Op { level, op, len })
    }

    // -- Unários, pós-fixos e seletores ------------------------------------

    /// Prefixos `-`, `!`, `~`, `++`, `--`, `await`, seguidos do pós-fixo.
    ///
    /// `constant_pattern` deixa o `!` pós-fixo final para o parser de padrões.
    pub(crate) fn parse_unary(&mut self, constant_pattern: bool) -> PResult<ExprId> {
        self.enter()?;
        let result = self.parse_unary_inner(constant_pattern);
        self.leave();
        result
    }

    fn parse_unary_inner(&mut self, constant_pattern: bool) -> PResult<ExprId> {
        let start = self.span();
        let op = match self.kind() {
            Kind::Op(Op::Minus) => UnaryOp::Neg,
            Kind::Op(Op::Bang) => UnaryOp::Not,
            Kind::Op(Op::Tilde) => UnaryOp::BitNot,
            Kind::Op(Op::PlusPlus) => UnaryOp::PrefixInc,
            Kind::Op(Op::MinusMinus) => UnaryOp::PrefixDec,
            // `parseUnaryExpression` do fasta: fora de `async`/`async*`, o
            // `await` que parece expressão (`looksLikeAwaitExpression`, só
            // em corpo síncrono comum; em `sync*` sempre) é lido como tal e
            // relatado (`AWAIT_IN_WRONG_CONTEXT` no `await`).
            Kind::Ident
                if self.text() == "await"
                    && (self.in_async || self.in_generator || self.parece_expressao_apos_await(self.pos, true)) =>
            {
                self.advance();
                let operand = self.parse_unary(constant_pattern)?;
                if !self.in_async {
                    self.erro_em(codigos::compile_time_error::AWAIT_IN_WRONG_CONTEXT, start, &[]);
                }
                return Ok(self.push(start, ExprKind::Await(operand)));
            }
            _ => return self.parse_postfix(constant_pattern),
        };
        self.advance();
        let operand = self.parse_unary(constant_pattern)?;
        if matches!(op, UnaryOp::PrefixInc | UnaryOp::PrefixDec) && !self.e_atribuivel(operand) {
            // `handleUnaryPrefixAssignmentExpression` (`ast_builder.dart:5704`):
            // no último token do operando.
            let ultimo = self.tokens[self.pos - 1].span;
            self.erro_em(codigos::parser::MISSING_ASSIGNABLE_SELECTOR, ultimo, &[]);
        }
        Ok(self.push(start, ExprKind::Unary { op, operand }))
    }

    /// `Expression.isAssignable` do analyzer (`ast.dart`): só identificador
    /// (simples ou prefixado), acesso a propriedade e índice.
    pub(crate) fn e_atribuivel(&self, id: ExprId) -> bool {
        matches!(
            self.ast.expr(id).kind,
            ExprKind::Identifier(_) | ExprKind::Property { .. } | ExprKind::Index { .. }
        )
    }

    /// `handleAssignmentExpression` (`ast_builder.dart:3752`): lado
    /// esquerdo não atribuível é `MISSING_ASSIGNABLE_SELECTOR` nele inteiro;
    /// `super` à direita (`reportErrorIfSuper`), idem no `super`.
    fn conferir_atribuicao(&mut self, alvo: ExprId, valor: ExprId) {
        if !self.e_atribuivel(alvo) {
            let span = self.ast.expr(alvo).span;
            self.erro_em(codigos::parser::MISSING_ASSIGNABLE_SELECTOR, span, &[]);
        }
        self.conferir_super_solto(valor);
    }

    /// `reportErrorIfSuper` (`ast_builder.dart:5840`): `super` sozinho como
    /// expressão é `MISSING_ASSIGNABLE_SELECTOR`.
    pub(crate) fn conferir_super_solto(&mut self, id: ExprId) {
        if matches!(self.ast.expr(id).kind, ExprKind::Super) {
            let span = self.ast.expr(id).span;
            self.erro_em(codigos::parser::MISSING_ASSIGNABLE_SELECTOR, span, &[]);
        }
    }

    /// `looksLikeExpressionAfterAwaitOrYield` do fasta, com o `await` ou
    /// `yield` em `pos`: o que segue só faz sentido como operando (`await
    /// f();`, `await f + 1`, `if (await f)`, `await null`). `unario` é o
    /// contexto `UnaryExpression` (também `await f,` e `await f;`).
    pub(crate) fn parece_expressao_apos_await(&self, pos: usize, unario: bool) -> bool {
        let binario = |k: Kind| {
            matches!(
                k,
                Kind::Op(
                    Op::Amp | Op::AmpAmp | Op::Pipe | Op::PipePipe | Op::Caret | Op::EqEq | Op::Gt | Op::Lt
                        | Op::LtEq | Op::LtLt | Op::Minus | Op::Percent | Op::Plus | Op::QuestionQuestion
                        | Op::Slash | Op::Star | Op::TildeSlash
                )
            )
        };
        match self.kind_of(pos + 1) {
            Kind::Ident => {
                let t = self.kind_of(pos + 2);
                match t {
                    Kind::Op(Op::LParen) => {
                        let Some(fecha) = self.matching_close(pos + 2) else { return false };
                        let d = self.kind_of(fecha + 1);
                        matches!(
                            d,
                            Kind::Op(
                                Op::Semicolon | Op::Dot | Op::Comma | Op::DotDot | Op::Question | Op::QuestionDot | Op::RParen
                            )
                        ) || binario(d)
                    }
                    Kind::Op(Op::Dot | Op::RParen | Op::RBracket) => true,
                    Kind::Op(Op::Comma | Op::Semicolon) => unario,
                    _ => binario(t),
                }
            }
            Kind::Keyword(Keyword::Null) => true,
            _ => false,
        }
    }

    /// Primária, seletores e `++`/`--`/`!` pós-fixos.
    fn parse_postfix(&mut self, constant_pattern: bool) -> PResult<ExprId> {
        let start = self.span();
        let primary = self.parse_primary()?;
        let expr = self.parse_selectors(start, primary, constant_pattern)?;
        let op = match self.kind() {
            Kind::Op(Op::PlusPlus) => UnaryOp::PostfixInc,
            Kind::Op(Op::MinusMinus) => UnaryOp::PostfixDec,
            _ => return Ok(expr),
        };
        if !self.e_atribuivel(expr) {
            // `handleUnaryPostfixAssignmentExpression` (`ast_builder.dart:5688`).
            let span = self.span();
            self.erro_em(codigos::parser::ILLEGAL_ASSIGNMENT_TO_NON_ASSIGNABLE, span, &[]);
        }
        self.advance();
        Ok(self.push(start, ExprKind::Unary { op, operand: expr }))
    }

    /// Cadeia de seletores sobre `expr`: `.x`, `?.x`, `[i]`, `?[i]`,
    /// `(args)`, `<T>(args)`, `<T>` e `!`.
    fn parse_selectors(
        &mut self,
        start: Span,
        mut expr: ExprId,
        constant_pattern: bool,
    ) -> PResult<ExprId> {
        loop {
            match self.kind() {
                Kind::Op(Op::Dot) | Kind::Op(Op::QuestionDot) => {
                    let null_aware = self.at_op(Op::QuestionDot);
                    self.advance();
                    let name = self.member_name()?;
                    expr = self.push(
                        start,
                        ExprKind::Property {
                            target: expr,
                            name,
                            null_aware,
                        },
                    );
                }
                Kind::Op(Op::LBracket) => {
                    self.advance();
                    let index = self.parse_expression()?;
                    self.expect_op(Op::RBracket)?;
                    expr = self.push(
                        start,
                        ExprKind::Index {
                            target: expr,
                            index,
                            null_aware: false,
                        },
                    );
                }
                Kind::Op(Op::Question) if self.at_op_at(1, Op::LBracket) => {
                    if self.can_parse_as_conditional() {
                        break;
                    }
                    self.advance();
                    self.advance();
                    let index = self.parse_expression()?;
                    self.expect_op(Op::RBracket)?;
                    expr = self.push(
                        start,
                        ExprKind::Index {
                            target: expr,
                            index,
                            null_aware: true,
                        },
                    );
                }
                Kind::Op(Op::LParen) => {
                    let arguments = Box::new(self.parse_arguments()?);
                    expr = self.push(
                        start,
                        ExprKind::Call {
                            target: expr,
                            arguments,
                        },
                    );
                }
                Kind::Op(Op::Lt) => {
                    if !self.method_type_arguments_ahead() {
                        break;
                    }
                    let lt_pos = self.pos;
                    let type_args = self.parse_type_arguments_opt()?;
                    if self.at_op(Op::LParen) {
                        let mut arguments = self.parse_arguments()?;
                        arguments.type_args = type_args.into_boxed_slice();
                        expr = self.push(
                            start,
                            ExprKind::Call {
                                target: expr,
                                arguments: Box::new(arguments),
                            },
                        );
                    } else {
                        if constant_pattern {
                            // `parsePrecedenceExpression`: argumentos de tipo
                            // sem `(` num padrão constante.
                            let lt = self.tokens[lt_pos].span;
                            self.erro_em(codigos::parser::INVALID_CONSTANT_PATTERN_GENERIC, lt, &[]);
                        }
                        expr = self.push(
                            start,
                            ExprKind::TypeArguments {
                                target: expr,
                                type_args: type_args.into_boxed_slice(),
                            },
                        );
                    }
                }
                Kind::Op(Op::Bang) => {
                    if constant_pattern && !self.bang_is_selector() {
                        break;
                    }
                    self.advance();
                    expr = self.push(
                        start,
                        ExprKind::Unary {
                            op: UnaryOp::NullAssert,
                            operand: expr,
                        },
                    );
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    /// O `!` corrente é seguido de algo que continua a cadeia de seletores
    /// (`x!.y`, `x![0]`, `f!()`, `x!?[0]`); senão é pós-fixo puro.
    fn bang_is_selector(&self) -> bool {
        matches!(
            self.kind_at(1),
            Kind::Op(Op::Dot | Op::QuestionDot | Op::Question | Op::LParen | Op::LBracket)
        )
    }

    /// Nome após `.`/`?.`/`..`: identificador ou `new` (tearoff de construtor).
    fn member_name(&mut self) -> PResult<Name> {
        if self.at_identifier() {
            return Ok(self.identifier());
        }
        if self.at_kw(Keyword::New) {
            let token = self.advance();
            return Ok(self.name_from("new", token.span));
        }
        // `x.this`, `x.super`, `x.null`…: o fasta lê a primária
        // (`parsePrimary`) e o `AstBuilder.doDotExpression`
        // (`ast_builder.dart:851`) a recusa como nome com `MISSING_IDENTIFIER`
        // no token, que vira o nome da propriedade.
        if matches!(
            self.kind(),
            Kind::Keyword(Keyword::This | Keyword::Super | Keyword::Null | Keyword::True | Keyword::False)
        ) {
            let texto = self.text().to_string();
            let token = self.advance();
            self.erro_em(codigos::parser::MISSING_IDENTIFIER, token.span, &[]);
            return Ok(self.name_from(&texto, token.span));
        }
        Err(self.erro_identificador())
    }

    /// O `<` corrente abre argumentos de tipo de um seletor: a lista é bem
    /// formada e o token seguinte é um dos que podem seguir argumentos de
    /// tipo (regra `_mayFollowTypeArgs` do SDK; o fechamento de interpolação
    /// entra porque o lexer o funde no trecho seguinte da string).
    fn method_type_arguments_ahead(&self) -> bool {
        let Some(end) = self.skip_type_arguments(self.pos) else {
            return false;
        };
        matches!(
            self.kind_of(end),
            Kind::Op(
                Op::LParen
                    | Op::Dot
                    | Op::EqEq
                    | Op::BangEq
                    | Op::RParen
                    | Op::RBracket
                    | Op::RBrace
                    | Op::Semicolon
                    | Op::Colon
                    | Op::Comma
            ) | Kind::StrMid(..)
                | Kind::StrEnd(..)
                | Kind::Eof
        )
    }

    /// O `?` corrente (seguido de `[`) começa uma condicional
    /// `? expressão : expressão`? Lê especulativamente e desfaz tudo.
    fn can_parse_as_conditional(&mut self) -> bool {
        let checkpoint = self.checkpoint();
        self.advance();
        let ok = self.parse_expression_without_cascade().is_ok()
            && self.eat_op(Op::Colon)
            && self.parse_expression_without_cascade().is_ok();
        self.restore(checkpoint);
        ok
    }

    fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            pos: self.pos,
            exprs: self.ast.exprs.len(),
            stmts: self.ast.stmts.len(),
            types: self.ast.types.len(),
            patterns: self.ast.patterns.len(),
            decls: self.ast.decls.len(),
            members: self.ast.members.len(),
            functions: self.ast.functions.len(),
            diagnostics: self.diagnostics.len(),
            depth: self.depth,
            in_async: self.in_async,
            in_generator: self.in_generator,
            in_type_args: self.in_type_args,
        }
    }

    fn restore(&mut self, checkpoint: Checkpoint) {
        self.pos = checkpoint.pos;
        self.ast.exprs.truncate(checkpoint.exprs);
        self.ast.stmts.truncate(checkpoint.stmts);
        self.ast.types.truncate(checkpoint.types);
        self.ast.patterns.truncate(checkpoint.patterns);
        self.ast.decls.truncate(checkpoint.decls);
        self.ast.members.truncate(checkpoint.members);
        self.ast.functions.truncate(checkpoint.functions);
        self.diagnostics.truncate(checkpoint.diagnostics);
        self.depth = checkpoint.depth;
        self.in_async = checkpoint.in_async;
        self.in_generator = checkpoint.in_generator;
        self.in_type_args = checkpoint.in_type_args;
    }

    // -- Primárias --------------------------------------------------------

    /// `primary`: literais, identificadores, `this`, `super`, coleções,
    /// records, parênteses, `new`/`const`, expressões de função e `switch`.
    fn parse_primary(&mut self) -> PResult<ExprId> {
        let start = self.span();
        match self.kind() {
            Kind::Ident => {
                if let Some(nome) = self.funcao_nomeada_a_frente() {
                    return self.parse_funcao_nomeada(nome);
                }
                let name = self.identifier();
                Ok(self.push(start, ExprKind::Identifier(name)))
            }
            // `void nome(…) {…}` em posição de expressão.
            Kind::Keyword(Keyword::Void) if self.funcao_nomeada_a_frente().is_some() => {
                let nome = self.funcao_nomeada_a_frente().expect("conferido na guarda");
                self.parse_funcao_nomeada(nome)
            }
            Kind::Int => {
                self.conferir_separadores(start);
                self.advance();
                Ok(self.push(start, ExprKind::Int(start)))
            }
            Kind::Double => {
                self.conferir_separadores(start);
                self.advance();
                Ok(self.push(start, ExprKind::Double(start)))
            }
            Kind::Str(_) | Kind::StrBegin(..) => {
                let lit = self.parse_string_literal()?;
                Ok(self.push(start, ExprKind::String(lit)))
            }
            Kind::Keyword(Keyword::True) => {
                self.advance();
                Ok(self.push(start, ExprKind::Bool(true)))
            }
            Kind::Keyword(Keyword::False) => {
                self.advance();
                Ok(self.push(start, ExprKind::Bool(false)))
            }
            Kind::Keyword(Keyword::Null) => {
                self.advance();
                Ok(self.push(start, ExprKind::Null))
            }
            Kind::Keyword(Keyword::This) => {
                self.advance();
                Ok(self.push(start, ExprKind::This))
            }
            Kind::Keyword(Keyword::Super) => {
                self.advance();
                // `parseSuperExpression` (`parser_impl.dart:6906`): `super?.`.
                if self.at_op(Op::QuestionDot) {
                    let span = self.span();
                    self.erro_em(codigos::parser::INVALID_OPERATOR_QUESTIONMARK_PERIOD_FOR_SUPER, span, &[]);
                }
                Ok(self.push(start, ExprKind::Super))
            }
            Kind::Op(Op::Hash) => self.parse_symbol_literal(),
            Kind::Op(Op::LBracket) => self.parse_list_literal(start, false, Vec::new()),
            Kind::Op(Op::LBrace) => self.parse_set_or_map_literal(start, false, Vec::new()),
            Kind::Op(Op::Lt) => {
                if self.generic_function_expression_ahead() {
                    return self.parse_function_expression(start);
                }
                let type_args = self.parse_type_arguments_opt()?;
                self.parse_typed_collection(start, false, type_args)
            }
            Kind::Op(Op::LParen) => {
                // `(params) {…}` é função só se a lista de parâmetros for
                // válida: `x = (v as T?) { … }` num inicializador de construtor
                // é expressão parentizada seguida do corpo.
                if self.function_expression_ahead(self.pos)
                    && self.speculate(|p| p.parse_formal_parameters().is_ok())
                {
                    return self.parse_function_expression(start);
                }
                self.parse_parenthesized_or_record(start, false)
            }
            Kind::Keyword(Keyword::New) => {
                let novo = self.advance().span;
                // `parseNewExpression` (`parser_impl.dart:7270`): literal com
                // `new` antes. `new List<T>[…]`, `new Map<K, V>{…}`,
                // `new Set<T>{…}` são `LITERAL_WITH_CLASS_AND_NEW` de `new` ao
                // nome; `new [..]`, `new {..}`, `new <T>[..]`,
                // `LITERAL_WITH_NEW` no `new`; nos dois casos o literal é
                // lido a partir do que segue.
                if self.at_identifier() && !self.at_op_at(1, Op::Dot) {
                    let classe = self.text();
                    let depois = if self.at_op_at(1, Op::Lt) {
                        self.skip_type_arguments(self.pos + 1)
                    } else {
                        Some(self.pos + 1)
                    };
                    let literal = match (classe, depois.map(|d| self.kind_of(d))) {
                        ("Map" | "Set", Some(Kind::Op(Op::LBrace))) => true,
                        ("List", Some(Kind::Op(Op::LBracket))) => true,
                        _ => false,
                    };
                    if literal {
                        let nome = self.advance().span;
                        let minusculo = classe.to_lowercase();
                        let span = Span { start: novo.start, end: nome.end };
                        self.erro_em(codigos::parser::LITERAL_WITH_CLASS_AND_NEW, span, &[&minusculo, classe]);
                        return self.parse_primary();
                    }
                } else if !self.at_identifier() {
                    let literal = match self.kind() {
                        Kind::Op(Op::LBrace | Op::LBracket) => true,
                        Kind::Op(Op::Lt) => self
                            .skip_type_arguments(self.pos)
                            .is_some_and(|d| matches!(self.kind_of(d), Kind::Op(Op::LBrace | Op::LBracket))),
                        _ => false,
                    };
                    if literal {
                        self.erro_em(codigos::parser::LITERAL_WITH_NEW, novo, &[]);
                        return self.parse_primary();
                    }
                }
                self.parse_instance_creation(start, Some(CreationKeyword::New))
            }
            // Atalho de ponto (3.10): `.id`, `.new`.
            Kind::Op(Op::Dot) => self.parse_dot_shorthand(start, false),
            Kind::Keyword(Keyword::Const) => {
                self.advance();
                match self.kind() {
                    // `const .id(args)` / `const .new(args)`.
                    Kind::Op(Op::Dot) => self.parse_dot_shorthand(start, true),
                    Kind::Op(Op::LBracket) => self.parse_list_literal(start, true, Vec::new()),
                    Kind::Op(Op::LBrace) => self.parse_set_or_map_literal(start, true, Vec::new()),
                    Kind::Op(Op::LParen) => self.parse_parenthesized_or_record(start, true),
                    Kind::Op(Op::Lt) => {
                        let type_args = self.parse_type_arguments_opt()?;
                        self.parse_typed_collection(start, true, type_args)
                    }
                    _ => self.parse_instance_creation(start, Some(CreationKeyword::Const)),
                }
            }
            Kind::Keyword(Keyword::Switch) => self.parse_switch_expression(start),
            _ => Err(self.erro_identificador()),
        }
    }

    /// O *head* de um atalho de ponto, com o `.` corrente (Dart 3.10,
    /// `<staticMemberShorthandHead>`): `.id` ou `.new`. Os seletores vêm
    /// depois, como em qualquer primária. A forma `const` exige argumentos.
    fn parse_dot_shorthand(&mut self, start: Span, const_: bool) -> PResult<ExprId> {
        let ponto = self.advance();
        let name = if self.at_kw(Keyword::New) {
            let token = self.advance();
            self.name_from("new", token.span)
        } else if self.at_identifier() {
            self.identifier()
        } else {
            return Err(self.erro_identificador());
        };
        // O analyzer 3.13.4 relata no `.` (ou no `const` de `const .x(…)`).
        self.exigir(Feature::DotShorthands, if const_ { start } else { ponto.span });
        if const_ && !self.at_op(Op::LParen) {
            return Err(self.erro_esperado("("));
        }
        Ok(self.push(start, ExprKind::DotShorthand { name, const_ }))
    }

    /// Após argumentos de tipo explícitos: `<T>[...]` ou `<K, V>{...}`.
    fn parse_typed_collection(
        &mut self,
        start: Span,
        const_: bool,
        type_args: Vec<TypeId>,
    ) -> PResult<ExprId> {
        match self.kind() {
            Kind::Op(Op::LBracket) => self.parse_list_literal(start, const_, type_args),
            Kind::Op(Op::LBrace) => self.parse_set_or_map_literal(start, const_, type_args),
            _ => Err(self.erro_esperado("[")),
        }
    }

    /// `[ elementos ]`.
    fn parse_list_literal(
        &mut self,
        start: Span,
        const_: bool,
        type_args: Vec<TypeId>,
    ) -> PResult<ExprId> {
        self.expect_op(Op::LBracket)?;
        let elements = self.parse_collection_elements(Op::RBracket)?;
        Ok(self.push(
            start,
            ExprKind::List {
                const_,
                type_args: type_args.into_boxed_slice(),
                elements: elements.into_boxed_slice(),
            },
        ))
    }

    /// `{ elementos }` — conjunto ou mapa, decidido depois pelos elementos.
    fn parse_set_or_map_literal(
        &mut self,
        start: Span,
        const_: bool,
        type_args: Vec<TypeId>,
    ) -> PResult<ExprId> {
        self.expect_op(Op::LBrace)?;
        let elements = self.parse_collection_elements(Op::RBrace)?;
        Ok(self.push(
            start,
            ExprKind::SetOrMap {
                const_,
                type_args: type_args.into_boxed_slice(),
                elements: elements.into_boxed_slice(),
            },
        ))
    }

    /// Elementos separados por vírgula (com vírgula final opcional) até
    /// `close`, que é consumido.
    fn parse_collection_elements(&mut self, close: Op) -> PResult<Vec<CollectionElement>> {
        let abre = self.pos.saturating_sub(1);
        let mut elements = Vec::new();
        while !self.at_op(close) {
            elements.push(self.parse_collection_element()?);
            if !self.eat_op(Op::Comma) {
                break;
            }
        }
        // `parseLiteralListSuffix`/`parseLiteralSetOrMapSuffix`: o que sobra
        // antes do fecho casado é `EXPECTED_TOKEN` (o fecho) no primeiro
        // token dela, e o literal acaba no fecho (`{0: throw "y"; }`).
        if !self.at_op(close)
            && !self.at_eof()
            && let Some(fecha) = self.matching_close(abre)
            && fecha > self.pos
            && self.kind_of(fecha) == Kind::Op(close)
        {
            self.erro_esperado(close.text());
            self.pos = fecha + 1;
            return Ok(elements);
        }
        self.expect_op(close)?;
        Ok(elements)
    }

    /// `element`: expressão, `k: v`, `...e`, `...?e`, `if (...) e else e`,
    /// `await? for (...) e`, aninhados à vontade.
    fn parse_collection_element(&mut self) -> PResult<CollectionElement> {
        self.enter()?;
        let result = self.parse_collection_element_inner();
        self.leave();
        result
    }

    fn parse_collection_element_inner(&mut self) -> PResult<CollectionElement> {
        if self.eat_op(Op::Ellipsis) {
            let value = self.parse_expression()?;
            return Ok(CollectionElement::Spread {
                value,
                null_aware: false,
            });
        }
        if self.eat_op(Op::EllipsisQuestion) {
            let value = self.parse_expression()?;
            return Ok(CollectionElement::Spread {
                value,
                null_aware: true,
            });
        }
        if self.eat_kw(Keyword::If) {
            self.expect_op(Op::LParen)?;
            let condition = self.parse_expression()?;
            let mut case_pattern = None;
            let mut guard = None;
            if self.eat_kw(Keyword::Case) {
                case_pattern = Some(self.parse_pattern()?);
                if self.eat_ident("when") {
                    guard = Some(self.parse_expression()?);
                }
            }
            self.expect_op(Op::RParen)?;
            let then = Box::new(self.parse_collection_element()?);
            let else_ = if self.eat_kw(Keyword::Else) {
                Some(Box::new(self.parse_collection_element()?))
            } else {
                None
            };
            return Ok(CollectionElement::If {
                condition,
                case_pattern,
                guard,
                then,
                else_,
            });
        }
        if self.at_kw(Keyword::For) || (self.at_ident("await") && self.at_kw_at(1, Keyword::For)) {
            let await_ = self.eat_ident("await");
            self.expect_kw(Keyword::For)?;
            let header = self.parse_for_header()?;
            let body = Box::new(self.parse_collection_element()?);
            return Ok(match header {
                ForHeader::Classic {
                    init,
                    condition,
                    updates,
                } => CollectionElement::For {
                    await_,
                    init,
                    condition,
                    updates: updates.into_boxed_slice(),
                    body,
                },
                ForHeader::In { target, iterable } => CollectionElement::ForIn {
                    await_,
                    target,
                    iterable,
                    body,
                },
            });
        }
        // `?e` é elemento null-aware (Dart 3.8); `?` nunca inicia expressão,
        // então não há ambiguidade com a condicional.
        let null_aware_key = self.at_op(Op::Question);
        if null_aware_key {
            let q = self.advance();
            self.exigir(Feature::NullAwareElements, q.span);
        }
        let key = self.parse_expression()?;
        if self.eat_op(Op::Colon) {
            let null_aware_value = self.at_op(Op::Question);
            if null_aware_value {
                let q = self.advance();
                self.exigir(Feature::NullAwareElements, q.span);
            }
            let value = self.parse_expression()?;
            return Ok(CollectionElement::MapEntry {
                key,
                value,
                null_aware_key,
                null_aware_value,
            });
        }
        Ok(if null_aware_key {
            CollectionElement::NullAwareExpression(key)
        } else {
            CollectionElement::Expression(key)
        })
    }

    /// `(e)`, `(a, b)`, `(a,)`, `(nome: e)`, `()`; `const_` força record.
    fn parse_parenthesized_or_record(&mut self, start: Span, const_: bool) -> PResult<ExprId> {
        let abre = self.pos;
        self.expect_op(Op::LParen)?;
        let mut positional = Vec::new();
        let mut named = Vec::new();
        let mut is_record = const_ || self.at_op(Op::RParen);
        let mut viu_virgula = false;
        // `(,)`: `EMPTY_RECORD_LITERAL_WITH_COMMA` na vírgula
        // (`parseParenthesizedExpressionOrRecordLiteral`).
        if self.at_op(Op::Comma) && self.at_op_at(1, Op::RParen) {
            let virgula = self.advance().span;
            self.erro_em(codigos::parser::EMPTY_RECORD_LITERAL_WITH_COMMA, virgula, &[]);
            is_record = true;
            viu_virgula = true;
        }
        while !self.at_op(Op::RParen) {
            if self.at_identifier() && self.at_op_at(1, Op::Colon) {
                let name = self.identifier();
                self.advance();
                let value = self.parse_expression()?;
                named.push((name, value));
                is_record = true;
            } else {
                positional.push(self.parse_expression()?);
            }
            if self.eat_op(Op::Comma) {
                is_record = true;
                viu_virgula = true;
            } else {
                break;
            }
        }
        self.garantir_fecha_parenteses(abre)?;
        // `const (e)`: um posicional sem vírgula, no `)`.
        if const_ && positional.len() == 1 && named.is_empty() && !viu_virgula {
            let fecha = self.tokens[self.pos - 1].span;
            self.erro_em(codigos::parser::RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA, fecha, &[]);
        }
        if is_record {
            return Ok(self.push(
                start,
                ExprKind::Record {
                    const_,
                    positional: positional.into_boxed_slice(),
                    named: named.into_boxed_slice(),
                },
            ));
        }
        let inner = positional[0];
        Ok(self.push(start, ExprKind::Parenthesized(inner)))
    }

    /// Depois do `)` que fecha o `(` em `open` vem corpo de função?
    fn function_expression_ahead(&self, open: usize) -> bool {
        let Some(close) = self.matching_close(open) else {
            return false;
        };
        match self.kind_of(close + 1) {
            Kind::Op(Op::LBrace | Op::Arrow) => true,
            Kind::Ident => matches!(self.text_of(close + 1), "async" | "sync"),
            _ => false,
        }
    }

    /// O `<` corrente abre parâmetros de tipo de uma expressão de função
    /// genérica `<T>(...) ...`: fecha o `<` e exige `(` com corpo depois.
    fn generic_function_expression_ahead(&self) -> bool {
        let Some(close) = self.angle_close(self.pos) else {
            return false;
        };
        self.kind_of(close + 1) == Kind::Op(Op::LParen) && self.function_expression_ahead(close + 1)
    }

    /// `parseSendOrFunctionLiteral` (`parser_impl.dart:7188-7207` do fasta
    /// 3.6.2): em posição de primária, um tipo opcional, um identificador,
    /// `<…>` opcional e `(…)` seguido de corpo (`{`, `=>`, `async`, `sync`)
    /// são uma função literal com nome. Devolve o índice do token do nome.
    /// Não vale onde o fasta desliga `mayParseFunctionExpressions`
    /// ([`Parser::sem_funcao_nomeada`]).
    fn funcao_nomeada_a_frente(&self) -> Option<usize> {
        if self.sem_funcao_nomeada {
            return None;
        }
        // O resto, a partir do nome.
        let resto = |nome: usize| -> bool {
            if self.kind_of(nome) != Kind::Ident {
                return false;
            }
            let mut abre = nome + 1;
            if self.kind_of(abre) == Kind::Op(Op::Lt) {
                match self.angle_close(abre) {
                    Some(fecha) => abre = fecha + 1,
                    None => return false,
                }
            }
            self.kind_of(abre) == Kind::Op(Op::LParen) && self.function_expression_ahead(abre)
        };
        // Sem tipo: `g() {}`.
        if self.kind() == Kind::Ident && resto(self.pos) {
            return Some(self.pos);
        }
        // Com tipo: `void g() {}`, `int g<T>() => 1`. Um identificador só
        // começa um tipo seguido de nome se depois dele vem outro
        // identificador, `<`, `.` ou `?` (o filtro poupa o `skip_type` em
        // quase toda primária).
        if self.kind() == Kind::Ident
            && !matches!(self.kind_of(self.pos + 1), Kind::Ident | Kind::Op(Op::Lt | Op::Dot | Op::Question))
        {
            return None;
        }
        let depois = self.skip_type(self.pos)?;
        (depois > self.pos && resto(depois)).then_some(depois)
    }

    /// A função literal com nome: `NAMED_FUNCTION_EXPRESSION` no nome
    /// (`parseNamedFunctionRest` com `isFunctionExpression`,
    /// `parser_impl.dart:5286-5289`), e a expressão de função sem ele, como
    /// o `AstBuilder` a monta. O tipo de retorno escrito é pulado.
    fn parse_funcao_nomeada(&mut self, nome: usize) -> PResult<ExprId> {
        while self.pos < nome {
            self.advance();
        }
        let span = self.span();
        self.erro_em(codigos::parser::NAMED_FUNCTION_EXPRESSION, span, &[]);
        self.advance();
        let inicio = self.span();
        self.parse_function_expression(inicio)
    }

    /// `<T>? (params) async? { }` / `=> e`.
    fn parse_function_expression(&mut self, start: Span) -> PResult<ExprId> {
        let type_params = self.parse_type_parameters_opt()?;
        let parameters = self.parse_formal_parameters()?;
        let (modifier, body) = self.parse_function_body_ex(false)?;
        let function = self.ast.push_function(Function {
            span: self.span_from(start),
            external: false,
            static_: false,
            kind: FunctionKind::Function,
            return_type: None,
            name: None,
            type_params: type_params.into_boxed_slice(),
            parameters: Some(parameters.into_boxed_slice()),
            modifier,
            body,
        });
        Ok(self.push(start, ExprKind::FunctionExpression(function)))
    }

    /// Depois de `new`/`const`: `T`, `p.T`, `T<A>`, `p.T<A>.n`, `T.n`
    /// (nome de duas partes, como o SDK) e os argumentos obrigatórios.
    fn parse_instance_creation(
        &mut self,
        start: Span,
        keyword: Option<CreationKeyword>,
    ) -> PResult<ExprId> {
        let type_start = self.span();
        let mut name = vec![self.expect_identifier()?];
        if self.at_op(Op::Dot) && self.at_identifier_at(1) {
            self.advance();
            name.push(self.identifier());
        }
        let args = self.parse_type_arguments_opt()?;
        let ty = self.ast.push_type(TypeAnnotation {
            span: self.span_from(type_start),
            nullable: false,
            kind: TypeKind::Named {
                name: name.into_boxed_slice(),
                args: args.into_boxed_slice(),
            },
        });
        let constructor = if self.eat_op(Op::Dot) {
            Some(self.member_name()?)
        } else {
            None
        };
        // `parseConstructorInvocationArguments` (`parser_impl.dart:7243`):
        // sem `(`, argumentos de tipo depois do nome são
        // `CONSTRUCTOR_WITH_TYPE_ARGUMENTS` no último token do nome (e são
        // lidos); senão `EXPECTED_TOKEN` (`(`) nele. Sem `(` depois disso, a
        // lista de argumentos é sintética, vazia, e a expressão continua.
        if !self.at_op(Op::LParen) && self.pos > 0 {
            let ultimo = self.tokens[self.pos - 1].span;
            if self.at_op(Op::Lt) && self.skip_type_arguments(self.pos).is_some() {
                self.erro_em(codigos::parser::CONSTRUCTOR_WITH_TYPE_ARGUMENTS, ultimo, &[]);
                self.parse_type_arguments_opt()?;
            } else {
                self.erro_em(codigos::parser::EXPECTED_TOKEN, ultimo, &["("]);
            }
            if !self.at_op(Op::LParen) {
                let vazio = Span { start: ultimo.end, end: ultimo.end };
                let arguments = Box::new(Arguments { span: vazio, type_args: Box::default(), args: Box::default() });
                return Ok(self.push(start, ExprKind::InstanceCreation { keyword, ty, constructor, arguments }));
            }
        }
        let arguments = Box::new(self.parse_arguments()?);
        Ok(self.push(
            start,
            ExprKind::InstanceCreation {
                keyword,
                ty,
                constructor,
                arguments,
            },
        ))
    }

    /// `switch (e) { p when g => v, ... }`.
    fn parse_switch_expression(&mut self, start: Span) -> PResult<ExprId> {
        self.expect_kw(Keyword::Switch)?;
        self.expect_op(Op::LParen)?;
        let value = self.parse_expression()?;
        self.expect_op(Op::RParen)?;
        let abre = self.pos;
        // `ensureBlock(…, switchExpression)`: sem `{`, o erro no `)` e um corpo
        // vazio sintético.
        if !self.at_op(Op::LBrace) {
            let fecha = self.tokens[self.pos - 1].span;
            self.erro_em(codigos::parser::EXPECTED_SWITCH_EXPRESSION_BODY, fecha, &[]);
            return Ok(self.push(start, ExprKind::Switch { value, cases: Vec::new().into_boxed_slice() }));
        }
        self.expect_op(Op::LBrace)?;
        let mut cases = Vec::new();
        while !self.at_op(Op::RBrace) {
            let case_start = self.span();
            // `parseSwitchExpression` (`parser_impl.dart`): a sintaxe de
            // comando dentro da expressão. `default` é
            // `DEFAULT_IN_SWITCH_EXPRESSION` e vale como `_`; `case` é
            // `UNEXPECTED_TOKEN` e é pulado; `:` no lugar de `=>` e `;` no de
            // `,` são `EXPECTED_TOKEN` neles.
            let pattern = if self.at_kw(Keyword::Default) {
                let t = self.advance().span;
                self.erro_em(codigos::parser::DEFAULT_IN_SWITCH_EXPRESSION, t, &[]);
                self.ast.push_pattern(crate::ast::Pattern { span: t, kind: crate::ast::PatternKind::Wildcard { ty: None } })
            } else {
                if self.at_kw(Keyword::Case) {
                    self.erro(codigos::parser::UNEXPECTED_TOKEN, &["case"]);
                    self.advance();
                }
                // O padrão e a guarda de um caso de `switch` expressão
                // não leem função literal nomeada (o `=>` é o do caso).
                let salvo = std::mem::replace(&mut self.sem_funcao_nomeada, true);
                let padrao = self.parse_pattern();
                self.sem_funcao_nomeada = salvo;
                padrao?
            };
            let guard = if self.eat_ident("when") {
                let salvo = std::mem::replace(&mut self.sem_funcao_nomeada, true);
                let guarda = self.parse_expression();
                self.sem_funcao_nomeada = salvo;
                Some(guarda?)
            } else {
                None
            };
            if self.at_op(Op::Colon) {
                self.erro(codigos::parser::EXPECTED_TOKEN, &["=>"]);
                self.advance();
            } else {
                self.expect_op(Op::Arrow)?;
            }
            let body = self.parse_expression()?;
            cases.push(SwitchExprCase {
                span: self.span_from(case_start),
                pattern,
                guard,
                body,
            });
            let mut virgula = self.eat_op(Op::Comma);
            if !virgula && self.at_op(Op::Semicolon) {
                self.erro(codigos::parser::EXPECTED_TOKEN, &[","]);
                self.advance();
                virgula = true;
            }
            if self.at_op(Op::RBrace) || virgula {
                continue;
            }
            // Sem vírgula: o que parece outro padrão é o caso seguinte (a
            // vírgula que falta é relatada nele); senão pula até a próxima
            // `,`/`;` antes do `}` casado, ou acaba no `}`.
            if self.parece_inicio_de_padrao(self.pos) {
                self.erro(codigos::parser::EXPECTED_TOKEN, &[","]);
                continue;
            }
            let Some(fecha) = self.matching_close(abre) else { break };
            let mut i = self.pos;
            let mut achou = None;
            while i < fecha {
                match self.kind_of(i) {
                    Kind::Op(Op::Comma | Op::Semicolon) => {
                        achou = Some(i);
                        break;
                    }
                    Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => {
                        i = self.matching_close(i).unwrap_or(fecha);
                    }
                    _ => {}
                }
                i += 1;
            }
            match achou {
                Some(v) => {
                    self.erro(codigos::parser::EXPECTED_TOKEN, &[","]);
                    self.pos = v + 1;
                }
                None => {
                    self.erro(codigos::parser::EXPECTED_TOKEN, &["}"]);
                    self.pos = fecha;
                    break;
                }
            }
        }
        self.expect_op(Op::RBrace)?;
        Ok(self.push(
            start,
            ExprKind::Switch {
                value,
                cases: cases.into_boxed_slice(),
            },
        ))
    }

    /// `#nome`, `#a.b.c`, `#void`, `#+`, `#[]=`, `#>>>`…
    fn parse_symbol_literal(&mut self) -> PResult<ExprId> {
        let start = self.expect_op(Op::Hash)?.span;
        let mut names = Vec::new();
        match self.kind() {
            Kind::Ident => {
                names.push(self.identifier());
                while self.at_op(Op::Dot) && self.at_identifier_at(1) {
                    self.advance();
                    names.push(self.identifier());
                }
            }
            Kind::Keyword(kw) => {
                let token = self.advance();
                names.push(self.name_from(kw.text(), token.span));
            }
            Kind::Op(Op::LBracket) => {
                let open = self.advance();
                self.expect_op(Op::RBracket)?;
                let text = if self.eat_op(Op::Assign) { "[]=" } else { "[]" };
                let span = self.span_from(open.span);
                names.push(self.name_from(text, span));
            }
            Kind::Op(Op::Gt) => {
                let Some(composed) = self.composed_gt() else {
                    return Err(self.erro_identificador());
                };
                let text = match composed {
                    ComposedGt::Gt => ">",
                    ComposedGt::GtEq => ">=",
                    ComposedGt::Shr => ">>",
                    ComposedGt::UShr => ">>>",
                    ComposedGt::ShrAssign | ComposedGt::UShrAssign => {
                        return Err(self.erro_identificador());
                    }
                };
                let first = self.span();
                self.eat_composed_gt(composed);
                let span = self.span_from(first);
                names.push(self.name_from(text, span));
            }
            Kind::Op(
                op @ (Op::Lt
                | Op::LtEq
                | Op::EqEq
                | Op::Minus
                | Op::Plus
                | Op::Slash
                | Op::TildeSlash
                | Op::Star
                | Op::Percent
                | Op::Pipe
                | Op::Caret
                | Op::Amp
                | Op::LtLt
                | Op::Tilde),
            ) => {
                let token = self.advance();
                names.push(self.name_from(op.text(), token.span));
            }
            _ => return Err(self.erro_identificador()),
        }
        Ok(self.push(start, ExprKind::Symbol(names.into_boxed_slice())))
    }

    // -- Strings ------------------------------------------------------------

    /// Lê as interpolações e trechos que seguem um `StrBegin` até o `StrEnd`.
    fn parse_interpolations(&mut self, first: Interp) -> PResult<()> {
        let mut previous = first;
        loop {
            let expr = match previous {
                // `$` sem nome (o lexer segue com o trecho seguinte):
                // `MISSING_IDENTIFIER` no primeiro caractere depois do `$`,
                // com comprimento 1, e nenhuma interpolação.
                Interp::Ident if !self.at_identifier() => {
                    let s = self.span().start;
                    self.erro_em(codigos::scanner::MISSING_IDENTIFIER, Span { start: s, end: s + 1 }, &[]);
                    None
                }
                Interp::Ident => {
                    let start = self.span();
                    let name = self.identifier();
                    Some(self.push(start, ExprKind::Identifier(name)))
                }
                Interp::Brace => Some(self.parse_expression()?),
            };
            if let Some(expr) = expr {
                self.scratch_parts.push(StringPart::Interpolation(expr));
            }
            let leading_brace = previous == Interp::Brace;
            match self.kind() {
                Kind::StrMid(flags, next) => {
                    let token = self.advance();
                    self.push_string_text(token, flags, false, leading_brace)?;
                    previous = next;
                }
                Kind::StrEnd(flags) => {
                    let token = self.advance();
                    self.push_string_text(token, flags, false, leading_brace)?;
                    return Ok(());
                }
                _ => return Err(self.erro_esperado("}")),
            }
        }
    }

    /// Decodifica o texto de um trecho de string e o anexa ao rascunho.
    ///
    /// `first` indica o trecho que começa na aspa de abertura (recebe o `r`
    /// e a remoção da primeira quebra de linha em strings triplas);
    /// `leading_brace` indica que o trecho começa na `}` de uma interpolação.
    fn push_string_text(
        &mut self,
        token: Token,
        flags: StrFlags,
        first: bool,
        leading_brace: bool,
    ) -> PResult<()> {
        let text = self.token_text(token);
        let quote_len = if flags.triple { 3 } else { 1 };
        let mut head = 0;
        if first {
            if flags.raw {
                head += 1;
            }
            head += quote_len;
        } else if leading_brace {
            head += 1;
        }
        let tail = match token.kind {
            Kind::Str(_) | Kind::StrEnd(_) => quote_len,
            Kind::StrBegin(_, Interp::Brace) | Kind::StrMid(_, Interp::Brace) => 2,
            _ => 1,
        };
        let content = text
            .get(head..text.len().saturating_sub(tail))
            .unwrap_or("");
        match crate::lexer::decode_string(content, flags.raw, first && flags.triple) {
            Ok(decoded) => {
                self.scratch_parts.push(StringPart::Text(decoded));
                Ok(())
            }
            Err(message) => {
                let _ = message;
                // `unescapeCodeUnits` (`quote.dart:186`): o escape inválido é
                // relatado com o código e o trecho do fasta, e a string segue
                // com o texto cru (o erro é recuperável).
                let base = token.span.start + head;
                match erro_de_escape(content.as_bytes()) {
                    Some((codigo, inicio, len)) => {
                        let span = Span { start: base + inicio, end: base + inicio + len };
                        // `INVALID_CODE_POINT` leva o texto literal `\u{...}`.
                        let args: &[&str] =
                            if codigo == codigos::parser::INVALID_CODE_POINT { &["\\u{...}"] } else { &[] };
                        self.erro_em(codigo, span, args);
                        self.scratch_parts.push(StringPart::Text(crate::text::DartStr::from(content)));
                        Ok(())
                    }
                    None => Err(self.erro_em(codigos::parser::INVALID_UNICODE_ESCAPE_STARTED, token.span, &[])),
                }
            }
        }
    }

    // -- Utilidades ---------------------------------------------------------

    /// Anexa uma expressão cujo span vai de `start` ao último token consumido.
    pub(crate) fn push(&mut self, start: Span, kind: ExprKind) -> ExprId {
        let span = self.span_from(start);
        self.ast.push_expr(Expr { span, kind })
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{
        AssignOp, BinaryOp, CollectionElement, CreationKeyword, ExprId, ExprKind, StringPart,
        UnaryOp,
    };
    use crate::parser::Parser;

    /// Constrói o parser e lê uma expressão inteira, exigindo consumir tudo.
    fn parse(src: &str) -> (Parser<'_, 'static>, ExprId) {
        parse_with(src, false)
    }

    fn parse_with(src: &str, in_async: bool) -> (Parser<'_, 'static>, ExprId) {
        let nomes: &'static mut dartforge_intern::Interner =
            Box::leak(Box::new(dartforge_intern::Interner::new()));
        let tokens = crate::lexer::lex(src).unwrap();
        let mut p = Parser::new(src, tokens, nomes);
        p.in_async = in_async;
        let id = p
            .parse_expression()
            .unwrap_or_else(|_| panic!("{src}: {:?}", p.diagnostics));
        assert!(p.diagnostics.is_empty(), "{src}: {:?}", p.diagnostics);
        assert!(p.at_eof(), "{src}: sobrou {:?}", p.peek());
        (p, id)
    }

    fn kind<'a>(p: &'a Parser<'_, '_>, id: ExprId) -> &'a ExprKind {
        &p.ast.expr(id).kind
    }

    fn ident_text<'a>(p: &'a Parser<'_, '_>, id: ExprId) -> &'a str {
        match kind(p, id) {
            ExprKind::Identifier(name) => p.interner.resolve(name.sym),
            other => panic!("esperava identificador, veio {other:?}"),
        }
    }

    fn int_text<'a>(p: &'a Parser<'_, '_>, id: ExprId) -> &'a str {
        match kind(p, id) {
            ExprKind::Int(span) => &p.source[span.start..span.end],
            other => panic!("esperava inteiro, veio {other:?}"),
        }
    }

    fn binary(p: &Parser<'_, '_>, id: ExprId) -> (BinaryOp, ExprId, ExprId) {
        match kind(p, id) {
            ExprKind::Binary { op, left, right } => (*op, *left, *right),
            other => panic!("esperava binário, veio {other:?}"),
        }
    }

    #[test]
    fn precedencia_aritmetica() {
        let (p, id) = parse("1 + 2 * 3");
        let (op, l, r) = binary(&p, id);
        assert_eq!(op, BinaryOp::Add);
        assert_eq!(int_text(&p, l), "1");
        let (op, l, r) = binary(&p, r);
        assert_eq!(op, BinaryOp::Mul);
        assert_eq!(int_text(&p, l), "2");
        assert_eq!(int_text(&p, r), "3");
    }

    #[test]
    fn associatividade_a_esquerda() {
        let (p, id) = parse("a - b - c");
        let (op, l, r) = binary(&p, id);
        assert_eq!(op, BinaryOp::Sub);
        assert_eq!(ident_text(&p, r), "c");
        let (op, l, r) = binary(&p, l);
        assert_eq!(op, BinaryOp::Sub);
        assert_eq!(ident_text(&p, l), "a");
        assert_eq!(ident_text(&p, r), "b");
    }

    #[test]
    fn if_null_e_logicos() {
        let (p, id) = parse("a ?? b ?? c");
        let (op, l, _) = binary(&p, id);
        assert_eq!(op, BinaryOp::IfNull);
        assert_eq!(binary(&p, l).0, BinaryOp::IfNull);

        let (p, id) = parse("a || b && c == d");
        let (op, _, r) = binary(&p, id);
        assert_eq!(op, BinaryOp::Or);
        let (op, _, r) = binary(&p, r);
        assert_eq!(op, BinaryOp::And);
        assert_eq!(binary(&p, r).0, BinaryOp::Eq);
    }

    #[test]
    fn shifts_e_comparacoes_com_maior_isolado() {
        let (p, id) = parse("x >= 3");
        assert_eq!(binary(&p, id).0, BinaryOp::GtEq);
        let (p, id) = parse("x > 3");
        assert_eq!(binary(&p, id).0, BinaryOp::Gt);
        let (p, id) = parse("x >> 2");
        assert_eq!(binary(&p, id).0, BinaryOp::Shr);
        let (p, id) = parse("x >>> 2");
        assert_eq!(binary(&p, id).0, BinaryOp::UShr);
        let (p, id) = parse("x << 2 | y & z ^ w");
        assert_eq!(binary(&p, id).0, BinaryOp::BitOr);
        // `a >> b > c` → `(a >> b) > c`: shift acima de relacional.
        let (p, id) = parse("a >> b > c");
        let (op, l, _) = binary(&p, id);
        assert_eq!(op, BinaryOp::Gt);
        assert_eq!(binary(&p, l).0, BinaryOp::Shr);
    }

    #[test]
    fn maior_sem_colagem_nao_e_shift() {
        let nomes: &'static mut dartforge_intern::Interner =
            Box::leak(Box::new(dartforge_intern::Interner::new()));
        let src = "a > > b";
        let tokens = crate::lexer::lex(src).unwrap();
        let mut p = Parser::new(src, tokens, nomes);
        assert!(p.parse_expression().is_err());
    }

    #[test]
    fn condicional_a_direita() {
        let (p, id) = parse("p ? a : b ? c : d");
        let ExprKind::Conditional {
            condition,
            then,
            else_,
        } = kind(&p, id)
        else {
            panic!()
        };
        assert_eq!(ident_text(&p, *condition), "p");
        assert_eq!(ident_text(&p, *then), "a");
        assert!(matches!(kind(&p, *else_), ExprKind::Conditional { .. }));
    }

    #[test]
    fn atribuicao_a_direita_e_compostas() {
        let (p, id) = parse("a = b = c");
        let ExprKind::Assign { op, target, value } = kind(&p, id) else {
            panic!()
        };
        assert_eq!(*op, AssignOp::Assign);
        assert_eq!(ident_text(&p, *target), "a");
        assert!(matches!(kind(&p, *value), ExprKind::Assign { .. }));

        for (src, expected) in [
            ("a += 1", BinaryOp::Add),
            ("a -= 1", BinaryOp::Sub),
            ("a *= 1", BinaryOp::Mul),
            ("a /= 1", BinaryOp::Div),
            ("a ~/= 1", BinaryOp::TruncDiv),
            ("a %= 1", BinaryOp::Rem),
            ("a <<= 1", BinaryOp::Shl),
            ("a >>= 1", BinaryOp::Shr),
            ("a >>>= 1", BinaryOp::UShr),
            ("a &= 1", BinaryOp::BitAnd),
            ("a |= 1", BinaryOp::BitOr),
            ("a ^= 1", BinaryOp::BitXor),
            ("a ??= 1", BinaryOp::IfNull),
        ] {
            let (p, id) = parse(src);
            let ExprKind::Assign { op, .. } = kind(&p, id) else {
                panic!("{src}")
            };
            assert_eq!(*op, AssignOp::Compound(expected), "{src}");
        }
    }

    #[test]
    fn unarios_prefixos_e_posfixos() {
        let (p, id) = parse("i++ + ++j");
        let (op, l, r) = binary(&p, id);
        assert_eq!(op, BinaryOp::Add);
        assert!(matches!(
            kind(&p, l),
            ExprKind::Unary {
                op: UnaryOp::PostfixInc,
                ..
            }
        ));
        assert!(matches!(
            kind(&p, r),
            ExprKind::Unary {
                op: UnaryOp::PrefixInc,
                ..
            }
        ));
        for (src, expected) in [
            ("-x", UnaryOp::Neg),
            ("~x", UnaryOp::BitNot),
            ("!f()", UnaryOp::Not),
            ("--x", UnaryOp::PrefixDec),
            ("x--", UnaryOp::PostfixDec),
            ("x!", UnaryOp::NullAssert),
        ] {
            let (p, id) = parse(src);
            let ExprKind::Unary { op, .. } = kind(&p, id) else {
                panic!("{src}")
            };
            assert_eq!(*op, expected, "{src}");
        }
        // `-x.y` é `-(x.y)`: o operando é o pós-fixo inteiro.
        let (p, id) = parse("-x.y");
        let ExprKind::Unary { operand, .. } = kind(&p, id) else {
            panic!()
        };
        assert!(matches!(kind(&p, *operand), ExprKind::Property { .. }));
    }

    #[test]
    fn seletores_null_aware_e_asserts() {
        let (p, id) = parse("a?.b?[0]?.c");
        let ExprKind::Property {
            target, null_aware, ..
        } = kind(&p, id)
        else {
            panic!()
        };
        assert!(null_aware);
        let ExprKind::Index {
            target, null_aware, ..
        } = kind(&p, *target)
        else {
            panic!()
        };
        assert!(null_aware);
        assert!(matches!(
            kind(&p, *target),
            ExprKind::Property {
                null_aware: true,
                ..
            }
        ));

        let (p, id) = parse("x!.y");
        let ExprKind::Property { target, .. } = kind(&p, id) else {
            panic!()
        };
        assert!(matches!(
            kind(&p, *target),
            ExprKind::Unary {
                op: UnaryOp::NullAssert,
                ..
            }
        ));
    }

    #[test]
    fn indice_null_aware_contra_condicional() {
        let (p, id) = parse("c ? [1] : [2]");
        assert!(matches!(kind(&p, id), ExprKind::Conditional { .. }));
        let (p, id) = parse("a?[0] ?? b");
        let (op, l, _) = binary(&p, id);
        assert_eq!(op, BinaryOp::IfNull);
        assert!(matches!(
            kind(&p, l),
            ExprKind::Index {
                null_aware: true,
                ..
            }
        ));
        assert!(p.ast.exprs.len() < 12, "especulação deixou nós na arena");
    }

    #[test]
    fn chamadas_e_metodos() {
        let (p, id) = parse("int.parse('1')");
        let ExprKind::Call { target, arguments } = kind(&p, id) else {
            panic!()
        };
        assert_eq!(arguments.args.len(), 1);
        let ExprKind::Property { target, name, .. } = kind(&p, *target) else {
            panic!()
        };
        assert_eq!(p.interner.resolve(name.sym), "parse");
        assert_eq!(ident_text(&p, *target), "int");

        let (p, id) = parse("f(1, b: 2, c: x ? y : z,)");
        let ExprKind::Call { arguments, .. } = kind(&p, id) else {
            panic!()
        };
        assert_eq!(arguments.args.len(), 3);
        assert!(arguments.args[0].name.is_none());
        assert_eq!(p.interner.resolve(arguments.args[1].name.unwrap().sym), "b");
        assert!(matches!(
            kind(&p, arguments.args[2].value),
            ExprKind::Conditional { .. }
        ));
    }

    #[test]
    fn super_e_this_como_alvos() {
        let (p, id) = parse("super.x + this[0]");
        let (_, l, r) = binary(&p, id);
        let ExprKind::Property { target, .. } = kind(&p, l) else {
            panic!()
        };
        assert!(matches!(kind(&p, *target), ExprKind::Super));
        let ExprKind::Index { target, .. } = kind(&p, r) else {
            panic!()
        };
        assert!(matches!(kind(&p, *target), ExprKind::This));
    }

    #[test]
    fn literais_numericos_booleanos_e_nulo() {
        let (p, id) = parse("1.5e3");
        assert!(matches!(kind(&p, id), ExprKind::Double(_)));
        let (p, id) = parse("0xFF");
        assert_eq!(int_text(&p, id), "0xFF");
        let (p, id) = parse("true");
        assert!(matches!(kind(&p, id), ExprKind::Bool(true)));
        let (p, id) = parse("null");
        assert!(matches!(kind(&p, id), ExprKind::Null));
    }

    #[test]
    fn strings_simples_adjacentes_e_cruas() {
        let (p, id) = parse(r#"'a\n' "b" r'\d+'"#);
        let ExprKind::String(lit) = kind(&p, id) else {
            panic!()
        };
        assert_eq!(lit.constant_value().unwrap(), "a\nb\\d+");
        assert_eq!(lit.parts.len(), 3);
    }

    #[test]
    fn strings_triplas_removem_primeira_quebra() {
        let (p, id) = parse("'''\n  x'''");
        let ExprKind::String(lit) = kind(&p, id) else {
            panic!()
        };
        assert_eq!(lit.constant_value().unwrap(), "  x");
        let (p, id) = parse("'''\n${a}\n'''");
        let ExprKind::String(lit) = kind(&p, id) else {
            panic!()
        };
        assert_eq!(lit.parts.len(), 3);
        assert!(matches!(&lit.parts[0], StringPart::Text(t) if t.is_empty()));
        assert!(matches!(&lit.parts[2], StringPart::Text(t) if *t == "\n"));
    }

    #[test]
    fn interpolacao_por_nome_e_por_chaves() {
        let (p, id) = parse(r#"'${a}b$c d${ {1}.length }e'"#);
        let ExprKind::String(lit) = kind(&p, id) else {
            panic!()
        };
        let textos: Vec<Option<&str>> = lit
            .parts
            .iter()
            .map(|part| match part {
                StringPart::Text(t) => t.as_str(),
                StringPart::Interpolation(_) => None,
            })
            .collect();
        assert_eq!(
            textos,
            [Some(""), None, Some("b"), None, Some(" d"), None, Some("e")]
        );
        let StringPart::Interpolation(a) = &lit.parts[1] else {
            panic!()
        };
        assert_eq!(ident_text(&p, *a), "a");
        let StringPart::Interpolation(c) = &lit.parts[3] else {
            panic!()
        };
        assert_eq!(ident_text(&p, *c), "c");
        let StringPart::Interpolation(len) = &lit.parts[5] else {
            panic!()
        };
        assert!(matches!(kind(&p, *len), ExprKind::Property { .. }));
    }

    #[test]
    fn interpolacao_aninhada() {
        let (p, id) = parse(r#""${'${x}'}""#);
        let ExprKind::String(lit) = kind(&p, id) else {
            panic!()
        };
        let StringPart::Interpolation(inner) = &lit.parts[1] else {
            panic!()
        };
        let ExprKind::String(inner) = kind(&p, *inner) else {
            panic!()
        };
        assert!(matches!(&inner.parts[1], StringPart::Interpolation(_)));
    }

    #[test]
    fn interpolacao_mal_fechada_e_erro() {
        let nomes: &'static mut dartforge_intern::Interner =
            Box::leak(Box::new(dartforge_intern::Interner::new()));
        let src = "'${a b}'";
        let tokens = crate::lexer::lex(src).unwrap();
        let mut p = Parser::new(src, tokens, nomes);
        assert!(p.parse_expression().is_err());
        assert!(p.diagnostics.iter().any(|d| d.code.is_some_and(|c| c.info().nome == "expected_token")));
    }

    #[test]
    fn simbolos() {
        let (p, id) = parse("#foo");
        let ExprKind::Symbol(names) = kind(&p, id) else {
            panic!()
        };
        assert_eq!(p.interner.resolve(names[0].sym), "foo");
        let (p, id) = parse("#a.b.c");
        let ExprKind::Symbol(names) = kind(&p, id) else {
            panic!()
        };
        assert_eq!(names.len(), 3);
        for (src, expected) in [
            ("#[]=", "[]="),
            ("#[]", "[]"),
            ("#+", "+"),
            ("#>>", ">>"),
            ("#>=", ">="),
            ("#void", "void"),
            ("#~/", "~/"),
        ] {
            let (p, id) = parse(src);
            let ExprKind::Symbol(names) = kind(&p, id) else {
                panic!("{src}")
            };
            assert_eq!(p.interner.resolve(names[0].sym), expected, "{src}");
        }
    }

    #[test]
    fn listas_com_elementos_de_controle() {
        let (p, id) = parse("[...a, ...?b, if (c) d else e, 1,]");
        let ExprKind::List {
            const_, elements, ..
        } = kind(&p, id)
        else {
            panic!()
        };
        assert!(!const_);
        assert_eq!(elements.len(), 4);
        assert!(matches!(
            elements[0],
            CollectionElement::Spread {
                null_aware: false,
                ..
            }
        ));
        assert!(matches!(
            elements[1],
            CollectionElement::Spread {
                null_aware: true,
                ..
            }
        ));
        let CollectionElement::If { then, else_, .. } = &elements[2] else {
            panic!()
        };
        assert!(matches!(**then, CollectionElement::Expression(_)));
        assert!(else_.is_some());
        let (p, id) = parse("const [1, 2]");
        assert!(matches!(kind(&p, id), ExprKind::List { const_: true, .. }));
    }

    #[test]
    fn mapas_e_conjuntos() {
        let (p, id) = parse("{'a': 1, 'b': x ? y : z}");
        let ExprKind::SetOrMap { elements, .. } = kind(&p, id) else {
            panic!()
        };
        assert_eq!(elements.len(), 2);
        assert!(matches!(elements[1], CollectionElement::MapEntry { .. }));
        let (p, id) = parse("{1, 2}");
        let ExprKind::SetOrMap { elements, .. } = kind(&p, id) else {
            panic!()
        };
        assert!(matches!(elements[0], CollectionElement::Expression(_)));
        let (p, id) = parse("{}");
        assert!(matches!(kind(&p, id), ExprKind::SetOrMap { .. }));
    }

    #[test]
    fn records_e_parenteses() {
        let (p, id) = parse("(a, b: 2)");
        let ExprKind::Record {
            positional, named, ..
        } = kind(&p, id)
        else {
            panic!()
        };
        assert_eq!(positional.len(), 1);
        assert_eq!(p.interner.resolve(named[0].0.sym), "b");
        let (p, id) = parse("(a,)");
        assert!(matches!(kind(&p, id), ExprKind::Record { .. }));
        let (p, id) = parse("()");
        assert!(matches!(kind(&p, id), ExprKind::Record { .. }));
        let (p, id) = parse("(a ? b : c)");
        let ExprKind::Parenthesized(inner) = kind(&p, id) else {
            panic!()
        };
        assert!(matches!(kind(&p, *inner), ExprKind::Conditional { .. }));
        let (p, id) = parse("const (1, 2)");
        assert!(matches!(
            kind(&p, id),
            ExprKind::Record { const_: true, .. }
        ));
    }

    #[test]
    fn cascatas() {
        let (p, id) = parse("a..b()..c = 1");
        let ExprKind::Cascade {
            target,
            sections,
            null_aware,
        } = kind(&p, id)
        else {
            panic!()
        };
        assert!(!null_aware);
        assert_eq!(ident_text(&p, *target), "a");
        assert_eq!(sections.len(), 2);
        let ExprKind::Call { target, .. } = kind(&p, sections[0]) else {
            panic!()
        };
        let ExprKind::Property { target, .. } = kind(&p, *target) else {
            panic!()
        };
        assert!(matches!(kind(&p, *target), ExprKind::CascadeTarget));
        let ExprKind::Assign { target, .. } = kind(&p, sections[1]) else {
            panic!()
        };
        assert!(matches!(kind(&p, *target), ExprKind::Property { .. }));

        // `a..b = c..d()`: a atribuição da seção termina antes do `..`.
        let (p, id) = parse("a..b = c..d()");
        let ExprKind::Cascade { sections, .. } = kind(&p, id) else {
            panic!()
        };
        assert_eq!(sections.len(), 2);

        let (p, id) = parse("x = a?..[0] = 1..m().n");
        let ExprKind::Assign { value, .. } = kind(&p, id) else {
            panic!()
        };
        let ExprKind::Cascade {
            sections,
            null_aware,
            ..
        } = kind(&p, *value)
        else {
            panic!()
        };
        assert!(null_aware);
        assert_eq!(sections.len(), 2);
    }

    #[test]
    fn throw_e_rethrow() {
        let (p, id) = parse("throw Error()");
        assert!(matches!(kind(&p, id), ExprKind::Throw(_)));
        let (p, id) = parse("x ?? (throw e)");
        let (_, _, r) = binary(&p, id);
        let ExprKind::Parenthesized(inner) = kind(&p, r) else {
            panic!()
        };
        assert!(matches!(kind(&p, *inner), ExprKind::Throw(_)));
        let (p, id) = parse("rethrow");
        assert!(matches!(kind(&p, id), ExprKind::Rethrow));
    }

    #[test]
    fn await_so_em_async() {
        let (p, id) = parse_with("await g()", true);
        assert!(matches!(kind(&p, id), ExprKind::Await(_)));
        let (p, id) = parse_with("await + 1", false);
        let (_, l, _) = binary(&p, id);
        assert_eq!(ident_text(&p, l), "await");
    }

    #[test]
    fn tipos_embutidos_sao_identificadores() {
        let (p, id) = parse("dynamic");
        assert_eq!(ident_text(&p, id), "dynamic");
        let (p, id) = parse("async");
        assert_eq!(ident_text(&p, id), "async");
    }

    #[test]
    fn spans_cobrem_a_expressao() {
        let (p, id) = parse("a + b * c");
        assert_eq!(p.ast.expr(id).span.start, 0);
        assert_eq!(p.ast.expr(id).span.end, 9);
    }

    #[test]
    fn erro_em_expressao_vazia() {
        let nomes: &'static mut dartforge_intern::Interner =
            Box::leak(Box::new(dartforge_intern::Interner::new()));
        for src in [")", "", "a +", "class"] {
            let tokens = crate::lexer::lex(src).unwrap();
            let mut p = Parser::new(src, tokens, nomes);
            assert!(p.parse_expression().is_err(), "{src}");
            assert!(!p.diagnostics.is_empty(), "{src}");
        }
    }

    #[test]
    fn profundidade_limitada() {
        let nomes: &'static mut dartforge_intern::Interner =
            Box::leak(Box::new(dartforge_intern::Interner::new()));
        let src = format!("{}1{}", "(".repeat(2000), ")".repeat(2000));
        let tokens = crate::lexer::lex(&src).unwrap();
        let mut p = Parser::new(&src, tokens, nomes);
        assert!(p.parse_expression().is_err());
        assert!(p.diagnostics.iter().any(|d| d.code.is_some_and(|c| c.info().nome == "stack_overflow")));
    }

    // -- Testes que dependem de outros módulos (types, patterns, statements,
    // declarations). Panicam com `not yet implemented` enquanto os stubs
    // existirem; ficam aqui para valerem assim que os módulos chegarem.

    #[test]
    fn chamada_generica_e_instanciacao_explicita() {
        let (p, id) = parse("f<int>(1)");
        let ExprKind::Call { arguments, .. } = kind(&p, id) else {
            panic!()
        };
        assert_eq!(arguments.type_args.len(), 1);
        let (p, id) = parse("Map<String, int>.from(x)");
        let ExprKind::Call { target, .. } = kind(&p, id) else {
            panic!()
        };
        let ExprKind::Property { target, .. } = kind(&p, *target) else {
            panic!()
        };
        assert!(matches!(kind(&p, *target), ExprKind::TypeArguments { .. }));
        // `a < b > c` lê como `(a < b) > c`, com o erro do fasta para
        // relacionais encadeados no segundo operador.
        let src = "a < b > c";
        let nomes: &'static mut dartforge_intern::Interner =
            Box::leak(Box::new(dartforge_intern::Interner::new()));
        let mut p = Parser::new(src, crate::lexer::lex(src).unwrap(), nomes);
        let id = p.parse_expression().unwrap();
        assert_eq!(binary(&p, id).0, BinaryOp::Gt);
        assert_eq!(p.diagnostics.len(), 1);
        assert_eq!(p.diagnostics[0].code, Some(dartforge_diagnostics::codigos::parser::EQUALITY_CANNOT_BE_EQUALITY_OPERAND));
        let (p, id) = parse("a < b && c > d");
        let (op, l, r) = binary(&p, id);
        assert_eq!(op, BinaryOp::And);
        assert_eq!(binary(&p, l).0, BinaryOp::Lt);
        assert_eq!(binary(&p, r).0, BinaryOp::Gt);
    }

    #[test]
    fn is_e_as() {
        let (p, id) = parse("x is! int");
        assert!(matches!(kind(&p, id), ExprKind::Is { negated: true, .. }));
        let (p, id) = parse("y as int?");
        assert!(matches!(kind(&p, id), ExprKind::As { .. }));
        let (p, id) = parse("x is int && y");
        assert_eq!(binary(&p, id).0, BinaryOp::And);
    }

    #[test]
    fn criacao_de_instancia() {
        let (p, id) = parse("new A.b()");
        let ExprKind::InstanceCreation {
            keyword,
            constructor,
            ..
        } = kind(&p, id)
        else {
            panic!()
        };
        assert_eq!(*keyword, Some(CreationKeyword::New));
        assert!(constructor.is_none(), "A.b fica como nome de duas partes");
        let (p, id) = parse("const p.T<A>.n(1)");
        let ExprKind::InstanceCreation {
            keyword,
            constructor,
            ..
        } = kind(&p, id)
        else {
            panic!()
        };
        assert_eq!(*keyword, Some(CreationKeyword::Const));
        assert!(constructor.is_some());
        let (p, id) = parse("const <String, List<int>>{}");
        assert!(matches!(
            kind(&p, id),
            ExprKind::SetOrMap { const_: true, .. }
        ));
    }

    #[test]
    fn expressoes_de_funcao() {
        let (p, id) = parse("() async {}");
        assert!(matches!(kind(&p, id), ExprKind::FunctionExpression(_)));
        let (p, id) = parse("(a, b) { return a; }");
        assert!(matches!(kind(&p, id), ExprKind::FunctionExpression(_)));
        let (p, id) = parse("<T>(T t) => t");
        assert!(matches!(kind(&p, id), ExprKind::FunctionExpression(_)));
        let (p, id) = parse("() async* { yield 1; }");
        assert!(matches!(kind(&p, id), ExprKind::FunctionExpression(_)));
    }

    #[test]
    fn switch_como_expressao() {
        let (p, id) = parse("switch (x) { 1 => 'a', _ when y => 'b', _ => 'c', }");
        let ExprKind::Switch { cases, .. } = kind(&p, id) else {
            panic!()
        };
        assert_eq!(cases.len(), 3);
        assert!(cases[1].guard.is_some());
    }

    #[test]
    fn for_em_colecao_e_if_case() {
        let (p, id) = parse("[for (var x in xs) x * 2, if (e case int v when v > 0) v]");
        let ExprKind::List { elements, .. } = kind(&p, id) else {
            panic!()
        };
        assert!(matches!(elements[0], CollectionElement::ForIn { .. }));
        assert!(matches!(
            elements[1],
            CollectionElement::If {
                case_pattern: Some(_),
                guard: Some(_),
                ..
            }
        ));
    }

    #[test]
    fn atribuicao_por_padrao() {
        let (p, id) = parse("(a, b) = e");
        assert!(matches!(kind(&p, id), ExprKind::PatternAssign { .. }));
        let (p, id) = parse("[x, y] = e");
        assert!(matches!(kind(&p, id), ExprKind::PatternAssign { .. }));
    }

    /// Regressões do corpus: o `?` depois de `is T`/`as T` é da condicional
    /// quando o que segue inicia expressão, e do tipo quando não.
    #[test]
    fn is_com_condicional_e_com_tipo_nulavel() {
        for src in [
            "x is T ? a : b",
            "x is Uint8List ? x : Uint8List.fromList(x)",
            "x is bool ? x : throw e",
            "x is int ? this == other : false",
            "x is Map ? {} : y",
            "x as int ? \"https\" : \"http\"",
        ] {
            let (p, id) = parse(src);
            let ExprKind::Conditional { condition, .. } = kind(&p, id) else {
                panic!("{src}: {:?}", kind(&p, id))
            };
            let ty = match kind(&p, *condition) {
                ExprKind::Is { ty, .. } | ExprKind::As { ty, .. } => *ty,
                other => panic!("{src}: {other:?}"),
            };
            assert!(!p.ast.ty(ty).nullable, "{src}");
        }
        for src in ["x is int?", "x is int? && y", "x is int? || y", "(x as T?)"] {
            let (p, id) = parse(src);
            let mut cur = id;
            loop {
                match kind(&p, cur) {
                    ExprKind::Is { ty, .. } | ExprKind::As { ty, .. } => {
                        assert!(p.ast.ty(*ty).nullable, "{src}");
                        break;
                    }
                    ExprKind::Binary { left, .. } => cur = *left,
                    ExprKind::Parenthesized(inner) => cur = *inner,
                    other => panic!("{src}: {other:?}"),
                }
            }
        }
    }

    /// `.5` é literal `double` (gramática: `'.' DIGIT+`).
    #[test]
    fn double_iniciado_por_ponto() {
        let (p, id) = parse("log(n - .5)");
        let ExprKind::Call { arguments, .. } = kind(&p, id) else {
            panic!()
        };
        let ExprKind::Binary { right, .. } = kind(&p, arguments.args[0].value) else {
            panic!()
        };
        assert!(matches!(kind(&p, *right), ExprKind::Double(_)));
    }

    /// Elementos null-aware (Dart 3.8): `?e`, `?k: v`, `k: ?v`.
    #[test]
    fn elementos_null_aware() {
        let (p, id) = parse("[a, ?b, ...c]");
        let ExprKind::List { elements, .. } = kind(&p, id) else {
            panic!()
        };
        assert!(matches!(elements[0], CollectionElement::Expression(_)));
        assert!(matches!(
            elements[1],
            CollectionElement::NullAwareExpression(_)
        ));
        let (p, id) = parse("{'k': ?v, ?k2: v2, ?k3: ?v3}");
        let ExprKind::SetOrMap { elements, .. } = kind(&p, id) else {
            panic!("{:?}", kind(&p, id))
        };
        let flags = |i: usize| match &elements[i] {
            CollectionElement::MapEntry {
                null_aware_key,
                null_aware_value,
                ..
            } => (*null_aware_key, *null_aware_value),
            other => panic!("{other:?}"),
        };
        assert_eq!(flags(0), (false, true));
        assert_eq!(flags(1), (true, false));
        assert_eq!(flags(2), (true, true));
    }

    /// Surrogates: par de escapes vira o escalar; solto é guardado como
    /// unidade UTF-16, como o Dart mede.
    #[test]
    fn strings_com_surrogates() {
        let (p, id) = parse(r"'👭'");
        let ExprKind::String(lit) = kind(&p, id) else {
            panic!()
        };
        let v = lit.constant_value().unwrap();
        assert_eq!(v.as_str(), Some("\u{1F46D}"));
        assert_eq!(v.utf16_len(), 2);
        let (p, id) = parse(r"'\uD800-\uDBFF'");
        let ExprKind::String(lit) = kind(&p, id) else {
            panic!()
        };
        let v = lit.constant_value().unwrap();
        assert_eq!(v.as_str(), None);
        assert_eq!(v.utf16_len(), 3);
        // Literais adjacentes concatenam por unidade: o par se forma.
        let (p, id) = parse(r"'\uD83D' '\uDC6D'");
        let ExprKind::String(lit) = kind(&p, id) else {
            panic!()
        };
        assert_eq!(lit.constant_value().unwrap().as_str(), Some("\u{1F46D}"));
    }
}
