//! Optional parameters that no call in the library fills in
//! (`UNUSED_ELEMENT_PARAMETER`, code `unused_element` in 3.6): the parameter
//! part of analyzer 6.11's `UnusedLocalElementsVerifier`
//! (`an611:src/error/unused_local_elements_verifier.dart`):
//!
//! * `visitFormalParameterList` (`:600-609`) reports each parameter that is
//!   not used;
//! * `_isUsedElement` (`:860-923`): only **optional** parameters of
//!   constructors, functions and methods; excluding those of constructors of
//!   generic classes and of generic executables; excluding those of
//!   executables accessible from outside the library
//!   (`_isPubliclyAccessible`, `:799-823`); excluding the one that
//!   corresponds to a required parameter of the superclass constructor or of
//!   an overridden member, or to a used parameter of the overridden one
//!   (`_overridesUsedParameter`, `:970-992`);
//! * "used" means receiving an argument in some call in the library
//!   (`_addParametersForArguments`, `:360-365`: the `staticParameterElement`
//!   of each argument), or being the target of a `super.x` (`:116-118`).
//!
//! The argument → parameter binding comes from the resolutions that body
//! inference stores (`crate::resolved`).

use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, MemberRef, Resolved};
use dartforge_diagnostics::codes::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{
    ClassId, ClassKind, Element, FunctionElementId, FunctionKind, FunctionRef, LibraryId, Program, UnitId,
};
use dartforge_frontend::ast::{self, DeclKind, ExprKind, MemberKind, ParameterKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashSet;

/// The written formal parameters of `f` (`None` without a node).
fn params_list<'p>(program: &'p Program, f: FunctionElementId) -> Option<(UnitId, &'p [ast::Parameter])> {
    match program.function(f).node {
        FunctionRef::Function { unit, function } => {
            program.unit(unit).ast.function(function).parameters.as_deref().map(|p| (unit, p))
        }
        FunctionRef::Constructor { unit, member } => match &program.unit(unit).ast.member(member).kind {
            MemberKind::Constructor(k) => Some((unit, &k.parameters[..])),
            _ => None,
        },
        FunctionRef::None => None,
    }
}

fn external_name(p: &ast::Parameter) -> Option<SymbolId> {
    p.public_name.or(p.name).map(|n| n.sym)
}

/// The parameter of `ps` that receives argument `i` of `args`.
fn argument_target(ps: &[ast::Parameter], args: &ast::Arguments, i: usize) -> Option<usize> {
    let a = &args.args[i];
    match a.name {
        Some(n) => ps.iter().position(|p| p.kind == ParameterKind::Named && external_name(p) == Some(n.sym)),
        None => {
            let pos = args.args[..i].iter().filter(|x| x.name.is_none()).count();
            ps.iter().enumerate().filter(|(_, p)| p.kind != ParameterKind::Named).nth(pos).map(|(j, _)| j)
        }
    }
}

/// `_getCorrespondingParameter`: by name (named) or by position in the
/// list.
fn corresponding(ps: &[ast::Parameter], j: usize, others: &[ast::Parameter]) -> Option<usize> {
    let p = &ps[j];
    if p.kind == ParameterKind::Named {
        others.iter().position(|o| o.kind == ParameterKind::Named && external_name(o) == external_name(p))
    } else if j < others.len() {
        Some(j)
    } else {
        None
    }
}

fn the_required(p: &ast::Parameter) -> bool {
    p.kind == ParameterKind::Required || (p.kind == ParameterKind::Named && p.required)
}

fn resolved_function(r: Option<&Resolved>) -> Option<FunctionElementId> {
    match r? {
        Resolved::Element(Element::Function(f)) => Some(*f),
        Resolved::Member { member: MemberRef::Function(f), .. } => Some(*f),
        Resolved::ExtensionMember { member, .. } => Some(*member),
        Resolved::Constructor(f) => Some(*f),
        _ => None,
    }
}

/// The constructor `the_name` (or the unnamed one) of class `c`.
fn the_constructor(program: &Program, interner: &Interner, c: ClassId, the_name: Option<ast::Name>) -> Option<FunctionElementId> {
    let s = match the_name {
        Some(n) => n.sym,
        None => interner.lookup("")?,
    };
    program.class(c).constructors.get(&s).copied()
}

