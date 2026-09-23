//! Literais de lista, conjunto e mapa: inferência como a de uma chamada
//! genérica `<E>(E, …) -> List<E>` (`<K, V>(…) -> Map<K, V>`), com os
//! elementos (inclusive `...`, `if`, `for`) gerando restrições; e a decisão
//! conjunto × mapa para `{}` pelos elementos e pelo contexto.

use super::corpo::Corpo;
use super::expr::{self, inferir, inferir_livre};
use super::BodyInferrer;
use crate::codes::*;
use crate::constraints::GenericInferrer;
use crate::table::{Type, TypeId, TypeParamId};
use dartforge_frontend::ast::{CollectionElement, ExprId, ExprKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Forma {
    Lista,
    Conjunto,
    Mapa,
}

impl<'a> BodyInferrer<'a> {
    /// Três parâmetros de tipo auxiliares (E, K, V) para os literais.
    fn params_colecao(&mut self) -> [TypeParamId; 3] {
        let nome = self.table.param(self.core.unknown_param).name;
        let o = self.core.object_nullable;
        if let Some(p) = self.params_colecao_cache {
            return p;
        }
        let mk = |t: &mut crate::table::TypeTable| t.alloc_type_param(nome, crate::table::TypeParamOwner::GenericFunctionType, o, crate::table::Variance::Unspecified);
        let p = [mk(self.table), mk(self.table), mk(self.table)];
        self.params_colecao_cache = Some(p);
        p
    }
}

/// Classificação sintática dos elementos de `{...}`: `Some(true)` mapa,
/// `Some(false)` conjunto, `None` indeciso (só espalhamentos ou vazio).
fn forma_pelos_elementos(els: &[CollectionElement]) -> Option<bool> {
    for el in els {
        match el {
            CollectionElement::MapEntry { .. } => return Some(true),
            CollectionElement::Expression(_) | CollectionElement::NullAwareExpression(_) => return Some(false),
            CollectionElement::If { then, else_, .. } => {
                if let Some(f) = forma_pelos_elementos(std::slice::from_ref(then)) {
                    return Some(f);
                }
                if let Some(e) = else_ {
                    if let Some(f) = forma_pelos_elementos(std::slice::from_ref(e)) {
                        return Some(f);
                    }
                }
            }
            CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => {
                if let Some(f) = forma_pelos_elementos(std::slice::from_ref(body)) {
                    return Some(f);
                }
            }
            CollectionElement::Spread { .. } => {}
        }
    }
    None
}

/// Infere um literal de coleção.
pub(crate) fn literal(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, ctx: TypeId) -> TypeId {
    let a = &inf.program.unit(cx.unit).ast;
    let (forma, type_args, elements, const_) = match &a.expr(e).kind {
        ExprKind::List { const_, type_args, elements } => (Forma::Lista, type_args, elements, *const_),
        ExprKind::SetOrMap { const_, type_args, elements } => {
            let forma = match type_args.len() {
                1 => Forma::Conjunto,
                2 => Forma::Mapa,
                _ => match forma_pelos_elementos(elements) {
                    Some(true) => Forma::Mapa,
                    Some(false) => Forma::Conjunto,
                    None => forma_pelo_contexto(inf, cx, elements, ctx),
                },
            };
            (forma, type_args, elements, *const_)
        }
        _ => unreachable!(),
    };
    let elements: &[CollectionElement] = elements;
    let t = if !type_args.is_empty() {
        let args: Vec<TypeId> = type_args.iter().map(|&t| inf.tipo_de_anotacao(cx, t)).collect();
        let (ce, ck, cv) = match forma {
            Forma::Mapa => (inf.core.dynamic_, args[0], args.get(1).copied().unwrap_or(inf.core.dynamic_)),
            _ => (args[0], inf.core.dynamic_, inf.core.dynamic_),
        };
        for el in elements {
            visitar(inf, cx, el, forma, [ce, ck, cv], None);
        }
        tipo_final(inf, forma, &args)
    } else {
        let p = inf.params_colecao();
        let params: Vec<TypeParamId> = match forma {
            Forma::Mapa => vec![p[1], p[2]],
            _ => vec![p[0]],
        };
        let vars: Vec<TypeId> = params.iter().map(|&x| inf.table.intern(Type::TypeParameter { param: x, nullable: false })).collect();
        let ret = tipo_final(inf, forma, &vars);
        let mut gi = GenericInferrer::new(&params);
        if !inf.e_desconhecido(ctx) {
            let mut env = inf.env();
            gi.constrain_return(ret, ctx, &mut env);
        }
        let mut env = inf.env();
        let prelim = gi.choose_preliminary(&mut env);
        drop(env);
        let d = inf.core.dynamic_;
        let ctxs = match forma {
            Forma::Mapa => [d, prelim[0], prelim[1]],
            _ => [prelim[0], d, d],
        };
        for el in elements {
            visitar(inf, cx, el, forma, ctxs, Some((&mut gi, &params[..])));
        }
        let mut env = inf.env();
        let finais = gi.choose_final(&mut env);
        drop(env);
        tipo_final(inf, forma, &finais)
    };
    if const_ {
        validar_colecao_const(inf, cx.unit, e);
    }
    t
}

fn tipo_final(inf: &mut BodyInferrer<'_>, forma: Forma, args: &[TypeId]) -> TypeId {
    match forma {
        Forma::Lista => inf.lista(args[0]),
        Forma::Conjunto => inf.conjunto(args[0]),
        Forma::Mapa => inf.tipo_mapa(args[0], args[1]),
    }
}

/// `{}` indeciso: pelo contexto (`Map`/`Iterable`), senão pelos
/// espalhamentos inferidos; `{}` vazio sem contexto é mapa.
fn forma_pelo_contexto(inf: &mut BodyInferrer<'_>, _cx: &mut Corpo, _els: &[CollectionElement], ctx: TypeId) -> Forma {
    if !inf.e_desconhecido(ctx) {
        let k = inf.fecho_maior(ctx);
        let k = inf.nao_nulo(k);
        let e_mapa = inf.como_instancia_de(k, inf.core.map_class).is_some() || matches!(inf.table.get(k), Type::Interface { class, .. } if Some(*class) == inf.core.map_class);
        let e_iter = inf.como_instancia_de(k, inf.core.iterable_class).is_some();
        if e_iter && !e_mapa {
            return Forma::Conjunto;
        }
        if e_mapa && !e_iter {
            return Forma::Mapa;
        }
        if let Type::FutureOr { arg, .. } = inf.table.get(k).clone() {
            if inf.como_instancia_de(arg, inf.core.iterable_class).is_some() {
                return Forma::Conjunto;
            }
        }
    }
    Forma::Mapa
}

type Inferidor<'g> = Option<(&'g mut GenericInferrer, &'g [TypeParamId])>;

fn restringir(inf: &mut BodyInferrer<'_>, gi: &mut Inferidor<'_>, t: TypeId, i: usize) {
    if let Some((g, params)) = gi {
        let v = inf.table.intern(Type::TypeParameter { param: params[i], nullable: false });
        let mut env = inf.env();
        g.constrain_argument(t, v, &mut env);
    }
}

/// Visita um elemento, gerando restrições. `ctxs = [E, K, V]`.
fn visitar(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, el: &CollectionElement, forma: Forma, ctxs: [TypeId; 3], mut gi: Inferidor<'_>) {
    let u = inf.core.unknown;
    match el {
        CollectionElement::Expression(x) => {
            let c = if forma == Forma::Mapa { u } else { ctxs[0] };
            let t = inferir(inf, cx, *x, c);
            if forma != Forma::Mapa {
                restringir(inf, &mut gi, t, 0);
            }
        }
        CollectionElement::NullAwareExpression(x) => {
            let c = if forma == Forma::Mapa || inf.e_desconhecido(ctxs[0]) { u } else { inf.anulavel(ctxs[0]) };
            let t = inferir(inf, cx, *x, c);
            let t = inf.nao_nulo(t);
            if forma != Forma::Mapa {
                restringir(inf, &mut gi, t, 0);
            }
        }
        CollectionElement::MapEntry { key, value, null_aware_key, null_aware_value } => {
            let ck = if *null_aware_key && !inf.e_desconhecido(ctxs[1]) { inf.anulavel(ctxs[1]) } else { ctxs[1] };
            let cv = if *null_aware_value && !inf.e_desconhecido(ctxs[2]) { inf.anulavel(ctxs[2]) } else { ctxs[2] };
            let tk = inferir(inf, cx, *key, ck);
            let tv = inferir(inf, cx, *value, cv);
            let tk = if *null_aware_key { inf.nao_nulo(tk) } else { tk };
            let tv = if *null_aware_value { inf.nao_nulo(tv) } else { tv };
            restringir(inf, &mut gi, tk, 0);
            restringir(inf, &mut gi, tv, 1);
        }
        CollectionElement::Spread { value, null_aware } => {
            let c = match forma {
                Forma::Mapa => inf.tipo_mapa(ctxs[1], ctxs[2]),
                _ => inf.iteravel(ctxs[0]),
            };
            let c = if *null_aware { inf.anulavel(c) } else { c };
            let t = inferir(inf, cx, *value, c);
            let t = inf.nao_nulo(t);
            if inf.e_dynamic(t) {
                let d = inf.core.dynamic_;
                restringir(inf, &mut gi, d, 0);
                if forma == Forma::Mapa {
                    restringir(inf, &mut gi, d, 1);
                }
            } else if matches!(inf.table.get(t), Type::Null | Type::Never) {
            } else if forma == Forma::Mapa {
                if let Some(args) = inf.como_instancia_de(t, inf.core.map_class) {
                    restringir(inf, &mut gi, args[0], 0);
                    restringir(inf, &mut gi, args[1], 1);
                }
            } else if let Some(args) = inf.como_instancia_de(t, inf.core.iterable_class) {
                restringir(inf, &mut gi, args[0], 0);
            }
        }
        CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
            let antes = cx.fluxo.clone();
            let (vf, ff) = match case_pattern {
                Some(p) => {
                    let t = inferir_livre(inf, cx, *condition);
                    cx.empurrar_escopo();
                    let r = super::padroes::caso(inf, cx, *p, t, *guard);
                    r
                }
                None => expr::condicao_verificada(inf, cx, *condition),
            };
            cx.fluxo = vf;
            reborrow_visitar(inf, cx, then, forma, ctxs, &mut gi);
            if case_pattern.is_some() {
                cx.tirar_escopo();
            }
            let depois_then = std::mem::replace(&mut cx.fluxo, ff);
            if let Some(e) = else_ {
                reborrow_visitar(inf, cx, e, forma, ctxs, &mut gi);
            }
            let depois_else = std::mem::replace(&mut cx.fluxo, antes);
            cx.fluxo = inf.juntar(&depois_then, &depois_else);
        }
        CollectionElement::For { init, condition, updates, body, .. } => {
            cx.empurrar_escopo();
            if let Some(i) = init {
                super::instrucoes::inicializacao_de_for(inf, cx, i);
            }
            let antes = cx.fluxo.clone();
            if let Some(c) = condition {
                let (vf, _ff) = expr::condicao_verificada(inf, cx, *c);
                cx.fluxo = vf;
            }
            reborrow_visitar(inf, cx, body, forma, ctxs, &mut gi);
            for u2 in updates.iter() {
                inferir_livre(inf, cx, *u2);
            }
            cx.fluxo = antes;
            cx.tirar_escopo();
        }
        CollectionElement::ForIn { await_, target, iterable, body } => {
            cx.empurrar_escopo();
            let antes = cx.fluxo.clone();
            super::instrucoes::cabecalho_for_in(inf, cx, target, *iterable, *await_);
            reborrow_visitar(inf, cx, body, forma, ctxs, &mut gi);
            cx.fluxo = antes;
            cx.tirar_escopo();
        }
    }
}

