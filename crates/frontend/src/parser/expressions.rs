//! Expressions, literals, arguments, cascades and function expressions.
//!
//! The Dart 3.6 expression grammar (§17 of the specification) is read by
//! recursive descent with *precedence climbing* for binary operators: a
//! single function (`Parser::parse_binary`) handles every level from `??` to
//! `*`, so the stack depth grows with the expression's real nesting and not
//! with the number of grammar levels.
//!
//! Disambiguation decisions that follow the SDK parser (`parser_impl.dart`),
//! because the target corpus was validated by it:
//!
//! * `>` arrives isolated from the lexer; `Parser::composed_gt` recomposes it
//!   into `>=`, `>>`, `>>>`, `>>=`, `>>>=` only between glued tokens and only
//!   outside type argument lists.
//! * `e<T>(...)`/`e<T>.x`: `<...>` is read as type arguments when the list is
//!   well formed **and** the next token is one of those that may follow type
//!   arguments (`(`, `.`, `==`, `!=`, `)`, `]`, `}`, `;`, `:`, `,`, end of
//!   interpolation, end of file). That way `a < b > c` is relational and
//!   `f<int>(1)` is a generic call.
//! * `e?[i]` versus `c ? [a] : b`: as in the SDK, what follows the `?` is
//!   tried speculatively as `expression : expression`; if that works it is a
//!   conditional, otherwise a null-aware index. The speculation restores the
//!   cursor, the arenas and the diagnostics.
//! * `(` in primary position is a function expression if the matching `)` is
//!   followed by `{`, `=>`, `async` or `sync`; otherwise it is a record or a
//!   parenthesized expression (a record when there is a comma, a named field
//!   or nothing inside).
//! * `new`/`const T.n()` stores the name like the SDK: `T.n` without type
//!   arguments becomes a two-part name and `constructor` stays `None`; only
//!   after type arguments (or an already qualified name) does `.n` become
//!   `constructor`. Telling a library prefix from a constructor name needs
//!   resolution.
//! * `throw` and `rethrow` are only accepted at the `expression` level, as in
//!   the grammar (`x ?? throw e` needs parentheses, and the corpus has them).
//! * `await` is an operator in an `async` body; outside it it is an
//!   identifier, except when it looks like an expression
//!   (`looksLikeAwaitExpression`, with an error).
//!
//! In constant patterns (`parse_unary(true)`) a trailing `!` not followed by
//! a selector is left to the pattern parser (it is a null-assert pattern),
//! exactly as the SDK does with `ConstantPatternContext`.
use super::{ComposedGt, ForHeader, PResult, Parser};
use crate::ast::{
    Argument, Arguments, AssignOp, BinaryOp, CollectionElement, CreationKeyword, Expr, ExprId,
    ExprKind, Function, FunctionKind, Name, StringLit, StringPart, SwitchExprCase, TypeAnnotation,
    TypeId, TypeKind, UnaryOp,
};
use crate::features::Feature;
use crate::token::{Interp, Keyword, Kind, Op, StrFlags, Token};
use dartforge_diagnostics::{Span, codes};

/// Precedence levels of the binary operators, from lowest to highest. The
/// values are contiguous because `Parser::parse_binary` uses `level + 1` for
/// the right operand (left associativity).
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
/// `PREFIX_PRECEDENCE`: the `~` between two operands (`a ~ b`).
const LEVEL_PREFIX: u8 = 11;

/// Binary operator found at the cursor, with its level and how many tokens
/// it takes.
#[derive(Debug, Clone, Copy)]
enum BinaryHere {
    Op { level: u8, op: BinaryOp, len: usize },
    Is,
    As,
}

/// State saved for speculative parsing (`?[` versus `? [...] :`).
///
/// Restoring discards the nodes created and the diagnostics emitted during
/// the speculation; since the arenas are append-only and nothing outside the
/// speculation references the new nodes, truncating is enough.
struct Checkpoint {
    pos: usize,
    splices: usize,
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

/// The first escape error of `content` (the text between the quotes, not
/// raw) as fasta's `unescapeCodeUnits` (`quote.dart:186`) reports it: code,
/// relative start (the `\`) and length.
fn escape_error(b: &[u8]) -> Option<(dartforge_diagnostics::Code, usize, usize)> {
    use codes::parser as c;
    let n = b.len();
    let hex = |x: u8| x.is_ascii_hexdigit();
    let value = |x: u8| (x as char).to_digit(16).unwrap_or(0);
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
        // fasta's `begin` is the index of the `x`/`u`; the error starts at the `\`.
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
                    return Some((
                        c::INVALID_UNICODE_ESCAPE_U_STARTED,
                        begin - 1,
                        n + 1 - begin,
                    ));
                }
                let mut code: u32 = 0;
                if b[i + 1] == b'{' {
                    i += 1;
                    let mut closed = false;
                    for j in 0..7 {
                        i += 1;
                        if i == n {
                            return Some((
                                c::INVALID_UNICODE_ESCAPE_U_BRACKET,
                                begin - 1,
                                i + 1 - begin,
                            ));
                        }
                        let d = b[i];
                        if j != 0 && d == b'}' {
                            closed = true;
                            break;
                        } else if j == 6 {
                            break;
                        }
                        if !hex(d) {
                            return Some((
                                c::INVALID_UNICODE_ESCAPE_U_BRACKET,
                                begin - 1,
                                i + 2 - begin,
                            ));
                        }
                        code = (code << 4) + value(d);
                    }
                    if !closed {
                        return Some((
                            c::INVALID_UNICODE_ESCAPE_U_BRACKET,
                            begin - 1,
                            i + 1 - begin,
                        ));
                    }
                } else {
                    if n <= i + 4 {
                        return Some((
                            c::INVALID_UNICODE_ESCAPE_U_NO_BRACKET,
                            begin - 1,
                            n + 1 - begin,
                        ));
                    }
                    for _ in 0..4 {
                        i += 1;
                        if !hex(b[i]) {
                            return Some((
                                c::INVALID_UNICODE_ESCAPE_U_NO_BRACKET,
                                begin - 1,
                                i + 1 - begin,
                            ));
                        }
                        code = (code << 4) + value(b[i]);
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
    // -- Public entry points of the contract -------------------------------

    /// Full `expression`, including assignment, cascade and `throw`.
    pub(crate) fn parse_expression(&mut self) -> PResult<ExprId> {
        self.parse_expression_ex(true)
    }

    /// `expressionWithoutCascade` — used in cascade arguments and in places
    /// where `..` would close the expression.
    pub(crate) fn parse_expression_without_cascade(&mut self) -> PResult<ExprId> {
        self.parse_expression_ex(false)
    }

    /// `bitwiseOrExpression` — operand of relational patterns.
    pub(crate) fn parse_bitwise_or_expression(&mut self) -> PResult<ExprId> {
        self.parse_binary(LEVEL_BIT_OR)
    }

    /// Digit separators of the numeric literal at `span`: when misplaced (not
    /// between two digits), `UNEXPECTED_SEPARATOR_IN_NUMBER` at the start of the
    /// literal, with length 1 (the scanner's error token); in a library before
    /// 3.6, `EXPERIMENT_NOT_ENABLED` on the whole literal (`digit-separators`
    /// arrived in 3.6).
    fn check_separators(&mut self, span: Span) {
        let text = &self.source.as_bytes()[span.start..span.end];
        if !text.contains(&b'_') {
            return;
        }
        let hex = text.len() > 1 && matches!(text[1], b'x' | b'X');
        let digit = |b: u8| {
            if hex {
                b.is_ascii_hexdigit()
            } else {
                b.is_ascii_digit()
            }
        };
        let start_offset = if hex { 2 } else { 0 };
        let mut outside = false;
        for (i, &b) in text.iter().enumerate().skip(start_offset) {
            if b != b'_' {
                continue;
            }
            let before = text[start_offset..i].iter().rev().find(|&&c| c != b'_');
            let after = text[i + 1..].iter().find(|&&c| c != b'_');
            if !before.is_some_and(|&c| digit(c)) || !after.is_some_and(|&c| digit(c)) {
                outside = true;
                break;
            }
        }
        if outside {
            let one = Span {
                start: span.start,
                end: span.start + 1,
            };
            self.error_at(codes::scanner::UNEXPECTED_SEPARATOR_IN_NUMBER, one, &[]);
        }
        if self.features.version() < crate::features::LanguageVersion::new(3, 6) {
            self.error_at(
                codes::parser::EXPERIMENT_NOT_ENABLED,
                span,
                &["digit-separators", "3.6.0", "3.6.0"],
            );
        }
    }

    /// `(args)` with the parentheses; without type arguments (the caller reads
    /// them).
    ///
    /// An argument is named when it is an identifier followed by `:`.
    pub(crate) fn parse_arguments(&mut self) -> PResult<Arguments> {
        self.with_functions(|p| p.parse_arguments_inner())
    }

    /// `mayParseFunctionExpressions = true` around `f`: fasta turns function
    /// literals back on inside arguments, parentheses, indices, collections and
    /// `assert`, and string interpolations (`parseArgumentsRest`,
    /// `parseParenthesizedExpressionOrRecordLiteral`, `parseLiteralListSuffix`,
    /// `parseLiteralSetOrMapSuffix`, `parseAssert`, `parseLiteralString`).
    pub(crate) fn with_functions<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let saved = std::mem::replace(&mut self.without_named_function, false);
        let r = f(self);
        self.without_named_function = saved;
        r
    }

    fn parse_arguments_inner(&mut self) -> PResult<Arguments> {
        let opener = self.pos;
        let start = self.expect_op(Op::LParen)?.span;
        // The arguments go to the shared scratch space from `base` (nested calls
        // push on top) and come out in a single exact-size allocation. A local
        // `Vec` would grow by doubling and reallocate when turned into a
        // `Box<[T]>`: in `new_sali` there were 57 thousand calls.
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
                    // `parseArgumentsRest`: what looks like the start of another expression is
                    // an argument after a missing comma; the rest closes in `ensureCloseParen`.
                    if !self.at_op(Op::RParen) && self.looks_like_expression_start(self.pos) {
                        self.error(codes::parser::EXPECTED_TOKEN, &[","]);
                        continue;
                    }
                    break;
                }
            }
            self.ensure_close_paren(opener)
                .map(|()| self.tokens[self.pos - 1])
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

