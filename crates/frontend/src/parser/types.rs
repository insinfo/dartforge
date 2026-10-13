//! Types, type parameters, type arguments and formal parameters.
//!
//! Covers the `type`, `typeParameters`, `typeArguments`,
//! `formalParameterList` and `parameterTypeList` sections of the Dart 3.6
//! grammar, plus the *lookaheads* the rest of the parser uses to decide
//! whether a token sequence is a type without consuming anything
//! (`Parser::skip_type`, `Parser::skip_type_arguments`,
//! `Parser::looks_like_type_then_identifier`).
//!
//! Decisions that are not a direct translation of the grammar:
//!
//! * **Chained function types** (`int Function(int) Function(String)`)
//!   associate to the left: the rightmost `Function` is the outer node and
//!   the left one is its return type. A `?` between two tails
//!   (`int Function()? Function()`) makes the left tail nullable.
//! * **Records** can be a function's return type (`(int, int) Function()`),
//!   as in the SDK parser (`computeRecordTypeGFT`).
//! * **`?` after `is`/`as`**: `x is int ? a : b` is a conditional, not a
//!   nullable type. `Parser::parse_type_after_is_or_as` reproduces the SDK's
//!   `computeTypeAfterIsOrAs` rule: the trailing `?` only belongs to the type
//!   if the next token cannot start a conditional's branch.
//! * **Built-in identifiers** (`get`, `set`, `static`, `typedef`…) do not
//!   count as a type name in the *lookaheads*, unless qualified (`p.get`):
//!   it is the SDK's `isValidNonRecordTypeReference` rule, which avoids
//!   reading `get x` as "type `get`, name `x`". `dynamic` and `Function` are
//!   regular type names.
//! * **One parameter list, two modes.** The same routine reads declaration
//!   parameter lists (`this.x`, `super.x`, defaults, the old form
//!   `int f(int x)`) and function type ones (`Function(int, String)`, where
//!   the name is optional). The mode is an internal boolean; publicly
//!   `Parser::parse_formal_parameters` is the declaration mode and
//!   `Parser::parse_function_type_parameters` the function type mode.
//! * `>` always arrives isolated from the lexer, so the `<` … `>` of types
//!   never needs backtracking; `in_type_args` is incremented while reading so
//!   that expressions inside metadata do not compose `>>`.
use super::modifiers::{ParameterOwner, ModifierTokens};
use super::{MAX_DEPTH, PResult, Parser};
use crate::ast::{
    Annotation, Name, Parameter, ParameterKind, TypeAnnotation, TypeId, TypeKind, TypeParameter,
    Variance,
};
use crate::features::Feature;
use crate::token::{Keyword, Kind, Op};
use dartforge_diagnostics::{Diagnostic, Span, codigos};

/// How to treat a trailing `?` when reading a type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrailingQuestion {
    /// Pure type context: a trailing `?` is always nullability.
    Always,
    /// After `is`/`as` in an expression: `?` only belongs to the type if the
    /// next token cannot start a conditional's `then` branch.
    AfterIsOrAs,
}

