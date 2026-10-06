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
    // `Some(ambos)`: o literal é ambíguo (`ambos`: o `_BOTH`).
    let mut ambigua: Option<bool> = None;
    let (forma, type_args, elements, const_) = match &a.expr(e).kind {
        ExprKind::List { const_, type_args, elements } => (Forma::Lista, type_args, elements, *const_),
        ExprKind::SetOrMap { const_, type_args, elements } => {
            let forma = match type_args.len() {
                1 => Forma::Conjunto,
                2 => Forma::Mapa,
                _ => match forma_pelos_elementos(elements) {
                    Some(true) => Forma::Mapa,
                    Some(false) => Forma::Conjunto,
                    None => {
                        let (f, amb) = forma_pelo_contexto(inf, cx, elements, ctx);
                        ambigua = amb;
                        f
                    }
                },
            };
            (forma, type_args, elements, *const_)
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
    let ctx = if const_ && type_args.is_empty() && !inf.e_desconhecido(ctx) {
        let mut ps = Vec::new();
        params_no_tipo(inf.table, ctx, &mut ps);
        // Só as variáveis de tipo em escopo (classe, método, funções
        // envolventes): as da invocação genérica sendo inferida (o `T` de
        // `Stream.fromIterable(const [1])`) não são variáveis livres do
        // literal — a CFE as tem como `_` no contexto
        // (type_schema_environment.dart `setupGenericTypeInference`).
        let visiveis = cx.parametros_de_tipo_visiveis();
        ps.retain(|p| visiveis.values().any(|q| q == p));
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
        let args: Vec<TypeId> = type_args.iter().map(|&t| inf.tipo_de_argumento_de_tipo(cx, t)).collect();
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
        validar_colecao_const(inf, cx, e);
    }
    // `_inferSetOrMapLiteralType` (`typed_literal_resolver.dart:579-597`):
    // ambíguo, o literal inteiro relata e fica `dynamic` (sem a verificação
    // dos elementos, que só roda em mapa ou conjunto).
    if let Some(ambos) = ambigua {
        let sp = inf.span_expr(cx.unit, e);
        let codigo = if ambos {
            dartforge_diagnostics::codigos::compile_time_error::AMBIGUOUS_SET_OR_MAP_LITERAL_BOTH
        } else {
            dartforge_diagnostics::codigos::compile_time_error::AMBIGUOUS_SET_OR_MAP_LITERAL_EITHER
        };
        inf.aviso_com_codigo(codigo, sp, &[]);
        return inf.core.dynamic_;
    }
    verificar_elementos(inf, cx, elements, forma, t, const_, false);
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
fn tearoff_implicita(inf: &BodyInferrer<'_>, de: TypeId, para: TypeId) -> bool {
    matches!(inf.table.get(para), Type::Function { .. } | Type::FutureOr { .. })
        && matches!(inf.table.get(de), Type::Interface { .. } | Type::ExtensionType { .. } | Type::TypeParameter { .. } | Type::Intersection { .. })
}

fn verificar_elementos(
    inf: &mut BodyInferrer<'_>,
    cx: &Corpo,
    elements: &[CollectionElement],
    forma: Forma,
    t: TypeId,
    const_: bool,
    em_controle: bool,
) {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    let args: Vec<TypeId> = match inf.table.get(t) {
        Type::Interface { args, .. } => args.to_vec(),
        _ => return,
    };
    let relatar = !const_ || em_controle;
    for el in elements {
        match el {
            CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) if forma != Forma::Mapa => {
                let Some(e_t) = args.first().copied() else { continue };
                let Some(tx) = inf.body_types.units[cx.unit.0 as usize].get_type(*x) else { continue };
                let tx = if matches!(el, CollectionElement::NullAwareExpression(_)) { inf.nao_nulo(tx) } else { tx };
                if !relatar || matches!(inf.table.get(tx), Type::Void) || inf.atribuivel(tx, e_t) || tearoff_implicita(inf, tx, e_t) {
                    continue;
                }
                let codigo = if forma == Forma::Lista { ce::LIST_ELEMENT_TYPE_NOT_ASSIGNABLE } else { ce::SET_ELEMENT_TYPE_NOT_ASSIGNABLE };
                let sp = inf.span_expr(cx.unit, *x);
                inf.aviso_com_args(codigo, sp, &[crate::exibicao::Arg::Tipo(tx), crate::exibicao::Arg::Tipo(e_t)]);
            }
            CollectionElement::MapEntry { key, value, null_aware_key, null_aware_value } if forma == Forma::Mapa => {
                if !relatar || args.len() < 2 {
                    continue;
                }
                let tipos = &inf.body_types.units[cx.unit.0 as usize];
                let (Some(tk), Some(tv)) = (tipos.get_type(*key), tipos.get_type(*value)) else { continue };
                // Chave ou valor `void`: `use_of_void_result`, e para.
                if matches!(inf.table.get(tk), Type::Void) || matches!(inf.table.get(tv), Type::Void) {
                    continue;
                }
                let tk = if *null_aware_key { inf.nao_nulo(tk) } else { tk };
                let tv = if *null_aware_value { inf.nao_nulo(tv) } else { tv };
                for (x, tx, alvo, codigo) in [(*key, tk, args[0], ce::MAP_KEY_TYPE_NOT_ASSIGNABLE), (*value, tv, args[1], ce::MAP_VALUE_TYPE_NOT_ASSIGNABLE)] {
                    if !inf.atribuivel(tx, alvo) && !tearoff_implicita(inf, tx, alvo) {
                        let sp = inf.span_expr(cx.unit, x);
                        inf.aviso_com_args(codigo, sp, &[crate::exibicao::Arg::Tipo(tx), crate::exibicao::Arg::Tipo(alvo)]);
                    }
                }
            }
            CollectionElement::Spread { value, null_aware } => {
                if !relatar {
                    continue;
                }
                let Some(tx) = inf.body_types.units[cx.unit.0 as usize].get_type(*value) else { continue };
                // `...e` (sem `?`) com `e` de tipo `Null`
                // (`literal_element_verifier.dart:180-200`, `:275-292`): na
                // expressão espalhada.
                if !*null_aware && matches!(inf.table.get(tx), Type::Null) {
                    let sp = inf.span_expr(cx.unit, *value);
                    inf.aviso_com_codigo(ce::NOT_NULL_AWARE_NULL_SPREAD, sp, &[]);
                }
                let tx = inf.nao_nulo(tx);
                if inf.e_dynamic(tx) || matches!(inf.table.get(tx), Type::Never | Type::Null | Type::Void) {
                    continue;
                }
                let sp = inf.span_expr(cx.unit, *value);
                // `_verifySpreadForListOrSet`/`_verifySpreadForMap`
                // (`literal_element_verifier.dart:176-213`, `:267-303`): o que
                // não é `Iterable`/`Map`.
                let classe = if forma == Forma::Mapa { inf.core.map_class } else { inf.core.iterable_class };
                if inf.como_instancia_de(tx, classe).is_none() {
                    let codigo = if forma == Forma::Mapa { ce::NOT_MAP_SPREAD } else { ce::NOT_ITERABLE_SPREAD };
                    inf.aviso_com_codigo(codigo, sp, &[]);
                    continue;
                }
                if forma == Forma::Mapa {
                    let Some(m) = inf.como_instancia_de(tx, inf.core.map_class) else { continue };
                    if args.len() < 2 || m.len() < 2 {
                        continue;
                    }
                    for (de, para, codigo) in [(m[0], args[0], ce::MAP_KEY_TYPE_NOT_ASSIGNABLE), (m[1], args[1], ce::MAP_VALUE_TYPE_NOT_ASSIGNABLE)] {
                        if !inf.atribuivel(de, para) && !tearoff_implicita(inf, de, para) {
                            inf.aviso_com_args(codigo, sp, &[crate::exibicao::Arg::Tipo(de), crate::exibicao::Arg::Tipo(para)]);
                        }
                    }
                } else {
                    let Some(i) = inf.como_instancia_de(tx, inf.core.iterable_class) else { continue };
                    let (Some(de), Some(para)) = (i.first().copied(), args.first().copied()) else { continue };
                    if !inf.atribuivel(de, para) && !tearoff_implicita(inf, de, para) {
                        let codigo = if forma == Forma::Lista { ce::LIST_ELEMENT_TYPE_NOT_ASSIGNABLE } else { ce::SET_ELEMENT_TYPE_NOT_ASSIGNABLE };
                        inf.aviso_com_args(codigo, sp, &[crate::exibicao::Arg::Tipo(de), crate::exibicao::Arg::Tipo(para)]);
                    }
                }
            }
            CollectionElement::If { then, else_, .. } => {
                verificar_elementos(inf, cx, std::slice::from_ref(&**then), forma, t, const_, true);
                if let Some(e) = else_ {
                    verificar_elementos(inf, cx, std::slice::from_ref(&**e), forma, t, const_, true);
                }
            }
            CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => {
                verificar_elementos(inf, cx, std::slice::from_ref(&**body), forma, t, const_, true);
            }
            _ => {}
        }
    }
}

/// As variáveis de tipo que aparecem em `t`.
fn params_no_tipo(table: &crate::table::TypeTable, t: TypeId, saida: &mut Vec<TypeParamId>) {
    match table.get(t) {
        Type::TypeParameter { param, .. } => {
            if !saida.contains(param) {
                saida.push(*param);
            }
        }
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => {
            for a in args.iter() {
                params_no_tipo(table, *a, saida);
            }
        }
        Type::FutureOr { arg, .. } => params_no_tipo(table, *arg, saida),
        Type::Function { ret, positional, optional, named, .. } => {
            params_no_tipo(table, *ret, saida);
            for a in positional.iter().chain(optional.iter()) {
                params_no_tipo(table, *a, saida);
            }
            for (_, a, _) in named.iter() {
                params_no_tipo(table, *a, saida);
            }
        }
        Type::Record { positional, named, .. } => {
            for a in positional.iter() {
                params_no_tipo(table, *a, saida);
            }
            for (_, a) in named.iter() {
                params_no_tipo(table, *a, saida);
            }
        }
        _ => {}
    }
}

fn tipo_final(inf: &mut BodyInferrer<'_>, forma: Forma, args: &[TypeId]) -> TypeId {
    match forma {
        Forma::Lista => inf.lista(args[0]),
        Forma::Conjunto => inf.conjunto(args[0]),
        Forma::Mapa => inf.tipo_mapa(args[0], args[1]),
    }
}

/// `_InferredCollectionElementTypeInformation`: o que um elemento admite
/// (`Some` = tipo de elemento / chave / valor conhecido).
#[derive(Clone, Copy)]
struct Admite {
    elemento: Option<TypeId>,
    chave: Option<TypeId>,
    valor: Option<TypeId>,
}

impl Admite {
    fn pode_mapa(&self) -> bool {
        self.chave.is_some() || self.valor.is_some()
    }
    fn pode_conjunto(&self) -> bool {
        self.elemento.is_some()
    }
    fn deve_mapa(&self) -> bool {
        self.pode_mapa() && self.elemento.is_none()
    }
    fn deve_conjunto(&self) -> bool {
        self.pode_conjunto() && self.chave.is_none() && self.valor.is_none()
    }
}

/// `_inferCollectionElementType` (`typed_literal_resolver.dart:344-430`) de
/// um literal `{…}` só de espalhamentos (os tipos deles já inferidos sem
/// contexto, em `espalhamentos_inferidos`).
fn admite(inf: &mut BodyInferrer<'_>, el: &CollectionElement) -> Admite {
    let nada = Admite { elemento: None, chave: None, valor: None };
    match el {
        CollectionElement::Spread { value, null_aware } => {
            let Some(&t) = inf.espalhamentos_inferidos.get(value) else { return nada };
            if let Some(a) = inf.como_instancia_de(t, inf.core.iterable_class) {
                return Admite { elemento: a.first().copied(), chave: None, valor: None };
            }
            if let Some(a) = inf.como_instancia_de(t, inf.core.map_class) {
                return Admite { elemento: None, chave: a.first().copied(), valor: a.get(1).copied() };
            }
            if inf.e_dynamic(t) {
                return Admite { elemento: Some(t), chave: Some(t), valor: Some(t) };
            }
            let n = inf.core.never;
            if inf.sub(t, n) || (*null_aware && matches!(inf.table.get(t), Type::Null)) {
                return Admite { elemento: Some(n), chave: Some(n), valor: Some(n) };
            }
            nada
        }
        CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => admite(inf, body),
        CollectionElement::If { then, else_, .. } => {
            let a = admite(inf, then);
            let Some(e) = else_ else { return a };
            let b = admite(inf, e);
            let dinamico = |x: &Admite, inf: &BodyInferrer<'_>| [x.elemento, x.chave, x.valor].iter().all(|t| t.is_some_and(|t| inf.e_dynamic(t)));
            let ou_dinamico = |t: Option<TypeId>, d: TypeId| t.map(|_| d);
            if dinamico(&a, inf) {
                let d = a.elemento.expect("dinâmico");
                return Admite { elemento: ou_dinamico(b.elemento, d), chave: ou_dinamico(b.chave, d), valor: ou_dinamico(b.valor, d) };
            }
            if dinamico(&b, inf) {
                let d = b.elemento.expect("dinâmico");
                return Admite { elemento: ou_dinamico(a.elemento, d), chave: ou_dinamico(a.chave, d), valor: ou_dinamico(a.valor, d) };
            }
            let mut juntar = |x: Option<TypeId>, y: Option<TypeId>| match (x, y) {
                (None, y) => y,
                (x, None) => x,
                (Some(x), Some(y)) => Some(inf.up(x, y)),
            };
            Admite { elemento: juntar(a.elemento, b.elemento), chave: juntar(a.chave, b.chave), valor: juntar(a.valor, b.valor) }
        }
        _ => nada,
    }
}

/// `{}` indeciso pela sintaxe: o contexto (`Map`/`Iterable`) decide; senão os
/// espalhamentos, inferidos sem contexto (`_inferSetOrMapLiteralType`,
/// `typed_literal_resolver.dart:516-597`): todos admitem conjunto e algum o
/// exige, conjunto; todos admitem mapa e algum o exige, mapa; vazio, mapa;
/// senão o literal é ambíguo (`Some(mustBeAMap && mustBeASet)` no segundo
/// campo; a visita segue como mapa, e o tipo fica `dynamic`).
fn forma_pelo_contexto(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, els: &[CollectionElement], ctx: TypeId) -> (Forma, Option<bool>) {
    if let Some(f) = forma_pelo_tipo_do_contexto(inf, ctx) {
        return (f, None);
    }
    if els.is_empty() {
        return (Forma::Mapa, None);
    }
    // Os espalhamentos são inferidos sem contexto; o tipo fica guardado para
    // a visita dos elementos. Só os que não dependem de variáveis do próprio
    // elemento (fora de `for` e de `if-case`) podem ser inferidos antes.
    fn antecipaveis(els: &[CollectionElement]) -> bool {
        els.iter().all(|el| match el {
            CollectionElement::If { case_pattern: None, then, else_, .. } => {
                antecipaveis(std::slice::from_ref(then)) && else_.as_ref().is_none_or(|e| antecipaveis(std::slice::from_ref(e)))
            }
            CollectionElement::If { .. } | CollectionElement::For { .. } | CollectionElement::ForIn { .. } => false,
            _ => true,
        })
    }
    fn inferir_espalhamentos(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, els: &[CollectionElement]) {
        let u = inf.core.unknown;
        for el in els {
            match el {
                CollectionElement::Spread { value, .. } => {
                    let t = inferir(inf, cx, *value, u);
                    inf.espalhamentos_inferidos.insert(*value, t);
                }
                CollectionElement::If { then, else_, .. } => {
                    inferir_espalhamentos(inf, cx, std::slice::from_ref(then));
                    if let Some(e) = else_ {
                        inferir_espalhamentos(inf, cx, std::slice::from_ref(e));
                    }
                }
                _ => {}
            }
        }
    }
    if !antecipaveis(els) {
        // Espalhamento dentro de `for`/`if-case`: o primeiro de topo que é
        // `Iterable` ou `Map` decide, sem relato de ambiguidade.
        let u = inf.core.unknown;
        let mut forma = None;
        for el in els {
            if let CollectionElement::Spread { value, .. } = el {
                let t = inferir(inf, cx, *value, u);
                inf.espalhamentos_inferidos.insert(*value, t);
                let t = inf.nao_nulo(t);
                if forma.is_none() {
                    if inf.como_instancia_de(t, inf.core.iterable_class).is_some() {
                        forma = Some(Forma::Conjunto);
                    } else if inf.como_instancia_de(t, inf.core.map_class).is_some() {
                        forma = Some(Forma::Mapa);
                    }
                }
            }
        }
        return (forma.unwrap_or(Forma::Mapa), None);
    }
    inferir_espalhamentos(inf, cx, els);
    let infos: Vec<Admite> = els.iter().map(|el| admite(inf, el)).collect();
    let pode_conjunto = infos.iter().all(|a| a.pode_conjunto());
    let deve_conjunto = infos.iter().any(|a| a.deve_conjunto());
    let pode_mapa = infos.iter().all(|a| a.pode_mapa());
    let deve_mapa = infos.iter().any(|a| a.deve_mapa());
    if pode_conjunto && deve_conjunto {
        return (Forma::Conjunto, None);
    }
    if pode_mapa && deve_mapa {
        return (Forma::Mapa, None);
    }
    (Forma::Mapa, Some(deve_mapa && deve_conjunto))
}

/// A forma que o contexto (`Map`/`Iterable`) impõe, se impõe.
fn forma_pelo_tipo_do_contexto(inf: &mut BodyInferrer<'_>, ctx: TypeId) -> Option<Forma> {
    if !inf.e_desconhecido(ctx) {
        let k = inf.fecho_maior(ctx);
        let k = inf.nao_nulo(k);
        let e_mapa = inf.como_instancia_de(k, inf.core.map_class).is_some() || matches!(inf.table.get(k), Type::Interface { class, .. } if Some(*class) == inf.core.map_class);
        let e_iter = inf.como_instancia_de(k, inf.core.iterable_class).is_some();
        if e_iter && !e_mapa {
            return Some(Forma::Conjunto);
        }
        if e_mapa && !e_iter {
            return Some(Forma::Mapa);
        }
        if let Type::FutureOr { arg, .. } = inf.table.get(k).clone() {
            if inf.como_instancia_de(arg, inf.core.iterable_class).is_some() {
                return Some(Forma::Conjunto);
            }
        }
    }
    None
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
            // Com argumentos de tipo escritos (sem inferência) e elemento
            // não `void`: valor `void` é `use_of_void_result`.
            if gi.is_none() && !matches!(inf.table.get(c), Type::Void) {
                super::expr::uso_de_void(inf, cx, *x, t);
            }
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
            if gi.is_none() {
                if !matches!(inf.table.get(ck), Type::Void) {
                    super::expr::uso_de_void(inf, cx, *key, tk);
                }
                if !matches!(inf.table.get(cv), Type::Void) {
                    super::expr::uso_de_void(inf, cx, *value, tv);
                }
            }
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
            let t = match inf.espalhamentos_inferidos.remove(value) {
                Some(t) => t,
                None => inferir(inf, cx, *value, c),
            };
            if *null_aware {
                super::expr::espalhamento_nulo_desnecessario(inf, cx, *value, t);
            } else {
                // `ResolverVisitor.visitSpreadElement` (`resolver.dart:3673-3692`).
                super::expr::desreferencia_anulavel(inf, cx, *value, t, dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_USE_OF_NULLABLE_VALUE_IN_SPREAD);
            }
            // `_computeElementType` / `_inferCollectionElementType`
            // (`typed_literal_resolver.dart:203-226, 380-425`), pelo tipo da
            // expressão: a instância de `Iterable`/`Map`; `dynamic`;
            // subtipo de `Never` → `Never`; subtipo de `Null` → `Never` com
            // `...?` (sem, `dynamic` na lista e nada no mapa); o resto,
            // `dynamic` na lista.
            let t0 = t;
            let t = inf.nao_nulo(t);
            let (never, null) = (inf.core.never, inf.core.null);
            let abaixo_de_never = inf.sub(t0, never);
            let abaixo_de_null = !abaixo_de_never && inf.sub(t0, null);
            let uma = |inf: &mut BodyInferrer<'_>, gi: &mut _, x: TypeId| {
                restringir(inf, gi, x, 0);
                if forma == Forma::Mapa {
                    restringir(inf, gi, x, 1);
                }
            };
            if inf.e_dynamic(t) {
                let d = inf.core.dynamic_;
                uma(inf, &mut gi, d);
            } else if abaixo_de_never || (abaixo_de_null && *null_aware) {
                uma(inf, &mut gi, never);
            } else if abaixo_de_null {
                if forma != Forma::Mapa {
                    let d = inf.core.dynamic_;
                    restringir(inf, &mut gi, d, 0);
                }
            } else if forma == Forma::Mapa {
                if let Some(args) = inf.como_instancia_de(t, inf.core.map_class) {
                    restringir(inf, &mut gi, args[0], 0);
                    restringir(inf, &mut gi, args[1], 1);
                }
            } else if let Some(args) = inf.como_instancia_de(t, inf.core.iterable_class) {
                restringir(inf, &mut gi, args[0], 0);
            } else {
                let d = inf.core.dynamic_;
                restringir(inf, &mut gi, d, 0);
            }
        }
        CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
            let antes = cx.fluxo.clone();
            let (vf, ff) = match case_pattern {
                Some(p) => {
                    let t = inferir_livre(inf, cx, *condition);
                    cx.empurrar_escopo();
                    let r = super::padroes::caso(inf, cx, *p, t, *guard, Some(*condition));
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
            // `for_conditionBegin`: o que a condição e as atualizações
            // escrevem perde a promoção (junção conservadora do laço).
            let mut partes = Vec::new();
            if let Some(c) = condition {
                partes.push(super::instrucoes::Parte::Expr(*c));
            }
            for u2 in updates.iter() {
                partes.push(super::instrucoes::Parte::Expr(*u2));
            }
            let (escritas, capturadas) = super::instrucoes::escritas_em(inf, cx, &partes);
            cx.fluxo.juncao_conservadora(&escritas, &capturadas);
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
pub(crate) fn validar_colecao_const(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId) {
    let unit = cx.unit;
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
            // O avaliador não sabe tudo (constantes referidas, operadores de
            // tipos do SDK): "não constante" só quando a expressão de fato
            // não é constante (especificação, "Constants").
            if m == CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE.template {
                let alvo = a.exprs.iter().position(|x| x.span == s).map(|i| ExprId(i as u32));
                if alvo.is_some_and(|x| inf.e_constante(cx, x)) {
                    continue;
                }
            }
            inf.aviso(m, s);
        }
    }
    for s in subs {
        validar_colecao_const(inf, cx, s);
    }
}
