//! Statements, declarações locais e cabeçalhos de `for`.
//!
//! Cobre `statement` da gramática de Dart 3.6: bloco, declaração de variável
//! local (inclusive por padrão), função local, `for`/`await for`, `while`,
//! `do`, `switch` (com padrões e guardas), `if` (com `case`), `try`, `break`,
//! `continue`, `return`, `yield`, `assert`, rótulos, `;` vazio e statement de
//! expressão.
//!
//! Desambiguação, na ordem em que o parser real do SDK (`parseStatementX`) a
//! faz:
//!
//! * `nome:` no início é rótulo;
//! * `{` é bloco, salvo quando o `}` correspondente é seguido de `=`
//!   (atribuição por padrão de mapa, que é uma expressão);
//! * `var`/`final` seguidos de `(`, `[` ou `{` começam uma declaração por
//!   padrão; `late`, `final`, `const`, `var` e `tipo nome` começam uma
//!   declaração de variáveis; `tipo? nome(...)` seguido de `{`, `=>`, `async`
//!   ou `sync` é função local — a diferença para a chamada `nome(args);` é só
//!   o que vem depois do `)`;
//! * `await` e `yield` só são especiais em corpos `async`/geradores, mas
//!   `await for` é sempre laço e `yield e;` fora de gerador é aceito como
//!   `yield` (não há statement de expressão válido com essa forma);
//! * o resto é statement de expressão.
//!
//! A ambiguidade clássica `a ? b : c;` contra `int? x = e;` é resolvida como no
//! SDK: um `?` logo após um nome simples só é tipo anulável se o identificador
//! seguinte for seguido de `;`, `,`, `)`, `in` ou de `=` cuja expressão **não**
//! é seguida de `:` (isso exige uma análise especulativa da expressão, que é
//! descartada depois).
//!
//! Recuperação: dentro de um bloco ou de um `case`, um statement que falha faz
//! o parser pular até o próximo `;` de mesmo nível ou até a `}` que fecha o
//! bloco, e continuar. O diagnóstico já foi registrado no ponto da falha.
//!
//! Limitações do contrato (ver relatório): metadata antes de uma declaração
//! local (`@pragma(...) final x = 1;`) é lida e descartada porque
//! [`StmtKind::Variables`] e [`StmtKind::Function`] não têm campo para ela; e
//! `for (var (a, b) = e; ...; ...)` (declaração por padrão como inicializador
//! clássico) não cabe em [`ForInit`] e produz erro.
use super::{ForHeader, PResult, ParseError, Parser};
use crate::ast::{
    CatchClause, Expr, ExprKind, ForInTarget, ForInit, Function, FunctionKind, Stmt, StmtId,
    StmtKind, SwitchCase, TypeId, Variable, VariableList,
};
use crate::token::{Keyword, Kind, Op};
use dartforge_diagnostics::Span;

impl<'s, 'i> Parser<'s, 'i> {
    /// `statement` completo, com rótulos.
    pub(crate) fn parse_statement(&mut self) -> PResult<StmtId> {
        self.enter()?;
        let result = self.parse_statement_inner();
        self.leave();
        result
    }

    /// `{ stmts }` com as chaves, recuperando de statements malformados.
    pub(crate) fn parse_block(&mut self) -> PResult<StmtId> {
        let start = self.expect_op(Op::LBrace)?.span;
        let stmts = self.parse_statement_list(false);
        if !self.at_op(Op::RBrace) {
            return Err(self.error("esperava '}' fechando o bloco"));
        }
        self.advance();
        Ok(self.push_stmt(start, StmtKind::Block(stmts)))
    }

