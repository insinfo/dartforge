//! Invocações: funções, métodos, construtores, `call`, e a inferência de
//! argumentos de tipo com inferência "horizontal" (`inference-update-1`):
//! literais de função cujos parâmetros dependem de variáveis ainda não
//! resolvidas são inferidos depois dos demais argumentos.

use super::body::Body;
use super::expr::{self, inferir, infer_free, receptor, type_reference, registrar, resolver, resolve_name, NameRef, TypeRef};
use super::members::Busca;
use super::BodyInferrer;
use crate::codes::*;
use crate::constraints::{instantiate_function, GenericInferrer};
use crate::resolved::Resolved;
use crate::table::{Type, TypeId, TypeParamId};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionElementId, FunctionKind};
use dartforge_frontend::ast::{self, ExprId, ExprKind};
use std::collections::HashMap;

/// Parâmetro formal correspondente a cada argumento.
fn argument_parameters(
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

/// O que a verificação de aridade precisa saber da chamada (o `nameNode` e
/// o `errorEntity` do analyzer), definido por quem conhece a forma dela.
#[derive(Debug, Clone, Default)]
pub(crate) struct ArityTarget {
    /// Nome citado em `NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_*`
    /// (`_reportNotEnoughPositionalArguments`,
    /// an611:src/generated/resolver.dart:4370-4420); `None` usa as
    /// variantes sem nome.
    pub nome: Option<String>,
    /// Intervalo do `MISSING_REQUIRED_ARGUMENT`
    /// (an611:src/error/required_parameters_verifier.dart:21-110): o nome do
    /// método, o nome do construtor, a chamada inteira ou a lista de
    /// argumentos, conforme a forma.
    pub entidade: Option<Span>,
    /// Parâmetros `super.x` posicionais do construtor corrente (argumentos
    /// implícitos de um `super(...)`, `verifySuperFormalParameters`,
    /// an611:src/error/super_formal_parameters_verifier.dart:11-37).
    pub super_positional_args: usize,
    /// Nomes dos parâmetros `super.x` nomeados do construtor corrente.
    pub super_named_args: Vec<dartforge_intern::SymbolId>,
}

/// Constante de enum sem argumentos (`v;`) com construtor sem nome que
/// exige argumentos: `NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_*` no nome da
/// constante (an611:src/generated/resolver.dart:2531-2544) e
/// `MISSING_REQUIRED_ARGUMENT` também nele
/// (an611:src/error/required_parameters_verifier.dart:37-43).
pub(crate) fn arity_without_arguments(inf: &mut BodyInferrer<'_>, f: FunctionElementId, enum_name: String, span: Span) {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    let sig = inf.outline.functions[f.0 as usize].signature;
    let Type::Function { positional, named, .. } = inf.table.get(sig).clone() else { return };
    let n = positional.len();
    if n == 1 {
        inf.warning_with_code(ce::NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_SINGULAR, span, &[&enum_name]);
    } else if n > 1 {
        inf.warning_with_code(ce::NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_PLURAL, span, &[&n.to_string(), "0", &enum_name]);
    }
    for (s, _, req) in named.iter() {
        if *req {
            let the_name = inf.interner.resolve(*s).to_string();
            inf.warning_with_code(ce::MISSING_REQUIRED_ARGUMENT, span, &[&the_name]);
        }
    }
}

/// Define o alvo da próxima verificação de aridade.
pub(crate) fn define_target(inf: &mut BodyInferrer<'_>, the_name: Option<String>, entity: Span) {
    inf.arity_target = Some(ArityTarget { nome: the_name, entidade: Some(entity), ..Default::default() });
}

/// O próximo token da fonte a partir de `pos` (pula espaços e comentários):
/// um identificador inteiro ou um caractere.
fn next_token(source: &str, mut pos: usize) -> Span {
    let b = source.as_bytes();
    loop {
        while pos < b.len() && (b[pos] as char).is_ascii_whitespace() {
            pos += 1;
        }
        if b.get(pos) == Some(&b'/') && b.get(pos + 1) == Some(&b'/') {
            while pos < b.len() && b[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }
        if b.get(pos) == Some(&b'/') && b.get(pos + 1) == Some(&b'*') {
            match source[pos + 2..].find("*/") {
                Some(i) => pos = pos + 2 + i + 2,
                None => pos = b.len(),
            }
            continue;
        }
        break;
    }
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    let mut end = pos;
    if b.get(pos).is_some_and(|&c| ident(c)) {
        while end < b.len() && ident(b[end]) {
            end += 1;
        }
    } else if pos < b.len() {
        end = pos + source[pos..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }
    Span { start: pos, end: end }
}

/// Diagnósticos de aridade e nomes (`resolveArgumentsToParameters`,
/// an611:src/generated/resolver.dart:4229-4355, e
/// `RequiredParametersVerifier`): nomeado indefinido e repetido no nome do
/// argumento; poucos posicionais no token depois do último posicional (ou
/// depois do `(`); posicionais demais no primeiro excedente, com a variante
/// `_COULD_BE_NAMED` quando sobram nomeados não usados; nomeado obrigatório
/// ausente na entidade da chamada.
fn verify_arity(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    positional: &[TypeId],
    optional: &[TypeId],
    named: &[(dartforge_intern::SymbolId, TypeId, bool)],
    args: &ast::Arguments,
    tgt: Option<ArityTarget>,
) {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    let tgt = tgt.unwrap_or_default();
    let a = &inf.program.unit(cx.unit).ast;
    let source = &inf.program.unit(cx.unit).source;
    let unnamed = positional.len() + optional.len();
    let mut npos = 0usize;
    let mut without_blank = true;
    let mut first_excess: Option<Span> = None;
    let mut last_positional: Option<Span> = None;
    for x in args.args.iter().filter(|x| x.name.is_none()) {
        let sp = a.expr(x.value).span;
        if sp.start >= sp.end {
            without_blank = false;
        }
        if npos >= unnamed && first_excess.is_none() {
            first_excess = Some(sp);
        }
        npos += 1;
        last_positional = Some(sp);
    }
    // Token do "poucos posicionais": depois do último posicional, ou o
    // primeiro depois do `(`.
    let token_few = match last_positional {
        Some(sp) => next_token(source, sp.end),
        None => {
            let begin = args.type_args.last().map(|&t| a.ty(t).span.end).unwrap_or(args.span.start);
            let open = source.get(begin..).and_then(|r| r.find('(')).map(|i| begin + i + 1).unwrap_or(args.span.start);
            next_token(source, open)
        }
    };
    // O `)` sintético do scanner (comprimento zero no fim dos argumentos):
    // quando o token seguinte é ele, o erro sai ali com comprimento zero.
    let synthetic_closure = args.span.end > args.span.start && source.as_bytes().get(args.span.end - 1) != Some(&b')');
    let token_few = if synthetic_closure && token_few.start >= args.span.end {
        Span { start: args.span.end, end: args.span.end }
    } else {
        token_few
    };
    let total_pos = npos + tgt.super_positional_args;
    let mut used: Vec<dartforge_intern::SymbolId> = tgt.super_named_args.clone();
    let mut warnings: Vec<(dartforge_diagnostics::Codigo, Span, Vec<String>)> = Vec::new();
    for x in args.args.iter() {
        if let Some(n) = &x.name {
            let text = inf.interner.resolve(n.sym).to_string();
            if !named.iter().any(|(s, _, _)| *s == n.sym) {
                warnings.push((ce::UNDEFINED_NAMED_PARAMETER, n.span, vec![text.clone()]));
            }
            if used.contains(&n.sym) {
                warnings.push((ce::DUPLICATE_NAMED_ARGUMENT, n.span, vec![text]));
            } else {
                used.push(n.sym);
            }
        }
    }
    if total_pos < positional.len() && without_blank {
        let required = positional.len();
        let plural = required > 1;
        let mut args_list: Vec<String> = Vec::new();
        if plural {
            args_list.push(required.to_string());
            args_list.push(total_pos.to_string());
        }
        let code = match (&tgt.nome, plural) {
            (None, true) => ce::NOT_ENOUGH_POSITIONAL_ARGUMENTS_PLURAL,
            (None, false) => ce::NOT_ENOUGH_POSITIONAL_ARGUMENTS_SINGULAR,
            (Some(_), true) => ce::NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_PLURAL,
            (Some(_), false) => ce::NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_SINGULAR,
        };
        if let Some(n) = &tgt.nome {
            args_list.push(n.clone());
        }
        warnings.push((code, token_few, args_list));
    } else if total_pos > unnamed && without_blank {
        let code = if named.len() > used.len() {
            ce::EXTRA_POSITIONAL_ARGUMENTS_COULD_BE_NAMED
        } else {
            ce::EXTRA_POSITIONAL_ARGUMENTS
        };
        if let Some(sp) = first_excess {
            warnings.push((code, sp, vec![unnamed.to_string(), total_pos.to_string()]));
        }
    }
    let entity = tgt.entidade.unwrap_or(args.span);
    for (s, _, req) in named.iter() {
        if *req && !args.args.iter().any(|x| x.name.map(|n| n.sym) == Some(*s)) && !tgt.super_named_args.contains(s) {
            warnings.push((ce::MISSING_REQUIRED_ARGUMENT, entity, vec![inf.interner.resolve(*s).to_string()]));
        }
    }
    for (code, sp, args_list) in warnings {
        let refs: Vec<&str> = args_list.iter().map(|s| s.as_str()).collect();
        inf.warning_with_code(code, sp, &refs);
    }
}

/// O argumento é um literal de função com algum parâmetro sem tipo (candidato
/// a ser adiado na inferência horizontal).
fn deferrable_function_literal(inf: &BodyInferrer<'_>, cx: &Body, e: ExprId) -> bool {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.expr(e).kind {
        ExprKind::FunctionExpression(f) => {
            let f = a.function(*f);
            f.parameters.as_ref().is_some_and(|ps| ps.iter().any(|p| p.ty.is_none() && p.function_parameters.is_none()))
        }
        ExprKind::Parenthesized(i) => deferrable_function_literal(inf, cx, *i),
        _ => false,
    }
}

fn mentions(inf: &BodyInferrer<'_>, t: TypeId, params: &[TypeParamId]) -> bool {
    match inf.table.get(t) {
        Type::TypeParameter { param, .. } => params.contains(param),
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.iter().any(|a| mentions(inf, *a, params)),
        Type::FutureOr { arg, .. } => mentions(inf, *arg, params),
        Type::Function { ret, positional, optional, named, .. } => {
            mentions(inf, *ret, params)
                || positional.iter().chain(optional.iter()).any(|a| mentions(inf, *a, params))
                || named.iter().any(|(_, a, _)| mentions(inf, *a, params))
        }
        Type::Record { positional, named, .. } => {
            positional.iter().any(|a| mentions(inf, *a, params)) || named.iter().any(|(_, a)| mentions(inf, *a, params))
        }
        _ => false,
    }
}

/// Ordem da inferência horizontal (`inference-update-1`): os argumentos não
/// adiados formam o estágio 0; os literais de função adiados seguem as
/// dependências entre si — um literal cujos tipos de parâmetro mencionam uma
/// variável que o retorno de outro literal adiado ainda fornece espera por
/// ele. Os que só dependem de variáveis já fornecidas formam o estágio
/// seguinte (na ordem do texto); num ciclo, os restantes vão juntos.
/// Devolve `(estágio, índice do argumento)` na ordem de inferência.
fn horizontal_stages(
    inf: &BodyInferrer<'_>,
    type_params: &[TypeParamId],
    params: &[Option<TypeId>],
    deferred: &[bool],
) -> Vec<(usize, usize)> {
    let mut order: Vec<(usize, usize)> = (0..deferred.len()).filter(|&i| !deferred[i]).map(|i| (0, i)).collect();
    // Para cada adiado: variáveis de que depende (nos tipos dos parâmetros
    // do formal) e as que fornece (no retorno do formal).
    let mut remaining: Vec<(usize, Vec<TypeParamId>, Vec<TypeParamId>)> = Vec::new();
    for i in (0..deferred.len()).filter(|&i| deferred[i]) {
        let Some(p) = params[i] else { continue };
        let (entries, output): (Vec<TypeId>, Option<TypeId>) = match inf.table.get(p) {
            Type::Function { ret, positional, optional, named, .. } => (
                positional.iter().chain(optional.iter()).copied().chain(named.iter().map(|(_, t, _)| *t)).collect(),
                Some(*ret),
            ),
            _ => (vec![p], None),
        };
        let depends: Vec<TypeParamId> =
            type_params.iter().copied().filter(|&v| entries.iter().any(|&t| mentions(inf, t, &[v]))).collect();
        let provides: Vec<TypeParamId> =
            type_params.iter().copied().filter(|&v| output.is_some_and(|t| mentions(inf, t, &[v]))).collect();
        remaining.push((i, depends, provides));
    }
    let mut stage = 1;
    while !remaining.is_empty() {
        let ready: Vec<usize> = (0..remaining.len())
            .filter(|&k| {
                remaining[k].1.iter().all(|v| {
                    !remaining.iter().enumerate().any(|(j, (_, _, provides))| j != k && provides.contains(v))
                })
            })
            .collect();
        let ready = if ready.is_empty() { (0..remaining.len()).collect() } else { ready };
        for &k in &ready {
            order.push((stage, remaining[k].0));
        }
        let mut k = 0;
        remaining.retain(|_| {
            let stays = !ready.contains(&k);
            k += 1;
            stays
        });
        stage += 1;
    }
    order
}

/// `TYPE_ARGUMENT_NOT_MATCHING_BOUNDS` de argumentos de tipo explícitos
/// contra os parâmetros de tipo `params` (o `_checkInvocationTypeArguments`
/// do `TypeArgumentsVerifier`, o `_checkTypeArgumentsMatchingBounds` do
/// `ExtensionMemberResolver` e o do `AnnotationInferrer`): só os primeiros
/// `min(params, tipos)`, com a substituição desses; parâmetro sem limite
/// escrito fica de fora. O erro vai no argumento escrito `nos[i]`, ou em
/// `alternativo`.
///
/// `visiveis`: os parâmetros de tipo em escopo na chamada. A substituição
/// de um tipo de função não troca os limites dos parâmetros de tipo dele
/// (o `m<T extends X>` de `C<num>` guarda o `X`), então um limite que ainda
/// menciona parâmetro de tipo fora do escopo e fora de `params` não é
/// conferido.
pub(crate) fn check_explicit_bounds(
    inf: &mut BodyInferrer<'_>,
    unit: dartforge_elements::model::UnitId,
    params: &[TypeParamId],
    types: &[TypeId],
    nodes: &[ast::TypeId],
    alternative: Span,
    visible: Option<&[TypeParamId]>,
) {
    let n = params.len().min(types.len());
    let map = inf.mapa(&params[..n], &types[..n]);
    let mut allowed: std::collections::HashSet<TypeParamId> = params.iter().copied().collect();
    if let Some(v) = visible {
        allowed.extend(v.iter().copied());
    }
    for i in 0..n {
        let data = inf.table.param(params[i]).clone();
        if !data.explicito {
            continue;
        }
        if visible.is_some() && mentions_free(&inf.table, data.bound, &allowed, 0) {
            continue;
        }
        let bnd = inf.subst(data.bound, &map);
        let ok = {
            let mut env = inf.env();
            crate::subtyping::is_subtype(types[i], bnd, &mut env)
        };
        if ok {
            continue;
        }
        let sp = nodes.get(i).map(|&x| inf.program.unit(unit).ast.ty(x).span).unwrap_or(alternative);
        let a = inf.table.format(types[i], inf.interner, inf.program);
        let the_name = inf.interner.resolve(data.name).to_string();
        let l = inf.table.format(bnd, inf.interner, inf.program);
        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::TYPE_ARGUMENT_NOT_MATCHING_BOUNDS, sp, &[&a, &the_name, &l]);
    }
}

/// `t` menciona um parâmetro de tipo que não está em `permitidos` nem é
/// formal de um tipo de função dentro de `t`.
fn mentions_free(table: &crate::table::TypeTable, t: TypeId, allowed: &std::collections::HashSet<TypeParamId>, prof: u32) -> bool {
    if prof > 32 {
        return false;
    }
    match table.get(t) {
        Type::TypeParameter { param, .. } => !allowed.contains(param),
        Type::Intersection { param, bound } => !allowed.contains(param) || mentions_free(table, *bound, allowed, prof + 1),
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.iter().any(|&a| mentions_free(table, a, allowed, prof + 1)),
        Type::FutureOr { arg, .. } => mentions_free(table, *arg, allowed, prof + 1),
        Type::Record { positional, named, .. } => {
            positional.iter().any(|&a| mentions_free(table, a, allowed, prof + 1)) || named.iter().any(|(_, a)| mentions_free(table, *a, allowed, prof + 1))
        }
        Type::Function { type_params, ret, positional, optional, named, .. } => {
            let mut inside = allowed.clone();
            inside.extend(type_params.iter().copied());
            type_params.iter().any(|&p| mentions_free(table, table.param(p).bound, &inside, prof + 1))
                || mentions_free(table, *ret, &inside, prof + 1)
                || positional.iter().chain(optional.iter()).any(|&a| mentions_free(table, a, &inside, prof + 1))
                || named.iter().any(|(_, a, _)| mentions_free(table, *a, &inside, prof + 1))
        }
        _ => false,
    }
}

/// `resolver.checkUnreachableNode(argumentList)` (`invocation_inferrer.dart:530`):
/// a lista de argumentos (do `(`) é o primeiro nó morto quando a invocação
/// chega inalcançável; o trecho vai ao fim do bloco básico em curso.
pub(crate) fn dead_in_list(inf: &mut BodyInferrer<'_>, cx: &mut Body, args: &ast::Arguments) {
    if cx.flow.alcancavel || cx.dead_code.is_some() {
        return;
    }
    let sp = args.span;
    let end = cx.flow_ends.last().copied().or(cx.block_ends.last().copied()).unwrap_or(sp.end).max(sp.end);
    inf.aviso(DEAD_CODE.template.to_string(), Span { start: sp.start, end: end });
    cx.dead_code = Some(cx.flow_ends.len());
}

/// Invoca um tipo de função com os argumentos; devolve `(retorno, função instanciada)`.
pub(crate) fn invoke(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    f: TypeId,
    args: &ast::Arguments,
    ctx: TypeId,
    explicit: Option<Vec<TypeId>>,
) -> (TypeId, TypeId) {
    let tgt = inf.arity_target.take();
    let implicit_call = std::mem::take(&mut inf.implicit_call);
    dead_in_list(inf, cx, args);
    let Type::Function { type_params, ret, positional, optional, named, .. } = inf.table.get(f).clone() else {
        inf.inference_entity = None;
        inf.nomes_posicionais = None;
        for a in args.args.iter() {
            infer_free(inf, cx, a.value);
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
            matches!(cx.buscar(n), Some(super::body::Nome::TypeParam(q)) if q == p)
        })
    {
        let new_ = inf.new_parameters(&type_params);
        let types: Vec<TypeId> = new_.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        let map = inf.mapa(&type_params, &types);
        let ret = inf.subst(ret, &map);
        let positional: Box<[TypeId]> = positional.iter().map(|&t| inf.subst(t, &map)).collect();
        let optional: Box<[TypeId]> = optional.iter().map(|&t| inf.subst(t, &map)).collect();
        let named: Box<[_]> = named.iter().map(|&(n, t, r)| (n, inf.subst(t, &map), r)).collect();
        let f2 = inf.table.intern(Type::Function { type_params: new_.into_boxed_slice(), ret, positional, optional, named, nullable: false });
        inf.arity_target = tgt;
        inf.implicit_call = implicit_call;
        return invoke(inf, cx, f2, args, ctx, explicit);
    }
    // `checkMethodInvocation` / `checkFunctionExpressionInvocation` do
    // `ErrorVerifier`: os explícitos contra os parâmetros de tipo do tipo
    // invocado (não no `call` implícito: o tipo da função é de interface).
    if let Some(ex) = &explicit
        && !type_params.is_empty()
        && !implicit_call
    {
        let ex = ex.clone();
        let visible: Vec<TypeParamId> = cx.visible_type_parameters().into_values().collect();
        check_explicit_bounds(inf, cx.unit, &type_params, &ex, &args.type_args, args.span, Some(&visible));
    }
    // O `errorEntity` vale só para esta invocação (os argumentos têm as suas).
    let entity = inf.inference_entity.take();
    let positional_names = inf.nomes_posicionais.take();
    // `FullInvocationInferrer.resolveInvocation`
    // (`invocation_inferrer.dart:175-184` do analyzer 3.6.2): a lista de
    // argumentos de tipo com contagem diferente da dos parâmetros de tipo
    // do tipo invocado é `WRONG_NUMBER_OF_TYPE_ARGUMENTS_METHOD`, na lista
    // `<…>`, com o tipo de função cru como primeiro argumento; a inferência
    // segue sem os explícitos. Sem a espécie e o nome do alvo (que só quem
    // chama conhece), a variante 3.13 do código sai na forma "tipo desta
    // função" (docs/ANALYZER-ESPECIFICACAO.md, T2 §8, caso c27).
    if let Some(ex) = &explicit
        && ex.len() != type_params.len()
        && let (Some(&first), Some(&last)) = (args.type_args.first(), args.type_args.last())
    {
        let the_program = inf.program;
        let unit_id = the_program.unit(cx.unit);
        let (of, until) = (unit_id.ast.ty(first).span.start, unit_id.ast.ty(last).span.end);
        let source = unit_id.source.as_str();
        let start = source.get(..of).and_then(|t| t.rfind('<')).unwrap_or(of);
        let end = source.get(until..).and_then(|t| t.find('>')).map_or(until, |i| until + i + 1);
        let typ = inf.table.format(f, inf.interner, inf.program);
        inf.warning_with_code(
            dartforge_diagnostics::codigos::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS_METHOD,
            Span { start: start, end: end },
            &[&typ, &type_params.len().to_string(), &ex.len().to_string()],
        );
    }
    // Argumentos de tipo explícitos: instancia e segue como não genérica.
    if !type_params.is_empty() {
        if let Some(ex) = explicit {
            // Contagem errada: `dynamic` para cada parâmetro de tipo, sem
            // inferência (`invocation_inferrer.dart:174-183`).
            let ex = if ex.len() == type_params.len() { ex } else { vec![inf.core.dynamic_; type_params.len()] };
            {
                let mut env = inf.env();
                let inst = instantiate_function(f, &ex, &mut env);
                drop(env);
                inf.body_types.units[cx.unit.0 as usize].set_instantiation(args.span.start, ex.into_boxed_slice());
                inf.arity_target = tgt;
                return invoke(inf, cx, inst, args, ctx, None);
            }
        }
    }
    verify_arity(inf, cx, &positional, &optional, &named, args, tgt);
    let params = argument_parameters(inf, &positional, &optional, &named, args);
    let numeric_context = inf.pending_numeric_context.take();
    if type_params.is_empty() {
        for (a, p) in args.args.iter().zip(params.iter()) {
            // `x.clamp(a, b)` / `x.remainder(a)`: o contexto refinado do analyzer.
            let c = match (numeric_context, a.name) {
                (Some(c), None) => c,
                _ => p.unwrap_or(u),
            };
            let t = inferir(inf, cx, a.value, c);
            if let Some(p) = p {
                inf.body_types.units[cx.unit.0 as usize].tipos_de_parametros.insert(a.value, *p);
                expr::verify_assignable_expr(inf, cx, a.value, t, *p, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
                closure_with_set(inf, cx, a.value, *p);
            }
        }
        return (ret, f);
    }
    // Genérica: para baixo (retorno × contexto), depois argumentos em estágios.
    let mut gi = GenericInferrer::new(&type_params);
    if !inf.the_is_unknown(ctx) {
        gi.with_origin(crate::constraints::Origem::Retorno { declarado: ret, contexto: ctx });
        let mut env = inf.env();
        gi.constrain_return(ret, ctx, &mut env);
    }
    let mut env = inf.env();
    let mut prelim = gi.choose_preliminary(&mut env);
    drop(env);
    let deferred: Vec<bool> = args
        .args
        .iter()
        .zip(params.iter())
        .map(|(a, p)| p.is_some_and(|p| mentions(inf, p, &type_params)) && deferrable_function_literal(inf, cx, a.value))
        .collect();
    let mut types: Vec<TypeId> = vec![inf.core.dynamic_; args.args.len()];
    for (stage, i) in horizontal_stages(inf, &type_params, &params, &deferred) {
        let a = &args.args[i];
        {
            if stage > 0 {
                let mut env = inf.env();
                prelim = gi.choose_preliminary(&mut env);
            }
            let context = match params[i] {
                Some(p) => {
                    let map: HashMap<TypeParamId, TypeId> = type_params.iter().copied().zip(prelim.iter().copied()).collect();
                    inf.subst(p, &map)
                }
                None => u,
            };
            let t = inferir(inf, cx, a.value, context);
            types[i] = t;
            if let Some(p) = params[i] {
                let t = inf.implicit_call_type(t, p).unwrap_or(t);
                let parameter = match a.name {
                    Some(n) => inf.interner.resolve(n.sym).to_string(),
                    None => {
                        let pos = args.args[..i].iter().filter(|x| x.name.is_none()).count();
                        positional_names.as_ref().and_then(|v| v.get(pos).cloned()).unwrap_or_default()
                    }
                };
                gi.with_origin(crate::constraints::Origem::Argumento { parametro: parameter, declarado: p, argumento: t, prefixo: None });
                let mut env = inf.env();
                gi.constrain_argument(t, p, &mut env);
            }
        }
    }
    let use_bounds = inf.program.library(cx.lib).features.tem(dartforge_frontend::Feature::InferenceUsingBounds);
    gi.generic_metadata = inf.program.library(cx.lib).features.tem(dartforge_frontend::Feature::GenericMetadata);
    let (interner, program) = (inf.interner, inf.program);
    let mut env = inf.env();
    if use_bounds {
        gi.restringir_pelos_limites(&mut env);
    }
    let finals = gi.choose_final(&mut env);
    let failures = match entity {
        Some(_) => gi.failures(&finals, &mut env, interner, program),
        None => Vec::new(),
    };
    let inst = instantiate_function(f, &finals, &mut env);
    drop(env);
    if let Some(sp) = entity {
        for (the_name, suffix) in failures {
            inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::COULD_NOT_INFER, sp, &[&the_name, &suffix]);
        }
    }
    inf.body_types.units[cx.unit.0 as usize].set_instantiation(args.span.start, finals.clone().into_boxed_slice());
    // Checagem com os parâmetros instanciados.
    if let Type::Function { positional: ip, optional: io, named: inm, ret: iret, .. } = inf.table.get(inst).clone() {
        let ips = argument_parameters(inf, &ip, &io, &inm, args);
        for (i, a) in args.args.iter().enumerate() {
            if let Some(p) = ips[i] {
                inf.body_types.units[cx.unit.0 as usize].tipos_de_parametros.insert(a.value, p);
                expr::verify_assignable_expr(inf, cx, a.value, types[i], p, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
                closure_with_set(inf, cx, a.value, p);
            }
        }
        return (iret, inst);
    }
    (inf.core.dynamic_, inst)
}

/// O argumento é uma closure (o próprio `FunctionExpression`, com o
/// `staticParameterElement`) e o parâmetro é um tipo de função: o
/// `UNNECESSARY_SET_LITERAL` pelo retorno dele.
fn closure_with_set(inf: &mut BodyInferrer<'_>, cx: &Body, argument: ExprId, parameter: TypeId) {
    let ExprKind::FunctionExpression(fid) = inf.program.unit(cx.unit).ast.expr(argument).kind else { return };
    if let Type::Function { ret, .. } = inf.table.get(parameter).clone() {
        super::functions::unnecessary_set(inf, cx.unit, fid, ret);
    }
}

/// Invoca um valor de tipo `t` (função, objeto com `call`, `dynamic`).
fn invoke_value(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, t: TypeId, args: &ast::Arguments, ctx: TypeId, explicit: Option<Vec<TypeId>>, span: Span) -> (TypeId, TypeId) {
    // O alvo da aridade só vale se a invocação de fato acontecer.
    let tgt = inf.arity_target.take();
    if let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind {
        let tgt = *target;
        if !matches!(inf.program.unit(cx.unit).ast.expr(tgt).kind, ExprKind::Property { .. }) && expr::never_receiver(inf, cx, tgt, t) {
            dead_in_list(inf, cx, args);
            for a in args.args.iter() {
                infer_free(inf, cx, a.value);
            }
            return (inf.core.never, t);
        }
    }
    // `x()` com `x` de tipo `Null` ou `Never?`: o `call` não existe e o
    // receptor pode ser nulo — `invalid_use_of_null_value` (`Null`) ou
    // `unchecked_use_of_nullable_value` (`Never?`) na função; o resultado é
    // `dynamic` (não `Never`: não há código morto depois).
    if matches!(inf.table.get(t), Type::Null)
        && let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind
    {
        let function = *target;
        expr::nullable_dereference(inf, cx, function, t, dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_INVOCATION_OF_NULLABLE_VALUE);
        for a in args.args.iter() {
            infer_free(inf, cx, a.value);
        }
        let _ = tgt;
        return (inf.core.dynamic_, t);
    }
    let t_nn = inf.non_null(t);
    // `x(3)` com `x` de tipo `void`: `use_of_void_result` na função.
    if matches!(inf.table.get(t_nn), Type::Void) {
        if let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind {
            let tgt = *target;
            expr::void_use(inf, cx, tgt, t_nn);
        }
    }
    match inf.table.get(t_nn).clone() {
        Type::Function { .. } => {
            // `FunctionExpressionInvocationResolver.resolve`
            // (`function_expression_invocation_resolver.dart:52-58`): a função
            // de tipo de função anulável, no nó da função.
            if let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind {
                let function = *target;
                expr::nullable_dereference(inf, cx, function, t, dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_INVOCATION_OF_NULLABLE_VALUE);
            }
            inf.arity_target = tgt;
            invoke(inf, cx, t_nn, args, ctx, explicit)
        }
        Type::Dynamic => {
            for a in args.args.iter() {
                infer_free(inf, cx, a.value);
            }
            (inf.core.dynamic_, t)
        }
        Type::Never => {
            for a in args.args.iter() {
                infer_free(inf, cx, a.value);
            }
            (inf.core.never, t)
        }
        Type::Interface { class, .. } if Some(class) == inf.core.function_class => {
            // `Function?`: o `call` pelo `TypePropertyResolver`, com o
            // receptor anulável relatado na função.
            if let Some(call) = inf.sym.call
                && inf.requires_null_check(cx.lib, t, call, false)
                && let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind
            {
                let function = *target;
                expr::nullable_dereference(inf, cx, function, t, dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_INVOCATION_OF_NULLABLE_VALUE);
            }
            for a in args.args.iter() {
                infer_free(inf, cx, a.value);
            }
            (inf.core.dynamic_, t)
        }
        _ => {
            // `FunctionExpressionInvocationResolver.resolve`
            // (`function_expression_invocation_resolver.dart:72-78`): fora do
            // tipo de função, o `call` pelo `TypePropertyResolver`; receptor
            // potencialmente anulável é `UNCHECKED_INVOCATION_OF_NULLABLE_VALUE`
            // na função (o pai é `FunctionExpressionInvocation`), e a
            // recuperação segue pelo limite.
            if let Some(call) = inf.sym.call
                && inf.requires_null_check(cx.lib, t, call, false)
                && let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind
            {
                let function = *target;
                expr::nullable_dereference(inf, cx, function, t, dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_INVOCATION_OF_NULLABLE_VALUE);
            }
            // `createT()('')` com `T extends int Function(int)`: a
            // `FunctionExpressionInvocation` do parser (alvo que não é nome
            // nem acesso a propriedade) não leva `T` ao limite; o
            // `TypePropertyResolver` acha o `call` do tipo de função do
            // limite só como `callFunctionType`, sem elemento, e o resolver
            // do 3.6.2 deixa a chamada `dynamic`, sem conferir os argumentos
            // (`function_expression_invocation_resolver.dart:80-95`). A
            // chamada por nome ou propriedade (`tValue('')`) é reescrita pelo
            // `MethodInvocationResolver`, que leva ao limite
            // (`_rewriteAsFunctionExpressionInvocation`, `resolveToBound`).
            if matches!(inf.table.get(t_nn), Type::TypeParameter { .. } | Type::Intersection { .. })
                && let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind
                && !matches!(inf.program.unit(cx.unit).ast.expr(*target).kind, ExprKind::Identifier(_) | ExprKind::Property { .. })
                && matches!(inf.table.get(inf.resolver_ao_limite(t_nn)), Type::Function { .. })
            {
                for a in args.args.iter() {
                    infer_free(inf, cx, a.value);
                }
                return (inf.core.dynamic_, t);
            }
            // O `call` achado que não é método (getter ou campo `call`, da
            // classe ou de extensão): `callElement.kind != METHOD`
            // (`function_expression_invocation_resolver.dart:96-103`),
            // relatado na função, com o resultado inválido.
            let call_not_method = |inf: &mut BodyInferrer<'_>, cx: &mut Body| -> (TypeId, TypeId) {
                if let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind {
                    let sp = inf.span_expr(cx.unit, *target);
                    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::INVOCATION_OF_NON_FUNCTION_EXPRESSION, sp, &[]);
                }
                for a in args.args.iter() {
                    infer_free(inf, cx, a.value);
                }
                (inf.table.invalid(inf.core.dynamic_), t)
            };
            // `super(…)`: o `call` pela cadeia de `super` (o
            // `TypePropertyResolver` com receptor `SuperExpression`); sem ele,
            // `INVOCATION_OF_NON_FUNCTION_EXPRESSION` no `super` e o tipo é
            // `InvalidType`.
            if let Some(call) = inf.sym.call
                && let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind
                && matches!(inf.program.unit(cx.unit).ast.expr(*target).kind, ExprKind::Super)
            {
                let super_target = *target;
                match expr::lookup_super_operator(inf, cx, call) {
                    Busca::Achado(m) if m.metodo => {
                        inf.arity_target = tgt;
                        return invoke(inf, cx, m.tipo, args, ctx, explicit);
                    }
                    _ => {
                        for a in args.args.iter() {
                            infer_free(inf, cx, a.value);
                        }
                        let sp = inf.span_expr(cx.unit, super_target);
                        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::INVOCATION_OF_NON_FUNCTION_EXPRESSION, sp, &[]);
                        return (inf.table.invalid(inf.core.dynamic_), t);
                    }
                }
            }
            if let Some(call) = inf.sym.call {
                if let Some(m) = inf.interface_member(t_nn, call, false) {
                    if !m.metodo {
                        return call_not_method(inf, cx);
                    }
                    inf.arity_target = tgt;
                    inf.implicit_call = true;
                    return invoke(inf, cx, m.tipo, args, ctx, explicit);
                }
                let search = inf.lookup_member(cx.lib, t_nn, call, false);
                if inf.extension_ambiguity.is_some() {
                    // `call` ambíguo: o analyzer relata a ambiguidade no alvo
                    // (`function_expression_invocation_resolver.dart`) e não
                    // resolve.
                    let sp = match &inf.program.unit(cx.unit).ast.expr(e).kind {
                        ExprKind::Call { target, .. } => inf.span_expr(cx.unit, *target),
                        _ => span,
                    };
                    inf.report_extension_ambiguity(sp);
                    for a in args.args.iter() {
                        infer_free(inf, cx, a.value);
                    }
                    return (inf.core.dynamic_, t);
                }
                if let Busca::Achado(m) = search {
                    // Receptor anulável: a extensão só vale se se aplica ao
                    // tipo anulável; senão não há `call` (o nulo já foi
                    // relatado, e o `needsGetterError` fica falso).
                    if m.of_extension && t != t_nn && !matches!(inf.lookup_member(cx.lib, t, call, false), Busca::Achado(_)) {
                        for a in args.args.iter() {
                            infer_free(inf, cx, a.value);
                        }
                        return (inf.core.dynamic_, t);
                    }
                    if !m.metodo {
                        return call_not_method(inf, cx);
                    }
                    // `valor(args)` com o `call` de uma extensão
                    // (`calloc<Int32>(4)`, o `AllocatorAlloc.call`): a
                    // chamada registra o membro, que o lowering invoca com
                    // o valor como receptor.
                    if m.of_extension {
                        super::expr::resolver(inf, cx, e, m.resolved.clone());
                    }
                    inf.arity_target = tgt;
                    inf.implicit_call = true;
                    return invoke(inf, cx, m.tipo, args, ctx, explicit);
                }
            }
            for a in args.args.iter() {
                infer_free(inf, cx, a.value);
            }
            // `o()`, `x.campo()`, `3(5)`: o analyzer relata
            // `invocation_of_non_function_expression` no alvo da chamada. Um
            // literal de tipo (`T<Null>()` com `T` alias de `dynamic`) é outro
            // código (`invocation_of_non_function`), fora daqui.
            // Ficam de fora (outros códigos ou nada): receptor anulável
            // (`unchecked_…`), `void` (`use_of_void_result`), parâmetro de
            // tipo (a chamada vai pelo limite) e nome solto que não é local
            // (`foo()` com só `set foo`: `undefined_method`).
            let _ = span;
            let a = &inf.program.unit(cx.unit).ast;
            let tgt = match &a.expr(e).kind {
                ExprKind::Call { target, .. } => Some(*target),
                _ => None,
            };
            // O nome que resolve para um getter ou variável (de topo,
            // estática, membro pelo `this` implícito, de extensão) é reescrito
            // em `FunctionExpressionInvocation` e relata
            // (`method_invocation_resolver.dart:585-592`, `:900-905`); o nome
            // sem getter (só setter) é `undefined_method`.
            let types = &inf.body_types.units[cx.unit.0 as usize];
            let getter = |x: ExprId| match types.get_resolved(x) {
                Some(Resolved::Element(Element::Variable(_))) | Some(Resolved::Member { member: crate::resolved::MemberRef::Variable(_), .. }) => true,
                Some(Resolved::Element(Element::Function(f)))
                | Some(Resolved::Member { member: crate::resolved::MemberRef::Function(f), .. })
                | Some(Resolved::ExtensionMember { member: f, .. }) => {
                    matches!(inf.program.function(*f).kind, dartforge_elements::model::FunctionKind::Getter | dartforge_elements::model::FunctionKind::ImplicitAccessor)
                }
                _ => false,
            };
            let non_local_name = tgt.is_some_and(|x| match &a.expr(x).kind {
                ExprKind::Identifier(n) => !matches!(cx.buscar(n.sym), Some(super::body::Nome::Local(_))) && !getter(x),
                _ => false,
            });
            // `Type` de um literal de tipo invocado (`T<Null>()`) é outro
            // código; entre parênteses (`(T)()`) é `FunctionExpressionInvocation`
            // sem `call` em `Type`, e relata.
            let parenthesized = tgt.is_some_and(|x| matches!(a.expr(x).kind, ExprKind::Parenthesized(_)));
            let outside = (t_nn == inf.core.type_ && !parenthesized)
                || t != t_nn
                || non_local_name
                || matches!(inf.table.get(t_nn), Type::Void | Type::TypeParameter { .. } | Type::Intersection { .. });
            if let (false, Some(tgt)) = (outside, tgt) {
                let sp = inf.span_expr(cx.unit, tgt);
                inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::INVOCATION_OF_NON_FUNCTION_EXPRESSION, sp, &[]);
                // `result.isGetterInvalid` (3.6.2
                // `function_expression_invocation_resolver.dart:89-92`): sem
                // `call` nenhum, o tipo é `InvalidType`.
                return (inf.table.invalid(inf.core.dynamic_), t);
            }
            (inf.core.dynamic_, t)
        }
    }
}

