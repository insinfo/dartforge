//! Limites superior e inferior: **UP** e **DOWN** de
//! `references/dart-language/resources/type-system/upper-lower-bounds.md`,
//! com os predicados auxiliares da especificação de null safety (**TOP**,
//! **OBJECT**, **BOTTOM**, **NULL**, **MORETOP**, **MOREBOTTOM**) e a extensão
//! para esquemas (`inference.md`, "Upper bound": `UP(T, _) = T`).
//!
//! As regras são avaliadas de cima para baixo, na ordem do documento; a
//! primeira que casa decide.

use crate::ops::{non_nullable, nullable};
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{Type, TypeId, TypeParamId};
use std::collections::HashMap;

/// **TOP**(`T`): `dynamic`, `void`, `S?` com **TOP**/**OBJECT**(`S`), `FutureOr<S>` com **TOP**(`S`).
pub fn is_top(t: TypeId, env: &SubtypeEnv) -> bool {
    match env.table.get(t) {
        Type::Dynamic | Type::Void => true,
        Type::Interface { class, nullable: true, .. } => Some(*class) == env.core.object_class,
        Type::FutureOr { arg, nullable } => {
            is_top(*arg, env) || (*nullable && is_object(*arg, env))
        }
        _ => false,
    }
}

/// **OBJECT**(`T`): `Object`, `FutureOr<S>` com **OBJECT**(`S`).
pub fn is_object(t: TypeId, env: &SubtypeEnv) -> bool {
    match env.table.get(t) {
        Type::Interface { class, nullable: false, .. } => Some(*class) == env.core.object_class,
        Type::FutureOr { arg, nullable: false } => is_object(*arg, env),
        _ => false,
    }
}

/// **BOTTOM**(`T`): `Never`, `X extends S` / `X & S` com **BOTTOM**(`S`).
pub fn is_bottom(t: TypeId, env: &SubtypeEnv) -> bool {
    match env.table.get(t) {
        Type::Never => true,
        Type::Intersection { bound, .. } => is_bottom(*bound, env),
        Type::TypeParameter { param, nullable: false } if *param != env.core.unknown_param => {
            let b = env.table.param(*param).bound;
            b != t && matches!(env.table.get(b), Type::Never)
        }
        _ => false,
    }
}

/// **NULL**(`T`): `Null`, `S?` com **BOTTOM**(`S`).
pub fn is_null(t: TypeId, env: &SubtypeEnv) -> bool {
    match env.table.get(t) {
        Type::Null => true,
        Type::TypeParameter { param, nullable: true } if *param != env.core.unknown_param => {
            let b = env.table.param(*param).bound;
            matches!(env.table.get(b), Type::Never)
        }
        _ => false,
    }
}

/// **MORETOP**(`S`, `T`): ordem total sobre tipos topo e `Object`.
fn more_top(s: TypeId, t: TypeId, env: &mut SubtypeEnv) -> bool {
    let (ts, tt) = (env.table.get(s).clone(), env.table.get(t).clone());
    if matches!(ts, Type::Void) {
        return true;
    }
    if matches!(tt, Type::Void) {
        return false;
    }
    if matches!(ts, Type::Dynamic) {
        return true;
    }
    if matches!(tt, Type::Dynamic) {
        return false;
    }
    if s == env.core.object {
        return true;
    }
    if t == env.core.object {
        return false;
    }
    let (sn, tn) = (ts.is_declared_nullable(), tt.is_declared_nullable());
    if sn && tn {
        let (s1, t1) = (non_nullable(s, env.table), non_nullable(t, env.table));
        return more_top(s1, t1, env);
    }
    if tn {
        return true;
    }
    if sn {
        return false;
    }
    if let (Type::FutureOr { arg: a, .. }, Type::FutureOr { arg: b, .. }) = (ts, tt) {
        return more_top(a, b, env);
    }
    false
}

/// **MOREBOTTOM**(`S`, `T`): ordem (quase) total sobre tipos fundo e `Null`.
fn more_bottom(s: TypeId, t: TypeId, env: &mut SubtypeEnv) -> bool {
    let (ts, tt) = (env.table.get(s).clone(), env.table.get(t).clone());
    if matches!(ts, Type::Never) {
        return true;
    }
    if matches!(tt, Type::Never) {
        return false;
    }
    if matches!(ts, Type::Null) {
        return true;
    }
    if matches!(tt, Type::Null) {
        return false;
    }
    let (sn, tn) = (ts.is_declared_nullable(), tt.is_declared_nullable());
    if sn && tn {
        let (s1, t1) = (non_nullable(s, env.table), non_nullable(t, env.table));
        return more_bottom(s1, t1, env);
    }
    if tn {
        return true;
    }
    if sn {
        return false;
    }
    if let (Type::TypeParameter { param: a, .. }, Type::TypeParameter { param: b, .. }) = (ts, tt) {
        let (ba, bb) = (env.table.param(a).bound, env.table.param(b).bound);
        return more_bottom(ba, bb, env);
    }
    false
}

