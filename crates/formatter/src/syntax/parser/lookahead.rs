//! Side-effect-free *lookahead*, ported from fasta's `type_info.dart` and
//! `type_info_impl.dart` (`computeType`, `computeTypeParamOrArg`,
//! `ComplexTypeInfo`) and from helpers in `parser_impl.dart`
//! (`skipOuterPattern`, `looksLikeLocalFunction`, `skipMetadata`...).
//!
//! Convention: the functions take the index of the construct's first token
//! (fasta's `token.next`) and return the index of the first token *after* it
//! (`end_index`). For "not a type", `end_index` is the starting index itself.

use dartforge_frontend::token::{Keyword, Op};

use super::{Parser, WordStyle, word_style};

/// Shape of the type found (fasta's `TypeInfo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Shape {
    /// `noType`.
    NoneKind,
    /// `simpleType`: identifier.
    Simple,
    /// `simpleNullableType`: identifier `?`.
    SimpleNullable,
    /// `voidType`.
    Void,
    /// `prefixedType`: identifier `.` identifier.
    Prefixed,
    /// `simpleTypeWith1Argument` and its nullable variant.
    WithArg1,
    /// `ComplexTypeInfo`.
    Complex,
}

/// Result of [`Parser::type_at`].
#[derive(Debug, Clone, Copy)]
pub(super) struct TypeInfo {
    pub shape: Shape,
    /// First token after the type.
    pub end_index: usize,
    /// Index of the token before the trailing `?`, if the `?` is part of the
    /// type.
    pub before_question: Option<usize>,
    /// `couldBeExpression`.
    pub could_be_expression: bool,
    /// `isRecordType`.
    pub record: bool,
    /// `gftReturnTypeHasRecordType`.
    pub record_in_return: bool,
    pub recovered: bool,
}

impl TypeInfo {
    pub fn none_(i: usize) -> TypeInfo {
        TypeInfo {
            shape: Shape::NoneKind,
            end_index: i,
            before_question: None,
            could_be_expression: false,
            record: false,
            record_in_return: false,
            recovered: false,
        }
    }

    fn simple(
        shape: Shape,
        end_index: usize,
        nullable: bool,
        could_be_expression: bool,
    ) -> TypeInfo {
        TypeInfo {
            shape,
            end_index,
            before_question: if nullable { Some(end_index - 2) } else { None },
            could_be_expression,
            ..TypeInfo::none_(end_index)
        }
    }

    pub fn is_none(&self) -> bool {
        self.shape == Shape::NoneKind
    }

    pub fn nullable(&self) -> bool {
        self.before_question.is_some()
    }

    /// `asNonNullable`: the trailing `?` is no longer part of the type.
    pub fn without_question(mut self) -> TypeInfo {
        if let Some(a) = self.before_question {
            self.end_index = a + 1;
            self.before_question = None;
            if self.shape == Shape::SimpleNullable {
                self.shape = Shape::Simple;
            }
        }
        self
    }
}

/// Result of [`Parser::type_args_at`] (`TypeParamOrArgInfo`).
#[derive(Debug, Clone, Copy)]
pub(super) struct InfoArgs {
    /// First token after the `>`.
    pub end_index: usize,
    /// `isSimpleTypeArgument`: `<` identifier `>`.
    pub simple: bool,
    pub recovered: bool,
}

/// State of `ComplexTypeInfo` during the computation.
struct Complex {
    start: usize,
    args: Option<InfoArgs>,
    before_question: Option<usize>,
    /// Last token of the type.
    end: usize,
    function_starts: usize,
    gft_return: Option<bool>,
    record: bool,
    record_in_return: bool,
    recovered: bool,
}

impl Complex {
    fn new(start: usize, args: Option<InfoArgs>) -> Complex {
        Complex {
            start,
            args,
            before_question: None,
            end: start,
            function_starts: 0,
            gft_return: None,
            record: false,
            record_in_return: false,
            recovered: args.is_some_and(|a| a.recovered),
        }
    }

    fn info(&self) -> TypeInfo {
        TypeInfo {
            shape: Shape::Complex,
            end_index: self.end + 1,
            before_question: self.before_question,
            could_be_expression: self.args.is_none() && self.function_starts == 0,
            record: self.record,
            record_in_return: self.record_in_return,
            recovered: self.recovered,
        }
    }
}