    /// `(init; cond; upd)` ou `(decl in e)`, com os parênteses, após `for`
    /// (e após `await`, que o chamador consome).
    pub(crate) fn parse_for_header(&mut self) -> PResult<ForHeader> {
        self.expect_op(Op::LParen)?;
        let metadata = if self.at_op(Op::At) {
            self.parse_metadata()?
        } else {
            Vec::new()
        };

        // `for (final (a, b) in e)` — declaração por padrão.
        if (self.at_kw(Keyword::Var) || self.at_kw(Keyword::Final))
            && self.looks_like_pattern_declaration(self.pos)
        {
            let final_ = self.at_kw(Keyword::Final);
            self.advance();
            let pattern = self.parse_pattern()?;
            if self.at_op(Op::Assign) {
                return Err(self.error(
                    "declaração por padrão como inicializador de 'for' clássico não é suportada",
                ));
            }
            self.expect_kw(Keyword::In)?;
            let iterable = self.parse_expression()?;
            self.expect_op(Op::RParen)?;
            return Ok(ForHeader::In {
                target: ForInTarget::Pattern { final_, pattern },
                iterable,
            });
        }

        // Declaração de variável: com modificador ou com tipo.
        let has_modifier =
            self.at_kw(Keyword::Var) || self.at_kw(Keyword::Final) || self.at_kw(Keyword::Const);
        if has_modifier || self.declaration_type_at(self.pos, true) {
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
                        metadata,
                        final_,
                        var_,
                        ty,
                        name,
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
                variables,
            };
            return self.parse_classic_for_rest(Some(ForInit::Variables(list)));
        }

        if self.at_op(Op::Semicolon) {
            return self.parse_classic_for_rest(None);
        }

        // `for (x in e)` ou `for (e; ...; ...)`.
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

    // -- Listas de statements e recuperação -------------------------------

    /// Statements até `}`/fim (ou até o próximo `case`/`default` quando
    /// `in_switch`), com recuperação por statement.
    fn parse_statement_list(&mut self, in_switch: bool) -> Vec<StmtId> {
        let mut stmts = Vec::new();
        loop {
            if self.at_eof() || self.at_op(Op::RBrace) {
                break;
            }
            if in_switch && self.at_switch_case_start() {
                break;
            }
            match self.parse_statement() {
                Ok(id) => stmts.push(id),
                Err(ParseError) => self.synchronize_statement(in_switch),
            }
        }
        stmts
    }

    /// Pula tokens até o próximo `;` de mesmo nível (consumido) ou até a
    /// `}` que fecha o bloco corrente (não consumida). Dentro de `switch`
    /// também para em `case`/`default`. Sempre avança pelo menos um token
    /// quando o corrente não é um ponto de parada, garantindo progresso.
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

