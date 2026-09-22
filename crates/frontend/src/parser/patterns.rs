//! Padrões de Dart 3: `switch`, `if case`, declarações e atribuições.
//!
//! Gramática coberta (Dart 3.6): `pattern` = `logicalOrPattern`, com
//! `logicalAndPattern`, `relationalPattern`, `unaryPattern` (`p?`, `p!`,
//! `p as T`) e `primaryPattern` (constante, variável, `_`, parênteses, lista,
//! mapa, record e objeto).
//!
//! Decisões que a resolução precisa conhecer:
//!
//! * Um identificador solto (não seguido de `(`, `<` ou `.`) vira **sempre**
//!   [`PatternKind::Variable`] sem `var`/`final`/tipo. Em contexto de
//!   declaração ou atribuição isso é uma variável; em contexto de
//!   correspondência (`case x:`) a linguagem lê como referência a uma
//!   constante. O parser não sabe em que contexto está (o mesmo `case` serve
//!   a `switch` e `if case`), então a fase seguinte decide pelo contexto.
//! * `tipo identificador` (`int x`, `List<int> xs`, `a.B c`, `(int, int) p`) é
//!   variável tipada; `int _` é [`PatternKind::Wildcard`] tipado. Como no SDK,
//!   `TIPO as`/`TIPO when` **não** é variável chamada `as`/`when`: `Foo as Bar`
//!   é o padrão `Foo` convertido para `Bar`, e `x when g` termina o padrão em
//!   `x`.
//! * `Nome(...)`, `p.Nome(...)`, `Nome<T>(...)` é objeto; o tipo é montado
//!   aqui (nunca tem `?`). `:x` em campo de record/objeto infere o nome do
//!   padrão de variável interno, atravessando `?`, `!`, `as` e parênteses.
//! * Constantes (literais, `-1`, `const [...]`, `a.b.c`) são lidas com
//!   `parse_unary_expression`, um superconjunto do que a gramática permite.
//! * `(p)` sem vírgula nem nome é parênteses; `(p,)`, `()` e `(a: p)` são
//!   records. `(int, int) x` é variável com tipo record.
//! * `<T>[...]`/`<K, V>{...}` levam argumentos de tipo; `<` que não abre uma
//!   lista de tipos válida seguida de `[`/`{` é o relacional `< e`.
//! * `as` não é palavra reservada (`at_ident("as")`); `>=` chega como `>`
//!   colado a `=` e é lido por `composed_gt`.
use super::{PResult, Parser};
use crate::ast::{
    BinaryOp, ListPatternElement, MapPatternEntry, Name, Pattern, PatternField, PatternId,
    PatternKind, TypeAnnotation, TypeKind,
};
use crate::token::{Keyword, Kind, Op};
use dartforge_diagnostics::Span;

impl<'s, 'i> Parser<'s, 'i> {
    /// `pattern` completo (com `||`, `&&`, `as`, `?`, `!`).
    pub(crate) fn parse_pattern(&mut self) -> PResult<PatternId> {
        self.enter()?;
        let result = self.parse_or_pattern();
        self.leave();
        result
    }

    /// Em `pos` começa uma declaração por padrão `var (…) =`, `final [`,
    /// `var {`, `final <T>[`? Para `(` exige `=` ou `in` após o `)`
    /// correspondente, porque `final (int, int) r = …` é um tipo record.
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
            // Padrão de objeto: `final MapEntry(key: k, value: v) in …`,
            // `final p.Nome<T>(…) = …`. Uma declaração comum nunca tem `(`
            // logo após o nome do tipo.
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

    /// O grupo `(...)` em `pos` fecha e é seguido de `=` ou `in`.
    fn paren_group_then_assign_or_in(&self, pos: usize) -> bool {
        match self.matching_close(pos) {
            Some(close) => matches!(
                self.kind_of(close + 1),
                Kind::Op(Op::Assign) | Kind::Keyword(Keyword::In)
            ),
            None => false,
        }
    }

    /// Em `pos` começa uma atribuição por padrão: um `outerPattern` —
    /// `(a, b)`, `[x]`, `{'k': v}`, `<T>[…]`, `Nome(…)`, `p.Nome<T>(…)` —
    /// cujo fechamento é seguido de `=` (e não de `==`, que o lexer já
    /// distingue)?
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

    // -- Níveis binários e unários ----------------------------------------

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

    /// `primaryPattern` seguido de `?`, `!` ou `as T`, quantos houver.
    fn parse_unary_pattern(&mut self) -> PResult<PatternId> {
        let start = self.span();
        let mut pattern = self.parse_primary_pattern()?;
        loop {
            if self.eat_op(Op::Question) {
                pattern = self.push_pattern(start, PatternKind::NullCheck(pattern));
            } else if self.eat_op(Op::Bang) {
                pattern = self.push_pattern(start, PatternKind::NullAssert(pattern));
            } else if self.eat_ident("as") {
                let ty = self.parse_type()?;
                pattern = self.push_pattern(start, PatternKind::Cast { pattern, ty });
            } else {
                return Ok(pattern);
            }
        }
    }

