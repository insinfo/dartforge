//! Invocations: functions, methods, constructors, `call`, and type argument
//! inference with "horizontal" inference (`inference-update-1`): function
//! literals whose parameters depend on variables not yet resolved are
//! inferred after the other arguments.

use super::body::Body;
use super::expr::{self, infer, infer_free, the_receiver, type_reference, record_type, resolver, resolve_name, NameRef, TypeRef};
use super::members::Search;
use super::BodyInferrer;
use crate::codes::*;
use crate::constraints::{instantiate_function, GenericInferrer};
use crate::resolved::Resolved;
use crate::table::{Type, TypeId, TypeParamId};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionElementId, FunctionKind};
use dartforge_frontend::ast::{self, ExprId, ExprKind};
use std::collections::HashMap;

/// Formal parameter corresponding to each argument.
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

/// What the arity check needs to know about the call (the analyzer's
/// `nameNode` and `errorEntity`), set by whoever knows its shape.
#[derive(Debug, Clone, Default)]
pub(crate) struct ArityTarget {
    /// Name cited in `NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_*`
    /// (`_reportNotEnoughPositionalArguments`,
    /// an611:src/generated/resolver.dart:4370-4420); `None` uses the unnamed
    /// variants.
    pub name: Option<String>,
    /// Range of `MISSING_REQUIRED_ARGUMENT`
    /// (an611:src/error/required_parameters_verifier.dart:21-110): the method
    /// name, the constructor name, the whole call or the argument list,
    /// depending on the shape.
    pub entity: Option<Span>,
    /// Positional `super.x` parameters of the current constructor (implicit
    /// arguments of a `super(...)`, `verifySuperFormalParameters`,
    /// an611:src/error/super_formal_parameters_verifier.dart:11-37).
    pub super_positional_args: usize,
    /// Names of the named `super.x` parameters of the current constructor.
    pub super_named_args: Vec<dartforge_intern::SymbolId>,
}

/// Enum constant without arguments (`v;`) with an unnamed constructor that
/// requires arguments: `NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_*` at the
/// constant's name (an611:src/generated/resolver.dart:2531-2544) and
/// `MISSING_REQUIRED_ARGUMENT` there too
/// (an611:src/error/required_parameters_verifier.dart:37-43).
pub(crate) fn arity_without_arguments(inf: &mut BodyInferrer<'_>, f: FunctionElementId, enum_name: String, span: Span) {
    use dartforge_diagnostics::codes::compile_time_error as ce;
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

/// Sets the target of the next arity check.
pub(crate) fn define_target(inf: &mut BodyInferrer<'_>, the_name: Option<String>, entity: Span) {
    inf.arity_target = Some(ArityTarget { name: the_name, entity: Some(entity), ..Default::default() });
}

/// The next source token starting at `pos` (skips whitespace and comments): a
/// whole identifier or a character.
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
    Span { start: pos, end }
}

/// Arity and name diagnostics (`resolveArgumentsToParameters`,
/// an611:src/generated/resolver.dart:4229-4355, and
/// `RequiredParametersVerifier`): an undefined or repeated named argument at
/// the argument's name; too few positionals at the token after the last
/// positional (or after the `(`); too many positionals at the first extra
/// one, with the `_COULD_BE_NAMED` variant when unused named parameters are
/// left; a missing required named one at the call's entity.
fn verify_arity(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    positional: &[TypeId],
    optional: &[TypeId],
    named: &[(dartforge_intern::SymbolId, TypeId, bool)],
    args: &ast::Arguments,
    tgt: Option<ArityTarget>,
) {
    use dartforge_diagnostics::codes::compile_time_error as ce;
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
    // Token for "too few positionals": after the last positional, or the
    // first one after the `(`.
    let token_few = match last_positional {
        Some(sp) => next_token(source, sp.end),
        None => {
            let begin = args.type_args.last().map(|&t| a.ty(t).span.end).unwrap_or(args.span.start);
            let open = source.get(begin..).and_then(|r| r.find('(')).map(|i| begin + i + 1).unwrap_or(args.span.start);
            next_token(source, open)
        }
    };
    // The scanner's synthetic `)` (zero length at the end of the arguments):
    // when the next token is it, the error goes there with zero length.
    let synthetic_closure = args.span.end > args.span.start && source.as_bytes().get(args.span.end - 1) != Some(&b')');
    let token_few = if synthetic_closure && token_few.start >= args.span.end {
        Span { start: args.span.end, end: args.span.end }
    } else {
        token_few
    };
    let total_pos = npos + tgt.super_positional_args;
    let mut used: Vec<dartforge_intern::SymbolId> = tgt.super_named_args.clone();
    let mut warnings: Vec<(dartforge_diagnostics::Code, Span, Vec<String>)> = Vec::new();
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
        let code = match (&tgt.name, plural) {
            (None, true) => ce::NOT_ENOUGH_POSITIONAL_ARGUMENTS_PLURAL,
            (None, false) => ce::NOT_ENOUGH_POSITIONAL_ARGUMENTS_SINGULAR,
            (Some(_), true) => ce::NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_PLURAL,
            (Some(_), false) => ce::NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_SINGULAR,
        };
        if let Some(n) = &tgt.name {
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
    let entity = tgt.entity.unwrap_or(args.span);
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

/// The argument is a function literal with some untyped parameter (a
/// candidate to be deferred in horizontal inference).
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

/// Order of horizontal inference (`inference-update-1`): the non-deferred
/// arguments form stage 0; the deferred function literals follow the
/// dependencies among themselves: a literal whose parameter types mention a
/// variable that another deferred literal's return still provides waits for
/// it. Those that only depend on already provided variables form the next
/// stage (in text order); in a cycle, the remaining ones go together.
/// Returns `(estágio, índice do argument)` in inference order.
fn horizontal_stages(
    inf: &BodyInferrer<'_>,
    type_params: &[TypeParamId],
    params: &[Option<TypeId>],
    deferred: &[bool],
) -> Vec<(usize, usize)> {
    let mut order: Vec<(usize, usize)> = (0..deferred.len()).filter(|&i| !deferred[i]).map(|i| (0, i)).collect();
    // For each deferred one: the variables it depends on (in the formal's
    // parameter types) and those it provides (in the formal's return).
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

/// `TYPE_ARGUMENT_NOT_MATCHING_BOUNDS` of explicit type arguments against
/// the type parameters `params` (`TypeArgumentsVerifier`'s
/// `_checkInvocationTypeArguments`, `ExtensionMemberResolver`'s
/// `_checkTypeArgumentsMatchingBounds` and `AnnotationInferrer`'s): only the
/// first `min(params, types)`, with their substitution; a parameter without
/// a written bound is left out. The error goes on the written argument
/// `nodes[i]`, or on `alternative`.
///
/// `visible`: the type parameters in scope at the call. Substituting a
/// function type does not replace the bounds of its type parameters (the
/// `m<T extends X>` of `C<num>` keeps the `X`), so a bound that still
/// mentions a type parameter out of scope and outside `params` is not
/// checked.
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
    let map = inf.map(&params[..n], &types[..n]);
    let mut allowed: std::collections::HashSet<TypeParamId> = params.iter().copied().collect();
    if let Some(v) = visible {
        allowed.extend(v.iter().copied());
    }
    for i in 0..n {
        let data = inf.table.param(params[i]).clone();
        if !data.explicit {
            continue;
        }
        if visible.is_some() && mentions_free(inf.table, data.bound, &allowed, 0) {
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
        inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::TYPE_ARGUMENT_NOT_MATCHING_BOUNDS, sp, &[&a, &the_name, &l]);
    }
}

/// `t` mentions a type parameter that is not in `allowed` and is not a
/// formal of a function type inside `t`.
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
/// the argument list (from the `(`) is the first dead node when the invocation
/// is reached as unreachable; the fragment runs to the end of the current
/// basic block.
pub(crate) fn dead_in_list(inf: &mut BodyInferrer<'_>, cx: &mut Body, args: &ast::Arguments) {
    if cx.flow.reachable || cx.dead_code.is_some() {
        return;
    }
    let sp = args.span;
    let end = cx.flow_ends.last().copied().or(cx.block_ends.last().copied()).unwrap_or(sp.end).max(sp.end);
    inf.warning(DEAD_CODE.template.to_string(), Span { start: sp.start, end });
    cx.dead_code = Some(cx.flow_ends.len());
}

/// Invokes a function type with the arguments; returns `(return type, instantiated function)`.
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
        inf.positional_names = None;
        for a in args.args.iter() {
            infer_free(inf, cx, a.value);
        }
        return (inf.core.dynamic_, f);
    };
    let u = inf.core.unknown;
    // A generic call whose type parameters are in scope (the function
    // calling itself, `_mergeSort(elements, keyOf, …)` inside
    // `_mergeSort<E, K>`): the arguments mention the same parameters that
    // inference resolves; renames to new parameters, like the
    // specification's instantiation (R-GEN-04: fresh variables).
    if !type_params.is_empty()
        && type_params.iter().any(|&p| {
            let n = inf.table.param(p).name;
            matches!(cx.lookup(n), Some(super::body::MemberName::TypeParam(q)) if q == p)
        })
    {
        let new_ = inf.new_parameters(&type_params);
        let types: Vec<TypeId> = new_.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        let map = inf.map(&type_params, &types);
        let ret = inf.subst(ret, &map);
        let positional: Box<[TypeId]> = positional.iter().map(|&t| inf.subst(t, &map)).collect();
        let optional: Box<[TypeId]> = optional.iter().map(|&t| inf.subst(t, &map)).collect();
        let named: Box<[_]> = named.iter().map(|&(n, t, r)| (n, inf.subst(t, &map), r)).collect();
        let f2 = inf.table.intern(Type::Function { type_params: new_.into_boxed_slice(), ret, positional, optional, named, nullable: false });
        inf.arity_target = tgt;
        inf.implicit_call = implicit_call;
        return invoke(inf, cx, f2, args, ctx, explicit);
    }
    // `ErrorVerifier`'s `checkMethodInvocation` /
    // `checkFunctionExpressionInvocation`: the explicit ones against the type
    // parameters of the invoked type (not for an implicit `call`: the
    // function's type is an interface).
    if let Some(ex) = &explicit
        && !type_params.is_empty()
        && !implicit_call
    {
        let ex = ex.clone();
        let visible: Vec<TypeParamId> = cx.visible_type_parameters().into_values().collect();
        check_explicit_bounds(inf, cx.unit, &type_params, &ex, &args.type_args, args.span, Some(&visible));
    }
    // The `errorEntity` only applies to this invocation (the arguments have
    // their own).
    let entity = inf.inference_entity.take();
    let positional_names = inf.positional_names.take();
    // `FullInvocationInferrer.resolveInvocation`
    // (`invocation_inferrer.dart:175-184` of analyzer 3.6.2): a type argument
    // list whose count differs from that of the invoked type's type
    // parameters is `WRONG_NUMBER_OF_TYPE_ARGUMENTS_METHOD`, at the `<…>`
    // list, with the raw function type as the first argument; inference goes
    // on without the explicit ones. Without the target's kind and name (which
    // only the caller knows), the 3.13 variant of the code comes out in the
    // "type of this function" form (docs/ANALYZER-ESPECIFICACAO.md, T2 §8,
    // case c27).
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
            dartforge_diagnostics::codes::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS_METHOD,
            Span { start, end },
            &[&typ, &type_params.len().to_string(), &ex.len().to_string()],
        );
    }
    // Explicit type arguments: instantiates and goes on as non-generic.
    if !type_params.is_empty()
        && let Some(ex) = explicit {
            // Wrong count: `dynamic` for each type parameter, without inference
            // (`invocation_inferrer.dart:174-183`).
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
    verify_arity(inf, cx, &positional, &optional, &named, args, tgt);
    let params = argument_parameters(inf, &positional, &optional, &named, args);
    let numeric_context = inf.pending_numeric_context.take();
    if type_params.is_empty() {
        for (a, p) in args.args.iter().zip(params.iter()) {
            // `x.clamp(a, b)` / `x.remainder(a)`: the analyzer's refined context.
            let c = match (numeric_context, a.name) {
                (Some(c), None) => c,
                _ => p.unwrap_or(u),
            };
            let t = infer(inf, cx, a.value, c);
            if let Some(p) = p {
                inf.body_types.units[cx.unit.0 as usize].parameter_types.insert(a.value, *p);
                expr::verify_assignable_expr(inf, cx, a.value, t, *p, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
                closure_with_set(inf, cx, a.value, *p);
            }
        }
        return (ret, f);
    }
    // Generic: downwards (return × context), then arguments in stages.
    let mut gi = GenericInferrer::new(&type_params);
    if !inf.the_is_unknown(ctx) {
        gi.with_origin(crate::constraints::Origin::Return { declared: ret, context: ctx });
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
            let t = infer(inf, cx, a.value, context);
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
                gi.with_origin(crate::constraints::Origin::Argument { parameter, declared: p, argument: t, prefix: None });
                let mut env = inf.env();
                gi.constrain_argument(t, p, &mut env);
            }
        }
    }
    let use_bounds = inf.program.library(cx.lib).features.has(dartforge_frontend::Feature::InferenceUsingBounds);
    gi.generic_metadata = inf.program.library(cx.lib).features.has(dartforge_frontend::Feature::GenericMetadata);
    let (interner, program) = (inf.interner, inf.program);
    let mut env = inf.env();
    if use_bounds {
        gi.restrict_by_bounds(&mut env);
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
            inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::COULD_NOT_INFER, sp, &[&the_name, &suffix]);
        }
    }
    inf.body_types.units[cx.unit.0 as usize].set_instantiation(args.span.start, finals.clone().into_boxed_slice());
    // Check with the instantiated parameters.
    if let Type::Function { positional: ip, optional: io, named: inm, ret: iret, .. } = inf.table.get(inst).clone() {
        let ips = argument_parameters(inf, &ip, &io, &inm, args);
        for (i, a) in args.args.iter().enumerate() {
            if let Some(p) = ips[i] {
                inf.body_types.units[cx.unit.0 as usize].parameter_types.insert(a.value, p);
                expr::verify_assignable_expr(inf, cx, a.value, types[i], p, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
                closure_with_set(inf, cx, a.value, p);
            }
        }
        return (iret, inst);
    }
    (inf.core.dynamic_, inst)
}

