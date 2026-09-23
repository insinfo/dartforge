//! Corpos de funções declaradas, construtores (inicializadores), expressões
//! de função (com inferência do retorno, `inference.md` "Function literal
//! return type inference"), funções locais, variáveis de topo sem tipo e
//! metadados.

use super::corpo::{Corpo, CtxFuncao, Local};
use super::expr::{self, declarar_local, inferir, inferir_livre};
use super::instrucoes;
use super::BodyInferrer;
use crate::codes::*;
use crate::table::{Type, TypeId, TypeParamId};
use dartforge_elements::model::{ClassId, Element, FunctionElementId, FunctionRef, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, AsyncModifier, FunctionBody};
use std::collections::HashMap;

impl<'a> BodyInferrer<'a> {
    /// Contexto das expressões de `return` para um retorno declarado `r`.
    pub(crate) fn contexto_de_retorno_declarado(&mut self, r: TypeId, m: AsyncModifier) -> TypeId {
        let u = self.core.unknown;
        match m {
            AsyncModifier::None => r,
            AsyncModifier::Async => {
                let fv = self.tipo_valor_futuro_esquema(r);
                self.futuro_ou(fv)
            }
            AsyncModifier::SyncStar => self.como_instancia_de(r, self.core.iterable_class).map(|a| a[0]).unwrap_or(u),
            AsyncModifier::AsyncStar => self.como_instancia_de(r, self.core.stream_class).map(|a| a[0]).unwrap_or(u),
        }
    }
}

/// Declara os parâmetros de uma lista escrita, com os tipos dados; infere
/// os valores padrão. `so_inicializadores` marca `this.x`/`super.x`.
fn declarar_parametros(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, params: &[ast::Parameter], tipos: &[TypeId], pular_inicializadores: bool) {
    for (i, p) in params.iter().enumerate() {
        let t = tipos.get(i).copied().unwrap_or(inf.core.dynamic_);
        if let Some(d) = p.default_value {
            inferir(inf, cx, d, t);
        }
        if pular_inicializadores && (p.this_ || p.super_) {
            continue;
        }
        if let Some(n) = &p.name {
            declarar_local(
                inf,
                cx,
                Local { nome: n.sym, tipo: t, final_: p.final_, late: false, const_: false, offset: n.span.start, funcao_local: false },
                true,
            );
        }
    }
}

