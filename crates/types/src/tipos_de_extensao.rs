//! `extension type` declarations (docs/ANALYZER-ESPECIFICACAO.md §A, group
//! 4):
//!
//! * the link cycles (`summary2/extension_type.dart`): a representation that
//!   depends on itself (`extension_type_representation_depends_on_itself`, at
//!   the name of each type in the component) and a cyclic `implements`
//!   (`extension_type_implements_itself`), through `DependencyWalker`
//!   (Tarjan, with the self-loop);
//! * `_checkForExtensionTypeRepresentationTypeBottom`
//!   (`error_verifier.dart:3503-3513`), at the representation type;
//! * `_checkForExtensionTypeMemberConflicts` (`:3448-3485`): one report per
//!   interface conflict (candidates, class member and extension type member,
//!   non-unique extension type member), at the name;
//! * `ResolutionVisitor._verifyExtensionElementImplements`
//!   (`resolution_visitor.dart:1786-1838`), at each type of the `implements`:
//!   `extension_type_implements_disallowed_type`,
//!   `extension_type_implements_representation_not_supertype` and
//!   `extension_type_implements_not_supertype`.
//!
//! Written without compiling or running (2026-10-05).

use crate::heranca::{Conflict, Heranca, ProvedorDoOutline};
use crate::resolve::OutlineTypes;
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::codes::compile_time_error as c;
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{ClassId, ClassKind, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind};
use dartforge_intern::Interner;
use std::collections::{HashMap, HashSet};

/// The extension types cited in a type (`_DependenciesCollector`: the type
/// arguments, the return, the bounds of the type parameters and the
/// parameters of a function, the fields of a record; not the bound of a
/// type parameter).
fn cited(table: &TypeTable, t: TypeId, output: &mut Vec<ClassId>, prof: u32) {
    if prof > 64 {
        return;
    }
    match table.get(t).clone() {
        Type::ExtensionType { decl, args, .. } => {
            output.push(decl);
            for a in args.iter() {
                cited(table, *a, output, prof + 1);
            }
        }
        Type::Interface { args, .. } => {
            for a in args.iter() {
                cited(table, *a, output, prof + 1);
            }
        }
        Type::FutureOr { arg, .. } => cited(table, arg, output, prof + 1),
        Type::Function { type_params, ret, positional, optional, named, .. } => {
            cited(table, ret, output, prof + 1);
            for p in type_params.iter() {
                let b = table.param(*p).bound;
                if table.param(*p).explicito {
                    cited(table, b, output, prof + 1);
                }
            }
            for x in positional.iter().chain(optional.iter()) {
                cited(table, *x, output, prof + 1);
            }
            for (_, x, _) in named.iter() {
                cited(table, *x, output, prof + 1);
            }
        }
        Type::Record { positional, named, .. } => {
            for x in positional.iter() {
                cited(table, *x, output, prof + 1);
            }
            for (_, x) in named.iter() {
                cited(table, *x, output, prof + 1);
            }
        }
        _ => {}
    }
}

/// The nodes of strongly connected components with more than one node or
/// with a self-loop (`DependencyWalker.evaluateScc`).
fn in_cycle(nodes: &[ClassId], deps: &HashMap<ClassId, Vec<ClassId>>) -> HashSet<ClassId> {
    struct State<'a> {
        deps: &'a HashMap<ClassId, Vec<ClassId>>,
        index: HashMap<ClassId, usize>,
        low: HashMap<ClassId, usize>,
        stack: Vec<ClassId>,
        on_stack: HashSet<ClassId>,
        next: usize,
        marked: HashSet<ClassId>,
    }
    fn connect(e: &mut State<'_>, v: ClassId) {
        e.index.insert(v, e.next);
        e.low.insert(v, e.next);
        e.next += 1;
        e.stack.push(v);
        e.on_stack.insert(v);
        let mut auto = false;
        let neighbors = e.deps.get(&v).cloned().unwrap_or_default();
        for w in neighbors {
            if !e.deps.contains_key(&w) {
                continue;
            }
            if w == v {
                auto = true;
            } else if !e.index.contains_key(&w) {
                connect(e, w);
                let bw = e.low[&w];
                if bw < e.low[&v] {
                    e.low.insert(v, bw);
                }
            } else if e.on_stack.contains(&w) {
                let iw = e.index[&w];
                if iw < e.low[&v] {
                    e.low.insert(v, iw);
                }
            }
        }
        if e.low[&v] == e.index[&v] {
            let mut component = Vec::new();
            while let Some(x) = e.stack.pop() {
                e.on_stack.remove(&x);
                component.push(x);
                if x == v {
                    break;
                }
            }
            if component.len() > 1 || auto {
                e.marked.extend(component);
            }
        }
    }
    let mut e = State { deps, index: HashMap::new(), low: HashMap::new(), stack: Vec::new(), on_stack: HashSet::new(), next: 1, marked: HashSet::new() };
    for &v in nodes {
        if !e.index.contains_key(&v) {
            connect(&mut e, v);
        }
    }
    e.marked
}