fn argumentos_de_tipo(inf: &mut BodyInferrer<'_>, cx: &Body, args: &ast::Arguments) -> Option<Vec<TypeId>> {
    if args.type_args.is_empty() {
        return None;
    }
    Some(args.type_args.iter().map(|&t| inf.type_argument_type(cx, t)).collect())
}

/// Prepara o `errorEntity` do `COULD_NOT_INFER` da próxima invocação
/// genérica e os nomes dos parâmetros posicionais do alvo `f`.
pub(crate) fn prepare_entity(inf: &mut BodyInferrer<'_>, sp: Span, f: Option<FunctionElementId>) {
    inf.inference_entity = Some(sp);
    inf.nomes_posicionais = f.map(|f| {
        inf.outline.functions[f.0 as usize]
            .parameters
            .iter()
            .filter(|p| p.kind != ast::ParameterKind::Named)
            .map(|p| p.name.map(|n| inf.interner.resolve(n).to_string()).unwrap_or_default())
            .collect()
    });
}

/// A função (de topo, membro, de extensão) a que `e` resolveu.
fn resolved_function(inf: &BodyInferrer<'_>, cx: &Body, e: ExprId) -> Option<FunctionElementId> {
    match inf.body_types.units[cx.unit.0 as usize].get_resolved(e)? {
        Resolved::Element(Element::Function(f)) => Some(*f),
        Resolved::Member { member: crate::resolved::MemberRef::Function(f), .. } => Some(*f),
        Resolved::ExtensionMember { member, .. } => Some(*member),
        _ => None,
    }
}

