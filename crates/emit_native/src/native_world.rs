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
    let mut types = table.clone();
    let core = dartforge_types::CoreTypes::init(&mut types, program, interner);
    let inferred = program
        .libraries
        .iter()
        .enumerate()
        .filter(|(_, library)| !library.is_sdk)
        .map(|(i, _)| dartforge_elements::model::LibraryId(i as u32))
        .collect();
    let weak = dartforge_types::static_weak_references::normalize_for(
        program,
        interner,
        &mut types,
        &core,
        outline,
        bodies,
        &inferred,
        dartforge_types::static_weak_references::TargetPolicy::Native,
    );
    let options = dartforge_world::Options {
        preserve_constant_conditions: !weak.calls.is_empty(),
        ..Default::default()
    };
    let entries = entry_points(program, interner, &mut types, &core, outline, bodies);
    compute_with_weak_references(
        dartforge_world::WorldInput {
            program,
            interner,
            table,
            outline,
            bodies,
        },
        weak,
        options,
        entries,
    )
}

/// Compute native roots with the backend's validated weak-reference edges.
/// An empty set retains ordinary calls and constant locals for reloadable code.
pub(crate) fn compute_with_weak_references(
    input: dartforge_world::WorldInput<'_>,
    weak: dartforge_types::static_weak_references::WeakReferences,
    options: dartforge_world::Options,
    mut r: dartforge_world::Roots,
) -> Option<dartforge_world::World> {
    let dartforge_world::WorldInput {
        program,
        interner,
        table,
        bodies,
        ..
    } = input;
    if std::env::var_os("DARTFORGE_NO_PROGRAM_PRUNING").is_some_and(|v| v != "0") {
        return None;
    }
    let main = crate::context::entry_main(program, interner)?;
    r.functions.push(FunctionElementId(main as u32));
    let mut names = std::collections::BTreeSet::new();
    for n in LOWERING_NAMES {
        names.insert((*n).to_string());
    }
    names_written_in_sdk(program, interner, &mut names);
    js_exported_names(program, interner, table, bodies, &mut names);
    r.selectors = names.into_iter().collect();
    Some(dartforge_world::compute_with_weak_references(
        input, &r, weak, options,
    ))
}

/// `@pragma('vm:entry-point')` in the program: the VM retains what the annotation
/// marks (the embedder calls by name); here it becomes a root — the function (top-level
/// or method) and the class.
pub(crate) fn entry_points(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &dartforge_types::CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
) -> dartforge_world::Roots {
    use dartforge_elements::model::{ClassId, FunctionRef, LibraryId, UnitId};
    use dartforge_types::const_eval::{
        evaluation::Engine,
        value::{Field, InstanceState},
    };
    let inferred = program
        .libraries
        .iter()
        .enumerate()
        .filter(|(_, library)| !library.is_sdk)
        .map(|(i, _)| LibraryId(i as u32))
        .collect();
    let mut engine = Engine::new(program, interner, table, core, outline, bodies, &inferred);
    let mut r = dartforge_world::Roots::default();
    // Metadata authority follows the evaluated core instance, never the spelling
    // of an annotation. False disables the root; aliases keep the same identity.
    let mut marked = |unit: UnitId, metadata: &[ast::Annotation]| {
        metadata.iter().any(|annotation| {
            let Some(value) = engine.evaluate_annotation(unit, annotation) else { return false; };
            let dartforge_types::Type::Interface { class, .. } = engine.table.get(value.ty) else { return false; };
            let class = program.class(*class);
            if program.library(class.library).uri != "dart:core" || interner.resolve(class.name) != "pragma" {
                return false;
            }
            let InstanceState::Generic { fields, .. } = &value.state else { return false; };
            let field = |name: &str| fields.iter().find_map(|(key, value)| {
                matches!(key, Field::MemberName(symbol) if interner.resolve(*symbol) == name).then_some(value)
            });
            matches!(field("name").map(|v| &v.state), Some(InstanceState::Str(Some(name)))
                if String::from_utf16_lossy(name) == "vm:entry-point")
                && !matches!(field("options").map(|v| &v.state), Some(InstanceState::Bool(Some(false))))
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
                    && marked(u, &d.metadata)
                    && let Some(&f) = by_node.get(&(u, fid))
                {
                    r.functions.push(f);
                }
            }
            for m in &a.members {
                if let ast::MemberKind::Method(fid) = m.kind
                    && marked(u, &m.metadata)
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
        if marked(d.unit, &a.decl(d.decl).metadata) {
            r.instantiated_classes.push(ClassId(i as u32));
        }
    }
    r
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
