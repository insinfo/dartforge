//! Statements, blocks, function bodies and `for` headers (fasta's
//! `parseStatementX`, `parseExpressionStatementOrDeclaration*`,
//! `parseFunctionBody`, `parseForLoopParts*`, `parseSwitchBlock`,
//! `parseTryStatement`), with the `AstBuilder` nodes.

use dartforge_frontend::token::{Keyword, Op};

use super::super::tree::{Node, NodeId};
use super::super::token::{TokenKind, Tk};
use super::lookahead::TypeInfo;
use super::patterns::PatternCtx;
use super::types::MemberKind;
use super::{AsyncKind, LoopState, Parser, R};

/// Start of a local declaration: metadata and modifiers already read.
pub(super) struct Decl {
    /// First token (metadata or modifier).
    pub start: usize,
    /// Where the type starts.
    pub before_type: usize,
    pub meta: Vec<NodeId>,
    pub late: Option<usize>,
    /// `var`, `final` or `const`.
    pub vfc: Option<usize>,
    pub ti: Option<TypeInfo>,
}

impl Decl {
    fn without_prefix(&self) -> bool {
        self.before_type == self.start
    }
}

/// A `switch` member before its statements: labels, `case`/`default`,
/// pattern (or expression), colon and start.
type SwitchMember = (Vec<NodeId>, usize, Option<NodeId>, usize, usize);

/// The parts of `assert(...)`: `assert`, `(`, condition, comma, message and
/// `)`.
pub(super) type AssertParts = (usize, usize, NodeId, Option<usize>, Option<NodeId>, usize);

/// What `parseExpressionStatementOrDeclaration` found in a `for`.
enum ForStart {
    Expression,
    Declaration(Decl, Option<NodeId>),
    Pattern(Vec<NodeId>, usize, NodeId, usize),
}