/// `f(args)`, `r.m(args)`, `C(args)`, `C.nome(args)`...
/// `ExtensionMemberResolver.resolveOverride`
/// (`an611:src/dart/resolver/extension_member_resolver.dart:177-187`): o
/// `E(x)` fora de um acesso — operando esquerdo de binária, função de uma
/// invocação, alvo de índice, de método ou de propriedade, operando de
/// prefixo — e fora de alvo de cascata: `EXTENSION_OVERRIDE_WITHOUT_ACCESS`
/// no override inteiro.
fn override_without_access(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) {
    let valid = match inf.parent_of(cx.unit, e) {
        dartforge_frontend::pais::Pai::Expr(p) => match &inf.program.unit(cx.unit).ast.expr(p).kind {
            ExprKind::Binary { left, .. } => *left == e,
            ExprKind::Call { target, .. } | ExprKind::Index { target, .. } | ExprKind::Property { target, .. } | ExprKind::Cascade { target, .. } => *target == e,
            ExprKind::Unary { op, operand } => {
                *operand == e && matches!(op, ast::UnaryOp::Neg | ast::UnaryOp::Not | ast::UnaryOp::BitNot | ast::UnaryOp::PrefixInc | ast::UnaryOp::PrefixDec)
            }
            _ => false,
        },
        _ => false,
    };
    if !valid {
        let sp = inf.span_expr(cx.unit, e);
        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::EXTENSION_OVERRIDE_WITHOUT_ACCESS, sp, &[]);
    }
}

/// A lista `<…>` escrita de argumentos de tipo: do `<` antes do primeiro
/// ao `>` depois do último.
pub(crate) fn type_list_range(inf: &BodyInferrer<'_>, unit: dartforge_elements::model::UnitId, types: &[ast::TypeId]) -> Option<Span> {
    let (first, last) = (types.first()?, types.last()?);
    let unit_id = inf.program.unit(unit);
    let (of, until) = (unit_id.ast.ty(*first).span.start, unit_id.ast.ty(*last).span.end);
    let source = &unit_id.source;
    let start = source.get(..of).and_then(|t| t.rfind('<')).unwrap_or(of);
    let end = source.get(until..).and_then(|t| t.find('>')).map_or(until, |i| until + i + 1);
    Some(Span { start: start, end: end })
}

/// `WRONG_NUMBER_OF_TYPE_ARGUMENTS_CONSTRUCTOR` (`ast_rewrite.dart:438-445`,
/// `:604-611`; `named_type_resolver.dart:337-343`;
/// `function_reference_resolver.dart:51-62`): argumentos de tipo depois do
/// nome de um construtor, na lista `<…>`; `{0}` o tipo como escrito (com o
/// prefixo), `{1}` o construtor.
pub(crate) fn types_in_constructor(inf: &mut BodyInferrer<'_>, unit: dartforge_elements::model::UnitId, written_class: Span, ctor: ast::Name, types: &[ast::TypeId]) {
    let Some(sp) = type_list_range(inf, unit, types) else { return };
    let source = &inf.program.unit(unit).source;
    let cls = source[written_class.start..written_class.end].split_whitespace().collect::<String>();
    let the_name = inf.interner.resolve(ctor.sym).to_string();
    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS_CONSTRUCTOR, sp, &[&cls, &the_name]);
}

