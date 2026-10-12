//! O tipo de contexto do completar: o `FeatureComputer.computeContextType`
//! e o `_ContextTypeVisitor` do analysis server 3.6.2
//! (`AS:src/services/completion/dart/feature_computer.dart:174-182`,
//! `:439-1270`; docs/LSP-ESPECIFICACAO.md §14.6.5), sobre a árvore do texto
//! real, com os tipos da consulta. O `bodyContext.contextType` é o do
//! `BodyInferenceContext` (`AN:src/dart/resolver/body_inference_context.dart`):
//! o tipo imposto (o retorno declarado do método, construtor ou função; o
//! retorno do tipo de contexto de uma closure, quando é tipo de função)
//! ajustado por `async`/`sync*`/`async*`.
//! Escrito sem compilar nem executar (2026-10-05).

use super::isp::Isp;
use super::*;
use crate::arvore_analyzer::Ligacao;
use dartforge_frontend::token::Op;

/// Os nomes com tipo implícito (`_impliedDartTypeWithName`, `FC:28-44`).
const NOMES_INT: &[&str] = &["i", "j", "index", "length"];
const NOMES_NUM: &[&str] = &["height", "width"];
const NOMES_LISTA: &[&str] = &["list", "items"];
const NOMES_STRING: &[&str] = &["key", "text", "url", "uri", "name", "str", "string"];

impl<'a, 'c> Isp<'a, 'c> {
    /// `computeContextType(node, offset)`: `null`/`dynamic` → nenhum; senão
    /// `resolveToBound`.
    pub(super) fn tipo_de_contexto_em(&mut self, no: usize, o: usize) -> Option<TypeId> {
        let t = self.ctx(no, o)?;
        if matches!(self.consulta.tabela.get(t), Type::Dynamic) {
            return None;
        }
        Some(self.resolver_ao_limite(t))
    }

    /// `typeSystem.resolveToBound`.
    fn resolver_ao_limite(&mut self, t: TypeId) -> TypeId {
        let mut atual = t;
        for _ in 0..32 {
            match self.consulta.tabela.get(atual).clone() {
                Type::TypeParameter { param, nullable } => {
                    let b = self.consulta.tabela.param(param).bound;
                    atual = if nullable { dartforge_types::nullable(b, &mut self.consulta.tabela) } else { b };
                }
                Type::Intersection { bound, .. } => atual = bound,
                _ => break,
            }
        }
        atual
    }

    /// `_visitParent`.
    fn ctx_pai(&mut self, n: usize, o: usize) -> Option<TypeId> {
        let p = self.pai(n)?;
        self.ctx(p, o)
    }

    /// `node.contains(o)` do visitante (`offset <= o <= end`).
    fn contem_o(&self, n: usize, o: usize) -> bool {
        self.ini(n) <= o && o <= self.fim(n)
    }

    /// O offset de um token (o do próximo token real quando falta).
    fn pos_op(&self, op: Op, de: usize, ate: usize) -> (usize, usize) {
        match self.op_em(op, de, ate) {
            Some(i) => (self.span_tok(i).start, self.span_tok(i).end),
            None => {
                let p = self.sintetico(de).start;
                (p, p)
            }
        }
    }

    /// `(` e `)` de um nó (os do primeiro `(` no nível do nó).
    fn parenteses(&self, n: usize) -> ((usize, usize), (usize, usize)) {
        match self.op_em(Op::LParen, self.ini(n), self.fim(n)) {
            Some(a) => {
                let abre = (self.span_tok(a).start, self.span_tok(a).end);
                let fecha = match self.par(a).filter(|&f| self.span_tok(f).end <= self.fim(n)) {
                    Some(f) => (self.span_tok(f).start, self.span_tok(f).end),
                    None => {
                        let p = self.sintetico(self.fim(n)).start;
                        (p, p)
                    }
                };
                (abre, fecha)
            }
            None => {
                let p = self.sintetico(self.ini(n)).start;
                ((p, p), (p, p))
            }
        }
    }

    /// `range.endStart(a, b).contains(o)`: `a.end <= o <= b.offset`.
    fn entre(a: (usize, usize), b: (usize, usize), o: usize) -> bool {
        a.1 <= o && o <= b.0
    }

    /// O tipo estático de um nó de expressão.
    fn estatico(&self, n: usize) -> Option<TypeId> {
        self.expr_real(n).and_then(|e| self.tipo_estatico(e))
    }

    /// Um tipo de interface com os argumentos.
    fn interface(&mut self, c: Option<ClassId>, args: &[TypeId]) -> Option<TypeId> {
        let c = c?;
        Some(self.consulta.tabela.intern(Type::Interface { class: c, args: args.into(), nullable: false }))
    }