/// The argument is a closure (the `FunctionExpression` itself, with the
/// `staticParameterElement`) and the parameter is a function type: the
/// `UNNECESSARY_SET_LITERAL` through its return.
fn closure_with_set(inf: &mut BodyInferrer<'_>, cx: &Body, argument: ExprId, parameter: TypeId) {
    let ExprKind::FunctionExpression(fid) = inf.program.unit(cx.unit).ast.expr(argument).kind else { return };
    if let Type::Function { ret, .. } = inf.table.get(parameter).clone() {
        super::functions::unnecessary_set(inf, cx.unit, fid, ret);
    }
}

/// Invokes a value of type `t` (function, object with `call`, `dynamic`).
fn invoke_value(inf: &mut BodyInferrer<'_>, cx: &mut Body, (e, span): (ExprId, Span), t: TypeId, args: &ast::Arguments, ctx: TypeId, explicit: Option<Vec<TypeId>>) -> (TypeId, TypeId) {
    // The arity target only applies if the invocation actually happens.
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
    // `x()` with `x` of type `Null` or `Never?`: `call` does not exist and
    // the receiver may be null: `invalid_use_of_null_value` (`Null`) or
    // `unchecked_use_of_nullable_value` (`Never?`) at the function; the
    // result is `dynamic` (not `Never`: there is no dead code after it).
    if matches!(inf.table.get(t), Type::Null)
        && let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind
    {
        let function = *target;
        expr::nullable_dereference(inf, cx, function, t, dartforge_diagnostics::codes::compile_time_error::UNCHECKED_INVOCATION_OF_NULLABLE_VALUE);
        for a in args.args.iter() {
            infer_free(inf, cx, a.value);
        }
        let _ = tgt;
        return (inf.core.dynamic_, t);
    }
    let t_nn = inf.non_null(t);
    // `x(3)` with `x` of type `void`: `use_of_void_result` at the function.
    if matches!(inf.table.get(t_nn), Type::Void)
        && let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind {
            let tgt = *target;
            expr::void_use(inf, cx, tgt, t_nn);
        }
    match inf.table.get(t_nn).clone() {
        Type::Function { .. } => {
            // `FunctionExpressionInvocationResolver.resolve`
            // (`function_expression_invocation_resolver.dart:52-58`): a function of
            // a nullable function type, at the function's node.
            if let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind {
                let function = *target;
                expr::nullable_dereference(inf, cx, function, t, dartforge_diagnostics::codes::compile_time_error::UNCHECKED_INVOCATION_OF_NULLABLE_VALUE);
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
            // `Function?`: the `call` through `TypePropertyResolver`, with the
            // nullable receiver reported at the function.
            if let Some(call) = inf.sym.call
                && inf.requires_null_check(cx.lib, t, call, false)
                && let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind
            {
                let function = *target;
                expr::nullable_dereference(inf, cx, function, t, dartforge_diagnostics::codes::compile_time_error::UNCHECKED_INVOCATION_OF_NULLABLE_VALUE);
            }
            for a in args.args.iter() {
                infer_free(inf, cx, a.value);
            }
            (inf.core.dynamic_, t)
        }
        _ => {
            // `FunctionExpressionInvocationResolver.resolve`
            // (`function_expression_invocation_resolver.dart:72-78`): outside a
            // function type, the `call` through `TypePropertyResolver`; a
            // potentially nullable receiver is `UNCHECKED_INVOCATION_OF_NULLABLE_VALUE`
            // at the function (the parent is `FunctionExpressionInvocation`), and
            // recovery goes on through the bound.
            if let Some(call) = inf.sym.call
                && inf.requires_null_check(cx.lib, t, call, false)
                && let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind
            {
                let function = *target;
                expr::nullable_dereference(inf, cx, function, t, dartforge_diagnostics::codes::compile_time_error::UNCHECKED_INVOCATION_OF_NULLABLE_VALUE);
            }
            // `createT()('')` with `T extends int Function(int)`: the parser's
            // `FunctionExpressionInvocation` (a target that is neither a name nor a
            // property access) does not take `T` to its bound; `TypePropertyResolver`
            // finds the `call` of the bound's function type only as a
            // `callFunctionType`, without an element, and the 3.6.2 resolver leaves
            // the call `dynamic`, without checking the arguments
            // (`function_expression_invocation_resolver.dart:80-95`). A call by name
            // or property (`tValue('')`) is rewritten by `MethodInvocationResolver`,
            // which takes it to the bound (`_rewriteAsFunctionExpressionInvocation`,
            // `resolveToBound`).
            if matches!(inf.table.get(t_nn), Type::TypeParameter { .. } | Type::Intersection { .. })
                && let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind
                && !matches!(inf.program.unit(cx.unit).ast.expr(*target).kind, ExprKind::Identifier(_) | ExprKind::Property { .. })
                && matches!(inf.table.get(inf.resolve_to_bound(t_nn)), Type::Function { .. })
            {
                for a in args.args.iter() {
                    infer_free(inf, cx, a.value);
                }
                return (inf.core.dynamic_, t);
            }
            // A `call` found that is not a method (a `call` getter or field, of the
            // class or of an extension): `callElement.kind != METHOD`
            // (`function_expression_invocation_resolver.dart:96-103`), reported at
            // the function, with the invalid result.
            let call_not_method = |inf: &mut BodyInferrer<'_>, cx: &mut Body| -> (TypeId, TypeId) {
                if let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind {
                    let sp = inf.span_expr(cx.unit, *target);
                    inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::INVOCATION_OF_NON_FUNCTION_EXPRESSION, sp, &[]);
                }
                for a in args.args.iter() {
                    infer_free(inf, cx, a.value);
                }
                (inf.table.invalid(inf.core.dynamic_), t)
            };
            // `super(…)`: the `call` through the `super` chain (the
            // `TypePropertyResolver` with a `SuperExpression` receiver); without it,
            // `INVOCATION_OF_NON_FUNCTION_EXPRESSION` at the `super` and the type is
            // `InvalidType`.
            if let Some(call) = inf.sym.call
                && let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind
                && matches!(inf.program.unit(cx.unit).ast.expr(*target).kind, ExprKind::Super)
            {
                let super_target = *target;
                match expr::lookup_super_operator(inf, cx, call) {
                    Search::Finding(m) if m.method => {
                        inf.arity_target = tgt;
                        return invoke(inf, cx, m.ty, args, ctx, explicit);
                    }
                    _ => {
                        for a in args.args.iter() {
                            infer_free(inf, cx, a.value);
                        }
                        let sp = inf.span_expr(cx.unit, super_target);
                        inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::INVOCATION_OF_NON_FUNCTION_EXPRESSION, sp, &[]);
                        return (inf.table.invalid(inf.core.dynamic_), t);
                    }
                }
            }
            if let Some(call) = inf.sym.call {
                if let Some(m) = inf.interface_member(t_nn, call, false) {
                    if !m.method {
                        return call_not_method(inf, cx);
                    }
                    inf.arity_target = tgt;
                    inf.implicit_call = true;
                    return invoke(inf, cx, m.ty, args, ctx, explicit);
                }
                let search = inf.lookup_member(cx.lib, t_nn, call, false);
                if inf.extension_ambiguity.is_some() {
                    // Ambiguous `call`: the analyzer reports the ambiguity at the target
                    // (`function_expression_invocation_resolver.dart`) and does not
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
                if let Search::Finding(m) = search {
                    // Nullable receiver: the extension only applies if it applies to the
                    // nullable type; otherwise there is no `call` (the null was already
                    // reported, and `needsGetterError` stays false).
                    if m.of_extension && t != t_nn && !matches!(inf.lookup_member(cx.lib, t, call, false), Search::Finding(_)) {
                        for a in args.args.iter() {
                            infer_free(inf, cx, a.value);
                        }
                        return (inf.core.dynamic_, t);
                    }
                    if !m.method {
                        return call_not_method(inf, cx);
                    }
                    // `value(args)` with an extension's `call` (`calloc<Int32>(4)`,
                    // `AllocatorAlloc.call`): the call records the member, which lowering
                    // invokes with the value as receiver.
                    if m.of_extension {
                        super::expr::resolver(inf, cx, e, m.resolved.clone());
                    }
                    inf.arity_target = tgt;
                    inf.implicit_call = true;
                    return invoke(inf, cx, m.ty, args, ctx, explicit);
                }
            }
            for a in args.args.iter() {
                infer_free(inf, cx, a.value);
            }
            // `o()`, `x.campo()`, `3(5)`: the analyzer reports
            // `invocation_of_non_function_expression` at the call's target. A type
            // literal (`T<Null>()` with `T` an alias of `dynamic`) is another code
            // (`invocation_of_non_function`), not handled here.
            // Left out (other codes or nothing): a nullable receiver
            // (`unchecked_…`), `void` (`use_of_void_result`), a type parameter (the
            // call goes through the bound) and a bare name that is not a local
            // (`foo()` with only `set foo`: `undefined_method`).
            let _ = span;
            let a = &inf.program.unit(cx.unit).ast;
            let tgt = match &a.expr(e).kind {
                ExprKind::Call { target, .. } => Some(*target),
                _ => None,
            };
            // A name that resolves to a getter or variable (top-level, static,
            // member through the implicit `this`, extension) is rewritten into a
            // `FunctionExpressionInvocation` and reports
            // (`method_invocation_resolver.dart:585-592`, `:900-905`); a name
            // without a getter (setter only) is `undefined_method`.
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
                ExprKind::Identifier(n) => !matches!(cx.lookup(n.sym), Some(super::body::MemberName::Local(_))) && !getter(x),
                _ => false,
            });
            // The `Type` of an invoked type literal (`T<Null>()`) is another code;
            // between parentheses (`(T)()`) it is a `FunctionExpressionInvocation`
            // without `call` on `Type`, and it reports.
            let parenthesized = tgt.is_some_and(|x| matches!(a.expr(x).kind, ExprKind::Parenthesized(_)));
            let outside = (t_nn == inf.core.type_ && !parenthesized)
                || t != t_nn
                || non_local_name
                || matches!(inf.table.get(t_nn), Type::Void | Type::TypeParameter { .. } | Type::Intersection { .. });
            if let (false, Some(tgt)) = (outside, tgt) {
                let sp = inf.span_expr(cx.unit, tgt);
                inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::INVOCATION_OF_NON_FUNCTION_EXPRESSION, sp, &[]);
                // `result.isGetterInvalid` (3.6.2
                // `function_expression_invocation_resolver.dart:89-92`): without any
                // `call`, the type is `InvalidType`.
                return (inf.table.invalid(inf.core.dynamic_), t);
            }
            (inf.core.dynamic_, t)
        }
    }
}

