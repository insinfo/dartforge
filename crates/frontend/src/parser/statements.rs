//! Statements, local declarations and `for` headers.
//!
//! Covers `statement` from the Dart 3.6 grammar: block, local variable
//! declaration (including pattern ones), local function, `for`/`await for`,
//! `while`, `do`, `switch` (with patterns and guards), `if` (with `case`),
//! `try`, `break`, `continue`, `return`, `yield`, `assert`, labels, empty
//! `;` and expression statement.
//!
//! Disambiguation, in the order the SDK's real parser (`parseStatementX`)
//! does it:
//!
//! * `name:` at the start is a label;
//! * `{` is a block, except when the matching `}` is followed by `=` (a map
//!   pattern assignment, which is an expression);
//! * `var`/`final` followed by `(`, `[` or `{` start a pattern declaration;
//!   `late`, `final`, `const`, `var` and `type name` start a variable
//!   declaration; `type? name(...)` followed by `{`, `=>`, `async` or `sync`
//!   is a local function — the only difference from the call `name(args);`
//!   is what comes after the `)`;
//! * `await` and `yield` are only special in `async`/generator bodies, but
//!   `await for` is always a loop and `yield e;` outside a generator is
//!   accepted as `yield` (no valid expression statement has that shape);
//! * the rest is an expression statement.
//!
//! The classic ambiguity `a ? b : c;` versus `int? x = e;` is resolved as in
//! the SDK: a `?` right after a simple name is only a nullable type if the
//! following identifier is followed by `;`, `,`, `)`, `in` or by `=` whose
//! expression is **not** followed by `:` (that needs a speculative parse of
//! the expression, which is discarded afterwards).
//!
//! Recovery: inside a block or a `case`, a statement that fails makes the
//! parser skip to the next `;` at the same level or to the `}` that closes
//! the block, and go on. The diagnostic was already recorded at the point of
//! failure.
//!
//! Contract limitations (see the report): metadata before a local
//! declaration (`@pragma(...) final x = 1;`) goes to the side table
//! `Ast::local_metadata` because [`StmtKind::Variables`] and
//! [`StmtKind::Function`] have no field for it; and
//! `for (var (a, b) = e; ...; ...)` (a pattern declaration as the classic
//! initializer) does not fit in [`ForInit`] and produces an error.
use super::{ForHeader, PResult, ParseError, Parser};
use crate::ast::{
    CatchClause, Expr, ExprKind, ForInTarget, ForInit, Function, FunctionKind, Stmt, StmtId,
    StmtKind, SwitchCase, TypeId, Variable, VariableList,
};
use crate::token::{Keyword, Kind, Op};
use dartforge_diagnostics::{Span, codes};

impl<'s, 'i> Parser<'s, 'i> {
    /// Full `statement`, with labels.
    pub(crate) fn parse_statement(&mut self) -> PResult<StmtId> {
        self.enter()?;
        let result = self.parse_statement_inner();
        self.leave();
        result
    }

    /// `{ stmts }` with the braces, recovering from malformed statements.
    pub(crate) fn parse_block(&mut self) -> PResult<StmtId> {
        self.parse_block_with(false)
    }

    /// The block of a function body (`parseFunctionBody`), whose loop reports a
    /// statement without progress at the last token read.
    pub(crate) fn parse_body_block(&mut self) -> PResult<StmtId> {
        self.parse_block_with(true)
    }

    fn parse_block_with(&mut self, body_node: bool) -> PResult<StmtId> {
        let start = self.expect_op(Op::LBrace)?.span;
        let stmts = self.parse_statement_list_with(false, body_node);
        if !self.at_op(Op::RBrace) {
            return Err(self.expected_error("}"));
        }
        self.advance();
        Ok(self.push_stmt(start, StmtKind::Block(stmts.into_boxed_slice())))
    }

    /// `(init; cond; upd)` or `(decl in e)`, with the parentheses, after `for`
    /// (and after `await`, which the caller consumes).
    pub(crate) fn parse_for_header(&mut self) -> PResult<ForHeader> {
        self.expect_op(Op::LParen)?;
        let metadata = if self.at_op(Op::At) {
            self.parse_metadata()?
        } else {
            Vec::new()
        };

        // `for (final (a, b) in e)` — a pattern declaration.
        if (self.at_kw(Keyword::Var) || self.at_kw(Keyword::Final))
            && self.looks_like_pattern_declaration(self.pos)
        {
            let final_ = self.at_kw(Keyword::Final);
            self.advance();
            let pattern = self.parse_pattern_in(super::patterns::PatternContext::Declaration)?;
            // `for (var (a, b) = e; c; u)`: with `=`, the pattern is the
            // initialization of a classic `for`
            // (`parseForStatement`, `handleForInitializerPatternVariableAssignment`).
            if self.eat_op(Op::Assign) {
                let value = self.parse_expression()?;
                return self.parse_classic_for_rest(Some(ForInit::Pattern {
                    final_,
                    pattern,
                    value,
                }));
            }
            self.expect_kw(Keyword::In)?;
            let iterable = self.parse_expression()?;
            self.expect_op(Op::RParen)?;
            return Ok(ForHeader::In {
                target: ForInTarget::Pattern { final_, pattern },
                iterable,
            });
        }

        // Variable declaration: with a modifier or with a type.
        let has_modifier =
            self.at_kw(Keyword::Var) || self.at_kw(Keyword::Final) || self.at_kw(Keyword::Const);
        if has_modifier || self.declaration_type_at(self.pos, true) {
            let mut final_ = false;
            let mut var_ = false;
            let mut const_ = false;
            let mut const_word = None;
            if self.eat_kw(Keyword::Final) {
                final_ = true;
            } else if self.eat_kw(Keyword::Var) {
                var_ = true;
            } else if self.at_kw(Keyword::Const) {
                const_word = Some(self.peek().span);
                self.advance();
                const_ = true;
            }
            let ty = if !var_ && self.declaration_type_at(self.pos, !has_modifier) {
                Some(self.parse_type()?)
            } else {
                None
            };
            let name = self.expect_identifier()?;
            if self.eat_kw(Keyword::In) {
                let iterable = self.parse_expression()?;
                self.expect_op(Op::RParen)?;
                return Ok(ForHeader::In {
                    target: ForInTarget::Declared {
                        metadata: metadata.into_boxed_slice(),
                        final_,
                        var_,
                        ty,
                        name,
                        const_: const_word,
                    },
                    iterable,
                });
            }
            let mut variables = Vec::new();
            let initializer = self.parse_initializer_opt()?;
            variables.push(Variable { name, initializer });
            while self.eat_op(Op::Comma) {
                let name = self.expect_identifier()?;
                let initializer = self.parse_initializer_opt()?;
                variables.push(Variable { name, initializer });
            }
            let list = VariableList {
                external: false,
                static_: false,
                abstract_: false,
                covariant: false,
                late: false,
                final_,
                const_,
                var_,
                ty,
                variables: variables.into_boxed_slice(),
            };
            return self.parse_classic_for_rest(Some(ForInit::Variables(list)));
        }

        if self.at_op(Op::Semicolon) {
            return self.parse_classic_for_rest(None);
        }

        // `for (x in e)` or `for (e; ...; ...)`.
        let expr = self.parse_expression()?;
        if self.eat_kw(Keyword::In) {
            let iterable = self.parse_expression()?;
            self.expect_op(Op::RParen)?;
            return Ok(ForHeader::In {
                target: ForInTarget::Expression(expr),
                iterable,
            });
        }
        self.parse_classic_for_rest(Some(ForInit::Expression(expr)))
    }

    // -- Statement lists and recovery ------------------------------------------

    /// Statements up to `}`/end (or up to the next `case`/`default` when
    /// `in_switch`), with per-statement recovery.
    fn parse_statement_list(&mut self, in_switch: bool) -> Vec<StmtId> {
        self.parse_statement_list_with(in_switch, false)
    }

    fn parse_statement_list_with(&mut self, in_switch: bool, body_node: bool) -> Vec<StmtId> {
        let mut stmts = Vec::new();
        loop {
            if self.at_eof() || self.at_op(Op::RBrace) {
                break;
            }
            if in_switch && self.at_switch_case_start() {
                break;
            }
            let statement_start = self.pos;
            match self.parse_statement() {
                Ok(id) => {
                    stmts.push(id);
                    // `parseBlock`/`parseFunctionBody`/`parseStatementsInSwitchCase`: without
                    // progress, `UNEXPECTED_TOKEN` and the token is skipped. In a function body
                    // the text is that of the last token read (the statement's synthetic `;`);
                    // in the others, that of the skipped token.
                    if self.pos == statement_start && !self.at_eof() {
                        let text = if body_node {
                            ";".to_string()
                        } else {
                            self.text().to_string()
                        };
                        self.error(codes::parser::UNEXPECTED_TOKEN, &[&text]);
                        self.advance();
                    }
                }
                Err(ParseError) => {
                    self.synchronize_statement(in_switch);
                    self.record_skipped(statement_start);
                }
            }
        }
        stmts
    }

