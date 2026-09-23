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
        Type::TypeParameter {
            param,
            nullable: is_null,
        } => {
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
        Type::Interface {
            class,
            args,
            nullable: is_null,
        } => {
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
        Type::ExtensionType {
            decl,
            args,
            nullable: is_null,
        } => {
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
        Type::FutureOr {
            arg,
            nullable: is_null,
        } => {
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
        Type::FutureOr {
            arg,
            nullable: is_null,
        } => {
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
                return if is_null {
                    core.object_nullable
                } else {
                    core.object
                };
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
        Type::Interface {
            class,
            args,
            nullable: is_null,
        } => {
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
        Type::ExtensionType {
            decl,
            args,
            nullable: is_null,
        } => {
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
        Type::ExtensionType {
            decl,
            args,
            nullable: is_null,
        } => {
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
        Type::Interface {
            class,
            args,
            nullable: is_null,
        } => {
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
        Type::FutureOr {
            arg,
            nullable: is_null,
        } => {
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

/// Menor supertipo comum: **UP**(`a`, `b`) de `upper-lower-bounds.md`.
pub use crate::bounds::up as lub;
/// Maior subtipo comum: **DOWN**(`a`, `b`) de `upper-lower-bounds.md`.
pub use crate::bounds::down as glb;