/// `Null <: t`.
pub fn is_nullable(t: TypeId, env: &mut SubtypeEnv) -> bool {
    is_subtype(env.core.null, t, env)
}

/// `t <: Object`.
pub fn is_non_nullable(t: TypeId, env: &mut SubtypeEnv) -> bool {
    is_subtype(t, env.core.object, env)
}

/// Fecho maior de `t` em relação a `params` (`inference.md`, "Type variable
/// elimination"): ocorrências covariantes viram `Object?`, contravariantes `Never`.
pub fn greatest_closure(t: TypeId, params: &[TypeParamId], env: &mut SubtypeEnv) -> TypeId {
    closure(t, params, true, env)
}

/// Fecho menor de `t` em relação a `params`.
pub fn least_closure(t: TypeId, params: &[TypeParamId], env: &mut SubtypeEnv) -> TypeId {
    closure(t, params, false, env)
}

fn contains_params(t: TypeId, params: &[TypeParamId], env: &SubtypeEnv) -> bool {
    match env.table.get(t) {
        Type::TypeParameter { param, .. } => params.contains(param),
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => {
            args.iter().any(|a| contains_params(*a, params, env))
        }
        Type::FutureOr { arg, .. } => contains_params(*arg, params, env),
        Type::Function { type_params, ret, positional, optional, named, .. } => {
            contains_params(*ret, params, env)
                || positional.iter().chain(optional.iter()).any(|a| contains_params(*a, params, env))
                || named.iter().any(|(_, a, _)| contains_params(*a, params, env))
                || type_params.iter().any(|p| {
                    let b = env.table.param(*p).bound;
                    contains_params(b, params, env)
                })
        }
        Type::Record { positional, named, .. } => {
            positional.iter().any(|a| contains_params(*a, params, env))
                || named.iter().any(|(_, a)| contains_params(*a, params, env))
        }
        _ => false,
    }
}

fn closure(t: TypeId, params: &[TypeParamId], greatest: bool, env: &mut SubtypeEnv) -> TypeId {
    if !contains_params(t, params, env) {
        return t;
    }
    let ty = env.table.get(t).clone();
    match ty {
        Type::TypeParameter { nullable: n, .. } => {
            let r = if greatest { env.core.object_nullable } else { env.core.never };
            if n { nullable(r, env.table) } else { r }
        }
        Type::Interface { class, args, nullable: n } => {
            let args: Vec<TypeId> = args.iter().map(|a| closure(*a, params, greatest, env)).collect();
            env.table.intern(Type::Interface { class, args: args.into_boxed_slice(), nullable: n })
        }
        Type::ExtensionType { decl, args, nullable: n } => {
            let args: Vec<TypeId> = args.iter().map(|a| closure(*a, params, greatest, env)).collect();
            env.table.intern(Type::ExtensionType { decl, args: args.into_boxed_slice(), nullable: n })
        }
        Type::FutureOr { arg, nullable: n } => {
            let arg = closure(arg, params, greatest, env);
            env.table.intern(Type::FutureOr { arg, nullable: n })
        }
        Type::Function { type_params, ret, positional, optional, named, nullable: n } => {
            if type_params.iter().any(|p| {
                let b = env.table.param(*p).bound;
                contains_params(b, params, env)
            }) {
                return if greatest { env.core.function } else { env.core.never };
            }
            let ret = closure(ret, params, greatest, env);
            let positional: Vec<TypeId> = positional.iter().map(|a| closure(*a, params, !greatest, env)).collect();
            let optional: Vec<TypeId> = optional.iter().map(|a| closure(*a, params, !greatest, env)).collect();
            let named: Vec<_> = named.iter().map(|(s, a, r)| (*s, closure(*a, params, !greatest, env), *r)).collect();
            env.table.intern(Type::Function {
                type_params,
                ret,
                positional: positional.into_boxed_slice(),
                optional: optional.into_boxed_slice(),
                named: named.into_boxed_slice(),
                nullable: n,
            })
        }
        Type::Record { positional, named, nullable: n } => {
            let positional: Vec<TypeId> = positional.iter().map(|a| closure(*a, params, greatest, env)).collect();
            let named: Vec<_> = named.iter().map(|(s, a)| (*s, closure(*a, params, greatest, env))).collect();
            env.table.intern(Type::Record { positional: positional.into_boxed_slice(), named: named.into_boxed_slice(), nullable: n })
        }
        _ => t,
    }
}