    /// Skips tokens up to the next `;` at the same level (consumed) or up to the
    /// `}` that closes the current block (not consumed). Inside a `switch` it
    /// also stops at `case`/`default`. Always advances at least one token when
    /// the current one is not a stopping point, guaranteeing progress.
    fn synchronize_statement(&mut self, in_switch: bool) {
        let mut depth = 0usize;
        loop {
            match self.kind() {
                Kind::Eof => return,
                Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => depth += 1,
                Kind::Op(Op::RParen | Op::RBracket) => depth = depth.saturating_sub(1),
                Kind::Op(Op::RBrace) => {
                    if depth == 0 {
                        return;
                    }
                    depth -= 1;
                }
                Kind::Op(Op::Semicolon) if depth == 0 => {
                    self.advance();
                    return;
                }
                Kind::Keyword(Keyword::Case | Keyword::Default) if in_switch && depth == 0 => {
                    return;
                }
                _ => {}
            }
            self.advance();
        }
    }

    /// The current token, going through `name:` labels, is `case` or `default`.
    fn at_switch_case_start(&self) -> bool {
        let mut i = self.pos;
        while self.kind_of(i) == Kind::Ident && self.kind_of(i + 1) == Kind::Op(Op::Colon) {
            i += 2;
        }
        matches!(
            self.kind_of(i),
            Kind::Keyword(Keyword::Case | Keyword::Default)
        )
    }

    // -- Statement --------------------------------------------------------

    fn parse_statement_inner(&mut self) -> PResult<StmtId> {
        let start = self.span();

        if self.at_label() {
            let mut labels = Vec::new();
            while self.at_label() {
                labels.push(self.identifier());
                self.advance();
            }
            let body = self.parse_statement()?;
            return Ok(self.push_stmt(
                start,
                StmtKind::Labeled {
                    labels: labels.into_boxed_slice(),
                    body,
                },
            ));
        }

        match self.kind() {
            Kind::Op(Op::LBrace) => {
                if self.looks_like_pattern_assignment(self.pos) {
                    self.parse_expression_statement(start)
                } else {
                    self.parse_block()
                }
            }
            Kind::Op(Op::Semicolon) => {
                self.advance();
                Ok(self.push_stmt(start, StmtKind::Empty))
            }
            Kind::Op(Op::At) => self.parse_annotated_declaration(start),
            Kind::Op(Op::LParen) => {
                if !self.looks_like_pattern_assignment(self.pos)
                    && self.declaration_type_at(self.pos, true)
                {
                    self.parse_typed_declaration(start)
                } else {
                    self.parse_expression_statement(start)
                }
            }
            // `)` and `]`: an expression statement with the synthetic identifier (the
            // block loop skips the token without progress).
            Kind::Op(Op::RParen | Op::RBracket) => self.parse_expression_statement(start),
            // `}` and the end in place of a statement (the body of `if (x) }`, a
            // label's statement): `parseExpressionStatement` with the synthetic
            // identifier (`MISSING_IDENTIFIER` at the token) and the missing `;` at the
            // previous token, without consuming the `}`.
            Kind::Op(Op::RBrace) | Kind::Eof => self.parse_expression_statement(start),
            Kind::ScriptTag => Err(self.statement_error()),
            Kind::Keyword(kw) => self.parse_keyword_statement(start, kw),
            Kind::Ident => self.parse_identifier_statement(start),
            _ => self.parse_expression_statement(start),
        }
    }

    /// Statement starting with a reserved word.
    fn parse_keyword_statement(&mut self, start: Span, kw: Keyword) -> PResult<StmtId> {
        match kw {
            Keyword::If => self.parse_if(start),
            Keyword::For => {
                self.advance();
                self.parse_for_rest(start, false)
            }
            Keyword::While => self.parse_while(start),
            Keyword::Do => self.parse_do(start),
            Keyword::Switch => self.parse_switch(start),
            Keyword::Try => self.parse_try(start),
            Keyword::Break => {
                self.advance();
                let label = if self.at_identifier() {
                    Some(self.identifier())
                } else {
                    None
                };
                self.expect_semicolon()?;
                Ok(self.push_stmt(start, StmtKind::Break(label)))
            }
            Keyword::Continue => {
                self.advance();
                let label = if self.at_identifier() {
                    Some(self.identifier())
                } else {
                    None
                };
                self.expect_semicolon()?;
                Ok(self.push_stmt(start, StmtKind::Continue(label)))
            }
            Keyword::Return => {
                self.advance();
                let value = if self.at_op(Op::Semicolon) {
                    None
                } else {
                    Some(self.parse_expression()?)
                };
                self.expect_semicolon()?;
                Ok(self.push_stmt(start, StmtKind::Return(value)))
            }
            Keyword::Rethrow => {
                self.advance();
                let expr = self.ast.push_expr(Expr {
                    span: self.span_from(start),
                    kind: ExprKind::Rethrow,
                });
                self.expect_semicolon()?;
                Ok(self.push_stmt(start, StmtKind::Expression(expr)))
            }
            Keyword::Assert => self.parse_assert(start),
            Keyword::Var | Keyword::Final => self.parse_local_declaration(start),
            Keyword::Const => {
                // `const x = 1;` / `const int x = 1;` are declarations;
                // `const Foo();` / `const [1].length;` are expressions.
                // `const x;` and `const x, y = 1;` too (a `const` without an initializer is
                // an error of the next phase, `CONST_NOT_INITIALIZED`).
                let declaration = self.declaration_type_at(self.pos + 1, false)
                    || (self.at_identifier_at(1)
                        && matches!(
                            self.kind_at(2),
                            Kind::Op(Op::Assign | Op::Semicolon | Op::Comma)
                        ));
                if declaration {
                    self.parse_local_declaration(start)
                } else {
                    self.parse_expression_statement(start)
                }
            }
            Keyword::Void => self.parse_typed_declaration(start),
            // `parseStatementX` does not handle them: an expression statement, where
            // `ensureIdentifier` uses the word as a name (or inserts a synthetic one,
            // for those that start a statement and for `is`) and
            // `handleExpressionStatement` reports `MISSING_STATEMENT`.
            Keyword::Case
            | Keyword::Catch
            | Keyword::Class
            | Keyword::Default
            | Keyword::Else
            | Keyword::Enum
            | Keyword::Extends
            | Keyword::Finally
            | Keyword::In
            | Keyword::Is
            | Keyword::With => self.parse_expression_statement(start),
            _ => self.parse_expression_statement(start),
        }
    }

    /// Statement starting with an identifier (including the contextual `await`,
    /// `yield` and `late`), already without labels.
    fn parse_identifier_statement(&mut self, start: Span) -> PResult<StmtId> {
        if self.at_ident("await") {
            if self.at_kw_at(1, Keyword::For) {
                self.advance();
                self.advance();
                return self.parse_for_rest(start, true);
            }
            // `await e;` (and `await` as a plain name) is always an expression.
            return self.parse_expression_statement(start);
        }
        if self.at_ident("yield") && self.looks_like_yield_statement() {
            return self.parse_yield(start);
        }
        if self.at_ident("late")
            && (self.at_kw_at(1, Keyword::Final)
                || self.at_kw_at(1, Keyword::Var)
                || self.declaration_type_at(self.pos + 1, false))
        {
            return self.parse_local_declaration(start);
        }
        if self.looks_like_local_function(self.pos) {
            return self.parse_local_function(start, None);
        }
        if self.declaration_type_at(self.pos, true) {
            return self.parse_typed_declaration(start);
        }
        self.parse_expression_statement(start)
    }

    /// `yield` is a statement when we are in a generator or when what follows
    /// only makes sense as an operand (`yield* e`, `yield x`, `yield 1`…).
    fn looks_like_yield_statement(&self) -> bool {
        // fasta's `parseStatementX`: in `async`, `sync*` and `async*` it is always
        // the statement; in a plain synchronous body only when what follows looks
        // like an operand (`looksLikeYieldStatement`, the same rule as `await`):
        // `yield 0;` there is the `yield` expression with the missing `;`.
        if self.in_generator || self.in_async {
            return true;
        }
        self.looks_like_expression_after_await(self.pos, false)
    }

    /// Expression statement `e;`.
    fn parse_expression_statement(&mut self, start: Span) -> PResult<StmtId> {
        let expr = self.parse_expression()?;
        // `handleExpressionStatement` (`ast_builder.dart:4253`): `super;` and
        // assignment to a non-assignable target (this one, besides the
        // assignment's `MISSING_ASSIGNABLE_SELECTOR`).
        self.check_stray_super(expr);
        // `handleExpressionStatement`: a statement that is just a reserved word
        // used as a name is `MISSING_STATEMENT` at it.
        if let crate::ast::ExprKind::Identifier(n) = self.ast.expr(expr).kind
            && n.span.start < n.span.end
            && crate::token::Keyword::from_text(&self.source[n.span.start..n.span.end]).is_some()
        {
            self.error_at(codes::parser::MISSING_STATEMENT, n.span, &[]);
        }
        if let crate::ast::ExprKind::Assign { target, .. } = self.ast.expr(expr).kind
            && !self.is_assignable(target)
        {
            let span = self.ast.expr(target).span;
            self.error_at(
                codes::parser::ILLEGAL_ASSIGNMENT_TO_NON_ASSIGNABLE,
                span,
                &[],
            );
        }
        self.expect_semicolon()?;
        Ok(self.push_stmt(start, StmtKind::Expression(expr)))
    }

