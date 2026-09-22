//! Operações centrais do sistema de tipos.
//!
//! Implementa substituição (`substitute`), nulabilidade (`nullable`/`non_nullable`),
//! apagamento de tipos de extensão (`erase_extension_type`), normalização
//! (`normalize`) e limites superior/inferior (`lub`/`glb`).

use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use dartforge_elements::model::ClassId;
use std::collections::HashMap;

/// Torna um tipo anulável (`T?`).
///
/// Tipos como `dynamic`, `void` e `Null` já são anuláveis por natureza e não mudam.
/// `Never?` é canonicamente simplificado para `Null`.
pub fn nullable(ty: TypeId, table: &mut TypeTable) -> TypeId {
    let t = table.get(ty).clone();
    match t {
        Type::Dynamic | Type::Void | Type::Null => ty,
        Type::Never => {
            // Never? === Null
            table.intern(Type::Null)
        }
        Type::Interface {
            class,
            args,
            nullable: false,
        } => table.intern(Type::Interface {
            class,
            args,
            nullable: true,
        }),
        Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            nullable: false,
        } => table.intern(Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            nullable: true,
        }),
        Type::Record {
            positional,
            named,
            nullable: false,
        } => table.intern(Type::Record {
            positional,
            named,
            nullable: true,
        }),
        Type::TypeParameter {
            param,
            nullable: false,
        } => table.intern(Type::TypeParameter {
            param,
            nullable: true,
        }),
        Type::FutureOr {
            arg,
            nullable: false,
        } => table.intern(Type::FutureOr {
            arg,
            nullable: true,
        }),
        Type::ExtensionType {
            decl,
            args,
            nullable: false,
        } => table.intern(Type::ExtensionType {
            decl,
            args,
            nullable: true,
        }),
        _ => ty,
    }
}

/// Remove a nulabilidade do tipo (`T`), se aplicável.
///
/// `Null` torna-se `Never`. `dynamic` e `void` permanecem inalterados.
pub fn non_nullable(ty: TypeId, table: &mut TypeTable) -> TypeId {
    let t = table.get(ty).clone();
    match t {
        Type::Null => table.intern(Type::Never),
        Type::Dynamic | Type::Void | Type::Never => ty,
        Type::Interface {
            class,
            args,
            nullable: true,
        } => table.intern(Type::Interface {
            class,
            args,
            nullable: false,
        }),
        Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            nullable: true,
        } => table.intern(Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            nullable: false,
        }),
        Type::Record {
            positional,
            named,
            nullable: true,
        } => table.intern(Type::Record {
            positional,
            named,
            nullable: false,
        }),
        Type::TypeParameter {
            param,
            nullable: true,
        } => table.intern(Type::TypeParameter {
            param,
            nullable: false,
        }),
        Type::FutureOr {
            arg,
            nullable: true,
        } => table.intern(Type::FutureOr {
            arg,
            nullable: false,
        }),
        Type::ExtensionType {
            decl,
            args,
            nullable: true,
        } => table.intern(Type::ExtensionType {
            decl,
            args,
            nullable: false,
        }),
        _ => ty,
    }
}

