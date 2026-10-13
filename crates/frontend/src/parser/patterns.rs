//! Dart 3 patterns: `switch`, `if case`, declarations and assignments.
//!
//! Grammar covered (Dart 3.6): `pattern` = `logicalOrPattern`, with
//! `logicalAndPattern`, `relationalPattern`, `unaryPattern` (`p?`, `p!`,
//! `p as T`) and `primaryPattern` (constant, variable, `_`, parentheses,
//! list, map, record and object).
//!
//! Decisions resolution needs to know about:
//!
//! * A bare identifier (not followed by `(`, `<` or `.`) **always** becomes
//!   a [`PatternKind::Variable`] without `var`/`final`/type. In a declaration
//!   or assignment context that is a variable; in a matching context
//!   (`case x:`) the language reads it as a reference to a constant. The
//!   parser does not know which context it is in (the same `case` serves
//!   `switch` and `if case`), so the next phase decides by context.
//! * `type identifier` (`int x`, `List<int> xs`, `a.B c`, `(int, int) p`) is
//!   a typed variable; `int _` is a typed [`PatternKind::Wildcard`]. As in
//!   the SDK, `TYPE as`/`TYPE when` is **not** a variable named `as`/`when`:
//!   `Foo as Bar` is the pattern `Foo` cast to `Bar`, and `x when g` ends the
//!   pattern at `x`.
//! * `Name(...)`, `p.Name(...)`, `Name<T>(...)` is an object; the type is
//!   built here (it never has `?`). `:x` in a record/object field infers the
//!   name from the inner variable pattern, going through `?`, `!`, `as` and
//!   parentheses.
//! * Constants (literals, `-1`, `const [...]`, `a.b.c`) are read as in
//!   fasta's `parsePrimaryPattern`: the unary expression and, to recover,
//!   the binary operators up to equality, with the `ConstantPatternContext`'s
//!   `INVALID_CONSTANT_*` errors.
//! * `(p)` without a comma or name is a parenthesized pattern; `(p,)`, `()`
//!   and `(a: p)` are records. `(int, int) x` is a variable with a record
//!   type.
//! * `<T>[...]`/`<K, V>{...}` take type arguments; a `<` that does not open a
//!   valid type list followed by `[`/`{` is the relational `< e`.
//! * `as` is not a reserved word (`at_ident("as")`); `>=` arrives as `>`
//!   glued to `=` and is read by `composed_gt`.
use super::{PResult, Parser};
use crate::ast::{
    BinaryOp, ExprId, ExprKind, ListPatternElement, MapPatternEntry, Name, Pattern, PatternField,
    PatternId, PatternKind, TypeAnnotation, TypeKind, UnaryOp,
};
use crate::token::{Keyword, Kind, Op};
use dartforge_diagnostics::{Span, codes};

/// Where the `a.b.c` chain of a `const` pattern ends
/// ([`Parser::send_chain`]).
enum SendKind {
    Invocation,
    Error(usize),
    Open,
}

/// fasta's `PatternContext`: where the pattern appears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PatternContext {
    /// `var (a, b) = e;`, `for (final (a, b) in e)`.
    Declaration,
    /// `case p`, `if (e case p)`, `switch` as an expression.
    Matching,
    /// `(a, b) = e`.
    Assignment,
}

/// `ConstantPatternContext` do fasta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConstContext {
    /// Outside a constant pattern.
    NoneFound,
    /// Constant pattern without `const`.
    Implicit,
    /// Constant pattern with `const`.
    Explicit,
    /// Operand of `-` in a constant pattern: numeric literals only.
    NumericOnly,
}

impl<'s, 'i> Parser<'s, 'i> {
    /// An outermost pattern in context `ctx`.
    pub(crate) fn parse_pattern_in(&mut self, ctx: PatternContext) -> PResult<PatternId> {
        let salvo = self.default_context;
        self.default_context = ctx;
        let r = self.parse_pattern();
        self.default_context = salvo;
        r
    }

    /// The errors `parseVariablePattern` (`parser_impl.dart:9908`) reports by
    /// context: `var`/`final` in a declaration is
    /// `VARIABLE_PATTERN_KEYWORD_IN_DECLARATION_CONTEXT`; `var` with a type in
    /// matching, `VAR_AND_TYPE`; a declared variable (not a bare identifier) in
    /// an assignment, `PATTERN_ASSIGNMENT_DECLARES_VARIABLE` at the name; the
    /// name `when`/`as`, `ILLEGAL_PATTERN_VARIABLE_NAME` (or
    /// `ILLEGAL_PATTERN_ASSIGNMENT_VARIABLE_NAME`, bare in an assignment).
    fn check_variable_pattern(&mut self, word: Option<Span>, var_: bool, typed: bool, nome: Name) {
        let stray = word.is_none() && !typed;
        // Bare in a refutable context it is a constant pattern
        // (`parsePrimaryPattern`): `when`/`as` there is
        // `ILLEGAL_PATTERN_IDENTIFIER_NAME`.
        if stray && self.default_context == PatternContext::Matching {
            let texto = &self.source[nome.span.start..nome.span.end];
            if matches!(texto, "when" | "as") {
                let texto = texto.to_string();
                self.error_at(codes::parser::ILLEGAL_PATTERN_IDENTIFIER_NAME, nome.span, &[&texto]);
            }
            return;
        }
        match self.default_context {
            PatternContext::Declaration => {
                if let Some(p) = word {
                    self.error_at(codes::parser::VARIABLE_PATTERN_KEYWORD_IN_DECLARATION_CONTEXT, p, &[]);
                }
            }
            PatternContext::Matching => {
                if let (Some(p), true, true) = (word, var_, typed) {
                    self.error_at(codes::parser::VAR_AND_TYPE, p, &[]);
                }
            }
            PatternContext::Assignment => {
                if !stray {
                    let texto = self.source[nome.span.start..nome.span.end].to_string();
                    let texto = if texto.is_empty() { "(unnamed)".to_string() } else { texto };
                    self.error_at(codes::parser::PATTERN_ASSIGNMENT_DECLARES_VARIABLE, nome.span, &[&texto]);
                }
            }
        }
        let texto = &self.source[nome.span.start..nome.span.end];
        if matches!(texto, "when" | "as") {
            let texto = texto.to_string();
            let code = if self.default_context == PatternContext::Assignment && stray {
                codes::parser::ILLEGAL_PATTERN_ASSIGNMENT_VARIABLE_NAME
            } else {
                codes::parser::ILLEGAL_PATTERN_VARIABLE_NAME
            };
            self.error_at(code, nome.span, &[&texto]);
        }
    }

    /// Full `pattern` (with `||`, `&&`, `as`, `?`, `!`).
    pub(crate) fn parse_pattern(&mut self) -> PResult<PatternId> {
        self.enter()?;
        let result = self.parse_or_pattern();
        self.leave();
        result
    }

    /// Does a pattern declaration `var (…) =`, `final [`, `var {`, `final <T>[`
    /// start at `pos`? For `(` it requires `=` or `in` after the matching `)`,
    /// because `final (int, int) r = …` is a record type.
    pub(crate) fn looks_like_pattern_declaration(&self, pos: usize) -> bool {
        if !matches!(
            self.kind_of(pos),
            Kind::Keyword(Keyword::Var | Keyword::Final)
        ) {
            return false;
        }
        let mut i = pos + 1;
        if self.kind_of(i) == Kind::Op(Op::Lt) {
            match self.skip_type_arguments(i) {
                Some(after) => i = after,
                None => return false,
            }
        }
        match self.kind_of(i) {
            Kind::Op(Op::LBracket | Op::LBrace) => true,
            Kind::Op(Op::LParen) if i == pos + 1 => self.paren_group_then_assign_or_in(i),
            // Object pattern: `final MapEntry(key: k, value: v) in …`,
            // `final p.Name<T>(…) = …`. A regular declaration never has `(` right after
            // the type name.
            Kind::Ident if i == pos + 1 => {
                let mut j = i + 1;
                if self.kind_of(j) == Kind::Op(Op::Dot) && self.kind_of(j + 1) == Kind::Ident {
                    j += 2;
                }
                if self.kind_of(j) == Kind::Op(Op::Lt) {
                    match self.skip_type_arguments(j) {
                        Some(after) => j = after,
                        None => return false,
                    }
                }
                self.kind_of(j) == Kind::Op(Op::LParen) && self.paren_group_then_assign_or_in(j)
            }
            _ => false,
        }
    }