fn type_arguments(inf: &mut BodyInferrer<'_>, cx: &Body, args: &ast::Arguments) -> Option<Vec<TypeId>> {
    if args.type_args.is_empty() {
        return None;
    }
    Some(args.type_args.iter().map(|&t| inf.type_argument_type(cx, t)).collect())
}

/// Prepares the `errorEntity` of `COULD_NOT_INFER` for the next generic
/// invocation and the names of the positional parameters of target `f`.
pub(crate) fn prepare_entity(inf: &mut BodyInferrer<'_>, sp: Span, f: Option<FunctionElementId>) {
    inf.inference_entity = Some(sp);
    inf.positional_names = f.map(|f| {
        inf.outline.functions[f.0 as usize]
            .parameters
            .iter()
            .filter(|p| p.kind != ast::ParameterKind::Named)
            .map(|p| p.name.map(|n| inf.interner.resolve(n).to_string()).unwrap_or_default())
            .collect()
    });
}

/// The function (top-level, member, extension) that `e` resolved to.
/// [`resolved_function`] for the other inference modules.
pub(crate) fn resolved_function_of(inf: &BodyInferrer<'_>, cx: &Body, e: ExprId) -> Option<FunctionElementId> {
    resolved_function(inf, cx, e)
}

fn resolved_function(inf: &BodyInferrer<'_>, cx: &Body, e: ExprId) -> Option<FunctionElementId> {
    match inf.body_types.units[cx.unit.0 as usize].get_resolved(e)? {
        Resolved::Element(Element::Function(f)) => Some(*f),
        Resolved::Member { member: crate::resolved::MemberRef::Function(f), .. } => Some(*f),
        Resolved::ExtensionMember { member, .. } => Some(*member),
        _ => None,
    }
}

/// `f(args)`, `r.m(args)`, `C(args)`, `C.the_name(args)`...
/// `ExtensionMemberResolver.resolveOverride`
/// (`an611:src/dart/resolver/extension_member_resolver.dart:177-187`): an
/// `E(x)` outside an access (left operand of a binary, function of an
/// invocation, target of an index, method or property, prefix operand) and
/// outside a cascade target: `EXTENSION_OVERRIDE_WITHOUT_ACCESS` on the
/// whole override.
fn override_without_access(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) {
    let valid = match inf.parent_of(cx.unit, e) {
        dartforge_frontend::parents::Parent::Expr(p) => match &inf.program.unit(cx.unit).ast.expr(p).kind {
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
        inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::EXTENSION_OVERRIDE_WITHOUT_ACCESS, sp, &[]);
    }
}

/// The written `<…>` type argument list: from the `<` before the first one
/// to the `>` after the last one.
pub(crate) fn type_list_range(inf: &BodyInferrer<'_>, unit: dartforge_elements::model::UnitId, types: &[ast::TypeId]) -> Option<Span> {
    let (first, last) = (types.first()?, types.last()?);
    let unit_id = inf.program.unit(unit);
    let (of, until) = (unit_id.ast.ty(*first).span.start, unit_id.ast.ty(*last).span.end);
    let source = &unit_id.source;
    let start = source.get(..of).and_then(|t| t.rfind('<')).unwrap_or(of);
    let end = source.get(until..).and_then(|t| t.find('>')).map_or(until, |i| until + i + 1);
    Some(Span { start, end })
}

/// `WRONG_NUMBER_OF_TYPE_ARGUMENTS_CONSTRUCTOR` (`ast_rewrite.dart:438-445`,
/// `:604-611`; `named_type_resolver.dart:337-343`;
/// `function_reference_resolver.dart:51-62`): type arguments after a
/// constructor name, at the `<…>` list; `{0}` the type as written (with the
/// prefix), `{1}` the constructor.
pub(crate) fn types_in_constructor(inf: &mut BodyInferrer<'_>, unit: dartforge_elements::model::UnitId, written_class: Span, ctor: ast::Name, types: &[ast::TypeId]) {
    let Some(sp) = type_list_range(inf, unit, types) else { return };
    let source = &inf.program.unit(unit).source;
    let cls = source[written_class.start..written_class.end].split_whitespace().collect::<String>();
    let the_name = inf.interner.resolve(ctor.sym).to_string();
    inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS_CONSTRUCTOR, sp, &[&cls, &the_name]);
}

