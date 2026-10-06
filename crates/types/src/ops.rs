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
    // Decorados de exibição (C9): o alias anulável continua o alias.
    if let Some(ex) = table.exibicao(ty).cloned() {
        let c = table.canonico(ty);
        let n = nullable(c, table);
        return match ex {
            crate::table::Exibicao::NeverAnulavel | crate::table::Exibicao::Invalido => ty,
            ex if n != c => table.decorar(n, ex),
            _ => ty,
        };
    }
    let t = table.get(ty).clone();
    match t {
        Type::Dynamic | Type::Void | Type::Null => ty,
        Type::Intersection { param, .. } => table.intern(Type::TypeParameter { param, nullable: true }),
        Type::Never => {
            // Never? === Null (exibido `Never?` quando a tabela preserva a exibição).
            let n = table.intern(Type::Null);
            table.decorar(n, crate::table::Exibicao::NeverAnulavel)
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
    if let Some(ex) = table.exibicao(ty).cloned() {
        let c = table.canonico(ty);
        let n = non_nullable(c, table);
        return match ex {
            crate::table::Exibicao::Alias { .. } if n != c => table.decorar(n, ex),
            crate::table::Exibicao::Alias { .. } => ty,
            crate::table::Exibicao::NeverAnulavel => n,
            crate::table::Exibicao::Invalido => ty,
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

/// Se `t` menciona algum dos parâmetros `params`.
pub fn menciona_parametros(t: TypeId, params: &[TypeParamId], table: &TypeTable) -> bool {
    match table.get(t) {
        Type::TypeParameter { param, .. } => params.contains(param),
        Type::Intersection { param, bound } => params.contains(param) || menciona_parametros(*bound, params, table),
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => {
            args.iter().any(|a| menciona_parametros(*a, params, table))
        }
        Type::FutureOr { arg, .. } => menciona_parametros(*arg, params, table),
        Type::Function { ret, positional, optional, named, type_params, .. } => {
            type_params.iter().any(|p| menciona_parametros(table.param(*p).bound, params, table))
                || menciona_parametros(*ret, params, table)
                || positional.iter().chain(optional.iter()).any(|a| menciona_parametros(*a, params, table))
                || named.iter().any(|(_, a, _)| menciona_parametros(*a, params, table))
        }
        Type::Record { positional, named, .. } => {
            positional.iter().any(|a| menciona_parametros(*a, params, table))
                || named.iter().any(|(_, a)| menciona_parametros(*a, params, table))
        }
        _ => false,
    }
}

/// Instanciação para os limites (*instantiate to bounds*) de `params`, com os
/// já conhecidos em `fixos`: parâmetro sem limite escrito vai a `dynamic`;
/// com limite, ao limite com os parâmetros já instanciados substituídos, na
/// ordem das dependências (`A<T extends U, U extends num>` é
/// `A<num, num>`). Os que restam num ciclo de limites (F-limites,
/// `T extends Comparable<T>`) têm os parâmetros do ciclo trocados por
/// `dynamic`.
pub fn instanciar_para_limites(
    params: &[TypeParamId],
    fixos: &[Option<TypeId>],
    table: &mut TypeTable,
    core: &CoreTypes,
) -> Vec<TypeId> {
    let n = params.len();
    let mut args: Vec<Option<TypeId>> = (0..n).map(|i| fixos.get(i).copied().flatten()).collect();
    for i in 0..n {
        let d = table.param(params[i]);
        if args[i].is_none() && (!d.explicito || matches!(table.get(d.bound), Type::Dynamic)) {
            args[i] = Some(core.dynamic_);
        }
    }
    loop {
        let pendentes: Vec<TypeParamId> = (0..n).filter(|&i| args[i].is_none()).map(|i| params[i]).collect();
        if pendentes.is_empty() {
            break;
        }
        let mapa: HashMap<TypeParamId, TypeId> =
            (0..n).filter_map(|i| args[i].map(|a| (params[i], a))).collect();
        let mut progresso = false;
        for i in 0..n {
            if args[i].is_some() {
                continue;
            }
            let b = table.param(params[i]).bound;
            if !menciona_parametros(b, &pendentes, table) {
                args[i] = Some(substitute(b, &mapa, table));
                progresso = true;
            }
        }
        if !progresso {
            let mapa: HashMap<TypeParamId, TypeId> =
                (0..n).map(|i| (params[i], args[i].unwrap_or(core.dynamic_))).collect();
            for i in 0..n {
                if args[i].is_none() {
                    let b = table.param(params[i]).bound;
                    args[i] = Some(substitute(b, &mapa, table));
                }
            }
            break;
        }
    }
    args.into_iter().map(|a| a.unwrap_or(core.dynamic_)).collect()
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
    // Decorado de exibição: substitui o alvo e os argumentos do alias.
    if let Some(ex) = table.exibicao(ty).cloned() {
        let c = table.canonico(ty);
        let nc = substitute(c, mapping, table);
        return match ex {
            crate::table::Exibicao::Alias { typedef, args } => {
                let novos: Box<[TypeId]> = args.iter().map(|&a| substitute(a, mapping, table)).collect();
                if nc == c && novos == args {
                    ty
                } else {
                    table.decorar(nc, crate::table::Exibicao::Alias { typedef, args: novos })
                }
            }
            crate::table::Exibicao::NeverAnulavel | crate::table::Exibicao::Invalido => ty,
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
            // Parâmetros de tipo genéricos locais sombreiam o mapeamento
            let mut active_mapping: HashMap<TypeParamId, TypeId> = if type_params.is_empty() {
                mapping.clone()
            } else {
                let mut m = mapping.clone();
                for &p in type_params.iter() {
                    m.remove(&p);
                }
                m
            };
            // Limites dos formais que mudam com a substituição (`m<T extends X>`
            // de `C<num>`): formais frescos com os limites substituídos, como o
            // `FunctionTypeImpl` substituído do analyzer.
            let mut type_params = type_params;
            let mut formais_mudaram = false;
            if !type_params.is_empty() && !active_mapping.is_empty() {
                let limites: Vec<TypeId> = type_params.iter().map(|&p| table.param(p).bound).collect();
                let novos_limites: Vec<TypeId> = limites.iter().map(|&b| substitute(b, &active_mapping, table)).collect();
                if novos_limites != limites {
                    let chave = (type_params.clone(), novos_limites.into_boxed_slice());
                    let frescos = match table.formais_frescos.get(&chave) {
                        Some(f) => f.clone(),
                        None => {
                            let frescos: Box<[TypeParamId]> = type_params
                                .iter()
                                .map(|&p| {
                                    let d = table.param(p).clone();
                                    let n = table.alloc_type_param(d.name, d.owner.clone(), d.bound, d.variance);
                                    table.param_mut(n).explicito = d.explicito;
                                    n
                                })
                                .collect();
                            let mut renomeio = active_mapping.clone();
                            for (&o, &n) in type_params.iter().zip(frescos.iter()) {
                                let tn = table.intern(Type::TypeParameter { param: n, nullable: false });
                                renomeio.insert(o, tn);
                            }
                            for (&b, &n) in limites.iter().zip(frescos.iter()) {
                                let nb = substitute(b, &renomeio, table);
                                table.param_mut(n).bound = nb;
                            }
                            table.formais_frescos.insert(chave, frescos.clone());
                            frescos
                        }
                    };
                    for (&o, &n) in type_params.iter().zip(frescos.iter()) {
                        let tn = table.intern(Type::TypeParameter { param: n, nullable: false });
                        active_mapping.insert(o, tn);
                    }
                    type_params = frescos;
                    formais_mudaram = true;
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

            if new_ret != ret || pos_changed || opt_changed || named_changed || formais_mudaram {
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
    let mut em_curso: Vec<TypeParamId> = Vec::new();
    normalizar(ty, table, core, &mut em_curso)
}

/// `NormalizeHelper` (`analyzer/lib/src/dart/element/normalize.dart` do
/// 3.6.2): `NORM(T)`.
fn normalizar(ty: TypeId, table: &mut TypeTable, core: &CoreTypes, em_curso: &mut Vec<TypeParamId>) -> TypeId {
    if table.e_invalido(ty) {
        return ty;
    }
    let t = table.get(ty).clone();
    if t.is_declared_nullable() && !matches!(t, Type::Null) {
        return normalizar_anulavel(ty, table, core, em_curso);
    }
    match t {
        // Primitivos.
        Type::Dynamic | Type::Void | Type::Never | Type::Null => ty,
        Type::Interface { ref args, .. } if args.is_empty() => ty,
        Type::FutureOr { arg, .. } => {
            // `NORM(FutureOr<T>)`.
            let s = normalizar(arg, table, core, em_curso);
            if e_topo_norm(table, core, s) {
                return s;
            }
            if s == core.object {
                return s;
            }
            if matches!(table.get(s), Type::Never) {
                if let Some(future_class) = core.future_class {
                    return table.intern(Type::Interface { class: future_class, args: Box::new([core.never]), nullable: false });
                }
            }
            if matches!(table.get(s), Type::Null) {
                if let Some(future_class) = core.future_class {
                    return table.intern(Type::Interface { class: future_class, args: Box::new([core.null]), nullable: true });
                }
            }
            if s == arg {
                return ty;
            }
            table.intern(Type::FutureOr { arg: s, nullable: false })
        }
        Type::TypeParameter { param, .. } => {
            // `NORM(X extends T)`.
            let dados = table.param(param).clone();
            if !dados.explicito {
                return ty;
            }
            if em_curso.contains(&param) {
                return ty;
            }
            em_curso.push(param);
            let s = normalizar(dados.bound, table, core, em_curso);
            em_curso.pop();
            if matches!(table.get(s), Type::Never) { core.never } else { ty }
        }
        Type::Intersection { param, bound } => {
            // `NORM(X & T)`.
            let s = normalizar(bound, table, core, em_curso);
            if matches!(table.get(s), Type::Never) {
                return core.never;
            }
            let x = table.intern(Type::TypeParameter { param, nullable: false });
            if e_topo_norm(table, core, s) {
                return x;
            }
            if matches!(table.get(s), Type::TypeParameter { param: q, nullable: false } if *q == param) {
                return x;
            }
            if s == core.object {
                let dados = table.param(param).clone();
                if dados.explicito {
                    let b = normalizar(dados.bound, table, core, em_curso);
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
            let novos: Vec<TypeId> = args.iter().map(|&a| normalizar(a, table, core, em_curso)).collect();
            if novos[..] == args[..] {
                return ty;
            }
            table.intern(Type::Interface { class, args: novos.into_boxed_slice(), nullable: false })
        }
        Type::ExtensionType { decl, args, .. } => {
            let novos: Vec<TypeId> = args.iter().map(|&a| normalizar(a, table, core, em_curso)).collect();
            if novos[..] == args[..] {
                return ty;
            }
            table.intern(Type::ExtensionType { decl, args: novos.into_boxed_slice(), nullable: false })
        }
        Type::Record { positional, named, .. } => {
            let pos: Vec<TypeId> = positional.iter().map(|&a| normalizar(a, table, core, em_curso)).collect();
            let nm: Vec<(dartforge_intern::SymbolId, TypeId)> = named.iter().map(|&(n, a)| (n, normalizar(a, table, core, em_curso))).collect();
            if pos[..] == positional[..] && nm[..] == named[..] {
                return ty;
            }
            table.intern(Type::Record { positional: pos.into_boxed_slice(), named: nm.into_boxed_slice(), nullable: false })
        }
        Type::Function { type_params, ret, positional, optional, named, .. } => {
            // `NORM(R Function<X extends B>(S))`: os limites, os parâmetros e
            // o retorno normalizados (formais frescos quando um limite muda).
            let mut formais = type_params.clone();
            let mut mapa: HashMap<TypeParamId, TypeId> = HashMap::new();
            if !type_params.is_empty() {
                let limites: Vec<TypeId> = type_params.iter().map(|&p| table.param(p).bound).collect();
                let novos: Vec<TypeId> = type_params
                    .iter()
                    .zip(limites.iter())
                    .map(|(&p, &b)| if table.param(p).explicito { normalizar(b, table, core, em_curso) } else { b })
                    .collect();
                if novos != limites {
                    let chave = (type_params.clone(), novos.clone().into_boxed_slice());
                    let frescos = match table.formais_frescos.get(&chave) {
                        Some(f) => f.clone(),
                        None => {
                            let frescos: Box<[TypeParamId]> = type_params
                                .iter()
                                .map(|&p| {
                                    let d = table.param(p).clone();
                                    let n = table.alloc_type_param(d.name, d.owner.clone(), d.bound, d.variance);
                                    table.param_mut(n).explicito = d.explicito;
                                    n
                                })
                                .collect();
                            let mut renomeio: HashMap<TypeParamId, TypeId> = HashMap::new();
                            for (&o, &n) in type_params.iter().zip(frescos.iter()) {
                                let tn = table.intern(Type::TypeParameter { param: n, nullable: false });
                                renomeio.insert(o, tn);
                            }
                            for (&b, &n) in novos.iter().zip(frescos.iter()) {
                                let nb = substitute(b, &renomeio, table);
                                table.param_mut(n).bound = nb;
                            }
                            table.formais_frescos.insert(chave, frescos.clone());
                            frescos
                        }
                    };
                    for (&o, &n) in type_params.iter().zip(frescos.iter()) {
                        let tn = table.intern(Type::TypeParameter { param: n, nullable: false });
                        mapa.insert(o, tn);
                    }
                    formais = frescos;
                }
            }
            let norm = |x: TypeId, table: &mut TypeTable, em_curso: &mut Vec<TypeParamId>| {
                let x = if mapa.is_empty() { x } else { substitute(x, &mapa, table) };
                normalizar(x, table, core, em_curso)
            };
            let r = norm(ret, table, em_curso);
            let pos: Vec<TypeId> = positional.iter().map(|&a| norm(a, table, em_curso)).collect();
            let opt: Vec<TypeId> = optional.iter().map(|&a| norm(a, table, em_curso)).collect();
            let nm: Vec<(dartforge_intern::SymbolId, TypeId, bool)> = named.iter().map(|&(n, a, req)| (n, norm(a, table, em_curso), req)).collect();
            if formais == type_params && r == ret && pos[..] == positional[..] && opt[..] == optional[..] && nm[..] == named[..] {
                return ty;
            }
            table.intern(Type::Function {
                type_params: formais,
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
fn normalizar_anulavel(ty: TypeId, table: &mut TypeTable, core: &CoreTypes, em_curso: &mut Vec<TypeParamId>) -> TypeId {
    let sem = sem_anulavel(ty, table);
    let s = normalizar(sem, table, core, em_curso);
    if e_topo_norm(table, core, s) {
        return s;
    }
    match table.get(s) {
        Type::Never | Type::Null => return core.null,
        Type::FutureOr { arg, nullable: false } => {
            let r = *arg;
            if anulavel_norm(table, r) {
                return s;
            }
        }
        _ => {}
    }
    nullable(s, table)
}

/// `T` sem o `?` (`withNullability(none)`).
fn sem_anulavel(ty: TypeId, table: &mut TypeTable) -> TypeId {
    let t = table.get(ty).clone();
    match t {
        Type::Interface { class, args, nullable: true } => table.intern(Type::Interface { class, args, nullable: false }),
        Type::ExtensionType { decl, args, nullable: true } => table.intern(Type::ExtensionType { decl, args, nullable: false }),
        Type::FutureOr { arg, nullable: true } => table.intern(Type::FutureOr { arg, nullable: false }),
        Type::TypeParameter { param, nullable: true } => table.intern(Type::TypeParameter { param, nullable: false }),
        Type::Record { positional, named, nullable: true } => table.intern(Type::Record { positional, named, nullable: false }),
        Type::Function { type_params, ret, positional, optional, named, nullable: true } => {
            table.intern(Type::Function { type_params, ret, positional, optional, named, nullable: false })
        }
        _ => ty,
    }
}

/// `TypeSystem.isTop`.
fn e_topo_norm(table: &TypeTable, core: &CoreTypes, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void => true,
        Type::Interface { class, nullable, .. } => *nullable && Some(*class) == core.object_class,
        Type::FutureOr { arg, nullable } => e_topo_norm(table, core, *arg) || (*nullable && table.canonico(*arg) == core.object),
        _ => table.e_invalido(t),
    }
}

/// `TypeSystem.isNullable`.
fn anulavel_norm(table: &TypeTable, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => true,
        Type::FutureOr { arg, nullable } => *nullable || anulavel_norm(table, *arg),
        other => other.is_declared_nullable() || table.e_invalido(t),
    }
}

/// `TopMergeHelper.topMerge` (`analyzer/lib/src/dart/element/top_merge.dart`):
/// `NNBD_TOP_MERGE(t, s)`; `None` onde o original lança (tipos que não são
/// estruturalmente iguais).
pub fn top_merge(table: &mut TypeTable, core: &CoreTypes, t: TypeId, s: TypeId) -> Option<TypeId> {
    top_merge_rec(table, core, t, s, 0)
}

fn objeto_anulavel_tm(table: &TypeTable, core: &CoreTypes, t: TypeId) -> bool {
    matches!(table.get(t), Type::Interface { class, nullable: true, .. } if Some(*class) == core.object_class)
}

fn anulavel_tm(table: &TypeTable, t: TypeId) -> bool {
    table.get(t).is_declared_nullable()
}

fn com_anulavel_tm(table: &mut TypeTable, t: TypeId, nullable: bool) -> TypeId {
    let novo = match table.get(t).clone() {
        Type::Interface { class, args, .. } => Type::Interface { class, args, nullable },
        Type::Function { type_params, ret, positional, optional, named, .. } => Type::Function { type_params, ret, positional, optional, named, nullable },
        Type::Record { positional, named, .. } => Type::Record { positional, named, nullable },
        Type::TypeParameter { param, .. } => Type::TypeParameter { param, nullable },
        Type::FutureOr { arg, .. } => Type::FutureOr { arg, nullable },
        Type::ExtensionType { decl, args, .. } => Type::ExtensionType { decl, args, nullable },
        _ => return t,
    };
    table.intern(novo)
}

fn top_merge_rec(table: &mut TypeTable, core: &CoreTypes, t: TypeId, s: TypeId, prof: u32) -> Option<TypeId> {
    if prof > 32 {
        return None;
    }
    let (tt, st) = (table.get(t).clone(), table.get(s).clone());
    let (t_oq, s_oq) = (objeto_anulavel_tm(table, core, t), objeto_anulavel_tm(table, core, s));
    if t_oq && s_oq {
        return Some(t);
    }
    let (t_inv, s_inv) = (table.e_invalido(t), table.e_invalido(s));
    let t_dyn = matches!(tt, Type::Dynamic) && !t_inv;
    let s_dyn = matches!(st, Type::Dynamic) && !s_inv;
    if t_dyn && s_dyn {
        return Some(core.dynamic_);
    }
    if t_inv || s_inv {
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
    let (tq, sq) = (anulavel_tm(table, t), anulavel_tm(table, s));
    if tq && sq {
        let tn = com_anulavel_tm(table, t, false);
        let sn = com_anulavel_tm(table, s, false);
        let r = top_merge_rec(table, core, tn, sn, prof + 1)?;
        return Some(com_anulavel_tm(table, r, true));
    } else if tq || sq {
        return None;
    }
    match (tt, st) {
        (Type::Interface { class: a, args: xa, .. }, Type::Interface { class: b, args: xb, .. }) => {
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
            Some(table.intern(Type::Interface { class: a, args: args.into_boxed_slice(), nullable: false }))
        }
        (Type::ExtensionType { decl: a, args: xa, .. }, Type::ExtensionType { decl: b, args: xb, .. }) => {
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
            Some(table.intern(Type::ExtensionType { decl: a, args: args.into_boxed_slice(), nullable: false }))
        }
        (Type::FutureOr { arg: a, .. }, Type::FutureOr { arg: b, .. }) => {
            let r = top_merge_rec(table, core, a, b, prof + 1)?;
            Some(table.intern(Type::FutureOr { arg: r, nullable: false }))
        }
        (Type::Function { type_params: tp_a, ret: ra, positional: pa, optional: oa, named: na, .. }, Type::Function { type_params: tp_b, ret: rb, positional: pb, optional: ob, named: nb, .. }) => {
            if tp_a.len() != tp_b.len() || pa.len() != pb.len() || oa.len() != ob.len() || na.len() != nb.len() {
                return None;
            }
            // Os parâmetros de tipo de `s` renomeados para os de `t`
            // (os limites têm de existir nos dois ou em nenhum e se
            // juntar).
            let mut mapa: HashMap<TypeParamId, TypeId> = HashMap::new();
            for (x, y) in tp_b.iter().zip(tp_a.iter()) {
                let ty = table.intern(Type::TypeParameter { param: *y, nullable: false });
                mapa.insert(*x, ty);
            }
            for (x, y) in tp_a.iter().zip(tp_b.iter()) {
                let (dx, dy) = (table.param(*x).clone(), table.param(*y).clone());
                if dx.explicito != dy.explicito {
                    return None;
                }
                if dx.explicito {
                    let by = substitute(dy.bound, &mapa, table);
                    top_merge_rec(table, core, dx.bound, by, prof + 1)?;
                }
            }
            let sub = |table: &mut TypeTable, x: TypeId| substitute(x, &mapa, table);
            let rb = sub(table, rb);
            let ret = top_merge_rec(table, core, ra, rb, prof + 1)?;
            let mut pos = Vec::new();
            for (x, y) in pa.iter().zip(pb.iter()) {
                let y = sub(table, *y);
                pos.push(top_merge_rec(table, core, *x, y, prof + 1)?);
            }
            let mut opc = Vec::new();
            for (x, y) in oa.iter().zip(ob.iter()) {
                let y = sub(table, *y);
                opc.push(top_merge_rec(table, core, *x, y, prof + 1)?);
            }
            let mut nomeados: Vec<(dartforge_intern::SymbolId, TypeId, bool)> = Vec::new();
            for ((n1, x, r1), (n2, y, r2)) in na.iter().zip(nb.iter()) {
                if n1 != n2 {
                    return None;
                }
                let y = sub(table, *y);
                nomeados.push((*n1, top_merge_rec(table, core, *x, y, prof + 1)?, *r1 || *r2));
            }
            Some(table.intern(Type::Function {
                type_params: tp_a,
                ret,
                positional: pos.into_boxed_slice(),
                optional: opc.into_boxed_slice(),
                named: nomeados.into_boxed_slice(),
                nullable: false,
            }))
        }
        (Type::Record { positional: pa, named: na, .. }, Type::Record { positional: pb, named: nb, .. }) => {
            if pa.len() != pb.len() || na.len() != nb.len() {
                return None;
            }
            let mut pos = Vec::new();
            for (x, y) in pa.iter().zip(pb.iter()) {
                pos.push(top_merge_rec(table, core, *x, *y, prof + 1)?);
            }
            let mut nomeados = Vec::new();
            for ((n1, x), (n2, y)) in na.iter().zip(nb.iter()) {
                if n1 != n2 {
                    return None;
                }
                nomeados.push((*n1, top_merge_rec(table, core, *x, *y, prof + 1)?));
            }
            Some(table.intern(Type::Record { positional: pos.into_boxed_slice(), named: nomeados.into_boxed_slice(), nullable: false }))
        }
        (Type::TypeParameter { param: a, .. }, Type::TypeParameter { param: b, .. }) if a == b => Some(t),
        (Type::Null, Type::Null) => Some(t),
        _ => None,
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

/// O tipo sem nenhuma decoração de exibição (alias, `Never?`), em toda a
/// estrutura: a identidade semântica, para comparar `TypeId`s
/// (`a as N` com `typedef N = num` e `a: num` é o mesmo tipo).
pub fn sem_exibicao(ty: TypeId, table: &mut TypeTable) -> TypeId {
    let c = table.canonico(ty);
    if table.exibicao_vazia() {
        return c;
    }
    let t = table.get(c).clone();
    let f = |x: TypeId, table: &mut TypeTable| sem_exibicao(x, table);
    let novo = match t {
        Type::Interface { class, args, nullable } => {
            Type::Interface { class, args: args.iter().map(|&a| f(a, table)).collect(), nullable }
        }
        Type::ExtensionType { decl, args, nullable } => {
            Type::ExtensionType { decl, args: args.iter().map(|&a| f(a, table)).collect(), nullable }
        }
        Type::FutureOr { arg, nullable } => Type::FutureOr { arg: f(arg, table), nullable },
        Type::Record { positional, named, nullable } => Type::Record {
            positional: positional.iter().map(|&a| f(a, table)).collect(),
            named: named.iter().map(|&(n, a)| (n, f(a, table))).collect(),
            nullable,
        },
        Type::Function { type_params, ret, positional, optional, named, nullable } => Type::Function {
            type_params,
            ret: f(ret, table),
            positional: positional.iter().map(|&a| f(a, table)).collect(),
            optional: optional.iter().map(|&a| f(a, table)).collect(),
            named: named.iter().map(|&(n, a, r)| (n, f(a, table), r)).collect(),
            nullable,
        },
        Type::Intersection { param, bound } => Type::Intersection { param, bound: f(bound, table) },
        _ => return c,
    };
    table.intern(novo)
}