/// `t` contém o desconhecido `_`.
pub fn has_unknown(t: TypeId, env: &SubtypeEnv) -> bool {
    contains_params(t, &[env.core.unknown_param], env)
}

/// Fecho maior de um esquema em relação a `_`.
pub fn schema_greatest(t: TypeId, env: &mut SubtypeEnv) -> TypeId {
    let u = env.core.unknown_param;
    greatest_closure(t, &[u], env)
}

/// Fecho menor de um esquema em relação a `_`.
pub fn schema_least(t: TypeId, env: &mut SubtypeEnv) -> TypeId {
    let u = env.core.unknown_param;
    least_closure(t, &[u], env)
}

fn is_function(t: TypeId, env: &SubtypeEnv) -> bool {
    matches!(env.table.get(t), Type::Function { nullable: false, .. })
}

fn is_record(t: TypeId, env: &SubtypeEnv) -> bool {
    matches!(env.table.get(t), Type::Record { nullable: false, .. })
}

fn is_iface(t: TypeId, env: &SubtypeEnv, c: Option<dartforge_elements::model::ClassId>) -> bool {
    match env.table.get(t) {
        Type::Interface { class, nullable: false, .. } => Some(*class) == c,
        _ => false,
    }
}

/// Subtipagem entre esquemas para UP: fechos menores (`inference.md`).
fn sub_up(a: TypeId, b: TypeId, env: &mut SubtypeEnv) -> bool {
    let a = schema_least(a, env);
    let b = schema_least(b, env);
    is_subtype(a, b, env)
}

/// Subtipagem entre esquemas para DOWN: fechos maiores.
fn sub_down(a: TypeId, b: TypeId, env: &mut SubtypeEnv) -> bool {
    let a = schema_greatest(a, env);
    let b = schema_greatest(b, env);
    is_subtype(a, b, env)
}

/// **UP**(`t1`, `t2`).
pub fn up(t1: TypeId, t2: TypeId, env: &mut SubtypeEnv) -> TypeId {
    up_depth(t1, t2, env, 0)
}