pub(crate) fn the_call(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, ctx: TypeId) -> (TypeId, bool) {
    let a = &inf.program.unit(cx.unit).ast;
    let ExprKind::Call { target, arguments } = &a.expr(e).kind else { unreachable!() };
    let (target, args): (ExprId, &ast::Arguments) = (*target, arguments);
    let span = a.expr(e).span;
    let explicit = type_arguments(inf, cx, args);
    // `C<T>.the_name(…)` is a creation only when fasta reads it as an implicit
    // creation (`implicit_creations`); with type arguments after the name
    // (`C<T>.the_name<U>()`) it is a method of the type literal, which
    // `AstRewriter` does not rewrite.
    let the_implicit = a.implicit_creations.contains(&e);
    let over_instantiation = matches!(&a.expr(target).kind, ExprKind::Property { target: r, .. } if matches!(a.expr(*r).kind, ExprKind::TypeArguments { .. }));
    let ctor = if over_instantiation && !the_implicit { None } else { target_constructor(inf, cx, target) };
    if the_implicit && let ExprKind::Property { target: recv, .. } = &a.expr(target).kind {
        expr::check_literal_arguments(inf, cx, *recv, true);
    }
    if ctor.is_none()
        && the_implicit
        && let ExprKind::Property { target: recv, name, .. } = &a.expr(target).kind
        && let Some(rt) = type_reference(inf, cx, *recv)
        && !matches!(rt, TypeRef::Extension(_))
    {
        // An implicit creation without the constructor: an
        // `InstanceCreationExpression` of an unresolved constructor
        // (`NEW_WITH_UNDEFINED_CONSTRUCTOR`, at the name, with the type as
        // written); the type is the named type's and the arguments have no
        // context.
        let (recv, name) = (*recv, *name);
        let (c, targs) = match &rt {
            TypeRef::Class(c, Some(ts)) => (*c, ts.iter().map(|&t| inf.type_argument_type(cx, t)).collect::<Vec<_>>()),
            TypeRef::Class(c, None) => (*c, Vec::new()),
            TypeRef::Alias(c, args, _) => (*c, args.clone().unwrap_or_default()),
            TypeRef::Extension(_) => unreachable!(),
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
            inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::NEW_WITH_UNDEFINED_CONSTRUCTOR, name.span, &[&qualified, &the_name]);
        }
        for x in args.args.iter() {
            infer_free(inf, cx, x.value);
        }
        return (inf.class_type_with_args(c, targs), false);
    }
    // `T<X>.the_name(…)` read as a creation with `T` an alias of a type that is
    // not a class: the creation's `CREATION_WITH_NON_TYPE`, and nothing
    // from the literal.
    if ctor.is_none()
        && the_implicit
        && let ExprKind::Property { target: recv, .. } = &a.expr(target).kind
        && let ExprKind::TypeArguments { target: base, type_args } = &a.expr(*recv).kind
        && expr::alias_without_class(inf, cx, *base)
        && !expr::alias_of_null_or_futureor(inf, cx, *base)
        && !expr::function_type_alias(inf, cx, *base)
    {
        // (`AstRewriter` rewrites a function type alias as a call on the type
        // literal, `ast_rewrite.dart:55-62`: it follows the common path.)
        let names: Vec<ast::Name> = match &a.expr(*base).kind {
            ExprKind::Identifier(n) => vec![*n],
            ExprKind::Property { target: p, name, .. } => match &a.expr(*p).kind {
                ExprKind::Identifier(p) => vec![*p, *name],
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };
        if expr::type_parameter_alias(inf, cx, *base) {
            // `_verifyTypeAliasForContext`: at the name, without the arguments.
            if let (Some(p), Some(u)) = (names.first(), names.last()) {
                let sp = Span { start: p.span.start, end: u.span.end };
                inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::INSTANTIATE_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER, sp, &[]);
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
    // `X<T>.the_name(…)` read as a creation with `X` not a type (undefined,
    // local, function, type parameter): `NamedTypeResolver` reports
    // `CREATION_WITH_NON_TYPE` through the creation (`_isInstanceCreation`,
    // 3.6.2 `named_type_resolver.dart:230-232, 274-276`), and not the
    // undefined name.
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
    // Constructor without `new`.
    if let Some((c, f, targs)) = ctor {
        if inf.program.class(c).kind == ClassKind::Enum
            && f.is_some_and(|f| !inf.program.function(f).factory)
        {
            inf.warning(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template.to_string(), a.expr(target).span);
            // The constructor stays resolved (`constructorName.staticElement`):
            // constant evaluation goes on through its call.
            if let Some(f) = f {
                resolver(inf, cx, e, Resolved::Constructor(f));
            }
            for arg in args.args.iter() {
                infer_free(inf, cx, arg.value);
            }
            return (inf.core.dynamic_, false);
        }
        if let Some(f) = f {
            // The named type of the implicit creation: `A`, `A<int>`, `p.A`
            // (without the constructor's name).
            let sp = match &a.expr(target).kind {
                ExprKind::Property { target: t, .. } if expr::type_reference(inf, cx, *t).is_some() => creation_named_type(inf, cx, *t, &[]),
                _ => creation_named_type(inf, cx, target, &args.type_args),
            };
            warn_abstract_class(inf, c, f, sp);
        }
        // `C(...)`/`p.C(...)` become `InstanceCreationExpression` in the
        // analyzer: the name is `C.new` (or the constructor's) and the entity,
        // the `constructorName` (type with prefix and arguments, plus `.the_name`).
        let (the_name, ent) = match &a.expr(target).kind {
            ExprKind::Property { target: t, name, .. } if expr::type_reference(inf, cx, *t).is_some() => {
                (inf.interner.resolve(name.sym).to_string(), a.expr(target).span)
            }
            _ => {
                let cls = last_identifier(inf, cx, target).map(|s| inf.interner.resolve(s).to_string()).unwrap_or_default();
                (format!("{cls}.new"), creation_named_type(inf, cx, target, &args.type_args))
            }
        };
        // `C.the_name<T>()`: the list belongs to the constructor (error), not to the
        // class.
        let explicit = match &a.expr(target).kind {
            ExprKind::Property { target: t, name, .. } if !args.type_args.is_empty() && expr::type_reference(inf, cx, *t).is_some() => {
                let written = a.expr(*t).span;
                types_in_constructor(inf, cx.unit, written, *name, &args.type_args);
                None
            }
            _ => explicit,
        };
        // The count of the type arguments written on the class (`C<T>()`,
        // `C<T>.the_name()`): the `NamedType` of the rewritten creation, with
        // `_buildTypeArguments`'s wrong count and the arguments as
        // `InvalidType`.
        let mut targs = targs;
        let mut explicit = explicit;
        // In `D<int>()`, the arguments stay in the call, while the reference
        // `D` was resolved without them. Apply them to the alias before using
        // the class's arguments (`D<X> = C<List<X>>`).
        if !args.type_args.is_empty()
            && let Some(TypeRef::Alias(_, _, td)) = type_reference(inf, cx, target)
            && let Some(TypeRef::Alias(_, inst, _)) = expr::alias_of(inf, td, explicit.clone())
        {
            targs = inst;
        }
        let a = &inf.program.unit(cx.unit).ast;
        let written_ones: Option<(usize, Span)> = match &a.expr(target).kind {
            ExprKind::Property { target: t, .. } => match &a.expr(*t).kind {
                ExprKind::TypeArguments { target: base, type_args } if matches!(type_reference(inf, cx, *base), Some(TypeRef::Class(..))) => {
                    let n = type_args.len();
                    let sp = creation_named_type(inf, cx, *t, &[]);
                    Some((n, sp))
                }
                _ => None,
            },
            _ if !args.type_args.is_empty() && matches!(type_reference(inf, cx, target), Some(TypeRef::Class(..))) => {
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
            None => build(inf, cx, Some(e), c, f, targs.or(explicit), args, ctx),
        };
        inf.arity_target = None;
        return (t, false);
    }
    // `E()`: the enum's implicit unnamed constructor is not in the
    // constructor table (so `target_constructor` did not find it), but it
    // exists and is generative. With an explicit name, the arm above already
    // resolved it (a factory is legal, a generative one reports); here only
    // the implicit one is left.
    if let Some(rt) = type_reference(inf, cx, target)
        && !matches!(rt, TypeRef::Extension(_))
    {
        let c = match rt {
            TypeRef::Class(c, _) | TypeRef::Alias(c, _, _) => c,
            TypeRef::Extension(_) => unreachable!(),
        };
        // `A()` of an abstract class without a declared constructor: the
        // implicit default is generative (`INSTANTIATE_ABSTRACT_CLASS`).
        {
            let k = inf.program.class(c);
            if matches!(k.kind, ClassKind::Class | ClassKind::MixinApplication)
                && (k.modifiers.abstract_ || k.modifiers.sealed)
                && k.constructors.is_empty()
            {
                let sp = creation_named_type(inf, cx, target, &args.type_args);
                inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::INSTANTIATE_ABSTRACT_CLASS, sp, &[]);
            }
        }
        if inf.program.class(c).kind == ClassKind::Enum
            && inf.sym.empty.is_some_and(|v| inf.constructor_of(c, v).is_none())
        {
            inf.warning(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template.to_string(), a.expr(target).span);
            for arg in args.args.iter() {
                infer_free(inf, cx, arg.value);
            }
            return (inf.core.dynamic_, false);
        }
        // `C()` rewritten as a creation (`ast_rewrite.dart`) without the unnamed
        // constructor, nor the implicit one: `NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT`
        // at the named type (`_checkForNewWithUndefinedConstructor`).
        if matches!(inf.program.class(c).kind, ClassKind::Class | ClassKind::MixinApplication)
            && inf.sym.empty.is_some_and(|v| inf.constructor_of(c, v).is_none())
            // A declared `A.new()` (without `constructor-tearoffs` the model keeps
            // the key `new`) is the unnamed one.
            && inf.sym.new_.is_none_or(|n| inf.constructor_of(c, n).is_none())
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
            inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT, sp, &[&qualified]);
            for arg in args.args.iter() {
                infer_free(inf, cx, arg.value);
            }
            // Without a constructor the creation still has the named type
            // (`constructorName.type.type`: the written arguments, or the
            // instantiation to bounds); through an alias, the alias's expansion,
            // displayed with the alias (`T<dynamic>` for `typedef T<X> = FutureOr<X>`).
            let (params, alias) = match rt {
                TypeRef::Alias(_, _, td) => (inf.outline.typedefs[td.0 as usize].type_params.to_vec(), Some(td)),
                _ => (inf.outline.classes[c.0 as usize].type_params.to_vec(), None),
            };
            let targs: Option<Vec<TypeId>> = if args.type_args.is_empty() {
                Some(inf.instantiate_to_bounds(&params))
            } else if args.type_args.len() == params.len() {
                Some(args.type_args.iter().map(|&t| inf.type_argument_type(cx, t)).collect())
            } else {
                None
            };
            if let Some(targs) = targs {
                let t = match alias {
                    None => inf.table.intern(Type::Interface { class: c, args: targs.into_boxed_slice(), nullable: false }),
                    Some(td) => {
                        let target_ty = inf.outline.typedefs[td.0 as usize].target_type;
                        let map = inf.map(&params, &targs);
                        let expanded = inf.subst(target_ty, &map);
                        inf.table.decorate(expanded, crate::table::DisplayDecoration::Alias { typedef: td, args: targs.into_boxed_slice() })
                    }
                };
                return (t, false);
            }
            return (inf.table.invalid(inf.core.dynamic_), false);
        }
    }
    let u = inf.core.unknown;
    // `E(a, b)` / `E()`: `INVALID_EXTENSION_ARGUMENT_COUNT` at the argument
    // list (an611:src/dart/resolver/extension_member_resolver.dart:189-197).
    if let Some(TypeRef::Extension(_)) = type_reference(inf, cx, target) {
        override_without_access(inf, cx, e);
    }
    if let Some(TypeRef::Extension(x)) = type_reference(inf, cx, target)
        && !(args.args.len() == 1 && args.args[0].name.is_none())
        && args.args.iter().all(|a| a.name.is_none())
        && args.args.len() != 1
    {
        let source = &inf.program.unit(cx.unit).source;
        let begin = args.type_args.last().map(|&t| a.ty(t).span.end).unwrap_or(args.span.start);
        let open = source.get(begin..).and_then(|r| r.find('(')).map(|i| begin + i).unwrap_or(args.span.start);
        let sp = Span { start: open, end: args.span.end.max(open) };
        inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::INVALID_EXTENSION_ARGUMENT_COUNT, sp, &[]);
        for arg in args.args.iter() {
            infer_free(inf, cx, arg.value);
        }
        let data = inf.outline.extensions[x.0 as usize].clone();
        let ext_args = inf.instantiate_to_bounds(&data.type_params);
        cx.overlays.insert(e, (x, ext_args));
        return (inf.core.dynamic_, false);
    }
    // `E(x)` / `E<T>(x)`: explicit extension override (R-EXT-02).
    if let Some(TypeRef::Extension(x)) = type_reference(inf, cx, target)
        && args.args.len() == 1
        && args.args[0].name.is_none()
    {
        let data = inf.outline.extensions[x.0 as usize].clone();
        // `E<A, B>(x)` with the wrong number of type arguments
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
                dartforge_diagnostics::codes::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS_EXTENSION,
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
                let map = inf.map(&data.type_params, ex);
                inf.subst(data.on, &map)
            }
            None => u,
        };
        let t = infer(inf, cx, args.args[0].value, ctx_arg);
        let explicit_ok = ext_args.is_some();
        if let Some(ex) = &ext_args {
            let ex = ex.clone();
            check_explicit_bounds(inf, cx.unit, &data.type_params, &ex, &args.type_args, args.span, None);
        }
        // `_inferTypeArguments` (an611:src/dart/resolver/extension_member_resolver.dart:321-367):
        // the explicit ones; with a wrong count, `dynamic`; without them,
        // `GenericInferrer` with the `errorEntity` at the extension's name and the
        // constraint `constrainArgument(receptor, extendedType)`.
        let ext_args = match (ext_args, &explicit) {
            (Some(ex), _) => ex,
            (None, Some(_)) => vec![inf.core.dynamic_; data.type_params.len()],
            (None, None) if data.type_params.is_empty() => Vec::new(),
            (None, None) => {
                // New parameters: inside the extension itself the receiver mentions
                // its parameters.
                let new_ = inf.new_parameters(&data.type_params);
                let types: Vec<TypeId> = new_.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
                let m0 = inf.map(&data.type_params, &types);
                let on_new = inf.subst(data.on, &m0);
                let entity = match &a.expr(target).kind {
                    ExprKind::Property { name, .. } => name.span,
                    _ => a.expr(target).span,
                };
                let mut gi = GenericInferrer::new(&new_);
                gi.with_origin(crate::constraints::Origin::Argument { parameter: "extendedType".to_string(), declared: on_new, argument: t, prefix: None });
                let use_bounds = inf.program.library(cx.lib).features.has(dartforge_frontend::Feature::InferenceUsingBounds);
                gi.generic_metadata = inf.program.library(cx.lib).features.has(dartforge_frontend::Feature::GenericMetadata);
                let (interner, program) = (inf.interner, inf.program);
                let mut env = inf.env();
                gi.constrain_argument(t, on_new, &mut env);
                if use_bounds {
                    gi.restrict_by_bounds(&mut env);
                }
                let finals = gi.choose_final(&mut env);
                let failures = gi.failures(&finals, &mut env, interner, program);
                drop(env);
                for (the_name, suffix) in failures {
                    inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::COULD_NOT_INFER, entity, &[&the_name, &suffix]);
                }
                finals
            }
        };
        // `EXTENSION_OVERRIDE_ARGUMENT_NOT_ASSIGNABLE` at the argument
        // (an611:src/dart/resolver/extension_member_resolver.dart:227-243);
        // `void` is `USE_OF_VOID_RESULT`. Only without type parameters or with
        // explicit arguments (a failed inference has another report).
        if data.type_params.is_empty() || explicit_ok {
            let arg = args.args[0].value;
            // `receiverType is VoidType`: on the whole argument (`atNode`, not the
            // method name of `f()`).
            let empty = matches!(inf.table.get(t), Type::Void);
            if empty {
                let sp = inf.span_expr(cx.unit, arg);
                inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::USE_OF_VOID_RESULT, sp, &[]);
            } else {
                let map = inf.map(&data.type_params, &ext_args);
                let on = inf.subst(data.on, &map);
                // An override that is the target of `?.`/`?[`/`?..` (`isNullAware`):
                // the checked argument type is the non-nullable one.
                let null_aware = match inf.parent_of(cx.unit, e) {
                    dartforge_frontend::parents::Parent::Expr(parent) => match &inf.program.unit(cx.unit).ast.expr(parent).kind {
                        ExprKind::Property { target: tgt, null_aware: true, .. }
                        | ExprKind::Index { target: tgt, null_aware: true, .. }
                        | ExprKind::Cascade { target: tgt, null_aware: true, .. } => *tgt == e,
                        _ => false,
                    },
                    _ => false,
                };
                let t = if null_aware { inf.non_null(t) } else { t };
                if !inf.assignable(t, on) {
                    let sp = inf.span_expr(cx.unit, arg);
                    let since = inf.diagnostics.len();
                    inf.warning_with_args(
                        dartforge_diagnostics::codes::compile_time_error::EXTENSION_OVERRIDE_ARGUMENT_NOT_ASSIGNABLE,
                        sp,
                        &[crate::display::Arg::Type_(t), crate::display::Arg::Type_(on)],
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
            if let ExprKind::Identifier(p) = &a.expr(recv).kind
                && matches!(resolve_name(inf, cx, p.sym, false), NameRef::Prefix) {
                    // `p?.f()`: a `MethodInvocation` target only counts as a prefix
                    // with `.` (`_isValidAsPrefix`).
                    if null_aware {
                        expr::prefix_without_dot(inf, *p);
                    }
                    // `p.f(args)` without a getter `f` in the prefix
                    // (`_resolveReceiverPrefix`,
                    // `an611:src/dart/resolver/method_invocation_resolver.dart:666-722`):
                    // `UNDEFINED_FUNCTION`, except `loadLibrary` of a deferred import and the
                    // ignored name of an import that does not exist.
                    let p = *p;
                    if inf.program.lookup_prefixed_in_unit(cx.unit, p.sym, name.sym).and_then(|b| b.getter).is_none()
                        && expr::load_library(inf, cx, p.sym, name.sym).is_none()
                    {
                        expr::resolver(inf, cx, recv, Resolved::Prefix(cx.lib));
                        if !crate::scope::should_ignore_undefined(inf.program, inf.interner, cx.unit, Some(p.sym), name.sym) {
                            let the_name = inf.interner.resolve(name.sym).to_string();
                            inf.warning_with_code(
                                dartforge_diagnostics::codes::compile_time_error::UNDEFINED_FUNCTION,
                                name.span,
                                &[&the_name],
                            );
                        }
                        let d = inf.core.dynamic_;
                        record_type(inf, cx, target, d);
                        for x in args.args.iter() {
                            infer_free(inf, cx, x.value);
                        }
                        return (d, false);
                    }
                    let t = infer(inf, cx, target, u);
                    let f = resolved_function(inf, cx, target);
                    prepare_entity(inf, name.span, f);
                    method_target(inf, f, name, span);
                    let (r, _) = invoke_value(inf, cx, (e, span), t, args, ctx, explicit);
                    inf.inference_entity = None;
                    inf.arity_target = None;
                    return (r, false);
                }
            // Static `C.m(args)` / `E.m(args)`. Only an identifier receiver
            // (`receiver is IdentifierImpl`, method_invocation_resolver.dart:129-143):
            // an instantiated type literal (`C<T>.m<U>()`) is an expression of
            // type `Type`, and the method is looked up on it.
            let instantiated = matches!(inf.program.unit(cx.unit).ast.expr(recv).kind, ExprKind::TypeArguments { .. });
            // The instantiated type literal of a function type alias
            // (`Fn<int>.foo()`, 3.6.2 `method_invocation_resolver.dart:169-182`):
            // no resolution is possible, `UNDEFINED_METHOD_ON_FUNCTION_TYPE` at the
            // name with the type's `qualifiedName`, and the recovery type.
            if let ExprKind::TypeArguments { target: base, .. } = &inf.program.unit(cx.unit).ast.expr(recv).kind
                && expr::function_type_alias(inf, cx, *base)
            {
                let base = *base;
                infer_free(inf, cx, recv);
                let sp_base = inf.span_expr(cx.unit, base);
                let qualified = inf.program.unit(cx.unit).source[sp_base.start..sp_base.end].to_string();
                let the_name = inf.interner.resolve(name.sym).to_string();
                inf.warning_with_code(
                    dartforge_diagnostics::codes::compile_time_error::UNDEFINED_METHOD_ON_FUNCTION_TYPE,
                    name.span,
                    &[&the_name, &qualified],
                );
                let d = inf.table.invalid(inf.core.dynamic_);
                record_type(inf, cx, target, d);
                for x in args.args.iter() {
                    infer_free(inf, cx, x.value);
                }
                return (d, false);
            }
            if !instantiated && let Some(rt) = type_reference(inf, cx, recv) {
                // `T?.m()` short-circuits except with `T` a simple class identifier
                // (`expr::shortening_waived`).
                let short_circuit_type = null_aware && !expr::shortening_waived(inf, cx, recv, &rt);
                if let TypeRef::Extension(x) = rt
                    && inf.extension_static_member(x, name.sym, false).is_none() {
                        if inf.program.extension(x).instance_members.contains_key(&name.sym) {
                            let msg = format!("{}: '{}'", STATIC_ACCESS_TO_INSTANCE_MEMBER.template, inf.interner.resolve(name.sym));
                            inf.warning(msg, name.span);
                            for arg in args.args.iter() {
                                infer_free(inf, cx, arg.value);
                            }
                            return (inf.core.dynamic_, false);
                        }
                        let extension = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                        let msg = format!("{}: '{}' in '{}'", UNDEFINED_EXTENSION_METHOD.template, inf.interner.resolve(name.sym), extension);
                        inf.warning(msg, name.span);
                        for arg in args.args.iter() {
                            infer_free(inf, cx, arg.value);
                        }
                        return (inf.core.dynamic_, false);
                    }
                // `C.m(args)` without a static member `m` nor an instance one (that one
                // is `STATIC_ACCESS_TO_INSTANCE_MEMBER`, through the access): the
                // analyzer reports `UNDEFINED_METHOD` with the element's **name**
                // (`_resolveReceiverTypeLiteral`,
                // an611:src/dart/resolver/method_invocation_resolver.dart:890-939;
                // an interface alias: the aliased element). `C.new()` and `C<T>.m()`
                // follow the common path.
                let cls = match &rt {
                    TypeRef::Class(c, _) | TypeRef::Alias(c, _, _) => Some(*c),
                    _ => None,
                };
                // `_resolveElement` (`:340-355`) only accepts a member accessible in
                // the calling library (`isAccessibleIn`): a private one from another
                // library does not exist there.
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
                    inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::UNDEFINED_METHOD, name.span, &[&the_name, &typ]);
                    // The receiver was already resolved by `ResolverVisitor` before
                    // `MethodInvocationResolver`: the element (the class or the alias)
                    // and the type literal's `Type` type.
                    let element = match &rt {
                        TypeRef::Alias(_, _, t) => Element::Typedef(*t),
                        _ => Element::Class(c),
                    };
                    expr::resolver(inf, cx, recv, Resolved::Element(element));
                    let type_type = inf.core.type_;
                    record_type(inf, cx, recv, type_type);
                    // `_setInvalidTypeResolution`: an invocation that does not resolve
                    // has the recovery type.
                    let d = inf.table.invalid(inf.core.dynamic_);
                    record_type(inf, cx, target, d);
                    for arg in args.args.iter() {
                        infer_free(inf, cx, arg.value);
                    }
                    return (d, false);
                }
                let t = infer(inf, cx, target, u);
                let t = if short_circuit_type { inf.non_null(t) } else { t };
                let f = resolved_function(inf, cx, target);
                prepare_entity(inf, name.span, f);
                method_target(inf, f, name, span);
                let (r, _) = invoke_value(inf, cx, (e, span), t, args, ctx, explicit);
                inf.inference_entity = None;
                inf.arity_target = None;
                return (r, short_circuit_type);
            }
            // `super.m(args)`.
            if matches!(a.expr(recv).kind, ExprKind::Super) {
                let this = cx.this_type.unwrap_or(inf.core.dynamic_);
                record_type(inf, cx, recv, this);
                let t = expr::super_member(inf, cx, target, name, expr::SuperUse::Invocation);
                // `super._f()` with `_f` a field: the read of `super._f` sees its
                // promotion (the `super` key, distinct from the `this` one).
                let t = expr::field_read(inf, cx, target, super::body::Base::Super, t);
                record_type(inf, cx, target, t);
                let f = resolved_function(inf, cx, target);
                method_target(inf, f, name, span);
                let (r, _) = invoke_value(inf, cx, (e, span), t, args, ctx, explicit);
                inf.arity_target = None;
                // `super?.m()`: null-aware as in `super?.x`.
                return (r, null_aware);
            }
            let (r_ty, short_circuit) = the_receiver(inf, cx, recv, null_aware);
            // `x?.m(args)` with `x` always null: the right side is dead, and the
            // first dead node is the argument list (from the `(`).
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
            // `void` and a nullable receiver: the same rules as property access
            // (`expr::propriedade`), in the invocation variant.
            if matches!(inf.table.get(r_ty), crate::table::Type::Void) {
                // In an invocation, the analyzer reports at the receiver (`this` in
                // `this.m()`); in a property access, at the name.
                // In a cascade section (`x..m()`), the report is at the cascade
                // target, made there.
                if !matches!(inf.program.unit(cx.unit).ast.expr(recv).kind, ExprKind::CascadeTarget) {
                    let sp = inf.span_expr(cx.unit, recv);
                    inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::USE_OF_VOID_RESULT, sp, &[]);
                }
                let d = inf.core.dynamic_;
                record_type(inf, cx, target, d);
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
                    dartforge_diagnostics::codes::compile_time_error::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE,
                    name.span,
                    &[&the_name],
                );
                inf.attach_non_promotion(since, cx, Some(recv), name.span);
            }
            let mut search = expr::lookup_target_member(inf, cx, recv, r_ty, name.sym, false);
            inf.report_extension_ambiguity(name.span);
            if matches!(search, Search::Missing) {
                if let Some((x, _)) = cx.overlays.get(&recv).cloned() {
                    if let Some(m) = inf.extension_static_member(x, name.sym, false) {
                        // The analyzer still resolves the method's signature, but rejects
                        // accessing it through `E(value).method()`.
                        inf.warning(
                            EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template.to_string(),
                            name.span,
                        );
                        search = Search::Finding(m);
                    }
                } else if let Some(m) = inf.instance_access_to_static(cx.lib, r_ty, name.sym, false, name.span) {
                    // The recovered static is the invocation's target
                    // (`method_invocation_resolver.dart:841-857`): a getter becomes a
                    // `FunctionExpressionInvocation` of the return type.
                    search = Search::Finding(m);
                }
            }
            match search {
                Search::Finding(m) => {
                    resolver(inf, cx, target, m.resolved.clone());
                    record_type(inf, cx, target, m.ty);
                    // The method name keeps the uninstantiated type (like the analyzer's
                    // `methodName.staticType`).
                    if m.method
                        && let Some(sym) = [inf.sym.remainder, inf.sym.clamp].into_iter().flatten().find(|s| *s == name.sym) {
                            let param = match inf.table.get(m.ty) {
                                Type::Function { positional, .. } => positional.first().copied(),
                                _ => None,
                            };
                            if let Some(pt) = param {
                                inf.pending_numeric_context = Some(expr::numeric_context(inf, r_ty, &m, sym, ctx, pt));
                            }
                        }
                    let (mut r, _) = if m.method {
                        let t = inf.non_null(m.ty);
                        prepare_entity(inf, name.span, m.function);
                        // `f.call(...)` with `f` of a function type: the entity of the
                        // required named one is the argument list.
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
                        let read_value = expr::property_read_type(inf, cx, target, m.ty);
                        let r = invoke_value(inf, cx, (e, span), read_value, args, ctx, explicit);
                        inf.arity_target = None;
                        r
                    };
                    if m.method {
                        // Numeric refinement of `remainder`/`clamp`.
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
                // An invocation on `InvalidType`: everything invalid, and the
                // arguments are inferred without a context
                // (`method_invocation_resolver.dart:459-463`).
                Search::Dynamic if inf.table.is_invalid(r_ty) => {
                    resolver(inf, cx, target, Resolved::Dynamic);
                    record_type(inf, cx, target, r_ty);
                    for x in args.args.iter() {
                        infer_free(inf, cx, x.value);
                    }
                    (r_ty, short_circuit)
                }
                Search::Dynamic => {
                    resolver(inf, cx, target, Resolved::Dynamic);
                    // `Object` methods keep their type even on `dynamic`.
                    let o = inf.core.object;
                    if let Some(m) = inf.interface_member(o, name.sym, false)
                        && m.method {
                            record_type(inf, cx, target, m.ty);
                            { let t = inf.interner.resolve(name.sym).to_string(); define_target(inf, Some(t), name.span); }
                            let (r, _) = invoke(inf, cx, m.ty, args, ctx, explicit);
                            return (r, short_circuit);
                        }
                    let d = inf.core.dynamic_;
                    record_type(inf, cx, target, d);
                    for x in args.args.iter() {
                        infer_free(inf, cx, x.value);
                    }
                    (inf.core.dynamic_, short_circuit)
                }
                Search::NeverType => {
                    let n = inf.core.never;
                    record_type(inf, cx, target, n);
                    for x in args.args.iter() {
                        infer_free(inf, cx, x.value);
                    }
                    (inf.core.never, short_circuit)
                }
                Search::Missing if check_null => {
                    let d = inf.core.dynamic_;
                    record_type(inf, cx, target, d);
                    for x in args.args.iter() {
                        infer_free(inf, cx, x.value);
                    }
                    (d, short_circuit)
                }
                Search::Missing => {
                    if let Some((x, _)) = cx.overlays.get(&recv).cloned() {
                        let extension = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                        let msg = format!("{}: '{}' in '{}'", UNDEFINED_EXTENSION_METHOD.template, inf.interner.resolve(name.sym), extension);
                        inf.warning(msg, name.span);
                        let d = inf.core.dynamic_;
                        record_type(inf, cx, target, d);
                        for x in args.args.iter() {
                            infer_free(inf, cx, x.value);
                        }
                        return (d, short_circuit);
                    }
                    // `{1}` is the element's name, not the type
                    // (`_resolveReceiverType`,
                    // an611:src/dart/resolver/method_invocation_resolver.dart:862-873).
                    let the_name = inf.interner.resolve(name.sym).to_string();
                    let typ = receiver_name(inf, r_ty);
                    inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::UNDEFINED_METHOD, name.span, &[&the_name, &typ]);
                    // `_setInvalidTypeResolution`
                    // (`method_invocation_resolver.dart:1062-1068`).
                    let d = inf.table.invalid(inf.core.dynamic_);
                    record_type(inf, cx, target, d);
                    for x in args.args.iter() {
                        infer_free(inf, cx, x.value);
                    }
                    (d, short_circuit)
                }
            }
        }
        _ => {
            // `assert(…)` as an expression: a `FunctionExpressionInvocation` of the
            // identifier `assert`, read as a name (`UNDEFINED_IDENTIFIER`) and
            // invoked as a value.
            let of_assert = a.assert_invocations.contains(&e);
            // `the_name(args)` without a getter in the lexical scope: the error is the
            // invocation's (`UNDEFINED_FUNCTION`/`UNDEFINED_METHOD`…), not the
            // undefined name's (see `expr::undefined_targetless_invocation`).
            if !of_assert
                && let ExprKind::Identifier(n) = &a.expr(target).kind
                && expr::undefined_targetless_invocation(inf, cx, *n)
            {
                let d = inf.table.invalid(inf.core.dynamic_);
                record_type(inf, cx, target, d);
                for x in args.args.iter() {
                    infer_free(inf, cx, x.value);
                }
                return (d, false);
            }
            // `T()` with `T` a type parameter (`_reportInvocationOfNonFunction`,
            // `method_invocation_resolver.dart:257-270`, from `_resolveReceiverNull`):
            // the element found in scope is neither an executable nor a variable. At
            // the name; the result is invalid.
            if !of_assert
                && let ExprKind::Identifier(n) = &a.expr(target).kind
                && (matches!(cx.lookup(n.sym), Some(super::body::MemberName::TypeParam(_)))
                    || (cx.lookup(n.sym).is_none() && expr::non_class_type_alias(inf, cx, target)))
            {
                let the_name = inf.interner.resolve(n.sym).to_string();
                inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::INVOCATION_OF_NON_FUNCTION, n.span, &[&the_name]);
                // The `methodName` keeps the type parameter as its element:
                // `visitSimpleIdentifier` reports the class's one in a static member
                // (`error_verifier.dart:1421`).
                if cx.static_member
                    && let Some(super::body::MemberName::TypeParam(p)) = cx.lookup(n.sym)
                    && crate::resolve::class_param(inf.table, p)
                {
                    inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::TYPE_PARAMETER_REFERENCED_BY_STATIC, n.span, &[]);
                }
                let d = inf.table.invalid(inf.core.dynamic_);
                record_type(inf, cx, target, d);
                for x in args.args.iter() {
                    infer_free(inf, cx, x.value);
                }
                return (d, false);
            }
            // The invocation belongs to the callee's null-shorting chain
            // (`a?.f(x)()`, `a?.l[0]()`: a `FunctionExpressionInvocation` is a
            // `NullShortableExpression`); a parenthesized callee closes its own.
            let (t, short_circuit) = expr::infer_at(inf, cx, target, u, true);
            if let Some((x, ext_args)) = cx.overlays.get(&target).cloned() {
                if let Some(call) = inf.sym.call {
                    if let Some(m) = inf.explicit_extension_member(x, &ext_args, call, false) {
                        define_target(inf, None, span);
                        let (r, _) = invoke(inf, cx, m.ty, args, ctx, explicit);
                        return (r, false);
                    }
                    if let Some(m) = inf.extension_static_member(x, call, false) {
                        // `E(value)()` uses the argument list as the error's location, as the
                        // official analyzer does.
                        inf.warning(
                            EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template.to_string(),
                            args.span,
                        );
                        define_target(inf, None, span);
                        let (r, _) = invoke(inf, cx, m.ty, args, ctx, explicit);
                        return (r, false);
                    }
                }
                let extension = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                let msg = format!("{}: '{}'", INVOCATION_OF_EXTENSION_WITHOUT_CALL.template, extension);
                inf.warning(msg, a.expr(target).span);
                for arg in args.args.iter() {
                    infer_free(inf, cx, arg.value);
                }
                return (inf.core.dynamic_, false);
            }
            if !of_assert && let ExprKind::Identifier(n) = &a.expr(target).kind {
                let f = resolved_function(inf, cx, target);
                prepare_entity(inf, a.expr(target).span, f);
                // A function (top-level, method, local) is a `MethodInvocation` (the
                // entity is the name); a variable or getter becomes a
                // `FunctionExpressionInvocation` (the whole call). The name is the
                // identifier in both cases.
                let function = match f {
                    Some(f) => matches!(inf.program.function(f).kind, FunctionKind::Function),
                    None => match cx.lookup(n.sym) {
                        Some(super::body::MemberName::Local(id)) => local_is_function(inf, cx, cx.locals[id.0 as usize].offset),
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
                // `.m(…)` of a static method is a `DotShorthandInvocation`, with the
                // method's name and the entity at the name.
                let (the_name, sp) = (inf.interner.resolve(name.sym).to_string(), name.span);
                define_target(inf, Some(the_name), sp);
            } else {
                define_target(inf, None, span);
            }
            let (r, _) = invoke_value(inf, cx, (e, span), t, args, ctx, explicit);
            inf.inference_entity = None;
            inf.arity_target = None;
            (r, short_circuit)
        }
    }
}

