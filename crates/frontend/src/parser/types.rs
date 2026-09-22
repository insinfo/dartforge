//! Tipos, parâmetros de tipo, argumentos de tipo e parâmetros formais.
//!
//! Cobre as seções `type`, `typeParameters`, `typeArguments`,
//! `formalParameterList` e `parameterTypeList` da gramática de Dart 3.6, mais
//! os *lookaheads* que o resto do parser usa para decidir se uma sequência de
//! tokens é um tipo sem consumir nada ([`Parser::skip_type`],
//! [`Parser::skip_type_arguments`], [`Parser::looks_like_type_then_identifier`]).
//!
//! Decisões que não são tradução direta da gramática:
//!
//! * **Tipos de função encadeados** (`int Function(int) Function(String)`)
//!   associam à esquerda: o `Function` mais à direita é o nó externo e o da
//!   esquerda é seu tipo de retorno. Um `?` entre duas caudas
//!   (`int Function()? Function()`) torna nulável a cauda da esquerda.
//! * **Records** podem ser tipo de retorno de função (`(int, int) Function()`),
//!   como no parser do SDK (`computeRecordTypeGFT`).
//! * **`?` após `is`/`as`**: `x is int ? a : b` é uma condicional, não um tipo
//!   nulável. [`Parser::parse_type_after_is_or_as`] reproduz a regra de
//!   `computeTypeAfterIsOrAs` do SDK: o `?` final só pertence ao tipo se o
//!   token seguinte não puder começar o ramo de uma condicional.
//! * **Identificadores embutidos** (`get`, `set`, `static`, `typedef`…) não
//!   contam como nome de tipo nos *lookaheads*, salvo se qualificados
//!   (`p.get`): é a regra `isValidNonRecordTypeReference` do SDK, que evita
//!   ler `get x` como "tipo `get`, nome `x`". `dynamic` e `Function` são
//!   nomes de tipo normais.
//! * **Uma lista de parâmetros, dois modos.** A mesma rotina lê listas de
//!   parâmetros de declaração (`this.x`, `super.x`, defaults, forma antiga
//!   `int f(int x)`) e de tipo de função (`Function(int, String)`, onde o nome
//!   é opcional). O modo é um booleano interno; publicamente
//!   [`Parser::parse_formal_parameters`] é o modo declaração e
//!   [`Parser::parse_function_type_parameters`] o modo tipo de função.
//! * `>` sempre chega isolado do lexer, então `<` … `>` de tipos nunca precisa
//!   de retrocesso; `in_type_args` é incrementado durante a leitura para que
//!   expressões dentro de metadata não componham `>>`.
use super::{MAX_DEPTH, PResult, Parser};
use crate::ast::{
    Annotation, Parameter, ParameterKind, TypeAnnotation, TypeId, TypeKind, TypeParameter,
};
use crate::token::{Keyword, Kind, Op};
use dartforge_diagnostics::Span;

/// Como tratar um `?` final ao ler um tipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrailingQuestion {
    /// Contexto de tipo puro: `?` final é sempre nulabilidade.
    Always,
    /// Após `is`/`as` numa expressão: `?` só pertence ao tipo se o token
    /// seguinte não puder iniciar o ramo `then` de uma condicional.
    AfterIsOrAs,
}

/// Identificadores embutidos (§17.1) que não servem de nome de tipo sem
/// qualificação. `dynamic` e `Function` ficam de fora porque são tipos.
fn is_builtin_identifier(text: &str) -> bool {
    matches!(
        text,
        "abstract"
            | "as"
            | "covariant"
            | "deferred"
            | "export"
            | "extension"
            | "external"
            | "factory"
            | "get"
            | "implements"
            | "import"
            | "interface"
            | "late"
            | "library"
            | "mixin"
            | "operator"
            | "part"
            | "required"
            | "set"
            | "static"
            | "typedef"
    )
}

impl<'s, 'i> Parser<'s, 'i> {
    // -- Tipos -----------------------------------------------------------

    /// `type`: nome qualificado com argumentos, `void`, tipo de função ou
    /// record, com `?` final.
    pub(crate) fn parse_type(&mut self) -> PResult<TypeId> {
        self.parse_type_with(TrailingQuestion::Always)
    }

    /// Tipo após `is`/`as` numa expressão: igual a [`Parser::parse_type`],
    /// mas um `?` final só é consumido quando o token seguinte é um dos que
    /// podem seguir um tipo nulável nesse lugar (`)`, `,`, `;`, `:`, `?`,
    /// `??`, `..`, `}`, `]`, `is`, `as` ou fim do arquivo). Assim
    /// `x is int ? a : b` deixa o `?` para a condicional, como no SDK.
    pub(crate) fn parse_type_after_is_or_as(&mut self) -> PResult<TypeId> {
        self.parse_type_with(TrailingQuestion::AfterIsOrAs)
    }

    fn parse_type_with(&mut self, mode: TrailingQuestion) -> PResult<TypeId> {
        self.enter()?;
        let result = self.parse_type_inner(mode);
        self.leave();
        result
    }

    /// Lê o tipo base e depois zero ou mais caudas `Function<T>(...)`,
    /// aplicando `?` onde a gramática permite.
    fn parse_type_inner(&mut self, mode: TrailingQuestion) -> PResult<TypeId> {
        let start = self.span();
        let mut ty = if self.at_function_tail(self.pos) {
            self.parse_function_tail(None, start)?
        } else {
            self.parse_non_function_type()?
        };
        loop {
            if self.at_op(Op::Question) && !self.is_bare_void(ty) {
                if self.at_function_tail(self.pos + 1) {
                    // `int? Function()` ou `int Function()? Function()`.
                    self.advance();
                    self.mark_nullable(ty);
                    ty = self.parse_function_tail(Some(ty), start)?;
                    continue;
                }
                if mode == TrailingQuestion::AfterIsOrAs
                    && !self.question_may_end_type(self.pos + 1)
                {
                    break;
                }
                self.advance();
                self.mark_nullable(ty);
                break;
            }
            if self.at_function_tail(self.pos) {
                ty = self.parse_function_tail(Some(ty), start)?;
                continue;
            }
            break;
        }
        Ok(ty)
    }

    /// `void`, record `(...)` ou nome qualificado com argumentos — sem `?` e
    /// sem cauda `Function`.
    fn parse_non_function_type(&mut self) -> PResult<TypeId> {
        let start = self.span();
        if self.eat_kw(Keyword::Void) {
            let span = self.span_from(start);
            return Ok(self.ast.push_type(TypeAnnotation {
                span,
                nullable: false,
                kind: TypeKind::Void,
            }));
        }
        if self.at_op(Op::LParen) {
            return self.parse_record_type();
        }
        if self.at_identifier() {
            let mut name = vec![self.identifier()];
            if self.at_op(Op::Dot) && self.at_identifier_at(1) {
                self.advance();
                name.push(self.identifier());
            }
            let args = self.parse_type_arguments_opt()?;
            let span = self.span_from(start);
            return Ok(self.ast.push_type(TypeAnnotation {
                span,
                nullable: false,
                kind: TypeKind::Named { name, args },
            }));
        }
        Err(self.error("esperava um tipo"))
    }