/// Realiza substituição simultânea de parâmetros de tipo por argumentos.
///
/// `T[A0/X0, ..., An/Xn]`
pub fn substitute(
    ty: TypeId,
    mapping: &HashMap<TypeParamId, TypeId>,
    table: &mut TypeTable,
) -> TypeId {
    if mapping.is_empty() {
        return ty;
    }

    let t = table.get(ty).clone();
    match t {
        Type::Dynamic | Type::Void | Type::Never | Type::Null => ty,
        Type::TypeParameter { param, nullable: is_null } => {
            if let Some(&replacement) = mapping.get(&param) {
                if is_null {
                    nullable(replacement, table)
                } else {
                    replacement
                }
            } else {
                ty
            }
        }
        Type::Interface { class, args, nullable: is_null } => {
            let mut changed = false;
            let mut new_args = Vec::with_capacity(args.len());
            for &arg in args.iter() {
                let subst_arg = substitute(arg, mapping, table);
                if subst_arg != arg {
                    changed = true;
                }
                new_args.push(subst_arg);
            }
            if changed {
                table.intern(Type::Interface {
                    class,
                    args: new_args.into_boxed_slice(),
                    nullable: is_null,
                })
            } else {
                ty
            }
        }
        Type::ExtensionType { decl, args, nullable: is_null } => {
            let mut changed = false;
            let mut new_args = Vec::with_capacity(args.len());
            for &arg in args.iter() {
                let subst_arg = substitute(arg, mapping, table);
                if subst_arg != arg {
                    changed = true;
                }
                new_args.push(subst_arg);
            }
            if changed {
                table.intern(Type::ExtensionType {
                    decl,
                    args: new_args.into_boxed_slice(),
                    nullable: is_null,
                })
            } else {
                ty
            }
        }
        Type::FutureOr { arg, nullable: is_null } => {
            let new_arg = substitute(arg, mapping, table);
            if new_arg != arg {
                table.intern(Type::FutureOr {
                    arg: new_arg,
                    nullable: is_null,
                })
            } else {
                ty
            }
        }
        Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            nullable: is_null,
        } => {
            // Parâmetros de tipo genéricos locais sombreiam o mapeamento
            let active_mapping: HashMap<TypeParamId, TypeId> = if type_params.is_empty() {
                mapping.clone()
            } else {
                let mut m = mapping.clone();
                for &p in type_params.iter() {
                    m.remove(&p);
                }
                m
            };

            let new_ret = substitute(ret, &active_mapping, table);
            let mut pos_changed = false;
            let mut new_pos = Vec::with_capacity(positional.len());
            for &p in positional.iter() {
                let s = substitute(p, &active_mapping, table);
                if s != p {
                    pos_changed = true;
                }
                new_pos.push(s);
            }

            let mut opt_changed = false;
            let mut new_opt = Vec::with_capacity(optional.len());
            for &p in optional.iter() {
                let s = substitute(p, &active_mapping, table);
                if s != p {
                    opt_changed = true;
                }
                new_opt.push(s);
            }

            let mut named_changed = false;
            let mut new_named = Vec::with_capacity(named.len());
            for &(sym, t, req) in named.iter() {
                let s = substitute(t, &active_mapping, table);
                if s != t {
                    named_changed = true;
                }
                new_named.push((sym, s, req));
            }

            if new_ret != ret || pos_changed || opt_changed || named_changed {
                table.intern(Type::Function {
                    type_params,
                    ret: new_ret,
                    positional: new_pos.into_boxed_slice(),
                    optional: new_opt.into_boxed_slice(),
                    named: new_named.into_boxed_slice(),
                    nullable: is_null,
                })
            } else {
                ty
            }
        }
        Type::Record {
            positional,
            named,
            nullable: is_null,
        } => {
            let mut pos_changed = false;
            let mut new_pos = Vec::with_capacity(positional.len());
            for &p in positional.iter() {
                let s = substitute(p, mapping, table);
                if s != p {
                    pos_changed = true;
                }
                new_pos.push(s);
            }

            let mut named_changed = false;
            let mut new_named = Vec::with_capacity(named.len());
            for &(sym, t) in named.iter() {
                let s = substitute(t, mapping, table);
                if s != t {
                    named_changed = true;
                }
                new_named.push((sym, s));
            }

            if pos_changed || named_changed {
                table.intern(Type::Record {
                    positional: new_pos.into_boxed_slice(),
                    named: new_named.into_boxed_slice(),
                    nullable: is_null,
                })
            } else {
                ty
            }
        }
    }
}