/// Arity target of `x.m(...)`/`p.f(...)`/`super.m(...)`: a method (or
/// function) is a `MethodInvocation` (name and entity at `m`); a getter or
/// variable becomes a `FunctionExpressionInvocation` over the access
/// (without a name, the whole call).
fn method_target(inf: &mut BodyInferrer<'_>, f: Option<FunctionElementId>, name: ast::Name, span: Span) {
    if f.is_some_and(|f| matches!(inf.program.function(f).kind, FunctionKind::Function)) {
        { let t = inf.interner.resolve(name.sym).to_string(); define_target(inf, Some(t), name.span); }
    } else {
        define_target(inf, None, span);
    }
}

/// The `{1}` of an instance `UNDEFINED_METHOD`: the name of the element of
/// the receiver's type (without type arguments), `'Function'` for function
/// types and `'<unknown>'` for the rest (records, type parameters).
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

/// The local declared at `offset` is a local function (the name is
/// followed by `(` or `<` in the declaration).
fn local_is_function(inf: &BodyInferrer<'_>, cx: &Body, offset: usize) -> bool {
    let source = &inf.program.unit(cx.unit).source;
    let rest = source.get(offset..).unwrap_or("");
    let after = rest.trim_start_matches(|c: char| c.is_alphanumeric() || c == '_' || c == '$').trim_start();
    after.starts_with('(') || after.starts_with('<')
}

