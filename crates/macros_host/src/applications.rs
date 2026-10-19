//! Macro applications: detection and order (macro spec, "Application
//! order", `:250-286`; the execution order is the one of CFE 3.6.2,
//! `kernel_macro_macro.dart`).
//!
//! **Detection** (zero-cost rule): an annotation is an application only if
//! its name resolves to a constructor of a class declared with `macro`
//! ([`Program::classes_macro`]); with no `macro` class in the program,
//! [`detect`] returns on its first line, without looking at any
//! annotation. An application is always a constructor call (`@M()`,
//! `@M.name(…)`, `@p.M(…)`).
//!
//! **Order**: within a declaration, right to left (`@C @B @A` runs `A`
//! first); members before the type that contains them. Across declarations,
//! the order of CFE 3.6.2, which is the oracle for the text:
//!
//! * types and definitions phases, per library: those on the `library`
//!   directive, then those on top-level functions and variables, then, per
//!   class in source order, those on the members (fields and methods, then
//!   constructors) and those on the class;
//! * declarations phase: the classes in hierarchy order (supertypes first) —
//!   members, then the class —, and only then, per library, those on the
//!   directive and the top-level ones.
use crate::executor::Phase;
use crate::model::{Key, View};
use dartforge_diagnostics::Span;
use dartforge_elements::model::*;
use dartforge_frontend::ast::{self, DeclKind, DirectiveKind, ExprKind, MemberKind};
use serde_json::{Value, json};
use std::collections::HashSet;

/// What receives the application.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    Library(String),
    Declaration(Key),
}

/// Where the application falls in the CFE order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Group {
    /// On the `library` directive.
    Library,
    /// On a top-level function or variable.
    TopLevel,
    /// On a member of the class `owner` (`constructor` goes after the
    /// others).
    Member { owner: Key, constructor: bool },
    /// On the class itself.
    Class(Key),
}

#[derive(Debug, Clone)]
pub struct Application {
    /// URI of the annotation's library (the one that receives the
    /// augmentation).
    pub library: String,
    pub target: Target,
    pub group: Group,
    /// `uri#Class` of the macro.
    pub macro_: String,
    /// Constructor name (`""` for the unnamed one).
    pub constructor: String,
    /// `{positional, named}` in the protocol's argument format.
    pub arguments: Value,
    /// Unit (URI) and range of the annotation, for diagnostics.
    pub unit: String,
    pub span: Span,
}

/// The `macro` class that an annotation invokes, and the constructor name.
fn annotation_macro(v: &View<'_>, lib: LibraryId, a: &ast::Annotation, macros: &HashSet<ClassId>) -> Option<(ClassId, String)> {
    a.arguments.as_ref()?;
    let class = |el: Option<Element>| match el {
        Some(Element::Class(c)) if macros.contains(&c) => Some(c),
        _ => None,
    };
    let name = |s: dartforge_intern::SymbolId| v.interner.resolve(s).to_string();
    match a.name.as_slice() {
        [c] => class(v.program.lookup(lib, c.sym).and_then(|b| b.getter)).map(|c| (c, String::new())),
        [x, y] => {
            if let Some(c) = class(v.program.lookup(lib, x.sym).and_then(|b| b.getter)) {
                return Some((c, name(y.sym)));
            }
            class(v.program.lookup_prefixed(lib, x.sym, y.sym).and_then(|b| b.getter)).map(|c| (c, String::new()))
        }
        [p, c, k] => class(v.program.lookup_prefixed(lib, p.sym, c.sym).and_then(|b| b.getter)).map(|c| (c, name(k.sym))),
        _ => None,
    }
}

/// An application argument (spec, "Macro arguments"): a literal or a
/// collection of literals; any other expression goes as `Code` with its
/// text.
fn argument(u: &Unit, e: ast::ExprId) -> Value {
    let ex = u.ast.expr(e);
    let text = |s: Span| u.source.get(s.start..s.end).unwrap_or("").to_string();
    let elements = |els: &[ast::CollectionElement]| -> Option<Vec<Value>> {
        els.iter()
            .map(|el| match el {
                ast::CollectionElement::Expression(x) => Some(argument(u, *x)),
                _ => None,
            })
            .collect()
    };
    match &ex.kind {
        ExprKind::Null => json!({"t": "null"}),
        ExprKind::Bool(b) => json!({"t": "bool", "v": b}),
        ExprKind::Int(s) => json!({"t": "int", "v": text(*s).replace('_', "")}),
        ExprKind::Double(s) => {
            json!({"t": "double", "v": text(*s).replace('_', "").parse::<f64>().unwrap_or(0.0)})
        }
        ExprKind::String(l) if l.parts.iter().all(|p| matches!(p, ast::StringPart::Text(_))) => {
            let s: String = l
                .parts
                .iter()
                .map(|p| match p {
                    ast::StringPart::Text(t) => t.to_string_lossy(),
                    _ => String::new(),
                })
                .collect();
            json!({"t": "string", "v": s})
        }
        ExprKind::Parenthesized(x) => argument(u, *x),
        ExprKind::List { elements: els, .. } => match elements(els) {
            Some(l) => json!({"t": "list", "v": l}),
            None => code_argument(text(ex.span)),
        },
        ExprKind::SetOrMap { elements: els, .. } => {
            let pairs: Option<Vec<Value>> = els
                .iter()
                .map(|el| match el {
                    ast::CollectionElement::MapEntry { key, value, null_aware_key: false, null_aware_value: false } => {
                        Some(json!([argument(u, *key), argument(u, *value)]))
                    }
                    _ => None,
                })
                .collect();
            match pairs {
                Some(p) if !p.is_empty() || els.is_empty() => json!({"t": "map", "v": p}),
                _ => match elements(els) {
                    Some(l) => json!({"t": "set", "v": l}),
                    None => code_argument(text(ex.span)),
                },
            }
        }
        _ => code_argument(text(ex.span)),
    }
}

