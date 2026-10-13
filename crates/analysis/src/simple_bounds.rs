//! Type parameter bounds (docs/ANALYZER-ESPECIFICACAO.md, §A):
//!
//! * `computeSimplyBounded` (`analyzer/lib/src/summary2/simply_bounded.dart`):
//!   a class, mixin, enum, extension type or alias is *simply bounded* when
//!   the bounds of its type parameters (and, in an alias, the right-hand side
//!   types) cite no type parameter (in an alias, the right-hand side may) nor
//!   a raw generic type that is not simply bounded; a cycle of raw types
//!   (`class A<T extends A>`) is not;
//! * `NOT_INSTANTIATED_BOUND` (`_UninstantiatedBoundChecker`,
//!   `error_verifier.dart:7167-7190`): in the bound of any type parameter,
//!   each raw named type whose element is not simply bounded;
//! * `TYPE_PARAMETER_SUPERTYPE_OF_ITS_BOUND`
//!   (`_checkForTypeParameterBoundRecursion`, `:5382-5416`): following the
//!   bounds that are parameters of the same list (erasing extension types),
//!   `parameters.length` steps close a cycle.
//!
//! Written without compiling or running (2026-10-05).

use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{ClassId, ClassKind, Element, LibraryId, Program, TypedefId, UnitId};
use dartforge_frontend::ast::{self, DeclKind, Parameter, TypeKind, TypeParameter, TypedefKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Node {
    Class(ClassId),
    Alias(TypedefId),
}

/// The declarations that are not simply bounded.
#[derive(Default)]
pub struct SimpleBounds {
    not_simple: HashSet<Node>,
}

/// `_TypeCollector`: the types of a function type (return, bounds of the type
/// parameters, parameters).
fn collect_from_params(ps: &[Parameter], out: &mut Vec<ast::TypeId>) {
    for p in ps {
        if p.this_ {
            continue;
        }
        if let Some(t) = p.ty {
            out.push(t);
        }
        if let Some(inner) = &p.function_parameters {
            collect_from_params(inner, out);
        }
    }
}

fn collect_from_function(ret: Option<ast::TypeId>, tps: &[TypeParameter], ps: &[Parameter], out: &mut Vec<ast::TypeId>) {
    if let Some(r) = ret {
        out.push(r);
    }
    for tp in tps {
        if let Some(b) = tp.bound {
            out.push(b);
        }
    }
    collect_from_params(ps, out);
}

/// The type element of a name (`p.Name` or `Name`) in the unit's scope.
fn element(program: &Program, u: UnitId, name: &[ast::Name]) -> Option<Element> {
    let b = match name {
        [n] => program.lookup_na_unidade(u, n.sym),
        [p, n] => program.lookup_prefixed_na_unidade(u, p.sym, n.sym),
        _ => None,
    }?;
    b.getter
}

fn node_of(program: &Program, el: Element) -> Option<Node> {
    match el {
        Element::Class(c) if program.class(c).decl.is_some() => Some(Node::Class(c)),
        Element::Typedef(t) => Some(Node::Alias(t)),
        _ => None,
    }
}

/// `SimplyBoundedNode._visitType`: `false` decides (not simply bounded); the
/// raw types of declarations become dependencies.
fn visit(program: &Program, u: UnitId, a: &ast::Ast, t: ast::TypeId, allows_params: bool, scope: &[SymbolId], deps: &mut Vec<Node>) -> bool {
    match &a.ty(t).kind {
        TypeKind::Named { name, args } => {
            if let [n] = &name[..]
                && scope.contains(&n.sym)
            {
                return allows_params;
            }
            if !args.is_empty() {
                return args.iter().all(|&x| visit(program, u, a, x, allows_params, scope, deps));
            }
            if let Some(node) = element(program, u, name).and_then(|el| node_of(program, el)) {
                deps.push(node);
            }
            true
        }
        TypeKind::Void => true,
        TypeKind::Function { return_type, type_params, parameters } => {
            let mut escopo2 = scope.to_vec();
            escopo2.extend(type_params.iter().map(|p| p.name.sym));
            let mut types = Vec::new();
            collect_from_function(*return_type, type_params, parameters, &mut types);
            types.into_iter().all(|x| visit(program, u, a, x, allows_params, &escopo2, deps))
        }
        TypeKind::Record { positional, named } => {
            positional.iter().chain(named.iter().map(|(_, x)| x)).all(|&x| visit(program, u, a, x, allows_params, scope, deps))
        }
    }
}

/// The type parameters and the right-hand side types of a node.
fn node_parts<'a>(program: &'a Program, node: Node) -> Option<(UnitId, &'a [TypeParameter], Vec<ast::TypeId>)> {
    let d = match node {
        Node::Class(c) => program.class(c).decl?,
        Node::Alias(t) => program.typedef(t).decl,
    };
    let a = &program.unit(d.unit).ast;
    let decl = a.decl(d.decl);
    let (tps, rhs): (&'a [TypeParameter], Vec<ast::TypeId>) = match &decl.kind {
        DeclKind::Class(x) => (&x.type_params, Vec::new()),
        DeclKind::Mixin(x) => (&x.type_params, Vec::new()),
        DeclKind::Enum(x) => (&x.type_params, Vec::new()),
        DeclKind::ExtensionType(x) => (&x.type_params, Vec::new()),
        DeclKind::Typedef(x) => {
            let mut rhs = Vec::new();
            match &x.kind {
                TypedefKind::Alias(t) => match &a.ty(*t).kind {
                    TypeKind::Function { return_type, type_params, parameters } => collect_from_function(*return_type, type_params, parameters, &mut rhs),
                    _ => rhs.push(*t),
                },
                TypedefKind::Legacy { return_type, parameters } => collect_from_function(*return_type, &[], parameters, &mut rhs),
            }
            (&x.type_params, rhs)
        }
        _ => return None,
    };
    Some((d.unit, tps, rhs))
}