/// The last identifier of a type reference (`C`, `p.C`, `C<int>`).
fn last_identifier(inf: &BodyInferrer<'_>, cx: &Body, e: ExprId) -> Option<dartforge_intern::SymbolId> {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.expr(e).kind {
        ExprKind::Identifier(n) => Some(n.sym),
        ExprKind::Property { name, .. } => Some(name.sym),
        ExprKind::TypeArguments { target, .. } => last_identifier(inf, cx, *target),
        _ => None,
    }
}

/// If `target` (the target of a call) names a constructor: `(cls, ctor, args explícitos)`.
fn target_constructor(inf: &mut BodyInferrer<'_>, cx: &mut Body, target: ExprId) -> Option<(ClassId, Option<FunctionElementId>, Option<Vec<TypeId>>)> {
    let empty = inf.sym.empty?;
    let the_type_args = |inf: &mut BodyInferrer<'_>, cx: &Body, rt: &TypeRef| -> (ClassId, Option<Vec<TypeId>>) {
        match rt {
            TypeRef::Class(c, Some(ts)) => (*c, Some(ts.iter().map(|&t| inf.type_argument_type(cx, t)).collect())),
            TypeRef::Class(c, None) => (*c, None),
            TypeRef::Alias(c, args, _) => (*c, args.clone()),
            TypeRef::Extension(_) => unreachable!(),
        }
    };
    if let Some(rt) = type_reference(inf, cx, target) {
        if matches!(rt, TypeRef::Extension(_)) {
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
        if matches!(rt, TypeRef::Extension(_)) {
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

/// Records the type/resolution of the nodes of a class reference.
fn record_reference(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) {
    let a = &inf.program.unit(cx.unit).ast;
    let tt = inf.core.type_;
    match &a.expr(e).kind {
        ExprKind::Identifier(n) => {
            if let Some(el) = inf.program.lookup_in_unit(cx.unit, n.sym).and_then(|b| b.getter) {
                resolver(inf, cx, e, Resolved::Element(el));
            }
            record_type(inf, cx, e, tt);
        }
        ExprKind::Property { target, name, .. } => {
            let (target, name) = (*target, *name);
            if let ExprKind::Identifier(p) = &a.expr(target).kind {
                resolver(inf, cx, target, Resolved::Prefix(cx.lib));
                if let Some(el) = inf.program.lookup_prefixed_in_unit(cx.unit, p.sym, name.sym).and_then(|b| b.getter) {
                    resolver(inf, cx, e, Resolved::Element(el));
                }
            }
            record_type(inf, cx, e, tt);
        }
        ExprKind::TypeArguments { target, .. } => {
            let t = *target;
            record_reference(inf, cx, t);
            record_type(inf, cx, e, tt);
        }
        _ => {}
    }
}

impl<'a> BodyInferrer<'a> {
    /// New parameters (per class) for inferring constructors' type arguments:
    /// the class's own may be in scope at the call site (`C(x)` inside `C<T>`),
    /// and must not be confused.
    /// Constructor `the_name` of `c`. A mixin application (`class A = B with M;`)
    /// has the superclass's generative constructors forwarded (specification,
    /// "Mixin Application").
    pub(crate) fn constructor_of(&self, c: ClassId, the_name: dartforge_intern::SymbolId) -> Option<FunctionElementId> {
        let cl = self.program.class(c);
        if let Some(&f) = cl.constructors.get(&the_name) {
            return Some(f);
        }
        if cl.kind != ClassKind::MixinApplication {
            return None;
        }
        let f = self.constructor_of(cl.supertype_class?, the_name)?;
        (!self.program.function(f).factory).then_some(f)
    }

    /// Constructor `the_name` of `c`, or `Some(None)` for the primary constructor of
    /// an extension type (`extension type Id(int v)`, R-EXT-04), which the
    /// element model does not create.
    pub(crate) fn constructor_or_primary(&self, c: ClassId, the_name: dartforge_intern::SymbolId) -> Option<Option<FunctionElementId>> {
        if let Some(f) = self.constructor_of(c, the_name) {
            return Some(Some(f));
        }
        let cl = self.program.class(c);
        if cl.kind != ClassKind::ExtensionType {
            return None;
        }
        let d = cl.decl?;
        match &self.program.unit(d.unit).ast.decl(d.decl).kind {
            ast::DeclKind::ExtensionType(et) if et.constructor.map(|n| n.sym).or(self.sym.empty) == Some(the_name) => Some(None),
            _ => None,
        }
    }

    /// [`Self::constructor_or_primary`] through the analyzer's
    /// `lookUpConstructor(the_name, library)`: a private constructor from another
    /// library is not found (`isAccessibleIn`).
    pub(crate) fn visible_constructor(&self, c: ClassId, the_name: dartforge_intern::SymbolId, lib: dartforge_elements::model::LibraryId) -> Option<Option<FunctionElementId>> {
        if self.interner.resolve(the_name).starts_with('_') && self.program.class(c).library != lib {
            return None;
        }
        self.constructor_or_primary(c, the_name)
    }

    /// `(Representação) -> E<parâmetros>`: the primary constructor.
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

    /// Signature of constructor `f` as seen from `c` (forwarded when `f` belongs
    /// to the superclass of a mixin application): types from the supertype's
    /// arguments and return `c<parâmetros>`.
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
            let map = self.map(&params, &args);
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

/// Constructor invocation: infers the class's type arguments when omitted
/// (like a generic function over the class's parameters).
#[allow(clippy::too_many_arguments)]
pub(crate) fn build(
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

/// The generic alias named in the creation (`target`: `A`, `p.A`, `A.the_name`,
/// `p.A.the_name`) without written type arguments, whose arguments are inferred
/// through its parameters ([`inferable_alias`]).
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

/// A generic class alias that does more than rename the parameters
/// (`typedef D<X> = C<List<X>>`, or `typedef A<X extends B> = C<X>` with a
/// bound different from the class's): without written arguments, a creation
/// through it is inferred as a generic function over the alias's parameters
/// (`ConstructorElementToInfer`, 3.6.2 `invocation_inferrer.dart`), with
/// their bounds (`COULD_NOT_INFER`), not instantiated to bounds.
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
    // Renaming: equivalent to the class only with the same bounds.
    let formals = inf.outline.classes[class.0 as usize].type_params.to_vec();
    let map = inf.map(&formals, &args);
    formals.iter().zip(params.iter()).any(|(&f, &p)| {
        let bf = inf.table.param(f).bound;
        let bf = inf.subst(bf, &map);
        bf != inf.table.param(p).bound
    })
}

/// [`build`] with the inferable alias ([`inferable_alias`]) through which
/// the class was named: the inferred parameters are the alias's and the
/// signature is the constructor's with the arguments of the alias's target.
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
    // A forwarded constructor of a mixin application: no resolution (the
    // element is the superclass's). `None`: the primary constructor of an
    // extension type (R-EXT-04), without an element.
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
            let map = inf.map(&formals, &target_args);
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
        let map = inf.map(&original, &ex);
        let s = inf.subst(sig, &map);
        let (r, _) = invoke(inf, cx, s, args, ctx, None);
        return r;
    }
    // Generic function `<T'..>(params) -> C<T'..>`.
    let types: Vec<TypeId> = new_.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
    let map = inf.map(&original, &types);
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
    // Context `C<...>?` or a supertype: the return constraint takes care
    // of it.
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
    // The instantiation recorded by `invoke` is that of the alias's
    // parameters; consumers read it as the class's arguments.
    if alias.is_some()
        && let Type::Interface { class, args: ca, .. } | Type::ExtensionType { decl: class, args: ca, .. } = inf.table.get(r).clone()
        && class == c
    {
        inf.body_types.units[cx.unit.0 as usize].set_instantiation(args.span.start, ca.to_vec().into_boxed_slice());
    }
    r
}

/// `AstRewriter.instanceCreationExpression` (3.6.2 `ast_rewrite.dart:38-118`)
/// over the implicit creation `X<T>.the_name(…)`: it stays a creation unless the
/// unprefixed `X` is a function, method or accessor in scope (it becomes an
/// invocation of the reference), or `p.X` has a `p` that is not an import
/// prefix, or `X` is a function in the prefix. (The caller already split off
/// the function type alias.)
fn stays_implicit_creation(inf: &mut BodyInferrer<'_>, cx: &Body, base: ExprId) -> bool {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.expr(base).kind {
        ExprKind::Identifier(n) => match resolve_name(inf, cx, n.sym, false) {
            NameRef::LexicalMember(..) => false,
            NameRef::Element(Element::Function(_)) => false,
            NameRef::Local(id) => {
                let l = cx.local(id);
                !(l.local_function || cx.local_functions.contains(&id))
            }
            _ => true,
        },
        ExprKind::Property { target: p, name, .. } => {
            let ExprKind::Identifier(p) = &a.expr(*p).kind else { return false };
            if !matches!(resolve_name(inf, cx, p.sym, false), NameRef::Prefix) {
                return false;
            }
            !matches!(
                inf.program.lookup_prefixed_in_unit(cx.unit, p.sym, name.sym).and_then(|b| b.getter),
                Some(Element::Function(f)) if inf.program.function(f).kind == FunctionKind::Function
            )
        }
        _ => false,
    }
}

/// `CREATION_WITH_NON_TYPE` (`new X()`, `const X()`, `X<T>.the_name()`): the name
/// written as the creation's type is not a class (undefined, local, type
/// parameter, function). The range is the written name; with `p.X` and `p` an
/// import prefix, only the `X`. The argument is the last part of the name
/// (`new A.foo()` with `A` undefined: `'foo'`).
///
/// Left out (the analyzer reports another code or nothing):
/// * `p.X` with `p` declared but not a prefix (`void p() {}`):
///   `prefix_shadowed_by_local_declaration`;
/// * `p.X` with `p` the prefix of an import that did not resolve: nothing.
fn creation_without_class(inf: &mut BodyInferrer<'_>, cx: &Body, name: &[ast::Name]) {
    let (Some(first), Some(last)) = (name.first(), name.last()) else { return };
    let mut prefix = false;
    if name.len() == 2 {
        if inf.program.prefixes_in_unit(cx.unit).contains_key(&first.sym) {
            // `shouldIgnoreUndefinedNamedType`
            // (`an611:src/dart/resolver/named_type_resolver.dart:310`): the prefix
            // of an import that does not exist.
            if crate::scope::should_ignore_undefined(inf.program, inf.interner, cx.unit, Some(first.sym), last.sym) {
                return;
            }
            prefix = true;
        } else if inf.program.lookup_in_unit(cx.unit, first.sym).is_some_and(|b| {
            !matches!(b.getter, Some(Element::Class(_)))
        }) || unresolved_import_prefix(inf, cx, first.sym)
        {
            return;
        }
    }
    if name.len() == 1
        && inf.program.lookup_in_unit(cx.unit, first.sym).is_none()
        && crate::scope::should_ignore_undefined(inf.program, inf.interner, cx.unit, None, first.sym)
    {
        return;
    }
    let start = if prefix { last.span.start } else { first.span.start };
    let text = inf.interner.resolve(last.sym).to_string();
    inf.warning_with_code(
        dartforge_diagnostics::codes::compile_time_error::NEW_WITH_NON_TYPE,
        Span { start, end: last.span.end },
        &[&text],
    );
    // The creation's `NamedType` resolves to the type parameter:
    // `ErrorVerifier`'s `visitNamedType` reports the class's one in a
    // static member (`error_verifier.dart:1266-1275`, `:5419-5434`).
    if name.len() == 1
        && cx.static_member
        && let Some(&p) = cx.visible_type_parameters().get(&first.sym)
        && crate::resolve::class_param(inf.table, p)
    {
        inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::TYPE_PARAMETER_REFERENCED_BY_STATIC, first.span, &[]);
    }
}