/// Infere o corpo de uma função/método/construtor declarado.
pub(crate) fn inferir_funcao_declarada(inf: &mut BodyInferrer<'_>, f: FunctionElementId) {
    let fe = inf.program.function(f);
    match fe.node {
        FunctionRef::Function { unit, function } => {
            let af = inf.program.unit(unit).ast.function(function);
            let dados = inf.outline.functions[f.0 as usize].clone();
            let mut cx = Corpo::para_funcao(inf, f, unit);
            for &p in dados.type_params.iter() {
                let nome = inf.table.param(p).name;
                cx.declarar_tipo_param(nome, p);
            }
            cx.empurrar_escopo();
            if let Some(ps) = &af.parameters {
                let tipos: Vec<TypeId> = dados.parameters.iter().map(|p| p.ty).collect();
                declarar_parametros(inf, &mut cx, ps, &tipos, false);
            }
            let ret = dados.return_type;
            let ctx_ret = inf.contexto_de_retorno_declarado(ret, af.modifier);
            cx.funcoes.push(CtxFuncao { modificador: af.modifier, retorno: Some(ret), contexto_retorno: ctx_ret, retornados: Vec::new(), retorno_vazio: false });
            corpo_de_funcao(inf, &mut cx, &af.body, af.modifier, ret);
            cx.funcoes.pop();
        }
        FunctionRef::Constructor { unit, member } => {
            let ast::MemberKind::Constructor(ctor) = &inf.program.unit(unit).ast.member(member).kind else { return };
            let dados = inf.outline.functions[f.0 as usize].clone();
            let mut cx = Corpo::para_funcao(inf, f, unit);
            if !fe.factory {
                cx.estatico = false;
                if let Some(c) = fe.class {
                    cx.tipo_this = Some(inf.tipo_this_classe(c));
                }
            }
            let tipos: Vec<TypeId> = dados.parameters.iter().map(|p| p.ty).collect();
            // Escopo dos inicializadores: todos os parâmetros.
            cx.empurrar_escopo();
            declarar_parametros(inf, &mut cx, &ctor.parameters, &tipos, false);
            // Inicializadores não veem `this` (salvo o acesso ao campo inicializado).
            let this_salvo = cx.tipo_this.take();
            let estatico_salvo = cx.estatico;
            cx.estatico = true;
            for init in ctor.initializers.iter() {
                inicializador(inf, &mut cx, fe.class, init);
            }
            cx.tipo_this = this_salvo;
            cx.estatico = estatico_salvo;
            cx.tirar_escopo();
            // Escopo do corpo: parâmetros sem `this.`/`super.`.
            cx.empurrar_escopo();
            let mut cx2 = cx;
            for (i, p) in ctor.parameters.iter().enumerate() {
                if p.this_ || p.super_ {
                    continue;
                }
                if let Some(n) = &p.name {
                    let t = tipos.get(i).copied().unwrap_or(inf.core.dynamic_);
                    let id = cx2.declarar(Local { nome: n.sym, tipo: t, final_: p.final_, late: false, const_: false, offset: n.span.start, funcao_local: false });
                    cx2.fluxo.inicializar(id);
                }
            }
            let v = inf.core.void_;
            let ret = if fe.factory { dados.return_type } else { v };
            let ctx_ret = ret;
            cx2.funcoes.push(CtxFuncao { modificador: AsyncModifier::None, retorno: Some(ret), contexto_retorno: ctx_ret, retornados: Vec::new(), retorno_vazio: false });
            corpo_de_funcao(inf, &mut cx2, &ctor.body, AsyncModifier::None, ret);
        }
        FunctionRef::None => {}
    }
}

/// Um inicializador de construtor.
fn inicializador(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, classe: Option<ClassId>, init: &ast::Initializer) {
    let u = inf.core.unknown;
    match init {
        ast::Initializer::Field { name, value, .. } => {
            let campo = classe.and_then(|c| inf.program.class(c).fields.iter().copied().find(|&v| inf.program.variable(v).name == name.sym));
            let t = campo.map(|v| inf.tipo_variavel(v)).unwrap_or(u);
            let tv = inferir(inf, cx, *value, t);
            if campo.is_some() {
                let sp = inf.span_expr(cx.unit, *value);
                inf.verificar_atribuivel(tv, t, sp, INVALID_ASSIGNMENT.template);
            }
        }
        ast::Initializer::Super { constructor, arguments, .. } => {
            let alvo = classe.and_then(|c| {
                let sup = inf.outline.classes[c.0 as usize].supertype?;
                let Type::Interface { class: sc, args, .. } = inf.table.get(sup).clone() else { return None };
                Some((sc, args))
            });
            chamar_construtor_de(inf, cx, alvo, *constructor, arguments);
        }
        ast::Initializer::Redirect { constructor, arguments, .. } => {
            let alvo = classe.map(|c| {
                let ps = inf.outline.classes[c.0 as usize].type_params.clone();
                let args: Vec<TypeId> = ps.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
                (c, args.into_boxed_slice())
            });
            chamar_construtor_de(inf, cx, alvo, *constructor, arguments);
        }
        ast::Initializer::Assert { condition, message, .. } => {
            expr::condicao_verificada(inf, cx, *condition);
            if let Some(m) = message {
                inferir_livre(inf, cx, *m);
            }
        }
    }
}