    /// `Function<T>(params)` com o tipo de retorno já lido (ou nenhum).
    fn parse_function_tail(&mut self, return_type: Option<TypeId>, start: Span) -> PResult<TypeId> {
        self.expect_ident("Function")?;
        let type_params = self.parse_type_parameters_opt()?;
        let parameters = self.parse_function_type_parameters()?;
        let span = self.span_from(start);
        Ok(self.ast.push_type(TypeAnnotation {
            span,
            nullable: false,
            kind: TypeKind::Function {
                return_type,
                type_params,
                parameters,
            },
        }))
    }

    /// `recordType`: `()`, `(T, U)`, `(T a, {U b})`, `({U b})`. O nome de um
    /// campo posicional e a metadata dos campos são aceitos e descartados —
    /// a árvore não os guarda.
    fn parse_record_type(&mut self) -> PResult<TypeId> {
        let start = self.span();
        self.expect_op(Op::LParen)?;
        let mut positional = Vec::new();
        let mut named = Vec::new();
        loop {
            if self.eat_op(Op::RParen) {
                break;
            }
            if self.eat_op(Op::LBrace) {
                loop {
                    if self.eat_op(Op::RBrace) {
                        break;
                    }
                    self.parse_metadata_opt()?;
                    let ty = self.parse_type()?;
                    let name = self.expect_identifier()?;
                    named.push((name, ty));
                    if !self.eat_op(Op::Comma) {
                        self.expect_op(Op::RBrace)?;
                        break;
                    }
                }
                self.expect_op(Op::RParen)?;
                break;
            }
            self.parse_metadata_opt()?;
            let ty = self.parse_type()?;
            if self.at_identifier() {
                self.identifier();
            }
            positional.push(ty);
            if !self.eat_op(Op::Comma) {
                self.expect_op(Op::RParen)?;
                break;
            }
        }
        let span = self.span_from(start);
        Ok(self.ast.push_type(TypeAnnotation {
            span,
            nullable: false,
            kind: TypeKind::Record { positional, named },
        }))
    }

    fn mark_nullable(&mut self, ty: TypeId) {
        let end = self.last_end();
        let node = &mut self.ast.types[ty.0 as usize];
        node.nullable = true;
        node.span.end = end.max(node.span.end);
    }

    fn is_bare_void(&self, ty: TypeId) -> bool {
        matches!(self.ast.ty(ty).kind, TypeKind::Void)
    }

    /// Token corrente é `Function` seguido de `<` ou `(` — início de uma
    /// cauda de tipo de função (`isGeneralizedFunctionType` do SDK).
    fn at_function_tail(&self, pos: usize) -> bool {
        self.kind_of(pos) == Kind::Ident
            && self.text_of(pos) == "Function"
            && matches!(
                self.kind_of(pos + 1),
                Kind::Op(Op::Lt) | Kind::Op(Op::LParen)
            )
    }

    /// Após `is`/`as`, o token em `pos` (o que segue o `?`) permite que o `?`
    /// seja parte do tipo. Lista de `computeTypeAfterIsOrAs` no SDK.
    /// Após `is T` / `as T`, o `?` é sufixo do tipo só quando o token
    /// seguinte não pode iniciar o ramo verdadeiro de uma condicional. É a
    /// lista de `computeTypeAfterIsOrAs` do parser do SDK
    /// (`parser_impl.dart`): fechamentos, separadores, `is`/`as`, `..`,
    /// `||`, `&&` e fim. `{` e `when` são ambíguos (`x is T ? {} : y` contra
    /// o último inicializador de um construtor com corpo) e o SDK resolve por
    /// tentativa: é condicional se `? expr : expr` analisa a partir dali.
    fn question_may_end_type(&mut self, pos: usize) -> bool {
        match self.kind_of(pos) {
            Kind::Op(
                Op::RParen
                | Op::Question
                | Op::QuestionQuestion
                | Op::Comma
                | Op::Semicolon
                | Op::Colon
                | Op::DotDot
                | Op::RBrace
                | Op::RBracket
                | Op::PipePipe
                | Op::AmpAmp,
            )
            | Kind::Keyword(Keyword::Is)
            | Kind::Eof => true,
            Kind::Op(Op::LBrace) => !self.conditional_parses_at(pos),
            Kind::Ident => match self.text_of(pos) {
                "as" => true,
                "when" => !self.conditional_parses_at(pos),
                _ => false,
            },
            _ => false,
        }
    }

    /// `expr : expr` analisa a partir de `pos` (o token após o `?`)? Desfaz
    /// tudo o que tentou.
    fn conditional_parses_at(&mut self, pos: usize) -> bool {
        let saved = self.pos;
        self.pos = pos;
        let ok = self.speculate(|p| {
            p.parse_expression_without_cascade().is_ok()
                && p.eat_op(Op::Colon)
                && p.parse_expression_without_cascade().is_ok()
        });
        self.pos = saved;
        ok
    }

    /// Metadata se houver `@`; caso contrário lista vazia sem tocar em nada.
    fn parse_metadata_opt(&mut self) -> PResult<Vec<Annotation>> {
        if self.at_op(Op::At) {
            self.parse_metadata()
        } else {
            Ok(Vec::new())
        }
    }

    // -- Argumentos e parâmetros de tipo ----------------------------------

    /// `<T, U>` de argumentos, ou lista vazia se não houver `<`.
    ///
    /// Em contexto de tipo `<` é sempre argumento; quem está em contexto de
    /// expressão decide antes com [`Parser::skip_type_arguments`].
    pub(crate) fn parse_type_arguments_opt(&mut self) -> PResult<Vec<TypeId>> {
        if !self.at_op(Op::Lt) {
            return Ok(Vec::new());
        }
        self.in_type_args += 1;
        let result = self.parse_type_arguments_inner();
        self.in_type_args -= 1;
        result
    }

    fn parse_type_arguments_inner(&mut self) -> PResult<Vec<TypeId>> {
        self.expect_op(Op::Lt)?;
        let mut args = Vec::new();
        loop {
            args.push(self.parse_type()?);
            if !self.eat_op(Op::Comma) {
                break;
            }
        }
        self.expect_op(Op::Gt)?;
        Ok(args)
    }

    /// `<@meta T extends B, U>` de parâmetros, ou lista vazia se não houver `<`.
    ///
    /// Dart 3.6 não tem variância explícita (`in`/`out` só sob experimento e
    /// sem uso no SDK), então não é reconhecida.
    pub(crate) fn parse_type_parameters_opt(&mut self) -> PResult<Vec<TypeParameter>> {
        if !self.at_op(Op::Lt) {
            return Ok(Vec::new());
        }
        self.in_type_args += 1;
        let result = self.parse_type_parameters_inner();
        self.in_type_args -= 1;
        result
    }

