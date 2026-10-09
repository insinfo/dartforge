//! Tipos de extensão (Dart 3.3) apagados.
//!
//! Em tempo de execução um valor de tipo de extensão **é** o valor da
//! representação (`apagamento.rs` apaga os tipos; nenhum tipo de extensão tem
//! classe no heap). Os membros são resolvidos estaticamente, como os de uma
//! extensão, e por isso o lowering não pode procurá-los no tipo (apagado) do
//! receptor: a inferência diz, em cada acesso, que o membro é de um tipo de
//! extensão (`Resolved::Member { class }`), e é ela que decide a rota.
//!
//! * membro de instância (método, getter, setter, operador) → a função
//!   `df.<biblioteca>.<E>.<m>(this, …)` com `this` na representação, como o
//!   membro de uma extensão (`extensoes.rs`); os argumentos de tipo de `E`
//!   vão na tupla, antes dos do membro, tirados do tipo estático (não
//!   apagado) do receptor;
//! * a representação (`e.v`) → o próprio receptor;
//! * construtor primário → o valor do argumento;
//! * outro construtor (generativo ou fábrica) → uma função que devolve a
//!   representação, chamada como uma fábrica (a tupla da criação no fim);
//! * estáticos → como os de uma classe.

use super::closures::{Padrao, ParamEntrada};
use super::fn_builder::FnBuilder;
use crate::context::Context;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionRef};
use dartforge_frontend::ast::{self, DeclKind, ExprId, ExprKind, ParameterKind};
use dartforge_intern::SymbolId;
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::TypeId;

/// O tipo de extensão que declara `fid`, quando é um membro de instância
/// escrito (método, getter, setter, operador; não o acessor da
/// representação nem um construtor).
pub fn dono_de_instancia(ctx: &Context, fid: usize) -> Option<ClassId> {
    let f = &ctx.program.functions[fid];
    let c = f.class?;
    (ctx.e_tipo_de_extensao(c) && !f.static_ && f.variable.is_none() && matches!(f.node, FunctionRef::Function { .. }))
        .then_some(c)
}