fn up_depth(t1: TypeId, t2: TypeId, env: &mut SubtypeEnv, depth: u32) -> TypeId {
    if t1 == t2 {
        return t1;
    }
    if depth > 32 {
        return env.core.object_nullable;
    }
    // Esquemas: UP(T, _) = T.
    if env.core.is_unknown(env.table, t1) {
        return t2;
    }
    if env.core.is_unknown(env.table, t2) {
        return t1;
    }
    let (top1, top2) = (is_top(t1, env), is_top(t2, env));
    if top1 && top2 {
        return if more_top(t1, t2, env) { t1 } else { t2 };
    }
    if top1 {
        return t1;
    }
    if top2 {
        return t2;
    }
    let (bot1, bot2) = (is_bottom(t1, env), is_bottom(t2, env));
    if bot1 && bot2 {
        return if more_bottom(t1, t2, env) { t2 } else { t1 };
    }
    if bot1 {
        return t2;
    }
    if bot2 {
        return t1;
    }
    // X1 & B1: tratado antes da nulabilidade (changelog de 2023.10.27).
    if let Type::Intersection { param, bound } = env.table.get(t1).clone() {
        let x1 = env.table.intern(Type::TypeParameter { param, nullable: false });
        if sub_up(x1, t2, env) {
            return t2;
        }
        if sub_up(t2, x1, env) {
            return x1;
        }
        let b = greatest_closure(bound, &[param], env);
        return up_depth(b, t2, env, depth + 1);
    }
    if let Type::Intersection { param, bound } = env.table.get(t2).clone() {
        let x2 = env.table.intern(Type::TypeParameter { param, nullable: false });
        if sub_up(t1, x2, env) {
            return x2;
        }
        if sub_up(x2, t1, env) {
            return t1;
        }
        let b = greatest_closure(bound, &[param], env);
        return up_depth(t1, b, env, depth + 1);
    }
    let (null1, null2) = (is_null(t1, env), is_null(t2, env));
    if null1 && null2 {
        return if more_bottom(t1, t2, env) { t2 } else { t1 };
    }
    if null1 {
        return if is_nullable(t2, env) { t2 } else { nullable(t2, env.table) };
    }
    if null2 {
        return if is_nullable(t1, env) { t1 } else { nullable(t1, env.table) };
    }
    let (obj1, obj2) = (is_object(t1, env), is_object(t2, env));
    if obj1 && obj2 {
        return if more_top(t1, t2, env) { t1 } else { t2 };
    }
    if obj1 {
        return if is_non_nullable(t2, env) { t1 } else { nullable(t1, env.table) };
    }
    if obj2 {
        return if is_non_nullable(t1, env) { t2 } else { nullable(t2, env.table) };
    }
    let (ty1, ty2) = (env.table.get(t1).clone(), env.table.get(t2).clone());
    let (n1, n2) = (ty1.is_declared_nullable(), ty2.is_declared_nullable());
    if n1 || n2 {
        let (a, b) = (non_nullable(t1, env.table), non_nullable(t2, env.table));
        let s = up_depth(a, b, env, depth + 1);
        return nullable(s, env.table);
    }
    // Variáveis de tipo.
    if let Type::TypeParameter { param, .. } = ty1 {
        if sub_up(t1, t2, env) {
            return t2;
        }
        if sub_up(t2, t1, env) {
            return t1;
        }
        let b = env.table.param(param).bound;
        let b = greatest_closure(b, &[param], env);
        return up_depth(b, t2, env, depth + 1);
    }
    if let Type::TypeParameter { param, .. } = ty2 {
        if sub_up(t1, t2, env) {
            return t2;
        }
        if sub_up(t2, t1, env) {
            return t1;
        }
        let b = env.table.param(param).bound;
        let b = greatest_closure(b, &[param], env);
        return up_depth(t1, b, env, depth + 1);
    }
    // Funções.
    let function_class = env.core.function_class;
    if is_function(t1, env) && is_iface(t2, env, function_class) {
        return t2;
    }
    if is_iface(t1, env, function_class) && is_function(t2, env) {
        return t1;
    }
    if let (
        Type::Function { type_params: tp0, ret: r0, positional: p0, optional: o0, named: nm0, .. },
        Type::Function { type_params: tp1, ret: r1, positional: p1, optional: o1, named: nm1, .. },
    ) = (&ty1, &ty2)
    {
        return up_functions(
            (tp0, *r0, p0, o0, nm0),
            (tp1, *r1, p1, o1, nm1),
            env,
            depth,
        )
        .unwrap_or(env.core.function);
    }
    if is_function(t1, env) {
        let o = env.core.object;
        return up_depth(o, t2, env, depth + 1);
    }
    if is_function(t2, env) {
        let o = env.core.object;
        return up_depth(t1, o, env, depth + 1);
    }
    // Records.
    let record_class = env.core.record_class;
    if is_record(t1, env) && is_iface(t2, env, record_class) {
        return t2;
    }
    if is_iface(t1, env, record_class) && is_record(t2, env) {
        return t1;
    }
    if let (Type::Record { positional: p0, named: nm0, .. }, Type::Record { positional: p1, named: nm1, .. }) = (&ty1, &ty2) {
        let mesma_forma = p0.len() == p1.len()
            && nm0.len() == nm1.len()
            && nm0.iter().zip(nm1.iter()).all(|((a, _), (b, _))| a == b);
        if !mesma_forma {
            return env.core.record;
        }
        let pos: Vec<TypeId> = p0.iter().zip(p1.iter()).map(|(a, b)| up_depth(*a, *b, env, depth + 1)).collect();
        let named: Vec<_> = nm0.iter().zip(nm1.iter()).map(|((s, a), (_, b))| (*s, up_depth(*a, *b, env, depth + 1))).collect();
        return env.table.intern(Type::Record { positional: pos.into_boxed_slice(), named: named.into_boxed_slice(), nullable: false });
    }
    if is_record(t1, env) {
        let o = env.core.object;
        return up_depth(o, t2, env, depth + 1);
    }
    if is_record(t2, env) {
        let o = env.core.object;
        return up_depth(t1, o, env, depth + 1);
    }
    // FutureOr.
    let future = env.core.future_class;
    let future_arg = |t: &Type| -> Option<TypeId> {
        match t {
            Type::Interface { class, args, nullable: false } if Some(*class) == future && args.len() == 1 => Some(args[0]),
            _ => None,
        }
    };
    match (&ty1, &ty2) {
        (Type::FutureOr { arg: a, .. }, Type::FutureOr { arg: b, .. }) => {
            let t3 = up_depth(*a, *b, env, depth + 1);
            return env.table.intern(Type::FutureOr { arg: t3, nullable: false });
        }
        (_, Type::FutureOr { arg: b, .. }) if future_arg(&ty1).is_some() => {
            let a = future_arg(&ty1).unwrap();
            let t3 = up_depth(a, *b, env, depth + 1);
            return env.table.intern(Type::FutureOr { arg: t3, nullable: false });
        }
        (Type::FutureOr { arg: a, .. }, _) if future_arg(&ty2).is_some() => {
            let b = future_arg(&ty2).unwrap();
            let t3 = up_depth(*a, b, env, depth + 1);
            return env.table.intern(Type::FutureOr { arg: t3, nullable: false });
        }
        (_, Type::FutureOr { arg: b, .. }) => {
            let t3 = up_depth(t1, *b, env, depth + 1);
            return env.table.intern(Type::FutureOr { arg: t3, nullable: false });
        }
        (Type::FutureOr { arg: a, .. }, _) => {
            let t3 = up_depth(*a, t2, env, depth + 1);
            return env.table.intern(Type::FutureOr { arg: t3, nullable: false });
        }
        _ => {}
    }
    if sub_up(t1, t2, env) {
        return t2;
    }
    if sub_up(t2, t1, env) {
        return t1;
    }
    // Interfaces (e tipos de extensão pelas suas superinterfaces).
    if let (
        Type::Interface { class: c1, args: a1, .. } | Type::ExtensionType { decl: c1, args: a1, .. },
        Type::Interface { class: c2, args: a2, .. } | Type::ExtensionType { decl: c2, args: a2, .. },
    ) = (&ty1, &ty2)
    {
        // Mesma declaração (classe ou tipo de extensão, R-UP-w §4.2.4 passo
        // 3): argumento a argumento.
        let ext1 = matches!(ty1, Type::ExtensionType { .. });
        let ext2 = matches!(ty2, Type::ExtensionType { .. });
        if c1 == c2 && a1.len() == a2.len() && ext1 == ext2 {
            let args: Vec<TypeId> = a1.iter().zip(a2.iter()).map(|(x, y)| up_depth(*x, *y, env, depth + 1)).collect();
            let args = args.into_boxed_slice();
            return if ext1 {
                env.table.intern(Type::ExtensionType { decl: *c1, args, nullable: false })
            } else {
                env.table.intern(Type::Interface { class: *c1, args, nullable: false })
            };
        }
        return interface_lub(t1, t2, env);
    }
    env.core.object
}