fn code_argument(text: String) -> Value {
    json!({"t": "code", "v": {"k": "expression", "p": [text]}})
}

fn arguments(u: &Unit, a: &ast::Annotation) -> Value {
    let mut positional = Vec::new();
    let mut named = serde_json::Map::new();
    for arg in a.arguments.iter().flat_map(|x| x.args.iter()) {
        match arg.name {
            Some(n) => {
                let name = u.source.get(n.span.start..n.span.end).unwrap_or("").to_string();
                named.insert(name, argument(u, arg.value));
            }
            None => positional.push(argument(u, arg.value)),
        }
    }
    json!({"positional": positional, "named": named})
}

/// The program's applications, in source order (the phase order comes from
/// [`order`]). Empty — at no cost — when there is no `macro` class.
pub fn detect(v: &View<'_>) -> Vec<Application> {
    if v.program.classes_macro.is_empty() {
        return Vec::new();
    }
    let macros: HashSet<ClassId> = v.program.classes_macro.iter().copied().collect();
    let mut out = Vec::new();
    for (li, lib) in v.program.libraries.iter().enumerate() {
        if lib.is_sdk {
            continue;
        }
        let lib_id = LibraryId(li as u32);
        let mut collect = |out: &mut Vec<Application>, uid: UnitId, meta: &[ast::Annotation], target: Target, group: Group| {
            let u = v.program.unit(uid);
            // Right to left: the last annotation runs first.
            for a in meta.iter().rev() {
                let Some((c, constructor)) = annotation_macro(v, lib_id, a, &macros) else { continue };
                let mc = v.program.class(c);
                out.push(Application {
                    library: lib.uri.clone(),
                    target: target.clone(),
                    group: group.clone(),
                    macro_: format!("{}#{}", v.program.library(mc.library).uri, v.interner.resolve(mc.name)),
                    constructor,
                    arguments: arguments(u, a),
                    unit: u.uri.clone(),
                    span: a.span,
                });
            }
        };
        for &uid in &lib.units {
            let u = v.program.unit(uid);
            for d in &u.unit.directives {
                if matches!(d.kind, DirectiveKind::Library { .. }) {
                    collect(&mut out, uid, &d.metadata, Target::Library(lib.uri.clone()), Group::Library);
                }
            }
            for &did in &u.unit.declarations {
                let d = u.ast.decl(did);
                if d.augment {
                    continue;
                }
                let n = |s: dartforge_intern::SymbolId| v.interner.resolve(s).to_string();
                match &d.kind {
                    DeclKind::Class(c) => {
                        let key = Key::Type { lib: lib.uri.clone(), name: n(c.name.sym) };
                        members(v, &mut out, &mut collect, uid, &c.members, &key);
                        collect(&mut out, uid, &d.metadata, Target::Declaration(key.clone()), Group::Class(key));
                    }
                    DeclKind::Mixin(m) => {
                        let key = Key::Type { lib: lib.uri.clone(), name: n(m.name.sym) };
                        members(v, &mut out, &mut collect, uid, &m.members, &key);
                        collect(&mut out, uid, &d.metadata, Target::Declaration(key.clone()), Group::Class(key));
                    }
                    DeclKind::Enum(e) => {
                        let key = Key::Type { lib: lib.uri.clone(), name: n(e.name.sym) };
                        collect(&mut out, uid, &d.metadata, Target::Declaration(key.clone()), Group::Class(key));
                    }
                    DeclKind::Function(fid) => {
                        let f = u.ast.function(*fid);
                        let Some(name) = f.name else { continue };
                        let key = Key::TopLevelFunction {
                            lib: lib.uri.clone(),
                            name: n(name.sym),
                            setter: f.kind == ast::FunctionKind::Setter,
                        };
                        collect(&mut out, uid, &d.metadata, Target::Declaration(key), Group::TopLevel);
                    }
                    DeclKind::Variables(l) => {
                        for var in l.variables.iter() {
                            let key = Key::TopLevelVariable { lib: lib.uri.clone(), name: n(var.name.sym) };
                            collect(&mut out, uid, &d.metadata, Target::Declaration(key), Group::TopLevel);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

type Collector<'c> = dyn FnMut(&mut Vec<Application>, UnitId, &[ast::Annotation], Target, Group) + 'c;

/// Members of a class: fields and methods in order, then constructors
/// (CFE's `localMemberIterator` and `localConstructorIterator`).
fn members(v: &View<'_>, out: &mut Vec<Application>, collect: &mut Collector<'_>, uid: UnitId, ids: &[ast::MemberId], class: &Key) {
    let u = v.program.unit(uid);
    let Key::Type { lib, name: owner } = class else { return };
    let n = |s: dartforge_intern::SymbolId| v.interner.resolve(s).to_string();
    for constructors in [false, true] {
        for &mid in ids {
            let m = u.ast.member(mid);
            if m.metadata.is_empty() || m.augment {
                continue;
            }
            let key = match (&m.kind, constructors) {
                (MemberKind::Constructor(k), true) => {
                    Key::Constructor { lib: lib.clone(), owner: owner.clone(), name: k.name.map(|x| n(x.sym)).unwrap_or_default() }
                }
                (MemberKind::Method(fid), false) => {
                    let f = u.ast.function(*fid);
                    let Some(name) = f.name else { continue };
                    Key::Method { lib: lib.clone(), owner: owner.clone(), name: n(name.sym), setter: f.kind == ast::FunctionKind::Setter }
                }
                (MemberKind::Field(l), false) => {
                    for var in l.variables.iter() {
                        let key = Key::Field { lib: lib.clone(), owner: owner.clone(), name: n(var.name.sym) };
                        collect(out, uid, &m.metadata, Target::Declaration(key), Group::Member { owner: class.clone(), constructor: false });
                    }
                    continue;
                }
                _ => continue,
            };
            collect(out, uid, &m.metadata, Target::Declaration(key), Group::Member { owner: class.clone(), constructor: constructors });
        }
    }
}

/// The execution order of the applications in a phase (indices into
/// `apps`).
pub fn order(phase: Phase, apps: &[Application], v: &View<'_>) -> Vec<usize> {
    let mut libraries: Vec<&str> = Vec::new();
    for a in apps {
        if !libraries.contains(&a.library.as_str()) {
            libraries.push(&a.library);
        }
    }
    let library_classes = |lib: &str| -> Vec<Key> {
        let mut cs: Vec<Key> = Vec::new();
        for a in apps.iter().filter(|a| a.library == lib) {
            if let Group::Member { owner: class, .. } | Group::Class(class) = &a.group
                && !cs.contains(class) {
                    cs.push(class.clone());
                }
        }
        cs
    };
    let of_class = |c: &Key, out: &mut Vec<usize>| {
        for constructor in [false, true] {
            out.extend(apps.iter().enumerate().filter(|(_, a)| matches!(&a.group, Group::Member { owner, constructor: k } if owner == c && *k == constructor)).map(|(i, _)| i));
        }
        out.extend(apps.iter().enumerate().filter(|(_, a)| matches!(&a.group, Group::Class(k) if k == c)).map(|(i, _)| i));
    };
    let top_level = |lib: &str, out: &mut Vec<usize>| {
        out.extend(apps.iter().enumerate().filter(|(_, a)| a.library == lib && a.group == Group::Library).map(|(i, _)| i));
        out.extend(apps.iter().enumerate().filter(|(_, a)| a.library == lib && a.group == Group::TopLevel).map(|(i, _)| i));
    };
    let mut out = Vec::new();
    match phase {
        Phase::Types | Phase::Definitions => {
            for lib in &libraries {
                top_level(lib, &mut out);
                for c in library_classes(lib) {
                    of_class(&c, &mut out);
                }
            }
        }
        Phase::Declarations => {
            // Supertypes first: post-order of the hierarchy (`sortedSourceClassBuilders`).
            let all: Vec<Key> = libraries.iter().flat_map(|l| library_classes(l)).collect();
            let ids: Vec<Option<ClassId>> = all.iter().map(|c| v.class_of(c)).collect();
            let mut done: HashSet<usize> = HashSet::new();
            fn visit(i: usize, ids: &[Option<ClassId>], v: &View<'_>, done: &mut HashSet<usize>, seq: &mut Vec<usize>, stack: &mut HashSet<usize>) {
                if done.contains(&i) || !stack.insert(i) {
                    return;
                }
                if let Some(c) = ids[i] {
                    let cl = v.program.class(c);
                    let supers = cl.supertype_class.into_iter().chain(cl.mixin_classes.iter().copied()).chain(cl.interface_classes.iter().copied()).chain(cl.on_classes.iter().copied());
                    for s in supers {
                        if let Some(j) = ids.iter().position(|x| *x == Some(s)) {
                            visit(j, ids, v, done, seq, stack);
                        }
                    }
                }
                stack.remove(&i);
                if done.insert(i) {
                    seq.push(i);
                }
            }
            let mut seq = Vec::new();
            for i in 0..all.len() {
                visit(i, &ids, v, &mut done, &mut seq, &mut HashSet::new());
            }
            for i in seq {
                of_class(&all[i], &mut out);
            }
            for lib in &libraries {
                top_level(lib, &mut out);
            }
        }
    }
    out
}