/// O nome do construtor primário de `c` (o símbolo vazio no sem nome).
pub fn nome_do_primario(ctx: &Context, c: ClassId) -> Option<SymbolId> {
    let decl = ctx.program.class(c).decl?;
    match &ctx.program.unit(decl.unit).ast.decl(decl.decl).kind {
        DeclKind::ExtensionType(ed) => match ed.constructor {
            Some(n) => Some(n.sym),
            None => ctx.interner.lookup(""),
        },
        _ => None,
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O padrão do receptor de um membro de instância chamado com o
    /// receptor como primeiro parâmetro: o número de parâmetros de tipo do
    /// dono e o tipo com que o do receptor é casado (o `on` de uma extensão,
    /// `E<T…>` de um tipo de extensão).
    pub fn padrao_do_receptor(&self, fid: usize) -> Option<(usize, TypeId)> {
        let f = &self.ctx.program.functions[fid];
        if let Some(e) = f.extension.filter(|_| !f.static_) {
            let x = self.ctx.outline.extensions.get(e.0 as usize)?;
            return Some((x.type_params.len(), x.on));
        }
        let c = dono_de_instancia(self.ctx, fid)?;
        let n = self.ctx.outline.classes.get(c.0 as usize).map_or(0, |d| d.type_params.len());
        Some((n, *self.ctx.te.this.get(&c)?))
    }

    /// A representação de `this` num membro de instância de extensão ou de
    /// tipo de extensão.
    pub fn repr_do_this_estatico(&self, fid: usize) -> Option<Type> {
        self.padrao_do_receptor(fid).map(|(_, t)| self.repr(t))
    }

    /// O tipo estático do receptor implícito (`this`) num membro de extensão
    /// ou de tipo de extensão.
    pub fn receptor_implicito(&self) -> Option<TypeId> {
        self.extensao_do_this.map(|(_, on)| on).or(self.tipo_ext_do_this.map(|(_, t)| t))
    }

    /// O membro de instância de tipo de extensão que a inferência resolveu
    /// para `e` (leitura, chamada, escrita ou operador).
    pub fn membro_te(&self, e: ExprId) -> Option<MemberRef> {
        let r = self.ctx.get_resolved(self.unit_id, e).cloned();
        self.membro_te_resolvido(r.as_ref())
    }

    /// [`Self::membro_te`] de uma resolução já lida.
    pub fn membro_te_resolvido(&self, r: Option<&Resolved>) -> Option<MemberRef> {
        let Some(Resolved::Member { class, member, via_super: false }) = r else { return None };
        if !self.ctx.e_tipo_de_extensao(*class) {
            return None;
        }
        let estatico = match member {
            MemberRef::Function(f) => self.ctx.program.functions[f.0 as usize].static_,
            MemberRef::Variable(v) => self.ctx.program.variables[v.0 as usize].static_,
        };
        (!estatico).then_some(*member)
    }

    /// O membro é a representação (o campo, ou o getter dele).
    pub fn e_representacao(&self, m: MemberRef) -> bool {
        let v = match m {
            MemberRef::Variable(v) => v,
            MemberRef::Function(f) => match self.ctx.program.functions[f.0 as usize].variable {
                Some(v) => v,
                None => return false,
            },
        };
        let var = &self.ctx.program.variables[v.0 as usize];
        var.class.is_some_and(|c| self.ctx.program.classes[c.0 as usize].representation == Some(v))
    }

    /// A função do membro de tipo de extensão resolvido para `e` (a
    /// representação fica de fora: não é chamada).
    pub fn funcao_te(&self, e: ExprId) -> Option<usize> {
        match self.membro_te(e)? {
            MemberRef::Function(f) if !self.e_representacao(MemberRef::Function(f)) => Some(f.0 as usize),
            _ => None,
        }
    }

    /// `recv.m`: a representação é o próprio receptor; o getter é chamado; o
    /// método vira tear-off.
    pub fn ler_membro_te(&mut self, recv: Operand, m: MemberRef, receptor: Option<TypeId>, span: Span) -> Operand {
        if self.e_representacao(m) {
            return recv;
        }
        match m {
            MemberRef::Function(f) => self.ler_extensao(recv, f.0 as usize, receptor, span),
            MemberRef::Variable(_) => self.nao_suportado("campo de instância de tipo de extensão", span),
        }
    }

    /// O tipo de extensão que a expressão nomeia (`E`, `p.E`), se é um.
    pub fn tipo_de_extensao_nomeado(&self, ast: &ast::Ast, e: ExprId) -> Option<ClassId> {
        let c = match &ast.expr(e).kind {
            ExprKind::Identifier(n) => match self.ctx.get_resolved(self.unit_id, e).cloned() {
                Some(Resolved::Element(Element::Class(c))) => c,
                None if self.buscar_local(n.sym).is_none() => match self.resolver_por_nome(n.sym)? {
                    Resolved::Element(Element::Class(c)) => c,
                    _ => return None,
                },
                _ => return None,
            },
            ExprKind::Property { target, name, null_aware: false } => match self.elemento_prefixado(ast, *target, name.sym)? {
                Element::Class(c) => c,
                _ => return None,
            },
            _ => return None,
        };
        self.ctx.e_tipo_de_extensao(c).then_some(c)
    }

    /// A criação de tipo de extensão que a chamada `alvo(…)` faz: `E(…)`,
    /// `E.nome(…)` (também com prefixo); o tipo e o nome do construtor.
    pub fn criacao_te(&self, ast: &ast::Ast, alvo: ExprId) -> Option<(ClassId, SymbolId)> {
        if let Some(c) = self.tipo_de_extensao_nomeado(ast, alvo) {
            return Some((c, self.ctx.interner.lookup("")?));
        }
        let ExprKind::Property { target, name, null_aware: false } = &ast.expr(alvo).kind else { return None };
        let c = self.tipo_de_extensao_nomeado(ast, *target)?;
        let e_construtor = nome_do_primario(self.ctx, c) == Some(name.sym)
            || self.ctx.program.classes[c.0 as usize].constructors.contains_key(&name.sym);
        e_construtor.then_some((c, name.sym))
    }

    /// `E.nome(args)` (`expr` é a criação, cujo tipo estático dá os
    /// argumentos de tipo): o primário é o valor do argumento; os demais, a
    /// função do construtor.
    pub fn construir_te(
        &mut self,
        ast: &ast::Ast,
        expr: ExprId,
        c: ClassId,
        nome: SymbolId,
        args: &[ast::Argument],
        span: Span,
    ) -> Operand {
        if nome_do_primario(self.ctx, c) == Some(nome) {
            let Some(a) = args.first() else {
                return self.nao_suportado("construtor primário sem argumento", span);
            };
            let v = self.lower_expr(ast, a.value);
            return match self.ctx.get_type(self.unit_id, expr) {
                Some(t) => {
                    let r = self.repr(t);
                    self.coagir(v, r)
                }
                None => v,
            };
        }
        let Some(&f) = self.ctx.program.classes[c.0 as usize].constructors.get(&nome) else {
            return self.nao_suportado("construtor de tipo de extensão não encontrado", span);
        };
        self.tipo_da_criacao = self.ctx.get_type_bruto(self.unit_id, expr);
        self.instanciar(ast, f, args, span)
    }

    /// Tear-off de construtor de tipo de extensão (`E.new`, `E.nome`): o do
    /// primário devolve o argumento; os demais são os de construtor comum.
    pub fn tearoff_de_construtor_te(&mut self, expr: ExprId, c: ClassId, nome: SymbolId, span: Span) -> Operand {
        if nome_do_primario(self.ctx, c) != Some(nome) {
            let Some(&f) = self.ctx.program.classes[c.0 as usize].constructors.get(&nome) else {
                return self.nao_suportado("construtor de tipo de extensão não encontrado", span);
            };
            return self.tearoff_de_construtor(f.0 as usize, span);
        }
        let classe = &self.ctx.program.classes[c.0 as usize];
        let simbolo_ent = format!(
            "df.{}.{}.$primario$tear",
            crate::context::escapar(&self.ctx.nome_da_biblioteca(classe.library)),
            crate::context::escapar(self.ctx.symbol_name(classe.name))
        );
        if !self.entradas_feitas.contains(&simbolo_ent) {
            self.entradas_feitas.insert(simbolo_ent.clone());
            let nome_rep = classe
                .representation
                .map(|v| self.ctx.symbol_name(self.ctx.program.variables[v.0 as usize].name).to_string());
            let mut e = FnBuilder::new(self.ctx, self.unit_id, simbolo_ent.clone(), "new".to_string(), Type::Ref);
            e.add_param("closure".to_string(), Type::Ref);
            let args = Operand::Val(e.add_param("args".to_string(), Type::Ptr));
            let desc = Operand::Val(e.add_param("desc".to_string(), Type::Ptr));
            let infos = [ParamEntrada { nome: nome_rep, kind: ParameterKind::Required, required: true, padrao: Padrao::Nenhum }];
            if let Some(vals) = e.desempacotar(&infos, args, desc) {
                let v = e.coagir(vals[0].clone(), Type::Ref);
                e.terminate(Terminator::Return(Some(v)));
            }
            self.absorver(e);
        }
        let t = self.emit(Instruction::TearOff { code_symbol: simbolo_ent }, Type::Ref);
        // A assinatura é o tipo estático do tear-off (`E Function(R)`),
        // apagado.
        if let Some(sig) = self.ctx.get_type_bruto(self.unit_id, expr) {
            let r = self.receita_de_tipo(sig);
            let tipo = self.rti_da_receita(&r);
            self.definir_rti(t.clone(), tipo);
        }
        t
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O corpo de um construtor generativo de tipo de extensão `c`: os
    /// inicializadores dão a representação (`this.v`, `v = e` ou `this(…)`),
    /// que é `this` no corpo e o valor devolvido. Os parâmetros já estão
    /// declarados; na classe genérica, a tupla vem no fim, como numa fábrica.
    pub fn lower_construtor_te(&mut self, ast: &ast::Ast, c: ClassId, ctor: &ast::Constructor, span: Span) {
        self.enclosing_class = Some(c);
        self.classe_do_membro = Some(c);
        if self.classe_generica(c) {
            let t = self.add_param("$tipos".to_string(), Type::I64);
            self.tupla_de_tipos = Some(Operand::Val(t));
            self.classe_por_tupla = true;
        }
        let this_t = self.ctx.te.this.get(&c).copied();
        let rep = self.ctx.program.classes[c.0 as usize].representation;
        let nome_rep = rep.map(|v| self.ctx.program.variables[v.0 as usize].name);
        let mut valor: Option<Operand> = None;
        for p in ctor.parameters.iter() {
            if p.this_
                && let Some(n) = p.name
                && Some(n.sym) == nome_rep
            {
                valor = self.ler_local_por_nome(n.sym);
            }
        }
        for init in ctor.initializers.iter() {
            match init {
                ast::Initializer::Field { name, value, .. } if Some(name.sym) == nome_rep => {
                    valor = Some(self.lower_expr(ast, *value));
                }
                ast::Initializer::Field { span, .. } => {
                    self.nao_suportado("campo de instância de tipo de extensão", *span);
                }
                ast::Initializer::Assert { condition, message, .. } => self.lower_assert(ast, *condition, *message),
                ast::Initializer::Redirect { constructor, arguments, span } => {
                    let nome = match constructor {
                        Some(n) => Some(n.sym),
                        None => self.ctx.interner.lookup(""),
                    };
                    valor = Some(match nome {
                        Some(n) if nome_do_primario(self.ctx, c) == Some(n) => match arguments.args.first() {
                            Some(a) => self.lower_expr(ast, a.value),
                            None => self.nao_suportado("construtor primário sem argumento", *span),
                        },
                        Some(n) => match self.ctx.program.classes[c.0 as usize].constructors.get(&n).copied() {
                            Some(f) => {
                                let avaliados = self.avaliar_args(ast, &arguments.args);
                                self.tipo_da_criacao = this_t;
                                self.instanciar_avaliados(f, &avaliados, *span)
                            }
                            None => self.nao_suportado("construtor redirecionado não encontrado", *span),
                        },
                        None => self.nao_suportado("construtor redirecionado não encontrado", *span),
                    });
                }
                ast::Initializer::Super { span, .. } => {
                    self.nao_suportado("`super` em construtor de tipo de extensão", *span);
                }
            }
            if self.is_terminated() {
                return;
            }
        }
        let ret = self.func.return_ty;
        let v = match valor {
            Some(v) => self.coagir(v, ret),
            None => self.nao_suportado("construtor de tipo de extensão sem a representação", span),
        };
        self.this_param = Some(v.clone());
        self.this_finalizavel = self.ctx.classificar_classe_finalizavel(c);
        self.tipo_ext_do_this = this_t.map(|t| (c, t));
        self.retorno_do_construtor = Some(v);
        match &ctor.body {
            ast::FunctionBody::Block(s) => self.lower_stmt(ast, *s),
            ast::FunctionBody::Expression(e) => {
                self.lower_expr(ast, *e);
            }
            _ => {}
        }
        if !self.is_terminated() {
            self.route_return(None);
        }
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `recv.m(args)` (ou `m(args)` com `this`) para o membro de tipo de
    /// extensão `m`: o método é chamado direto; a representação e o getter
    /// são lidos e o valor chamado.
    #[allow(clippy::too_many_arguments)]
    pub fn chamar_membro_te(
        &mut self,
        ast: &ast::Ast,
        recv: Operand,
        m: MemberRef,
        receptor: Option<TypeId>,
        expr: ExprId,
        arguments: &ast::Arguments,
        span: Span,
    ) -> Operand {
        let getter = match m {
            MemberRef::Function(f) => {
                self.ctx.program.functions[f.0 as usize].kind == dartforge_elements::model::FunctionKind::Getter
            }
            MemberRef::Variable(_) => true,
        };
        if getter || self.e_representacao(m) {
            let v = self.ler_membro_te(recv, m, receptor, span);
            let avaliados = self.avaliar_args(ast, &arguments.args);
            return self.chamar_valor_funcao(v, &avaliados);
        }
        let MemberRef::Function(f) = m else { unreachable!("campo tratado acima") };
        let avaliados = self.avaliar_args(ast, &arguments.args);
        self.chamar_extensao(recv, f.0 as usize, &avaliados, receptor, Some((expr, arguments)), span)
    }

    /// O membro `sym` visto sem receptor no corpo de um membro do tipo de
    /// extensão corrente: o do próprio tipo, senão o de um tipo de extensão
    /// que ele implementa (em largura); `None` para o resto (os membros da
    /// representação que ele implementa ficam com a resolução comum).
    pub fn membro_te_por_nome(&self, sym: SymbolId) -> Option<Resolved> {
        let (c0, _) = self.tipo_ext_do_this?;
        self.membro_te_na_classe(c0, sym)
    }

    /// O membro `sym` do tipo de extensão `c0` (ou de um tipo de extensão
    /// que ele implementa), como a inferência o resolveria.
    pub fn membro_te_na_classe(&self, c0: ClassId, sym: SymbolId) -> Option<Resolved> {
        let mut fila = std::collections::VecDeque::from([c0]);
        let mut vistos = std::collections::HashSet::new();
        while let Some(c) = fila.pop_front() {
            if !vistos.insert(c) {
                continue;
            }
            let classe = &self.ctx.program.classes[c.0 as usize];
            if let Some(&f) = classe.instance_members.get(&sym).or_else(|| classe.static_members.get(&sym)) {
                return Some(Resolved::Member { class: c, member: MemberRef::Function(f), via_super: false });
            }
            if let Some(&v) = classe.fields.iter().find(|&&v| self.ctx.program.variables[v.0 as usize].name == sym) {
                return Some(Resolved::Member { class: c, member: MemberRef::Variable(v), via_super: false });
            }
            fila.extend(classe.interface_classes.iter().copied().filter(|&i| self.ctx.e_tipo_de_extensao(i)));
        }
        None
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O membro de instância `sym` do dono de `fid` (a mesma extensão; o
    /// mesmo tipo de extensão ou um tipo de extensão que ele implementa):
    /// o par do getter/setter e o `[]=` do `[]` numa escrita composta.
    pub fn acessor_do_mesmo_dono(&self, fid: usize, sym: SymbolId) -> Option<dartforge_elements::model::FunctionElementId> {
        let f = &self.ctx.program.functions[fid];
        if let Some(e) = f.extension {
            return self.ctx.program.extensions[e.0 as usize].instance_members.get(&sym).copied();
        }
        let c0 = f.class.filter(|&c| self.ctx.e_tipo_de_extensao(c))?;
        let mut fila = std::collections::VecDeque::from([c0]);
        let mut vistos = std::collections::HashSet::new();
        while let Some(c) = fila.pop_front() {
            if !vistos.insert(c) {
                continue;
            }
            let classe = &self.ctx.program.classes[c.0 as usize];
            if let Some(&g) = classe.instance_members.get(&sym) {
                return Some(g);
            }
            fila.extend(classe.interface_classes.iter().copied().filter(|&i| self.ctx.e_tipo_de_extensao(i)));
        }
        None
    }
}