/// `super(...)`/`this(...)`: invoca o construtor da classe instanciada.
fn chamar_construtor_de(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, alvo: Option<(ClassId, Box<[TypeId]>)>, nome: Option<ast::Name>, args: &ast::Arguments) {
    let u = inf.core.unknown;
    let chave = nome.map(|n| n.sym).or(inf.sym.vazio);
    let f = alvo.as_ref().and_then(|(c, _)| chave.and_then(|k| inf.program.class(*c).constructors.get(&k).copied()));
    match (alvo, f) {
        (Some((c, targs)), Some(f)) => {
            let sig = inf.outline.functions[f.0 as usize].signature;
            let ps = inf.outline.classes[c.0 as usize].type_params.clone();
            let sig = if ps.len() == targs.len() {
                let mapa = inf.mapa(&ps, &targs);
                inf.subst(sig, &mapa)
            } else {
                sig
            };
            super::chamadas::invocar(inf, cx, sig, args, u, None);
        }
        _ => {
            for a in args.args.iter() {
                inferir_livre(inf, cx, a.value);
            }
        }
    }
}

/// Infere um corpo (bloco ou expressão) com o retorno já no contexto.
fn corpo_de_funcao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, body: &FunctionBody, m: AsyncModifier, ret: TypeId) {
    match body {
        FunctionBody::Expression(e) => {
            let ctx = cx.funcoes.last().map(|f| f.contexto_retorno).unwrap_or(inf.core.unknown);
            let t = inferir(inf, cx, *e, ctx);
            if m == AsyncModifier::None && !matches!(inf.table.get(ret), Type::Void | Type::Dynamic) {
                let sp = inf.span_expr(cx.unit, *e);
                inf.verificar_atribuivel(t, ret, sp, RETURN_OF_INVALID_TYPE.template);
            }
        }
        FunctionBody::Block(s) => {
            instrucoes::inferir_instrucao(inf, cx, *s);
        }
        _ => {}
    }
}

/// Tipo de uma variável de topo/campo sem tipo: sobreposição (campo que
/// sobrepõe um getter) ou inicializador (`Null` vira `dynamic`).
pub(crate) fn inferir_tipo_de_variavel_sem_tipo(inf: &mut BodyInferrer<'_>, vid: VariableId) -> TypeId {
    let v = inf.program.variable(vid);
    // Sobreposição de membro herdado (campos de instância).
    if !v.static_ {
        if let Some(c) = v.class {
            if let Some(t) = tipo_sobreposto(inf, c, v.name) {
                if let Some((unit, init)) = inf.inicializador(vid) {
                    let mut cx = Corpo::para_variavel(inf, vid, unit);
                    inferir(inf, &mut cx, init, t);
                }
                return t;
            }
        }
    }
    match inf.inicializador(vid) {
        Some((unit, init)) => {
            if inf.program.library(v.library).is_sdk && inf.body_types.units[unit.0 as usize].static_types.is_empty() {
                // Unidade do SDK: infere sem tabela lateral (os `set_*` ignoram).
            }
            let mut cx = Corpo::para_variavel(inf, vid, unit);
            let u = inf.core.unknown;
            let t = inferir(inf, &mut cx, init, u);
            if matches!(inf.table.get(t), Type::Null) {
                inf.core.dynamic_
            } else {
                t
            }
        }
        None => match v.node {
            VariableRef::EnumConstant { .. } => v.class.map(|c| inf.tipo_this_classe(c)).unwrap_or(inf.core.dynamic_),
            _ => inf.core.dynamic_,
        },
    }
}

/// Tipo do getter homônimo nas superinterfaces diretas da classe, se houver.
fn tipo_sobreposto(inf: &mut BodyInferrer<'_>, c: ClassId, nome: dartforge_intern::SymbolId) -> Option<TypeId> {
    let this = inf.tipo_this_classe(c);
    for (sup, _) in crate::scope::supertipos_ordenados(inf.program, &inf.outline.hierarchy, c) {
        if let Some(&f) = inf.program.class(sup).instance_members.get(&nome) {
            let (t, metodo) = inf.tipo_do_membro_declarado(f, false);
            if metodo {
                return None;
            }
            return Some(inf.substituir_do_dono(this, c, sup, t));
        }
    }
    None
}