/// `p` is the prefix of some `import … as p` directive of the library, but
/// did not reach the scope (the import's target does not exist).
fn unresolved_import_prefix(inf: &BodyInferrer<'_>, cx: &Body, p: dartforge_intern::SymbolId) -> bool {
    let lib = inf.program.library(cx.lib);
    !inf.program.prefixes_in_unit(cx.unit).contains_key(&p)
        && lib.units.iter().any(|u| {
            inf.program.unit(*u).unit.directives.iter().any(|d| {
                matches!(&d.kind, ast::DirectiveKind::Import { prefix: Some(n), .. } if n.sym == p)
            })
        })
}

/// `_checkForConstWithUndefinedConstructor` (`error_verifier.dart:3020-3045`):
/// `const C.the_name()` without the constructor is
/// `CONST_WITH_UNDEFINED_CONSTRUCTOR` at the name (with the written qualified
/// name and the constructor's); `const C()` without the unnamed one,
/// `CONST_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT` on the whole named type.
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

/// `_checkForConstWithUndefinedConstructor` and
/// `_checkForNewWithUndefinedConstructor` (`error_verifier.dart:4595-4625`):
/// at the constructor's name (`[namedType.qualifiedName, the_name]`), or, without
/// a name, on the whole `constructorName`, which is the type.
fn undefined_constructor(
    inf: &mut BodyInferrer<'_>,
    a: &ast::Ast,
    ty: ast::TypeId,
    name: &[ast::Name],
    constructor: Option<ast::Name>,
    ctor_of_name: bool,
    constant: bool,
) {
    use dartforge_diagnostics::codes::compile_time_error as c;
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

/// `new C<T>.the_name(args)` / `const C(args)`.
pub(crate) fn instantiation(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, ctx: TypeId) -> TypeId {
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
    // `NamedTypeResolver.resolve` (`named_type_resolver.dart:89-121`): the
    // first name of `p.N` that, in scope, is not a prefix, class or alias (a
    // local, a type parameter, a member, a top-level function).
    if name.len() == 2 && name[0].span.start != name[0].span.end {
        let p = name[0].sym;
        let local = cx.visible_type_parameters().contains_key(&p) || cx.visible_locals(inf.interner).contains_key(&p);
        if crate::resolve::shadowed_prefix(inf.program, inf.interner, cx.unit, p, local, cx.class, cx.extension) {
            let the_name = inf.interner.resolve(p).to_string();
            inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::PREFIX_SHADOWED_BY_LOCAL_DECLARATION, name[0].span, &[&the_name]);
            for x in args.args.iter() {
                infer_free(inf, cx, x.value);
            }
            return inf.table.invalid(inf.core.dynamic_);
        }
    }
    // The simple name is looked up in the lexical scope (`nameScope`): a local,
    // a type parameter or a member of the class or extension (`const B()` with
    // the method `B`) hides the top-level type, and the creation is of a
    // non-type (`_isInstanceCreation`, `named_type_resolver.dart:274-276`).
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
        inf.program.lookup_prefixed_in_unit(cx.unit, name[0].sym, name[1].sym)
    } else {
        inf.program.lookup_in_unit(cx.unit, name[0].sym)
    };
    // `new C.the_name()` arrives as a two-part type when `C` is not a prefix.
    let (binding, constructor, ctor_of_name) = match (binding, name.len()) {
        (None, 2) => match inf.program.lookup_in_unit(cx.unit, name[0].sym) {
            Some(b) if matches!(b.getter, Some(Element::Class(_))) && constructor.is_none() => (Some(b), Some(name[1]), true),
            _ => (binding, constructor, false),
        },
        _ => (binding, constructor, false),
    };
    // `_verifyTypeAliasForContext` (`named_type_resolver.dart:420-450`): an
    // alias whose type is a type parameter (`typedef A<T> = T`, also through
    // another alias) is not instantiated; at the name, without the arguments.
    {
        let parameter_alias = |inf: &BodyInferrer<'_>, el: Option<Element>| match el {
            Some(Element::Typedef(td)) => matches!(inf.table.get(inf.outline.typedefs[td.0 as usize].target_type), Type::TypeParameter { .. }),
            _ => false,
        };
        let range = if parameter_alias(inf, binding.and_then(|b| b.getter)) {
            name.first().zip(name.last()).map(|(p, n)| Span { start: p.span.start, end: n.span.end })
        } else if binding.is_none() && name.len() == 2 && parameter_alias(inf, inf.program.lookup_in_unit(cx.unit, name[0].sym).and_then(|b| b.getter)) {
            Some(name[0].span)
        } else {
            None
        };
        if let Some(sp) = range {
            inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::INSTANTIATE_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER, sp, &[]);
            for x in args.args.iter() {
                infer_free(inf, cx, x.value);
            }
            return inf.core.dynamic_;
        }
    }
    let mut explicit_alias = None;
    let (c, explicit) = match binding.and_then(|b| b.getter) {
        // `new C.the_name<T>()`: read as `prefix.Type<T>`; the list belongs to the
        // constructor (`_rewriteToConstructorName`).
        Some(Element::Class(c)) if ctor_of_name && !targs.is_empty() => {
            if let Some(n) = constructor {
                types_in_constructor(inf, cx.unit, name[0].span, n, targs);
            }
            (c, None)
        }
        Some(Element::Class(c)) => {
            let mut ex = if targs.is_empty() { None } else { Some(targs.iter().map(|&t| inf.type_argument_type(cx, t)).collect::<Vec<_>>()) };
            // `NamedTypeResolver._buildTypeArguments`
            // (`named_type_resolver.dart:137-148`): wrong count on the whole named
            // type, and the arguments become `InvalidType`.
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
                    // `typedef F = void Function()`: the alias does not name a class.
                    // Other targets (`typedef T = dynamic`, `typedef A<X> = X`, which is
                    // `instantiate_type_alias_expands_…`) stay silent.
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
        _ => inf.sym.empty,
    };
    let f = key.and_then(|k| inf.visible_constructor(c, k, cx.lib));
    // `new E()` / `const E()`: the enum's generative constructor (or the
    // implicit unnamed one) does not instantiate outside the creation of
    // constants. An explicit name without a target (`const E.foo()`) is
    // another diagnostic (`const_with_undefined_constructor`, out of scope)
    // and stays silent. Factories follow the normal path (`construir`). The
    // range is the constructor's name, or the class's when it is implicit.
    // `new M()` / `const M()` with `M` a mixin (`error_verifier.dart:2976-2984`):
    // at the named type; a constructor the mixin does not have stays silent.
    if inf.program.class(c).kind == ClassKind::Mixin {
        let sp = if constructor.is_some() && ctor_of_name { name[0].span } else { a.ty(ty).span };
        inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::MIXIN_INSTANTIATE, sp, &[]);
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
        inf.warning(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template.to_string(), span);
        for x in args.args.iter() {
            infer_free(inf, cx, x.value);
        }
        return inf.core.dynamic_;
    }
    let Some(f) = f else {
        // Without a declared constructor, the implicit unnamed one exists (the
        // synthetic element): nothing to report for it.
        // In a mixin application the constructors are the generative ones
        // forwarded from the superclass: the implicit unnamed one only exists
        // if the superclass (recursively) declares no constructor.
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
        // Without a declared constructor: the implicit default (generative).
        if constructor.is_none() && inf.program.class(c).constructors.is_empty() {
            let k = inf.program.class(c);
            if matches!(k.kind, ClassKind::Class | ClassKind::MixinApplication) && (k.modifiers.abstract_ || k.modifiers.sealed) {
                let sp = a.ty(ty).span;
                inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::INSTANTIATE_ABSTRACT_CLASS, sp, &[]);
            }
        }
        for x in args.args.iter() {
            infer_free(inf, cx, x.value);
        }
        return inf.class_type_with_args(c, explicit.unwrap_or_default());
    };
    if let Some(fid) = f {
        // The named type: without the constructor's name that came as the 2nd
        // part.
        let sp = if constructor.is_some() && ctor_of_name {
            name[0].span
        } else {
            a.ty(ty).span
        };
        warn_abstract_class(inf, c, fid, sp);
    }
    // `constructorName`: the type (with prefix and arguments) and the
    // `.the_name`; the cited name is the constructor's, or `C.new` (without the
    // prefix).
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

/// The range of the named type of an implicit creation: `A`, `p.A` or
/// `A<int>` (up to the `>`).
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
            // `A<int>()`: the type arguments come in the call's list.
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
/// `an611:src/generated/error_verifier.dart:2950-2973`): creation of an
/// abstract (or `sealed`) class through a constructor that is not a
/// `factory`, at the named type.
fn warn_abstract_class(inf: &mut BodyInferrer<'_>, c: ClassId, f: FunctionElementId, span: Span) {
    let k = inf.program.class(c);
    if !matches!(k.kind, ClassKind::Class | ClassKind::MixinApplication) || !(k.modifiers.abstract_ || k.modifiers.sealed) {
        return;
    }
    if inf.program.function(f).factory {
        return;
    }
    inf.warning_with_code(dartforge_diagnostics::codes::compile_time_error::INSTANTIATE_ABSTRACT_CLASS, span, &[]);
}