/// `isBottom`: `Never`, or a non-nullable type parameter whose bound is
/// bottom.
fn is_bottom(table: &TypeTable, t: TypeId, prof: u32) -> bool {
    if prof > 16 {
        return false;
    }
    match table.get(t) {
        Type::Never => true,
        Type::TypeParameter { param, nullable: false } => is_bottom(table, table.param(*param).bound, prof + 1),
        Type::Intersection { bound, .. } => is_bottom(table, *bound, prof + 1),
        _ => false,
    }
}

/// The written type is literally `dynamic` (and not a name that did not
/// resolve, which the outline also leaves as `dynamic`).
fn written_dynamic(interner: &Interner, a: &ast::Ast, t: ast::TypeId) -> bool {
    matches!(&a.ty(t).kind, ast::TypeKind::Named { name, .. } if name.len() == 1 && interner.resolve(name[0].sym) == "dynamic")
}

/// The type of the declared representation (`InvalidType` becomes `None`).
fn the_representation(program: &Program, outline: &OutlineTypes, table: &TypeTable, interner: &Interner, c: ClassId) -> Option<TypeId> {
    let v = program.class(c).representation?;
    let t = outline.variables[v.0 as usize].declared_type?;
    if table.e_invalido(t) {
        return None;
    }
    if matches!(table.get(t), Type::Dynamic) {
        let d = program.class(c).decl?;
        let a = &program.unit(d.unit).ast;
        let DeclKind::ExtensionType(et) = &a.decl(d.decl).kind else { return None };
        if !written_dynamic(interner, a, et.representation_type) {
            return None;
        }
    }
    Some(t)
}