pub(crate) fn chamada(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, ctx: TypeId) -> (TypeId, bool) {
    let a = &inf.program.unit(cx.unit).ast;
    let ExprKind::Call { target, arguments } = &a.expr(e).kind else { unreachable!() };
    let (target, args): (ExprId, &ast::Arguments) = (*target, arguments);
    let span = a.expr(e).span;
    let explicit = argumentos_de_tipo(inf, cx, args);
    // `C<T>.nome(…)` é criação só quando o fasta a lê como criação implícita
    // (`criacoes_implicitas`); com argumentos de tipo depois do nome
    // (`C<T>.nome<U>()`) é método do literal de tipo, que o `AstRewriter` não
    // reescreve.
    let the_implicit = a.criacoes_implicitas.contains(&e);
    let over_instantiation = matches!(&a.expr(target).kind, ExprKind::Property { target: r, .. } if matches!(a.expr(*r).kind, ExprKind::TypeArguments { .. }));
    let ctor = if over_instantiation && !the_implicit { None } else { target_constructor(inf, cx, target) };
    if the_implicit && let ExprKind::Property { target: recv, .. } = &a.expr(target).kind {
        expr::check_literal_arguments(inf, cx, *recv, true);
    }
    if ctor.is_none()
        && the_implicit
        && let ExprKind::Property { target: recv, name, .. } = &a.expr(target).kind
        && let Some(rt) = type_reference(inf, cx, *recv)
        && !matches!(rt, TypeRef::Extensao(_))
    {
        // A criação implícita sem o construtor: `InstanceCreationExpression`
        // de construtor não resolvido (`NEW_WITH_UNDEFINED_CONSTRUCTOR`, no
        // nome, com o tipo como escrito); o tipo é o do tipo nomeado e os
        // argumentos ficam sem contexto.
        let (recv, name) = (*recv, *name);
        let (c, targs) = match &rt {
            TypeRef::Classe(c, Some(ts)) => (*c, ts.iter().map(|&t| inf.type_argument_type(cx, t)).collect::<Vec<_>>()),
            TypeRef::Classe(c, None) => (*c, Vec::new()),
            TypeRef::Alias(c, args, _) => (*c, args.clone().unwrap_or_default()),
            TypeRef::Extensao(_) => unreachable!(),
        };
        record_reference(inf, cx, recv);
        if !matches!(inf.program.class(c).kind, ClassKind::Enum | ClassKind::Mixin) {
            let base = match &a.expr(recv).kind {
                ExprKind::TypeArguments { target: b, .. } => *b,
                _ => recv,
            };
            let qualified = match &a.expr(base).kind {
                ExprKind::Identifier(n) => inf.interner.resolve(n.sym).to_string(),
                ExprKind::Property { target: p, name: n, .. } => match &a.expr(*p).kind {
                    ExprKind::Identifier(p) => format!("{}.{}", inf.interner.resolve(p.sym), inf.interner.resolve(n.sym)),
                    _ => inf.interner.resolve(n.sym).to_string(),
                },
                _ => String::new(),
            };
            let the_name = inf.interner.resolve(name.sym).to_string();
            inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::NEW_WITH_UNDEFINED_CONSTRUCTOR, name.span, &[&qualified, &the_name]);
        }
        for x in args.args.iter() {
            infer_free(inf, cx, x.value);
        }
        return (inf.class_type_with_args(c, targs), false);
    }
    // `T<X>.nome(…)` lido como criação com `T` alias de um tipo que não é
    // classe: o `CREATION_WITH_NON_TYPE` da criação, e nada do literal.
    if ctor.is_none()
        && the_implicit
        && let ExprKind::Property { target: recv, .. } = &a.expr(target).kind
        && let ExprKind::TypeArguments { target: base, type_args } = &a.expr(*recv).kind
        && expr::alias_without_class(inf, cx, *base)
        && !expr::alias_of_null_or_futureor(inf, cx, *base)
        && !expr::function_type_alias(inf, cx, *base)
    {
        // (O alias de tipo de função o `AstRewriter` reescreve como chamada
        // no literal de tipo, `ast_rewrite.dart:55-62`: segue o caminho comum.)
        let names: Vec<ast::Name> = match &a.expr(*base).kind {
            ExprKind::Identifier(n) => vec![*n],
            ExprKind::Property { target: p, name, .. } => match &a.expr(*p).kind {
                ExprKind::Identifier(p) => vec![*p, *name],
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };
        if expr::type_parameter_alias(inf, cx, *base) {
            // `_verifyTypeAliasForContext`: no nome, sem os argumentos.
            if let (Some(p), Some(u)) = (names.first(), names.last()) {
                let sp = Span { start: p.span.start, end: u.span.end };
                inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::INSTANTIATE_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER, sp, &[]);
            }
        } else if !names.is_empty() {
            creation_without_class(inf, cx, &names);
        }
        for &x in type_args.iter() {
            inf.type_argument_type(cx, x);
        }
        for x in args.args.iter() {
            infer_free(inf, cx, x.value);
        }
        return (inf.table.invalid(inf.core.dynamic_), false);
    }
    // `X<T>.nome(…)` lido como criação com `X` que não é tipo (indefinido,
    // local, função, parâmetro de tipo): o `NamedTypeResolver` relata
    // `CREATION_WITH_NON_TYPE` pela criação (`_isInstanceCreation`, 3.6.2
    // `named_type_resolver.dart:230-232, 274-276`), e não o nome indefinido.
    if ctor.is_none()
        && the_implicit
        && let ExprKind::Property { target: recv, .. } = &a.expr(target).kind
        && let ExprKind::TypeArguments { target: base, type_args } = &a.expr(*recv).kind
        && type_reference(inf, cx, *base).is_none()
        && !expr::function_type_alias(inf, cx, *base)
        && stays_implicit_creation(inf, cx, *base)
    {
        let names: Vec<ast::Name> = match &a.expr(*base).kind {
            ExprKind::Identifier(n) => vec![*n],
            ExprKind::Property { target: p, name, .. } => match &a.expr(*p).kind {
                ExprKind::Identifier(p) => vec![*p, *name],
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };
        if !names.is_empty() {
            creation_without_class(inf, cx, &names);
            for &x in type_args.iter() {
                inf.type_argument_type(cx, x);
            }
            for x in args.args.iter() {
                infer_free(inf, cx, x.value);
            }
            return (inf.table.invalid(inf.core.dynamic_), false);
        }
    }
    // Construtor sem `new`.
    if let Some((c, f, targs)) = ctor {
        if inf.program.class(c).kind == ClassKind::Enum
            && f.is_some_and(|f| !inf.program.function(f).factory)
        {
            inf.aviso(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template.to_string(), a.expr(target).span);
            // O construtor fica resolvido (`constructorName.staticElement`):
            // a avaliação constante segue pela chamada dele.
            if let Some(f) = f {
                resolver(inf, cx, e, Resolved::Constructor(f));
            }
            for arg in args.args.iter() {
                infer_free(inf, cx, arg.value);
            }
            return (inf.core.dynamic_, false);
        }
        if let Some(f) = f {
            // O tipo nomeado da criação implícita: `A`, `A<int>`, `p.A`
            // (sem o nome do construtor).
            let sp = match &a.expr(target).kind {
                ExprKind::Property { target: t, .. } if expr::type_reference(inf, cx, *t).is_some() => creation_named_type(inf, cx, *t, &[]),
                _ => creation_named_type(inf, cx, target, &args.type_args),
            };
            warn_abstract_class(inf, c, f, sp);
        }
        // `C(...)`/`p.C(...)` viram `InstanceCreationExpression` no analyzer:
        // o nome é `C.new` (ou o do construtor) e a entidade, o
        // `constructorName` (tipo com prefixo e argumentos, mais `.nome`).
        let (the_name, ent) = match &a.expr(target).kind {
            ExprKind::Property { target: t, name, .. } if expr::type_reference(inf, cx, *t).is_some() => {
                (inf.interner.resolve(name.sym).to_string(), a.expr(target).span)
            }
            _ => {
                let cls = last_identifier(inf, cx, target).map(|s| inf.interner.resolve(s).to_string()).unwrap_or_default();
                (format!("{cls}.new"), creation_named_type(inf, cx, target, &args.type_args))
            }
        };
        // `C.nome<T>()`: a lista é do construtor (erro) e não da classe.
        let explicit = match &a.expr(target).kind {
            ExprKind::Property { target: t, name, .. } if !args.type_args.is_empty() && expr::type_reference(inf, cx, *t).is_some() => {
                let written = a.expr(*t).span;
                types_in_constructor(inf, cx.unit, written, *name, &args.type_args);
                None
            }
            _ => explicit,
        };
        // A contagem dos argumentos de tipo escritos na classe (`C<T>()`,
        // `C<T>.nome()`): o `NamedType` da criação reescrita, com a
        // contagem errada do `_buildTypeArguments` e os argumentos
        // `InvalidType`.
        let mut targs = targs;
        let mut explicit = explicit;
        // Em `D<int>()`, os argumentos ficam na chamada, enquanto a
        // referência `D` foi resolvida sem eles. Aplique-os ao alias antes
        // de usar os argumentos da classe (`D<X> = C<List<X>>`).
        if !args.type_args.is_empty()
            && let Some(TypeRef::Alias(_, _, td)) = type_reference(inf, cx, target)
            && let Some(TypeRef::Alias(_, inst, _)) = expr::alias_of(inf, td, explicit.clone())
        {
            targs = inst;
        }
        let a = &inf.program.unit(cx.unit).ast;
        let written_ones: Option<(usize, Span)> = match &a.expr(target).kind {
            ExprKind::Property { target: t, .. } => match &a.expr(*t).kind {
                ExprKind::TypeArguments { target: base, type_args } if matches!(type_reference(inf, cx, *base), Some(TypeRef::Classe(..))) => {
                    let n = type_args.len();
                    let sp = creation_named_type(inf, cx, *t, &[]);
                    Some((n, sp))
                }
                _ => None,
            },
            _ if !args.type_args.is_empty() && matches!(type_reference(inf, cx, target), Some(TypeRef::Classe(..))) => {
                Some((args.type_args.len(), creation_named_type(inf, cx, target, &args.type_args)))
            }
            _ => None,
        };
        let n_params = inf.outline.classes[c.0 as usize].type_params.len();
        if let Some((n, sp)) = written_ones
            && n != n_params
        {
            let text = inf.interner.resolve(inf.program.class(c).name).to_string();
            inf.report_type_arguments(&text, n_params, n, sp);
            let invalid = vec![inf.table.invalid(inf.core.dynamic_); n_params];
            targs = Some(invalid);
            explicit = None;
        }
        define_target(inf, Some(the_name), ent);
        let alias = if args.type_args.is_empty() { creation_alias(inf, cx, target) } else { None };
        let t = match alias {
            Some(_) => build_with(inf, cx, Some(e), c, f, None, args, ctx, alias),
            None => construir(inf, cx, Some(e), c, f, targs.or(explicit), args, ctx),
        };
        inf.arity_target = None;
        return (t, false);
    }
    // `E()`: o sem nome implícito do enum não está na tabela de
    // construtores (então `alvo_construtor` não o achou), mas existe e é
    // gerador. Com nome explícito, o braço acima já resolveu (factory é
    // legal, gerador acusa); aqui só falta o implícito.
    if let Some(rt) = type_reference(inf, cx, target)
        && !matches!(rt, TypeRef::Extensao(_))
    {
        let c = match rt {
            TypeRef::Classe(c, _) | TypeRef::Alias(c, _, _) => c,
            TypeRef::Extensao(_) => unreachable!(),
        };
        // `A()` de classe abstrata sem construtor declarado: o padrão
        // implícito é gerador (`INSTANTIATE_ABSTRACT_CLASS`).
        {
            let k = inf.program.class(c);
            if matches!(k.kind, ClassKind::Class | ClassKind::MixinApplication)
                && (k.modifiers.abstract_ || k.modifiers.sealed)
                && k.constructors.is_empty()
            {
                let sp = creation_named_type(inf, cx, target, &args.type_args);
                inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::INSTANTIATE_ABSTRACT_CLASS, sp, &[]);
            }
        }
        if inf.program.class(c).kind == ClassKind::Enum
            && inf.sym.vazio.is_some_and(|v| inf.construtor_de(c, v).is_none())
        {
            inf.aviso(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template.to_string(), a.expr(target).span);
            for arg in args.args.iter() {
                infer_free(inf, cx, arg.value);
            }
            return (inf.core.dynamic_, false);
        }
        // `C()` reescrita como criação (`ast_rewrite.dart`) sem o construtor
        // sem nome, nem o implícito: `NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT`
        // no tipo nomeado (`_checkForNewWithUndefinedConstructor`).
        if matches!(inf.program.class(c).kind, ClassKind::Class | ClassKind::MixinApplication)
            && inf.sym.vazio.is_some_and(|v| inf.construtor_de(c, v).is_none())
            // `A.new()` declarado (sem `constructor-tearoffs` o modelo guarda
            // a chave `new`) é o sem nome.
            && inf.sym.new_.is_none_or(|n| inf.construtor_de(c, n).is_none())
            && !super::functions::without_implicit_name(inf, c)
        {
            let sp = creation_named_type(inf, cx, target, &args.type_args);
            let base = match &a.expr(target).kind {
                ExprKind::TypeArguments { target: b, .. } => *b,
                _ => target,
            };
            let qualified = match &a.expr(base).kind {
                ExprKind::Identifier(n) => inf.interner.resolve(n.sym).to_string(),
                ExprKind::Property { target: p, name, .. } => match &a.expr(*p).kind {
                    ExprKind::Identifier(p) => format!("{}.{}", inf.interner.resolve(p.sym), inf.interner.resolve(name.sym)),
                    _ => inf.interner.resolve(name.sym).to_string(),
                },
                _ => String::new(),
            };
            inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT, sp, &[&qualified]);
            for arg in args.args.iter() {
                infer_free(inf, cx, arg.value);
            }
            return (inf.table.invalid(inf.core.dynamic_), false);
        }
    }
    let u = inf.core.unknown;
    // `E(a, b)` / `E()`: `INVALID_EXTENSION_ARGUMENT_COUNT` na lista de
    // argumentos (an611:src/dart/resolver/extension_member_resolver.dart:189-197).
    if let Some(TypeRef::Extensao(_)) = type_reference(inf, cx, target) {
        override_without_access(inf, cx, e);
    }
    if let Some(TypeRef::Extensao(x)) = type_reference(inf, cx, target)
        && !(args.args.len() == 1 && args.args[0].name.is_none())
        && args.args.iter().all(|a| a.name.is_none())
        && args.args.len() != 1
    {
        let source = &inf.program.unit(cx.unit).source;
        let begin = args.type_args.last().map(|&t| a.ty(t).span.end).unwrap_or(args.span.start);
        let open = source.get(begin..).and_then(|r| r.find('(')).map(|i| begin + i).unwrap_or(args.span.start);
        let sp = Span { start: open, end: args.span.end.max(open) };
        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::INVALID_EXTENSION_ARGUMENT_COUNT, sp, &[]);
        for arg in args.args.iter() {
            infer_free(inf, cx, arg.value);
        }
        let data = inf.outline.extensions[x.0 as usize].clone();
        let ext_args = inf.instantiate_to_bounds(&data.type_params);
        cx.overlays.insert(e, (x, ext_args));
        return (inf.core.dynamic_, false);
    }
    // `E(x)` / `E<T>(x)`: sobreposição explícita de extensão (R-EXT-02).
    if let Some(TypeRef::Extensao(x)) = type_reference(inf, cx, target)
        && args.args.len() == 1
        && args.args[0].name.is_none()
    {
        let data = inf.outline.extensions[x.0 as usize].clone();
        // `E<A, B>(x)` com número errado de argumentos de tipo
        // (an611:src/dart/resolver/extension_member_resolver.dart:333-345).
        if let Some(ex) = &explicit
            && ex.len() != data.type_params.len()
            && let (Some(&p), Some(&u2)) = (args.type_args.first(), args.type_args.last())
        {
            let source = &inf.program.unit(cx.unit).source;
            let start_t = a.ty(p).span.start;
            let end_t = a.ty(u2).span.end;
            let open = source.get(..start_t).and_then(|r| r.rfind('<')).unwrap_or(start_t);
            let closes = source.get(end_t..).and_then(|r| r.find('>')).map(|i| end_t + i + 1).unwrap_or(end_t);
            let the_name = inf.program.extension(x).name.map(|n| inf.interner.resolve(n).to_string()).unwrap_or_default();
            let (np, at) = (data.type_params.len().to_string(), ex.len().to_string());
            inf.warning_with_code(
                dartforge_diagnostics::codigos::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS_EXTENSION,
                Span { start: open, end: closes },
                &[&the_name, &np, &at],
            );
        }
        let ext_args = match &explicit {
            Some(ex) if ex.len() == data.type_params.len() => Some(ex.clone()),
            _ => None,
        };
        let ctx_arg = match &ext_args {
            Some(ex) => {
                let map = inf.mapa(&data.type_params, ex);
                inf.subst(data.on, &map)
            }
            None => u,
        };
        let t = inferir(inf, cx, args.args[0].value, ctx_arg);
        let explicit_ok = ext_args.is_some();
        if let Some(ex) = &ext_args {
            let ex = ex.clone();
            check_explicit_bounds(inf, cx.unit, &data.type_params, &ex, &args.type_args, args.span, None);
        }
        // `_inferTypeArguments` (an611:src/dart/resolver/extension_member_resolver.dart:321-367):
        // os explícitos; com contagem errada, `dynamic`; sem eles, o
        // `GenericInferrer` com o `errorEntity` no nome da extensão e a
        // restrição `constrainArgument(receptor, extendedType)`.
        let ext_args = match (ext_args, &explicit) {
            (Some(ex), _) => ex,
            (None, Some(_)) => vec![inf.core.dynamic_; data.type_params.len()],
            (None, None) if data.type_params.is_empty() => Vec::new(),
            (None, None) => {
                // Parâmetros novos: dentro da própria extensão o receptor
                // menciona os parâmetros dela.
                let new_ = inf.new_parameters(&data.type_params);
                let types: Vec<TypeId> = new_.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
                let m0 = inf.mapa(&data.type_params, &types);
                let on_novo = inf.subst(data.on, &m0);
                let entity = match &a.expr(target).kind {
                    ExprKind::Property { name, .. } => name.span,
                    _ => a.expr(target).span,
                };
                let mut gi = GenericInferrer::new(&new_);
                gi.with_origin(crate::constraints::Origem::Argumento { parametro: "extendedType".to_string(), declarado: on_novo, argumento: t, prefixo: None });
                let use_bounds = inf.program.library(cx.lib).features.tem(dartforge_frontend::Feature::InferenceUsingBounds);
                gi.generic_metadata = inf.program.library(cx.lib).features.tem(dartforge_frontend::Feature::GenericMetadata);
                let (interner, program) = (inf.interner, inf.program);
                let mut env = inf.env();
                gi.constrain_argument(t, on_novo, &mut env);
                if use_bounds {
                    gi.restringir_pelos_limites(&mut env);
                }
                let finals = gi.choose_final(&mut env);
                let failures = gi.failures(&finals, &mut env, interner, program);
                drop(env);
                for (the_name, suffix) in failures {
                    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::COULD_NOT_INFER, entity, &[&the_name, &suffix]);
                }
                finals
            }
        };
        // `EXTENSION_OVERRIDE_ARGUMENT_NOT_ASSIGNABLE` no argumento
        // (an611:src/dart/resolver/extension_member_resolver.dart:227-243);
        // `void` é `USE_OF_VOID_RESULT`. Só sem parâmetros de tipo ou com
        // argumentos explícitos (a inferência falha tem outro relato).
        if data.type_params.is_empty() || explicit_ok {
            let arg = args.args[0].value;
            // `receiverType is VoidType`: no argumento inteiro (`atNode`, não
            // o nome do método de `f()`).
            let empty = matches!(inf.table.get(t), Type::Void);
            if empty {
                let sp = inf.span_expr(cx.unit, arg);
                inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::USE_OF_VOID_RESULT, sp, &[]);
            } else {
                let map = inf.mapa(&data.type_params, &ext_args);
                let on = inf.subst(data.on, &map);
                // Override alvo de `?.`/`?[`/`?..` (`isNullAware`): o tipo do
                // argumento conferido é o não anulável.
                let null_aware = match inf.parent_of(cx.unit, e) {
                    dartforge_frontend::pais::Pai::Expr(parent) => match &inf.program.unit(cx.unit).ast.expr(parent).kind {
                        ExprKind::Property { target: tgt, null_aware: true, .. }
                        | ExprKind::Index { target: tgt, null_aware: true, .. }
                        | ExprKind::Cascade { target: tgt, null_aware: true, .. } => *tgt == e,
                        _ => false,
                    },
                    _ => false,
                };
                let t = if null_aware { inf.non_null(t) } else { t };
                if !inf.atribuivel(t, on) {
                    let sp = inf.span_expr(cx.unit, arg);
                    let since = inf.diagnostics.len();
                    inf.warning_with_args(
                        dartforge_diagnostics::codigos::compile_time_error::EXTENSION_OVERRIDE_ARGUMENT_NOT_ASSIGNABLE,
                        sp,
                        &[crate::exibicao::Arg::Tipo(t), crate::exibicao::Arg::Tipo(on)],
                    );
                    inf.attach_non_promotion(since, cx, Some(arg), sp);
                }
            }
        }
        cx.overlays.insert(e, (x, ext_args));
        return (t, false);
    }
    match &a.expr(target).kind {
        ExprKind::Property { target: recv, name, null_aware } => {
            let (recv, name, null_aware) = (*recv, *name, *null_aware);
            if null_aware && expr::type_reference(inf, cx, recv).is_some() {
                expr::null_operator_on_type(inf, cx, recv);
            }
            // `p.f(args)`
            if let ExprKind::Identifier(p) = &a.expr(recv).kind {
                if matches!(resolve_name(inf, cx, p.sym, false), NameRef::Prefixo) {
                    // `p?.f()`: o alvo de `MethodInvocation` só vale como
                    // prefixo com `.` (`_isValidAsPrefix`).
                    if null_aware {
                        expr::prefix_without_dot(inf, *p);
                    }
                    // `p.f(args)` sem getter `f` no prefixo
                    // (`_resolveReceiverPrefix`,
                    // `an611:src/dart/resolver/method_invocation_resolver.dart:666-722`):
                    // `UNDEFINED_FUNCTION`, salvo `loadLibrary` de import
                    // adiado e o nome ignorado de import que não existe.
                    let p = *p;
                    if inf.program.lookup_prefixed_na_unidade(cx.unit, p.sym, name.sym).and_then(|b| b.getter).is_none()
                        && expr::load_library(inf, cx, p.sym, name.sym).is_none()
                    {
                        expr::resolver(inf, cx, recv, Resolved::Prefix(cx.lib));
                        if !crate::scope::should_ignore_undefined(inf.program, inf.interner, cx.unit, Some(p.sym), name.sym) {
                            let the_name = inf.interner.resolve(name.sym).to_string();
                            inf.warning_with_code(
                                dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_FUNCTION,
                                name.span,
                                &[&the_name],
                            );
                        }
                        let d = inf.core.dynamic_;
                        registrar(inf, cx, target, d);
                        for x in args.args.iter() {
                            infer_free(inf, cx, x.value);
                        }
                        return (d, false);
                    }
                    let t = inferir(inf, cx, target, u);
                    let f = resolved_function(inf, cx, target);
                    prepare_entity(inf, name.span, f);
                    method_target(inf, f, name, span);
                    let (r, _) = invoke_value(inf, cx, e, t, args, ctx, explicit, span);
                    inf.inference_entity = None;
                    inf.arity_target = None;
                    return (r, false);
                }
            }
            // `C.m(args)` estático / `E.m(args)`. Só um receptor identificador
            // (`receiver is IdentifierImpl`, method_invocation_resolver.dart:129-143):
            // o literal de tipo instanciado (`C<T>.m<U>()`) é expressão de
            // tipo `Type`, e o método é procurado nele.
            let instantiated = matches!(inf.program.unit(cx.unit).ast.expr(recv).kind, ExprKind::TypeArguments { .. });
            // O literal de tipo instanciado de um alias de tipo de função
            // (`Fn<int>.foo()`, 3.6.2 `method_invocation_resolver.dart:169-182`):
            // nenhuma resolução possível, `UNDEFINED_METHOD_ON_FUNCTION_TYPE`
            // no nome com o `qualifiedName` do tipo, e o tipo de recuperação.
            if let ExprKind::TypeArguments { target: base, .. } = &inf.program.unit(cx.unit).ast.expr(recv).kind
                && expr::function_type_alias(inf, cx, *base)
            {
                let base = *base;
                infer_free(inf, cx, recv);
                let sp_base = inf.span_expr(cx.unit, base);
                let qualified = inf.program.unit(cx.unit).source[sp_base.start..sp_base.end].to_string();
                let the_name = inf.interner.resolve(name.sym).to_string();
                inf.warning_with_code(
                    dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_METHOD_ON_FUNCTION_TYPE,
                    name.span,
                    &[&the_name, &qualified],
                );
                let d = inf.table.invalid(inf.core.dynamic_);
                registrar(inf, cx, target, d);
                for x in args.args.iter() {
                    infer_free(inf, cx, x.value);
                }
                return (d, false);
            }
            if !instantiated && let Some(rt) = type_reference(inf, cx, recv) {
                // `T?.m()` encurta salvo com `T` identificador simples de
                // classe (`expr::encurtamento_dispensado`).
                let short_circuit_type = null_aware && !expr::shortening_waived(inf, cx, recv, &rt);
                if let TypeRef::Extensao(x) = rt {
                    if inf.extension_static_member(x, name.sym, false).is_none() {
                        if inf.program.extension(x).instance_members.contains_key(&name.sym) {
                            let msg = format!("{}: '{}'", STATIC_ACCESS_TO_INSTANCE_MEMBER.template, inf.interner.resolve(name.sym));
                            inf.aviso(msg, name.span);
                            for arg in args.args.iter() {
                                infer_free(inf, cx, arg.value);
                            }
                            return (inf.core.dynamic_, false);
                        }
                        let extension = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                        let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_METHOD.template, inf.interner.resolve(name.sym), extension);
                        inf.aviso(msg, name.span);
                        for arg in args.args.iter() {
                            infer_free(inf, cx, arg.value);
                        }
                        return (inf.core.dynamic_, false);
                    }
                }
                // `C.m(args)` sem membro estático `m` nem de instância (este
                // é `STATIC_ACCESS_TO_INSTANCE_MEMBER`, pelo acesso): o
                // analyzer relata `UNDEFINED_METHOD` com o **nome** do
                // elemento (`_resolveReceiverTypeLiteral`,
                // an611:src/dart/resolver/method_invocation_resolver.dart:890-939;
                // alias de interface: o elemento aliasado). `C.new()` e
                // `C<T>.m()` seguem o caminho comum.
                let cls = match &rt {
                    TypeRef::Classe(c, _) | TypeRef::Alias(c, _, _) => Some(*c),
                    _ => None,
                };
                // `_resolveElement` (`:340-355`) só aceita o membro acessível
                // na biblioteca que chama (`isAccessibleIn`): o privado de
                // outra biblioteca não existe ali.
                let inaccessible = |inf: &BodyInferrer<'_>, c: ClassId| {
                    inf.interner.resolve(name.sym).starts_with('_') && inf.program.class(c).library != cx.lib
                };
                if let Some(c) = cls
                    && !matches!(a.expr(recv).kind, ExprKind::TypeArguments { .. })
                    && Some(name.sym) != inf.sym.new_
                    && (inaccessible(inf, c)
                        || (inf.static_member(c, name.sym, false).is_none() && !inf.program.class(c).instance_members.contains_key(&name.sym)))
                {
                    let the_name = inf.interner.resolve(name.sym).to_string();
                    let typ = inf.interner.resolve(inf.program.class(c).name).to_string();
                    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_METHOD, name.span, &[&the_name, &typ]);
                    // O receptor já foi resolvido pelo `ResolverVisitor`
                    // antes do `MethodInvocationResolver`: o elemento (a
                    // classe ou o alias) e o tipo `Type` do literal de tipo.
                    let element = match &rt {
                        TypeRef::Alias(_, _, t) => Element::Typedef(*t),
                        _ => Element::Class(c),
                    };
                    expr::resolver(inf, cx, recv, Resolved::Element(element));
                    let type_type = inf.core.type_;
                    registrar(inf, cx, recv, type_type);
                    // `_setInvalidTypeResolution`: a invocação que não
                    // resolve tem o tipo de recuperação.
                    let d = inf.table.invalid(inf.core.dynamic_);
                    registrar(inf, cx, target, d);
                    for arg in args.args.iter() {
                        infer_free(inf, cx, arg.value);
                    }
                    return (d, false);
                }
                let t = inferir(inf, cx, target, u);
                let t = if short_circuit_type { inf.non_null(t) } else { t };
                let f = resolved_function(inf, cx, target);
                prepare_entity(inf, name.span, f);
                method_target(inf, f, name, span);
                let (r, _) = invoke_value(inf, cx, e, t, args, ctx, explicit, span);
                inf.inference_entity = None;
                inf.arity_target = None;
                return (r, short_circuit_type);
            }
            // `super.m(args)`.
            if matches!(a.expr(recv).kind, ExprKind::Super) {
                let this = cx.tipo_this.unwrap_or(inf.core.dynamic_);
                registrar(inf, cx, recv, this);
                let t = expr::super_member(inf, cx, target, name, expr::SuperUse::Invocacao);
                // `super._f()` com `_f` campo: a leitura de `super._f` vê a
                // promoção dele (a chave `super`, distinta da de `this`).
                let t = expr::field_read(inf, cx, target, super::body::Base::Super, t);
                registrar(inf, cx, target, t);
                let f = resolved_function(inf, cx, target);
                method_target(inf, f, name, span);
                let (r, _) = invoke_value(inf, cx, e, t, args, ctx, explicit, span);
                inf.arity_target = None;
                // `super?.m()`: null-aware como em `super?.x`.
                return (r, null_aware);
            }
            let (r_ty, short_circuit) = receptor(inf, cx, recv, null_aware);
            // `x?.m(args)` com `x` sempre nulo: o lado direito é morto, e o
            // primeiro nó morto é a lista de argumentos (do `(`).
            if null_aware {
                dead_in_list(inf, cx, args);
            }
            if !null_aware && !cx.overlays.contains_key(&recv) && expr::never_receiver(inf, cx, recv, r_ty) {
                dead_in_list(inf, cx, args);
                for arg in args.args.iter() {
                    infer_free(inf, cx, arg.value);
                }
                return (inf.core.never, short_circuit);
            }
            // `void` e receptor anulável: as mesmas regras do acesso a
            // propriedade (`expr::propriedade`), na variante de invocação.
            if matches!(inf.table.get(r_ty), crate::table::Type::Void) {
                // Na invocação, o analyzer relata no receptor (`this` em
                // `this.m()`); no acesso a propriedade, no nome.
                // Numa seção de cascata (`x..m()`), o relato é no alvo da
                // cascata, feito lá.
                if !matches!(inf.program.unit(cx.unit).ast.expr(recv).kind, ExprKind::CascadeTarget) {
                    let sp = inf.span_expr(cx.unit, recv);
                    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::USE_OF_VOID_RESULT, sp, &[]);
                }
                let d = inf.core.dynamic_;
                registrar(inf, cx, target, d);
                for x in args.args.iter() {
                    infer_free(inf, cx, x.value);
                }
                return (d, short_circuit);
            }
            let check_null =
                !cx.overlays.contains_key(&recv) && inf.requires_null_check(cx.lib, r_ty, name.sym, false);
            if check_null {
                let the_name = inf.interner.resolve(name.sym).to_string();
                let since = inf.diagnostics.len();
                inf.null_warning(
                    r_ty,
                    dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE,
                    name.span,
                    &[&the_name],
                );
                inf.attach_non_promotion(since, cx, Some(recv), name.span);
            }
            let mut search = expr::lookup_target_member(inf, cx, recv, r_ty, name.sym, false);
            inf.report_extension_ambiguity(name.span);
            if matches!(search, Busca::Missing) {
                if let Some((x, _)) = cx.overlays.get(&recv).cloned() {
                    if let Some(m) = inf.extension_static_member(x, name.sym, false) {
                        // O analyzer ainda resolve a assinatura do método,
                        // mas rejeita seu acesso via `E(valor).metodo()`.
                        inf.aviso(
                            EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template.to_string(),
                            name.span,
                        );
                        search = Busca::Achado(m);
                    }
                } else if let Some(m) = inf.instance_access_to_static(cx.lib, r_ty, name.sym, false, name.span) {
                    // O estático recuperado é o alvo da invocação
                    // (`method_invocation_resolver.dart:841-857`): um getter
                    // vira `FunctionExpressionInvocation` do tipo de retorno.
                    search = Busca::Achado(m);
                }
            }
            match search {
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
                                inf.pending_numeric_context = Some(expr::numeric_context(inf, r_ty, &m, sym, ctx, pt));
                            }
                        }
                    }
                    let (mut r, _) = if m.metodo {
                        let t = inf.non_null(m.tipo);
                        prepare_entity(inf, name.span, m.funcao);
                        // `f.call(...)` com `f` de tipo função: a entidade do
                        // nomeado obrigatório é a lista de argumentos.
                        let r_nn = inf.non_null(r_ty);
                        let ent = if Some(name.sym) == inf.sym.call && matches!(inf.table.get(r_nn), Type::Function { .. }) {
                            args.span
                        } else {
                            name.span
                        };
                        { let t = inf.interner.resolve(name.sym).to_string(); define_target(inf, Some(t), ent); }
                        let r = invoke(inf, cx, t, args, ctx, explicit);
                        inf.inference_entity = None;
                        r
                    } else {
                        define_target(inf, None, span);
                        let read_value = expr::property_read_type(inf, cx, target, m.tipo);
                        let r = invoke_value(inf, cx, e, read_value, args, ctx, explicit, span);
                        inf.arity_target = None;
                        r
                    };
                    if m.metodo {
                        // Refinamento numérico de `remainder`/`clamp`.
                        if let Some(sym) = [inf.sym.remainder, inf.sym.clamp].into_iter().flatten().find(|s| *s == name.sym) {
                            let types: Vec<TypeId> = args
                                .args
                                .iter()
                                .map(|x| inf.body_types.units[cx.unit.0 as usize].get_type(x.value).unwrap_or(inf.core.dynamic_))
                                .collect();
                            r = expr::refine_numeric(inf, r_ty, &m, sym, &types, r);
                        }
                    }
                    (r, short_circuit)
                }
                // Invocação sobre `InvalidType`: tudo inválido, e os
                // argumentos são inferidos sem contexto
                // (`method_invocation_resolver.dart:459-463`).
                Busca::Dinamico if inf.table.e_invalido(r_ty) => {
                    resolver(inf, cx, target, Resolved::Dynamic);
                    registrar(inf, cx, target, r_ty);
                    for x in args.args.iter() {
                        infer_free(inf, cx, x.value);
                    }
                    (r_ty, short_circuit)
                }
                Busca::Dinamico => {
                    resolver(inf, cx, target, Resolved::Dynamic);
                    // Métodos de `Object` mantêm o tipo mesmo em `dynamic`.
                    let o = inf.core.object;
                    if let Some(m) = inf.interface_member(o, name.sym, false) {
                        if m.metodo {
                            registrar(inf, cx, target, m.tipo);
                            { let t = inf.interner.resolve(name.sym).to_string(); define_target(inf, Some(t), name.span); }
                            let (r, _) = invoke(inf, cx, m.tipo, args, ctx, explicit);
                            return (r, short_circuit);
                        }
                    }
                    let d = inf.core.dynamic_;
                    registrar(inf, cx, target, d);
                    for x in args.args.iter() {
                        infer_free(inf, cx, x.value);
                    }
                    (inf.core.dynamic_, short_circuit)
                }
                Busca::NeverType => {
                    let n = inf.core.never;
                    registrar(inf, cx, target, n);
                    for x in args.args.iter() {
                        infer_free(inf, cx, x.value);
                    }
                    (inf.core.never, short_circuit)
                }
                Busca::Missing if check_null => {
                    let d = inf.core.dynamic_;
                    registrar(inf, cx, target, d);
                    for x in args.args.iter() {
                        infer_free(inf, cx, x.value);
                    }
                    (d, short_circuit)
                }
                Busca::Missing => {
                    if let Some((x, _)) = cx.overlays.get(&recv).cloned() {
                        let extension = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                        let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_METHOD.template, inf.interner.resolve(name.sym), extension);
                        inf.aviso(msg, name.span);
                        let d = inf.core.dynamic_;
                        registrar(inf, cx, target, d);
                        for x in args.args.iter() {
                            infer_free(inf, cx, x.value);
                        }
                        return (d, short_circuit);
                    }
                    // `{1}` é o nome do elemento, não o tipo
                    // (`_resolveReceiverType`,
                    // an611:src/dart/resolver/method_invocation_resolver.dart:862-873).
                    let the_name = inf.interner.resolve(name.sym).to_string();
                    let typ = receiver_name(inf, r_ty);
                    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_METHOD, name.span, &[&the_name, &typ]);
                    // `_setInvalidTypeResolution`
                    // (`method_invocation_resolver.dart:1062-1068`).
                    let d = inf.table.invalid(inf.core.dynamic_);
                    registrar(inf, cx, target, d);
                    for x in args.args.iter() {
                        infer_free(inf, cx, x.value);
                    }
                    (d, short_circuit)
                }
            }
        }
        _ => {
            // `assert(…)` como expressão: `FunctionExpressionInvocation` do
            // identificador `assert`, lido como nome (`UNDEFINED_IDENTIFIER`)
            // e invocado como valor.
            let of_assert = a.invocacoes_de_assert.contains(&e);
            // `nome(args)` sem getter no escopo léxico: o erro é o da
            // invocação (`UNDEFINED_FUNCTION`/`UNDEFINED_METHOD`…), não o
            // de nome indefinido (ver `expr::invocacao_sem_alvo_indefinida`).
            if !of_assert
                && let ExprKind::Identifier(n) = &a.expr(target).kind
                && expr::undefined_targetless_invocation(inf, cx, *n)
            {
                let d = inf.table.invalid(inf.core.dynamic_);
                registrar(inf, cx, target, d);
                for x in args.args.iter() {
                    infer_free(inf, cx, x.value);
                }
                return (d, false);
            }
            // `T()` com `T` parâmetro de tipo (`_reportInvocationOfNonFunction`,
            // `method_invocation_resolver.dart:257-270`, de
            // `_resolveReceiverNull`): o elemento achado no escopo não é
            // executável nem variável. No nome; o resultado é inválido.
            if !of_assert
                && let ExprKind::Identifier(n) = &a.expr(target).kind
                && (matches!(cx.buscar(n.sym), Some(super::body::Nome::TypeParam(_)))
                    || (cx.buscar(n.sym).is_none() && expr::non_class_type_alias(inf, cx, target)))
            {
                let the_name = inf.interner.resolve(n.sym).to_string();
                inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::INVOCATION_OF_NON_FUNCTION, n.span, &[&the_name]);
                // O `methodName` fica com o parâmetro de tipo como elemento:
                // o `visitSimpleIdentifier` relata o da classe num membro
                // estático (`error_verifier.dart:1421`).
                if cx.static_member
                    && let Some(super::body::Nome::TypeParam(p)) = cx.buscar(n.sym)
                    && crate::resolve::class_param(inf.table, p)
                {
                    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::TYPE_PARAMETER_REFERENCED_BY_STATIC, n.span, &[]);
                }
                let d = inf.table.invalid(inf.core.dynamic_);
                registrar(inf, cx, target, d);
                for x in args.args.iter() {
                    infer_free(inf, cx, x.value);
                }
                return (d, false);
            }
            let t = inferir(inf, cx, target, u);
            if let Some((x, ext_args)) = cx.overlays.get(&target).cloned() {
                if let Some(call) = inf.sym.call {
                    if let Some(m) = inf.explicit_extension_member(x, &ext_args, call, false) {
                        define_target(inf, None, span);
                        let (r, _) = invoke(inf, cx, m.tipo, args, ctx, explicit);
                        return (r, false);
                    }
                    if let Some(m) = inf.extension_static_member(x, call, false) {
                        // `E(valor)()` usa a lista de argumentos como localização
                        // do erro, conforme o analyzer oficial.
                        inf.aviso(
                            EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template.to_string(),
                            args.span,
                        );
                        define_target(inf, None, span);
                        let (r, _) = invoke(inf, cx, m.tipo, args, ctx, explicit);
                        return (r, false);
                    }
                }
                let extension = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                let msg = format!("{}: '{}'", INVOCATION_OF_EXTENSION_WITHOUT_CALL.template, extension);
                inf.aviso(msg, a.expr(target).span);
                for arg in args.args.iter() {
                    infer_free(inf, cx, arg.value);
                }
                return (inf.core.dynamic_, false);
            }
            if !of_assert && let ExprKind::Identifier(n) = &a.expr(target).kind {
                let f = resolved_function(inf, cx, target);
                prepare_entity(inf, a.expr(target).span, f);
                // Função (de topo, método, local) é `MethodInvocation` (a
                // entidade é o nome); variável ou getter vira
                // `FunctionExpressionInvocation` (a chamada inteira). O nome
                // é o identificador nos dois casos.
                let function = match f {
                    Some(f) => matches!(inf.program.function(f).kind, FunctionKind::Function),
                    None => match cx.buscar(n.sym) {
                        Some(super::body::Nome::Local(id)) => local_is_function(inf, cx, cx.locais[id.0 as usize].offset),
                        _ => false,
                    },
                };
                let ent = if function { n.span } else { span };
                { let t = inf.interner.resolve(n.sym).to_string(); define_target(inf, Some(t), ent); }
            } else if let ExprKind::DotShorthand { name, .. } = &a.expr(target).kind
                && let Some(Resolved::Element(Element::Class(d))) = inf.body_types.units[cx.unit.0 as usize].get_resolved(target).cloned()
                && let Some(&f) = inf.program.class(d).static_members.get(&name.sym)
                && matches!(inf.program.function(f).kind, FunctionKind::Function)
            {
                // `.m(…)` de método estático é `DotShorthandInvocation`, com o
                // nome do método e a entidade no nome.
                let (the_name, sp) = (inf.interner.resolve(name.sym).to_string(), name.span);
                define_target(inf, Some(the_name), sp);
            } else {
                define_target(inf, None, span);
            }
            let (r, _) = invoke_value(inf, cx, e, t, args, ctx, explicit, span);
            inf.inference_entity = None;
            inf.arity_target = None;
            (r, false)
        }
    }
}

