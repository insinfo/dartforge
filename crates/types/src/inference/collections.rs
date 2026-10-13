//! Literais de lista, conjunto e mapa: inferência como a de uma chamada
//! genérica `<E>(E, …) -> List<E>` (`<K, V>(…) -> Map<K, V>`), com os
//! elementos (inclusive `...`, `if`, `for`) gerando restrições; e a decisão
//! conjunto × mapa para `{}` pelos elementos e pelo contexto.

use super::body::Body;
use super::expr::{self, inferir, infer_free};
use super::BodyInferrer;
use crate::codes::*;
use crate::constraints::GenericInferrer;
use crate::table::{Type, TypeId, TypeParamId};
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{CollectionElement, ExprId, ExprKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Forma {
    Lista,
    Conjunto,
    Mapa,
}

impl<'a> BodyInferrer<'a> {
    /// Três parâmetros de tipo auxiliares (E, K, V) para os literais.
    fn collection_params(&mut self) -> [TypeParamId; 3] {
        let o = self.core.object_nullable;
        if let Some(p) = self.collection_params_cache {
            return p;
        }
        // Os nomes do SDK (`List<E>`/`Set<E>`, `Map<K, V>`): aparecem no
        // `COULD_NOT_INFER` do literal.
        let unnamed = self.table.param(self.core.unknown_param).name;
        let names = ["E", "K", "V"].map(|n| self.interner.lookup(n).unwrap_or(unnamed));
        let mk = |t: &mut crate::table::TypeTable, name| t.alloc_type_param(name, crate::table::TypeParamOwner::GenericFunctionType, o, crate::table::Variance::Unspecified);
        let p = [mk(self.table, names[0]), mk(self.table, names[1]), mk(self.table, names[2])];
        self.collection_params_cache = Some(p);
        p
    }
}

/// Classificação sintática dos elementos de `{...}`: `Some(true)` mapa,
/// `Some(false)` conjunto, `None` indeciso (só espalhamentos ou vazio).
fn shape_from_elements(els: &[CollectionElement]) -> Option<bool> {
    for el in els {
        match el {
            CollectionElement::MapEntry { .. } => return Some(true),
            CollectionElement::Expression(_) | CollectionElement::NullAwareExpression(_) => return Some(false),
            CollectionElement::If { then, else_, .. } => {
                if let Some(f) = shape_from_elements(std::slice::from_ref(then)) {
                    return Some(f);
                }
                if let Some(e) = else_ {
                    if let Some(f) = shape_from_elements(std::slice::from_ref(e)) {
                        return Some(f);
                    }
                }
            }
            CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => {
                if let Some(f) = shape_from_elements(std::slice::from_ref(body)) {
                    return Some(f);
                }
            }
            CollectionElement::Spread { .. } => {}
        }
    }
    None
}

/// Infere um literal de coleção.
pub(crate) fn literal(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, ctx: TypeId) -> TypeId {
    let a = &inf.program.unit(cx.unit).ast;
    // `Some(ambos)`: o literal é ambíguo (`ambos`: o `_BOTH`).
    let mut ambiguous: Option<bool> = None;
    // `{…}` sem argumentos de tipo: depois da visita, os tipos dos
    // elementos decidem a forma de novo (`_inferSetOrMapLiteralType`).
    let mut keys = false;
    let (mut shape, type_args, elements, const_) = match &a.expr(e).kind {
        ExprKind::List { const_, type_args, elements } => (Forma::Lista, type_args, elements, *const_),
        ExprKind::SetOrMap { const_, type_args, elements } => {
            let shape = match type_args.len() {
                1 => Forma::Conjunto,
                2 => Forma::Mapa,
                _ => match shape_from_elements(elements) {
                    Some(true) => {
                        keys = true;
                        Forma::Mapa
                    }
                    Some(false) => {
                        keys = true;
                        Forma::Conjunto
                    }
                    None => {
                        keys = true;
                        let (f, the_amb) = shape_from_context(inf, cx, elements, ctx);
                        ambiguous = the_amb;
                        f
                    }
                },
            };
            (shape, type_args, elements, *const_)
        }
        _ => unreachable!(),
    };
    let elements: &[CollectionElement] = elements;
    // Num literal `const` o contexto vale pelo fecho menor em relação às
    // variáveis de tipo (uma constante não pode depender delas): o padrão
    // `{Iterable<T> m = const []}` de uma classe genérica é
    // `const <Never>[]`, como na VM (o `Router<T>` do angel3; sem isto o
    // literal saía `List<dynamic>` no nativo e o argumento padrão falhava na
    // conferência do parâmetro, docs/NATIVO-PROJETOS-REAIS.md C12).
    let ctx = if const_ && type_args.is_empty() && !inf.the_is_unknown(ctx) {
        let mut ps = Vec::new();
        params_in_type(inf.table, ctx, &mut ps);
        // Só as variáveis de tipo em escopo (classe, método, funções
        // envolventes): as da invocação genérica sendo inferida (o `T` de
        // `Stream.fromIterable(const [1])`) não são variáveis livres do
        // literal — a CFE as tem como `_` no contexto
        // (type_schema_environment.dart `setupGenericTypeInference`).
        let visible = cx.visible_type_parameters();
        ps.retain(|p| visible.values().any(|q| q == p));
        if ps.is_empty() {
            ctx
        } else {
            let mut env = inf.env();
            crate::bounds::least_closure(ctx, &ps, &mut env)
        }
    } else {
        ctx
    };
    let t = if !type_args.is_empty() {
        let args: Vec<TypeId> = type_args.iter().map(|&t| inf.type_argument_type(cx, t)).collect();
        let (ce, ck, cv) = match shape {
            Forma::Mapa => (inf.core.dynamic_, args[0], args.get(1).copied().unwrap_or(inf.core.dynamic_)),
            _ => (args[0], inf.core.dynamic_, inf.core.dynamic_),
        };
        for el in elements {
            visitar(inf, cx, el, shape, [ce, ck, cv], None);
        }
        final_type(inf, shape, &args)
    } else {
        let p = inf.collection_params();
        let params: Vec<TypeParamId> = match shape {
            Forma::Mapa => vec![p[1], p[2]],
            _ => vec![p[0]],
        };
        let vars: Vec<TypeId> = params.iter().map(|&x| inf.table.intern(Type::TypeParameter { param: x, nullable: false })).collect();
        let ret = final_type(inf, shape, &vars);
        let mut gi = GenericInferrer::new(&params);
        gi.generic_metadata = inf.program.library(cx.lib).features.tem(dartforge_frontend::Feature::GenericMetadata);
        if !inf.the_is_unknown(ctx) {
            gi.with_origin(crate::constraints::Origem::Retorno { declarado: ret, contexto: ctx });
            let mut env = inf.env();
            gi.constrain_return(ret, ctx, &mut env);
        }
        let mut env = inf.env();
        let prelim = gi.choose_preliminary(&mut env);
        drop(env);
        let d = inf.core.dynamic_;
        let ctxs = match shape {
            Forma::Mapa => [d, prelim[0], prelim[1]],
            _ => [prelim[0], d, d],
        };
        for el in elements {
            visitar(inf, cx, el, shape, ctxs, Some((&mut gi, &params[..])));
        }
        // `_inferSetOrMapLiteralType` (`typed_literal_resolver.dart:516-597`):
        // depois da visita, os tipos dos elementos decidem antes do contexto
        // — todos admitem conjunto e algum o exige, conjunto; senão, todos
        // admitem mapa e algum o exige, mapa; senão a forma do contexto
        // (`Iterable` sem `Map`, conjunto; `Map` sem `Iterable`, mapa); vazio,
        // mapa; senão o literal é ambíguo (o inferidor sai sem relatar).
        // `{...[1, 2]}` com contexto `Map` é `Set<int>`; `{null, ...o}` com
        // `o` de tipo `Object?` é `AMBIGUOUS_SET_OR_MAP_LITERAL_EITHER`.
        // Contra a forma da visita, o inferidor dela é abandonado e um novo,
        // sem contexto, recebe os tipos dos elementos (`_toSetType`/
        // `_toMapType`, `:740-815`).
        let mut gi = gi;
        if keys && ambiguous.is_none() {
            let infos: Vec<Admits> = elements.iter().map(|el| admits(inf, el, true)).collect();
            let set = infos.iter().all(|a| a.can_set()) && infos.iter().any(|a| a.must_set());
            let map = infos.iter().all(|a| a.can_map()) && infos.iter().any(|a| a.must_map());
            let decided = if set {
                Some(Forma::Conjunto)
            } else if map {
                Some(Forma::Mapa)
            } else if let Some(f) = shape_from_context_type(inf, ctx) {
                Some(f)
            } else if elements.is_empty() {
                Some(Forma::Mapa)
            } else {
                None
            };
            let other = match decided {
                None => {
                    ambiguous = Some(infos.iter().any(|a| a.must_map()) && infos.iter().any(|a| a.must_set()));
                    None
                }
                Some(f) if f != shape => Some(f),
                Some(_) => None,
            };
            if let Some(new_) = other {
                shape = new_;
                let ps: Vec<TypeParamId> = match new_ {
                    Forma::Mapa => vec![p[1], p[2]],
                    _ => vec![p[0]],
                };
                gi = GenericInferrer::new(&ps);
                gi.generic_metadata = inf.program.library(cx.lib).features.tem(dartforge_frontend::Feature::GenericMetadata);
                let d = inf.core.dynamic_;
                for a in &infos {
                    let mut g: Inferrer<'_> = Some((&mut gi, &ps[..]));
                    if new_ == Forma::Mapa {
                        restrict(inf, &mut g, a.chave.unwrap_or(d), 0);
                        restrict(inf, &mut g, a.valor.unwrap_or(d), 1);
                    } else {
                        restrict(inf, &mut g, a.elemento.unwrap_or(d), 0);
                    }
                }
            }
        }
        let (interner, program) = (inf.interner, inf.program);
        let mut env = inf.env();
        let finals = gi.choose_final(&mut env);
        // O `chooseFinalTypes` com o relator e o literal como entidade (o
        // literal ambíguo sai da inferência sem ele).
        let failures = if ambiguous.is_none() { gi.failures(&finals, &mut env, interner, program) } else { Vec::new() };
        drop(env);
        let sp = inf.span_expr(cx.unit, e);
        for (name, suffix) in failures {
            inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::COULD_NOT_INFER, sp, &[&name, &suffix]);
        }
        final_type(inf, shape, &finals)
    };
    if const_ {
        validate_const_collection(inf, cx, e);
    }
    // `_inferSetOrMapLiteralType` (`typed_literal_resolver.dart:579-597`):
    // ambíguo, o literal inteiro relata e fica `dynamic` (sem a verificação
    // dos elementos, que só roda em mapa ou conjunto).
    if let Some(both) = ambiguous {
        let sp = inf.span_expr(cx.unit, e);
        let code = if both {
            dartforge_diagnostics::codigos::compile_time_error::AMBIGUOUS_SET_OR_MAP_LITERAL_BOTH
        } else {
            dartforge_diagnostics::codigos::compile_time_error::AMBIGUOUS_SET_OR_MAP_LITERAL_EITHER
        };
        inf.warning_with_code(code, sp, &[]);
        return inf.core.dynamic_;
    }
    verify_elements(inf, cx, elements, shape, t, const_, false);
    t
}