    /// `@annotation` before a local declaration. The metadata goes to the side
    /// table [`Ast::local_metadata`](crate::ast::Ast::local_metadata), by
    /// statement, because the statement nodes have no field for it.
    fn parse_annotated_declaration(&mut self, start: Span) -> PResult<StmtId> {
        let metadata = self.parse_metadata()?;
        let s = if self.at_kw(Keyword::Var)
            || self.at_kw(Keyword::Final)
            || self.at_kw(Keyword::Const)
            || self.at_ident("late")
        {
            self.parse_local_declaration(start)?
        } else if self.looks_like_local_function(self.pos) {
            self.parse_local_function(start, None)?
        } else if self.declaration_type_at(self.pos, false) {
            self.parse_typed_declaration(start)?
        } else {
            return Err(self.error(codes::parser::MISSING_STATEMENT, &[]));
        };
        if !metadata.is_empty() {
            self.ast
                .local_metadata
                .push((s, metadata.into_boxed_slice()));
        }
        Ok(s)
    }

    // -- Local declarations ----------------------------------------------------

    /// `late? (final | var | const)? type? name (= e)? (, name (= e)?)* ;` or,
    /// with `var`/`final` followed by `(`/`[`/`{`, a pattern declaration.
    fn parse_local_declaration(&mut self, start: Span) -> PResult<StmtId> {
        if (self.at_kw(Keyword::Var) || self.at_kw(Keyword::Final))
            && self.looks_like_pattern_declaration(self.pos)
        {
            return self.parse_pattern_variables(start);
        }
        let late = self.eat_ident("late");
        let mut final_ = false;
        let mut var_ = false;
        let mut const_ = false;
        if self.eat_kw(Keyword::Final) {
            final_ = true;
        } else if self.eat_kw(Keyword::Var) {
            var_ = true;
        } else if self.eat_kw(Keyword::Const) {
            const_ = true;
        }
        let ty = if !var_ && self.declaration_type_at(self.pos, false) {
            Some(self.parse_type()?)
        } else {
            None
        };
        if !late && !final_ && !var_ && !const_ && ty.is_none() {
            return Err(self.error(codes::parser::MISSING_CONST_FINAL_VAR_OR_TYPE, &[]));
        }
        let list = VariableList {
            external: false,
            static_: false,
            abstract_: false,
            covariant: false,
            late,
            final_,
            const_,
            var_,
            ty,
            variables: Box::default(),
        };
        self.parse_variables_rest(start, list)
    }

    /// `var (a, b) = e;` / `final [x, y] = e;`
    fn parse_pattern_variables(&mut self, start: Span) -> PResult<StmtId> {
        let final_ = self.at_kw(Keyword::Final);
        self.advance();
        let pattern = self.parse_pattern_in(super::patterns::PatternContext::Declaration)?;
        self.expect_op(Op::Assign)?;
        let value = self.parse_expression()?;
        self.expect_semicolon()?;
        Ok(self.push_stmt(
            start,
            StmtKind::PatternVariables {
                final_,
                pattern,
                value,
            },
        ))
    }

    /// Declaration starting with a type (`int x = 1;`, `void f() {}`,
    /// `List<int> Function() g() => ...;`).
    fn parse_typed_declaration(&mut self, start: Span) -> PResult<StmtId> {
        let ty = self.parse_type()?;
        if self.looks_like_local_function(self.pos) {
            return self.parse_local_function(start, Some(ty));
        }
        let list = VariableList {
            external: false,
            static_: false,
            abstract_: false,
            covariant: false,
            late: false,
            final_: false,
            const_: false,
            var_: false,
            ty: Some(ty),
            variables: Box::default(),
        };
        self.parse_variables_rest(start, list)
    }

    /// `name (= e)? (, name (= e)?)* ;` after the modifiers and the type.
    fn parse_variables_rest(&mut self, start: Span, mut list: VariableList) -> PResult<StmtId> {
        let mut variables = Vec::new();
        loop {
            let name = self.local_variable_name()?;
            self.check_await_yield_as_name(name.span);
            let initializer = self.parse_initializer_opt()?;
            variables.push(Variable { name, initializer });
            if !self.eat_op(Op::Comma) {
                break;
            }
        }
        list.variables = variables.into_boxed_slice();
        if !self.at_op(Op::Semicolon)
            && let Some(s) = self.prefix_without_semicolon(start, &list)
        {
            return Ok(s);
        }
        self.expect_semicolon()?;
        Ok(self.push_stmt(start, StmtKind::Variables(list)))
    }

    /// `LocalVariableDeclarationIdentifierContext.ensureIdentifier`
    /// (`identifier_context_impl.dart:868`): a reserved word that does not
    /// start a statement becomes the variable's name, with
    /// `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD` at it (`var enum;` declares
    /// `enum`); one that starts a statement gets `MISSING_IDENTIFIER` and an
    /// empty synthetic name before it (`var if;` is `var <synthetic>` and an
    /// `if`); anything else that is not an identifier is the usual error.
    fn local_variable_name(&mut self) -> PResult<crate::ast::Name> {
        if !self.at_identifier() && matches!(self.kind(), Kind::Keyword(_)) {
            if self.looks_like_statement_start(self.pos) {
                self.error(codes::parser::MISSING_IDENTIFIER, &[]);
                let here = self.span().start;
                return Ok(self.name_from(
                    "",
                    Span {
                        start: here,
                        end: here,
                    },
                ));
            }
            let text = self.text().to_string();
            let token = self.advance();
            self.error_at(
                codes::parser::EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD,
                token.span,
                &[&text],
            );
            return Ok(self.name_from(&text, token.span));
        }
        self.expect_identifier()
    }

    /// The `AstBuilder`'s `endVariablesDeclaration` (`ast_builder.dart:3601`,
    /// issue 53964): the declaration of a single variable of a prefixed type,
    /// without the `;` (which `ensureSemicolon` inserts), is the incomplete
    /// code `x.foo` followed by another statement on the next line. It becomes
    /// the expression statement `x.foo;` and the name goes back to the token
    /// stream; with `await` in place of the type name (`x.` and, below,
    /// `await y.foo();`), the expression is `x.` with a synthetic identifier
    /// and the `await` and the name go back. The modifiers, the `?`, the type
    /// arguments and the initializer already read are lost, as there.
    fn prefix_without_semicolon(&mut self, start: Span, list: &VariableList) -> Option<StmtId> {
        let [variable] = &*list.variables else {
            return None;
        };
        let crate::ast::TypeKind::Named { name, .. } = &self.ast.ty(list.ty?).kind else {
            return None;
        };
        let [prefix, name] = **name else {
            return None;
        };
        let index = |p: &Self, span: Span| (0..p.pos).rev().find(|&i| p.tokens[i].span == span);
        let i_type = index(self, name.span)?;
        let i_var = index(self, variable.name.span)?;
        let (member, reinjected) = if self.text_of(i_type) == "await" {
            let s = self.tokens[i_type].span.start;
            (
                self.name_from("", Span { start: s, end: s }),
                vec![i_type, i_var],
            )
        } else if self.is_plain_identifier(i_var) || self.text_of(i_var) == "await" {
            (name, vec![i_var])
        } else {
            return None;
        };
        self.expected_error(";");
        let target_expr = self.ast.push_expr(Expr {
            span: prefix.span,
            kind: ExprKind::Identifier(prefix),
        });
        let expr = self.ast.push_expr(Expr {
            span: Span {
                start: prefix.span.start,
                end: member.span.end,
            },
            kind: ExprKind::Property {
                target: target_expr,
                name: member,
                null_aware: false,
            },
        });
        let s = self.push_stmt(start, StmtKind::Expression(expr));
        self.reinject(&reinjected);
        Some(s)
    }

    /// `checkAsyncAwaitYieldAsIdentifier` (`identifier_context_impl.dart:1387`):
    /// outside a plain synchronous body, `await`/`yield` as a declared name is
    /// `ASYNC_KEYWORD_USED_AS_IDENTIFIER`.
    pub(crate) fn check_await_yield_as_name(&mut self, name: Span) {
        if (self.in_async || self.in_generator)
            && matches!(&self.source[name.start..name.end], "await" | "yield")
        {
            self.error_at(codes::parser::ASYNC_KEYWORD_USED_AS_IDENTIFIER, name, &[]);
        }
    }

    /// Optional `= e` after a variable name.
    fn parse_initializer_opt(&mut self) -> PResult<Option<crate::ast::ExprId>> {
        if self.eat_op(Op::Assign) {
            Ok(Some(self.parse_expression()?))
        } else {
            Ok(None)
        }
    }

    /// `name<T>(params) body` after the optional return type.
    fn parse_local_function(
        &mut self,
        start: Span,
        return_type: Option<TypeId>,
    ) -> PResult<StmtId> {
        let name = self.expect_identifier()?;
        let type_params = self.parse_type_parameters_opt()?;
        let parameters = self.parse_formal_parameters()?;
        let (modifier, body) = self.parse_function_body()?;
        let function = Function {
            span: self.span_from(start),
            external: false,
            static_: false,
            kind: FunctionKind::Function,
            return_type,
            name: Some(name),
            type_params: type_params.into_boxed_slice(),
            parameters: Some(parameters.into_boxed_slice()),
            modifier,
            body,
        };
        let id = self.ast.push_function(function);
        Ok(self.push_stmt(start, StmtKind::Function(id)))
    }

    // -- Compound statements ---------------------------------------------------

    /// `if (e) s`, `if (e) s else s`, `if (e case p when g) s`.
    fn parse_if(&mut self, start: Span) -> PResult<StmtId> {
        self.advance();
        let opener = self.pos;
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
        self.ensure_close_paren(opener)?;
        let then = self.parse_statement()?;
        let else_ = if self.eat_kw(Keyword::Else) {
            Some(self.parse_statement()?)
        } else {
            None
        };
        Ok(self.push_stmt(
            start,
            StmtKind::If {
                condition,
                case_pattern,
                guard,
                then,
                else_,
            },
        ))
    }