    fn parse_type_parameters_inner(&mut self) -> PResult<Vec<TypeParameter>> {
        self.expect_op(Op::Lt)?;
        let mut params = Vec::new();
        loop {
            let start = self.span();
            let metadata = self.parse_metadata_opt()?;
            let name = self.expect_identifier()?;
            let bound = if self.eat_kw(Keyword::Extends) {
                Some(self.parse_type()?)
            } else {
                None
            };
            params.push(TypeParameter {
                span: self.span_from(start),
                metadata,
                name,
                bound,
            });
            if !self.eat_op(Op::Comma) {
                break;
            }
        }
        self.expect_op(Op::Gt)?;
        Ok(params)
    }

    // -- Parâmetros formais ------------------------------------------------

    /// `(a, [b = 1], {required c})` completo, com os parênteses, no modo
    /// declaração: nomes obrigatórios, `this.x`/`super.x`, defaults e a forma
    /// antiga `int f(int x)`.
    pub(crate) fn parse_formal_parameters(&mut self) -> PResult<Vec<Parameter>> {
        self.parse_parameter_list(false)
    }

    /// `parameterTypeList` de um tipo de função, com os parênteses:
    /// `(int, String name, [int], {required int a})`. Um item sem nome vira
    /// `Parameter { ty: Some(_), name: None }`.
    pub(crate) fn parse_function_type_parameters(&mut self) -> PResult<Vec<Parameter>> {
        self.parse_parameter_list(true)
    }

    fn parse_parameter_list(&mut self, in_function_type: bool) -> PResult<Vec<Parameter>> {
        self.expect_op(Op::LParen)?;
        let mut params = Vec::new();
        loop {
            if self.eat_op(Op::RParen) {
                return Ok(params);
            }
            let group = if self.at_op(Op::LBracket) {
                Some((ParameterKind::Optional, Op::RBracket))
            } else if self.at_op(Op::LBrace) {
                Some((ParameterKind::Named, Op::RBrace))
            } else {
                None
            };
            if let Some((kind, close)) = group {
                self.advance();
                loop {
                    if self.eat_op(close) {
                        break;
                    }
                    params.push(self.parse_formal_parameter(kind, in_function_type)?);
                    if !self.eat_op(Op::Comma) {
                        self.expect_op(close)?;
                        break;
                    }
                }
                self.expect_op(Op::RParen)?;
                return Ok(params);
            }
            params.push(self.parse_formal_parameter(ParameterKind::Required, in_function_type)?);
            if !self.eat_op(Op::Comma) {
                self.expect_op(Op::RParen)?;
                return Ok(params);
            }
        }
    }

    /// Um parâmetro: metadata, modificadores, tipo opcional, nome (opcional
    /// só em tipo de função), parte de função da forma antiga e default.
    fn parse_formal_parameter(
        &mut self,
        kind: ParameterKind,
        in_function_type: bool,
    ) -> PResult<Parameter> {
        let start = self.span();
        let metadata = self.parse_metadata_opt()?;
        let mut required = false;
        let mut covariant = false;
        let mut final_ = false;
        let mut var_ = false;
        let mut const_ = false;
        // Modificadores em qualquer ordem: rejeitar ordem errada é papel das
        // fases seguintes. `required`/`covariant` são identificadores comuns
        // quando nada declarável os segue (`{required}` é um parâmetro
        // chamado `required`).
        loop {
            if self.eat_kw(Keyword::Final) {
                final_ = true;
            } else if self.eat_kw(Keyword::Var) {
                var_ = true;
            } else if self.eat_kw(Keyword::Const) {
                const_ = true;
            } else if self.at_ident("required") && self.modifier_precedes_declaration() {
                self.advance();
                required = true;
            } else if self.at_ident("covariant") && self.modifier_precedes_declaration() {
                self.advance();
                covariant = true;
            } else {
                break;
            }
        }

        let ty = if self.at_field_formal(self.pos) {
            None
        } else if self.looks_like_type_then_name(self.pos) || in_function_type {
            Some(self.parse_type()?)
        } else {
            None
        };

        let mut this_ = false;
        let mut super_ = false;
        let name = if self.at_field_formal(self.pos) {
            this_ = self.at_kw(Keyword::This);
            super_ = !this_;
            self.advance();
            self.advance();
            Some(self.expect_identifier()?)
        } else if self.at_identifier() {
            Some(self.identifier())
        } else if in_function_type && ty.is_some() {
            None
        } else {
            return Err(self.error("esperava o nome do parâmetro"));
        };

        // Forma antiga de parâmetro-função: `int f<T>(T x)`, `this.f(int x)`.
        let mut function_type_params = Vec::new();
        let mut function_parameters = None;
        if !in_function_type && name.is_some() && (self.at_op(Op::LParen) || self.at_op(Op::Lt)) {
            function_type_params = self.parse_type_parameters_opt()?;
            function_parameters = Some(self.parse_formal_parameters()?);
            // `int f(int x)?` — nulabilidade da forma antiga; sem lugar na árvore.
            self.eat_op(Op::Question);
        }

        let default_value =
            if self.at_op(Op::Assign) || (kind == ParameterKind::Named && self.at_op(Op::Colon)) {
                self.advance();
                Some(self.parse_expression()?)
            } else {
                None
            };

        Ok(Parameter {
            span: self.span_from(start),
            metadata,
            kind,
            required,
            covariant,
            final_,
            var_,
            const_,
            ty,
            this_,
            super_,
            name,
            function_type_params,
            function_parameters,
            default_value,
        })
    }

    /// O token após um possível `required`/`covariant` inicia mesmo um
    /// parâmetro (identificador, `this`, `super`, `final`, `var`, `void`…).
    fn modifier_precedes_declaration(&self) -> bool {
        matches!(self.kind_at(1), Kind::Ident | Kind::Keyword(_))
    }

    /// `this.` ou `super.` em `pos`.
    fn at_field_formal(&self, pos: usize) -> bool {
        matches!(
            self.kind_of(pos),
            Kind::Keyword(Keyword::This) | Kind::Keyword(Keyword::Super)
        ) && self.kind_of(pos + 1) == Kind::Op(Op::Dot)
    }

    /// Variante de [`Parser::looks_like_type_then_identifier`] para listas de
    /// parâmetros: aceita `this.`/`super.` como "nome" e não aplica a
    /// heurística da condicional, porque `{int? a: 1}` é legítimo aqui.
    fn looks_like_type_then_name(&self, pos: usize) -> bool {
        let Some(end) = self.skip_type(pos) else {
            return false;
        };
        self.kind_of(end) == Kind::Ident || self.at_field_formal(end)
    }

    // -- Lookaheads --------------------------------------------------------

    /// Posição após um tipo que começa em `pos`, se houver um sintaticamente.
    ///
    /// Aceita `p.Nome<Args>?`, `void`, caudas `Function<T>(...)` (parênteses
    /// pulados com [`Parser::matching_close`]), records `(...)` com campos
    /// validados como tipos, e `?` final. Conservador em `<`: se os argumentos
    /// não formam uma lista bem formada, o tipo termina antes do `<`
    /// (`a < b` fica como expressão).
    pub(crate) fn skip_type(&self, pos: usize) -> Option<usize> {
        self.skip_type_at(pos, 0)
    }