    /// O token corrente, passando por rótulos `nome:`, é `case` ou `default`.
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
            return Ok(self.push_stmt(start, StmtKind::Labeled { labels, body }));
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
            Kind::Op(Op::RParen | Op::RBracket | Op::RBrace) | Kind::Eof | Kind::ScriptTag => {
                Err(self.error("esperava um statement"))
            }
            Kind::Keyword(kw) => self.parse_keyword_statement(start, kw),
            Kind::Ident => self.parse_identifier_statement(start),
            _ => self.parse_expression_statement(start),
        }
    }

    /// Statement que começa com palavra reservada.
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
                // `const x = 1;` / `const int x = 1;` são declarações;
                // `const Foo();` / `const [1].length;` são expressões.
                let declaration = self.declaration_type_at(self.pos + 1, false)
                    || (self.at_identifier_at(1) && self.at_op_at(2, Op::Assign));
                if declaration {
                    self.parse_local_declaration(start)
                } else {
                    self.parse_expression_statement(start)
                }
            }
            Keyword::Void => self.parse_typed_declaration(start),
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
            | Keyword::With => Err(self.error("esperava um statement")),
            _ => self.parse_expression_statement(start),
        }
    }

    /// Statement que começa com identificador (inclusive os contextuais
    /// `await`, `yield` e `late`), já sem rótulos.
    fn parse_identifier_statement(&mut self, start: Span) -> PResult<StmtId> {
        if self.at_ident("await") {
            if self.at_kw_at(1, Keyword::For) {
                self.advance();
                self.advance();
                return self.parse_for_rest(start, true);
            }
            // `await e;` (e `await` como nome comum) é sempre expressão.
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

    /// `yield` é statement quando estamos num gerador ou quando o que segue
    /// só faz sentido como operando (`yield* e`, `yield x`, `yield 1`…).
    fn looks_like_yield_statement(&self) -> bool {
        if self.in_generator {
            return true;
        }
        match self.kind_at(1) {
            Kind::Op(Op::Star) => self.kind_at(2) != Kind::Op(Op::Assign),
            Kind::Ident
            | Kind::Int
            | Kind::Double
            | Kind::Str(_)
            | Kind::StrBegin(..)
            | Kind::Keyword(_)
            | Kind::Op(Op::LParen | Op::LBracket | Op::LBrace | Op::Hash | Op::Bang | Op::Tilde) => {
                true
            }
            _ => false,
        }
    }

    /// Statement de expressão `e;`.
    fn parse_expression_statement(&mut self, start: Span) -> PResult<StmtId> {
        let expr = self.parse_expression()?;
        self.expect_semicolon()?;
        Ok(self.push_stmt(start, StmtKind::Expression(expr)))
    }

    /// `@anotação` antes de uma declaração local. A metadata é descartada
    /// porque os nós de statement não têm campo para ela (ver módulo).
    fn parse_annotated_declaration(&mut self, start: Span) -> PResult<StmtId> {
        let _metadata = self.parse_metadata()?;
        if self.at_kw(Keyword::Var)
            || self.at_kw(Keyword::Final)
            || self.at_kw(Keyword::Const)
            || self.at_ident("late")
        {
            return self.parse_local_declaration(start);
        }
        if self.looks_like_local_function(self.pos) {
            return self.parse_local_function(start, None);
        }
        if self.declaration_type_at(self.pos, false) {
            return self.parse_typed_declaration(start);
        }
        Err(self.error("esperava uma declaração após a anotação"))
    }

    // -- Declarações locais -----------------------------------------------

    /// `late? (final | var | const)? tipo? nome (= e)? (, nome (= e)?)* ;`
    /// ou, com `var`/`final` seguido de `(`/`[`/`{`, uma declaração por padrão.
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
            return Err(self.error("esperava 'var', 'final', 'const', 'late' ou um tipo"));
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
            variables: Vec::new(),
        };
        self.parse_variables_rest(start, list)
    }

    /// `var (a, b) = e;` / `final [x, y] = e;`
    fn parse_pattern_variables(&mut self, start: Span) -> PResult<StmtId> {
        let final_ = self.at_kw(Keyword::Final);
        self.advance();
        let pattern = self.parse_pattern()?;
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

    /// Declaração que começa por um tipo (`int x = 1;`, `void f() {}`,
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
            variables: Vec::new(),
        };
        self.parse_variables_rest(start, list)
    }

    /// `nome (= e)? (, nome (= e)?)* ;` após os modificadores e o tipo.
    fn parse_variables_rest(&mut self, start: Span, mut list: VariableList) -> PResult<StmtId> {
        loop {
            let name = self.expect_identifier()?;
            let initializer = self.parse_initializer_opt()?;
            list.variables.push(Variable { name, initializer });
            if !self.eat_op(Op::Comma) {
                break;
            }
        }
        self.expect_semicolon()?;
        Ok(self.push_stmt(start, StmtKind::Variables(list)))
    }

    /// `= e` opcional após um nome de variável.
    fn parse_initializer_opt(&mut self) -> PResult<Option<crate::ast::ExprId>> {
        if self.eat_op(Op::Assign) {
            Ok(Some(self.parse_expression()?))
        } else {
            Ok(None)
        }
    }

    /// `nome<T>(params) corpo` após o tipo de retorno opcional.
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
            type_params,
            parameters: Some(parameters),
            modifier,
            body,
        };
        let id = self.ast.push_function(function);
        Ok(self.push_stmt(start, StmtKind::Function(id)))
    }

    // -- Statements compostos ---------------------------------------------

    /// `if (e) s`, `if (e) s else s`, `if (e case p when g) s`.
    fn parse_if(&mut self, start: Span) -> PResult<StmtId> {
        self.advance();
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

    /// Cabeçalho e corpo de `for`, com `for` (e `await`) já consumidos.
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
                updates,
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

    /// `; cond? ; (e (, e)*)? )` após o inicializador de um `for` clássico.
    fn parse_classic_for_rest(&mut self, init: Option<ForInit>) -> PResult<ForHeader> {
        self.expect_semicolon()?;
        let condition = if self.at_op(Op::Semicolon) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect_semicolon()?;
        let mut updates = Vec::new();
        // A gramática escrita não tem vírgula final aqui, mas o SDK aceita
        // `i += 15,)` (verificado no Dart 3.6.2), e o corpus usa.
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
        self.expect_op(Op::LParen)?;
        let condition = self.parse_expression()?;
        self.expect_op(Op::RParen)?;
        let body = self.parse_statement()?;
        Ok(self.push_stmt(start, StmtKind::While { condition, body }))
    }

    fn parse_do(&mut self, start: Span) -> PResult<StmtId> {
        self.advance();
        let body = self.parse_statement()?;
        self.expect_kw(Keyword::While)?;
        self.expect_op(Op::LParen)?;
        let condition = self.parse_expression()?;
        self.expect_op(Op::RParen)?;
        self.expect_semicolon()?;
        Ok(self.push_stmt(start, StmtKind::DoWhile { body, condition }))
    }

    /// `switch (e) { (rótulo:)* case p when g: stmts ... default: stmts }`.
    ///
    /// Cada `case` vira um [`SwitchCase`]; em `case a: case b: stmts` o corpo
    /// fica no último. Um erro dentro do corpo pula até a `}` que fecha o
    /// `switch` e devolve os cases já lidos.
    fn parse_switch(&mut self, start: Span) -> PResult<StmtId> {
        self.advance();
        self.expect_op(Op::LParen)?;
        let value = self.parse_expression()?;
        self.expect_op(Op::RParen)?;
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
        Ok(self.push_stmt(start, StmtKind::Switch { value, cases }))
    }

    fn parse_switch_cases(&mut self, cases: &mut Vec<SwitchCase>) -> PResult<()> {
        while !self.at_op(Op::RBrace) && !self.at_eof() {
            let case_start = self.span();
            let mut labels = Vec::new();
            while self.at_label() {
                labels.push(self.identifier());
                self.advance();
            }
            let (pattern, guard) = if self.eat_kw(Keyword::Case) {
                let pattern = self.parse_pattern()?;
                let guard = if self.eat_ident("when") {
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                self.expect_op(Op::Colon)?;
                (Some(pattern), guard)
            } else if self.eat_kw(Keyword::Default) {
                self.expect_op(Op::Colon)?;
                (None, None)
            } else {
                return Err(self.error("esperava 'case' ou 'default'"));
            };
            let body = self.parse_statement_list(true);
            cases.push(SwitchCase {
                span: self.span_from(case_start),
                labels,
                pattern,
                guard,
                body,
            });
        }
        Ok(())
    }

    /// `try {} (on T (catch (e, st))? {})* (catch (e, st) {})* (finally {})?`
    fn parse_try(&mut self, start: Span) -> PResult<StmtId> {
        self.advance();
        let body = self.parse_block()?;
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
            let body = self.parse_block()?;
            catches.push(CatchClause {
                span: self.span_from(clause_start),
                on_type,
                exception,
                stack_trace,
                body,
            });
        }
        let finally_ = if self.eat_kw(Keyword::Finally) {
            Some(self.parse_block()?)
        } else {
            None
        };
        if catches.is_empty() && finally_.is_none() {
            return Err(self.error("esperava 'on', 'catch' ou 'finally' após o bloco 'try'"));
        }
        Ok(self.push_stmt(
            start,
            StmtKind::Try {
                body,
                catches,
                finally_,
            },
        ))
    }

    /// `(e)` ou `(e, st)` após `catch`.
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
        self.advance();
        let star = self.eat_op(Op::Star);
        let value = self.parse_expression()?;
        self.expect_semicolon()?;
        Ok(self.push_stmt(start, StmtKind::Yield { star, value }))
    }

    /// `assert(c);`, `assert(c, msg);`, `assert(c, msg,);`
    fn parse_assert(&mut self, start: Span) -> PResult<StmtId> {
        self.advance();
        self.expect_op(Op::LParen)?;
        let condition = self.parse_expression()?;
        let mut message = None;
        if self.eat_op(Op::Comma) && !self.at_op(Op::RParen) {
            message = Some(self.parse_expression()?);
            self.eat_op(Op::Comma);
        }
        self.expect_op(Op::RParen)?;
        self.expect_semicolon()?;
        Ok(self.push_stmt(start, StmtKind::Assert { condition, message }))
    }

    // -- Lookaheads -------------------------------------------------------

    /// `nome:` no token corrente.
    fn at_label(&self) -> bool {
        self.at_identifier() && self.at_op_at(1, Op::Colon)
    }

    /// Em `pos` há `nome<T>?(params)` seguido de `{`, `=>`, `async` ou
    /// `sync` — uma função local, e não uma chamada.
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

    /// Posição após o grupo `<...>` que começa em `pos`, contando só `<` e
    /// `>` (que chegam isolados do lexer). Desiste em `;`, `{`, `}`, `=>`,
    /// `<<` ou fim — nada disso cabe numa lista de parâmetros de tipo.
    fn skip_angle_group(&self, pos: usize) -> Option<usize> {
        let mut depth = 0usize;
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
                Kind::Op(Op::Semicolon | Op::LBrace | Op::RBrace | Op::Arrow | Op::LtLt)
                | Kind::Eof => return None,
                _ => {}
            }
            i += 1;
        }
    }

    /// Em `pos` começa `tipo identificador` de uma declaração local?
    ///
    /// Pré-filtra as formas que nunca são tipo (para não pagar o lookahead
    /// completo em todo statement de expressão), aplica as regras do SDK que
    /// são do statement e não do tipo — `x as y` é expressão, `(…) async` é
    /// função anônima, e `a ? b : c` contra `int? x` quando
    /// `could_be_expression` — e delega o resto a
    /// [`Parser::looks_like_type_then_identifier`].
    fn declaration_type_at(&mut self, pos: usize, could_be_expression: bool) -> bool {
        match self.kind_of(pos) {
            Kind::Keyword(Keyword::Void) => true,
            Kind::Ident => {
                if matches!(self.text_of(pos), "await" | "yield") {
                    return false;
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
                    // `skip_type_arguments`, não `skip_angle_group`: um record nomeado
                    // `Future<({int a, int b})>` tem `{` dentro dos `<...>`.
                    Kind::Op(Op::Lt) => match self.skip_type_arguments(pos + 1) {
                        Some(after) => {
                            matches!(self.kind_of(after), Kind::Ident | Kind::Op(Op::Question))
                                && !self.is_expression_continuation(after)
                                && self.looks_like_type_then_identifier(pos)
                        }
                        None => false,
                    },
                    Kind::Op(Op::Question) => {
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

    /// O identificador em `pos` continua uma expressão em vez de nomear uma
    /// variável: `as` (salvo `x as = 1;`/`x as;`, que o SDK lê como variável
    /// chamada `as`), e `async`/`sync` de corpo de função anônima.
    fn is_expression_continuation(&self, pos: usize) -> bool {
        match self.text_of(pos) {
            "async" | "sync" => true,
            "as" => !matches!(
                self.kind_of(pos + 1),
                Kind::Op(Op::Assign | Op::Semicolon | Op::Comma)
            ),
            _ => false,
        }
    }

    /// Em `q` há `?` após um tipo simples; o que segue é uma expressão
    /// condicional (`a ? b : c`) em vez de `tipo? nome`?
    ///
    /// Segue o SDK: só é declaração quando o identificador seguinte é
    /// seguido de `;`, `,`, `)`, `in`, fim, ou de `=` cuja expressão não é
    /// seguida de `:` — este último caso exige análise especulativa.
    fn conditional_after_question(&mut self, q: usize) -> bool {
        if self.kind_of(q + 1) != Kind::Ident {
            return false;
        }
        match self.kind_of(q + 2) {
            Kind::Op(Op::Semicolon | Op::Comma | Op::RParen) | Kind::Eof => false,
            Kind::Keyword(Keyword::In) => false,
            // `Object? convert(Object? o) {` é função local com retorno
            // anulável; `Object ? convert(o) : x` é condicional. O que
            // distingue é o que vem depois dos parênteses.
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

    /// Executa `f` e desfaz tudo o que ela produziu: posição, nós, diagnósticos
    /// e profundidade. Serve para decidir uma ambiguidade olhando adiante.
    pub(crate) fn speculate<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let pos = self.pos;
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
        self.pos = pos;
        self.depth = depth;
        self.diagnostics.truncate(diagnostics);
        self.ast.exprs.truncate(exprs);
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

    fn expect_semicolon(&mut self) -> PResult<()> {
        self.expect_op(Op::Semicolon).map(|_| ())
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

    /// Resultado de um `parse_statement` sobre `src`.
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
            assert_eq!(p.depth, 0, "profundidade não restaurada");
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

    /// Statement aceito sem diagnósticos, consumindo toda a fonte.
    fn ok(src: &str) -> Out {
        let out = stmt(src);
        assert!(out.diagnostics.is_empty(), "{src}: {:?}", out.diagnostics);
        assert!(out.result.is_ok(), "{src}");
        assert!(out.at_eof, "{src}: sobrou entrada");
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

    // -- Independentes de outros módulos ------------------------------------

    #[test]
    fn bloco_vazio_e_statement_vazio() {
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
    fn rotulos() {
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
    fn for_classico_vazio_e_await_for() {
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
    fn variaveis_locais_sem_tipo_nem_inicializador() {
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

    #[test]
    fn switch_sem_expressoes_falha_cedo_mas_recupera_na_chave() {
        // Sem `case`/`default` o corpo é inválido; o parser pula até a `}`.
        let out = stmt("switch (x) { break; }");
        // `x` exige o módulo de expressões; se ele ainda é stub o teste
        // panica, e isso é esperado (ver módulo `dependentes`).
        assert!(out.result.is_ok());
        assert_eq!(out.diagnostics.len(), 1);
        assert!(out.at_eof);
    }

    #[test]
    fn recuperacao_em_bloco() {
        // `)` não começa statement: pula até o `;` e continua.
        let out = stmt("{ ) ; break; }");
        assert!(out.result.is_ok());
        assert_eq!(out.diagnostics.len(), 1);
        assert!(matches!(out.kind(), StmtKind::Block(s) if s.len() == 1));
        assert!(out.at_eof);

        // Falta `;`: para na `}` do bloco sem consumi-la.
        let out = stmt("{ break }");
        assert!(out.result.is_ok());
        assert_eq!(out.diagnostics.len(), 1);
        assert!(matches!(out.kind(), StmtKind::Block(s) if s.is_empty()));
        assert!(out.at_eof);

        // Grupos aninhados são pulados inteiros durante a sincronização.
        let out = stmt("{ ) [ ; { ; } ] ; break; }");
        assert!(out.result.is_ok());
        assert_eq!(out.diagnostics.len(), 1);
        assert!(matches!(out.kind(), StmtKind::Block(s) if s.len() == 1));

        // Bloco sem fechamento.
        let out = stmt("{ break;");
        assert!(out.result.is_err());
        assert!(out.diagnostics[0].message.contains("fim do arquivo"));
    }

    #[test]
    fn palavras_reservadas_que_nao_comecam_statement() {
        for src in ["else;", "case 1:", "class A {}", "finally {}"] {
            let out = stmt(src);
            assert!(out.result.is_err(), "{src}");
            assert!(out.diagnostics[0].message.contains("esperava um statement"));
        }
    }

    #[test]
    fn limite_de_profundidade() {
        let depth = super::super::MAX_DEPTH as usize + 10;
        let src = format!("{}{}", "{".repeat(depth), "}".repeat(depth));
        let out = stmt(&src);
        assert!(out.result.is_ok());
        assert_eq!(out.diagnostics.len(), 1);
        assert!(out.diagnostics[0].message.contains("aninhamento"));
        assert!(out.at_eof);
    }

    #[test]
    fn lookahead_de_funcao_local() {
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

    // -- Dependentes de expressions.rs / types.rs / declarations.rs ---------
    //
    // Enquanto esses módulos forem stubs `todo!()`, estes testes panicam com
    // "not yet implemented"; passam a valer quando os módulos existirem.
    mod dependentes {
        use super::*;

        /// Regressões do corpus: função local com retorno anulável, campo
        /// `operator`, padrão de objeto em `for-in`, vírgula final em `for`.
        #[test]
        fn funcao_local_com_retorno_nulavel() {
            for src in [
                "Object? convert(Object? o) { return o; }",
                "Object? anything() => _inscrutable(0);",
                "_Credentials? find(_Scheme scheme) { return null; }",
                "T? g<T>(T t) => t;",
                // Record nomeado dentro de `<...>` (visto no projeto new_sali).
                "Future<({int total, List<Map<String, dynamic>> itens})> paginar() async {}",
            ] {
                let out = ok(src);
                assert!(matches!(out.kind(), StmtKind::Function(_)), "{src}");
            }
            // A condicional continua sendo condicional.
            let out = ok("Object ? convert(o) : x;");
            assert!(matches!(out.kind(), StmtKind::Expression(_)));
        }

        #[test]
        fn for_in_com_padrao_de_objeto_e_virgula_final() {
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
        fn if_else_e_if_case() {
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
        fn while_e_do() {
            let out = ok("while (true) { break; }");
            assert!(matches!(out.kind(), StmtKind::While { .. }));
            let out = ok("do { x++; } while (x < 10);");
            assert!(matches!(out.kind(), StmtKind::DoWhile { .. }));
        }

        #[test]
        fn for_classico() {
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
        fn try_com_on() {
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
        fn declaracoes_de_variaveis() {
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
        fn declaracoes_por_padrao() {
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
            // Tipo record não é padrão.
            let out = ok("final (int, int) r = (1, 2);");
            assert!(matches!(out.kind(), StmtKind::Variables(_)));
        }

        #[test]
        fn funcoes_locais() {
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
            // Chamada, não função.
            let out = ok("f(x);");
            assert!(matches!(out.kind(), StmtKind::Expression(_)));
        }

        #[test]
        fn statements_de_expressao() {
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
            // `await x;` só é expressão dentro de `async`; fora, o SDK lê
            // `await` como nome de tipo de uma declaração (`await x;`).
            let out = stmt_with("await x;", |p| p.in_async = true);
            assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
            assert!(matches!(out.kind(), StmtKind::Expression(_)));
            // `int? x = c;` continua sendo declaração.
            let out = ok("int? x = c;");
            assert!(matches!(out.kind(), StmtKind::Variables(_)));
            let out = ok("int? x;");
            assert!(matches!(out.kind(), StmtKind::Variables(_)));
        }

        #[test]
        fn bloco_com_varios_statements() {
            let out = ok("{ int x = 1; x++; if (x > 1) return; }");
            assert!(matches!(out.kind(), StmtKind::Block(s) if s.len() == 3));
        }
    }
}