    /// `for` header and body, with `for` (and `await`) already consumed.
    fn parse_for_rest(&mut self, start: Span, await_: bool) -> PResult<StmtId> {
        let header = self.parse_for_header()?;
        let body = self.parse_statement()?;
        let kind = match header {
            ForHeader::Classic {
                init,
                condition,
                updates,
            } => StmtKind::For {
                await_,
                init,
                condition,
                updates: updates.into_boxed_slice(),
                body,
            },
            ForHeader::In { target, iterable } => StmtKind::ForIn {
                await_,
                target,
                iterable,
                body,
            },
        };
        Ok(self.push_stmt(start, kind))
    }

    /// `; cond? ; (e (, e)*)? )` after the initializer of a classic `for`.
    fn parse_classic_for_rest(&mut self, init: Option<ForInit>) -> PResult<ForHeader> {
        self.expect_semicolon()?;
        let condition = if self.at_op(Op::Semicolon) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect_semicolon()?;
        let mut updates = Vec::new();
        // The written grammar has no trailing comma here, but the SDK accepts
        // `i += 15,)` (verified on Dart 3.6.2), and the corpus uses it.
        while !self.at_op(Op::RParen) {
            updates.push(self.parse_expression()?);
            if !self.eat_op(Op::Comma) {
                break;
            }
        }
        self.expect_op(Op::RParen)?;
        Ok(ForHeader::Classic {
            init,
            condition,
            updates,
        })
    }

    fn parse_while(&mut self, start: Span) -> PResult<StmtId> {
        self.advance();
        let opener = self.pos;
        self.expect_op(Op::LParen)?;
        let condition = self.parse_expression()?;
        self.ensure_close_paren(opener)?;
        let body = self.parse_statement()?;
        Ok(self.push_stmt(start, StmtKind::While { condition, body }))
    }

    fn parse_do(&mut self, start: Span) -> PResult<StmtId> {
        self.advance();
        let body = self.parse_statement()?;
        self.expect_kw(Keyword::While)?;
        let opener = self.pos;
        self.expect_op(Op::LParen)?;
        let condition = self.parse_expression()?;
        self.ensure_close_paren(opener)?;
        self.expect_semicolon()?;
        Ok(self.push_stmt(start, StmtKind::DoWhile { body, condition }))
    }

    /// `switch (e) { (label:)* case p when g: stmts ... default: stmts }`.
    ///
    /// Each `case` becomes a [`SwitchCase`]; in `case a: case b: stmts` the body
    /// goes on the last one. An error inside the body skips to the `}` that
    /// closes the `switch` and returns the cases already read.
    fn parse_switch(&mut self, start: Span) -> PResult<StmtId> {
        self.advance();
        let opener = self.pos;
        self.expect_op(Op::LParen)?;
        let value = self.parse_expression()?;
        self.ensure_close_paren(opener)?;
        // `ensureBlock(…, switchStatement)`: without `{`, the error at the `)` and
        // a synthetic empty body; what comes next is the next statement.
        if !self.at_op(Op::LBrace) {
            let closes = self.tokens[self.pos - 1].span;
            self.error_at(codes::parser::EXPECTED_SWITCH_STATEMENT_BODY, closes, &[]);
            return Ok(self.push_stmt(
                start,
                StmtKind::Switch {
                    value,
                    cases: Vec::new().into_boxed_slice(),
                },
            ));
        }
        let close = self.matching_close(self.pos);
        self.expect_op(Op::LBrace)?;
        let mut cases = Vec::new();
        match self.parse_switch_cases(&mut cases) {
            Ok(()) => {
                self.expect_op(Op::RBrace)?;
            }
            Err(ParseError) => match close {
                Some(close) => self.pos = close + 1,
                None => return Err(ParseError),
            },
        }
        Ok(self.push_stmt(
            start,
            StmtKind::Switch {
                value,
                cases: cases.into_boxed_slice(),
            },
        ))
    }