/// `LiteralElementVerifier` (an611:src/error/literal_element_verifier.dart:52-325),
/// chamado pelo `ErrorVerifier` para todo literal (const ou não) com o tipo
/// estático final: elemento, chave/valor e espalhamento contra o tipo do
/// literal. Num literal `const`, o verificador de constantes já relata os
/// elementos avaliados; aqui só os de dentro de `if`/`for` (ramos que a
/// avaliação pode não visitar), para não duplicar o relato.
/// Objeto (com `call`) num contexto de tipo de função: a conversão implícita
/// de `call` muda o tipo; fica de fora (sem a regra completa, nada relatar).
fn implicit_tearoff(inf: &BodyInferrer<'_>, of: TypeId, for_: TypeId) -> bool {
    matches!(inf.table.get(for_), Type::Function { .. } | Type::FutureOr { .. })
        && matches!(inf.table.get(of), Type::Interface { .. } | Type::ExtensionType { .. } | Type::TypeParameter { .. } | Type::Intersection { .. })
}

fn verify_elements(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    elements: &[CollectionElement],
    shape: Forma,
    t: TypeId,
    const_: bool,
    in_control: bool,
) {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    let args: Vec<TypeId> = match inf.table.get(t) {
        Type::Interface { args, .. } => args.to_vec(),
        _ => return,
    };
    let report = !const_ || in_control;
    for el in elements {
        match el {
            CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) if shape != Forma::Mapa => {
                let Some(e_t) = args.first().copied() else { continue };
                let Some(tx) = inf.body_types.units[cx.unit.0 as usize].get_type(*x) else { continue };
                let tx = if matches!(el, CollectionElement::NullAwareExpression(_)) { inf.non_null(tx) } else { tx };
                if !report || matches!(inf.table.get(tx), Type::Void) || inf.atribuivel(tx, e_t) || implicit_tearoff(inf, tx, e_t) {
                    continue;
                }
                let code = if shape == Forma::Lista { ce::LIST_ELEMENT_TYPE_NOT_ASSIGNABLE } else { ce::SET_ELEMENT_TYPE_NOT_ASSIGNABLE };
                let sp = inf.span_expr(cx.unit, *x);
                inf.warning_with_args(code, sp, &[crate::exibicao::Arg::Tipo(tx), crate::exibicao::Arg::Tipo(e_t)]);
            }
            CollectionElement::MapEntry { key, value, null_aware_key, null_aware_value } if shape == Forma::Mapa => {
                if !report || args.len() < 2 {
                    continue;
                }
                let types = &inf.body_types.units[cx.unit.0 as usize];
                let (Some(tk), Some(tv)) = (types.get_type(*key), types.get_type(*value)) else { continue };
                // `_verifyMapLiteralEntry` (3.13.4 `literal_element_verifier.dart:144-172`):
                // a chave (depois o valor) `void` para a entrada quando tem
                // o `?` ou quando o tipo dela no mapa não é `void` (o
                // `use_of_void_result` sai na inferência); com `<K, void>`,
                // o valor `void` segue para a conferência da chave.
                let void = |t: TypeId| matches!(inf.table.get(t), Type::Void);
                let (void_key, void_value) = (void(tk), void(tv));
                if void_key && (*null_aware_key || !void(args[0])) {
                    continue;
                }
                if void_value && (*null_aware_value || !void(args[1])) {
                    continue;
                }
                let tk = if *null_aware_key { inf.non_null(tk) } else { tk };
                let tv = if *null_aware_value { inf.non_null(tv) } else { tv };
                for (x, tx, tgt, code) in [(*key, tk, args[0], ce::MAP_KEY_TYPE_NOT_ASSIGNABLE), (*value, tv, args[1], ce::MAP_VALUE_TYPE_NOT_ASSIGNABLE)] {
                    if !inf.atribuivel(tx, tgt) && !implicit_tearoff(inf, tx, tgt) {
                        let sp = inf.span_expr(cx.unit, x);
                        inf.warning_with_args(code, sp, &[crate::exibicao::Arg::Tipo(tx), crate::exibicao::Arg::Tipo(tgt)]);
                    }
                }
            }
            CollectionElement::Spread { value, null_aware } => {
                if !report {
                    continue;
                }
                let Some(tx) = inf.body_types.units[cx.unit.0 as usize].get_type(*value) else { continue };
                // `...e` (sem `?`) com `e` de tipo `Null`
                // (`literal_element_verifier.dart:180-200`, `:275-292`): na
                // expressão espalhada.
                // `_verifySpreadForListOrSet`/`_verifySpreadForMap`: subtipo de
                // `Never` não relata; subtipo de `Null` (inclusive o parâmetro
                // de tipo de limite `Null`) só sem `?`; o resto pelo
                // `asInstanceOf` (sem o `?`, parâmetro de tipo pelo limite).
                if inf.e_dynamic(tx) {
                    continue;
                }
                let never = inf.core.never;
                if inf.sub(tx, never) {
                    continue;
                }
                let the_null = inf.core.null;
                if inf.sub(tx, the_null) {
                    if !*null_aware {
                        let sp = inf.span_expr(cx.unit, *value);
                        inf.warning_with_code(ce::NOT_NULL_AWARE_NULL_SPREAD, sp, &[]);
                    }
                    continue;
                }
                let tx = inf.non_null(tx);
                if inf.e_dynamic(tx) || matches!(inf.table.get(tx), Type::Never | Type::Null | Type::Void) {
                    continue;
                }
                let sp = inf.span_expr(cx.unit, *value);
                // `_verifySpreadForListOrSet`/`_verifySpreadForMap`
                // (`literal_element_verifier.dart:176-213`, `:267-303`): o que
                // não é `Iterable`/`Map`.
                let cls = if shape == Forma::Mapa { inf.core.map_class } else { inf.core.iterable_class };
                if inf.as_instance_of(tx, cls).is_none() {
                    let code = if shape == Forma::Mapa { ce::NOT_MAP_SPREAD } else { ce::NOT_ITERABLE_SPREAD };
                    inf.warning_with_code(code, sp, &[]);
                    continue;
                }
                if shape == Forma::Mapa {
                    let Some(m) = inf.as_instance_of(tx, inf.core.map_class) else { continue };
                    if args.len() < 2 || m.len() < 2 {
                        continue;
                    }
                    for (of, for_, code) in [(m[0], args[0], ce::MAP_KEY_TYPE_NOT_ASSIGNABLE), (m[1], args[1], ce::MAP_VALUE_TYPE_NOT_ASSIGNABLE)] {
                        if !inf.atribuivel(of, for_) && !implicit_tearoff(inf, of, for_) {
                            inf.warning_with_args(code, sp, &[crate::exibicao::Arg::Tipo(of), crate::exibicao::Arg::Tipo(for_)]);
                        }
                    }
                } else {
                    let Some(i) = inf.as_instance_of(tx, inf.core.iterable_class) else { continue };
                    let (Some(of), Some(for_)) = (i.first().copied(), args.first().copied()) else { continue };
                    if !inf.atribuivel(of, for_) && !implicit_tearoff(inf, of, for_) {
                        let code = if shape == Forma::Lista { ce::LIST_ELEMENT_TYPE_NOT_ASSIGNABLE } else { ce::SET_ELEMENT_TYPE_NOT_ASSIGNABLE };
                        inf.warning_with_args(code, sp, &[crate::exibicao::Arg::Tipo(of), crate::exibicao::Arg::Tipo(for_)]);
                    }
                }
            }
            CollectionElement::If { then, else_, .. } => {
                verify_elements(inf, cx, std::slice::from_ref(&**then), shape, t, const_, true);
                if let Some(e) = else_ {
                    verify_elements(inf, cx, std::slice::from_ref(&**e), shape, t, const_, true);
                }
            }
            CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => {
                verify_elements(inf, cx, std::slice::from_ref(&**body), shape, t, const_, true);
            }
            // Entrada `k: v` numa lista ou num conjunto (o `forList`/`forSet`
            // do `LiteralElementVerifier`, `literal_element_verifier.dart:93-100`):
            // relatada sempre, const ou não (o verificador de constantes não
            // relata a entrada fora de mapa).
            CollectionElement::MapEntry { key, value, null_aware_key, .. } => {
                if args.len() != 1 {
                    continue;
                }
                let k = inf.span_expr(cx.unit, *key);
                let v = inf.span_expr(cx.unit, *value);
                let start = if *null_aware_key { back_up_question_mark(inf, cx, k.start) } else { k.start };
                inf.warning_with_code(ce::MAP_ENTRY_NOT_IN_MAP, Span { start: start, end: v.end }, &[]);
            }
            // Expressão (ou `?e`) num mapa (`:79-85`, `:107-114`).
            CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => {
                if args.len() != 2 {
                    continue;
                }
                let s = inf.span_expr(cx.unit, *x);
                let start = if matches!(el, CollectionElement::NullAwareExpression(_)) { back_up_question_mark(inf, cx, s.start) } else { s.start };
                inf.warning_with_code(ce::EXPRESSION_IN_MAP, Span { start: start, end: s.end }, &[]);
            }
        }
    }
}

