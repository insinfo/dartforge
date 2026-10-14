//! Expressions (fasta's `parseExpression`, `parsePrecedenceExpression`,
//! `parseUnaryExpression`, `parsePrimary`, `parseSend`,
//! `parseArgumentOrIndexStar`, `parseCascadeExpression` and the literals),
//! building the nodes like the `AstBuilder`:
//!
//! * `handleSend` with arguments: `MethodInvocation` if the receiver is a
//!   `SimpleIdentifier`, otherwise `FunctionExpressionInvocation`;
//! * `doDotExpression`: `a.b` with `a` a simple identifier is a
//!   `PrefixedIdentifier`; other accesses are `PropertyAccess`; an
//!   invocation to the right of the `.` receives the target;
//! * type arguments without a call (`List<int>`, `a.b<T>`) are a
//!   `FunctionReference` (`handleTypeArgumentApplication`).

use dartforge_frontend::token::{Keyword, Op};

use super::super::tree::{Node, NodeId};
use super::super::token::{TokenKind, Tk};
use super::patterns::PatternCtx;
use super::types::MemberKind;
use super::{AsyncKind, Parser, R, prec};

/// `ConstantPatternContext` do fasta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConstCtx {
    NoneKind,
    Implicit,
    Explicit,
    NumericOnly,
}

