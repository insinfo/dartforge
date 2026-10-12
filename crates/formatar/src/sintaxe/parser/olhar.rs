//! *Lookahead* sem efeitos, portado de `type_info.dart` e
//! `type_info_impl.dart` do fasta (`computeType`, `computeTypeParamOrArg`,
//! `ComplexTypeInfo`) e de utilidades do `parser_impl.dart`
//! (`skipOuterPattern`, `looksLikeLocalFunction`, `skipMetadata`...).
//!
//! Convenção: as funções recebem o índice do primeiro token da construção
//! (o `token.next` do fasta) e devolvem o índice do primeiro token *depois*
//! dela (`ate`). Para "não é tipo", `ate` é o próprio índice de partida.

use dartforge_frontend::token::{Keyword, Op};

use super::{Estilo, Parser, estilo};

/// Forma do tipo encontrado (`TypeInfo` do fasta).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Forma {
    /// `noType`.
    Nenhum,
    /// `simpleType`: identificador.
    Simples,
    /// `simpleNullableType`: identificador `?`.
    SimplesNulavel,
    /// `voidType`.
    Void,
    /// `prefixedType`: identificador `.` identificador.
    Prefixado,
    /// `simpleTypeWith1Argument` e variante anulável.
    ComArg1,
    /// `ComplexTypeInfo`.
    Complexo,
}

/// Resultado de [`Parser::tipo_em`].
#[derive(Debug, Clone, Copy)]
pub(super) struct InfoTipo {
    pub forma: Forma,
    /// Primeiro token depois do tipo.
    pub ate: usize,
    /// Índice do token antes do `?` final, se o `?` faz parte do tipo.
    pub antes_interrogacao: Option<usize>,
    /// `couldBeExpression`.
    pub pode_ser_expressao: bool,
    /// `isRecordType`.
    pub registro: bool,
    /// `gftReturnTypeHasRecordType`.
    pub registro_no_retorno: bool,
    pub recuperado: bool,
}

impl InfoTipo {
    pub fn nenhum(i: usize) -> InfoTipo {
        InfoTipo {
            forma: Forma::Nenhum,
            ate: i,
            antes_interrogacao: None,
            pode_ser_expressao: false,
            registro: false,
            registro_no_retorno: false,
            recuperado: false,
        }
    }

    fn simples(forma: Forma, ate: usize, nulavel: bool, pode_ser_expressao: bool) -> InfoTipo {
        InfoTipo {
            forma,
            ate,
            antes_interrogacao: if nulavel { Some(ate - 2) } else { None },
            pode_ser_expressao,
            ..InfoTipo::nenhum(ate)
        }
    }

    pub fn e_nenhum(&self) -> bool {
        self.forma == Forma::Nenhum
    }

    pub fn nulavel(&self) -> bool {
        self.antes_interrogacao.is_some()
    }

    /// `asNonNullable`: o `?` final deixa de fazer parte do tipo.
    pub fn sem_interrogacao(mut self) -> InfoTipo {
        if let Some(a) = self.antes_interrogacao {
            self.ate = a + 1;
            self.antes_interrogacao = None;
            if self.forma == Forma::SimplesNulavel {
                self.forma = Forma::Simples;
            }
        }
        self
    }
}

/// Resultado de [`Parser::args_tipo_em`] (`TypeParamOrArgInfo`).
#[derive(Debug, Clone, Copy)]
pub(super) struct InfoArgs {
    /// Primeiro token depois do `>`.
    pub ate: usize,
    /// `isSimpleTypeArgument`: `<` identificador `>`.
    pub simples: bool,
    pub recuperado: bool,
}

/// Estado de `ComplexTypeInfo` durante o cálculo.
struct Complexo {
    inicio: usize,
    args: Option<InfoArgs>,
    antes_interrogacao: Option<usize>,
    /// Último token do tipo.
    fim: usize,
    inicios_funcao: usize,
    retorno_gft: Option<bool>,
    registro: bool,
    registro_no_retorno: bool,
    recuperado: bool,
}

impl Complexo {
    fn novo(inicio: usize, args: Option<InfoArgs>) -> Complexo {
        Complexo {
            inicio,
            args,
            antes_interrogacao: None,
            fim: inicio,
            inicios_funcao: 0,
            retorno_gft: None,
            registro: false,
            registro_no_retorno: false,
            recuperado: args.is_some_and(|a| a.recuperado),
        }
    }

