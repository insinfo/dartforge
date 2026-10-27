//! Core operations of the type system.
//!
//! Implements substitution (`substitute`), nullability (`nullable`/`non_nullable`),
//! extension type erasure (`erase_extension_type`), normalization
//! (`normalize`) and upper/lower bounds (`lub`/`glb`).

use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use dartforge_elements::model::{ClassId, TypedefId};
use std::collections::HashMap;

/// Makes a type nullable (`T?`).
///
/// Types such as `dynamic`, `void` and `Null` are already nullable by nature and
/// do not change. `Never?` is canonically simplified to `Null`.
pub fn nullable(ty: TypeId, table: &mut TypeTable) -> TypeId {
    // Display decorations (C9): the nullable alias stays the alias.
    if let Some(ex) = table.decoration(ty).cloned() {
        let c = table.canonical(ty);
        let n = nullable(c, table);
        return match ex {
            crate::table::DisplayDecoration::NeverNullable
            | crate::table::DisplayDecoration::Invalid => ty,
            ex if n != c => table.decorate(n, ex),
            _ => ty,
        };
    }
    let t = table.get(ty).clone();
    match t {
        Type::Dynamic | Type::Void | Type::Null => ty,
        Type::Intersection { param, .. } => table.intern(Type::TypeParameter {
            param,
            nullable: true,
        }),
        Type::Never => {
            // Never? === Null (displayed as `Never?` when the table preserves the display).
            let n = table.intern(Type::Null);
            table.decorate(n, crate::table::DisplayDecoration::NeverNullable)
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

/// Removes the nullability of the type (`T`), if applicable.
///
/// `Null` becomes `Never`. `dynamic` and `void` remain unchanged.
pub fn non_nullable(ty: TypeId, table: &mut TypeTable) -> TypeId {
    if let Some(ex) = table.decoration(ty).cloned() {
        let c = table.canonical(ty);
        let n = non_nullable(c, table);
        return match ex {
            crate::table::DisplayDecoration::Alias { .. } if n != c => table.decorate(n, ex),
            crate::table::DisplayDecoration::Alias { .. } => ty,
            crate::table::DisplayDecoration::NeverNullable => n,
            crate::table::DisplayDecoration::Invalid => ty,
        };
    }
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

/// Whether `t` mentions any of the parameters `params`.
pub fn mentions_parameters(t: TypeId, params: &[TypeParamId], table: &TypeTable) -> bool {
    match table.get(t) {
        Type::TypeParameter { param, .. } => params.contains(param),
        Type::Intersection { param, bound } => {
            params.contains(param) || mentions_parameters(*bound, params, table)
        }
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => {
            args.iter().any(|a| mentions_parameters(*a, params, table))
        }
        Type::FutureOr { arg, .. } => mentions_parameters(*arg, params, table),
        Type::Function {
            ret,
            positional,
            optional,
            named,
            type_params,
            ..
        } => {
            type_params
                .iter()
                .any(|p| mentions_parameters(table.param(*p).bound, params, table))
                || mentions_parameters(*ret, params, table)
                || positional
                    .iter()
                    .chain(optional.iter())
                    .any(|a| mentions_parameters(*a, params, table))
                || named
                    .iter()
                    .any(|(_, a, _)| mentions_parameters(*a, params, table))
        }
        Type::Record {
            positional, named, ..
        } => {
            positional
                .iter()
                .any(|a| mentions_parameters(*a, params, table))
                || named
                    .iter()
                    .any(|(_, a)| mentions_parameters(*a, params, table))
        }
        _ => false,
    }
}

/// Instantiation to bounds (*instantiate to bounds*) of `params`, with the
/// already known ones in `fixed`: a parameter without a written bound goes to
/// `dynamic`; with a bound, to the bound with the already instantiated
/// parameters substituted, in dependency order (`A<T extends U, U extends num>`
/// is `A<num, num>`). Those left in a bound cycle (F-bounds,
/// `T extends Comparable<T>`) have the cycle's parameters replaced by
/// `dynamic`.
pub fn instantiate_to_bounds(
    params: &[TypeParamId],
    fixed: &[Option<TypeId>],
    table: &mut TypeTable,
    core: &CoreTypes,
) -> Vec<TypeId> {
    let n = params.len();
    let mut args: Vec<Option<TypeId>> = (0..n).map(|i| fixed.get(i).copied().flatten()).collect();
    for i in 0..n {
        let d = table.param(params[i]);
        if args[i].is_none() && (!d.explicit || matches!(table.get(d.bound), Type::Dynamic)) {
            args[i] = Some(core.dynamic_);
        }
    }
    loop {
        let pending: Vec<TypeParamId> = (0..n)
            .filter(|&i| args[i].is_none())
            .map(|i| params[i])
            .collect();
        if pending.is_empty() {
            break;
        }
        let map: HashMap<TypeParamId, TypeId> = (0..n)
            .filter_map(|i| args[i].map(|a| (params[i], a)))
            .collect();
        let mut progress = false;
        for i in 0..n {
            if args[i].is_some() {
                continue;
            }
            let b = table.param(params[i]).bound;
            if !mentions_parameters(b, &pending, table) {
                args[i] = Some(substitute(b, &map, table));
                progress = true;
            }
        }
        if !progress {
            let map: HashMap<TypeParamId, TypeId> = (0..n)
                .map(|i| (params[i], args[i].unwrap_or(core.dynamic_)))
                .collect();
            for i in 0..n {
                if args[i].is_none() {
                    let b = table.param(params[i]).bound;
                    args[i] = Some(substitute(b, &map, table));
                }
            }
            break;
        }
    }
    args.into_iter()
        .map(|a| a.unwrap_or(core.dynamic_))
        .collect()
}

/// Whether an occurrence is replaced by the lower type (`Never`) or by the
/// upper one in [`default_types`]: the `_variance` of the analyzer's
/// `_UpperLowerReplacementVisitor`, where only covariant and contravariant
/// flip and every other variance (`unrelated`, `invariant`) takes the upper.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Position {
    Covariant,
    Contravariant,
    Fixed,
}

impl Position {
    fn flip(self) -> Position {
        match self {
            Position::Covariant => Position::Contravariant,
            Position::Contravariant => Position::Covariant,
            Position::Fixed => Position::Fixed,
        }
    }

    /// The position of an alias's parameter (`VarianceBuilder`'s variance of
    /// the parameter in the alias's target) inside this one (`Variance.combine`).
    fn inside(self, table: &TypeTable, target: TypeId, p: TypeParamId) -> Position {
        use crate::bound_checks::AliasVariance;
        match crate::bound_checks::variance_of_alias_parameter(table, target, p) {
            AliasVariance::Covariant => self,
            AliasVariance::Contravariant => self.flip(),
            AliasVariance::Other => Position::Fixed,
        }
    }
}

/// The parameters and the target of an alias, for [`default_types`].
pub type AliasLookup<'a> = &'a dyn Fn(TypedefId) -> Option<(Vec<TypeParamId>, TypeId)>;

/// The default types of `params` (`TypeParameterElementImpl.defaultType`),
/// which raw class and alias types take: `DefaultTypesBuilder._computeBounds`
/// of the 3.6.2 analyzer (`src/summary2/default_types_builder.dart:195-247`).
/// A parameter without a written bound has `dynamic`. In each strongly
/// connected component of the bound dependencies, the component's parameters
/// are replaced in each bound by `dynamic`, or by `Never` in a contravariant
/// position; then each parameter, in order, is replaced in every bound by its
/// own (or by `Never` in a contravariant position): `C<T extends void
/// Function(U), U>` is `C<void Function(Never), dynamic>`. The starting
/// position of a bound is the parameter's variance: covariant for a class,
/// that of the parameter in the target `alias_target` for an alias
/// (`VarianceBuilder`), so `typedef F<X extends A<X>> = Function(X)` is
/// `F<A<Never>>`. A bound written with an alias (`X extends Finv<Y>`) is
/// walked through the alias's arguments with its parameters' variances, as
/// the analyzer walks the unexpanded `NamedTypeBuilder`; `aliases` gives the
/// parameters and target of each alias. Unlike [`instantiate_to_bounds`]
/// (`instantiateTypeFormalsToBounds`), which replaces cycles by `dynamic`
/// everywhere.
pub fn default_types(
    params: &[TypeParamId],
    alias_target: Option<TypeId>,
    aliases: AliasLookup<'_>,
    table: &mut TypeTable,
    core: &CoreTypes,
) -> Vec<TypeId> {
    let n = params.len();
    let starts: Vec<Position> = params
        .iter()
        .map(|&p| match alias_target {
            None => Position::Covariant,
            Some(tgt) => Position::Covariant.inside(table, tgt, p),
        })
        .collect();
    let mut bounds: Vec<TypeId> = params
        .iter()
        .map(|&p| {
            let d = table.param(p);
            if !d.explicit || matches!(table.get(d.bound), Type::Dynamic) {
                core.dynamic_
            } else {
                d.bound
            }
        })
        .collect();
    // `reaches[i][j]`: the bound of `i` depends, directly or not, on `j`.
    let mut reaches: Vec<Vec<bool>> = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| mentions_parameters(bounds[i], &params[j..=j], table))
                .collect()
        })
        .collect();
    for k in 0..n {
        let through = reaches[k].clone();
        for row in reaches.iter_mut().filter(|row| row[k]) {
            for (cell, &next) in row.iter_mut().zip(through.iter()) {
                *cell |= next;
            }
        }
    }
    let original = bounds.clone();
    for i in 0..n {
        let component: HashMap<TypeParamId, (TypeId, TypeId)> = (0..n)
            .filter(|&j| j == i || (reaches[i][j] && reaches[j][i]))
            .map(|j| (params[j], (core.dynamic_, core.never)))
            .collect();
        bounds[i] = replace_by_position(original[i], &component, starts[i], aliases, table);
    }
    for i in 0..n {
        let one: HashMap<TypeParamId, (TypeId, TypeId)> =
            std::iter::once((params[i], (bounds[i], core.never))).collect();
        for j in 0..n {
            bounds[j] = replace_by_position(bounds[j], &one, starts[j], aliases, table);
        }
    }
    bounds
}