    /// String literal (with adjacent ones and interpolation) in URI or `native`
    /// position.
    ///
    /// Adjacent literals (`'a' 'b'`) become a single [`StringLit`] with the
    /// pieces concatenated in `parts`. Each text piece arrives already decoded
    /// ([`crate::lexer::decode_string`]).
    pub(crate) fn parse_string_literal(&mut self) -> PResult<StringLit> {
        let start = self.span();
        // Pieces go to the shared scratch space from `base` (a string inside an
        // interpolation pushes on top) and come out in an exact allocation: most
        // literals have a single piece, and a local `Vec` reserved four.
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
                return Err(self.error(codes::parser::EXPECTED_STRING_LITERAL, &[]));
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

    // -- Lower levels: throw, pattern assignment, assignment ---------------

    /// `expression` or `expressionWithoutCascade`, depending on `allow_cascade`.
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
        // fasta's `allowPatterns`: before 3.0 (patterns), `(a) = 0` is an
        // assignment to a parenthesized expression.
        if self.features.version() >= crate::features::LanguageVersion::new(3, 0)
            && self.pattern_assignment_ahead()
            && self.looks_like_pattern_assignment(self.pos)
        {
            let pattern = self.parse_pattern_in(super::patterns::PatternContext::Assignment)?;
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
            self.check_assignment(left, value);
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

    /// The rest of an expression whose primary the parser's recovery already
    /// built (a rewriter's synthetic tokens): its selectors, the binary
    /// operators, the conditional, the cascade and the assignment, as
    /// [`Parser::parse_expression`] reads them after a primary.
    pub(crate) fn parse_expression_after_primary(
        &mut self,
        start: Span,
        primary: ExprId,
    ) -> PResult<ExprId> {
        let left = self.parse_selectors_at(start, primary, false, false)?;
        let left = self.parse_binary_rest(start, left, LEVEL_IF_NULL)?;
        let mut left = self.parse_conditional_rest(start, left)?;
        if matches!(self.kind(), Kind::Op(Op::DotDot | Op::QuestionDotDot)) {
            left = self.parse_cascade_rest(start, left)?;
        }
        if let Some((op, len)) = self.assignment_op_here() {
            for _ in 0..len {
                self.advance();
            }
            let value = self.parse_expression()?;
            self.check_assignment(left, value);
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

    /// Cheap pre-filter for pattern assignment: an `outerPattern` (`(…)`,
    /// `[…]`, `{…}`, `<T>[…]`, `<T>{…}`, `Name(…)`, `p.Name<T>(…)`) whose
    /// closing delimiter is followed by `=`. Only then is it worth asking
    /// `looks_like_pattern_assignment`, which checks the content.
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

    /// Index of the `>` that closes the `<` at `open`, counting nesting, or
    /// `None` if `{`, `}`, `;` or end of file appears first (none of that fits
    /// in a type parameter or argument list).
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

    /// Assignment operator at the cursor and how many tokens it takes (`>>=`
    /// and `>>>=` arrive as isolated `>`).
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

    /// A `>` composition valid in the current context: inside type arguments
    /// `>` never becomes a shift (only `>` or `>=`).
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

    // -- Cascade -----------------------------------------------------------

    /// Sections `..x`, `?..x`, `..[i]`, each one an expression whose innermost
    /// receiver is [`ExprKind::CascadeTarget`]. A section may end in an
    /// assignment, whose right-hand side is `expressionWithoutCascade`.
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
                // `parseCascadeExpression`: the section's name is `parseSend`.
                let name = self.member_name(false)?;
                self.push(
                    section_start,
                    ExprKind::Property {
                        target: receiver,
                        name,
                        null_aware: false,
                    },
                )
            };
            section = self.parse_selectors_at(section_start, section, false, true)?;
            if let Some((op, len)) = self.assignment_op_here() {
                for _ in 0..len {
                    self.advance();
                }
                let value = self.parse_expression_without_cascade()?;
                self.check_assignment(section, value);
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

    // -- Conditional and binaries ------------------------------------------

    /// `c ? a : b`, right associative; the branches are
    /// `expressionWithoutCascade`.
    fn parse_conditional(&mut self) -> PResult<ExprId> {
        let start = self.span();
        let condition = self.parse_binary(LEVEL_IF_NULL)?;
        self.parse_conditional_rest(start, condition)
    }

    /// Continuation of [`Parser::parse_conditional`] with the condition read.
    fn parse_conditional_rest(&mut self, start: Span, condition: ExprId) -> PResult<ExprId> {
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

    /// Binary operators from `??` to `*` by *precedence climbing*: reads a unary
    /// operand and, while the operator at the cursor has level ≥ `min_level`,
    /// reads the right operand at the next level. `is`/`as` enter at the
    /// relational level with a type on the right.
    fn parse_binary(&mut self, min_level: u8) -> PResult<ExprId> {
        let start = self.span();
        let left = self.parse_unary(false)?;
        self.parse_binary_rest(start, left, min_level)
    }

    /// Continuation of `Parser::parse_binary` with the left operand already
    /// read (the constant pattern reads the operand with its own rules).
    pub(crate) fn parse_binary_rest(
        &mut self,
        start: Span,
        mut left: ExprId,
        min_level: u8,
    ) -> PResult<ExprId> {
        // fasta's `lastBinaryExpressionLevel` (`parser_impl.dart:6094`): two
        // equality (`a == b == c`) or relational (`a < b < c`) operators in a row
        // at the same level are `EQUALITY_CANNOT_BE_EQUALITY_OPERAND` at the
        // second, and reading goes on.
        let mut last_level: Option<u8> = None;
        loop {
            let Some(here) = self.binary_here() else {
                match self.spelled_out_operator(min_level) {
                    Some((level, op, text)) => {
                        let word = self.text().to_string();
                        self.error(codes::parser::BINARY_OPERATOR_WRITTEN_OUT, &[&word, text]);
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
                        if last_level == Some(level) {
                            let end_offset = self.peek_at(len - 1).span.end;
                            let span = Span {
                                start: self.span().start,
                                end: end_offset,
                            };
                            self.error_at(
                                codes::parser::EQUALITY_CANNOT_BE_EQUALITY_OPERAND,
                                span,
                                &[],
                            );
                        }
                        last_level = Some(level);
                    } else {
                        last_level = None;
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
                    let ty = self.type_after_is_or_as()?;
                    left = self.push(
                        start,
                        ExprKind::Is {
                            value: left,
                            ty,
                            negated,
                        },
                    );
                    self.skip_chained_is_as()?;
                }
                BinaryHere::As => {
                    if LEVEL_RELATIONAL < min_level {
                        break;
                    }
                    self.advance();
                    let ty = self.type_after_is_or_as()?;
                    left = self.push(start, ExprKind::As { value: left, ty });
                    self.skip_chained_is_as()?;
                }
            }
        }
        Ok(left)
    }

    /// `looksLikePatternStart` (`identifier_context.dart:372`).
    fn looks_like_pattern_start(&self, pos: usize) -> bool {
        match self.kind_of(pos) {
            Kind::Ident | Kind::Int | Kind::Double | Kind::Str(_) | Kind::StrBegin(..) => true,
            Kind::Keyword(k) => matches!(
                k,
                Keyword::Null
                    | Keyword::False
                    | Keyword::True
                    | Keyword::Var
                    | Keyword::Final
                    | Keyword::Const
            ),
            Kind::Op(op) => matches!(
                op,
                Op::Hash
                    | Op::LBrace
                    | Op::LParen
                    | Op::LBracket
                    | Op::Lt
                    | Op::LtEq
                    | Op::Gt
                    | Op::BangEq
                    | Op::EqEq
            ),
            _ => false,
        }
    }

    /// The type after `is`/`as` (`computeTypeAfterIsOrAs` +
    /// `ensureTypeNotVoid`): what cannot start a type is `EXPECTED_TYPE_NAME` at
    /// it, with an empty synthetic type, and the expression goes on
    /// (`a is "A"`).
    pub(crate) fn type_after_is_or_as(&mut self) -> PResult<TypeId> {
        if matches!(
            self.kind(),
            Kind::Ident | Kind::Keyword(Keyword::Void) | Kind::Op(Op::LParen)
        ) {
            return self.parse_type_after_is_or_as();
        }
        let here = self.span();
        self.error_at(codes::parser::EXPECTED_TYPE_NAME, here, &[]);
        let s = Span {
            start: here.start,
            end: here.start,
        };
        let name = self.name_from("", s);
        Ok(self.ast.push_type(crate::ast::TypeAnnotation {
            span: s,
            nullable: false,
            kind: crate::ast::TypeKind::Named {
                name: vec![name].into_boxed_slice(),
                args: Box::default(),
            },
        }))
    }

    /// `skipChainedAsIsOperators` (`parser_impl.dart`): `is`/`as` right after
    /// another one is `UNEXPECTED_TOKEN` at it, and its type is skipped.
    fn skip_chained_is_as(&mut self) -> PResult<()> {
        while self.at_kw(Keyword::Is) || self.at_ident("as") {
            let text = self.text().to_string();
            self.error(codes::parser::UNEXPECTED_TOKEN, &[&text]);
            self.advance();
            self.eat_op(Op::Bang);
            self.type_after_is_or_as()?;
        }
        Ok(())
    }

    /// fasta's `_attemptPrecedenceLevelRecovery`: an identifier `and`, `or`,
    /// `xor`, `shl` or `shr` where a binary operator fits is read, tentatively,
    /// as each operator it describes (`and`: `&`, then `&&`); the first read
    /// without errors that ends at a plausible point (`;`, `,`, `)`, `{`, `}`,
    /// `|`, `||`, `&`, `&&`, end or another of these words) is accepted, with
    /// `BINARY_OPERATOR_WRITTEN_OUT`.
    fn spelled_out_operator(&mut self, min_level: u8) -> Option<(u8, BinaryOp, &'static str)> {
        if self.recovering_operator || !self.is_plain_identifier(self.pos) {
            return None;
        }
        let swaps: &[(u8, BinaryOp, &'static str)] = match self.text() {
            "xor" => &[(LEVEL_BIT_XOR, BinaryOp::BitXor, "^")],
            "and" => &[
                (LEVEL_BIT_AND, BinaryOp::BitAnd, "&"),
                (LEVEL_AND, BinaryOp::And, "&&"),
            ],
            "or" => &[
                (LEVEL_BIT_OR, BinaryOp::BitOr, "|"),
                (LEVEL_OR, BinaryOp::Or, "||"),
            ],
            "shl" => &[(LEVEL_SHIFT, BinaryOp::Shl, "<<")],
            "shr" => &[(LEVEL_SHIFT, BinaryOp::Shr, ">>")],
            _ => return None,
        };
        for &(level, op, text) in swaps {
            if level < min_level {
                continue;
            }
            let cp = self.checkpoint();
            self.recovering_operator = true;
            self.advance();
            let start_offset = self.span();
            let ok = self
                .parse_binary(level + 1)
                .and_then(|r| self.parse_binary_rest(start_offset, r, min_level))
                .is_ok();
            let accepts = ok
                && self.diagnostics.len() == cp.diagnostics
                && (matches!(
                    self.kind(),
                    Kind::Op(
                        Op::Semicolon
                            | Op::Comma
                            | Op::RParen
                            | Op::LBrace
                            | Op::RBrace
                            | Op::Pipe
                            | Op::PipePipe
                            | Op::Amp
                            | Op::AmpAmp
                    ) | Kind::Eof
                ) || (self.is_plain_identifier(self.pos)
                    && matches!(self.text(), "xor" | "and" | "or" | "shl" | "shr")));
            self.recovering_operator = false;
            self.restore(cp);
            if accepts {
                return Some((level, op, text));
            }
        }
        None
    }

    /// Binary operator (or `is`) at the cursor with a level from equality up:
    /// the level, the text of the fasta token (`>>`, `>=`, `is`) and its range.
    /// `as` is left out (it is the cast pattern).
    pub(crate) fn pattern_binary_operator(&self) -> Option<(u8, &'static str, Span)> {
        let start_offset = self.span();
        let (level, text, len) = match self.binary_here()? {
            BinaryHere::As => return None,
            BinaryHere::Is => (LEVEL_RELATIONAL, "is", 1),
            BinaryHere::Op { level, op, len } => {
                let text = match op {
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
                (level, text, len)
            }
        };
        if level < LEVEL_EQUALITY {
            return None;
        }
        let end_offset = self.peek_at(len - 1).span.end;
        Some((
            level,
            text,
            Span {
                start: start_offset.start,
                end: end_offset,
            },
        ))
    }

    /// Binary operator at the cursor, if any, with its level and size in tokens.
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
            // `as` is a built-in identifier; after an operand it can only be the cast
            // operator.
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
            // `parsePrecedenceExpression` (`parser_impl.dart:6077-6113`): a
            // `PREFIX_PRECEDENCE` token after the operand falls into the binary
            // expression branch.
            Kind::Op(Op::Tilde) => (LEVEL_PREFIX, BinaryOp::NotBinary, 1),
            _ => return None,
        };
        Some(BinaryHere::Op { level, op, len })
    }

    // -- Unaries, postfixes and selectors ----------------------------------

    /// Prefixes `-`, `!`, `~`, `++`, `--`, `await`, followed by the postfix.
    ///
    /// `constant_pattern` leaves the trailing postfix `!` to the pattern parser.
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
            // fasta's `parseUnaryExpression`: outside `async`/`async*`, an `await` that
            // looks like an expression (`looksLikeAwaitExpression`, only in a plain
            // synchronous body; in `sync*` always) is read as such and reported
            // (`AWAIT_IN_WRONG_CONTEXT` at the `await`).
            Kind::Ident
                if self.text() == "await"
                    && (self.in_async
                        || self.in_generator
                        || self.looks_like_expression_after_await(self.pos, true)) =>
            {
                self.advance();
                let operand = self.parse_unary(constant_pattern)?;
                if !self.in_async {
                    self.error_at(
                        codes::compile_time_error::AWAIT_IN_WRONG_CONTEXT,
                        start,
                        &[],
                    );
                }
                return Ok(self.push(start, ExprKind::Await(operand)));
            }
            _ => return self.parse_postfix(constant_pattern),
        };
        self.advance();
        let operand = self.parse_unary(constant_pattern)?;
        if matches!(op, UnaryOp::PrefixInc | UnaryOp::PrefixDec) && !self.is_assignable(operand) {
            // `handleUnaryPrefixAssignmentExpression` (`ast_builder.dart:5704`): at the
            // operand's last token.
            let last = self.tokens[self.pos - 1].span;
            self.error_at(codes::parser::MISSING_ASSIGNABLE_SELECTOR, last, &[]);
        }
        Ok(self.push(start, ExprKind::Unary { op, operand }))
    }

    /// The analyzer's `Expression.isAssignable` (`ast.dart`): only identifiers
    /// (simple or prefixed), property access and index.
    pub(crate) fn is_assignable(&self, id: ExprId) -> bool {
        matches!(
            self.ast.expr(id).kind,
            ExprKind::Identifier(_) | ExprKind::Property { .. } | ExprKind::Index { .. }
        )
    }

    /// `handleAssignmentExpression` (`ast_builder.dart:3752`): a non-assignable
    /// left-hand side is `MISSING_ASSIGNABLE_SELECTOR` on the whole of it;
    /// `super` on the right (`reportErrorIfSuper`), likewise at the `super`.
    fn check_assignment(&mut self, target_expr: ExprId, value: ExprId) {
        if !self.is_assignable(target_expr) {
            let span = self.ast.expr(target_expr).span;
            self.error_at(codes::parser::MISSING_ASSIGNABLE_SELECTOR, span, &[]);
        }
        self.check_stray_super(value);
    }

    /// `reportErrorIfSuper` (`ast_builder.dart:5840`): `super` alone as an
    /// expression is `MISSING_ASSIGNABLE_SELECTOR`.
    pub(crate) fn check_stray_super(&mut self, id: ExprId) {
        if matches!(self.ast.expr(id).kind, ExprKind::Super) {
            let span = self.ast.expr(id).span;
            self.error_at(codes::parser::MISSING_ASSIGNABLE_SELECTOR, span, &[]);
        }
    }

    /// fasta's `looksLikeExpressionAfterAwaitOrYield`, with the `await` or
    /// `yield` at `pos`: what follows only makes sense as an operand
    /// (`await f();`, `await f + 1`, `if (await f)`, `await null`). `unario` is
    /// the `UnaryExpression` context (also `await f,` and `await f;`).
    pub(crate) fn looks_like_expression_after_await(&self, pos: usize, unary: bool) -> bool {
        let binary = |k: Kind| {
            matches!(
                k,
                Kind::Op(
                    Op::Amp
                        | Op::AmpAmp
                        | Op::Pipe
                        | Op::PipePipe
                        | Op::Caret
                        | Op::EqEq
                        | Op::Gt
                        | Op::Lt
                        | Op::LtEq
                        | Op::LtLt
                        | Op::Minus
                        | Op::Percent
                        | Op::Plus
                        | Op::QuestionQuestion
                        | Op::Slash
                        | Op::Star
                        | Op::TildeSlash
                )
            )
        };
        match self.kind_of(pos + 1) {
            Kind::Ident => {
                let t = self.kind_of(pos + 2);
                match t {
                    Kind::Op(Op::LParen) => {
                        let Some(closes) = self.matching_close(pos + 2) else {
                            return false;
                        };
                        let d = self.kind_of(closes + 1);
                        matches!(
                            d,
                            Kind::Op(
                                Op::Semicolon
                                    | Op::Dot
                                    | Op::Comma
                                    | Op::DotDot
                                    | Op::Question
                                    | Op::QuestionDot
                                    | Op::RParen
                            )
                        ) || binary(d)
                    }
                    Kind::Op(Op::Dot | Op::RParen | Op::RBracket) => true,
                    Kind::Op(Op::Comma | Op::Semicolon) => unary,
                    _ => binary(t),
                }
            }
            Kind::Keyword(Keyword::Null) => true,
            _ => false,
        }
    }

    /// Primary, selectors and postfix `++`/`--`/`!`.
    fn parse_postfix(&mut self, constant_pattern: bool) -> PResult<ExprId> {
        let start = self.span();
        let creation = self.implicit_creation_ahead();
        let mark = self.ast.exprs.len();
        let primary = self.parse_primary()?;
        let expr = self.parse_selectors_at(start, primary, constant_pattern, false)?;
        if let Some(parens) = creation {
            let start_offset = self.tokens[parens].span.start;
            let call = (mark..self.ast.exprs.len()).find(|&i| {
                matches!(&self.ast.exprs[i].kind, ExprKind::Call { arguments, .. } if arguments.span.start == start_offset)
            });
            if let Some(i) = call {
                self.ast.implicit_creations.push(ExprId(i as u32));
            }
        }
        let op = match self.kind() {
            Kind::Op(Op::PlusPlus) => UnaryOp::PostfixInc,
            Kind::Op(Op::MinusMinus) => UnaryOp::PostfixDec,
            _ => return Ok(expr),
        };
        if !self.is_assignable(expr) {
            // `handleUnaryPostfixAssignmentExpression` (`ast_builder.dart:5688`).
            let span = self.span();
            self.error_at(
                codes::parser::ILLEGAL_ASSIGNMENT_TO_NON_ASSIGNABLE,
                span,
                &[],
            );
        }
        self.advance();
        Ok(self.push(start, ExprKind::Unary { op, operand: expr }))
    }

    /// `parseUnaryExpression` (`parser_impl.dart:6426-6447`): at the start of
    /// the unary expression, `id ('.' id)? <types> '.' (new|id) '('` is an
    /// implicit creation. Returns the position of the `(`.
    fn implicit_creation_ahead(&self) -> Option<usize> {
        if self.kind_of(self.pos) != Kind::Ident {
            return None;
        }
        let mut i = self.pos + 1;
        if self.kind_of(i) == Kind::Op(Op::Dot) && self.kind_of(i + 1) == Kind::Ident {
            i += 2;
        }
        let after = self.skip_type_arguments(i)?;
        if self.kind_of(after) != Kind::Op(Op::Dot) {
            return None;
        }
        let name = after + 1;
        if !matches!(
            self.kind_of(name),
            Kind::Ident | Kind::Keyword(Keyword::New)
        ) {
            return None;
        }
        (self.kind_of(name + 1) == Kind::Op(Op::LParen)).then_some(name + 1)
    }

    /// Selector chain over `expr`: `.x`, `?.x`, `[i]`, `?[i]`, `(args)`,
    /// `<T>(args)`, `<T>` and `!`.
    /// With `cascade`, the selectors of a cascade section, whose `.`/`?.` read
    /// the name with `parseSend` (elsewhere, with `parsePrimary`).
    pub(crate) fn parse_selectors_at(
        &mut self,
        start: Span,
        mut expr: ExprId,
        constant_pattern: bool,
        cascade: bool,
    ) -> PResult<ExprId> {
        loop {
            match self.kind() {
                Kind::Op(Op::Dot) | Kind::Op(Op::QuestionDot) => {
                    let null_aware = self.at_op(Op::QuestionDot);
                    self.advance();
                    let name = self.member_name(!cascade)?;
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
                    let index = self.with_functions(|p| p.parse_expression())?;
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
                    let index = self.with_functions(|p| p.parse_expression())?;
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
                            // `parsePrecedenceExpression`: type arguments without `(` in a constant
                            // pattern.
                            let lt = self.tokens[lt_pos].span;
                            self.error_at(codes::parser::INVALID_CONSTANT_PATTERN_GENERIC, lt, &[]);
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

    /// The current `!` is followed by something that continues the selector
    /// chain (`x!.y`, `x![0]`, `f!()`, `x!?[0]`); otherwise it is a plain
    /// postfix.
    fn bang_is_selector(&self) -> bool {
        matches!(
            self.kind_at(1),
            Kind::Op(Op::Dot | Op::QuestionDot | Op::Question | Op::LParen | Op::LBracket)
        )
    }

    /// Name after `.`/`?.`/`..`: identifier or `new` (constructor tearoff).
    /// With `primary`, the name of a `.`/`?.` selector outside a cascade, which
    /// fasta reads with `parsePrimary` (`parsePrecedenceExpression`): a
    /// literal, `(…)`, `[…]`, `{…}` or `<…>` is read whole and
    /// `AstBuilder.doDotExpression` (`ast_builder.dart:851`) rejects it with
    /// `MISSING_IDENTIFIER` at the first token, which becomes the name.
    pub(crate) fn member_name(&mut self, primary: bool) -> PResult<Name> {
        if primary
            && (matches!(
                self.kind(),
                Kind::Int
                    | Kind::Double
                    | Kind::Str(_)
                    | Kind::StrBegin(..)
                    | Kind::Op(Op::Hash | Op::LParen | Op::LBracket | Op::LBrace | Op::Lt)
            ) || matches!(self.kind(), Kind::Keyword(Keyword::Const | Keyword::Switch)))
        {
            let token = self.peek();
            let text = self.text().to_string();
            self.parse_primary()?;
            self.error_at(codes::parser::MISSING_IDENTIFIER, token.span, &[]);
            return Ok(self.name_from(&text, token.span));
        }
        if self.at_identifier() {
            return Ok(self.identifier());
        }
        if self.at_kw(Keyword::New) {
            let token = self.advance();
            return Ok(self.name_from("new", token.span));
        }
        // `x.this`, `x.super`, `x.null`…: fasta reads the primary (`parsePrimary`)
        // and `AstBuilder.doDotExpression` (`ast_builder.dart:851`) rejects it as a
        // name with `MISSING_IDENTIFIER` at the token, which becomes the property
        // name.
        if matches!(
            self.kind(),
            Kind::Keyword(
                Keyword::This | Keyword::Super | Keyword::Null | Keyword::True | Keyword::False
            )
        ) {
            let text = self.text().to_string();
            let token = self.advance();
            self.error_at(codes::parser::MISSING_IDENTIFIER, token.span, &[]);
            return Ok(self.name_from(&text, token.span));
        }
        // Another token (`b.` before `}`, `)`, `;`, `=>`…): the
        // `IdentifierContext.expressionContinuation.ensureIdentifier`.
        Ok(self.recovery_name(true))
    }

    /// The current `<` opens a selector's type arguments: the list is well
    /// formed and the next token is one of those that may follow type arguments
    /// (the SDK's `_mayFollowTypeArgs` rule; the interpolation closer counts
    /// because the lexer merges it into the next string piece), and fasta did
    /// not recover inside it (`computeMethodTypeArguments`).
    fn method_type_arguments_ahead(&self) -> bool {
        let Some(end) = self.skip_type_arguments_unrecovered(self.pos) else {
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

    /// Does the current `?` (followed by `[`) start a conditional
    /// `? expression : expression`? Reads speculatively and undoes everything.
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
            splices: self.splices.len(),
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
        self.undo_splices(checkpoint.splices);
        self.pos = checkpoint.pos;
        self.ast.exprs.truncate(checkpoint.exprs);
        self.ast
            .assert_invocations
            .retain(|e| (e.0 as usize) < checkpoint.exprs);
        self.ast
            .implicit_creations
            .retain(|e| (e.0 as usize) < checkpoint.exprs);
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

    // -- Primaries ---------------------------------------------------------

    /// `primary`: literals, identifiers, `this`, `super`, collections, records,
    /// parentheses, `new`/`const`, function expressions and `switch`.
    fn parse_primary(&mut self) -> PResult<ExprId> {
        let start = self.span();
        match self.kind() {
            Kind::Ident => {
                if let Some(name) = self.named_function_ahead() {
                    return self.parse_named_function(name);
                }
                let name = self.identifier();
                Ok(self.push(start, ExprKind::Identifier(name)))
            }
            // `void name(…) {…}` in expression position.
            Kind::Keyword(Keyword::Void) if self.named_function_ahead().is_some() => {
                let name = self.named_function_ahead().expect("checked by the guard");
                self.parse_named_function(name)
            }
            Kind::Int => {
                self.check_separators(start);
                self.advance();
                Ok(self.push(start, ExprKind::Int(start)))
            }
            Kind::Double => {
                self.check_separators(start);
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
                    self.error_at(
                        codes::parser::INVALID_OPERATOR_QUESTIONMARK_PERIOD_FOR_SUPER,
                        span,
                        &[],
                    );
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
                // `(params) {…}` is a function only if the parameter list is valid:
                // `x = (v as T?) { … }` in a constructor initializer is a parenthesized
                // expression followed by the body.
                if !self.without_named_function
                    && self.function_expression_ahead(self.pos)
                    && self.speculate(|p| p.parse_formal_parameters().is_ok())
                {
                    return self.parse_function_expression(start);
                }
                self.parse_parenthesized_or_record(start, false, false)
            }
            Kind::Keyword(Keyword::New) => {
                let new_span = self.advance().span;
                // `parseNewExpression` (`parser_impl.dart:7270`): a literal with `new`
                // before it. `new List<T>[…]`, `new Map<K, V>{…}`, `new Set<T>{…}` are
                // `LITERAL_WITH_CLASS_AND_NEW` from `new` to the name; `new [..]`,
                // `new {..}`, `new <T>[..]`, `LITERAL_WITH_NEW` at the `new`; in both cases
                // the literal is read from what follows.
                if self.at_identifier() && !self.at_op_at(1, Op::Dot) {
                    let class = self.text();
                    let after = if self.at_op_at(1, Op::Lt) {
                        self.skip_type_arguments(self.pos + 1)
                    } else {
                        Some(self.pos + 1)
                    };
                    let literal = matches!(
                        (class, after.map(|d| self.kind_of(d))),
                        ("Map" | "Set", Some(Kind::Op(Op::LBrace)))
                            | ("List", Some(Kind::Op(Op::LBracket)))
                    );
                    if literal {
                        let name = self.advance().span;
                        let lowercase = class.to_lowercase();
                        let span = Span {
                            start: new_span.start,
                            end: name.end,
                        };
                        self.error_at(
                            codes::parser::LITERAL_WITH_CLASS_AND_NEW,
                            span,
                            &[&lowercase, class],
                        );
                        return self.parse_primary();
                    }
                } else if !self.at_identifier() {
                    let literal = match self.kind() {
                        Kind::Op(Op::LBrace | Op::LBracket) => true,
                        Kind::Op(Op::Lt) => self.skip_type_arguments(self.pos).is_some_and(|d| {
                            matches!(self.kind_of(d), Kind::Op(Op::LBrace | Op::LBracket))
                        }),
                        _ => false,
                    };
                    if literal {
                        self.error_at(codes::parser::LITERAL_WITH_NEW, new_span, &[]);
                        return self.parse_primary();
                    }
                }
                self.parse_instance_creation(start, Some(CreationKeyword::New))
            }
            // Dot shorthand (3.10): `.id`, `.new`.
            Kind::Op(Op::Dot) => self.parse_dot_shorthand(start, false),
            Kind::Keyword(Keyword::Const) => {
                self.advance();
                match self.kind() {
                    // `const .id(args)` / `const .new(args)`.
                    Kind::Op(Op::Dot) => self.parse_dot_shorthand(start, true),
                    Kind::Op(Op::LBracket) => self.parse_list_literal(start, true, Vec::new()),
                    Kind::Op(Op::LBrace) => self.parse_set_or_map_literal(start, true, Vec::new()),
                    Kind::Op(Op::LParen) => self.parse_parenthesized_or_record(start, true, false),
                    Kind::Op(Op::Lt) => {
                        let type_args = self.parse_type_arguments_opt()?;
                        self.parse_typed_collection(start, true, type_args)
                    }
                    _ => self.parse_instance_creation(start, Some(CreationKeyword::Const)),
                }
            }
            Kind::Keyword(Keyword::Switch) => self.parse_switch_expression(start),
            // `parsePrimary`/`parseAssert(Assert.Expression)`: `assert(…)` as an
            // expression; fasta's `AssertAsExpression` has no code in the analyzer, and
            // the `AstBuilder` builds the invocation of `assert`.
            Kind::Keyword(Keyword::Assert) if self.at_op_at(1, Op::LParen) => {
                let token = self.advance();
                let name = self.name_from("assert", token.span);
                let target_expr = self.ast.push_expr(Expr {
                    span: token.span,
                    kind: ExprKind::Identifier(name),
                });
                let arguments = self.parse_arguments()?;
                let call = self.push(
                    start,
                    ExprKind::Call {
                        target: target_expr,
                        arguments: Box::new(arguments),
                    },
                );
                self.ast.assert_invocations.push(call);
                Ok(call)
            }
            // `parsePrimary`: `return` in place of an expression is `UNEXPECTED_TOKEN`
            // and the following primary is read.
            Kind::Keyword(Keyword::Return) => {
                let token = self.advance();
                self.error_at(codes::parser::UNEXPECTED_TOKEN, token.span, &["return"]);
                self.parse_primary()
            }
            _ => Ok(self.recovery_identifier()),
        }
    }

    /// `ExpressionIdentifierContext.ensureIdentifier`
    /// (`identifier_context_impl.dart:324`) when the token is not an
    /// identifier: a reserved word that does not start a statement becomes the
    /// name (`EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD`); otherwise
    /// `MISSING_IDENTIFIER` and an empty synthetic identifier, after consuming
    /// the token when it is not an operator, group punctuation or the start of
    /// a statement.
    fn recovery_identifier(&mut self) -> ExprId {
        let name = self.recovery_name(false);
        self.ast.push_expr(Expr {
            span: name.span,
            kind: ExprKind::Identifier(name),
        })
    }

    /// The name from `ExpressionIdentifierContext.ensureIdentifier` over a token
    /// that is not an identifier; `continuation` is the `expressionContinuation`
    /// context (after `.`), in which `as` and `is` also become a name.
    fn recovery_name(&mut self, continuation: bool) -> Name {
        let starts_statement = self.looks_like_statement_start(self.pos);
        if !starts_statement
            && let Kind::Keyword(_) = self.kind()
            && (continuation || !matches!(self.text(), "as" | "is"))
        {
            let text = self.text().to_string();
            let token = self.advance();
            self.error_at(
                codes::parser::EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD,
                token.span,
                &[&text],
            );
            return self.name_from(&text, token.span);
        }
        // `!identifier.isOperator` and outside the list's punctuation: when in
        // doubt, the token is consumed (the synthetic identifier comes after it).
        // `StrMid`/`StrEnd` start with the `}` that closes the interpolation (the
        // `}` in fasta's list): they are not consumed.
        let consume = !starts_statement
            && !matches!(
                self.kind(),
                Kind::Ident | Kind::Keyword(_) | Kind::Eof | Kind::StrMid(..) | Kind::StrEnd(_)
            )
            && match self.kind() {
                Kind::Op(op) => matches!(
                    op,
                    Op::At | Op::Hash | Op::Arrow | Op::Ellipsis | Op::EllipsisQuestion
                ),
                _ => true,
            };
        self.error(codes::parser::MISSING_IDENTIFIER, &[]);
        if consume {
            self.advance();
        }
        // `insertSyntheticIdentifier`: at the start of the next token.
        let s = self.span().start;
        self.name_from("", Span { start: s, end: s })
    }

    /// The *head* of a dot shorthand, with the current `.` (Dart 3.10,
    /// `<staticMemberShorthandHead>`): `.id` or `.new`. The selectors come
    /// after, as with any primary. The `const` form requires arguments.
    fn parse_dot_shorthand(&mut self, start: Span, const_: bool) -> PResult<ExprId> {
        let dot = self.advance();
        let name = if self.at_kw(Keyword::New) {
            let token = self.advance();
            self.name_from("new", token.span)
        } else if self.at_identifier() {
            self.identifier()
        } else {
            return Err(self.identifier_error());
        };
        // The 3.13.4 analyzer reports at the `.` (`handleDotShorthandHead` and
        // `handleDotShorthandContext`) and, in `const .x(…)`, also at the `const`
        // (`endConstDotShorthand`).
        self.require(Feature::DotShorthands, dot.span);
        if const_ {
            self.require(Feature::DotShorthands, start);
        }
        // `const .x<T>(…)`: the type arguments come before the parentheses.
        if const_
            && !self.at_op(Op::LParen)
            && !(self.at_op(Op::Lt) && self.method_type_arguments_ahead())
        {
            return Err(self.expected_error("("));
        }
        Ok(self.push(start, ExprKind::DotShorthand { name, const_ }))
    }

    /// After explicit type arguments: `<T>[...]` or `<K, V>{...}`.
    fn parse_typed_collection(
        &mut self,
        start: Span,
        const_: bool,
        type_args: Vec<TypeId>,
    ) -> PResult<ExprId> {
        match self.kind() {
            Kind::Op(Op::LBracket) => self.parse_list_literal(start, const_, type_args),
            Kind::Op(Op::LBrace) => self.parse_set_or_map_literal(start, const_, type_args),
            _ => Err(self.expected_error("[")),
        }
    }

    /// `[ elements_ ]`.
    fn parse_list_literal(
        &mut self,
        start: Span,
        const_: bool,
        type_args: Vec<TypeId>,
    ) -> PResult<ExprId> {
        self.expect_op(Op::LBracket)?;
        let before = std::mem::replace(&mut self.in_list, true);
        let elements = self.parse_collection_elements(Op::RBracket);
        self.in_list = before;
        let elements = elements?;
        Ok(self.push(
            start,
            ExprKind::List {
                const_,
                type_args: type_args.into_boxed_slice(),
                elements: elements.into_boxed_slice(),
            },
        ))
    }

    /// `{ elements }` — a set or a map, decided later by the elements.
    fn parse_set_or_map_literal(
        &mut self,
        start: Span,
        const_: bool,
        type_args: Vec<TypeId>,
    ) -> PResult<ExprId> {
        self.expect_op(Op::LBrace)?;
        let before = std::mem::replace(&mut self.in_list, false);
        let elements = self.parse_collection_elements(Op::RBrace);
        self.in_list = before;
        let elements = elements?;
        Ok(self.push(
            start,
            ExprKind::SetOrMap {
                const_,
                type_args: type_args.into_boxed_slice(),
                elements: elements.into_boxed_slice(),
            },
        ))
    }

    /// Comma-separated elements (with an optional trailing comma) up to `close`,
    /// which is consumed.
    fn parse_collection_elements(&mut self, close: Op) -> PResult<Vec<CollectionElement>> {
        self.with_functions(|p| p.parse_collection_elements_inner(close))
    }

    fn parse_collection_elements_inner(&mut self, close: Op) -> PResult<Vec<CollectionElement>> {
        let opener = self.pos.saturating_sub(1);
        let mut elements = Vec::new();
        while !self.at_op(close) {
            let element = self.parse_collection_element()?;
            let ifs = open_ifs(&element);
            elements.push(element);
            if !self.eat_op(Op::Comma) {
                if self.at_op(close) {
                    break;
                }
                // `parseLiteralListSuffix`/`parseLiteralSetOrMapSuffix`: what looks like
                // another element is an element after a missing comma (`rewriteAndRecover`,
                // the error at the token).
                if self.looks_like_literal_entry(self.pos) {
                    if ifs > 0 {
                        self.error(codes::parser::EXPECTED_ELSE_OR_COMMA, &[]);
                    } else {
                        self.error(codes::parser::EXPECTED_TOKEN, &[","]);
                    }
                    continue;
                }
                // The list brings the synthetic `]` here (`moveSynthetic`).
                if close == Op::RBracket && self.move_synthetic_closer(opener) {
                    self.advance();
                    return Ok(elements);
                }
                break;
            }
        }
        // `parseLiteralListSuffix`/`parseLiteralSetOrMapSuffix`: leftovers before
        // the matching closer are `EXPECTED_TOKEN` (the closer) at their first
        // token, and the literal ends at the closer (`{0: throw "y"; }`).
        if !self.at_op(close)
            && !self.at_eof()
            && let Some(closes) = self.matching_close(opener)
            && closes > self.pos
            && self.kind_of(closes) == Kind::Op(close)
        {
            self.expected_error(close.text());
            self.pos = closes + 1;
            return Ok(elements);
        }
        self.expect_op(close)?;
        Ok(elements)
    }

    /// `looksLikeLiteralEntry` (`literal_entry_info.dart:75`).
    fn looks_like_literal_entry(&self, pos: usize) -> bool {
        self.looks_like_expression_start(pos)
            || matches!(
                self.kind_of(pos),
                Kind::Op(Op::Ellipsis | Op::EllipsisQuestion)
                    | Kind::Keyword(Keyword::If | Keyword::For)
            )
            || (self.text_of(pos) == "await"
                && self.kind_of(pos + 1) == Kind::Keyword(Keyword::For))
    }

    /// `element`: expression, `k: v`, `...e`, `...?e`, `if (...) e else e`,
    /// `await? for (...) e`, nested at will.
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
        // `?e` is a null-aware element (Dart 3.8); `?` never starts an expression,
        // so there is no ambiguity with the conditional (`NullAwareEntry`). With
        // the feature disabled, the `AstBuilder` reports once (at the key's `?`, or
        // at the value's) and drops the `?`: the element is a plain one
        // (`handleNullAwareElement`, `handleLiteralMapEntry`).
        let enabled_flag = self.features.has(Feature::NullAwareElements);
        let q_key = if self.at_op(Op::Question) {
            Some(self.advance().span)
        } else {
            None
        };
        let key = self.parse_expression()?;
        // In a list, only the null-aware entry reads `:`.
        if (q_key.is_some() || !self.in_list) && self.eat_op(Op::Colon) {
            let q_value = if self.at_op(Op::Question) {
                Some(self.advance().span)
            } else {
                None
            };
            if let Some(q) = q_key.or(q_value) {
                self.require(Feature::NullAwareElements, q);
            }
            let value = self.parse_expression()?;
            return Ok(CollectionElement::MapEntry {
                key,
                value,
                null_aware_key: enabled_flag && q_key.is_some(),
                null_aware_value: enabled_flag && q_value.is_some(),
            });
        }
        if let Some(q) = q_key {
            self.require(Feature::NullAwareElements, q);
        }
        Ok(if enabled_flag && q_key.is_some() {
            CollectionElement::NullAwareExpression(key)
        } else {
            CollectionElement::Expression(key)
        })
    }

    /// `(e)`, `(a, b)`, `(a,)`, `(name: e)`, `()`; `const_` forces a record.
    /// `from_pattern`: the `(` after a constant pattern's `const`, which fasta
    /// reads without `constKeywordForRecord` (`parsePrimaryPattern`): `(e)` is a
    /// parenthesized expression; a record comes out constant, like the context.
    pub(crate) fn parse_parenthesized_or_record(
        &mut self,
        start: Span,
        const_: bool,
        from_pattern: bool,
    ) -> PResult<ExprId> {
        self.with_functions(|p| p.parse_parenthesized_or_record_inner(start, const_, from_pattern))
    }

    fn parse_parenthesized_or_record_inner(
        &mut self,
        start: Span,
        const_: bool,
        from_pattern: bool,
    ) -> PResult<ExprId> {
        let opener = self.pos;
        self.expect_op(Op::LParen)?;
        let mut positional = Vec::new();
        let mut named = Vec::new();
        let mut is_record = const_ || self.at_op(Op::RParen);
        let mut saw_comma = false;
        // `(,)`: `EMPTY_RECORD_LITERAL_WITH_COMMA` at the comma
        // (`parseParenthesizedExpressionOrRecordLiteral`).
        if self.at_op(Op::Comma) && self.at_op_at(1, Op::RParen) {
            let comma = self.advance().span;
            self.error_at(codes::parser::EMPTY_RECORD_LITERAL_WITH_COMMA, comma, &[]);
            is_record = true;
            saw_comma = true;
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
                saw_comma = true;
            } else {
                break;
            }
        }
        self.ensure_close_paren(opener)?;
        // `const (e)`: one positional without a comma, at the `)`.
        if const_ && positional.len() == 1 && named.is_empty() && !saw_comma {
            let closes = self.tokens[self.pos - 1].span;
            self.error_at(
                codes::parser::RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA,
                closes,
                &[],
            );
        }
        if is_record {
            return Ok(self.push(
                start,
                ExprKind::Record {
                    const_: const_ || from_pattern,
                    positional: positional.into_boxed_slice(),
                    named: named.into_boxed_slice(),
                },
            ));
        }
        let inner = positional[0];
        Ok(self.push(start, ExprKind::Parenthesized(inner)))
    }

    /// Does a function body come after the `)` that closes the `(` at `open`?
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

    /// The current `<` opens the type parameters of a generic function
    /// expression `<T>(...) ...`: it closes the `<` and requires `(` with a body
    /// after it.
    fn generic_function_expression_ahead(&self) -> bool {
        let Some(close) = self.angle_close(self.pos) else {
            return false;
        };
        self.kind_of(close + 1) == Kind::Op(Op::LParen) && self.function_expression_ahead(close + 1)
    }

    /// `parseSendOrFunctionLiteral` (`parser_impl.dart:7188-7207` of fasta
    /// 3.6.2): in primary position, an optional type, an identifier, optional
    /// `<…>` and `(…)` followed by a body (`{`, `=>`, `async`, `sync`) are a
    /// named function literal. Returns the index of the name token. Does not
    /// apply where fasta turns off `mayParseFunctionExpressions`
    /// ([`Parser::without_named_function`]).
    fn named_function_ahead(&self) -> Option<usize> {
        if self.without_named_function {
            return None;
        }
        // The rest, from the name.
        let rest = |name: usize| -> bool {
            if self.kind_of(name) != Kind::Ident {
                return false;
            }
            let mut opener = name + 1;
            if self.kind_of(opener) == Kind::Op(Op::Lt) {
                match self.angle_close(opener) {
                    Some(closes) => opener = closes + 1,
                    None => return false,
                }
            }
            self.kind_of(opener) == Kind::Op(Op::LParen) && self.function_expression_ahead(opener)
        };
        // Without a type: `g() {}`.
        if self.kind() == Kind::Ident && rest(self.pos) {
            return Some(self.pos);
        }
        // With a type: `void g() {}`, `int g<T>() => 1`. An identifier only starts
        // a type followed by a name if it is followed by another identifier, `<`,
        // `.` or `?` (the filter spares the `skip_type` on almost every primary).
        if self.kind() == Kind::Ident
            && !matches!(
                self.kind_of(self.pos + 1),
                Kind::Ident | Kind::Op(Op::Lt | Op::Dot | Op::Question)
            )
        {
            return None;
        }
        let after = self.skip_type(self.pos)?;
        (after > self.pos && rest(after)).then_some(after)
    }

    /// The named function literal: `NAMED_FUNCTION_EXPRESSION` at the name
    /// (`parseNamedFunctionRest` with `isFunctionExpression`,
    /// `parser_impl.dart:5286-5289`), and the function expression without it,
    /// as the `AstBuilder` builds it. The written return type is skipped.
    fn parse_named_function(&mut self, name: usize) -> PResult<ExprId> {
        while self.pos < name {
            self.advance();
        }
        let span = self.span();
        self.error_at(codes::parser::NAMED_FUNCTION_EXPRESSION, span, &[]);
        self.advance();
        let start_offset = self.span();
        self.parse_function_expression(start_offset)
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

    /// After `new`/`const`: `T`, `p.T`, `T<A>`, `p.T<A>.n`, `T.n` (a two-part
    /// name, like the SDK) and the required arguments.
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
            Some(self.member_name(false)?)
        } else {
            None
        };
        // `parseConstructorInvocationArguments` (`parser_impl.dart:7243`): without
        // `(`, type arguments after the name are `CONSTRUCTOR_WITH_TYPE_ARGUMENTS`
        // at the name's last token (and are read); otherwise `EXPECTED_TOKEN` (`(`)
        // at it. Without `(` after that, the argument list is synthetic and empty,
        // and the expression goes on.
        if !self.at_op(Op::LParen) && self.pos > 0 {
            let last = self.tokens[self.pos - 1].span;
            if self.at_op(Op::Lt) && self.skip_type_arguments(self.pos).is_some() {
                self.error_at(codes::parser::CONSTRUCTOR_WITH_TYPE_ARGUMENTS, last, &[]);
                self.parse_type_arguments_opt()?;
            } else {
                self.error_at(codes::parser::EXPECTED_TOKEN, last, &["("]);
            }
            if !self.at_op(Op::LParen) {
                let empty = Span {
                    start: last.end,
                    end: last.end,
                };
                let arguments = Box::new(Arguments {
                    span: empty,
                    type_args: Box::default(),
                    args: Box::default(),
                });
                return Ok(self.push(
                    start,
                    ExprKind::InstanceCreation {
                        keyword,
                        ty,
                        constructor,
                        arguments,
                    },
                ));
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
        let opener = self.pos;
        // `ensureBlock(…, switchExpression)`: without `{`, the error at the `)` and
        // a synthetic empty body.
        if !self.at_op(Op::LBrace) {
            let closes = self.tokens[self.pos - 1].span;
            self.error_at(codes::parser::EXPECTED_SWITCH_EXPRESSION_BODY, closes, &[]);
            return Ok(self.push(
                start,
                ExprKind::Switch {
                    value,
                    cases: Vec::new().into_boxed_slice(),
                },
            ));
        }
        self.expect_op(Op::LBrace)?;
        let mut cases = Vec::new();
        while !self.at_op(Op::RBrace) {
            let case_start = self.span();
            // `parseSwitchExpression` (`parser_impl.dart`): statement syntax inside the
            // expression. `default` is `DEFAULT_IN_SWITCH_EXPRESSION` and counts as
            // `_`; `case` is `UNEXPECTED_TOKEN` and is skipped; `:` in place of `=>` and
            // `;` in place of `,` are `EXPECTED_TOKEN` at them.
            let pattern = if self.at_kw(Keyword::Default) {
                let t = self.advance().span;
                self.error_at(codes::parser::DEFAULT_IN_SWITCH_EXPRESSION, t, &[]);
                self.ast.push_pattern(crate::ast::Pattern {
                    span: t,
                    kind: crate::ast::PatternKind::Wildcard { ty: None },
                })
            } else {
                if self.at_kw(Keyword::Case) {
                    self.error(codes::parser::UNEXPECTED_TOKEN, &["case"]);
                    self.advance();
                }
                // The pattern and guard of a `switch` expression case do not read a named
                // function literal (the `=>` is the case's).
                let saved = std::mem::replace(&mut self.without_named_function, true);
                let case_pattern = self.parse_pattern();
                self.without_named_function = saved;
                case_pattern?
            };
            let guard = if self.eat_ident("when") {
                let saved = std::mem::replace(&mut self.without_named_function, true);
                let guard_expr = self.parse_expression();
                self.without_named_function = saved;
                Some(guard_expr?)
            } else {
                None
            };
            if self.at_op(Op::Colon) {
                self.error(codes::parser::EXPECTED_TOKEN, &["=>"]);
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
            let mut comma = self.eat_op(Op::Comma);
            if !comma && self.at_op(Op::Semicolon) {
                self.error(codes::parser::EXPECTED_TOKEN, &[","]);
                self.advance();
                comma = true;
            }
            if self.at_op(Op::RBrace) || comma {
                continue;
            }
            // Without a comma: what looks like another pattern is the next case (the
            // missing comma is reported at it); otherwise skips to the next `,`/`;`
            // before the matching `}`, or ends at the `}`.
            if self.looks_like_pattern_start(self.pos) {
                self.error(codes::parser::EXPECTED_TOKEN, &[","]);
                continue;
            }
            let Some(closes) = self.matching_close(opener) else {
                break;
            };
            let mut i = self.pos;
            let mut found_it = None;
            while i < closes {
                match self.kind_of(i) {
                    Kind::Op(Op::Comma | Op::Semicolon) => {
                        found_it = Some(i);
                        break;
                    }
                    Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => {
                        i = self.matching_close(i).unwrap_or(closes);
                    }
                    _ => {}
                }
                i += 1;
            }
            match found_it {
                Some(v) => {
                    self.error(codes::parser::EXPECTED_TOKEN, &[","]);
                    self.pos = v + 1;
                }
                None => {
                    self.error(codes::parser::EXPECTED_TOKEN, &["}"]);
                    self.pos = closes;
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

    /// `#name`, `#a.b.c`, `#void`, `#+`, `#[]=`, `#>>>`…
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
                    return Err(self.identifier_error());
                };
                let text = match composed {
                    ComposedGt::Gt => ">",
                    ComposedGt::GtEq => ">=",
                    ComposedGt::Shr => ">>",
                    ComposedGt::UShr => ">>>",
                    ComposedGt::ShrAssign | ComposedGt::UShrAssign => {
                        return Err(self.identifier_error());
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
            _ => return Err(self.identifier_error()),
        }
        Ok(self.push(start, ExprKind::Symbol(names.into_boxed_slice())))
    }

    // -- Strings -------------------------------------------------------------

    /// Reads the interpolations and pieces that follow a `StrBegin` up to the
    /// `StrEnd`.
    fn parse_interpolations(&mut self, first: Interp) -> PResult<()> {
        let mut previous = first;
        loop {
            let expr = match previous {
                // `$` without a name (the lexer goes on with the next piece):
                // `MISSING_IDENTIFIER` at the first character after the `$`, with length 1,
                // and the interpolation of the scanner's synthetic identifier, empty, at
                // that position (`"$"` in a constant is `INVALID_CONSTANT` there).
                Interp::Ident if !self.at_identifier() => {
                    let s = self.span().start;
                    self.error_at(
                        codes::scanner::MISSING_IDENTIFIER,
                        Span {
                            start: s,
                            end: s + 1,
                        },
                        &[],
                    );
                    let at = Span { start: s, end: s };
                    let name = self.name_from("", at);
                    Some(self.ast.push_expr(Expr {
                        span: at,
                        kind: ExprKind::Identifier(name),
                    }))
                }
                Interp::Ident => {
                    let start = self.span();
                    // `parseIdentifierExpression` (`parser_impl.dart:7584-7593`): except for
                    // `this`, the `ensureIdentifier` of the `expression` context
                    // (`identifier_context_impl.dart:346-354`) reports the reserved word after
                    // the `$` (`"$class"`) and uses it as the identifier.
                    let text = self.text().to_string();
                    if text != "this"
                        && matches!(
                            super::fasta::word_style(&text),
                            Some(super::fasta::WordStyle::Reserved)
                        )
                    {
                        self.error(codes::parser::EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD, &[&text]);
                    }
                    let name = self.identifier();
                    Some(self.push(start, ExprKind::Identifier(name)))
                }
                // `parseLiteralString` (`parser_impl.dart:7500-7514`) turns function
                // literals back on: `x = "${() => y}"` in a constructor initializer.
                Interp::Brace => Some(self.with_functions(|p| p.parse_expression())?),
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
                _ => return Err(self.expected_error("}")),
            }
        }
    }

    /// Decodes the text of a string piece and appends it to the scratch space.
    ///
    /// `first` marks the piece that starts at the opening quote (it gets the
    /// `r` and the removal of the first line break in triple strings);
    /// `leading_brace` marks a piece that starts at an interpolation's `}`.
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
            // Without the closer (`StrFlags::unterminated`): the content runs to the end.
            Kind::Str(f) | Kind::StrEnd(f) if f.unterminated => 0,
            Kind::Str(_) | Kind::StrEnd(_) => quote_len,
            Kind::StrBegin(_, Interp::Brace) | Kind::StrMid(_, Interp::Brace) => 2,
            _ => 1,
        };
        let content = text
            .get(head..text.len().saturating_sub(tail))
            .unwrap_or("");
        // In a triple-quoted string the source's line breaks (CR LF and CR) count
        // as LF (the `multiline_newline_test`: the same constant in CR, CRLF and LF
        // files); a `\r` written as an escape stays.
        let normalized: std::borrow::Cow<'_, str> = if flags.triple && content.contains('\r') {
            std::borrow::Cow::Owned(content.replace("\r\n", "\n").replace('\r', "\n"))
        } else {
            std::borrow::Cow::Borrowed(content)
        };
        let content: &str = &normalized;
        match crate::lexer::decode_string(content, flags.raw, first && flags.triple) {
            Ok(decoded) => {
                self.scratch_parts.push(StringPart::Text(decoded));
                Ok(())
            }
            Err(message) => {
                let _ = message;
                // `unescapeCodeUnits` (`quote.dart:186`): the invalid escape is reported
                // with fasta's code and excerpt, and the string goes on with the raw text
                // (the error is recoverable).
                let base = token.span.start + head;
                match escape_error(content.as_bytes()) {
                    Some((code, start_offset, len)) => {
                        let span = Span {
                            start: base + start_offset,
                            end: base + start_offset + len,
                        };
                        // `INVALID_CODE_POINT` carries the literal text `\u{...}`.
                        let args: &[&str] = if code == codes::parser::INVALID_CODE_POINT {
                            &["\\u{...}"]
                        } else {
                            &[]
                        };
                        self.error_at(code, span, args);
                        self.scratch_parts
                            .push(StringPart::Text(crate::text::DartStr::from(content)));
                        Ok(())
                    }
                    None => Err(self.error_at(
                        codes::parser::INVALID_UNICODE_ESCAPE_STARTED,
                        token.span,
                        &[],
                    )),
                }
            }
        }
    }

    // -- Utilidades ---------------------------------------------------------

    /// Appends an expression whose span runs from `start` to the last consumed
    /// token.
    pub(crate) fn push(&mut self, start: Span, kind: ExprKind) -> ExprId {
        let span = self.span_from(start);
        self.ast.push_expr(Expr { span, kind })
    }
}

/// The sum of the `ifConditionDelta` of an element's entries: `+1` per
/// `if`, `-1` per `else`.
fn open_ifs(e: &CollectionElement) -> i32 {
    match e {
        CollectionElement::If { then, else_, .. } => {
            1 + open_ifs(then) + else_.as_ref().map_or(0, |e| open_ifs(e) - 1)
        }
        CollectionElement::For { body, .. } => open_ifs(body),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{
        AssignOp, BinaryOp, CollectionElement, CreationKeyword, ExprId, ExprKind, StringPart,
        UnaryOp,
    };
    use crate::parser::Parser;

    /// Builds the parser and reads a whole expression, requiring everything to
    /// be consumed.
    fn parse(src: &str) -> (Parser<'_, 'static>, ExprId) {
        parse_with(src, false)
    }

    fn parse_with(src: &str, in_async: bool) -> (Parser<'_, 'static>, ExprId) {
        let names: &'static mut dartforge_intern::Interner =
            Box::leak(Box::new(dartforge_intern::Interner::new()));
        let tokens = crate::lexer::lex(src).unwrap();
        let mut p = Parser::new(src, tokens, names);
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
            other => panic!("expected an identifier, got {other:?}"),
        }
    }

    fn int_text<'a>(p: &'a Parser<'_, '_>, id: ExprId) -> &'a str {
        match kind(p, id) {
            ExprKind::Int(span) => &p.source[span.start..span.end],
            other => panic!("expected an integer, got {other:?}"),
        }
    }

    fn binary(p: &Parser<'_, '_>, id: ExprId) -> (BinaryOp, ExprId, ExprId) {
        match kind(p, id) {
            ExprKind::Binary { op, left, right } => (*op, *left, *right),
            other => panic!("expected a binary, got {other:?}"),
        }
    }

    #[test]
    fn recovered_record_type_is_not_a_type_argument() {
        // `computeMethodTypeArguments` drops the `recovered` list: `(x)` is a
        // record with one positional field and no trailing comma, so `<` and
        // `>` are comparisons and the call has two arguments.
        for src in ["f(x < (x), C > (x))", "f(x < (x), C >= (x))"] {
            let (p, id) = parse(src);
            let ExprKind::Call { arguments, .. } = kind(&p, id) else {
                panic!("{src}: expected a call, got {:?}", kind(&p, id));
            };
            assert_eq!(arguments.args.len(), 2, "{src}");
            for a in arguments.args.iter() {
                assert!(
                    matches!(kind(&p, a.value), ExprKind::Binary { .. }),
                    "{src}"
                );
            }
        }
        // With the trailing comma the record is a type: a generic call.
        let (p, id) = parse("f(x<(int,), C>(x))");
        let ExprKind::Call { arguments, .. } = kind(&p, id) else {
            panic!("expected a call");
        };
        assert_eq!(arguments.args.len(), 1);
    }

    #[test]
    fn interpolation_allows_function_literals_in_initializers() {
        // `parseLiteralString` turns `mayParseFunctionExpressions` back on:
        // `x = "${() => y}"` in a constructor initializer.
        let src = r#""**${() => y}--${() { return 2; }()}""#;
        let names: &'static mut dartforge_intern::Interner =
            Box::leak(Box::new(dartforge_intern::Interner::new()));
        let tokens = crate::lexer::lex(src).unwrap();
        let mut p = Parser::new(src, tokens, names);
        p.without_named_function = true;
        let id = p.parse_expression().unwrap();
        assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
        assert!(p.at_eof());
        let ExprKind::String(lit) = kind(&p, id) else {
            panic!("expected a string");
        };
        let interpolations = lit
            .parts
            .iter()
            .filter(|x| matches!(x, StringPart::Interpolation(_)))
            .count();
        assert_eq!(interpolations, 2);
        assert!(p.without_named_function);
    }

    #[test]
    fn arithmetic_precedence() {
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
    fn left_associativity() {
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
    fn if_null_and_logical() {
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
    fn shifts_and_comparisons_with_isolated_greater() {
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
        // `a >> b > c` → `(a >> b) > c`: shift above relational.
        let (p, id) = parse("a >> b > c");
        let (op, l, _) = binary(&p, id);
        assert_eq!(op, BinaryOp::Gt);
        assert_eq!(binary(&p, l).0, BinaryOp::Shr);
    }

    #[test]
    fn unglued_greater_is_not_shift() {
        let names: &'static mut dartforge_intern::Interner =
            Box::leak(Box::new(dartforge_intern::Interner::new()));
        let src = "a > > b";
        let tokens = crate::lexer::lex(src).unwrap();
        let mut p = Parser::new(src, tokens, names);
        // The second `>` has no left operand: `MISSING_IDENTIFIER` and a synthetic
        // identifier, not `>>`.
        let id = p.parse_expression().unwrap();
        assert!(!p.diagnostics.is_empty());
        assert_eq!(binary(&p, id).0, BinaryOp::Gt);
    }

    #[test]
    fn right_conditional() {
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
    fn right_assignment_and_compound() {
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
    fn prefix_and_postfix_unaries() {
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
        // `-x.y` is `-(x.y)`: the operand is the whole postfix.
        let (p, id) = parse("-x.y");
        let ExprKind::Unary { operand, .. } = kind(&p, id) else {
            panic!()
        };
        assert!(matches!(kind(&p, *operand), ExprKind::Property { .. }));
    }

    #[test]
    fn null_aware_selectors_and_asserts() {
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
    fn null_aware_index_versus_conditional() {
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
        assert!(
            p.ast.exprs.len() < 12,
            "speculation left nodes in the arena"
        );
    }

    #[test]
    fn calls_and_methods() {
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
    fn super_and_this_as_targets() {
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
    fn numeric_boolean_and_null_literals() {
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
    fn simple_adjacent_and_raw_strings() {
        let (p, id) = parse(r#"'a\n' "b" r'\d+'"#);
        let ExprKind::String(lit) = kind(&p, id) else {
            panic!()
        };
        assert_eq!(lit.constant_value().unwrap(), "a\nb\\d+");
        assert_eq!(lit.parts.len(), 3);
    }

    #[test]
    fn triple_strings_drop_first_newline() {
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
    fn interpolation_by_name_and_by_braces() {
        let (p, id) = parse(r#"'${a}b$c d${ {1}.length }e'"#);
        let ExprKind::String(lit) = kind(&p, id) else {
            panic!()
        };
        let texts: Vec<Option<&str>> = lit
            .parts
            .iter()
            .map(|part| match part {
                StringPart::Text(t) => t.as_str(),
                StringPart::Interpolation(_) => None,
            })
            .collect();
        assert_eq!(
            texts,
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
    fn nested_interpolation() {
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
    fn badly_closed_interpolation_is_error() {
        let names: &'static mut dartforge_intern::Interner =
            Box::leak(Box::new(dartforge_intern::Interner::new()));
        let src = "'${a b}'";
        let tokens = crate::lexer::lex(src).unwrap();
        let mut p = Parser::new(src, tokens, names);
        assert!(p.parse_expression().is_err());
        assert!(
            p.diagnostics
                .iter()
                .any(|d| d.code.is_some_and(|c| c.info().name == "expected_token"))
        );
    }

    #[test]
    fn symbols() {
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
    fn lists_with_control_elements() {
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
    fn maps_and_sets() {
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
    fn records_and_parens() {
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
    fn cascades() {
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

        // `a..b = c..d()`: the section's assignment ends before the `..`.
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
    fn throw_and_rethrow() {
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
    fn await_only_in_async() {
        let (p, id) = parse_with("await g()", true);
        assert!(matches!(kind(&p, id), ExprKind::Await(_)));
        let (p, id) = parse_with("await + 1", false);
        let (_, l, _) = binary(&p, id);
        assert_eq!(ident_text(&p, l), "await");
    }

    #[test]
    fn builtin_types_are_identifiers() {
        let (p, id) = parse("dynamic");
        assert_eq!(ident_text(&p, id), "dynamic");
        let (p, id) = parse("async");
        assert_eq!(ident_text(&p, id), "async");
    }

    #[test]
    fn spans_cover_the_expression() {
        let (p, id) = parse("a + b * c");
        assert_eq!(p.ast.expr(id).span.start, 0);
        assert_eq!(p.ast.expr(id).span.end, 9);
    }

    #[test]
    fn error_on_empty_expression() {
        let names: &'static mut dartforge_intern::Interner =
            Box::leak(Box::new(dartforge_intern::Interner::new()));
        for src in [")", "", "a +", "class"] {
            let tokens = crate::lexer::lex(src).unwrap();
            let mut p = Parser::new(src, tokens, names);
            // `ensureIdentifier`: the error comes out and the expression goes on with a
            // synthetic identifier (or with the reserved word as the name).
            let _ = p.parse_expression();
            assert!(!p.diagnostics.is_empty(), "{src}");
        }
    }

    #[test]
    fn depth_limited() {
        let names: &'static mut dartforge_intern::Interner =
            Box::leak(Box::new(dartforge_intern::Interner::new()));
        let src = format!("{}1{}", "(".repeat(2000), ")".repeat(2000));
        let tokens = crate::lexer::lex(&src).unwrap();
        let mut p = Parser::new(&src, tokens, names);
        assert!(p.parse_expression().is_err());
        assert!(
            p.diagnostics
                .iter()
                .any(|d| d.code.is_some_and(|c| c.info().name == "stack_overflow"))
        );
    }

    // -- Tests that depend on other modules (types, patterns, statements,
    // declarations). They panic with `not yet implemented` while the stubs
    // exist; they stay here to apply as soon as the modules arrive.

    #[test]
    fn generic_call_and_explicit_instantiation() {
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
        // `a < b > c` reads as `(a < b) > c`, with fasta's error for chained
        // relationals at the second operator.
        let src = "a < b > c";
        let names: &'static mut dartforge_intern::Interner =
            Box::leak(Box::new(dartforge_intern::Interner::new()));
        let mut p = Parser::new(src, crate::lexer::lex(src).unwrap(), names);
        let id = p.parse_expression().unwrap();
        assert_eq!(binary(&p, id).0, BinaryOp::Gt);
        assert_eq!(p.diagnostics.len(), 1);
        assert_eq!(
            p.diagnostics[0].code,
            Some(dartforge_diagnostics::codes::parser::EQUALITY_CANNOT_BE_EQUALITY_OPERAND)
        );
        let (p, id) = parse("a < b && c > d");
        let (op, l, r) = binary(&p, id);
        assert_eq!(op, BinaryOp::And);
        assert_eq!(binary(&p, l).0, BinaryOp::Lt);
        assert_eq!(binary(&p, r).0, BinaryOp::Gt);
    }

    #[test]
    fn is_and_as() {
        let (p, id) = parse("x is! int");
        assert!(matches!(kind(&p, id), ExprKind::Is { negated: true, .. }));
        let (p, id) = parse("y as int?");
        assert!(matches!(kind(&p, id), ExprKind::As { .. }));
        let (p, id) = parse("x is int && y");
        assert_eq!(binary(&p, id).0, BinaryOp::And);
    }

    #[test]
    fn instance_creation() {
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
        assert!(constructor.is_none(), "A.b stays as a two-part name");
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
    fn function_expressions() {
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
    fn switch_as_expression() {
        let (p, id) = parse("switch (x) { 1 => 'a', _ when y => 'b', _ => 'c', }");
        let ExprKind::Switch { cases, .. } = kind(&p, id) else {
            panic!()
        };
        assert_eq!(cases.len(), 3);
        assert!(cases[1].guard.is_some());
    }

    #[test]
    fn for_in_collection_and_if_case() {
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
    fn pattern_assignment() {
        let (p, id) = parse("(a, b) = e");
        assert!(matches!(kind(&p, id), ExprKind::PatternAssign { .. }));
        let (p, id) = parse("[x, y] = e");
        assert!(matches!(kind(&p, id), ExprKind::PatternAssign { .. }));
    }

    /// Corpus regressions: the `?` after `is T`/`as T` belongs to the
    /// conditional when what follows starts an expression, and to the type when
    /// it does not.
    #[test]
    fn is_with_conditional_and_nullable_type() {
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

    /// `.5` is a `double` literal (grammar: `'.' DIGIT+`).
    #[test]
    fn double_starting_with_dot() {
        let (p, id) = parse("log(n - .5)");
        let ExprKind::Call { arguments, .. } = kind(&p, id) else {
            panic!()
        };
        let ExprKind::Binary { right, .. } = kind(&p, arguments.args[0].value) else {
            panic!()
        };
        assert!(matches!(kind(&p, *right), ExprKind::Double(_)));
    }

    /// Null-aware elements (Dart 3.8): `?e`, `?k: v`, `k: ?v`.
    #[test]
    fn null_aware_elements() {
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

    /// Surrogates: an escape pair becomes the scalar; a lone one is kept as a
    /// UTF-16 unit, as Dart measures it.
    #[test]
    fn strings_with_surrogates() {
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
        // Adjacent literals concatenate by unit: the pair forms.
        let (p, id) = parse(r"'\uD83D' '\uDC6D'");
        let ExprKind::String(lit) = kind(&p, id) else {
            panic!()
        };
        assert_eq!(lit.constant_value().unwrap().as_str(), Some("\u{1F46D}"));
    }
}
