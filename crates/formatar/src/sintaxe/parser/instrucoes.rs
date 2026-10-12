//! Comandos, blocos, corpos de função e cabeçalhos de `for`
//! (`parseStatementX`, `parseExpressionStatementOrDeclaration*`,
//! `parseFunctionBody`, `parseForLoopParts*`, `parseSwitchBlock`,
//! `parseTryStatement` do fasta), com os nós do `AstBuilder`.

use dartforge_frontend::token::{Keyword, Op};

use super::super::arvore::{No, NoId};
use super::super::token::{TipoToken, Tk};
use super::olhar::InfoTipo;
use super::padroes::CtxPadrao;
use super::tipos::Membro;
use super::{Assinc, Laco, Parser, R};

/// Início de uma declaração local: metadata e modificadores já lidos.
pub(super) struct Decl {
    /// Primeiro token (metadata ou modificador).
    pub inicio: usize,
    /// Onde começa o tipo.
    pub antes_tipo: usize,
    pub meta: Vec<NoId>,
    pub late: Option<usize>,
    /// `var`, `final` ou `const`.
    pub vfc: Option<usize>,
    pub ti: Option<InfoTipo>,
}

impl Decl {
    fn sem_prefixo(&self) -> bool {
        self.antes_tipo == self.inicio
    }
}

/// O que `parseExpressionStatementOrDeclaration` encontrou num `for`.
enum InicioFor {
    Expressao,
    Declaracao(Decl, Option<NoId>),
    Padrao(Vec<NoId>, usize, NoId, usize),
}