    /// The `(...)` group at `pos` closes and is followed by `=` or `in`.
    fn paren_group_then_assign_or_in(&self, pos: usize) -> bool {
        match self.matching_close(pos) {
            Some(close) => matches!(
                self.kind_of(close + 1),
                Kind::Op(Op::Assign) | Kind::Keyword(Keyword::In)
            ),
            None => false,
        }
    }

    /// Does a pattern assignment start at `pos`: an `outerPattern` — `(a, b)`,
    /// `[x]`, `{'k': v}`, `<T>[…]`, `Name(…)`, `p.Name<T>(…)` — whose closer is
    /// followed by `=` (and not `==`, which the lexer already tells apart)?
    pub(crate) fn looks_like_pattern_assignment(&self, pos: usize) -> bool {
        let mut i = pos;
        match self.kind_of(i) {
            Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => {}
            Kind::Op(Op::Lt) => {
                match self.skip_type_arguments(i) {
                    Some(after) => i = after,
                    None => return false,
                }
                if !matches!(self.kind_of(i), Kind::Op(Op::LBracket | Op::LBrace)) {
                    return false;
                }
            }
            Kind::Ident => {
                let Some(paren) = self.object_pattern_paren(i) else {
                    return false;
                };
                i = paren;
            }
            _ => return false,
        }
        match self.matching_close(i) {
            Some(close) => self.kind_of(close + 1) == Kind::Op(Op::Assign),
            None => false,
        }
    }

    // -- Binary and unary levels -----------------------------------------------

    fn parse_or_pattern(&mut self) -> PResult<PatternId> {
        let start = self.span();
        let mut left = self.parse_and_pattern()?;
        while self.eat_op(Op::PipePipe) {
            let right = self.parse_and_pattern()?;
            left = self.push_pattern(start, PatternKind::Or(left, right));
        }
        Ok(left)
    }

    fn parse_and_pattern(&mut self) -> PResult<PatternId> {
        let start = self.span();
        let mut left = self.parse_unary_pattern()?;
        while self.eat_op(Op::AmpAmp) {
            let right = self.parse_unary_pattern()?;
            left = self.push_pattern(start, PatternKind::And(left, right));
        }
        Ok(left)
    }

    /// `primaryPattern` followed by `?`, `!` or `as T`, as many as there are.
    fn parse_unary_pattern(&mut self) -> PResult<PatternId> {
        let start = self.span();
        let mut pattern = self.parse_primary_pattern()?;
        // fasta's `isLastPatternAllowedInsideUnaryPattern`: a relational or unary
        // pattern inside another unary one (`_ as int as num`, `> 1?`) is
        // `INVALID_INSIDE_UNARY_PATTERN`, from the start to the last token.
        let mut allowed = !matches!(
            self.ast.pattern(pattern).kind,
            PatternKind::Relational { .. }
        );
        loop {
            if !allowed
                && (self.at_op(Op::Question) || self.at_op(Op::Bang) || self.at_ident("as"))
            {
                let end_offset = self.tokens[self.pos - 1].span.end;
                self.error_at(
                    codes::parser::INVALID_INSIDE_UNARY_PATTERN,
                    Span {
                        start: start.start,
                        end: end_offset,
                    },
                    &[],
                );
            }
            allowed = false;
            if self.eat_op(Op::Question) {
                pattern = self.push_pattern(start, PatternKind::NullCheck(pattern));
            } else if self.eat_op(Op::Bang) {
                pattern = self.push_pattern(start, PatternKind::NullAssert(pattern));
            } else if self.eat_ident("as") {
                // `computeTypeAfterIsOrAs` + `ensureTypeNotVoid`, as in the `as`
                // expression: without a type, `EXPECTED_TYPE_NAME` and an empty one.
                let ty = self.type_after_is_or_as()?;
                pattern = self.push_pattern(start, PatternKind::Cast { pattern, ty });
            } else {
                return Ok(pattern);
            }
        }
    }

    // -- Primaries -------------------------------------------------------------

    fn parse_primary_pattern(&mut self) -> PResult<PatternId> {
        let start = self.span();
        match self.kind() {
            Kind::Op(Op::Lt) => {
                if let Some(after) = self.skip_type_arguments(self.pos)
                    && matches!(self.kind_of(after), Kind::Op(Op::LBracket | Op::LBrace))
                {
                    let type_args = self.parse_type_arguments_opt()?;
                    return if self.at_op(Op::LBracket) {
                        self.parse_list_pattern(start, type_args)
                    } else {
                        self.parse_map_pattern(start, type_args)
                    };
                }
                self.parse_relational_pattern(start)
            }
            Kind::Op(Op::LBracket) => self.parse_list_pattern(start, Vec::new()),
            Kind::Op(Op::LBrace) => self.parse_map_pattern(start, Vec::new()),
            Kind::Op(Op::LParen) => self.parse_parenthesized_or_record_pattern(start),
            Kind::Op(Op::EqEq | Op::BangEq | Op::LtEq | Op::Gt) => {
                self.parse_relational_pattern(start)
            }
            Kind::Keyword(Keyword::Var | Keyword::Final) => self.parse_variable_pattern(start),
            Kind::Keyword(Keyword::Const) => self.parse_constant_pattern(start),
            // `void Function() f` / `void Function() _`: a typed variable.
            Kind::Keyword(Keyword::Void) if self.variable_pattern_type_end(self.pos).is_some() => {
                let ty = self.parse_type()?;
                let name = self.expect_identifier()?;
                self.check_variable_pattern(None, false, true, name);
                Ok(self.finish_variable(start, false, false, Some(ty), name))
            }
            Kind::Ident => self.parse_identifier_pattern(start),
            Kind::Int
            | Kind::Double
            | Kind::Str(_)
            | Kind::StrBegin(..)
            | Kind::Keyword(
                Keyword::True
                | Keyword::False
                | Keyword::Null
                | Keyword::This
                | Keyword::Super
                | Keyword::New
                | Keyword::Switch,
            )
            | Kind::Op(Op::Minus | Op::Hash | Op::Bang | Op::Tilde | Op::Dot) => {
                // `.x` is a constant dot shorthand (3.10).
                self.parse_constant_pattern(start)
            }
            // `parsePrimaryPattern`: the rest is a constant pattern
            // (`parsePrecedenceExpression`), with expression recovery.
            _ => self.parse_constant_pattern(start),
        }
    }

    /// `== e`, `!= e`, `< e`, `<= e`, `> e`, `>= e`.
    fn parse_relational_pattern(&mut self, start: Span) -> PResult<PatternId> {
        let op = match self.kind() {
            Kind::Op(Op::EqEq) => BinaryOp::Eq,
            Kind::Op(Op::BangEq) => BinaryOp::NotEq,
            Kind::Op(Op::Lt) => BinaryOp::Lt,
            Kind::Op(Op::LtEq) => BinaryOp::LtEq,
            Kind::Op(Op::Gt) => match self.composed_gt() {
                Some(super::ComposedGt::Gt) => BinaryOp::Gt,
                Some(super::ComposedGt::GtEq) => {
                    self.advance();
                    BinaryOp::GtEq
                }
                _ => return Err(self.identifier_error()),
            },
            _ => return Err(self.identifier_error()),
        };
        self.advance();
        let value = self.parse_bitwise_or_expression()?;
        Ok(self.push_pattern(start, PatternKind::Relational { op, value }))
    }