/// Normaliza tipos semânticos conforme regras da especificação do Dart.
///
/// Ex.:
/// - `FutureOr<Never>` → `Future<Never>`
/// - `FutureOr<Object>` → `Object`
/// - `FutureOr<dynamic>` → `dynamic`
/// - `FutureOr<void>` → `void`
/// - `Null?` → `Null`
/// - `dynamic?` → `dynamic`
/// - `void?` → `void`
pub fn normalize(ty: TypeId, table: &mut TypeTable, core: &CoreTypes) -> TypeId {
    let t = table.get(ty).clone();
    match t {
        Type::Dynamic | Type::Void | Type::Null => ty,
        Type::Never => ty,
        Type::FutureOr { arg, nullable: is_null } => {
            let norm_arg = normalize(arg, table, core);
            if norm_arg == core.never {
                if let Some(future_class) = core.future_class {
                    let fut = table.intern(Type::Interface {
                        class: future_class,
                        args: Box::new([core.never]),
                        nullable: is_null,
                    });
                    return fut;
                }
            } else if norm_arg == core.object {
                return if is_null { core.object_nullable } else { core.object };
            } else if norm_arg == core.dynamic_ {
                return core.dynamic_;
            } else if norm_arg == core.void_ {
                return core.void_;
            } else if norm_arg == core.object_nullable {
                return core.object_nullable;
            }

            if norm_arg != arg {
                table.intern(Type::FutureOr {
                    arg: norm_arg,
                    nullable: is_null,
                })
            } else {
                ty
            }
        }
        Type::Interface { class, args, nullable: is_null } => {
            let mut changed = false;
            let mut new_args = Vec::with_capacity(args.len());
            for &a in args.iter() {
                let n = normalize(a, table, core);
                if n != a {
                    changed = true;
                }
                new_args.push(n);
            }
            if changed {
                table.intern(Type::Interface {
                    class,
                    args: new_args.into_boxed_slice(),
                    nullable: is_null,
                })
            } else {
                ty
            }
        }
        Type::ExtensionType { decl, args, nullable: is_null } => {
            let mut changed = false;
            let mut new_args = Vec::with_capacity(args.len());
            for &a in args.iter() {
                let n = normalize(a, table, core);
                if n != a {
                    changed = true;
                }
                new_args.push(n);
            }
            if changed {
                table.intern(Type::ExtensionType {
                    decl,
                    args: new_args.into_boxed_slice(),
                    nullable: is_null,
                })
            } else {
                ty
            }
        }
        _ => ty,
    }
}

/// Função de consulta para o tipo de representação instanciado de um extension type.
pub type ExtensionTypeEraser<'a> = &'a dyn Fn(ClassId, &[TypeId], &mut TypeTable) -> Option<TypeId>;

/// Apaga recursivamente tipos de extensão (*extension type erasure*),
/// substituindo-os pelo seu tipo de representação instanciado.
pub fn erase_extension_type(
    ty: TypeId,
    table: &mut TypeTable,
    rep_fn: ExtensionTypeEraser<'_>,
) -> TypeId {
    let t = table.get(ty).clone();
    match t {
        Type::ExtensionType { decl, args, nullable: is_null } => {
            // Apaga os argumentos primeiro
            let mut erased_args = Vec::with_capacity(args.len());
            for &a in args.iter() {
                erased_args.push(erase_extension_type(a, table, rep_fn));
            }
            if let Some(rep) = rep_fn(decl, &erased_args, table) {
                let fully_erased = erase_extension_type(rep, table, rep_fn);
                if is_null {
                    nullable(fully_erased, table)
                } else {
                    fully_erased
                }
            } else {
                ty
            }
        }
        Type::Interface { class, args, nullable: is_null } => {
            let mut changed = false;
            let mut new_args = Vec::with_capacity(args.len());
            for &a in args.iter() {
                let e = erase_extension_type(a, table, rep_fn);
                if e != a {
                    changed = true;
                }
                new_args.push(e);
            }
            if changed {
                table.intern(Type::Interface {
                    class,
                    args: new_args.into_boxed_slice(),
                    nullable: is_null,
                })
            } else {
                ty
            }
        }
        Type::FutureOr { arg, nullable: is_null } => {
            let e = erase_extension_type(arg, table, rep_fn);
            if e != arg {
                table.intern(Type::FutureOr {
                    arg: e,
                    nullable: is_null,
                })
            } else {
                ty
            }
        }
        Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            nullable: is_null,
        } => {
            let new_ret = erase_extension_type(ret, table, rep_fn);
            let mut pos_changed = false;
            let mut new_pos = Vec::with_capacity(positional.len());
            for &p in positional.iter() {
                let e = erase_extension_type(p, table, rep_fn);
                if e != p {
                    pos_changed = true;
                }
                new_pos.push(e);
            }

            let mut opt_changed = false;
            let mut new_opt = Vec::with_capacity(optional.len());
            for &p in optional.iter() {
                let e = erase_extension_type(p, table, rep_fn);
                if e != p {
                    opt_changed = true;
                }
                new_opt.push(e);
            }

            let mut named_changed = false;
            let mut new_named = Vec::with_capacity(named.len());
            for &(sym, t, req) in named.iter() {
                let e = erase_extension_type(t, table, rep_fn);
                if e != t {
                    named_changed = true;
                }
                new_named.push((sym, e, req));
            }

            if new_ret != ret || pos_changed || opt_changed || named_changed {
                table.intern(Type::Function {
                    type_params,
                    ret: new_ret,
                    positional: new_pos.into_boxed_slice(),
                    optional: new_opt.into_boxed_slice(),
                    named: new_named.into_boxed_slice(),
                    nullable: is_null,
                })
            } else {
                ty
            }
        }
        Type::Record {
            positional,
            named,
            nullable: is_null,
        } => {
            let mut pos_changed = false;
            let mut new_pos = Vec::with_capacity(positional.len());
            for &p in positional.iter() {
                let e = erase_extension_type(p, table, rep_fn);
                if e != p {
                    pos_changed = true;
                }
                new_pos.push(e);
            }

            let mut named_changed = false;
            let mut new_named = Vec::with_capacity(named.len());
            for &(sym, t) in named.iter() {
                let e = erase_extension_type(t, table, rep_fn);
                if e != t {
                    named_changed = true;
                }
                new_named.push((sym, e));
            }

            if pos_changed || named_changed {
                table.intern(Type::Record {
                    positional: new_pos.into_boxed_slice(),
                    named: new_named.into_boxed_slice(),
                    nullable: is_null,
                })
            } else {
                ty
            }
        }
        _ => ty,
    }
}