/// O início do elemento null-aware: o `?` antes de `i`, saltando brancos.
fn back_up_question_mark(inf: &BodyInferrer<'_>, cx: &Body, i: usize) -> usize {
    let source = inf.program.unit(cx.unit).source.as_bytes();
    let mut j = i;
    while j > 0 && source[j - 1].is_ascii_whitespace() {
        j -= 1;
    }
    if j > 0 && source[j - 1] == b'?' { j - 1 } else { i }
}

/// As variáveis de tipo que aparecem em `t`.
fn params_in_type(table: &crate::table::TypeTable, t: TypeId, output: &mut Vec<TypeParamId>) {
    match table.get(t) {
        Type::TypeParameter { param, .. } => {
            if !output.contains(param) {
                output.push(*param);
            }
        }
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => {
            for a in args.iter() {
                params_in_type(table, *a, output);
            }
        }
        Type::FutureOr { arg, .. } => params_in_type(table, *arg, output),
        Type::Function { ret, positional, optional, named, .. } => {
            params_in_type(table, *ret, output);
            for a in positional.iter().chain(optional.iter()) {
                params_in_type(table, *a, output);
            }
            for (_, a, _) in named.iter() {
                params_in_type(table, *a, output);
            }
        }
        Type::Record { positional, named, .. } => {
            for a in positional.iter() {
                params_in_type(table, *a, output);
            }
            for (_, a) in named.iter() {
                params_in_type(table, *a, output);
            }
        }
        _ => {}
    }
}

