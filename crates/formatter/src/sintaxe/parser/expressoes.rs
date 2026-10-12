//! Expressões (`parseExpression`, `parsePrecedenceExpression`,
//! `parseUnaryExpression`, `parsePrimary`, `parseSend`,
//! `parseArgumentOrIndexStar`, `parseCascadeExpression` e os literais do
//! fasta), montando os nós como o `AstBuilder`:
//!
//! * `handleSend` com argumentos: `MethodInvocation` se o receptor é
//!   `SimpleIdentifier`, senão `FunctionExpressionInvocation`;
//! * `doDotExpression`: `a.b` com `a` identificador simples é
//!   `PrefixedIdentifier`; os demais acessos são `PropertyAccess`; uma
//!   invocação à direita do `.` recebe o alvo;
//! * argumentos de tipo sem chamada (`List<int>`, `a.b<T>`) são
//!   `FunctionReference` (`handleTypeArgumentApplication`).

use dartforge_frontend::token::{Keyword, Op};

use super::super::arvore::{No, NoId};
use super::super::token::{TipoToken, Tk};
use super::padroes::CtxPadrao;
use super::tipos::Membro;
use super::{Assinc, Parser, R, prec};

/// `ConstantPatternContext` do fasta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CtxConst {
    Nenhum,
    Implicito,
    Explicito,
    SoNumerico,
}

