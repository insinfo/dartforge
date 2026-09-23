//! Algoritmo de subtipagem normativa de Dart 3 (`is_subtype`).
//!
//! Implementa estritamente a ordem das regras de:
//! `references/dart-language/resources/type-system/subtyping.md`
//!
//! A convenção é que as regras são avaliadas de cima para baixo; a primeira regra
//! cujo padrão sintático casar define integralmente a resposta da consulta.

use crate::hierarchy::ClassHierarchy;
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use std::collections::HashSet;

/// Contexto de execução para consultas de subtipagem.
pub struct SubtypeEnv<'a> {
    pub table: &'a mut TypeTable,
    pub hierarchy: &'a ClassHierarchy,
    pub core: &'a CoreTypes,
    visiting: HashSet<(TypeId, TypeId)>,
}

impl<'a> SubtypeEnv<'a> {
    pub fn new(
        table: &'a mut TypeTable,
        hierarchy: &'a ClassHierarchy,
        core: &'a CoreTypes,
    ) -> Self {
        Self {
            table,
            hierarchy,
            core,
            visiting: HashSet::new(),
        }
    }
}

/// Verifica se `t0` é subtipo de `t1` (`t0 <: t1`) de acordo com as regras normativas.
pub fn is_subtype(t0: TypeId, t1: TypeId, env: &mut SubtypeEnv) -> bool {
    // 1. Reflexivity: if T0 and T1 are the same type then T0 <: T1
    if t0 == t1 {
        return true;
    }

    if !env.visiting.insert((t0, t1)) {
        // Evita recursão infinita em F-bounds cíclicos
        return true;
    }

    let result = is_subtype_inner(t0, t1, env);
    env.visiting.remove(&(t0, t1));
    result
}