/// Menor limite superior de duas interfaces "como no Dart 1": entre as
/// superinterfaces comuns (instanciadas igualmente), a de maior profundidade
/// que seja única nessa profundidade.
fn interface_lub(t1: TypeId, t2: TypeId, env: &mut SubtypeEnv) -> TypeId {
    // `_addSuperinterfaces` do analyzer: o próprio tipo, os supertipos
    // (classes e tipos de extensão), `Object` quando há classe na cadeia e
    // sempre `Object?` (fim de toda cadeia; tipo de extensão o acrescenta
    // direto).
    let supers = |t: TypeId, env: &mut SubtypeEnv| -> Vec<TypeId> {
        let (class, args) = match env.table.get(t) {
            Type::Interface { class, args, .. } | Type::ExtensionType { decl: class, args, .. } => (*class, args.clone()),
            _ => return vec![],
        };
        let mut out = vec![t];
        if let Some(data) = env.hierarchy.get(class) {
            let mapa: HashMap<TypeParamId, TypeId> = data.type_params.iter().copied().zip(args.iter().copied()).collect();
            let lista = data.all_supertypes.clone();
            for s in lista {
                let si = crate::ops::substitute(s, &mapa, env.table);
                // `Object` entra abaixo só se houver classe na cadeia (a
                // hierarquia o põe também sobre tipo de extensão).
                if si != env.core.object && matches!(env.table.get(si), Type::Interface { .. } | Type::ExtensionType { .. }) && !out.contains(&si) {
                    out.push(si);
                }
            }
        }
        let tem_classe = out.iter().any(|x| matches!(env.table.get(*x), Type::Interface { .. }));
        if tem_classe && !out.contains(&env.core.object) {
            out.push(env.core.object);
        }
        if !out.contains(&env.core.object_nullable) {
            out.push(env.core.object_nullable);
        }
        out
    };
    let s1 = supers(t1, env);
    let s2 = supers(t2, env);
    let comuns: Vec<TypeId> = s1.into_iter().filter(|x| s2.contains(x)).collect();
    let mut por_prof: HashMap<u32, Vec<TypeId>> = HashMap::new();
    for c in comuns {
        // Profundidade: `Object?` 0, `Object` 1, os demais a da hierarquia + 1
        // (tipo de extensão sem `implements` = 1, como no analyzer).
        let d = if c == env.core.object_nullable {
            0
        } else {
            match env.table.get(c) {
                Type::Interface { class, .. } if Some(*class) == env.core.object_class => 1,
                Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => {
                    env.hierarchy.get(*class).map(|d| d.depth).unwrap_or(0) + 1
                }
                _ => 0,
            }
        };
        let v = por_prof.entry(d).or_default();
        if !v.contains(&c) {
            v.push(c);
        }
    }
    let mut profs: Vec<u32> = por_prof.keys().copied().collect();
    profs.sort_unstable_by(|a, b| b.cmp(a));
    for d in profs {
        let v = &por_prof[&d];
        if v.len() == 1 {
            return v[0];
        }
    }
    env.core.object
}