fn final_type(inf: &mut BodyInferrer<'_>, shape: Forma, args: &[TypeId]) -> TypeId {
    match shape {
        Forma::Lista => inf.lista(args[0]),
        Forma::Conjunto => inf.conjunto(args[0]),
        Forma::Mapa => inf.map_type(args[0], args[1]),
    }
}

/// `_InferredCollectionElementTypeInformation`: o que um elemento admite
/// (`Some` = tipo de elemento / chave / valor conhecido).
#[derive(Clone, Copy)]
struct Admits {
    elemento: Option<TypeId>,
    chave: Option<TypeId>,
    valor: Option<TypeId>,
}

impl Admits {
    fn can_map(&self) -> bool {
        self.chave.is_some() || self.valor.is_some()
    }
    fn can_set(&self) -> bool {
        self.elemento.is_some()
    }
    fn must_map(&self) -> bool {
        self.can_map() && self.elemento.is_none()
    }
    fn must_set(&self) -> bool {
        self.can_set() && self.chave.is_none() && self.valor.is_none()
    }
}

/// `_inferCollectionElementType` (`typed_literal_resolver.dart:344-430`) de
/// um elemento de `{…}`: antes da visita (`depois` falso), pelos tipos dos
/// espalhamentos inferidos sem contexto (`espalhamentos_inferidos`); depois
/// dela, pelos de todo elemento visitado (`espalhamentos_visitados`: a
/// expressão, a sem `?` do `?e`, e a chave e o valor da entrada, sem `?` nos
/// `?k`/`?v`).
fn admits(inf: &mut BodyInferrer<'_>, el: &CollectionElement, after: bool) -> Admits {
    let nothing = Admits { elemento: None, chave: None, valor: None };
    let ty = |inf: &BodyInferrer<'_>, x: &dartforge_frontend::ast::ExprId| {
        if after { inf.visited_spreads.get(x).copied() } else { inf.inferred_spreads.get(x).copied() }
    };
    match el {
        CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => Admits { elemento: ty(inf, x), chave: None, valor: None },
        CollectionElement::MapEntry { key, value, .. } => Admits { elemento: None, chave: ty(inf, key), valor: ty(inf, value) },
        CollectionElement::Spread { value, null_aware } => {
            let Some(t) = ty(inf, value) else { return nothing };
            if let Some(a) = inf.as_instance_of(t, inf.core.iterable_class) {
                return Admits { elemento: a.first().copied(), chave: None, valor: None };
            }
            if let Some(a) = inf.as_instance_of(t, inf.core.map_class) {
                return Admits { elemento: None, chave: a.first().copied(), valor: a.get(1).copied() };
            }
            if inf.e_dynamic(t) {
                return Admits { elemento: Some(t), chave: Some(t), valor: Some(t) };
            }
            let n = inf.core.never;
            // `isSubtypeOf(expressionType, nullNone)` (`:413-420`): `X extends
            // Null` também.
            let null = inf.core.null;
            if inf.sub(t, n) || (*null_aware && inf.sub(t, null)) {
                return Admits { elemento: Some(n), chave: Some(n), valor: Some(n) };
            }
            nothing
        }
        CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => admits(inf, body, after),
        CollectionElement::If { then, else_, .. } => {
            let a = admits(inf, then, after);
            let Some(e) = else_ else { return a };
            let b = admits(inf, e, after);
            let dynamic = |x: &Admits, inf: &BodyInferrer<'_>| [x.elemento, x.chave, x.valor].iter().all(|t| t.is_some_and(|t| inf.e_dynamic(t)));
            let or_dynamic = |t: Option<TypeId>, d: TypeId| t.map(|_| d);
            if dynamic(&a, inf) {
                let d = a.elemento.expect("dinâmico");
                return Admits { elemento: or_dynamic(b.elemento, d), chave: or_dynamic(b.chave, d), valor: or_dynamic(b.valor, d) };
            }
            if dynamic(&b, inf) {
                let d = b.elemento.expect("dinâmico");
                return Admits { elemento: or_dynamic(a.elemento, d), chave: or_dynamic(a.chave, d), valor: or_dynamic(a.valor, d) };
            }
            let mut join = |x: Option<TypeId>, y: Option<TypeId>| match (x, y) {
                (None, y) => y,
                (x, None) => x,
                (Some(x), Some(y)) => Some(inf.up(x, y)),
            };
            Admits { elemento: join(a.elemento, b.elemento), chave: join(a.chave, b.chave), valor: join(a.valor, b.valor) }
        }
    }
}