impl Parser<'_> {
    /// `parseStatement`.
    pub(super) fn statement(&mut self) -> R<NodeId> {
        self.enter_()?;
        let r = self.statement_x();
        self.sair();
        r
    }

    fn statement_x(&mut self) -> R<NodeId> {
        let i = self.pos;
        if self.plain_ident(i) {
            if self.op(i + 1, Op::Colon) {
                return self.labeled();
            }
            return self.expr_or_decl_after_modifier(Decl {
                start: i,
                before_type: i,
                meta: Vec::new(),
                late: None,
                vfc: None,
                ti: None,
            });
        }
        match self.kind_(i) {
            TokenKind::Op(Op::LBrace) => {
                if self.patterns_ && self.op(self.pair(i) + 1, Op::Assign) {
                    self.expression_statement()
                } else {
                    self.block()
                }
            }
            TokenKind::Op(Op::Semicolon) => {
                self.advance();
                Ok(self.node(EmptyStatement(i), i))
            }
            TokenKind::Reserved(k) => match k {
                Keyword::Return => self.return_(),
                Keyword::Var | Keyword::Final => {
                    if !self.modifier(i + 1) {
                        self.advance();
                        return self.expr_or_decl_after_modifier(Decl {
                            start: i,
                            before_type: i + 1,
                            meta: Vec::new(),
                            late: None,
                            vfc: Some(i),
                            ti: None,
                        });
                    }
                    self.expr_or_decl()
                }
                Keyword::If => self.if_(),
                Keyword::For => self.to(None),
                Keyword::Rethrow => {
                    self.advance();
                    let r = self.node(
                        Node::RethrowExpression {
                            rethrow_keyword: i as Tk,
                        },
                        i,
                    );
                    let s = self.expect_op(Op::Semicolon)?;
                    Ok(self.node(
                        Node::ExpressionStatement {
                            expression: r,
                            semicolon: Some(s as Tk),
                        },
                        i,
                    ))
                }
                Keyword::While => self.while_(),
                Keyword::Do => self.do_(),
                Keyword::Try => self.attempt(),
                Keyword::Switch => self.choice(),
                Keyword::Break | Keyword::Continue => self.split(k == Keyword::Break),
                Keyword::Assert => self.assertion(),
                Keyword::Const => self.const_decl_or_expr(),
                _ => self.expr_or_decl(),
            },
            TokenKind::Identifier => {
                let t = self.txt(i);
                if t == "await" && self.kw(i + 1, Keyword::For) {
                    let a = self.advance();
                    return self.to(Some(a));
                }
                if t == "yield" {
                    match self.async_kind {
                        AsyncKind::SyncKind => {
                            if self.op(i + 1, Op::Colon) {
                                return self.labeled();
                            }
                            if self.looks_like_expression_after(i, false) {
                                return self.ceder();
                            }
                            return self.expr_or_decl();
                        }
                        _ => return self.ceder(),
                    }
                }
                if t == "await" {
                    if self.async_kind == AsyncKind::SyncKind && !self.looks_like_expression_after(i, false) {
                        return self.expr_or_decl();
                    }
                    return self.expression_statement();
                }
                if t == "set" && self.ident(i + 1) {
                    return Err(self.error("Unexpected 'set'."));
                }
                if self.op(i + 1, Op::Colon) {
                    return self.labeled();
                }
                self.expr_or_decl()
            }
            _ => self.expr_or_decl(),
        }
    }

    /// `parseExpressionStatement`.
    fn expression_statement(&mut self) -> R<NodeId> {
        let start_ = self.pos;
        let expression = self.expression()?;
        if matches!(self.tree_.node(expression), Node::SuperExpression { .. }) {
            return Err(self.error_at(start_, "Can't use 'super' as an expression."));
        }
        let s = self.expect_op(Op::Semicolon)?;
        Ok(self.node(
            Node::ExpressionStatement {
                expression,
                semicolon: Some(s as Tk),
            },
            start_,
        ))
    }

    /// Metadata and modifiers of a local declaration
    /// (`parseExpressionStatementOrDeclaration`).
    fn local_modifiers(&mut self) -> R<Decl> {
        let start = self.pos;
        let meta = self.metadata_()?;
        let mut late = None;
        let mut vfc = None;
        if self.modifier(self.pos) {
            if self.word(self.pos, "augment") && self.kw(self.pos + 1, Keyword::Super) {
                return Err(self.error("Augmentations aren't supported here."));
            }
            if self.kw(self.pos, Keyword::Var)
                || self.kw(self.pos, Keyword::Final)
                || self.kw(self.pos, Keyword::Const)
            {
                vfc = Some(self.advance());
            } else if self.word(self.pos, "late") {
                late = Some(self.advance());
                if self.kw(self.pos, Keyword::Var) || self.kw(self.pos, Keyword::Final) {
                    vfc = Some(self.advance());
                }
            }
            if self.modifier(self.pos) {
                return Err(self.error(&format!(
                    "Can't have modifier '{}' here.",
                    self.txt(self.pos)
                )));
            }
        }
        Ok(Decl {
            start,
            before_type: self.pos,
            meta,
            late,
            vfc,
            ti: None,
        })
    }

    fn expr_or_decl(&mut self) -> R<NodeId> {
        let d = self.local_modifiers()?;
        self.expr_or_decl_after_modifier(d)
    }

    /// `parseExpressionStatementOrConstDeclaration`.
    fn const_decl_or_expr(&mut self) -> R<NodeId> {
        let c = self.pos;
        if !self.modifier(c + 1) {
            let ti = self.type_at(c + 1, false, false, false);
            if ti.is_none() {
                let n = c + 1;
                if !self.ident(n) {
                    return self.expression_statement();
                }
                let nn = n + 1;
                if !(self.op(nn, Op::Assign)
                    || self.word_(nn)
                    || self.op(nn, Op::Semicolon)
                    || self.op(nn, Op::Comma)
                    || self.op(nn, Op::LBrace))
                {
                    return self.expression_statement();
                }
            }
            self.advance();
            return self.expr_or_decl_after_modifier(Decl {
                start: c,
                before_type: c + 1,
                meta: Vec::new(),
                late: None,
                vfc: Some(c),
                ti: Some(ti),
            });
        }
        self.expr_or_decl()
    }

    /// The `parseExpressionStatementOrDeclarationAfterModifiers` test for
    /// `Type? name`: decides whether it is a declaration or the start of a
    /// conditional.
    fn question_and_conditional(&mut self, ti: TypeInfo) -> R<bool> {
        let next = ti.end_index;
        if !self.looks_like_name(next) {
            return Err(self.error_at(next, "Expected an identifier."));
        }
        let after = next + 1;
        if self.op(after, Op::Assign) {
            let reached = self.probe(|p| {
                p.pos = after + 1;
                p.expression_without_cascade()
            });
            return Ok(reached.is_some_and(|f| self.op(f, Op::Colon)));
        }
        let keyword_ = self.reserved(after) || self.built_in(after) || self.pseudo(after);
        Ok(!keyword_
            && !(self.eof(after)
                || self.op(after, Op::Semicolon)
                || self.op(after, Op::Comma)
                || self.op(after, Op::RParen)))
    }

    /// `parseExpressionStatementOrDeclarationAfterModifiers` (statement).
    fn expr_or_decl_after_modifier(&mut self, d: Decl) -> R<NodeId> {
        if self.patterns_
            && let Some(v) = d.vfc
            && (self.kw(v, Keyword::Var) || self.kw(v, Keyword::Final))
            && let Some(f) = self.skip_outer_pattern(d.before_type)
            && self.op(f + 1, Op::Assign)
        {
            if d.late.is_some() {
                return Err(self.error_at(
                    d.late.unwrap_or(0),
                    "A pattern variable declaration may not use the `late` keyword.",
                ));
            }
            return self.pattern_declaration(d, v);
        }
        let mut ti =
            d.ti.unwrap_or_else(|| self.type_at(d.before_type, false, false, false));
        if self.looks_like_local_function(ti.end_index) {
            if d.vfc.is_some() || d.late.is_some() {
                return Err(
                    self.error_at(d.vfc.or(d.late).unwrap_or(0), "Can't have modifier here.")
                );
            }
            return self.local_function(d, ti);
        }
        if d.without_prefix()
            && ti.nullable()
            && ti.could_be_expression
            && self.question_and_conditional(ti)?
        {
            ti = TypeInfo::none_(d.before_type);
        }
        if ti.is_none() && d.without_prefix() {
            return self.expression_statement();
        }
        let next = ti.end_index;
        if self.built_in(next) && d.without_prefix() && ti.could_be_expression && self.word(next, "as") {
            let k = next + 1;
            if !(self.op(k, Op::Assign) || self.op(k, Op::Semicolon) || self.op(k, Op::Comma)) {
                return self.expression_statement();
            }
        }
        self.check_decl(&d, &ti)?;
        let start_ = d.start;
        let type_ = if ti.is_none() {
            None
        } else {
            Some(self.type_until(ti.end_index)?)
        };
        let variables = self.variables_(d, type_)?;
        let s = self.expect_op(Op::Semicolon)?;
        Ok(self.node(
            Node::VariableDeclarationStatement {
                variables,
                semicolon: s as Tk,
            },
            start_,
        ))
    }

    /// Errors for a missing or redundant `var`/type.
    fn check_decl(&self, d: &Decl, ti: &TypeInfo) -> R<()> {
        if self.ident(ti.end_index) {
            match d.vfc {
                None if ti.is_none() => {
                    return Err(self.error_at(ti.end_index, "Variables must be declared using the keywords 'const', 'final', 'var' or a type name."));
                }
                Some(v) if self.kw(v, Keyword::Var) && !ti.is_none() => {
                    return Err(self.error_at(
                        v,
                        "Variables can't be declared using both 'var' and a type name.",
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// `name_ (= expr)? (, name_ (= expr)?)*` numa [`Node::VariableDeclarationList`].
    fn variables_(&mut self, d: Decl, type_: Option<NodeId>) -> R<NodeId> {
        let mut vars = Vec::new();
        loop {
            let n = self.expect_declared_name()?;
            let (equals, initializer) = if self.op(self.pos, Op::Assign) {
                let e = self.advance();
                (Some(e as Tk), Some(self.expression()?))
            } else {
                (None, None)
            };
            vars.push(self.node(
                Node::VariableDeclaration {
                    name: n as Tk,
                    equals,
                    initializer,
                },
                n,
            ));
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.advance();
        }
        Ok(self.node(
            Node::VariableDeclarationList {
                metadata: d.meta,
                late_keyword: d.late.map(|l| l as Tk),
                keyword: d.vfc.map(|v| v as Tk),
                type_,
                variables: vars,
            },
            d.start,
        ))
    }

    /// `parsePatternVariableDeclarationStatement`.
    fn pattern_declaration(&mut self, d: Decl, keyword: usize) -> R<NodeId> {
        let start_ = d.start;
        self.pos = d.before_type;
        let pattern = self.pattern(PatternCtx::Declaration)?;
        let equals = self.expect_op(Op::Assign)?;
        let expression = self.expression()?;
        let decl = self.node(
            Node::PatternVariableDeclaration {
                metadata: d.meta,
                keyword: keyword as Tk,
                pattern,
                equals: equals as Tk,
                expression,
            },
            start_,
        );
        let s = self.expect_op(Op::Semicolon)?;
        Ok(self.node(
            Node::PatternVariableDeclarationStatement {
                declaration: decl,
                semicolon: s as Tk,
            },
            start_,
        ))
    }

    /// Local function (`parseNamedFunctionRest` with
    /// `isFunctionExpression: false`).
    fn local_function(&mut self, d: Decl, ti: TypeInfo) -> R<NodeId> {
        let start_ = d.start;
        self.pos = d.before_type;
        let return_type = if ti.is_none() {
            None
        } else {
            Some(self.type_until(ti.end_index)?)
        };
        let name_ = self.expect_declared_name()?;
        let fi = self.pos;
        let type_parameters = if self.op(self.pos, Op::Lt) {
            Some(self.type_params()?)
        } else {
            None
        };
        if !self.op(self.pos, Op::LParen) {
            return Err(self.expected("("));
        }
        let parameters = self.formal_params(MemberKind::Local)?;
        self.without_field_parameters(Some(parameters))?;
        if self.op(self.pos, Op::Colon) {
            return Err(self.error("Only constructors can have initializers."));
        }
        let body = self.function_body_(false, false)?;
        let fe = self.node(
            Node::FunctionExpression {
                type_parameters,
                parameters: Some(parameters),
                body,
            },
            fi,
        );
        let fd = self.node(
            Node::FunctionDeclaration {
                metadata: d.meta,
                augment_keyword: None,
                external_keyword: None,
                return_type,
                property_keyword: None,
                name: name_ as Tk,
                function_expression: fe,
            },
            start_,
        );
        Ok(self.node(
            Node::FunctionDeclarationStatement {
                function_declaration: fd,
            },
            start_,
        ))
    }

    /// `parseLabeledStatement`.
    fn labeled(&mut self) -> R<NodeId> {
        let start_ = self.pos;
        let mut labels = Vec::new();
        while self.ident(self.pos) && self.op(self.pos + 1, Op::Colon) {
            let n = self.advance();
            let s = self.simple(n);
            let c = self.advance();
            labels.push(self.node(
                Node::Label {
                    label: s,
                    colon: c as Tk,
                },
                n,
            ));
        }
        let statement = self.statement()?;
        Ok(self.node(Node::LabeledStatement { labels, statement }, start_))
    }

    /// `parseBlock`.
    pub(super) fn block(&mut self) -> R<NodeId> {
        let start_ = self.pos;
        let lb = self.expect_op(Op::LBrace)?;
        let mut statements = Vec::new();
        while !self.op(self.pos, Op::RBrace) {
            if self.eof(self.pos) {
                return Err(self.expected("}"));
            }
            statements.push(self.statement()?);
        }
        let rb = self.advance();
        Ok(self.node(
            Node::Block {
                left_bracket: lb as Tk,
                statements,
                right_bracket: rb as Tk,
            },
            start_,
        ))
    }

    /// `parseReturnStatement`.
    fn return_(&mut self) -> R<NodeId> {
        let r = self.advance();
        let expression = if self.op(self.pos, Op::Semicolon) {
            None
        } else {
            Some(self.expression()?)
        };
        let s = self.expect_op(Op::Semicolon)?;
        Ok(self.node(
            Node::ReturnStatement {
                return_keyword: r as Tk,
                expression,
                semicolon: s as Tk,
            },
            r,
        ))
    }

    /// `parseYieldStatement`.
    fn ceder(&mut self) -> R<NodeId> {
        let y = self.advance();
        let star = self.optional_op(Op::Star).map(|s| s as Tk);
        let expression = self.expression()?;
        let s = self.expect_op(Op::Semicolon)?;
        Ok(self.node(
            Node::YieldStatement {
                yield_keyword: y as Tk,
                star,
                expression,
                semicolon: s as Tk,
            },
            y,
        ))
    }

    /// `parseIfStatement`.
    fn if_(&mut self) -> R<NodeId> {
        let i = self.advance();
        let (lp, expression, case_clause, rp) = self.parenthesized_condition(self.patterns_)?;
        let then_statement = self.statement()?;
        let (else_keyword, else_statement) = if self.kw(self.pos, Keyword::Else) {
            let e = self.advance();
            (Some(e as Tk), Some(self.statement()?))
        } else {
            (None, None)
        };
        Ok(self.node(
            Node::IfStatement {
                if_keyword: i as Tk,
                left_parenthesis: lp as Tk,
                expression,
                case_clause,
                right_parenthesis: rp as Tk,
                then_statement,
                else_keyword,
                else_statement,
            },
            i,
        ))
    }

    /// Loop body (`loopState = InsideLoop`).
    fn loop_body(&mut self) -> R<NodeId> {
        let salvo = self.loop_;
        self.loop_ = LoopState::Repetition;
        let r = self.statement();
        self.loop_ = salvo;
        r
    }

    /// `parseWhileStatement`.
    fn while_(&mut self) -> R<NodeId> {
        let w = self.advance();
        let (lp, condition, _, rp) = self.parenthesized_condition(false)?;
        let body = self.loop_body()?;
        Ok(self.node(
            Node::WhileStatement {
                while_keyword: w as Tk,
                left_parenthesis: lp as Tk,
                condition,
                right_parenthesis: rp as Tk,
                body,
            },
            w,
        ))
    }

    /// `parseDoWhileStatement`.
    fn do_(&mut self) -> R<NodeId> {
        let d = self.advance();
        let body = self.loop_body()?;
        let w = self.expect_kw(Keyword::While)?;
        let (lp, condition, _, rp) = self.parenthesized_condition(false)?;
        let s = self.expect_op(Op::Semicolon)?;
        Ok(self.node(
            Node::DoStatement {
                do_keyword: d as Tk,
                body,
                while_keyword: w as Tk,
                left_parenthesis: lp as Tk,
                condition,
                right_parenthesis: rp as Tk,
                semicolon: s as Tk,
            },
            d,
        ))
    }

    /// `parseForStatement`.
    fn to(&mut self, await_keyword: Option<usize>) -> R<NodeId> {
        let start_ = await_keyword.unwrap_or(self.pos);
        let f = self.expect_kw(Keyword::For)?;
        let (lp, for_loop_parts, rp) = self.for_parts(await_keyword)?;
        let body = self.loop_body()?;
        Ok(self.node(
            Node::ForStatement {
                await_keyword: await_keyword.map(|a| a as Tk),
                for_keyword: f as Tk,
                left_parenthesis: lp as Tk,
                for_loop_parts,
                right_parenthesis: rp as Tk,
                body,
            },
            start_,
        ))
    }

    /// `parseExpressionStatementOrDeclaration` with `forPartsContext`.
    fn for_start(&mut self) -> R<ForStart> {
        let d = self.local_modifiers()?;
        if self.patterns_
            && let Some(v) = d.vfc
            && (self.kw(v, Keyword::Var) || self.kw(v, Keyword::Final))
            && let Some(f) = self.skip_outer_pattern(d.before_type)
            && (self.op(f + 1, Op::Assign) || self.kw(f + 1, Keyword::In))
        {
            if d.late.is_some() {
                return Err(self.error_at(
                    d.late.unwrap_or(0),
                    "A pattern variable declaration may not use the `late` keyword.",
                ));
            }
            let pattern = self.pattern(PatternCtx::Declaration)?;
            return Ok(ForStart::Pattern(d.meta, v, pattern, d.start));
        }
        let mut ti = self.type_at(d.before_type, false, false, false);
        if let Some(l) = d.late {
            return Err(self.error_at(l, "Can't have modifier 'late' here."));
        }
        if d.without_prefix()
            && ti.nullable()
            && ti.could_be_expression
            && self.question_and_conditional(ti)?
        {
            ti = TypeInfo::none_(d.before_type);
        }
        if ti.is_none() && d.without_prefix() {
            return Ok(ForStart::Expression);
        }
        let next = ti.end_index;
        if self.built_in(next) && d.without_prefix() && ti.could_be_expression && self.word(next, "as") {
            let k = next + 1;
            if !(self.op(k, Op::Assign) || self.op(k, Op::Semicolon) || self.op(k, Op::Comma))
                && !self.kw(k, Keyword::In)
            {
                return Ok(ForStart::Expression);
            }
        }
        self.check_decl(&d, &ti)?;
        let type_ = if ti.is_none() {
            None
        } else {
            Some(self.type_until(ti.end_index)?)
        };
        Ok(ForStart::Declaration(d, type_))
    }

    /// The `( ... )` header of `for` in statements and collections.
    pub(super) fn for_parts(&mut self, await_keyword: Option<usize>) -> R<(usize, NodeId, usize)> {
        let lp = self.expect_op(Op::LParen)?;
        // `AwaitForNotAsync` is not a syntax error in the analyzer.
        let check_await_in = |_: &Self| -> R<()> { Ok(()) };
        match self.for_start()? {
            ForStart::Pattern(meta, keyword, pattern, start_) => {
                if self.op(self.pos, Op::Assign) {
                    let eq = self.advance();
                    let expression = self.expression()?;
                    let variables = self.node(
                        Node::PatternVariableDeclaration {
                            metadata: meta,
                            keyword: keyword as Tk,
                            pattern,
                            equals: eq as Tk,
                            expression,
                        },
                        start_,
                    );
                    let parts =
                        self.for_rest(await_keyword, start_, |l, c, r, u| Node::ForPartsWithPattern {
                            variables,
                            left_separator: l,
                            condition: c,
                            right_separator: r,
                            updaters: u,
                        })?;
                    let rp = self.expect_op(Op::RParen)?;
                    return Ok((lp, parts, rp));
                }
                let in_kw = self.expect_kw(Keyword::In)?;
                check_await_in(self)?;
                let iterable = self.expression()?;
                let parts = self.node(
                    Node::ForEachPartsWithPattern {
                        metadata: meta,
                        keyword: keyword as Tk,
                        pattern,
                        in_keyword: in_kw as Tk,
                        iterable,
                    },
                    start_,
                );
                let rp = self.expect_op(Op::RParen)?;
                Ok((lp, parts, rp))
            }
            ForStart::Declaration(d, type_) => {
                if self.ident(self.pos)
                    && (self.kw(self.pos + 1, Keyword::In) || self.op(self.pos + 1, Op::Colon))
                {
                    if self.op(self.pos + 1, Op::Colon) {
                        return Err(self
                            .error_at(self.pos + 1, "For-in loops use 'in' rather than a colon."));
                    }
                    let n = self.expect_declared_name()?;
                    let start_ = d.start;
                    let loop_variable = self.node(
                        Node::DeclaredIdentifier {
                            metadata: d.meta,
                            keyword: d.vfc.map(|v| v as Tk),
                            type_,
                            name: n as Tk,
                        },
                        start_,
                    );
                    let in_kw = self.advance();
                    check_await_in(self)?;
                    let iterable = self.expression()?;
                    let parts = self.node(
                        Node::ForEachPartsWithDeclaration {
                            loop_variable,
                            in_keyword: in_kw as Tk,
                            iterable,
                        },
                        start_,
                    );
                    let rp = self.expect_op(Op::RParen)?;
                    return Ok((lp, parts, rp));
                }
                let start_ = d.start;
                let variables = self.variables_(d, type_)?;
                let parts = self.for_rest(await_keyword, start_, |l, c, r, u| {
                    Node::ForPartsWithDeclarations {
                        variables,
                        left_separator: l,
                        condition: c,
                        right_separator: r,
                        updaters: u,
                    }
                })?;
                let rp = self.expect_op(Op::RParen)?;
                Ok((lp, parts, rp))
            }
            ForStart::Expression => {
                let start_ = self.pos;
                let initialization = if self.op(self.pos, Op::Semicolon) {
                    None
                } else {
                    Some(self.expression()?)
                };
                if self.kw(self.pos, Keyword::In) || self.op(self.pos, Op::Colon) {
                    if self.op(self.pos, Op::Colon) {
                        return Err(self.error("For-in loops use 'in' rather than a colon."));
                    }
                    let Some(identifier) = initialization.filter(|&e| self.is_simple(e)) else {
                        return Err(self
                            .error_at(start_, "A for-in loop variable must be a simple identifier."));
                    };
                    let in_kw = self.advance();
                    check_await_in(self)?;
                    let iterable = self.expression()?;
                    let parts = self.node(
                        Node::ForEachPartsWithIdentifier {
                            identifier,
                            in_keyword: in_kw as Tk,
                            iterable,
                        },
                        start_,
                    );
                    let rp = self.expect_op(Op::RParen)?;
                    return Ok((lp, parts, rp));
                }
                let parts = self.for_rest(await_keyword, start_, |l, c, r, u| {
                    Node::ForPartsWithExpression {
                        initialization,
                        left_separator: l,
                        condition: c,
                        right_separator: r,
                        updaters: u,
                    }
                })?;
                let rp = self.expect_op(Op::RParen)?;
                Ok((lp, parts, rp))
            }
        }
    }

    /// `parseForLoopPartsRest`: `; condition? ; updaters`.
    fn for_rest(
        &mut self,
        await_keyword: Option<usize>,
        start_: usize,
        assemble: impl FnOnce(Tk, Option<NodeId>, Tk, Vec<NodeId>) -> Node,
    ) -> R<NodeId> {
        if let Some(a) = await_keyword {
            return Err(self.error_at(
                a,
                "The keyword 'await' isn't allowed for a normal 'for' statement.",
            ));
        }
        let l = self.expect_op(Op::Semicolon)?;
        let condition = if self.op(self.pos, Op::Semicolon) {
            None
        } else {
            Some(self.expression()?)
        };
        let r = self.expect_op(Op::Semicolon)?;
        let mut updaters = Vec::new();
        loop {
            if self.op(self.pos, Op::RParen) {
                break;
            }
            updaters.push(self.expression()?);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.advance();
        }
        // `endToken` is the last updater (without the trailing comma) or the `;`.
        let end = updaters.last().map_or(r, |&u| self.end(u));
        let node = assemble(l as Tk, condition, r as Tk, updaters);
        Ok(self.node_between(node, start_, end))
    }

    /// `parseSwitchStatement` e `parseSwitchBlock`.
    fn choice(&mut self) -> R<NodeId> {
        let sw = self.advance();
        let (lp, expression, _, rp) = self.parenthesized_condition(false)?;
        let salvo = self.loop_;
        if self.loop_ == LoopState::Fora {
            self.loop_ = LoopState::Switch;
        }
        let lb = self.expect_op(Op::LBrace)?;
        let mut members_ = Vec::new();
        let mut saw_default = false;
        while !self.op(self.pos, Op::RBrace) {
            if self.eof(self.pos) {
                return Err(self.expected("}"));
            }
            let mut group: Vec<SwitchMember> = Vec::new();
            let mut peek = self.past_labels(self.pos);
            loop {
                if self.kw(peek, Keyword::Default) || self.kw(peek, Keyword::Case) {
                    let start_ = self.pos;
                    let mut labels = Vec::new();
                    while self.pos < peek {
                        let n = self.advance();
                        let s = self.simple(n);
                        let c = self.advance();
                        labels.push(self.node(
                            Node::Label {
                                label: s,
                                colon: c as Tk,
                            },
                            n,
                        ));
                    }
                    if self.kw(peek, Keyword::Default) {
                        if saw_default {
                            return Err(self.error("The 'default' case can only be declared once."));
                        }
                        saw_default = true;
                        let d = self.advance();
                        let c = self.expect_op(Op::Colon)?;
                        group.push((labels, d, None, c, start_));
                        peek = self.pos;
                        break;
                    }
                    if saw_default {
                        return Err(self.error(
                            "The default case should be the last case in a switch statement.",
                        ));
                    }
                    let k = self.advance();
                    let e = if self.patterns_ {
                        self.guarded_pattern_()?
                    } else {
                        let e = self.expression()?;
                        if self.word(self.pos, "when") {
                            return Err(self.error("Guards ('when') require patterns."));
                        }
                        e
                    };
                    let c = self.expect_op(Op::Colon)?;
                    group.push((labels, k, Some(e), c, start_));
                    peek = self.past_labels(self.pos);
                } else if !group.is_empty() {
                    break;
                } else {
                    return Err(self.error_at(peek, "Expected 'case'."));
                }
            }
            let mut statements = Vec::new();
            while !self.eof(self.pos) {
                if self.kw(peek, Keyword::Case)
                    || self.kw(peek, Keyword::Default)
                    || (self.op(peek, Op::RBrace) && peek == self.pos)
                {
                    break;
                }
                statements.push(self.statement()?);
                peek = self.past_labels(self.pos);
            }
            let n = group.len();
            for (j, (labels, k, e, c, start_)) in group.into_iter().enumerate() {
                let sts = if j + 1 == n {
                    std::mem::take(&mut statements)
                } else {
                    Vec::new()
                };
                let end = sts.last().map_or(c, |&s| self.end(s));
                let node = match e {
                    None => Node::SwitchDefault {
                        labels,
                        keyword: k as Tk,
                        colon: c as Tk,
                        statements: sts,
                    },
                    Some(g) if self.patterns_ => Node::SwitchPatternCase {
                        labels,
                        keyword: k as Tk,
                        guarded_pattern: g,
                        colon: c as Tk,
                        statements: sts,
                    },
                    Some(x) => Node::SwitchCase {
                        labels,
                        keyword: k as Tk,
                        expression: x,
                        colon: c as Tk,
                        statements: sts,
                    },
                };
                members_.push(self.node_between(node, start_, end));
            }
        }
        let rb = self.advance();
        self.loop_ = salvo;
        Ok(self.node(
            Node::SwitchStatement {
                switch_keyword: sw as Tk,
                left_parenthesis: lp as Tk,
                expression,
                right_parenthesis: rp as Tk,
                left_bracket: lb as Tk,
                members: members_,
                right_bracket: rb as Tk,
            },
            sw,
        ))
    }

    /// `peekPastLabels`.
    fn past_labels(&self, mut i: usize) -> usize {
        while self.ident(i) && self.op(i + 1, Op::Colon) {
            i += 2;
        }
        i
    }

    /// `parseTryStatement`.
    fn attempt(&mut self) -> R<NodeId> {
        let t = self.advance();
        let body = self.block()?;
        let mut catch_clauses = Vec::new();
        while self.word(self.pos, "on") || self.kw(self.pos, Keyword::Catch) {
            let ci = self.pos;
            let mut on_keyword = None;
            let mut exception_type = None;
            if self.word(self.pos, "on") {
                on_keyword = Some(self.advance() as Tk);
                exception_type = Some(self.required_type()?);
            }
            let mut catch_keyword = None;
            let mut lp = None;
            let mut exception_parameter = None;
            let mut comma = None;
            let mut stack_trace_parameter = None;
            let mut rp = None;
            if self.kw(self.pos, Keyword::Catch) {
                catch_keyword = Some(self.advance() as Tk);
                lp = Some(self.expect_op(Op::LParen)? as Tk);
                let e = self.expect_declared_name()?;
                exception_parameter = Some(self.node(Node::CatchClauseParameter { name: e as Tk }, e));
                if self.op(self.pos, Op::Comma) {
                    comma = Some(self.advance() as Tk);
                    let s = self.expect_declared_name()?;
                    stack_trace_parameter =
                        Some(self.node(Node::CatchClauseParameter { name: s as Tk }, s));
                }
                rp = Some(self.expect_op(Op::RParen)? as Tk);
            }
            let b = self.block()?;
            catch_clauses.push(self.node(
                Node::CatchClause {
                    on_keyword,
                    exception_type,
                    catch_keyword,
                    left_parenthesis: lp,
                    exception_parameter,
                    comma,
                    stack_trace_parameter,
                    right_parenthesis: rp,
                    body: b,
                },
                ci,
            ));
        }
        let (finally_keyword, finally_block) = if self.kw(self.pos, Keyword::Finally) {
            let f = self.advance();
            (Some(f as Tk), Some(self.block()?))
        } else {
            (None, None)
        };
        if catch_clauses.is_empty() && finally_block.is_none() {
            return Err(self.error_at(
                t,
                "A try block must be followed by an 'on', 'catch', or 'finally' clause.",
            ));
        }
        Ok(self.node(
            Node::TryStatement {
                try_keyword: t as Tk,
                body,
                catch_clauses,
                finally_keyword,
                finally_block,
            },
            t,
        ))
    }

    /// `parseBreakStatement` e `parseContinueStatement`.
    fn split(&mut self, is_break: bool) -> R<NodeId> {
        let k = self.advance();
        let label = if self.ident(self.pos) {
            let n = self.advance();
            Some(self.simple(n))
        } else {
            None
        };
        if is_break {
            if label.is_none() && self.loop_ == LoopState::Fora {
                return Err(self.error_at(
                    k,
                    "A break statement can't be used outside of a loop or switch statement.",
                ));
            }
        } else if label.is_some() {
            if self.loop_ == LoopState::Fora {
                return Err(self.error_at(
                    k,
                    "A continue statement can't be used outside of a loop or switch statement.",
                ));
            }
        } else if self.loop_ != LoopState::Repetition {
            return Err(self.error_at(
                k,
                "A continue statement can't be used outside of a loop or switch statement.",
            ));
        }
        let s = self.expect_op(Op::Semicolon)?;
        let node = if is_break {
            Node::BreakStatement {
                break_keyword: k as Tk,
                label,
                semicolon: s as Tk,
            }
        } else {
            Node::ContinueStatement {
                continue_keyword: k as Tk,
                label,
                semicolon: s as Tk,
            }
        };
        Ok(self.node(node, k))
    }

    /// `assert ( condition (, message)? ,? )` — the parts shared by
    /// [`Node::AssertStatement`] and [`Node::AssertInitializer`].
    pub(super) fn assert_parts(&mut self) -> R<AssertParts> {
        let a = self.expect_kw(Keyword::Assert)?;
        let lp = self.expect_op(Op::LParen)?;
        let old = self.functions;
        self.functions = true;
        let condition = self.expression()?;
        let mut comma = None;
        let mut message = None;
        if self.op(self.pos, Op::Comma) {
            let c = self.advance();
            if !self.op(self.pos, Op::RParen) {
                comma = Some(c);
                message = Some(self.expression()?);
                if self.op(self.pos, Op::Comma) {
                    self.advance();
                }
            }
        }
        let rp = self.expect_op(Op::RParen)?;
        self.functions = old;
        Ok((a, lp, condition, comma, message, rp))
    }

    /// `parseAssertStatement`.
    fn assertion(&mut self) -> R<NodeId> {
        let (a, lp, condition, comma, message, rp) = self.assert_parts()?;
        let s = self.expect_op(Op::Semicolon)?;
        Ok(self.node(
            Node::AssertStatement {
                assert_keyword: a as Tk,
                left_parenthesis: lp as Tk,
                condition,
                comma: comma.map(|c| c as Tk),
                message,
                right_parenthesis: rp as Tk,
                semicolon: s as Tk,
            },
            a,
        ))
    }

    /// `parseAsyncModifierOpt` and `parseFunctionBody`. `from_expression` is the
    /// body of a function expression (the `=>` has no `;`).
    pub(super) fn function_body_(&mut self, from_expression: bool, allows_abstract: bool) -> R<NodeId> {
        let (async_kind, loop_) = (self.async_kind, self.loop_);
        let r = self.inner_function_body(from_expression, allows_abstract);
        self.async_kind = async_kind;
        self.loop_ = loop_;
        r
    }

    fn inner_function_body(&mut self, from_expression: bool, allows_abstract: bool) -> R<NodeId> {
        let mut keyword = None;
        let mut star = None;
        self.async_kind = AsyncKind::SyncKind;
        if self.word(self.pos, "async") {
            keyword = Some(self.advance());
            if self.op(self.pos, Op::Star) {
                star = Some(self.advance());
                self.async_kind = AsyncKind::Star;
            } else {
                self.async_kind = AsyncKind::Plain;
            }
        } else if self.word(self.pos, "sync") {
            keyword = Some(self.advance());
            if self.op(self.pos, Op::Star) {
                star = Some(self.advance());
                self.async_kind = AsyncKind::SyncStar;
            } else {
                return Err(self.error_at(keyword.unwrap_or(0), "Invalid modifier 'sync'."));
            }
        }
        let start_ = keyword.unwrap_or(self.pos);
        if self.word(self.pos, "native") {
            let n = self.advance();
            let string_literal = if self.text(self.pos) {
                Some(self.string_literal()?)
            } else {
                None
            };
            let s = self.expect_op(Op::Semicolon)?;
            return Ok(self.node(
                Node::NativeFunctionBody {
                    native_keyword: n as Tk,
                    string_literal,
                    semicolon: s as Tk,
                },
                n,
            ));
        }
        if self.op(self.pos, Op::Semicolon) {
            if !allows_abstract {
                return Err(self.error("A function body must be provided."));
            }
            let s = self.advance();
            return Ok(self.node(Node::EmptyFunctionBody { semicolon: s as Tk }, s));
        }
        if self.op(self.pos, Op::Arrow) {
            let a = self.advance();
            let expression = self.expression()?;
            let semicolon = if from_expression {
                None
            } else {
                Some(self.expect_op(Op::Semicolon)? as Tk)
            };
            return Ok(self.node(
                Node::ExpressionFunctionBody {
                    keyword: keyword.map(|k| k as Tk),
                    star: star.map(|s| s as Tk),
                    function_definition: a as Tk,
                    expression,
                    semicolon,
                },
                start_,
            ));
        }
        if !self.op(self.pos, Op::LBrace) {
            return Err(self.error("A function body must be provided."));
        }
        self.loop_ = LoopState::Fora;
        let block = self.block()?;
        Ok(self.node(
            Node::BlockFunctionBody {
                keyword: keyword.map(|k| k as Tk),
                star: star.map(|s| s as Tk),
                block,
            },
            start_,
        ))
    }
}

#[allow(non_snake_case)]
fn EmptyStatement(i: usize) -> Node {
    Node::EmptyStatement { semicolon: i as Tk }
}