/// Alvo da aridade de `x.m(...)`/`p.f(...)`/`super.m(...)`: um método (ou
/// função) é `MethodInvocation` (nome e entidade no `m`); um getter ou
/// variável vira `FunctionExpressionInvocation` sobre o acesso (sem nome,
/// a chamada inteira).
fn method_target(inf: &mut BodyInferrer<'_>, f: Option<FunctionElementId>, name: ast::Name, span: Span) {
    if f.is_some_and(|f| matches!(inf.program.function(f).kind, FunctionKind::Function)) {
        { let t = inf.interner.resolve(name.sym).to_string(); define_target(inf, Some(t), name.span); }
    } else {
        define_target(inf, None, span);
    }
}

/// O `{1}` do `UNDEFINED_METHOD` de instância: o nome do elemento do tipo
/// do receptor (sem argumentos de tipo), `'Function'` para tipos de função
/// e `'<unknown>'` para o resto (records, parâmetros de tipo).
pub(crate) fn receiver_name(inf: &BodyInferrer<'_>, t: TypeId) -> String {
    match inf.table.get(t) {
        Type::Interface { class, .. } => inf.interner.resolve(inf.program.class(*class).name).to_string(),
        Type::ExtensionType { decl, .. } => inf.interner.resolve(inf.program.class(*decl).name).to_string(),
        Type::FutureOr { .. } => "FutureOr".to_string(),
        Type::Null => "Null".to_string(),
        Type::Function { .. } => "Function".to_string(),
        _ => "<unknown>".to_string(),
    }
}