/// O primeiro escape inválido de `b` (o texto entre as aspas), como o
/// `unescapeCodeUnits` do fasta (`quote.dart`).
fn erro_de_escape(b: &[u8]) -> Option<&'static str> {
    let n = b.len();
    let hex = |x: u8| x.is_ascii_hexdigit();
    let valor = |x: u8| (x as char).to_digit(16).unwrap_or(0);
    let mut i = 0;
    while i < n {
        if b[i] != b'\\' {
            i += 1;
            continue;
        }
        i += 1;
        if i == n {
            return Some("An escape sequence starting with '\\' must be followed by a character.");
        }
        match b[i] {
            b'x' => {
                if n <= i + 2 || !hex(b[i + 1]) || !hex(b[i + 2]) {
                    return Some(
                        "An escape sequence starting with '\\x' must be followed by 2 hexadecimal digits.",
                    );
                }
                i += 2;
            }
            b'u' => {
                if n == i + 1 {
                    return Some(
                        "An escape sequence starting with '\\u' must be followed by 4 hexadecimal digits.",
                    );
                }
                let mut code: u32 = 0;
                if b[i + 1] == b'{' {
                    i += 1;
                    let mut fechou = false;
                    for j in 0..7 {
                        i += 1;
                        if i == n {
                            return Some("Invalid '\\u{...}' escape.");
                        }
                        let d = b[i];
                        if j != 0 && d == b'}' {
                            fechou = true;
                            break;
                        } else if j == 6 || !hex(d) {
                            return Some("Invalid '\\u{...}' escape.");
                        }
                        code = (code << 4) + valor(d);
                    }
                    if !fechou {
                        return Some("Invalid '\\u{...}' escape.");
                    }
                } else {
                    if n <= i + 4 {
                        return Some(
                            "An escape sequence starting with '\\u' must be followed by 4 hexadecimal digits.",
                        );
                    }
                    for _ in 0..4 {
                        i += 1;
                        if !hex(b[i]) {
                            return Some(
                                "An escape sequence starting with '\\u' must be followed by 4 hexadecimal digits.",
                            );
                        }
                        code = (code << 4) + valor(b[i]);
                    }
                }
                if code > 0x10FFFF {
                    return Some("The escape sequence isn't a valid code point.");
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

impl Parser<'_> {
    /// `parseExpression`.
    pub(super) fn expressao(&mut self) -> R<NoId> {
        self.entrar()?;
        let r = self.expressao_interna();
        self.sair();
        r
    }

    fn expressao_interna(&mut self) -> R<NoId> {
        if self.padroes
            && let Some(f) = self.pular_padrao_externo(self.pos)
            && self.op(f + 1, Op::Assign)
        {
            return self.atribuicao_padrao();
        }
        if self.kw(self.pos, Keyword::Throw) {
            return self.lancar(true);
        }
        self.precedencia(prec::ATRIBUICAO, true, CtxConst::Nenhum)
    }

    /// `parseExpressionWithoutCascade`.
    pub(super) fn expressao_sem_cascata(&mut self) -> R<NoId> {
        if self.kw(self.pos, Keyword::Throw) {
            return self.lancar(false);
        }
        self.entrar()?;
        let r = self.precedencia(prec::ATRIBUICAO, false, CtxConst::Nenhum);
        self.sair();
        r
    }

    /// `parsePatternAssignment`.
    fn atribuicao_padrao(&mut self) -> R<NoId> {
        let pattern = self.padrao(CtxPadrao::Atribuicao)?;
        let ini = self.ini(pattern);
        let equals = self.espera_op(Op::Assign)?;
        let expression = self.expressao()?;
        Ok(self.no(
            No::PatternAssignment {
                pattern,
                equals: equals as Tk,
                expression,
            },
            ini,
        ))
    }

    /// `parseThrowExpression`.
    fn lancar(&mut self, cascatas: bool) -> R<NoId> {
        let ini = self.avancar();
        if self.op(self.pos, Op::Semicolon) {
            return Err(self.erro("Missing expression after 'throw'."));
        }
        let expression = if cascatas {
            self.expressao()?
        } else {
            self.expressao_sem_cascata()?
        };
        Ok(self.no(
            No::ThrowExpression {
                throw_keyword: ini as Tk,
                expression,
            },
            ini,
        ))
    }

    /// `parsePrecedenceExpression`.
    pub(super) fn precedencia(&mut self, nivel: u8, cascatas: bool, ctx: CtxConst) -> R<NoId> {
        let e = self.unario(cascatas, ctx)?;
        let (e, targs) = self.args_metodo_apos(e, ctx)?;
        self.laco_precedencia(nivel, cascatas, targs, e, ctx)
    }

    /// Argumentos de tipo de método depois de uma expressão (com `!`
    /// opcional): `e<T>(...)` deixa os argumentos pendentes para a chamada;
    /// `e<T>` sem chamada vira `FunctionReference`.
    fn args_metodo_apos(&mut self, e: NoId, ctx: CtxConst) -> R<(NoId, Option<NoId>)> {
        let ultimo = self.pos - 1;
        let bang = if self.op(self.pos, Op::Bang) {
            self.pos
        } else {
            ultimo
        };
        if self.args_metodo_em(bang).is_none() {
            return Ok((e, None));
        }
        let mut e = e;
        if bang == self.pos {
            let b = self.avancar();
            let ini = self.ini(e);
            e = self.no(
                No::PostfixExpression {
                    operand: e,
                    operator: b as Tk,
                },
                ini,
            );
        }
        let ta = self.args_tipo()?;
        if !self.op(self.pos, Op::LParen) {
            if ctx != CtxConst::Nenhum {
                return Err(self.erro_em(
                    self.ini(ta),
                    "This expression isn't a valid constant pattern.",
                ));
            }
            self.recurso(self.ini(ta), (2, 15), "constructor-tearoffs");
            let ini = self.ini(e);
            let r = self.no(
                No::FunctionReference {
                    function: e,
                    type_arguments: Some(ta),
                },
                ini,
            );
            return Ok((r, None));
        }
        Ok((e, Some(ta)))
    }

    /// Precedência do token `i` (`_computePrecedence`).
    pub(super) fn precedencia_em(&mut self, i: usize, padrao: bool) -> u8 {
        use prec::*;
        match self.tipo(i) {
            TipoToken::Op(o) => match o {
                Op::Bang => {
                    if matches!(
                        self.tipo(i + 1),
                        TipoToken::Op(
                            Op::Dot | Op::Question | Op::LParen | Op::LBracket | Op::QuestionDot
                        )
                    ) {
                        SELETOR
                    } else {
                        POSFIXO
                    }
                }
                Op::Question => {
                    if padrao || (self.op(i + 1, Op::LBracket) && !self.pode_ser_condicional(i)) {
                        SELETOR
                    } else {
                        CONDICIONAL
                    }
                }
                Op::Gt => match self.toks.maior_composto(i as Tk) {
                    Some(">>" | ">>>") => DESLOCAMENTO,
                    Some(">>=" | ">>>=") => ATRIBUICAO,
                    _ => RELACIONAL,
                },
                Op::LParen | Op::LBracket | Op::Dot | Op::QuestionDot => SELETOR,
                Op::PlusPlus | Op::MinusMinus => POSFIXO,
                Op::Tilde => PREFIXO,
                Op::Star | Op::Slash | Op::TildeSlash | Op::Percent => MULTIPLICATIVA,
                Op::Plus | Op::Minus => ADITIVA,
                Op::LtLt => DESLOCAMENTO,
                Op::Amp => BIT_E,
                Op::Caret => BIT_XOU,
                Op::Pipe => BIT_OU,
                Op::Lt | Op::LtEq => RELACIONAL,
                Op::EqEq | Op::BangEq => IGUALDADE,
                Op::AmpAmp => E,
                Op::PipePipe => OU,
                Op::QuestionQuestion => SE_NULO,
                Op::DotDot | Op::QuestionDotDot => CASCATA,
                Op::Assign
                | Op::PlusAssign
                | Op::MinusAssign
                | Op::StarAssign
                | Op::SlashAssign
                | Op::TildeSlashAssign
                | Op::PercentAssign
                | Op::LtLtAssign
                | Op::AmpAssign
                | Op::PipeAssign
                | Op::CaretAssign
                | Op::QuestionQuestionAssign => ATRIBUICAO,
                _ => 0,
            },
            TipoToken::Reservada(Keyword::Is) => RELACIONAL,
            TipoToken::Identificador if self.txt(i) == "as" => RELACIONAL,
            _ => 0,
        }
    }

    /// Operador de atribuição em `i` e quantos tokens ocupa.
    fn op_atribuicao_em(&self, i: usize) -> Option<usize> {
        match self.tipo(i) {
            TipoToken::Op(
                Op::Assign
                | Op::PlusAssign
                | Op::MinusAssign
                | Op::StarAssign
                | Op::SlashAssign
                | Op::TildeSlashAssign
                | Op::PercentAssign
                | Op::LtLtAssign
                | Op::AmpAssign
                | Op::PipeAssign
                | Op::CaretAssign
                | Op::QuestionQuestionAssign,
            ) => Some(1),
            TipoToken::Op(Op::Gt) => match self.toks.maior_composto(i as Tk) {
                Some(">>=") => Some(3),
                Some(">>>=") => Some(4),
                _ => None,
            },
            _ => None,
        }
    }

    /// Consome o operador composto ou simples em `self.pos`.
    fn consumir_operador(&mut self) -> usize {
        let i = self.pos;
        if self.op(i, Op::Gt)
            && let Some(a) = self.toks.maior_composto(i as Tk)
        {
            self.fundir_maior(i, a);
            self.pos += a.len();
            return i;
        }
        self.pos += 1;
        i
    }

    /// `canParseAsConditional`: `?` em `q` inicia um condicional válido.
    pub(super) fn pode_ser_condicional(&mut self, q: usize) -> bool {
        self.sondar(|p| {
            p.pos = q + 1;
            p.expressao_sem_cascata()?;
            if !p.op(p.pos, Op::Colon) {
                return Err(p.erro("Expected ':'."));
            }
            p.pos += 1;
            p.expressao_sem_cascata()
        })
        .is_some()
    }

    /// `isAssignable` do analyzer.
    fn atribuivel(&self, e: NoId) -> bool {
        matches!(
            self.arv.no(e),
            No::SimpleIdentifier { .. }
                | No::PrefixedIdentifier { .. }
                | No::PropertyAccess { .. }
                | No::IndexExpression { .. }
        )
    }

    /// `reportErrorIfSuper`: `super` sozinho como operando.
    pub(super) fn sem_super(&self, e: NoId) -> R<()> {
        if matches!(self.arv.no(e), No::SuperExpression { .. }) {
            return Err(self.erro_em(
                self.ini(e),
                "Missing selector such as '.identifier' or '[0]'.",
            ));
        }
        Ok(())
    }

    fn atribuicao(&mut self, lhs: NoId, operador: usize, rhs: NoId) -> R<NoId> {
        self.sem_super(rhs)?;
        if !self.atribuivel(lhs) {
            return Err(self.erro_em(
                self.ini(lhs),
                "Missing selector such as '.identifier' or '[0]'.",
            ));
        }
        let ini = self.ini(lhs);
        Ok(self.no(
            No::AssignmentExpression {
                left_hand_side: lhs,
                operator: operador as Tk,
                right_hand_side: rhs,
            },
            ini,
        ))
    }

    /// `_parsePrecedenceExpressionLoop`.
    fn laco_precedencia(
        &mut self,
        nivel_min: u8,
        cascatas: bool,
        mut targs: Option<NoId>,
        mut e: NoId,
        ctx: CtxConst,
    ) -> R<NoId> {
        let mut ctx = ctx;
        let mut nivel_token = self.precedencia_em(self.pos, false);
        if ctx != CtxConst::Nenhum {
            if self.op(self.pos, Op::Bang) {
                if nivel_token == prec::POSFIXO || self.op(self.pos + 1, Op::Question) {
                    return Ok(e);
                }
            } else if self.pal(self.pos, "as") {
                return Ok(e);
            }
            if nivel_min <= nivel_token && nivel_token < prec::SELETOR {
                if ctx == CtxConst::Explicito || nivel_token <= prec::MULTIPLICATIVA {
                    return Err(self.erro("This expression isn't a valid constant pattern."));
                }
                ctx = CtxConst::Nenhum;
            }
        }
        let mut nivel = nivel_token as i32;
        while nivel >= nivel_min as i32 {
            let mut ultimo_binario: i32 = -1;
            let mut ultima_cascata = false;
            while nivel_token as i32 == nivel {
                let operador = self.pos;
                if nivel_token == prec::CASCATA {
                    if !cascatas {
                        return Ok(e);
                    }
                    if ultima_cascata && self.op(self.pos, Op::QuestionDotDot) {
                        return Err(self.erro(
                            "The '?..' cascade operator must be first in the cascade sequence.",
                        ));
                    }
                    ultima_cascata = true;
                    e = self.cascata(e)?;
                } else if nivel_token == prec::ATRIBUICAO {
                    let op_tok = self.consumir_operador();
                    let rhs = if self.kw(self.pos, Keyword::Throw) {
                        self.lancar(false)?
                    } else {
                        self.precedencia(nivel as u8, cascatas, CtxConst::Nenhum)?
                    };
                    e = self.atribuicao(e, op_tok, rhs)?;
                } else if nivel_token == prec::POSFIXO {
                    if self.op(self.pos, Op::PlusPlus)
                        || self.op(self.pos, Op::MinusMinus)
                        || self.op(self.pos, Op::Bang)
                    {
                        if !self.op(self.pos, Op::Bang) && !self.atribuivel(e) {
                            return Err(self.erro_em(
                                self.ini(e),
                                "Illegal assignment to non-assignable expression.",
                            ));
                        }
                        let o = self.avancar();
                        let ini = self.ini(e);
                        e = self.no(
                            No::PostfixExpression {
                                operand: e,
                                operator: o as Tk,
                            },
                            ini,
                        );
                    }
                } else if nivel_token == prec::SELETOR {
                    if self.op(self.pos, Op::Dot) || self.op(self.pos, Op::QuestionDot) {
                        let ponto = self.avancar();
                        let direita = self.primario_continuacao(ctx)?;
                        e = self.ponto(Some(e), ponto, direita)?;
                        let (e2, t2) = self.args_metodo_apos(e, ctx)?;
                        e = e2;
                        targs = t2;
                    } else if self.op(self.pos, Op::LParen) || self.op(self.pos, Op::LBracket) {
                        e = self.args_ou_indice(e, targs.take(), false)?;
                    } else if self.op(self.pos, Op::Question) {
                        e = self.args_ou_indice(e, targs.take(), true)?;
                    } else if self.op(self.pos, Op::Bang) {
                        let o = self.avancar();
                        let ini = self.ini(e);
                        e = self.no(
                            No::PostfixExpression {
                                operand: e,
                                operator: o as Tk,
                            },
                            ini,
                        );
                    } else {
                        return Err(self.erro("Unexpected token."));
                    }
                } else if self.kw(self.pos, Keyword::Is) {
                    e = self.is_resto(e)?;
                } else if self.pal(self.pos, "as") {
                    e = self.as_resto(e)?;
                } else if self.op(self.pos, Op::Question) {
                    e = self.condicional_resto(e)?;
                } else {
                    if nivel == prec::IGUALDADE as i32 || nivel == prec::RELACIONAL as i32 {
                        if ultimo_binario == nivel {
                            return Err(self.erro("An equality expression can't be an operand of another equality expression."));
                        }
                        ultimo_binario = nivel;
                    }
                    let o = self.consumir_operador();
                    let _ = operador;
                    let direita =
                        self.precedencia((nivel + 1) as u8, cascatas, CtxConst::Nenhum)?;
                    self.sem_super(direita)?;
                    let ini = self.ini(e);
                    e = self.no(
                        No::BinaryExpression {
                            left_operand: e,
                            operator: o as Tk,
                            right_operand: direita,
                        },
                        ini,
                    );
                }
                nivel_token = self.precedencia_em(self.pos, false);
                if ctx != CtxConst::Nenhum {
                    if self.op(self.pos, Op::Bang) {
                        if nivel_token == prec::POSFIXO || self.op(self.pos + 1, Op::Question) {
                            return Ok(e);
                        }
                    } else if self.pal(self.pos, "as") {
                        return Ok(e);
                    }
                }
            }
            nivel -= 1;
        }
        Ok(e)
    }

    /// `computeTypeAfterIsOrAs` seguido do tipo.
    pub(super) fn tipo_apos_is_as(&mut self) -> R<NoId> {
        let i = self.pos;
        let mut ti = self.tipo_em(i, true, false, false);
        if ti.e_nenhum() {
            return Err(self.erro(&format!("Expected a type, but got '{}'.", self.txt(i))));
        }
        if ti.nulavel() {
            let nx = ti.ate;
            let fim_ok = self.eof(nx)
                || matches!(
                    self.tipo(nx),
                    TipoToken::Op(
                        Op::RParen
                            | Op::RBrace
                            | Op::RBracket
                            | Op::Question
                            | Op::QuestionQuestion
                            | Op::Comma
                            | Op::Semicolon
                            | Op::Colon
                            | Op::DotDot
                            | Op::PipePipe
                            | Op::AmpAmp
                    )
                )
                || self.kw(nx, Keyword::Is)
                || self.pal(nx, "as");
            if !fim_ok {
                let manter = (self.op(nx, Op::LBrace) || self.pal(nx, "when"))
                    && !self.pode_ser_condicional(nx - 1);
                if !manter {
                    ti = ti.sem_interrogacao();
                }
            }
        }
        if self.kw(i, Keyword::Void) && ti.ate == i + 1 {
            return Err(self.erro_em(i, "The type 'void' can't be used here."));
        }
        self.tipo_ate(ti.ate)
    }

    fn sem_is_as_encadeado(&self) -> R<()> {
        if self.kw(self.pos, Keyword::Is) || self.pal(self.pos, "as") {
            return Err(self.erro("Unexpected token."));
        }
        Ok(())
    }

    /// `parseIsOperatorRest`.
    fn is_resto(&mut self, e: NoId) -> R<NoId> {
        let is = self.avancar();
        let not = self.opcional_op(Op::Bang).map(|b| b as Tk);
        let type_ = self.tipo_apos_is_as()?;
        let ini = self.ini(e);
        let r = self.no(
            No::IsExpression {
                expression: e,
                is_operator: is as Tk,
                not_operator: not,
                type_,
            },
            ini,
        );
        self.sem_is_as_encadeado()?;
        Ok(r)
    }

    /// `parseAsOperatorRest`.
    fn as_resto(&mut self, e: NoId) -> R<NoId> {
        let a = self.avancar();
        let type_ = self.tipo_apos_is_as()?;
        let ini = self.ini(e);
        let r = self.no(
            No::AsExpression {
                expression: e,
                as_operator: a as Tk,
                type_,
            },
            ini,
        );
        self.sem_is_as_encadeado()?;
        Ok(r)
    }

    /// `parseConditionalExpressionRest`.
    fn condicional_resto(&mut self, c: NoId) -> R<NoId> {
        let q = self.avancar();
        let then_expression = self.expressao_sem_cascata()?;
        let colon = self.espera_op(Op::Colon)?;
        let else_expression = self.expressao_sem_cascata()?;
        let ini = self.ini(c);
        Ok(self.no(
            No::ConditionalExpression {
                condition: c,
                question: q as Tk,
                then_expression,
                colon: colon as Tk,
                else_expression,
            },
            ini,
        ))
    }

    /// `parseCascadeExpression`.
    fn cascata(&mut self, alvo: NoId) -> R<NoId> {
        let op_c = self.avancar();
        let mut sec = if self.op(self.pos, Op::LBracket) {
            let lb = self.avancar();
            let old = self.funcoes;
            self.funcoes = true;
            let index = self.expressao()?;
            self.funcoes = old;
            let rb = self.espera_op(Op::RBracket)?;
            let s = self.no(
                No::IndexExpression {
                    target: None,
                    period: Some(op_c as Tk),
                    question: None,
                    left_bracket: lb as Tk,
                    index,
                    right_bracket: rb as Tk,
                },
                op_c,
            );
            let (s, t) = self.args_metodo_apos(s, CtxConst::Nenhum)?;
            self.args_ou_indice(s, t, false)?
        } else {
            let d = self.enviar(CtxConst::Nenhum, true)?;
            self.ponto(None, op_c, d)?
        };
        loop {
            let marca = self.pos;
            if self.op(self.pos, Op::Dot) || self.op(self.pos, Op::QuestionDot) {
                let p = self.avancar();
                let d = self.enviar(CtxConst::Nenhum, true)?;
                sec = self.ponto(Some(sec), p, d)?;
            } else if self.op(self.pos, Op::Bang) {
                let b = self.avancar();
                let ini = self.ini(sec);
                sec = self.no(
                    No::PostfixExpression {
                        operand: sec,
                        operator: b as Tk,
                    },
                    ini,
                );
            }
            let mut targs = None;
            if self.args_metodo_em(self.pos - 1).is_some() {
                let ta = self.args_tipo()?;
                if !self.op(self.pos, Op::LParen) {
                    self.recurso(self.ini(ta), (2, 15), "constructor-tearoffs");
                    let ini = self.ini(sec);
                    sec = self.no(
                        No::FunctionReference {
                            function: sec,
                            type_arguments: Some(ta),
                        },
                        ini,
                    );
                } else {
                    targs = Some(ta);
                }
            }
            sec = self.args_ou_indice(sec, targs, false)?;
            if self.pos == marca {
                break;
            }
        }
        if self.op_atribuicao_em(self.pos).is_some() {
            let o = self.consumir_operador();
            let rhs = self.expressao_sem_cascata()?;
            sec = self.atribuicao(sec, o, rhs)?;
        }
        let fim = self.fim(sec);
        if let No::CascadeExpression {
            cascade_sections, ..
        } = &mut self.arv.nos[alvo.0 as usize].no
        {
            cascade_sections.push(sec);
            self.arv.nos[alvo.0 as usize].fim = fim as Tk;
            return Ok(alvo);
        }
        let ini = self.ini(alvo);
        Ok(self.no(
            No::CascadeExpression {
                target: alvo,
                cascade_sections: vec![sec],
            },
            ini,
        ))
    }

    /// `parseArgumentOrIndexStar`.
    fn args_ou_indice(&mut self, e: NoId, targs: Option<NoId>, verificado: bool) -> R<NoId> {
        let mut e = e;
        let mut targs = targs;
        loop {
            let mut potencial =
                self.op(self.pos, Op::Question) && self.op(self.pos + 1, Op::LBracket);
            if potencial && !verificado && self.pode_ser_condicional(self.pos) {
                potencial = false;
            }
            if self.op(self.pos, Op::LBracket) || potencial {
                let question = if self.op(self.pos, Op::Question) {
                    Some(self.avancar() as Tk)
                } else {
                    None
                };
                let lb = self.avancar();
                let old = self.funcoes;
                self.funcoes = true;
                let index = self.expressao()?;
                self.funcoes = old;
                let rb = self.espera_op(Op::RBracket)?;
                let ini = self.ini(e);
                e = self.no(
                    No::IndexExpression {
                        target: Some(e),
                        period: None,
                        question,
                        left_bracket: lb as Tk,
                        index,
                        right_bracket: rb as Tk,
                    },
                    ini,
                );
                let (e2, t2) = self.args_metodo_apos(e, CtxConst::Nenhum)?;
                e = e2;
                targs = t2;
            } else if self.op(self.pos, Op::LParen) {
                let args = self.argumentos()?;
                e = self.invocacao(e, targs.take(), args);
                let (e2, t2) = self.args_metodo_apos(e, CtxConst::Nenhum)?;
                e = e2;
                targs = t2;
            } else {
                break;
            }
        }
        Ok(e)
    }

    /// `doInvocation`.
    fn invocacao(&mut self, receptor: NoId, targs: Option<NoId>, args: NoId) -> NoId {
        let ini = self.ini(receptor);
        if self.e_simples(receptor) {
            return self.no(
                No::MethodInvocation {
                    target: None,
                    operator: None,
                    method_name: receptor,
                    type_arguments: targs,
                    argument_list: args,
                },
                ini,
            );
        }
        self.no(
            No::FunctionExpressionInvocation {
                function: receptor,
                type_arguments: targs,
                argument_list: args,
            },
            ini,
        )
    }

    /// `doDotExpression`: `receptor` é `None` numa seção de cascata.
    fn ponto(&mut self, receptor: Option<NoId>, ponto: usize, direita: NoId) -> R<NoId> {
        let ini = receptor.map_or(ponto, |r| self.ini(r));
        match self.arv.no(direita) {
            No::SimpleIdentifier { .. } => {
                if let Some(r) = receptor
                    && self.e_simples(r)
                    && self.op(ponto, Op::Dot)
                {
                    return Ok(self.no(
                        No::PrefixedIdentifier {
                            prefix: r,
                            period: ponto as Tk,
                            identifier: direita,
                        },
                        ini,
                    ));
                }
                Ok(self.no(
                    No::PropertyAccess {
                        target: receptor,
                        operator: ponto as Tk,
                        property_name: direita,
                    },
                    ini,
                ))
            }
            No::MethodInvocation { target: None, .. } => {
                let d = &mut self.arv.nos[direita.0 as usize];
                if let No::MethodInvocation {
                    target, operator, ..
                } = &mut d.no
                {
                    *target = receptor;
                    *operator = Some(ponto as Tk);
                }
                d.inicio = ini as Tk;
                Ok(direita)
            }
            _ => Err(self.erro_em(self.ini(direita), "Expected an identifier.")),
        }
    }

    /// `parseUnaryExpression`.
    fn unario(&mut self, cascatas: bool, ctx: CtxConst) -> R<NoId> {
        let i = self.pos;
        if self.pal(i, "await") {
            // Fora de corpo `async`, o fasta relata `AwaitNotAsync`, que no
            // analyzer não é erro sintático, e monta o `AwaitExpression`.
            if self.assinc == Assinc::Sinc && !self.parece_expressao_apos(i, true) {
                return self.primario(ctx);
            }
            self.avancar();
            let expression = self.precedencia(prec::POSFIXO, cascatas, CtxConst::Nenhum)?;
            return Ok(self.no(
                No::AwaitExpression {
                    await_keyword: i as Tk,
                    expression,
                },
                i,
            ));
        }
        match self.tipo(i) {
            TipoToken::Op(Op::Plus) => return Err(self.erro("'+' is not a prefix operator.")),
            TipoToken::Op(Op::Bang | Op::Tilde) => {
                if ctx != CtxConst::Nenhum {
                    return Err(self.erro("This expression isn't a valid constant pattern."));
                }
                self.avancar();
                let operand = self.precedencia(prec::POSFIXO, cascatas, CtxConst::Nenhum)?;
                return Ok(self.no(
                    No::PrefixExpression {
                        operator: i as Tk,
                        operand,
                    },
                    i,
                ));
            }
            TipoToken::Op(Op::Minus) => {
                if ctx == CtxConst::Explicito {
                    return Err(self.erro("The expression can't be prefixed by 'const'."));
                }
                self.avancar();
                let c = if ctx != CtxConst::Nenhum {
                    CtxConst::SoNumerico
                } else {
                    CtxConst::Nenhum
                };
                let operand = self.precedencia(prec::POSFIXO, cascatas, c)?;
                return Ok(self.no(
                    No::PrefixExpression {
                        operator: i as Tk,
                        operand,
                    },
                    i,
                ));
            }
            TipoToken::Op(Op::PlusPlus | Op::MinusMinus) => {
                self.avancar();
                let operand = self.precedencia(prec::POSFIXO, cascatas, CtxConst::Nenhum)?;
                if !self.atribuivel(operand) {
                    return Err(self.erro_em(
                        self.ini(operand),
                        "Illegal assignment to non-assignable expression.",
                    ));
                }
                return Ok(self.no(
                    No::PrefixExpression {
                        operator: i as Tk,
                        operand,
                    },
                    i,
                ));
            }
            _ => {}
        }
        if self.ident(i) {
            let mut id = i;
            if self.op(id + 1, Op::Dot) {
                id += 2;
            }
            if self.ident(id)
                && self.op(id + 1, Op::Lt)
                && let Some(a) = self.args_tipo_em(id, false)
                && self.op(a.ate, Op::Dot)
                && (self.ident(a.ate + 1) || self.kw(a.ate + 1, Keyword::New))
                && self.op(a.ate + 2, Op::LParen)
            {
                return self.criacao(None);
            }
        }
        self.primario(ctx)
    }

    /// `parsePrimary` no contexto de expressão.
    fn primario(&mut self, ctx: CtxConst) -> R<NoId> {
        let i = self.pos;
        let explicito = ctx == CtxConst::Explicito;
        let numerico = ctx == CtxConst::SoNumerico;
        let invalido = |p: &Self| Err(p.erro("This expression isn't a valid constant pattern."));
        match self.tipo(i) {
            TipoToken::Identificador => {
                if numerico {
                    return invalido(self);
                }
                self.enviar_ou_funcao(ctx)
            }
            TipoToken::Inteiro => {
                if explicito {
                    return invalido(self);
                }
                self.conferir_numero(i)?;
                self.avancar();
                Ok(self.no(No::IntegerLiteral { literal: i as Tk }, i))
            }
            TipoToken::Duplo => {
                if explicito {
                    return invalido(self);
                }
                self.conferir_numero(i)?;
                self.avancar();
                Ok(self.no(No::DoubleLiteral { literal: i as Tk }, i))
            }
            TipoToken::Texto => {
                if explicito || numerico {
                    return invalido(self);
                }
                self.literal_texto()
            }
            TipoToken::Op(Op::Hash) => {
                if explicito || numerico {
                    return invalido(self);
                }
                self.simbolo()
            }
            TipoToken::Reservada(k) => match k {
                Keyword::True | Keyword::False => {
                    if explicito || numerico {
                        return invalido(self);
                    }
                    self.avancar();
                    Ok(self.no(No::BooleanLiteral { literal: i as Tk }, i))
                }
                Keyword::Null => {
                    if explicito || numerico {
                        return invalido(self);
                    }
                    self.avancar();
                    Ok(self.no(No::NullLiteral { literal: i as Tk }, i))
                }
                Keyword::This => self.este_ou_super(true),
                Keyword::Super => self.este_ou_super(false),
                Keyword::New => self.criacao(Some(i)),
                Keyword::Const => {
                    if explicito {
                        return Err(self.erro("Duplicate 'const' keyword."));
                    }
                    self.expressao_const()
                }
                Keyword::Switch if self.padroes => self.expressao_switch(),
                Keyword::Assert => self.assert_expressao(),
                _ => Err(self.erro(&format!(
                    "Expected an expression, but got '{}'.",
                    self.txt(i)
                ))),
            },
            TipoToken::Op(Op::LParen) => self.parenteses_funcao_ou_registro(ctx),
            TipoToken::Op(Op::LBracket) => self.lista(None, None),
            TipoToken::Op(Op::LBrace) => self.conjunto_ou_mapa(None, None),
            TipoToken::Op(Op::Lt) => self.colecao_ou_funcao_generica(None),
            _ => Err(self.erro(&format!(
                "Expected an expression, but got '{}'.",
                self.txt(i)
            ))),
        }
    }

    /// `parsePrimary` depois de `.`/`?.` (`expressionContinuation`).
    fn primario_continuacao(&mut self, ctx: CtxConst) -> R<NoId> {
        if self.ident(self.pos) {
            return self.enviar_ou_funcao(ctx);
        }
        if self.kw(self.pos, Keyword::New) {
            return self.enviar(ctx, true);
        }
        Err(self.erro(&format!(
            "Expected an identifier, but got '{}'.",
            self.txt(self.pos)
        )))
    }

    /// `parseSendOrFunctionLiteral`.
    fn enviar_ou_funcao(&mut self, ctx: CtxConst) -> R<NoId> {
        if self.funcoes {
            let ti = self.tipo_em(self.pos, false, false, false);
            let nome = ti.ate;
            if self.ident(nome) {
                let nx = self.args_tipo_em(nome, false).map_or(nome + 1, |a| a.ate);
                if self.op(nx, Op::LParen) && self.parece_corpo(self.par(nx) + 1) {
                    return Err(self.erro_em(nome, "A function expression can't have a name."));
                }
            }
        }
        self.enviar(ctx, false)
    }

    /// `parseSend`.
    fn enviar(&mut self, ctx: CtxConst, permite_new: bool) -> R<NoId> {
        let i = self.pos;
        if !(self.ident(i) || (permite_new && self.kw(i, Keyword::New))) {
            return Err(self.erro(&format!(
                "Expected an identifier, but got '{}'.",
                self.txt(i)
            )));
        }
        if self.ident(i) {
            if self.txt(i) == "await" && self.ident(i + 1) {
                return Err(self.erro_em(i, "Unexpected 'await'."));
            }
            self.conferir_await_yield(i)?;
        }
        self.avancar();
        let tp = self.args_tipo_em(i, false);
        let depois = tp.map_or(i + 1, |a| a.ate);
        let targs = if self.op(depois, Op::LParen) && tp.is_some_and(|a| !a.recuperado) {
            Some(self.args_tipo()?)
        } else {
            None
        };
        if ctx == CtxConst::Explicito
            && !(self.op(self.pos, Op::Dot)
                || self.op(self.pos, Op::LParen)
                || self.op(self.pos, Op::Lt))
        {
            return Err(self.erro_em(i, "The expression can't be prefixed by 'const'."));
        }
        let nome = self.simples(i);
        if self.op(self.pos, Op::LParen) {
            let args = self.argumentos()?;
            return Ok(self.no(
                No::MethodInvocation {
                    target: None,
                    operator: None,
                    method_name: nome,
                    type_arguments: targs,
                    argument_list: args,
                },
                i,
            ));
        }
        Ok(nome)
    }

    /// `parseThisExpression`/`parseSuperExpression`.
    fn este_ou_super(&mut self, este: bool) -> R<NoId> {
        let i = self.avancar();
        let mut e = if este {
            self.no(
                No::ThisExpression {
                    this_keyword: i as Tk,
                },
                i,
            )
        } else {
            self.no(
                No::SuperExpression {
                    super_keyword: i as Tk,
                },
                i,
            )
        };
        if self.op(self.pos, Op::LParen) {
            let args = self.argumentos()?;
            e = self.no(
                No::FunctionExpressionInvocation {
                    function: e,
                    type_arguments: None,
                    argument_list: args,
                },
                i,
            );
        } else if !este && self.op(self.pos, Op::QuestionDot) {
            return Err(self.erro("The operator '?.' cannot be used with 'super'."));
        }
        Ok(e)
    }

    /// `assert(...)` como expressão: o `AstBuilder` monta uma invocação de
    /// função chamada `assert` (o erro `AssertAsExpression` não tem código
    /// no analyzer).
    fn assert_expressao(&mut self) -> R<NoId> {
        let (a, lp, condition, _comma, message, rp) = self.partes_assert()?;
        let function = self.simples(a);
        let mut arguments = vec![condition];
        arguments.extend(message);
        let argument_list = self.no(
            No::ArgumentList {
                left_parenthesis: lp as Tk,
                arguments,
                right_parenthesis: rp as Tk,
            },
            lp,
        );
        Ok(self.no(
            No::FunctionExpressionInvocation {
                function,
                type_arguments: None,
                argument_list,
            },
            a,
        ))
    }

    /// `parseArgumentsRest`.
    pub(super) fn argumentos(&mut self) -> R<NoId> {
        let ini = self.pos;
        let lp = self.espera_op(Op::LParen)?;
        let old = self.funcoes;
        self.funcoes = true;
        let mut args = Vec::new();
        let mut viu_nomeado = false;
        loop {
            if self.op(self.pos, Op::RParen) {
                break;
            }
            if self.op(self.pos + 1, Op::Colon) {
                viu_nomeado = true;
                args.push(self.expressao_nomeada()?);
            } else {
                if viu_nomeado && self.versao < (2, 17) {
                    return Err(
                        self.erro("Positional arguments must occur before named arguments.")
                    );
                }
                args.push(self.expressao()?);
            }
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.avancar();
        }
        let rp = self.espera_op(Op::RParen)?;
        self.funcoes = old;
        Ok(self.no(
            No::ArgumentList {
                left_parenthesis: lp as Tk,
                arguments: args,
                right_parenthesis: rp as Tk,
            },
            ini,
        ))
    }

    /// `nome: expressão` (argumento nomeado ou campo de record).
    fn expressao_nomeada(&mut self) -> R<NoId> {
        let ini = self.pos;
        let n = self.espera_ident()?;
        let label = self.simples(n);
        let colon = self.espera_op(Op::Colon)?;
        let l = self.no(
            No::Label {
                label,
                colon: colon as Tk,
            },
            ini,
        );
        let expression = self.expressao()?;
        Ok(self.no(
            No::NamedExpression {
                name: l,
                expression,
            },
            ini,
        ))
    }

    /// `parseNewExpression`, `parseImplicitCreationExpression` e o fim de
    /// `parseConstExpression`: `palavra? ConstructorName argumentos`.
    fn criacao(&mut self, palavra: Option<usize>) -> R<NoId> {
        let ini = palavra.unwrap_or(self.pos);
        if palavra.is_some() {
            self.avancar();
        }
        let constructor_name = self.nome_construtor()?;
        if !self.op(self.pos, Op::LParen) {
            return Err(self.esperado("("));
        }
        let argument_list = self.argumentos()?;
        Ok(self.no(
            No::InstanceCreationExpression {
                keyword: palavra.map(|p| p as Tk),
                constructor_name,
                argument_list,
            },
            ini,
        ))
    }

    /// `parseConstructorReference`.
    pub(super) fn nome_construtor(&mut self) -> R<NoId> {
        let ini = self.pos;
        let a = self.espera_ident()?;
        let mut prefixo = None;
        let mut nome = a;
        if self.op(self.pos, Op::Dot) {
            let d = self.avancar();
            if !(self.ident(self.pos) || self.kw(self.pos, Keyword::New)) {
                return Err(self.erro("Expected an identifier."));
            }
            nome = self.avancar();
            prefixo = Some((a as Tk, d as Tk));
        }
        let type_arguments = if self.op(self.pos, Op::Lt) {
            if self.args_tipo_lt(self.pos, false).is_none() {
                return Err(self.erro("Invalid type arguments."));
            }
            Some(self.args_tipo()?)
        } else {
            None
        };
        let tipo = self.no(
            No::NamedType {
                import_prefix: prefixo,
                name2: nome as Tk,
                type_arguments,
                question: None,
            },
            ini,
        );
        let mut period = None;
        let mut name = None;
        if self.op(self.pos, Op::Dot) {
            period = Some(self.avancar() as Tk);
            if !(self.ident(self.pos) || self.kw(self.pos, Keyword::New)) {
                return Err(self.erro("Expected an identifier."));
            }
            let n = self.avancar();
            name = Some(self.simples(n));
        }
        Ok(self.no(
            No::ConstructorName {
                type_: tipo,
                period,
                name,
            },
            ini,
        ))
    }

    /// `parseConstExpression`.
    fn expressao_const(&mut self) -> R<NoId> {
        let c = self.pos;
        match self.tipo(c + 1) {
            TipoToken::Op(Op::LBracket) => {
                self.avancar();
                self.lista(Some(c), None)
            }
            TipoToken::Op(Op::LParen) => {
                self.avancar();
                self.parenteses_ou_registro(Some(c), CtxConst::Nenhum)
            }
            TipoToken::Op(Op::LBrace) => {
                self.avancar();
                self.conjunto_ou_mapa(Some(c), None)
            }
            TipoToken::Op(Op::Lt) => {
                self.avancar();
                self.colecao_ou_funcao_generica(Some(c))
            }
            _ => self.criacao(Some(c)),
        }
    }

    /// `parseLiteralListSetMapOrFunction`.
    fn colecao_ou_funcao_generica(&mut self, constante: Option<usize>) -> R<NoId> {
        let lt = self.pos;
        let Some(info) = self.args_tipo_lt(lt, true) else {
            return Err(self.erro("Expected type arguments."));
        };
        if self.op(info.ate, Op::LParen) {
            if constante.is_some() {
                return Err(self.erro_em(constante.unwrap_or(lt), "Unexpected 'const'."));
            }
            let tp = self.params_tipo()?;
            let d = self.par(self.pos) + 1;
            if !(self.op(d, Op::Arrow)
                || self.op(d, Op::LBrace)
                || self.pal(d, "async")
                || self.pal(d, "sync"))
            {
                return Err(self.erro_em(d, "Unexpected token."));
            }
            return self.funcao_expressao(Some(tp), lt);
        }
        let ta = self.args_tipo()?;
        if self.op(self.pos, Op::LBrace) {
            return self.conjunto_ou_mapa(constante, Some(ta));
        }
        if self.op(self.pos, Op::LBracket) {
            return self.lista(constante, Some(ta));
        }
        Err(self.esperado("["))
    }

    /// Expressão de função (`parseFunctionExpression`) a partir dos
    /// parâmetros formais.
    fn funcao_expressao(&mut self, type_parameters: Option<NoId>, ini: usize) -> R<NoId> {
        let parameters = self.params_formais(Membro::Local)?;
        let body = self.corpo_funcao(true, false)?;
        Ok(self.no(
            No::FunctionExpression {
                type_parameters,
                parameters: Some(parameters),
                body,
            },
            ini,
        ))
    }

    /// `parseParenthesizedExpressionFunctionLiteralOrRecordLiteral`.
    fn parenteses_funcao_ou_registro(&mut self, ctx: CtxConst) -> R<NoId> {
        let abre = self.pos;
        if self.funcoes {
            let nx = self.par(abre) + 1;
            let funcao = if self.op(nx, Op::Arrow) || self.op(nx, Op::LBrace) {
                true
            } else if self.palavra(nx) {
                self.pal(nx, "async")
                    || self.pal(nx, "sync")
                    || self.op(nx + 1, Op::Arrow)
                    || self.op(nx + 1, Op::LBrace)
            } else {
                false
            };
            if funcao {
                return self.funcao_expressao(None, abre);
            }
        }
        let old = self.funcoes;
        self.funcoes = true;
        let r = self.parenteses_ou_registro(None, ctx);
        self.funcoes = old;
        r
    }

    /// `parseParenthesizedExpressionOrRecordLiteral`.
    fn parenteses_ou_registro(&mut self, constante: Option<usize>, ctx: CtxConst) -> R<NoId> {
        let ini = constante.unwrap_or(self.pos);
        let lp = self.espera_op(Op::LParen)?;
        let mut campos = Vec::new();
        let mut registro = constante.is_some();
        let mut registro_valido = false;
        let mut virgula_ilegal = false;
        loop {
            if self.op(self.pos, Op::RParen) {
                if campos.is_empty() {
                    registro = true;
                }
                break;
            }
            if campos.is_empty()
                && self.op(self.pos, Op::Comma)
                && self.op(self.pos + 1, Op::RParen)
            {
                virgula_ilegal = true;
                registro = true;
                self.avancar();
                break;
            }
            if self.op(self.pos + 1, Op::Colon) {
                registro = true;
                registro_valido = true;
                campos.push(self.expressao_nomeada()?);
            } else {
                campos.push(self.expressao()?);
            }
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.avancar();
            registro = true;
            registro_valido = true;
        }
        let rp = self.espera_op(Op::RParen)?;
        if registro {
            if self.versao < (3, 0) {
                return Err(self.erro_em(
                    lp,
                    "This requires the 'records' language feature to be enabled.",
                ));
            }
            if campos.is_empty() && virgula_ilegal {
                return Err(self.erro_em(
                    rp,
                    "A record literal without fields can't have a trailing comma.",
                ));
            }
            if campos.len() == 1 && !registro_valido {
                return Err(self.erro_em(
                    rp,
                    "A record literal with exactly one positional field requires a trailing comma.",
                ));
            }
            if campos.is_empty() && ctx != CtxConst::Nenhum {
                return Err(self.erro_em(
                    lp,
                    "The empty record literal is not supported as a constant pattern.",
                ));
            }
            return Ok(self.no(
                No::RecordLiteral {
                    const_keyword: constante.map(|c| c as Tk),
                    left_parenthesis: lp as Tk,
                    fields: campos,
                    right_parenthesis: rp as Tk,
                },
                ini,
            ));
        }
        let expression = campos[0];
        Ok(self.no(
            No::ParenthesizedExpression {
                left_parenthesis: lp as Tk,
                expression,
                right_parenthesis: rp as Tk,
            },
            ini,
        ))
    }

    /// `parseLiteralString`: literais adjacentes viram `AdjacentStrings`.
    pub(super) fn literal_texto(&mut self) -> R<NoId> {
        let ini = self.pos;
        let old = self.funcoes;
        self.funcoes = true;
        let mut partes = vec![self.texto_unico()?];
        while self.texto(self.pos) {
            partes.push(self.texto_unico()?);
        }
        self.funcoes = old;
        if partes.len() == 1 {
            return Ok(partes[0]);
        }
        Ok(self.no(No::AdjacentStrings { strings: partes }, ini))
    }

    /// `parseSingleLiteralString`.
    fn texto_unico(&mut self) -> R<NoId> {
        let ini = self.pos;
        if !self.texto(ini) {
            return Err(self.erro("Expected a string literal."));
        }
        self.avancar();
        let cru = self.txt(ini).starts_with('r');
        let unico = !matches!(
            self.tipo(self.pos),
            TipoToken::InterpolacaoExpressao | TipoToken::InterpolacaoIdentificador
        );
        if !cru {
            self.conferir_escapes(ini, true, unico)?;
        }
        if unico {
            return Ok(self.no(No::SimpleStringLiteral { literal: ini as Tk }, ini));
        }
        let primeiro = self.no(
            No::InterpolationString {
                contents: ini as Tk,
            },
            ini,
        );
        let mut elementos = vec![primeiro];
        loop {
            let lb = self.pos;
            match self.tipo(lb) {
                TipoToken::InterpolacaoExpressao => {
                    self.avancar();
                    let expression = self.expressao()?;
                    let rb = self.espera_op(Op::RBrace)?;
                    elementos.push(self.no(
                        No::InterpolationExpression {
                            left_bracket: lb as Tk,
                            expression,
                            right_bracket: Some(rb as Tk),
                        },
                        lb,
                    ));
                }
                TipoToken::InterpolacaoIdentificador => {
                    self.avancar();
                    let i = self.pos;
                    let expression = if self.kw(i, Keyword::This) || self.pal(i, "this") {
                        self.avancar();
                        self.no(
                            No::ThisExpression {
                                this_keyword: i as Tk,
                            },
                            i,
                        )
                    } else {
                        if !self.ident(i)
                            || dartforge_frontend::token::Keyword::from_text(self.txt(i)).is_some()
                        {
                            return Err(self.erro(&format!(
                                "'{}' can't be used as an identifier because it's a keyword.",
                                self.txt(i)
                            )));
                        }
                        self.conferir_await_yield(i)?;
                        self.avancar();
                        self.simples(i)
                    };
                    elementos.push(self.no(
                        No::InterpolationExpression {
                            left_bracket: lb as Tk,
                            expression,
                            right_bracket: None,
                        },
                        lb,
                    ));
                }
                _ => break,
            }
            let s = self.pos;
            if !self.texto(s) {
                return Err(self.erro("Expected a string."));
            }
            self.avancar();
            let ultimo = !matches!(
                self.tipo(self.pos),
                TipoToken::InterpolacaoExpressao | TipoToken::InterpolacaoIdentificador
            );
            self.conferir_escapes(s, false, ultimo)?;
            elementos.push(self.no(No::InterpolationString { contents: s as Tk }, s));
        }
        Ok(self.no(
            No::StringInterpolation {
                elements: elementos,
            },
            ini,
        ))
    }

    /// Literal numérico malformado ou com separadores de dígitos (só no
    /// Dart 3.7), que o scanner do analyzer recusa.
    fn conferir_numero(&self, i: usize) -> R<()> {
        let t = self.txt(i);
        if t.contains('_') {
            // `recurso` precisa de `&mut self`; o erro de versão é direto.
            if self.versao < (3, 6) {
                return Err(self.erro_em(
                    i,
                    "This requires the 'digit-separators' language feature to be enabled.",
                ));
            }
        }
        let b = t.as_bytes();
        let hexa = b.len() >= 2 && b[0] == b'0' && (b[1] == b'x' || b[1] == b'X');
        let digito = |c: u8| {
            if hexa {
                c.is_ascii_hexdigit()
            } else {
                c.is_ascii_digit()
            }
        };
        for (k, &c) in b.iter().enumerate() {
            if c != b'_' {
                continue;
            }
            // Cada sequência de `_` precisa de dígitos dos dois lados.
            let antes = b[..k].iter().rev().find(|&&x| x != b'_').copied();
            let depois = b[k + 1..].iter().find(|&&x| x != b'_').copied();
            let ok =
                antes.is_some_and(|x| digito(x) && !(hexa && k == 2)) && depois.is_some_and(digito);
            if !ok || (hexa && k <= 2) {
                return Err(self.erro_em(i, "Digit separators ('_') in a number literal can only be placed between two digits."));
            }
        }
        if b.len() >= 2 && b[0] == b'0' && (b[1] == b'x' || b[1] == b'X') {
            if b.len() == 2 {
                return Err(self.erro_em(i, "Hexadecimal digit expected."));
            }
            return Ok(());
        }
        if let Some(e) = t.find(['e', 'E']) {
            let resto = t[e + 1..].trim_start_matches(['+', '-']);
            if resto.is_empty() || !resto.bytes().all(|c| c.is_ascii_digit()) {
                return Err(self.erro_em(i, "Decimal digit expected."));
            }
        }
        Ok(())
    }

    /// Escapes inválidos num pedaço de string não crua (`unescape` do
    /// fasta). `primeiro`/`ultimo` dizem se o pedaço tem as aspas de
    /// abertura/fechamento.
    fn conferir_escapes(&self, i: usize, primeiro: bool, ultimo: bool) -> R<()> {
        let mut t = self.txt(i);
        if primeiro {
            for q in ["\"\"\"", "'''", "\"", "'"] {
                if let Some(r) = t.strip_prefix(q) {
                    t = r;
                    if ultimo {
                        t = t.strip_suffix(q).unwrap_or(t);
                    }
                    break;
                }
            }
        } else if ultimo {
            for q in ["\"\"\"", "'''", "\"", "'"] {
                if let Some(r) = t.strip_suffix(q) {
                    t = r;
                    break;
                }
            }
        }
        if let Some(m) = erro_de_escape(t.as_bytes()) {
            return Err(self.erro_em(i, m));
        }
        Ok(())
    }

    /// `parseLiteralSymbol`.
    fn simbolo(&mut self) -> R<NoId> {
        let h = self.avancar();
        let mut componentes = Vec::new();
        if self.operador_definivel(self.pos).is_some() {
            componentes.push(self.consumir_operador_definivel()? as Tk);
        } else if self.kw(self.pos, Keyword::Void) {
            componentes.push(self.avancar() as Tk);
        } else {
            componentes.push(self.espera_ident()? as Tk);
            while self.op(self.pos, Op::Dot) {
                self.avancar();
                if !self.palavra(self.pos) {
                    return Err(self.erro("Expected an identifier."));
                }
                componentes.push(self.avancar() as Tk);
            }
        }
        Ok(self.no(
            No::SymbolLiteral {
                pound_sign: h as Tk,
                components: componentes,
            },
            h,
        ))
    }

    /// `parseLiteralListSuffix`.
    fn lista(&mut self, constante: Option<usize>, targs: Option<NoId>) -> R<NoId> {
        let ini = constante.or(targs.map(|t| self.ini(t))).unwrap_or(self.pos);
        let lb = self.espera_op(Op::LBracket)?;
        let old = self.funcoes;
        self.funcoes = true;
        let mut elementos = Vec::new();
        loop {
            if self.op(self.pos, Op::RBracket) {
                break;
            }
            elementos.push(self.elemento(false)?);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.avancar();
        }
        let rb = self.espera_op(Op::RBracket)?;
        self.funcoes = old;
        Ok(self.no(
            No::ListLiteral {
                const_keyword: constante.map(|c| c as Tk),
                type_arguments: targs,
                left_bracket: lb as Tk,
                elements: elementos,
                right_bracket: rb as Tk,
            },
            ini,
        ))
    }

    /// `parseLiteralSetOrMapSuffix`.
    fn conjunto_ou_mapa(&mut self, constante: Option<usize>, targs: Option<NoId>) -> R<NoId> {
        let ini = constante.or(targs.map(|t| self.ini(t))).unwrap_or(self.pos);
        let lb = self.espera_op(Op::LBrace)?;
        let old = self.funcoes;
        self.funcoes = true;
        let mut elementos = Vec::new();
        loop {
            if self.op(self.pos, Op::RBrace) {
                break;
            }
            elementos.push(self.elemento(true)?);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.avancar();
        }
        let rb = self.espera_op(Op::RBrace)?;
        self.funcoes = old;
        Ok(self.no(
            No::SetOrMapLiteral {
                const_keyword: constante.map(|c| c as Tk),
                type_arguments: targs,
                left_bracket: lb as Tk,
                elements: elementos,
                right_bracket: rb as Tk,
            },
            ini,
        ))
    }

    /// Um elemento de coleção (`computeLiteralEntry` e passos seguintes).
    fn elemento(&mut self, mapa: bool) -> R<NoId> {
        self.entrar()?;
        let r = self.elemento_interno(mapa);
        self.sair();
        r
    }

    fn elemento_interno(&mut self, mapa: bool) -> R<NoId> {
        let i = self.pos;
        if self.kw(i, Keyword::If) {
            self.avancar();
            let (lp, expression, case_clause, rp) = self.condicao_parenteses(self.padroes)?;
            let then_element = self.elemento(mapa)?;
            let (else_keyword, else_element) = if self.kw(self.pos, Keyword::Else) {
                let e = self.avancar();
                (Some(e as Tk), Some(self.elemento(mapa)?))
            } else {
                (None, None)
            };
            return Ok(self.no(
                No::IfElement {
                    if_keyword: i as Tk,
                    left_parenthesis: lp as Tk,
                    expression,
                    case_clause,
                    right_parenthesis: rp as Tk,
                    then_element,
                    else_keyword,
                    else_element,
                },
                i,
            ));
        }
        if self.kw(i, Keyword::For) || (self.pal(i, "await") && self.kw(i + 1, Keyword::For)) {
            let await_keyword = if self.pal(i, "await") {
                Some(self.avancar() as Tk)
            } else {
                None
            };
            let f = self.avancar();
            let (lp, for_loop_parts, rp) = self.partes_for(await_keyword.map(|a| a as usize))?;
            let body = self.elemento(mapa)?;
            return Ok(self.no(
                No::ForElement {
                    await_keyword,
                    for_keyword: f as Tk,
                    left_parenthesis: lp as Tk,
                    for_loop_parts,
                    right_parenthesis: rp as Tk,
                    body,
                },
                i,
            ));
        }
        if self.op(i, Op::Ellipsis) || self.op(i, Op::EllipsisQuestion) {
            self.avancar();
            let expression = self.expressao()?;
            return Ok(self.no(
                No::SpreadElement {
                    spread_operator: i as Tk,
                    expression,
                },
                i,
            ));
        }
        if self.op(i, Op::Question) {
            return Err(self.erro("Null-aware elements aren't supported in Dart 3.6."));
        }
        let e = self.expressao()?;
        if mapa && self.op(self.pos, Op::Colon) {
            let s = self.avancar();
            if self.op(self.pos, Op::Question) {
                return Err(self.erro("Null-aware elements aren't supported in Dart 3.6."));
            }
            let value = self.expressao()?;
            let ini = self.ini(e);
            return Ok(self.no(
                No::MapLiteralEntry {
                    key: e,
                    separator: s as Tk,
                    value,
                },
                ini,
            ));
        }
        Ok(e)
    }

    /// `ensureParenthesizedCondition`: `( expressão (case padrão (when
    /// expressão)?)? )`.
    pub(super) fn condicao_parenteses(
        &mut self,
        permite_case: bool,
    ) -> R<(usize, NoId, Option<NoId>, usize)> {
        let lp = self.espera_op(Op::LParen)?;
        let expression = self.expressao()?;
        let mut case_clause = None;
        if permite_case && self.kw(self.pos, Keyword::Case) {
            let c = self.avancar();
            let guarded_pattern = self.padrao_guardado()?;
            case_clause = Some(self.no(
                No::CaseClause {
                    case_keyword: c as Tk,
                    guarded_pattern,
                },
                c,
            ));
        }
        let rp = self.espera_op(Op::RParen)?;
        Ok((lp, expression, case_clause, rp))
    }

    /// Padrão com `when` opcional ([`No::GuardedPattern`]).
    pub(super) fn padrao_guardado(&mut self) -> R<NoId> {
        let pattern = self.padrao(CtxPadrao::Correspondencia)?;
        let ini = self.ini(pattern);
        let when_clause = if self.pal(self.pos, "when") {
            let w = self.avancar();
            let expression = self.expressao()?;
            Some(self.no(
                No::WhenClause {
                    when_keyword: w as Tk,
                    expression,
                },
                w,
            ))
        } else {
            None
        };
        Ok(self.no(
            No::GuardedPattern {
                pattern,
                when_clause,
            },
            ini,
        ))
    }

    /// `parseSwitchExpression`.
    fn expressao_switch(&mut self) -> R<NoId> {
        let sw = self.avancar();
        let old = self.funcoes;
        self.funcoes = true;
        let (lp, expression, _, rp) = self.condicao_parenteses(false)?;
        let lb = self.espera_op(Op::LBrace)?;
        let mut casos = Vec::new();
        if !self.op(self.pos, Op::RBrace) {
            self.funcoes = false;
            loop {
                if self.kw(self.pos, Keyword::Default) || self.kw(self.pos, Keyword::Case) {
                    return Err(self.erro("'default'/'case' can't be used in a switch expression."));
                }
                let guarded_pattern = self.padrao_guardado()?;
                let ci = self.ini(guarded_pattern);
                let arrow = self.espera_op(Op::Arrow)?;
                self.funcoes = true;
                let e = self.expressao()?;
                self.funcoes = false;
                casos.push(self.no(
                    No::SwitchExpressionCase {
                        guarded_pattern,
                        arrow: arrow as Tk,
                        expression: e,
                    },
                    ci,
                ));
                let virgula = self.opcional_op(Op::Comma).is_some();
                if self.op(self.pos, Op::RBrace) {
                    break;
                }
                if !virgula {
                    return Err(self.esperado(","));
                }
            }
        }
        let rb = self.espera_op(Op::RBrace)?;
        self.funcoes = old;
        Ok(self.no(
            No::SwitchExpression {
                switch_keyword: sw as Tk,
                left_parenthesis: lp as Tk,
                expression,
                right_parenthesis: rp as Tk,
                left_bracket: lb as Tk,
                cases: casos,
                right_bracket: rb as Tk,
            },
            sw,
        ))
    }

    /// Expressão de padrão constante (`parsePrecedenceExpression` com
    /// `EQUALITY_PRECEDENCE`), usada pelos padrões.
    pub(super) fn expressao_padrao_constante(&mut self, ctx: CtxConst) -> R<NoId> {
        self.precedencia(prec::IGUALDADE, false, ctx)
    }

    /// Operando de padrão relacional (`BITWISE_OR_PRECEDENCE`).
    pub(super) fn operando_relacional(&mut self) -> R<NoId> {
        self.precedencia(prec::BIT_OU, false, CtxConst::Nenhum)
    }
}
