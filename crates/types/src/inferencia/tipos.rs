//! Operações de tipo usadas pela inferência: subtipagem, UP/DOWN, fechos de
//! esquema, `flatten`, construção de tipos do núcleo e resolução de anotações
//! escritas dentro de corpos.

use super::corpo::Corpo;
use super::BodyInferrer;
use crate::bounds;
use crate::ops::{non_nullable, nullable, substitute};
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{Type, TypeId, TypeParamId, TypeParamOwner, Variance};
use dartforge_elements::model::{ClassId, ClassKind, Element, UnitId};
use dartforge_frontend::ast;
use std::collections::HashMap;

impl<'a> BodyInferrer<'a> {
    pub(crate) fn env(&mut self) -> SubtypeEnv<'_> {
        SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core)
    }

    pub(crate) fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        if a == b {
            return true;
        }
        let mut env = self.env();
        is_subtype(a, b, &mut env)
    }

    pub(crate) fn up(&mut self, a: TypeId, b: TypeId) -> TypeId {
        let mut env = self.env();
        bounds::up(a, b, &mut env)
    }

    pub(crate) fn down(&mut self, a: TypeId, b: TypeId) -> TypeId {
        let mut env = self.env();
        bounds::down(a, b, &mut env)
    }

    pub(crate) fn anulavel(&mut self, t: TypeId) -> TypeId {
        nullable(t, self.table)
    }

    pub(crate) fn nao_nulo(&mut self, t: TypeId) -> TypeId {
        match self.table.get(t) {
            Type::FutureOr { arg, nullable: true } => {
                let a = *arg;
                self.table.intern(Type::FutureOr { arg: a, nullable: false })
            }
            _ => non_nullable(t, self.table),
        }
    }

    /// `Null <: t`.
    pub(crate) fn e_anulavel(&mut self, t: TypeId) -> bool {
        let n = self.core.null;
        self.sub(n, t)
    }

    /// `t <: Object`.
    pub(crate) fn e_nao_anulavel(&mut self, t: TypeId) -> bool {
        let o = self.core.object;
        self.sub(t, o)
    }

    pub(crate) fn e_dynamic(&self, t: TypeId) -> bool {
        matches!(self.table.get(t), Type::Dynamic)
    }

    /// **BOTTOM**(`t`).
    pub(crate) fn e_fundo(&mut self, t: TypeId) -> bool {
        let env = self.env();
        bounds::is_bottom(t, &env)
    }

    pub(crate) fn e_desconhecido(&self, t: TypeId) -> bool {
        self.core.is_unknown(self.table, t)
    }


    /// Fecho maior de um esquema em relação a `_`.
    pub(crate) fn fecho_maior(&mut self, t: TypeId) -> TypeId {
        if self.e_desconhecido(t) {
            return self.core.object_nullable;
        }
        let mut env = self.env();
        bounds::schema_greatest(t, &mut env)
    }


    pub(crate) fn iface(&mut self, class: Option<ClassId>, args: Vec<TypeId>) -> TypeId {
        match class {
            Some(c) => self.table.intern(Type::Interface { class: c, args: args.into_boxed_slice(), nullable: false }),
            None => self.core.dynamic_,
        }
    }

    pub(crate) fn futuro(&mut self, t: TypeId) -> TypeId {
        let c = self.core.future_class;
        self.iface(c, vec![t])
    }

    pub(crate) fn futuro_ou(&mut self, t: TypeId) -> TypeId {
        self.table.intern(Type::FutureOr { arg: t, nullable: false })
    }

    pub(crate) fn iteravel(&mut self, t: TypeId) -> TypeId {
        let c = self.core.iterable_class;
        self.iface(c, vec![t])
    }

    pub(crate) fn fluxo_de(&mut self, t: TypeId) -> TypeId {
        let c = self.core.stream_class;
        self.iface(c, vec![t])
    }

    pub(crate) fn lista(&mut self, t: TypeId) -> TypeId {
        let c = self.core.list_class;
        self.iface(c, vec![t])
    }

    pub(crate) fn conjunto(&mut self, t: TypeId) -> TypeId {
        let c = self.core.set_class;
        self.iface(c, vec![t])
    }

    pub(crate) fn tipo_mapa(&mut self, k: TypeId, v: TypeId) -> TypeId {
        let c = self.core.map_class;
        self.iface(c, vec![k, v])
    }

    /// Um tipo de interface cru (`List` sem argumentos, como o outline às
    /// vezes guarda) ganha os argumentos da instanciação para os limites.
    pub(crate) fn completar_args(&mut self, t: TypeId) -> TypeId {
        match self.table.get(t).clone() {
            Type::Interface { class, args, nullable } if args.is_empty() => {
                let params = self.outline.classes[class.0 as usize].type_params.clone();
                if params.is_empty() {
                    return t;
                }
                let args = self.instanciar_para_limites(&params);
                self.table.intern(Type::Interface { class, args: args.into_boxed_slice(), nullable })
            }
            _ => t,
        }
    }

    /// Argumentos de `class` em `t` visto como instância dela (`List<int>`
    /// como `Iterable` dá `[int]`), ignorando a anulabilidade de `t`.
    pub(crate) fn como_instancia_de(&mut self, t: TypeId, class: Option<ClassId>) -> Option<Vec<TypeId>> {
        let class = class?;
        let t = self.nao_nulo(t);
        let t = self.completar_args(t);
        match self.table.get(t).clone() {
            Type::Interface { class: c, args, .. } if c == class => return Some(args.to_vec()),
            Type::Interface { .. } | Type::ExtensionType { .. } => {}
            Type::Intersection { bound, .. } => return self.como_instancia_de(bound, Some(class)),
            Type::TypeParameter { param, .. } if param != self.core.unknown_param => {
                let b = self.table.param(param).bound;
                if b == t {
                    return None;
                }
                return self.como_instancia_de(b, Some(class));
            }
            _ => return None,
        }
        let s = self.outline.hierarchy.supertype_of(t, class, self.table, self.core)?;
        match self.table.get(s) {
            Type::Interface { args, .. } => Some(args.to_vec()),
            _ => None,
        }
    }

    /// `flatten(T)` (especificação, "Function Expressions"; `flatten` do CFE).
    pub(crate) fn flatten(&mut self, t: TypeId) -> TypeId {
        match self.table.get(t).clone() {
            Type::Dynamic | Type::Void => t,
            Type::Intersection { bound, .. } => match self.como_instancia_de(bound, self.core.future_class) {
                Some(a) => a[0],
                None => t,
            },
            Type::FutureOr { arg, nullable: n } => {
                if n {
                    self.anulavel(arg)
                } else {
                    arg
                }
            }
            ty if ty.is_declared_nullable() => {
                let s = self.nao_nulo(t);
                let f = self.flatten(s);
                self.anulavel(f)
            }
            Type::TypeParameter { param, .. } if param != self.core.unknown_param => {
                let b = self.table.param(param).bound;
                if b == t || self.table.param(param).bound == self.core.object_nullable {
                    return t;
                }
                // `X extends Future<S>`: flatten é S.
                match self.como_instancia_de(b, self.core.future_class) {
                    Some(a) => a[0],
                    None => t,
                }
            }
            _ => match self.como_instancia_de(t, self.core.future_class) {
                Some(a) => a[0],
                None => t,
            },
        }
    }

    /// **futureValueTypeSchema**(`S`) (`inference.md`).
    pub(crate) fn tipo_valor_futuro_esquema(&mut self, s: TypeId) -> TypeId {
        if self.e_desconhecido(s) {
            return s;
        }
        match self.table.get(s).clone() {
            Type::Void | Type::Dynamic => s,
            Type::FutureOr { arg, .. } => arg,
            Type::Interface { class, args, .. } if Some(class) == self.core.future_class && args.len() == 1 => args[0],
            _ => self.core.object_nullable,
        }
    }

    /// Atribuível: subtipo, `dynamic` (cast implícito) ou coerção por `call`.
    pub(crate) fn atribuivel(&mut self, de: TypeId, para: TypeId) -> bool {
        if self.e_dynamic(de) || self.sub(de, para) {
            return true;
        }
        if matches!(self.table.get(de), Type::Interface { .. }) {
            if let Some(call) = self.sym.call {
                if let Some(m) = self.membro_de_interface(de, call, false) {
                    if m.metodo {
                        return self.sub(m.tipo, para);
                    }
                }
            }
        }
        false
    }

    /// Coerção por `call` (*implicit call tearoff*): um objeto de tipo de
    /// interface com método `call`, num contexto de tipo função, vale o tipo
    /// do seu `call`. É com esse tipo que o argumento restringe os parâmetros
    /// de tipo de uma chamada genérica (`[1].map(somador)` infere `T` pelo
    /// retorno de `Somador.call`). `None` quando a coerção não se aplica.
    pub(crate) fn tipo_do_call_implicito(&mut self, de: TypeId, contexto: TypeId) -> Option<TypeId> {
        if !matches!(self.table.get(contexto), Type::Function { .. }) || !matches!(self.table.get(de), Type::Interface { nullable: false, .. }) {
            return None;
        }
        let call = self.sym.call?;
        let m = self.membro_de_interface(de, call, false)?;
        m.metodo.then_some(m.tipo)
    }

    /// Tipo `this` da classe (argumentos = os próprios parâmetros).
    pub(crate) fn tipo_this_classe(&mut self, c: ClassId) -> TypeId {
        let params = self.outline.classes[c.0 as usize].type_params.clone();
        let args: Vec<TypeId> = params
            .iter()
            .map(|&p| self.table.intern(Type::TypeParameter { param: p, nullable: false }))
            .collect();
        if self.program.class(c).kind == ClassKind::ExtensionType {
            self.table.intern(Type::ExtensionType { decl: c, args: args.into_boxed_slice(), nullable: false })
        } else {
            self.table.intern(Type::Interface { class: c, args: args.into_boxed_slice(), nullable: false })
        }
    }

    /// Instanciação para os limites (`instantiate to bounds`) dos parâmetros
    /// de uma classe usada crua (`List` → `List<dynamic>`).
    pub(crate) fn instanciar_para_limites(&mut self, params: &[TypeParamId]) -> Vec<TypeId> {
        crate::ops::instanciar_para_limites(params, &[], self.table, self.core)
    }

    /// Cópias frescas (reusadas por lista) de parâmetros de tipo, com os
    /// limites reescritos nelas.
    pub(crate) fn parametros_novos(&mut self, originais: &[TypeParamId]) -> Vec<TypeParamId> {
        if originais.is_empty() {
            return Vec::new();
        }
        if let Some(n) = self.params_construtor.get(&originais[0].0) {
            return n.clone();
        }
        let novos: Vec<TypeParamId> = originais
            .iter()
            .map(|&p| {
                let d = self.table.param(p).clone();
                self.table.alloc_type_param(d.name, TypeParamOwner::GenericFunctionType, d.bound, d.variance)
            })
            .collect();
        let tipos: Vec<TypeId> = novos.iter().map(|&p| self.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        let mapa = self.mapa(originais, &tipos);
        for (&p, &o) in novos.iter().zip(originais) {
            let b = self.table.param(p).bound;
            let b = self.subst(b, &mapa);
            self.table.set_type_param_bound(p, b);
            self.table.param_mut(p).explicito = self.table.param(o).explicito;
        }
        self.params_construtor.insert(originais[0].0, novos.clone());
        novos
    }

    /// Mapa de substituição `params → args`.
    pub(crate) fn mapa(&self, params: &[TypeParamId], args: &[TypeId]) -> HashMap<TypeParamId, TypeId> {
        params.iter().copied().zip(args.iter().copied()).collect()
    }

    pub(crate) fn subst(&mut self, t: TypeId, mapa: &HashMap<TypeParamId, TypeId>) -> TypeId {
        substitute(t, mapa, self.table)
    }

    /// Tipo de uma anotação escrita num corpo (parâmetros de tipo em escopo
    /// vêm de `cx`; os membros da classe ou extensão de `cx` escondem os
    /// nomes de topo, como o `InstanceScope` do analyzer).
    pub(crate) fn tipo_de_anotacao(&mut self, cx: &Corpo, t: ast::TypeId) -> TypeId {
        let unit = cx.unit;
        let lib = cx.lib;
        let escopo = cx.parametros_de_tipo_visiveis();
        let antes = std::mem::replace(&mut self.conteiner_de_tipos, (cx.classe, cx.extensao));
        let estatico_antes = std::mem::replace(&mut self.em_membro_estatico, cx.membro_estatico);
        let r = self.resolver_anotacao(unit, lib, t, &escopo);
        self.conteiner_de_tipos = antes;
        self.em_membro_estatico = estatico_antes;
        // `TypeAnnotation.type` do analyzer, para as regras que o leem.
        self.body_types.units[unit.0 as usize].tipos_de_anotacoes.insert(t, r);
        r
    }

    /// Como [`BodyInferrer::tipo_de_anotacao`], para a anotação que aparece
    /// no `contexto` dado (`is`, `as`, `catch`, argumento de tipo): é o que
    /// escolhe o código de um nome que não é tipo.
    pub(crate) fn tipo_no_contexto(&mut self, cx: &Corpo, t: ast::TypeId, contexto: crate::resolve::ContextoDeTipo) -> TypeId {
        self.contexto_de_tipo = contexto;
        self.tipo_de_anotacao(cx, t)
    }

    /// Um argumento de tipo escrito (`f<T>()`, `<T>[]`, `C<T>()`).
    pub(crate) fn tipo_de_argumento_de_tipo(&mut self, cx: &Corpo, t: ast::TypeId) -> TypeId {
        self.tipo_no_contexto(cx, t, crate::resolve::ContextoDeTipo::ArgumentoDeTipo)
    }

    pub(crate) fn resolver_anotacao(
        &mut self,
        unit: UnitId,
        lib: dartforge_elements::model::LibraryId,
        t: ast::TypeId,
        escopo: &HashMap<dartforge_intern::SymbolId, TypeParamId>,
    ) -> TypeId {
        let node = self.program.unit(unit).ast.ty(t);
        let anulavel = node.nullable;
        let span = node.span;
        let contexto = std::mem::take(&mut self.contexto_de_tipo);
        let r = match &node.kind {
            ast::TypeKind::Void => return self.core.void_,
            ast::TypeKind::Named { name, args } => {
                // Faixa dos erros de nome: do prefixo ao fim do nome
                // (`_getErrorRange`); o número de argumentos vai no tipo todo.
                let faixa = dartforge_diagnostics::Span { start: name[0].span.start, end: name[name.len() - 1].span.end };
                let texto = self.interner.resolve(name[name.len() - 1].sym).to_string();
                let binding = if name.len() == 2 {
                    self.program.lookup_prefixed_na_unidade(unit, name[0].sym, name[1].sym)
                } else {
                    let sym = name[0].sym;
                    if let Some(&pid) = escopo.get(&sym) {
                        if !args.is_empty() {
                            self.relatar_argumentos_de_tipo(&texto, 0, args.len(), span);
                        }
                        if self.em_membro_estatico && crate::resolve::param_da_classe(self.table, pid) {
                            self.aviso_com_codigo(
                                dartforge_diagnostics::codigos::compile_time_error::TYPE_PARAMETER_REFERENCED_BY_STATIC,
                                faixa,
                                &[],
                            );
                        }
                        let tp = self.table.intern(Type::TypeParameter { param: pid, nullable: false });
                        return if anulavel { self.anulavel(tp) } else { tp };
                    }
                    match self.interner.resolve(sym) {
                        "dynamic" if self.program.lookup_na_unidade(unit, sym).is_none() => return self.core.dynamic_,
                        "void" => return self.core.void_,
                        "Never" if self.program.lookup_na_unidade(unit, sym).and_then(|b| b.getter).is_none() => {
                            return if anulavel { self.core.null } else { self.core.never };
                        }
                        _ => {}
                    }
                    // O escopo de instância esconde o de topo.
                    let (classe, extensao) = self.conteiner_de_tipos;
                    if faixa.start != faixa.end {
                        match crate::resolve::nome_no_conteiner(self.program, self.interner, classe, extensao, sym) {
                            Some(crate::resolve::NoConteiner::Getter) => {
                                self.relatar_nome_de_tipo(contexto, true, &texto, faixa);
                                return self.table.invalido(self.core.dynamic_);
                            }
                            Some(crate::resolve::NoConteiner::SoSetter) => {
                                self.relatar_nome_de_tipo(contexto, false, &texto, faixa);
                                return self.table.invalido(self.core.dynamic_);
                            }
                            None => {}
                        }
                    }
                    self.program.lookup_na_unidade(unit, sym)
                };
                let args: Vec<ast::TypeId> = args.to_vec();
                let resolvidos: Vec<TypeId> = args
                    .iter()
                    .map(|&a| {
                        self.contexto_de_tipo = crate::resolve::ContextoDeTipo::ArgumentoDeTipo;
                        self.resolver_anotacao(unit, lib, a, escopo)
                    })
                    .collect();
                // Quantos parâmetros o tipo nomeado declara (para o número de
                // argumentos escritos).
                let n_params = match binding.and_then(|b| b.getter) {
                    Some(Element::Class(cid)) => Some(self.outline.classes[cid.0 as usize].type_params.len()),
                    Some(Element::Typedef(tid)) => Some(self.outline.typedefs[tid.0 as usize].type_params.len()),
                    _ => None,
                };
                if let Some(n) = n_params {
                    if !args.is_empty() && args.len() != n {
                        self.relatar_argumentos_de_tipo(&texto, n, args.len(), span);
                    }
                }
                match binding.and_then(|b| b.getter) {
                    // `Null` do `dart:core` é uma classe, mas o tipo é o `Null`
                    // da tabela (como no outline): senão `flatten(Future<Null>)`
                    // não era `Null` e `return Future<Null>…` em
                    // `Future<void> f() async` virava erro.
                    Some(Element::Class(cid)) if Some(cid) == self.core.null_class => self.core.null,
                    Some(Element::Class(cid)) => {
                        // Número errado de argumentos de tipo: todos
                        // inválidos (`named_type_resolver.dart:148`).
                        let resolvidos = match n_params {
                            Some(n) if !args.is_empty() && args.len() != n => vec![self.table.invalido(self.core.dynamic_); n],
                            _ => resolvidos,
                        };
                        self.tipo_de_classe_com_args(cid, resolvidos)
                    }
                    Some(Element::Typedef(tid)) => {
                        let data = self.outline.typedefs[tid.0 as usize].clone();
                        let args = if resolvidos.len() == data.type_params.len() {
                            resolvidos
                        } else if !args.is_empty() {
                            vec![self.table.invalido(self.core.dynamic_); data.type_params.len()]
                        } else {
                            self.instanciar_para_limites(&data.type_params)
                        };
                        let mapa = self.mapa(&data.type_params, &args);
                        let r = self.subst(data.target_type, &mapa);
                        self.table.decorar(r, crate::table::Exibicao::Alias { typedef: tid, args: args.into_boxed_slice() })
                    }
                    _ => {
                        let nome = self.interner.resolve(name[name.len() - 1].sym);
                        match nome {
                            "dynamic" if name.len() == 1 => return self.core.dynamic_,
                            "Never" if name.len() == 1 => {
                                return if anulavel { self.table.decorar(self.core.null, crate::table::Exibicao::NeverAnulavel) } else { self.core.never };
                            }
                            "Null" if name.len() == 1 => return self.core.null,
                            _ => {
                                // Nome que não resolve para tipo
                                // (`NamedTypeResolver`); um nome sintético da
                                // recuperação (sem largura) e uma referência
                                // ambígua (relatada à parte) ficam fora.
                                let ambiguo = binding.is_some_and(|b| b.ambiguous);
                                if faixa.start != faixa.end && !ambiguo {
                                    let achou = binding.is_some_and(|b| b.getter.is_some());
                                    self.relatar_nome_de_tipo(contexto, achou, &texto, faixa);
                                }
                                // O tipo de recuperação: `InvalidType`.
                                self.table.invalido(self.core.dynamic_)
                            }
                        }
                    }
                }
            }
            ast::TypeKind::Function { return_type, type_params, parameters } => {
                let mut local = escopo.clone();
                let mut tps = Vec::new();
                for tp in type_params.iter() {
                    let pid = self.table.alloc_type_param(
                        tp.name.sym,
                        TypeParamOwner::GenericFunctionType,
                        self.core.object_nullable,
                        Variance::Unspecified,
                    );
                    local.insert(tp.name.sym, pid);
                    tps.push(pid);
                }
                for (tp, &pid) in type_params.iter().zip(tps.iter()) {
                    if let Some(b) = tp.bound {
                        let bt = self.resolver_anotacao(unit, lib, b, &local);
                        self.table.set_type_param_bound(pid, bt);
                    }
                }
                let ret = match return_type {
                    Some(r) => self.resolver_anotacao(unit, lib, *r, &local),
                    None => self.core.dynamic_,
                };
                let (pos, opt, named) = self.tipos_de_parametros(unit, lib, parameters, &local);
                self.table.intern(Type::Function {
                    type_params: tps.into_boxed_slice(),
                    ret,
                    positional: pos.into_boxed_slice(),
                    optional: opt.into_boxed_slice(),
                    named: named.into_boxed_slice(),
                    nullable: false,
                })
            }
            ast::TypeKind::Record { positional, named } => {
                let positional: Vec<ast::TypeId> = positional.to_vec();
                let named: Vec<(dartforge_intern::SymbolId, ast::TypeId)> = named.iter().map(|(n, t)| (n.sym, *t)).collect();
                let pos: Vec<TypeId> = positional.iter().map(|&t| self.resolver_anotacao(unit, lib, t, escopo)).collect();
                let nm: Vec<_> = named.iter().map(|(n, t)| (*n, self.resolver_anotacao(unit, lib, *t, escopo))).collect();
                self.table.intern(Type::Record { positional: pos.into_boxed_slice(), named: nm.into_boxed_slice(), nullable: false })
            }
        };
        if anulavel {
            self.anulavel(r)
        } else {
            r
        }
    }

    /// Nome de tipo que não resolve para tipo, no código do contexto.
    fn relatar_nome_de_tipo(&mut self, contexto: crate::resolve::ContextoDeTipo, achou: bool, nome: &str, faixa: dartforge_diagnostics::Span) {
        // O nome sintético não é relatado (`reportNullOrNonTypeElement`).
        if nome.is_empty() {
            return;
        }
        let d = crate::resolve::diagnostico_de_nome_de_tipo(contexto, achou, nome, faixa);
        let args: Vec<&str> = d.args.iter().map(|a| &**a).collect();
        let codigo = d.code.expect("com código");
        self.aviso_com_codigo(codigo, faixa, &args);
    }

    /// `WRONG_NUMBER_OF_TYPE_ARGUMENTS` no tipo inteiro.
    fn relatar_argumentos_de_tipo(&mut self, nome: &str, parametros: usize, argumentos: usize, span: dartforge_diagnostics::Span) {
        let (p, a) = (parametros.to_string(), argumentos.to_string());
        self.aviso_com_codigo(
            dartforge_diagnostics::codigos::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS,
            span,
            &[nome, &p, &a],
        );
    }

    /// `extensionTypeErasure` (o tipo de representação no lugar de cada tipo
    /// de extensão).
    pub(crate) fn apagar_extensao(&mut self, t: TypeId) -> TypeId {
        let outline = &*self.outline;
        let program = self.program;
        let rep = |decl: ClassId, args: &[TypeId], table: &mut crate::table::TypeTable| -> Option<TypeId> {
            let v = program.class(decl).representation?;
            let t = outline.variables[v.0 as usize].declared_type?;
            let params = &outline.classes[decl.0 as usize].type_params;
            if params.is_empty() || params.len() != args.len() {
                return Some(t);
            }
            let mapa: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
            Some(crate::ops::substitute(t, &mapa, table))
        };
        crate::ops::erase_extension_type(t, self.table, &rep)
    }

    /// `C<args>` (com instanciação para os limites se faltarem argumentos).
    pub(crate) fn tipo_de_classe_com_args(&mut self, cid: ClassId, args: Vec<TypeId>) -> TypeId {
        let params = self.outline.classes[cid.0 as usize].type_params.clone();
        let args = if args.len() == params.len() { args } else { self.instanciar_para_limites(&params) };
        let class = self.program.class(cid);
        if class.kind == ClassKind::ExtensionType {
            return self.table.intern(Type::ExtensionType { decl: cid, args: args.into_boxed_slice(), nullable: false });
        }
        if Some(class.library) == self.core.async_library && self.interner.resolve(class.name) == "FutureOr" {
            let a = args.first().copied().unwrap_or(self.core.dynamic_);
            return self.table.intern(Type::FutureOr { arg: a, nullable: false });
        }
        self.table.intern(Type::Interface { class: cid, args: args.into_boxed_slice(), nullable: false })
    }

    /// Tipos (posicionais, opcionais, nomeados) de uma lista de parâmetros escrita.
    pub(crate) fn tipos_de_parametros(
        &mut self,
        unit: UnitId,
        lib: dartforge_elements::model::LibraryId,
        parameters: &[ast::Parameter],
        escopo: &HashMap<dartforge_intern::SymbolId, TypeParamId>,
    ) -> (Vec<TypeId>, Vec<TypeId>, Vec<(dartforge_intern::SymbolId, TypeId, bool)>) {
        let mut pos = Vec::new();
        let mut opt = Vec::new();
        let mut named = Vec::new();
        for p in parameters.iter() {
            let t = self.tipo_de_parametro_escrito(unit, lib, p, escopo).unwrap_or(self.core.dynamic_);
            match p.kind {
                ast::ParameterKind::Required => pos.push(t),
                ast::ParameterKind::Optional => opt.push(t),
                ast::ParameterKind::Named => {
                    if let Some(n) = &p.name {
                        named.push((n.sym, t, p.required));
                    }
                }
            }
        }
        (pos, opt, named)
    }

    /// Tipo escrito de um parâmetro (inclusive a forma antiga `int f(int x)`).
    pub(crate) fn tipo_de_parametro_escrito(
        &mut self,
        unit: UnitId,
        lib: dartforge_elements::model::LibraryId,
        p: &ast::Parameter,
        escopo: &HashMap<dartforge_intern::SymbolId, TypeParamId>,
    ) -> Option<TypeId> {
        if let Some(fps) = &p.function_parameters {
            let mut local = escopo.clone();
            let mut tps = Vec::new();
            for tp in p.function_type_params.iter() {
                let pid = self.table.alloc_type_param(
                    tp.name.sym,
                    TypeParamOwner::GenericFunctionType,
                    self.core.object_nullable,
                    Variance::Unspecified,
                );
                local.insert(tp.name.sym, pid);
                tps.push(pid);
            }
            let ret = match p.ty {
                Some(r) => self.resolver_anotacao(unit, lib, r, &local),
                None => self.core.dynamic_,
            };
            let (pos, opt, named) = self.tipos_de_parametros(unit, lib, fps, &local);
            let f = self.table.intern(Type::Function {
                type_params: tps.into_boxed_slice(),
                ret,
                positional: pos.into_boxed_slice(),
                optional: opt.into_boxed_slice(),
                named: named.into_boxed_slice(),
                nullable: p.function_nullable,
            });
            return Some(f);
        }
        p.ty.map(|t| self.resolver_anotacao(unit, lib, t, escopo))
    }
}