/// `{}` indeciso pela sintaxe: o contexto (`Map`/`Iterable`) decide; senão os
/// espalhamentos, inferidos sem contexto (`_inferSetOrMapLiteralType`,
/// `typed_literal_resolver.dart:516-597`): todos admitem conjunto e algum o
/// exige, conjunto; todos admitem mapa e algum o exige, mapa; vazio, mapa;
/// senão o literal é ambíguo (`Some(mustBeAMap && mustBeASet)` no segundo
/// campo; a visita segue como mapa, e o tipo fica `dynamic`).
fn shape_from_context(inf: &mut BodyInferrer<'_>, cx: &mut Body, els: &[CollectionElement], ctx: TypeId) -> (Forma, Option<bool>) {
    if let Some(f) = shape_from_context_type(inf, ctx) {
        return (f, None);
    }
    if els.is_empty() {
        return (Forma::Mapa, None);
    }
    // Os espalhamentos são inferidos sem contexto; o tipo fica guardado para
    // a visita dos elementos. Só os que não dependem de variáveis do próprio
    // elemento (fora de `for` e de `if-case`) podem ser inferidos antes.
    fn anticipatable(els: &[CollectionElement]) -> bool {
        els.iter().all(|el| match el {
            CollectionElement::If { case_pattern: None, then, else_, .. } => {
                anticipatable(std::slice::from_ref(then)) && else_.as_ref().is_none_or(|e| anticipatable(std::slice::from_ref(e)))
            }
            CollectionElement::If { .. } | CollectionElement::For { .. } | CollectionElement::ForIn { .. } => false,
            _ => true,
        })
    }
    fn infer_spreads(inf: &mut BodyInferrer<'_>, cx: &mut Body, els: &[CollectionElement]) {
        let u = inf.core.unknown;
        for el in els {
            match el {
                CollectionElement::Spread { value, .. } => {
                    let t = inferir(inf, cx, *value, u);
                    inf.inferred_spreads.insert(*value, t);
                }
                CollectionElement::If { then, else_, .. } => {
                    infer_spreads(inf, cx, std::slice::from_ref(then));
                    if let Some(e) = else_ {
                        infer_spreads(inf, cx, std::slice::from_ref(e));
                    }
                }
                _ => {}
            }
        }
    }
    if !anticipatable(els) {
        // Espalhamento dentro de `for`/`if-case`: o primeiro de topo que é
        // `Iterable` ou `Map` decide, sem relato de ambiguidade.
        let u = inf.core.unknown;
        let mut shape = None;
        for el in els {
            if let CollectionElement::Spread { value, .. } = el {
                let t = inferir(inf, cx, *value, u);
                inf.inferred_spreads.insert(*value, t);
                let t = inf.non_null(t);
                if shape.is_none() {
                    if inf.as_instance_of(t, inf.core.iterable_class).is_some() {
                        shape = Some(Forma::Conjunto);
                    } else if inf.as_instance_of(t, inf.core.map_class).is_some() {
                        shape = Some(Forma::Mapa);
                    }
                }
            }
        }
        return (shape.unwrap_or(Forma::Mapa), None);
    }
    infer_spreads(inf, cx, els);
    let infos: Vec<Admits> = els.iter().map(|el| admits(inf, el, false)).collect();
    let can_set = infos.iter().all(|a| a.can_set());
    let must_set = infos.iter().any(|a| a.must_set());
    let can_map = infos.iter().all(|a| a.can_map());
    let must_map = infos.iter().any(|a| a.must_map());
    if can_set && must_set {
        return (Forma::Conjunto, None);
    }
    if can_map && must_map {
        return (Forma::Mapa, None);
    }
    (Forma::Mapa, Some(must_map && must_set))
}