impl Parser<'_> {
    /// `isGeneralizedFunctionType`: `Function` followed by `<` or `(`.
    pub(super) fn gft(&self, i: usize) -> bool {
        self.word(i, "Function") && (self.op(i + 1, Op::Lt) || self.op(i + 1, Op::LParen))
    }

    /// `isValidNonRecordTypeReference`.
    pub(super) fn valid_type_ref(&self, i: usize) -> bool {
        if self.plain_ident(i) || self.pseudo(i) || self.kw(i, Keyword::Void) {
            return true;
        }
        if self.built_in(i) {
            let t = self.txt(i);
            return self.op(i + 1, Op::Dot) || t == "dynamic" || t == "Function";
        }
        false
    }

    /// `looksLikeName`.
    pub(super) fn looks_like_name(&self, i: usize) -> bool {
        self.plain_ident(i)
            || self.kw(i, Keyword::This)
            || self.kw(i, Keyword::Super)
            || (self.ident(i) && !(self.txt(i) == "typedef" && self.ident(i + 1)))
    }

    /// `looksLikeNameOrEndOfBlock`.
    fn looks_like_name_or_end(&self, i: usize) -> bool {
        self.looks_like_name(i) || self.eof(i) || self.op(i, Op::RBrace)
    }

    /// `isCloser`: only `>` (composite ones are isolated `>` here).
    pub(super) fn type_closer(&self, i: usize) -> bool {
        self.op(i, Op::Gt)
    }

    /// `computeType(token, required, inDeclaration,
    /// acceptKeywordForSimpleType)` for a type starting at `i`.
    pub(super) fn type_at(
        &self,
        i: usize,
        required_: bool,
        decl: bool,
        accepts_word: bool,
    ) -> TypeInfo {
        let n = i;
        if !self.valid_type_ref(n) && !self.op(n, Op::LParen) {
            if self.built_in(n) {
                if let Some(tp) = self.type_args_at(n, decl) {
                    if required_ || self.looks_like_name(tp.end_index) {
                        let mut c = Complex::new(n, Some(tp));
                        c.end = tp.end_index - 1;
                        self.type_rest(&mut c, required_);
                        c.recovered = true;
                        return c.info();
                    }
                } else if required_ || self.gft(n + 1) {
                    let v = self.txt(n);
                    if !(matches!(v, "get" | "set" | "factory" | "operator")
                        || (v == "typedef" && self.ident(n + 1)))
                    {
                        let mut c = Complex::new(n, None);
                        c.end = n;
                        self.type_rest(&mut c, required_);
                        c.recovered = true;
                        return c.info();
                    }
                }
            }
            return TypeInfo::none_(i);
        }
        if self.kw(n, Keyword::Void) {
            if self.gft(n + 1) {
                let mut c = Complex::new(n, None);
                c.end = n;
                self.type_rest(&mut c, required_);
                if c.gft_return.is_none() {
                    return TypeInfo::simple(Shape::Void, n + 1, false, false);
                }
                return c.info();
            }
            return TypeInfo::simple(Shape::Void, n + 1, false, false);
        }
        if self.gft(n) {
            // `computeNoTypeGFT`: the `Function` is the start of the type.
            let mut c = Complex::new(n, None);
            c.end = n.wrapping_sub(1);
            self.type_rest_from(&mut c, n, required_);
            if c.gft_return.is_none() {
                return if required_ {
                    TypeInfo::simple(Shape::Simple, n + 1, false, true)
                } else {
                    TypeInfo::none_(i)
                };
            }
            return c.info();
        }
        if self.op(n, Op::LParen) {
            let after = self.pair(n) + 1;
            if self.gft(after) || (self.op(after, Op::Question) && self.gft(after + 1)) {
                let mut c = Complex::new(n, None);
                c.end = after - 1;
                self.type_rest(&mut c, required_);
                if c.gft_return.is_none() {
                    return self.record_type(n, required_);
                }
                c.record_in_return = true;
                return c.info();
            }
            return self.record_type(n, required_);
        }
        // Identifier.
        if let Some(tp) = self.type_args_at(n, decl) {
            if tp.simple {
                let mut nx = tp.end_index;
                if self.op(nx, Op::Question) {
                    nx += 1;
                    if !self.gft(nx) {
                        if required_ || self.looks_like_name(nx) {
                            return TypeInfo {
                                shape: Shape::WithArg1,
                                end_index: tp.end_index + 1,
                                before_question: Some(tp.end_index - 1),
                                ..TypeInfo::none_(i)
                            };
                        }
                        return TypeInfo::none_(i);
                    }
                } else if !self.gft(nx) {
                    if required_ || self.looks_like_name(nx) {
                        return TypeInfo {
                            shape: Shape::WithArg1,
                            end_index: tp.end_index,
                            ..TypeInfo::none_(i)
                        };
                    }
                    return TypeInfo::none_(i);
                }
            }
            // `computeSimpleWithTypeArguments`.
            let mut c = Complex::new(n, Some(tp));
            c.end = tp.end_index - 1;
            self.type_rest(&mut c, required_);
            if !required_ && !self.looks_like_name_or_end(c.end + 1) && c.gft_return.is_none() {
                return TypeInfo::none_(i);
            }
            return c.info();
        }
        let mut nx = n + 1;
        if self.op(nx, Op::Dot) {
            nx += 1;
            if self.valid_type_ref(nx) {
                let tp2 = self.type_args_at(nx, decl);
                let nn = nx + 1;
                if tp2.is_none() {
                    if self.op(nn, Op::Question) {
                        let m = nn + 1;
                        if !self.gft(m) && !(required_ || self.looks_like_name(m)) {
                            return TypeInfo::none_(i);
                        }
                    } else if !self.gft(nn) {
                        if required_ || self.looks_like_name(nn) {
                            return TypeInfo::simple(Shape::Prefixed, nx + 1, false, true);
                        }
                        return TypeInfo::none_(i);
                    }
                }
                // `computePrefixedType`.
                let mut c = Complex::new(n, tp2);
                c.end = tp2.map_or(nx, |a| a.end_index - 1);
                self.type_rest(&mut c, required_);
                if !required_ && !self.looks_like_name(c.end + 1) && c.gft_return.is_none() {
                    return TypeInfo::none_(i);
                }
                return c.info();
            }
            if required_ {
                // `identifier . non-identifier` (recovery): the prefixed type uses the
                // following word, if any.
                let mut c = Complex::new(n, None);
                let mut t = nx - 1;
                if self.word_(nx) {
                    t = nx;
                }
                let tp = self.type_args_at(t, decl);
                c.args = tp;
                c.end = tp.map_or(t, |a| a.end_index - 1);
                self.type_rest(&mut c, required_);
                c.recovered = true;
                return c.info();
            }
            return TypeInfo::none_(i);
        }
        if self.gft(nx) {
            let mut c = Complex::new(n, None);
            c.end = n;
            self.type_rest(&mut c, required_);
            if c.gft_return.is_none() {
                return TypeInfo::simple(Shape::Simple, n + 1, false, true);
            }
            return c.info();
        }
        if self.op(nx, Op::Question) {
            if self.gft(nx + 1) {
                let mut c = Complex::new(n, None);
                c.end = n;
                self.type_rest(&mut c, required_);
                if c.gft_return.is_none() {
                    return TypeInfo::simple(Shape::SimpleNullable, n + 2, true, true);
                }
                return c.info();
            } else if required_ || self.looks_like_name(nx + 1) {
                return TypeInfo::simple(Shape::SimpleNullable, n + 2, true, true);
            }
        } else if required_
            || self.looks_like_name(nx)
            || (accepts_word
                && self.word_(nx)
                && (self.op(nx + 1, Op::Assign)
                    || self.op(nx + 1, Op::Colon)
                    || self.op(nx + 1, Op::Comma)
                    || self.op(nx + 1, Op::RParen)
                    || self.op(nx + 1, Op::RBracket)
                    || self.op(nx + 1, Op::RBrace)))
        {
            return TypeInfo::simple(Shape::Simple, n + 1, false, true);
        }
        TypeInfo::none_(i)
    }

    /// `computeRest` starting from the last token `c.end`.
    fn type_rest(&self, c: &mut Complex, required_: bool) {
        let mut t = c.end;
        if self.op(t + 1, Op::Question) {
            c.before_question = Some(t);
            t += 1;
            c.end = t;
        }
        self.type_rest_from(c, t + 1, required_);
    }

    /// The `Function` loop of `computeRest`, starting at `t`.
    fn type_rest_from(&self, c: &mut Complex, mut t: usize, required_: bool) {
        while self.word(t, "Function") {
            let var_start = t;
            if let Some(a) = self.type_args_at(t, true) {
                t = a.end_index - 1;
            }
            t += 1;
            if !self.op(t, Op::LParen) {
                break;
            }
            t = self.pair(t);
            if !required_ {
                let mut nx = t + 1;
                if self.op(nx, Op::Question) {
                    nx += 1;
                }
                if !(self.ident(nx) || self.kw(nx, Keyword::This) || self.kw(nx, Keyword::Super)) {
                    break;
                }
            }
            c.gft_return.get_or_insert(var_start != c.start);
            c.function_starts += 1;
            c.before_question = None;
            c.end = t;
            t += 1;
            if self.op(t, Op::Question) {
                c.before_question = Some(c.end);
                c.end = t;
                t += 1;
            }
        }
    }

    /// `computeRecordType` for the `(` at `n`.
    fn record_type(&self, n: usize, required_: bool) -> TypeInfo {
        let mut c = Complex::new(n, None);
        let closer_ = self.pair(n);
        c.recovered = self.recovered_record(n, closer_);
        let token = closer_;
        if !required_ {
            let mut next = token + 1;
            if self.op(next, Op::Question) {
                next += 1;
            }
            let mut get_set = false;
            if (self.word(next, "get") || self.word(next, "set")) && self.ident(next + 1) {
                get_set = true;
                next += 1;
            }
            if self.ident(next) {
                let after = next + 1;
                let ok = self.eof(after)
                    || matches!(
                        self.kind_(after),
                        super::TokenKind::Op(
                            Op::Semicolon
                                | Op::Assign
                                | Op::Lt
                                | Op::LParen
                                | Op::Comma
                                | Op::RParen
                                | Op::RBrace
                                | Op::Colon
                                | Op::RBracket
                        )
                    )
                    || self.kw(after, Keyword::In);
                if !ok {
                    let body_ = self.op(after, Op::Arrow)
                        || self.op(after, Op::LBrace)
                        || self.word(after, "async")
                        || self.word(after, "sync")
                        || self.eof(after);
                    let operator_ = self.word(next, "operator")
                        && self.user_definable_operator(after).is_some();
                    if !(get_set && body_) && !operator_ {
                        return TypeInfo::none_(n);
                    }
                }
            } else if !((self.kw(next, Keyword::This) || self.kw(next, Keyword::Super))
                && self.op(next + 1, Op::Dot))
                && (c.recovered
                    || !(self.op(next, Op::Comma) || self.op(next, Op::Gt) || self.eof(next)))
            {
                return TypeInfo::none_(n);
            }
        }
        c.end = token;
        if self.op(token + 1, Op::Question) {
            c.before_question = Some(token);
            c.end = token + 1;
        }
        c.record = true;
        c.info()
    }

    /// `_checkIfRecordTypeParenthesisAreRecovered`.
    fn recovered_record(&self, opens: usize, closer_: usize) -> bool {
        let mut token = opens;
        let mut count = 0;
        let mut named_args = false;
        let mut comma = false;
        loop {
            let next = token + 1;
            if self.op(next, Op::RParen) {
                token = next;
                break;
            } else if named_args && self.op(next, Op::RBrace) && self.op(next + 1, Op::RParen) {
                token = next + 1;
                break;
            }
            count += 1;
            if !named_args && self.op(next, Op::LBrace) {
                named_args = true;
                token += 1;
            }
            if self.op(token + 1, Op::At) {
                token = self.skip_metadata(token + 1) - 1;
            }
            let t = self.type_at(token + 1, true, false, false);
            if t.recovered {
                return true;
            }
            token = t.end_index - 1;
            if self.ident(token + 1) {
                token += 1;
            } else if named_args {
                return true;
            }
            let next = token + 1;
            if !self.op(next, Op::Comma) {
                if self.op(next, Op::RParen) {
                    token = next;
                } else if self.op(next, Op::RBrace) && self.op(next + 1, Op::RParen) {
                    token = next + 1;
                } else {
                    return true;
                }
                break;
            }
            comma = true;
            token = next;
        }
        (count == 1 && !named_args && !comma) || token != closer_
    }

    /// `computeTypeParamOrArg(token)`: the `<` is at `before + 1`.
    pub(super) fn type_args_at(&self, before: usize, decl: bool) -> Option<InfoArgs> {
        self.type_args_lt(before + 1, decl)
    }

    /// `computeTypeParamOrArg` with the `<` at `lt`.
    pub(super) fn type_args_lt(&self, lt: usize, decl: bool) -> Option<InfoArgs> {
        if !self.op(lt, Op::Lt) {
            return None;
        }
        let next = lt + 1;
        if self.plain_ident(next) || self.pseudo(next) {
            if self.op(next + 1, Op::Gt) {
                return Some(InfoArgs {
                    end_index: next + 2,
                    simple: true,
                    recovered: false,
                });
            }
        } else if self.op(next, Op::LParen) {
            let t = self.type_at(next, false, false, false);
            if !(t.shape == Shape::Complex && (t.record || t.record_in_return) && !t.recovered) {
                return None;
            }
        }
        // `ComplexTypeParamOrArgInfo.compute`.
        let mut recovered = false;
        let mut next = lt;
        let mut token;
        loop {
            let mut ti = self.type_at(next + 1, true, decl, false);
            recovered |= ti.recovered;
            if ti.is_none() {
                while ti.is_none() && self.op(next + 1, Op::At) {
                    next = self.skip_metadata(next + 1) - 1;
                    ti = self.type_at(next + 1, true, decl, false);
                }
                if ti.is_none() {
                    if next == lt && !decl && !self.type_closer(next + 1) {
                        return None;
                    }
                    if !self.op(next + 1, Op::Comma) {
                        token = next;
                        next = token + 1;
                        let _ = token;
                        break;
                    }
                }
            }
            token = if ti.is_none() { next } else { ti.end_index - 1 };
            next = token + 1;
            if self.kw(next, Keyword::Extends) {
                let b = self.type_at(next + 1, true, decl, false);
                token = if b.is_none() { next } else { b.end_index - 1 };
                next = token + 1;
            }
            if !self.op(next, Op::Comma) {
                if self.type_closer(next) {
                    return Some(InfoArgs {
                        end_index: next + 1,
                        simple: false,
                        recovered,
                    });
                }
                if !decl {
                    return None;
                }
                // Recovery: missing comma.
                if !(self.plain_ident(next)
                    && (self.plain_ident(next + 1)
                        || self.op(next + 1, Op::Comma)
                        || self.type_closer(next + 1)))
                {
                    break;
                }
                next = token;
            }
        }
        // Recovery.
        if self.type_closer(next) {
            return Some(InfoArgs {
                end_index: next + 1,
                simple: false,
                recovered,
            });
        }
        let mut n = next;
        if self.op(n, Op::LParen) {
            n = self.pair(n) + 1;
        }
        let end_index = if self.type_closer(n) {
            n + 1
        } else if self.type_closer(n + 1) {
            n + 2
        } else {
            n
        };
        Some(InfoArgs {
            end_index,
            simple: false,
            recovered: true,
        })
    }

    /// `computeMethodTypeArguments`: type arguments followed by a token that
    /// may come after them in an expression (`mayFollowTypeArgs`).
    pub(super) fn method_args_at(&self, before: usize) -> Option<InfoArgs> {
        let a = self.type_args_at(before, false)?;
        if a.recovered {
            return None;
        }
        let t = a.end_index;
        let ok = self.eof(t)
            || matches!(
                self.kind_(t),
                super::TokenKind::Op(
                    Op::LParen
                        | Op::Dot
                        | Op::EqEq
                        | Op::BangEq
                        | Op::RParen
                        | Op::RBracket
                        | Op::RBrace
                        | Op::Semicolon
                        | Op::Colon
                        | Op::Comma
                )
            );
        if ok { Some(a) } else { None }
    }

    /// `skipMetadata` for the `@` at `i`: returns the index after the
    /// annotation.
    pub(super) fn skip_metadata(&self, i: usize) -> usize {
        let mut token = i;
        let mut next = token + 1;
        if self.ident(next) {
            token = next;
            next = token + 1;
            if self.op(next, Op::Dot) {
                token = next;
                next = token + 1;
                if self.ident(next) {
                    token = next;
                    next = token + 1;
                }
            }
            if self.op(next, Op::Lt)
                && let Some(a) = self.type_args_at(token, true)
            {
                token = a.end_index - 1;
                next = token + 1;
            }
            if self.op(next, Op::Dot) {
                token = next;
                next = token + 1;
                if self.ident(next) {
                    token = next;
                    next = token + 1;
                }
            }
            if self.op(next, Op::LParen) {
                token = self.pair(next);
            }
        }
        token + 1
    }

    /// `computeVariablePatternType`.
    pub(super) fn variable_pattern_type_at(&self, i: usize, required_: bool) -> TypeInfo {
        let t = self.type_at(i, required_, false, false);
        if !t.is_none()
            && self.ident(t.end_index)
            && (self.word(t.end_index, "as") || self.word(t.end_index, "when"))
        {
            return TypeInfo::none_(i);
        }
        t
    }

    /// `looksLikeLocalFunction` for token `i`.
    pub(super) fn looks_like_local_function(&self, i: usize) -> bool {
        if !self.ident(i) {
            return false;
        }
        let mut t = i;
        if self.op(t + 1, Op::Lt) {
            match self.type_args_at(t, false) {
                Some(a) => t = a.end_index - 1,
                None => return false,
            }
        }
        t += 1;
        if self.op(t, Op::LParen) {
            let d = self.pair(t) + 1;
            return self.op(d, Op::LBrace)
                || self.op(d, Op::Arrow)
                || self.word(d, "async")
                || self.word(d, "sync");
        }
        self.op(t, Op::Arrow)
    }

    /// `looksLikeFunctionBody`.
    pub(super) fn looks_like_body(&self, i: usize) -> bool {
        self.op(i, Op::LBrace)
            || self.op(i, Op::Arrow)
            || self.word(i, "async")
            || self.word(i, "sync")
    }

    /// `skipOuterPattern`: returns the last token of the outer pattern starting
    /// at `i`, if any.
    pub(super) fn skip_outer_pattern(&self, i: usize) -> Option<usize> {
        if self.ident(i) {
            if !self.op(i + 1, Op::Dot) {
                return self.skip_rest_of_object(i);
            }
            if self.ident(i + 2) {
                return self.skip_rest_of_object(i + 2);
            }
            return None;
        }
        let tp = self.type_args_lt(i, false);
        let next = tp.map_or(i, |a| a.end_index);
        if self.op(next, Op::LBracket) || self.op(next, Op::LBrace) {
            return Some(self.pair(next));
        }
        if tp.is_none() && self.op(next, Op::LParen) {
            return Some(self.pair(next));
        }
        None
    }

    fn skip_rest_of_object(&self, last_name: usize) -> Option<usize> {
        let next = self
            .type_args_at(last_name, false)
            .map_or(last_name + 1, |a| a.end_index);
        if !self.op(next, Op::LParen) {
            return None;
        }
        Some(self.pair(next))
    }

    /// `looksLikeExpressionAfterAwaitOrYield` for the word at `i`; `unary` is the
    /// `UnaryExpression` context.
    pub(super) fn looks_like_expression_after(&self, i: usize, unary: bool) -> bool {
        let t = i + 1;
        if self.kw(t, Keyword::Null) {
            return true;
        }
        if !self.ident(t) {
            return false;
        }
        let t = t + 1;
        if self.op(t, Op::LParen) {
            let d = self.pair(t) + 1;
            return matches!(
                self.kind_(d),
                super::TokenKind::Op(
                    Op::Semicolon
                        | Op::Dot
                        | Op::Comma
                        | Op::DotDot
                        | Op::Question
                        | Op::QuestionDot
                        | Op::RParen
                )
            ) || self.binary_operator(d);
        }
        self.op(t, Op::Dot)
            || self.op(t, Op::RParen)
            || self.op(t, Op::RBracket)
            || self.binary_operator(t)
            || (unary && (self.op(t, Op::Comma) || self.op(t, Op::Semicolon)))
    }

    /// `type.isBinaryOperator`.
    pub(super) fn binary_operator(&self, i: usize) -> bool {
        use super::TokenKind::Op as O;
        matches!(
            self.kind_(i),
            O(Op::Gt)
                | O(Op::Plus
                    | Op::Minus
                    | Op::Star
                    | Op::Slash
                    | Op::TildeSlash
                    | Op::Percent
                    | Op::Lt
                    | Op::LtEq
                    | Op::EqEq
                    | Op::LtLt
                    | Op::Amp
                    | Op::Pipe
                    | Op::Caret
                    | Op::AmpAmp
                    | Op::PipePipe
                    | Op::QuestionQuestion,)
        )
    }

    /// Reserved word (`isReservedWord`).
    pub(super) fn reserved(&self, i: usize) -> bool {
        matches!(self.kind_(i), super::TokenKind::Reserved(_))
    }

    /// Style of the word at `i`.
    #[allow(dead_code)]
    pub(super) fn word_style_at(&self, i: usize) -> WordStyle {
        if self.ident(i) {
            if self.version < (2, 12) && matches!(self.txt(i), "late" | "required") {
                return WordStyle::NoneKind;
            }
            word_style(self.txt(i))
        } else {
            WordStyle::NoneKind
        }
    }
}