    fn info(&self) -> InfoTipo {
        InfoTipo {
            forma: Forma::Complexo,
            ate: self.fim + 1,
            antes_interrogacao: self.antes_interrogacao,
            pode_ser_expressao: self.args.is_none() && self.inicios_funcao == 0,
            registro: self.registro,
            registro_no_retorno: self.registro_no_retorno,
            recuperado: self.recuperado,
        }
    }
}

impl Parser<'_> {
    /// `isGeneralizedFunctionType`: `Function` seguido de `<` ou `(`.
    pub(super) fn gft(&self, i: usize) -> bool {
        self.pal(i, "Function") && (self.op(i + 1, Op::Lt) || self.op(i + 1, Op::LParen))
    }

    /// `isValidNonRecordTypeReference`.
    pub(super) fn ref_tipo_valida(&self, i: usize) -> bool {
        if self.ident_puro(i) || self.pseudo(i) || self.kw(i, Keyword::Void) {
            return true;
        }
        if self.embutida(i) {
            let t = self.txt(i);
            return self.op(i + 1, Op::Dot) || t == "dynamic" || t == "Function";
        }
        false
    }

    /// `looksLikeName`.
    pub(super) fn parece_nome(&self, i: usize) -> bool {
        self.ident_puro(i)
            || self.kw(i, Keyword::This)
            || self.kw(i, Keyword::Super)
            || (self.ident(i) && !(self.txt(i) == "typedef" && self.ident(i + 1)))
    }

    /// `looksLikeNameOrEndOfBlock`.
    fn parece_nome_ou_fim(&self, i: usize) -> bool {
        self.parece_nome(i) || self.eof(i) || self.op(i, Op::RBrace)
    }

    /// `isCloser`: só `>` (os compostos são `>` isolados aqui).
    pub(super) fn fecho_tipo(&self, i: usize) -> bool {
        self.op(i, Op::Gt)
    }

    /// `computeType(token, required, inDeclaration,
    /// acceptKeywordForSimpleType)` para um tipo que começa em `i`.
    pub(super) fn tipo_em(&self, i: usize, requerido: bool, decl: bool, aceita_palavra: bool) -> InfoTipo {
        let n = i;
        if !self.ref_tipo_valida(n) && !self.op(n, Op::LParen) {
            if self.embutida(n) {
                if let Some(tp) = self.args_tipo_em(n, decl) {
                    if requerido || self.parece_nome(tp.ate) {
                        let mut c = Complexo::novo(n, Some(tp));
                        c.fim = tp.ate - 1;
                        self.resto_tipo(&mut c, requerido);
                        c.recuperado = true;
                        return c.info();
                    }
                } else if requerido || self.gft(n + 1) {
                    let v = self.txt(n);
                    if !(matches!(v, "get" | "set" | "factory" | "operator") || (v == "typedef" && self.ident(n + 1))) {
                        let mut c = Complexo::novo(n, None);
                        c.fim = n;
                        self.resto_tipo(&mut c, requerido);
                        c.recuperado = true;
                        return c.info();
                    }
                }
            }
            return InfoTipo::nenhum(i);
        }
        if self.kw(n, Keyword::Void) {
            if self.gft(n + 1) {
                let mut c = Complexo::novo(n, None);
                c.fim = n;
                self.resto_tipo(&mut c, requerido);
                if c.retorno_gft.is_none() {
                    return InfoTipo::simples(Forma::Void, n + 1, false, false);
                }
                return c.info();
            }
            return InfoTipo::simples(Forma::Void, n + 1, false, false);
        }
        if self.gft(n) {
            // `computeNoTypeGFT`: o `Function` é o início do tipo.
            let mut c = Complexo::novo(n, None);
            c.fim = n.wrapping_sub(1);
            self.resto_tipo_desde(&mut c, n, requerido);
            if c.retorno_gft.is_none() {
                return if requerido {
                    InfoTipo::simples(Forma::Simples, n + 1, false, true)
                } else {
                    InfoTipo::nenhum(i)
                };
            }
            return c.info();
        }
        if self.op(n, Op::LParen) {
            let depois = self.par(n) + 1;
            if self.gft(depois) || (self.op(depois, Op::Question) && self.gft(depois + 1)) {
                let mut c = Complexo::novo(n, None);
                c.fim = depois - 1;
                self.resto_tipo(&mut c, requerido);
                if c.retorno_gft.is_none() {
                    return self.tipo_registro(n, requerido);
                }
                c.registro_no_retorno = true;
                return c.info();
            }
            return self.tipo_registro(n, requerido);
        }
        // Identificador.
        if let Some(tp) = self.args_tipo_em(n, decl) {
            if tp.simples {
                let mut nx = tp.ate;
                if self.op(nx, Op::Question) {
                    nx += 1;
                    if !self.gft(nx) {
                        if requerido || self.parece_nome(nx) {
                            return InfoTipo {
                                forma: Forma::ComArg1,
                                ate: tp.ate + 1,
                                antes_interrogacao: Some(tp.ate - 1),
                                ..InfoTipo::nenhum(i)
                            };
                        }
                        return InfoTipo::nenhum(i);
                    }
                } else if !self.gft(nx) {
                    if requerido || self.parece_nome(nx) {
                        return InfoTipo {
                            forma: Forma::ComArg1,
                            ate: tp.ate,
                            ..InfoTipo::nenhum(i)
                        };
                    }
                    return InfoTipo::nenhum(i);
                }
            }
            // `computeSimpleWithTypeArguments`.
            let mut c = Complexo::novo(n, Some(tp));
            c.fim = tp.ate - 1;
            self.resto_tipo(&mut c, requerido);
            if !requerido && !self.parece_nome_ou_fim(c.fim + 1) && c.retorno_gft.is_none() {
                return InfoTipo::nenhum(i);
            }
            return c.info();
        }
        let mut nx = n + 1;
        if self.op(nx, Op::Dot) {
            nx += 1;
            if self.ref_tipo_valida(nx) {
                let tp2 = self.args_tipo_em(nx, decl);
                let nn = nx + 1;
                if tp2.is_none() {
                    if self.op(nn, Op::Question) {
                        let m = nn + 1;
                        if !self.gft(m) && !(requerido || self.parece_nome(m)) {
                            return InfoTipo::nenhum(i);
                        }
                    } else if !self.gft(nn) {
                        if requerido || self.parece_nome(nn) {
                            return InfoTipo::simples(Forma::Prefixado, nx + 1, false, true);
                        }
                        return InfoTipo::nenhum(i);
                    }
                }
                // `computePrefixedType`.
                let mut c = Complexo::novo(n, tp2);
                c.fim = tp2.map_or(nx, |a| a.ate - 1);
                self.resto_tipo(&mut c, requerido);
                if !requerido && !self.parece_nome(c.fim + 1) && c.retorno_gft.is_none() {
                    return InfoTipo::nenhum(i);
                }
                return c.info();
            }
            if requerido {
                // `identifier . non-identifier` (recuperação): o tipo prefixado
                // usa a palavra seguinte, se houver.
                let mut c = Complexo::novo(n, None);
                let mut t = nx - 1;
                if self.palavra(nx) {
                    t = nx;
                }
                let tp = self.args_tipo_em(t, decl);
                c.args = tp;
                c.fim = tp.map_or(t, |a| a.ate - 1);
                self.resto_tipo(&mut c, requerido);
                c.recuperado = true;
                return c.info();
            }
            return InfoTipo::nenhum(i);
        }
        if self.gft(nx) {
            let mut c = Complexo::novo(n, None);
            c.fim = n;
            self.resto_tipo(&mut c, requerido);
            if c.retorno_gft.is_none() {
                return InfoTipo::simples(Forma::Simples, n + 1, false, true);
            }
            return c.info();
        }
        if self.op(nx, Op::Question) {
            if self.gft(nx + 1) {
                let mut c = Complexo::novo(n, None);
                c.fim = n;
                self.resto_tipo(&mut c, requerido);
                if c.retorno_gft.is_none() {
                    return InfoTipo::simples(Forma::SimplesNulavel, n + 2, true, true);
                }
                return c.info();
            } else if requerido || self.parece_nome(nx + 1) {
                return InfoTipo::simples(Forma::SimplesNulavel, n + 2, true, true);
            }
        } else if requerido
            || self.parece_nome(nx)
            || (aceita_palavra
                && self.palavra(nx)
                && (self.op(nx + 1, Op::Assign)
                    || self.op(nx + 1, Op::Colon)
                    || self.op(nx + 1, Op::Comma)
                    || self.op(nx + 1, Op::RParen)
                    || self.op(nx + 1, Op::RBracket)
                    || self.op(nx + 1, Op::RBrace)))
        {
            return InfoTipo::simples(Forma::Simples, n + 1, false, true);
        }
        InfoTipo::nenhum(i)
    }

    /// `computeRest` a partir do último token `c.fim`.
    fn resto_tipo(&self, c: &mut Complexo, requerido: bool) {
        let mut t = c.fim;
        if self.op(t + 1, Op::Question) {
            c.antes_interrogacao = Some(t);
            t += 1;
            c.fim = t;
        }
        self.resto_tipo_desde(c, t + 1, requerido);
    }

    /// Laço de `Function` de `computeRest`, começando em `t`.
    fn resto_tipo_desde(&self, c: &mut Complexo, mut t: usize, requerido: bool) {
        while self.pal(t, "Function") {
            let inicio_var = t;
            if let Some(a) = self.args_tipo_em(t, true) {
                t = a.ate - 1;
            }
            t += 1;
            if !self.op(t, Op::LParen) {
                break;
            }
            t = self.par(t);
            if !requerido {
                let mut nx = t + 1;
                if self.op(nx, Op::Question) {
                    nx += 1;
                }
                if !(self.ident(nx) || self.kw(nx, Keyword::This) || self.kw(nx, Keyword::Super)) {
                    break;
                }
            }
            c.retorno_gft.get_or_insert(inicio_var != c.inicio);
            c.inicios_funcao += 1;
            c.antes_interrogacao = None;
            c.fim = t;
            t += 1;
            if self.op(t, Op::Question) {
                c.antes_interrogacao = Some(c.fim);
                c.fim = t;
                t += 1;
            }
        }
    }

    /// `computeRecordType` para o `(` em `n`.
    fn tipo_registro(&self, n: usize, requerido: bool) -> InfoTipo {
        let mut c = Complexo::novo(n, None);
        let fecho = self.par(n);
        c.recuperado = self.registro_recuperado(n, fecho);
        let token = fecho;
        if !requerido {
            let mut next = token + 1;
            if self.op(next, Op::Question) {
                next += 1;
            }
            let mut get_set = false;
            if (self.pal(next, "get") || self.pal(next, "set")) && self.ident(next + 1) {
                get_set = true;
                next += 1;
            }
            if self.ident(next) {
                let depois = next + 1;
                let ok = self.eof(depois)
                    || matches!(
                        self.tipo(depois),
                        super::TipoToken::Op(
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
                    || self.kw(depois, Keyword::In);
                if !ok {
                    let corpo = self.op(depois, Op::Arrow)
                        || self.op(depois, Op::LBrace)
                        || self.pal(depois, "async")
                        || self.pal(depois, "sync")
                        || self.eof(depois);
                    if get_set && corpo {
                    } else if self.pal(next, "operator") && self.operador_definivel(depois).is_some() {
                    } else {
                        return InfoTipo::nenhum(n);
                    }
                }
            } else if (self.kw(next, Keyword::This) || self.kw(next, Keyword::Super)) && self.op(next + 1, Op::Dot) {
            } else if c.recuperado || !(self.op(next, Op::Comma) || self.op(next, Op::Gt) || self.eof(next)) {
                return InfoTipo::nenhum(n);
            }
        }
        c.fim = token;
        if self.op(token + 1, Op::Question) {
            c.antes_interrogacao = Some(token);
            c.fim = token + 1;
        }
        c.registro = true;
        c.info()
    }

    /// `_checkIfRecordTypeParenthesisAreRecovered`.
    fn registro_recuperado(&self, abre: usize, fecho: usize) -> bool {
        let mut token = abre;
        let mut contagem = 0;
        let mut nomeados = false;
        let mut virgula = false;
        loop {
            let next = token + 1;
            if self.op(next, Op::RParen) {
                token = next;
                break;
            } else if nomeados && self.op(next, Op::RBrace) && self.op(next + 1, Op::RParen) {
                token = next + 1;
                break;
            }
            contagem += 1;
            if !nomeados && self.op(next, Op::LBrace) {
                nomeados = true;
                token += 1;
            }
            if self.op(token + 1, Op::At) {
                token = self.pular_metadado(token + 1) - 1;
            }
            let t = self.tipo_em(token + 1, true, false, false);
            if t.recuperado {
                return true;
            }
            token = t.ate - 1;
            if self.ident(token + 1) {
                token += 1;
            } else if nomeados {
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
            virgula = true;
            token = next;
        }
        (contagem == 1 && !nomeados && !virgula) || token != fecho
    }

    /// `computeTypeParamOrArg(token)`: o `<` está em `antes + 1`.
    pub(super) fn args_tipo_em(&self, antes: usize, decl: bool) -> Option<InfoArgs> {
        self.args_tipo_lt(antes + 1, decl)
    }

    /// `computeTypeParamOrArg` com o `<` em `lt`.
    pub(super) fn args_tipo_lt(&self, lt: usize, decl: bool) -> Option<InfoArgs> {
        if !self.op(lt, Op::Lt) {
            return None;
        }
        let next = lt + 1;
        if self.ident_puro(next) || self.pseudo(next) {
            if self.op(next + 1, Op::Gt) {
                return Some(InfoArgs {
                    ate: next + 2,
                    simples: true,
                    recuperado: false,
                });
            }
        } else if self.op(next, Op::LParen) {
            let t = self.tipo_em(next, false, false, false);
            if !(t.forma == Forma::Complexo && (t.registro || t.registro_no_retorno) && !t.recuperado) {
                return None;
            }
        }
        // `ComplexTypeParamOrArgInfo.compute`.
        let mut recuperado = false;
        let mut next = lt;
        let mut token;
        loop {
            let mut ti = self.tipo_em(next + 1, true, decl, false);
            recuperado |= ti.recuperado;
            if ti.e_nenhum() {
                while ti.e_nenhum() && self.op(next + 1, Op::At) {
                    next = self.pular_metadado(next + 1) - 1;
                    ti = self.tipo_em(next + 1, true, decl, false);
                }
                if ti.e_nenhum() {
                    if next == lt && !decl && !self.fecho_tipo(next + 1) {
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
            token = if ti.e_nenhum() { next } else { ti.ate - 1 };
            next = token + 1;
            if self.kw(next, Keyword::Extends) {
                let b = self.tipo_em(next + 1, true, decl, false);
                token = if b.e_nenhum() { next } else { b.ate - 1 };
                next = token + 1;
            }
            if !self.op(next, Op::Comma) {
                if self.fecho_tipo(next) {
                    return Some(InfoArgs {
                        ate: next + 1,
                        simples: false,
                        recuperado,
                    });
                }
                if !decl {
                    return None;
                }
                // Recuperação: vírgula ausente.
                if !(self.ident_puro(next)
                    && (self.ident_puro(next + 1) || self.op(next + 1, Op::Comma) || self.fecho_tipo(next + 1)))
                {
                    break;
                }
                next = token;
            }
        }
        // Recuperação.
        if self.fecho_tipo(next) {
            return Some(InfoArgs {
                ate: next + 1,
                simples: false,
                recuperado,
            });
        }
        let mut n = next;
        if self.op(n, Op::LParen) {
            n = self.par(n) + 1;
        }
        let ate = if self.fecho_tipo(n) {
            n + 1
        } else if self.fecho_tipo(n + 1) {
            n + 2
        } else {
            n
        };
        Some(InfoArgs {
            ate,
            simples: false,
            recuperado: true,
        })
    }

    /// `computeMethodTypeArguments`: argumentos de tipo seguidos de um token
    /// que pode vir depois deles numa expressão (`mayFollowTypeArgs`).
    pub(super) fn args_metodo_em(&self, antes: usize) -> Option<InfoArgs> {
        let a = self.args_tipo_em(antes, false)?;
        if a.recuperado {
            return None;
        }
        let t = a.ate;
        let ok = self.eof(t)
            || matches!(
                self.tipo(t),
                super::TipoToken::Op(
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

    /// `skipMetadata` para o `@` em `i`: devolve o índice depois da anotação.
    pub(super) fn pular_metadado(&self, i: usize) -> usize {
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
            if self.op(next, Op::Lt) {
                if let Some(a) = self.args_tipo_em(token, true) {
                    token = a.ate - 1;
                    next = token + 1;
                }
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
                token = self.par(next);
            }
        }
        token + 1
    }

    /// `computeVariablePatternType`.
    pub(super) fn tipo_de_padrao_variavel_em(&self, i: usize, requerido: bool) -> InfoTipo {
        let t = self.tipo_em(i, requerido, false, false);
        if !t.e_nenhum() && self.ident(t.ate) && (self.pal(t.ate, "as") || self.pal(t.ate, "when")) {
            return InfoTipo::nenhum(i);
        }
        t
    }

    /// `looksLikeLocalFunction` para o token `i`.
    pub(super) fn parece_funcao_local(&self, i: usize) -> bool {
        if !self.ident(i) {
            return false;
        }
        let mut t = i;
        if self.op(t + 1, Op::Lt) {
            match self.args_tipo_em(t, false) {
                Some(a) => t = a.ate - 1,
                None => return false,
            }
        }
        t += 1;
        if self.op(t, Op::LParen) {
            let d = self.par(t) + 1;
            return self.op(d, Op::LBrace) || self.op(d, Op::Arrow) || self.pal(d, "async") || self.pal(d, "sync");
        }
        self.op(t, Op::Arrow)
    }

    /// `looksLikeFunctionBody`.
    pub(super) fn parece_corpo(&self, i: usize) -> bool {
        self.op(i, Op::LBrace) || self.op(i, Op::Arrow) || self.pal(i, "async") || self.pal(i, "sync")
    }

    /// `skipOuterPattern`: devolve o último token do padrão externo que
    /// começa em `i`, se houver.
    pub(super) fn pular_padrao_externo(&self, i: usize) -> Option<usize> {
        if self.ident(i) {
            if !self.op(i + 1, Op::Dot) {
                return self.pular_resto_objeto(i);
            }
            if self.ident(i + 2) {
                return self.pular_resto_objeto(i + 2);
            }
            return None;
        }
        let tp = self.args_tipo_lt(i, false);
        let next = tp.map_or(i, |a| a.ate);
        if self.op(next, Op::LBracket) || self.op(next, Op::LBrace) {
            return Some(self.par(next));
        }
        if tp.is_none() && self.op(next, Op::LParen) {
            return Some(self.par(next));
        }
        None
    }

    fn pular_resto_objeto(&self, ultimo_nome: usize) -> Option<usize> {
        let next = self.args_tipo_em(ultimo_nome, false).map_or(ultimo_nome + 1, |a| a.ate);
        if !self.op(next, Op::LParen) {
            return None;
        }
        Some(self.par(next))
    }

    /// `looksLikeExpressionAfterAwaitOrYield` para a palavra em `i`;
    /// `unario` é o contexto `UnaryExpression`.
    pub(super) fn parece_expressao_apos(&self, i: usize, unario: bool) -> bool {
        let t = i + 1;
        if self.ident(t) {
            let t = t + 1;
            if self.op(t, Op::LParen) {
                let d = self.par(t) + 1;
                if matches!(
                    self.tipo(d),
                    super::TipoToken::Op(
                        Op::Semicolon | Op::Dot | Op::Comma | Op::DotDot | Op::Question | Op::QuestionDot | Op::RParen
                    )
                ) {
                    return true;
                }
                if self.operador_binario(d) {
                    return true;
                }
            } else if self.op(t, Op::Dot) || self.op(t, Op::RParen) || self.op(t, Op::RBracket) {
                return true;
            } else if self.op(t, Op::Comma) && unario {
                return true;
            } else if self.operador_binario(t) {
                return true;
            } else if self.op(t, Op::Semicolon) && unario {
                return true;
            }
        } else if self.kw(t, Keyword::Null) {
            return true;
        }
        false
    }

    /// `type.isBinaryOperator`.
    pub(super) fn operador_binario(&self, i: usize) -> bool {
        use super::TipoToken::Op as O;
        match self.tipo(i) {
            O(Op::Gt) => true,
            O(
                Op::Plus
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
                | Op::QuestionQuestion,
            ) => true,
            _ => false,
        }
    }

    /// Palavra reservada (`isReservedWord`).
    pub(super) fn reservada(&self, i: usize) -> bool {
        matches!(self.tipo(i), super::TipoToken::Reservada(_))
    }

    /// Estilo da palavra em `i`.
    #[allow(dead_code)]
    pub(super) fn estilo_em(&self, i: usize) -> Estilo {
        if self.ident(i) { estilo(self.txt(i)) } else { Estilo::Nenhum }
    }
}