/// A forma que o contexto (`Map`/`Iterable`) impõe, se impõe.
fn shape_from_context_type(inf: &mut BodyInferrer<'_>, ctx: TypeId) -> Option<Forma> {
    if !inf.the_is_unknown(ctx) {
        let k = inf.greatest_closure_of(ctx);
        let k = inf.non_null(k);
        let is_map = inf.as_instance_of(k, inf.core.map_class).is_some() || matches!(inf.table.get(k), Type::Interface { class, .. } if Some(*class) == inf.core.map_class);
        let e_iter = inf.as_instance_of(k, inf.core.iterable_class).is_some();
        if e_iter && !is_map {
            return Some(Forma::Conjunto);
        }
        if is_map && !e_iter {
            return Some(Forma::Mapa);
        }
        if let Type::FutureOr { arg, .. } = inf.table.get(k).clone() {
            if inf.as_instance_of(arg, inf.core.iterable_class).is_some() {
                return Some(Forma::Conjunto);
            }
        }
    }
    None
}

type Inferrer<'g> = Option<(&'g mut GenericInferrer, &'g [TypeParamId])>;

fn restrict(inf: &mut BodyInferrer<'_>, gi: &mut Inferrer<'_>, t: TypeId, i: usize) {
    if let Some((g, params)) = gi {
        let v = inf.table.intern(Type::TypeParameter { param: params[i], nullable: false });
        // `_toListType`/`_toSetType`/`_toMapType`
        // (`typed_literal_resolver.dart:460-490`, `:740-815`): parâmetros
        // sintéticos `element`, `key` e `value`, sem `genericClass` (a origem
        // é `Parameter '…'`).
        let parameter = if params.len() == 2 { ["key", "value"][i] } else { "element" };
        g.with_origin(crate::constraints::Origem::Argumento { parametro: parameter.to_string(), declarado: v, argumento: t, prefixo: None });
        let mut env = inf.env();
        g.constrain_argument(t, v, &mut env);
    }
}