    fn skip_type_at(&self, pos: usize, depth: u32) -> Option<usize> {
        if depth > MAX_DEPTH {
            return None;
        }
        let mut bare_void = false;
        let mut p = match self.kind_of(pos) {
            Kind::Keyword(Keyword::Void) => {
                bare_void = true;
                pos + 1
            }
            Kind::Op(Op::LParen) => self.skip_record_type(pos, depth)?,
            Kind::Ident if self.at_function_tail(pos) => pos,
            Kind::Ident => {
                if is_builtin_identifier(self.text_of(pos))
                    && self.kind_of(pos + 1) != Kind::Op(Op::Dot)
                {
                    return None;
                }
                let mut p = pos + 1;
                if self.kind_of(p) == Kind::Op(Op::Dot) && self.kind_of(p + 1) == Kind::Ident {
                    p += 2;
                }
                if let Some(after) = self.skip_type_arguments_at(p, depth) {
                    p = after;
                }
                p
            }
            _ => return None,
        };
        loop {
            if self.at_function_tail(p) {
                p = self.skip_function_tail(p, depth)?;
                bare_void = false;
                continue;
            }
            if self.kind_of(p) == Kind::Op(Op::Question)
                && !bare_void
                && self.at_function_tail(p + 1)
            {
                p += 1;
                continue;
            }
            break;
        }
        if self.kind_of(p) == Kind::Op(Op::Question) && !bare_void {
            p += 1;
        }
        Some(p)
    }

    /// Pula `Function<T>(...)` a partir do `Function` em `pos`.
    fn skip_function_tail(&self, pos: usize, depth: u32) -> Option<usize> {
        let mut p = pos + 1;
        if self.kind_of(p) == Kind::Op(Op::Lt) {
            p = self.skip_type_arguments_at(p, depth)?;
        }
        if self.kind_of(p) != Kind::Op(Op::LParen) {
            return None;
        }
        Some(self.matching_close(p)? + 1)
    }

    /// Pula `(campos)` de um record a partir do `(` em `pos`, validando que
    /// cada campo é `metadata tipo nome?` (ou `{metadata tipo nome, ...}`).
    fn skip_record_type(&self, pos: usize, depth: u32) -> Option<usize> {
        let mut p = pos + 1;
        loop {
            match self.kind_of(p) {
                Kind::Op(Op::RParen) => return Some(p + 1),
                Kind::Op(Op::LBrace) => {
                    p += 1;
                    loop {
                        if self.kind_of(p) == Kind::Op(Op::RBrace) {
                            p += 1;
                            break;
                        }
                        p = self.skip_metadata(p, depth);
                        p = self.skip_type_at(p, depth + 1)?;
                        if self.kind_of(p) != Kind::Ident {
                            return None;
                        }
                        p += 1;
                        match self.kind_of(p) {
                            Kind::Op(Op::Comma) => p += 1,
                            Kind::Op(Op::RBrace) => {}
                            _ => return None,
                        }
                    }
                    return if self.kind_of(p) == Kind::Op(Op::RParen) {
                        Some(p + 1)
                    } else {
                        None
                    };
                }
                _ => {}
            }
            p = self.skip_metadata(p, depth);
            p = self.skip_type_at(p, depth + 1)?;
            if self.kind_of(p) == Kind::Ident {
                p += 1;
            }
            match self.kind_of(p) {
                Kind::Op(Op::Comma) => p += 1,
                Kind::Op(Op::RParen) => return Some(p + 1),
                _ => return None,
            }
        }
    }

    /// Pula zero ou mais `@nome.membro<T>(args)` a partir de `pos`. Uma
    /// anotação malformada interrompe e devolve a posição do seu `@`.
    fn skip_metadata(&self, pos: usize, depth: u32) -> usize {
        let mut p = pos;
        while self.kind_of(p) == Kind::Op(Op::At) {
            let mut q = p + 1;
            if self.kind_of(q) != Kind::Ident {
                return p;
            }
            q += 1;
            while self.kind_of(q) == Kind::Op(Op::Dot) && self.kind_of(q + 1) == Kind::Ident {
                q += 2;
            }
            if let Some(after) = self.skip_type_arguments_at(q, depth) {
                q = after;
                if self.kind_of(q) == Kind::Op(Op::Dot) && self.kind_of(q + 1) == Kind::Ident {
                    q += 2;
                }
            }
            if self.kind_of(q) == Kind::Op(Op::LParen) {
                match self.matching_close(q) {
                    Some(close) => q = close + 1,
                    None => return p,
                }
            }
            p = q;
        }
        p
    }

    /// Posição após `<...>` que começa em `pos`, se for uma lista de
    /// argumentos de tipo bem formada.
    ///
    /// Exige `<`, um ou mais tipos separados por vírgula e um `>` de
    /// fechamento; aceita também `extends B` e metadata por item, para servir
    /// como lookahead de listas de parâmetros de tipo. Qualquer outra coisa
    /// (`a < b`, `a < b, c > d` sem `>` no lugar certo) devolve `None`.
    pub(crate) fn skip_type_arguments(&self, pos: usize) -> Option<usize> {
        self.skip_type_arguments_at(pos, 0)
    }

    fn skip_type_arguments_at(&self, pos: usize, depth: u32) -> Option<usize> {
        if depth > MAX_DEPTH || self.kind_of(pos) != Kind::Op(Op::Lt) {
            return None;
        }
        let mut p = pos + 1;
        loop {
            p = self.skip_metadata(p, depth + 1);
            p = self.skip_type_at(p, depth + 1)?;
            if self.kind_of(p) == Kind::Keyword(Keyword::Extends) {
                p = self.skip_type_at(p + 1, depth + 1)?;
            }
            match self.kind_of(p) {
                Kind::Op(Op::Comma) => p += 1,
                Kind::Op(Op::Gt) => return Some(p + 1),
                _ => return None,
            }
        }
    }

    /// Em `pos` começa `tipo identificador` (declaração de variável, parâmetro
    /// ou função)? Não consome nada.
    ///
    /// Segue `computeType(required: false)` do SDK: o tipo precisa ser
    /// seguido de um identificador. Quando o tipo termina em `?`, aplica
    /// também a regra de `parseExpressionStatementOrDeclarationAfterModifiers`
    /// para não ler `a ? b : c` como declaração: depois do identificador tem
    /// de vir `=`, `;`, `,`, `)`, uma palavra reservada, outro identificador,
    /// o fim do arquivo, ou `<T>(...)` seguido de corpo de função
    /// (`int? f() {}`). O SDK ainda desfaz `a ? b = c : d` analisando a
    /// expressão; aqui `=` conta como declaração.
    pub(crate) fn looks_like_type_then_identifier(&self, pos: usize) -> bool {
        let Some(end) = self.skip_type(pos) else {
            return false;
        };
        if self.kind_of(end) != Kind::Ident {
            return false;
        }
        // `T operator +(...)` é operador; `final O operator;` usa `operator`
        // como nome comum (é identificador embutido, não reservado).
        if self.text_of(end) == "operator" && self.operator_follows_at(end) {
            return true;
        }
        if self.kind_of(end - 1) == Kind::Op(Op::Question) {
            return self.declaration_may_continue_at(end + 1);
        }
        true
    }