fn reborrow_visitar(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, el: &CollectionElement, forma: Forma, ctxs: [TypeId; 3], gi: &mut Inferidor<'_>) {
    let sub: Inferidor<'_> = match gi {
        Some((g, p)) => Some((&mut **g, *p)),
        None => None,
    };
    visitar(inf, cx, el, forma, ctxs, sub);
}

/// Diagnósticos de coleções `const` (chaves/elementos repetidos, não constantes).
pub(crate) fn validar_colecao_const(inf: &mut BodyInferrer<'_>, unit: dartforge_elements::model::UnitId, e: ExprId) {
    let a = &inf.program.unit(unit).ast;
    let mut subs = Vec::new();
    {
        let mut av = crate::constant::ConstantEvaluator::new(inf.program, inf.interner, inf.table, inf.core);
        let mut diags: Vec<(String, dartforge_diagnostics::Span)> = Vec::new();
        match &a.expr(e).kind {
            ExprKind::SetOrMap { elements, .. } => {
                let e_mapa = elements.iter().any(|el| matches!(el, CollectionElement::MapEntry { .. }));
                let mut vistos = Vec::new();
                for el in elements.iter() {
                    match el {
                        CollectionElement::MapEntry { key, value, .. } if e_mapa => {
                            match av.evaluate_expr(unit, *key) {
                                Some(v) => {
                                    if vistos.contains(&v) {
                                        diags.push((EQUAL_KEYS_IN_CONST_MAP.template.to_string(), a.expr(*key).span));
                                    } else {
                                        vistos.push(v);
                                    }
                                }
                                None => diags.push((CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE.template.to_string(), a.expr(*key).span)),
                            }
                            if av.evaluate_expr(unit, *value).is_none() {
                                diags.push((CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE.template.to_string(), a.expr(*value).span));
                            }
                            subs.push(*key);
                            subs.push(*value);
                        }
                        CollectionElement::Expression(x) if !e_mapa => {
                            match av.evaluate_expr(unit, *x) {
                                Some(v) => {
                                    if vistos.contains(&v) {
                                        diags.push((EQUAL_ELEMENTS_IN_CONST_SET.template.to_string(), a.expr(*x).span));
                                    } else {
                                        vistos.push(v);
                                    }
                                }
                                None => diags.push((CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE.template.to_string(), a.expr(*x).span)),
                            }
                            subs.push(*x);
                        }
                        _ => {}
                    }
                }
            }
            ExprKind::List { elements, .. } => {
                for el in elements.iter() {
                    if let CollectionElement::Expression(x) = el {
                        subs.push(*x);
                    }
                }
            }
            ExprKind::Parenthesized(i) => subs.push(*i),
            _ => {}
        }
        drop(av);
        for (m, s) in diags {
            inf.aviso(m, s);
        }
    }
    for s in subs {
        validar_colecao_const(inf, unit, s);
    }
}