/// O local declarado em `offset` é uma função local (o nome segue de `(`
/// ou `<` na declaração).
fn local_is_function(inf: &BodyInferrer<'_>, cx: &Body, offset: usize) -> bool {
    let source = &inf.program.unit(cx.unit).source;
    let rest = source.get(offset..).unwrap_or("");
    let after = rest.trim_start_matches(|c: char| c.is_alphanumeric() || c == '_' || c == '$').trim_start();
    after.starts_with('(') || after.starts_with('<')
}

/// O último identificador de uma referência a tipo (`C`, `p.C`, `C<int>`).
fn last_identifier(inf: &BodyInferrer<'_>, cx: &Body, e: ExprId) -> Option<dartforge_intern::SymbolId> {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.expr(e).kind {
        ExprKind::Identifier(n) => Some(n.sym),
        ExprKind::Property { name, .. } => Some(name.sym),
        ExprKind::TypeArguments { target, .. } => last_identifier(inf, cx, *target),
        _ => None,
    }
}

/// Se `target` (alvo de uma chamada) nomeia um construtor: `(classe, construtor, args explícitos)`.
fn target_constructor(inf: &mut BodyInferrer<'_>, cx: &mut Body, target: ExprId) -> Option<(ClassId, Option<FunctionElementId>, Option<Vec<TypeId>>)> {
    let empty = inf.sym.vazio?;
    let the_type_args = |inf: &mut BodyInferrer<'_>, cx: &Body, rt: &TypeRef| -> (ClassId, Option<Vec<TypeId>>) {
        match rt {
            TypeRef::Classe(c, Some(ts)) => (*c, Some(ts.iter().map(|&t| inf.type_argument_type(cx, t)).collect())),
            TypeRef::Classe(c, None) => (*c, None),
            TypeRef::Alias(c, args, _) => (*c, args.clone()),
            TypeRef::Extensao(_) => unreachable!(),
        }
    };
    if let Some(rt) = type_reference(inf, cx, target) {
        if matches!(rt, TypeRef::Extensao(_)) {
            return None;
        }
        let (c, args) = the_type_args(inf, cx, &rt);
        let f = inf.constructor_or_primary(c, empty)?;
        record_reference(inf, cx, target);
        return Some((c, f, args));
    }
    let a = &inf.program.unit(cx.unit).ast;
    if let ExprKind::Property { target: recv, name, null_aware: false } = &a.expr(target).kind {
        let (recv, name) = (*recv, *name);
        let rt = type_reference(inf, cx, recv)?;
        if matches!(rt, TypeRef::Extensao(_)) {
            return None;
        }
        let (c, args) = the_type_args(inf, cx, &rt);
        let key = if Some(name.sym) == inf.sym.new_ { empty } else { name.sym };
        let f = inf.visible_constructor(c, key, cx.lib)?;
        record_reference(inf, cx, recv);
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
fn record_reference(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) {
    let a = &inf.program.unit(cx.unit).ast;
    let tt = inf.core.type_;
    match &a.expr(e).kind {
        ExprKind::Identifier(n) => {
            if let Some(el) = inf.program.lookup_na_unidade(cx.unit, n.sym).and_then(|b| b.getter) {
                resolver(inf, cx, e, Resolved::Element(el));
            }
            registrar(inf, cx, e, tt);
        }
        ExprKind::Property { target, name, .. } => {
            let (target, name) = (*target, *name);
            if let ExprKind::Identifier(p) = &a.expr(target).kind {
                resolver(inf, cx, target, Resolved::Prefix(cx.lib));
                if let Some(el) = inf.program.lookup_prefixed_na_unidade(cx.unit, p.sym, name.sym).and_then(|b| b.getter) {
                    resolver(inf, cx, e, Resolved::Element(el));
                }
            }
            registrar(inf, cx, e, tt);
        }
        ExprKind::TypeArguments { target, .. } => {
            let t = *target;
            record_reference(inf, cx, t);
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
    pub(crate) fn construtor_de(&self, c: ClassId, the_name: dartforge_intern::SymbolId) -> Option<FunctionElementId> {
        let cl = self.program.class(c);
        if let Some(&f) = cl.constructors.get(&the_name) {
            return Some(f);
        }
        if cl.kind != ClassKind::MixinApplication {
            return None;
        }
        let f = self.construtor_de(cl.supertype_class?, the_name)?;
        (!self.program.function(f).factory).then_some(f)
    }

    /// Construtor `nome` de `c`, ou `Some(None)` para o construtor primário
    /// de um tipo de extensão (`extension type Id(int v)`, R-EXT-04), que o
    /// modelo de elementos não cria.
    pub(crate) fn constructor_or_primary(&self, c: ClassId, the_name: dartforge_intern::SymbolId) -> Option<Option<FunctionElementId>> {
        if let Some(f) = self.construtor_de(c, the_name) {
            return Some(Some(f));
        }
        let cl = self.program.class(c);
        if cl.kind != ClassKind::ExtensionType {
            return None;
        }
        let d = cl.decl?;
        match &self.program.unit(d.unit).ast.decl(d.decl).kind {
            ast::DeclKind::ExtensionType(et) if et.constructor.map(|n| n.sym).or(self.sym.vazio) == Some(the_name) => Some(None),
            _ => None,
        }
    }

    /// [`Self::construtor_ou_primario`] pelo `lookUpConstructor(nome, library)`
    /// do analyzer: o construtor privado de outra biblioteca não é achado
    /// (`isAccessibleIn`).
    pub(crate) fn visible_constructor(&self, c: ClassId, the_name: dartforge_intern::SymbolId, lib: dartforge_elements::model::LibraryId) -> Option<Option<FunctionElementId>> {
        if self.interner.resolve(the_name).starts_with('_') && self.program.class(c).library != lib {
            return None;
        }
        self.constructor_or_primary(c, the_name)
    }

    /// `(Representação) -> E<parâmetros>`: o construtor primário.
    pub(crate) fn primary_signature(&mut self, c: ClassId) -> TypeId {
        let rep = self.program.class(c).representation;
        let t = rep
            .and_then(|v| self.outline.variables[v.0 as usize].declared_type)
            .unwrap_or(self.core.dynamic_);
        let this = self.class_this_type(c);
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
    pub(crate) fn constructor_signature(&mut self, c: ClassId, f: FunctionElementId) -> TypeId {
        let sig = self.outline.functions[f.0 as usize].signature;
        if self.program.function(f).class == Some(c) {
            return sig;
        }
        let Some(sup) = self.outline.classes[c.0 as usize].supertype else { return sig };
        let Type::Interface { class: sc, args, .. } = self.table.get(sup).clone() else { return sig };
        let s = self.constructor_signature(sc, f);
        let params = self.outline.classes[sc.0 as usize].type_params.clone();
        let s = if params.len() == args.len() {
            let map = self.mapa(&params, &args);
            self.subst(s, &map)
        } else {
            s
        };
        let this = self.class_this_type(c);
        match self.table.get(s).clone() {
            Type::Function { type_params, positional, optional, named, nullable, .. } => {
                self.table.intern(Type::Function { type_params, ret: this, positional, optional, named, nullable })
            }
            _ => s,
        }
    }

    fn constructor_parameters(&mut self, c: ClassId) -> (Vec<TypeParamId>, Vec<TypeParamId>) {
        let original = self.outline.classes[c.0 as usize].type_params.to_vec();
        let new_ = self.new_parameters(&original);
        (original, new_)
    }
}

/// Invocação de construtor: infere os argumentos de tipo da classe quando
/// omitidos (como uma função genérica sobre os parâmetros da classe).
#[allow(clippy::too_many_arguments)]
pub(crate) fn construir(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: Option<ExprId>,
    c: ClassId,
    f: Option<FunctionElementId>,
    explicit: Option<Vec<TypeId>>,
    args: &ast::Arguments,
    ctx: TypeId,
) -> TypeId {
    build_with(inf, cx, e, c, f, explicit, args, ctx, None)
}

/// O alias genérico nomeado na criação (`target`: `A`, `p.A`, `A.nome`,
/// `p.A.nome`) sem argumentos de tipo escritos, cujos argumentos se inferem
/// pelos parâmetros dele ([`alias_inferivel`]).
fn creation_alias(inf: &mut BodyInferrer<'_>, cx: &Body, target: ExprId) -> Option<dartforge_elements::model::TypedefId> {
    let td = match type_reference(inf, cx, target) {
        Some(TypeRef::Alias(_, _, td)) => td,
        Some(_) => return None,
        None => {
            let a = &inf.program.unit(cx.unit).ast;
            let ExprKind::Property { target: t, null_aware: false, .. } = &a.expr(target).kind else { return None };
            let t = *t;
            if matches!(a.expr(t).kind, ExprKind::TypeArguments { .. }) {
                return None;
            }
            match type_reference(inf, cx, t) {
                Some(TypeRef::Alias(_, _, td)) => td,
                _ => return None,
            }
        }
    };
    inferable_alias(inf, td).then_some(td)
}

/// Alias genérico de classe que não só renomeia os parâmetros (`typedef
/// D<X> = C<List<X>>`, ou `typedef A<X extends B> = C<X>` com o limite
/// diferente do da classe): sem argumentos escritos, a criação por ele é
/// inferida como função genérica nos parâmetros do alias
/// (`ConstructorElementToInfer`, 3.6.2 `invocation_inferrer.dart`), com os
/// limites deles (`COULD_NOT_INFER`) — não instanciada para os limites.
pub(crate) fn inferable_alias(inf: &mut BodyInferrer<'_>, td: dartforge_elements::model::TypedefId) -> bool {
    let d = &inf.outline.typedefs[td.0 as usize];
    if d.type_params.is_empty() {
        return false;
    }
    let params = d.type_params.to_vec();
    let (class, args) = match inf.table.get(d.target_type) {
        Type::Interface { class, args, .. } | Type::ExtensionType { decl: class, args, .. } => (*class, args.to_vec()),
        _ => return false,
    };
    let renames = args.len() == params.len()
        && args.iter().zip(params.iter()).all(|(&a, &p)| matches!(inf.table.get(a), Type::TypeParameter { param, nullable: false } if *param == p));
    if !renames {
        return true;
    }
    // Renomeia: equivale à classe só com os mesmos limites.
    let formals = inf.outline.classes[class.0 as usize].type_params.to_vec();
    let map = inf.mapa(&formals, &args);
    formals.iter().zip(params.iter()).any(|(&f, &p)| {
        let bf = inf.table.param(f).bound;
        let bf = inf.subst(bf, &map);
        bf != inf.table.param(p).bound
    })
}

/// [`construir`] com o alias inferível ([`alias_inferivel`]) pelo qual a
/// classe foi nomeada: os parâmetros inferidos são os do alias e a
/// assinatura, a do construtor com os argumentos do alvo do alias.
#[allow(clippy::too_many_arguments)]
fn build_with(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: Option<ExprId>,
    c: ClassId,
    f: Option<FunctionElementId>,
    explicit: Option<Vec<TypeId>>,
    args: &ast::Arguments,
    ctx: TypeId,
    alias: Option<dartforge_elements::model::TypedefId>,
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
            inf.constructor_signature(c, f)
        }
        None => inf.primary_signature(c),
    };
    let alias = alias.filter(|_| explicit.is_none());
    let (original, new_, sig) = match alias {
        Some(td) => {
            let d = &inf.outline.typedefs[td.0 as usize];
            let (params, tgt) = (d.type_params.to_vec(), d.target_type);
            let target_args = match inf.table.get(tgt).clone() {
                Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.to_vec(),
                _ => Vec::new(),
            };
            let formals = inf.outline.classes[c.0 as usize].type_params.to_vec();
            let map = inf.mapa(&formals, &target_args);
            let sig = inf.subst(sig, &map);
            let new_ = inf.new_parameters(&params);
            (params, new_, sig)
        }
        None => {
            let (o, n) = inf.constructor_parameters(c);
            (o, n, sig)
        }
    };
    if original.is_empty() {
        let (r, _) = invoke(inf, cx, sig, args, ctx, None);
        return r;
    }
    if let Some(ex) = explicit.filter(|x| x.len() == original.len()) {
        inf.body_types.units[cx.unit.0 as usize].set_instantiation(args.span.start, ex.clone().into_boxed_slice());
        let map = inf.mapa(&original, &ex);
        let s = inf.subst(sig, &map);
        let (r, _) = invoke(inf, cx, s, args, ctx, None);
        return r;
    }
    // Função genérica `<T'..>(params) -> C<T'..>`.
    let types: Vec<TypeId> = new_.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
    let map = inf.mapa(&original, &types);
    let s = inf.subst(sig, &map);
    let Type::Function { ret, positional, optional, named, .. } = inf.table.get(s).clone() else { return inf.core.dynamic_ };
    let generic = inf.table.intern(Type::Function {
        type_params: new_.into_boxed_slice(),
        ret,
        positional,
        optional,
        named,
        nullable: false,
    });
    // Contexto `C<...>?` ou supertipo: a restrição do retorno cuida.
    if let Some(e) = e {
        let a = &inf.program.unit(cx.unit).ast;
        let sp = match &a.expr(e).kind {
            ExprKind::Call { target, .. } => a.expr(*target).span,
            ExprKind::InstanceCreation { ty, constructor, .. } => {
                let t = a.ty(*ty).span;
                Span { start: t.start, end: constructor.map(|n| n.span.end).unwrap_or(t.end) }
            }
            _ => a.expr(e).span,
        };
        prepare_entity(inf, sp, f);
    }
    let (r, _) = invoke(inf, cx, generic, args, ctx, None);
    inf.inference_entity = None;
    // A instanciação gravada pelo `invocar` é a dos parâmetros do alias; os
    // consumidores a leem como argumentos da classe.
    if alias.is_some()
        && let Type::Interface { class, args: ca, .. } | Type::ExtensionType { decl: class, args: ca, .. } = inf.table.get(r).clone()
        && class == c
    {
        inf.body_types.units[cx.unit.0 as usize].set_instantiation(args.span.start, ca.to_vec().into_boxed_slice());
    }
    r
}

/// O `AstRewriter.instanceCreationExpression` (3.6.2 `ast_rewrite.dart:38-118`)
/// sobre a criação implícita `X<T>.nome(…)`: fica criação salvo quando o `X`
/// sem prefixo é função, método ou acessor no escopo (vira invocação da
/// referência), ou quando o `p.X` tem `p` que não é prefixo de import, ou `X`
/// função no prefixo. (O alias de tipo de função o chamador já separou.)
fn stays_implicit_creation(inf: &mut BodyInferrer<'_>, cx: &Body, base: ExprId) -> bool {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.expr(base).kind {
        ExprKind::Identifier(n) => match resolve_name(inf, cx, n.sym, false) {
            NameRef::LexicalMember(..) => false,
            NameRef::Elemento(Element::Function(_)) => false,
            NameRef::Local(id) => {
                let l = cx.local(id);
                !(l.funcao_local || cx.funcoes_locais.contains(&id))
            }
            _ => true,
        },
        ExprKind::Property { target: p, name, .. } => {
            let ExprKind::Identifier(p) = &a.expr(*p).kind else { return false };
            if !matches!(resolve_name(inf, cx, p.sym, false), NameRef::Prefixo) {
                return false;
            }
            !matches!(
                inf.program.lookup_prefixed_na_unidade(cx.unit, p.sym, name.sym).and_then(|b| b.getter),
                Some(Element::Function(f)) if inf.program.function(f).kind == FunctionKind::Function
            )
        }
        _ => false,
    }
}

/// `CREATION_WITH_NON_TYPE` (`new X()`, `const X()`, `X<T>.nome()`): o nome
/// escrito como tipo da criação não é classe — indefinido, local, parâmetro
/// de tipo, função. O intervalo é o nome escrito; com `p.X` e `p` prefixo de
/// import, só o `X`. O argumento é a última parte do nome (`new A.foo()` com
/// `A` indefinido: `'foo'`).
///
/// Ficam de fora (o analyzer relata outro código ou nada):
/// * `p.X` com `p` declarado mas não prefixo (`void p() {}`):
///   `prefix_shadowed_by_local_declaration`;
/// * `p.X` com `p` prefixo de um import que não resolveu: nada.
fn creation_without_class(inf: &mut BodyInferrer<'_>, cx: &Body, name: &[ast::Name]) {
    let (Some(first), Some(last)) = (name.first(), name.last()) else { return };
    let mut prefix = false;
    if name.len() == 2 {
        if inf.program.prefixos_na_unidade(cx.unit).contains_key(&first.sym) {
            // `shouldIgnoreUndefinedNamedType`
            // (`an611:src/dart/resolver/named_type_resolver.dart:310`): o
            // prefixo de um import que não existe.
            if crate::scope::should_ignore_undefined(inf.program, inf.interner, cx.unit, Some(first.sym), last.sym) {
                return;
            }
            prefix = true;
        } else if inf.program.lookup_na_unidade(cx.unit, first.sym).is_some_and(|b| {
            !matches!(b.getter, Some(Element::Class(_)))
        }) || unresolved_import_prefix(inf, cx, first.sym)
        {
            return;
        }
    }
    if name.len() == 1
        && inf.program.lookup_na_unidade(cx.unit, first.sym).is_none()
        && crate::scope::should_ignore_undefined(inf.program, inf.interner, cx.unit, None, first.sym)
    {
        return;
    }
    let start = if prefix { last.span.start } else { first.span.start };
    let text = inf.interner.resolve(last.sym).to_string();
    inf.warning_with_code(
        dartforge_diagnostics::codigos::compile_time_error::NEW_WITH_NON_TYPE,
        Span { start: start, end: last.span.end },
        &[&text],
    );
    // O `NamedType` da criação resolve para o parâmetro de tipo: o
    // `visitNamedType` do `ErrorVerifier` relata o da classe num membro
    // estático (`error_verifier.dart:1266-1275`, `:5419-5434`).
    if name.len() == 1
        && cx.static_member
        && let Some(&p) = cx.visible_type_parameters().get(&first.sym)
        && crate::resolve::class_param(inf.table, p)
    {
        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::TYPE_PARAMETER_REFERENCED_BY_STATIC, first.span, &[]);
    }
}