/// Visita um elemento, gerando restrições. `ctxs = [E, K, V]`.
fn visitar(inf: &mut BodyInferrer<'_>, cx: &mut Body, el: &CollectionElement, shape: Forma, ctxs: [TypeId; 3], mut gi: Inferrer<'_>) {
    let u = inf.core.unknown;
    match el {
        CollectionElement::Expression(x) => {
            let c = if shape == Forma::Mapa { u } else { ctxs[0] };
            let t = inferir(inf, cx, *x, c);
            // Com argumentos de tipo escritos (sem inferência) e elemento
            // não `void`: valor `void` é `use_of_void_result`
            // (`LiteralElementVerifier._verifyElement`, só em lista e
            // conjunto; no mapa a expressão é `EXPRESSION_IN_MAP`).
            if gi.is_none() && shape != Forma::Mapa && !matches!(inf.table.get(c), Type::Void) {
                super::expr::void_use(inf, cx, *x, t);
            }
            inf.visited_spreads.insert(*x, t);
            if shape != Forma::Mapa {
                restrict(inf, &mut gi, t, 0);
            }
        }
        CollectionElement::NullAwareExpression(x) => {
            let c = if shape == Forma::Mapa || inf.the_is_unknown(ctxs[0]) { u } else { inf.anulavel(ctxs[0]) };
            let t = inferir(inf, cx, *x, c);
            // O `void` do valor antes da promoção (`:107-112`).
            if gi.is_none() && shape != Forma::Mapa && !matches!(inf.table.get(ctxs[0]), Type::Void) {
                super::expr::void_use(inf, cx, *x, t);
            }
            let t = inf.non_null(t);
            inf.visited_spreads.insert(*x, t);
            if shape != Forma::Mapa {
                restrict(inf, &mut gi, t, 0);
            }
        }
        CollectionElement::MapEntry { key, value, null_aware_key, null_aware_value } => {
            let ck = if *null_aware_key && !inf.the_is_unknown(ctxs[1]) { inf.anulavel(ctxs[1]) } else { ctxs[1] };
            let cv = if *null_aware_value && !inf.the_is_unknown(ctxs[2]) { inf.anulavel(ctxs[2]) } else { ctxs[2] };
            let tk = inferir(inf, cx, *key, ck);
            let tv = inferir(inf, cx, *value, cv);
            // `_verifyMapLiteralEntry` (`literal_element_verifier.dart:130-141`):
            // a chave `void` encerra a entrada, sem olhar o valor.
            if gi.is_none() {
                let void_key = !matches!(inf.table.get(ck), Type::Void) && super::expr::void_use(inf, cx, *key, tk);
                if !void_key && !matches!(inf.table.get(cv), Type::Void) {
                    super::expr::void_use(inf, cx, *value, tv);
                }
            }
            let tk = if *null_aware_key { inf.non_null(tk) } else { tk };
            let tv = if *null_aware_value { inf.non_null(tv) } else { tv };
            inf.visited_spreads.insert(*key, tk);
            inf.visited_spreads.insert(*value, tv);
            restrict(inf, &mut gi, tk, 0);
            restrict(inf, &mut gi, tv, 1);
        }
        CollectionElement::Spread { value, null_aware } => {
            let c = match shape {
                Forma::Mapa => inf.map_type(ctxs[1], ctxs[2]),
                _ => inf.iterable(ctxs[0]),
            };
            let c = if *null_aware { inf.anulavel(c) } else { c };
            let t = match inf.inferred_spreads.remove(value) {
                Some(t) => t,
                None => inferir(inf, cx, *value, c),
            };
            inf.visited_spreads.insert(*value, t);
            if *null_aware {
                super::expr::unnecessary_null_spread(inf, cx, *value, t);
            } else {
                // `ResolverVisitor.visitSpreadElement` (`resolver.dart:3673-3692`).
                super::expr::nullable_dereference(inf, cx, *value, t, dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_USE_OF_NULLABLE_VALUE_IN_SPREAD);
            }
            // `_computeElementType` / `_inferCollectionElementType`
            // (`typed_literal_resolver.dart:203-226, 380-425`), pelo tipo da
            // expressão: a instância de `Iterable`/`Map`; `dynamic`;
            // subtipo de `Never` → `Never`; subtipo de `Null` → `Never` com
            // `...?` (sem, `dynamic` na lista e nada no mapa); o resto,
            // `dynamic` na lista.
            let t0 = t;
            let t = inf.non_null(t);
            let (never, null) = (inf.core.never, inf.core.null);
            let below_never = inf.sub(t0, never);
            let below_null = !below_never && inf.sub(t0, null);
            let one = |inf: &mut BodyInferrer<'_>, gi: &mut _, x: TypeId| {
                restrict(inf, gi, x, 0);
                if shape == Forma::Mapa {
                    restrict(inf, gi, x, 1);
                }
            };
            if inf.e_dynamic(t) {
                let d = inf.core.dynamic_;
                one(inf, &mut gi, d);
            } else if below_never || (below_null && *null_aware) {
                one(inf, &mut gi, never);
            } else if below_null {
                if shape != Forma::Mapa {
                    let d = inf.core.dynamic_;
                    restrict(inf, &mut gi, d, 0);
                }
            } else if shape == Forma::Mapa {
                if let Some(args) = inf.as_instance_of(t, inf.core.map_class) {
                    restrict(inf, &mut gi, args[0], 0);
                    restrict(inf, &mut gi, args[1], 1);
                }
            } else if let Some(args) = inf.as_instance_of(t, inf.core.iterable_class) {
                restrict(inf, &mut gi, args[0], 0);
            } else {
                let d = inf.core.dynamic_;
                restrict(inf, &mut gi, d, 0);
            }
        }
        CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
            let before = cx.flow.clone();
            let (vf, ff) = match case_pattern {
                Some(p) => {
                    let t = infer_free(inf, cx, *condition);
                    cx.push_scope();
                    let r = super::patterns::caso(inf, cx, *p, t, *guard, Some(*condition));
                    r
                }
                None => expr::checked_condition(inf, cx, *condition),
            };
            cx.flow = vf;
            // `visitIfElement`: cada ramo fecha um bloco básico
            // (`nullSafetyDeadCodeVerifier.flowEnd(ifTrue/ifFalse)`).
            let open = enter_branch(inf, cx, then);
            reborrow_visit(inf, cx, then, shape, ctxs, &mut gi);
            exit_branch(cx, open);
            if case_pattern.is_some() {
                cx.pop_scope();
            }
            let after_then = std::mem::replace(&mut cx.flow, ff);
            if let Some(e) = else_ {
                let open = enter_branch(inf, cx, e);
                reborrow_visit(inf, cx, e, shape, ctxs, &mut gi);
                exit_branch(cx, open);
            }
            let after_else = std::mem::replace(&mut cx.flow, before);
            cx.flow = inf.juntar(&after_then, &after_else);
        }
        CollectionElement::For { init, condition, updates, body, .. } => {
            cx.push_scope();
            if let Some(i) = init {
                super::statements::for_initialization(inf, cx, i);
            }
            // Inicialização que não completa (`[for (var (i) = throw 0; c; u) e]`),
            // como no comando `for`: o primeiro nó morto é a condição, e o
            // `flowEnd` que o encerra (o do corpo da função) vai dele ao fim
            // da última atualização (o pai é o `ForParts`); o resto do
            // elemento fica no mesmo trecho.
            let dead_at_start = !cx.flow.alcancavel && cx.dead_code.is_none();
            if dead_at_start {
                let a = &inf.program.unit(cx.unit).ast;
                if let (Some(c), Some(u)) = (condition, updates.last()) {
                    let sp = dartforge_diagnostics::Span { start: a.expr(*c).span.start, end: a.expr(*u).span.end };
                    inf.aviso(crate::codes::DEAD_CODE.template.to_string(), sp);
                }
                cx.dead_code = Some(usize::MAX);
            }
            // `for_conditionBegin`: o que o laço escreve — a condição, as
            // atualizações e o corpo (o `AssignedVariables` do nó inteiro,
            // como no comando `for`) — perde a promoção e deixa de estar
            // definitivamente não atribuído (junção conservadora do laço).
            let mut parts = vec![super::statements::Parte::Elemento(body)];
            if let Some(c) = condition {
                parts.push(super::statements::Parte::Expr(*c));
            }
            for u2 in updates.iter() {
                parts.push(super::statements::Parte::Expr(*u2));
            }
            let (writes, captured) = super::statements::writes_in(inf, cx, &parts);
            cx.flow.conservative_join(&writes, &captured);
            let before = cx.flow.clone();
            if let Some(c) = condition {
                let (vf, _ff) = expr::checked_condition(inf, cx, *c);
                cx.flow = vf;
            }
            reborrow_visit(inf, cx, body, shape, ctxs, &mut gi);
            for u2 in updates.iter() {
                infer_free(inf, cx, *u2);
            }
            cx.flow = before;
            if dead_at_start {
                cx.dead_code = if condition.is_some() && updates.is_empty() { Some(0) } else { None };
            }
            cx.pop_scope();
        }
        CollectionElement::ForIn { await_, target, iterable, body } => {
            cx.push_scope();
            let write = super::statements::for_in_header(inf, cx, target, *iterable, *await_);
            // `forEach_bodyBegin`: o que o corpo escreve, como no comando
            // `for-in` (junção conservadora do laço); o estado daí entra no
            // `forEach_end`, juntado ao do fim do corpo.
            let (writes, captured) = super::statements::writes_in(inf, cx, &[super::statements::Parte::Elemento(body)]);
            cx.flow.conservative_join(&writes, &captured);
            let before = cx.flow.clone();
            if let Some(x) = write {
                x.aplicar(inf, cx);
            }
            reborrow_visit(inf, cx, body, shape, ctxs, &mut gi);
            let after = std::mem::replace(&mut cx.flow, before);
            let before = cx.flow.clone();
            cx.flow = inf.juntar(&after, &before);
            cx.pop_scope();
        }
    }
}