impl Parser<'_> {
    /// `parseStatement`.
    pub(super) fn instrucao(&mut self) -> R<NoId> {
        self.entrar()?;
        let r = self.instrucao_x();
        self.sair();
        r
    }

    fn instrucao_x(&mut self) -> R<NoId> {
        let i = self.pos;
        if self.ident_puro(i) {
            if self.op(i + 1, Op::Colon) {
                return self.rotulada();
            }
            return self.expr_ou_decl_apos_mod(Decl {
                inicio: i,
                antes_tipo: i,
                meta: Vec::new(),
                late: None,
                vfc: None,
                ti: None,
            });
        }
        match self.tipo(i) {
            TipoToken::Op(Op::LBrace) => {
                if self.padroes && self.op(self.par(i) + 1, Op::Assign) {
                    self.instrucao_expressao()
                } else {
                    self.bloco()
                }
            }
            TipoToken::Op(Op::Semicolon) => {
                self.avancar();
                Ok(self.no(EmptyStatement(i), i))
            }
            TipoToken::Reservada(k) => match k {
                Keyword::Return => self.retorno(),
                Keyword::Var | Keyword::Final => {
                    if !self.modificador(i + 1) {
                        self.avancar();
                        return self.expr_ou_decl_apos_mod(Decl {
                            inicio: i,
                            antes_tipo: i + 1,
                            meta: Vec::new(),
                            late: None,
                            vfc: Some(i),
                            ti: None,
                        });
                    }
                    self.expr_ou_decl()
                }
                Keyword::If => self.se(),
                Keyword::For => self.para(None),
                Keyword::Rethrow => {
                    self.avancar();
                    let r = self.no(No::RethrowExpression { rethrow_keyword: i as Tk }, i);
                    let s = self.espera_op(Op::Semicolon)?;
                    Ok(self.no(
                        No::ExpressionStatement {
                            expression: r,
                            semicolon: Some(s as Tk),
                        },
                        i,
                    ))
                }
                Keyword::While => self.enquanto(),
                Keyword::Do => self.faca(),
                Keyword::Try => self.tente(),
                Keyword::Switch => self.escolha(),
                Keyword::Break | Keyword::Continue => self.quebra(k == Keyword::Break),
                Keyword::Assert => self.afirmacao(),
                Keyword::Const => self.const_decl_ou_expr(),
                _ => self.expr_ou_decl(),
            },
            TipoToken::Identificador => {
                let t = self.txt(i);
                if t == "await" && self.kw(i + 1, Keyword::For) {
                    let a = self.avancar();
                    return self.para(Some(a));
                }
                if t == "yield" {
                    match self.assinc {
                        Assinc::Sinc => {
                            if self.op(i + 1, Op::Colon) {
                                return self.rotulada();
                            }
                            if self.parece_expressao_apos(i, false) {
                                return self.ceder();
                            }
                            return self.expr_ou_decl();
                        }
                        _ => return self.ceder(),
                    }
                }
                if t == "await" {
                    if self.assinc == Assinc::Sinc && !self.parece_expressao_apos(i, false) {
                        return self.expr_ou_decl();
                    }
                    return self.instrucao_expressao();
                }
                if t == "set" && self.ident(i + 1) {
                    return Err(self.erro("Unexpected 'set'."));
                }
                if self.op(i + 1, Op::Colon) {
                    return self.rotulada();
                }
                self.expr_ou_decl()
            }
            _ => self.expr_ou_decl(),
        }
    }

    /// `parseExpressionStatement`.
    fn instrucao_expressao(&mut self) -> R<NoId> {
        let ini = self.pos;
        let expression = self.expressao()?;
        if matches!(self.arv.no(expression), No::SuperExpression { .. }) {
            return Err(self.erro_em(ini, "Can't use 'super' as an expression."));
        }
        let s = self.espera_op(Op::Semicolon)?;
        Ok(self.no(
            No::ExpressionStatement {
                expression,
                semicolon: Some(s as Tk),
            },
            ini,
        ))
    }

    /// Metadata e modificadores de uma declaração local
    /// (`parseExpressionStatementOrDeclaration`).
    fn modificadores_locais(&mut self) -> R<Decl> {
        let inicio = self.pos;
        let meta = self.metadados()?;
        let mut late = None;
        let mut vfc = None;
        if self.modificador(self.pos) {
            if self.pal(self.pos, "augment") && self.kw(self.pos + 1, Keyword::Super) {
                return Err(self.erro("Augmentations aren't supported here."));
            }
            if self.kw(self.pos, Keyword::Var) || self.kw(self.pos, Keyword::Final) || self.kw(self.pos, Keyword::Const) {
                vfc = Some(self.avancar());
            } else if self.pal(self.pos, "late") {
                late = Some(self.avancar());
                if self.kw(self.pos, Keyword::Var) || self.kw(self.pos, Keyword::Final) {
                    vfc = Some(self.avancar());
                }
            }
            if self.modificador(self.pos) {
                return Err(self.erro(&format!("Can't have modifier '{}' here.", self.txt(self.pos))));
            }
        }
        Ok(Decl {
            inicio,
            antes_tipo: self.pos,
            meta,
            late,
            vfc,
            ti: None,
        })
    }

    fn expr_ou_decl(&mut self) -> R<NoId> {
        let d = self.modificadores_locais()?;
        self.expr_ou_decl_apos_mod(d)
    }

    /// `parseExpressionStatementOrConstDeclaration`.
    fn const_decl_ou_expr(&mut self) -> R<NoId> {
        let c = self.pos;
        if !self.modificador(c + 1) {
            let ti = self.tipo_em(c + 1, false, false, false);
            if ti.e_nenhum() {
                let n = c + 1;
                if !self.ident(n) {
                    return self.instrucao_expressao();
                }
                let nn = n + 1;
                if !(self.op(nn, Op::Assign)
                    || self.palavra(nn)
                    || self.op(nn, Op::Semicolon)
                    || self.op(nn, Op::Comma)
                    || self.op(nn, Op::LBrace))
                {
                    return self.instrucao_expressao();
                }
            }
            self.avancar();
            return self.expr_ou_decl_apos_mod(Decl {
                inicio: c,
                antes_tipo: c + 1,
                meta: Vec::new(),
                late: None,
                vfc: Some(c),
                ti: Some(ti),
            });
        }
        self.expr_ou_decl()
    }

    /// O teste de `parseExpressionStatementOrDeclarationAfterModifiers` para
    /// `Tipo? nome`: decide se é declaração ou o início de um condicional.
    fn interrogacao_e_condicional(&mut self, ti: InfoTipo) -> R<bool> {
        let next = ti.ate;
        if !self.parece_nome(next) {
            return Err(self.erro_em(next, "Expected an identifier."));
        }
        let depois = next + 1;
        if self.op(depois, Op::Assign) {
            let alcancado = self.sondar(|p| {
                p.pos = depois + 1;
                p.expressao_sem_cascata()
            });
            return Ok(alcancado.is_some_and(|f| self.op(f, Op::Colon)));
        }
        let palavra_chave = self.reservada(depois) || self.embutida(depois) || self.pseudo(depois);
        Ok(!palavra_chave
            && !(self.eof(depois)
                || self.op(depois, Op::Semicolon)
                || self.op(depois, Op::Comma)
                || self.op(depois, Op::RParen)))
    }

    /// `parseExpressionStatementOrDeclarationAfterModifiers` (comando).
    fn expr_ou_decl_apos_mod(&mut self, d: Decl) -> R<NoId> {
        if self.padroes
            && let Some(v) = d.vfc
            && (self.kw(v, Keyword::Var) || self.kw(v, Keyword::Final))
            && let Some(f) = self.pular_padrao_externo(d.antes_tipo)
            && self.op(f + 1, Op::Assign)
        {
            if d.late.is_some() {
                return Err(self.erro_em(d.late.unwrap_or(0), "A pattern variable declaration may not use the `late` keyword."));
            }
            return self.declaracao_padrao(d, v);
        }
        let mut ti = d.ti.unwrap_or_else(|| self.tipo_em(d.antes_tipo, false, false, false));
        if self.parece_funcao_local(ti.ate) {
            if d.vfc.is_some() || d.late.is_some() {
                return Err(self.erro_em(d.vfc.or(d.late).unwrap_or(0), "Can't have modifier here."));
            }
            return self.funcao_local(d, ti);
        }
        if d.sem_prefixo() && ti.nulavel() && ti.pode_ser_expressao && self.interrogacao_e_condicional(ti)? {
            ti = InfoTipo::nenhum(d.antes_tipo);
        }
        if ti.e_nenhum() && d.sem_prefixo() {
            return self.instrucao_expressao();
        }
        let next = ti.ate;
        if self.embutida(next) && d.sem_prefixo() && ti.pode_ser_expressao && self.pal(next, "as") {
            let k = next + 1;
            if !(self.op(k, Op::Assign) || self.op(k, Op::Semicolon) || self.op(k, Op::Comma)) {
                return self.instrucao_expressao();
            }
        }
        self.conferir_decl(&d, &ti)?;
        let ini = d.inicio;
        let type_ = if ti.e_nenhum() {
            None
        } else {
            Some(self.tipo_ate(ti.ate)?)
        };
        let variables = self.variaveis(d, type_)?;
        let s = self.espera_op(Op::Semicolon)?;
        Ok(self.no(
            No::VariableDeclarationStatement {
                variables,
                semicolon: s as Tk,
            },
            ini,
        ))
    }

    /// Erros de `var`/tipo ausente ou redundante.
    fn conferir_decl(&self, d: &Decl, ti: &InfoTipo) -> R<()> {
        if self.ident(ti.ate) {
            match d.vfc {
                None if ti.e_nenhum() => {
                    return Err(self.erro_em(ti.ate, "Variables must be declared using the keywords 'const', 'final', 'var' or a type name."));
                }
                Some(v) if self.kw(v, Keyword::Var) && !ti.e_nenhum() => {
                    return Err(self.erro_em(v, "Variables can't be declared using both 'var' and a type name."));
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// `nome (= expr)? (, nome (= expr)?)*` numa [`No::VariableDeclarationList`].
    fn variaveis(&mut self, d: Decl, type_: Option<NoId>) -> R<NoId> {
        let mut vars = Vec::new();
        loop {
            let n = self.espera_nome_declarado()?;
            let (equals, initializer) = if self.op(self.pos, Op::Assign) {
                let e = self.avancar();
                (Some(e as Tk), Some(self.expressao()?))
            } else {
                (None, None)
            };
            vars.push(self.no(
                No::VariableDeclaration {
                    name: n as Tk,
                    equals,
                    initializer,
                },
                n,
            ));
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.avancar();
        }
        Ok(self.no(
            No::VariableDeclarationList {
                metadata: d.meta,
                late_keyword: d.late.map(|l| l as Tk),
                keyword: d.vfc.map(|v| v as Tk),
                type_,
                variables: vars,
            },
            d.inicio,
        ))
    }

    /// `parsePatternVariableDeclarationStatement`.
    fn declaracao_padrao(&mut self, d: Decl, keyword: usize) -> R<NoId> {
        let ini = d.inicio;
        self.pos = d.antes_tipo;
        let pattern = self.padrao(CtxPadrao::Declaracao)?;
        let equals = self.espera_op(Op::Assign)?;
        let expression = self.expressao()?;
        let decl = self.no(
            No::PatternVariableDeclaration {
                metadata: d.meta,
                keyword: keyword as Tk,
                pattern,
                equals: equals as Tk,
                expression,
            },
            ini,
        );
        let s = self.espera_op(Op::Semicolon)?;
        Ok(self.no(
            No::PatternVariableDeclarationStatement {
                declaration: decl,
                semicolon: s as Tk,
            },
            ini,
        ))
    }

    /// Função local (`parseNamedFunctionRest` com
    /// `isFunctionExpression: false`).
    fn funcao_local(&mut self, d: Decl, ti: InfoTipo) -> R<NoId> {
        let ini = d.inicio;
        self.pos = d.antes_tipo;
        let return_type = if ti.e_nenhum() {
            None
        } else {
            Some(self.tipo_ate(ti.ate)?)
        };
        let nome = self.espera_nome_declarado()?;
        let fi = self.pos;
        let type_parameters = if self.op(self.pos, Op::Lt) {
            Some(self.params_tipo()?)
        } else {
            None
        };
        if !self.op(self.pos, Op::LParen) {
            return Err(self.esperado("("));
        }
        let parameters = self.params_formais(Membro::Local)?;
        if self.op(self.pos, Op::Colon) {
            return Err(self.erro("Only constructors can have initializers."));
        }
        let body = self.corpo_funcao(false, false)?;
        let fe = self.no(
            No::FunctionExpression {
                type_parameters,
                parameters: Some(parameters),
                body,
            },
            fi,
        );
        let fd = self.no(
            No::FunctionDeclaration {
                metadata: d.meta,
                augment_keyword: None,
                external_keyword: None,
                return_type,
                property_keyword: None,
                name: nome as Tk,
                function_expression: fe,
            },
            ini,
        );
        Ok(self.no(No::FunctionDeclarationStatement { function_declaration: fd }, ini))
    }

    /// `parseLabeledStatement`.
    fn rotulada(&mut self) -> R<NoId> {
        let ini = self.pos;
        let mut labels = Vec::new();
        while self.ident(self.pos) && self.op(self.pos + 1, Op::Colon) {
            let n = self.avancar();
            let s = self.simples(n);
            let c = self.avancar();
            labels.push(self.no(
                No::Label {
                    label: s,
                    colon: c as Tk,
                },
                n,
            ));
        }
        let statement = self.instrucao()?;
        Ok(self.no(No::LabeledStatement { labels, statement }, ini))
    }

    /// `parseBlock`.
    pub(super) fn bloco(&mut self) -> R<NoId> {
        let ini = self.pos;
        let lb = self.espera_op(Op::LBrace)?;
        let mut statements = Vec::new();
        while !self.op(self.pos, Op::RBrace) {
            if self.eof(self.pos) {
                return Err(self.esperado("}"));
            }
            statements.push(self.instrucao()?);
        }
        let rb = self.avancar();
        Ok(self.no(
            No::Block {
                left_bracket: lb as Tk,
                statements,
                right_bracket: rb as Tk,
            },
            ini,
        ))
    }

    fn gerador(&self) -> bool {
        matches!(self.assinc, Assinc::SincEstrela | Assinc::AssincEstrela)
    }

    fn assincrono(&self) -> bool {
        matches!(self.assinc, Assinc::Assinc | Assinc::AssincEstrela)
    }

    /// `parseReturnStatement`.
    fn retorno(&mut self) -> R<NoId> {
        let r = self.avancar();
        let expression = if self.op(self.pos, Op::Semicolon) {
            None
        } else {
            if self.gerador() {
                return Err(self.erro_em(r, "Generators can't return values."));
            }
            Some(self.expressao()?)
        };
        let s = self.espera_op(Op::Semicolon)?;
        Ok(self.no(
            No::ReturnStatement {
                return_keyword: r as Tk,
                expression,
                semicolon: s as Tk,
            },
            r,
        ))
    }

    /// `parseYieldStatement`.
    fn ceder(&mut self) -> R<NoId> {
        let y = self.avancar();
        let star = self.opcional_op(Op::Star).map(|s| s as Tk);
        let expression = self.expressao()?;
        let s = self.espera_op(Op::Semicolon)?;
        if !self.gerador() {
            return Err(self.erro_em(y, "Yield statements must be in a generator method."));
        }
        Ok(self.no(
            No::YieldStatement {
                yield_keyword: y as Tk,
                star,
                expression,
                semicolon: s as Tk,
            },
            y,
        ))
    }

    /// `parseIfStatement`.
    fn se(&mut self) -> R<NoId> {
        let i = self.avancar();
        let (lp, expression, case_clause, rp) = self.condicao_parenteses(self.padroes)?;
        let then_statement = self.instrucao()?;
        let (else_keyword, else_statement) = if self.kw(self.pos, Keyword::Else) {
            let e = self.avancar();
            (Some(e as Tk), Some(self.instrucao()?))
        } else {
            (None, None)
        };
        Ok(self.no(
            No::IfStatement {
                if_keyword: i as Tk,
                left_parenthesis: lp as Tk,
                expression,
                case_clause,
                right_parenthesis: rp as Tk,
                then_statement,
                else_keyword,
                else_statement,
            },
            i,
        ))
    }

    /// Corpo de laço (`loopState = InsideLoop`).
    fn corpo_laco(&mut self) -> R<NoId> {
        let salvo = self.laco;
        self.laco = Laco::DentroDeLaco;
        let r = self.instrucao();
        self.laco = salvo;
        r
    }

    /// `parseWhileStatement`.
    fn enquanto(&mut self) -> R<NoId> {
        let w = self.avancar();
        let (lp, condition, _, rp) = self.condicao_parenteses(false)?;
        let body = self.corpo_laco()?;
        Ok(self.no(
            No::WhileStatement {
                while_keyword: w as Tk,
                left_parenthesis: lp as Tk,
                condition,
                right_parenthesis: rp as Tk,
                body,
            },
            w,
        ))
    }

    /// `parseDoWhileStatement`.
    fn faca(&mut self) -> R<NoId> {
        let d = self.avancar();
        let body = self.corpo_laco()?;
        let w = self.espera_kw(Keyword::While)?;
        let (lp, condition, _, rp) = self.condicao_parenteses(false)?;
        let s = self.espera_op(Op::Semicolon)?;
        Ok(self.no(
            No::DoStatement {
                do_keyword: d as Tk,
                body,
                while_keyword: w as Tk,
                left_parenthesis: lp as Tk,
                condition,
                right_parenthesis: rp as Tk,
                semicolon: s as Tk,
            },
            d,
        ))
    }

    /// `parseForStatement`.
    fn para(&mut self, await_keyword: Option<usize>) -> R<NoId> {
        let ini = await_keyword.unwrap_or(self.pos);
        let f = self.espera_kw(Keyword::For)?;
        let (lp, for_loop_parts, rp) = self.partes_for(await_keyword)?;
        let body = self.corpo_laco()?;
        Ok(self.no(
            No::ForStatement {
                await_keyword: await_keyword.map(|a| a as Tk),
                for_keyword: f as Tk,
                left_parenthesis: lp as Tk,
                for_loop_parts,
                right_parenthesis: rp as Tk,
                body,
            },
            ini,
        ))
    }

    /// `parseExpressionStatementOrDeclaration` com `forPartsContext`.
    fn inicio_for(&mut self) -> R<InicioFor> {
        let d = self.modificadores_locais()?;
        if self.padroes
            && let Some(v) = d.vfc
            && (self.kw(v, Keyword::Var) || self.kw(v, Keyword::Final))
            && let Some(f) = self.pular_padrao_externo(d.antes_tipo)
            && (self.op(f + 1, Op::Assign) || self.kw(f + 1, Keyword::In))
        {
            if d.late.is_some() {
                return Err(self.erro_em(d.late.unwrap_or(0), "A pattern variable declaration may not use the `late` keyword."));
            }
            let pattern = self.padrao(CtxPadrao::Declaracao)?;
            return Ok(InicioFor::Padrao(d.meta, v, pattern, d.inicio));
        }
        let mut ti = self.tipo_em(d.antes_tipo, false, false, false);
        if let Some(l) = d.late {
            return Err(self.erro_em(l, "Can't have modifier 'late' here."));
        }
        if d.sem_prefixo() && ti.nulavel() && ti.pode_ser_expressao && self.interrogacao_e_condicional(ti)? {
            ti = InfoTipo::nenhum(d.antes_tipo);
        }
        if ti.e_nenhum() && d.sem_prefixo() {
            return Ok(InicioFor::Expressao);
        }
        let next = ti.ate;
        if self.embutida(next) && d.sem_prefixo() && ti.pode_ser_expressao && self.pal(next, "as") {
            let k = next + 1;
            if !(self.op(k, Op::Assign) || self.op(k, Op::Semicolon) || self.op(k, Op::Comma)) && !self.kw(k, Keyword::In) {
                return Ok(InicioFor::Expressao);
            }
        }
        self.conferir_decl(&d, &ti)?;
        let type_ = if ti.e_nenhum() {
            None
        } else {
            Some(self.tipo_ate(ti.ate)?)
        };
        Ok(InicioFor::Declaracao(d, type_))
    }

    /// O cabeçalho `( ... )` de `for` em comandos e coleções.
    pub(super) fn partes_for(&mut self, await_keyword: Option<usize>) -> R<(usize, NoId, usize)> {
        let lp = self.espera_op(Op::LParen)?;
        let conferir_await_in = |p: &Self| -> R<()> {
            if let Some(a) = await_keyword
                && !p.assincrono()
            {
                return Err(p.erro_em(a, "The asynchronous for-in can only be used in functions marked with 'async' or 'async*'."));
            }
            Ok(())
        };
        match self.inicio_for()? {
            InicioFor::Padrao(meta, keyword, pattern, ini) => {
                if self.op(self.pos, Op::Assign) {
                    let eq = self.avancar();
                    let expression = self.expressao()?;
                    let variables = self.no(
                        No::PatternVariableDeclaration {
                            metadata: meta,
                            keyword: keyword as Tk,
                            pattern,
                            equals: eq as Tk,
                            expression,
                        },
                        ini,
                    );
                    let parts = self.resto_for(await_keyword, ini, |l, c, r, u| No::ForPartsWithPattern {
                        variables,
                        left_separator: l,
                        condition: c,
                        right_separator: r,
                        updaters: u,
                    })?;
                    let rp = self.espera_op(Op::RParen)?;
                    return Ok((lp, parts, rp));
                }
                let in_kw = self.espera_kw(Keyword::In)?;
                conferir_await_in(self)?;
                let iterable = self.expressao()?;
                let parts = self.no(
                    No::ForEachPartsWithPattern {
                        metadata: meta,
                        keyword: keyword as Tk,
                        pattern,
                        in_keyword: in_kw as Tk,
                        iterable,
                    },
                    ini,
                );
                let rp = self.espera_op(Op::RParen)?;
                Ok((lp, parts, rp))
            }
            InicioFor::Declaracao(d, type_) => {
                if self.ident(self.pos) && (self.kw(self.pos + 1, Keyword::In) || self.op(self.pos + 1, Op::Colon)) {
                    if self.op(self.pos + 1, Op::Colon) {
                        return Err(self.erro_em(self.pos + 1, "For-in loops use 'in' rather than a colon."));
                    }
                    let n = self.espera_nome_declarado()?;
                    let ini = d.inicio;
                    let loop_variable = self.no(
                        No::DeclaredIdentifier {
                            metadata: d.meta,
                            keyword: d.vfc.map(|v| v as Tk),
                            type_,
                            name: n as Tk,
                        },
                        ini,
                    );
                    let in_kw = self.avancar();
                    conferir_await_in(self)?;
                    let iterable = self.expressao()?;
                    let parts = self.no(
                        No::ForEachPartsWithDeclaration {
                            loop_variable,
                            in_keyword: in_kw as Tk,
                            iterable,
                        },
                        ini,
                    );
                    let rp = self.espera_op(Op::RParen)?;
                    return Ok((lp, parts, rp));
                }
                let ini = d.inicio;
                let variables = self.variaveis(d, type_)?;
                let parts = self.resto_for(await_keyword, ini, |l, c, r, u| No::ForPartsWithDeclarations {
                    variables,
                    left_separator: l,
                    condition: c,
                    right_separator: r,
                    updaters: u,
                })?;
                let rp = self.espera_op(Op::RParen)?;
                Ok((lp, parts, rp))
            }
            InicioFor::Expressao => {
                let ini = self.pos;
                let initialization = if self.op(self.pos, Op::Semicolon) {
                    None
                } else {
                    Some(self.expressao()?)
                };
                if self.kw(self.pos, Keyword::In) || self.op(self.pos, Op::Colon) {
                    if self.op(self.pos, Op::Colon) {
                        return Err(self.erro("For-in loops use 'in' rather than a colon."));
                    }
                    let Some(identifier) = initialization.filter(|&e| self.e_simples(e)) else {
                        return Err(self.erro_em(ini, "A for-in loop variable must be a simple identifier."));
                    };
                    let in_kw = self.avancar();
                    conferir_await_in(self)?;
                    let iterable = self.expressao()?;
                    let parts = self.no(
                        No::ForEachPartsWithIdentifier {
                            identifier,
                            in_keyword: in_kw as Tk,
                            iterable,
                        },
                        ini,
                    );
                    let rp = self.espera_op(Op::RParen)?;
                    return Ok((lp, parts, rp));
                }
                let parts = self.resto_for(await_keyword, ini, |l, c, r, u| No::ForPartsWithExpression {
                    initialization,
                    left_separator: l,
                    condition: c,
                    right_separator: r,
                    updaters: u,
                })?;
                let rp = self.espera_op(Op::RParen)?;
                Ok((lp, parts, rp))
            }
        }
    }

    /// `parseForLoopPartsRest`: `; condição? ; atualizações`.
    fn resto_for(
        &mut self,
        await_keyword: Option<usize>,
        ini: usize,
        montar: impl FnOnce(Tk, Option<NoId>, Tk, Vec<NoId>) -> No,
    ) -> R<NoId> {
        if let Some(a) = await_keyword {
            return Err(self.erro_em(a, "The keyword 'await' isn't allowed for a normal 'for' statement."));
        }
        let l = self.espera_op(Op::Semicolon)?;
        let condition = if self.op(self.pos, Op::Semicolon) {
            None
        } else {
            Some(self.expressao()?)
        };
        let r = self.espera_op(Op::Semicolon)?;
        let mut updaters = Vec::new();
        loop {
            if self.op(self.pos, Op::RParen) {
                break;
            }
            updaters.push(self.expressao()?);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.avancar();
        }
        let no = montar(l as Tk, condition, r as Tk, updaters);
        Ok(self.no(no, ini))
    }

    /// `parseSwitchStatement` e `parseSwitchBlock`.
    fn escolha(&mut self) -> R<NoId> {
        let sw = self.avancar();
        let (lp, expression, _, rp) = self.condicao_parenteses(false)?;
        let salvo = self.laco;
        if self.laco == Laco::Fora {
            self.laco = Laco::DentroDeSwitch;
        }
        let lb = self.espera_op(Op::LBrace)?;
        let mut membros = Vec::new();
        let mut viu_default = false;
        while !self.op(self.pos, Op::RBrace) {
            if self.eof(self.pos) {
                return Err(self.esperado("}"));
            }
            // Membros do grupo: (rótulos, palavra, padrão/expressão, dois-pontos, início).
            let mut grupo: Vec<(Vec<NoId>, usize, Option<NoId>, usize, usize)> = Vec::new();
            let mut peek = self.alem_dos_rotulos(self.pos);
            loop {
                if self.kw(peek, Keyword::Default) || self.kw(peek, Keyword::Case) {
                    let ini = self.pos;
                    let mut labels = Vec::new();
                    while self.pos < peek {
                        let n = self.avancar();
                        let s = self.simples(n);
                        let c = self.avancar();
                        labels.push(self.no(
                            No::Label {
                                label: s,
                                colon: c as Tk,
                            },
                            n,
                        ));
                    }
                    if self.kw(peek, Keyword::Default) {
                        if viu_default {
                            return Err(self.erro("The 'default' case can only be declared once."));
                        }
                        viu_default = true;
                        let d = self.avancar();
                        let c = self.espera_op(Op::Colon)?;
                        grupo.push((labels, d, None, c, ini));
                        peek = self.pos;
                        break;
                    }
                    if viu_default {
                        return Err(self.erro("The default case should be the last case in a switch statement."));
                    }
                    let k = self.avancar();
                    let e = if self.padroes {
                        self.padrao_guardado()?
                    } else {
                        let e = self.expressao()?;
                        if self.pal(self.pos, "when") {
                            return Err(self.erro("Guards ('when') require patterns."));
                        }
                        e
                    };
                    let c = self.espera_op(Op::Colon)?;
                    grupo.push((labels, k, Some(e), c, ini));
                    peek = self.alem_dos_rotulos(self.pos);
                } else if !grupo.is_empty() {
                    break;
                } else {
                    return Err(self.erro_em(peek, "Expected 'case'."));
                }
            }
            let mut statements = Vec::new();
            while !self.eof(self.pos) {
                if self.kw(peek, Keyword::Case) || self.kw(peek, Keyword::Default) || (self.op(peek, Op::RBrace) && peek == self.pos)
                {
                    break;
                }
                statements.push(self.instrucao()?);
                peek = self.alem_dos_rotulos(self.pos);
            }
            let n = grupo.len();
            for (j, (labels, k, e, c, ini)) in grupo.into_iter().enumerate() {
                let sts = if j + 1 == n { std::mem::take(&mut statements) } else { Vec::new() };
                let fim = sts.last().map_or(c, |&s| self.fim(s));
                let no = match e {
                    None => No::SwitchDefault {
                        labels,
                        keyword: k as Tk,
                        colon: c as Tk,
                        statements: sts,
                    },
                    Some(g) if self.padroes => No::SwitchPatternCase {
                        labels,
                        keyword: k as Tk,
                        guarded_pattern: g,
                        colon: c as Tk,
                        statements: sts,
                    },
                    Some(x) => No::SwitchCase {
                        labels,
                        keyword: k as Tk,
                        expression: x,
                        colon: c as Tk,
                        statements: sts,
                    },
                };
                membros.push(self.no_entre(no, ini, fim));
            }
        }
        let rb = self.avancar();
        self.laco = salvo;
        Ok(self.no(
            No::SwitchStatement {
                switch_keyword: sw as Tk,
                left_parenthesis: lp as Tk,
                expression,
                right_parenthesis: rp as Tk,
                left_bracket: lb as Tk,
                members: membros,
                right_bracket: rb as Tk,
            },
            sw,
        ))
    }

    /// `peekPastLabels`.
    fn alem_dos_rotulos(&self, mut i: usize) -> usize {
        while self.ident(i) && self.op(i + 1, Op::Colon) {
            i += 2;
        }
        i
    }

    /// `parseTryStatement`.
    fn tente(&mut self) -> R<NoId> {
        let t = self.avancar();
        let body = self.bloco()?;
        let mut catch_clauses = Vec::new();
        while self.pal(self.pos, "on") || self.kw(self.pos, Keyword::Catch) {
            let ci = self.pos;
            let mut on_keyword = None;
            let mut exception_type = None;
            if self.pal(self.pos, "on") {
                on_keyword = Some(self.avancar() as Tk);
                exception_type = Some(self.tipo_requerido()?);
            }
            let mut catch_keyword = None;
            let mut lp = None;
            let mut exception_parameter = None;
            let mut comma = None;
            let mut stack_trace_parameter = None;
            let mut rp = None;
            if self.kw(self.pos, Keyword::Catch) {
                catch_keyword = Some(self.avancar() as Tk);
                lp = Some(self.espera_op(Op::LParen)? as Tk);
                let e = self.espera_nome_declarado()?;
                exception_parameter = Some(self.no(No::CatchClauseParameter { name: e as Tk }, e));
                if self.op(self.pos, Op::Comma) {
                    comma = Some(self.avancar() as Tk);
                    let s = self.espera_nome_declarado()?;
                    stack_trace_parameter = Some(self.no(No::CatchClauseParameter { name: s as Tk }, s));
                }
                rp = Some(self.espera_op(Op::RParen)? as Tk);
            }
            let b = self.bloco()?;
            catch_clauses.push(self.no(
                No::CatchClause {
                    on_keyword,
                    exception_type,
                    catch_keyword,
                    left_parenthesis: lp,
                    exception_parameter,
                    comma,
                    stack_trace_parameter,
                    right_parenthesis: rp,
                    body: b,
                },
                ci,
            ));
        }
        let (finally_keyword, finally_block) = if self.kw(self.pos, Keyword::Finally) {
            let f = self.avancar();
            (Some(f as Tk), Some(self.bloco()?))
        } else {
            (None, None)
        };
        if catch_clauses.is_empty() && finally_block.is_none() {
            return Err(self.erro_em(t, "A try block must be followed by an 'on', 'catch', or 'finally' clause."));
        }
        Ok(self.no(
            No::TryStatement {
                try_keyword: t as Tk,
                body,
                catch_clauses,
                finally_keyword,
                finally_block,
            },
            t,
        ))
    }

    /// `parseBreakStatement` e `parseContinueStatement`.
    fn quebra(&mut self, e_break: bool) -> R<NoId> {
        let k = self.avancar();
        let label = if self.ident(self.pos) {
            let n = self.avancar();
            Some(self.simples(n))
        } else {
            None
        };
        if e_break {
            if label.is_none() && self.laco == Laco::Fora {
                return Err(self.erro_em(k, "A break statement can't be used outside of a loop or switch statement."));
            }
        } else if label.is_some() {
            if self.laco == Laco::Fora {
                return Err(self.erro_em(k, "A continue statement can't be used outside of a loop or switch statement."));
            }
        } else if self.laco != Laco::DentroDeLaco {
            return Err(self.erro_em(k, "A continue statement can't be used outside of a loop or switch statement."));
        }
        let s = self.espera_op(Op::Semicolon)?;
        let no = if e_break {
            No::BreakStatement {
                break_keyword: k as Tk,
                label,
                semicolon: s as Tk,
            }
        } else {
            No::ContinueStatement {
                continue_keyword: k as Tk,
                label,
                semicolon: s as Tk,
            }
        };
        Ok(self.no(no, k))
    }

    /// `assert ( condição (, mensagem)? ,? )` — as partes comuns a
    /// [`No::AssertStatement`] e [`No::AssertInitializer`].
    pub(super) fn partes_assert(&mut self) -> R<(usize, usize, NoId, Option<usize>, Option<NoId>, usize)> {
        let a = self.espera_kw(Keyword::Assert)?;
        let lp = self.espera_op(Op::LParen)?;
        let old = self.funcoes;
        self.funcoes = true;
        let condition = self.expressao()?;
        let mut comma = None;
        let mut message = None;
        if self.op(self.pos, Op::Comma) {
            let c = self.avancar();
            if !self.op(self.pos, Op::RParen) {
                comma = Some(c);
                message = Some(self.expressao()?);
                if self.op(self.pos, Op::Comma) {
                    self.avancar();
                }
            }
        }
        let rp = self.espera_op(Op::RParen)?;
        self.funcoes = old;
        Ok((a, lp, condition, comma, message, rp))
    }

    /// `parseAssertStatement`.
    fn afirmacao(&mut self) -> R<NoId> {
        let (a, lp, condition, comma, message, rp) = self.partes_assert()?;
        let s = self.espera_op(Op::Semicolon)?;
        Ok(self.no(
            No::AssertStatement {
                assert_keyword: a as Tk,
                left_parenthesis: lp as Tk,
                condition,
                comma: comma.map(|c| c as Tk),
                message,
                right_parenthesis: rp as Tk,
                semicolon: s as Tk,
            },
            a,
        ))
    }

    /// `parseAsyncModifierOpt` e `parseFunctionBody`. `de_expressao` é o
    /// corpo de uma expressão de função (o `=>` não tem `;`).
    pub(super) fn corpo_funcao(&mut self, de_expressao: bool, permite_abstrato: bool) -> R<NoId> {
        let (assinc, laco) = (self.assinc, self.laco);
        let r = self.corpo_funcao_interno(de_expressao, permite_abstrato);
        self.assinc = assinc;
        self.laco = laco;
        r
    }

    fn corpo_funcao_interno(&mut self, de_expressao: bool, permite_abstrato: bool) -> R<NoId> {
        let mut keyword = None;
        let mut star = None;
        self.assinc = Assinc::Sinc;
        if self.pal(self.pos, "async") {
            keyword = Some(self.avancar());
            if self.op(self.pos, Op::Star) {
                star = Some(self.avancar());
                self.assinc = Assinc::AssincEstrela;
            } else {
                self.assinc = Assinc::Assinc;
            }
        } else if self.pal(self.pos, "sync") {
            keyword = Some(self.avancar());
            if self.op(self.pos, Op::Star) {
                star = Some(self.avancar());
                self.assinc = Assinc::SincEstrela;
            } else {
                return Err(self.erro_em(keyword.unwrap_or(0), "Invalid modifier 'sync'."));
            }
        }
        let ini = keyword.unwrap_or(self.pos);
        if self.assinc != Assinc::Sinc && self.op(self.pos, Op::Semicolon) {
            return Err(self.erro("Abstract methods can't use 'async', 'async*', or 'sync*'."));
        }
        if self.pal(self.pos, "native") {
            return Err(self.erro("Native clause can only be used in the SDK and code that is loaded through native extensions."));
        }
        if self.op(self.pos, Op::Semicolon) {
            if !permite_abstrato {
                return Err(self.erro("A function body must be provided."));
            }
            let s = self.avancar();
            return Ok(self.no(No::EmptyFunctionBody { semicolon: s as Tk }, s));
        }
        if self.op(self.pos, Op::Arrow) {
            let a = self.avancar();
            let expression = self.expressao()?;
            let semicolon = if de_expressao {
                None
            } else {
                Some(self.espera_op(Op::Semicolon)? as Tk)
            };
            if self.gerador() {
                return Err(self.erro_em(a, "Generators can't return values."));
            }
            return Ok(self.no(
                No::ExpressionFunctionBody {
                    keyword: keyword.map(|k| k as Tk),
                    star: star.map(|s| s as Tk),
                    function_definition: a as Tk,
                    expression,
                    semicolon,
                },
                ini,
            ));
        }
        if !self.op(self.pos, Op::LBrace) {
            return Err(self.erro("A function body must be provided."));
        }
        self.laco = Laco::Fora;
        let block = self.bloco()?;
        Ok(self.no(
            No::BlockFunctionBody {
                keyword: keyword.map(|k| k as Tk),
                star: star.map(|s| s as Tk),
                block,
            },
            ini,
        ))
    }
}

#[allow(non_snake_case)]
fn EmptyStatement(i: usize) -> No {
    No::EmptyStatement { semicolon: i as Tk }
}
