//! Anotações de tipo, argumentos e parâmetros de tipo, parâmetros formais e
//! metadata (`parseType`, `parseRecordType`, `parseFormalParameter`,
//! `parseMetadata` do fasta, com os nós do `AstBuilder`).
//!
//! Os tipos são lidos em duas etapas, como no fasta: o *lookahead*
//! ([`Parser::tipo_em`]) decide a extensão do tipo e se o `?` final é dele;
//! [`Parser::tipo_ate`] lê a gramática até essa fronteira.

use dartforge_frontend::token::{Keyword, Op};

use super::super::tree::{No, NoId, TipoParametro};
use super::{Parser, R};

/// Contexto da lista de parâmetros (`MemberKind` do fasta), no grão que
/// muda a leitura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Membro {
    /// Parâmetros de `Function(...)`: nomes opcionais, tipos obrigatórios.
    TipoFuncao,
    /// Parâmetros de um parâmetro com tipo de função (`void f(int x)`).
    ParametroFuncao,
    /// `typedef void F(int x);`.
    AliasFuncao,
    Local,
    MetodoInstancia,
    MetodoEstatico,
    FuncaoTopo,
    Fabrica,
    Extensao,
}

impl Parser<'_> {
    /// Lê um tipo que termina exatamente antes de `ate` (calculado por
    /// [`Parser::tipo_em`]).
    pub(super) fn tipo_ate(&mut self, ate: usize) -> R<NoId> {
        let ini = self.pos;
        let mut tipo: Option<NoId> = None;
        let gft_inicial = self.pal(self.pos, "Function")
            && self.pos + 1 < ate
            && (self.op(self.pos + 1, Op::Lt) || self.op(self.pos + 1, Op::LParen));
        if self.kw(self.pos, Keyword::Void) {
            let v = self.avancar();
            tipo = Some(self.no(
                No::NamedType {
                    import_prefix: None,
                    name2: v as u32,
                    type_arguments: None,
                    question: None,
                },
                v,
            ));
        } else if gft_inicial {
        } else if self.op(self.pos, Op::LParen) {
            tipo = Some(self.tipo_registro_no(ate)?);
        } else {
            tipo = Some(self.tipo_nomeado(ate)?);
        }
        while self.pos < ate && self.pal(self.pos, "Function") {
            let f = self.avancar();
            let type_parameters = if self.op(self.pos, Op::Lt) {
                Some(self.params_tipo()?)
            } else {
                None
            };
            if !self.op(self.pos, Op::LParen) {
                return Err(self.esperado("("));
            }
            let parameters = self.params_formais(Membro::TipoFuncao)?;
            let question = if self.pos < ate && self.op(self.pos, Op::Question) {
                Some(self.avancar() as u32)
            } else {
                None
            };
            tipo = Some(self.no(
                No::GenericFunctionType {
                    return_type: tipo,
                    function_keyword: f as u32,
                    type_parameters,
                    parameters,
                    question,
                },
                ini,
            ));
        }
        if self.pos != ate {
            return Err(self.erro("Unexpected token in type."));
        }
        tipo.ok_or_else(|| self.erro("Expected a type."))
    }

    /// `prefixo.Nome<args>?`.
    fn tipo_nomeado(&mut self, ate: usize) -> R<NoId> {
        let ini = self.pos;
        if !self.palavra(self.pos) {
            return Err(self.erro(&format!(
                "Expected a type, but got '{}'.",
                self.txt(self.pos)
            )));
        }
        let mut import_prefix = None;
        if self.op(self.pos + 1, Op::Dot) && self.pos + 2 < ate {
            let p = self.avancar();
            let d = self.avancar();
            import_prefix = Some((p as u32, d as u32));
        }
        if !self.palavra(self.pos) {
            return Err(self.erro("Expected a type name."));
        }
        let nome = self.avancar();
        let type_arguments = if self.pos < ate && self.op(self.pos, Op::Lt) {
            Some(self.args_tipo()?)
        } else {
            None
        };
        let question = if self.pos < ate && self.op(self.pos, Op::Question) {
            Some(self.avancar() as u32)
        } else {
            None
        };
        Ok(self.no(
            No::NamedType {
                import_prefix,
                name2: nome as u32,
                type_arguments,
                question,
            },
            ini,
        ))
    }

    /// Tipo record (`parseRecordType`).
    fn tipo_registro_no(&mut self, ate: usize) -> R<NoId> {
        let ini = self.pos;
        if self.versao < (3, 0) {
            return Err(self.erro("This requires the 'records' language feature to be enabled."));
        }
        let lp = self.espera_op(Op::LParen)?;
        let mut posicionais = Vec::new();
        let mut nomeados = None;
        let mut virgula = false;
        loop {
            if self.op(self.pos, Op::RParen) {
                break;
            }
            if posicionais.is_empty()
                && self.op(self.pos, Op::Comma)
                && self.op(self.pos + 1, Op::RParen)
            {
                return Err(self.erro("A record type without fields can't have a trailing comma."));
            }
            if self.op(self.pos, Op::LBrace) {
                nomeados = Some(self.campos_nomeados_registro()?);
                break;
            }
            let fi = self.pos;
            let metadata = self.metadados()?;
            let type_ = self.tipo_requerido()?;
            let name = if self.ident(self.pos) {
                Some(self.avancar() as u32)
            } else {
                None
            };
            posicionais.push(self.no(
                No::RecordTypeAnnotationPositionalField {
                    metadata,
                    type_,
                    name,
                },
                fi,
            ));
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.avancar();
            virgula = true;
        }
        let rp = self.espera_op(Op::RParen)?;
        if posicionais.len() == 1 && nomeados.is_none() && !virgula {
            return Err(self.erro_em(
                rp,
                "A record type with exactly one positional field requires a trailing comma.",
            ));
        }
        let question = if self.pos < ate && self.op(self.pos, Op::Question) {
            Some(self.avancar() as u32)
        } else {
            None
        };
        Ok(self.no(
            No::RecordTypeAnnotation {
                left_parenthesis: lp as u32,
                positional_fields: posicionais,
                named_fields: nomeados,
                right_parenthesis: rp as u32,
                question,
            },
            ini,
        ))
    }

    fn campos_nomeados_registro(&mut self) -> R<NoId> {
        let ini = self.pos;
        let lb = self.espera_op(Op::LBrace)?;
        let mut campos = Vec::new();
        loop {
            if self.op(self.pos, Op::RBrace) {
                break;
            }
            let fi = self.pos;
            let metadata = self.metadados()?;
            let type_ = self.tipo_requerido()?;
            let name = self.espera_ident()? as u32;
            campos.push(self.no(
                No::RecordTypeAnnotationNamedField {
                    metadata,
                    type_,
                    name,
                },
                fi,
            ));
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.avancar();
        }
        let rb = self.espera_op(Op::RBrace)?;
        if campos.is_empty() {
            return Err(self.erro_em(
                rb,
                "The list of named fields in a record type can't be empty.",
            ));
        }
        Ok(self.no(
            No::RecordTypeAnnotationNamedFields {
                left_bracket: lb as u32,
                fields: campos,
                right_bracket: rb as u32,
            },
            ini,
        ))
    }

    /// `computeType(required: true)` seguido de `parseType`.
    pub(super) fn tipo_requerido(&mut self) -> R<NoId> {
        let info = self.tipo_em(self.pos, true, false, false);
        if info.e_nenhum() {
            return Err(self.erro(&format!(
                "Expected a type, but got '{}'.",
                self.txt(self.pos)
            )));
        }
        self.tipo_ate(info.ate)
    }

    /// Tipo que não pode ser `void` nem record/função onde só cabe um
    /// [`No::NamedType`] (`extends`, `with`, `implements`, `on`).
    pub(super) fn tipo_nomeado_requerido(&mut self) -> R<NoId> {
        let i = self.pos;
        let t = self.tipo_requerido()?;
        if !matches!(self.arv.no(t), No::NamedType { .. }) || self.kw(i, Keyword::Void) {
            return Err(self.erro_em(i, "Expected a class name."));
        }
        Ok(t)
    }

    /// `<` tipos `>`.
    pub(super) fn args_tipo(&mut self) -> R<NoId> {
        let ini = self.pos;
        let lt = self.espera_op(Op::Lt)?;
        let mut args = Vec::new();
        loop {
            args.push(self.tipo_requerido()?);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.avancar();
        }
        let gt = self.espera_op(Op::Gt)?;
        Ok(self.no(
            No::TypeArgumentList {
                left_bracket: lt as u32,
                arguments: args,
                right_bracket: gt as u32,
            },
            ini,
        ))
    }

    /// `<` parâmetros de tipo `>`.
    pub(super) fn params_tipo(&mut self) -> R<NoId> {
        let ini = self.pos;
        if self.args_tipo_lt(self.pos, true).is_none() {
            return Err(self.erro("Invalid type parameters."));
        }
        let lt = self.espera_op(Op::Lt)?;
        let mut params = Vec::new();
        loop {
            let pi = self.pos;
            let metadata = self.metadados()?;
            if matches!(self.txt(self.pos), "in" | "out" | "inout") && self.ident(self.pos + 1) {
                return Err(self.erro("Variance modifiers aren't supported."));
            }
            if !self.ident(self.pos) {
                return Err(self.erro("Expected a type variable name."));
            }
            let name = self.avancar() as u32;
            let (extends_keyword, bound) = if self.kw(self.pos, Keyword::Extends) {
                let e = self.avancar() as u32;
                (Some(e), Some(self.tipo_requerido()?))
            } else {
                (None, None)
            };
            params.push(self.no(
                No::TypeParameter {
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
            self.avancar();
        }
        let gt = self.espera_op(Op::Gt)?;
        Ok(self.no(
            No::TypeParameterList {
                left_bracket: lt as u32,
                type_parameters: params,
                right_bracket: gt as u32,
            },
            ini,
        ))
    }

    /// Lista de parâmetros formais (`parseFormalParametersRest`).
    pub(super) fn params_formais(&mut self, membro: Membro) -> R<NoId> {
        let ini = self.pos;
        let lp = self.espera_op(Op::LParen)?;
        let mut params = Vec::new();
        let mut ld = None;
        let mut rd = None;
        loop {
            if self.op(self.pos, Op::RParen) {
                break;
            }
            if self.op(self.pos, Op::LBracket) || self.op(self.pos, Op::LBrace) {
                let (abre, fecha, tipo) = if self.op(self.pos, Op::LBracket) {
                    (
                        Op::LBracket,
                        Op::RBracket,
                        TipoParametro::OpcionalPosicional,
                    )
                } else {
                    (Op::LBrace, Op::RBrace, TipoParametro::Nomeado)
                };
                ld = Some(self.espera_op(abre)? as u32);
                let antes = params.len();
                loop {
                    if self.op(self.pos, fecha) {
                        break;
                    }
                    params.push(self.param_formal(tipo, membro)?);
                    if !self.op(self.pos, Op::Comma) {
                        break;
                    }
                    self.avancar();
                }
                if params.len() == antes {
                    return Err(self.erro("Optional parameter lists can't be empty."));
                }
                rd = Some(self.espera_op(fecha)? as u32);
                break;
            }
            params.push(self.param_formal(TipoParametro::Obrigatorio, membro)?);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.avancar();
        }
        let rp = self.espera_op(Op::RParen)?;
        Ok(self.no(
            No::FormalParameterList {
                left_parenthesis: lp as u32,
                parameters: params,
                left_delimiter: ld,
                right_delimiter: rd,
                right_parenthesis: rp as u32,
            },
            ini,
        ))
    }

    /// Um parâmetro formal (`parseFormalParameter`), já embrulhado em
    /// [`No::DefaultFormalParameter`] quando opcional.
    fn param_formal(&mut self, tipo: TipoParametro, membro: Membro) -> R<NoId> {
        let ini = self.pos;
        let metadata = self.metadados()?;
        let em_tipo_funcao = membro == Membro::TipoFuncao;
        let mut required = None;
        let mut covariant = None;
        let mut keyword = None;
        if self.modificador(self.pos) {
            if self.pal(self.pos, "required") && tipo == TipoParametro::Nomeado {
                required = Some(self.avancar() as u32);
            }
            if self.modificador(self.pos) {
                if self.pal(self.pos, "covariant") {
                    if matches!(
                        membro,
                        Membro::MetodoEstatico | Membro::FuncaoTopo | Membro::Extensao
                    ) {
                        return Err(self.erro("Can't have modifier 'covariant' here."));
                    }
                    covariant = Some(self.avancar() as u32);
                }
                if self.modificador(self.pos) {
                    if !em_tipo_funcao
                        && (self.kw(self.pos, Keyword::Var) || self.kw(self.pos, Keyword::Final))
                    {
                        keyword = Some(self.avancar() as u32);
                    }
                    if self.modificador(self.pos) {
                        return Err(self.erro(&format!(
                            "Can't have modifier '{}' here.",
                            self.txt(self.pos)
                        )));
                    }
                }
            }
        }
        let info = self.tipo_em(self.pos, em_tipo_funcao, false, true);
        let mut nx = info.ate;
        let mut info = info;
        if info.e_nenhum() && (self.op(nx, Op::Dot) || (self.ident(nx) && self.op(nx + 1, Op::Dot)))
        {
            // Recuperação do fasta: tipo prefixado malformado (por exemplo
            // `p.import`), relida com `required`; o erro não é sintático.
            info = self.tipo_em(self.pos, true, false, false);
            nx = info.ate;
        }
        let mut this_super: Option<(usize, usize)> = None;
        if !em_tipo_funcao && (self.kw(nx, Keyword::This) || self.kw(nx, Keyword::Super)) {
            if self.op(nx + 1, Op::Dot) {
                this_super = Some((nx, nx + 1));
                nx += 2;
            } else {
                return Err(self.erro_em(nx, "Expected '.' after 'this' or 'super'."));
            }
        }
        let mut ultimo = nx;
        if self.ident(nx) {
            ultimo = nx;
            nx += 1;
        } else {
            ultimo = ultimo.wrapping_sub(1);
        }
        let mut funcao_inline = false;
        if self.op(nx, Op::Lt) {
            if let Some(a) = self.args_tipo_lt(nx, false)
                && self.op(a.ate, Op::LParen)
            {
                funcao_inline = true;
            }
        } else if self.op(nx, Op::LParen) {
            funcao_inline = true;
        }
        let _ = ultimo;
        if funcao_inline && keyword.is_some() {
            return Err(
                self.erro("Function-typed parameters can't specify 'const', 'final' or 'var'.")
            );
        }
        if !info.e_nenhum() && keyword.is_some_and(|k| self.kw(k as usize, Keyword::Var)) {
            return Err(self.erro_em(
                keyword.unwrap_or(0) as usize,
                "Variables can't be declared using both 'var' and a type name.",
            ));
        }
        let type_ = if info.e_nenhum() {
            None
        } else {
            Some(self.tipo_ate(info.ate)?)
        };
        let (this_kw, period) = match this_super {
            Some((t, p)) => {
                if self.kw(t, Keyword::Super) {
                    self.recurso(t, (2, 17), "super-parameters");
                }
                self.pos = p + 1;
                (Some(t), Some(p))
            }
            None => (None, None),
        };
        let nomeado = tipo == TipoParametro::Nomeado;
        let nome = if em_tipo_funcao && !nomeado && !self.palavra(self.pos) && !funcao_inline {
            None
        } else {
            let n = self.espera_ident()?;
            if this_kw.is_none() {
                self.conferir_await_yield(n)?;
            }
            Some(n)
        };
        let (type_parameters, parameters, question) = if funcao_inline {
            let tp = if self.op(self.pos, Op::Lt) {
                Some(self.params_tipo()?)
            } else {
                None
            };
            let ps = self.params_formais(Membro::ParametroFuncao)?;
            let q = self.opcional_op(Op::Question).map(|q| q as u32);
            (tp, Some(ps), q)
        } else {
            (None, None, None)
        };
        let parametro = if let Some(t) = this_kw {
            let name = nome.ok_or_else(|| self.erro("Expected a name."))? as u32;
            let no = if self.kw(t, Keyword::This) {
                No::FieldFormalParameter {
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
                    kind: tipo,
                }
            } else {
                No::SuperFormalParameter {
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
                    kind: tipo,
                }
            };
            self.no(no, ini)
        } else if funcao_inline {
            let name = nome.ok_or_else(|| self.erro("Expected a name."))? as u32;
            self.no(
                No::FunctionTypedFormalParameter {
                    metadata,
                    covariant_keyword: covariant,
                    required_keyword: required,
                    return_type: type_,
                    name,
                    type_parameters,
                    parameters: parameters.unwrap_or(NoId(0)),
                    question,
                    kind: tipo,
                },
                ini,
            )
        } else {
            self.no(
                No::SimpleFormalParameter {
                    metadata,
                    covariant_keyword: covariant,
                    required_keyword: required,
                    keyword,
                    type_,
                    name: nome.map(|n| n as u32),
                    kind: tipo,
                },
                ini,
            )
        };
        let mut separator = None;
        let mut default_value = None;
        if self.op(self.pos, Op::Assign) || self.op(self.pos, Op::Colon) {
            let s = self.pos;
            if tipo == TipoParametro::Obrigatorio {
                return Err(self.erro("Non-optional parameters can't have a default value."));
            }
            if tipo == TipoParametro::OpcionalPosicional && self.op(s, Op::Colon) {
                return Err(self.erro(
                    "Positional optional parameters can't use ':' to specify a default value.",
                ));
            }
            if matches!(
                membro,
                Membro::TipoFuncao | Membro::AliasFuncao | Membro::ParametroFuncao
            ) {
                return Err(self.erro("Can't have a default value in a function type."));
            }
            self.avancar();
            separator = Some(s as u32);
            default_value = Some(self.expressao()?);
        }
        if tipo == TipoParametro::Obrigatorio {
            return Ok(parametro);
        }
        Ok(self.no(
            No::DefaultFormalParameter {
                parameter: parametro,
                separator,
                default_value,
                kind: tipo,
            },
            ini,
        ))
    }

    /// `checkFieldFormalParameters` do `AstBuilder` (métodos e funções
    /// locais): `this.x` posicional obrigatório fora de construtor.
    pub(super) fn sem_parametros_de_campo(&self, lista: Option<NoId>) -> R<()> {
        let Some(l) = lista else {
            return Ok(());
        };
        let No::FormalParameterList { parameters, .. } = self.arv.no(l) else {
            return Ok(());
        };
        for &p in parameters {
            if matches!(self.arv.no(p), No::FieldFormalParameter { .. }) {
                return Err(self.erro_em(
                    self.ini(p),
                    "Field formal parameters can only be used in a constructor.",
                ));
            }
        }
        Ok(())
    }

    /// `metadata*`.
    pub(super) fn metadados(&mut self) -> R<Vec<NoId>> {
        let mut v = Vec::new();
        while self.op(self.pos, Op::At) {
            v.push(self.anotacao()?);
        }
        Ok(v)
    }

    /// Uma anotação (`parseMetadata`).
    fn anotacao(&mut self) -> R<NoId> {
        let ini = self.pos;
        let at = self.espera_op(Op::At)?;
        let id = self.espera_ident()?;
        let mut name = self.simples(id);
        if self.op(self.pos, Op::Dot) {
            let d = self.avancar();
            let id2 = self.espera_ident_ou_new()?;
            let s2 = self.simples(id2);
            name = self.no(
                No::PrefixedIdentifier {
                    prefix: name,
                    period: d as u32,
                    identifier: s2,
                },
                id,
            );
        }
        let com_args_tipo = self.op(self.pos, Op::Lt);
        if com_args_tipo {
            self.recurso(self.pos, (2, 14), "generic-metadata");
        }
        let type_arguments = if com_args_tipo {
            if self.args_tipo_lt(self.pos, false).is_none() {
                return Err(self.erro("Invalid type arguments in annotation."));
            }
            Some(self.args_tipo()?)
        } else {
            None
        };
        let mut period = None;
        let mut constructor_name = None;
        if self.op(self.pos, Op::Dot) {
            period = Some(self.avancar() as u32);
            let c = self.espera_ident_ou_new()?;
            constructor_name = Some(self.simples(c));
        }
        if com_args_tipo && !self.op(self.pos, Op::LParen) {
            return Err(self
                .erro("An annotation with type arguments must be followed by an argument list."));
        }
        let mut arguments = None;
        if self.op(self.pos, Op::LParen) {
            if self.tk(self.pos - 1).colado {
                arguments = Some(self.argumentos()?);
            } else if com_args_tipo {
                return Err(
                    self.erro("Annotations can't have spaces or comments before the parenthesis.")
                );
            } else {
                let depois = self.par(self.pos) + 1;
                if self.kw(depois, Keyword::Class) || self.kw(depois, Keyword::Enum) {
                    return Err(self.erro(
                        "Annotations can't have spaces or comments before the parenthesis.",
                    ));
                }
            }
        }
        Ok(self.no(
            No::Annotation {
                at_sign: at as u32,
                name,
                type_arguments,
                period,
                constructor_name,
                arguments,
            },
            ini,
        ))
    }
}
