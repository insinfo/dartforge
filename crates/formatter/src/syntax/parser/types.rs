//! Type annotations, type arguments and parameters, formal parameters and
//! metadata (fasta's `parseType`, `parseRecordType`, `parseFormalParameter`,
//! `parseMetadata`, with the `AstBuilder` nodes).
//!
//! Types are read in two steps, as in fasta: the *lookahead*
//! ([`Parser::type_at`]) decides the extent of the type and whether the
//! trailing `?` belongs to it; [`Parser::type_until`] reads the grammar up to
//! that boundary.

use dartforge_frontend::token::{Keyword, Op};

use super::super::tree::{Node, NodeId, ParameterKind};
use super::{Parser, R};

/// Context of the parameter list (fasta's `MemberKind`), at the granularity
/// that changes how it is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MemberKind {
    /// Parameters of `Function(...)`: optional names, required types.
    FunctionType,
    /// Parameters of a function-typed parameter (`void f(int x)`).
    FunctionParameter,
    /// `typedef void F(int x);`.
    FunctionAlias,
    Local,
    InstanceMethod,
    StaticMethod,
    TopLevelFunction,
    Factory,
    Extension,
}

impl Parser<'_> {
    /// Reads a type that ends exactly before `end_index` (computed by
    /// [`Parser::type_at`]).
    pub(super) fn type_until(&mut self, end_index: usize) -> R<NodeId> {
        let start_ = self.pos;
        let mut kind_: Option<NodeId> = None;
        let initial_gft = self.word(self.pos, "Function")
            && self.pos + 1 < end_index
            && (self.op(self.pos + 1, Op::Lt) || self.op(self.pos + 1, Op::LParen));
        if self.kw(self.pos, Keyword::Void) {
            let v = self.advance();
            kind_ = Some(self.node(
                Node::NamedType {
                    import_prefix: None,
                    name2: v as u32,
                    type_arguments: None,
                    question: None,
                },
                v,
            ));
        } else if initial_gft {
        } else if self.op(self.pos, Op::LParen) {
            kind_ = Some(self.record_type_node(end_index)?);
        } else {
            kind_ = Some(self.named_type_(end_index)?);
        }
        while self.pos < end_index && self.word(self.pos, "Function") {
            let f = self.advance();
            let type_parameters = if self.op(self.pos, Op::Lt) {
                Some(self.type_params()?)
            } else {
                None
            };
            if !self.op(self.pos, Op::LParen) {
                return Err(self.expected("("));
            }
            let parameters = self.formal_params(MemberKind::FunctionType)?;
            let question = if self.pos < end_index && self.op(self.pos, Op::Question) {
                Some(self.advance() as u32)
            } else {
                None
            };
            kind_ = Some(self.node(
                Node::GenericFunctionType {
                    return_type: kind_,
                    function_keyword: f as u32,
                    type_parameters,
                    parameters,
                    question,
                },
                start_,
            ));
        }
        if self.pos != end_index {
            return Err(self.error("Unexpected token in type."));
        }
        kind_.ok_or_else(|| self.error("Expected a type."))
    }

    /// `prefix.Nome<args>?`.
    fn named_type_(&mut self, end_index: usize) -> R<NodeId> {
        let start_ = self.pos;
        if !self.word_(self.pos) {
            return Err(self.error(&format!(
                "Expected a type, but got '{}'.",
                self.txt(self.pos)
            )));
        }
        let mut import_prefix = None;
        if self.op(self.pos + 1, Op::Dot) && self.pos + 2 < end_index {
            let p = self.advance();
            let d = self.advance();
            import_prefix = Some((p as u32, d as u32));
        }
        if !self.word_(self.pos) {
            return Err(self.error("Expected a type name."));
        }
        let name_ = self.advance();
        let type_arguments = if self.pos < end_index && self.op(self.pos, Op::Lt) {
            Some(self.type_args()?)
        } else {
            None
        };
        let question = if self.pos < end_index && self.op(self.pos, Op::Question) {
            Some(self.advance() as u32)
        } else {
            None
        };
        Ok(self.node(
            Node::NamedType {
                import_prefix,
                name2: name_ as u32,
                type_arguments,
                question,
            },
            start_,
        ))
    }

    /// Record type (`parseRecordType`).
    fn record_type_node(&mut self, end_index: usize) -> R<NodeId> {
        let start_ = self.pos;
        if self.version < (3, 0) {
            return Err(self.error("This requires the 'records' language feature to be enabled."));
        }
        let lp = self.expect_op(Op::LParen)?;
        let mut positionals = Vec::new();
        let mut named_args = None;
        let mut comma = false;
        loop {
            if self.op(self.pos, Op::RParen) {
                break;
            }
            if positionals.is_empty()
                && self.op(self.pos, Op::Comma)
                && self.op(self.pos + 1, Op::RParen)
            {
                return Err(self.error("A record type without fields can't have a trailing comma."));
            }
            if self.op(self.pos, Op::LBrace) {
                named_args = Some(self.record_named_fields()?);
                break;
            }
            let fi = self.pos;
            let metadata = self.metadata_()?;
            let type_ = self.required_type()?;
            let name = if self.ident(self.pos) {
                Some(self.advance() as u32)
            } else {
                None
            };
            positionals.push(self.node(
                Node::RecordTypeAnnotationPositionalField {
                    metadata,
                    type_,
                    name,
                },
                fi,
            ));
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.advance();
            comma = true;
        }
        let rp = self.expect_op(Op::RParen)?;
        if positionals.len() == 1 && named_args.is_none() && !comma {
            return Err(self.error_at(
                rp,
                "A record type with exactly one positional field requires a trailing comma.",
            ));
        }
        let question = if self.pos < end_index && self.op(self.pos, Op::Question) {
            Some(self.advance() as u32)
        } else {
            None
        };
        Ok(self.node(
            Node::RecordTypeAnnotation {
                left_parenthesis: lp as u32,
                positional_fields: positionals,
                named_fields: named_args,
                right_parenthesis: rp as u32,
                question,
            },
            start_,
        ))
    }

    fn record_named_fields(&mut self) -> R<NodeId> {
        let start_ = self.pos;
        let lb = self.expect_op(Op::LBrace)?;
        let mut fields_ = Vec::new();
        loop {
            if self.op(self.pos, Op::RBrace) {
                break;
            }
            let fi = self.pos;
            let metadata = self.metadata_()?;
            let type_ = self.required_type()?;
            let name = self.expect_ident()? as u32;
            fields_.push(self.node(
                Node::RecordTypeAnnotationNamedField {
                    metadata,
                    type_,
                    name,
                },
                fi,
            ));
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.advance();
        }
        let rb = self.expect_op(Op::RBrace)?;
        if fields_.is_empty() {
            return Err(self.error_at(
                rb,
                "The list of named fields in a record type can't be empty.",
            ));
        }
        Ok(self.node(
            Node::RecordTypeAnnotationNamedFields {
                left_bracket: lb as u32,
                fields: fields_,
                right_bracket: rb as u32,
            },
            start_,
        ))
    }

    /// `computeType(required: true)` followed by `parseType`.
    pub(super) fn required_type(&mut self) -> R<NodeId> {
        let info = self.type_at(self.pos, true, false, false);
        if info.is_none() {
            return Err(self.error(&format!(
                "Expected a type, but got '{}'.",
                self.txt(self.pos)
            )));
        }
        self.type_until(info.end_index)
    }

    /// A type that cannot be `void` or a record/function type where only a
    /// [`Node::NamedType`] fits (`extends`, `with`, `implements`, `on`).
    pub(super) fn required_named_type(&mut self) -> R<NodeId> {
        let i = self.pos;
        let t = self.required_type()?;
        if !matches!(self.tree_.node(t), Node::NamedType { .. }) || self.kw(i, Keyword::Void) {
            return Err(self.error_at(i, "Expected a class name."));
        }
        Ok(t)
    }

    /// `<` types `>`.
    pub(super) fn type_args(&mut self) -> R<NodeId> {
        let start_ = self.pos;
        let lt = self.expect_op(Op::Lt)?;
        let mut args = Vec::new();
        loop {
            args.push(self.required_type()?);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.advance();
        }
        let gt = self.expect_op(Op::Gt)?;
        Ok(self.node(
            Node::TypeArgumentList {
                left_bracket: lt as u32,
                arguments: args,
                right_bracket: gt as u32,
            },
            start_,
        ))
    }

    /// `<` type parameters `>`.
    pub(super) fn type_params(&mut self) -> R<NodeId> {
        let start_ = self.pos;
        if self.type_args_lt(self.pos, true).is_none() {
            return Err(self.error("Invalid type parameters."));
        }
        let lt = self.expect_op(Op::Lt)?;
        let mut params = Vec::new();
        loop {
            let pi = self.pos;
            let metadata = self.metadata_()?;
            if matches!(self.txt(self.pos), "in" | "out" | "inout") && self.ident(self.pos + 1) {
                return Err(self.error("Variance modifiers aren't supported."));
            }
            if !self.ident(self.pos) {
                return Err(self.error("Expected a type variable name."));
            }
            let name = self.advance() as u32;
            let (extends_keyword, bound) = if self.kw(self.pos, Keyword::Extends) {
                let e = self.advance() as u32;
                (Some(e), Some(self.required_type()?))
            } else {
                (None, None)
            };
            params.push(self.node(
                Node::TypeParameter {
                    metadata,
                    name,
                    extends_keyword,
                    bound,
                },
                pi,
            ));
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.advance();
        }
        let gt = self.expect_op(Op::Gt)?;
        Ok(self.node(
            Node::TypeParameterList {
                left_bracket: lt as u32,
                type_parameters: params,
                right_bracket: gt as u32,
            },
            start_,
        ))
    }

    /// Formal parameter list (`parseFormalParametersRest`).
    pub(super) fn formal_params(&mut self, member: MemberKind) -> R<NodeId> {
        let start_ = self.pos;
        let lp = self.expect_op(Op::LParen)?;
        let mut params = Vec::new();
        let mut ld = None;
        let mut rd = None;
        loop {
            if self.op(self.pos, Op::RParen) {
                break;
            }
            if self.op(self.pos, Op::LBracket) || self.op(self.pos, Op::LBrace) {
                let (opens, closer, kind_) = if self.op(self.pos, Op::LBracket) {
                    (
                        Op::LBracket,
                        Op::RBracket,
                        ParameterKind::OptionalPositional,
                    )
                } else {
                    (Op::LBrace, Op::RBrace, ParameterKind::Named)
                };
                ld = Some(self.expect_op(opens)? as u32);
                let before = params.len();
                loop {
                    if self.op(self.pos, closer) {
                        break;
                    }
                    params.push(self.param_formal(kind_, member)?);
                    if !self.op(self.pos, Op::Comma) {
                        break;
                    }
                    self.advance();
                }
                if params.len() == before {
                    return Err(self.error("Optional parameter lists can't be empty."));
                }
                rd = Some(self.expect_op(closer)? as u32);
                break;
            }
            params.push(self.param_formal(ParameterKind::Required, member)?);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.advance();
        }
        let rp = self.expect_op(Op::RParen)?;
        Ok(self.node(
            Node::FormalParameterList {
                left_parenthesis: lp as u32,
                parameters: params,
                left_delimiter: ld,
                right_delimiter: rd,
                right_parenthesis: rp as u32,
            },
            start_,
        ))
    }

    /// One formal parameter (`parseFormalParameter`), already wrapped in
    /// [`Node::DefaultFormalParameter`] when optional.
    fn param_formal(&mut self, kind_: ParameterKind, member: MemberKind) -> R<NodeId> {
        let start_ = self.pos;
        let metadata = self.metadata_()?;
        let in_function_type = member == MemberKind::FunctionType;
        let mut required = None;
        let mut covariant = None;
        let mut keyword = None;
        if self.modifier(self.pos) {
            if self.word(self.pos, "required") && kind_ == ParameterKind::Named {
                required = Some(self.advance() as u32);
            }
            if self.modifier(self.pos) {
                if self.word(self.pos, "covariant") {
                    if matches!(
                        member,
                        MemberKind::StaticMethod
                            | MemberKind::TopLevelFunction
                            | MemberKind::Extension
                    ) {
                        return Err(self.error("Can't have modifier 'covariant' here."));
                    }
                    covariant = Some(self.advance() as u32);
                }
                if self.modifier(self.pos) {
                    if !in_function_type
                        && (self.kw(self.pos, Keyword::Var) || self.kw(self.pos, Keyword::Final))
                    {
                        keyword = Some(self.advance() as u32);
                    }
                    if self.modifier(self.pos) {
                        return Err(self.error(&format!(
                            "Can't have modifier '{}' here.",
                            self.txt(self.pos)
                        )));
                    }
                }
            }
        }
        let info = self.type_at(self.pos, in_function_type, false, true);
        let mut nx = info.end_index;
        let mut info = info;
        if info.is_none() && (self.op(nx, Op::Dot) || (self.ident(nx) && self.op(nx + 1, Op::Dot)))
        {
            // fasta recovery: a malformed prefixed type (for example `p.import`), read
            // again with `required`; the error is not syntactic.
            info = self.type_at(self.pos, true, false, false);
            nx = info.end_index;
        }
        let mut this_super: Option<(usize, usize)> = None;
        if !in_function_type && (self.kw(nx, Keyword::This) || self.kw(nx, Keyword::Super)) {
            if self.op(nx + 1, Op::Dot) {
                this_super = Some((nx, nx + 1));
                nx += 2;
            } else {
                return Err(self.error_at(nx, "Expected '.' after 'this' or 'super'."));
            }
        }
        let mut last = nx;
        if self.ident(nx) {
            last = nx;
            nx += 1;
        } else {
            last = last.wrapping_sub(1);
        }
        let mut inline_function = false;
        if self.op(nx, Op::Lt) {
            if let Some(a) = self.type_args_lt(nx, false)
                && self.op(a.end_index, Op::LParen)
            {
                inline_function = true;
            }
        } else if self.op(nx, Op::LParen) {
            inline_function = true;
        }
        let _ = last;
        if inline_function && keyword.is_some() {
            return Err(
                self.error("Function-typed parameters can't specify 'const', 'final' or 'var'.")
            );
        }
        if !info.is_none() && keyword.is_some_and(|k| self.kw(k as usize, Keyword::Var)) {
            return Err(self.error_at(
                keyword.unwrap_or(0) as usize,
                "Variables can't be declared using both 'var' and a type name.",
            ));
        }
        let type_ = if info.is_none() {
            None
        } else {
            Some(self.type_until(info.end_index)?)
        };
        let (this_kw, period) = match this_super {
            Some((t, p)) => {
                if self.kw(t, Keyword::Super) {
                    self.feature(t, (2, 17), "super-parameters");
                }
                self.pos = p + 1;
                (Some(t), Some(p))
            }
            None => (None, None),
        };
        let named = kind_ == ParameterKind::Named;
        let name_ = if in_function_type && !named && !self.word_(self.pos) && !inline_function {
            None
        } else {
            let n = self.expect_ident()?;
            if this_kw.is_none() {
                self.check_await_yield(n)?;
            }
            Some(n)
        };
        let (type_parameters, parameters, question) = if inline_function {
            let tp = if self.op(self.pos, Op::Lt) {
                Some(self.type_params()?)
            } else {
                None
            };
            let ps = self.formal_params(MemberKind::FunctionParameter)?;
            let q = self.optional_op(Op::Question).map(|q| q as u32);
            (tp, Some(ps), q)
        } else {
            (None, None, None)
        };
        let parameter_ = if let Some(t) = this_kw {
            let name = name_.ok_or_else(|| self.error("Expected a name."))? as u32;
            let node = if self.kw(t, Keyword::This) {
                Node::FieldFormalParameter {
                    metadata,
                    covariant_keyword: covariant,
                    required_keyword: required,
                    keyword,
                    type_,
                    this_keyword: t as u32,
                    period: period.unwrap_or(0) as u32,
                    name,
                    type_parameters,
                    parameters,
                    question,
                    kind: kind_,
                }
            } else {
                Node::SuperFormalParameter {
                    metadata,
                    covariant_keyword: covariant,
                    required_keyword: required,
                    keyword,
                    type_,
                    super_keyword: t as u32,
                    period: period.unwrap_or(0) as u32,
                    name,
                    type_parameters,
                    parameters,
                    question,
                    kind: kind_,
                }
            };
            self.node(node, start_)
        } else if inline_function {
            let name = name_.ok_or_else(|| self.error("Expected a name."))? as u32;
            self.node(
                Node::FunctionTypedFormalParameter {
                    metadata,
                    covariant_keyword: covariant,
                    required_keyword: required,
                    return_type: type_,
                    name,
                    type_parameters,
                    parameters: parameters.unwrap_or(NodeId(0)),
                    question,
                    kind: kind_,
                },
                start_,
            )
        } else {
            self.node(
                Node::SimpleFormalParameter {
                    metadata,
                    covariant_keyword: covariant,
                    required_keyword: required,
                    keyword,
                    type_,
                    name: name_.map(|n| n as u32),
                    kind: kind_,
                },
                start_,
            )
        };
        let mut separator = None;
        let mut default_value = None;
        if self.op(self.pos, Op::Assign) || self.op(self.pos, Op::Colon) {
            let s = self.pos;
            if kind_ == ParameterKind::Required {
                return Err(self.error("Non-optional parameters can't have a default value."));
            }
            if kind_ == ParameterKind::OptionalPositional && self.op(s, Op::Colon) {
                return Err(self.error(
                    "Positional optional parameters can't use ':' to specify a default value.",
                ));
            }
            if matches!(
                member,
                MemberKind::FunctionType
                    | MemberKind::FunctionAlias
                    | MemberKind::FunctionParameter
            ) {
                return Err(self.error("Can't have a default value in a function type."));
            }
            self.advance();
            separator = Some(s as u32);
            default_value = Some(self.expression()?);
        }
        if kind_ == ParameterKind::Required {
            return Ok(parameter_);
        }
        Ok(self.node(
            Node::DefaultFormalParameter {
                parameter: parameter_,
                separator,
                default_value,
                kind: kind_,
            },
            start_,
        ))
    }

    /// The `AstBuilder`'s `checkFieldFormalParameters` (methods and local
    /// functions): a required positional `this.x` outside a constructor.
    pub(super) fn without_field_parameters(&self, list: Option<NodeId>) -> R<()> {
        let Some(l) = list else {
            return Ok(());
        };
        let Node::FormalParameterList { parameters, .. } = self.tree_.node(l) else {
            return Ok(());
        };
        for &p in parameters {
            if matches!(self.tree_.node(p), Node::FieldFormalParameter { .. }) {
                return Err(self.error_at(
                    self.start_(p),
                    "Field formal parameters can only be used in a constructor.",
                ));
            }
        }
        Ok(())
    }

    /// `metadata*`.
    pub(super) fn metadata_(&mut self) -> R<Vec<NodeId>> {
        let mut v = Vec::new();
        while self.op(self.pos, Op::At) {
            v.push(self.annotation()?);
        }
        Ok(v)
    }

    /// One annotation (`parseMetadata`).
    fn annotation(&mut self) -> R<NodeId> {
        let start_ = self.pos;
        let at = self.expect_op(Op::At)?;
        let id = self.expect_ident()?;
        let mut name = self.simple(id);
        if self.op(self.pos, Op::Dot) {
            let d = self.advance();
            let id2 = self.expect_ident_or_new()?;
            let s2 = self.simple(id2);
            name = self.node(
                Node::PrefixedIdentifier {
                    prefix: name,
                    period: d as u32,
                    identifier: s2,
                },
                id,
            );
        }
        let with_type_args = self.op(self.pos, Op::Lt);
        if with_type_args {
            self.feature(self.pos, (2, 14), "generic-metadata");
        }
        let type_arguments = if with_type_args {
            if self.type_args_lt(self.pos, false).is_none() {
                return Err(self.error("Invalid type arguments in annotation."));
            }
            Some(self.type_args()?)
        } else {
            None
        };
        let mut period = None;
        let mut constructor_name = None;
        if self.op(self.pos, Op::Dot) {
            period = Some(self.advance() as u32);
            let c = self.expect_ident_or_new()?;
            constructor_name = Some(self.simple(c));
        }
        if with_type_args && !self.op(self.pos, Op::LParen) {
            return Err(self
                .error("An annotation with type arguments must be followed by an argument list."));
        }
        let mut arguments = None;
        if self.op(self.pos, Op::LParen) {
            if self.tk(self.pos - 1).glued {
                arguments = Some(self.arguments_()?);
            } else if with_type_args {
                return Err(
                    self.error("Annotations can't have spaces or comments before the parenthesis.")
                );
            } else {
                let after = self.pair(self.pos) + 1;
                if self.kw(after, Keyword::Class) || self.kw(after, Keyword::Enum) {
                    return Err(self.error(
                        "Annotations can't have spaces or comments before the parenthesis.",
                    ));
                }
            }
        }
        Ok(self.node(
            Node::Annotation {
                at_sign: at as u32,
                name,
                type_arguments,
                period,
                constructor_name,
                arguments,
            },
            start_,
        ))
    }
}