type FnParts<'x> = (
    &'x [TypeParamId],
    TypeId,
    &'x [TypeId],
    &'x [TypeId],
    &'x [(dartforge_intern::SymbolId, TypeId, bool)],
);

/// Renomeia os parâmetros de tipo de `b` para os de `a` (mesmos limites).
fn alinhar_genericos(a: &[TypeParamId], b: &[TypeParamId], env: &mut SubtypeEnv) -> Option<HashMap<TypeParamId, TypeId>> {
    if a.len() != b.len() {
        return None;
    }
    let mut mapa = HashMap::new();
    for (&pa, &pb) in a.iter().zip(b.iter()) {
        let ta = env.table.intern(Type::TypeParameter { param: pa, nullable: false });
        mapa.insert(pb, ta);
    }
    for (&pa, &pb) in a.iter().zip(b.iter()) {
        let ba = env.table.param(pa).bound;
        let bb = env.table.param(pb).bound;
        let bb = crate::ops::substitute(bb, &mapa, env.table);
        if ba != bb {
            return None;
        }
    }
    Some(mapa)
}

fn up_functions(f0: FnParts<'_>, f1: FnParts<'_>, env: &mut SubtypeEnv, depth: u32) -> Option<TypeId> {
    let mapa = alinhar_genericos(f0.0, f1.0, env)?;
    let s = |t: TypeId, env: &mut SubtypeEnv| crate::ops::substitute(t, &mapa, env.table);
    let r1 = s(f1.1, env);
    let p1: Vec<TypeId> = f1.2.iter().map(|t| s(*t, env)).collect();
    let o1: Vec<TypeId> = f1.3.iter().map(|t| s(*t, env)).collect();
    let n1: Vec<_> = f1.4.iter().map(|(n, t, r)| (*n, s(*t, env), *r)).collect();
    let (p0, o0, n0) = (f0.2, f0.3, f0.4);
    if n0.is_empty() && n1.is_empty() {
        if p0.len() != p1.len() {
            return None;
        }
        let ret = up_depth(f0.1, r1, env, depth + 1);
        let pos: Vec<TypeId> = p0.iter().zip(p1.iter()).map(|(a, b)| down_depth(*a, *b, env, depth + 1)).collect();
        let q = o0.len().min(o1.len());
        let opt: Vec<TypeId> = (0..q).map(|i| down_depth(o0[i], o1[i], env, depth + 1)).collect();
        return Some(env.table.intern(Type::Function {
            type_params: f0.0.into(),
            ret,
            positional: pos.into_boxed_slice(),
            optional: opt.into_boxed_slice(),
            named: Box::new([]),
            nullable: false,
        }));
    }
    if !o0.is_empty() || !o1.is_empty() || p0.len() != p1.len() {
        return None;
    }
    for (nm, _, req) in n1.iter() {
        if *req && !n0.iter().any(|(m, _, _)| m == nm) {
            return None;
        }
    }
    for (nm, _, req) in n0.iter() {
        if *req && !n1.iter().any(|(m, _, _)| m == nm) {
            return None;
        }
    }
    let ret = up_depth(f0.1, r1, env, depth + 1);
    let pos: Vec<TypeId> = p0.iter().zip(p1.iter()).map(|(a, b)| down_depth(*a, *b, env, depth + 1)).collect();
    let mut named = Vec::new();
    for (nm, t0, r0) in n0.iter() {
        if let Some((_, t1, r1)) = n1.iter().find(|(m, _, _)| m == nm) {
            named.push((*nm, down_depth(*t0, *t1, env, depth + 1), *r0 || *r1));
        }
    }
    Some(env.table.intern(Type::Function {
        type_params: f0.0.into(),
        ret,
        positional: pos.into_boxed_slice(),
        optional: Box::new([]),
        named: named.into_boxed_slice(),
        nullable: false,
    }))
}

