//! Invocações: funções, métodos, construtores, `call`, e a inferência de
//! argumentos de tipo com inferência "horizontal" (`inference-update-1`):
//! literais de função cujos parâmetros dependem de variáveis ainda não
//! resolvidas são inferidos depois dos demais argumentos.

use super::corpo::Corpo;
use super::expr::{self, inferir, inferir_livre, receptor, referencia_a_tipo, registrar, resolver, resolver_nome, RefNome, RefTipo};
use super::membros::Busca;
use super::BodyInferrer;
use crate::codes::*;
use crate::constraints::{instanciar_funcao, GenericInferrer};
use crate::resolved::Resolved;
use crate::table::{Type, TypeId, TypeParamId};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionElementId};
use dartforge_frontend::ast::{self, ExprId, ExprKind};
use std::collections::HashMap;

/// Parâmetro formal correspondente a cada argumento.
fn parametros_dos_argumentos(
    inf: &BodyInferrer<'_>,
    positional: &[TypeId],
    optional: &[TypeId],
    named: &[(dartforge_intern::SymbolId, TypeId, bool)],
    args: &ast::Arguments,
) -> Vec<Option<TypeId>> {
    let _ = inf;
    let mut i = 0usize;
    args.args
        .iter()
        .map(|a| match &a.name {
            None => {
                let t = if i < positional.len() {
                    Some(positional[i])
                } else {
                    optional.get(i - positional.len()).copied()
                };
                i += 1;
                t
            }
            Some(n) => named.iter().find(|(s, _, _)| *s == n.sym).map(|(_, t, _)| *t),
        })
        .collect()
}

/// Diagnósticos de aridade e nomes.
fn verificar_aridade(
    inf: &mut BodyInferrer<'_>,
    positional: &[TypeId],
    optional: &[TypeId],
    named: &[(dartforge_intern::SymbolId, TypeId, bool)],
    args: &ast::Arguments,
) {
    let npos = args.args.iter().filter(|a| a.name.is_none()).count();
    if npos < positional.len() {
        let msg = format!("{}: esperava pelo menos {}, recebeu {}", NOT_ENOUGH_POSITIONAL_ARGUMENTS.template, positional.len(), npos);
        inf.aviso(msg, args.span);
    } else if npos > positional.len() + optional.len() {
        let msg = format!("{}: esperava no máximo {}, recebeu {}", EXTRA_POSITIONAL_ARGUMENTS.template, positional.len() + optional.len(), npos);
        inf.aviso(msg, args.span);
    }
    for a in args.args.iter() {
        if let Some(n) = &a.name {
            if !named.iter().any(|(s, _, _)| *s == n.sym) {
                let msg = format!("{}: '{}'", UNDEFINED_NAMED_PARAMETER.template, inf.interner.resolve(n.sym));
                inf.aviso(msg, n.span);
            }
        }
    }
    for (s, _, req) in named.iter() {
        if *req && !args.args.iter().any(|a| a.name.map(|n| n.sym) == Some(*s)) {
            let msg = format!("{}: '{}'", MISSING_REQUIRED_ARGUMENT.template, inf.interner.resolve(*s));
            inf.aviso(msg, args.span);
        }
    }
}

