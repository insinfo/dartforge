//! Dart 3 patterns (`parsePattern`, `parsePrimaryPattern`,
//! `parseVariablePattern` and fasta's list, map, record and object suffixes),
//! with the `AstBuilder` nodes.

use dartforge_frontend::token::{Keyword, Op};

use super::super::tree::{Node, NodeId};
use super::super::token::{TokenKind, Tk};
use super::expressions::ConstCtx;
use super::lookahead::{Shape, TypeInfo};
use super::{Parser, R};

/// `PatternContext` do fasta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PatternCtx {
    /// Declaration (`var (a, b) = ...`): identifiers declare variables.
    Declaration,
    /// `case`, `if-case` and `switch` as an expression: identifiers are
    /// constants.
    Matching,
    /// Pattern assignment: identifiers are existing variables.
    Assignment,
}

impl Parser<'_> {
    /// `parsePattern`.
    pub(super) fn pattern(&mut self, ctx: PatternCtx) -> R<NodeId> {
        self.pattern_prec(ctx, 1)
    }

    fn pattern_prec(&mut self, ctx: PatternCtx, precedence: u8) -> R<NodeId> {
        self.enter_()?;
        let r = self.inner_pattern_prec(ctx, precedence);
        self.leave();
        r
    }

    fn inner_pattern_prec(&mut self, ctx: PatternCtx, precedence: u8) -> R<NodeId> {
        let mut p = self.primary_pattern(ctx)?;
        // Compound patterns start where the left operand starts (which may not be
        // the first token: see `WildcardPattern`).
        let start_ = self.start_(p);
        loop {
            let level = self.precedence_at(self.pos, true);
            if level < precedence {
                break;
            }
            let o = self.pos;
            if self.word(o, "as") {
                if !self.unary_pattern_ok {
                    return Err(
                        self.error_at(start_, "This pattern cannot appear inside a unary pattern.")
                    );
                }
                self.advance();
                let type_ = self.type_after_is_as()?;
                p = self.node(
                    Node::CastPattern {
                        pattern: p,
                        as_token: o as Tk,
                        type_,
                    },
                    start_,
                );
            } else if self.op(o, Op::Bang) || self.op(o, Op::Question) {
                if !self.unary_pattern_ok {
                    return Err(
                        self.error_at(start_, "This pattern cannot appear inside a unary pattern.")
                    );
                }
                self.advance();
                p = if self.op(o, Op::Bang) {
                    self.node(
                        Node::NullAssertPattern {
                            pattern: p,
                            operator: o as Tk,
                        },
                        start_,
                    )
                } else {
                    self.node(
                        Node::NullCheckPattern {
                            pattern: p,
                            operator: o as Tk,
                        },
                        start_,
                    )
                };
            } else if self.op(o, Op::AmpAmp) || self.op(o, Op::PipePipe) {
                self.advance();
                let right = self.pattern_prec(ctx, level + 1)?;
                p = if self.op(o, Op::AmpAmp) {
                    self.node(
                        Node::LogicalAndPattern {
                            left_operand: p,
                            operator: o as Tk,
                            right_operand: right,
                        },
                        start_,
                    )
                } else {
                    self.node(
                        Node::LogicalOrPattern {
                            left_operand: p,
                            operator: o as Tk,
                            right_operand: right,
                        },
                        start_,
                    )
                };
            } else {
                break;
            }
            self.unary_pattern_ok = false;
        }
        Ok(p)
    }

    /// `looksLikePatternStart`.
    fn looks_like_pattern_start(&self, i: usize) -> bool {
        matches!(
            self.kind_(i),
            TokenKind::Identifier
                | TokenKind::Double
                | TokenKind::Integer
                | TokenKind::Text
                | TokenKind::Op(
                    Op::Hash
                        | Op::LBrace
                        | Op::LParen
                        | Op::LBracket
                        | Op::Lt
                        | Op::LtEq
                        | Op::Gt
                        | Op::BangEq
                        | Op::EqEq
                )
                | TokenKind::Reserved(
                    Keyword::Null
                        | Keyword::False
                        | Keyword::True
                        | Keyword::Var
                        | Keyword::Final
                        | Keyword::Const
                )
        )
    }

    /// `parsePrimaryPattern`.
    fn primary_pattern(&mut self, ctx: PatternCtx) -> R<NodeId> {
        let start_ = self.pos;
        let ta = self.type_args_lt(start_, true);
        let next = ta.map_or(start_, |a| a.end_index);
        if self.op(next, Op::LBracket) || self.op(next, Op::LBrace) {
            let targs = if ta.is_some() {
                Some(self.type_args()?)
            } else {
                None
            };
            let r = if self.op(self.pos, Op::LBracket) {
                self.list_pattern(targs, ctx, start_)?
            } else {
                self.map_pattern(targs, ctx, start_)?
            };
            self.unary_pattern_ok = true;
            return Ok(r);
        }
        let i = start_;
        if self.kw(i, Keyword::Var) || self.kw(i, Keyword::Final) {
            self.unary_pattern_ok = true;
            return self.variable_pattern(ctx, None);
        }
        if self.op(i, Op::LParen) {
            let after = self.pair(i) + 1;
            if self.ident(after) || (self.op(after, Op::Question) && self.ident(after + 1)) {
                let ti = self.variable_pattern_type_at(i, true);
                if ti.shape == Shape::Complex && ti.record && !ti.recovered {
                    self.unary_pattern_ok = true;
                    return self.variable_pattern(ctx, Some(ti));
                }
            }
            let r = if self.op(i + 1, Op::RParen) {
                let lp = self.advance();
                let rp = self.advance();
                self.node(
                    Node::RecordPattern {
                        left_parenthesis: lp as Tk,
                        fields: Vec::new(),
                        right_parenthesis: rp as Tk,
                    },
                    lp,
                )
            } else {
                self.parenthesized_or_record_pattern(ctx)?
            };
            self.unary_pattern_ok = true;
            return Ok(r);
        }
        if self.kw(i, Keyword::Const) {
            self.advance();
            let expression = self.constant_pattern_expression(ConstCtx::Explicit)?;
            self.unary_pattern_ok = true;
            return Ok(self.node(
                Node::ConstantPattern {
                    const_keyword: Some(i as Tk),
                    expression,
                },
                i,
            ));
        }
        let relational = match self.kind_(i) {
            TokenKind::Op(Op::Lt | Op::LtEq | Op::EqEq | Op::BangEq) => true,
            TokenKind::Op(Op::Gt) => matches!(self.toks.composed_greater(i as Tk), None | Some(">=")),
            _ => false,
        };
        if relational {
            let o = if self.op(i, Op::Gt) && self.toks.composed_greater(i as Tk) == Some(">=") {
                self.merge_greater_at(i, ">=");
                self.pos += 2;
                i
            } else {
                self.advance()
            };
            let operand = self.relational_operand()?;
            self.unary_pattern_ok = false;
            return Ok(self.node(
                Node::RelationalPattern {
                    operator: o as Tk,
                    operand,
                },
                i,
            ));
        }
        let ti = self.variable_pattern_type_at(i, false);
        if !ti.is_none() {
            self.unary_pattern_ok = true;
            return self.variable_pattern(ctx, Some(ti));
        }
        if self.ident(i) {
            let first = i;
            let mut token = i;
            let mut second = None;
            if self.op(i + 1, Op::Dot) {
                if !self.ident(i + 2) {
                    return Err(self.error_at(i + 2, "Expected an identifier."));
                }
                second = Some(i + 2);
                token = i + 2;
            }
            let potential = self.type_args_at(token, false);
            let after = potential.map_or(token + 1, |a| a.end_index);
            if self.op(after, Op::LParen) && !potential.is_some_and(|a| a.recovered) {
                let r = self.object_pattern(first, second, potential.is_some(), ctx)?;
                self.unary_pattern_ok = true;
                return Ok(r);
            } else if second.is_none() {
                let name_ = self.txt(first);
                if ctx != PatternCtx::Matching || name_ == "_" {
                    self.unary_pattern_ok = true;
                    return self.variable_pattern(ctx, None);
                } else if name_ == "when" || name_ == "as" {
                    return Err(self.error_at(
                        first,
                        "A pattern can't refer to an identifier named 'when' or 'as'.",
                    ));
                }
            }
        }
        let expression = self.constant_pattern_expression(ConstCtx::Implicit)?;
        self.unary_pattern_ok = true;
        Ok(self.node(
            Node::ConstantPattern {
                const_keyword: None,
                expression,
            },
            i,
        ))
    }

    /// `parseVariablePattern`.
    fn variable_pattern(&mut self, ctx: PatternCtx, info: Option<TypeInfo>) -> R<NodeId> {
        let start_ = self.pos;
        let mut keyword = None;
        let mut type_ = None;
        let mut nu = false;
        if let Some(ti) = info.filter(|t| !t.is_none()) {
            type_ = Some(self.type_until(ti.end_index)?);
        } else if self.kw(self.pos, Keyword::Var) || self.kw(self.pos, Keyword::Final) {
            keyword = Some(self.advance());
            let parenthesized = self.op(self.pos, Op::LParen);
            let ti = self.variable_pattern_type_at(self.pos, parenthesized);
            if !ti.is_none() {
                type_ = Some(self.type_until(ti.end_index)?);
            }
        } else {
            nu = true;
        }
        let name_ = self.expect_ident()?;
        let text = self.txt(name_);
        match ctx {
            PatternCtx::Declaration => {
                if let Some(k) = keyword {
                    return Err(self.error_at(
                        k,
                        "Variable patterns in declaration context can't specify 'var' or 'final'.",
                    ));
                }
            }
            PatternCtx::Matching => {
                if type_.is_some() && keyword.is_some_and(|k| self.kw(k, Keyword::Var)) {
                    return Err(self.error_at(
                        keyword.unwrap_or(name_),
                        "Variables can't be declared using both 'var' and a type name.",
                    ));
                }
            }
            PatternCtx::Assignment => {
                if !nu {
                    return Err(self.error_at(
                        name_,
                        "Variable patterns in assignments can't declare variables.",
                    ));
                }
            }
        }
        if text == "_" {
            // `WildcardPatternImpl.beginToken` is `type?.beginToken ?? name`: the
            // `var`/`final` stays outside the node's range.
            let start_ = type_.map_or(name_, |t| self.start_(t));
            return Ok(self.node(
                Node::WildcardPattern {
                    keyword: keyword.map(|k| k as Tk),
                    type_,
                    name: name_ as Tk,
                },
                start_,
            ));
        }
        if text == "when" || text == "as" {
            return Err(self.error_at(name_, "A variable pattern can't be named 'when' or 'as'."));
        }
        self.check_await_yield(name_)?;
        if ctx == PatternCtx::Assignment && nu {
            return Ok(self.node(Node::AssignedVariablePattern { name: name_ as Tk }, start_));
        }
        Ok(self.node(
            Node::DeclaredVariablePattern {
                keyword: keyword.map(|k| k as Tk),
                type_,
                name: name_ as Tk,
            },
            start_,
        ))
    }

    /// `...` with an optional subpattern.
    fn pattern_rest(&mut self, ctx: PatternCtx) -> R<NodeId> {
        let o = self.advance();
        let pattern = if self.looks_like_pattern_start(self.pos) {
            Some(self.pattern(ctx)?)
        } else {
            None
        };
        Ok(self.node(
            Node::RestPatternElement {
                operator: o as Tk,
                pattern,
            },
            o,
        ))
    }

    /// `parseListPatternSuffix`.
    fn list_pattern(&mut self, targs: Option<NodeId>, ctx: PatternCtx, start_: usize) -> R<NodeId> {
        let lb = self.expect_op(Op::LBracket)?;
        let old = self.functions;
        self.functions = true;
        let mut elements_ = Vec::new();
        loop {
            if self.op(self.pos, Op::RBracket) {
                break;
            }
            if self.op(self.pos, Op::Ellipsis) {
                elements_.push(self.pattern_rest(ctx)?);
            } else {
                elements_.push(self.pattern(ctx)?);
            }
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.advance();
        }
        let rb = self.expect_op(Op::RBracket)?;
        self.functions = old;
        Ok(self.node(
            Node::ListPattern {
                type_arguments: targs,
                left_bracket: lb as Tk,
                elements: elements_,
                right_bracket: rb as Tk,
            },
            start_,
        ))
    }

    /// `parseMapPatternSuffix`.
    fn map_pattern(&mut self, targs: Option<NodeId>, ctx: PatternCtx, start_: usize) -> R<NodeId> {
        let lb = self.expect_op(Op::LBrace)?;
        let old = self.functions;
        self.functions = true;
        let mut elements_ = Vec::new();
        if !self.op(self.pos, Op::RBrace) {
            loop {
                if self.op(self.pos, Op::Ellipsis) {
                    elements_.push(self.pattern_rest(ctx)?);
                } else {
                    let key = self.expression()?;
                    let s = self.expect_op(Op::Colon)?;
                    let value = self.pattern(ctx)?;
                    let ki = self.start_(key);
                    elements_.push(self.node(
                        Node::MapPatternEntry {
                            key,
                            separator: s as Tk,
                            value,
                        },
                        ki,
                    ));
                }
                let comma = self.optional_op(Op::Comma).is_some();
                if self.op(self.pos, Op::RBrace) {
                    break;
                }
                if !comma {
                    return Err(self.expected("}"));
                }
            }
        }
        let rb = self.expect_op(Op::RBrace)?;
        self.functions = old;
        Ok(self.node(
            Node::MapPattern {
                type_arguments: targs,
                left_bracket: lb as Tk,
                elements: elements_,
                right_bracket: rb as Tk,
            },
            start_,
        ))
    }

    /// Pattern field with an optional name (`(name)? :`).
    fn pattern_field(&mut self, ctx: PatternCtx, allows_parenthesized_name: bool) -> R<(NodeId, bool)> {
        let fi = self.pos;
        let mut name_ = None;
        if self.op(self.pos, Op::Colon) {
            let c = self.advance();
            name_ = Some(self.node(
                Node::PatternFieldName {
                    name: None,
                    colon: c as Tk,
                },
                c,
            ));
        } else if (allows_parenthesized_name || !self.op(self.pos, Op::LParen))
            && self.op(self.pos + 1, Op::Colon)
        {
            let n = self.expect_ident()?;
            let c = self.advance();
            name_ = Some(self.node(
                Node::PatternFieldName {
                    name: Some(n as Tk),
                    colon: c as Tk,
                },
                n,
            ));
        }
        let has_name = name_.is_some();
        let pattern = self.pattern(ctx)?;
        if has_name {
            return Ok((
                self.node(
                    Node::PatternField {
                        name: name_,
                        pattern,
                    },
                    fi,
                ),
                true,
            ));
        }
        Ok((pattern, false))
    }

    /// `parseParenthesizedPatternOrRecordPattern`.
    fn parenthesized_or_record_pattern(&mut self, ctx: PatternCtx) -> R<NodeId> {
        let start_ = self.pos;
        let lp = self.expect_op(Op::LParen)?;
        let old = self.functions;
        self.functions = true;
        let mut fields_: Vec<NodeId> = Vec::new();
        let mut record = false;
        let mut valid = false;
        loop {
            if (!fields_.is_empty() || record) && self.op(self.pos, Op::RParen) {
                break;
            }
            let (c, named) = self.pattern_field(ctx, false)?;
            let c = if record && !named {
                self.unnamed_field(c)
            } else {
                c
            };
            if named {
                record = true;
                valid = true;
            }
            fields_.push(c);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            if !record {
                let last = fields_.len() - 1;
                fields_[last] = self.unnamed_field(fields_[last]);
            }
            record = true;
            valid = true;
            self.advance();
        }
        let rp = self.expect_op(Op::RParen)?;
        self.functions = old;
        if record {
            if fields_.len() == 1 && !valid {
                return Err(self.error_at(
                    rp,
                    "A record pattern with exactly one positional field requires a trailing comma.",
                ));
            }
            return Ok(self.node(
                Node::RecordPattern {
                    left_parenthesis: lp as Tk,
                    fields: fields_,
                    right_parenthesis: rp as Tk,
                },
                start_,
            ));
        }
        let pattern = fields_[0];
        Ok(self.node(
            Node::ParenthesizedPattern {
                left_parenthesis: lp as Tk,
                pattern,
                right_parenthesis: rp as Tk,
            },
            start_,
        ))
    }

    /// Wraps a positional pattern in an unnamed [`Node::PatternField`].
    fn unnamed_field(&mut self, pattern: NodeId) -> NodeId {
        if matches!(self.tree_.node(pattern), Node::PatternField { .. }) {
            return pattern;
        }
        let (a, b) = (self.start_(pattern), self.end(pattern));
        self.node_between(
            Node::PatternField {
                name: None,
                pattern,
            },
            a,
            b,
        )
    }

    /// `objectPattern ::= typeName typeArguments? '(' patternFields? ')'`.
    fn object_pattern(
        &mut self,
        first: usize,
        second: Option<usize>,
        with_args: bool,
        ctx: PatternCtx,
    ) -> R<NodeId> {
        let start_ = self.pos;
        let mut import_prefix = None;
        let name_ = match second {
            Some(s) => {
                import_prefix = Some((first as Tk, (first + 1) as Tk));
                s
            }
            None => first,
        };
        self.pos = name_ + 1;
        let type_arguments = if with_args {
            Some(self.type_args()?)
        } else {
            None
        };
        let type_ = self.node(
            Node::NamedType {
                import_prefix,
                name2: name_ as Tk,
                type_arguments,
                question: None,
            },
            start_,
        );
        let lp = self.expect_op(Op::LParen)?;
        let old = self.functions;
        self.functions = true;
        let mut fields_ = Vec::new();
        loop {
            if self.op(self.pos, Op::RParen) {
                break;
            }
            let (c, _) = self.pattern_field(ctx, true)?;
            let c = self.unnamed_field(c);
            fields_.push(c);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.advance();
        }
        let rp = self.expect_op(Op::RParen)?;
        self.functions = old;
        Ok(self.node(
            Node::ObjectPattern {
                type_,
                left_parenthesis: lp as Tk,
                fields: fields_,
                right_parenthesis: rp as Tk,
            },
            start_,
        ))
    }
}