/// `computeSimplyBounded` over every declaration of the program.
pub fn compute(program: &Program) -> SimpleBounds {
    let mut nodes: Vec<Node> = Vec::new();
    for (i, ce) in program.classes.iter().enumerate() {
        if ce.decl.is_some() && matches!(ce.kind, ClassKind::Class | ClassKind::Mixin | ClassKind::Enum | ClassKind::ExtensionType | ClassKind::MixinApplication) {
            nodes.push(Node::Class(ClassId(i as u32)));
        }
    }
    for i in 0..program.typedefs.len() {
        nodes.push(Node::Alias(TypedefId(i as u32)));
    }
    // `computeDependencies`: `None` decides false right away.
    let mut deps: HashMap<Node, Option<Vec<Node>>> = HashMap::new();
    for &node in &nodes {
        let Some((u, tps, rhs)) = node_parts(program, node) else {
            deps.insert(node, Some(Vec::new()));
            continue;
        };
        let a = &program.unit(u).ast;
        let scope: Vec<SymbolId> = tps.iter().map(|p| p.name.sym).collect();
        let mut v = Vec::new();
        let mut ok = true;
        for tp in tps {
            if let Some(b) = tp.bound
                && !visit(program, u, a, b, false, &scope, &mut v)
            {
                ok = false;
                break;
            }
        }
        if ok {
            for &t in &rhs {
                if !visit(program, u, a, t, true, &scope, &mut v) {
                    ok = false;
                    break;
                }
            }
        }
        deps.insert(node, ok.then_some(v));
    }
    // Tarjan (the `DependencyWalker`): the components come out with their
    // dependencies already evaluated; a cycle (or self-loop) is not simply bounded.
    struct State<'d> {
        deps: &'d HashMap<Node, Option<Vec<Node>>>,
        index: HashMap<Node, usize>,
        low: HashMap<Node, usize>,
        stack: Vec<Node>,
        on_stack: HashSet<Node>,
        next: usize,
        not_simple: HashSet<Node>,
    }
    fn connect(e: &mut State<'_>, v: Node) {
        e.index.insert(v, e.next);
        e.low.insert(v, e.next);
        e.next += 1;
        e.stack.push(v);
        e.on_stack.insert(v);
        let mut self_loop = false;
        let neighbours: Vec<Node> = match e.deps.get(&v) {
            Some(Some(d)) => d.clone(),
            _ => Vec::new(),
        };
        for w in neighbours.iter().copied() {
            if !e.deps.contains_key(&w) {
                continue;
            }
            if w == v {
                self_loop = true;
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
            if component.len() > 1 || self_loop {
                e.not_simple.extend(component);
            } else {
                // `_evaluate`: false right away, or some false dependency.
                let is_false = match e.deps.get(&v) {
                    Some(None) => true,
                    Some(Some(d)) => d.iter().any(|x| e.not_simple.contains(x)),
                    None => false,
                };
                if is_false {
                    e.not_simple.insert(v);
                }
            }
        }
    }
    let mut e = State { deps: &deps, index: HashMap::new(), low: HashMap::new(), stack: Vec::new(), on_stack: HashSet::new(), next: 1, not_simple: HashSet::new() };
    for &v in &nodes {
        if !e.index.contains_key(&v) {
            connect(&mut e, v);
        }
    }
    SimpleBounds { not_simple: e.not_simple }
}