/// Replaces the parameters of `subs` in `t` by their upper type, or by their
/// lower type in a contravariant position. When no lower type is chosen, the
/// result is the ordinary substitution by the upper types, which keeps the
/// display decorations (aliases).
fn replace_by_position(
    t: TypeId,
    subs: &HashMap<TypeParamId, (TypeId, TypeId)>,
    start: Position,
    aliases: AliasLookup<'_>,
    table: &mut TypeTable,
) -> TypeId {
    let keys: Vec<TypeParamId> = subs.keys().copied().collect();
    if !mentions_parameters(t, &keys, table) {
        return t;
    }
    let upper: HashMap<TypeParamId, TypeId> = subs.iter().map(|(&p, &(u, _))| (p, u)).collect();
    let by_upper = substitute(t, &upper, table);
    let by_position = replace_rec(t, subs, &upper, start, aliases, table);
    if table.canonical(by_position) == table.canonical(by_upper) {
        by_upper
    } else {
        by_position
    }
}

fn replace_rec(
    t: TypeId,
    subs: &HashMap<TypeParamId, (TypeId, TypeId)>,
    upper: &HashMap<TypeParamId, TypeId>,
    position: Position,
    aliases: AliasLookup<'_>,
    table: &mut TypeTable,
) -> TypeId {
    if let Some(crate::table::DisplayDecoration::Alias { typedef, args }) =
        table.decoration(t).cloned()
        && let Some((alias_params, target)) = aliases(typedef)
        && alias_params.len() == args.len()
    {
        let new_args: Box<[TypeId]> = args
            .iter()
            .zip(alias_params.iter())
            .map(|(&a, &p)| {
                let inside = position.inside(table, target, p);
                replace_rec(a, subs, upper, inside, aliases, table)
            })
            .collect();
        let map: HashMap<TypeParamId, TypeId> = alias_params
            .iter()
            .copied()
            .zip(new_args.iter().copied())
            .collect();
        let mut expanded = substitute(table.canonical(target), &map, table);
        if table.get(t).is_declared_nullable() {
            expanded = nullable(expanded, table);
        }
        return table.decorate(
            expanded,
            crate::table::DisplayDecoration::Alias {
                typedef,
                args: new_args,
            },
        );
    }
    let t = table.canonical(t);
    match table.get(t).clone() {
        // `visitTypeParameterType` returns the replacement itself.
        Type::TypeParameter { param, .. } => match subs.get(&param) {
            Some(&(_, lower)) if position == Position::Contravariant => lower,
            Some(&(u, _)) => u,
            None => t,
        },
        Type::Interface {
            class,
            args,
            nullable,
        } => {
            let args: Box<[TypeId]> = args
                .iter()
                .map(|&a| replace_rec(a, subs, upper, position, aliases, table))
                .collect();
            table.intern(Type::Interface {
                class,
                args,
                nullable,
            })
        }
        Type::ExtensionType {
            decl,
            args,
            nullable,
        } => {
            let args: Box<[TypeId]> = args
                .iter()
                .map(|&a| replace_rec(a, subs, upper, position, aliases, table))
                .collect();
            table.intern(Type::ExtensionType {
                decl,
                args,
                nullable,
            })
        }
        Type::FutureOr { arg, nullable } => {
            let arg = replace_rec(arg, subs, upper, position, aliases, table);
            table.intern(Type::FutureOr { arg, nullable })
        }
        Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            nullable,
        } => {
            let keys: Vec<TypeParamId> = subs.keys().copied().collect();
            // Bounds of the function's own type parameters that mention the
            // replaced ones: the upper types (a rare shape), with the
            // parameters renamed by the substitution.
            if type_params
                .iter()
                .any(|&p| mentions_parameters(table.param(p).bound, &keys, table))
            {
                return substitute(t, upper, table);
            }
            let against = position.flip();
            let ret = replace_rec(ret, subs, upper, position, aliases, table);
            let positional: Box<[TypeId]> = positional
                .iter()
                .map(|&a| replace_rec(a, subs, upper, against, aliases, table))
                .collect();
            let optional: Box<[TypeId]> = optional
                .iter()
                .map(|&a| replace_rec(a, subs, upper, against, aliases, table))
                .collect();
            let named: Box<[(dartforge_intern::SymbolId, TypeId, bool)]> = named
                .iter()
                .map(|&(s, a, r)| (s, replace_rec(a, subs, upper, against, aliases, table), r))
                .collect();
            table.intern(Type::Function {
                type_params,
                ret,
                positional,
                optional,
                named,
                nullable,
            })
        }
        Type::Record {
            positional,
            named,
            nullable,
        } => {
            let positional: Box<[TypeId]> = positional
                .iter()
                .map(|&a| replace_rec(a, subs, upper, position, aliases, table))
                .collect();
            let named: Box<[(dartforge_intern::SymbolId, TypeId)]> = named
                .iter()
                .map(|&(s, a)| (s, replace_rec(a, subs, upper, position, aliases, table)))
                .collect();
            table.intern(Type::Record {
                positional,
                named,
                nullable,
            })
        }
        _ => substitute(t, upper, table),
    }
}