    /// Constant pattern, like fasta's `parsePrimaryPattern`: with `const`
    /// (explicit context) or without (implicit), read by
    /// `parsePrecedenceExpression(EQUALITY_PRECEDENCE)` — the grammar only
    /// accepts selector precedence, but fasta reads up to equality to recover
    /// and reports the leftovers (`INVALID_CONSTANT_PATTERN_*`,
    /// `INVALID_CONSTANT_CONST_PREFIX`). The tree is that of the expression
    /// read.
    fn parse_constant_pattern(&mut self, start: Span) -> PResult<PatternId> {
        let explicit = self.at_kw(Keyword::Const);
        let ctx = if explicit {
            ConstContext::Explicit
        } else {
            ConstContext::Implicit
        };
        let start_offset = self.span();
        let expr = if explicit {
            self.explicit_const_pattern()?
        } else {
            self.unary_const_pattern(ctx)?
        };
        let expr = self.binary_const_pattern(start_offset, expr, ctx)?;
        Ok(self.push_pattern(start, PatternKind::Constant(expr)))
    }

    /// `_parsePrecedenceExpressionLoop` in constant pattern context: a binary
    /// operator (or `is`) between equality and selector is reported — with
    /// `const`, `INVALID_CONSTANT_CONST_PREFIX` at the last token read; without,
    /// `INVALID_CONSTANT_PATTERN_BINARY` at the operator — and the expression
    /// goes on without context. `as` and the trailing `!` are left to the
    /// pattern.
    fn binary_const_pattern(
        &mut self,
        start_offset: Span,
        expr: ExprId,
        ctx: ConstContext,
    ) -> PResult<ExprId> {
        let Some((_, texto, span)) = self.pattern_binary_operator() else {
            return Ok(expr);
        };
        if ctx == ConstContext::Explicit {
            let last = self.tokens[self.pos - 1].span;
            self.single_error(codes::parser::INVALID_CONSTANT_CONST_PREFIX, last, &[]);
        } else {
            self.error_at(
                codes::parser::INVALID_CONSTANT_PATTERN_BINARY,
                span,
                &[texto],
            );
        }
        self.parse_binary_rest(start_offset, expr, super::expressions::LEVEL_EQUALITY)
    }

    /// `const` followed by the rest: the forms the grammar accepts
    /// (`const [..]`, `const {..}`, `const <T>…`, `const (..)`,
    /// `const C(..)`) are read with the `const`; the others, without it, with
    /// the error from fasta's `parsePrimary`/`parseSend`.
    fn explicit_const_pattern(&mut self) -> PResult<ExprId> {
        let after = self.pos + 1;
        match self.kind_of(after) {
            Kind::Op(Op::LBracket | Op::LBrace | Op::Lt) => self.parse_unary(true),
            Kind::Op(Op::LParen) => {
                if self.kind_of(after + 1) == Kind::Op(Op::RParen) {
                    let abre = self.tokens[after].span;
                    self.error_at(
                        codes::parser::INVALID_CONSTANT_PATTERN_EMPTY_RECORD_LITERAL,
                        abre,
                        &[],
                    );
                }
                // `parsePrecedenceExpression` after the `const`: the `(` is the primary,
                // without the record's `const`.
                self.advance();
                let start_offset = self.span();
                let e = self.parse_parenthesized_or_record(start_offset, false, true)?;
                self.parse_selectors_at(start_offset, e, true, false)
            }
            Kind::Keyword(Keyword::Const) => {
                let second = self.tokens[after].span;
                self.error_at(
                    codes::parser::INVALID_CONSTANT_PATTERN_DUPLICATE_CONST,
                    second,
                    &[],
                );
                self.advance();
                self.parse_unary(true)
            }
            Kind::Ident if self.text_of(after) == "await" => {
                // `await` is a prefix (`parseUnaryExpression`), without the error.
                self.advance();
                self.parse_unary(false)
            }
            Kind::Ident => {
                // `parseSend` on each identifier of the `a.b.c` chain: the one not followed
                // by `.`, `(` or `<` (a constant invocation may still come) gets the error.
                match self.send_chain(after) {
                    SendKind::Invocation => self.parse_unary(true),
                    SendKind::Error(end_offset) => {
                        let span = self.tokens[end_offset].span;
                        self.error_at(codes::parser::INVALID_CONSTANT_CONST_PREFIX, span, &[]);
                        self.advance();
                        self.parse_unary(true)
                    }
                    SendKind::Open => {
                        self.advance();
                        self.parse_unary(true)
                    }
                }
            }
            _ => {
                self.advance();
                self.unary_const_pattern(ConstContext::Explicit)
            }
        }
    }

    /// The `a.b.c` chain starting at `pos` (after `const`), following fasta's
    /// `parseSend`: it ends at an invocation (`C(`, `C<T>(`, `C<T>.n(`,
    /// `p.C.n(`); or at the identifier (or at the `>` of its type arguments) not
    /// followed by `.`, `(` or `<`, where the error is reported; or at a `<`
    /// that does not open an invocation (the error, if any, is that of type
    /// arguments without `(`).
    fn send_chain(&self, mut pos: usize) -> SendKind {
        loop {
            if self.kind_of(pos) != Kind::Ident {
                return SendKind::Open;
            }
            let next_ = pos + 1;
            match self.kind_of(next_) {
                Kind::Op(Op::Dot) => pos = next_ + 1,
                Kind::Op(Op::LParen) => return SendKind::Invocation,
                Kind::Op(Op::Lt) => {
                    let Some(after) = self.skip_type_arguments(next_) else {
                        return SendKind::Open;
                    };
                    let invokes = self.kind_of(after) == Kind::Op(Op::LParen)
                        || (self.kind_of(after) == Kind::Op(Op::Dot)
                            && matches!(
                                self.kind_of(after + 1),
                                Kind::Ident | Kind::Keyword(Keyword::New)
                            )
                            && self.kind_of(after + 2) == Kind::Op(Op::LParen));
                    return if invokes {
                        SendKind::Invocation
                    } else {
                        SendKind::Open
                    };
                }
                _ => return SendKind::Error(pos),
            }
        }
    }