/// A type parameter list and where its names are valid.
struct TypeParamList<'a> {
    params: &'a [TypeParameter],
    scope: Span,
    /// The declaration goes through linking (`DefaultTypesBuilder`: class,
    /// mixin, enum, extension, extension type, alias, method, top-level
    /// function), which replaces a cyclic bound with `dynamic` (`_breakSelfCycles`).
    linked: bool,
}

fn param_lists<'a>(ps: &'a [Parameter], out: &mut Vec<TypeParamList<'a>>) {
    for p in ps {
        if !p.function_type_params.is_empty() {
            out.push(TypeParamList { params: &p.function_type_params, scope: p.span, linked: false });
        }
        if let Some(inner) = &p.function_parameters {
            param_lists(inner, out);
        }
    }
}

/// Every type parameter list of the unit.
fn lists(a: &ast::Ast) -> Vec<TypeParamList<'_>> {
    let mut v = Vec::new();
    for d in &a.decls {
        let tps: &[TypeParameter] = match &d.kind {
            DeclKind::Class(x) => &x.type_params,
            DeclKind::Mixin(x) => &x.type_params,
            DeclKind::Enum(x) => &x.type_params,
            DeclKind::ExtensionType(x) => &x.type_params,
            DeclKind::Extension(x) => &x.type_params,
            DeclKind::Typedef(x) => {
                if let TypedefKind::Legacy { parameters, .. } = &x.kind {
                    param_lists(parameters, &mut v);
                }
                &x.type_params
            }
            _ => &[],
        };
        if !tps.is_empty() {
            v.push(TypeParamList { params: tps, scope: d.span, linked: true });
        }
    }
    // Top-level functions and methods (local functions and literals do not go
    // through linking).
    let mut ligadas: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for d in &a.decls {
        if let DeclKind::Function(fid) = &d.kind {
            ligadas.insert(fid.0);
        }
    }
    for m in &a.members {
        if let ast::MemberKind::Method(fid) = &m.kind {
            ligadas.insert(fid.0);
        }
    }
    for (i, f) in a.functions.iter().enumerate() {
        if !f.type_params.is_empty() {
            v.push(TypeParamList { params: &f.type_params, scope: f.span, linked: ligadas.contains(&(i as u32)) });
        }
        if let Some(ps) = &f.parameters {
            param_lists(ps, &mut v);
        }
    }
    for m in &a.members {
        if let ast::MemberKind::Constructor(k) = &m.kind {
            param_lists(&k.parameters, &mut v);
        }
    }
    for t in &a.types {
        if let TypeKind::Function { type_params, parameters, .. } = &t.kind {
            if !type_params.is_empty() {
                v.push(TypeParamList { params: type_params, scope: t.span, linked: false });
            }
            param_lists(parameters, &mut v);
        }
    }
    v
}