    /// `asInstanceOf(classe).typeArguments`.
    fn argumentos_como(&mut self, t: TypeId, c: Option<ClassId>) -> Option<Vec<TypeId>> {
        let c = c?;
        let Consulta { tabela, outline, core, .. } = &mut *self.consulta;
        let s = outline.hierarchy.supertype_of(t, c, tabela, core)?;
        match tabela.get(s) {
            Type::Interface { args, .. } => Some(args.to_vec()),
            _ => None,
        }
    }

    /// O primeiro parâmetro do operador `nome` no tipo `alvo`
    /// (`staticParameterElement`/`element.formalParameters[0]`).
    fn parametro_do_operador(&mut self, alvo: TypeId, nome: &str) -> Option<TypeId> {
        let s = self.consulta.nomes.lookup(nome)?;
        let lib = self.biblioteca();
        let (_, t) = self.consulta.resolvedor().lookup_member(alvo, s, false, lib)?;
        match self.consulta.tabela.get(t) {
            Type::Function { positional, .. } => positional.first().copied(),
            _ => None,
        }
    }

    /// `node.writeType` de uma atribuição: o tipo declarado do alvo (local,
    /// campo, variável de topo, setter), não o promovido.
    fn tipo_de_escrita(&mut self, alvo: usize) -> Option<TypeId> {
        let e = self.expr_real(alvo)?;
        let c = self.expr_da_consulta(e)?;
        let corpos = &self.consulta.corpos.units[self.unidade.0 as usize];
        if let Some(&off) = corpos.declaracoes_de_locais.get(&c)
            && let Some(t) = corpos.tipo_local(off)
        {
            return Some(t);
        }
        match corpos.get_resolved(c).cloned() {
            Some(Resolved::Element(Element::Variable(v))) => self.consulta.tipo_da_variavel(v),
            Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => self.consulta.tipo_da_variavel(v),
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::Element(Element::Function(f))) | Some(Resolved::ExtensionMember { member: f, .. }) => {
                let dados = &self.consulta.outline.functions[f.0 as usize];
                match self.consulta.programa.function(f).kind {
                    FunctionKind::Setter => dados.parameters.first().map(|p| p.ty),
                    FunctionKind::ImplicitAccessor => self.consulta.programa.function(f).variable.and_then(|v| self.consulta.tipo_da_variavel(v)),
                    _ => corpos.get_type(c),
                }
            }
            _ => corpos.get_type(c),
        }
    }

    /// `futureValueType`.
    fn valor_futuro(&mut self, t: TypeId) -> TypeId {
        match self.consulta.tabela.get(t).clone() {
            Type::Void | Type::Dynamic => t,
            Type::FutureOr { arg, .. } => arg,
            Type::Interface { class, args, .. } if Some(class) == self.consulta.core.future_class && args.len() == 1 => args[0],
            _ => self.consulta.core.object_nullable,
        }
    }

    /// `_contextTypeForImposed`.
    fn contexto_do_imposto(&mut self, imposto: Option<TypeId>, modificador: ast::AsyncModifier) -> Option<TypeId> {
        let r = imposto?;
        let (assincrono, gerador) = match modificador {
            ast::AsyncModifier::None => (false, false),
            ast::AsyncModifier::Async => (true, false),
            ast::AsyncModifier::SyncStar => (false, true),
            ast::AsyncModifier::AsyncStar => (true, true),
        };
        if !assincrono && !gerador {
            return Some(r);
        }
        if gerador && assincrono {
            let stream = self.consulta.core.stream_class;
            if let Some(a) = self.argumentos_como(r, stream) {
                return a.first().copied();
            }
        }
        if gerador && !assincrono {
            let iterable = self.consulta.core.iterable_class;
            if let Some(a) = self.argumentos_como(r, iterable) {
                return a.first().copied();
            }
        }
        let v = self.valor_futuro(r);
        Some(self.consulta.tabela.intern(Type::FutureOr { arg: v, nullable: false }))
    }

    /// `functionBody.bodyContext?.contextType` do corpo `corpo`.
    fn contexto_do_corpo(&mut self, corpo: usize) -> Option<TypeId> {
        let dono = self.pai(corpo)?;
        let (imposto, modificador) = match self.especie(dono) {
            "MethodDeclaration" => {
                let f = self.funcao_real(dono)?;
                let modificador = f.modifier;
                let el = f.name.and_then(|n| self.funcao_declarada(n.span.start));
                let r = el.map(|e| self.consulta.outline.functions[e.0 as usize].return_type);
                (r.filter(|&t| !matches!(self.consulta.tabela.get(t), Type::Dynamic)), modificador)
            }
            "ConstructorDeclaration" => {
                let c = self.pai(dono).and_then(|k| self.classe_do_no(k));
                (c.map(|c| self.tipo_this(c)), ast::AsyncModifier::None)
            }
            "FunctionExpression" => {
                let f = self.funcao_real(dono)?;
                let modificador = f.modifier;
                let declaracao = self.pai(dono).filter(|&d| self.especie(d) == "FunctionDeclaration");
                let imposto = match declaracao {
                    Some(_) => {
                        // O tipo do elemento: o retorno escrito (local), ou o
                        // do outline (topo).
                        let local = f.name.and_then(|n| self.tipo_do_local(n.span.start));
                        match local {
                            Some(t) => match f.return_type {
                                Some(_) => match self.consulta.tabela.get(t) {
                                    Type::Function { ret, .. } => Some(*ret),
                                    _ => None,
                                },
                                None => None,
                            },
                            None => f.name.and_then(|n| self.funcao_declarada(n.span.start)).map(|e| self.consulta.outline.functions[e.0 as usize].return_type),
                        }
                    }
                    None => {
                        // A closure: o retorno do tipo de contexto dela.
                        let ctx = self.pai(dono).and_then(|p| self.ctx(p, self.ini(dono)));
                        match ctx.map(|t| self.consulta.tabela.get(t).clone()) {
                            Some(Type::Function { ret, .. }) => Some(ret),
                            _ => None,
                        }
                    }
                };
                (imposto.filter(|&t| !matches!(self.consulta.tabela.get(t), Type::Dynamic)), modificador)
            }
            _ => (None, ast::AsyncModifier::None),
        };
        self.contexto_do_imposto(imposto, modificador)
    }

    /// `_impliedDartTypeWithName`.
    fn tipo_pelo_nome(&mut self, nome: &str) -> Option<TypeId> {
        let core = &self.consulta.core;
        if nome.is_empty() {
            return None;
        }
        if NOMES_INT.contains(&nome) {
            return Some(core.int);
        }
        if NOMES_NUM.contains(&nome) {
            return Some(core.num);
        }
        if NOMES_STRING.contains(&nome) {
            return Some(core.string);
        }
        let d = core.dynamic_;
        if NOMES_LISTA.contains(&nome) {
            let l = core.list_class;
            return self.interface(l, &[d]);
        }
        if nome == "iterator" {
            let i = core.iterable_class;
            return self.interface(i, &[d]);
        }
        if nome == "map" {
            let m = core.map_class;
            return self.interface(m, &[d, d]);
        }
        None
    }

    /// O tipo escrito de uma lista de variáveis (`parent.type?.type`).
    fn tipo_da_lista(&self, lista: usize) -> Option<TypeId> {
        let l = self.lista_real(lista)?;
        let t = l.ty?;
        let no = self.filhos(lista).iter().copied().find(|&k| self.lig(k) == Ligacao::Tipo(t))?;
        self.tipo_do_no_de_tipo(no)
    }

    /// `visitVariableDeclaration` de um `VariableDeclaration`.
    fn ctx_variavel(&mut self, v: usize, o: usize) -> Option<TypeId> {
        let var = self.variavel_real(v)?;
        let igual = var.initializer.map(|_| self.pos_op(Op::Assign, var.name.span.end, self.fim(v).max(var.name.span.end + 1)));
        let Some((_, fim_igual)) = igual else { return None };
        if fim_igual > o {
            return None;
        }
        let lista = self.pai(v).filter(|&l| self.especie(l) == "VariableDeclarationList")?;
        if let Some(t) = self.tipo_da_lista(lista) {
            return Some(t);
        }
        let nome = self.nome_real(var.name).to_string();
        self.tipo_pelo_nome(&nome)
    }

    /// O `requiredType` de um padrão (`tipos_de_padroes`).
    fn tipo_requerido(&self, padrao: usize) -> Option<TypeId> {
        let Ligacao::Padrao(pid) = self.lig(padrao) else { return None };
        let s = self.a.pattern(pid).span;
        let (ini, fim) = (self.mapear(s.start), self.mapear_fim(s.end));
        let ast_c = &self.consulta.programa.unit(self.unidade).ast;
        let id = ast_c.patterns.iter().position(|x| x.span.start == ini && x.span.end == fim)?;
        self.consulta.corpos.units[self.unidade.0 as usize].tipos_de_padroes.get(&ast::PatternId(id as u32)).copied()
    }

    /// `_requiredTypeOfPattern`.
    fn tipo_requerido_do_padrao(&mut self, mut p: usize) -> Option<TypeId> {
        while self.especie(p) == "ParenthesizedPattern" {
            p = *self.filhos(p).first()?;
        }
        match self.especie(p) {
            "AssignedVariablePattern" => {
                let i = self.tok_depois(self.ini(p))?;
                let e = self.consulta.corpos.units[self.unidade.0 as usize].declaracoes_de_padroes.iter().find(|(pid, _)| {
                    let s = self.consulta.programa.unit(self.unidade).ast.pattern(**pid).span;
                    s.start == self.mapear(self.span_tok(i).start)
                });
                let off = e.map(|(_, &off)| off)?;
                self.consulta.corpos.units[self.unidade.0 as usize].tipo_local(off)
            }
            "DeclaredVariablePattern" => {
                let i = self.nome_do_padrao_de_variavel(p)?;
                self.tipo_do_local(self.span_tok(i).start)
            }
            "ListPattern" => self.tipo_requerido(p),
            _ => None,
        }
    }

    /// `_ContextTypeVisitor.visit*` no nó `n` com o offset `o`.
    pub(super) fn ctx(&mut self, n: usize, o: usize) -> Option<TypeId> {
        let core_bool = self.consulta.core.bool_;
        match self.especie(n) {
            "AdjacentStrings" => {
                if o == self.ini(n) {
                    self.ctx_pai(n, o)
                } else {
                    Some(self.consulta.core.string)
                }
            }
            "ArgumentList" => self.ctx_argumentos(n, o),
            "AsExpression" => {
                let expr = *self.filhos(n).first()?;
                let como = self.palavra_em("as", self.fim(expr), self.fim(n))?;
                if self.span_tok(como).end < o { self.estatico(expr) } else { None }
            }
            "AssertInitializer" | "AssertStatement" => {
                let (abre, fecha) = self.parenteses(n);
                let virgula = self.op_em(Op::Comma, abre.1, fecha.0.max(abre.1));
                let direita = virgula.map_or(fecha, |v| (self.span_tok(v).start, self.span_tok(v).end));
                if Self::entre(abre, direita, o) { Some(core_bool) } else { None }
            }
            "AssignmentExpression" => {
                let (&l, &r) = (self.filhos(n).first()?, self.filhos(n).get(1)?);
                let op = self.tok_depois(self.fim(l)).filter(|&i| self.span_tok(i).end <= self.ini(r))?;
                if self.span_tok(op).end > o {
                    return None;
                }
                let texto = self.texto_tok(op);
                if texto == "=" {
                    return self.tipo_de_escrita(l);
                }
                // Composto: o método do operador (`+=` → `+`; `??=` não tem).
                let metodo = texto.trim_end_matches('=');
                if metodo == "??" {
                    return None;
                }
                let alvo = self.estatico(l)?;
                self.parametro_do_operador(alvo, metodo)
            }
            "AwaitExpression" | "ConstructorName" | "ConstructorReference" | "PrefixedIdentifier" | "PropertyAccess" | "SimpleIdentifier" => self.ctx_pai(n, o),
            "BinaryExpression" => {
                let (&l, &r) = (self.filhos(n).first()?, self.filhos(n).get(1)?);
                let op = self.tok_depois(self.fim(l)).filter(|&i| self.span_tok(i).end <= self.ini(r));
                match op {
                    Some(i) if self.span_tok(i).end <= o => {
                        let texto = self.texto_tok(i);
                        if matches!(texto, "&&" | "||" | "??") {
                            return None;
                        }
                        let metodo = if texto == "!=" { "==" } else { texto };
                        let alvo = self.estatico(l)?;
                        self.parametro_do_operador(alvo, metodo)
                    }
                    _ => self.ctx_pai(n, o),
                }
            }
            "CascadeExpression" => {
                let alvo = *self.filhos(n).first()?;
                if o == self.ini(alvo) { self.ctx_pai(n, o) } else { None }
            }
            "ConditionalExpression" => {
                let c = *self.filhos(n).first()?;
                let (q, _) = self.pos_op(Op::Question, self.fim(c), self.fim(n));
                if o <= q { Some(core_bool) } else { self.ctx_pai(n, o) }
            }
            "ConstructorFieldInitializer" => {
                let (_, fim_igual) = self.pos_op(Op::Assign, self.ini(n), self.fim(n).max(self.ini(n) + 1));
                if fim_igual > o {
                    return None;
                }
                let ctor = self.pai(n)?;
                let v = self.campo_do_inicializador(ctor, n)?;
                self.consulta.tipo_da_variavel(v)
            }
            "DefaultFormalParameter" => {
                let interno = *self.filhos(n).first()?;
                let sep = self.op_em(Op::Assign, self.fim(interno), self.fim(n)).or_else(|| self.op_em(Op::Colon, self.fim(interno), self.fim(n)))?;
                if self.span_tok(sep).end > o {
                    return None;
                }
                let prm = self.parametro_real(interno)?;
                let nome = prm.name?;
                self.tipo_do_local(nome.span.start).or_else(|| {
                    let texto = self.nome_real(nome).to_string();
                    self.tipo_do_parametro_declarado(interno, &texto)
                })
            }
            "DoStatement" | "IfElement" | "IfStatement" | "WhileStatement" => {
                let (abre, fecha) = self.parenteses(n);
                if Self::entre(abre, fecha, o) { Some(core_bool) } else { None }
            }
            "ExpressionFunctionBody" => {
                let seta = self.op_em(Op::Arrow, self.ini(n), self.fim(n))?;
                if !(self.span_tok(seta).end <= o && o <= self.fim(n)) {
                    return None;
                }
                let pai = self.pai(n)?;
                match self.especie(pai) {
                    "MethodDeclaration" => self.contexto_do_corpo(n),
                    "FunctionExpression" => {
                        if let Some(t) = self.contexto_do_corpo(n) {
                            return Some(t);
                        }
                        if self.pai(pai).is_some_and(|d| self.especie(d) == "FunctionDeclaration") {
                            return None;
                        }
                        self.ctx_pai(pai, o)
                    }
                    _ => None,
                }
            }
            "FieldDeclaration" | "TopLevelVariableDeclaration" => {
                let l = self.filho(n, "VariableDeclarationList")?;
                if self.contem_o(l, o) { self.ctx(l, o) } else { None }
            }
            "ForEachPartsWithDeclaration" | "ForEachPartsWithIdentifier" | "ForEachPartsWithPattern" => {
                let em = self.palavra_em("in", self.ini(n), self.fim(n))?;
                if !(self.span_tok(em).end <= o && o <= self.fim(n)) {
                    return None;
                }
                let d = self.consulta.core.dynamic_;
                let com_await = self.especie(n) != "ForEachPartsWithPattern"
                    && self.pai(n).is_some_and(|p| matches!(self.especie(p), "ForStatement" | "ForElement") && self.tok_depois(self.ini(p)).is_some_and(|i| self.texto_tok(i) == "await"));
                if com_await {
                    let s = self.consulta.core.stream_class;
                    self.interface(s, &[d])
                } else {
                    let i = self.consulta.core.iterable_class;
                    self.interface(i, &[d])
                }
            }
            "ForPartsWithDeclarations" | "ForPartsWithExpression" | "ForPartsWithPattern" => {
                let p1 = self.op_em(Op::Semicolon, self.ini(n), self.fim(n).max(self.ini(n) + 1));
                let a = p1.map_or_else(|| (self.sintetico(self.ini(n)).start, self.sintetico(self.ini(n)).start), |i| (self.span_tok(i).start, self.span_tok(i).end));
                let p2 = p1.and_then(|i| self.op_em(Op::Semicolon, self.span_tok(i).end, self.fim(n).max(self.span_tok(i).end + 1)));
                let b = p2.map_or_else(|| (self.sintetico(a.1).start, self.sintetico(a.1).start), |i| (self.span_tok(i).start, self.span_tok(i).end));
                if Self::entre(a, b, o) { Some(core_bool) } else { None }
            }
            "FunctionExpressionInvocation" => {
                let f = *self.filhos(n).first()?;
                if self.contem_o(f, o) { self.ctx_pai(n, o) } else { None }
            }
            "IndexExpression" => {
                let alvo = *self.filhos(n).first()?;
                let abre = self.op_em(Op::LBracket, self.fim(alvo), self.fim(n))?;
                let fecha = self.par(abre).map_or_else(|| self.sintetico(self.fim(n)).start, |f| self.span_tok(f).start);
                if !(self.span_tok(abre).end <= o && o <= fecha) {
                    return None;
                }
                let t = self.estatico(alvo)?;
                self.parametro_do_operador(t, "[]")
            }
            "IsExpression" => {
                let expr = *self.filhos(n).first()?;
                let e = self.palavra_em("is", self.fim(expr), self.fim(n))?;
                if self.span_tok(e).end < o { self.estatico(expr) } else { None }
            }
            "Label" => {
                if o == self.ini(n) || self.fim(n) <= o {
                    self.ctx_pai(n, o)
                } else {
                    None
                }
            }
            "ListLiteral" => {
                let abre = self.op_em(Op::LBracket, self.ini(n), self.fim(n))?;
                let fecha = self.par(abre).map_or_else(|| self.sintetico(self.fim(n)).start, |f| self.span_tok(f).start);
                if !(self.span_tok(abre).end <= o && o <= fecha) {
                    return None;
                }
                match self.estatico(n).map(|t| self.consulta.tabela.get(t).clone()) {
                    Some(Type::Interface { args, .. }) => args.first().copied(),
                    _ => None,
                }
            }
            "ListPattern" => {
                let abre = self.op_em(Op::LBracket, self.ini(n), self.fim(n))?;
                let fecha = self.par(abre).map_or_else(|| self.sintetico(self.fim(n)).start, |f| self.span_tok(f).start);
                if !(self.span_tok(abre).end <= o && o <= fecha) {
                    return None;
                }
                match self.tipo_requerido(n).map(|t| self.consulta.tabela.get(t).clone()) {
                    Some(Type::Interface { args, .. }) => args.first().copied(),
                    _ => None,
                }
            }
            "MapLiteralEntry" => {
                let lit = self.ancestral(n, &["SetOrMapLiteral"])?;
                let t = self.estatico(lit)?;
                let Type::Interface { class, args, .. } = self.consulta.tabela.get(t).clone() else { return None };
                if Some(class) != self.consulta.core.map_class || args.len() != 2 {
                    return None;
                }
                let chave = *self.filhos(n).first()?;
                let (sep, _) = self.pos_op(Op::Colon, self.fim(chave), self.fim(n).max(self.fim(chave) + 1));
                Some(if o <= sep { args[0] } else { args[1] })
            }
            "MapPattern" => {
                let abre = self.op_em(Op::LBrace, self.ini(n), self.fim(n))?;
                let fecha = self.par(abre).map_or_else(|| self.sintetico(self.fim(n)).start, |f| self.span_tok(f).start);
                if !(self.span_tok(abre).end <= o && o <= fecha) {
                    return None;
                }
                match self.tipo_requerido(n).map(|t| self.consulta.tabela.get(t).clone()) {
                    Some(Type::Interface { class, args, .. }) if Some(class) == self.consulta.core.map_class && args.len() == 2 => Some(args[0]),
                    _ => None,
                }
            }
            "MapPatternEntry" => {
                let padrao = self.pai(n).filter(|&p| self.especie(p) == "MapPattern")?;
                let Type::Interface { class, args, .. } = self.consulta.tabela.get(self.tipo_requerido(padrao)?).clone() else { return None };
                if Some(class) != self.consulta.core.map_class || args.len() != 2 {
                    return None;
                }
                let chave = *self.filhos(n).first()?;
                let (sep, _) = self.pos_op(Op::Colon, self.fim(chave), self.fim(n).max(self.fim(chave) + 1));
                Some(if o <= sep { args[0] } else { args[1] })
            }
            "MethodInvocation" => {
                if o == self.ini(n) {
                    self.ctx_pai(n, o)
                } else {
                    None
                }
            }
            "NamedExpression" => {
                let rotulo = self.filho(n, "Label");
                if o == self.ini(n) || rotulo.is_some_and(|r| self.fim(r) <= o) {
                    self.ctx_pai(n, o)
                } else {
                    None
                }
            }
            "ParenthesizedExpression" => {
                let t = self.ctx_pai(n, o)?;
                match self.consulta.tabela.get(t).clone() {
                    Type::Record { positional, .. } => positional.first().copied(),
                    _ => Some(t),
                }
            }
            "PatternAssignment" | "PatternVariableDeclaration" => {
                let padrao = self.filhos(n).iter().copied().find(|&k| super::isp::e_padrao(self.especie(k)))?;
                let (_, fim_igual) = self.pos_op(Op::Assign, self.fim(padrao), self.fim(n).max(self.fim(padrao) + 1));
                if o >= fim_igual { self.tipo_requerido_do_padrao(padrao) } else { None }
            }
            "PatternField" => {
                let pai = self.pai(n)?;
                match self.especie(pai) {
                    "ObjectPattern" => self.ctx_campo_de_objeto(pai, n, o),
                    "RecordPattern" => self.ctx_campo_de_record(pai, n, o),
                    _ => None,
                }
            }
            "PostfixExpression" | "PrefixExpression" => None,
            "RecordLiteral" => self.ctx_record(n, o),
            "RecordPattern" => {
                let (abre, fecha) = self.parenteses(n);
                if !Self::entre(abre, fecha, o) {
                    return None;
                }
                let Type::Record { positional, .. } = self.consulta.tabela.get(self.tipo_casado(n)?).clone() else { return None };
                let i = self.indice_posicional(n, o);
                positional.get(i).copied()
            }
            "ReturnStatement" => {
                let palavra = self.tok_depois(self.ini(n))?;
                if self.span_tok(palavra).end < o {
                    let corpo = self.corpo_que_contem(n)?;
                    self.contexto_do_corpo(corpo)
                } else {
                    None
                }
            }
            "SetOrMapLiteral" => {
                let t = self.estatico(n)?;
                let abre = self.op_em(Op::LBrace, self.ini(n), self.fim(n))?;
                let fecha = self.par(abre).map_or_else(|| self.sintetico(self.fim(n)).start, |f| self.span_tok(f).start);
                let Type::Interface { class, args, .. } = self.consulta.tabela.get(t).clone() else { return None };
                let mapa_ou_conjunto = Some(class) == self.consulta.core.map_class || Some(class) == self.consulta.core.set_class;
                if self.span_tok(abre).end <= o && o <= fecha && mapa_ou_conjunto { args.first().copied() } else { None }
            }
            "SimpleStringLiteral" => None,
            "SpreadElement" => {
                let op = self.tok_depois(self.ini(n))?;
                if self.span_tok(op).end > o {
                    return None;
                }
                let d = self.consulta.core.dynamic_;
                let mut atual = self.pai(n);
                while let Some(k) = atual {
                    match self.especie(k) {
                        "ListLiteral" => {
                            let i = self.consulta.core.iterable_class;
                            return self.interface(i, &[d]);
                        }
                        "SetOrMapLiteral" => {
                            if self.e_conjunto(k) {
                                let i = self.consulta.core.iterable_class;
                                return self.interface(i, &[d]);
                            }
                            let m = self.consulta.core.map_class;
                            return self.interface(m, &[d, d]);
                        }
                        _ => {}
                    }
                    atual = self.pai(k);
                }
                None
            }
            "SwitchCase" => {
                let palavra = self.tok_depois(self.ini(n))?;
                let (dp, _) = self.pos_op(Op::Colon, self.span_tok(palavra).end, self.fim(n).max(self.span_tok(palavra).end + 1));
                if !(self.span_tok(palavra).end <= o && o <= dp) {
                    return None;
                }
                let sw = self.pai(n).filter(|&p| self.especie(p) == "SwitchStatement")?;
                let expr = self.filhos(sw).iter().copied().find(|&k| super::isp::ESPECIES_DE_EXPRESSAO.contains(&self.especie(k)))?;
                self.estatico(expr)
            }
            "VariableDeclaration" => self.ctx_variavel(n, o),
            "VariableDeclarationList" => {
                for v in self.filhos_de(n, &["VariableDeclaration"]) {
                    if self.contem_o(v, o) {
                        let var = self.variavel_real(v)?;
                        let igual = var.initializer.map(|_| self.pos_op(Op::Assign, var.name.span.end, self.fim(v).max(var.name.span.end + 1)));
                        if let Some((_, fim_igual)) = igual
                            && fim_igual <= o
                        {
                            if let Some(t) = self.tipo_da_lista(n) {
                                return Some(t);
                            }
                            let nome = self.nome_real(var.name).to_string();
                            return self.tipo_pelo_nome(&nome);
                        }
                    }
                }
                None
            }
            "WhenClause" => Some(core_bool),
            "YieldStatement" => {
                let palavra = self.tok_depois(self.ini(n))?;
                let pv = self.tok_antes(self.fim(n)).filter(|&i| self.e_op(i, Op::Semicolon)).map_or_else(|| self.sintetico(self.fim(n)).start, |i| self.span_tok(i).start);
                if self.span_tok(palavra).end <= o && o <= pv {
                    let corpo = self.corpo_que_contem(n)?;
                    self.contexto_do_corpo(corpo)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// `SetOrMapLiteral.isSet` (pelo tipo estático; sem tipo, pelos
    /// elementos: nenhum `MapLiteralEntry`).
    fn e_conjunto(&self, n: usize) -> bool {
        match self.estatico(n).map(|t| self.consulta.tabela.get(t)) {
            Some(Type::Interface { class, .. }) => Some(*class) == self.consulta.core.set_class,
            _ => !self.filhos(n).iter().any(|&k| self.especie(k) == "MapLiteralEntry"),
        }
    }

    /// `visitArgumentList`.
    fn ctx_argumentos(&mut self, n: usize, o: usize) -> Option<TypeId> {
        let abre = self.tok_em(self.ini(n)).filter(|&i| self.e_op(i, Op::LParen))?;
        let fecha = self.tok_antes(self.fim(n)).filter(|&i| self.e_op(i, Op::RParen) && i != abre).map_or_else(|| self.sintetico(self.fim(n)).start, |i| self.span_tok(i).start);
        if !(self.span_tok(abre).end <= o && o <= fecha) {
            return None;
        }
        let pai = self.pai(n)?;
        if !matches!(self.especie(pai), "InstanceCreationExpression" | "MethodInvocation" | "FunctionExpressionInvocation") {
            return None;
        }
        let parametros = self.parametros_invocados(n)?;
        let mut indice = 0usize;
        let posicional = |indice: usize| parametros.get(indice).filter(|p| !p.nomeado).map(|p| p.tipo);
        let mut anterior: Option<usize> = None;
        let mut contagem_posicional = 0usize;
        for a in self.filhos(n).to_vec() {
            if self.especie(a) == "NamedExpression" {
                if o <= self.ini(a) {
                    return posicional(indice);
                }
                if self.contem_o(a, o) {
                    let rotulo = self.filho(a, "Label")?;
                    let fim_nome = self.tok_depois(self.ini(rotulo)).map_or(self.ini(rotulo), |i| self.span_tok(i).end);
                    if o >= fim_nome {
                        let nome = self.fonte[self.ini(rotulo)..self.fim(rotulo)].trim_end_matches(':').trim().to_string();
                        return parametros.iter().find(|p| p.nomeado && p.nome == nome).map(|p| p.tipo);
                    }
                    return None;
                }
            } else {
                if anterior.is_none_or(|p| self.fim(p) < o) && o <= self.fim(a) {
                    // `staticParameterElement`: o posicional de mesma ordem.
                    let posicionais: Vec<TypeId> = parametros.iter().filter(|p| !p.nomeado).map(|p| p.tipo).collect();
                    return posicionais.get(contagem_posicional).copied();
                }
                anterior = Some(a);
                indice += 1;
                contagem_posicional += 1;
            }
        }
        posicional(indice)
    }

    /// `visitRecordLiteral`.
    fn ctx_record(&mut self, n: usize, o: usize) -> Option<TypeId> {
        let p = self.pai(n)?;
        let t = self.ctx(p, o)?;
        let Type::Record { positional, named, .. } = self.consulta.tabela.get(t).clone() else { return None };
        let mut indice = 0usize;
        for a in self.filhos(n).to_vec() {
            if self.especie(a) == "NamedExpression" {
                if o <= self.ini(a) {
                    return positional.get(indice).copied();
                }
                if self.contem_o(a, o) {
                    let rotulo = self.filho(a, "Label")?;
                    if o >= self.fim(rotulo) {
                        let nome = self.fonte[self.ini(rotulo)..self.fim(rotulo)].trim_end_matches(':').trim().to_string();
                        return named.iter().find(|(s, _)| self.consulta.nome(*s) == nome).map(|(_, t)| *t);
                    }
                    return None;
                }
            } else {
                if o <= self.fim(a) {
                    return positional.get(indice).copied();
                }
                indice += 1;
            }
        }
        positional.get(indice).copied()
    }

    /// `_computePositionalIndex`.
    fn indice_posicional(&self, n: usize, o: usize) -> usize {
        let campos = self.filhos_de(n, &["PatternField"]);
        let mut indice = 0usize;
        for c in campos {
            let mut direita = self.ultimo_token(c).map_or(self.fim(c), |i| self.span_tok(i).start);
            if let Some(v) = self.tok_depois(self.fim(c))
                && self.e_op(v, Op::Comma)
            {
                direita = self.span_tok(v).start;
            }
            if o <= direita {
                return indice;
            }
            if self.filho(c, "PatternFieldName").is_none() {
                indice += 1;
            }
        }
        indice
    }

    /// `_visitFieldInObjectPattern`.
    fn ctx_campo_de_objeto(&mut self, objeto: usize, campo: usize, o: usize) -> Option<TypeId> {
        let nome = self.filho(campo, "PatternFieldName")?;
        if o < self.fim(nome) {
            return None;
        }
        let i = self.tok_depois(self.ini(nome))?;
        if !(self.palavra(i) && self.span_tok(i).end <= self.fim(nome)) {
            return None;
        }
        let texto = self.texto_tok(i).to_string();
        let t = self.tipo_do_no_de_tipo(self.filho(objeto, "NamedType")?)?;
        if !matches!(self.consulta.tabela.get(t), Type::Interface { .. }) {
            return None;
        }
        let s = self.consulta.nomes.lookup(&texto)?;
        let lib = self.biblioteca();
        let (r, tipo) = self.consulta.resolvedor().lookup_member(t, s, false, lib)?;
        match r {
            Resolved::Member { member: MemberRef::Function(f), .. } => match self.consulta.programa.function(f).kind {
                FunctionKind::Getter | FunctionKind::ImplicitAccessor | FunctionKind::Function => Some(tipo),
                _ => None,
            },
            Resolved::Member { member: MemberRef::Variable(_), .. } => Some(tipo),
            _ => None,
        }
    }

    /// `_visitFieldInRecordPattern`.
    fn ctx_campo_de_record(&mut self, record: usize, campo: usize, o: usize) -> Option<TypeId> {
        let Type::Record { positional, named, .. } = self.consulta.tabela.get(self.tipo_casado(record)?).clone() else { return None };
        let Some(nome) = self.filho(campo, "PatternFieldName") else {
            let campos = self.filhos_de(record, &["PatternField"]);
            let i = campos.iter().position(|&c| c == campo)?;
            let k = campos[..i].iter().filter(|&&c| self.filho(c, "PatternFieldName").is_none()).count();
            return positional.get(k).copied();
        };
        if o < self.fim(nome) {
            return None;
        }
        let i = self.tok_depois(self.ini(nome))?;
        if !(self.palavra(i) && self.span_tok(i).end <= self.fim(nome)) {
            return None;
        }
        let texto = self.texto_tok(i);
        named.iter().find(|(s, _)| self.consulta.nome(*s) == texto).map(|(_, t)| *t)
    }
}