/// Performs simultaneous substitution of type parameters by arguments.
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
    // Display decoration: substitutes the alias's target and arguments.
    if let Some(ex) = table.decoration(ty).cloned() {
        let c = table.canonical(ty);
        let nc = substitute(c, mapping, table);
        return match ex {
            crate::table::DisplayDecoration::Alias { typedef, args } => {
                let new_: Box<[TypeId]> = args
                    .iter()
                    .map(|&a| substitute(a, mapping, table))
                    .collect();
                if nc == c && new_ == args {
                    ty
                } else {
                    table.decorate(
                        nc,
                        crate::table::DisplayDecoration::Alias {
                            typedef,
                            args: new_,
                        },
                    )
                }
            }
            crate::table::DisplayDecoration::NeverNullable
            | crate::table::DisplayDecoration::Invalid => ty,
        };
    }

    let t = table.get(ty).clone();
    match t {
        Type::Dynamic | Type::Void | Type::Never | Type::Null => ty,
        Type::Intersection { param, bound } => {
            if let Some(&replacement) = mapping.get(&param) {
                replacement
            } else {
                let b = substitute(bound, mapping, table);
                if b == bound {
                    ty
                } else {
                    table.intern(Type::Intersection { param, bound: b })
                }
            }
        }
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
            // Local generic type parameters shadow the mapping
            let mut active_mapping: HashMap<TypeParamId, TypeId> = if type_params.is_empty() {
                mapping.clone()
            } else {
                let mut m = mapping.clone();
                for &p in type_params.iter() {
                    m.remove(&p);
                }
                m
            };
            // Bounds of formals that change with the substitution (`m<T extends X>`
            // of `C<num>`): fresh formals with the substituted bounds, like the
            // analyzer's substituted `FunctionTypeImpl`.
            let mut type_params = type_params;
            let mut formals_changed = false;
            if !type_params.is_empty() && !active_mapping.is_empty() {
                let limits: Vec<TypeId> =
                    type_params.iter().map(|&p| table.param(p).bound).collect();
                let new_bounds: Vec<TypeId> = limits
                    .iter()
                    .map(|&b| substitute(b, &active_mapping, table))
                    .collect();
                if new_bounds != limits {
                    let key = (type_params.clone(), new_bounds.into_boxed_slice());
                    let fresh = match table.fresh_formals.get(&key) {
                        Some(f) => f.clone(),
                        None => {
                            let fresh: Box<[TypeParamId]> = type_params
                                .iter()
                                .map(|&p| {
                                    let d = table.param(p).clone();
                                    let n = table
                                        .alloc_type_param(d.name, d.owner, d.bound, d.variance);
                                    table.param_mut(n).explicit = d.explicit;
                                    n
                                })
                                .collect();
                            let mut renaming = active_mapping.clone();
                            for (&o, &n) in type_params.iter().zip(fresh.iter()) {
                                let tn = table.intern(Type::TypeParameter {
                                    param: n,
                                    nullable: false,
                                });
                                renaming.insert(o, tn);
                            }
                            for (&b, &n) in limits.iter().zip(fresh.iter()) {
                                let nb = substitute(b, &renaming, table);
                                table.param_mut(n).bound = nb;
                            }
                            table.fresh_formals.insert(key, fresh.clone());
                            fresh
                        }
                    };
                    for (&o, &n) in type_params.iter().zip(fresh.iter()) {
                        let tn = table.intern(Type::TypeParameter {
                            param: n,
                            nullable: false,
                        });
                        active_mapping.insert(o, tn);
                    }
                    type_params = fresh;
                    formals_changed = true;
                }
            }

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

            if new_ret != ret || pos_changed || opt_changed || named_changed || formals_changed {
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

/// Normalizes semantic types according to the rules of the Dart
/// specification.
///
/// E.g.:
/// - `FutureOr<Never>` → `Future<Never>`
/// - `FutureOr<Object>` → `Object`
/// - `FutureOr<dynamic>` → `dynamic`
/// - `FutureOr<void>` → `void`
/// - `Null?` → `Null`
/// - `dynamic?` → `dynamic`
/// - `void?` → `void`
pub fn normalize(ty: TypeId, table: &mut TypeTable, core: &CoreTypes) -> TypeId {
    let mut in_progress: Vec<TypeParamId> = Vec::new();
    normalize_(ty, table, core, &mut in_progress)
}

/// `NormalizeHelper` (`analyzer/lib/src/dart/element/normalize.dart` from
/// 3.6.2): `NORM(T)`.
fn normalize_(
    ty: TypeId,
    table: &mut TypeTable,
    core: &CoreTypes,
    in_progress: &mut Vec<TypeParamId>,
) -> TypeId {
    if table.is_invalid(ty) {
        return ty;
    }
    let t = table.get(ty).clone();
    if t.is_declared_nullable() && !matches!(t, Type::Null) {
        return normalize_nullable(ty, table, core, in_progress);
    }
    match t {
        // Primitivos.
        Type::Dynamic | Type::Void | Type::Never | Type::Null => ty,
        Type::Interface { ref args, .. } if args.is_empty() => ty,
        Type::FutureOr { arg, .. } => {
            // `NORM(FutureOr<T>)`.
            let s = normalize_(arg, table, core, in_progress);
            if is_top_norm(table, core, s) {
                return s;
            }
            if s == core.object {
                return s;
            }
            if matches!(table.get(s), Type::Never)
                && let Some(future_class) = core.future_class
            {
                return table.intern(Type::Interface {
                    class: future_class,
                    args: Box::new([core.never]),
                    nullable: false,
                });
            }
            if matches!(table.get(s), Type::Null)
                && let Some(future_class) = core.future_class
            {
                return table.intern(Type::Interface {
                    class: future_class,
                    args: Box::new([core.null]),
                    nullable: true,
                });
            }
            if s == arg {
                return ty;
            }
            table.intern(Type::FutureOr {
                arg: s,
                nullable: false,
            })
        }
        Type::TypeParameter { param, .. } => {
            // `NORM(X extends T)`.
            let data = table.param(param).clone();
            if !data.explicit {
                return ty;
            }
            if in_progress.contains(&param) {
                return ty;
            }
            in_progress.push(param);
            let s = normalize_(data.bound, table, core, in_progress);
            in_progress.pop();
            if matches!(table.get(s), Type::Never) {
                core.never
            } else {
                ty
            }
        }
        Type::Intersection { param, bound } => {
            // `NORM(X & T)`.
            let s = normalize_(bound, table, core, in_progress);
            if matches!(table.get(s), Type::Never) {
                return core.never;
            }
            let x = table.intern(Type::TypeParameter {
                param,
                nullable: false,
            });
            if is_top_norm(table, core, s) {
                return x;
            }
            if matches!(table.get(s), Type::TypeParameter { param: q, nullable: false } if *q == param)
            {
                return x;
            }
            if s == core.object {
                let data = table.param(param).clone();
                if data.explicit {
                    let b = normalize_(data.bound, table, core, in_progress);
                    if b == core.object {
                        return x;
                    }
                }
            }
            if s == bound {
                return ty;
            }
            table.intern(Type::Intersection { param, bound: s })
        }
        Type::Interface { class, args, .. } => {
            let new_: Vec<TypeId> = args
                .iter()
                .map(|&a| normalize_(a, table, core, in_progress))
                .collect();
            if new_[..] == args[..] {
                return ty;
            }
            table.intern(Type::Interface {
                class,
                args: new_.into_boxed_slice(),
                nullable: false,
            })
        }
        Type::ExtensionType { decl, args, .. } => {
            let new_: Vec<TypeId> = args
                .iter()
                .map(|&a| normalize_(a, table, core, in_progress))
                .collect();
            if new_[..] == args[..] {
                return ty;
            }
            table.intern(Type::ExtensionType {
                decl,
                args: new_.into_boxed_slice(),
                nullable: false,
            })
        }
        Type::Record {
            positional, named, ..
        } => {
            let pos: Vec<TypeId> = positional
                .iter()
                .map(|&a| normalize_(a, table, core, in_progress))
                .collect();
            let nm: Vec<(dartforge_intern::SymbolId, TypeId)> = named
                .iter()
                .map(|&(n, a)| (n, normalize_(a, table, core, in_progress)))
                .collect();
            if pos[..] == positional[..] && nm[..] == named[..] {
                return ty;
            }
            table.intern(Type::Record {
                positional: pos.into_boxed_slice(),
                named: nm.into_boxed_slice(),
                nullable: false,
            })
        }
        Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            ..
        } => {
            // `NORM(R Function<X extends B>(S))`: the bounds, the parameters and the
            // return normalized (fresh formals when a bound changes).
            let mut formals = type_params.clone();
            let mut map: HashMap<TypeParamId, TypeId> = HashMap::new();
            if !type_params.is_empty() {
                let limits: Vec<TypeId> =
                    type_params.iter().map(|&p| table.param(p).bound).collect();
                let new_: Vec<TypeId> = type_params
                    .iter()
                    .zip(limits.iter())
                    .map(|(&p, &b)| {
                        if table.param(p).explicit {
                            normalize_(b, table, core, in_progress)
                        } else {
                            b
                        }
                    })
                    .collect();
                if new_ != limits {
                    let key = (type_params.clone(), new_.clone().into_boxed_slice());
                    let fresh = match table.fresh_formals.get(&key) {
                        Some(f) => f.clone(),
                        None => {
                            let fresh: Box<[TypeParamId]> = type_params
                                .iter()
                                .map(|&p| {
                                    let d = table.param(p).clone();
                                    let n = table
                                        .alloc_type_param(d.name, d.owner, d.bound, d.variance);
                                    table.param_mut(n).explicit = d.explicit;
                                    n
                                })
                                .collect();
                            let mut renaming: HashMap<TypeParamId, TypeId> = HashMap::new();
                            for (&o, &n) in type_params.iter().zip(fresh.iter()) {
                                let tn = table.intern(Type::TypeParameter {
                                    param: n,
                                    nullable: false,
                                });
                                renaming.insert(o, tn);
                            }
                            for (&b, &n) in new_.iter().zip(fresh.iter()) {
                                let nb = substitute(b, &renaming, table);
                                table.param_mut(n).bound = nb;
                            }
                            table.fresh_formals.insert(key, fresh.clone());
                            fresh
                        }
                    };
                    for (&o, &n) in type_params.iter().zip(fresh.iter()) {
                        let tn = table.intern(Type::TypeParameter {
                            param: n,
                            nullable: false,
                        });
                        map.insert(o, tn);
                    }
                    formals = fresh;
                }
            }
            let norm = |x: TypeId, table: &mut TypeTable, in_progress: &mut Vec<TypeParamId>| {
                let x = if map.is_empty() {
                    x
                } else {
                    substitute(x, &map, table)
                };
                normalize_(x, table, core, in_progress)
            };
            let r = norm(ret, table, in_progress);
            let pos: Vec<TypeId> = positional
                .iter()
                .map(|&a| norm(a, table, in_progress))
                .collect();
            let opt: Vec<TypeId> = optional
                .iter()
                .map(|&a| norm(a, table, in_progress))
                .collect();
            let nm: Vec<(dartforge_intern::SymbolId, TypeId, bool)> = named
                .iter()
                .map(|&(n, a, req)| (n, norm(a, table, in_progress), req))
                .collect();
            if formals == type_params
                && r == ret
                && pos[..] == positional[..]
                && opt[..] == optional[..]
                && nm[..] == named[..]
            {
                return ty;
            }
            table.intern(Type::Function {
                type_params: formals,
                ret: r,
                positional: pos.into_boxed_slice(),
                optional: opt.into_boxed_slice(),
                named: nm.into_boxed_slice(),
                nullable: false,
            })
        }
    }
}

/// `NORM(T?)`.
fn normalize_nullable(
    ty: TypeId,
    table: &mut TypeTable,
    core: &CoreTypes,
    in_progress: &mut Vec<TypeParamId>,
) -> TypeId {
    let without = without_nullable(ty, table);
    let s = normalize_(without, table, core, in_progress);
    if is_top_norm(table, core, s) {
        return s;
    }
    match table.get(s) {
        Type::Never | Type::Null => return core.null,
        Type::FutureOr {
            arg,
            nullable: false,
        } => {
            let r = *arg;
            if nullable_norm(table, r) {
                return s;
            }
        }
        _ => {}
    }
    nullable(s, table)
}

/// `T` without the `?` (`withNullability(none)`).
fn without_nullable(ty: TypeId, table: &mut TypeTable) -> TypeId {
    let t = table.get(ty).clone();
    match t {
        Type::Interface {
            class,
            args,
            nullable: true,
        } => table.intern(Type::Interface {
            class,
            args,
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
        Type::FutureOr {
            arg,
            nullable: true,
        } => table.intern(Type::FutureOr {
            arg,
            nullable: false,
        }),
        Type::TypeParameter {
            param,
            nullable: true,
        } => table.intern(Type::TypeParameter {
            param,
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
        _ => ty,
    }
}

/// `TypeSystem.isTop`.
fn is_top_norm(table: &TypeTable, core: &CoreTypes, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void => true,
        Type::Interface {
            class, nullable, ..
        } => *nullable && Some(*class) == core.object_class,
        Type::FutureOr { arg, nullable } => {
            is_top_norm(table, core, *arg) || (*nullable && table.canonical(*arg) == core.object)
        }
        _ => table.is_invalid(t),
    }
}

/// `TypeSystem.isNullable`.
fn nullable_norm(table: &TypeTable, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => true,
        Type::FutureOr { arg, nullable } => *nullable || nullable_norm(table, *arg),
        other => other.is_declared_nullable() || table.is_invalid(t),
    }
}

/// `TopMergeHelper.topMerge` (`analyzer/lib/src/dart/element/top_merge.dart`):
/// `NNBD_TOP_MERGE(t, s)`; `None` where the original throws (types that are
/// not structurally equal).
pub fn top_merge(table: &mut TypeTable, core: &CoreTypes, t: TypeId, s: TypeId) -> Option<TypeId> {
    top_merge_rec(table, core, t, s, 0)
}

fn nullable_object_tm(table: &TypeTable, core: &CoreTypes, t: TypeId) -> bool {
    matches!(table.get(t), Type::Interface { class, nullable: true, .. } if Some(*class) == core.object_class)
}

fn nullable_tm(table: &TypeTable, t: TypeId) -> bool {
    table.get(t).is_declared_nullable()
}

fn with_nullable_tm(table: &mut TypeTable, t: TypeId, nullable: bool) -> TypeId {
    let the_new_ = match table.get(t).clone() {
        Type::Interface { class, args, .. } => Type::Interface {
            class,
            args,
            nullable,
        },
        Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            ..
        } => Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            nullable,
        },
        Type::Record {
            positional, named, ..
        } => Type::Record {
            positional,
            named,
            nullable,
        },
        Type::TypeParameter { param, .. } => Type::TypeParameter { param, nullable },
        Type::FutureOr { arg, .. } => Type::FutureOr { arg, nullable },
        Type::ExtensionType { decl, args, .. } => Type::ExtensionType {
            decl,
            args,
            nullable,
        },
        _ => return t,
    };
    table.intern(the_new_)
}

fn top_merge_rec(
    table: &mut TypeTable,
    core: &CoreTypes,
    t: TypeId,
    s: TypeId,
    prof: u32,
) -> Option<TypeId> {
    if prof > 32 {
        return None;
    }
    let (tt, st) = (table.get(t).clone(), table.get(s).clone());
    let (t_oq, s_oq) = (
        nullable_object_tm(table, core, t),
        nullable_object_tm(table, core, s),
    );
    if t_oq && s_oq {
        return Some(t);
    }
    let (t_inv, the_s_inv) = (table.is_invalid(t), table.is_invalid(s));
    let t_dyn = matches!(tt, Type::Dynamic) && !t_inv;
    let s_dyn = matches!(st, Type::Dynamic) && !the_s_inv;
    if t_dyn && s_dyn {
        return Some(core.dynamic_);
    }
    if t_inv || the_s_inv {
        return Some(if t_inv { t } else { s });
    }
    if matches!(tt, Type::Never) && matches!(st, Type::Never) {
        return Some(core.never);
    }
    let (t_void, s_void) = (matches!(tt, Type::Void), matches!(st, Type::Void));
    if t_void && s_void {
        return Some(core.void_);
    }
    if (t_oq && s_void) || (t_void && s_oq) || (t_dyn && s_void) || (t_void && s_dyn) {
        return Some(core.object_nullable);
    }
    if t_oq && s_dyn {
        return Some(t);
    }
    if t_dyn && s_oq {
        return Some(s);
    }
    let (tq, sq) = (nullable_tm(table, t), nullable_tm(table, s));
    if tq && sq {
        let tn = with_nullable_tm(table, t, false);
        let sn = with_nullable_tm(table, s, false);
        let r = top_merge_rec(table, core, tn, sn, prof + 1)?;
        return Some(with_nullable_tm(table, r, true));
    } else if tq || sq {
        return None;
    }
    match (tt, st) {
        (
            Type::Interface {
                class: a, args: xa, ..
            },
            Type::Interface {
                class: b, args: xb, ..
            },
        ) => {
            if a != b {
                return None;
            }
            if xa.is_empty() {
                return Some(t);
            }
            let mut args = Vec::with_capacity(xa.len());
            for (x, y) in xa.iter().zip(xb.iter()) {
                args.push(top_merge_rec(table, core, *x, *y, prof + 1)?);
            }
            Some(table.intern(Type::Interface {
                class: a,
                args: args.into_boxed_slice(),
                nullable: false,
            }))
        }
        (
            Type::ExtensionType {
                decl: a, args: xa, ..
            },
            Type::ExtensionType {
                decl: b, args: xb, ..
            },
        ) => {
            if a != b {
                return None;
            }
            if xa.is_empty() {
                return Some(t);
            }
            let mut args = Vec::with_capacity(xa.len());
            for (x, y) in xa.iter().zip(xb.iter()) {
                args.push(top_merge_rec(table, core, *x, *y, prof + 1)?);
            }
            Some(table.intern(Type::ExtensionType {
                decl: a,
                args: args.into_boxed_slice(),
                nullable: false,
            }))
        }
        (Type::FutureOr { arg: a, .. }, Type::FutureOr { arg: b, .. }) => {
            let r = top_merge_rec(table, core, a, b, prof + 1)?;
            Some(table.intern(Type::FutureOr {
                arg: r,
                nullable: false,
            }))
        }
        (
            Type::Function {
                type_params: tp_a,
                ret: ra,
                positional: pa,
                optional: oa,
                named: at,
                ..
            },
            Type::Function {
                type_params: tp_b,
                ret: rb,
                positional: pb,
                optional: ob,
                named: nb,
                ..
            },
        ) => {
            if tp_a.len() != tp_b.len()
                || pa.len() != pb.len()
                || oa.len() != ob.len()
                || at.len() != nb.len()
            {
                return None;
            }
            // The type parameters of `s` renamed to those of `t`
            // (bounds must exist on both or on neither and be
            // merged).
            let mut map: HashMap<TypeParamId, TypeId> = HashMap::new();
            for (x, y) in tp_b.iter().zip(tp_a.iter()) {
                let ty = table.intern(Type::TypeParameter {
                    param: *y,
                    nullable: false,
                });
                map.insert(*x, ty);
            }
            for (x, y) in tp_a.iter().zip(tp_b.iter()) {
                let (dx, dy) = (table.param(*x).clone(), table.param(*y).clone());
                if dx.explicit != dy.explicit {
                    return None;
                }
                if dx.explicit {
                    let by = substitute(dy.bound, &map, table);
                    top_merge_rec(table, core, dx.bound, by, prof + 1)?;
                }
            }
            let sub = |table: &mut TypeTable, x: TypeId| substitute(x, &map, table);
            let rb = sub(table, rb);
            let ret = top_merge_rec(table, core, ra, rb, prof + 1)?;
            let mut pos = Vec::new();
            for (x, y) in pa.iter().zip(pb.iter()) {
                let y = sub(table, *y);
                pos.push(top_merge_rec(table, core, *x, y, prof + 1)?);
            }
            let mut the_opt = Vec::new();
            for (x, y) in oa.iter().zip(ob.iter()) {
                let y = sub(table, *y);
                the_opt.push(top_merge_rec(table, core, *x, y, prof + 1)?);
            }
            let mut named_args: Vec<(dartforge_intern::SymbolId, TypeId, bool)> = Vec::new();
            for ((n1, x, r1), (n2, y, r2)) in at.iter().zip(nb.iter()) {
                if n1 != n2 {
                    return None;
                }
                let y = sub(table, *y);
                named_args.push((
                    *n1,
                    top_merge_rec(table, core, *x, y, prof + 1)?,
                    *r1 || *r2,
                ));
            }
            Some(table.intern(Type::Function {
                type_params: tp_a,
                ret,
                positional: pos.into_boxed_slice(),
                optional: the_opt.into_boxed_slice(),
                named: named_args.into_boxed_slice(),
                nullable: false,
            }))
        }
        (
            Type::Record {
                positional: pa,
                named: at,
                ..
            },
            Type::Record {
                positional: pb,
                named: nb,
                ..
            },
        ) => {
            if pa.len() != pb.len() || at.len() != nb.len() {
                return None;
            }
            let mut pos = Vec::new();
            for (x, y) in pa.iter().zip(pb.iter()) {
                pos.push(top_merge_rec(table, core, *x, *y, prof + 1)?);
            }
            let mut named_args = Vec::new();
            for ((n1, x), (n2, y)) in at.iter().zip(nb.iter()) {
                if n1 != n2 {
                    return None;
                }
                named_args.push((*n1, top_merge_rec(table, core, *x, *y, prof + 1)?));
            }
            Some(table.intern(Type::Record {
                positional: pos.into_boxed_slice(),
                named: named_args.into_boxed_slice(),
                nullable: false,
            }))
        }
        (Type::TypeParameter { param: a, .. }, Type::TypeParameter { param: b, .. }) if a == b => {
            Some(t)
        }
        (Type::Null, Type::Null) => Some(t),
        _ => None,
    }
}

/// Query function for the instantiated representation type of an extension type.
pub type ExtensionTypeEraser<'a> = &'a dyn Fn(ClassId, &[TypeId], &mut TypeTable) -> Option<TypeId>;

/// Recursively erases extension types (*extension type erasure*), replacing
/// them with their instantiated representation type.
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
            // Erases the arguments first
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

/// Greatest lower bound: **DOWN**(`a`, `b`) from `upper-lower-bounds.md`.
pub use crate::bounds::down as glb;
/// Least upper bound: **UP**(`a`, `b`) from `upper-lower-bounds.md`.
pub use crate::bounds::up as lub;

/// The type without any display decoration (alias, `Never?`), throughout
/// the structure: the semantic identity, for comparing `TypeId`s
/// (`a as N` with `typedef N = num` and `a: num` is the same type).
pub fn without_display(ty: TypeId, table: &mut TypeTable) -> TypeId {
    let c = table.canonical(ty);
    if table.empty_display() {
        return c;
    }
    let t = table.get(c).clone();
    let f = |x: TypeId, table: &mut TypeTable| without_display(x, table);
    let the_new_ = match t {
        Type::Interface {
            class,
            args,
            nullable,
        } => Type::Interface {
            class,
            args: args.iter().map(|&a| f(a, table)).collect(),
            nullable,
        },
        Type::ExtensionType {
            decl,
            args,
            nullable,
        } => Type::ExtensionType {
            decl,
            args: args.iter().map(|&a| f(a, table)).collect(),
            nullable,
        },
        Type::FutureOr { arg, nullable } => Type::FutureOr {
            arg: f(arg, table),
            nullable,
        },
        Type::Record {
            positional,
            named,
            nullable,
        } => Type::Record {
            positional: positional.iter().map(|&a| f(a, table)).collect(),
            named: named.iter().map(|&(n, a)| (n, f(a, table))).collect(),
            nullable,
        },
        Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            nullable,
        } => Type::Function {
            type_params,
            ret: f(ret, table),
            positional: positional.iter().map(|&a| f(a, table)).collect(),
            optional: optional.iter().map(|&a| f(a, table)).collect(),
            named: named.iter().map(|&(n, a, r)| (n, f(a, table), r)).collect(),
            nullable,
        },
        Type::Intersection { param, bound } => Type::Intersection {
            param,
            bound: f(bound, table),
        },
        _ => return c,
    };
    table.intern(the_new_)
}