/// Built-in identifiers (§17.1) that cannot serve as a type name without
/// qualification. `dynamic` and `Function` are left out because they are
/// types.
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
    // -- Types -----------------------------------------------------------------

    /// `type`: a qualified name with arguments, `void`, a function or record
    /// type, with a trailing `?`.
    pub(crate) fn parse_type(&mut self) -> PResult<TypeId> {
        self.parse_type_with(TrailingQuestion::Always)
    }

    /// Type after `is`/`as` in an expression: like [`Parser::parse_type`], but a
    /// trailing `?` is only consumed when the next token is one of those that
    /// may follow a nullable type in that place (`)`, `,`, `;`, `:`, `?`, `??`,
    /// `..`, `}`, `]`, `is`, `as` or end of file). That way `x is int ? a : b`
    /// leaves the `?` to the conditional, as in the SDK.
    pub(crate) fn parse_type_after_is_or_as(&mut self) -> PResult<TypeId> {
        self.parse_type_with(TrailingQuestion::AfterIsOrAs)
    }

    fn parse_type_with(&mut self, mode: TrailingQuestion) -> PResult<TypeId> {
        self.enter()?;
        let result = self.parse_type_inner(mode);
        self.leave();
        result
    }

    /// Reads the base type and then zero or more `Function<T>(...)` tails,
    /// applying `?` where the grammar allows it.
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
                    // `int? Function()` or `int Function()? Function()`.
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

    /// `void`, a record `(...)` or a qualified name with arguments — without
    /// `?` and without a `Function` tail.
    fn parse_non_function_type(&mut self) -> PResult<TypeId> {
        let start = self.span();
        if self.eat_kw(Keyword::Void) {
            let span = self.span_from(start);
            // `VoidType.parseType` (`type_info_impl.dart:446`): type arguments after
            // `void` are `VOID_WITH_TYPE_ARGUMENTS` at the `<` and are read.
            if self.at_op(Op::Lt) && self.skip_type_arguments(self.pos).is_some() {
                self.erro(codigos::parser::VOID_WITH_TYPE_ARGUMENTS, &[]);
                self.parse_type_arguments_opt()?;
            }
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
                kind: TypeKind::Named {
                    name: name.into_boxed_slice(),
                    args: args.into_boxed_slice(),
                },
            }));
        }
        Err(self.erro(codigos::parser::EXPECTED_TYPE_NAME, &[]))
    }

    /// `Function<T>(params)` with the return type already read (or none).
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
                type_params: type_params.into_boxed_slice(),
                parameters: parameters.into_boxed_slice(),
            },
        }))
    }

    /// `recordType`: `()`, `(T, U)`, `(T a, {U b})`, `({U b})`. The name of a
    /// positional field and the fields' metadata are accepted and discarded —
    /// the tree does not keep them.
    fn parse_record_type(&mut self) -> PResult<TypeId> {
        let start = self.span();
        self.expect_op(Op::LParen)?;
        let mut positional = Vec::new();
        let mut named = Vec::new();
        let mut saw_comma = false;
        let mut with_named = false;
        // `(,)`: `EMPTY_RECORD_TYPE_WITH_COMMA` at the comma
        // (`parser_impl.dart:1673-1676`).
        if self.at_op(Op::Comma) && self.at_op_at(1, Op::RParen) {
            let comma = self.advance().span;
            self.error_at(codigos::parser::EMPTY_RECORD_TYPE_WITH_COMMA, comma, &[]);
            saw_comma = true;
        }
        loop {
            if self.eat_op(Op::RParen) {
                break;
            }
            if self.eat_op(Op::LBrace) {
                with_named = true;
                let mut fields_ = 0usize;
                loop {
                    if self.at_op(Op::RBrace) {
                        // `parseRecordTypeNamedFields` (`:1743-1745`): an empty group, at the `}`.
                        if fields_ == 0 {
                            let closes = self.span();
                            self.error_at(codigos::parser::EMPTY_RECORD_TYPE_NAMED_FIELDS_LIST, closes, &[]);
                        }
                        self.advance();
                        break;
                    }
                    fields_ += 1;
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
            if self.eat_op(Op::Comma) {
                saw_comma = true;
            } else {
                self.expect_op(Op::RParen)?;
                break;
            }
        }
        // `(int)`: one positional, without named fields and without a comma, at the
        // `)` (`parser_impl.dart:1677-1680`).
        if positional.len() == 1 && !with_named && !saw_comma {
            let closes = self.tokens[self.pos - 1].span;
            self.error_at(codigos::parser::RECORD_TYPE_ONE_POSITIONAL_NO_TRAILING_COMMA, closes, &[]);
        }
        let span = self.span_from(start);
        Ok(self.ast.push_type(TypeAnnotation {
            span,
            nullable: false,
            kind: TypeKind::Record {
                positional: positional.into_boxed_slice(),
                named: named.into_boxed_slice(),
            },
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

    /// The current token is `Function` followed by `<` or `(` — the start of a
    /// function type tail (the SDK's `isGeneralizedFunctionType`).
    pub(crate) fn at_function_tail(&self, pos: usize) -> bool {
        self.kind_of(pos) == Kind::Ident
            && self.text_of(pos) == "Function"
            && matches!(
                self.kind_of(pos + 1),
                Kind::Op(Op::Lt) | Kind::Op(Op::LParen)
            )
    }

    /// After `is`/`as`, the token at `pos` (the one after the `?`) lets the `?`
    /// be part of the type. The list of `computeTypeAfterIsOrAs` in the SDK.
    /// After `is T` / `as T`, the `?` is a suffix of the type only when the next
    /// token cannot start a conditional's true branch. It is the list of the
    /// SDK parser's `computeTypeAfterIsOrAs` (`parser_impl.dart`): closers,
    /// separators, `is`/`as`, `..`, `||`, `&&` and end. `{` and `when` are
    /// ambiguous (`x is T ? {} : y` versus the last initializer of a
    /// constructor with a body) and the SDK resolves them by trying: it is a
    /// conditional if `? expr : expr` parses from there.
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

    /// Does `expr : expr` parse from `pos` (the token after the `?`)? Undoes
    /// everything it tried.
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

    /// Metadata if there is an `@`; otherwise an empty list without touching
    /// anything.
    fn parse_metadata_opt(&mut self) -> PResult<Vec<Annotation>> {
        if self.at_op(Op::At) {
            self.parse_metadata()
        } else {
            Ok(Vec::new())
        }
    }

    // -- Type arguments and parameters -----------------------------------------

    /// `<T, U>` arguments, or an empty list if there is no `<`.
    ///
    /// In type context `<` is always an argument; whoever is in expression
    /// context decides beforehand with `Parser::skip_type_arguments`.
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
            // An annotation in a type argument is `ANNOTATION_ON_TYPE_ARGUMENT` at it
            // (`AstBuilder`), and is read and discarded.
            if self.at_op(Op::At) {
                for a in self.parse_metadata()? {
                    self.error_at(codigos::parser::ANNOTATION_ON_TYPE_ARGUMENT, a.span, &[]);
                }
            }
            // `NoType.ensureTypeOrVoid`: what does not start a type is
            // `EXPECTED_TYPE_NAME` at it and an empty synthetic type.
            if !matches!(self.kind(), Kind::Ident | Kind::Keyword(Keyword::Void) | Kind::Op(Op::LParen)) {
                self.erro(codigos::parser::EXPECTED_TYPE_NAME, &[]);
                let s = self.span().start;
                let nome = self.name_from("", Span { start: s, end: s });
                args.push(self.ast.push_type(TypeAnnotation {
                    span: Span { start: s, end: s },
                    nullable: false,
                    kind: TypeKind::Named { name: vec![nome].into_boxed_slice(), args: Box::default() },
                }));
            } else {
                args.push(self.parse_type()?);
            }
            if !self.eat_op(Op::Comma) {
                break;
            }
        }
        self.expect_op(Op::Gt)?;
        Ok(args)
    }

    /// `<@meta T extends B, U>` parameters, or an empty list if there is no `<`,
    /// without variance modifiers (functions, methods, `typedef`, extensions):
    /// `in`/`out` there are names.
    pub(crate) fn parse_type_parameters_opt(&mut self) -> PResult<Vec<TypeParameter>> {
        self.parse_type_parameters_with(false)
    }

    /// Like [`Parser::parse_type_parameters_opt`], where fasta accepts variance
    /// (`allowsVariance`: class, mixin, enum).
    pub(crate) fn parse_type_parameters_opt_variance(&mut self) -> PResult<Vec<TypeParameter>> {
        self.parse_type_parameters_with(true)
    }

    fn parse_type_parameters_with(&mut self, variance_: bool) -> PResult<Vec<TypeParameter>> {
        if !self.at_op(Op::Lt) {
            return Ok(Vec::new());
        }
        self.in_type_args += 1;
        let result = self.parse_type_parameters_inner(variance_);
        self.in_type_args -= 1;
        result
    }

    /// Port of `ComplexTypeParamOrArgInfo.parseVariables`
    /// (`type_info_impl.dart`): metadata, variance (with
    /// `MULTIPLE_VARIANCE_MODIFIERS` for the extras and the `variance`
    /// experiment reported at the first modifier, like
    /// `AstBuilder.endTypeVariable`), the name via the
    /// `typeVariableDeclaration` context, the bound, and recovery: a missing
    /// comma between two names (`parseMissingComma`) and an unexpected end
    /// (`parseUnexpectedEnd`).
    fn parse_type_parameters_inner(&mut self, variance_: bool) -> PResult<Vec<TypeParameter>> {
        let abre = self.pos;
        self.expect_op(Op::Lt)?;
        let mut params = Vec::new();
        // The last consumed token is synthetic (an inserted name).
        let mut synthetic;
        loop {
            let start = self.span();
            let metadata = self.parse_metadata_opt()?;
            let mut variance = None;
            if variance_
                && self.is_word_or_identifier(self.pos)
                && super::fasta::is_variance(self.text())
                && self.is_word_or_identifier(self.pos + 1)
            {
                let first_ = self.peek();
                while self.is_word_or_identifier(self.pos + 1)
                    && super::fasta::is_variance(self.text_at(1))
                    && self.is_word_or_identifier(self.pos + 2)
                {
                    let leftover = self.peek_at(1).span;
                    self.error_at(codigos::parser::MULTIPLE_VARIANCE_MODIFIERS, leftover, &[]);
                    self.advance();
                }
                self.advance();
                let texto = self.token_text(first_);
                variance = Variance::from_text_(texto).map(|v| (v, first_.span));
                self.error_at(
                    codigos::parser::EXPERIMENT_NOT_ENABLED_OFF_BY_DEFAULT,
                    first_.span,
                    &["variance"],
                );
            }
            let (name, s) = self.ensure_type_variable_name();
            synthetic = s;
            let bound = if self.eat_kw(Keyword::Extends) {
                synthetic = false;
                Some(self.parse_type()?)
            } else {
                None
            };
            params.push(TypeParameter {
                span: self.span_from(start),
                metadata: metadata.into_boxed_slice(),
                name,
                bound,
                variance,
            });
            if self.eat_op(Op::Comma) {
                continue;
            }
            // `isCloser(token)`: the bound ended at a `>`.
            let last_closer =
                self.pos > 0 && self.tokens[self.pos - 1].kind == Kind::Op(Op::Gt) && !synthetic;
            if last_closer || !self.looks_like_type_param_or_arg(true, self.pos) {
                break;
            }
            // `parseMissingComma`: `Expected to find ','.` at the next token.
            self.erro(codigos::parser::EXPECTED_TOKEN, &[","]);
        }
        if self.eat_op(Op::Gt) {
            return Ok(params);
        }
        self.unexpected_end_of_type_parameters(abre, synthetic)?;
        Ok(params)
    }

    /// `ensureIdentifier` in the `typeVariableDeclaration` context
    /// (`TypeVariableDeclarationIdentifierContext`): a pseudo-keyword or
    /// built-in identifier serves as a name; a token that looks like the start
    /// of the next declaration, member or statement, or that follows a name
    /// (`<`, `>`, `;`, `}`, `extends`, `super`, `=`, end), stays where it is and
    /// the name is synthetic (`MISSING_IDENTIFIER`); a reserved word becomes the
    /// name (`EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD`); any other token is consumed
    /// in place of the name. Returns the name and whether it is synthetic.
    fn ensure_type_variable_name(&mut self) -> (Name, bool) {
        use super::fasta::WordStyle;
        let pos = self.pos;
        if self.kind() == Kind::Ident {
            // Identifier, pseudo-keyword or built-in: the built-in one is reported by
            // `analise` (`BUILT_IN_IDENTIFIER_IN_DECLARATION`)…
            let segue = self.looks_like_top_level_start(pos)
                || self.looks_like_member_start(pos)
                || self.looks_like_statement_start(pos);
            // …except when it looks like the start of something else.
            if !(segue && self.word_style_of(pos) == Some(WordStyle::BuiltIn)) {
                return (self.identifier(), false);
            }
        }
        let name_follows = matches!(
            self.kind(),
            Kind::Op(Op::Lt | Op::Gt | Op::Semicolon | Op::RBrace | Op::Assign)
                | Kind::Keyword(Keyword::Extends | Keyword::Super)
                | Kind::Eof
        );
        let span = self.span();
        let synthetic = |p: &mut Self| {
            let s = Span {
                start: span.start,
                end: span.start,
            };
            p.name_from("", s)
        };
        if self.looks_like_top_level_start(pos)
            || self.looks_like_member_start(pos)
            || self.looks_like_statement_start(pos)
            || name_follows
        {
            self.erro(codigos::parser::MISSING_IDENTIFIER, &[]);
            return (synthetic(self), true);
        }
        if let Kind::Keyword(_) = self.kind() {
            let texto = self.text().to_string();
            self.erro(
                codigos::parser::EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD,
                &[&texto],
            );
            return (self.identifier(), false);
        }
        self.erro(codigos::parser::MISSING_IDENTIFIER, &[]);
        self.advance();
        (synthetic(self), true)
    }

    /// `parseUnexpectedEnd` from `type_info_impl.dart` for type parameters:
    /// `Expected to find '>'.` at the last token read (only once), and the rest
    /// skipped up to the `>` that may close the list — after `extends` and a
    /// type, a nested `<…>` list, `(…)`, or the `>` the scanner matched with the
    /// opening `<`. Without any `>`, the list ends there (fasta inserts a
    /// synthetic `>`).
    fn unexpected_end_of_type_parameters(
        &mut self,
        abre: usize,
        synthetic: bool,
    ) -> PResult<()> {
        let last = self.tokens[self.pos.saturating_sub(1)].span;
        let mut reported = synthetic;
        let report = |p: &mut Self, reported: &mut bool, where_: Span| {
            if !*reported {
                p.error_at(codigos::parser::EXPECTED_TOKEN, where_, &[">"]);
                *reported = true;
            }
        };
        let mut after_extends = false;
        if self.at_kw(Keyword::Extends) {
            report(self, &mut reported, last);
            self.advance();
            after_extends = self.is_valid_type_reference(self.pos);
            if self.eat_op(Op::Gt) {
                return Ok(());
            }
        }
        let where_ = self.tokens[self.pos.saturating_sub(1)].span;
        if (after_extends
            || self.at_kw(Keyword::Void)
            || matches!(self.text(), "dynamic" | "Function") && self.kind() == Kind::Ident)
            && self.skip_type(self.pos).is_some()
        {
            report(self, &mut reported, where_);
            let n = self.diagnostics.len();
            let _ = self.parse_type();
            self.diagnostics.truncate(n);
            if self.eat_op(Op::Gt) {
                return Ok(());
            }
        }
        let where_ = self.tokens[self.pos.saturating_sub(1)].span;
        if self.at_op(Op::Lt) {
            report(self, &mut reported, where_);
            let n = self.diagnostics.len();
            let _ = self.parse_type_parameters_inner(false);
            self.diagnostics.truncate(n);
            if self.eat_op(Op::Gt) {
                return Ok(());
            }
        }
        let where_ = self.tokens[self.pos.saturating_sub(1)].span;
        if self.at_op(Op::LParen)
            && let Some(closes) = self.matching_close(self.pos)
        {
            report(self, &mut reported, where_);
            self.pos = closes + 1;
            if self.eat_op(Op::Gt) {
                return Ok(());
            }
        }
        let where_ = self.tokens[self.pos.saturating_sub(1)].span;
        report(self, &mut reported, where_);
        if self.at_op_at(1, Op::Gt) {
            self.advance();
            self.advance();
            return Ok(());
        }
        if let Some(end_offset) = self.lt_group_end(abre)
            && end_offset >= self.pos
        {
            self.pos = end_offset + 1;
        }
        Ok(())
    }

    // -- Formal parameters -----------------------------------------------------

    /// Full `(a, [b = 1], {required c})`, with the parentheses, in declaration
    /// mode: required names, `this.x`/`super.x`, defaults and the old form
    /// `int f(int x)`.
    pub(crate) fn parse_formal_parameters(&mut self) -> PResult<Vec<Parameter>> {
        self.parse_parameter_list(false)
    }

    /// The `parameterTypeList` of a function type, with the parentheses:
    /// `(int, String name, [int], {required int a})`. An unnamed item becomes
    /// `Parameter { ty: Some(_), name: None }`.
    pub(crate) fn parse_function_type_parameters(&mut self) -> PResult<Vec<Parameter>> {
        self.parse_parameter_list(true)
    }

    fn parse_parameter_list(&mut self, in_function_type: bool) -> PResult<Vec<Parameter>> {
        // The extension type primary constructor arrives as `PrimaryConstructor`;
        // the class and enum one (3.13) arrives as `Other`: its `covariant` is
        // checked during elaboration, which knows whether the parameter declares a
        // field.
        let owner = if in_function_type {
            ParameterOwner::FunctionTypeKind
        } else {
            std::mem::replace(&mut self.params_owner, ParameterOwner::Other)
        };
        self.params_owner = ParameterOwner::Other;
        let abre = self.pos;
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
                let open_group = self.pos;
                self.advance();
                // Empty group (`({})`, `([])`): `MISSING_IDENTIFIER` at the closer and the
                // synthetic parameter fasta inserts (untyped and with an empty name:
                // `C Function({dynamic })`).
                if self.at_op(close) {
                    self.erro(codigos::parser::MISSING_IDENTIFIER, &[]);
                    params.push(self.synthetic_parameter(kind));
                }
                loop {
                    if self.eat_op(close) {
                        break;
                    }
                    // A group inside the group (`{a, {b}}`): the parameter's
                    // `ensureIdentifier` reports `MISSING_IDENTIFIER` at the token and adds a
                    // synthetic parameter (omitted here).
                    if matches!(self.kind(), Kind::Op(Op::LBrace | Op::LBracket)) {
                        self.erro(codigos::parser::MISSING_IDENTIFIER, &[]);
                        params.push(self.synthetic_parameter(kind));
                    } else {
                        params.push(self.parse_formal_parameter(kind, in_function_type, owner)?);
                    }
                    if !self.eat_op(Op::Comma) {
                        // `parseOptionalNamedParameters`/`…PositionalParameters`: without a `,` or
                        // closer, `EXPECTED_TOKEN` (the closer) at the token and the group ends at
                        // the closer matched with the opener.
                        if !self.at_op(close)
                            && let Some(closes) = self.matching_close(open_group)
                            && closes > self.pos
                        {
                            self.erro(codigos::parser::EXPECTED_TOKEN, &[close.text()]);
                            let de = self.pos;
                            self.pos = closes;
                            self.record_skipped(de);
                        }
                        self.expect_op(close)?;
                        break;
                    }
                }
                self.expect_op(Op::RParen)?;
                self.check_public_names(&params);
                self.check_required_default(&params);
                return Ok(params);
            }
            params.push(self.parse_formal_parameter(ParameterKind::Required, in_function_type, owner)?);
            if !self.eat_op(Op::Comma) {
                // `parseFormalParametersRest`: the misplaced synthetic `)` comes here; two
                // identifiers in a row are the missing comma (`EXPECTED_TOKEN` at the
                // second); the rest is `ensureCloseParen`, up to the matching `)`.
                if !self.at_op(Op::RParen) {
                    if self.move_synthetic_closer(abre) {
                        // The `)` is now the current token.
                    } else if self.is_plain_identifier(self.pos) && self.is_plain_identifier(self.pos + 1) {
                        self.erro(codigos::parser::EXPECTED_TOKEN, &[","]);
                        continue;
                    } else {
                        self.ensure_close_paren(abre)?;
                        self.check_public_names(&params);
                        return Ok(params);
                    }
                }
                self.expect_op(Op::RParen)?;
                self.check_public_names(&params);
                return Ok(params);
            }
        }
    }

    /// The parameter `ensureIdentifier` inserts in place of the missing name:
    /// untyped, with an empty name at the current token.
    fn synthetic_parameter(&mut self, kind: ParameterKind) -> Parameter {
        let here = self.span().start;
        let s = Span { start: here, end: here };
        let name = self.name_from("", s);
        Parameter {
            span: s,
            metadata: Box::default(),
            kind,
            required: false,
            covariant: false,
            final_: false,
            var_: false,
            const_: false,
            ty: None,
            this_: false,
            super_: false,
            name: Some(name),
            function_type_params: Box::default(),
            function_parameters: None,
            function_nullable: false,
            default_value: None,
            public_name: None,
            declarante: false,
        }
    }

    /// One parameter: metadata, modifiers, optional type, name (optional only in
    /// function types), the old-form function part and the default.
    fn parse_formal_parameter(
        &mut self,
        kind: ParameterKind,
        in_function_type: bool,
        owner: ParameterOwner,
    ) -> PResult<Parameter> {
        let start = self.span();
        let metadata = self.parse_metadata_opt()?;
        // Modifiers in any order (superset), read by fasta's `ModifierContext`
        // (`parseFormalParameterModifiers`), which reports wrong order, repetition
        // and what does not fit in a parameter. `required`/`covariant`… are plain
        // identifiers when nothing declarable follows them (`{required}` is a
        // parameter named `required`).
        let mut f = ModifierTokens::default();
        self.modifier_context(&mut f, false);
        for p in [f.final_, f.var_].into_iter().flatten() {
            let span = self.tokens[p].span;
            self.check_parameter_modifier(span, in_function_type);
        }
        self.report_parameter_modifiers(&f, kind == ParameterKind::Named, owner);
        let required = f.required.is_some();
        let covariant = f.covariant.is_some();
        let final_ = f.final_.is_some();
        let var_ = f.var_.is_some();
        let const_ = f.const_.is_some();

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
            // `final`/`var` with `this.`/`super.` in the primary constructor: the
            // parameter would be both declaring and initializing (the 3.13.4 parser
            // reports it at the `this`/`super`).
            if self.in_primary_constructor && (final_ || var_) {
                let code = if this_ {
                    codigos::parser::INITIALIZING_DECLARING_PARAMETER
                } else {
                    codigos::parser::SUPER_INITIALIZING_DECLARING_PARAMETER
                };
                let sp = self.span();
                self.error_at(code, sp, &[]);
            }
            self.advance();
            self.advance();
            Some(self.expect_identifier()?)
        } else if self.at_identifier() {
            Some(self.identifier())
        } else if in_function_type && ty.is_some() && kind == ParameterKind::Named {
            // Named in an unnamed function type (`Function({int})`): the type read and
            // `MISSING_IDENTIFIER` at the next token (the name is required,
            // `parseFormalParameter`), with a synthetic name.
            self.erro(codigos::parser::MISSING_IDENTIFIER, &[]);
            let s = self.span().start;
            Some(self.name_from("", Span { start: s, end: s }))
        } else if in_function_type && ty.is_some() {
            None
        } else if matches!(self.kind(), Kind::Op(Op::Comma | Op::RParen | Op::RBracket | Op::RBrace)) {
            // Unnamed parameter before `,` or the closer (`f(,[])`):
            // `MISSING_IDENTIFIER` at it and a synthetic name; the list goes on.
            self.erro(codigos::parser::MISSING_IDENTIFIER, &[]);
            let s = self.span().start;
            Some(self.name_from("", Span { start: s, end: s }))
        } else {
            return Err(self.identifier_error());
        };

        // Old form of a function parameter: `int f<T>(T x)`, `this.f(int x)`.
        let mut function_type_params = Vec::new();
        let mut function_parameters = None;
        let mut function_nullable = false;
        if !in_function_type && name.is_some() && (self.at_op(Op::LParen) || self.at_op(Op::Lt)) {
            function_type_params = self.parse_type_parameters_opt()?;
            self.params_owner = ParameterOwner::FunctionParameter;
            function_parameters = Some(self.parse_formal_parameters()?);
            // `int f(int x)?` — old-form nullability.
            function_nullable = self.eat_op(Op::Question);
        }

        // `parseFormalParameterRest` (`parser_impl.dart:2228-2300` in the main
        // checkout), in the primary header with the feature disabled: the
        // `var`/`final`/`const` of a function-form parameter, and `var` followed by
        // a type, are the disabled feature (outside the primary one, they are
        // `FUNCTION_TYPED_PARAMETER_VAR` and `VAR_AND_TYPE`).
        if owner == ParameterOwner::PrimaryConstructor && !self.in_representation && !self.features.tem(Feature::PrimaryConstructors) {
            let word = if function_parameters.is_some() {
                f.var_final_or_const()
            } else if ty.is_some() {
                f.var_
            } else {
                None
            };
            if let Some(tk) = word {
                // The 3.13.4 oracle's fix carries `3.13` (the `AstBuilder` form), not
                // `3.13.0`.
                let sp = self.span_of(tk);
                self.require_in_ast(Feature::PrimaryConstructors, sp);
            }
        }

        let default_value =
            if self.at_op(Op::Assign) || (kind == ParameterKind::Named && self.at_op(Op::Colon)) {
                // `parseFormalParameter` (`parser_impl.dart:2170`): a default on a required
                // positional is `NAMED_PARAMETER_OUTSIDE_GROUP`; in a function type, an old
                // `typedef` or a function parameter, `DEFAULT_VALUE_IN_FUNCTION_TYPE` (at
                // the `=`/`:`).
                let equals_ = self.span();
                if kind == ParameterKind::Required {
                    self.error_at(codigos::parser::NAMED_PARAMETER_OUTSIDE_GROUP, equals_, &[]);
                } else if matches!(
                    owner,
                    ParameterOwner::FunctionTypeKind | ParameterOwner::TypeAlias | ParameterOwner::FunctionParameter
                ) {
                    self.error_at(codigos::parser::DEFAULT_VALUE_IN_FUNCTION_TYPE, equals_, &[]);
                }
                self.advance();
                Some(self.parse_expression()?)
            } else {
                None
            };

        let public_name = match name {
            Some(n) if kind == ParameterKind::Named && !in_function_type => {
                self.public_name_of_named(n, this_, var_ || final_)
            }
            _ => None,
        };

        Ok(Parameter {
            span: self.span_from(start),
            metadata: metadata.into_boxed_slice(),
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
            function_type_params: function_type_params.into_boxed_slice(),
            function_parameters: function_parameters.map(Vec::into_boxed_slice),
            function_nullable,
            default_value,
            public_name,
            declarante: false,
        })
    }

    /// Named parameter with a private name (`{this._x}`): only allowed when it
    /// initializes a field (`this._x`) or declares it (`var`/`final` in a
    /// primary constructor), from 3.12 on, and if there is a matching public
    /// name (`accepted/3.12/private-named-parameters`, "Static semantics").
    /// Returns the public name; errors are recorded and parsing goes on.
    fn public_name_of_named(&mut self, n: Name, this_: bool, declarante: bool) -> Option<Name> {
        let texto = &self.source[n.span.start..n.span.end];
        let rest = texto.strip_prefix('_')?;
        let initializes_field = this_ || (declarante && self.in_primary_constructor);
        // `_checkPrivateOptionalParameter` (`error_verifier.dart:8622-8661` in the
        // main checkout): with the feature disabled, only the syntactic `this._x`
        // (`FieldFormalParameter`) is the disabled feature; the declaring one
        // (`final`/`var`, syntactically simple) and the rest are
        // `PRIVATE_OPTIONAL_PARAMETER`.
        if !initializes_field || (!this_ && !self.features.tem(Feature::PrivateNamedParameters)) {
            self.diagnostics.push(Diagnostic::com_codigo(
                codigos::compile_time_error::PRIVATE_OPTIONAL_PARAMETER,
                n.span,
                Vec::<&str>::new(),
            ));
            return None;
        }
        self.require(Feature::PrivateNamedParameters, n.span);
        // With the feature disabled, the parameter keeps the private name (the
        // 3.13.4 oracle does not rename: `this._foo` and `foo` do not collide, and
        // two `this._foo` collide as `_foo`).
        if !self.features.tem(Feature::PrivateNamedParameters) {
            return None;
        }
        let valid = rest
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '$')
            && Keyword::from_text(rest).is_none();
        if !valid {
            // Without the feature, the 3.13.4 analyzer only reports the disabled
            // feature, without checking the public name (recorded oracle, 3.6 library).
            if self.features.tem(Feature::PrivateNamedParameters) {
                self.diagnostics.push(Diagnostic::new(
                    format!(
                        "The private named parameter '{texto}' has no corresponding public name."
                    ),
                    n.span,
                ));
            }
            return None;
        }
        let sym = self.interner.intern(rest);
        Some(Name { sym, span: n.span })
    }

    /// `var`/`final` on a parameter: from 3.13 on they only exist on a primary
    /// constructor's declaring parameter (a 3.13 break,
    /// `primary-constructors` "Language versioning"; the CFE says `Can't have
    /// modifier 'final' here`). Before that, `final` is allowed on any
    /// declaration parameter.
    fn check_parameter_modifier(&mut self, span: Span, in_function_type: bool) {
        if in_function_type
            || self.in_primary_constructor
            || !self.features.tem(Feature::PrimaryConstructors)
        {
            return;
        }
        let texto = &self.source[span.start..span.end];
        self.diagnostics.push(Diagnostic::com_codigo(
            codigos::parser::EXTRANEOUS_MODIFIER,
            span,
            [texto],
        ));
    }

    /// A private named parameter's public name that repeats another
    /// parameter's is an error, even against a positional one (3.12 spec).
    /// `_checkUseOfDefaultValuesInParameters` (3.6.2 `error_verifier.dart:6211-6221`,
    /// in `visitFormalParameterList`, every list, including function type lists
    /// and abstract method lists): a `required` named parameter with a default
    /// value reports `DEFAULT_VALUE_ON_REQUIRED_PARAMETER` at the name
    /// (`_parameterName`) or, without a name, at the parameter.
    fn check_required_default(&mut self, params: &[Parameter]) {
        for p in params {
            if p.kind == ParameterKind::Named && p.required && p.default_value.is_some() {
                let sp = p.name.map(|n| n.span).unwrap_or(p.span);
                self.diagnostics.push(Diagnostic::com_codigo(
                    codigos::compile_time_error::DEFAULT_VALUE_ON_REQUIRED_PARAMETER,
                    sp,
                    Vec::<&str>::new(),
                ));
            }
        }
    }

    fn check_public_names(&mut self, params: &[Parameter]) {
        for (i, p) in params.iter().enumerate() {
            let Some(public) = p.public_name else {
                continue;
            };
            let collides = params
                .iter()
                .enumerate()
                .any(|(j, q)| j != i && q.name.is_some_and(|qn| qn.sym == public.sym));
            if collides {
                let texto = self.interner.resolve(public.sym).to_string();
                self.diagnostics.push(Diagnostic::com_codigo(
                    codigos::compile_time_error::DUPLICATE_DEFINITION,
                    public.span,
                    [texto.as_str()],
                ));
            }
        }
    }

    /// `this.` or `super.` at `pos`.
    fn at_field_formal(&self, pos: usize) -> bool {
        matches!(
            self.kind_of(pos),
            Kind::Keyword(Keyword::This) | Kind::Keyword(Keyword::Super)
        ) && self.kind_of(pos + 1) == Kind::Op(Op::Dot)
    }

    /// Variant of `Parser::looks_like_type_then_identifier` for parameter
    /// lists: accepts `this.`/`super.` as the "name" and does not apply the
    /// conditional heuristic, because `{int? a: 1}` is legitimate here.
    fn looks_like_type_then_name(&self, pos: usize) -> bool {
        let Some(end) = self.skip_type(pos) else {
            return false;
        };
        self.kind_of(end) == Kind::Ident || self.at_field_formal(end)
    }

    // -- Lookaheads --------------------------------------------------------

    /// Position after a type starting at `pos`, if there is one syntactically.
    ///
    /// Accepts `p.Name<Args>?`, `void`, `Function<T>(...)` tails (parentheses
    /// skipped with [`Parser::matching_close`]), records `(...)` with fields
    /// validated as types, and a trailing `?`. Conservative on `<`: if the
    /// arguments do not form a well-formed list, the type ends before the `<`
    /// (`a < b` stays an expression).
    /// `looksLikeName` (`type_info.dart`): identifier, `this` or `super`;
    /// `typedef` followed by an identifier is not (it is another declaration).
    pub(crate) fn looks_like_name(&self, pos: usize) -> bool {
        match self.kind_of(pos) {
            Kind::Ident => !(self.text_of(pos) == "typedef" && self.kind_of(pos + 1) == Kind::Ident),
            Kind::Keyword(Keyword::This | Keyword::Super) => true,
            _ => false,
        }
    }

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
                // `computeType`: a built-in followed by `<…>` and a name is a type
                // (recovery, `import<int> x = [];`).
                if is_builtin_identifier(self.text_of(pos))
                    && self.kind_of(pos + 1) != Kind::Op(Op::Dot)
                    && !self.skip_type_arguments_at(pos + 1, depth).is_some_and(|d| self.looks_like_name(d))
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

    /// Skips `Function<T>(...)` from the `Function` at `pos`.
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

    /// Skips the `(fields)` of a record from the `(` at `pos`, validating that
    /// each field is `metadata type name?` (or `{metadata type name, ...}`).
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

    /// Skips zero or more `@name.member<T>(args)` from `pos`. A malformed
    /// annotation stops it and returns the position of its `@`.
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

    /// Position after the `<...>` starting at `pos`, if it is a well-formed type
    /// argument list.
    ///
    /// Requires `<`, one or more comma-separated types and a closing `>`; also
    /// accepts `extends B` and per-item metadata, to serve as a lookahead for
    /// type parameter lists. Anything else (`a < b`, `a < b, c > d` without `>`
    /// in the right place) returns `None`.
    pub(crate) fn skip_type_arguments(&self, pos: usize) -> Option<usize> {
        self.skip_type_arguments_at(pos, 0)
    }

    fn skip_type_arguments_at(&self, pos: usize, depth: u32) -> Option<usize> {
        if depth > MAX_DEPTH || self.kind_of(pos) != Kind::Op(Op::Lt) {
            return None;
        }
        let mut p = pos + 1;
        let mut first = true;
        loop {
            p = self.skip_metadata(p, depth + 1);
            // `ComplexTypeParamOrArgInfo.compute`: without a type (`<>`, `<int,>`) the
            // list is still valid, with the synthetic type `ensureTypeOrVoid` adds; in
            // the first position, only before the `>`.
            p = match self.skip_type_at(p, depth + 1) {
                Some(d) => d,
                None if self.kind_of(p) == Kind::Op(Op::Gt) || (!first && self.kind_of(p) == Kind::Op(Op::Comma)) => p,
                None => return None,
            };
            first = false;
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

    /// Does a `type identifier` (variable, parameter or function declaration)
    /// start at `pos`? Consumes nothing.
    ///
    /// Follows the SDK's `computeType(required: false)`: the type must be
    /// followed by an identifier. When the type ends with `?`, it also applies
    /// the `parseExpressionStatementOrDeclarationAfterModifiers` rule so as not
    /// to read `a ? b : c` as a declaration: after the identifier there must be
    /// `=`, `;`, `,`, `)`, a reserved word, another identifier, end of file, or
    /// `<T>(...)` followed by a function body (`int? f() {}`). The SDK still
    /// undoes `a ? b = c : d` by parsing the expression; here `=` counts as a
    /// declaration.
    pub(crate) fn looks_like_type_then_identifier(&self, pos: usize) -> bool {
        self.looks_like_type_then_identifier_with(pos, false)
    }

    /// Variant of `Parser::looks_like_type_then_identifier` for contexts that
    /// have already decided on a declaration (class member, top level):
    /// `int? a]` and `int? a}` stay declarations — fasta 3.6.2 reports the
    /// missing `;` at the name and flags the closer (`expected_class_member`/
    /// `expected_executable` at it), instead of falling into the conditional
    /// branch. In a function body the restricted form prevails (`int ? a : ...`
    /// is a conditional).
    pub(crate) fn looks_like_type_then_identifier_in_declaration(&self, pos: usize) -> bool {
        self.looks_like_type_then_identifier_with(pos, true)
    }

    fn looks_like_type_then_identifier_with(&self, pos: usize, closer_counts: bool) -> bool {
        let Some(end) = self.skip_type(pos) else {
            return false;
        };
        // `looksLikeName`: `typedef` followed by an identifier is not a name
        // (`base typedef E = Enum;`: `base` is a field's name).
        if self.kind_of(end) != Kind::Ident || !self.looks_like_name(end) {
            return false;
        }
        // `T operator +(...)` is an operator; `final O operator;` uses `operator`
        // as a plain name (it is a built-in identifier, not reserved).
        if self.text_of(end) == "operator" && self.operator_follows_at(end) {
            return true;
        }
        if self.kind_of(end - 1) == Kind::Op(Op::Question) {
            return self.declaration_may_continue_at(end + 1, closer_counts);
        }
        true
    }

    /// Is the token at `pos` `operator` followed by a declarable operator?
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

    /// After `T? name`, the token at `pos` is compatible with a declaration (and
    /// not with a conditional's branch). With `closer_counts` (declaration
    /// context: member, top level), `]`/`}` also count — the `;` is reported at
    /// the name and the closer is flagged separately, as fasta does.
    fn declaration_may_continue_at(&self, pos: usize, closer_counts: bool) -> bool {
        match self.kind_of(pos) {
            Kind::Ident | Kind::Keyword(_) | Kind::Eof => true,
            Kind::Op(Op::Assign | Op::Semicolon | Op::Comma | Op::RParen) => true,
            Kind::Op(Op::RBracket | Op::RBrace) => closer_counts,
            Kind::Op(Op::LParen | Op::Lt) => self.looks_like_function_rest(pos),
            _ => false,
        }
    }

    /// `<T>(...)` or `(...)` at `pos` followed by `{`, `=>`, `async`, `sync`,
    /// `;` (abstract/external methods) or `native` — the rest of a function
    /// declaration, not a call.
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

    /// Builds a parser over `src` and runs `f` with it.
    fn with_parser<T>(src: &str, f: impl FnOnce(&mut Parser<'_, '_>) -> T) -> T {
        let mut names = Interner::new();
        let tokens = lex(src).unwrap();
        let mut p = Parser::new(src, tokens, &mut names);
        // The parameter grammar tested here is 3.6's (`final` on any parameter);
        // 3.13's is in `tests/versions.rs`.
        p.features = crate::features::LibraryFeatures::piso();
        f(&mut p)
    }

    fn name_text(src: &str, name: &Name) -> String {
        src[name.span.start..name.span.end].to_string()
    }

    /// Rewrites a type in a canonical form, to compare textually.
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

    /// Reads the whole `src` as a type and returns the canonical form.
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

    /// Reads the whole `src` as a formal parameter list (declaration mode).
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
            assert!(r.is_err() || !p.at_eof(), "accepted the whole '{src}'");
        })
    }

    // -- parse_type -------------------------------------------------------

    #[test]
    fn simple_and_qualified_names() {
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
    fn names_with_nested_arguments() {
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
    fn function_types() {
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
    fn chained_function_types() {
        // The rightmost `Function` is the outer type.
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
    fn invalid_type() {
        type_fails("");
        type_fails("123");
        type_fails("List<int");
        type_fails("(int");
        type_fails("int Function");
    }

    #[test]
    fn question_after_is_or_as() {
        // `x is int ? a : b`: the `?` goes to the conditional.
        with_parser("int ? a : b", |p| {
            let id = p.parse_type_after_is_or_as().unwrap();
            assert!(!p.ast.ty(id).nullable);
            assert!(p.at_op(Op::Question));
        });
        // `(x is int?)`, `x is int?;`, `x is int? ? a : b`: the `?` belongs to the
        // type.
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
        // `x is int? && y`: in the SDK `&&` closes the type (it does not start an
        // expression).
        with_parser("int? && y", |p| {
            let id = p.parse_type_after_is_or_as().unwrap();
            assert!(p.ast.ty(id).nullable);
        });
        // `x is T ? a : b` with `a` starting with `{` or `when`: a conditional.
        for src in ["int ? {} : b", "int ? when : b"] {
            with_parser(src, |p| {
                let id = p.parse_type_after_is_or_as().unwrap();
                assert!(!p.ast.ty(id).nullable, "{src}");
            });
        }
        // A `{` that does not form a conditional closes the type (a constructor
        // body).
        with_parser("int? { }", |p| {
            let id = p.parse_type_after_is_or_as().unwrap();
            assert!(p.ast.ty(id).nullable);
        });
        // In pure type context the `?` always goes in.
        with_parser("int ? a : b", |p| {
            let id = p.parse_type().unwrap();
            assert!(p.ast.ty(id).nullable);
        });
    }

    #[test]
    fn depth_limited() {
        let src = format!("{}int{}", "List<".repeat(600), ">".repeat(600));
        with_parser(&src, |p| {
            assert!(p.parse_type().is_err());
            assert!(!p.diagnostics.is_empty());
            assert_eq!(p.depth, 0);
            assert_eq!(p.in_type_args, 0);
        });
    }

    // -- type parameters -------------------------------------------------------

    #[test]
    fn type_parameters_() {
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

    /// `(code, line:column)` of the diagnostics of `src`, parsed at 3.6.
    fn diagnostics_36(src: &str) -> Vec<(String, usize, usize)> {
        let mut names = Interner::new();
        let out =
            crate::parser::parse_com(src, &mut names, crate::features::LibraryFeatures::piso());
        out.diagnostics
            .iter()
            .map(|d| {
                let before = &src[..d.span.start];
                let line = before.matches('\n').count() + 1;
                let column = d.span.start - before.rfind('\n').map_or(0, |i| i + 1) + 1;
                (
                    d.code
                        .map(|c| c.info().nome.to_string())
                        .unwrap_or_default(),
                    line,
                    column,
                )
            })
            .collect()
    }

    /// Cases checked against `dart analyze` 3.6.2 (syntax errors only).
    #[test]
    fn variance_and_type_parameter_recovery() {
        let d = |c: &str, l, col| (c.to_string(), l, col);
        let mut v = diagnostics_36("class B<out X, in Y, inout Z> {}");
        v.sort();
        let exp = "experiment_not_enabled_off_by_default";
        assert_eq!(v, [d(exp, 1, 9), d(exp, 1, 16), d(exp, 1, 22)]);
        let mut v = diagnostics_36("class C<in out X> {}");
        v.sort();
        assert_eq!(v, [d(exp, 1, 9), d("multiple_variance_modifiers", 1, 12)]);
        // `out` followed by `>` is the name.
        assert_eq!(diagnostics_36("class G<out out> {}"), [d(exp, 1, 9)]);
        // Outside class, mixin and enum there is no variance: `out` is a name and
        // the comma before `T` is missing.
        assert_eq!(
            diagnostics_36("typedef E<out T> = T Function(T a);"),
            [d("expected_token", 1, 15)]
        );
        assert_eq!(
            diagnostics_36("void f<in T>() {}"),
            [
                d("expected_identifier_but_got_keyword", 1, 8),
                d("expected_token", 1, 11)
            ]
        );
        assert_eq!(
            diagnostics_36("class H<T extends int U> {}"),
            [d("expected_token", 1, 23)]
        );
        assert_eq!(
            diagnostics_36("class I<T U V> {}"),
            [d("expected_token", 1, 11), d("expected_token", 1, 13)]
        );
        // The variance stays in the tree.
        let mut names = Interner::new();
        let out = crate::parser::parse_com(
            "mixin M<inout T> {}",
            &mut names,
            crate::features::LibraryFeatures::piso(),
        );
        let crate::ast::DeclKind::Mixin(m) =
            &out.ast.decls[out.unit.declarations[0].0 as usize].kind
        else {
            panic!("mixin")
        };
        assert_eq!(
            m.type_params[0].variance.map(|v| v.0),
            Some(Variance::Inout)
        );
    }

    // -- formal parameters -----------------------------------------------------

    #[test]
    fn positional_parameters() {
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
        // `const` on a parameter is `EXTRANEOUS_MODIFIER` (fasta).
        assert_eq!(params("(final x, var y)"), "(final x, var y)");
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
    fn this_super_covariant_parameters() {
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
    fn optional_and_named_parameters_without_default() {
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
    fn old_function_parameter_form() {
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

    // They depend on `parse_expression` (expressions.rs) and `parse_metadata`
    // (declarations.rs); they panic with `not yet implemented` while those are
    // stubs.
    #[test]
    fn parameters_with_default_and_metadata() {
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
    fn unnamed_function_type_parameters() {
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
    fn skip_type_accepts_all_forms() {
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
    fn skip_type_is_conservative() {
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
    fn required_with_record_type() {
        // `_fe_analyzer_shared` (`analyzeMapPattern`): `required ({…})? nome`.
        for src in [
            "void f({required ({int a, int b})? t, required List<int> e}) {}",
            "void g({required (int, int)? t}) {}",
            "class C { C({covariant (int,) p = (1,)}); }",
        ] {
            let mut names = Interner::new();
            let p = crate::parser::parse(src, &mut names);
            assert!(p.diagnostics.is_empty(), "{src}: {:?}", p.diagnostics);
        }
    }

    #[test]
    fn skip_type_arguments_requires_well_formed_list() {
        assert_eq!(skip_args("<T, U<V>>"), Some(8));
        assert_eq!(skip_args("<int>"), Some(3));
        assert_eq!(skip_args("<int?>"), Some(4));
        assert_eq!(skip_args("<void>"), Some(3));
        assert_eq!(skip_args("<T extends num, U>"), Some(7));
        assert_eq!(skip_args("<(int, int)>"), Some(7));
        assert_eq!(skip_args("<int Function(int)>"), Some(7));
        assert_eq!(skip_args("< b"), None);
        assert_eq!(skip_args("< b, c > d"), Some(5));
        // `ComplexTypeParamOrArgInfo.compute`: the empty list and the trailing
        // comma are valid (the missing type is `parseArguments` recovery).
        assert_eq!(skip_args("<>"), Some(2));
        assert_eq!(skip_args("<1>"), None);
        assert_eq!(skip_args("<a + b>"), None);
        assert_eq!(skip_args("<T,>"), Some(4));
        assert_eq!(skip_args("x"), None);
    }

    #[test]
    fn looks_like_type_then_identifier_disambiguates() {
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