/// `p` é o prefixo de alguma diretiva `import … as p` da biblioteca, mas
/// não chegou ao escopo (o alvo do import não existe).
fn unresolved_import_prefix(inf: &BodyInferrer<'_>, cx: &Body, p: dartforge_intern::SymbolId) -> bool {
    let lib = inf.program.library(cx.lib);
    !inf.program.prefixos_na_unidade(cx.unit).contains_key(&p)
        && lib.units.iter().any(|u| {
            inf.program.unit(*u).unit.directives.iter().any(|d| {
                matches!(&d.kind, ast::DirectiveKind::Import { prefix: Some(n), .. } if n.sym == p)
            })
        })
}

/// `_checkForConstWithUndefinedConstructor` (`error_verifier.dart:3020-3045`):
/// `const C.nome()` sem o construtor é `CONST_WITH_UNDEFINED_CONSTRUCTOR` no
/// nome (com o nome qualificado escrito e o do construtor); `const C()` sem
/// o sem nome, `CONST_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT` no tipo nomeado
/// inteiro.
fn undefined_const_constructor(
    inf: &mut BodyInferrer<'_>,
    a: &ast::Ast,
    ty: ast::TypeId,
    name: &[ast::Name],
    constructor: Option<ast::Name>,
    ctor_of_name: bool,
) {
    undefined_constructor(inf, a, ty, name, constructor, ctor_of_name, true);
}