/// Computa o menor supertipo comum (*Least Upper Bound* / `UP`) de dois tipos.
pub fn lub(a: TypeId, b: TypeId, env: &mut crate::subtyping::SubtypeEnv) -> TypeId {
    if a == b {
        return a;
    }

    if crate::subtyping::is_subtype(a, b, env) {
        return b;
    }
    if crate::subtyping::is_subtype(b, a, env) {
        return a;
    }

    if a == env.core.never {
        return b;
    }
    if b == env.core.never {
        return a;
    }

    if a == env.core.null {
        return nullable(b, env.table);
    }
    if b == env.core.null {
        return nullable(a, env.table);
    }

    if a == env.core.dynamic_ || b == env.core.dynamic_ {
        return env.core.dynamic_;
    }
    if a == env.core.void_ || b == env.core.void_ {
        return env.core.void_;
    }

    let a_null = env.table.get(a).is_declared_nullable();
    let b_null = env.table.get(b).is_declared_nullable();

    if a_null || b_null {
        let non_a = non_nullable(a, env.table);
        let non_b = non_nullable(b, env.table);
        let u = lub(non_a, non_b, env);
        return nullable(u, env.table);
    }

    // Se ambos são tipos de interface não nulos
    let t_a = env.table.get(a).clone();
    let t_b = env.table.get(b).clone();

    if let (Type::Interface { class: c_a, .. }, Type::Interface { class: c_b, .. }) = (t_a, t_b)
        && let (Some(data_a), Some(_)) = (env.hierarchy.get(c_a), env.hierarchy.get(c_b))
    {
        let mut candidates = Vec::new();
            for &candidate_ty in data_a.all_supertypes.iter() {
                if crate::subtyping::is_subtype(b, candidate_ty, env) {
                    let depth = if let Type::Interface { class: c_cand, .. } = env.table.get(candidate_ty) {
                        env.hierarchy.get(*c_cand).map(|d| d.depth).unwrap_or(0)
                    } else {
                        0
                    };
                    candidates.push((candidate_ty, depth));
                }
            }

            if !candidates.is_empty() {
                candidates.sort_by_key(|&(_, depth)| std::cmp::Reverse(depth));
                return candidates[0].0;
            }
    }

    env.core.object
}

/// Computa o maior subtipo comum (*Greatest Lower Bound* / `DOWN`) de dois tipos.
pub fn glb(a: TypeId, b: TypeId, env: &mut crate::subtyping::SubtypeEnv) -> TypeId {
    if a == b {
        return a;
    }

    if crate::subtyping::is_subtype(a, b, env) {
        return a;
    }
    if crate::subtyping::is_subtype(b, a, env) {
        return b;
    }

    if a == env.core.never || b == env.core.never {
        return env.core.never;
    }

    if a == env.core.dynamic_ {
        return b;
    }
    if b == env.core.dynamic_ {
        return a;
    }
    if a == env.core.void_ {
        return b;
    }
    if b == env.core.void_ {
        return a;
    }

    env.core.never
}