    fn parse_switch_cases(&mut self, cases: &mut Vec<SwitchCase>) -> PResult<()> {
        let mut saw_default = false;
        // The most recent `default` (start of the word): `parseSwitchBlock`
        // (`parser_impl.dart:8975-9030`) keeps it after its group and passes it to
        // the `endSwitchCase` of each following group; the `AstBuilder`
        // (`ast_builder.dart:3210-3247`) then appends to the end of each `case`
        // group a `SwitchDefault` with that same word, which carries the group's
        // statements.
        let mut last_default: Option<usize> = None;
        while !self.at_op(Op::RBrace) && !self.at_eof() {
            let case_start = self.span();
            let mut labels = Vec::new();
            while self.at_label() {
                labels.push(self.identifier());
                self.advance();
            }
            // `parseSwitchCase` (`parser_impl.dart:9044`): `case` after `default` is
            // `SWITCH_HAS_CASE_AFTER_DEFAULT_CASE` at the `case`; another `default`,
            // `SWITCH_HAS_MULTIPLE_DEFAULT_CASES`.
            if saw_default && self.at_kw(Keyword::Case) {
                self.error(codes::parser::SWITCH_HAS_CASE_AFTER_DEFAULT_CASE, &[]);
            } else if saw_default && self.at_kw(Keyword::Default) {
                self.error(codes::parser::SWITCH_HAS_MULTIPLE_DEFAULT_CASES, &[]);
            }
            saw_default |= self.at_kw(Keyword::Default);
            if self.at_kw(Keyword::Default) {
                last_default = Some(self.span().start);
            }
            let (pattern, guard) = if self.eat_kw(Keyword::Case) {
                // Before 3.0 (`allowPatterns` false), `case` takes an expression
                // (`parseSwitchCase`: `parseExpression`).
                let pattern =
                    if self.features.version() < crate::features::LanguageVersion::new(3, 0) {
                        let start_offset = self.span();
                        let e = self.parse_expression()?;
                        self.push_pattern(start_offset, crate::ast::PatternKind::Constant(e))
                    } else {
                        self.parse_pattern()?
                    };
                let guard = if self.eat_ident("when") {
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                self.ensure_colon();
                (Some(pattern), guard)
            } else if self.eat_kw(Keyword::Default) {
                self.ensure_colon();
                (None, None)
            } else {
                return Err(self.error(codes::parser::EXPECTED_CASE_OR_DEFAULT, &[]));
            };
            // End of a `case` group after a `default`: the next member (after the
            // labels) is neither `case` nor `default`.
            let group_end_after_default = pattern.is_some() && last_default.is_some() && {
                let mut k = 0;
                while self.at_identifier_at(k) && self.at_op_at(k + 1, Op::Colon) {
                    k += 2;
                }
                !matches!(
                    self.kind_at(k),
                    Kind::Keyword(Keyword::Case | Keyword::Default)
                )
            };
            if let (true, Some(d)) = (group_end_after_default, last_default) {
                cases.push(SwitchCase {
                    span: self.span_from(case_start),
                    labels: labels.into_boxed_slice(),
                    pattern,
                    guard,
                    body: Box::default(),
                });
                let body = self.parse_statement_list(true);
                let end_offset = self.last_end().max(d);
                cases.push(SwitchCase {
                    span: Span {
                        start: d,
                        end: end_offset,
                    },
                    labels: Box::default(),
                    pattern: None,
                    guard: None,
                    body: body.into_boxed_slice(),
                });
                continue;
            }
            let body = self.parse_statement_list(true);
            cases.push(SwitchCase {
                span: self.span_from(case_start),
                labels: labels.into_boxed_slice(),
                pattern,
                guard,
                body: body.into_boxed_slice(),
            });
        }
        Ok(())
    }

    /// `try {} (on T (catch (e, st))? {})* (catch (e, st) {})* (finally {})?`
    /// `ensureBlock(token, kind)` of a `try`/`catch`/`finally`: without `{`, the
    /// `EXPECTED_*_BODY` at the previous token and a synthetic empty block in
    /// place of the next one.
    fn required_block(&mut self, code: dartforge_diagnostics::Code) -> PResult<StmtId> {
        if self.at_op(Op::LBrace) {
            return self.parse_block();
        }
        let previous = self.tokens[self.pos.saturating_sub(1)].span;
        self.error_at(code, previous, &[]);
        let here = self.span().start;
        Ok(self.ast.push_stmt(crate::ast::Stmt {
            span: Span {
                start: here,
                end: here,
            },
            kind: StmtKind::Block(Vec::new().into_boxed_slice()),
        }))
    }

    fn parse_try(&mut self, start: Span) -> PResult<StmtId> {
        self.advance();
        let body = self.required_block(codes::parser::EXPECTED_TRY_STATEMENT_BODY)?;
        let mut catches = Vec::new();
        loop {
            let clause_start = self.span();
            let on_type;
            let (exception, stack_trace);
            if self.eat_ident("on") {
                on_type = Some(self.parse_type()?);
                (exception, stack_trace) = if self.eat_kw(Keyword::Catch) {
                    self.parse_catch_parameters()?
                } else {
                    (None, None)
                };
            } else if self.eat_kw(Keyword::Catch) {
                on_type = None;
                (exception, stack_trace) = self.parse_catch_parameters()?;
            } else {
                break;
            }
            let body = self.required_block(codes::parser::EXPECTED_CATCH_CLAUSE_BODY)?;
            catches.push(CatchClause {
                span: self.span_from(clause_start),
                on_type,
                exception,
                stack_trace,
                body,
            });
        }
        let finally_ = if self.eat_kw(Keyword::Finally) {
            Some(self.required_block(codes::parser::EXPECTED_FINALLY_CLAUSE_BODY)?)
        } else {
            None
        };
        if catches.is_empty() && finally_.is_none() {
            return Err(self.error(codes::parser::MISSING_CATCH_OR_FINALLY, &[]));
        }
        Ok(self.push_stmt(
            start,
            StmtKind::Try {
                body,
                catches: catches.into_boxed_slice(),
                finally_,
            },
        ))
    }

    /// `(e)` or `(e, st)` after `catch`.
    fn parse_catch_parameters(
        &mut self,
    ) -> PResult<(Option<crate::ast::Name>, Option<crate::ast::Name>)> {
        self.expect_op(Op::LParen)?;
        let exception = self.expect_identifier()?;
        let stack_trace = if self.eat_op(Op::Comma) {
            Some(self.expect_identifier()?)
        } else {
            None
        };
        self.expect_op(Op::RParen)?;
        Ok((Some(exception), stack_trace))
    }

    /// `yield e;` / `yield* e;`
    fn parse_yield(&mut self, start: Span) -> PResult<StmtId> {
        let word = self.span();
        self.advance();
        let star = self.eat_op(Op::Star);
        let value = self.parse_expression()?;
        self.expect_semicolon()?;
        // `parseYieldStatement`: outside a generator, `messageYieldNotGenerator` at
        // the `yield` (the analyzer's `YIELD_IN_NON_GENERATOR`, also with `*`); the
        // statement stays in the tree, and the resolver reports it again
        // (`c2_sintaticos`).
        if !self.in_generator {
            self.error_at(codes::compile_time_error::YIELD_IN_NON_GENERATOR, word, &[]);
        }
        Ok(self.push_stmt(start, StmtKind::Yield { star, value }))
    }

    /// `assert(c);`, `assert(c, msg);`, `assert(c, msg,);`
    fn parse_assert(&mut self, start: Span) -> PResult<StmtId> {
        self.advance();
        let opener = self.pos;
        self.expect_op(Op::LParen)?;
        let condition = self.parse_expression()?;
        let mut message = None;
        if self.eat_op(Op::Comma) && !self.at_op(Op::RParen) {
            message = Some(self.parse_expression()?);
            self.eat_op(Op::Comma);
        }
        // `parseAssert` (`parser_impl.dart`): leftovers before the matching `)` are
        // `UNEXPECTED_TOKEN` at their first token, and the rest is skipped.
        if !self.at_op(Op::RParen)
            && let Some(closes) = self.matching_close(opener)
            && closes > self.pos
            && self.kind_of(closes) == Kind::Op(Op::RParen)
        {
            let text = self.text().to_string();
            self.error(codes::parser::UNEXPECTED_TOKEN, &[&text]);
            self.pos = closes;
        }
        self.expect_op(Op::RParen)?;
        self.expect_semicolon()?;
        Ok(self.push_stmt(start, StmtKind::Assert { condition, message }))
    }

    // -- Lookaheads -------------------------------------------------------

    /// `name:` at the current token.
    fn at_label(&self) -> bool {
        self.at_identifier() && self.at_op_at(1, Op::Colon)
    }

    /// Is there `name<T>?(params)` at `pos` followed by `{`, `=>`, `async` or
    /// `sync` — a local function, and not a call?
    pub(crate) fn looks_like_local_function(&self, pos: usize) -> bool {
        if self.kind_of(pos) != Kind::Ident {
            return false;
        }
        let mut i = pos + 1;
        if self.kind_of(i) == Kind::Op(Op::Lt) {
            match self.skip_angle_group(i) {
                Some(after) => i = after,
                None => return false,
            }
        }
        if self.kind_of(i) != Kind::Op(Op::LParen) {
            return false;
        }
        let Some(close) = self.matching_close(i) else {
            return false;
        };
        match self.kind_of(close + 1) {
            Kind::Op(Op::LBrace | Op::Arrow) => true,
            Kind::Ident => matches!(self.text_of(close + 1), "async" | "sync"),
            _ => false,
        }
    }

    /// Position after the `<...>` group starting at `pos`, counting only `<` and
    /// `>` (which arrive isolated from the lexer). Gives up at `;`, `=>`, `<<`,
    /// end, or a `{`/`}` outside parentheses — none of that fits in a type
    /// parameter list (braces only appear in a function type's named
    /// parameters: `<X extends void Function({X p})>`).
    fn skip_angle_group(&self, pos: usize) -> Option<usize> {
        let mut depth = 0usize;
        let mut parens = 0usize;
        let mut i = pos;
        loop {
            match self.kind_of(i) {
                Kind::Op(Op::Lt) => depth += 1,
                Kind::Op(Op::Gt) => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i + 1);
                    }
                }
                Kind::Op(Op::LParen) => parens += 1,
                Kind::Op(Op::RParen) => parens = parens.checked_sub(1)?,
                Kind::Op(Op::LBrace | Op::RBrace) if parens > 0 => {}
                Kind::Op(Op::Semicolon | Op::LBrace | Op::RBrace | Op::Arrow | Op::LtLt)
                | Kind::Eof => return None,
                _ => {}
            }
            i += 1;
        }
    }

    /// Does a local declaration's `type identifier` start at `pos`?
    ///
    /// Pre-filters the forms that are never a type (so as not to pay the full
    /// lookahead on every expression statement), applies the SDK rules that
    /// belong to the statement and not to the type — `x as y` is an expression,
    /// `(…) async` is an anonymous function, and `a ? b : c` versus `int? x`
    /// when `could_be_expression` — and delegates the rest to
    /// [`Parser::looks_like_type_then_identifier`].
    fn declaration_type_at(&mut self, pos: usize, could_be_expression: bool) -> bool {
        match self.kind_of(pos) {
            Kind::Keyword(Keyword::Void) => true,
            Kind::Ident => {
                // `await` can only be the type of a declaration in a plain synchronous
                // body, when it does not look like an expression (`parseStatementX`'s
                // `looksLikeAwaitExpression`).
                if self.text_of(pos) == "yield"
                    || self.text_of(pos) == "await"
                        && (self.in_async
                            || self.in_generator
                            || self.looks_like_expression_after_await(pos, false))
                {
                    return false;
                }
                // `Function(...) name`/`Function<T>(...) name`: a function type.
                if self.at_function_tail(pos) {
                    return self.looks_like_type_then_identifier(pos);
                }
                match self.kind_of(pos + 1) {
                    Kind::Ident => {
                        if self.is_expression_continuation(pos + 1) {
                            return false;
                        }
                        self.looks_like_type_then_identifier(pos)
                    }
                    Kind::Op(Op::Dot) => {
                        matches!(
                            self.kind_of(pos + 3),
                            Kind::Ident | Kind::Op(Op::Lt | Op::Question)
                        ) && !self.is_expression_continuation(pos + 3)
                            && self.looks_like_type_then_identifier(pos)
                    }
                    // `skip_type_arguments`, not `skip_angle_group`: a named record
                    // `Future<({int a, int b})>` has `{` inside the `<...>`.
                    Kind::Op(Op::Lt) => match self.skip_type_arguments(pos + 1) {
                        Some(after) => {
                            matches!(self.kind_of(after), Kind::Ident | Kind::Op(Op::Question))
                                && !self.is_expression_continuation(after)
                                && self.looks_like_type_then_identifier(pos)
                        }
                        None => false,
                    },
                    Kind::Op(Op::Question) => {
                        // `T? Function(…) name`: after the function type tail comes a name, which
                        // no conditional allows — it is a declaration, even with `? … :` ahead.
                        if self.at_function_tail(pos + 2)
                            && self.looks_like_type_then_identifier(pos)
                        {
                            return true;
                        }
                        if could_be_expression && self.conditional_after_question(pos + 1) {
                            return false;
                        }
                        self.looks_like_type_then_identifier(pos)
                    }
                    _ => false,
                }
            }
            Kind::Op(Op::LParen) => {
                let Some(close) = self.matching_close(pos) else {
                    return false;
                };
                match self.kind_of(close + 1) {
                    Kind::Ident => {
                        !self.is_expression_continuation(close + 1)
                            && self.looks_like_type_then_identifier(pos)
                    }
                    Kind::Op(Op::Question) => {
                        if could_be_expression && self.conditional_after_question(close + 1) {
                            return false;
                        }
                        self.looks_like_type_then_identifier(pos)
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// The identifier at `pos` continues an expression instead of naming a
    /// variable: `as` (except `x as = 1;`/`x as;`, which the SDK reads as a
    /// variable named `as`), and `async`/`sync` of an anonymous function body.
    fn is_expression_continuation(&self, pos: usize) -> bool {
        match self.text_of(pos) {
            // Only with the body right after: `int async;` declares `async`.
            "async" | "sync" => matches!(
                self.kind_of(pos + 1),
                Kind::Op(Op::LBrace | Op::Arrow | Op::Star)
            ),
            "as" => !matches!(
                self.kind_of(pos + 1),
                Kind::Op(Op::Assign | Op::Semicolon | Op::Comma)
            ),
            _ => false,
        }
    }

    /// Is there a `?` at `q` after a simple type, and what follows is a
    /// conditional expression (`a ? b : c`) rather than `type? name`?
    ///
    /// Follows the SDK: it is only a declaration when the following identifier
    /// is followed by `;`, `,`, `)`, `in`, end, or by `=` whose expression is
    /// not followed by `:` — this last case needs a speculative parse.
    fn conditional_after_question(&mut self, q: usize) -> bool {
        if self.kind_of(q + 1) != Kind::Ident {
            return false;
        }
        match self.kind_of(q + 2) {
            Kind::Op(Op::Semicolon | Op::Comma | Op::RParen) | Kind::Eof => false,
            Kind::Keyword(Keyword::In) => false,
            // `Object? convert(Object? o) {` is a local function with a nullable
            // return; `Object ? convert(o) : x` is a conditional. What tells them apart
            // is what comes after the parentheses.
            Kind::Op(Op::LParen | Op::Lt) => !self.looks_like_function_rest(q + 2),
            Kind::Op(Op::Assign) => {
                let saved = self.pos;
                self.pos = q + 3;
                let colon_follows = self.speculate(|p| {
                    p.parse_expression_without_cascade().is_ok() && p.at_op(Op::Colon)
                });
                self.pos = saved;
                colon_follows
            }
            _ => true,
        }
    }

    /// Runs `f` and undoes everything it produced: position, nodes, diagnostics
    /// and depth. Used to settle an ambiguity by looking ahead.
    pub(crate) fn speculate<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let pos = self.pos;
        let splices = self.splices.len();
        let depth = self.depth;
        let diagnostics = self.diagnostics.len();
        let exprs = self.ast.exprs.len();
        let stmts = self.ast.stmts.len();
        let types = self.ast.types.len();
        let patterns = self.ast.patterns.len();
        let decls = self.ast.decls.len();
        let members = self.ast.members.len();
        let functions = self.ast.functions.len();
        let in_async = self.in_async;
        let in_generator = self.in_generator;
        let in_type_args = self.in_type_args;
        let result = f(self);
        self.undo_splices(splices);
        self.pos = pos;
        self.depth = depth;
        self.diagnostics.truncate(diagnostics);
        self.ast.exprs.truncate(exprs);
        self.ast
            .assert_invocations
            .retain(|e| (e.0 as usize) < exprs);
        self.ast
            .implicit_creations
            .retain(|e| (e.0 as usize) < exprs);
        self.ast.stmts.truncate(stmts);
        self.ast.types.truncate(types);
        self.ast.patterns.truncate(patterns);
        self.ast.decls.truncate(decls);
        self.ast.members.truncate(members);
        self.ast.functions.truncate(functions);
        self.in_async = in_async;
        self.in_generator = in_generator;
        self.in_type_args = in_type_args;
        result
    }

    // -- Utilidades -------------------------------------------------------

    /// `ensureColon`: the missing `:` is `EXPECTED_TOKEN` at the next token
    /// (`rewriteAndRecover`) and a synthetic `:` follows in its place.
    fn ensure_colon(&mut self) {
        if !self.eat_op(Op::Colon) {
            self.error(codes::parser::EXPECTED_TOKEN, &[":"]);
        }
    }

    /// fasta's `ensureSemicolon`: the missing `;` is reported and inserted.
    fn expect_semicolon(&mut self) -> PResult<()> {
        self.ensure_semicolon()
    }

    fn push_stmt(&mut self, start: Span, kind: StmtKind) -> StmtId {
        let span = self.span_from(start);
        self.ast.push_stmt(Stmt { span, kind })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Ast, ExprKind, PatternKind, StmtKind};
    use dartforge_diagnostics::Diagnostic;
    use dartforge_intern::Interner;

    /// Result of a `parse_statement` on `src`.
    struct Out {
        names: Interner,
        ast: Ast,
        result: PResult<StmtId>,
        diagnostics: Vec<Diagnostic>,
        at_eof: bool,
    }

    fn stmt_with(src: &str, setup: impl FnOnce(&mut Parser<'_, '_>)) -> Out {
        let mut names = Interner::new();
        let tokens = crate::lexer::lex(src).unwrap();
        let (ast, result, diagnostics, at_eof) = {
            let mut p = Parser::new(src, tokens, &mut names);
            setup(&mut p);
            let result = p.parse_statement();
            let at_eof = p.at_eof();
            assert_eq!(p.depth, 0, "depth not restored");
            (p.ast, result, p.diagnostics, at_eof)
        };
        Out {
            names,
            ast,
            result,
            diagnostics,
            at_eof,
        }
    }

    fn stmt(src: &str) -> Out {
        stmt_with(src, |_| {})
    }

    /// A statement accepted without diagnostics, consuming the whole source.
    fn ok(src: &str) -> Out {
        let out = stmt(src);
        assert!(out.diagnostics.is_empty(), "{src}: {:?}", out.diagnostics);
        assert!(out.result.is_ok(), "{src}");
        assert!(out.at_eof, "{src}: input left over");
        out
    }

    impl Out {
        fn kind(&self) -> &StmtKind {
            &self.ast.stmt(self.result.unwrap()).kind
        }
        fn name(&self, name: &crate::ast::Name) -> &str {
            self.names.resolve(name.sym)
        }
    }

    // -- Independent of other modules ------------------------------------------

    /// `int? Function(int) f = … ? … : …;` is a declaration (the `?` belongs to
    /// the nullable type, not a conditional, even with `? :` in the
    /// initializer); a real conditional stays an expression.
    #[test]
    fn function_type_after_nullable_type() {
        let out = ok("int? Function(int) talvez = (x) => x > 0 ? x : null;");
        assert!(
            matches!(out.kind(), StmtKind::Variables(_)),
            "{:?}",
            out.kind()
        );
        let out = ok("String? Function<T>(T) g = h;");
        assert!(
            matches!(out.kind(), StmtKind::Variables(_)),
            "{:?}",
            out.kind()
        );
        let out = ok("a ? b : c;");
        assert!(
            matches!(out.kind(), StmtKind::Expression(_)),
            "{:?}",
            out.kind()
        );
        let out = ok("a ? Function : c;");
        assert!(
            matches!(out.kind(), StmtKind::Expression(_)),
            "{:?}",
            out.kind()
        );
    }

    #[test]
    fn empty_block_and_empty_statement() {
        let out = ok("{}");
        assert!(matches!(out.kind(), StmtKind::Block(s) if s.is_empty()));
        let out = ok(";");
        assert!(matches!(out.kind(), StmtKind::Empty));
        let out = ok("{ ; ; {} }");
        let StmtKind::Block(stmts) = out.kind() else {
            panic!()
        };
        assert_eq!(stmts.len(), 3);
        assert!(matches!(out.ast.stmt(stmts[2]).kind, StmtKind::Block(_)));
        assert_eq!(out.ast.stmt(out.result.unwrap()).span.end, 10);
    }

    #[test]
    fn break_continue_return() {
        let out = ok("break;");
        assert!(matches!(out.kind(), StmtKind::Break(None)));
        let out = ok("break outer;");
        let StmtKind::Break(Some(label)) = out.kind() else {
            panic!()
        };
        assert_eq!(out.name(label), "outer");
        let out = ok("continue;");
        assert!(matches!(out.kind(), StmtKind::Continue(None)));
        let out = ok("continue l;");
        assert!(matches!(out.kind(), StmtKind::Continue(Some(_))));
        let out = ok("return;");
        assert!(matches!(out.kind(), StmtKind::Return(None)));
        let out = ok("rethrow;");
        let StmtKind::Expression(e) = out.kind() else {
            panic!()
        };
        assert!(matches!(out.ast.expr(*e).kind, ExprKind::Rethrow));
    }

    #[test]
    fn labels_() {
        let out = ok("a: b: {}");
        let StmtKind::Labeled { labels, body } = out.kind() else {
            panic!()
        };
        assert_eq!(labels.len(), 2);
        assert_eq!(out.name(&labels[0]), "a");
        assert_eq!(out.name(&labels[1]), "b");
        assert!(matches!(out.ast.stmt(*body).kind, StmtKind::Block(_)));

        let out = ok("outer: for (;;) { break outer; continue outer; }");
        let StmtKind::Labeled { body, .. } = out.kind() else {
            panic!()
        };
        let StmtKind::For {
            await_: false,
            init: None,
            condition: None,
            updates,
            body,
        } = &out.ast.stmt(*body).kind
        else {
            panic!()
        };
        assert!(updates.is_empty());
        assert!(matches!(&out.ast.stmt(*body).kind, StmtKind::Block(s) if s.len() == 2));
    }

    #[test]
    fn empty_classic_for_and_await_for() {
        let out = ok("for (;;) ;");
        assert!(matches!(out.kind(), StmtKind::For { await_: false, .. }));
        let out = ok("await for (;;) {}");
        assert!(matches!(out.kind(), StmtKind::For { await_: true, .. }));
    }

    #[test]
    fn try_catch_finally() {
        let out = ok("try {} catch (e) {}");
        let StmtKind::Try {
            catches, finally_, ..
        } = out.kind()
        else {
            panic!()
        };
        assert_eq!(catches.len(), 1);
        assert!(catches[0].on_type.is_none());
        assert_eq!(out.name(catches[0].exception.as_ref().unwrap()), "e");
        assert!(catches[0].stack_trace.is_none());
        assert!(finally_.is_none());

        let out = ok("try {} catch (e, st) {} finally {}");
        let StmtKind::Try {
            catches, finally_, ..
        } = out.kind()
        else {
            panic!()
        };
        assert_eq!(out.name(catches[0].stack_trace.as_ref().unwrap()), "st");
        assert!(finally_.is_some());

        let out = ok("try {} finally {}");
        assert!(
            matches!(out.kind(), StmtKind::Try { catches, finally_: Some(_), .. } if catches.is_empty())
        );

        let out = stmt("try {}");
        assert!(out.result.is_err());
        assert!(out.diagnostics[0].message.contains("finally"));
    }

    #[test]
    fn local_variables_without_type_or_initializer() {
        let out = ok("late final x;");
        let StmtKind::Variables(list) = out.kind() else {
            panic!()
        };
        assert!(list.late && list.final_ && !list.var_ && list.ty.is_none());
        assert_eq!(list.variables.len(), 1);
        assert_eq!(out.name(&list.variables[0].name), "x");

        let out = ok("var x, y;");
        let StmtKind::Variables(list) = out.kind() else {
            panic!()
        };
        assert!(list.var_ && !list.late);
        assert_eq!(list.variables.len(), 2);

        let out = ok("final x;");
        assert!(matches!(out.kind(), StmtKind::Variables(l) if l.final_));

        let out = ok("late var x;");
        assert!(matches!(out.kind(), StmtKind::Variables(l) if l.late && l.var_));
    }

    /// `LocalVariableDeclarationIdentifierContext`: a reserved word as the name
    /// of a local variable is the name (the analyzer then reports it unused);
    /// one that starts a statement leaves a synthetic name and is not consumed.
    #[test]
    fn reserved_word_as_local_variable_name() {
        let out = stmt("var enum;");
        let StmtKind::Variables(list) = out.kind() else {
            panic!()
        };
        assert_eq!(out.name(&list.variables[0].name), "enum");
        assert_eq!(out.diagnostics.len(), 1);
        assert!(
            out.diagnostics[0]
                .message
                .contains("'enum' can't be used as an identifier")
        );
        assert!(out.at_eof);

        let out = stmt("var if;");
        let StmtKind::Variables(list) = out.kind() else {
            panic!()
        };
        assert_eq!(out.name(&list.variables[0].name), "");
        assert!(
            out.diagnostics[0]
                .message
                .contains("Expected an identifier")
        );
        assert!(!out.at_eof, "the `if` is left for the next statement");
    }

    #[test]
    fn switch_without_expressions_fails_early_but_recovers_at_brace() {
        // Without `case`/`default` the body is invalid; the parser skips to the `}`.
        let out = stmt("switch (x) { break; }");
        // `x` needs the expressions module; if it is still a stub the test panics,
        // and that is expected (see the `dependents` module).
        assert!(out.result.is_ok());
        assert_eq!(out.diagnostics.len(), 1);
        assert!(out.at_eof);
    }

    #[test]
    fn recovery_in_block() {
        // `)` at the start: an expression statement with a synthetic identifier
        // (missing `;` at the `{`, `MISSING_IDENTIFIER` at the `)`) and, without
        // progress, `UNEXPECTED_TOKEN` at the `)`, which is skipped; then the empty
        // `;` and the `break` (probed on SDK 3.6.2).
        let out = stmt("{ ) ; break; }");
        assert!(out.result.is_ok());
        assert_eq!(out.diagnostics.len(), 3, "{:?}", out.diagnostics);
        assert!(matches!(out.kind(), StmtKind::Block(s) if s.len() == 3));
        assert!(out.at_eof);

        // Missing `;`: fasta's `ensureSemicolon` inserts it; the `break` stays and
        // the block's `}` is not consumed by it.
        let out = stmt("{ break }");
        assert!(out.result.is_ok());
        assert_eq!(out.diagnostics.len(), 1);
        assert!(matches!(out.kind(), StmtKind::Block(s) if s.len() == 1));
        assert!(out.at_eof);

        // The same, with the list `[ ; { ; } ]` read as a literal (SDK 3.6.2's
        // errors: the three of the `)` and, at the `;`, the missing element and
        // `]`).
        let out = stmt("{ ) [ ; { ; } ] ; break; }");
        assert!(out.result.is_ok());
        assert_eq!(out.diagnostics.len(), 5, "{:?}", out.diagnostics);

        // Unclosed block.
        let out = stmt("{ break;");
        assert!(out.result.is_err());
        assert!(
            out.diagnostics
                .iter()
                .any(|d| d.code.is_some_and(|c| c.info().name == "expected_token"))
        );
    }

    #[test]
    fn reserved_words_that_do_not_start_statement() {
        // Expression statement with the word as a name (`MISSING_STATEMENT` from
        // `handleExpressionStatement`), as in fasta.
        for src in ["case 1:", "class A {}", "finally {}"] {
            let out = stmt(src);
            assert!(
                out.diagnostics
                    .iter()
                    .any(|d| d.code.is_some_and(|c| c.info().name == "missing_statement")),
                "{src}"
            );
        }
        // `else` starts a statement: a synthetic identifier before it.
        let out = stmt("else;");
        assert!(out.diagnostics.iter().any(|d| {
            d.code
                .is_some_and(|c| c.info().name == "missing_identifier")
        }));
    }

    #[test]
    fn depth_limit() {
        let depth = super::super::MAX_DEPTH as usize + 10;
        let src = format!("{}{}", "{".repeat(depth), "}".repeat(depth));
        let out = stmt(&src);
        assert!(out.result.is_ok());
        assert_eq!(out.diagnostics.len(), 1);
        assert!(
            out.diagnostics
                .iter()
                .any(|d| d.code.is_some_and(|c| c.info().name == "stack_overflow"))
        );
        assert!(out.at_eof);
    }

    #[test]
    fn local_function_lookahead() {
        let lookahead = |src: &str| {
            let mut names = Interner::new();
            let tokens = crate::lexer::lex(src).unwrap();
            let p = Parser::new(src, tokens, &mut names);
            p.looks_like_local_function(0)
        };
        assert!(lookahead("f() {}"));
        assert!(lookahead("f() => 1;"));
        assert!(lookahead("f<T>(T x) async {}"));
        assert!(lookahead("f() sync* {}"));
        assert!(!lookahead("f();"));
        assert!(!lookahead("f() + 1;"));
        assert!(!lookahead("f<T>;"));
        assert!(!lookahead("f"));
    }

    // -- Depending on expressions.rs / types.rs / declarations.rs --------------
    //
    // While those modules are `todo!()` stubs, these tests panic with
    // "not yet implemented"; they become meaningful once the modules exist.
    mod dependents {
        use super::*;

        /// Corpus regressions: local function with a nullable return, `operator`
        /// field, object pattern in `for-in`, trailing comma in `for`.
        #[test]
        fn local_function_with_nullable_return() {
            for src in [
                "Object? convert(Object? o) { return o; }",
                "Object? anything() => _inscrutable(0);",
                "_Credentials? find(_Scheme scheme) { return null; }",
                "T? g<T>(T t) => t;",
                // Named record inside `<...>` (seen in the new_sali project).
                "Future<({int total, List<Map<String, dynamic>> itens})> paginar() async {}",
            ] {
                let out = ok(src);
                assert!(matches!(out.kind(), StmtKind::Function(_)), "{src}");
            }
            // The conditional stays a conditional.
            let out = ok("Object ? convert(o) : x;");
            assert!(matches!(out.kind(), StmtKind::Expression(_)));
        }

        #[test]
        fn for_in_with_object_pattern_and_trailing_comma() {
            for src in [
                "for (final MapEntry(key: k, value: v) in m.entries) {}",
                "for (final MapEntry(key: String k, value: Object? v) in m.entries) {}",
                "for (final p.Nome<int>(:a) in xs) {}",
                "for (var (a, b) in xs) {}",
            ] {
                let out = ok(src);
                assert!(
                    matches!(
                        out.kind(),
                        StmtKind::ForIn {
                            target: ForInTarget::Pattern { .. },
                            ..
                        }
                    ),
                    "{src}"
                );
            }
            let out = ok("for (var i = 0; i < n; i += 15,) {}");
            assert!(matches!(out.kind(), StmtKind::For { updates, .. } if updates.len() == 1));
        }

        #[test]
        fn if_else_and_if_case() {
            let out = ok("if (a) b(); else { c(); }");
            assert!(matches!(
                out.kind(),
                StmtKind::If {
                    case_pattern: None,
                    guard: None,
                    else_: Some(_),
                    ..
                }
            ));
            let out = ok("if (x case int y when y > 0) {}");
            let StmtKind::If {
                case_pattern: Some(p),
                guard: Some(_),
                else_: None,
                ..
            } = out.kind()
            else {
                panic!()
            };
            assert!(matches!(
                out.ast.pattern(*p).kind,
                PatternKind::Variable { ty: Some(_), .. }
            ));
        }

        #[test]
        fn while_and_do() {
            let out = ok("while (true) { break; }");
            assert!(matches!(out.kind(), StmtKind::While { .. }));
            let out = ok("do { x++; } while (x < 10);");
            assert!(matches!(out.kind(), StmtKind::DoWhile { .. }));
        }

        #[test]
        fn classic_for() {
            let out = ok("for (var i = 0, n = x.length; i < n; i++) {}");
            let StmtKind::For {
                init: Some(ForInit::Variables(list)),
                condition: Some(_),
                updates,
                ..
            } = out.kind()
            else {
                panic!()
            };
            assert!(list.var_);
            assert_eq!(list.variables.len(), 2);
            assert_eq!(updates.len(), 1);

            let out = ok("for (int i = 0; i < 3; i++, j--) {}");
            let StmtKind::For {
                init: Some(ForInit::Variables(list)),
                updates,
                ..
            } = out.kind()
            else {
                panic!()
            };
            assert!(list.ty.is_some());
            assert_eq!(updates.len(), 2);

            let out = ok("for (i = 0; ; ) {}");
            assert!(matches!(
                out.kind(),
                StmtKind::For {
                    init: Some(ForInit::Expression(_)),
                    condition: None,
                    ..
                }
            ));
        }

        #[test]
        fn for_in() {
            let out = ok("for (final x in xs) {}");
            let StmtKind::ForIn {
                await_: false,
                target:
                    ForInTarget::Declared {
                        final_: true,
                        var_: false,
                        ty: None,
                        ..
                    },
                ..
            } = out.kind()
            else {
                panic!()
            };
            let out = ok("for (int x in xs) {}");
            assert!(matches!(
                out.kind(),
                StmtKind::ForIn {
                    target: ForInTarget::Declared { ty: Some(_), .. },
                    ..
                }
            ));
            let out = ok("for (x in xs) {}");
            assert!(matches!(
                out.kind(),
                StmtKind::ForIn {
                    target: ForInTarget::Expression(_),
                    ..
                }
            ));
            let out = ok("for (a.b in xs) {}");
            assert!(matches!(
                out.kind(),
                StmtKind::ForIn {
                    target: ForInTarget::Expression(_),
                    ..
                }
            ));
            let out = ok("await for (var x in stream) {}");
            assert!(matches!(out.kind(), StmtKind::ForIn { await_: true, .. }));
            let out = ok("for (@a var x in y) {}");
            let StmtKind::ForIn {
                target: ForInTarget::Declared { metadata, .. },
                ..
            } = out.kind()
            else {
                panic!()
            };
            assert_eq!(metadata.len(), 1);
            let out = ok("for (final (k, v) in map.entries) {}");
            let StmtKind::ForIn {
                target:
                    ForInTarget::Pattern {
                        final_: true,
                        pattern,
                    },
                ..
            } = out.kind()
            else {
                panic!()
            };
            assert!(matches!(
                out.ast.pattern(*pattern).kind,
                PatternKind::Record { .. }
            ));
        }

        #[test]
        fn switch_statement() {
            let out = ok(
                "switch (x) { case int a when a > 0: case 2: print(a); break; l: default: return; }",
            );
            let StmtKind::Switch { cases, .. } = out.kind() else {
                panic!()
            };
            assert_eq!(cases.len(), 3);
            assert!(cases[0].pattern.is_some() && cases[0].guard.is_some());
            assert!(cases[0].body.is_empty());
            assert!(cases[1].guard.is_none());
            assert_eq!(cases[1].body.len(), 2);
            assert!(cases[2].pattern.is_none());
            assert_eq!(cases[2].labels.len(), 1);
            assert_eq!(cases[2].body.len(), 1);

            let out = ok(
                "switch (x) { case [int a, ...]: case {'k': var v}: case (a, b): case Point(x: 0): case null: case 'a' || 'b': case > 0: case _: break; }",
            );
            let StmtKind::Switch { cases, .. } = out.kind() else {
                panic!()
            };
            assert_eq!(cases.len(), 8);

            let out = ok("switch (x) {}");
            assert!(matches!(out.kind(), StmtKind::Switch { cases, .. } if cases.is_empty()));
        }

        #[test]
        fn try_with_on() {
            let out =
                ok("try {} on FormatException catch (e) {} on int {} catch (e, st) {} finally {}");
            let StmtKind::Try { catches, .. } = out.kind() else {
                panic!()
            };
            assert_eq!(catches.len(), 3);
            assert!(catches[0].on_type.is_some() && catches[0].exception.is_some());
            assert!(catches[1].on_type.is_some() && catches[1].exception.is_none());
            assert!(catches[2].on_type.is_none() && catches[2].stack_trace.is_some());
        }

        #[test]
        fn return_yield_assert() {
            let out = ok("return a + b;");
            assert!(matches!(out.kind(), StmtKind::Return(Some(_))));
            let out = stmt_with("yield x;", |p| p.in_generator = true);
            assert!(matches!(out.kind(), StmtKind::Yield { star: false, .. }));
            let out = stmt_with("yield* xs;", |p| p.in_generator = true);
            assert!(matches!(out.kind(), StmtKind::Yield { star: true, .. }));
            let out = ok("assert(a, 'msg',);");
            assert!(matches!(
                out.kind(),
                StmtKind::Assert {
                    message: Some(_),
                    ..
                }
            ));
            let out = ok("assert(a);");
            assert!(matches!(out.kind(), StmtKind::Assert { message: None, .. }));
        }

        #[test]
        fn variable_declarations() {
            let out = ok("int x = 1, y;");
            let StmtKind::Variables(list) = out.kind() else {
                panic!()
            };
            assert!(list.ty.is_some());
            assert_eq!(list.variables.len(), 2);
            assert!(list.variables[0].initializer.is_some());
            let out = ok("final int x = 1;");
            assert!(matches!(out.kind(), StmtKind::Variables(l) if l.final_ && l.ty.is_some()));
            let out = ok("late final int x;");
            assert!(
                matches!(out.kind(), StmtKind::Variables(l) if l.late && l.final_ && l.ty.is_some())
            );
            let out = ok("late int x;");
            assert!(matches!(out.kind(), StmtKind::Variables(l) if l.late && l.ty.is_some()));
            let out = ok("const y = 1;");
            assert!(matches!(out.kind(), StmtKind::Variables(l) if l.const_ && l.ty.is_none()));
            let out = ok("const int y = 1;");
            assert!(matches!(out.kind(), StmtKind::Variables(l) if l.const_ && l.ty.is_some()));
            let out = ok("var x = 1;");
            assert!(matches!(out.kind(), StmtKind::Variables(l) if l.var_));
            let out = ok("List<int>? xs;");
            assert!(matches!(out.kind(), StmtKind::Variables(_)));
            let out = ok("a.B c;");
            assert!(matches!(out.kind(), StmtKind::Variables(_)));
            let out = ok("(int, int) p = (1, 2);");
            assert!(matches!(out.kind(), StmtKind::Variables(_)));
            let out = ok("void Function() cb;");
            assert!(matches!(out.kind(), StmtKind::Variables(_)));
            let out = ok("@pragma('x') final int x = 1;");
            assert!(matches!(out.kind(), StmtKind::Variables(_)));
        }

        #[test]
        fn pattern_declarations() {
            let out = ok("var (a, b) = pair;");
            assert!(matches!(
                out.kind(),
                StmtKind::PatternVariables { final_: false, .. }
            ));
            let out = ok("final [x, y] = list;");
            assert!(matches!(
                out.kind(),
                StmtKind::PatternVariables { final_: true, .. }
            ));
            let out = ok("final {'k': v} = map;");
            assert!(matches!(out.kind(), StmtKind::PatternVariables { .. }));
            // A record type is not a pattern.
            let out = ok("final (int, int) r = (1, 2);");
            assert!(matches!(out.kind(), StmtKind::Variables(_)));
        }

        #[test]
        fn local_functions() {
            let out = ok("int f() => 1;");
            let StmtKind::Function(f) = out.kind() else {
                panic!()
            };
            assert!(out.ast.function(*f).return_type.is_some());
            let out = ok("f(int x) { return x; }");
            let StmtKind::Function(f) = out.kind() else {
                panic!()
            };
            assert!(out.ast.function(*f).return_type.is_none());
            let out = ok("void f<T>(T x) async {}");
            assert!(matches!(out.kind(), StmtKind::Function(_)));
            let out = ok("int Function(int) g() => h;");
            assert!(matches!(out.kind(), StmtKind::Function(_)));
            // A call, not a function.
            let out = ok("f(x);");
            assert!(matches!(out.kind(), StmtKind::Expression(_)));
        }

        #[test]
        fn expression_statements() {
            for src in [
                "x = 1;",
                "f(a, b);",
                "a.b.c();",
                "a < b;",
                "Foo<int>(1);",
                "x as int;",
                "a ? b : c;",
                "a ? b = c : d;",
                "(a, b) = (b, a);",
                "[x, y] = list;",
                "{'k': v} = map;",
                "throw e;",
                "const Foo();",
                "const [1].length;",
                "new Foo();",
                "(x) => x;",
            ] {
                let out = ok(src);
                assert!(matches!(out.kind(), StmtKind::Expression(_)), "{src}");
            }
            // `await x;` is only an expression inside `async`; outside, the SDK reads
            // `await` as the type name of a declaration (`await x;`).
            let out = stmt_with("await x;", |p| p.in_async = true);
            assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
            assert!(matches!(out.kind(), StmtKind::Expression(_)));
            // `int? x = c;` stays a declaration.
            let out = ok("int? x = c;");
            assert!(matches!(out.kind(), StmtKind::Variables(_)));
            let out = ok("int? x;");
            assert!(matches!(out.kind(), StmtKind::Variables(_)));
        }

        #[test]
        fn block_with_several_statements() {
            let out = ok("{ int x = 1; x++; if (x > 1) return; }");
            assert!(matches!(out.kind(), StmtKind::Block(s) if s.len() == 3));
        }
    }
}