/// The parameter of the list that bound `t` designates after erasing
/// extension types (`boundType.extensionTypeErasure.element`).
fn param_of_bound(program: &Program, u: UnitId, a: &ast::Ast, t: ast::TypeId, list: &[TypeParameter], depth: u32) -> Option<usize> {
    if depth > 8 {
        return None;
    }
    let TypeKind::Named { name, args } = &a.ty(t).kind else { return None };
    if let [n] = &name[..]
        && let Some(i) = list.iter().position(|p| p.name.sym == n.sym)
    {
        return Some(i);
    }
    // An extension type whose representation is one of its parameters: the
    // erasure is the corresponding argument.
    let Some(Element::Class(c)) = element(program, u, name) else { return None };
    let ce = program.class(c);
    if ce.kind != ClassKind::ExtensionType {
        return None;
    }
    let d = ce.decl?;
    let ad = &program.unit(d.unit).ast;
    let DeclKind::ExtensionType(et) = &ad.decl(d.decl).kind else { return None };
    let TypeKind::Named { name: rn, args: ra } = &ad.ty(et.representation_type).kind else { return None };
    if !ra.is_empty() {
        return None;
    }
    let [r] = &rn[..] else { return None };
    let k = et.type_params.iter().position(|p| p.name.sym == r.sym)?;
    let &arg = args.get(k)?;
    param_of_bound(program, u, a, arg, list, depth + 1)
}

/// `_UninstantiatedBoundChecker` num limite.
fn uninstantiated(program: &Program, u: UnitId, a: &ast::Ast, t: ast::TypeId, visible: &dyn Fn(usize, SymbolId) -> bool, bounds: &SimpleBounds, out: &mut Vec<(UnitId, Diagnostic)>) {
    let no_tipo = a.ty(t);
    match &no_tipo.kind {
        TypeKind::Named { name, args } => {
            if !args.is_empty() {
                for &x in args.iter() {
                    uninstantiated(program, u, a, x, visible, bounds, out);
                }
                return;
            }
            if let [n] = &name[..]
                && visible(n.span.start, n.sym)
            {
                return;
            }
            if let Some(node) = element(program, u, name).and_then(|el| node_of(program, el))
                && bounds.not_simple.contains(&node)
            {
                out.push((u, Diagnostic::com_codigo(c::NOT_INSTANTIATED_BOUND, no_tipo.span, [] as [&str; 0])));
            }
        }
        TypeKind::Void => {}
        TypeKind::Function { return_type, type_params, parameters } => {
            if let Some(r) = return_type {
                uninstantiated(program, u, a, *r, visible, bounds, out);
            }
            for tp in type_params.iter() {
                if let Some(b) = tp.bound {
                    uninstantiated(program, u, a, b, visible, bounds, out);
                }
            }
            let mut types = Vec::new();
            collect_from_params(parameters, &mut types);
            for x in types {
                uninstantiated(program, u, a, x, visible, bounds, out);
            }
        }
        TypeKind::Record { positional, named } => {
            for &x in positional.iter().chain(named.iter().map(|(_, x)| x)) {
                uninstantiated(program, u, a, x, visible, bounds, out);
            }
        }
    }
}