/// `_checkForConstWithUndefinedConstructor` e
/// `_checkForNewWithUndefinedConstructor` (`error_verifier.dart:4595-4625`):
/// no nome do construtor (`[namedType.qualifiedName, nome]`), ou, sem nome,
/// no `constructorName` inteiro, que é o tipo.
fn undefined_constructor(
    inf: &mut BodyInferrer<'_>,
    a: &ast::Ast,
    ty: ast::TypeId,
    name: &[ast::Name],
    constructor: Option<ast::Name>,
    ctor_of_name: bool,
    constant: bool,
) {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    let parts = if ctor_of_name { &name[..1] } else { name };
    let qualified = parts.iter().map(|n| inf.interner.resolve(n.sym)).collect::<Vec<_>>().join(".");
    match constructor {
        Some(n) => {
            let the_name = inf.interner.resolve(n.sym).to_string();
            let code = if constant { c::CONST_WITH_UNDEFINED_CONSTRUCTOR } else { c::NEW_WITH_UNDEFINED_CONSTRUCTOR };
            inf.warning_with_code(code, n.span, &[&qualified, &the_name]);
        }
        None => {
            let sp = a.ty(ty).span;
            let code = if constant { c::CONST_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT } else { c::NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT };
            inf.warning_with_code(code, sp, &[&qualified]);
        }
    }
}

/// `new C<T>.nome(args)` / `const C(args)`.
pub(crate) fn instanciacao(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, ctx: TypeId) -> TypeId {
    let a = &inf.program.unit(cx.unit).ast;
    let ExprKind::InstanceCreation { ty, constructor, arguments, keyword } = &a.expr(e).kind else { unreachable!() };
    let (ty, constructor, args): (ast::TypeId, Option<ast::Name>, &ast::Arguments) = (*ty, *constructor, arguments);
    let constant = matches!(keyword, Some(ast::CreationKeyword::Const));
    let node = a.ty(ty);
    let ast::TypeKind::Named { name, args: targs } = &node.kind else {
        for x in args.args.iter() {
            infer_free(inf, cx, x.value);
        }
        return inf.core.dynamic_;
    };
    // `NamedTypeResolver.resolve` (`named_type_resolver.dart:89-121`): o
    // primeiro nome de `p.N` que, no escopo, não é prefixo, classe nem alias
    // (um local, um parâmetro de tipo, um membro, uma função de topo).
    if name.len() == 2 && name[0].span.start != name[0].span.end {
        let p = name[0].sym;
        let local = cx.visible_type_parameters().contains_key(&p) || cx.visible_locals(inf.interner).contains_key(&p);
        if crate::resolve::shadowed_prefix(inf.program, inf.interner, cx.unit, p, local, cx.classe, cx.extensao) {
            let the_name = inf.interner.resolve(p).to_string();
            inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::PREFIX_SHADOWED_BY_LOCAL_DECLARATION, name[0].span, &[&the_name]);
            for x in args.args.iter() {
                infer_free(inf, cx, x.value);
            }
            return inf.table.invalid(inf.core.dynamic_);
        }
    }
    // O nome simples é procurado no escopo léxico (`nameScope`): um local, um
    // parâmetro de tipo ou um membro da classe ou extensão (`const B()` com o
    // método `B`) esconde o tipo de topo, e a criação é de um não tipo
    // (`_isInstanceCreation`, `named_type_resolver.dart:274-276`).
    if name.len() == 1
        && matches!(
            resolve_name(inf, cx, name[0].sym, false),
            NameRef::Local(_) | NameRef::TypeParam(_) | NameRef::LexicalMember(..) | NameRef::ConstantEnum(_)
        )
    {
        creation_without_class(inf, cx, name);
        for &x in targs.iter() {
            inf.type_argument_type(cx, x);
        }
        for x in args.args.iter() {
            infer_free(inf, cx, x.value);
        }
        return inf.table.invalid(inf.core.dynamic_);
    }
    let binding = if name.len() == 2 {
        inf.program.lookup_prefixed_na_unidade(cx.unit, name[0].sym, name[1].sym)
    } else {
        inf.program.lookup_na_unidade(cx.unit, name[0].sym)
    };
    // `new C.nome()` chega como tipo de duas partes quando `C` não é prefixo.
    let (binding, constructor, ctor_of_name) = match (binding, name.len()) {
        (None, 2) => match inf.program.lookup_na_unidade(cx.unit, name[0].sym) {
            Some(b) if matches!(b.getter, Some(Element::Class(_))) && constructor.is_none() => (Some(b), Some(name[1]), true),
            _ => (binding, constructor, false),
        },
        _ => (binding, constructor, false),
    };
    // `_verifyTypeAliasForContext` (`named_type_resolver.dart:420-450`): o
    // alias cujo tipo é um parâmetro de tipo (`typedef A<T> = T`, também por
    // outro alias) não se instancia; no nome, sem os argumentos.
    {
        let parameter_alias = |inf: &BodyInferrer<'_>, el: Option<Element>| match el {
            Some(Element::Typedef(td)) => matches!(inf.table.get(inf.outline.typedefs[td.0 as usize].target_type), Type::TypeParameter { .. }),
            _ => false,
        };
        let range = if parameter_alias(inf, binding.and_then(|b| b.getter)) {
            name.first().zip(name.last()).map(|(p, n)| Span { start: p.span.start, end: n.span.end })
        } else if binding.is_none() && name.len() == 2 && parameter_alias(inf, inf.program.lookup_na_unidade(cx.unit, name[0].sym).and_then(|b| b.getter)) {
            Some(name[0].span)
        } else {
            None
        };
        if let Some(sp) = range {
            inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::INSTANTIATE_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER, sp, &[]);
            for x in args.args.iter() {
                infer_free(inf, cx, x.value);
            }
            return inf.core.dynamic_;
        }
    }
    let mut explicit_alias = None;
    let (c, explicit) = match binding.and_then(|b| b.getter) {
        // `new C.nome<T>()`: lido como `prefixo.Tipo<T>`; a lista é do
        // construtor (`_rewriteToConstructorName`).
        Some(Element::Class(c)) if ctor_of_name && !targs.is_empty() => {
            if let Some(n) = constructor {
                types_in_constructor(inf, cx.unit, name[0].span, n, targs);
            }
            (c, None)
        }
        Some(Element::Class(c)) => {
            let mut ex = if targs.is_empty() { None } else { Some(targs.iter().map(|&t| inf.type_argument_type(cx, t)).collect::<Vec<_>>()) };
            // `NamedTypeResolver._buildTypeArguments`
            // (`named_type_resolver.dart:137-148`): contagem errada no tipo
            // nomeado inteiro, e os argumentos viram `InvalidType`.
            let n_params = inf.outline.classes[c.0 as usize].type_params.len();
            if !targs.is_empty() && targs.len() != n_params {
                let text = inf.interner.resolve(name[name.len() - 1].sym).to_string();
                inf.report_type_arguments(&text, n_params, targs.len(), node.span);
                ex = Some(vec![inf.table.invalid(inf.core.dynamic_); n_params]);
            }
            (c, ex)
        }
        Some(Element::Typedef(td)) => {
            let t = inf.type_from_annotation(cx, ty);
            match inf.table.get(t).clone() {
                Type::Interface { class, args, .. } | Type::ExtensionType { decl: class, args, .. } => {
                    if targs.is_empty() && inferable_alias(inf, td) {
                        explicit_alias = Some(td);
                    }
                    (class, Some(args.to_vec()))
                }
                ref other => {
                    // `typedef F = void Function()`: o alias não nomeia
                    // classe. Outros alvos (`typedef T = dynamic`, `typedef
                    // A<X> = X`, que é `instantiate_type_alias_expands_…`)
                    // ficam mudos.
                    if matches!(other, Type::Function { .. }) {
                        creation_without_class(inf, cx, name);
                    }
                    for x in args.args.iter() {
                        infer_free(inf, cx, x.value);
                    }
                    return inf.core.dynamic_;
                }
            }
        }
        _ => {
            if !binding.is_some_and(|b| b.ambiguous) {
                creation_without_class(inf, cx, name);
            }
            for x in args.args.iter() {
                infer_free(inf, cx, x.value);
            }
            return inf.core.dynamic_;
        }
    };
    let key = match constructor {
        Some(n) if Some(n.sym) != inf.sym.new_ => Some(n.sym),
        _ => inf.sym.vazio,
    };
    let f = key.and_then(|k| inf.visible_constructor(c, k, cx.lib));
    // `new E()` / `const E()`: o construtor gerador do enum (ou o sem nome
    // implícito) não instancia fora da criação de constantes. Nome
    // explícito sem alvo (`const E.foo()`) é outro diagnóstico
    // (`const_with_undefined_constructor`, fora do escopo) e segue mudo.
    // Factories seguem o caminho normal (`construir`). O intervalo é o nome
    // do construtor, ou o da classe quando ele é implícito.
    // `new M()` / `const M()` com `M` mixin (`error_verifier.dart:2976-2984`):
    // no tipo nomeado; o construtor que o mixin não tem fica mudo.
    if inf.program.class(c).kind == ClassKind::Mixin {
        let sp = if constructor.is_some() && ctor_of_name { name[0].span } else { a.ty(ty).span };
        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::MIXIN_INSTANTIATE, sp, &[]);
        if constant {
            undefined_const_constructor(inf, a, ty, name, constructor, ctor_of_name);
        }
        for x in args.args.iter() {
            infer_free(inf, cx, x.value);
        }
        return inf.class_type_with_args(c, explicit.unwrap_or_default());
    }
    let generator = f.is_some_and(|f| f.is_some_and(|f| !inf.program.function(f).factory));
    if inf.program.class(c).kind == ClassKind::Enum && (generator || (constructor.is_none() && f.is_none()))
    {
        let span = constructor.map(|n| n.span).unwrap_or_else(|| name.last().map(|n| n.span).unwrap_or(a.expr(e).span));
        inf.aviso(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template.to_string(), span);
        for x in args.args.iter() {
            infer_free(inf, cx, x.value);
        }
        return inf.core.dynamic_;
    }
    let Some(f) = f else {
        // Sem construtor declarado, o sem nome implícito existe (o elemento
        // sintético): nada a relatar por ele.
        // Numa aplicação de mixin os construtores são os geradores
        // encaminhados da superclasse: o sem nome implícito só existe se a
        // superclasse (recursivamente) não declara construtor.
        let mut owner = c;
        let mut rounds = 0;
        while inf.program.class(owner).kind == ClassKind::MixinApplication
            && inf.program.class(owner).constructors.is_empty()
            && let Some(s) = inf.program.class(owner).supertype_class
            && rounds < 32
        {
            owner = s;
            rounds += 1;
        }
        let implicit = constructor.is_none() && inf.program.class(owner).constructors.is_empty();
        if !implicit {
            undefined_constructor(inf, a, ty, name, constructor, ctor_of_name, constant);
        }
        // Sem construtor declarado: o padrão implícito (gerador).
        if constructor.is_none() && inf.program.class(c).constructors.is_empty() {
            let k = inf.program.class(c);
            if matches!(k.kind, ClassKind::Class | ClassKind::MixinApplication) && (k.modifiers.abstract_ || k.modifiers.sealed) {
                let sp = a.ty(ty).span;
                inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::INSTANTIATE_ABSTRACT_CLASS, sp, &[]);
            }
        }
        for x in args.args.iter() {
            infer_free(inf, cx, x.value);
        }
        return inf.class_type_with_args(c, explicit.unwrap_or_default());
    };
    if let Some(fid) = f {
        // O tipo nomeado: sem o nome do construtor que veio como 2ª parte.
        let sp = if constructor.is_some() && ctor_of_name {
            name[0].span
        } else {
            a.ty(ty).span
        };
        warn_abstract_class(inf, c, fid, sp);
    }
    // `constructorName`: o tipo (com prefixo e argumentos) e o `.nome`; o
    // nome citado é o do construtor, ou `C.new` (sem o prefixo).
    {
        let cls = if ctor_of_name { name[0] } else { *name.last().unwrap_or(&name[0]) };
        let the_name = match constructor {
            Some(n) => inf.interner.resolve(n.sym).to_string(),
            None => format!("{}.new", inf.interner.resolve(cls.sym)),
        };
        let t = a.ty(ty).span;
        let ent = Span { start: t.start, end: constructor.map(|n| n.span.end).unwrap_or(t.end).max(t.end) };
        define_target(inf, Some(the_name), ent);
    }
    let explicit = if explicit_alias.is_some() { None } else { explicit };
    let r = build_with(inf, cx, Some(e), c, f, explicit, args, ctx, explicit_alias);
    inf.arity_target = None;
    r
}

/// O intervalo do tipo nomeado de uma criação implícita: `A`, `p.A` ou
/// `A<int>` (até o `>`).
fn creation_named_type(inf: &BodyInferrer<'_>, cx: &Body, tgt: ExprId, targs: &[ast::TypeId]) -> Span {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.expr(tgt).kind {
        ExprKind::TypeArguments { target, type_args } => {
            let begin = a.expr(*target).span.start;
            let end = type_args.last().map(|&t| a.ty(t).span.end).unwrap_or(a.expr(*target).span.end);
            let source = &inf.program.unit(cx.unit).source;
            let rest = source.get(end..).unwrap_or("");
            let closes = rest.find('>').map(|i| end + i + 1).unwrap_or(end);
            Span { start: begin, end: closes }
        }
        _ => {
            let sp = a.expr(tgt).span;
            // `A<int>()`: os argumentos de tipo vêm na lista da chamada.
            match targs.last() {
                Some(&t) => {
                    let end = a.ty(t).span.end;
                    let rest = inf.program.unit(cx.unit).source.get(end..).unwrap_or("");
                    let closes = rest.find('>').map(|i| end + i + 1).unwrap_or(end);
                    Span { start: sp.start, end: closes }
                }
                None => sp,
            }
        }
    }
}

/// `INSTANTIATE_ABSTRACT_CLASS` (`_checkForConstOrNewWithAbstractClass`,
/// `an611:src/generated/error_verifier.dart:2950-2973`): criação de uma
/// classe abstrata (ou `sealed`) por um construtor que não é `factory`, no
/// tipo nomeado.
fn warn_abstract_class(inf: &mut BodyInferrer<'_>, c: ClassId, f: FunctionElementId, span: Span) {
    let k = inf.program.class(c);
    if !matches!(k.kind, ClassKind::Class | ClassKind::MixinApplication) || !(k.modifiers.abstract_ || k.modifiers.sealed) {
        return;
    }
    if inf.program.function(f).factory {
        return;
    }
    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::INSTANTIATE_ABSTRACT_CLASS, span, &[]);
}