/// O argumento é um literal de função com algum parâmetro sem tipo (candidato
/// a ser adiado na inferência horizontal).
fn literal_de_funcao_adiavel(inf: &BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> bool {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.expr(e).kind {
        ExprKind::FunctionExpression(f) => {
            let f = a.function(*f);
            f.parameters.as_ref().is_some_and(|ps| ps.iter().any(|p| p.ty.is_none() && p.function_parameters.is_none()))
        }
        ExprKind::Parenthesized(i) => literal_de_funcao_adiavel(inf, cx, *i),
        _ => false,
    }
}

fn menciona(inf: &BodyInferrer<'_>, t: TypeId, params: &[TypeParamId]) -> bool {
    match inf.table.get(t) {
        Type::TypeParameter { param, .. } => params.contains(param),
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.iter().any(|a| menciona(inf, *a, params)),
        Type::FutureOr { arg, .. } => menciona(inf, *arg, params),
        Type::Function { ret, positional, optional, named, .. } => {
            menciona(inf, *ret, params)
                || positional.iter().chain(optional.iter()).any(|a| menciona(inf, *a, params))
                || named.iter().any(|(_, a, _)| menciona(inf, *a, params))
        }
        Type::Record { positional, named, .. } => {
            positional.iter().any(|a| menciona(inf, *a, params)) || named.iter().any(|(_, a)| menciona(inf, *a, params))
        }
        _ => false,
    }
}

/// Invoca um tipo de função com os argumentos; devolve `(retorno, função instanciada)`.
pub(crate) fn invocar(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Corpo,
    f: TypeId,
    args: &ast::Arguments,
    ctx: TypeId,
    explicitos: Option<Vec<TypeId>>,
) -> (TypeId, TypeId) {
    let Type::Function { type_params, ret, positional, optional, named, .. } = inf.table.get(f).clone() else {
        for a in args.args.iter() {
            inferir_livre(inf, cx, a.value);
        }
        return (inf.core.dynamic_, f);
    };
    let u = inf.core.unknown;
    // Argumentos de tipo explícitos: instancia e segue como não genérica.
    if !type_params.is_empty() {
        if let Some(ex) = explicitos {
            if ex.len() == type_params.len() {
                let mut env = inf.env();
                let inst = instanciar_funcao(f, &ex, &mut env);
                drop(env);
                return invocar(inf, cx, inst, args, ctx, None);
            }
        }
    }
    verificar_aridade(inf, &positional, &optional, &named, args);
    let params = parametros_dos_argumentos(inf, &positional, &optional, &named, args);
    if type_params.is_empty() {
        for (a, p) in args.args.iter().zip(params.iter()) {
            let t = inferir(inf, cx, a.value, p.unwrap_or(u));
            if let Some(p) = p {
                let sp = inf.span_expr(cx.unit, a.value);
                inf.verificar_atribuivel(t, *p, sp, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
            }
        }
        return (ret, f);
    }
    // Genérica: para baixo (retorno × contexto), depois argumentos em estágios.
    let mut gi = GenericInferrer::new(&type_params);
    if !inf.e_desconhecido(ctx) {
        let mut env = inf.env();
        gi.constrain_return(ret, ctx, &mut env);
    }
    let mut env = inf.env();
    let mut prelim = gi.choose_preliminary(&mut env);
    drop(env);
    let adiados: Vec<bool> = args
        .args
        .iter()
        .zip(params.iter())
        .map(|(a, p)| p.is_some_and(|p| menciona(inf, p, &type_params)) && literal_de_funcao_adiavel(inf, cx, a.value))
        .collect();
    let mut tipos: Vec<TypeId> = vec![inf.core.dynamic_; args.args.len()];
    for estagio in [false, true] {
        for (i, a) in args.args.iter().enumerate() {
            if adiados[i] != estagio {
                continue;
            }
            if estagio {
                let mut env = inf.env();
                prelim = gi.choose_preliminary(&mut env);
            }
            let contexto = match params[i] {
                Some(p) => {
                    let mapa: HashMap<TypeParamId, TypeId> = type_params.iter().copied().zip(prelim.iter().copied()).collect();
                    inf.subst(p, &mapa)
                }
                None => u,
            };
            let t = inferir(inf, cx, a.value, contexto);
            tipos[i] = t;
            if let Some(p) = params[i] {
                let mut env = inf.env();
                gi.constrain_argument(t, p, &mut env);
            }
        }
    }
    let mut env = inf.env();
    let finais = gi.choose_final(&mut env);
    let inst = instanciar_funcao(f, &finais, &mut env);
    drop(env);
    // Checagem com os parâmetros instanciados.
    if let Type::Function { positional: ip, optional: io, named: inm, ret: iret, .. } = inf.table.get(inst).clone() {
        let ips = parametros_dos_argumentos(inf, &ip, &io, &inm, args);
        for (i, a) in args.args.iter().enumerate() {
            if let Some(p) = ips[i] {
                let sp = inf.span_expr(cx.unit, a.value);
                inf.verificar_atribuivel(tipos[i], p, sp, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
            }
        }
        return (iret, inst);
    }
    (inf.core.dynamic_, inst)
}

/// Invoca um valor de tipo `t` (função, objeto com `call`, `dynamic`).
fn invocar_valor(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, t: TypeId, args: &ast::Arguments, ctx: TypeId, explicitos: Option<Vec<TypeId>>, span: Span) -> (TypeId, TypeId) {
    let t_nn = inf.nao_nulo(t);
    match inf.table.get(t_nn).clone() {
        Type::Function { .. } => invocar(inf, cx, t_nn, args, ctx, explicitos),
        Type::Dynamic => {
            for a in args.args.iter() {
                inferir_livre(inf, cx, a.value);
            }
            (inf.core.dynamic_, t)
        }
        Type::Never => {
            for a in args.args.iter() {
                inferir_livre(inf, cx, a.value);
            }
            (inf.core.never, t)
        }
        Type::Interface { class, .. } if Some(class) == inf.core.function_class => {
            for a in args.args.iter() {
                inferir_livre(inf, cx, a.value);
            }
            (inf.core.dynamic_, t)
        }
        _ => {
            if let Some(call) = inf.sym.call {
                if let Some(m) = inf.membro_de_interface(t_nn, call, false) {
                    return invocar(inf, cx, m.tipo, args, ctx, explicitos);
                }
                if let Busca::Achado(m) = inf.buscar_membro(cx.lib, t_nn, call, false) {
                    return invocar(inf, cx, m.tipo, args, ctx, explicitos);
                }
            }
            for a in args.args.iter() {
                inferir_livre(inf, cx, a.value);
            }
            let msg = format!(
                "{}: expressão de tipo '{}' não é invocável",
                UNDEFINED_METHOD.template,
                inf.table.format(t, inf.interner, inf.program)
            );
            inf.aviso(msg, span);
            (inf.core.dynamic_, t)
        }
    }
}

fn argumentos_de_tipo(inf: &mut BodyInferrer<'_>, cx: &Corpo, args: &ast::Arguments) -> Option<Vec<TypeId>> {
    if args.type_args.is_empty() {
        return None;
    }
    Some(args.type_args.iter().map(|&t| inf.tipo_de_anotacao(cx, t)).collect())
}

/// `f(args)`, `r.m(args)`, `C(args)`, `C.nome(args)`...
pub(crate) fn chamada(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, ctx: TypeId) -> (TypeId, bool) {
    let a = &inf.program.unit(cx.unit).ast;
    let ExprKind::Call { target, arguments } = &a.expr(e).kind else { unreachable!() };
    let (target, args): (ExprId, &ast::Arguments) = (*target, arguments);
    let span = a.expr(e).span;
    let explicitos = argumentos_de_tipo(inf, cx, args);
    // Construtor sem `new`.
    if let Some((c, f, targs)) = alvo_construtor(inf, cx, target) {
        let t = construir(inf, cx, Some(e), c, f, targs.or(explicitos), args, ctx);
        return (t, false);
    }
    let u = inf.core.unknown;
    match &a.expr(target).kind {
        ExprKind::Property { target: recv, name, null_aware } => {
            let (recv, name, null_aware) = (*recv, *name, *null_aware);
            // `p.f(args)`
            if let ExprKind::Identifier(p) = &a.expr(recv).kind {
                if matches!(resolver_nome(inf, cx, p.sym, false), RefNome::Prefixo) {
                    let t = inferir(inf, cx, target, u);
                    let (r, inst) = invocar_valor(inf, cx, t, args, ctx, explicitos, span);
                    if inst != t {
                        registrar(inf, cx, target, inst);
                    }
                    return (r, false);
                }
            }
            // `C.m(args)` estático / `E.m(args)`.
            if referencia_a_tipo(inf, cx, recv).is_some() {
                let t = inferir(inf, cx, target, u);
                let (r, inst) = invocar_valor(inf, cx, t, args, ctx, explicitos, span);
                if inst != t {
                    registrar(inf, cx, target, inst);
                }
                return (r, false);
            }
            // `super.m(args)`.
            if matches!(a.expr(recv).kind, ExprKind::Super) {
                let this = cx.tipo_this.unwrap_or(inf.core.dynamic_);
                registrar(inf, cx, recv, this);
                let t = expr::membro_super(inf, cx, target, name, false);
                registrar(inf, cx, target, t);
                let (r, inst) = invocar_valor(inf, cx, t, args, ctx, explicitos, span);
                registrar(inf, cx, target, inst);
                return (r, false);
            }
            let (r_ty, curto) = receptor(inf, cx, recv, null_aware);
            match inf.buscar_membro(cx.lib, r_ty, name.sym, false) {
                Busca::Achado(m) => {
                    resolver(inf, cx, target, m.resolved.clone());
                    registrar(inf, cx, target, m.tipo);
                    let (mut r, inst) = if m.metodo {
                        let t = inf.nao_nulo(m.tipo);
                        invocar(inf, cx, t, args, ctx, explicitos)
                    } else {
                        invocar_valor(inf, cx, m.tipo, args, ctx, explicitos, span)
                    };
                    if m.metodo {
                        registrar(inf, cx, target, inst);
                        // Refinamento numérico de `remainder`/`clamp`.
                        if let Some(sym) = [inf.sym.remainder, inf.sym.clamp].into_iter().flatten().find(|s| *s == name.sym) {
                            let tipos: Vec<TypeId> = args
                                .args
                                .iter()
                                .map(|x| inf.body_types.units[cx.unit.0 as usize].get_type(x.value).unwrap_or(inf.core.dynamic_))
                                .collect();
                            r = expr::refinar_numerico(inf, r_ty, &m, sym, &tipos, r);
                        }
                    }
                    (r, curto)
                }
                Busca::Dinamico => {
                    resolver(inf, cx, target, Resolved::Dynamic);
                    // Métodos de `Object` mantêm o tipo mesmo em `dynamic`.
                    let o = inf.core.object;
                    if let Some(m) = inf.membro_de_interface(o, name.sym, false) {
                        if m.metodo {
                            registrar(inf, cx, target, m.tipo);
                            let (r, _) = invocar(inf, cx, m.tipo, args, ctx, explicitos);
                            return (r, curto);
                        }
                    }
                    let d = inf.core.dynamic_;
                    registrar(inf, cx, target, d);
                    for x in args.args.iter() {
                        inferir_livre(inf, cx, x.value);
                    }
                    (inf.core.dynamic_, curto)
                }
                Busca::Nunca => {
                    let n = inf.core.never;
                    registrar(inf, cx, target, n);
                    for x in args.args.iter() {
                        inferir_livre(inf, cx, x.value);
                    }
                    (inf.core.never, curto)
                }
                Busca::Ausente => {
                    let msg = format!(
                        "{}: '{}' para o tipo '{}'",
                        UNDEFINED_METHOD.template,
                        inf.interner.resolve(name.sym),
                        inf.table.format(r_ty, inf.interner, inf.program)
                    );
                    inf.aviso(msg, name.span);
                    let d = inf.core.dynamic_;
                    registrar(inf, cx, target, d);
                    for x in args.args.iter() {
                        inferir_livre(inf, cx, x.value);
                    }
                    (inf.core.dynamic_, curto)
                }
            }
        }
        _ => {
            let t = inferir(inf, cx, target, u);
            let (r, inst) = invocar_valor(inf, cx, t, args, ctx, explicitos, span);
            if inst != t && matches!(inf.table.get(t), Type::Function { .. }) {
                registrar(inf, cx, target, inst);
            }
            (r, false)
        }
    }
}

/// Se `target` (alvo de uma chamada) nomeia um construtor: `(classe, construtor, args explícitos)`.
fn alvo_construtor(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, target: ExprId) -> Option<(ClassId, FunctionElementId, Option<Vec<TypeId>>)> {
    let vazio = inf.sym.vazio?;
    let tipo_args = |inf: &mut BodyInferrer<'_>, cx: &Corpo, rt: &RefTipo| -> (ClassId, Option<Vec<TypeId>>) {
        match rt {
            RefTipo::Classe(c, Some(ts)) => (*c, Some(ts.iter().map(|&t| inf.tipo_de_anotacao(cx, t)).collect())),
            RefTipo::Classe(c, None) => (*c, None),
            RefTipo::Alias(c, args) => (*c, if args.is_empty() { None } else { Some(args.clone()) }),
            RefTipo::Extensao(_) => unreachable!(),
        }
    };
    if let Some(rt) = referencia_a_tipo(inf, cx, target) {
        if matches!(rt, RefTipo::Extensao(_)) {
            return None;
        }
        let (c, args) = tipo_args(inf, cx, &rt);
        let &f = inf.program.class(c).constructors.get(&vazio)?;
        registrar_referencia(inf, cx, target);
        return Some((c, f, args));
    }
    let a = &inf.program.unit(cx.unit).ast;
    if let ExprKind::Property { target: recv, name, null_aware: false } = &a.expr(target).kind {
        let (recv, name) = (*recv, *name);
        let rt = referencia_a_tipo(inf, cx, recv)?;
        if matches!(rt, RefTipo::Extensao(_)) {
            return None;
        }
        let (c, args) = tipo_args(inf, cx, &rt);
        let chave = if Some(name.sym) == inf.sym.new_ { vazio } else { name.sym };
        let &f = inf.program.class(c).constructors.get(&chave)?;
        registrar_referencia(inf, cx, recv);
        resolver(inf, cx, target, Resolved::Constructor(f));
        return Some((c, f, args));
    }
    None
}

/// Registra tipo/resolução dos nós de uma referência a classe.
fn registrar_referencia(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId) {
    let a = &inf.program.unit(cx.unit).ast;
    let tt = inf.core.type_;
    match &a.expr(e).kind {
        ExprKind::Identifier(n) => {
            if let Some(el) = inf.program.lookup(cx.lib, n.sym).and_then(|b| b.getter) {
                resolver(inf, cx, e, Resolved::Element(el));
            }
            registrar(inf, cx, e, tt);
        }
        ExprKind::Property { target, name, .. } => {
            let (target, name) = (*target, *name);
            if let ExprKind::Identifier(p) = &a.expr(target).kind {
                resolver(inf, cx, target, Resolved::Prefix(cx.lib));
                if let Some(el) = inf.program.lookup_prefixed(cx.lib, p.sym, name.sym).and_then(|b| b.getter) {
                    resolver(inf, cx, e, Resolved::Element(el));
                }
            }
            registrar(inf, cx, e, tt);
        }
        ExprKind::TypeArguments { target, .. } => {
            let t = *target;
            registrar_referencia(inf, cx, t);
            registrar(inf, cx, e, tt);
        }
        _ => {}
    }
}

impl<'a> BodyInferrer<'a> {
    /// Parâmetros novos (por classe, reusados) para inferir os argumentos
    /// de tipo de construtores: os da classe podem estar em escopo no ponto
    /// da chamada (`C(x)` dentro de `C<T>`), e não podem ser confundidos.
    fn parametros_de_construtor(&mut self, c: ClassId) -> (Vec<TypeParamId>, Vec<TypeParamId>) {
        let originais = self.outline.classes[c.0 as usize].type_params.to_vec();
        if originais.is_empty() {
            return (originais, Vec::new());
        }
        if let Some(n) = self.params_construtor.get(&c.0) {
            return (originais, n.clone());
        }
        let novos: Vec<TypeParamId> = originais
            .iter()
            .map(|&p| {
                let d = self.table.param(p).clone();
                self.table.alloc_type_param(d.name, crate::table::TypeParamOwner::GenericFunctionType, d.bound, d.variance)
            })
            .collect();
        let tipos: Vec<TypeId> = novos.iter().map(|&p| self.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        let mapa = self.mapa(&originais, &tipos);
        for &p in &novos {
            let b = self.table.param(p).bound;
            let b = self.subst(b, &mapa);
            self.table.set_type_param_bound(p, b);
        }
        self.params_construtor.insert(c.0, novos.clone());
        (originais, novos)
    }
}

/// Invocação de construtor: infere os argumentos de tipo da classe quando
/// omitidos (como uma função genérica sobre os parâmetros da classe).
#[allow(clippy::too_many_arguments)]
pub(crate) fn construir(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Corpo,
    e: Option<ExprId>,
    c: ClassId,
    f: FunctionElementId,
    explicitos: Option<Vec<TypeId>>,
    args: &ast::Arguments,
    ctx: TypeId,
) -> TypeId {
    if let Some(e) = e {
        resolver(inf, cx, e, Resolved::Constructor(f));
    }
    let sig = inf.outline.functions[f.0 as usize].signature;
    let (originais, novos) = inf.parametros_de_construtor(c);
    if originais.is_empty() {
        let (r, _) = invocar(inf, cx, sig, args, ctx, None);
        return r;
    }
    if let Some(ex) = explicitos.filter(|x| x.len() == originais.len()) {
        let mapa = inf.mapa(&originais, &ex);
        let s = inf.subst(sig, &mapa);
        let (r, _) = invocar(inf, cx, s, args, ctx, None);
        return r;
    }
    // Função genérica `<T'..>(params) -> C<T'..>`.
    let tipos: Vec<TypeId> = novos.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
    let mapa = inf.mapa(&originais, &tipos);
    let s = inf.subst(sig, &mapa);
    let Type::Function { ret, positional, optional, named, .. } = inf.table.get(s).clone() else { return inf.core.dynamic_ };
    let generica = inf.table.intern(Type::Function {
        type_params: novos.into_boxed_slice(),
        ret,
        positional,
        optional,
        named,
        nullable: false,
    });
    // Contexto `C<...>?` ou supertipo: a restrição do retorno cuida.
    let (r, _) = invocar(inf, cx, generica, args, ctx, None);
    r
}

/// `new C<T>.nome(args)` / `const C(args)`.
pub(crate) fn instanciacao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, ctx: TypeId) -> TypeId {
    let a = &inf.program.unit(cx.unit).ast;
    let ExprKind::InstanceCreation { ty, constructor, arguments, .. } = &a.expr(e).kind else { unreachable!() };
    let (ty, constructor, args): (ast::TypeId, Option<ast::Name>, &ast::Arguments) = (*ty, *constructor, arguments);
    let node = a.ty(ty);
    let ast::TypeKind::Named { name, args: targs } = &node.kind else {
        for x in args.args.iter() {
            inferir_livre(inf, cx, x.value);
        }
        return inf.core.dynamic_;
    };
    let binding = if name.len() == 2 {
        inf.program.lookup_prefixed(cx.lib, name[0].sym, name[1].sym)
    } else {
        inf.program.lookup(cx.lib, name[0].sym)
    };
    let (c, explicitos) = match binding.and_then(|b| b.getter) {
        Some(Element::Class(c)) => {
            let ex = if targs.is_empty() { None } else { Some(targs.iter().map(|&t| inf.tipo_de_anotacao(cx, t)).collect::<Vec<_>>()) };
            (c, ex)
        }
        Some(Element::Typedef(_)) => {
            let t = inf.tipo_de_anotacao(cx, ty);
            match inf.table.get(t).clone() {
                Type::Interface { class, args, .. } | Type::ExtensionType { decl: class, args, .. } => (class, Some(args.to_vec())),
                _ => {
                    for x in args.args.iter() {
                        inferir_livre(inf, cx, x.value);
                    }
                    return inf.core.dynamic_;
                }
            }
        }
        _ => {
            for x in args.args.iter() {
                inferir_livre(inf, cx, x.value);
            }
            return inf.core.dynamic_;
        }
    };
    let chave = match constructor {
        Some(n) if Some(n.sym) != inf.sym.new_ => Some(n.sym),
        _ => inf.sym.vazio,
    };
    let Some(&f) = chave.and_then(|k| inf.program.class(c).constructors.get(&k)) else {
        for x in args.args.iter() {
            inferir_livre(inf, cx, x.value);
        }
        return inf.tipo_de_classe_com_args(c, explicitos.unwrap_or_default());
    };
    construir(inf, cx, Some(e), c, f, explicitos, args, ctx)
}
