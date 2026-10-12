//! Padrões do Dart 3 (`parsePattern`, `parsePrimaryPattern`,
//! `parseVariablePattern` e os sufixos de lista, mapa, record e objeto do
//! fasta), com os nós do `AstBuilder`.

use dartforge_frontend::token::{Keyword, Op};

use super::super::arvore::{No, NoId};
use super::super::token::{TipoToken, Tk};
use super::expressoes::CtxConst;
use super::olhar::{Forma, InfoTipo};
use super::{Parser, R};

/// `PatternContext` do fasta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CtxPadrao {
    /// Declaração (`var (a, b) = ...`): identificadores declaram variáveis.
    Declaracao,
    /// `case`, `if-case` e `switch` como expressão: identificadores são
    /// constantes.
    Correspondencia,
    /// Atribuição por padrão: identificadores são variáveis existentes.
    Atribuicao,
}

impl Parser<'_> {
    /// `parsePattern`.
    pub(super) fn padrao(&mut self, ctx: CtxPadrao) -> R<NoId> {
        self.padrao_prec(ctx, 1)
    }

    fn padrao_prec(&mut self, ctx: CtxPadrao, precedencia: u8) -> R<NoId> {
        self.entrar()?;
        let r = self.padrao_prec_interno(ctx, precedencia);
        self.sair();
        r
    }

    fn padrao_prec_interno(&mut self, ctx: CtxPadrao, precedencia: u8) -> R<NoId> {
        let ini = self.pos;
        let mut p = self.padrao_primario(ctx)?;
        loop {
            let nivel = self.precedencia_em(self.pos, true);
            if nivel < precedencia {
                break;
            }
            let o = self.pos;
            if self.pal(o, "as") {
                if !self.padrao_unario_ok {
                    return Err(self.erro_em(ini, "This pattern cannot appear inside a unary pattern."));
                }
                self.avancar();
                let type_ = self.tipo_apos_is_as()?;
                p = self.no(
                    No::CastPattern {
                        pattern: p,
                        as_token: o as Tk,
                        type_,
                    },
                    ini,
                );
            } else if self.op(o, Op::Bang) || self.op(o, Op::Question) {
                if !self.padrao_unario_ok {
                    return Err(self.erro_em(ini, "This pattern cannot appear inside a unary pattern."));
                }
                self.avancar();
                p = if self.op(o, Op::Bang) {
                    self.no(
                        No::NullAssertPattern {
                            pattern: p,
                            operator: o as Tk,
                        },
                        ini,
                    )
                } else {
                    self.no(
                        No::NullCheckPattern {
                            pattern: p,
                            operator: o as Tk,
                        },
                        ini,
                    )
                };
            } else if self.op(o, Op::AmpAmp) || self.op(o, Op::PipePipe) {
                self.avancar();
                let direita = self.padrao_prec(ctx, nivel + 1)?;
                p = if self.op(o, Op::AmpAmp) {
                    self.no(
                        No::LogicalAndPattern {
                            left_operand: p,
                            operator: o as Tk,
                            right_operand: direita,
                        },
                        ini,
                    )
                } else {
                    self.no(
                        No::LogicalOrPattern {
                            left_operand: p,
                            operator: o as Tk,
                            right_operand: direita,
                        },
                        ini,
                    )
                };
            } else {
                break;
            }
            self.padrao_unario_ok = false;
        }
        Ok(p)
    }

    /// `looksLikePatternStart`.
    fn parece_inicio_padrao(&self, i: usize) -> bool {
        matches!(
            self.tipo(i),
            TipoToken::Identificador
                | TipoToken::Duplo
                | TipoToken::Inteiro
                | TipoToken::Texto
                | TipoToken::Op(
                    Op::Hash
                        | Op::LBrace
                        | Op::LParen
                        | Op::LBracket
                        | Op::Lt
                        | Op::LtEq
                        | Op::Gt
                        | Op::BangEq
                        | Op::EqEq
                )
                | TipoToken::Reservada(
                    Keyword::Null | Keyword::False | Keyword::True | Keyword::Var | Keyword::Final | Keyword::Const
                )
        )
    }

    /// `parsePrimaryPattern`.
    fn padrao_primario(&mut self, ctx: CtxPadrao) -> R<NoId> {
        let ini = self.pos;
        let ta = self.args_tipo_lt(ini, true);
        let next = ta.map_or(ini, |a| a.ate);
        if self.op(next, Op::LBracket) || self.op(next, Op::LBrace) {
            let targs = if ta.is_some() { Some(self.args_tipo()?) } else { None };
            let r = if self.op(self.pos, Op::LBracket) {
                self.lista_padrao(targs, ctx, ini)?
            } else {
                self.mapa_padrao(targs, ctx, ini)?
            };
            self.padrao_unario_ok = true;
            return Ok(r);
        }
        let i = ini;
        if self.kw(i, Keyword::Var) || self.kw(i, Keyword::Final) {
            self.padrao_unario_ok = true;
            return self.padrao_variavel(ctx, None);
        }
        if self.op(i, Op::LParen) {
            let depois = self.par(i) + 1;
            if self.ident(depois) || (self.op(depois, Op::Question) && self.ident(depois + 1)) {
                let ti = self.tipo_de_padrao_variavel_em(i, true);
                if ti.forma == Forma::Complexo && ti.registro && !ti.recuperado {
                    self.padrao_unario_ok = true;
                    return self.padrao_variavel(ctx, Some(ti));
                }
            }
            let r = if self.op(i + 1, Op::RParen) {
                let lp = self.avancar();
                let rp = self.avancar();
                self.no(
                    No::RecordPattern {
                        left_parenthesis: lp as Tk,
                        fields: Vec::new(),
                        right_parenthesis: rp as Tk,
                    },
                    lp,
                )
            } else {
                self.parenteses_ou_registro_padrao(ctx)?
            };
            self.padrao_unario_ok = true;
            return Ok(r);
        }
        if self.kw(i, Keyword::Const) {
            self.avancar();
            let expression = self.expressao_padrao_constante(CtxConst::Explicito)?;
            self.padrao_unario_ok = true;
            return Ok(self.no(
                No::ConstantPattern {
                    const_keyword: Some(i as Tk),
                    expression,
                },
                i,
            ));
        }
        let relacional = match self.tipo(i) {
            TipoToken::Op(Op::Lt | Op::LtEq | Op::EqEq | Op::BangEq) => true,
            TipoToken::Op(Op::Gt) => matches!(self.toks.maior_composto(i as Tk), None | Some(">=")),
            _ => false,
        };
        if relacional {
            let o = if self.op(i, Op::Gt) && self.toks.maior_composto(i as Tk) == Some(">=") {
                self.fundir_maior(i, ">=");
                self.pos += 2;
                i
            } else {
                self.avancar()
            };
            let operand = self.operando_relacional()?;
            self.padrao_unario_ok = false;
            return Ok(self.no(
                No::RelationalPattern {
                    operator: o as Tk,
                    operand,
                },
                i,
            ));
        }
        let ti = self.tipo_de_padrao_variavel_em(i, false);
        if !ti.e_nenhum() {
            self.padrao_unario_ok = true;
            return self.padrao_variavel(ctx, Some(ti));
        }
        if self.ident(i) {
            let primeiro = i;
            let mut token = i;
            let mut segundo = None;
            if self.op(i + 1, Op::Dot) {
                if !self.ident(i + 2) {
                    return Err(self.erro_em(i + 2, "Expected an identifier."));
                }
                segundo = Some(i + 2);
                token = i + 2;
            }
            let potencial = self.args_tipo_em(token, false);
            let depois = potencial.map_or(token + 1, |a| a.ate);
            if self.op(depois, Op::LParen) && !potencial.is_some_and(|a| a.recuperado) {
                let r = self.objeto_padrao(primeiro, segundo, potencial.is_some(), ctx)?;
                self.padrao_unario_ok = true;
                return Ok(r);
            } else if segundo.is_none() {
                let nome = self.txt(primeiro);
                if ctx != CtxPadrao::Correspondencia || nome == "_" {
                    self.padrao_unario_ok = true;
                    return self.padrao_variavel(ctx, None);
                } else if nome == "when" || nome == "as" {
                    return Err(self.erro_em(primeiro, "A pattern can't refer to an identifier named 'when' or 'as'."));
                }
            }
        }
        let expression = self.expressao_padrao_constante(CtxConst::Implicito)?;
        self.padrao_unario_ok = true;
        Ok(self.no(
            No::ConstantPattern {
                const_keyword: None,
                expression,
            },
            i,
        ))
    }

    /// `parseVariablePattern`.
    fn padrao_variavel(&mut self, ctx: CtxPadrao, info: Option<InfoTipo>) -> R<NoId> {
        let ini = self.pos;
        let mut keyword = None;
        let mut type_ = None;
        let mut nu = false;
        if let Some(ti) = info.filter(|t| !t.e_nenhum()) {
            type_ = Some(self.tipo_ate(ti.ate)?);
        } else if self.kw(self.pos, Keyword::Var) || self.kw(self.pos, Keyword::Final) {
            keyword = Some(self.avancar());
            let parenteses = self.op(self.pos, Op::LParen);
            let ti = self.tipo_de_padrao_variavel_em(self.pos, parenteses);
            if !ti.e_nenhum() {
                type_ = Some(self.tipo_ate(ti.ate)?);
            }
        } else {
            nu = true;
        }
        let nome = self.espera_ident()?;
        let texto = self.txt(nome);
        match ctx {
            CtxPadrao::Declaracao => {
                if let Some(k) = keyword {
                    return Err(self.erro_em(k, "Variable patterns in declaration context can't specify 'var' or 'final'."));
                }
            }
            CtxPadrao::Correspondencia => {
                if type_.is_some() && keyword.is_some_and(|k| self.kw(k, Keyword::Var)) {
                    return Err(self.erro_em(keyword.unwrap_or(nome), "Variables can't be declared using both 'var' and a type name."));
                }
            }
            CtxPadrao::Atribuicao => {
                if !nu {
                    return Err(self.erro_em(nome, "Variable patterns in assignments can't declare variables."));
                }
            }
        }
        if texto == "_" {
            return Ok(self.no(
                No::WildcardPattern {
                    keyword: keyword.map(|k| k as Tk),
                    type_,
                    name: nome as Tk,
                },
                ini,
            ));
        }
        if texto == "when" || texto == "as" {
            return Err(self.erro_em(nome, "A variable pattern can't be named 'when' or 'as'."));
        }
        self.conferir_await_yield(nome)?;
        if ctx == CtxPadrao::Atribuicao && nu {
            return Ok(self.no(No::AssignedVariablePattern { name: nome as Tk }, ini));
        }
        Ok(self.no(
            No::DeclaredVariablePattern {
                keyword: keyword.map(|k| k as Tk),
                type_,
                name: nome as Tk,
            },
            ini,
        ))
    }

    /// `...` com subpadrão opcional.
    fn resto_padrao(&mut self, ctx: CtxPadrao) -> R<NoId> {
        let o = self.avancar();
        let pattern = if self.parece_inicio_padrao(self.pos) {
            Some(self.padrao(ctx)?)
        } else {
            None
        };
        Ok(self.no(
            No::RestPatternElement {
                operator: o as Tk,
                pattern,
            },
            o,
        ))
    }

    /// `parseListPatternSuffix`.
    fn lista_padrao(&mut self, targs: Option<NoId>, ctx: CtxPadrao, ini: usize) -> R<NoId> {
        let lb = self.espera_op(Op::LBracket)?;
        let old = self.funcoes;
        self.funcoes = true;
        let mut elementos = Vec::new();
        loop {
            if self.op(self.pos, Op::RBracket) {
                break;
            }
            if self.op(self.pos, Op::Ellipsis) {
                elementos.push(self.resto_padrao(ctx)?);
            } else {
                elementos.push(self.padrao(ctx)?);
            }
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.avancar();
        }
        let rb = self.espera_op(Op::RBracket)?;
        self.funcoes = old;
        Ok(self.no(
            No::ListPattern {
                type_arguments: targs,
                left_bracket: lb as Tk,
                elements: elementos,
                right_bracket: rb as Tk,
            },
            ini,
        ))
    }

    /// `parseMapPatternSuffix`.
    fn mapa_padrao(&mut self, targs: Option<NoId>, ctx: CtxPadrao, ini: usize) -> R<NoId> {
        let lb = self.espera_op(Op::LBrace)?;
        let old = self.funcoes;
        self.funcoes = true;
        let mut elementos = Vec::new();
        if !self.op(self.pos, Op::RBrace) {
            loop {
                if self.op(self.pos, Op::Ellipsis) {
                    elementos.push(self.resto_padrao(ctx)?);
                } else {
                    let key = self.expressao()?;
                    let s = self.espera_op(Op::Colon)?;
                    let value = self.padrao(ctx)?;
                    let ki = self.ini(key);
                    elementos.push(self.no(
                        No::MapPatternEntry {
                            key,
                            separator: s as Tk,
                            value,
                        },
                        ki,
                    ));
                }
                let virgula = self.opcional_op(Op::Comma).is_some();
                if self.op(self.pos, Op::RBrace) {
                    break;
                }
                if !virgula {
                    return Err(self.esperado("}"));
                }
            }
        }
        let rb = self.espera_op(Op::RBrace)?;
        self.funcoes = old;
        Ok(self.no(
            No::MapPattern {
                type_arguments: targs,
                left_bracket: lb as Tk,
                elements: elementos,
                right_bracket: rb as Tk,
            },
            ini,
        ))
    }

    /// Campo de padrão com nome opcional (`(nome)? :`).
    fn campo_padrao(&mut self, ctx: CtxPadrao, permite_parenteses_nome: bool) -> R<(NoId, bool)> {
        let fi = self.pos;
        let mut nome = None;
        if self.op(self.pos, Op::Colon) {
            let c = self.avancar();
            nome = Some(self.no(
                No::PatternFieldName {
                    name: None,
                    colon: c as Tk,
                },
                c,
            ));
        } else if (permite_parenteses_nome || !self.op(self.pos, Op::LParen)) && self.op(self.pos + 1, Op::Colon) {
            let n = self.espera_ident()?;
            let c = self.avancar();
            nome = Some(self.no(
                No::PatternFieldName {
                    name: Some(n as Tk),
                    colon: c as Tk,
                },
                n,
            ));
        }
        let tem_nome = nome.is_some();
        let pattern = self.padrao(ctx)?;
        if tem_nome {
            return Ok((self.no(No::PatternField { name: nome, pattern }, fi), true));
        }
        Ok((pattern, false))
    }

    /// `parseParenthesizedPatternOrRecordPattern`.
    fn parenteses_ou_registro_padrao(&mut self, ctx: CtxPadrao) -> R<NoId> {
        let ini = self.pos;
        let lp = self.espera_op(Op::LParen)?;
        let old = self.funcoes;
        self.funcoes = true;
        let mut campos: Vec<NoId> = Vec::new();
        let mut registro = false;
        let mut valido = false;
        loop {
            if (!campos.is_empty() || registro) && self.op(self.pos, Op::RParen) {
                break;
            }
            let (c, nomeado) = self.campo_padrao(ctx, false)?;
            let c = if registro && !nomeado { self.campo_sem_nome(c) } else { c };
            if nomeado {
                registro = true;
                valido = true;
            }
            campos.push(c);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            if !registro {
                let ultimo = campos.len() - 1;
                campos[ultimo] = self.campo_sem_nome(campos[ultimo]);
            }
            registro = true;
            valido = true;
            self.avancar();
        }
        let rp = self.espera_op(Op::RParen)?;
        self.funcoes = old;
        if registro {
            if campos.len() == 1 && !valido {
                return Err(self.erro_em(rp, "A record pattern with exactly one positional field requires a trailing comma."));
            }
            return Ok(self.no(
                No::RecordPattern {
                    left_parenthesis: lp as Tk,
                    fields: campos,
                    right_parenthesis: rp as Tk,
                },
                ini,
            ));
        }
        let pattern = campos[0];
        Ok(self.no(
            No::ParenthesizedPattern {
                left_parenthesis: lp as Tk,
                pattern,
                right_parenthesis: rp as Tk,
            },
            ini,
        ))
    }

    /// Embrulha um padrão posicional em [`No::PatternField`] sem nome.
    fn campo_sem_nome(&mut self, pattern: NoId) -> NoId {
        if matches!(self.arv.no(pattern), No::PatternField { .. }) {
            return pattern;
        }
        let (a, b) = (self.ini(pattern), self.fim(pattern));
        self.no_entre(No::PatternField { name: None, pattern }, a, b)
    }

    /// `objectPattern ::= typeName typeArguments? '(' patternFields? ')'`.
    fn objeto_padrao(&mut self, primeiro: usize, segundo: Option<usize>, com_args: bool, ctx: CtxPadrao) -> R<NoId> {
        let ini = self.pos;
        let mut import_prefix = None;
        let nome = match segundo {
            Some(s) => {
                import_prefix = Some((primeiro as Tk, (primeiro + 1) as Tk));
                s
            }
            None => primeiro,
        };
        self.pos = nome + 1;
        let type_arguments = if com_args { Some(self.args_tipo()?) } else { None };
        let type_ = self.no(
            No::NamedType {
                import_prefix,
                name2: nome as Tk,
                type_arguments,
                question: None,
            },
            ini,
        );
        let lp = self.espera_op(Op::LParen)?;
        let old = self.funcoes;
        self.funcoes = true;
        let mut campos = Vec::new();
        loop {
            if self.op(self.pos, Op::RParen) {
                break;
            }
            let (c, _) = self.campo_padrao(ctx, true)?;
            let c = self.campo_sem_nome(c);
            campos.push(c);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.avancar();
        }
        let rp = self.espera_op(Op::RParen)?;
        self.funcoes = old;
        Ok(self.no(
            No::ObjectPattern {
                type_,
                left_parenthesis: lp as Tk,
                fields: campos,
                right_parenthesis: rp as Tk,
            },
            ini,
        ))
    }
}