    /// `parseUnaryExpression` + `parsePrimary` with the constant pattern
    /// context: `!`/`~` are not accepted (`INVALID_CONSTANT_PATTERN_UNARY`), `-`
    /// only negates a numeric literal (`INVALID_CONSTANT_PATTERN_NEGATION`), and
    /// with `const` no literal or `-` is accepted.
    fn unary_const_pattern(&mut self, ctx: ConstContext) -> PResult<ExprId> {
        let start = self.span();
        match self.kind() {
            Kind::Op(op @ (Op::Bang | Op::Tilde)) => {
                if ctx != ConstContext::NoneFound {
                    self.erro(
                        codes::parser::INVALID_CONSTANT_PATTERN_UNARY,
                        &[op.text()],
                    );
                }
                self.advance();
                let operand = self.parse_unary(false)?;
                let op = if op == Op::Bang {
                    UnaryOp::Not
                } else {
                    UnaryOp::BitNot
                };
                Ok(self.push(start, ExprKind::Unary { op, operand }))
            }
            Kind::Op(Op::Minus) => {
                let mut inner = ctx;
                if ctx == ConstContext::Explicit {
                    self.erro(codes::parser::INVALID_CONSTANT_CONST_PREFIX, &[]);
                    inner = ConstContext::NoneFound;
                }
                self.advance();
                let operand = if inner == ConstContext::NoneFound {
                    self.parse_unary(false)?
                } else {
                    self.unary_const_pattern(ConstContext::NumericOnly)?
                };
                Ok(self.push(
                    start,
                    ExprKind::Unary {
                        op: UnaryOp::Neg,
                        operand,
                    },
                ))
            }
            Kind::Op(Op::PlusPlus | Op::MinusMinus) => self.parse_unary(false),
            Kind::Ident if self.text() == "await" => self.parse_unary(false),
            _ if ctx == ConstContext::NoneFound => self.parse_unary(false),
            kind => {
                let literal = matches!(
                    kind,
                    Kind::Str(_)
                        | Kind::StrBegin(..)
                        | Kind::Op(Op::Hash)
                        | Kind::Keyword(Keyword::True | Keyword::False | Keyword::Null)
                );
                match (ctx, kind) {
                    (ConstContext::Explicit, Kind::Int | Kind::Double) => {
                        self.erro(codes::parser::INVALID_CONSTANT_CONST_PREFIX, &[]);
                    }
                    (ConstContext::Explicit, _) if literal => {
                        self.erro(codes::parser::INVALID_CONSTANT_CONST_PREFIX, &[]);
                    }
                    (ConstContext::NumericOnly, Kind::Ident) => {
                        // `parsePrimary` of each identifier of the chain (`-p.x`: both), at the
                        // level outside the groups.
                        self.erro(codes::parser::INVALID_CONSTANT_PATTERN_NEGATION, &[]);
                        let p0 = self.pos;
                        let expr = self.parse_unary(true)?;
                        let mut prof = 0usize;
                        for i in p0 + 1..self.pos {
                            match self.kind_of(i) {
                                Kind::Op(Op::LParen | Op::LBracket | Op::LBrace | Op::Lt) => {
                                    prof += 1
                                }
                                Kind::Op(Op::RParen | Op::RBracket | Op::RBrace | Op::Gt) => {
                                    prof = prof.saturating_sub(1)
                                }
                                Kind::Op(Op::Dot | Op::QuestionDot)
                                    if prof == 0 && self.kind_of(i + 1) == Kind::Ident =>
                                {
                                    let span = self.tokens[i + 1].span;
                                    self.error_at(
                                        codes::parser::INVALID_CONSTANT_PATTERN_NEGATION,
                                        span,
                                        &[],
                                    );
                                }
                                _ => {}
                            }
                        }
                        return Ok(expr);
                    }
                    (ConstContext::NumericOnly, _) if literal => {
                        // The whole string (with interpolations) is the fasta token.
                        let span = self.string_or_token_span();
                        self.error_at(
                            codes::parser::INVALID_CONSTANT_PATTERN_NEGATION,
                            span,
                            &[],
                        );
                    }
                    _ => {}
                }
                self.parse_unary(true)
            }
        }
    }

    /// The range of the current token; in a string with interpolation, up to
    /// its end.
    fn string_or_token_span(&self) -> Span {
        let s = self.span();
        if let Kind::StrBegin(..) = self.kind() {
            let mut i = self.pos + 1;
            let mut prof = 1usize;
            while i < self.tokens.len() {
                match self.kind_of(i) {
                    Kind::StrBegin(..) => prof += 1,
                    Kind::StrEnd(_) => {
                        prof -= 1;
                        if prof == 0 {
                            return Span {
                                start: s.start,
                                end: self.tokens[i].span.end,
                            };
                        }
                    }
                    Kind::Eof => break,
                    _ => {}
                }
                i += 1;
            }
        }
        s
    }

    /// `var x`, `final x`, `final int x`, `var _`.
    fn parse_variable_pattern(&mut self, start: Span) -> PResult<PatternId> {
        let final_ = self.at_kw(Keyword::Final);
        let var_ = !final_;
        let word = self.advance().span;
        // `var int x` (an error: `VAR_AND_TYPE` in matching) also reads the type,
        // like fasta's `computeVariablePatternType`.
        let ty = if self.variable_pattern_type_end(self.pos).is_some() {
            Some(self.parse_type()?)
        } else {
            None
        };
        let name = self.expect_identifier()?;
        self.check_variable_pattern(Some(word), var_, ty.is_some(), name);
        Ok(self.finish_variable(start, final_, var_, ty, name))
    }

    /// A pattern starting with an identifier: `_`, `int x`, `int _`, `Name(…)`,
    /// a constant `a.b` or a bare identifier.
    fn parse_identifier_pattern(&mut self, start: Span) -> PResult<PatternId> {
        let pos = self.pos;
        let next = self.kind_of(pos + 1);
        if self.text() == "_" && !matches!(next, Kind::Op(Op::LParen | Op::Lt | Op::Dot)) {
            self.advance();
            return Ok(self.push_pattern(start, PatternKind::Wildcard { ty: None }));
        }
        if self.variable_pattern_type_end(pos).is_some() {
            let ty = self.parse_type()?;
            let name = self.expect_identifier()?;
            self.check_variable_pattern(None, false, true, name);
            return Ok(self.finish_variable(start, false, false, Some(ty), name));
        }
        if self.object_pattern_paren(pos).is_some() {
            return self.parse_object_pattern(start);
        }
        // `await e` in a refutable context: the bare identifier goes to the
        // constant pattern (`parsePrecedenceExpression`), which reads the `await`.
        if self.text() == "await" && (self.in_async || self.looks_like_expression_after_await(pos, true)) && self.default_context == PatternContext::Matching {
            return self.parse_constant_pattern(start);
        }
        // `a--`/`a++`: fasta reads the whole `unaryExpression` as a constant
        // pattern (and the analyzer rejects it as non-constant).
        if matches!(
            next,
            Kind::Op(Op::Dot | Op::Lt | Op::PlusPlus | Op::MinusMinus)
        ) {
            return self.parse_constant_pattern(start);
        }
        // `a + 1`: a lone identifier, in a refutable context, is a constant pattern
        // in fasta, and the operator becomes `INVALID_CONSTANT_PATTERN_BINARY`.
        {
            self.advance();
            let binary = self.pattern_binary_operator().is_some();
            self.pos = pos;
            if binary {
                return self.parse_constant_pattern(start);
            }
        }
        let name = self.identifier();
        self.check_variable_pattern(None, false, false, name);
        Ok(self.finish_variable(start, false, false, None, name))
    }

    /// A variable or, if the name is `_`, a wildcard (with the same type).
    fn finish_variable(
        &mut self,
        start: Span,
        final_: bool,
        var_: bool,
        ty: Option<crate::ast::TypeId>,
        name: Name,
    ) -> PatternId {
        if self.interner.resolve(name.sym) == "_" {
            return self.push_pattern(start, PatternKind::Wildcard { ty });
        }
        self.push_pattern(
            start,
            PatternKind::Variable {
                final_,
                var_,
                ty,
                name,
            },
        )
    }

    /// `Name<T>(fields)` / `p.Name(fields)`, with the `(` guaranteed by
    /// [`Parser::object_pattern_paren`].
    fn parse_object_pattern(&mut self, start: Span) -> PResult<PatternId> {
        let mut name = vec![self.identifier()];
        if self.eat_op(Op::Dot) {
            name.push(self.expect_identifier()?);
        }
        let args = self.parse_type_arguments_opt()?;
        let ty = self.ast.push_type(TypeAnnotation {
            span: self.span_from(start),
            nullable: false,
            kind: TypeKind::Named {
                name: name.into_boxed_slice(),
                args: args.into_boxed_slice(),
            },
        });
        let fields = self.parse_pattern_fields()?;
        Ok(self.push_pattern(
            start,
            PatternKind::Object {
                ty,
                fields: fields.into_boxed_slice(),
            },
        ))
    }