pub fn verificar(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut output = Vec::new();
    let types: Vec<ClassId> = program
        .classes
        .iter()
        .enumerate()
        .filter(|(_, ce)| ce.kind == ClassKind::ExtensionType && ce.decl.is_some_and(|d| !program.unit(d.unit).ast.decl(d.decl).augment))
        .map(|(i, _)| ClassId(i as u32))
        .collect();
    if !types.iter().any(|&t| program.class(t).library == lib) {
        return output;
    }
    // The representation graph and the `implements` graph (only non-nullable
    // extension types: the others leave the element's `interfaces`).
    let mut deps_rep: HashMap<ClassId, Vec<ClassId>> = HashMap::new();
    let mut deps_impl: HashMap<ClassId, Vec<ClassId>> = HashMap::new();
    for &t in &types {
        let mut v = Vec::new();
        if let Some(rep) = program.class(t).representation
            && let Some(r) = outline.variables[rep.0 as usize].declared_type
        {
            cited(table, r, &mut v, 0);
        }
        deps_rep.insert(t, v);
        let interfaces = outline.classes[t.0 as usize].interfaces.clone();
        let mut w = Vec::new();
        for i in interfaces.iter() {
            if let Type::ExtensionType { decl, nullable: false, .. } = table.get(*i) {
                w.push(*decl);
            }
        }
        deps_impl.insert(t, w);
    }
    let rep_in_cycle = in_cycle(&types, &deps_rep);
    let impl_in_cycle = in_cycle(&types, &deps_impl);
    let empty: [&str; 0] = [];
    let mut inheritance = Heranca::default();
    for &t in types.iter().filter(|&&t| program.class(t).library == lib) {
        let ce = program.class(t);
        let Some(d) = ce.decl else { continue };
        let a = &program.unit(d.unit).ast;
        let DeclKind::ExtensionType(et) = &a.decl(d.decl).kind else { continue };
        let u = d.unit;
        let the_name = interner.resolve(et.name.sym).to_string();
        let rep = if rep_in_cycle.contains(&t) { None } else { the_representation(program, outline, table, interner, t) };

        // `ResolutionVisitor` (before `ErrorVerifier`): the `implements`.
        if let Some(r) = rep {
            let interfaces = outline.classes[t.0 as usize].interfaces.clone();
            for (k, &written) in et.implements.iter().enumerate() {
                let Some(&ty) = interfaces.get(k) else { continue };
                // `_verifyNullability` (`named_type_resolver.dart:388-414`): the
                // hierarchy type loses the `?` after
                // `NULLABLE_TYPE_IN_IMPLEMENTS_CLAUSE`. `hasErrorReported` only
                // applies to an alias that expands to a type parameter: an undefined
                // name is `InvalidType`, and `isValidExtensionTypeSuperinterface`
                // (`type_system.dart:1361-1378`) rejects it.
                let invalid = table.e_invalido(ty);
                let ty = if invalid { ty } else { crate::ops::non_nullable(ty, table) };
                let sp = a.ty(written).span;
                let valid = !invalid
                    && match table.get(ty).clone() {
                        Type::Interface { class, nullable: false, .. } => {
                            Some(class) != core.function_class && Some(class) != core.null_class && Some(class) != core.record_class
                        }
                        Type::ExtensionType { nullable: false, .. } => true,
                        _ => false,
                    };
                if !valid {
                    // The type as the analyzer writes it: with the alias it was
                    // written with (`implements X` with `typedef X = void Function()`
                    // gives `X`, and `X?` too), except `void` and `dynamic` (which
                    // take no alias), and `InvalidType` for what did not resolve.
                    let text = if invalid {
                        "InvalidType".to_string()
                    } else if matches!(table.get(ty), Type::Dynamic | Type::Void) {
                        let s = crate::ops::without_display(ty, table);
                        table.format(s, interner, program)
                    } else {
                        table.format(ty, interner, program)
                    };
                    output.push((u, Diagnostic::with_code(c::EXTENSION_TYPE_IMPLEMENTS_DISALLOWED_TYPE, sp, [text.as_str()])));
                    continue;
                }
                let fits = {
                    let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
                    is_subtype(r, ty, &mut env)
                };
                if fits {
                    continue;
                }
                if let Type::ExtensionType { decl, args, .. } = table.get(ty).clone() {
                    let Some(rep_impl) = the_representation(program, outline, table, interner, decl) else { continue };
                    let params = outline.classes[decl.0 as usize].type_params.clone();
                    let rep_impl = if params.len() == args.len() {
                        let map: HashMap<_, _> = params.iter().copied().zip(args.iter().copied()).collect();
                        crate::ops::substitute(rep_impl, &map, table)
                    } else {
                        rep_impl
                    };
                    let fits_in_rep = {
                        let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
                        is_subtype(r, rep_impl, &mut env)
                    };
                    if !fits_in_rep {
                        let ri = table.format(rep_impl, interner, program);
                        let ni = interner.resolve(program.class(decl).name).to_string();
                        let rd = table.format(r, interner, program);
                        output.push((
                            u,
                            Diagnostic::with_code(
                                c::EXTENSION_TYPE_IMPLEMENTS_REPRESENTATION_NOT_SUPERTYPE,
                                sp,
                                [ri.as_str(), ni.as_str(), rd.as_str(), the_name.as_str()],
                            ),
                        ));
                    }
                    continue;
                }
                let ti = table.format(ty, interner, program);
                let rd = table.format(r, interner, program);
                output.push((u, Diagnostic::with_code(c::EXTENSION_TYPE_IMPLEMENTS_NOT_SUPERTYPE, sp, [ti.as_str(), rd.as_str()])));
            }
        }

        // `ErrorVerifier.visitExtensionTypeDeclaration`, in order.
        if rep_in_cycle.contains(&t) {
            output.push((u, Diagnostic::with_code(c::EXTENSION_TYPE_REPRESENTATION_DEPENDS_ON_ITSELF, et.name.span, empty.iter().copied())));
        } else if let Some(r) = rep
            && is_bottom(table, r, 0)
        {
            let sp = a.ty(et.representation_type).span;
            output.push((u, Diagnostic::with_code(c::EXTENSION_TYPE_REPRESENTATION_TYPE_BOTTOM, sp, empty.iter().copied())));
        }
        if impl_in_cycle.contains(&t) {
            output.push((u, Diagnostic::with_code(c::EXTENSION_TYPE_IMPLEMENTS_ITSELF, et.name.span, empty.iter().copied())));
        }
        let interface = {
            let mut p = ProvedorDoOutline { program, interner, core, outline, table: &mut *table };
            inheritance.interface(&mut p, t)
        };
        for conflict in interface.conflicts.iter() {
            // `_checkForExtensionTypeMemberConflicts`: the candidates (the
            // non-extension ones before the extension ones) become the context
            // messages.
            let (n, candidates): (_, Vec<&crate::heranca::Membro>) = match conflict {
                Conflict::Candidates { nome: the_name, candidatos: candidates } | Conflict::NonUniqueExtension { nome: the_name, candidatos: candidates } => (the_name, candidates.iter().collect()),
                Conflict::ExtensionAndNonExtension { nome: the_name, non_extension, extensao: extension } => (the_name, non_extension.iter().chain(extension.iter()).collect()),
                Conflict::GetterMethod { .. } => continue,
            };
            // The setter's `Name.name` is `x=`; the model key, `x_=`.
            let text = interner.resolve(n.key);
            let member = text.strip_suffix("_=").map_or_else(|| text.to_string(), |s| format!("{s}="));
            let mut d = Diagnostic::with_code(c::EXTENSION_TYPE_INHERITED_MEMBER_CONFLICT, et.name.span, [the_name.as_str(), member.as_str()]);
            for m in candidates {
                let Some((mu, span)) = program.non_synthetic_function_name(m.funcao) else { continue };
                let file = (mu != u).then(|| program.unit_path(mu).into());
                let message = format!("Inherited from '{}'", interner.resolve(program.class(m.class).name));
                d.context.push(dartforge_diagnostics::ContextMessage { file: file, span, message: message.into() });
            }
            output.push((u, d));
        }
    }
    output
}