fn down_functions(f0: FnParts<'_>, f1: FnParts<'_>, env: &mut SubtypeEnv, depth: u32) -> Option<TypeId> {
    let mapa = alinhar_genericos(f0.0, f1.0, env)?;
    let s = |t: TypeId, env: &mut SubtypeEnv| crate::ops::substitute(t, &mapa, env.table);
    let r1 = s(f1.1, env);
    let p1: Vec<TypeId> = f1.2.iter().map(|t| s(*t, env)).collect();
    let o1: Vec<TypeId> = f1.3.iter().map(|t| s(*t, env)).collect();
    let n1: Vec<_> = f1.4.iter().map(|(n, t, r)| (*n, s(*t, env), *r)).collect();
    let (p0, o0, n0) = (f0.2, f0.3, f0.4);
    let ret = down_depth(f0.1, r1, env, depth + 1);
    if n0.is_empty() && n1.is_empty() {
        // Todos os posicionais (obrigatórios e opcionais) em sequência.
        let todos0: Vec<TypeId> = p0.iter().chain(o0.iter()).copied().collect();
        let todos1: Vec<TypeId> = p1.iter().chain(o1.iter()).copied().collect();
        let q = todos0.len().max(todos1.len());
        let req = p0.len().min(p1.len());
        let mut pos = Vec::new();
        let mut opt = Vec::new();
        for i in 0..q {
            let t = match (todos0.get(i), todos1.get(i)) {
                (Some(a), Some(b)) => up_depth(*a, *b, env, depth + 1),
                (Some(a), None) => *a,
                (None, Some(b)) => *b,
                _ => unreachable!(),
            };
            if i < req { pos.push(t) } else { opt.push(t) }
        }
        return Some(env.table.intern(Type::Function {
            type_params: f0.0.into(),
            ret,
            positional: pos.into_boxed_slice(),
            optional: opt.into_boxed_slice(),
            named: Box::new([]),
            nullable: false,
        }));
    }
    if !o0.is_empty() || !o1.is_empty() || p0.len() != p1.len() {
        return None;
    }
    let pos: Vec<TypeId> = p0.iter().zip(p1.iter()).map(|(a, b)| up_depth(*a, *b, env, depth + 1)).collect();
    let mut named = Vec::new();
    for (nm, t0, r0) in n0.iter() {
        match n1.iter().find(|(m, _, _)| m == nm) {
            Some((_, t1, r1)) => named.push((*nm, up_depth(*t0, *t1, env, depth + 1), *r0 && *r1)),
            None => named.push((*nm, *t0, false)),
        }
    }
    for (nm, t1, _) in n1.iter() {
        if !n0.iter().any(|(m, _, _)| m == nm) {
            named.push((*nm, *t1, false));
        }
    }
    Some(env.table.intern(Type::Function {
        type_params: f0.0.into(),
        ret,
        positional: pos.into_boxed_slice(),
        optional: Box::new([]),
        named: named.into_boxed_slice(),
        nullable: false,
    }))
}

/// **DOWN**(`t1`, `t2`).
pub fn down(t1: TypeId, t2: TypeId, env: &mut SubtypeEnv) -> TypeId {
    down_depth(t1, t2, env, 0)
}

