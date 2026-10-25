//! Pruning of the program code by the closed world, before the lowering
//! (docs/NATIVO-PROJETOS-REAIS.md, cause C7).
//!
//! The program module used to lower EVERY function of every library the
//! program loads: in new_sali/backend, 311 thousand functions and 2.8 GB of LLVM IR
//! (the Unicode tables of `unorm_dart`, the 180 MB of generated FFI bindings of
//! `openssl_bindings`, the grammars of `highlight`…), almost all of it
//! unreachable from `main`. Dart AOT only compiles what the type flow
//! analysis retains (`VM/pkg/vm/lib/transformations/type_flow/`,
//! `VM/runtime/vm/compiler/aot/precompiler.cc` `DropFunctions`).
//!
//! Here reachability is that of `crates/world` — the RTA of production JS:
//! instantiated classes × invoked selectors, with the SDK as the boundary
//! (members that implement an SDK supertype, those of `Object` and `call`
//! live in every instantiated class). What the compiled SDK (outside the world)
//! may call on a program object enters as an external selector: EVERY member
//! name written in the SDK libraries (with the `native_sdk/`
//! overlay) — `x.name`, a bare `name` (the implicit `this.name()` of an
//! SDK class extended by the program), `#name` symbols and object-pattern
//! fields. It is a syntactic superset: the SDK only calls a member by
//! writing its name (no mirrors), or through `noSuchMethod`/`call`, which
//! are always live. Plus the names the lowering itself calls by selector
//! (`iterator`, `moveNext`, `current`, `[]=`…).
//!
//! An instance, top-level or static function that the world does not reach becomes a
//! body that throws (`pruned`); a global variable, a getter that throws. The
//! symbols stay defined (the method tables, the uniform entries
//! and the class registration cite them), but the body — and what it would
//! carry along — goes away. If the world is wrong, the error is loud, with the name:
//! `UnsupportedError` "código podado como inalcançável foi chamado: <símbolo>".
//! `DARTFORGE_NO_PROGRAM_PRUNING=1` turns it off (measurement).

use dartforge_elements::model::{FunctionElementId, Program};
use dartforge_frontend::ast;
use dartforge_intern::Interner;
use dartforge_types::resolve::OutlineTypes;
use dartforge_types::resolved::BodyTypes;
use dartforge_types::table::TypeTable;

/// The names the lowering calls by selector on program values, besides
/// those the SDK writes (`lower/*.rs`: `call_by_name`,
/// `method_by_selector`, `property_by_selector`).
const LOWERING_NAMES: &[&str] = &[
    "iterator",
    "moveNext",
    "current",
    "[]",
    "[]=",
    "==",
    "add",
    "addAll",
    "length",
    "hashCode",
    "toString",
    "fillRange",
    "containsKey",
    "cancel",
    "*",
    "call",
    "noSuchMethod",
    "then",
];

/// Computes the program world. `None` when the pruning is off.
pub fn compute(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
) -> Option<dartforge_world::World> {
    if std::env::var_os("DARTFORGE_NO_PROGRAM_PRUNING").is_some_and(|v| v != "0") {
        return None;
    }
    let mut r = dartforge_world::Roots::default();
    let main = crate::context::entry_main(program, interner)?;
    r.functions.push(FunctionElementId(main as u32));
    marked_entries(program, interner, &mut r);
    let mut names = std::collections::BTreeSet::new();
    for n in LOWERING_NAMES {
        names.insert((*n).to_string());
    }
    names_written_in_sdk(program, interner, &mut names);
    js_exported_names(program, interner, table, bodies, &mut names);
    r.selectors = names.into_iter().collect();
    let e = dartforge_world::WorldInput {
        program,
        interner,
        table,
        outline,
        bodies,
    };
    Some(dartforge_world::compute(e, &r))
}