    /// `(p)`, `()`, `(p,)`, `(a, b: p, :x)` or `(int, int) name`.
    fn parse_parenthesized_or_record_pattern(&mut self, start: Span) -> PResult<PatternId> {
        if self.variable_pattern_type_end(self.pos).is_some() {
            let ty = self.parse_type()?;
            let name = self.expect_identifier()?;
            return Ok(self.finish_variable(start, false, false, Some(ty), name));
        }
        let abre = self.pos;
        self.expect_op(Op::LParen)?;
        if self.eat_op(Op::RParen) {
            return Ok(self.push_pattern(
                start,
                PatternKind::Record {
                    fields: Vec::new().into_boxed_slice(),
                },
            ));
        }
        // `parseParenthesizedPatternOrRecordPattern`: without a comma after the
        // field, the loop stops and `ensureCloseParen` skips to the matching `)`.
        let first = self.parse_pattern_field()?;
        if first.name.is_none() && !self.at_op(Op::Comma) {
            self.ensure_close_paren(abre)?;
            return Ok(self.push_pattern(start, PatternKind::Parenthesized(first.pattern)));
        }
        let mut fields = vec![first];
        while self.eat_op(Op::Comma) {
            if self.at_op(Op::RParen) {
                break;
            }
            fields.push(self.parse_pattern_field()?);
        }
        self.ensure_close_paren(abre)?;
        Ok(self.push_pattern(
            start,
            PatternKind::Record {
                fields: fields.into_boxed_slice(),
            },
        ))
    }

    /// `(field, name: p, :x)` of an object pattern, with the parentheses.
    fn parse_pattern_fields(&mut self) -> PResult<Vec<PatternField>> {
        self.expect_op(Op::LParen)?;
        let mut fields = Vec::new();
        while !self.at_op(Op::RParen) {
            fields.push(self.parse_pattern_field()?);
            if !self.eat_op(Op::Comma) {
                break;
            }
        }
        self.expect_op(Op::RParen)?;
        Ok(fields)
    }

    /// `p`, `name: p` or `:p` (name inferred from the variable pattern).
    fn parse_pattern_field(&mut self) -> PResult<PatternField> {
        let start = self.span();
        if self.eat_op(Op::Colon) {
            let pattern = self.parse_pattern()?;
            let name = self.inferred_field_name(pattern)?;
            return Ok(PatternField {
                span: self.span_from(start),
                name: Some(name),
                pattern,
            });
        }
        let name = if self.at_identifier() && self.at_op_at(1, Op::Colon) {
            let name = self.identifier();
            self.advance();
            Some(name)
        } else {
            None
        };
        let pattern = self.parse_pattern()?;
        Ok(PatternField {
            span: self.span_from(start),
            name,
            pattern,
        })
    }

    /// Name of the `:x` field: that of the variable inside `x`, `x?`, `x!`,
    /// `x as T`, `(x)`, `var x`, `final int x`.
    fn inferred_field_name(&mut self, pattern: PatternId) -> PResult<Name> {
        let mut current = pattern;
        loop {
            match &self.ast.pattern(current).kind {
                PatternKind::Variable { name, .. } => return Ok(*name),
                PatternKind::NullCheck(inner)
                | PatternKind::NullAssert(inner)
                | PatternKind::Cast { pattern: inner, .. }
                | PatternKind::Parenthesized(inner) => current = *inner,
                _ => {
                    let span = self.ast.pattern(pattern).span;
                    return Err(self.error_at(codes::parser::MISSING_IDENTIFIER, span, &[]));
                }
            }
        }
    }

    /// `[p, ..., ...rest]` after the type arguments.
    fn parse_list_pattern(
        &mut self,
        start: Span,
        type_args: Vec<crate::ast::TypeId>,
    ) -> PResult<PatternId> {
        self.expect_op(Op::LBracket)?;
        let mut elements = Vec::new();
        while !self.at_op(Op::RBracket) {
            if self.eat_op(Op::Ellipsis) {
                let sub = if self.at_op(Op::Comma) || self.at_op(Op::RBracket) {
                    None
                } else {
                    Some(self.parse_pattern()?)
                };
                elements.push(ListPatternElement::Rest(sub));
            } else {
                elements.push(ListPatternElement::Pattern(self.parse_pattern()?));
            }
            if !self.eat_op(Op::Comma) {
                break;
            }
        }
        self.expect_op(Op::RBracket)?;
        Ok(self.push_pattern(
            start,
            PatternKind::List {
                type_args: type_args.into_boxed_slice(),
                elements: elements.into_boxed_slice(),
            },
        ))
    }

    /// `{k: p, ...}` after the type arguments; `...` only without a subpattern.
    fn parse_map_pattern(
        &mut self,
        start: Span,
        type_args: Vec<crate::ast::TypeId>,
    ) -> PResult<PatternId> {
        self.expect_op(Op::LBrace)?;
        let mut entries = Vec::new();
        let mut rest = false;
        while !self.at_op(Op::RBrace) {
            if self.eat_op(Op::Ellipsis) {
                rest = true;
            } else {
                let key = self.parse_expression()?;
                self.expect_op(Op::Colon)?;
                let value = self.parse_pattern()?;
                entries.push(MapPatternEntry { key, value });
            }
            if !self.eat_op(Op::Comma) {
                break;
            }
        }
        self.expect_op(Op::RBrace)?;
        Ok(self.push_pattern(
            start,
            PatternKind::Map {
                type_args: type_args.into_boxed_slice(),
                entries: entries.into_boxed_slice(),
                rest,
            },
        ))
    }

    // -- Lookaheads -------------------------------------------------------

    /// If a variable pattern's `type identifier` is at `pos`, the position of
    /// the identifier. Applies the SDK rule that `TYPE as`/`TYPE when` is not a
    /// variable, and pre-filters the forms that are never a type before asking
    /// [`Parser::looks_like_type_then_identifier`].
    fn variable_pattern_type_end(&self, pos: usize) -> Option<usize> {
        let candidate = match self.kind_of(pos) {
            Kind::Ident => match self.kind_of(pos + 1) {
                Kind::Ident => pos + 1,
                // `int? x`: a pattern has no conditional (the constant is a
                // `unaryExpression`), so `T? name` is always a type and a variable,
                // whatever follows — including another `?` (`int? x?`, a null check).
                Kind::Op(Op::Question) if self.kind_of(pos + 2) == Kind::Ident => pos + 2,
                Kind::Op(Op::Dot | Op::Lt | Op::Question) => {
                    if !self.looks_like_type_then_identifier(pos) {
                        return None;
                    }
                    self.skip_type(pos)?
                }
                _ => return None,
            },
            Kind::Keyword(Keyword::Void) => self.skip_type(pos)?,
            Kind::Op(Op::LParen) => {
                let close = self.matching_close(pos)?;
                let ident = if self.kind_of(close + 1) == Kind::Op(Op::Question) {
                    close + 2
                } else {
                    close + 1
                };
                if self.kind_of(ident) != Kind::Ident
                    || self.is_pattern_terminator(ident)
                    || !self.looks_like_type_then_identifier(pos)
                {
                    return None;
                }
                ident
            }
            _ => return None,
        };
        if self.kind_of(candidate) != Kind::Ident || self.is_pattern_terminator(candidate) {
            return None;
        }
        Some(candidate)
    }

    /// `as` and `when` end the previous pattern instead of naming a variable.
    fn is_pattern_terminator(&self, pos: usize) -> bool {
        matches!(self.text_of(pos), "as" | "when")
    }

