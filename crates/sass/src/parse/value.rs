use std::{iter::Iterator, marker::PhantomData, sync::Arc};

use codemap::Spanned;

use crate::{
    ast::*,
    color::{Color, ColorFormat, NAMED_COLORS},
    common::{unvendor, BinaryOp, Brackets, Identifier, ListSeparator, QuoteKind, UnaryOp},
    error::SassResult,
    unit::Unit,
    utils::as_hex,
    value::Number,
    ContextFlags, Token,
};

use super::StylesheetParser;

pub(crate) type Predicate<'c, P> = &'c dyn Fn(&mut P) -> SassResult<bool>;

fn is_hex_color(interpolation: &Interpolation) -> bool {
    if let Some(plain) = interpolation.as_plain() {
        if ![3, 4, 6, 8].contains(&plain.len()) {
            return false;
        }

        return plain.chars().all(|c| c.is_ascii_hexdigit());
    }

    false
}

pub(crate) struct ValueParser<'a, 'c, P: StylesheetParser<'a>> {
    comma_expressions: Option<Vec<Spanned<AstExpr>>>,
    space_expressions: Option<Vec<Spanned<AstExpr>>>,
    binary_operators: Option<Vec<BinaryOp>>,
    operands: Option<Vec<Spanned<AstExpr>>>,
    allow_slash: bool,
    single_expression: Option<Spanned<AstExpr>>,
    start: usize,
    inside_bracketed_list: bool,
    single_equals: bool,
    parse_until: Option<Predicate<'c, P>>,
    _a: PhantomData<&'a ()>,
}