fn is_subtype_inner(t0_id: TypeId, t1_id: TypeId, env: &mut SubtypeEnv) -> bool {
    let t0 = env.table.get(t0_id).clone();
    let t1 = env.table.get(t1_id).clone();

    // 2. Right Top: if T1 is a top type (dynamic, void, Object?) then T0 <: T1
    if matches!(t1, Type::Dynamic | Type::Void) || t1_id == env.core.object_nullable {
        return true;
    }
    if let Type::Interface {
        class,
        nullable: true,
        ..
    } = t1
        && let Some(obj_class) = env.core.object_class
        && class == obj_class
    {
        return true;
    }

    // 3. Left Top: if T0 is dynamic or void then T0 <: T1 if Object? <: T1
    if matches!(t0, Type::Dynamic | Type::Void) {
        return is_subtype(env.core.object_nullable, t1_id, env);
    }

    // 4. Left Bottom: if T0 is Never then T0 <: T1
    if matches!(t0, Type::Never) {
        return true;
    }

    // Variáveis promovidas `X & S` (subtyping.md, "Right/Left Promoted Variable").
    if let Type::Intersection { param, bound } = t1 {
        let x1 = env.table.intern(Type::TypeParameter { param, nullable: false });
        return is_subtype(t0_id, x1, env) && is_subtype(t0_id, bound, env);
    }
    if let Type::Intersection { param, bound } = t0 {
        let x0 = env.table.intern(Type::TypeParameter { param, nullable: false });
        return is_subtype(x0, t1_id, env) || is_subtype(bound, t1_id, env);
    }

    // 5. Right Object: if T1 is Object then:
    if is_non_nullable_object(t1_id, &t1, env) {
        return check_right_object(t0_id, &t0, t1_id, env);
    }

    // 6. Left Null: if T0 is Null then:
    if matches!(t0, Type::Null) {
        return check_left_null(&t1, env);
    }

    // 7. Left FutureOr: if T0 is FutureOr<S0> then Future<S0> <: T1 and S0 <: T1
    if let Type::FutureOr {
        arg: s0,
        nullable: false,
    } = t0
    {
        if let Some(future_class) = env.core.future_class {
            let future_s0 = env.table.intern(Type::Interface {
                class: future_class,
                args: Box::new([s0]),
                nullable: false,
            });
            return is_subtype(future_s0, t1_id, env) && is_subtype(s0, t1_id, env);
        } else {
            return is_subtype(s0, t1_id, env);
        }
    }

    // 8. Left Nullable: if T0 is S0? then S0 <: T1 and Null <: T1
    if t0.is_declared_nullable() {
        let s0 = strip_nullability(t0_id, &t0, env.table);
        return is_subtype(s0, t1_id, env) && is_subtype(env.core.null, t1_id, env);
    }

    // 9. Type Variable Reflexivity 1 & 2: if T0 is X0 and T1 is X0
    if let (Type::TypeParameter { param: p0, .. }, Type::TypeParameter { param: p1, .. }) =
        (&t0, &t1)
        && p0 == p1
    {
        return true;
    }

    // 10. Right FutureOr: if T1 is FutureOr<S1> then:
    if let Type::FutureOr {
        arg: s1,
        nullable: false,
    } = t1
    {
        // either T0 <: Future<S1>
        if let Some(future_class) = env.core.future_class {
            let future_s1 = env.table.intern(Type::Interface {
                class: future_class,
                args: Box::new([s1]),
                nullable: false,
            });
            if is_subtype(t0_id, future_s1, env) {
                return true;
            }
        }
        // or T0 <: S1
        if is_subtype(t0_id, s1, env) {
            return true;
        }
        // or T0 is X0 and X0 has bound S0 and S0 <: T1
        if let Type::TypeParameter { param, .. } = t0 {
            let bound = env.table.param(param).bound;
            if is_subtype(bound, t1_id, env) {
                return true;
            }
        }
        return false;
    }

    // 11. Right Nullable: if T1 is S1? then:
    if t1.is_declared_nullable() {
        let s1 = strip_nullability(t1_id, &t1, env.table);
        // either T0 <: S1
        if is_subtype(t0_id, s1, env) {
            return true;
        }
        // or T0 <: Null
        if is_subtype(t0_id, env.core.null, env) {
            return true;
        }
        // or T0 is X0 and X0 has bound S0 and S0 <: T1
        if let Type::TypeParameter { param, .. } = t0 {
            let bound = env.table.param(param).bound;
            if is_subtype(bound, t1_id, env) {
                return true;
            }
        }
        return false;
    }

    // 12. Left Type Variable Bound: T0 is X0 with bound B0 and B0 <: T1
    if let Type::TypeParameter { param, .. } = t0 {
        let bound = env.table.param(param).bound;
        return is_subtype(bound, t1_id, env);
    }

    // 13. Function Type / Function: T0 is a function type and T1 is Function
    if matches!(t0, Type::Function { .. })
        && is_interface_class(t1_id, &t1, env.core.function_class)
    {
        return true;
    }

    // 14. Record Type / Record: T0 is a record type and T1 is Record
    if matches!(t0, Type::Record { .. }) && is_interface_class(t1_id, &t1, env.core.record_class) {
        return true;
    }

    // 15. Interface Compositionality e Super-Interface
    if let (
        Type::Interface {
            class: c0,
            args: args0,
            nullable: false,
        },
        Type::Interface {
            class: c1,
            args: args1,
            nullable: false,
        },
    ) = (&t0, &t1)
    {
        if c0 == c1 {
            if args0.len() == args1.len() {
                for (&a0, &a1) in args0.iter().zip(args1.iter()) {
                    if !is_subtype(a0, a1, env) {
                        return false;
                    }
                }
                return true;
            }
        } else if let Some(super_t) = env.hierarchy.supertype_of(t0_id, *c1, env.table, env.core) {
            return is_subtype(super_t, t1_id, env);
        }
    }

    // 16. Extension Types (Dart 3.3+)
    if let Type::ExtensionType {
        decl: e0,
        args: args0,
        nullable: false,
    } = &t0
    {
        if let Type::ExtensionType {
            decl: e1,
            args: args1,
            nullable: false,
        } = &t1
            && e0 == e1
            && args0.len() == args1.len()
        {
            for (&a0, &a1) in args0.iter().zip(args1.iter()) {
                if !is_subtype(a0, a1, env) {
                    return false;
                }
            }
            return true;
        }
        // Subinterfaces declaradas no implements do extension type
        if let Type::Interface { class: c1, .. } = &t1
            && let Some(super_t) = env.hierarchy.supertype_of(t0_id, *c1, env.table, env.core)
        {
            return is_subtype(super_t, t1_id, env);
        }
    }

    // 17. Positional & Named Function Types
    if let (
        Type::Function {
            type_params: tp0,
            ret: ret0,
            positional: pos0,
            optional: opt0,
            named: named0,
            nullable: false,
        },
        Type::Function {
            type_params: tp1,
            ret: ret1,
            positional: pos1,
            optional: opt1,
            named: named1,
            nullable: false,
        },
    ) = (&t0, &t1)
    {
        // Se ambos têm parâmetros de tipo, o número deve coincidir
        if tp0.len() != tp1.len() {
            return false;
        }
        // Funções genéricas: renomeia os parâmetros de T1 para os de T0
        // (Z fresco da especificação, "Subtype Rules" 17) e exige limites
        // mutuamente subtipos.
        if !tp0.is_empty() && tp0 != tp1 {
            let mut m = std::collections::HashMap::new();
            for (&y, &x) in tp1.iter().zip(tp0.iter()) {
                let tx = env.table.intern(Type::TypeParameter { param: x, nullable: false });
                m.insert(y, tx);
            }
            for (&y, &x) in tp1.iter().zip(tp0.iter()) {
                let b1 = env.table.param(y).bound;
                let b1 = crate::ops::substitute(b1, &m, env.table);
                let b0 = env.table.param(x).bound;
                if !(is_subtype(b0, b1, env) && is_subtype(b1, b0, env)) {
                    return false;
                }
            }
            let (ret1, pos1, opt1, named1) = (*ret1, pos1.clone(), opt1.clone(), named1.clone());
            let sub = |t: TypeId, env: &mut SubtypeEnv| crate::ops::substitute(t, &m, env.table);
            let ret = sub(ret1, env);
            let positional: Box<[TypeId]> = pos1.iter().map(|&t| sub(t, env)).collect();
            let optional: Box<[TypeId]> = opt1.iter().map(|&t| sub(t, env)).collect();
            let named: Box<[_]> = named1.iter().map(|&(n, t, r)| (n, sub(t, env), r)).collect();
            let t1r = env.table.intern(Type::Function {
                type_params: tp0.clone(),
                ret,
                positional,
                optional,
                named,
                nullable: false,
            });
            return is_subtype(t0_id, t1r, env);
        }

        // Funções com parâmetros posicionais opcionais
        if !opt0.is_empty() || !opt1.is_empty() || (named0.is_empty() && named1.is_empty()) {
            if !named0.is_empty() || !named1.is_empty() {
                return false;
            }
            let n = pos0.len();
            let p = pos1.len();
            let m = n + opt0.len();
            let q = p + opt1.len();

            // p >= n e m >= q
            if !(p >= n && m >= q) {
                return false;
            }

            // Parâmetros são contravariantes: S_i <: V_i
            let get_param0 =
                |idx: usize| -> TypeId { if idx < n { pos0[idx] } else { opt0[idx - n] } };
            let get_param1 =
                |idx: usize| -> TypeId { if idx < p { pos1[idx] } else { opt1[idx - p] } };

            for i in 0..q {
                let v_i = get_param0(i);
                let s_i = get_param1(i);
                if !is_subtype(s_i, v_i, env) {
                    return false;
                }
            }

            // Retorno é covariante: U0 <: U1
            return is_subtype(*ret0, *ret1, env);
        }

        // Funções com parâmetros nomeados
        if pos0.len() != pos1.len() {
            return false;
        }
        for (&p0, &p1) in pos0.iter().zip(pos1.iter()) {
            if !is_subtype(p1, p0, env) {
                return false;
            }
        }

        // Todos os nomeados de T1 devem estar presentes em T0 (T1 subset of T0)
        for &(sym1, ty1, req1) in named1.iter() {
            if let Some(&(_, ty0, req0)) = named0.iter().find(|(s, _, _)| *s == sym1) {
                // Parâmetros nomeados contravariantes
                if !is_subtype(ty1, ty0, env) {
                    return false;
                }
                // Se era required em T0, deve ser required em T1
                if req0 && !req1 {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Se algum parâmetro em T0 é required, ele deve existir em T1 como required
        for &(sym0, _, req0) in named0.iter() {
            if req0 {
                let found_req1 = named1.iter().any(|(s, _, r)| *s == sym0 && *r);
                if !found_req1 {
                    return false;
                }
            }
        }

        // Retorno covariante
        return is_subtype(*ret0, *ret1, env);
    }

    // 18. Record Types
    if let (
        Type::Record {
            positional: pos0,
            named: named0,
            nullable: false,
        },
        Type::Record {
            positional: pos1,
            named: named1,
            nullable: false,
        },
    ) = (&t0, &t1)
    {
        if pos0.len() != pos1.len() || named0.len() != named1.len() {
            return false;
        }

        // Covariância campo a campo
        for (&p0, &p1) in pos0.iter().zip(pos1.iter()) {
            if !is_subtype(p0, p1, env) {
                return false;
            }
        }

        for (&(sym0, ty0), &(sym1, ty1)) in named0.iter().zip(named1.iter()) {
            if sym0 != sym1 || !is_subtype(ty0, ty1, env) {
                return false;
            }
        }

        return true;
    }

    false
}

fn is_non_nullable_object(ty_id: TypeId, ty: &Type, env: &SubtypeEnv) -> bool {
    if ty_id == env.core.object {
        return true;
    }
    if let Type::Interface {
        class,
        nullable: false,
        ..
    } = ty
        && let Some(obj_class) = env.core.object_class
    {
        return *class == obj_class;
    }
    false
}

fn is_interface_class(
    _ty_id: TypeId,
    ty: &Type,
    target_class: Option<dartforge_elements::model::ClassId>,
) -> bool {
    let target = match target_class {
        Some(t) => t,
        None => return false,
    };
    if let Type::Interface {
        class,
        nullable: false,
        ..
    } = ty
    {
        *class == target
    } else {
        false
    }
}

fn check_right_object(_t0_id: TypeId, t0: &Type, t1_id: TypeId, env: &mut SubtypeEnv) -> bool {
    // - if T0 is an unpromoted type variable with bound B then T0 <: T1 iff B <: Object
    if let Type::TypeParameter {
        param,
        nullable: false,
    } = t0
    {
        let bound = env.table.param(*param).bound;
        return is_subtype(bound, t1_id, env);
    }
    // - if T0 is FutureOr<S> then S <: Object
    if let Type::FutureOr {
        arg: s,
        nullable: false,
    } = t0
    {
        return is_subtype(*s, t1_id, env);
    }
    // - if T0 is Null, dynamic, void, or S? for any S, then the subtyping does not hold
    if matches!(t0, Type::Null | Type::Dynamic | Type::Void) || t0.is_declared_nullable() {
        return false;
    }
    // - Otherwise T0 <: T1 is true
    true
}

fn check_left_null(t1: &Type, env: &mut SubtypeEnv) -> bool {
    // - if T1 is a type variable (promoted or not) the query is false
    if matches!(t1, Type::TypeParameter { nullable: false, .. } | Type::Intersection { .. }) {
        return false;
    }
    // - if T1 is FutureOr<S> for some S, then the query is true iff Null <: S
    if let Type::FutureOr { arg: s, .. } = t1 {
        return is_subtype(env.core.null, *s, env);
    }
    // - if T1 is Null, S? for some S, then the query is true
    if matches!(t1, Type::Null) || t1.is_declared_nullable() {
        return true;
    }
    // - Otherwise, the query is false
    false
}

fn strip_nullability(ty_id: TypeId, ty: &Type, table: &mut TypeTable) -> TypeId {
    match ty {
        Type::Interface { class, args, .. } => table.intern(Type::Interface {
            class: *class,
            args: args.clone(),
            nullable: false,
        }),
        Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            ..
        } => table.intern(Type::Function {
            type_params: type_params.clone(),
            ret: *ret,
            positional: positional.clone(),
            optional: optional.clone(),
            named: named.clone(),
            nullable: false,
        }),
        Type::Record {
            positional, named, ..
        } => table.intern(Type::Record {
            positional: positional.clone(),
            named: named.clone(),
            nullable: false,
        }),
        Type::TypeParameter { param, .. } => table.intern(Type::TypeParameter {
            param: *param,
            nullable: false,
        }),
        Type::FutureOr { arg, .. } => table.intern(Type::FutureOr {
            arg: *arg,
            nullable: false,
        }),
        Type::ExtensionType { decl, args, .. } => table.intern(Type::ExtensionType {
            decl: *decl,
            args: args.clone(),
            nullable: false,
        }),
        _ => ty_id,
    }
}