fn down_depth(t1: TypeId, t2: TypeId, env: &mut SubtypeEnv, depth: u32) -> TypeId {
    if t1 == t2 {
        return t1;
    }
    if depth > 32 {
        return env.core.never;
    }
    if env.core.is_unknown(env.table, t1) {
        return t2;
    }
    if env.core.is_unknown(env.table, t2) {
        return t1;
    }
    let (top1, top2) = (is_top(t1, env), is_top(t2, env));
    if top1 && top2 {
        return if more_top(t2, t1, env) { t1 } else { t2 };
    }
    if top1 {
        return t2;
    }
    if top2 {
        return t1;
    }
    let (bot1, bot2) = (is_bottom(t1, env), is_bottom(t2, env));
    if bot1 && bot2 {
        return if more_bottom(t1, t2, env) { t1 } else { t2 };
    }
    if bot2 {
        return t2;
    }
    if bot1 {
        return t1;
    }
    let (null1, null2) = (is_null(t1, env), is_null(t2, env));
    if null1 && null2 {
        return if more_bottom(t1, t2, env) { t1 } else { t2 };
    }
    if null1 {
        return if is_nullable(t2, env) { t1 } else { env.core.never };
    }
    if null2 {
        return if is_nullable(t1, env) { t2 } else { env.core.never };
    }
    let (obj1, obj2) = (is_object(t1, env), is_object(t2, env));
    if obj1 && obj2 {
        return if more_top(t2, t1, env) { t1 } else { t2 };
    }
    if obj1 {
        if is_non_nullable(t2, env) {
            return t2;
        }
        let nn = non_nullable(t2, env.table);
        return if is_non_nullable(nn, env) { nn } else { env.core.never };
    }
    if obj2 {
        if is_non_nullable(t1, env) {
            return t1;
        }
        let nn = non_nullable(t1, env.table);
        return if is_non_nullable(nn, env) { nn } else { env.core.never };
    }
    let (ty1, ty2) = (env.table.get(t1).clone(), env.table.get(t2).clone());
    let (n1, n2) = (ty1.is_declared_nullable(), ty2.is_declared_nullable());
    if n1 && n2 {
        let (a, b) = (non_nullable(t1, env.table), non_nullable(t2, env.table));
        let s = down_depth(a, b, env, depth + 1);
        return nullable(s, env.table);
    }
    if n1 {
        let a = non_nullable(t1, env.table);
        return down_depth(a, t2, env, depth + 1);
    }
    if n2 {
        let b = non_nullable(t2, env.table);
        return down_depth(t1, b, env, depth + 1);
    }
    if let (
        Type::Function { type_params: tp0, ret: r0, positional: p0, optional: o0, named: nm0, .. },
        Type::Function { type_params: tp1, ret: r1, positional: p1, optional: o1, named: nm1, .. },
    ) = (&ty1, &ty2)
    {
        return down_functions((tp0, *r0, p0, o0, nm0), (tp1, *r1, p1, o1, nm1), env, depth)
            .unwrap_or(env.core.never);
    }
    if let (Type::Record { positional: p0, named: nm0, .. }, Type::Record { positional: p1, named: nm1, .. }) = (&ty1, &ty2) {
        let mesma_forma = p0.len() == p1.len()
            && nm0.len() == nm1.len()
            && nm0.iter().zip(nm1.iter()).all(|((a, _), (b, _))| a == b);
        if !mesma_forma {
            return env.core.never;
        }
        let pos: Vec<TypeId> = p0.iter().zip(p1.iter()).map(|(a, b)| down_depth(*a, *b, env, depth + 1)).collect();
        let named: Vec<_> = nm0.iter().zip(nm1.iter()).map(|((s, a), (_, b))| (*s, down_depth(*a, *b, env, depth + 1))).collect();
        return env.table.intern(Type::Record { positional: pos.into_boxed_slice(), named: named.into_boxed_slice(), nullable: false });
    }
    if sub_down(t1, t2, env) {
        return t1;
    }
    if sub_down(t2, t1, env) {
        return t2;
    }
    let future = env.core.future_class;
    let future_arg = |t: &Type| -> Option<TypeId> {
        match t {
            Type::Interface { class, args, nullable: false } if Some(*class) == future && args.len() == 1 => Some(args[0]),
            _ => None,
        }
    };
    match (&ty1, &ty2) {
        (Type::FutureOr { arg: a, .. }, Type::FutureOr { arg: b, .. }) => {
            let s = down_depth(*a, *b, env, depth + 1);
            return env.table.intern(Type::FutureOr { arg: s, nullable: false });
        }
        (Type::FutureOr { arg: a, .. }, _) if future_arg(&ty2).is_some() => {
            let b = future_arg(&ty2).unwrap();
            let s = down_depth(*a, b, env, depth + 1);
            return env.table.intern(Type::Interface { class: future.unwrap(), args: Box::new([s]), nullable: false });
        }
        (_, Type::FutureOr { arg: b, .. }) if future_arg(&ty1).is_some() => {
            let a = future_arg(&ty1).unwrap();
            let s = down_depth(a, *b, env, depth + 1);
            return env.table.intern(Type::Interface { class: future.unwrap(), args: Box::new([s]), nullable: false });
        }
        (Type::FutureOr { arg: a, .. }, _) => return down_depth(*a, t2, env, depth + 1),
        (_, Type::FutureOr { arg: b, .. }) => return down_depth(t1, *b, env, depth + 1),
        _ => {}
    }
    env.core.never
}