    /// Position of the `(` of an object pattern starting at `pos`: `Name(`,
    /// `p.Name(`, `Name<T>(`.
    fn object_pattern_paren(&self, pos: usize) -> Option<usize> {
        if self.kind_of(pos) != Kind::Ident {
            return None;
        }
        let mut i = pos + 1;
        if self.kind_of(i) == Kind::Op(Op::Dot) {
            if self.kind_of(i + 1) != Kind::Ident {
                return None;
            }
            i += 2;
        }
        if self.kind_of(i) == Kind::Op(Op::Lt) {
            i = self.skip_type_arguments(i)?;
        }
        if self.kind_of(i) == Kind::Op(Op::LParen) {
            Some(i)
        } else {
            None
        }
    }

    pub(crate) fn push_pattern(&mut self, start: Span, kind: PatternKind) -> PatternId {
        let span = self.span_from(start);
        self.ast.push_pattern(Pattern { span, kind })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Ast;
    use dartforge_diagnostics::Diagnostic;
    use dartforge_intern::Interner;

    /// `(code, range text)` of a unit's diagnostics.
    fn unit_errors(fonte: &str) -> Vec<(String, String)> {
        let mut names = Interner::new();
        let out = crate::parser::parse(fonte, &mut names);
        out.diagnostics
            .iter()
            .map(|d| {
                (
                    d.code
                        .map(|c| c.info().name.to_string())
                        .unwrap_or_default(),
                    fonte[d.span.start..d.span.end].to_string(),
                )
            })
            .collect()
    }

    /// Constant patterns with fasta's recovery (checked against
    /// `dart analyze` 3.6.2).
    #[test]
    fn constant_patterns_with_fasta_errors() {
        let case_ = |p: &str| {
            unit_errors(&format!(
                "void f(Object o) {{ switch (o) {{ case {p}: break; }} }}"
            ))
        };
        let e = |c: &str, t: &str| vec![(c.to_string(), t.to_string())];
        assert_eq!(case_("const 1 + 2"), e("invalid_constant_const_prefix", "1"));
        assert_eq!(case_("const a"), e("invalid_constant_const_prefix", "a"));
        assert_eq!(case_("const A.b"), e("invalid_constant_const_prefix", "b"));
        assert_eq!(case_("const A.n()"), vec![]);
        assert_eq!(case_("const A<int>.new()"), vec![]);
        assert_eq!(case_("const -1"), e("invalid_constant_const_prefix", "-"));
        assert_eq!(case_("-a"), e("invalid_constant_pattern_negation", "a"));
        assert_eq!(
            case_("-p.v"),
            vec![
                (
                    "invalid_constant_pattern_negation".to_string(),
                    "p".to_string()
                ),
                (
                    "invalid_constant_pattern_negation".to_string(),
                    "v".to_string()
                )
            ]
        );
        assert_eq!(case_("!true"), e("invalid_constant_pattern_unary", "!"));
        assert_eq!(case_("1 >> 2"), e("invalid_constant_pattern_binary", ">>"));
        assert_eq!(case_("1 is int"), e("invalid_constant_pattern_binary", "is"));
        assert_eq!(case_("a + 1"), e("invalid_constant_pattern_binary", "+"));
        assert_eq!(
            case_("const const A()"),
            e("invalid_constant_pattern_duplicate_const", "const")
        );
        assert_eq!(
            case_("const ()"),
            e("invalid_constant_pattern_empty_record_literal", "(")
        );
        // `const (e)`: a parenthesized expression, without the record error.
        assert_eq!(case_("const (b * 2)"), vec![]);
        assert_eq!(case_("const (1, 2)"), vec![]);
        assert_eq!(case_("A<int>"), e("invalid_constant_pattern_generic", "<"));
        assert_eq!(
            case_("_ as int as num"),
            e("invalid_inside_unary_pattern", "_ as int")
        );
        assert_eq!(case_("> 1?"), e("invalid_inside_unary_pattern", "> 1"));
    }

    /// `parseInitializers` turns off `mayParseFunctionExpressions`:
    /// `(v == null) {` in an initializer is an expression followed by the body;
    /// inside arguments the function literal comes back.
    #[test]
    fn initializer_without_function_literal() {
        assert_eq!(
            unit_errors("class A { bool b; A(Object? v, {bool? c}) : b = c ?? (v == null) { print(1); } }"),
            vec![]
        );
        assert_eq!(unit_errors("class A { var f; A() : f = g((x) { return x; }) {} }"), vec![]);
    }

    /// `ensureCloseParen`: the missing `)` is reported at the current token and
    /// the rest up to the matching `)` is skipped; the following statements go
    /// on.
    #[test]
    fn closes_parens_like_fasta() {
        assert_eq!(
            unit_errors("void f(x) { if (x case == 1 < 2) {} if (x case == 1 == 2) {} }"),
            vec![
                ("expected_token".to_string(), "<".to_string()),
                ("expected_token".to_string(), "==".to_string())
            ]
        );
        assert_eq!(
            unit_errors("void f() { g(1 2); }"),
            vec![("expected_token".to_string(), "2".to_string())]
        );
    }

    struct Out {
        names: Interner,
        ast: Ast,
        result: PResult<PatternId>,
        diagnostics: Vec<Diagnostic>,
        /// Text of the token where the parser stopped.
        rest: String,
    }

    fn pattern(src: &str) -> Out {
        let mut names = Interner::new();
        let tokens = crate::lexer::lex(src).unwrap();
        let (ast, result, diagnostics, rest) = {
            let mut p = Parser::new(src, tokens, &mut names);
            let result = p.parse_pattern();
            assert_eq!(p.depth, 0, "depth not restored");
            let rest = src[p.span().start..].to_string();
            (p.ast, result, p.diagnostics, rest)
        };
        Out {
            names,
            ast,
            result,
            diagnostics,
            rest,
        }
    }

    /// A pattern accepted without diagnostics, consuming the whole source.
    fn ok(src: &str) -> Out {
        let out = pattern(src);
        assert!(out.diagnostics.is_empty(), "{src}: {:?}", out.diagnostics);
        assert!(out.result.is_ok(), "{src}");
        assert!(out.rest.is_empty(), "{src}: sobrou {:?}", out.rest);
        out
    }

    fn lookahead(src: &str, f: impl FnOnce(&Parser<'_, '_>) -> bool) -> bool {
        let mut names = Interner::new();
        let tokens = crate::lexer::lex(src).unwrap();
        let p = Parser::new(src, tokens, &mut names);
        f(&p)
    }

    impl Out {
        fn kind(&self) -> &PatternKind {
            self.kind_of(self.result.unwrap())
        }
        fn kind_of(&self, id: PatternId) -> &PatternKind {
            &self.ast.pattern(id).kind
        }
        fn name(&self, name: &Name) -> &str {
            self.names.resolve(name.sym)
        }
        fn var_name(&self, id: PatternId) -> &str {
            match self.kind_of(id) {
                PatternKind::Variable { name, .. } => self.name(name),
                other => panic!("not a variable: {other:?}"),
            }
        }
    }

    // -- Independent of other modules ------------------------------------------

    #[test]
    fn nullable_variable_with_null_check() {
        // `case (1, int? x?)` (tests/language/patterns/flow_analysis).
        let out = ok("int? x?");
        let PatternKind::NullCheck(inside) = out.kind() else {
            panic!("{:?}", out.kind())
        };
        assert_eq!(out.var_name(*inside), "x");
        let out = ok("(0, int? x?)");
        assert!(
            matches!(out.kind(), PatternKind::Record { .. }),
            "{:?}",
            out.kind()
        );
        let out = ok("int? x!");
        assert!(matches!(out.kind(), PatternKind::NullAssert(_)));
    }

    #[test]
    fn postfix_increment_and_constant_pattern() {
        // `if (x case a--)`: a (non-constant) constant, not the variable `a`.
        let out = ok("a--");
        assert!(
            matches!(out.kind(), PatternKind::Constant(_)),
            "{:?}",
            out.kind()
        );
    }

    #[test]
    fn wildcard_and_variables() {
        let out = ok("_");
        assert!(matches!(out.kind(), PatternKind::Wildcard { ty: None }));
        let out = ok("x");
        let PatternKind::Variable {
            final_: false,
            var_: false,
            ty: None,
            name,
        } = out.kind()
        else {
            panic!()
        };
        assert_eq!(out.name(name), "x");
        let out = ok("var x");
        assert!(matches!(
            out.kind(),
            PatternKind::Variable {
                var_: true,
                final_: false,
                ty: None,
                ..
            }
        ));
        let out = ok("final x");
        assert!(matches!(
            out.kind(),
            PatternKind::Variable {
                var_: false,
                final_: true,
                ty: None,
                ..
            }
        ));
        let out = ok("var _");
        assert!(matches!(out.kind(), PatternKind::Wildcard { ty: None }));
    }

    #[test]
    fn identifier_stops_before_when() {
        let out = pattern("x when x > 0");
        assert!(out.result.is_ok() && out.diagnostics.is_empty());
        assert_eq!(out.var_name(out.result.unwrap()), "x");
        assert!(out.rest.starts_with("when"));
        let out = pattern("final x when g");
        assert!(matches!(
            out.kind(),
            PatternKind::Variable {
                final_: true,
                ty: None,
                ..
            }
        ));
        assert!(out.rest.starts_with("when"));
    }

    #[test]
    fn postfix_unaries() {
        let out = ok("x?");
        let PatternKind::NullCheck(inner) = out.kind() else {
            panic!()
        };
        assert_eq!(out.var_name(*inner), "x");
        let out = ok("x!");
        assert!(matches!(out.kind(), PatternKind::NullAssert(_)));
        let out = ok("Foo()?");
        let PatternKind::NullCheck(inner) = out.kind() else {
            panic!()
        };
        assert!(matches!(out.kind_of(*inner), PatternKind::Object { .. }));
        let out = ok("_?");
        assert!(matches!(out.kind(), PatternKind::NullCheck(_)));
    }

    #[test]
    fn logical_with_precedence_and_associativity() {
        let out = ok("a || b && c");
        let PatternKind::Or(l, r) = out.kind() else {
            panic!()
        };
        assert_eq!(out.var_name(*l), "a");
        assert!(matches!(out.kind_of(*r), PatternKind::And(..)));

        let out = ok("a && b && c");
        let PatternKind::And(l, r) = out.kind() else {
            panic!()
        };
        assert!(matches!(out.kind_of(*l), PatternKind::And(..)));
        assert_eq!(out.var_name(*r), "c");

        let out = ok("a || b || c");
        let PatternKind::Or(l, _) = out.kind() else {
            panic!()
        };
        assert!(matches!(out.kind_of(*l), PatternKind::Or(..)));
    }

    #[test]
    fn parens_and_records() {
        let out = ok("(x)");
        assert!(matches!(out.kind(), PatternKind::Parenthesized(_)));
        let out = ok("()");
        assert!(matches!(out.kind(), PatternKind::Record { fields } if fields.is_empty()));
        let out = ok("(x,)");
        assert!(matches!(out.kind(), PatternKind::Record { fields } if fields.len() == 1));
        let out = ok("(a, b)");
        let PatternKind::Record { fields } = out.kind() else {
            panic!()
        };
        assert_eq!(fields.len(), 2);
        assert!(fields.iter().all(|f| f.name.is_none()));

        let out = ok("(:a, b: c, d)");
        let PatternKind::Record { fields } = out.kind() else {
            panic!()
        };
        assert_eq!(out.name(fields[0].name.as_ref().unwrap()), "a");
        assert_eq!(out.var_name(fields[0].pattern), "a");
        assert_eq!(out.name(fields[1].name.as_ref().unwrap()), "b");
        assert_eq!(out.var_name(fields[1].pattern), "c");
        assert!(fields[2].name.is_none());

        let out = ok("(a: p)");
        assert!(matches!(out.kind(), PatternKind::Record { fields } if fields.len() == 1));

        let out = ok("((:a, :b), :c)");
        let PatternKind::Record { fields } = out.kind() else {
            panic!()
        };
        assert!(matches!(
            out.kind_of(fields[0].pattern),
            PatternKind::Record { .. }
        ));
    }

    #[test]
    fn inferred_name_goes_through_unaries() {
        let out = ok("(:x?, :var y!, :final z)");
        let PatternKind::Record { fields } = out.kind() else {
            panic!()
        };
        assert_eq!(out.name(fields[0].name.as_ref().unwrap()), "x");
        assert_eq!(out.name(fields[1].name.as_ref().unwrap()), "y");
        assert_eq!(out.name(fields[2].name.as_ref().unwrap()), "z");

        let out = pattern("(:_)");
        assert!(out.result.is_err());
        assert!(out.diagnostics.iter().any(|d| {
            d.code
                .is_some_and(|c| c.info().name == "missing_identifier")
        }));
    }

    #[test]
    fn lists() {
        let out = ok("[]");
        assert!(matches!(out.kind(), PatternKind::List { elements, .. } if elements.is_empty()));
        let out = ok("[a, b]");
        let PatternKind::List {
            type_args,
            elements,
        } = out.kind()
        else {
            panic!()
        };
        assert!(type_args.is_empty());
        assert_eq!(elements.len(), 2);
        let out = ok("[a, ...]");
        let PatternKind::List { elements, .. } = out.kind() else {
            panic!()
        };
        assert!(matches!(elements[1], ListPatternElement::Rest(None)));
        let out = ok("[..., a,]");
        let PatternKind::List { elements, .. } = out.kind() else {
            panic!()
        };
        assert_eq!(elements.len(), 2);
        let out = ok("[a, ...rest]");
        let PatternKind::List { elements, .. } = out.kind() else {
            panic!()
        };
        let ListPatternElement::Rest(Some(rest)) = elements[1] else {
            panic!()
        };
        assert_eq!(out.var_name(rest), "rest");
    }

    #[test]
    fn maps_without_expression_keys() {
        let out = ok("{}");
        assert!(
            matches!(out.kind(), PatternKind::Map { entries, rest: false, .. } if entries.is_empty())
        );
        let out = ok("{...}");
        assert!(matches!(out.kind(), PatternKind::Map { rest: true, .. }));
    }

    #[test]
    fn objects() {
        let out = ok("Foo()");
        let PatternKind::Object { ty, fields } = out.kind() else {
            panic!()
        };
        assert!(fields.is_empty());
        let ty = out.ast.ty(*ty);
        assert!(!ty.nullable);
        let TypeKind::Named { name, args } = &ty.kind else {
            panic!()
        };
        assert_eq!(name.len(), 1);
        assert!(args.is_empty());

        let out = ok("p.Foo(:x, y: z, w)");
        let PatternKind::Object { ty, fields } = out.kind() else {
            panic!()
        };
        let TypeKind::Named { name, .. } = &out.ast.ty(*ty).kind else {
            panic!()
        };
        assert_eq!(name.len(), 2);
        assert_eq!(out.name(&name[1]), "Foo");
        assert_eq!(fields.len(), 3);
        assert_eq!(out.name(fields[0].name.as_ref().unwrap()), "x");
        assert_eq!(out.name(fields[1].name.as_ref().unwrap()), "y");
        assert!(fields[2].name.is_none());
    }

    #[test]
    fn errors() {
        // `parsePrimaryPattern`: a constant pattern, with the expression's
        // synthetic identifier.
        let out = pattern(")");
        assert!(out.diagnostics.iter().any(|d| {
            d.code
                .is_some_and(|c| c.info().name == "missing_identifier")
        }));
        let out = pattern("[a");
        assert!(out.result.is_err());
        let out = pattern("(a, b");
        assert!(out.result.is_err());
    }

    #[test]
    fn pattern_declaration_lookahead() {
        assert!(lookahead("var (a, b) = e;", |p| p.looks_like_pattern_declaration(0)));
        assert!(lookahead("final [a, b] = e;", |p| p.looks_like_pattern_declaration(0)));
        assert!(lookahead("final {'k': v} = e;", |p| p
            .looks_like_pattern_declaration(0)));
        assert!(lookahead("final (k, v) in m", |p| p.looks_like_pattern_declaration(0)));
        assert!(!lookahead("final (int, int) r = e;", |p| p
            .looks_like_pattern_declaration(0)));
        assert!(!lookahead("var x = 1;", |p| p.looks_like_pattern_declaration(0)));
        assert!(!lookahead("x (a, b) = e;", |p| p.looks_like_pattern_declaration(0)));
        assert!(lookahead("late var (a, b) = e;", |p| p
            .looks_like_pattern_declaration(1)));
    }

    #[test]
    fn pattern_assignment_lookahead() {
        assert!(lookahead("(a, b) = (b, a);", |p| p.looks_like_pattern_assignment(0)));
        assert!(lookahead("[x, y] = list;", |p| p.looks_like_pattern_assignment(0)));
        assert!(lookahead("{'k': v} = map;", |p| p.looks_like_pattern_assignment(0)));
        assert!(lookahead("Foo(:x) = p;", |p| p.looks_like_pattern_assignment(0)));
        assert!(lookahead("p.Foo(:x) = q;", |p| p.looks_like_pattern_assignment(0)));
        assert!(!lookahead("(a, b) == c;", |p| p.looks_like_pattern_assignment(0)));
        assert!(!lookahead("[1, 2] == x;", |p| p.looks_like_pattern_assignment(0)));
        assert!(!lookahead("foo(x);", |p| p.looks_like_pattern_assignment(0)));
        assert!(!lookahead("x = 1;", |p| p.looks_like_pattern_assignment(0)));
        assert!(!lookahead("(a, b", |p| p.looks_like_pattern_assignment(0)));
        assert!(!lookahead("{ x = 1; }", |p| p.looks_like_pattern_assignment(0)));
    }

    #[test]
    fn depth_limit() {
        let depth = super::super::MAX_DEPTH as usize + 10;
        let src = format!("{}x{}", "(".repeat(depth), ")".repeat(depth));
        let out = pattern(&src);
        assert!(out.result.is_err());
        assert!(
            out.diagnostics
                .iter()
                .any(|d| d.code.is_some_and(|c| c.info().name == "stack_overflow"))
        );
    }

    // -- Depending on expressions.rs / types.rs --------------------------------
    //
    // They panic with "not yet implemented" while those modules are stubs.
    mod dependents {
        use super::*;

        #[test]
        fn constants_() {
            for src in [
                "1",
                "-1",
                "'a'",
                "null",
                "true",
                "#sym",
                "const [1]",
                "const C()",
                "a.b",
                "a.b.c",
            ] {
                let out = ok(src);
                assert!(matches!(out.kind(), PatternKind::Constant(_)), "{src}");
            }
        }

        #[test]
        fn relationals() {
            let out = ok("== 1");
            assert!(matches!(
                out.kind(),
                PatternKind::Relational {
                    op: BinaryOp::Eq,
                    ..
                }
            ));
            let out = ok("!= null");
            assert!(matches!(
                out.kind(),
                PatternKind::Relational {
                    op: BinaryOp::NotEq,
                    ..
                }
            ));
            let out = ok("> 0");
            assert!(matches!(
                out.kind(),
                PatternKind::Relational {
                    op: BinaryOp::Gt,
                    ..
                }
            ));
            let out = ok(">= 0");
            assert!(matches!(
                out.kind(),
                PatternKind::Relational {
                    op: BinaryOp::GtEq,
                    ..
                }
            ));
            let out = ok("< 0");
            assert!(matches!(
                out.kind(),
                PatternKind::Relational {
                    op: BinaryOp::Lt,
                    ..
                }
            ));
            let out = ok("<= a | b");
            assert!(matches!(
                out.kind(),
                PatternKind::Relational {
                    op: BinaryOp::LtEq,
                    ..
                }
            ));
            let out = ok("> 0 && < 10");
            assert!(matches!(out.kind(), PatternKind::And(..)));
        }

        #[test]
        fn typed_variables() {
            let out = ok("int x");
            assert!(matches!(
                out.kind(),
                PatternKind::Variable {
                    ty: Some(_),
                    final_: false,
                    ..
                }
            ));
            let out = ok("final int x");
            assert!(matches!(
                out.kind(),
                PatternKind::Variable {
                    ty: Some(_),
                    final_: true,
                    ..
                }
            ));
            let out = ok("int _");
            assert!(matches!(out.kind(), PatternKind::Wildcard { ty: Some(_) }));
            let out = ok("List<int> xs");
            assert!(matches!(
                out.kind(),
                PatternKind::Variable { ty: Some(_), .. }
            ));
            let out = ok("a.B c");
            assert!(matches!(
                out.kind(),
                PatternKind::Variable { ty: Some(_), .. }
            ));
            let out = ok("int? x");
            let PatternKind::Variable { ty: Some(ty), .. } = out.kind() else {
                panic!()
            };
            assert!(out.ast.ty(*ty).nullable);
            let out = ok("(int, int) p");
            assert!(matches!(
                out.kind(),
                PatternKind::Variable { ty: Some(_), .. }
            ));
            let out = pattern("String s when s.isEmpty");
            assert!(matches!(
                out.kind(),
                PatternKind::Variable { ty: Some(_), .. }
            ));
            assert!(out.rest.starts_with("when"));
        }

        #[test]
        fn cast_and_type_followed_by_as() {
            let out = ok("x as int");
            assert!(matches!(out.kind(), PatternKind::Cast { .. }));
            // `Foo as Bar` is the pattern `Foo` cast, not the variable `as`.
            let out = ok("Foo as Bar");
            let PatternKind::Cast { pattern, .. } = out.kind() else {
                panic!()
            };
            assert_eq!(out.var_name(*pattern), "Foo");
            let out = ok("(a, b) as (int, int)");
            assert!(matches!(out.kind(), PatternKind::Cast { .. }));
        }

        #[test]
        fn typed_collections_and_maps() {
            let out = ok("<int>[a, b]");
            assert!(
                matches!(out.kind(), PatternKind::List { type_args, .. } if type_args.len() == 1)
            );
            let out = ok("<String, int>{'k': v, ...}");
            let PatternKind::Map {
                type_args,
                entries,
                rest: true,
            } = out.kind()
            else {
                panic!()
            };
            assert_eq!(type_args.len(), 2);
            assert_eq!(entries.len(), 1);
            let out = ok("{'k': var v, 1: _}");
            assert!(
                matches!(out.kind(), PatternKind::Map { entries, rest: false, .. } if entries.len() == 2)
            );
            let out = ok("[int a, ...]");
            assert!(matches!(out.kind(), PatternKind::List { .. }));
        }

        #[test]
        fn generic_objects_and_qualified_constants() {
            let out = ok("Foo<int>(x: 0)");
            let PatternKind::Object { ty, fields } = out.kind() else {
                panic!()
            };
            let TypeKind::Named { args, .. } = &out.ast.ty(*ty).kind else {
                panic!()
            };
            assert_eq!(args.len(), 1);
            assert_eq!(fields.len(), 1);
            let out = ok("Point(x: 0, y: > 1)");
            assert!(matches!(out.kind(), PatternKind::Object { .. }));
            let out = ok("'a' || 'b'");
            assert!(matches!(out.kind(), PatternKind::Or(..)));
        }
    }
}