/// The written type is a generic function type (directly or through an alias of one).
fn generic_function(program: &Program, u: UnitId, a: &ast::Ast, t: ast::TypeId, depth: u32) -> bool {
    match &a.ty(t).kind {
        TypeKind::Function { type_params, .. } => !type_params.is_empty(),
        TypeKind::Named { name, args } if args.is_empty() && depth < 8 => {
            let Some(Element::Typedef(td)) = element(program, u, name) else { return false };
            let d = program.typedef(td).decl;
            let ad = &program.unit(d.unit).ast;
            match &ad.decl(d.decl).kind {
                DeclKind::Typedef(x) => match &x.kind {
                    TypedefKind::Alias(body) => generic_function(program, d.unit, ad, *body, depth + 1),
                    TypedefKind::Legacy { .. } => false,
                },
                _ => false,
            }
        }
        _ => false,
    }
}

pub fn check(program: &Program, lib: LibraryId, names: &Interner, bounds: &SimpleBounds) -> Vec<(UnitId, Diagnostic)> {
    let mut out = Vec::new();
    // `generic-metadata` (2.14): before it, a generic function type as a
    // written type argument is `GENERIC_FUNCTION_TYPE_CANNOT_BE_TYPE_ARGUMENT`
    // (`type_arguments_verifier.dart:300-310`), on the argument.
    let without_generic_metadata = program.library(lib).features.versao() < dartforge_frontend::features::LanguageVersion::new(2, 14);
    for &u in &program.library(lib).units {
        let a = &program.unit(u).ast;
        if without_generic_metadata {
            for t in &a.types {
                let TypeKind::Named { args, .. } = &t.kind else { continue };
                for &x in args.iter() {
                    if generic_function(program, u, a, x, 0) {
                        out.push((u, Diagnostic::com_codigo(c::GENERIC_FUNCTION_TYPE_CANNOT_BE_TYPE_ARGUMENT, a.ty(x).span, [] as [&str; 0])));
                    }
                }
            }
        }
        let all = lists(a);
        // The type parameters in scope at a position: those of the lists
        // whose owner contains it.
        let visible = |pos: usize, name: SymbolId| {
            all.iter().any(|l| l.scope.start <= pos && pos <= l.scope.end && l.params.iter().any(|p| p.name.sym == name))
        };
        for l in &all {
            // `visitTypeParameter`: the bound of each parameter.
            for tp in l.params {
                if let Some(b) = tp.bound {
                    uninstantiated(program, u, a, b, &visible, bounds, &mut out);
                }
            }
            // `_checkForTypeParameterBoundRecursion`.
            let n = l.params.len();
            for (i, tp) in l.params.iter().enumerate() {
                if tp.bound.is_none() {
                    continue;
                }
                let mut current = Some(i);
                let mut step = 0usize;
                while let Some(k) = current {
                    current = l.params[k].bound.and_then(|b| param_of_bound(program, u, a, b, l.params, 0));
                    if step == n {
                        // `{1}` (only in the correction): the `element.bound`.
                        // In the linked declaration, `_breakSelfCycles`
                        // (`default_types_builder.dart:137-174`) replaces with
                        // `dynamic` the bound whose chain of simple names from
                        // the list leads back to it; otherwise, the bound as written.
                        let name = names.resolve(tp.name.sym).to_string();
                        let name_cycle = {
                            let mut current = Some(i);
                            for _ in 0..n {
                                current = current.and_then(|k| l.params[k].bound).and_then(|b| match &a.ty(b).kind {
                                    TypeKind::Named { name, .. } if name.len() == 1 => l.params.iter().position(|p| p.name.sym == name[0].sym),
                                    _ => None,
                                });
                            }
                            current.is_some()
                        };
                        let bound = if l.linked && name_cycle {
                            "dynamic".to_string()
                        } else {
                            tp.bound.map(|b| a.ty(b).span).map(|s| program.unit(u).source[s.start..s.end].to_string()).unwrap_or_default()
                        };
                        out.push((u, Diagnostic::com_codigo(c::TYPE_PARAMETER_SUPERTYPE_OF_ITS_BOUND, tp.name.span, [name.as_str(), bound.as_str()])));
                        break;
                    }
                    step += 1;
                }
            }
        }
    }
    out
}