    /// O token em `pos` é `operator` seguido de um operador declarável?
    pub(crate) fn operator_follows_at(&self, pos: usize) -> bool {
        match self.kind_of(pos + 1) {
            Kind::Op(op) => match op {
                Op::Tilde
                | Op::LtEq
                | Op::LtLt
                | Op::Plus
                | Op::Minus
                | Op::Star
                | Op::Slash
                | Op::TildeSlash
                | Op::Percent
                | Op::Pipe
                | Op::Caret
                | Op::Amp
                | Op::EqEq
                | Op::Gt => true,
                Op::LBracket => self.kind_of(pos + 2) == Kind::Op(Op::RBracket),
                Op::Lt => self.kind_of(pos + 2) == Kind::Op(Op::LParen),
                _ => false,
            },
            _ => false,
        }
    }

    /// Depois de `T? nome`, o token em `pos` é compatível com uma declaração
    /// (e não com o ramo de uma condicional).
    fn declaration_may_continue_at(&self, pos: usize) -> bool {
        match self.kind_of(pos) {
            Kind::Ident | Kind::Keyword(_) | Kind::Eof => true,
            Kind::Op(Op::Assign | Op::Semicolon | Op::Comma | Op::RParen) => true,
            Kind::Op(Op::LParen | Op::Lt) => self.looks_like_function_rest(pos),
            _ => false,
        }
    }