fn reborrow_visit(inf: &mut BodyInferrer<'_>, cx: &mut Body, el: &CollectionElement, shape: Forma, ctxs: [TypeId; 3], gi: &mut Inferrer<'_>) {
    let sub: Inferrer<'_> = match gi {
        Some((g, p)) => Some((&mut **g, *p)),
        None => None,
    };
    visitar(inf, cx, el, shape, ctxs, sub);
}

/// Diagnósticos de coleções `const` (chaves/elementos repetidos, não constantes).
pub(crate) fn validate_const_collection(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) {
    let unit = cx.unit;
    let a = &inf.program.unit(unit).ast;
    let mut subs = Vec::new();
    {
        let mut av = crate::constant::ConstantEvaluator::new(inf.program, inf.interner, inf.table, inf.core);
        let mut diags: Vec<(String, dartforge_diagnostics::Span)> = Vec::new();
        match &a.expr(e).kind {
            ExprKind::SetOrMap { elements, .. } => {
                let is_map = elements.iter().any(|el| matches!(el, CollectionElement::MapEntry { .. }));
                let mut seen = Vec::new();
                for el in elements.iter() {
                    match el {
                        CollectionElement::MapEntry { key, value, .. } if is_map => {
                            match av.evaluate_expr(unit, *key) {
                                Some(v) => {
                                    if seen.contains(&v) {
                                        diags.push((EQUAL_KEYS_IN_CONST_MAP.template.to_string(), a.expr(*key).span));
                                    } else {
                                        seen.push(v);
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
                        CollectionElement::Expression(x) if !is_map => {
                            match av.evaluate_expr(unit, *x) {
                                Some(v) => {
                                    if seen.contains(&v) {
                                        diags.push((EQUAL_ELEMENTS_IN_CONST_SET.template.to_string(), a.expr(*x).span));
                                    } else {
                                        seen.push(v);
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
            // O avaliador não sabe tudo (constantes referidas, operadores de
            // tipos do SDK): "não constante" só quando a expressão de fato
            // não é constante (especificação, "Constants").
            if m == CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE.template {
                let tgt = a.exprs.iter().position(|x| x.span == s).map(|i| ExprId(i as u32));
                if tgt.is_some_and(|x| inf.is_constant(cx, x)) {
                    continue;
                }
            }
            inf.aviso(m, s);
        }
    }
    for s in subs {
        validate_const_collection(inf, cx, s);
    }
}

/// A faixa de um elemento de coleção (o nó do `flowEnd`).
fn element_range(inf: &BodyInferrer<'_>, cx: &Body, el: &CollectionElement) -> dartforge_diagnostics::Span {
    let a = &inf.program.unit(cx.unit).ast;
    let source = inf.program.unit(cx.unit).source.as_str();
    let before = |start: usize, keyword: &str| -> usize {
        source.get(..start).and_then(|s| s.rfind(keyword)).unwrap_or(start)
    };
    match el {
        CollectionElement::Expression(e) => a.expr(*e).span,
        CollectionElement::NullAwareExpression(e) => {
            let s = a.expr(*e).span;
            dartforge_diagnostics::Span { start: before(s.start, "?"), end: s.end }
        }
        CollectionElement::MapEntry { key, value, .. } => dartforge_diagnostics::Span { start: a.expr(*key).span.start, end: a.expr(*value).span.end },
        CollectionElement::Spread { value, .. } => {
            let s = a.expr(*value).span;
            dartforge_diagnostics::Span { start: before(s.start, "..."), end: s.end }
        }
        CollectionElement::If { condition, then, else_, .. } => {
            let end = element_range(inf, cx, else_.as_deref().unwrap_or(then)).end;
            dartforge_diagnostics::Span { start: before(a.expr(*condition).span.start, "if"), end: end }
        }
        CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => {
            let b = element_range(inf, cx, body);
            let begin = source.get(..b.start).and_then(|s| s.rfind("for")).unwrap_or(b.start);
            dartforge_diagnostics::Span { start: begin, end: b.end }
        }
    }
}

/// Entra no ramo de um `if` de coleção como bloco básico: se ele começa
/// inalcançável e não há trecho aberto, o trecho é o ramo inteiro.
fn enter_branch(inf: &mut BodyInferrer<'_>, cx: &mut Body, el: &CollectionElement) -> bool {
    let sp = element_range(inf, cx, el);
    let open = !cx.flow.alcancavel && cx.dead_code.is_none();
    if open {
        inf.aviso(crate::codes::DEAD_CODE.template.to_string(), sp);
    }
    super::statements::enter_flow(cx, sp.end);
    if open {
        cx.dead_code = Some(cx.flow_ends.len());
    }
    open
}

fn exit_branch(cx: &mut Body, _open: bool) {
    super::statements::exit_flow(cx);
}