/// The first invalid escape in `b` (the text between the quotes), like
/// fasta's `unescapeCodeUnits` (`quote.dart`).
fn escape_error(b: &[u8]) -> Option<&'static str> {
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
            return Some("An escape sequence starting with '\\' must be followed by a character.");
        }
        match b[i] {
            b'x' => {
                if n <= i + 2 || !hex(b[i + 1]) || !hex(b[i + 2]) {
                    return Some(
                        "An escape sequence starting with '\\x' must be followed by 2 hexadecimal digits.",
                    );
                }
                i += 2;
            }
            b'u' => {
                if n == i + 1 {
                    return Some(
                        "An escape sequence starting with '\\u' must be followed by 4 hexadecimal digits.",
                    );
                }
                let mut code: u32 = 0;
                if b[i + 1] == b'{' {
                    i += 1;
                    let mut closed = false;
                    for j in 0..7 {
                        i += 1;
                        if i == n {
                            return Some("Invalid '\\u{...}' escape.");
                        }
                        let d = b[i];
                        if j != 0 && d == b'}' {
                            closed = true;
                            break;
                        } else if j == 6 || !hex(d) {
                            return Some("Invalid '\\u{...}' escape.");
                        }
                        code = (code << 4) + value(d);
                    }
                    if !closed {
                        return Some("Invalid '\\u{...}' escape.");
                    }
                } else {
                    if n <= i + 4 {
                        return Some(
                            "An escape sequence starting with '\\u' must be followed by 4 hexadecimal digits.",
                        );
                    }
                    for _ in 0..4 {
                        i += 1;
                        if !hex(b[i]) {
                            return Some(
                                "An escape sequence starting with '\\u' must be followed by 4 hexadecimal digits.",
                            );
                        }
                        code = (code << 4) + value(b[i]);
                    }
                }
                if code > 0x10FFFF {
                    return Some("The escape sequence isn't a valid code point.");
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

impl Parser<'_> {
    /// `parseExpression`.
    pub(super) fn expression(&mut self) -> R<NodeId> {
        self.enter_()?;
        let r = self.inner_expression();
        self.leave();
        r
    }

    fn inner_expression(&mut self) -> R<NodeId> {
        if self.patterns_
            && let Some(f) = self.skip_outer_pattern(self.pos)
            && self.op(f + 1, Op::Assign)
        {
            return self.pattern_assignment();
        }
        if self.kw(self.pos, Keyword::Throw) {
            return self.throw_(true);
        }
        self.precedence(prec::ASSIGNMENT, true, ConstCtx::NoneKind)
    }

    /// `parseExpressionWithoutCascade`.
    pub(super) fn expression_without_cascade(&mut self) -> R<NodeId> {
        if self.kw(self.pos, Keyword::Throw) {
            return self.throw_(false);
        }
        self.enter_()?;
        let r = self.precedence(prec::ASSIGNMENT, false, ConstCtx::NoneKind);
        self.leave();
        r
    }

    /// `parsePatternAssignment`.
    fn pattern_assignment(&mut self) -> R<NodeId> {
        let pattern = self.pattern(PatternCtx::Assignment)?;
        let start_ = self.start_(pattern);
        let equals = self.expect_op(Op::Assign)?;
        let expression = self.expression()?;
        Ok(self.node(
            Node::PatternAssignment {
                pattern,
                equals: equals as Tk,
                expression,
            },
            start_,
        ))
    }

    /// `parseThrowExpression`.
    fn throw_(&mut self, cascades: bool) -> R<NodeId> {
        let start_ = self.advance();
        if self.op(self.pos, Op::Semicolon) {
            return Err(self.error("Missing expression after 'throw'."));
        }
        let expression = if cascades {
            self.expression()?
        } else {
            self.expression_without_cascade()?
        };
        Ok(self.node(
            Node::ThrowExpression {
                throw_keyword: start_ as Tk,
                expression,
            },
            start_,
        ))
    }

    /// `parsePrecedenceExpression`.
    pub(super) fn precedence(&mut self, level: u8, cascades: bool, ctx: ConstCtx) -> R<NodeId> {
        let e = self.unary(cascades, ctx)?;
        let (e, targs) = self.method_args_after(e, ctx)?;
        self.precedence_loop(level, cascades, targs, e, ctx)
    }

    /// Method type arguments after an expression (with an optional `!`):
    /// `e<T>(...)` leaves the arguments pending for the call; `e<T>` without a
    /// call becomes a `FunctionReference`.
    fn method_args_after(&mut self, e: NodeId, ctx: ConstCtx) -> R<(NodeId, Option<NodeId>)> {
        let last = self.pos - 1;
        let bang = if self.op(self.pos, Op::Bang) {
            self.pos
        } else {
            last
        };
        if self.method_args_at(bang).is_none() {
            return Ok((e, None));
        }
        let mut e = e;
        if bang == self.pos {
            let b = self.advance();
            let start_ = self.start_(e);
            e = self.node(
                Node::PostfixExpression {
                    operand: e,
                    operator: b as Tk,
                },
                start_,
            );
        }
        let ta = self.type_args()?;
        if !self.op(self.pos, Op::LParen) {
            if ctx != ConstCtx::NoneKind {
                return Err(self.error_at(
                    self.start_(ta),
                    "This expression isn't a valid constant pattern.",
                ));
            }
            self.feature(self.start_(ta), (2, 15), "constructor-tearoffs");
            let start_ = self.start_(e);
            let r = self.node(
                Node::FunctionReference {
                    function: e,
                    type_arguments: Some(ta),
                },
                start_,
            );
            return Ok((r, None));
        }
        Ok((e, Some(ta)))
    }

    /// Precedence of token `i` (`_computePrecedence`).
    pub(super) fn precedence_at(&mut self, i: usize, pattern: bool) -> u8 {
        use prec::*;
        match self.kind_(i) {
            TokenKind::Op(o) => match o {
                Op::Bang => {
                    if matches!(
                        self.kind_(i + 1),
                        TokenKind::Op(
                            Op::Dot | Op::Question | Op::LParen | Op::LBracket | Op::QuestionDot
                        )
                    ) {
                        SELECTOR
                    } else {
                        POSTFIX
                    }
                }
                Op::Question => {
                    if pattern || (self.op(i + 1, Op::LBracket) && !self.could_be_conditional(i)) {
                        SELECTOR
                    } else {
                        CONDITIONAL
                    }
                }
                Op::Gt => match self.toks.composed_greater(i as Tk) {
                    Some(">>" | ">>>") => SHIFT,
                    Some(">>=" | ">>>=") => ASSIGNMENT,
                    _ => RELATIONAL,
                },
                Op::LParen | Op::LBracket | Op::Dot | Op::QuestionDot => SELECTOR,
                Op::PlusPlus | Op::MinusMinus => POSTFIX,
                Op::Tilde => PREFIX,
                Op::Star | Op::Slash | Op::TildeSlash | Op::Percent => MULTIPLICATIVE,
                Op::Plus | Op::Minus => ADDITIVE,
                Op::LtLt => SHIFT,
                Op::Amp => BIT_AND,
                Op::Caret => BIT_XOR,
                Op::Pipe => BIT_OR,
                Op::Lt | Op::LtEq => RELATIONAL,
                Op::EqEq | Op::BangEq => EQUALITY,
                Op::AmpAmp => E,
                Op::PipePipe => OR,
                Op::QuestionQuestion => IF_NULL,
                Op::DotDot | Op::QuestionDotDot => CASCADE,
                Op::Assign
                | Op::PlusAssign
                | Op::MinusAssign
                | Op::StarAssign
                | Op::SlashAssign
                | Op::TildeSlashAssign
                | Op::PercentAssign
                | Op::LtLtAssign
                | Op::AmpAssign
                | Op::PipeAssign
                | Op::CaretAssign
                | Op::QuestionQuestionAssign => ASSIGNMENT,
                _ => 0,
            },
            TokenKind::Reserved(Keyword::Is) => RELATIONAL,
            TokenKind::Identifier if self.txt(i) == "as" => RELATIONAL,
            _ => 0,
        }
    }

    /// Assignment operator at `i` and how many tokens it takes.
    fn assignment_op_at(&self, i: usize) -> Option<usize> {
        match self.kind_(i) {
            TokenKind::Op(
                Op::Assign
                | Op::PlusAssign
                | Op::MinusAssign
                | Op::StarAssign
                | Op::SlashAssign
                | Op::TildeSlashAssign
                | Op::PercentAssign
                | Op::LtLtAssign
                | Op::AmpAssign
                | Op::PipeAssign
                | Op::CaretAssign
                | Op::QuestionQuestionAssign,
            ) => Some(1),
            TokenKind::Op(Op::Gt) => match self.toks.composed_greater(i as Tk) {
                Some(">>=") => Some(3),
                Some(">>>=") => Some(4),
                _ => None,
            },
            _ => None,
        }
    }

    /// Consumes the composite or simple operator at `self.pos`.
    fn consume_operator(&mut self) -> usize {
        let i = self.pos;
        if self.op(i, Op::Gt)
            && let Some(a) = self.toks.composed_greater(i as Tk)
        {
            self.merge_greater_at(i, a);
            self.pos += a.len();
            return i;
        }
        self.pos += 1;
        i
    }

    /// `canParseAsConditional`: the `?` at `q` starts a valid conditional.
    pub(super) fn could_be_conditional(&mut self, q: usize) -> bool {
        self.probe(|p| {
            p.pos = q + 1;
            p.expression_without_cascade()?;
            if !p.op(p.pos, Op::Colon) {
                return Err(p.error("Expected ':'."));
            }
            p.pos += 1;
            p.expression_without_cascade()
        })
        .is_some()
    }

    /// `isAssignable` do analyzer.
    fn assignable(&self, e: NodeId) -> bool {
        matches!(
            self.tree_.node(e),
            Node::SimpleIdentifier { .. }
                | Node::PrefixedIdentifier { .. }
                | Node::PropertyAccess { .. }
                | Node::IndexExpression { .. }
        )
    }

    /// `reportErrorIfSuper`: `super` alone as an operand.
    pub(super) fn without_super(&self, e: NodeId) -> R<()> {
        if matches!(self.tree_.node(e), Node::SuperExpression { .. }) {
            return Err(self.error_at(
                self.start_(e),
                "Missing selector such as '.identifier' or '[0]'.",
            ));
        }
        Ok(())
    }

    fn assignment(&mut self, lhs: NodeId, operator_: usize, rhs: NodeId) -> R<NodeId> {
        self.without_super(rhs)?;
        if !self.assignable(lhs) {
            return Err(self.error_at(
                self.start_(lhs),
                "Missing selector such as '.identifier' or '[0]'.",
            ));
        }
        let start_ = self.start_(lhs);
        Ok(self.node(
            Node::AssignmentExpression {
                left_hand_side: lhs,
                operator: operator_ as Tk,
                right_hand_side: rhs,
            },
            start_,
        ))
    }

    /// `_parsePrecedenceExpressionLoop`.
    fn precedence_loop(
        &mut self,
        min_level: u8,
        cascades: bool,
        mut targs: Option<NodeId>,
        mut e: NodeId,
        ctx: ConstCtx,
    ) -> R<NodeId> {
        let mut ctx = ctx;
        let mut token_level = self.precedence_at(self.pos, false);
        if ctx != ConstCtx::NoneKind {
            if self.op(self.pos, Op::Bang) {
                if token_level == prec::POSTFIX || self.op(self.pos + 1, Op::Question) {
                    return Ok(e);
                }
            } else if self.word(self.pos, "as") {
                return Ok(e);
            }
            if min_level <= token_level && token_level < prec::SELECTOR {
                if ctx == ConstCtx::Explicit || token_level <= prec::MULTIPLICATIVE {
                    return Err(self.error("This expression isn't a valid constant pattern."));
                }
                ctx = ConstCtx::NoneKind;
            }
        }
        let mut level = token_level as i32;
        while level >= min_level as i32 {
            let mut last_binary: i32 = -1;
            let mut last_cascade = false;
            while token_level as i32 == level {
                let operator_ = self.pos;
                if token_level == prec::CASCADE {
                    if !cascades {
                        return Ok(e);
                    }
                    if last_cascade && self.op(self.pos, Op::QuestionDotDot) {
                        return Err(self.error(
                            "The '?..' cascade operator must be first in the cascade sequence.",
                        ));
                    }
                    last_cascade = true;
                    e = self.cascade(e)?;
                } else if token_level == prec::ASSIGNMENT {
                    let op_tok = self.consume_operator();
                    let rhs = if self.kw(self.pos, Keyword::Throw) {
                        self.throw_(false)?
                    } else {
                        self.precedence(level as u8, cascades, ConstCtx::NoneKind)?
                    };
                    e = self.assignment(e, op_tok, rhs)?;
                } else if token_level == prec::POSTFIX {
                    if self.op(self.pos, Op::PlusPlus)
                        || self.op(self.pos, Op::MinusMinus)
                        || self.op(self.pos, Op::Bang)
                    {
                        if !self.op(self.pos, Op::Bang) && !self.assignable(e) {
                            return Err(self.error_at(
                                self.start_(e),
                                "Illegal assignment to non-assignable expression.",
                            ));
                        }
                        let o = self.advance();
                        let start_ = self.start_(e);
                        e = self.node(
                            Node::PostfixExpression {
                                operand: e,
                                operator: o as Tk,
                            },
                            start_,
                        );
                    }
                } else if token_level == prec::SELECTOR {
                    if self.op(self.pos, Op::Dot) || self.op(self.pos, Op::QuestionDot) {
                        let dot = self.advance();
                        let right = self.primary_continuation(ctx)?;
                        e = self.dot(Some(e), dot, right)?;
                        let (e2, t2) = self.method_args_after(e, ctx)?;
                        e = e2;
                        targs = t2;
                    } else if self.op(self.pos, Op::LParen) || self.op(self.pos, Op::LBracket) {
                        e = self.args_or_index(e, targs.take(), false)?;
                    } else if self.op(self.pos, Op::Question) {
                        e = self.args_or_index(e, targs.take(), true)?;
                    } else if self.op(self.pos, Op::Bang) {
                        let o = self.advance();
                        let start_ = self.start_(e);
                        e = self.node(
                            Node::PostfixExpression {
                                operand: e,
                                operator: o as Tk,
                            },
                            start_,
                        );
                    } else {
                        return Err(self.error("Unexpected token."));
                    }
                } else if self.kw(self.pos, Keyword::Is) {
                    e = self.is_rest(e)?;
                } else if self.word(self.pos, "as") {
                    e = self.as_rest(e)?;
                } else if self.op(self.pos, Op::Question) {
                    e = self.conditional_rest(e)?;
                } else {
                    if level == prec::EQUALITY as i32 || level == prec::RELATIONAL as i32 {
                        if last_binary == level {
                            return Err(self.error("An equality expression can't be an operand of another equality expression."));
                        }
                        last_binary = level;
                    }
                    let o = self.consume_operator();
                    let _ = operator_;
                    let right =
                        self.precedence((level + 1) as u8, cascades, ConstCtx::NoneKind)?;
                    self.without_super(right)?;
                    let start_ = self.start_(e);
                    e = self.node(
                        Node::BinaryExpression {
                            left_operand: e,
                            operator: o as Tk,
                            right_operand: right,
                        },
                        start_,
                    );
                }
                token_level = self.precedence_at(self.pos, false);
                if ctx != ConstCtx::NoneKind {
                    if self.op(self.pos, Op::Bang) {
                        if token_level == prec::POSTFIX || self.op(self.pos + 1, Op::Question) {
                            return Ok(e);
                        }
                    } else if self.word(self.pos, "as") {
                        return Ok(e);
                    }
                }
            }
            level -= 1;
        }
        Ok(e)
    }

    /// `computeTypeAfterIsOrAs` followed by the type.
    pub(super) fn type_after_is_as(&mut self) -> R<NodeId> {
        let i = self.pos;
        let mut ti = self.type_at(i, true, false, false);
        if ti.is_none() {
            return Err(self.error(&format!("Expected a type, but got '{}'.", self.txt(i))));
        }
        if ti.nullable() {
            let nx = ti.end_index;
            let end_ok = self.eof(nx)
                || matches!(
                    self.kind_(nx),
                    TokenKind::Op(
                        Op::RParen
                            | Op::RBrace
                            | Op::RBracket
                            | Op::Question
                            | Op::QuestionQuestion
                            | Op::Comma
                            | Op::Semicolon
                            | Op::Colon
                            | Op::DotDot
                            | Op::PipePipe
                            | Op::AmpAmp
                    )
                )
                || self.kw(nx, Keyword::Is)
                || self.word(nx, "as");
            if !end_ok {
                let keep = (self.op(nx, Op::LBrace) || self.word(nx, "when"))
                    && !self.could_be_conditional(nx - 1);
                if !keep {
                    ti = ti.without_question();
                }
            }
        }
        if self.kw(i, Keyword::Void) && ti.end_index == i + 1 {
            return Err(self.error_at(i, "The type 'void' can't be used here."));
        }
        self.type_until(ti.end_index)
    }

    fn no_chained_is_as(&self) -> R<()> {
        if self.kw(self.pos, Keyword::Is) || self.word(self.pos, "as") {
            return Err(self.error("Unexpected token."));
        }
        Ok(())
    }

    /// `parseIsOperatorRest`.
    fn is_rest(&mut self, e: NodeId) -> R<NodeId> {
        let is = self.advance();
        let not = self.optional_op(Op::Bang).map(|b| b as Tk);
        let type_ = self.type_after_is_as()?;
        let start_ = self.start_(e);
        let r = self.node(
            Node::IsExpression {
                expression: e,
                is_operator: is as Tk,
                not_operator: not,
                type_,
            },
            start_,
        );
        self.no_chained_is_as()?;
        Ok(r)
    }

    /// `parseAsOperatorRest`.
    fn as_rest(&mut self, e: NodeId) -> R<NodeId> {
        let a = self.advance();
        let type_ = self.type_after_is_as()?;
        let start_ = self.start_(e);
        let r = self.node(
            Node::AsExpression {
                expression: e,
                as_operator: a as Tk,
                type_,
            },
            start_,
        );
        self.no_chained_is_as()?;
        Ok(r)
    }

    /// `parseConditionalExpressionRest`.
    fn conditional_rest(&mut self, c: NodeId) -> R<NodeId> {
        let q = self.advance();
        let then_expression = self.expression_without_cascade()?;
        let colon = self.expect_op(Op::Colon)?;
        let else_expression = self.expression_without_cascade()?;
        let start_ = self.start_(c);
        Ok(self.node(
            Node::ConditionalExpression {
                condition: c,
                question: q as Tk,
                then_expression,
                colon: colon as Tk,
                else_expression,
            },
            start_,
        ))
    }

    /// `parseCascadeExpression`.
    fn cascade(&mut self, target_: NodeId) -> R<NodeId> {
        let op_c = self.advance();
        let mut sec = if self.op(self.pos, Op::LBracket) {
            let lb = self.advance();
            let old = self.functions;
            self.functions = true;
            let index = self.expression()?;
            self.functions = old;
            let rb = self.expect_op(Op::RBracket)?;
            let s = self.node(
                Node::IndexExpression {
                    target: None,
                    period: Some(op_c as Tk),
                    question: None,
                    left_bracket: lb as Tk,
                    index,
                    right_bracket: rb as Tk,
                },
                op_c,
            );
            let (s, t) = self.method_args_after(s, ConstCtx::NoneKind)?;
            self.args_or_index(s, t, false)?
        } else {
            let d = self.send(ConstCtx::NoneKind, true)?;
            self.dot(None, op_c, d)?
        };
        loop {
            let mark = self.pos;
            if self.op(self.pos, Op::Dot) || self.op(self.pos, Op::QuestionDot) {
                let p = self.advance();
                let d = self.send(ConstCtx::NoneKind, true)?;
                sec = self.dot(Some(sec), p, d)?;
            } else if self.op(self.pos, Op::Bang) {
                let b = self.advance();
                let start_ = self.start_(sec);
                sec = self.node(
                    Node::PostfixExpression {
                        operand: sec,
                        operator: b as Tk,
                    },
                    start_,
                );
            }
            let mut targs = None;
            if self.method_args_at(self.pos - 1).is_some() {
                let ta = self.type_args()?;
                if !self.op(self.pos, Op::LParen) {
                    self.feature(self.start_(ta), (2, 15), "constructor-tearoffs");
                    let start_ = self.start_(sec);
                    sec = self.node(
                        Node::FunctionReference {
                            function: sec,
                            type_arguments: Some(ta),
                        },
                        start_,
                    );
                } else {
                    targs = Some(ta);
                }
            }
            sec = self.args_or_index(sec, targs, false)?;
            if self.pos == mark {
                break;
            }
        }
        if self.assignment_op_at(self.pos).is_some() {
            let o = self.consume_operator();
            let rhs = self.expression_without_cascade()?;
            sec = self.assignment(sec, o, rhs)?;
        }
        let end = self.end(sec);
        if let Node::CascadeExpression {
            cascade_sections, ..
        } = &mut self.tree_.nodes[target_.0 as usize].node
        {
            cascade_sections.push(sec);
            self.tree_.nodes[target_.0 as usize].end = end as Tk;
            return Ok(target_);
        }
        let start_ = self.start_(target_);
        Ok(self.node(
            Node::CascadeExpression {
                target: target_,
                cascade_sections: vec![sec],
            },
            start_,
        ))
    }

    /// `parseArgumentOrIndexStar`.
    fn args_or_index(&mut self, e: NodeId, targs: Option<NodeId>, checked: bool) -> R<NodeId> {
        let mut e = e;
        let mut targs = targs;
        loop {
            let mut potential =
                self.op(self.pos, Op::Question) && self.op(self.pos + 1, Op::LBracket);
            if potential && !checked && self.could_be_conditional(self.pos) {
                potential = false;
            }
            if self.op(self.pos, Op::LBracket) || potential {
                let question = if self.op(self.pos, Op::Question) {
                    Some(self.advance() as Tk)
                } else {
                    None
                };
                let lb = self.advance();
                let old = self.functions;
                self.functions = true;
                let index = self.expression()?;
                self.functions = old;
                let rb = self.expect_op(Op::RBracket)?;
                let start_ = self.start_(e);
                e = self.node(
                    Node::IndexExpression {
                        target: Some(e),
                        period: None,
                        question,
                        left_bracket: lb as Tk,
                        index,
                        right_bracket: rb as Tk,
                    },
                    start_,
                );
                let (e2, t2) = self.method_args_after(e, ConstCtx::NoneKind)?;
                e = e2;
                targs = t2;
            } else if self.op(self.pos, Op::LParen) {
                let args = self.arguments_()?;
                e = self.invocation(e, targs.take(), args);
                let (e2, t2) = self.method_args_after(e, ConstCtx::NoneKind)?;
                e = e2;
                targs = t2;
            } else {
                break;
            }
        }
        Ok(e)
    }

    /// `doInvocation`.
    fn invocation(&mut self, receiver: NodeId, targs: Option<NodeId>, args: NodeId) -> NodeId {
        let start_ = self.start_(receiver);
        if self.is_simple(receiver) {
            return self.node(
                Node::MethodInvocation {
                    target: None,
                    operator: None,
                    method_name: receiver,
                    type_arguments: targs,
                    argument_list: args,
                },
                start_,
            );
        }
        self.node(
            Node::FunctionExpressionInvocation {
                function: receiver,
                type_arguments: targs,
                argument_list: args,
            },
            start_,
        )
    }

    /// `doDotExpression`: `receiver` is `None` in a cascade section.
    fn dot(&mut self, receiver: Option<NodeId>, dot: usize, right: NodeId) -> R<NodeId> {
        let start_ = receiver.map_or(dot, |r| self.start_(r));
        match self.tree_.node(right) {
            Node::SimpleIdentifier { .. } => {
                if let Some(r) = receiver
                    && self.is_simple(r)
                    && self.op(dot, Op::Dot)
                {
                    return Ok(self.node(
                        Node::PrefixedIdentifier {
                            prefix: r,
                            period: dot as Tk,
                            identifier: right,
                        },
                        start_,
                    ));
                }
                Ok(self.node(
                    Node::PropertyAccess {
                        target: receiver,
                        operator: dot as Tk,
                        property_name: right,
                    },
                    start_,
                ))
            }
            Node::MethodInvocation { target: None, .. } => {
                let d = &mut self.tree_.nodes[right.0 as usize];
                if let Node::MethodInvocation {
                    target, operator, ..
                } = &mut d.node
                {
                    *target = receiver;
                    *operator = Some(dot as Tk);
                }
                d.start = start_ as Tk;
                Ok(right)
            }
            _ => Err(self.error_at(self.start_(right), "Expected an identifier.")),
        }
    }

    /// `parseUnaryExpression`.
    fn unary(&mut self, cascades: bool, ctx: ConstCtx) -> R<NodeId> {
        let i = self.pos;
        if self.word(i, "await") {
            // Outside an `async` body, fasta reports `AwaitNotAsync`, which is not a
            // syntax error in the analyzer, and builds the `AwaitExpression`.
            if self.async_kind == AsyncKind::SyncKind && !self.looks_like_expression_after(i, true) {
                return self.primary(ctx);
            }
            self.advance();
            let expression = self.precedence(prec::POSTFIX, cascades, ConstCtx::NoneKind)?;
            return Ok(self.node(
                Node::AwaitExpression {
                    await_keyword: i as Tk,
                    expression,
                },
                i,
            ));
        }
        match self.kind_(i) {
            TokenKind::Op(Op::Plus) => return Err(self.error("'+' is not a prefix operator.")),
            TokenKind::Op(Op::Bang | Op::Tilde) => {
                if ctx != ConstCtx::NoneKind {
                    return Err(self.error("This expression isn't a valid constant pattern."));
                }
                self.advance();
                let operand = self.precedence(prec::POSTFIX, cascades, ConstCtx::NoneKind)?;
                return Ok(self.node(
                    Node::PrefixExpression {
                        operator: i as Tk,
                        operand,
                    },
                    i,
                ));
            }
            TokenKind::Op(Op::Minus) => {
                if ctx == ConstCtx::Explicit {
                    return Err(self.error("The expression can't be prefixed by 'const'."));
                }
                self.advance();
                let c = if ctx != ConstCtx::NoneKind {
                    ConstCtx::NumericOnly
                } else {
                    ConstCtx::NoneKind
                };
                let operand = self.precedence(prec::POSTFIX, cascades, c)?;
                return Ok(self.node(
                    Node::PrefixExpression {
                        operator: i as Tk,
                        operand,
                    },
                    i,
                ));
            }
            TokenKind::Op(Op::PlusPlus | Op::MinusMinus) => {
                self.advance();
                let operand = self.precedence(prec::POSTFIX, cascades, ConstCtx::NoneKind)?;
                if !self.assignable(operand) {
                    return Err(self.error_at(
                        self.start_(operand),
                        "Illegal assignment to non-assignable expression.",
                    ));
                }
                return Ok(self.node(
                    Node::PrefixExpression {
                        operator: i as Tk,
                        operand,
                    },
                    i,
                ));
            }
            _ => {}
        }
        if self.ident(i) {
            let mut id = i;
            if self.op(id + 1, Op::Dot) {
                id += 2;
            }
            if self.ident(id)
                && self.op(id + 1, Op::Lt)
                && let Some(a) = self.type_args_at(id, false)
                && self.op(a.end_index, Op::Dot)
                && (self.ident(a.end_index + 1) || self.kw(a.end_index + 1, Keyword::New))
                && self.op(a.end_index + 2, Op::LParen)
            {
                return self.creation(None);
            }
        }
        self.primary(ctx)
    }

    /// `parsePrimary` in expression context.
    fn primary(&mut self, ctx: ConstCtx) -> R<NodeId> {
        let i = self.pos;
        let explicit = ctx == ConstCtx::Explicit;
        let numeric = ctx == ConstCtx::NumericOnly;
        let invalid = |p: &Self| Err(p.error("This expression isn't a valid constant pattern."));
        match self.kind_(i) {
            TokenKind::Identifier => {
                if numeric {
                    return invalid(self);
                }
                self.send_or_function(ctx)
            }
            TokenKind::Integer => {
                if explicit {
                    return invalid(self);
                }
                self.check_number(i)?;
                self.advance();
                Ok(self.node(Node::IntegerLiteral { literal: i as Tk }, i))
            }
            TokenKind::Double => {
                if explicit {
                    return invalid(self);
                }
                self.check_number(i)?;
                self.advance();
                Ok(self.node(Node::DoubleLiteral { literal: i as Tk }, i))
            }
            TokenKind::Text => {
                if explicit || numeric {
                    return invalid(self);
                }
                self.string_literal()
            }
            TokenKind::Op(Op::Hash) => {
                if explicit || numeric {
                    return invalid(self);
                }
                self.symbol()
            }
            TokenKind::Reserved(k) => match k {
                Keyword::True | Keyword::False => {
                    if explicit || numeric {
                        return invalid(self);
                    }
                    self.advance();
                    Ok(self.node(Node::BooleanLiteral { literal: i as Tk }, i))
                }
                Keyword::Null => {
                    if explicit || numeric {
                        return invalid(self);
                    }
                    self.advance();
                    Ok(self.node(Node::NullLiteral { literal: i as Tk }, i))
                }
                Keyword::This => self.this_or_super(true),
                Keyword::Super => self.this_or_super(false),
                Keyword::New => self.creation(Some(i)),
                Keyword::Const => {
                    if explicit {
                        return Err(self.error("Duplicate 'const' keyword."));
                    }
                    self.const_expression()
                }
                Keyword::Switch if self.patterns_ => self.switch_expression(),
                Keyword::Assert => self.assert_expression(),
                _ => Err(self.error(&format!(
                    "Expected an expression, but got '{}'.",
                    self.txt(i)
                ))),
            },
            TokenKind::Op(Op::LParen) => self.parenthesized_function_or_record(ctx),
            TokenKind::Op(Op::LBracket) => self.list(None, None),
            TokenKind::Op(Op::LBrace) => self.set_or_map(None, None),
            TokenKind::Op(Op::Lt) => self.collection_or_generic_function(None),
            _ => Err(self.error(&format!(
                "Expected an expression, but got '{}'.",
                self.txt(i)
            ))),
        }
    }

    /// `parsePrimary` after `.`/`?.` (`expressionContinuation`).
    fn primary_continuation(&mut self, ctx: ConstCtx) -> R<NodeId> {
        if self.ident(self.pos) {
            return self.send_or_function(ctx);
        }
        if self.kw(self.pos, Keyword::New) {
            self.feature(self.pos, (2, 15), "constructor-tearoffs");
            return self.send(ctx, true);
        }
        Err(self.error(&format!(
            "Expected an identifier, but got '{}'.",
            self.txt(self.pos)
        )))
    }

    /// `parseSendOrFunctionLiteral`.
    fn send_or_function(&mut self, ctx: ConstCtx) -> R<NodeId> {
        if self.functions {
            let ti = self.type_at(self.pos, false, false, false);
            let name_ = ti.end_index;
            if self.ident(name_) {
                let nx = self.type_args_at(name_, false).map_or(name_ + 1, |a| a.end_index);
                if self.op(nx, Op::LParen) && self.looks_like_body(self.pair(nx) + 1) {
                    return Err(self.error_at(name_, "A function expression can't have a name."));
                }
            }
        }
        self.send(ctx, false)
    }

    /// `parseSend`.
    fn send(&mut self, ctx: ConstCtx, allows_new: bool) -> R<NodeId> {
        let i = self.pos;
        if !(self.ident(i) || (allows_new && self.kw(i, Keyword::New))) {
            return Err(self.error(&format!(
                "Expected an identifier, but got '{}'.",
                self.txt(i)
            )));
        }
        if self.ident(i) {
            if self.txt(i) == "await" && self.ident(i + 1) {
                return Err(self.error_at(i, "Unexpected 'await'."));
            }
            self.check_await_yield(i)?;
        }
        self.advance();
        let tp = self.type_args_at(i, false);
        let after = tp.map_or(i + 1, |a| a.end_index);
        let targs = if self.op(after, Op::LParen) && tp.is_some_and(|a| !a.recovered) {
            Some(self.type_args()?)
        } else {
            None
        };
        if ctx == ConstCtx::Explicit
            && !(self.op(self.pos, Op::Dot)
                || self.op(self.pos, Op::LParen)
                || self.op(self.pos, Op::Lt))
        {
            return Err(self.error_at(i, "The expression can't be prefixed by 'const'."));
        }
        let name_ = self.simple(i);
        if self.op(self.pos, Op::LParen) {
            let args = self.arguments_()?;
            return Ok(self.node(
                Node::MethodInvocation {
                    target: None,
                    operator: None,
                    method_name: name_,
                    type_arguments: targs,
                    argument_list: args,
                },
                i,
            ));
        }
        Ok(name_)
    }

    /// `parseThisExpression`/`parseSuperExpression`.
    fn this_or_super(&mut self, this_: bool) -> R<NodeId> {
        let i = self.advance();
        let mut e = if this_ {
            self.node(
                Node::ThisExpression {
                    this_keyword: i as Tk,
                },
                i,
            )
        } else {
            self.node(
                Node::SuperExpression {
                    super_keyword: i as Tk,
                },
                i,
            )
        };
        if self.op(self.pos, Op::LParen) {
            let args = self.arguments_()?;
            e = self.node(
                Node::FunctionExpressionInvocation {
                    function: e,
                    type_arguments: None,
                    argument_list: args,
                },
                i,
            );
        } else if !this_ && self.op(self.pos, Op::QuestionDot) {
            return Err(self.error("The operator '?.' cannot be used with 'super'."));
        }
        Ok(e)
    }

    /// `assert(...)` as an expression: the `AstBuilder` builds an invocation of
    /// a function named `assert` (the `AssertAsExpression` error has no code in
    /// the analyzer).
    fn assert_expression(&mut self) -> R<NodeId> {
        let (a, lp, condition, _comma, message, rp) = self.assert_parts()?;
        let function = self.simple(a);
        let mut arguments = vec![condition];
        arguments.extend(message);
        let argument_list = self.node(
            Node::ArgumentList {
                left_parenthesis: lp as Tk,
                arguments,
                right_parenthesis: rp as Tk,
            },
            lp,
        );
        Ok(self.node(
            Node::FunctionExpressionInvocation {
                function,
                type_arguments: None,
                argument_list,
            },
            a,
        ))
    }

    /// `parseArgumentsRest`.
    pub(super) fn arguments_(&mut self) -> R<NodeId> {
        let start_ = self.pos;
        let lp = self.expect_op(Op::LParen)?;
        let old = self.functions;
        self.functions = true;
        let mut args = Vec::new();
        let mut saw_named = false;
        loop {
            if self.op(self.pos, Op::RParen) {
                break;
            }
            if self.op(self.pos + 1, Op::Colon) {
                saw_named = true;
                args.push(self.named_expression()?);
            } else {
                if saw_named && self.version < (2, 17) {
                    return Err(
                        self.error("Positional arguments must occur before named arguments.")
                    );
                }
                args.push(self.expression()?);
            }
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.advance();
        }
        let rp = self.expect_op(Op::RParen)?;
        self.functions = old;
        Ok(self.node(
            Node::ArgumentList {
                left_parenthesis: lp as Tk,
                arguments: args,
                right_parenthesis: rp as Tk,
            },
            start_,
        ))
    }

    /// `name: expression` (named argument or record field).
    fn named_expression(&mut self) -> R<NodeId> {
        let start_ = self.pos;
        let n = self.expect_ident()?;
        let label = self.simple(n);
        let colon = self.expect_op(Op::Colon)?;
        let l = self.node(
            Node::Label {
                label,
                colon: colon as Tk,
            },
            start_,
        );
        let expression = self.expression()?;
        Ok(self.node(
            Node::NamedExpression {
                name: l,
                expression,
            },
            start_,
        ))
    }

    /// `parseNewExpression`, `parseImplicitCreationExpression` and the end of
    /// `parseConstExpression`: `keyword? ConstructorName arguments`.
    fn creation(&mut self, word_: Option<usize>) -> R<NodeId> {
        let start_ = word_.unwrap_or(self.pos);
        if word_.is_some() {
            self.advance();
        }
        let constructor_name = self.constructor_name_()?;
        if !self.op(self.pos, Op::LParen) {
            return Err(self.expected("("));
        }
        let argument_list = self.arguments_()?;
        Ok(self.node(
            Node::InstanceCreationExpression {
                keyword: word_.map(|p| p as Tk),
                constructor_name,
                argument_list,
            },
            start_,
        ))
    }

    /// `parseConstructorReference`.
    pub(super) fn constructor_name_(&mut self) -> R<NodeId> {
        let start_ = self.pos;
        let a = self.expect_ident()?;
        let mut prefix = None;
        let mut name_ = a;
        if self.op(self.pos, Op::Dot) {
            let d = self.advance();
            name_ = self.expect_ident_or_new()?;
            prefix = Some((a as Tk, d as Tk));
        }
        let type_arguments = if self.op(self.pos, Op::Lt) {
            if self.type_args_lt(self.pos, false).is_none() {
                return Err(self.error("Invalid type arguments."));
            }
            Some(self.type_args()?)
        } else {
            None
        };
        let kind_ = self.node(
            Node::NamedType {
                import_prefix: prefix,
                name2: name_ as Tk,
                type_arguments,
                question: None,
            },
            start_,
        );
        let mut period = None;
        let mut name = None;
        if self.op(self.pos, Op::Dot) {
            period = Some(self.advance() as Tk);
            let n = self.expect_ident_or_new()?;
            name = Some(self.simple(n));
        }
        Ok(self.node(
            Node::ConstructorName {
                type_: kind_,
                period,
                name,
            },
            start_,
        ))
    }

    /// `parseConstExpression`.
    fn const_expression(&mut self) -> R<NodeId> {
        let c = self.pos;
        match self.kind_(c + 1) {
            TokenKind::Op(Op::LBracket) => {
                self.advance();
                self.list(Some(c), None)
            }
            TokenKind::Op(Op::LParen) => {
                self.advance();
                self.parenthesized_or_record(Some(c), ConstCtx::NoneKind)
            }
            TokenKind::Op(Op::LBrace) => {
                self.advance();
                self.set_or_map(Some(c), None)
            }
            TokenKind::Op(Op::Lt) => {
                self.advance();
                self.collection_or_generic_function(Some(c))
            }
            _ => self.creation(Some(c)),
        }
    }

    /// `parseLiteralListSetMapOrFunction`.
    fn collection_or_generic_function(&mut self, constant: Option<usize>) -> R<NodeId> {
        let lt = self.pos;
        let Some(info) = self.type_args_lt(lt, true) else {
            return Err(self.error("Expected type arguments."));
        };
        if self.op(info.end_index, Op::LParen) {
            if constant.is_some() {
                return Err(self.error_at(constant.unwrap_or(lt), "Unexpected 'const'."));
            }
            let tp = self.type_params()?;
            let d = self.pair(self.pos) + 1;
            if !(self.op(d, Op::Arrow)
                || self.op(d, Op::LBrace)
                || self.word(d, "async")
                || self.word(d, "sync"))
            {
                return Err(self.error_at(d, "Unexpected token."));
            }
            return self.function_expression_(Some(tp), lt);
        }
        let ta = self.type_args()?;
        if self.op(self.pos, Op::LBrace) {
            return self.set_or_map(constant, Some(ta));
        }
        if self.op(self.pos, Op::LBracket) {
            return self.list(constant, Some(ta));
        }
        Err(self.expected("["))
    }

    /// Function expression (`parseFunctionExpression`) starting from the formal
    /// parameters.
    fn function_expression_(&mut self, type_parameters: Option<NodeId>, start_: usize) -> R<NodeId> {
        let parameters = self.formal_params(MemberKind::Local)?;
        let body = self.function_body_(true, false)?;
        Ok(self.node(
            Node::FunctionExpression {
                type_parameters,
                parameters: Some(parameters),
                body,
            },
            start_,
        ))
    }

    /// `parseParenthesizedExpressionFunctionLiteralOrRecordLiteral`.
    fn parenthesized_function_or_record(&mut self, ctx: ConstCtx) -> R<NodeId> {
        let opens = self.pos;
        if self.functions {
            let nx = self.pair(opens) + 1;
            let function = if self.op(nx, Op::Arrow) || self.op(nx, Op::LBrace) {
                true
            } else if self.word_(nx) {
                self.word(nx, "async")
                    || self.word(nx, "sync")
                    || self.op(nx + 1, Op::Arrow)
                    || self.op(nx + 1, Op::LBrace)
            } else {
                false
            };
            if function {
                return self.function_expression_(None, opens);
            }
        }
        let old = self.functions;
        self.functions = true;
        let r = self.parenthesized_or_record(None, ctx);
        self.functions = old;
        r
    }

    /// `parseParenthesizedExpressionOrRecordLiteral`.
    fn parenthesized_or_record(&mut self, constant: Option<usize>, ctx: ConstCtx) -> R<NodeId> {
        let start_ = constant.unwrap_or(self.pos);
        let lp = self.expect_op(Op::LParen)?;
        let mut fields_ = Vec::new();
        let mut record = constant.is_some();
        let mut valid_record = false;
        let mut illegal_comma = false;
        loop {
            if self.op(self.pos, Op::RParen) {
                if fields_.is_empty() {
                    record = true;
                }
                break;
            }
            if fields_.is_empty()
                && self.op(self.pos, Op::Comma)
                && self.op(self.pos + 1, Op::RParen)
            {
                illegal_comma = true;
                record = true;
                self.advance();
                break;
            }
            if self.op(self.pos + 1, Op::Colon) {
                record = true;
                valid_record = true;
                fields_.push(self.named_expression()?);
            } else {
                fields_.push(self.expression()?);
            }
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.advance();
            record = true;
            valid_record = true;
        }
        let rp = self.expect_op(Op::RParen)?;
        if record {
            if self.version < (3, 0) {
                return Err(self.error_at(
                    lp,
                    "This requires the 'records' language feature to be enabled.",
                ));
            }
            if fields_.is_empty() && illegal_comma {
                return Err(self.error_at(
                    rp,
                    "A record literal without fields can't have a trailing comma.",
                ));
            }
            if fields_.len() == 1 && !valid_record {
                return Err(self.error_at(
                    rp,
                    "A record literal with exactly one positional field requires a trailing comma.",
                ));
            }
            if fields_.is_empty() && ctx != ConstCtx::NoneKind {
                return Err(self.error_at(
                    lp,
                    "The empty record literal is not supported as a constant pattern.",
                ));
            }
            return Ok(self.node(
                Node::RecordLiteral {
                    const_keyword: constant.map(|c| c as Tk),
                    left_parenthesis: lp as Tk,
                    fields: fields_,
                    right_parenthesis: rp as Tk,
                },
                start_,
            ));
        }
        let expression = fields_[0];
        Ok(self.node(
            Node::ParenthesizedExpression {
                left_parenthesis: lp as Tk,
                expression,
                right_parenthesis: rp as Tk,
            },
            start_,
        ))
    }

    /// `parseLiteralString`: adjacent literals become `AdjacentStrings`.
    pub(super) fn string_literal(&mut self) -> R<NodeId> {
        let start_ = self.pos;
        let old = self.functions;
        self.functions = true;
        let mut parts_ = vec![self.single_text()?];
        while self.text(self.pos) {
            parts_.push(self.single_text()?);
        }
        self.functions = old;
        if parts_.len() == 1 {
            return Ok(parts_[0]);
        }
        Ok(self.node(Node::AdjacentStrings { strings: parts_ }, start_))
    }

    /// `parseSingleLiteralString`.
    fn single_text(&mut self) -> R<NodeId> {
        let start_ = self.pos;
        if !self.text(start_) {
            return Err(self.error("Expected a string literal."));
        }
        self.advance();
        let raw_ = self.txt(start_).starts_with('r');
        let single = !matches!(
            self.kind_(self.pos),
            TokenKind::ExpressionInterpolation | TokenKind::IdentifierInterpolation
        );
        if !raw_ {
            self.check_escapes(start_, true, single)?;
        }
        if single {
            return Ok(self.node(Node::SimpleStringLiteral { literal: start_ as Tk }, start_));
        }
        let first = self.node(
            Node::InterpolationString {
                contents: start_ as Tk,
            },
            start_,
        );
        let mut elements_ = vec![first];
        loop {
            let lb = self.pos;
            match self.kind_(lb) {
                TokenKind::ExpressionInterpolation => {
                    self.advance();
                    let expression = self.expression()?;
                    let rb = self.expect_op(Op::RBrace)?;
                    elements_.push(self.node(
                        Node::InterpolationExpression {
                            left_bracket: lb as Tk,
                            expression,
                            right_bracket: Some(rb as Tk),
                        },
                        lb,
                    ));
                }
                TokenKind::IdentifierInterpolation => {
                    self.advance();
                    let i = self.pos;
                    let expression = if self.kw(i, Keyword::This) || self.word(i, "this") {
                        self.advance();
                        self.node(
                            Node::ThisExpression {
                                this_keyword: i as Tk,
                            },
                            i,
                        )
                    } else {
                        if !self.ident(i)
                            || dartforge_frontend::token::Keyword::from_text(self.txt(i)).is_some()
                        {
                            return Err(self.error(&format!(
                                "'{}' can't be used as an identifier because it's a keyword.",
                                self.txt(i)
                            )));
                        }
                        self.check_await_yield(i)?;
                        self.advance();
                        self.simple(i)
                    };
                    elements_.push(self.node(
                        Node::InterpolationExpression {
                            left_bracket: lb as Tk,
                            expression,
                            right_bracket: None,
                        },
                        lb,
                    ));
                }
                _ => break,
            }
            let s = self.pos;
            if !self.text(s) {
                return Err(self.error("Expected a string."));
            }
            self.advance();
            let last = !matches!(
                self.kind_(self.pos),
                TokenKind::ExpressionInterpolation | TokenKind::IdentifierInterpolation
            );
            self.check_escapes(s, false, last)?;
            elements_.push(self.node(Node::InterpolationString { contents: s as Tk }, s));
        }
        Ok(self.node(
            Node::StringInterpolation {
                elements: elements_,
            },
            start_,
        ))
    }

    /// Malformed numeric literal, or one with digit separators (Dart 3.7 only),
    /// which the analyzer's scanner rejects.
    fn check_number(&self, i: usize) -> R<()> {
        let t = self.txt(i);
        if t.contains('_') {
            // `feature` needs `&mut self`; the version error is direct.
            if self.version < (3, 6) {
                return Err(self.error_at(
                    i,
                    "This requires the 'digit-separators' language feature to be enabled.",
                ));
            }
        }
        let b = t.as_bytes();
        let hexa = b.len() >= 2 && b[0] == b'0' && (b[1] == b'x' || b[1] == b'X');
        let digit = |c: u8| {
            if hexa {
                c.is_ascii_hexdigit()
            } else {
                c.is_ascii_digit()
            }
        };
        for (k, &c) in b.iter().enumerate() {
            if c != b'_' {
                continue;
            }
            // Each run of `_` needs digits on both sides.
            let before = b[..k].iter().rev().find(|&&x| x != b'_').copied();
            let after = b[k + 1..].iter().find(|&&x| x != b'_').copied();
            let ok =
                before.is_some_and(|x| digit(x) && !(hexa && k == 2)) && after.is_some_and(digit);
            if !ok || (hexa && k <= 2) {
                return Err(self.error_at(i, "Digit separators ('_') in a number literal can only be placed between two digits."));
            }
        }
        if b.len() >= 2 && b[0] == b'0' && (b[1] == b'x' || b[1] == b'X') {
            if b.len() == 2 {
                return Err(self.error_at(i, "Hexadecimal digit expected."));
            }
            return Ok(());
        }
        if let Some(e) = t.find(['e', 'E']) {
            let rest = t[e + 1..].trim_start_matches(['+', '-']);
            if rest.is_empty() || !rest.bytes().all(|c| c.is_ascii_digit()) {
                return Err(self.error_at(i, "Decimal digit expected."));
            }
        }
        Ok(())
    }

    /// Invalid escapes in a piece of non-raw string (fasta's `unescape`).
    /// `first`/`last` tell whether the piece has the opening/closing quotes.
    fn check_escapes(&self, i: usize, first: bool, last: bool) -> R<()> {
        let mut t = self.txt(i);
        if first {
            for q in ["\"\"\"", "'''", "\"", "'"] {
                if let Some(r) = t.strip_prefix(q) {
                    t = r;
                    if last {
                        t = t.strip_suffix(q).unwrap_or(t);
                    }
                    break;
                }
            }
        } else if last {
            for q in ["\"\"\"", "'''", "\"", "'"] {
                if let Some(r) = t.strip_suffix(q) {
                    t = r;
                    break;
                }
            }
        }
        if let Some(m) = escape_error(t.as_bytes()) {
            return Err(self.error_at(i, m));
        }
        Ok(())
    }

    /// `parseLiteralSymbol`.
    fn symbol(&mut self) -> R<NodeId> {
        let h = self.advance();
        let mut components_ = Vec::new();
        if self.user_definable_operator(self.pos).is_some() {
            components_.push(self.consume_user_definable_operator()? as Tk);
        } else if self.kw(self.pos, Keyword::Void) {
            components_.push(self.advance() as Tk);
        } else {
            components_.push(self.expect_ident()? as Tk);
            while self.op(self.pos, Op::Dot) {
                self.advance();
                if !self.word_(self.pos) {
                    return Err(self.error("Expected an identifier."));
                }
                components_.push(self.advance() as Tk);
            }
        }
        Ok(self.node(
            Node::SymbolLiteral {
                pound_sign: h as Tk,
                components: components_,
            },
            h,
        ))
    }

    /// `parseLiteralListSuffix`.
    fn list(&mut self, constant: Option<usize>, targs: Option<NodeId>) -> R<NodeId> {
        let start_ = constant.or(targs.map(|t| self.start_(t))).unwrap_or(self.pos);
        let lb = self.expect_op(Op::LBracket)?;
        let old = self.functions;
        self.functions = true;
        let mut elements_ = Vec::new();
        loop {
            if self.op(self.pos, Op::RBracket) {
                break;
            }
            elements_.push(self.element(false)?);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.advance();
        }
        let rb = self.expect_op(Op::RBracket)?;
        self.functions = old;
        Ok(self.node(
            Node::ListLiteral {
                const_keyword: constant.map(|c| c as Tk),
                type_arguments: targs,
                left_bracket: lb as Tk,
                elements: elements_,
                right_bracket: rb as Tk,
            },
            start_,
        ))
    }

    /// `parseLiteralSetOrMapSuffix`.
    fn set_or_map(&mut self, constant: Option<usize>, targs: Option<NodeId>) -> R<NodeId> {
        let start_ = constant.or(targs.map(|t| self.start_(t))).unwrap_or(self.pos);
        let lb = self.expect_op(Op::LBrace)?;
        let old = self.functions;
        self.functions = true;
        let mut elements_ = Vec::new();
        loop {
            if self.op(self.pos, Op::RBrace) {
                break;
            }
            elements_.push(self.element(true)?);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.advance();
        }
        let rb = self.expect_op(Op::RBrace)?;
        self.functions = old;
        Ok(self.node(
            Node::SetOrMapLiteral {
                const_keyword: constant.map(|c| c as Tk),
                type_arguments: targs,
                left_bracket: lb as Tk,
                elements: elements_,
                right_bracket: rb as Tk,
            },
            start_,
        ))
    }

    /// A collection element (`computeLiteralEntry` and the following steps).
    fn element(&mut self, map: bool) -> R<NodeId> {
        self.enter_()?;
        let r = self.inner_element(map);
        self.leave();
        r
    }

    fn inner_element(&mut self, map: bool) -> R<NodeId> {
        let i = self.pos;
        if self.kw(i, Keyword::If) {
            self.advance();
            let (lp, expression, case_clause, rp) = self.parenthesized_condition(self.patterns_)?;
            let then_element = self.element(map)?;
            let (else_keyword, else_element) = if self.kw(self.pos, Keyword::Else) {
                let e = self.advance();
                (Some(e as Tk), Some(self.element(map)?))
            } else {
                (None, None)
            };
            return Ok(self.node(
                Node::IfElement {
                    if_keyword: i as Tk,
                    left_parenthesis: lp as Tk,
                    expression,
                    case_clause,
                    right_parenthesis: rp as Tk,
                    then_element,
                    else_keyword,
                    else_element,
                },
                i,
            ));
        }
        if self.kw(i, Keyword::For) || (self.word(i, "await") && self.kw(i + 1, Keyword::For)) {
            let await_keyword = if self.word(i, "await") {
                Some(self.advance() as Tk)
            } else {
                None
            };
            let f = self.advance();
            let (lp, for_loop_parts, rp) = self.for_parts(await_keyword.map(|a| a as usize))?;
            let body = self.element(map)?;
            return Ok(self.node(
                Node::ForElement {
                    await_keyword,
                    for_keyword: f as Tk,
                    left_parenthesis: lp as Tk,
                    for_loop_parts,
                    right_parenthesis: rp as Tk,
                    body,
                },
                i,
            ));
        }
        if self.op(i, Op::Ellipsis) || self.op(i, Op::EllipsisQuestion) {
            self.advance();
            let expression = self.expression()?;
            return Ok(self.node(
                Node::SpreadElement {
                    spread_operator: i as Tk,
                    expression,
                },
                i,
            ));
        }
        if self.op(i, Op::Question) {
            return Err(self.error("Null-aware elements aren't supported in Dart 3.6."));
        }
        let e = self.expression()?;
        if map && self.op(self.pos, Op::Colon) {
            let s = self.advance();
            if self.op(self.pos, Op::Question) {
                return Err(self.error("Null-aware elements aren't supported in Dart 3.6."));
            }
            let value = self.expression()?;
            let start_ = self.start_(e);
            return Ok(self.node(
                Node::MapLiteralEntry {
                    key: e,
                    separator: s as Tk,
                    value,
                },
                start_,
            ));
        }
        Ok(e)
    }

    /// `ensureParenthesizedCondition`: `( expression (case pattern (when
    /// expression)?)? )`.
    pub(super) fn parenthesized_condition(
        &mut self,
        allows_case: bool,
    ) -> R<(usize, NodeId, Option<NodeId>, usize)> {
        let lp = self.expect_op(Op::LParen)?;
        let expression = self.expression()?;
        let mut case_clause = None;
        if allows_case && self.kw(self.pos, Keyword::Case) {
            let c = self.advance();
            let guarded_pattern = self.guarded_pattern_()?;
            case_clause = Some(self.node(
                Node::CaseClause {
                    case_keyword: c as Tk,
                    guarded_pattern,
                },
                c,
            ));
        }
        let rp = self.expect_op(Op::RParen)?;
        Ok((lp, expression, case_clause, rp))
    }

    /// Pattern with an optional `when` ([`Node::GuardedPattern`]).
    pub(super) fn guarded_pattern_(&mut self) -> R<NodeId> {
        let pattern = self.pattern(PatternCtx::Matching)?;
        let start_ = self.start_(pattern);
        let when_clause = if self.word(self.pos, "when") {
            let w = self.advance();
            let expression = self.expression()?;
            Some(self.node(
                Node::WhenClause {
                    when_keyword: w as Tk,
                    expression,
                },
                w,
            ))
        } else {
            None
        };
        Ok(self.node(
            Node::GuardedPattern {
                pattern,
                when_clause,
            },
            start_,
        ))
    }

    /// `parseSwitchExpression`.
    fn switch_expression(&mut self) -> R<NodeId> {
        let sw = self.advance();
        let old = self.functions;
        self.functions = true;
        let (lp, expression, _, rp) = self.parenthesized_condition(false)?;
        let lb = self.expect_op(Op::LBrace)?;
        let mut cases_ = Vec::new();
        if !self.op(self.pos, Op::RBrace) {
            self.functions = false;
            loop {
                if self.kw(self.pos, Keyword::Default) || self.kw(self.pos, Keyword::Case) {
                    return Err(self.error("'default'/'case' can't be used in a switch expression."));
                }
                let guarded_pattern = self.guarded_pattern_()?;
                let ci = self.start_(guarded_pattern);
                let arrow = self.expect_op(Op::Arrow)?;
                self.functions = true;
                let e = self.expression()?;
                self.functions = false;
                cases_.push(self.node(
                    Node::SwitchExpressionCase {
                        guarded_pattern,
                        arrow: arrow as Tk,
                        expression: e,
                    },
                    ci,
                ));
                let comma = self.optional_op(Op::Comma).is_some();
                if self.op(self.pos, Op::RBrace) {
                    break;
                }
                if !comma {
                    return Err(self.expected(","));
                }
            }
        }
        let rb = self.expect_op(Op::RBrace)?;
        self.functions = old;
        Ok(self.node(
            Node::SwitchExpression {
                switch_keyword: sw as Tk,
                left_parenthesis: lp as Tk,
                expression,
                right_parenthesis: rp as Tk,
                left_bracket: lb as Tk,
                cases: cases_,
                right_bracket: rb as Tk,
            },
            sw,
        ))
    }

    /// Constant pattern expression (`parsePrecedenceExpression` with
    /// `EQUALITY_PRECEDENCE`), used by patterns.
    pub(super) fn constant_pattern_expression(&mut self, ctx: ConstCtx) -> R<NodeId> {
        self.precedence(prec::EQUALITY, false, ctx)
    }

    /// Operand of a relational pattern (`BITWISE_OR_PRECEDENCE`).
    pub(super) fn relational_operand(&mut self) -> R<NodeId> {
        self.precedence(prec::BIT_OR, false, ConstCtx::NoneKind)
    }
}