    /// `<T>(...)` ou `(...)` em `pos` seguido de `{`, `=>`, `async`, `sync`,
    /// `;` (métodos abstratos/externos) ou `native` — o resto de uma declaração
    /// de função, não uma chamada.
    pub(crate) fn looks_like_function_rest(&self, pos: usize) -> bool {
        let mut p = pos;
        if self.kind_of(p) == Kind::Op(Op::Lt) {
            let Some(after) = self.skip_type_arguments(p) else {
                return false;
            };
            p = after;
        }
        if self.kind_of(p) != Kind::Op(Op::LParen) {
            return false;
        }
        let Some(close) = self.matching_close(p) else {
            return false;
        };
        match self.kind_of(close + 1) {
            Kind::Op(Op::LBrace | Op::Arrow | Op::Semicolon) => true,
            Kind::Ident => matches!(self.text_of(close + 1), "async" | "sync" | "native"),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Ast, Name, Parameter, TypeKind};
    use crate::lexer::lex;
    use dartforge_intern::Interner;

    /// Constrói um parser sobre `src` e roda `f` com ele.
    fn with_parser<T>(src: &str, f: impl FnOnce(&mut Parser<'_, '_>) -> T) -> T {
        let mut nomes = Interner::new();
        let tokens = lex(src).unwrap();
        let mut p = Parser::new(src, tokens, &mut nomes);
        f(&mut p)
    }

    fn name_text(src: &str, name: &Name) -> String {
        src[name.span.start..name.span.end].to_string()
    }

    /// Reescreve um tipo numa forma canônica, para comparar textualmente.
    fn render_type(src: &str, ast: &Ast, id: TypeId) -> String {
        let ty = ast.ty(id);
        let mut out = match &ty.kind {
            TypeKind::Void => "void".to_string(),
            TypeKind::Named { name, args } => {
                let mut s = name
                    .iter()
                    .map(|n| name_text(src, n))
                    .collect::<Vec<_>>()
                    .join(".");
                if !args.is_empty() {
                    s.push('<');
                    s.push_str(&render_types(src, ast, args));
                    s.push('>');
                }
                s
            }
            TypeKind::Function {
                return_type,
                type_params,
                parameters,
            } => {
                let mut s = String::new();
                if let Some(r) = return_type {
                    s.push_str(&render_type(src, ast, *r));
                    s.push(' ');
                }
                s.push_str("Function");
                s.push_str(&render_type_params(src, ast, type_params));
                s.push_str(&render_params(src, ast, parameters));
                s
            }
            TypeKind::Record { positional, named } => {
                let mut items: Vec<String> = positional
                    .iter()
                    .map(|t| render_type(src, ast, *t))
                    .collect();
                if !named.is_empty() {
                    let inner: Vec<String> = named
                        .iter()
                        .map(|(n, t)| {
                            format!("{} {}", render_type(src, ast, *t), name_text(src, n))
                        })
                        .collect();
                    items.push(format!("{{{}}}", inner.join(", ")));
                }
                format!("({})", items.join(", "))
            }
        };
        if ty.nullable {
            out.push('?');
        }
        out
    }

    fn render_types(src: &str, ast: &Ast, ids: &[TypeId]) -> String {
        ids.iter()
            .map(|t| render_type(src, ast, *t))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn render_type_params(src: &str, ast: &Ast, params: &[TypeParameter]) -> String {
        if params.is_empty() {
            return String::new();
        }
        let items: Vec<String> = params
            .iter()
            .map(|p| {
                let mut s = String::new();
                for m in &p.metadata {
                    s.push_str(&src[m.span.start..m.span.end]);
                    s.push(' ');
                }
                s.push_str(&name_text(src, &p.name));
                if let Some(b) = p.bound {
                    s.push_str(" extends ");
                    s.push_str(&render_type(src, ast, b));
                }
                s
            })
            .collect();
        format!("<{}>", items.join(", "))
    }

    fn render_param(src: &str, ast: &Ast, p: &Parameter) -> String {
        let mut s = String::new();
        for m in &p.metadata {
            s.push_str(&src[m.span.start..m.span.end]);
            s.push(' ');
        }
        if p.required {
            s.push_str("required ");
        }
        if p.covariant {
            s.push_str("covariant ");
        }
        if p.final_ {
            s.push_str("final ");
        }
        if p.var_ {
            s.push_str("var ");
        }
        if p.const_ {
            s.push_str("const ");
        }
        if let Some(t) = p.ty {
            s.push_str(&render_type(src, ast, t));
            if p.name.is_some() {
                s.push(' ');
            }
        }
        if p.this_ {
            s.push_str("this.");
        }
        if p.super_ {
            s.push_str("super.");
        }
        if let Some(n) = &p.name {
            s.push_str(&name_text(src, n));
        }
        s.push_str(&render_type_params(src, ast, &p.function_type_params));
        if let Some(fp) = &p.function_parameters {
            s.push_str(&render_params(src, ast, fp));
        }
        if let Some(d) = p.default_value {
            let e = ast.expr(d);
            s.push_str(" = ");
            s.push_str(&src[e.span.start..e.span.end]);
        }
        s
    }

    fn render_params(src: &str, ast: &Ast, params: &[Parameter]) -> String {
        let mut s = String::from("(");
        let mut current = ParameterKind::Required;
        let mut first = true;
        for p in params {
            if p.kind != current {
                if !first {
                    s.push_str(", ");
                }
                s.push(if p.kind == ParameterKind::Optional {
                    '['
                } else {
                    '{'
                });
                current = p.kind;
                first = true;
            }
            if !first {
                s.push_str(", ");
            }
            first = false;
            s.push_str(&render_param(src, ast, p));
        }
        if current == ParameterKind::Optional {
            s.push(']');
        } else if current == ParameterKind::Named {
            s.push('}');
        }
        s.push(')');
        s
    }

    /// Lê `src` inteiro como um tipo e devolve a forma canônica.
    fn ty(src: &str) -> String {
        with_parser(src, |p| {
            let id = p.parse_type().unwrap();
            assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
            assert!(p.at_eof(), "sobrou '{}'", p.text());
            assert_eq!(p.ast.ty(id).span.start, 0);
            assert_eq!(p.ast.ty(id).span.end, src.len());
            render_type(src, &p.ast, id)
        })
    }

    /// Lê `src` inteiro como lista de parâmetros formais (modo declaração).
    fn params(src: &str) -> String {
        with_parser(src, |p| {
            let list = p.parse_formal_parameters().unwrap();
            assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
            assert!(p.at_eof(), "sobrou '{}'", p.text());
            render_params(src, &p.ast, &list)
        })
    }

    fn type_fails(src: &str) {
        with_parser(src, |p| {
            let r = p.parse_type();
            assert!(r.is_err() || !p.at_eof(), "aceitou '{src}' inteiro");
        })
    }

    // -- parse_type -------------------------------------------------------

    #[test]
    fn nomes_simples_e_qualificados() {
        assert_eq!(ty("int"), "int");
        assert_eq!(ty("dynamic"), "dynamic");
        assert_eq!(ty("Never"), "Never");
        assert_eq!(ty("Function"), "Function");
        assert_eq!(ty("Function?"), "Function?");
        assert_eq!(ty("p.Nome"), "p.Nome");
        assert_eq!(ty("core.List<int>"), "core.List<int>");
        assert_eq!(ty("void"), "void");
        assert_eq!(ty("int?"), "int?");
        assert_eq!(ty("p.Nome?"), "p.Nome?");
    }

    #[test]
    fn nomes_com_argumentos_aninhados() {
        assert_eq!(ty("Map<String, List<int>>?"), "Map<String, List<int>>?");
        assert_eq!(ty("List<List<List<int>>>"), "List<List<List<int>>>");
        assert_eq!(ty("Map<K, V?>"), "Map<K, V?>");
        assert_eq!(ty("List<void>"), "List<void>");
        assert_eq!(ty("List<dynamic>"), "List<dynamic>");
        with_parser("List<int>", |p| {
            let id = p.parse_type().unwrap();
            let TypeKind::Named { name, args } = &p.ast.ty(id).kind else {
                panic!()
            };
            assert_eq!(name.len(), 1);
            assert_eq!(args.len(), 1);
            assert_eq!(p.in_type_args, 0);
        });
    }

    #[test]
    fn tipos_de_funcao() {
        assert_eq!(ty("int Function(int)?"), "int Function(int)?");
        assert_eq!(ty("void Function<T>(T)"), "void Function<T>(T)");
        assert_eq!(ty("Function()"), "Function()");
        assert_eq!(ty("Function<T>(T x)"), "Function<T>(T x)");
        assert_eq!(ty("int Function(int, String)"), "int Function(int, String)");
        assert_eq!(
            ty("int Function(int a, String b)"),
            "int Function(int a, String b)"
        );
        assert_eq!(ty("int Function([int a])"), "int Function([int a])");
        assert_eq!(ty("int Function([int])"), "int Function([int])");
        assert_eq!(
            ty("int Function({required int a, String? b})"),
            "int Function({required int a, String? b})"
        );
        assert_eq!(
            ty("int Function(int, [String])"),
            "int Function(int, [String])"
        );
        assert_eq!(
            ty("T Function<T extends num>(T)"),
            "T Function<T extends num>(T)"
        );
        assert_eq!(ty("int? Function()"), "int? Function()");
        assert_eq!(ty("void Function()?"), "void Function()?");
        assert_eq!(ty("int Function(int,)"), "int Function(int)");
        assert_eq!(
            ty("void Function(void Function() cb)"),
            "void Function(void Function() cb)"
        );
    }

    #[test]
    fn tipos_de_funcao_encadeados() {
        // O `Function` mais à direita é o tipo externo.
        with_parser("int Function(int) Function(String)", |p| {
            let id = p.parse_type().unwrap();
            assert!(p.diagnostics.is_empty());
            let TypeKind::Function {
                return_type: Some(inner),
                parameters,
                ..
            } = &p.ast.ty(id).kind
            else {
                panic!()
            };
            assert_eq!(parameters.len(), 1);
            assert_eq!(
                render_type("int Function(int) Function(String)", &p.ast, *inner),
                "int Function(int)"
            );
            assert_eq!(
                render_type(
                    "int Function(int) Function(String)",
                    &p.ast,
                    parameters[0].ty.unwrap()
                ),
                "String"
            );
        });
        assert_eq!(
            ty("Future<void> Function() Function()"),
            "Future<void> Function() Function()"
        );
        assert_eq!(
            ty("int Function()? Function()"),
            "int Function()? Function()"
        );
        assert_eq!(
            ty("int Function()? Function()?"),
            "int Function()? Function()?"
        );
    }

    #[test]
    fn records() {
        assert_eq!(ty("(int, String)"), "(int, String)");
        assert_eq!(ty("(int a, {String b})"), "(int, {String b})");
        assert_eq!(ty("({int a})"), "({int a})");
        assert_eq!(ty("({int a, String b})"), "({int a, String b})");
        assert_eq!(ty("()"), "()");
        assert_eq!(ty("(int,)"), "(int)");
        assert_eq!(ty("(int, int)?"), "(int, int)?");
        assert_eq!(ty("(int, {int a})"), "(int, {int a})");
        assert_eq!(ty("List<(int, String)>"), "List<(int, String)>");
        assert_eq!(ty("(int, String) Function()"), "(int, String) Function()");
        assert_eq!(ty("(int, String)? Function()"), "(int, String)? Function()");
        assert_eq!(
            ty("(int Function(int), {void Function() f})"),
            "(int Function(int), {void Function() f})"
        );
        assert_eq!(
            ty("((int, int), List<(int,)>)"),
            "((int, int), List<(int)>)"
        );
    }

    #[test]
    fn tipo_invalido() {
        type_fails("");
        type_fails("123");
        type_fails("List<int");
        type_fails("(int");
        type_fails("int Function");
    }

    #[test]
    fn interrogacao_apos_is_ou_as() {
        // `x is int ? a : b`: o `?` fica para a condicional.
        with_parser("int ? a : b", |p| {
            let id = p.parse_type_after_is_or_as().unwrap();
            assert!(!p.ast.ty(id).nullable);
            assert!(p.at_op(Op::Question));
        });
        // `(x is int?)`, `x is int?;`, `x is int? ? a : b`: o `?` é do tipo.
        for src in [
            "int?)",
            "int?;",
            "int? ? a : b",
            "int?",
            "List<int>?,",
            "int? as",
        ] {
            with_parser(src, |p| {
                let id = p.parse_type_after_is_or_as().unwrap();
                assert!(p.ast.ty(id).nullable, "{src}");
            });
        }
        // `x is int? && y`: no SDK `&&` fecha o tipo (não inicia expressão).
        with_parser("int? && y", |p| {
            let id = p.parse_type_after_is_or_as().unwrap();
            assert!(p.ast.ty(id).nullable);
        });
        // `x is T ? a : b` com `a` iniciando por `{` ou `when`: condicional.
        for src in ["int ? {} : b", "int ? when : b"] {
            with_parser(src, |p| {
                let id = p.parse_type_after_is_or_as().unwrap();
                assert!(!p.ast.ty(id).nullable, "{src}");
            });
        }
        // `{` que não forma condicional fecha o tipo (corpo de construtor).
        with_parser("int? { }", |p| {
            let id = p.parse_type_after_is_or_as().unwrap();
            assert!(p.ast.ty(id).nullable);
        });
        // Em contexto de tipo puro o `?` sempre entra.
        with_parser("int ? a : b", |p| {
            let id = p.parse_type().unwrap();
            assert!(p.ast.ty(id).nullable);
        });
    }

    #[test]
    fn profundidade_limitada() {
        let src = format!("{}int{}", "List<".repeat(600), ">".repeat(600));
        with_parser(&src, |p| {
            assert!(p.parse_type().is_err());
            assert!(!p.diagnostics.is_empty());
            assert_eq!(p.depth, 0);
            assert_eq!(p.in_type_args, 0);
        });
    }

    // -- parâmetros de tipo -------------------------------------------------

    #[test]
    fn parametros_de_tipo() {
        with_parser("<T extends Bound, U, V extends Map<K, V>>", |p| {
            let list = p.parse_type_parameters_opt().unwrap();
            assert!(p.diagnostics.is_empty());
            assert!(p.at_eof());
            assert_eq!(
                render_type_params("<T extends Bound, U, V extends Map<K, V>>", &p.ast, &list),
                "<T extends Bound, U, V extends Map<K, V>>"
            );
            assert_eq!(p.in_type_args, 0);
        });
        with_parser("x", |p| {
            assert!(p.parse_type_parameters_opt().unwrap().is_empty());
            assert!(p.parse_type_arguments_opt().unwrap().is_empty());
            assert_eq!(p.pos, 0);
        });
        with_parser("<T extends>", |p| {
            assert!(p.parse_type_parameters_opt().is_err());
            assert_eq!(p.in_type_args, 0);
        });
    }

    // -- parâmetros formais -------------------------------------------------

    #[test]
    fn parametros_posicionais() {
        assert_eq!(params("()"), "()");
        assert_eq!(params("(int x)"), "(int x)");
        assert_eq!(params("(x)"), "(x)");
        assert_eq!(params("(x, y)"), "(x, y)");
        assert_eq!(params("(int x,)"), "(int x)");
        assert_eq!(params("(a.b x)"), "(a.b x)");
        assert_eq!(params("(List<int> x)"), "(List<int> x)");
        assert_eq!(
            params("(Map<String, List<int>>? x)"),
            "(Map<String, List<int>>? x)"
        );
        assert_eq!(
            params("(final x, var y, const z)"),
            "(final x, var y, const z)"
        );
        assert_eq!(params("(final int x)"), "(final int x)");
        assert_eq!(params("(void Function() cb)"), "(void Function() cb)");
        assert_eq!(params("(int Function(int)? cb)"), "(int Function(int)? cb)");
        assert_eq!(params("(void Function<T>(T) g)"), "(void Function<T>(T) g)");
        assert_eq!(
            params("(Future<void> Function() Function() h)"),
            "(Future<void> Function() Function() h)"
        );
        assert_eq!(params("(({int a, String b}) r)"), "(({int a, String b}) r)");
        assert_eq!(params("((int, {int a}) r2)"), "((int, {int a}) r2)");
        assert_eq!(params("(List<(int, String)> l)"), "(List<(int, String)> l)");
        assert_eq!(
            params("(dynamic x, Function f, Object? o)"),
            "(dynamic x, Function f, Object? o)"
        );
    }

    #[test]
    fn parametros_this_super_covariant() {
        assert_eq!(params("(this.x)"), "(this.x)");
        assert_eq!(params("(int this.x)"), "(int this.x)");
        assert_eq!(params("(super.key)"), "(super.key)");
        assert_eq!(params("(final super.key)"), "(final super.key)");
        assert_eq!(params("(covariant int y)"), "(covariant int y)");
        assert_eq!(params("(covariant x)"), "(covariant x)");
        assert_eq!(params("(covariant)"), "(covariant)");
        assert_eq!(params("(int required)"), "(int required)");
        assert_eq!(params("({required})"), "({required})");
        assert_eq!(
            params("({required this.x, super.key})"),
            "({required this.x, super.key})"
        );
        assert_eq!(params("({required int x})"), "({required int x})");
        assert_eq!(
            params("({required final int x})"),
            "({required final int x})"
        );
        assert_eq!(params("({covariant int y})"), "({covariant int y})");
    }

    #[test]
    fn parametros_opcionais_e_nomeados_sem_default() {
        assert_eq!(params("([int a])"), "([int a])");
        assert_eq!(params("([a, b])"), "([a, b])");
        assert_eq!(params("(int x, [int? y])"), "(int x, [int? y])");
        assert_eq!(params("(int x, {int? y})"), "(int x, {int? y})");
        assert_eq!(params("({int? a, String? b,})"), "({int? a, String? b})");
        assert_eq!(params("(x, [y,])"), "(x, [y])");
        with_parser("(int x, [int y], {int z})", |p| {
            assert!(p.parse_formal_parameters().is_err());
        });
    }

    #[test]
    fn forma_antiga_de_parametro_funcao() {
        assert_eq!(
            params("(void f(int callback(int x, [String y])))"),
            "(void f(int callback(int x, [String y])))"
        );
        assert_eq!(params("(f(int x))"), "(f(int x))");
        assert_eq!(params("(int f(int x))"), "(int f(int x))");
        assert_eq!(params("(this.f(int x))"), "(this.f(int x))");
        assert_eq!(params("(f<T>(T x))"), "(f<T>(T x))");
        assert_eq!(params("(int f(int x)?)"), "(int f(int x))");
        assert_eq!(params("(Function f())"), "(Function f())");
        assert_eq!(params("(Function() f)"), "(Function() f)");
    }

    // Dependem de `parse_expression` (expressions.rs) e de `parse_metadata`
    // (declarations.rs); panicam com `not yet implemented` enquanto forem stubs.
    #[test]
    fn parametros_com_default_e_metadata() {
        assert_eq!(params("({int z = 3})"), "({int z = 3})");
        assert_eq!(params("({int z: 3})"), "({int z = 3})");
        assert_eq!(params("([int a = 1, b])"), "([int a = 1, b])");
        assert_eq!(
            params("([int a = 1, b = a + 1])"),
            "([int a = 1, b = a + 1])"
        );
        assert_eq!(
            params(
                "({required this.x, super.key, covariant int y, int z = 3, @Deprecated('x') int w})"
            ),
            "({required this.x, super.key, covariant int y, int z = 3, @Deprecated('x') int w})"
        );
        assert_eq!(
            params("(@pragma('dart2js:noInline') int x)"),
            "(@pragma('dart2js:noInline') int x)"
        );
        assert_eq!(
            ty("void Function(@covariant int x)"),
            "void Function(@covariant int x)"
        );
        with_parser("<@meta T extends Bound>", |p| {
            let list = p.parse_type_parameters_opt().unwrap();
            assert_eq!(list.len(), 1);
            assert_eq!(list[0].metadata.len(), 1);
        });
    }

    #[test]
    fn parametros_de_tipo_de_funcao_sem_nome() {
        with_parser("(int, String name, [int])", |p| {
            let list = p.parse_function_type_parameters().unwrap();
            assert!(p.diagnostics.is_empty());
            assert!(p.at_eof());
            assert_eq!(list.len(), 3);
            assert!(list[0].ty.is_some() && list[0].name.is_none());
            assert!(list[1].ty.is_some() && list[1].name.is_some());
            assert_eq!(list[2].kind, ParameterKind::Optional);
            assert!(list[2].name.is_none());
        });
        with_parser("({required int a, int? b})", |p| {
            let list = p.parse_function_type_parameters().unwrap();
            assert!(p.diagnostics.is_empty());
            assert!(p.at_eof());
            assert_eq!(list.len(), 2);
            assert_eq!(list[0].kind, ParameterKind::Named);
            assert!(list[0].required && list[0].name.is_some());
            assert!(!list[1].required);
        });
        with_parser("(int x, [int y], {int z})", |p| {
            assert!(p.parse_function_type_parameters().is_err());
        });
    }

    // -- lookaheads ---------------------------------------------------------

    fn skip(src: &str) -> Option<usize> {
        with_parser(src, |p| p.skip_type(0))
    }

    fn skip_args(src: &str) -> Option<usize> {
        with_parser(src, |p| p.skip_type_arguments(0))
    }

    fn looks(src: &str) -> bool {
        with_parser(src, |p| p.looks_like_type_then_identifier(0))
    }

    #[test]
    fn skip_type_aceita_todas_as_formas() {
        assert_eq!(skip("int x"), Some(1));
        assert_eq!(skip("int? x"), Some(2));
        assert_eq!(skip("p.Nome<int> x"), Some(6));
        assert_eq!(skip("void x"), Some(1));
        assert_eq!(skip("void? x"), Some(1));
        assert_eq!(skip("void Function() x"), Some(4));
        assert_eq!(skip("void Function()? x"), Some(5));
        assert_eq!(skip("Function<T>(T) x"), Some(7));
        assert_eq!(skip("int Function(int) Function(String)? x"), Some(10));
        assert_eq!(skip("int Function()? Function() x"), Some(8));
        assert_eq!(skip("(int, String) x"), Some(5));
        assert_eq!(skip("(int a, {String b})? x"), Some(10));
        assert_eq!(skip("() x"), Some(2));
        assert_eq!(skip("(int,) x"), Some(4));
        assert_eq!(skip("(int, int) Function() x"), Some(8));
        assert_eq!(skip("List<(int, int)> x"), Some(8));
        assert_eq!(skip("dynamic x"), Some(1));
        assert_eq!(skip("Function x"), Some(1));
        assert_eq!(skip("Function? x"), Some(2));
        assert_eq!(skip("p.get x"), Some(3));
    }

    #[test]
    fn skip_type_e_conservador() {
        assert_eq!(skip("123"), None);
        assert_eq!(skip("(1, 2)"), None);
        assert_eq!(skip("(a + b)"), None);
        assert_eq!(skip("get x"), None);
        assert_eq!(skip("required x"), None);
        assert_eq!(skip("Function("), None);
        assert_eq!(skip("List<int"), Some(1));
        assert_eq!(skip("a < b"), Some(1));
        assert_eq!(skip("a.b.c x"), Some(3));
    }

    #[test]
    fn skip_type_arguments_exige_lista_bem_formada() {
        assert_eq!(skip_args("<T, U<V>>"), Some(8));
        assert_eq!(skip_args("<int>"), Some(3));
        assert_eq!(skip_args("<int?>"), Some(4));
        assert_eq!(skip_args("<void>"), Some(3));
        assert_eq!(skip_args("<T extends num, U>"), Some(7));
        assert_eq!(skip_args("<(int, int)>"), Some(7));
        assert_eq!(skip_args("<int Function(int)>"), Some(7));
        assert_eq!(skip_args("< b"), None);
        assert_eq!(skip_args("< b, c > d"), Some(5));
        assert_eq!(skip_args("<>"), None);
        assert_eq!(skip_args("<1>"), None);
        assert_eq!(skip_args("<a + b>"), None);
        assert_eq!(skip_args("<T,>"), None);
        assert_eq!(skip_args("x"), None);
    }

    #[test]
    fn looks_like_type_then_identifier_desambigua() {
        assert!(looks("int x"));
        assert!(looks("int x;"));
        assert!(looks("List<int> x = []"));
        assert!(looks("Map<String, List<int>>? x"));
        assert!(looks("int? x"));
        assert!(looks("int? x;"));
        assert!(looks("int? x = 1;"));
        assert!(looks("int? x, y;"));
        assert!(looks("int? x in xs"));
        assert!(looks("int? f() {}"));
        assert!(looks("int? f() => 1;"));
        assert!(looks("int? f<T>(T x) async {}"));
        assert!(looks("int? get x => 1;"));
        assert!(looks("void f()"));
        assert!(looks("(int, int) r = (1, 2)"));
        assert!(looks("int Function(int)? f"));
        assert!(looks("a.b x"));
        assert!(looks("dynamic x"));

        assert!(!looks("a < b"));
        assert!(!looks("a < b;"));
        assert!(!looks("a ? b : c"));
        assert!(!looks("a ? b() : c"));
        assert!(!looks("a ? b<c>(d) : e"));
        assert!(!looks("a ? b.c : d"));
        assert!(!looks("a ? b[0] : c"));
        assert!(!looks("(a) ? b : c"));
        assert!(!looks("a.b.c"));
        assert!(!looks("x = 1"));
        assert!(!looks("f(x)"));
        assert!(!looks("get x"));
        assert!(!looks("123 x"));
        assert!(!looks("x"));
        assert!(!looks(""));
    }
}