/// `@pragma('vm:entry-point')` in the program: the VM retains what the annotation
/// marks (the embedder calls by name); here it becomes a root — the function (top-level
/// or method) and the class.
fn marked_entries(program: &Program, interner: &Interner, r: &mut dartforge_world::Roots) {
    use dartforge_elements::model::{ClassId, FunctionRef};
    let marked = |a: &ast::Ast, meta: &[ast::Annotation]| {
        meta.iter().any(|m| {
            m.name.last().is_some_and(|n| interner.resolve(n.sym) == "pragma")
                && m.arguments.as_ref().and_then(|args| args.args.first()).is_some_and(|arg| {
                    matches!(&a.expr(arg.value).kind, ast::ExprKind::String(s)
                        if s.constant_value().is_some_and(|v| v.to_string_lossy().starts_with("vm:entry-point")))
                })
        })
    };
    let mut by_node = std::collections::HashMap::new();
    for (i, f) in program.functions.iter().enumerate() {
        if let FunctionRef::Function { unit, function } = f.node
            && !program.library(f.library).is_sdk
        {
            by_node.insert((unit, function), FunctionElementId(i as u32));
        }
    }
    for lib in &program.libraries {
        if lib.is_sdk {
            continue;
        }
        for &u in &lib.units {
            let a = &program.unit(u).ast;
            for d in &a.decls {
                if let ast::DeclKind::Function(fid) = d.kind
                    && marked(a, &d.metadata)
                    && let Some(&f) = by_node.get(&(u, fid))
                {
                    r.functions.push(f);
                }
            }
            for m in &a.members {
                if let ast::MemberKind::Method(fid) = m.kind
                    && marked(a, &m.metadata)
                    && let Some(&f) = by_node.get(&(u, fid))
                {
                    r.functions.push(f);
                }
            }
        }
    }
    for (i, c) in program.classes.iter().enumerate() {
        if program.library(c.library).is_sdk {
            continue;
        }
        let Some(d) = c.decl else { continue };
        let a = &program.unit(d.unit).ast;
        if marked(a, &a.decl(d.decl).metadata) {
            r.instantiated_classes.push(ClassId(i as u32));
        }
    }
}

/// Every member name written in the SDK libraries.
fn names_written_in_sdk(
    program: &Program,
    interner: &Interner,
    names: &mut std::collections::BTreeSet<String>,
) {
    for lib in &program.libraries {
        if !lib.is_sdk {
            continue;
        }
        for &u in &lib.units {
            let a = &program.unit(u).ast;
            for e in &a.exprs {
                match &e.kind {
                    ast::ExprKind::Identifier(n) | ast::ExprKind::Property { name: n, .. } => {
                        names.insert(interner.resolve(n.sym).to_string());
                    }
                    ast::ExprKind::Symbol(ns) => {
                        for n in ns.iter() {
                            names.insert(interner.resolve(n.sym).to_string());
                        }
                    }
                    _ => {}
                }
            }
            for p in &a.patterns {
                if let ast::PatternKind::Object { fields, .. } = &p.kind {
                    for f in fields.iter() {
                        if let Some(n) = &f.name {
                            names.insert(interner.resolve(n.sym).to_string());
                        }
                    }
                }
            }
        }
    }
}

/// `createJSInteropWrapper<T>(o)` (WebAssembly interop, `lower/js_members.rs`):
/// JavaScript calls the exported members of `T`'s class by name on whatever
/// object `o` is. Every instance member name of the program classes in that
/// class's chain becomes an external selector — a superset of the
/// `@JSExport` members.
fn js_exported_names(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    bodies: &BodyTypes,
    names: &mut std::collections::BTreeSet<String>,
) {
    use dartforge_types::table::Type as Ty;
    let Some(wrapper) = interner.lookup("createJSInteropWrapper") else {
        return;
    };
    for lib in &program.libraries {
        if lib.is_sdk {
            continue;
        }
        for &u in &lib.units {
            let a = &program.unit(u).ast;
            for e in &a.exprs {
                let ast::ExprKind::Call { target, arguments } = &e.kind else {
                    continue;
                };
                let callee = match &a.expr(*target).kind {
                    ast::ExprKind::Identifier(n) | ast::ExprKind::Property { name: n, .. } => n.sym,
                    _ => continue,
                };
                if callee != wrapper {
                    continue;
                }
                let Some(&ty) = bodies.units[u.0 as usize]
                    .instantiation(arguments.span.start)
                    .and_then(|t| t.first())
                else {
                    continue;
                };
                let Ty::Interface { class, .. } = table.get(ty) else {
                    continue;
                };
                let mut chain = vec![*class];
                while let Some(c) = chain.pop() {
                    let class = &program.classes[c.0 as usize];
                    if program.library(class.library).is_sdk {
                        continue;
                    }
                    for n in class.instance_members.keys() {
                        names.insert(interner.resolve(*n).trim_end_matches('=').to_string());
                    }
                    chain.extend(class.supertype_class);
                    chain.extend(class.mixin_classes.iter().copied());
                }
            }
        }
    }
}