/// The superclass constructor that `k` (of class `c`) invokes, explicit
/// (`super.the_name(...)`) or implicit (the unnamed one).
fn superclass_constructor(program: &Program, interner: &Interner, c: ClassId, k: &ast::Constructor) -> Option<FunctionElementId> {
    if k.factory || k.initializers.iter().any(|i| matches!(i, ast::Initializer::Redirect { .. })) {
        return None;
    }
    let mut sup = program.class(c).supertype_class?;
    // Synthetic mixin applications forward the superclass constructors.
    let mut guard = 0;
    while program.class(sup).decl.is_none() && guard < 50 {
        sup = program.class(sup).supertype_class?;
        guard += 1;
    }
    let the_name = k.initializers.iter().find_map(|i| match i {
        ast::Initializer::Super { constructor, .. } => Some(*constructor),
        _ => None,
    });
    the_constructor(program, interner, sup, the_name.flatten())
}

/// The diagnostics of `lib`.
pub fn parametros_nao_usados(
    program: &Program,
    interner: &Interner,
    outline: &OutlineTypes,
    body: &BodyTypes,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut used: HashSet<(FunctionElementId, usize)> = HashSet::new();
    let mark = |used: &mut HashSet<(FunctionElementId, usize)>, f: FunctionElementId, args: &ast::Arguments| {
        if let Some((_, ps)) = params_list(program, f) {
            for i in 0..args.args.len() {
                if let Some(j) = argument_target(ps, args, i) {
                    used.insert((f, j));
                }
            }
        }
    };
    // Executables whose parameters all count as used: a reference that is not
    // called (tear-off; `visitSimpleIdentifier`, `:290-305`), and the name of
    // `this.the_name(...)`/`super.the_name(...)` (the identifier is not in a
    // `MethodInvocation`).
    let mut all: HashSet<FunctionElementId> = HashSet::new();
    // Names read by identifiers, per unit (`_useIdentifierElement`: reading
    // the parameter uses it), with the offset.
    let mut reads: Vec<(UnitId, SymbolId, usize)> = Vec::new();
    let mut shorthands: HashSet<SymbolId> = HashSet::new();
    let units = program.library(lib).units.clone();
    for &u in &units {
        let a = &program.unit(u).ast;
        let Some(b) = body.units.get(u.0 as usize) else { continue };
        // Call targets (the called name and the method's `Property`).
        let mut targets: HashSet<ast::ExprId> = HashSet::new();
        for e in a.exprs.iter() {
            if let ExprKind::Call { target, .. } = &e.kind {
                targets.insert(*target);
                if let ExprKind::Property { .. } = &a.expr(*target).kind {
                    targets.insert(*target);
                }
            }
        }
        for (i, e) in a.exprs.iter().enumerate() {
            let id = ast::ExprId(i as u32);
            match &e.kind {
                ExprKind::Call { target, arguments } => {
                    let f = resolved_function(b.get_resolved(id)).or_else(|| resolved_function(b.get_resolved(*target)));
                    if let Some(f) = f {
                        mark(&mut used, f, arguments);
                    }
                }
                ExprKind::InstanceCreation { arguments, .. } => {
                    if let Some(f) = resolved_function(b.get_resolved(id)) {
                        mark(&mut used, f, arguments);
                    }
                }
                ExprKind::Identifier(n) => {
                    reads.push((u, n.sym, n.span.start));
                    if !targets.contains(&id) {
                        if let Some(f) = resolved_function(b.get_resolved(id)) {
                            all.insert(f);
                        }
                    }
                }
                ExprKind::Property { name, .. } => {
                    if !targets.contains(&id) {
                        if let Some(f) = resolved_function(b.get_resolved(id)) {
                            all.insert(f);
                        }
                        // `A._named` (constructor tear-off): by name.
                        reads.push((u, name.sym, usize::MAX));
                    }
                }
                ExprKind::DotShorthand { name, .. } => {
                    shorthands.insert(name.sym);
                }
                _ => {}
            }
        }
    }
    // Annotation arguments (`@A(valor: 1)`): the label and the position use
    // the constructor parameter.
    for &u in &units {
        let a = &program.unit(u).ast;
        let mut annotations: Vec<&ast::Annotation> = Vec::new();
        for d in a.decls.iter() {
            annotations.extend(d.metadata.iter());
        }
        for m in a.members.iter() {
            annotations.extend(m.metadata.iter());
        }
        for an in annotations {
            let Some(args) = &an.arguments else { continue };
            let (class, ctor) = match &an.name[..] {
                [c] => (program.lookup_na_unidade(u, c.sym), None),
                [c, n] => match program.lookup_na_unidade(u, c.sym) {
                    Some(b) if matches!(b.getter, Some(Element::Class(_))) => (Some(b), Some(*n)),
                    _ => (program.lookup_prefixed_na_unidade(u, c.sym, n.sym), None),
                },
                [p, c, n] => (program.lookup_prefixed_na_unidade(u, p.sym, c.sym), Some(*n)),
                _ => (None, None),
            };
            if let Some(Element::Class(c)) = class.and_then(|b| b.getter) {
                if let Some(f) = the_constructor(program, interner, c, ctor) {
                    mark(&mut used, f, args);
                }
            }
        }
    }
    // Initializers (`super(...)`, `this(...)`), `super.x` and enum constants.
    for (ci, k) in program.classes.iter().enumerate() {
        let c = ClassId(ci as u32);
        if k.library != lib {
            continue;
        }
        for (_, &f) in k.constructors.iter() {
            let FunctionRef::Constructor { unit, member } = program.function(f).node else { continue };
            let MemberKind::Constructor(kk) = &program.unit(unit).ast.member(member).kind else { continue };
            for start in kk.initializers.iter() {
                match start {
                    ast::Initializer::Super { constructor: n, arguments, .. } => {
                        if let Some(s) = superclass_constructor(program, interner, c, kk) {
                            mark(&mut used, s, arguments);
                            if n.is_some() {
                                all.insert(s);
                            }
                        }
                    }
                    ast::Initializer::Redirect { constructor: n, arguments, .. } => {
                        if let Some(s) = the_constructor(program, interner, c, *n) {
                            mark(&mut used, s, arguments);
                            if n.is_some() {
                                all.insert(s);
                            }
                        }
                    }
                    _ => {}
                }
            }
            // Redirecting factory (`factory C() = D._;`): the target's corresponding
            // parameters count as used
            // (`visitConstructorDeclaration`, `_matchParameters`, `:93-110`).
            if let Some(rd) = &kk.redirect {
                let ast_u = &program.unit(unit).ast;
                if let ast::TypeKind::Named { name, .. } = &ast_u.ty(rd.ty).kind {
                    let tgt = match (&name[..], rd.constructor) {
                        ([cn], ctor) => (program.lookup_na_unidade(unit, cn.sym), ctor),
                        ([a, b], None) => match program.lookup_na_unidade(unit, a.sym) {
                            Some(x) if matches!(x.getter, Some(Element::Class(_))) => (Some(x), Some(*b)),
                            _ => (program.lookup_prefixed_na_unidade(unit, a.sym, b.sym), None),
                        },
                        ([p, cn], ctor) => (program.lookup_prefixed_na_unidade(unit, p.sym, cn.sym), ctor),
                        ([p, cn, n], None) => (program.lookup_prefixed_na_unidade(unit, p.sym, cn.sym), Some(*n)),
                        _ => (None, None),
                    };
                    if let (Some(Element::Class(dc)), ctor) = (tgt.0.and_then(|b| b.getter), tgt.1) {
                        if let Some(t) = the_constructor(program, interner, dc, ctor) {
                            if let Some((_, tps)) = params_list(program, t) {
                                for (j, _) in kk.parameters.iter().enumerate() {
                                    if let Some(k2) = corresponding(&kk.parameters, j, tps) {
                                        used.insert((t, k2));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if kk.parameters.iter().any(|p| p.super_) {
                if let Some(s) = superclass_constructor(program, interner, c, kk) {
                    if let Some((_, sps)) = params_list(program, s) {
                        let mut pos = 0usize;
                        for p in kk.parameters.iter() {
                            if p.kind != ParameterKind::Named && p.super_ {
                                if let Some((j, _)) = sps.iter().enumerate().filter(|(_, x)| x.kind != ParameterKind::Named).nth(pos) {
                                    used.insert((s, j));
                                }
                            } else if p.super_ {
                                if let Some(j) = sps.iter().position(|x| x.kind == ParameterKind::Named && external_name(x) == external_name(p)) {
                                    used.insert((s, j));
                                }
                            }
                            if p.kind != ParameterKind::Named && p.super_ {
                                pos += 1;
                            }
                        }
                    }
                }
            }
        }
        if k.kind == ClassKind::Enum {
            if let Some(d) = k.decl {
                if let DeclKind::Enum(en) = &program.unit(d.unit).ast.decl(d.decl).kind {
                    for constant in en.constants.iter() {
                        if let (Some(args), Some(f)) = (&constant.arguments, the_constructor(program, interner, c, constant.constructor)) {
                            mark(&mut used, f, args);
                        }
                    }
                }
            }
        }
    }

    let mut output = Vec::new();
    for (fi, fe) in program.functions.iter().enumerate() {
        let f = FunctionElementId(fi as u32);
        if fe.library != lib {
            continue;
        }
        let constructor_ = fe.kind == FunctionKind::Constructor;
        if !(constructor_ || fe.kind == FunctionKind::Function || fe.kind == FunctionKind::Operator) {
            continue;
        }
        let Some((unit, ps)) = params_list(program, f) else { continue };
        if all.contains(&f) || shorthands.contains(&fe.name) {
            continue;
        }
        // The name of the named constructor read as a property (tear-off).
        if constructor_ && reads.iter().any(|&(_, n, o)| o == usize::MAX && n == fe.name) {
            continue;
        }
        let region = executable_region(program, f);
        if !ps.iter().any(|p| p.kind != ParameterKind::Required && !p.required) {
            continue;
        }
        // Generic executable, or constructor of a generic class.
        if !outline.functions[fi].type_params.is_empty() {
            continue;
        }
        if constructor_ && fe.class.is_some_and(|c| !program.class(c).type_params.is_empty()) {
            continue;
        }
        if publicly_accessible(program, interner, f) {
            continue;
        }
        let superior = match (constructor_, fe.class, program.function(f).node) {
            (true, Some(c), FunctionRef::Constructor { unit: uu, member }) => match &program.unit(uu).ast.member(member).kind {
                MemberKind::Constructor(kk) => superclass_constructor(program, interner, c, kk),
                _ => None,
            },
            _ => None,
        };
        let overridden = if !constructor_ && !fe.static_ { the_overridden(program, outline, f) } else { Vec::new() };
        for (j, p) in ps.iter().enumerate() {
            if the_required(p) || p.kind == ParameterKind::Required {
                continue;
            }
            let Some(the_name) = p.name else { continue };
            if p.metadata.iter().any(|m| m.name.iter().any(|n| interner.resolve(n.sym) == "pragma")) {
                continue;
            }
            if used.contains(&(f, j)) {
                continue;
            }
            // Read in the body or in the initializers.
            if let Some((ru, start, end)) = region {
                if reads.iter().any(|&(u, n, o)| u == ru && n == the_name.sym && o >= start && o < end && o != the_name.span.start) {
                    continue;
                }
            }
            if let Some(s) = superior {
                if let Some((_, sps)) = params_list(program, s) {
                    if corresponding(ps, j, sps).is_some_and(|k| the_required(&sps[k])) {
                        continue;
                    }
                }
            }
            let mut the_used = false;
            for &o in &overridden {
                if let Some((_, ops)) = params_list(program, o) {
                    if let Some(k) = corresponding(ps, j, ops) {
                        if the_required(&ops[k]) || used.contains(&(o, k)) {
                            the_used = true;
                            break;
                        }
                    }
                }
            }
            if the_used {
                continue;
            }
            let text = interner.resolve(the_name.sym).to_string();
            output.push((
                unit,
                Diagnostic::with_code(w::UNUSED_ELEMENT_PARAMETER, Span { start: the_name.span.start, end: the_name.span.end }, [text]),
            ));
        }
    }
    output
}

/// The range of the executable's declaration (where reads of the
/// parameters count).
fn executable_region(program: &Program, f: FunctionElementId) -> Option<(UnitId, usize, usize)> {
    match program.function(f).node {
        FunctionRef::Function { unit, function } => {
            let s = program.unit(unit).ast.function(function).span;
            Some((unit, s.start, s.end))
        }
        FunctionRef::Constructor { unit, member } => {
            let s = program.unit(unit).ast.member(member).span;
            Some((unit, s.start, s.end))
        }
        FunctionRef::None => None,
    }
}

/// `_isPubliclyAccessible`.
fn publicly_accessible(program: &Program, interner: &Interner, f: FunctionElementId) -> bool {
    let fe = program.function(f);
    let the_name = interner.resolve(fe.name);
    if the_name.starts_with('_') {
        return false;
    }
    if let Some(c) = fe.class {
        let k = program.class(c);
        if k.kind == ClassKind::Enum && fe.kind == FunctionKind::Constructor && !fe.factory {
            return false;
        }
        if interner.resolve(k.name).starts_with('_') && (fe.static_ || fe.kind == FunctionKind::Constructor) {
            return false;
        }
    }
    if let Some(x) = fe.extension {
        return program.extension(x).name.is_some_and(|n| !interner.resolve(n).starts_with('_'));
    }
    true
}

/// The members that the instance method `f` overrides (`getOverridden2`):
/// those with the same name in the supertypes.
fn the_overridden(program: &Program, outline: &OutlineTypes, f: FunctionElementId) -> Vec<FunctionElementId> {
    let fe = program.function(f);
    let Some(c) = fe.class else { return Vec::new() };
    let mut r = Vec::new();
    let Some(h) = outline.hierarchy.get(c) else { return r };
    for (&s, _) in h.supertypes.iter() {
        if s == c {
            continue;
        }
        if let Some(&o) = program.class(s).instance_members.get(&fe.name) {
            if o != f && !r.contains(&o) {
                r.push(o);
            }
        }
    }
    r
}