impl<'a, 'c, P: StylesheetParser<'a>> ValueParser<'a, 'c, P> {
    pub fn parse_expression(
        parser: &mut P,
        parse_until: Option<Predicate<'c, P>>,
        inside_bracketed_list: bool,
        single_equals: bool,
    ) -> SassResult<Spanned<AstExpr>> {
        let start = parser.toks().cursor();
        let mut value_parser = Self::new(parser, parse_until, inside_bracketed_list, single_equals);

        if let Some(parse_until) = value_parser.parse_until {
            if parse_until(parser)? {
                return Err(("Expected expression.", parser.toks().current_span()).into());
            }
        }

        if value_parser.inside_bracketed_list {
            let bracket_start = parser.toks().cursor();

            parser.expect_char('[')?;
            parser.whitespace()?;

            if parser.scan_char(']') {
                return Ok(AstExpr::List(ListExpr {
                    elems: Vec::new(),
                    separator: ListSeparator::Undecided,
                    brackets: Brackets::Bracketed,
                })
                .span(parser.toks_mut().span_from(bracket_start)));
            }
        };

        value_parser.start = parser.toks().cursor();

        value_parser.single_expression = Some(value_parser.parse_single_expression(parser)?);

        let mut value = value_parser.parse_value(parser)?;
        value.span = parser.toks_mut().span_from(start);

        Ok(value)
    }

    pub fn new(
        parser: &mut P,
        parse_until: Option<Predicate<'c, P>>,
        inside_bracketed_list: bool,
        single_equals: bool,
    ) -> Self {
        Self {
            comma_expressions: None,
            space_expressions: None,
            binary_operators: None,
            operands: None,
            allow_slash: true,
            start: parser.toks().cursor(),
            single_expression: None,
            parse_until,
            inside_bracketed_list,
            single_equals,
            _a: PhantomData,
        }
    }

    /// Parse a value from a stream of tokens
    ///
    /// This function will cease parsing if the predicate returns true.
    pub(crate) fn parse_value(&mut self, parser: &mut P) -> SassResult<Spanned<AstExpr>> {
        parser.whitespace()?;

        let start = parser.toks().cursor();

        let was_in_parens = parser.flags().in_parens();

        loop {
            parser.whitespace()?;

            if let Some(parse_until) = self.parse_until {
                if parse_until(parser)? {
                    break;
                }
            }

            let first = parser.toks().peek();

            match first {
                Some(Token { kind: '(', .. }) => {
                    let expr = self.parse_paren_expr(parser)?;
                    self.add_single_expression(expr, parser)?;
                }
                Some(Token { kind: '[', .. }) => {
                    let expr = parser.parse_expression(None, Some(true), None)?;
                    self.add_single_expression(expr, parser)?;
                }
                Some(Token { kind: '$', .. }) => {
                    let expr = Self::parse_variable(parser)?;
                    self.add_single_expression(expr, parser)?;
                }
                Some(Token { kind: '&', .. }) => {
                    let expr = Self::parse_selector(parser)?;
                    self.add_single_expression(expr, parser)?;
                }
                Some(Token { kind: '"', .. }) | Some(Token { kind: '\'', .. }) => {
                    let expr = parser
                        .parse_interpolated_string()?
                        .map_node(|s| AstExpr::String(s, parser.toks_mut().span_from(start)));
                    self.add_single_expression(expr, parser)?;
                }
                Some(Token { kind: '#', .. }) => {
                    let expr = self.parse_hash(parser)?;
                    self.add_single_expression(expr, parser)?;
                }
                Some(Token { kind: '=', .. }) => {
                    parser.toks_mut().next();
                    if self.single_equals
                        && !matches!(parser.toks().peek(), Some(Token { kind: '=', .. }))
                    {
                        self.add_operator(
                            Spanned {
                                node: BinaryOp::SingleEq,
                                span: parser.toks_mut().span_from(start),
                            },
                            parser,
                        )?;
                    } else {
                        parser.expect_char('=')?;
                        self.add_operator(
                            Spanned {
                                node: BinaryOp::Equal,
                                span: parser.toks_mut().span_from(start),
                            },
                            parser,
                        )?;
                    }
                }
                Some(Token { kind: '!', .. }) => match parser.toks().peek_n(1) {
                    Some(Token { kind: '=', .. }) => {
                        parser.toks_mut().next();
                        parser.toks_mut().next();
                        self.add_operator(
                            Spanned {
                                node: BinaryOp::NotEqual,
                                span: parser.toks_mut().span_from(start),
                            },
                            parser,
                        )?;
                    }
                    Some(Token { kind, .. })
                        if kind.is_ascii_whitespace() || kind == 'i' || kind == 'I' =>
                    {
                        let expr = Self::parse_important_expr(parser)?;
                        self.add_single_expression(expr, parser)?;
                    }
                    None => {
                        let expr = Self::parse_important_expr(parser)?;
                        self.add_single_expression(expr, parser)?;
                    }
                    Some(..) => break,
                },
                Some(Token { kind: '<', .. }) => {
                    parser.toks_mut().next();
                    self.add_operator(
                        Spanned {
                            node: if parser.scan_char('=') {
                                BinaryOp::LessThanEqual
                            } else {
                                BinaryOp::LessThan
                            },
                            span: parser.toks_mut().span_from(start),
                        },
                        parser,
                    )?;
                }
                Some(Token { kind: '>', .. }) => {
                    parser.toks_mut().next();
                    self.add_operator(
                        Spanned {
                            node: if parser.scan_char('=') {
                                BinaryOp::GreaterThanEqual
                            } else {
                                BinaryOp::GreaterThan
                            },
                            span: parser.toks_mut().span_from(start),
                        },
                        parser,
                    )?;
                }
                Some(Token { kind: '*', .. }) => {
                    parser.toks_mut().next();
                    self.add_operator(
                        Spanned {
                            node: BinaryOp::Mul,
                            span: parser.toks().current_span(),
                        },
                        parser,
                    )?;
                }
                Some(Token { kind: '+', .. }) => {
                    if self.single_expression.is_none() {
                        let expr = self.parse_unary_operation(parser)?;
                        self.add_single_expression(expr, parser)?;
                    } else {
                        parser.toks_mut().next();
                        self.add_operator(
                            Spanned {
                                node: BinaryOp::Plus,
                                span: parser.toks_mut().span_from(start),
                            },
                            parser,
                        )?;
                    }
                }
                Some(Token { kind: '-', .. }) => {
                    if matches!(
                        parser.toks().peek_n(1),
                        Some(Token {
                            kind: '0'..='9' | '.',
                            ..
                        })
                    ) && (self.single_expression.is_none()
                        || matches!(
                            parser.toks_mut().peek_previous(),
                            Some(Token {
                                kind: ' ' | '\t' | '\n' | '\r',
                                ..
                            })
                        ))
                    {
                        let expr = ValueParser::parse_number(parser)?;
                        self.add_single_expression(expr, parser)?;
                    } else if parser.looking_at_interpolated_identifier() {
                        let expr = self.parse_identifier_like(parser)?;
                        self.add_single_expression(expr, parser)?;
                    } else if self.single_expression.is_none() {
                        let expr = self.parse_unary_operation(parser)?;
                        self.add_single_expression(expr, parser)?;
                    } else {
                        parser.toks_mut().next();
                        self.add_operator(
                            Spanned {
                                node: BinaryOp::Minus,
                                span: parser.toks_mut().span_from(start),
                            },
                            parser,
                        )?;
                    }
                }
                Some(Token { kind: '/', .. }) => {
                    if self.single_expression.is_none() {
                        let expr = self.parse_unary_operation(parser)?;
                        self.add_single_expression(expr, parser)?;
                    } else {
                        parser.toks_mut().next();
                        self.add_operator(
                            Spanned {
                                node: BinaryOp::Div,
                                span: parser.toks_mut().span_from(start),
                            },
                            parser,
                        )?;
                    }
                }
                Some(Token { kind: '%', .. }) => {
                    parser.toks_mut().next();
                    self.add_operator(
                        Spanned {
                            node: BinaryOp::Rem,
                            span: parser.toks().current_span(),
                        },
                        parser,
                    )?;
                }
                Some(Token {
                    kind: '0'..='9', ..
                }) => {
                    let expr = ValueParser::parse_number(parser)?;
                    self.add_single_expression(expr, parser)?;
                }
                Some(Token { kind: '.', .. }) => {
                    if matches!(parser.toks().peek_n(1), Some(Token { kind: '.', .. })) {
                        break;
                    }
                    let expr = ValueParser::parse_number(parser)?;
                    self.add_single_expression(expr, parser)?;
                }
                Some(Token { kind: 'a', .. }) => {
                    if !parser.is_plain_css() && parser.scan_identifier("and", false)? {
                        self.add_operator(
                            Spanned {
                                node: BinaryOp::And,
                                span: parser.toks_mut().span_from(start),
                            },
                            parser,
                        )?;
                    } else {
                        let expr = self.parse_identifier_like(parser)?;
                        self.add_single_expression(expr, parser)?;
                    }
                }
                Some(Token { kind: 'o', .. }) => {
                    if !parser.is_plain_css() && parser.scan_identifier("or", false)? {
                        self.add_operator(
                            Spanned {
                                node: BinaryOp::Or,
                                span: parser.toks_mut().span_from(start),
                            },
                            parser,
                        )?;
                    } else {
                        let expr = self.parse_identifier_like(parser)?;
                        self.add_single_expression(expr, parser)?;
                    }
                }
                Some(Token { kind: 'u', .. }) | Some(Token { kind: 'U', .. }) => {
                    if matches!(parser.toks().peek_n(1), Some(Token { kind: '+', .. })) {
                        let expr = Self::parse_unicode_range(parser)?;
                        self.add_single_expression(expr, parser)?;
                    } else {
                        let expr = self.parse_identifier_like(parser)?;
                        self.add_single_expression(expr, parser)?;
                    }
                }
                Some(Token {
                    kind: 'b'..='z', ..
                })
                | Some(Token {
                    kind: 'A'..='Z', ..
                })
                | Some(Token { kind: '_', .. })
                | Some(Token { kind: '\\', .. })
                | Some(Token {
                    kind: '\u{80}'..=std::char::MAX,
                    ..
                }) => {
                    let expr = self.parse_identifier_like(parser)?;
                    self.add_single_expression(expr, parser)?;
                }
                Some(Token { kind: ',', .. }) => {
                    // If we discover we're parsing a list whose first element is a
                    // division operation, and we're in parentheses, reparse outside of a
                    // paren context. This ensures that `(1/2, 1)` doesn't perform division
                    // on its first element.
                    if parser.flags().in_parens() {
                        parser.flags_mut().set(ContextFlags::IN_PARENS, false);
                        if self.allow_slash {
                            self.reset_state(parser)?;
                            continue;
                        }
                        // todo: does this branch ever get hit
                    }

                    if self.single_expression.is_none() {
                        return Err(("Expected expression.", parser.toks().current_span()).into());
                    }

                    self.resolve_space_expressions(parser)?;

                    // [resolveSpaceExpressions] can modify [singleExpression_], but it
                    // can't set it to null`.
                    self.comma_expressions
                        .get_or_insert_with(Default::default)
                        .push(self.single_expression.take().unwrap());
                    parser.toks_mut().next();
                    self.allow_slash = true;
                }
                Some(..) | None => break,
            }
        }

        if self.inside_bracketed_list {
            parser.expect_char(']')?;
        }

        if self.comma_expressions.is_some() {
            self.resolve_space_expressions(parser)?;

            parser
                .flags_mut()
                .set(ContextFlags::IN_PARENS, was_in_parens);

            if let Some(single_expression) = self.single_expression.take() {
                self.comma_expressions
                    .as_mut()
                    .unwrap()
                    .push(single_expression);
            }

            Ok(AstExpr::List(ListExpr {
                elems: self.comma_expressions.take().unwrap(),
                separator: ListSeparator::Comma,
                brackets: if self.inside_bracketed_list {
                    Brackets::Bracketed
                } else {
                    Brackets::None
                },
            })
            .span(parser.toks_mut().span_from(start)))
        } else if self.inside_bracketed_list && self.space_expressions.is_some() {
            self.resolve_operations(parser)?;

            self.space_expressions
                .as_mut()
                .unwrap()
                .push(self.single_expression.take().unwrap());

            Ok(AstExpr::List(ListExpr {
                elems: self.space_expressions.take().unwrap(),
                separator: ListSeparator::Space,
                brackets: Brackets::Bracketed,
            })
            .span(parser.toks_mut().span_from(start)))
        } else {
            self.resolve_space_expressions(parser)?;

            if self.inside_bracketed_list {
                return Ok(AstExpr::List(ListExpr {
                    elems: vec![self.single_expression.take().unwrap()],
                    separator: ListSeparator::Undecided,
                    brackets: Brackets::Bracketed,
                })
                .span(parser.toks_mut().span_from(start)));
            }

            Ok(self.single_expression.take().unwrap())
        }
    }

    fn parse_single_expression(&mut self, parser: &mut P) -> SassResult<Spanned<AstExpr>> {
        let start = parser.toks().cursor();
        let first = parser.toks().peek();

        match first {
            Some(Token { kind: '(', .. }) => self.parse_paren_expr(parser),
            Some(Token { kind: '/', .. }) => self.parse_unary_operation(parser),
            Some(Token { kind: '[', .. }) => Self::parse_expression(parser, None, true, false),
            Some(Token { kind: '$', .. }) => Self::parse_variable(parser),
            Some(Token { kind: '&', .. }) => Self::parse_selector(parser),
            Some(Token { kind: '"', .. }) | Some(Token { kind: '\'', .. }) => Ok(parser
                .parse_interpolated_string()?
                .map_node(|s| AstExpr::String(s, parser.toks_mut().span_from(start)))),
            Some(Token { kind: '#', .. }) => self.parse_hash(parser),
            Some(Token { kind: '+', .. }) => self.parse_plus_expr(parser),
            Some(Token { kind: '-', .. }) => self.parse_minus_expr(parser),
            Some(Token { kind: '!', .. }) => Self::parse_important_expr(parser),
            // `_percentExpression`: um `%` solto é o texto `%`.
            Some(Token { kind: '%', .. }) => {
                parser.toks_mut().next();
                let span = parser.toks_mut().span_from(start);
                Ok(AstExpr::String(
                    StringExpr(Interpolation::new_plain("%".to_owned()), QuoteKind::None),
                    span,
                )
                .span(span))
            }
            Some(Token { kind: 'u', .. }) | Some(Token { kind: 'U', .. }) => {
                if matches!(parser.toks().peek_n(1), Some(Token { kind: '+', .. })) {
                    Self::parse_unicode_range(parser)
                } else {
                    self.parse_identifier_like(parser)
                }
            }
            Some(Token {
                kind: '0'..='9', ..
            })
            | Some(Token { kind: '.', .. }) => ValueParser::parse_number(parser),
            Some(Token {
                kind: 'a'..='z', ..
            })
            | Some(Token {
                kind: 'A'..='Z', ..
            })
            | Some(Token { kind: '_', .. })
            | Some(Token { kind: '\\', .. })
            | Some(Token {
                kind: '\u{80}'..=std::char::MAX,
                ..
            }) => self.parse_identifier_like(parser),
            Some(..) | None => Err((
                "Expected expression.",
                parser.toks_mut().span_from(self.start),
            )
                .into()),
        }
    }

    fn resolve_one_operation(&mut self, parser: &mut P) -> SassResult<()> {
        let operator = self.binary_operators.as_mut().unwrap().pop().unwrap();
        let operands = self.operands.as_mut().unwrap();

        let left = operands.pop().unwrap();
        let right = match self.single_expression.take() {
            Some(val) => val,
            None => return Err(("Expected expression.", left.span).into()),
        };

        let span = left.span.merge(right.span);

        if self.allow_slash
            && !parser.flags().in_parens()
            && operator == BinaryOp::Div
            && left.node.is_slash_operand()
            && right.node.is_slash_operand()
        {
            self.single_expression = Some(AstExpr::slash(left.node, right.node, span).span(span));
        } else {
            self.single_expression = Some(
                AstExpr::BinaryOp(Arc::new(BinaryOpExpr {
                    lhs: left.node,
                    op: operator,
                    rhs: right.node,
                    allows_slash: false,
                    span,
                    lhs_span: left.span,
                    rhs_span: right.span,
                }))
                .span(span),
            );
            self.allow_slash = false;
        }

        Ok(())
    }

    fn resolve_operations(&mut self, parser: &mut P) -> SassResult<()> {
        loop {
            let should_break = match self.binary_operators.as_ref() {
                Some(bin) => bin.is_empty(),
                None => true,
            };

            if should_break {
                break;
            }

            self.resolve_one_operation(parser)?;
        }

        Ok(())
    }

    fn add_single_expression(
        &mut self,
        expression: Spanned<AstExpr>,
        parser: &mut P,
    ) -> SassResult<()> {
        if self.single_expression.is_some() {
            // If we discover we're parsing a list whose first element is a division
            // operation, and we're in parentheses, reparse outside of a paren
            // context. This ensures that `(1/2 1)` doesn't perform division on its
            // first element.
            if parser.flags().in_parens() {
                parser.flags_mut().set(ContextFlags::IN_PARENS, false);

                if self.allow_slash {
                    self.reset_state(parser)?;

                    return Ok(());
                }
            }

            if self.space_expressions.is_none() {
                self.space_expressions = Some(Vec::new());
            }

            self.resolve_operations(parser)?;

            self.space_expressions
                .as_mut()
                .unwrap()
                .push(self.single_expression.take().unwrap());

            self.allow_slash = true;
        }

        self.single_expression = Some(expression);

        Ok(())
    }

    fn add_operator(&mut self, op: Spanned<BinaryOp>, parser: &mut P) -> SassResult<()> {
        if parser.is_plain_css() && op.node != BinaryOp::Div && op.node != BinaryOp::SingleEq {
            return Err(("Operators aren't allowed in plain CSS.", op.span).into());
        }

        self.allow_slash = self.allow_slash && op.node == BinaryOp::Div;

        if self.binary_operators.is_none() {
            self.binary_operators = Some(Vec::new());
        }

        if self.operands.is_none() {
            self.operands = Some(Vec::new());
        }

        while let Some(last_op) = self.binary_operators.as_ref().unwrap_or(&Vec::new()).last() {
            if last_op.precedence() < op.precedence() {
                break;
            }

            self.resolve_one_operation(parser)?;
        }
        self.binary_operators
            .get_or_insert_with(Default::default)
            .push(op.node);

        match self.single_expression.take() {
            Some(expr) => {
                self.operands.get_or_insert_with(Vec::new).push(expr);
            }
            None => return Err(("Expected expression.", op.span).into()),
        }

        parser.whitespace()?;

        self.single_expression = Some(self.parse_single_expression(parser)?);

        Ok(())
    }

    fn resolve_space_expressions(&mut self, parser: &mut P) -> SassResult<()> {
        self.resolve_operations(parser)?;

        if let Some(mut space_expressions) = self.space_expressions.take() {
            let single_expression = match self.single_expression.take() {
                Some(val) => val,
                None => return Err(("Expected expression.", parser.toks().current_span()).into()),
            };

            let span = single_expression.span;

            space_expressions.push(single_expression);

            self.single_expression = Some(
                AstExpr::List(ListExpr {
                    elems: space_expressions,
                    separator: ListSeparator::Space,
                    brackets: Brackets::None,
                })
                .span(span),
            );
        }

        Ok(())
    }

    fn parse_map(
        parser: &mut P,
        first: Spanned<AstExpr>,
        start: usize,
    ) -> SassResult<Spanned<AstExpr>> {
        let mut pairs = vec![(first, parser.parse_expression_until_comma(false)?.node)];

        while parser.scan_char(',') {
            parser.whitespace()?;
            if !parser.looking_at_expression() {
                break;
            }

            let key = parser.parse_expression_until_comma(false)?;
            parser.expect_char(':')?;
            parser.whitespace()?;
            let value = parser.parse_expression_until_comma(false)?;
            pairs.push((key, value.node));
        }

        parser.expect_char(')')?;

        Ok(AstExpr::Map(AstSassMap(pairs)).span(parser.toks_mut().span_from(start)))
    }

    fn parse_paren_expr(&mut self, parser: &mut P) -> SassResult<Spanned<AstExpr>> {
        let start = parser.toks().cursor();
        if parser.is_plain_css() {
            return Err((
                "Parentheses aren't allowed in plain CSS.",
                parser.toks().current_span(),
            )
                .into());
        }

        // `_calculationValue` do 1.66: `(` com interpolação no nível de cima
        // é o texto cru (`CalculationInterpolation`).
        if parser.flags().in_calc_166() {
            parser.expect_char('(')?;
            if Self::contains_calculation_interpolation_166(parser)? {
                let texto = parser.parse_interpolated_declaration_value(false, false, true)?;
                parser.whitespace()?;
                parser.expect_char(')')?;
                let span = parser.toks_mut().span_from(start);
                return Ok(AstExpr::CalcInterp166(texto, span).span(span));
            }
            parser.toks_mut().set_cursor(start);
        }

        let was_in_parentheses = parser.flags().in_parens();
        parser.flags_mut().set(ContextFlags::IN_PARENS, true);

        parser.expect_char('(')?;
        parser.whitespace()?;
        if !parser.looking_at_expression() {
            parser.expect_char(')')?;
            parser
                .flags_mut()
                .set(ContextFlags::IN_PARENS, was_in_parentheses);
            return Ok(AstExpr::List(ListExpr {
                elems: Vec::new(),
                separator: ListSeparator::Undecided,
                brackets: Brackets::None,
            })
            .span(parser.toks_mut().span_from(start)));
        }

        let first = parser.parse_expression_until_comma(false)?;
        if parser.scan_char(':') {
            parser.whitespace()?;
            parser
                .flags_mut()
                .set(ContextFlags::IN_PARENS, was_in_parentheses);
            return Self::parse_map(parser, first, start);
        }

        if !parser.scan_char(',') {
            parser.expect_char(')')?;
            parser
                .flags_mut()
                .set(ContextFlags::IN_PARENS, was_in_parentheses);
            // `ParenthesizedExpression(..., spanFrom(start))`: com os parênteses.
            return Ok(
                AstExpr::Paren(Arc::new(first.node)).span(parser.toks_mut().span_from(start))
            );
        }

        parser.whitespace()?;

        let mut expressions = vec![first];

        loop {
            if !parser.looking_at_expression() {
                break;
            }
            expressions.push(parser.parse_expression_until_comma(false)?);
            if !parser.scan_char(',') {
                break;
            }
            parser.whitespace()?;
        }

        parser.expect_char(')')?;

        parser
            .flags_mut()
            .set(ContextFlags::IN_PARENS, was_in_parentheses);

        Ok(AstExpr::List(ListExpr {
            elems: expressions,
            separator: ListSeparator::Comma,
            brackets: Brackets::None,
        })
        .span(parser.toks_mut().span_from(start)))
    }

    fn parse_variable(parser: &mut P) -> SassResult<Spanned<AstExpr>> {
        let start = parser.toks().cursor();
        let name = parser.parse_variable_name()?;

        if parser.is_plain_css() {
            return Err((
                "Sass variables aren't allowed in plain CSS.",
                parser.toks_mut().span_from(start),
            )
                .into());
        }

        Ok(AstExpr::Variable {
            name: Spanned {
                node: Identifier::from(name),
                span: parser.toks_mut().span_from(start),
            },
            namespace: None,
        }
        .span(parser.toks_mut().span_from(start)))
    }

    fn parse_selector(parser: &mut P) -> SassResult<Spanned<AstExpr>> {
        if parser.is_plain_css() {
            return Err((
                "The parent selector isn't allowed in plain CSS.",
                parser.toks().current_span(),
            )
                .into());
        }

        let start = parser.toks().cursor();

        parser.expect_char('&')?;

        if parser.toks().next_char_is('&') {
            // todo: emit a warning here
            //   warn(
            //       'In Sass, "&&" means two copies of the parent selector. You '
            //       'probably want to use "and" instead.',
            //       scanner.spanFrom(start));
            //   scanner.position--;
        }

        Ok(AstExpr::ParentSelector.span(parser.toks_mut().span_from(start)))
    }

    fn parse_hash(&mut self, parser: &mut P) -> SassResult<Spanned<AstExpr>> {
        let start = parser.toks().cursor();
        debug_assert!(matches!(
            parser.toks().peek(),
            Some(Token { kind: '#', .. })
        ));

        if matches!(parser.toks().peek_n(1), Some(Token { kind: '{', .. })) {
            return self.parse_identifier_like(parser);
        }

        parser.expect_char('#')?;

        if matches!(
            parser.toks().peek(),
            Some(Token {
                kind: '0'..='9',
                ..
            })
        ) {
            let color = self.parse_hex_color_contents(parser)?;
            return Ok(AstExpr::Color(Arc::new(color)).span(parser.toks_mut().span_from(start)));
        }

        let after_hash = parser.toks().cursor();
        let ident = parser.parse_interpolated_identifier()?;
        if is_hex_color(&ident) {
            parser.toks_mut().set_cursor(after_hash);
            let color = self.parse_hex_color_contents(parser)?;
            return Ok(
                AstExpr::Color(Arc::new(color)).span(parser.toks_mut().span_from(after_hash))
            );
        }

        let mut buffer = Interpolation::new();

        buffer.add_char('#');
        buffer.add_interpolation(ident);

        let span = parser.toks_mut().span_from(start);

        Ok(AstExpr::String(StringExpr(buffer, QuoteKind::None), span).span(span))
    }

    fn parse_hex_digit(&mut self, parser: &mut P) -> SassResult<u32> {
        match parser.toks().peek() {
            Some(Token { kind, .. }) if kind.is_ascii_hexdigit() => {
                parser.toks_mut().next();
                Ok(as_hex(kind))
            }
            _ => Err(("Expected hex digit.", parser.toks().current_span()).into()),
        }
    }

    fn parse_hex_color_contents(&mut self, parser: &mut P) -> SassResult<Color> {
        let start = parser.toks().cursor();

        let digit1 = self.parse_hex_digit(parser)?;
        let digit2 = self.parse_hex_digit(parser)?;
        let digit3 = self.parse_hex_digit(parser)?;

        let red: u32;
        let green: u32;
        let blue: u32;
        let mut alpha: f64 = 1.0;
        let mut has_alpha = false;

        if parser.next_is_hex() {
            let digit4 = self.parse_hex_digit(parser)?;

            if parser.next_is_hex() {
                red = (digit1 << 4) + digit2;
                green = (digit3 << 4) + digit4;
                blue = (self.parse_hex_digit(parser)? << 4) + self.parse_hex_digit(parser)?;

                if parser.next_is_hex() {
                    has_alpha = true;
                    alpha = ((self.parse_hex_digit(parser)? << 4) + self.parse_hex_digit(parser)?)
                        as f64
                        / 0xff as f64;
                }
            } else {
                // #abcd
                has_alpha = true;
                red = (digit1 << 4) + digit1;
                green = (digit2 << 4) + digit2;
                blue = (digit3 << 4) + digit3;
                alpha = ((digit4 << 4) + digit4) as f64 / 0xff as f64;
            }
        } else {
            // #abc
            red = (digit1 << 4) + digit1;
            green = (digit2 << 4) + digit2;
            blue = (digit3 << 4) + digit3;
        }

        Ok(Color::rgb_internal(
            Some(f64::from(red)),
            Some(f64::from(green)),
            Some(f64::from(blue)),
            Some(alpha),
            // dart-sass `_hexColorContents`: `alpha == null ?
            // SpanColorFormat(...) : null` — hex de 4 ou 8 dígitos não guarda
            // o texto original (sai como `rgba(...)`).
            if has_alpha {
                ColorFormat::Infer
            } else {
                ColorFormat::Literal(parser.toks_mut().raw_text(start - 1))
            },
        ))
    }

    fn parse_unary_operation(&mut self, parser: &mut P) -> SassResult<Spanned<AstExpr>> {
        let op_span = parser.toks().current_span();
        let operator = Self::expect_unary_operator(parser)?;

        if parser.is_plain_css() && operator != UnaryOp::Div {
            return Err(("Operators aren't allowed in plain CSS.", op_span).into());
        }

        parser.whitespace()?;

        let operand = self.parse_single_expression(parser)?;

        let span = op_span.merge(parser.toks().current_span());

        Ok(AstExpr::UnaryOp(operator, Arc::new(operand.node), span).span(span))
    }

    fn expect_unary_operator(parser: &mut P) -> SassResult<UnaryOp> {
        let span = parser.toks().current_span();
        Ok(match parser.toks_mut().next() {
            Some(Token { kind: '+', .. }) => UnaryOp::Plus,
            Some(Token { kind: '-', .. }) => UnaryOp::Neg,
            Some(Token { kind: '/', .. }) => UnaryOp::Div,
            Some(..) | None => return Err(("Expected unary operator.", span).into()),
        })
    }

    fn consume_natural_number(parser: &mut P) -> SassResult<()> {
        if !matches!(
            parser.toks_mut().next(),
            Some(Token {
                kind: '0'..='9',
                ..
            })
        ) {
            return Err(("Expected digit.", parser.toks().prev_span()).into());
        }

        while matches!(
            parser.toks().peek(),
            Some(Token {
                kind: '0'..='9',
                ..
            })
        ) {
            parser.toks_mut().next();
        }

        Ok(())
    }

    fn parse_number(parser: &mut P) -> SassResult<Spanned<AstExpr>> {
        let start = parser.toks().cursor();

        if !parser.scan_char('+') {
            parser.scan_char('-');
        }

        let after_sign = parser.toks().cursor();

        if !parser.toks().next_char_is('.') {
            ValueParser::consume_natural_number(parser)?;
        }

        ValueParser::try_decimal(parser, parser.toks().cursor() != after_sign)?;
        ValueParser::try_exponent(parser)?;

        let number: f64 = parser.toks_mut().raw_text(start).parse().unwrap();

        let unit = if parser.scan_char('%') {
            Unit::Percent
        } else if parser.looking_at_identifier()
            && (!matches!(parser.toks().peek(), Some(Token { kind: '-', .. }))
                || !matches!(parser.toks().peek_n(1), Some(Token { kind: '-', .. })))
        {
            Unit::from(parser.parse_identifier(false, true)?)
        } else {
            Unit::None
        };

        Ok(AstExpr::Number {
            n: Number::from(number),
            unit,
        }
        .span(parser.toks_mut().span_from(start)))
    }

    fn try_decimal(parser: &mut P, allow_trailing_dot: bool) -> SassResult<Option<String>> {
        if !matches!(parser.toks().peek(), Some(Token { kind: '.', .. })) {
            return Ok(None);
        }

        match parser.toks().peek_n(1) {
            Some(Token { kind, .. }) if !kind.is_ascii_digit() => {
                if allow_trailing_dot {
                    return Ok(None);
                }

                return Err(("Expected digit.", parser.toks().current_span()).into());
            }
            Some(..) => {}
            None => return Err(("Expected digit.", parser.toks().current_span()).into()),
        }

        let mut buffer = String::new();

        parser.expect_char('.')?;
        buffer.push('.');

        while let Some(Token { kind, .. }) = parser.toks().peek() {
            if !kind.is_ascii_digit() {
                break;
            }
            buffer.push(kind);
            parser.toks_mut().next();
        }

        Ok(Some(buffer))
    }

    fn try_exponent(parser: &mut P) -> SassResult<Option<String>> {
        let mut buffer = String::new();

        match parser.toks().peek() {
            Some(Token {
                kind: 'e' | 'E', ..
            }) => buffer.push('e'),
            _ => return Ok(None),
        }

        let next = match parser.toks().peek_n(1) {
            Some(Token {
                kind: kind @ ('0'..='9' | '-' | '+'),
                ..
            }) => kind,
            _ => return Ok(None),
        };

        parser.toks_mut().next();

        if next == '+' || next == '-' {
            parser.toks_mut().next();
            buffer.push(next);
        }

        match parser.toks().peek() {
            Some(Token {
                kind: '0'..='9', ..
            }) => {}
            _ => return Err(("Expected digit.", parser.toks().current_span()).into()),
        }

        while let Some(tok) = parser.toks().peek() {
            if !tok.kind.is_ascii_digit() {
                break;
            }

            buffer.push(tok.kind);

            parser.toks_mut().next();
        }

        Ok(Some(buffer))
    }

    fn parse_plus_expr(&mut self, parser: &mut P) -> SassResult<Spanned<AstExpr>> {
        debug_assert!(parser.toks().next_char_is('+'));

        match parser.toks().peek_n(1) {
            Some(Token {
                kind: '0'..='9' | '.',
                ..
            }) => ValueParser::parse_number(parser),
            _ => self.parse_unary_operation(parser),
        }
    }

    fn parse_minus_expr(&mut self, parser: &mut P) -> SassResult<Spanned<AstExpr>> {
        debug_assert!(parser.toks().next_char_is('-'));

        if matches!(
            parser.toks().peek_n(1),
            Some(Token {
                kind: '0'..='9' | '.',
                ..
            })
        ) {
            return ValueParser::parse_number(parser);
        }

        if parser.looking_at_interpolated_identifier() {
            return self.parse_identifier_like(parser);
        }

        self.parse_unary_operation(parser)
    }

    fn parse_important_expr(parser: &mut P) -> SassResult<Spanned<AstExpr>> {
        let start = parser.toks().cursor();
        parser.expect_char('!')?;
        parser.whitespace()?;
        parser.expect_identifier("important", false)?;

        let span = parser.toks_mut().span_from(start);

        Ok(AstExpr::String(
            StringExpr(
                Interpolation::new_plain("!important".to_owned()),
                QuoteKind::None,
            ),
            span,
        )
        .span(span))
    }

    fn parse_identifier_like(&mut self, parser: &mut P) -> SassResult<Spanned<AstExpr>> {
        if let Some(func) = P::IDENTIFIER_LIKE {
            return func(parser);
        }

        let start = parser.toks().cursor();

        let identifier = parser.parse_interpolated_identifier()?;

        let ident_span = parser.toks_mut().span_from(start);

        let plain = identifier.as_plain();
        let lower = plain.map(str::to_ascii_lowercase);

        if let Some(plain) = plain {
            if plain == "if" && parser.toks().next_char_is('(') {
                // dart-sass 1.102 `identifierLike`: tenta o `if()` antigo do Sass
                // (argumentos) e, se não analisar, é o `if()` do CSS.
                let antes = parser.toks().cursor();
                if let Ok(call_args) = parser.parse_argument_invocation(false, false) {
                    let span = call_args.span;
                    return Ok(AstExpr::If(Arc::new(Ternary(call_args))).span(span));
                }
                parser.toks_mut().set_cursor(antes);
                return Self::parse_css_if(parser, start);
            } else if lower.as_deref() == Some("if") && parser.toks().next_char_is('(') {
                return Self::parse_css_if(parser, start);
            } else if plain == "not" {
                parser.whitespace()?;

                let value = self.parse_single_expression(parser)?;

                let span = parser.toks_mut().span_from(start);

                return Ok(AstExpr::UnaryOp(UnaryOp::Not, Arc::new(value.node), span).span(span));
            }

            let lower_ref = lower.as_ref().unwrap();

            if !parser.toks().next_char_is('(') {
                match plain {
                    "null" => return Ok(AstExpr::Null.span(parser.toks_mut().span_from(start))),
                    "true" => return Ok(AstExpr::True.span(parser.toks_mut().span_from(start))),
                    "false" => return Ok(AstExpr::False.span(parser.toks_mut().span_from(start))),
                    _ => {}
                }

                if let Some(color) = NAMED_COLORS.get_by_name(lower_ref.as_str()) {
                    return Ok(AstExpr::Color(Arc::new(Color::new(
                        color[0],
                        color[1],
                        color[2],
                        color[3],
                        plain.to_owned(),
                    )))
                    .span(parser.toks_mut().span_from(start)));
                }
            }

            if let Some(func) = ValueParser::try_parse_special_function(parser, lower_ref, start)? {
                return Ok(func);
            }
        }

        match parser.toks().peek() {
            Some(Token { kind: '.', .. }) => {
                if matches!(parser.toks().peek_n(1), Some(Token { kind: '.', .. })) {
                    return Ok(AstExpr::String(
                        StringExpr(identifier, QuoteKind::None),
                        parser.toks_mut().span_from(start),
                    )
                    .span(parser.toks_mut().span_from(start)));
                }
                parser.toks_mut().next();

                match plain {
                    Some(s) => Self::namespaced_expression(
                        Spanned {
                            node: Identifier::from(s),
                            span: ident_span,
                        },
                        start,
                        parser,
                    ),
                    None => Err(("Interpolation isn't allowed in namespaces.", ident_span).into()),
                }
            }
            Some(Token { kind: '(', .. }) => {
                if let Some(plain) = plain {
                    let calc_166 = parser.options().v166()
                        && matches!(lower.as_deref(), Some("calc" | "clamp" | "min" | "max"));
                    let dentro = parser.flags().in_calc_166();
                    parser.flags_mut().set(ContextFlags::IN_CALC_166, calc_166);
                    let arguments = if calc_166 {
                        Self::calculation_arguments_166(parser)
                    } else {
                        parser.parse_argument_invocation(false, lower.as_deref() == Some("var"))
                    };
                    parser.flags_mut().set(ContextFlags::IN_CALC_166, dentro);
                    let arguments = arguments?;

                    Ok(AstExpr::FunctionCall(FunctionCallExpr {
                        namespace: None,
                        name: Identifier::from(plain),
                        arguments: Arc::new(arguments),
                        span: parser.toks_mut().span_from(start),
                    })
                    .span(parser.toks_mut().span_from(start)))
                } else {
                    let dentro = parser.flags().in_calc_166();
                    parser.flags_mut().set(ContextFlags::IN_CALC_166, false);
                    let arguments = parser.parse_argument_invocation(false, false);
                    parser.flags_mut().set(ContextFlags::IN_CALC_166, dentro);
                    let arguments = arguments?;
                    Ok(
                        AstExpr::InterpolatedFunction(Arc::new(InterpolatedFunction {
                            name: identifier,
                            arguments,
                            span: parser.toks_mut().span_from(start),
                        }))
                        .span(parser.toks_mut().span_from(start)),
                    )
                }
            }
            _ => Ok(AstExpr::String(
                StringExpr(identifier, QuoteKind::None),
                parser.toks_mut().span_from(start),
            )
            .span(parser.toks_mut().span_from(start))),
        }
    }

    /// Os argumentos de `calc()`/`clamp()`/`min()`/`max()` no modo 1.66
    /// (`_calculationArguments`): com interpolação no nível de cima, um só
    /// argumento com o texto cru; senão, os argumentos comuns (com
    /// [`ContextFlags::IN_CALC_166`] ligado pelo chamador).
    fn calculation_arguments_166(parser: &mut P) -> SassResult<ArgumentInvocation> {
        let start = parser.toks().cursor();
        parser.expect_char('(')?;
        if Self::contains_calculation_interpolation_166(parser)? {
            let arg_start = parser.toks().cursor();
            let texto = parser.parse_interpolated_declaration_value(false, false, true)?;
            let arg_span = parser.toks_mut().span_from(arg_start);
            parser.expect_char(')')?;
            return Ok(ArgumentInvocation {
                positional: vec![AstExpr::CalcInterp166(texto, arg_span)],
                named: Default::default(),
                rest: None,
                keyword_rest: None,
                span: parser.toks_mut().span_from(start),
                positional_spans: vec![arg_span],
                named_spans: Default::default(),
            });
        }
        parser.toks_mut().set_cursor(start);
        parser.parse_argument_invocation(false, false)
    }

    /// `_containsCalculationInterpolation` do 1.66: o texto até o próximo
    /// `)`, `]` ou `}` sem par tem `#{` fora de parênteses internos (e fora
    /// de strings e comentários)? Não consome nada.
    fn contains_calculation_interpolation_166(parser: &mut P) -> SassResult<bool> {
        let start = parser.toks().cursor();
        let mut parens = 0i32;
        let mut brackets: Vec<char> = Vec::new();
        let resultado = loop {
            let Some(next) = parser.toks().peek().map(|t| t.kind) else {
                break false;
            };
            match next {
                '\\' => {
                    parser.toks_mut().next();
                    parser.toks_mut().next();
                }
                '/' => {
                    if !parser.scan_comment()? {
                        parser.toks_mut().next();
                    }
                }
                '\'' | '"' => {
                    parser.parse_interpolated_string()?;
                }
                '#' => {
                    if parens == 0 && parser.toks().peek_n(1).is_some_and(|t| t.kind == '{') {
                        break true;
                    }
                    parser.toks_mut().next();
                }
                '(' | '{' | '[' => {
                    if next == '(' {
                        parens += 1;
                    }
                    brackets.push(match next {
                        '(' => ')',
                        '{' => '}',
                        _ => ']',
                    });
                    parser.toks_mut().next();
                }
                ')' | '}' | ']' => {
                    if next == ')' {
                        parens -= 1;
                    }
                    if brackets.pop() != Some(next) {
                        break false;
                    }
                    parser.toks_mut().next();
                }
                _ => {
                    parser.toks_mut().next();
                }
            }
        };
        parser.toks_mut().set_cursor(start);
        Ok(resultado)
    }

    fn namespaced_expression(
        namespace: Spanned<Identifier>,
        start: usize,
        parser: &mut P,
    ) -> SassResult<Spanned<AstExpr>> {
        if parser.toks().next_char_is('$') {
            let name_start = parser.toks().cursor();
            let name = parser.parse_variable_name()?;
            let span = parser.toks_mut().span_from(start);
            P::assert_public(&name, span)?;

            if parser.is_plain_css() {
                return Err(("Module namespaces aren't allowed in plain CSS.", span).into());
            }

            return Ok(AstExpr::Variable {
                name: Spanned {
                    node: Identifier::from(name),
                    span: parser.toks_mut().span_from(name_start),
                },
                namespace: Some(namespace),
            }
            .span(span));
        }

        let name = parser.parse_public_identifier()?;
        let args = parser.parse_argument_invocation(false, false)?;
        let span = parser.toks_mut().span_from(start);

        if parser.is_plain_css() {
            return Err(("Module namespaces aren't allowed in plain CSS.", span).into());
        }

        Ok(AstExpr::FunctionCall(FunctionCallExpr {
            namespace: Some(namespace),
            name: Identifier::from(name),
            arguments: Arc::new(args),
            span,
        })
        .span(span))
    }

    fn parse_unicode_range(parser: &mut P) -> SassResult<Spanned<AstExpr>> {
        let start = parser.toks().cursor();
        parser.expect_ident_char('u', false)?;
        parser.expect_char('+')?;

        let mut first_range_length = 0;

        while let Some(next) = parser.toks().peek() {
            if !next.kind.is_ascii_hexdigit() {
                break;
            }

            parser.toks_mut().next();
            first_range_length += 1;
        }

        let mut has_question_mark = false;

        while parser.scan_char('?') {
            has_question_mark = true;
            first_range_length += 1;
        }

        let span = parser.toks_mut().span_from(start);
        if first_range_length == 0 {
            return Err(("Expected hex digit or \"?\".", parser.toks().current_span()).into());
        } else if first_range_length > 6 {
            return Err(("Expected at most 6 digits.", span).into());
        } else if has_question_mark {
            return Ok(AstExpr::String(
                StringExpr(
                    Interpolation::new_plain(parser.toks_mut().raw_text(start)),
                    QuoteKind::None,
                ),
                span,
            )
            .span(span));
        }

        if parser.scan_char('-') {
            let second_range_start = parser.toks().cursor();
            let mut second_range_length = 0;

            while let Some(next) = parser.toks().peek() {
                if !next.kind.is_ascii_hexdigit() {
                    break;
                }

                parser.toks_mut().next();
                second_range_length += 1;
            }

            if second_range_length == 0 {
                return Err(("Expected hex digit.", parser.toks().current_span()).into());
            } else if second_range_length > 6 {
                return Err((
                    "Expected at most 6 digits.",
                    parser.toks_mut().span_from(second_range_start),
                )
                    .into());
            }
        }

        if parser.looking_at_interpolated_identifier_body() {
            return Err(("Expected end of identifier.", parser.toks().current_span()).into());
        }

        let span = parser.toks_mut().span_from(start);

        Ok(AstExpr::String(
            StringExpr(
                Interpolation::new_plain(parser.toks_mut().raw_text(start)),
                QuoteKind::None,
            ),
            span,
        )
        .span(span))
    }

    pub(crate) fn try_parse_special_function(
        parser: &mut P,
        name: &str,
        start: usize,
    ) -> SassResult<Option<Spanned<AstExpr>>> {
        // dart-sass 1.102 `trySpecialFunction`: `calc()` e as outras funções
        // de cálculo são chamadas comuns (o avaliador decide); só `type(`,
        // `calc` com prefixo de fornecedor, `expression`, `element`,
        // `progid:` e `url` têm análise crua.
        let normalized = unvendor(name);
        let vendored = normalized.len() != name.len();

        let mut buffer;

        match normalized {
            _ if name == "type" && parser.toks().next_char_is('(') => {
                parser.scan_char('(');
                buffer = Interpolation::new_plain(name.to_owned());
                buffer.add_char('(');
            }
            "calc" if vendored && parser.toks().next_char_is('(') => {
                parser.scan_char('(');
                buffer = Interpolation::new_plain(name.to_owned());
                buffer.add_char('(');
            }
            "element" | "expression" => {
                if !parser.scan_char('(') {
                    return Ok(None);
                }

                buffer = Interpolation::new_plain(name.to_owned());
                buffer.add_char('(');
            }
            "progid" => {
                if !parser.scan_char(':') {
                    return Ok(None);
                }
                buffer = Interpolation::new_plain(name.to_owned());
                buffer.add_char(':');

                while let Some(Token { kind, .. }) = parser.toks().peek() {
                    if !kind.is_alphabetic() && kind != '.' {
                        break;
                    }
                    buffer.add_char(kind);
                    parser.toks_mut().next();
                }
                parser.expect_char('(')?;
                buffer.add_char('(');
            }
            "url" => {
                return Ok(parser.try_url_contents(None)?.map(|contents| {
                    AstExpr::String(
                        StringExpr(contents, QuoteKind::None),
                        parser.toks_mut().span_from(start),
                    )
                    .span(parser.toks_mut().span_from(start))
                }))
            }
            _ => return Ok(None),
        }

        buffer.add_interpolation(parser.parse_interpolated_declaration_value(false, true, true)?);
        parser.expect_char(')')?;
        buffer.add_char(')');

        Ok(Some(
            AstExpr::String(
                StringExpr(buffer, QuoteKind::None),
                parser.toks_mut().span_from(start),
            )
            .span(parser.toks_mut().span_from(start)),
        ))
    }

    /// `ifExpression`: o `if()` do CSS.
    fn parse_css_if(parser: &mut P, start: usize) -> SassResult<Spanned<AstExpr>> {
        parser.expect_char('(')?;
        parser.whitespace()?;
        let mut branches = Vec::new();
        while !parser.toks().next_char_is(')') {
            let condition = if parser.scan_identifier("else", false)? {
                None
            } else {
                Some(Self::if_condition_expression(parser)?)
            };
            parser.whitespace()?;
            parser.expect_char(':')?;
            parser.whitespace()?;
            let expr = parser.parse_expression(None, None, None)?.node;
            branches.push((condition, expr));
            parser.whitespace()?;
            if !parser.scan_char(';') {
                break;
            }
            parser.whitespace()?;
        }
        parser.expect_char(')')?;
        let span = parser.toks_mut().span_from(start);
        if branches.is_empty() {
            return Err(("Expected expression.", span).into());
        }
        Ok(AstExpr::CssIf(Arc::new(CssIfExpr { branches, span })).span(span))
    }

    fn if_operation(mut groups: Vec<IfCondition>, op: Option<bool>) -> IfCondition {
        if groups.len() == 1 {
            groups.pop().unwrap()
        } else {
            IfCondition::Op(groups, op.unwrap_or(true))
        }
    }

    /// `_ifConditionExpression`.
    fn if_condition_expression(parser: &mut P) -> SassResult<IfCondition> {
        if parser.scan_identifier("not", false)? {
            if parser.toks().next_char_is('(') {
                return Err((
                    "Whitespace is required between \"not\" and \"(\"",
                    parser.toks().current_span(),
                )
                    .into());
            }
            parser.whitespace()?;
            let group = Self::if_group(parser)?;
            return Ok(IfCondition::Not(Box::new(group)));
        }
        let mut groups = vec![Self::if_group(parser)?];
        let mut op: Option<bool> = None;
        parser.whitespace()?;
        loop {
            if op != Some(false) && parser.scan_identifier("and", false)? {
                if parser.toks().next_char_is('(') {
                    return Err((
                        "Whitespace is required between \"and\" and \"(\"",
                        parser.toks().current_span(),
                    )
                        .into());
                }
                parser.whitespace()?;
                op.get_or_insert(true);
                groups.push(Self::if_group(parser)?);
            } else if op != Some(true) && parser.scan_identifier("or", false)? {
                if parser.toks().next_char_is('(') {
                    return Err((
                        "Whitespace is required between \"and\" and \"(\"",
                        parser.toks().current_span(),
                    )
                        .into());
                }
                parser.whitespace()?;
                op.get_or_insert(false);
                groups.push(Self::if_group(parser)?);
            } else if parser
                .toks()
                .peek()
                .is_some_and(|t| t.kind != ')' && t.kind != ':')
                && groups.last().unwrap().is_arbitrary_substitution()
            {
                let preceding = Self::if_operation(groups, op);
                let next = Self::if_group(parser)?;
                return Self::if_condition_raw(parser, preceding, next);
            } else if let Some(sub) = Self::try_arbitrary_substitution(parser)? {
                let preceding = Self::if_operation(groups, op);
                return Self::if_condition_raw(parser, preceding, sub);
            } else {
                break;
            }
            parser.whitespace()?;
        }
        Ok(Self::if_operation(groups, op))
    }

    fn if_interpolation(c: &IfCondition) -> SassResult<Interpolation> {
        c.to_interpolation().map_err(|span| {
            (
                "if() conditions with arbitrary substitutions may not contain sass() expressions.",
                span,
            )
                .into()
        })
    }

    /// `_ifConditionRaw`.
    fn if_condition_raw(
        parser: &mut P,
        preceding: IfCondition,
        next: IfCondition,
    ) -> SassResult<IfCondition> {
        let valido = preceding.is_arbitrary_substitution()
            || matches!(&preceding, IfCondition::Op(es, _) if es.last().is_some_and(IfCondition::is_arbitrary_substitution))
            || next.is_arbitrary_substitution();
        if !valido {
            return Err((
                "Expected an arbitrary substitution.",
                parser.toks().current_span(),
            )
                .into());
        }
        let mut buffer = Self::if_interpolation(&preceding)?;
        buffer.add_char(' ');
        buffer.add_interpolation(Self::if_interpolation(&next)?);
        let last_group = next;
        let mut op = match &preceding {
            IfCondition::Op(_, and) => Some(*and),
            _ => None,
        };
        let mut last_group = last_group;
        parser.whitespace()?;
        loop {
            if op != Some(false) && parser.scan_identifier("and", false)? {
                if parser.toks().next_char_is('(') {
                    return Err((
                        "Whitespace is required between \"and\" and \"(\"",
                        parser.toks().current_span(),
                    )
                        .into());
                }
                parser.whitespace()?;
                op.get_or_insert(true);
                // No dart-sass este `lastGroup` é uma variável nova (não
                // atualiza o último grupo); o mesmo aqui.
                let g = Self::if_group(parser)?;
                buffer.add_string(" and ".to_owned());
                buffer.add_interpolation(Self::if_interpolation(&g)?);
            } else if op != Some(true) && parser.scan_identifier("or", false)? {
                if parser.toks().next_char_is('(') {
                    return Err((
                        "Whitespace is required between \"or\" and \"(\"",
                        parser.toks().current_span(),
                    )
                        .into());
                }
                parser.whitespace()?;
                op.get_or_insert(false);
                let g = Self::if_group(parser)?;
                buffer.add_string(" or ".to_owned());
                buffer.add_interpolation(Self::if_interpolation(&g)?);
            } else if parser
                .toks()
                .peek()
                .is_some_and(|t| t.kind != ')' && t.kind != ':')
                && last_group.is_arbitrary_substitution()
            {
                last_group = Self::if_group(parser)?;
                buffer.add_char(' ');
                buffer.add_interpolation(Self::if_interpolation(&last_group)?);
            } else if let Some(n) = Self::try_arbitrary_substitution(parser)? {
                last_group = n;
                buffer.add_char(' ');
                buffer.add_interpolation(Self::if_interpolation(&last_group)?);
            } else {
                break;
            }
            parser.whitespace()?;
        }
        Ok(IfCondition::Raw(buffer))
    }

    /// `_ifGroup`.
    fn if_group(parser: &mut P) -> SassResult<IfCondition> {
        let start = parser.toks().cursor();
        if parser.toks().next_char_is('(') {
            parser.expect_char('(')?;
            parser.whitespace()?;
            let e = Self::if_condition_expression(parser)?;
            parser.whitespace()?;
            parser.expect_char(')')?;
            return Ok(IfCondition::Paren(Box::new(e)));
        }
        if parser.scan_identifier("sass", true)? {
            parser.expect_char('(')?;
            parser.whitespace()?;
            let e = parser.parse_expression(None, None, None)?.node;
            parser.whitespace()?;
            parser.expect_char(')')?;
            let span = parser.toks_mut().span_from(start);
            if parser.is_plain_css() {
                return Err(("sass() conditions aren't allowed in plain CSS", span).into());
            }
            return Ok(IfCondition::Sass(e, span));
        }
        let identifier = parser.parse_interpolated_identifier()?;
        if identifier.contents.len() == 1
            && matches!(identifier.contents[0], InterpolationPart::Expr(..))
            && !parser.toks().next_char_is('(')
        {
            return Ok(IfCondition::Raw(identifier));
        }
        if let Some(p) = identifier.as_plain() {
            if matches!(p.to_ascii_lowercase().as_str(), "and" | "or" | "not")
                && parser.toks().next_char_is('(')
            {
                return Err((
                    format!("Whitespace is required between \"{p}\" and \"(\""),
                    parser.toks().current_span(),
                )
                    .into());
            }
        }
        parser.expect_char('(')?;
        parser.whitespace()?;
        let args = parser.parse_interpolated_declaration_value(true, true, true)?;
        parser.whitespace()?;
        parser.expect_char(')')?;
        Ok(IfCondition::Function {
            name: identifier,
            args,
        })
    }

    /// `_tryArbitrarySubstitution`.
    fn try_arbitrary_substitution(parser: &mut P) -> SassResult<Option<IfCondition>> {
        if parser.toks().next_char_is('#') {
            return Ok(Some(IfCondition::Raw(parser.parse_single_interpolation()?)));
        }
        let start = parser.toks().cursor();
        let name = if parser.scan_identifier("if", false)? {
            Some(Interpolation::new_plain("if".to_owned()))
        } else if parser.scan_identifier("var", false)? {
            Some(Interpolation::new_plain("var".to_owned()))
        } else if parser.scan_identifier("attr", false)? {
            Some(Interpolation::new_plain("attr".to_owned()))
        } else if parser.toks().next_char_is('-')
            && matches!(parser.toks().peek_n(1), Some(Token { kind: '-', .. }))
        {
            Some(parser.parse_interpolated_identifier()?)
        } else {
            None
        };
        let Some(name) = name else { return Ok(None) };
        if !parser.scan_char('(') {
            parser.toks_mut().set_cursor(start);
            return Ok(None);
        }
        let args = parser.parse_interpolated_declaration_value(true, true, true)?;
        parser.expect_char(')')?;
        Ok(Some(IfCondition::Function { name, args }))
    }

    fn reset_state(&mut self, parser: &mut P) -> SassResult<()> {
        self.comma_expressions = None;
        self.space_expressions = None;
        self.binary_operators = None;
        self.operands = None;
        parser.toks_mut().set_cursor(self.start);
        self.allow_slash = true;
        self.single_expression = Some(self.parse_single_expression(parser)?);

        Ok(())
    }
}