/// Extrai de um contexto o tipo de função (através de `?` e `FutureOr`).
fn funcao_do_contexto(inf: &mut BodyInferrer<'_>, ctx: TypeId) -> Option<TypeId> {
    if inf.e_desconhecido(ctx) {
        return None;
    }
    let t = inf.nao_nulo(ctx);
    match inf.table.get(t).clone() {
        Type::Function { .. } => Some(t),
        Type::FutureOr { arg, .. } => funcao_do_contexto(inf, arg),
        _ => None,
    }
}

/// Expressão de função (closure) no contexto `ctx`.
pub(crate) fn expressao_de_funcao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, fid: ast::FunctionId, ctx: TypeId) -> TypeId {
    let (t, _) = funcao_literal(inf, cx, fid, ctx, None);
    t
}

/// Infere uma função literal ou local. `local` é o id da função local
/// (declarada antes, para chamadas recursivas).
fn funcao_literal(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, fid: ast::FunctionId, ctx: TypeId, local: Option<crate::resolved::LocalId>) -> (TypeId, ()) {
    let af = inf.program.unit(cx.unit).ast.function(fid);
    let u = inf.core.unknown;
    let ctx_fn = funcao_do_contexto(inf, ctx);
    // Captura de escrita: locais externos escritos dentro da closure perdem
    // as promoções (aqui e depois dela).
    let escritos = instrucoes::nomes_escritos_em_funcao(inf, cx.unit, af);
    let mut capturados: Vec<crate::resolved::LocalId> = Vec::new();
    for n in &escritos {
        if let Some(super::corpo::Nome::Local(id)) = cx.buscar(*n) {
            capturados.push(id);
        }
    }
    let fluxo_antes = cx.fluxo.clone();
    let mut fluxo_dentro = cx.fluxo.clone();
    for &id in &capturados {
        fluxo_dentro.capturar(id);
    }
    for (i, e) in cx.escritos_em_closure.clone().iter().enumerate() {
        let _ = i;
        if let Some(super::corpo::Nome::Local(id)) = cx.buscar(*e) {
            fluxo_dentro.capturar(id);
        }
    }
    cx.escritos_em_closure.extend(escritos.iter().copied());
    cx.fluxo = fluxo_dentro;
    cx.empurrar_escopo();
    // Parâmetros de tipo da função literal.
    let mut tps: Vec<TypeParamId> = Vec::new();
    for tp in af.type_params.iter() {
        let p = inf.table.alloc_type_param(tp.name.sym, crate::table::TypeParamOwner::GenericFunctionType, inf.core.object_nullable, crate::table::Variance::Unspecified);
        cx.declarar_tipo_param(tp.name.sym, p);
        tps.push(p);
    }
    for (tp, &p) in af.type_params.iter().zip(tps.iter()) {
        if let Some(b) = tp.bound {
            let bt = inf.tipo_de_anotacao(cx, b);
            inf.table.set_type_param_bound(p, bt);
        }
    }
    // Contexto de função com os parâmetros de tipo renomeados para os nossos.
    let (cpos, copt, cnamed, cret) = match ctx_fn.map(|t| inf.table.get(t).clone()) {
        Some(Type::Function { type_params, ret, positional, optional, named, .. }) => {
            let mapa: HashMap<TypeParamId, TypeId> = if type_params.len() == tps.len() {
                type_params.iter().copied().zip(tps.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false }))).collect()
            } else {
                HashMap::new()
            };
            let s = |inf: &mut BodyInferrer<'_>, t: TypeId| inf.subst(t, &mapa);
            let pos: Vec<TypeId> = positional.iter().map(|&t| s(inf, t)).collect();
            let opt: Vec<TypeId> = optional.iter().map(|&t| s(inf, t)).collect();
            let named: Vec<(dartforge_intern::SymbolId, TypeId, bool)> = named.iter().map(|(n, t, r)| (*n, s(inf, *t), *r)).collect();
            let r = s(inf, ret);
            (pos, opt, named, Some(r))
        }
        _ => (Vec::new(), Vec::new(), Vec::new(), None),
    };
    let todos_ctx: Vec<TypeId> = cpos.iter().chain(copt.iter()).copied().collect();
    let mut pos = Vec::new();
    let mut opt = Vec::new();
    let mut named = Vec::new();
    let mut ipos = 0usize;
    if let Some(ps) = &af.parameters {
        for p in ps.iter() {
            let escopo = cx.parametros_de_tipo_visiveis();
            let escrito = inf.tipo_de_parametro_escrito(cx.unit, cx.lib, p, &escopo);
            let do_ctx = match p.kind {
                ast::ParameterKind::Named => p.name.and_then(|n| cnamed.iter().find(|(s, _, _)| *s == n.sym).map(|(_, t, _)| *t)),
                _ => {
                    let t = todos_ctx.get(ipos).copied();
                    ipos += 1;
                    t
                }
            };
            let t = match escrito {
                Some(t) => t,
                None => match do_ctx {
                    Some(k) => {
                        let s = inf.fecho_maior(k);
                        let n = inf.core.null;
                        if inf.sub(s, n) {
                            inf.core.object_nullable
                        } else {
                            s
                        }
                    }
                    None => inf.core.dynamic_,
                },
            };
            if let Some(d) = p.default_value {
                inferir(inf, cx, d, t);
            }
            match p.kind {
                ast::ParameterKind::Required => pos.push(t),
                ast::ParameterKind::Optional => opt.push(t),
                ast::ParameterKind::Named => {
                    if let Some(n) = &p.name {
                        named.push((n.sym, t, p.required));
                    }
                }
            }
            if let Some(n) = &p.name {
                declarar_local(
                    inf,
                    cx,
                    Local { nome: n.sym, tipo: t, final_: p.final_, late: false, const_: false, offset: n.span.start, funcao_local: false },
                    true,
                );
            }
        }
    }
    // Retorno: escrito, ou inferido com o esquema imposto pelo contexto.
    let escrito_ret = af.return_type.map(|r| inf.tipo_de_anotacao(cx, r));
    let m = af.modifier;
    let (ctx_ret, declarado) = match escrito_ret {
        Some(r) => (inf.contexto_de_retorno_declarado(r, m), Some(r)),
        None => {
            let s = cret.unwrap_or(u);
            let k = match m {
                AsyncModifier::None => s,
                AsyncModifier::AsyncStar => inf.como_instancia_de_esquema(s, inf.core.stream_class).unwrap_or(u),
                AsyncModifier::SyncStar => inf.como_instancia_de_esquema(s, inf.core.iterable_class).unwrap_or(u),
                AsyncModifier::Async => {
                    let fv = inf.tipo_valor_futuro_esquema(s);
                    inf.futuro_ou(fv)
                }
            };
            (k, None)
        }
    };
    // Função local com retorno escrito: tipo disponível para recursão.
    if let (Some(id), Some(r)) = (local, declarado) {
        let ft = inf.table.intern(Type::Function {
            type_params: tps.clone().into_boxed_slice(),
            ret: r,
            positional: pos.clone().into_boxed_slice(),
            optional: opt.clone().into_boxed_slice(),
            named: named.clone().into_boxed_slice(),
            nullable: false,
        });
        cx.locais[id.0 as usize].tipo = ft;
        cx.locais[id.0 as usize].funcao_local = false;
    }
    cx.funcoes.push(CtxFuncao { modificador: m, retorno: declarado, contexto_retorno: ctx_ret, retornados: Vec::new(), retorno_vazio: false });
    let saltos_salvos = std::mem::take(&mut cx.saltos);
    let cascatas_salvas = std::mem::take(&mut cx.cascatas);
    let (corpo_t, completa) = match &af.body {
        FunctionBody::Expression(e) => {
            let t = inferir(inf, cx, *e, ctx_ret);
            (Some(t), false)
        }
        FunctionBody::Block(s) => {
            instrucoes::inferir_instrucao(inf, cx, *s);
            (None, cx.fluxo.alcancavel)
        }
        _ => (None, false),
    };
    cx.saltos = saltos_salvos;
    cx.cascatas = cascatas_salvas;
    let fc = cx.funcoes.pop().unwrap();
    cx.tirar_escopo();
    cx.fluxo = fluxo_antes;
    for &id in &capturados {
        cx.fluxo.capturar(id);
    }
    let ret = match declarado {
        Some(r) => r,
        None => {
            // Tipo efetivamente retornado.
            let mut t = match corpo_t {
                Some(t) => t,
                None => {
                    if completa && !matches!(m, AsyncModifier::SyncStar | AsyncModifier::AsyncStar) {
                        inf.core.null
                    } else {
                        inf.core.never
                    }
                }
            };
            if corpo_t.is_none() {
                for &r in &fc.retornados {
                    t = inf.up(r, t);
                }
                if fc.retorno_vazio && !matches!(m, AsyncModifier::SyncStar | AsyncModifier::AsyncStar) {
                    let n = inf.core.null;
                    t = inf.up(n, t);
                }
                if completa && matches!(m, AsyncModifier::SyncStar | AsyncModifier::AsyncStar) && fc.retornados.is_empty() {
                    // Gerador sem `yield`: elemento `Never`.
                }
            } else if m == AsyncModifier::Async {
                t = inf.flatten(t);
            }
            let r = inf.fecho_maior(ctx_ret);
            let s = if matches!(inf.table.get(r), Type::Void)
                || (m == AsyncModifier::Async && matches!(inf.table.get(r), Type::FutureOr { arg, .. } if matches!(inf.table.get(*arg), Type::Void)))
            {
                inf.core.void_
            } else if inf.sub(t, r) {
                t
            } else {
                r
            };
            match m {
                AsyncModifier::Async => {
                    let f = inf.flatten(s);
                    inf.futuro(f)
                }
                AsyncModifier::AsyncStar => inf.fluxo_de(s),
                AsyncModifier::SyncStar => inf.iteravel(s),
                AsyncModifier::None => s,
            }
        }
    };
    let ft = inf.table.intern(Type::Function {
        type_params: tps.into_boxed_slice(),
        ret,
        positional: pos.into_boxed_slice(),
        optional: opt.into_boxed_slice(),
        named: named.into_boxed_slice(),
        nullable: false,
    });
    (ft, ())
}