    // -- Primários --------------------------------------------------------

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
            | Kind::Op(Op::Minus | Op::Hash | Op::Bang | Op::Tilde) => {
                self.parse_constant_pattern(start)
            }
            _ => Err(self.error("esperava um padrão")),
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
                _ => return Err(self.error("esperava um operador relacional")),
            },
            _ => return Err(self.error("esperava um operador relacional")),
        };
        self.advance();
        let value = self.parse_bitwise_or_expression()?;
        Ok(self.push_pattern(start, PatternKind::Relational { op, value }))
    }

    /// Literal, `-1`, `const …`, `a.b.c`: qualquer `unaryExpression`.
    fn parse_constant_pattern(&mut self, start: Span) -> PResult<PatternId> {
        let expr = self.parse_unary_expression()?;
        Ok(self.push_pattern(start, PatternKind::Constant(expr)))
    }

    /// `var x`, `final x`, `final int x`, `var _`.
    fn parse_variable_pattern(&mut self, start: Span) -> PResult<PatternId> {
        let final_ = self.at_kw(Keyword::Final);
        let var_ = !final_;
        self.advance();
        let ty = if final_ && self.variable_pattern_type_end(self.pos).is_some() {
            Some(self.parse_type()?)
        } else {
            None
        };
        let name = self.expect_identifier()?;
        Ok(self.finish_variable(start, final_, var_, ty, name))
    }

    /// Padrão que começa com identificador: `_`, `int x`, `int _`, `Nome(…)`,
    /// `a.b` constante ou identificador solto.
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
            return Ok(self.finish_variable(start, false, false, Some(ty), name));
        }
        if self.object_pattern_paren(pos).is_some() {
            return self.parse_object_pattern(start);
        }
        if matches!(next, Kind::Op(Op::Dot | Op::Lt)) {
            return self.parse_constant_pattern(start);
        }
        let name = self.identifier();
        Ok(self.finish_variable(start, false, false, None, name))
    }

    /// Variável ou, se o nome é `_`, curinga (com o mesmo tipo).
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

    /// `Nome<T>(campos)` / `p.Nome(campos)`, com o `(` garantido por
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

    /// `(p)`, `()`, `(p,)`, `(a, b: p, :x)` ou `(int, int) nome`.
    fn parse_parenthesized_or_record_pattern(&mut self, start: Span) -> PResult<PatternId> {
        if self.variable_pattern_type_end(self.pos).is_some() {
            let ty = self.parse_type()?;
            let name = self.expect_identifier()?;
            return Ok(self.finish_variable(start, false, false, Some(ty), name));
        }
        self.expect_op(Op::LParen)?;
        if self.eat_op(Op::RParen) {
            return Ok(self.push_pattern(
                start,
                PatternKind::Record {
                    fields: Vec::new().into_boxed_slice(),
                },
            ));
        }
        let first = self.parse_pattern_field()?;
        if first.name.is_none() && self.at_op(Op::RParen) {
            self.advance();
            return Ok(self.push_pattern(start, PatternKind::Parenthesized(first.pattern)));
        }
        let mut fields = vec![first];
        while self.eat_op(Op::Comma) {
            if self.at_op(Op::RParen) {
                break;
            }
            fields.push(self.parse_pattern_field()?);
        }
        self.expect_op(Op::RParen)?;
        Ok(self.push_pattern(
            start,
            PatternKind::Record {
                fields: fields.into_boxed_slice(),
            },
        ))
    }

    /// `(campo, nome: p, :x)` de um padrão de objeto, com os parênteses.
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

    /// `p`, `nome: p` ou `:p` (nome inferido do padrão de variável).
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

    /// Nome do campo `:x`: o da variável dentro de `x`, `x?`, `x!`, `x as T`,
    /// `(x)`, `var x`, `final int x`.
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
                    return Err(
                        self.error_at(span, "campo ':' sem nome exige um padrão de variável")
                    );
                }
            }
        }
    }

    /// `[p, ..., ...rest]` após os argumentos de tipo.
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

    /// `{k: p, ...}` após os argumentos de tipo; `...` só sem sub-padrão.
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

    /// Se em `pos` há `tipo identificador` de padrão de variável, a posição
    /// do identificador. Aplica a regra do SDK de que `TIPO as`/`TIPO when`
    /// não é variável, e pré-filtra as formas que nunca são tipo antes de
    /// consultar [`Parser::looks_like_type_then_identifier`].
    fn variable_pattern_type_end(&self, pos: usize) -> Option<usize> {
        let candidate = match self.kind_of(pos) {
            Kind::Ident => match self.kind_of(pos + 1) {
                Kind::Ident => pos + 1,
                Kind::Op(Op::Dot | Op::Lt | Op::Question) => {
                    if !self.looks_like_type_then_identifier(pos) {
                        return None;
                    }
                    self.skip_type(pos)?
                }
                _ => return None,
            },
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

    /// `as` e `when` terminam o padrão anterior em vez de nomear uma variável.
    fn is_pattern_terminator(&self, pos: usize) -> bool {
        matches!(self.text_of(pos), "as" | "when")
    }

    /// Posição do `(` de um padrão de objeto que começa em `pos`:
    /// `Nome(`, `p.Nome(`, `Nome<T>(`.
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

    fn push_pattern(&mut self, start: Span, kind: PatternKind) -> PatternId {
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

    struct Out {
        names: Interner,
        ast: Ast,
        result: PResult<PatternId>,
        diagnostics: Vec<Diagnostic>,
        /// Texto do token onde o parser parou.
        rest: String,
    }

    fn pattern(src: &str) -> Out {
        let mut names = Interner::new();
        let tokens = crate::lexer::lex(src).unwrap();
        let (ast, result, diagnostics, rest) = {
            let mut p = Parser::new(src, tokens, &mut names);
            let result = p.parse_pattern();
            assert_eq!(p.depth, 0, "profundidade não restaurada");
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

    /// Padrão aceito sem diagnósticos, consumindo toda a fonte.
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
                other => panic!("não é variável: {other:?}"),
            }
        }
    }

    // -- Independentes de outros módulos ------------------------------------

    #[test]
    fn curinga_e_variaveis() {
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
    fn identificador_para_antes_de_when() {
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
    fn unarios_posfixos() {
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
    fn logicos_com_precedencia_e_associatividade() {
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
    fn parenteses_e_records() {
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
    fn nome_inferido_atravessa_unarios() {
        let out = ok("(:x?, :var y!, :final z)");
        let PatternKind::Record { fields } = out.kind() else {
            panic!()
        };
        assert_eq!(out.name(fields[0].name.as_ref().unwrap()), "x");
        assert_eq!(out.name(fields[1].name.as_ref().unwrap()), "y");
        assert_eq!(out.name(fields[2].name.as_ref().unwrap()), "z");

        let out = pattern("(:_)");
        assert!(out.result.is_err());
        assert!(out.diagnostics[0].message.contains("variável"));
    }

    #[test]
    fn listas() {
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
    fn mapas_sem_chaves_de_expressao() {
        let out = ok("{}");
        assert!(
            matches!(out.kind(), PatternKind::Map { entries, rest: false, .. } if entries.is_empty())
        );
        let out = ok("{...}");
        assert!(matches!(out.kind(), PatternKind::Map { rest: true, .. }));
    }

    #[test]
    fn objetos() {
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
    fn erros() {
        let out = pattern(")");
        assert!(out.result.is_err());
        assert!(out.diagnostics[0].message.contains("esperava um padrão"));
        let out = pattern("[a");
        assert!(out.result.is_err());
        let out = pattern("(a, b");
        assert!(out.result.is_err());
    }

    #[test]
    fn lookahead_de_declaracao_por_padrao() {
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
    fn lookahead_de_atribuicao_por_padrao() {
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
    fn limite_de_profundidade() {
        let depth = super::super::MAX_DEPTH as usize + 10;
        let src = format!("{}x{}", "(".repeat(depth), ")".repeat(depth));
        let out = pattern(&src);
        assert!(out.result.is_err());
        assert!(out.diagnostics[0].message.contains("aninhamento"));
    }

    // -- Dependentes de expressions.rs / types.rs ---------------------------
    //
    // Panicam com "not yet implemented" enquanto esses módulos forem stubs.
    mod dependentes {
        use super::*;

        #[test]
        fn constantes() {
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
        fn relacionais() {
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
        fn variaveis_tipadas() {
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
        fn cast_e_tipo_seguido_de_as() {
            let out = ok("x as int");
            assert!(matches!(out.kind(), PatternKind::Cast { .. }));
            // `Foo as Bar` é o padrão `Foo` convertido, não variável `as`.
            let out = ok("Foo as Bar");
            let PatternKind::Cast { pattern, .. } = out.kind() else {
                panic!()
            };
            assert_eq!(out.var_name(*pattern), "Foo");
            let out = ok("(a, b) as (int, int)");
            assert!(matches!(out.kind(), PatternKind::Cast { .. }));
        }

        #[test]
        fn colecoes_tipadas_e_mapas() {
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
        fn objetos_genericos_e_constantes_qualificadas() {
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
