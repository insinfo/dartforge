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
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionElementId};
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
    // Chamada genérica cujos parâmetros de tipo estão em escopo (a função
    // chamando a si mesma, `_mergeSort(elements, keyOf, …)` dentro de
    // `_mergeSort<E, K>`): os argumentos mencionam os mesmos parâmetros que
    // a inferência resolve; renomeia para parâmetros novos, como a
    // instanciação da especificação (R-GEN-04: variáveis frescas).
    if !type_params.is_empty()
        && type_params.iter().any(|&p| {
            let n = inf.table.param(p).name;
            matches!(cx.buscar(n), Some(super::corpo::Nome::TipoParam(q)) if q == p)
        })
    {
        let novos = inf.parametros_novos(&type_params);
        let tipos: Vec<TypeId> = novos.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        let mapa = inf.mapa(&type_params, &tipos);
        let ret = inf.subst(ret, &mapa);
        let positional: Box<[TypeId]> = positional.iter().map(|&t| inf.subst(t, &mapa)).collect();
        let optional: Box<[TypeId]> = optional.iter().map(|&t| inf.subst(t, &mapa)).collect();
        let named: Box<[_]> = named.iter().map(|&(n, t, r)| (n, inf.subst(t, &mapa), r)).collect();
        let f2 = inf.table.intern(Type::Function { type_params: novos.into_boxed_slice(), ret, positional, optional, named, nullable: false });
        return invocar(inf, cx, f2, args, ctx, explicitos);
    }
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
    let contexto_numerico = inf.contexto_numerico_pendente.take();
    if type_params.is_empty() {
        for (a, p) in args.args.iter().zip(params.iter()) {
            // `x.clamp(a, b)` / `x.remainder(a)`: o contexto refinado do analyzer.
            let c = match (contexto_numerico, a.name) {
                (Some(c), None) => c,
                _ => p.unwrap_or(u),
            };
            let t = inferir(inf, cx, a.value, c);
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
        if inf.program.class(c).kind == ClassKind::Enum
            && f.is_some_and(|f| !inf.program.function(f).factory)
        {
            inf.aviso(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template.to_string(), a.expr(target).span);
            for arg in args.args.iter() {
                inferir_livre(inf, cx, arg.value);
            }
            return (inf.core.dynamic_, false);
        }
        let t = construir(inf, cx, Some(e), c, f, targs.or(explicitos), args, ctx);
        return (t, false);
    }
    // `E()`: o sem nome implícito do enum não está na tabela de
    // construtores (então `alvo_construtor` não o achou), mas existe e é
    // gerador. Com nome explícito, o braço acima já resolveu (factory é
    // legal, gerador acusa); aqui só falta o implícito.
    if let Some(rt) = referencia_a_tipo(inf, cx, target)
        && !matches!(rt, RefTipo::Extensao(_))
    {
        let c = match rt {
            RefTipo::Classe(c, _) | RefTipo::Alias(c, _, _) => c,
            RefTipo::Extensao(_) => unreachable!(),
        };
        if inf.program.class(c).kind == ClassKind::Enum
            && inf.sym.vazio.is_some_and(|v| inf.construtor_de(c, v).is_none())
        {
            inf.aviso(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template.to_string(), a.expr(target).span);
            for arg in args.args.iter() {
                inferir_livre(inf, cx, arg.value);
            }
            return (inf.core.dynamic_, false);
        }
    }
    let u = inf.core.unknown;
    // `E(x)` / `E<T>(x)`: sobreposição explícita de extensão (R-EXT-02).
    if let Some(RefTipo::Extensao(x)) = referencia_a_tipo(inf, cx, target)
        && args.args.len() == 1
        && args.args[0].name.is_none()
    {
        let dados = inf.outline.extensions[x.0 as usize].clone();
        let ext_args = match &explicitos {
            Some(ex) if ex.len() == dados.type_params.len() => Some(ex.clone()),
            _ => None,
        };
        let ctx_arg = match &ext_args {
            Some(ex) => {
                let mapa = inf.mapa(&dados.type_params, ex);
                inf.subst(dados.on, &mapa)
            }
            None => u,
        };
        let t = inferir(inf, cx, args.args[0].value, ctx_arg);
        let ext_args = match ext_args {
            Some(ex) => ex,
            None => inf.extensao_aplicavel(x, t).unwrap_or_else(|| inf.instanciar_para_limites(&dados.type_params)),
        };
        cx.sobreposicoes.insert(e, (x, ext_args));
        return (t, false);
    }
    match &a.expr(target).kind {
        ExprKind::Property { target: recv, name, null_aware } => {
            let (recv, name, null_aware) = (*recv, *name, *null_aware);
            // `p.f(args)`
            if let ExprKind::Identifier(p) = &a.expr(recv).kind {
                if matches!(resolver_nome(inf, cx, p.sym, false), RefNome::Prefixo) {
                    let t = inferir(inf, cx, target, u);
                    let (r, _) = invocar_valor(inf, cx, t, args, ctx, explicitos, span);
                    return (r, false);
                }
            }
            // `C.m(args)` estático / `E.m(args)`.
            if let Some(rt) = referencia_a_tipo(inf, cx, recv) {
                if let RefTipo::Extensao(x) = rt {
                    if inf.membro_estatico_de_extensao(x, name.sym, false).is_none() {
                        if inf.program.extension(x).instance_members.contains_key(&name.sym) {
                            let msg = format!("{}: '{}'", STATIC_ACCESS_TO_INSTANCE_MEMBER.template, inf.interner.resolve(name.sym));
                            inf.aviso(msg, name.span);
                            for arg in args.args.iter() {
                                inferir_livre(inf, cx, arg.value);
                            }
                            return (inf.core.dynamic_, false);
                        }
                        let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                        let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_METHOD.template, inf.interner.resolve(name.sym), extensao);
                        inf.aviso(msg, name.span);
                        for arg in args.args.iter() {
                            inferir_livre(inf, cx, arg.value);
                        }
                        return (inf.core.dynamic_, false);
                    }
                }
                let t = inferir(inf, cx, target, u);
                let (r, _) = invocar_valor(inf, cx, t, args, ctx, explicitos, span);
                return (r, false);
            }
            // `super.m(args)`.
            if matches!(a.expr(recv).kind, ExprKind::Super) {
                let this = cx.tipo_this.unwrap_or(inf.core.dynamic_);
                registrar(inf, cx, recv, this);
                let t = expr::membro_super(inf, cx, target, name, false);
                registrar(inf, cx, target, t);
                let (r, _) = invocar_valor(inf, cx, t, args, ctx, explicitos, span);
                return (r, false);
            }
            let (r_ty, curto) = receptor(inf, cx, recv, null_aware);
            let mut busca = expr::buscar_membro_do_alvo(inf, cx, recv, r_ty, name.sym, false);
            if matches!(busca, Busca::Ausente) {
                if let Some((x, _)) = cx.sobreposicoes.get(&recv).cloned() {
                    if let Some(m) = inf.membro_estatico_de_extensao(x, name.sym, false) {
                        // O analyzer ainda resolve a assinatura do método,
                        // mas rejeita seu acesso via `E(valor).metodo()`.
                        inf.aviso(
                            EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template.to_string(),
                            name.span,
                        );
                        busca = Busca::Achado(m);
                    }
                }
            }
            match busca {
                Busca::Achado(m) => {
                    resolver(inf, cx, target, m.resolved.clone());
                    registrar(inf, cx, target, m.tipo);
                    // O nome do método guarda o tipo não instanciado (como o
                    // `methodName.staticType` do analyzer).
                    if m.metodo {
                        if let Some(sym) = [inf.sym.remainder, inf.sym.clamp].into_iter().flatten().find(|s| *s == name.sym) {
                            let param = match inf.table.get(m.tipo) {
                                Type::Function { positional, .. } => positional.first().copied(),
                                _ => None,
                            };
                            if let Some(pt) = param {
                                inf.contexto_numerico_pendente = Some(expr::contexto_numerico(inf, r_ty, &m, sym, ctx, pt));
                            }
                        }
                    }
                    let (mut r, _) = if m.metodo {
                        let t = inf.nao_nulo(m.tipo);
                        invocar(inf, cx, t, args, ctx, explicitos)
                    } else {
                        invocar_valor(inf, cx, m.tipo, args, ctx, explicitos, span)
                    };
                    if m.metodo {
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
                    if let Some((x, _)) = cx.sobreposicoes.get(&recv).cloned() {
                        let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                        let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_METHOD.template, inf.interner.resolve(name.sym), extensao);
                        inf.aviso(msg, name.span);
                        let d = inf.core.dynamic_;
                        registrar(inf, cx, target, d);
                        for x in args.args.iter() {
                            inferir_livre(inf, cx, x.value);
                        }
                        return (d, curto);
                    }
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
            if let Some((x, ext_args)) = cx.sobreposicoes.get(&target).cloned() {
                if let Some(call) = inf.sym.call {
                    if let Some(m) = inf.membro_de_extensao_explicita(x, &ext_args, call, false) {
                        let (r, _) = invocar(inf, cx, m.tipo, args, ctx, explicitos);
                        return (r, false);
                    }
                    if let Some(m) = inf.membro_estatico_de_extensao(x, call, false) {
                        // `E(valor)()` usa a lista de argumentos como localização
                        // do erro, conforme o analyzer oficial.
                        inf.aviso(
                            EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template.to_string(),
                            args.span,
                        );
                        let (r, _) = invocar(inf, cx, m.tipo, args, ctx, explicitos);
                        return (r, false);
                    }
                }
                let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                let msg = format!("{}: '{}'", INVOCATION_OF_EXTENSION_WITHOUT_CALL.template, extensao);
                inf.aviso(msg, a.expr(target).span);
                for arg in args.args.iter() {
                    inferir_livre(inf, cx, arg.value);
                }
                return (inf.core.dynamic_, false);
            }
            let (r, _) = invocar_valor(inf, cx, t, args, ctx, explicitos, span);
            (r, false)
        }
    }
}

/// Se `target` (alvo de uma chamada) nomeia um construtor: `(classe, construtor, args explícitos)`.
fn alvo_construtor(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, target: ExprId) -> Option<(ClassId, Option<FunctionElementId>, Option<Vec<TypeId>>)> {
    let vazio = inf.sym.vazio?;
    let tipo_args = |inf: &mut BodyInferrer<'_>, cx: &Corpo, rt: &RefTipo| -> (ClassId, Option<Vec<TypeId>>) {
        match rt {
            RefTipo::Classe(c, Some(ts)) => (*c, Some(ts.iter().map(|&t| inf.tipo_de_anotacao(cx, t)).collect())),
            RefTipo::Classe(c, None) => (*c, None),
            RefTipo::Alias(c, args, _) => (*c, args.clone()),
            RefTipo::Extensao(_) => unreachable!(),
        }
    };
    if let Some(rt) = referencia_a_tipo(inf, cx, target) {
        if matches!(rt, RefTipo::Extensao(_)) {
            return None;
        }
        let (c, args) = tipo_args(inf, cx, &rt);
        let f = inf.construtor_ou_primario(c, vazio)?;
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
        let f = inf.construtor_ou_primario(c, chave)?;
        registrar_referencia(inf, cx, recv);
        if let Some(f) = f
            && inf.program.function(f).class == Some(c)
        {
            resolver(inf, cx, target, Resolved::Constructor(f));
        }
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
    /// Parâmetros novos (por classe) para inferir os argumentos de tipo de
    /// construtores: os da classe podem estar em escopo no ponto da chamada
    /// (`C(x)` dentro de `C<T>`), e não podem ser confundidos.
    /// Construtor `nome` de `c`. Aplicação de mixin (`class A = B with M;`)
    /// tem os construtores generativos da superclasse encaminhados
    /// (especificação, "Mixin Application").
    pub(crate) fn construtor_de(&self, c: ClassId, nome: dartforge_intern::SymbolId) -> Option<FunctionElementId> {
        let cl = self.program.class(c);
        if let Some(&f) = cl.constructors.get(&nome) {
            return Some(f);
        }
        if cl.kind != ClassKind::MixinApplication {
            return None;
        }
        let f = self.construtor_de(cl.supertype_class?, nome)?;
        (!self.program.function(f).factory).then_some(f)
    }

    /// Construtor `nome` de `c`, ou `Some(None)` para o construtor primário
    /// de um tipo de extensão (`extension type Id(int v)`, R-EXT-04), que o
    /// modelo de elementos não cria.
    pub(crate) fn construtor_ou_primario(&self, c: ClassId, nome: dartforge_intern::SymbolId) -> Option<Option<FunctionElementId>> {
        if let Some(f) = self.construtor_de(c, nome) {
            return Some(Some(f));
        }
        let cl = self.program.class(c);
        if cl.kind != ClassKind::ExtensionType {
            return None;
        }
        let d = cl.decl?;
        match &self.program.unit(d.unit).ast.decl(d.decl).kind {
            ast::DeclKind::ExtensionType(et) if et.constructor.map(|n| n.sym).or(self.sym.vazio) == Some(nome) => Some(None),
            _ => None,
        }
    }

    /// `(Representação) -> E<parâmetros>`: o construtor primário.
    pub(crate) fn assinatura_primario(&mut self, c: ClassId) -> TypeId {
        let rep = self.program.class(c).representation;
        let t = rep
            .and_then(|v| self.outline.variables[v.0 as usize].declared_type)
            .unwrap_or(self.core.dynamic_);
        let this = self.tipo_this_classe(c);
        self.table.intern(Type::Function {
            type_params: Box::new([]),
            ret: this,
            positional: Box::new([t]),
            optional: Box::new([]),
            named: Box::new([]),
            nullable: false,
        })
    }

    /// Assinatura do construtor `f` vista de `c` (encaminhado quando `f` é
    /// da superclasse de uma aplicação de mixin): tipos pelos argumentos do
    /// supertipo e retorno `c<parâmetros>`.
    pub(crate) fn assinatura_construtor(&mut self, c: ClassId, f: FunctionElementId) -> TypeId {
        let sig = self.outline.functions[f.0 as usize].signature;
        if self.program.function(f).class == Some(c) {
            return sig;
        }
        let Some(sup) = self.outline.classes[c.0 as usize].supertype else { return sig };
        let Type::Interface { class: sc, args, .. } = self.table.get(sup).clone() else { return sig };
        let s = self.assinatura_construtor(sc, f);
        let params = self.outline.classes[sc.0 as usize].type_params.clone();
        let s = if params.len() == args.len() {
            let mapa = self.mapa(&params, &args);
            self.subst(s, &mapa)
        } else {
            s
        };
        let this = self.tipo_this_classe(c);
        match self.table.get(s).clone() {
            Type::Function { type_params, positional, optional, named, nullable, .. } => {
                self.table.intern(Type::Function { type_params, ret: this, positional, optional, named, nullable })
            }
            _ => s,
        }
    }

    fn parametros_de_construtor(&mut self, c: ClassId) -> (Vec<TypeParamId>, Vec<TypeParamId>) {
        let originais = self.outline.classes[c.0 as usize].type_params.to_vec();
        let novos = self.parametros_novos(&originais);
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
    f: Option<FunctionElementId>,
    explicitos: Option<Vec<TypeId>>,
    args: &ast::Arguments,
    ctx: TypeId,
) -> TypeId {
    // Construtor encaminhado de aplicação de mixin: sem resolução (o
    // elemento é o da superclasse). `None`: construtor primário de tipo de
    // extensão (R-EXT-04), sem elemento.
    let sig = match f {
        Some(f) => {
            if let Some(e) = e
                && inf.program.function(f).class == Some(c)
            {
                resolver(inf, cx, e, Resolved::Constructor(f));
            }
            inf.assinatura_construtor(c, f)
        }
        None => inf.assinatura_primario(c),
    };
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
    // `new C.nome()` chega como tipo de duas partes quando `C` não é prefixo.
    let (binding, constructor) = match (binding, name.len()) {
        (None, 2) => match inf.program.lookup(cx.lib, name[0].sym) {
            Some(b) if matches!(b.getter, Some(Element::Class(_))) && constructor.is_none() => (Some(b), Some(name[1])),
            _ => (binding, constructor),
        },
        _ => (binding, constructor),
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
    let f = chave.and_then(|k| inf.construtor_ou_primario(c, k));
    // `new E()` / `const E()`: o construtor gerador do enum (ou o sem nome
    // implícito) não instancia fora da criação de constantes. Factories
    // seguem o caminho normal (`construir`). O intervalo é o nome do
    // construtor, ou o da classe quando ele é implícito.
    if inf.program.class(c).kind == ClassKind::Enum
        && !f.is_some_and(|f| f.is_some_and(|f| inf.program.function(f).factory))
    {
        let span = constructor.map(|n| n.span).unwrap_or_else(|| name.last().map(|n| n.span).unwrap_or(a.expr(e).span));
        inf.aviso(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template.to_string(), span);
        for x in args.args.iter() {
            inferir_livre(inf, cx, x.value);
        }
        return inf.core.dynamic_;
    }
    let Some(f) = f else {
        for x in args.args.iter() {
            inferir_livre(inf, cx, x.value);
        }
        return inf.tipo_de_classe_com_args(c, explicitos.unwrap_or_default());
    };
    construir(inf, cx, Some(e), c, f, explicitos, args, ctx)
}