impl<'a> BodyInferrer<'a> {
    /// `S` é `C<S1>` (sem subtipos): `S1`.
    pub(crate) fn como_instancia_de_esquema(&mut self, s: TypeId, c: Option<ClassId>) -> Option<TypeId> {
        let c = c?;
        match self.table.get(s) {
            Type::Interface { class, args, .. } if *class == c && args.len() == 1 => Some(args[0]),
            _ => None,
        }
    }
}

/// Declaração de função local.
pub(crate) fn funcao_local(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, fid: ast::FunctionId) {
    let af = inf.program.unit(cx.unit).ast.function(fid);
    let Some(nome) = af.name else { return };
    let d = inf.core.dynamic_;
    let id = cx.declarar(Local { nome: nome.sym, tipo: d, final_: true, late: false, const_: false, offset: nome.span.start, funcao_local: true });
    cx.fluxo.inicializar(id);
    let u = inf.core.unknown;
    let (t, _) = funcao_literal(inf, cx, fid, u, Some(id));
    cx.locais[id.0 as usize].tipo = t;
    cx.locais[id.0 as usize].funcao_local = false;
    inf.body_types.units[cx.unit.0 as usize].set_tipo_local(nome.span.start, t);
}

/// Metadados (anotações) e argumentos de constantes de enum de uma unidade.
pub(crate) fn inferir_metadados_da_unidade(inf: &mut BodyInferrer<'_>, unit: UnitId) {
    let a = &inf.program.unit(unit).ast;
    for d in a.decls.iter() {
        let (classe, extensao) = classe_da_decl(inf, unit, d);
        for m in d.metadata.iter() {
            anotacao(inf, unit, classe, extensao, m);
        }
        match &d.kind {
            ast::DeclKind::Enum(en) => {
                let Some(c) = classe else { continue };
                for k in en.constants.iter() {
                    for m in k.metadata.iter() {
                        anotacao(inf, unit, Some(c), None, m);
                    }
                    if let Some(args) = &k.arguments {
                        let mut cx = Corpo::novo(inf, unit, Some(c), None, true);
                        let chave = k.constructor.map(|n| n.sym).or(inf.sym.vazio);
                        let f = chave.and_then(|ch| inf.program.class(c).constructors.get(&ch).copied());
                        if let Some(f) = f {
                            let explicitos = if k.type_args.is_empty() {
                                None
                            } else {
                                Some(k.type_args.iter().map(|&t| inf.tipo_de_anotacao(&cx, t)).collect())
                            };
                            let u = inf.core.unknown;
                            super::chamadas::construir(inf, &mut cx, None, c, f, explicitos, args, u);
                        } else {
                            for x in args.args.iter() {
                                inferir_livre(inf, &mut cx, x.value);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    for mb in a.members.iter() {
        for m in mb.metadata.iter() {
            // Classe dona desconhecida aqui: resolve no escopo da biblioteca.
            anotacao(inf, unit, None, None, m);
        }
        if let ast::MemberKind::Constructor(ctor) = &mb.kind {
            for p in ctor.parameters.iter() {
                for m in p.metadata.iter() {
                    anotacao(inf, unit, None, None, m);
                }
            }
        }
    }
    for f in a.functions.iter() {
        if let Some(ps) = &f.parameters {
            for p in ps.iter() {
                for m in p.metadata.iter() {
                    anotacao(inf, unit, None, None, m);
                }
            }
        }
    }
}

fn classe_da_decl(inf: &BodyInferrer<'_>, unit: UnitId, d: &ast::Decl) -> (Option<ClassId>, Option<dartforge_elements::model::ExtensionId>) {
    let lib = inf.program.unit(unit).library;
    let nome = match &d.kind {
        ast::DeclKind::Class(c) => Some(c.name.sym),
        ast::DeclKind::Mixin(m) => Some(m.name.sym),
        ast::DeclKind::Enum(e) => Some(e.name.sym),
        ast::DeclKind::ExtensionType(e) => Some(e.name.sym),
        _ => None,
    };
    match nome.and_then(|n| inf.program.library(lib).declared.get(&n)).and_then(|b| b.getter) {
        Some(Element::Class(c)) => (Some(c), None),
        _ => (None, None),
    }
}

/// Uma anotação: `@x`, `@C(args)`, `@C.nome(args)`, `@p.C(args)`.
fn anotacao(inf: &mut BodyInferrer<'_>, unit: UnitId, classe: Option<ClassId>, _ext: Option<dartforge_elements::model::ExtensionId>, m: &ast::Annotation) {
    let Some(args) = &m.arguments else { return };
    let lib = inf.program.unit(unit).library;
    let mut cx = Corpo::novo(inf, unit, classe, None, true);
    let nomes: Vec<dartforge_intern::SymbolId> = m.name.iter().map(|n| n.sym).collect();
    // Classe e construtor.
    let (c, ctor) = match nomes.as_slice() {
        [c] => (inf.program.lookup(lib, *c).and_then(|b| b.getter), None),
        [a, b] => match inf.program.lookup(lib, *a).and_then(|x| x.getter) {
            Some(el @ Element::Class(_)) => (Some(el), Some(*b)),
            _ => (inf.program.lookup_prefixed(lib, *a, *b).and_then(|x| x.getter), None),
        },
        [p, c, n] => (inf.program.lookup_prefixed(lib, *p, *c).and_then(|x| x.getter), Some(*n)),
        _ => (None, None),
    };
    let u = inf.core.unknown;
    if let Some(Element::Class(c)) = c {
        let chave = ctor.or(inf.sym.vazio);
        if let Some(f) = chave.and_then(|k| inf.program.class(c).constructors.get(&k).copied()) {
            let explicitos = if m.type_args.is_empty() {
                None
            } else {
                Some(m.type_args.iter().map(|&t| inf.tipo_de_anotacao(&cx, t)).collect())
            };
            super::chamadas::construir(inf, &mut cx, None, c, f, explicitos, args, u);
            return;
        }
    }
    for x in args.args.iter() {
        inferir_livre(inf, &mut cx, x.value);
    }
}
