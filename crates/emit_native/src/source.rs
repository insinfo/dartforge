//! SDK libraries compiled **from source** together with the program (P6,
//! docs/NATIVO-PLANO.md §7.7).
//!
//! Decision 1 of round 2 (NATIVO-PLANO §7.1) is that the native SDK comes from the
//! source of the `vm` section with the `native_sdk/` overlay. `dart:async` is the
//! first library to come in this way, because `async`/`await` needs it
//! whole (`_Future`, `Zone`, microtasks, `Timer`) and none of its classes
//! has its own representation in the Rust runtime: they are all ordinary heap
//! objects, like the program's classes.
//!
//! **How it comes in.** A source library is treated by the lowering as a
//! program library: the classes get an id and a layout, the functions become
//! stable symbols (`df.dart$3aasync.…`), the bodies are inferred. What
//! makes that happen is turning `is_sdk` off for that library in **this compiler's
//! copy of the `Program`**, before inference — the only effect of
//! `is_sdk` in `crates/types` is skipping body inference, and that is
//! exactly what must stop happening (δ's request to the owner of
//! inference, docs/NATIVO-PEDIDOS.md, asks for the same thing through a parameter;
//! when it exists, the change is one line here).
//!
//! **Zero cost for those who do not use it** (PLANO.md, governing rule): only the
//! program that uses `dart:async` (an `async` function/generator, `await`, the name
//! `Future`/`Stream`/`FutureOr` or `import 'dart:async'`) compiles it; the
//! others emit the same IR as before.
//!
//! **Pruning.** The whole library is lowered (it is the open world of an
//! SDK library, as in the per-library module of P5c), and then the
//! module is pruned starting from the program: only the source functions the
//! program reaches stay (call, closure, tear-off, a class's `toString`). The
//! diagnostic of an unsupported construct in a source function only counts if
//! it stays — what the program does not reach cannot prevent it from compiling.

use crate::hir::{Instruction, Module};
use dartforge_elements::model::{LibraryId, Program};
use dartforge_frontend::ast::{AsyncModifier, ExprKind, TypeKind};
use dartforge_intern::Interner;
use std::collections::{HashMap, HashSet};

/// The SDK libraries the program compiles from source.
pub fn source_libraries(program: &Program, interner: &Interner) -> Vec<LibraryId> {
    let find_library = |uri: &str| program.libraries.iter().position(|l| l.uri == uri).map(|i| LibraryId(i as u32));
    let Some(async_lib) = find_library("dart:async") else {
        return Vec::new();
    };
    if !uses_dart_async(program, interner, async_lib) {
        return Vec::new();
    }
    // `dart:_internal` comes along: `dart:async` uses its `unsafeCast`,
    // `IterableElementError`, `checkNotNullable`… (pure Dart code, except for the
    // intrinsics, `lower/externals.rs`).
    let mut v = vec![async_lib];
    v.extend(find_library("dart:_internal"));
    v
}

/// The `part`s of `dart:core` that the source `dart:async` needs and that the
/// runtime does not represent: pure Dart code, without `external`. `Timer` receives
/// a `Duration` (`Future()`, `Future.delayed`, `Timer.run` use
/// `Duration.zero`); the Rust runtime never had `Duration`.
const CORE_PARTS: &[&str] = &["core/duration.dart"];

/// Takes the units of `CORE_PARTS` out of `dart:core` into a new
/// library, with the same URI and the same scope, that is not an SDK one — their classes
/// become compiled from source like those of `dart:async`, and the rest of
/// `dart:core` remains the runtime's. The elements keep their ids
/// (whoever references them still references them); only the owning library changes.
/// Returns the new library.
pub fn split_core_parts(program: &mut Program) -> Option<LibraryId> {
    use dartforge_elements::model::{FunctionRef, Library, VariableRef};
    let core = program.core?;
    let units: Vec<dartforge_elements::model::UnitId> = program.libraries[core.0 as usize]
        .units
        .iter()
        .copied()
        .filter(|u| {
            program.unit(*u).path.as_ref().is_some_and(|p| {
                let s = p.to_string_lossy().replace('\\', "/");
                CORE_PARTS.iter().any(|suf| s.ends_with(suf))
            })
        })
        .collect();
    if units.is_empty() {
        return None;
    }
    let origin = &program.libraries[core.0 as usize];
    let new_one = Library {
        uri: origin.uri.clone(),
        name: origin.name.clone(),
        units: units.clone(),
        imports: origin.imports.clone(),
        exports: Vec::new(),
        declared: origin.declared.clone(),
        exported: origin.exported.clone(),
        scope: origin.scope.clone(),
        prefixes: origin.prefixes.clone(),
        is_sdk: false,
        // The language version and the features per library (modern Dart, P2).
        features: origin.features,
        // The scope of each unit (A04) and the part that included it follow the
        // units that come into the new library.
        unit_parents: origin.unit_parents.iter().filter(|(u, _)| units.contains(u)).map(|(u, p)| (*u, *p)).collect(),
        representative: Default::default(),
        group_owner: Default::default(),
        unit_scopes: origin
            .unit_scopes
            .iter()
            .filter(|(u, _)| units.contains(u))
            .map(|(u, e)| (*u, e.clone()))
            .collect(),
    };
    let id = LibraryId(program.libraries.len() as u32);
    program.libraries.push(new_one);
    program.libraries[core.0 as usize].units.retain(|u| !units.contains(u));
    for u in &units {
        program.units[u.0 as usize].library = id;
    }
    for c in &mut program.classes {
        if c.decl.is_some_and(|d| units.contains(&d.unit)) {
            c.library = id;
        }
    }
    for f in &mut program.functions {
        let unit = match f.node {
            FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => Some(unit),
            FunctionRef::None => None,
        };
        // Synthetic constructor: the unit is that of the class.
        let unit = unit.or_else(|| f.class.and_then(|c| program.classes[c.0 as usize].decl.map(|d| d.unit)));
        if unit.is_some_and(|u| units.contains(&u)) {
            f.library = id;
        }
    }
    for v in &mut program.variables {
        let unit = match v.node {
            VariableRef::TopLevel { unit, .. } | VariableRef::Field { unit, .. } | VariableRef::EnumConstant { unit, .. } => {
                Some(unit)
            }
            _ => None,
        };
        if unit.is_some_and(|u| units.contains(&u)) {
            v.library = id;
        }
    }
    Some(id)
}

/// Does the program (the libraries that are not SDK ones) use `dart:async`?
fn uses_dart_async(program: &Program, interner: &Interner, async_lib: LibraryId) -> bool {
    let names: HashSet<&str> = ["Future", "Stream", "FutureOr"].into_iter().collect();
    // `dart:isolate` and `dart:io` deliver results through the isolate queue
    // (ports, IOService): whoever imports them needs the event loop.
    let with_loop: Vec<LibraryId> = program
        .libraries
        .iter()
        .enumerate()
        .filter(|(_, l)| l.uri == "dart:isolate" || l.uri == "dart:io")
        .map(|(i, _)| LibraryId(i as u32))
        .collect();
    for lib in program.libraries.iter().filter(|l| !l.is_sdk) {
        if lib.imports.iter().any(|i| i.library == async_lib || with_loop.contains(&i.library)) {
            return true;
        }
        for &u in &lib.units {
            let ast = &program.unit(u).ast;
            if ast.functions.iter().any(|f| f.modifier != AsyncModifier::None) {
                return true;
            }
            for e in &ast.exprs {
                match &e.kind {
                    ExprKind::Await(_) => return true,
                    ExprKind::Identifier(n) if names.contains(interner.resolve(n.sym)) => return true,
                    _ => {}
                }
            }
            for t in &ast.types {
                if let TypeKind::Named { name, .. } = &t.kind
                    && name.last().is_some_and(|n| names.contains(interner.resolve(n.sym)))
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Function `fid` has `@pragma("<text>")` on the `Member`/`Decl` that declares it.
pub fn has_pragma(program: &Program, interner: &Interner, fid: usize, text: &str) -> bool {
    use dartforge_elements::model::FunctionRef;
    use dartforge_frontend::ast;
    let f = &program.functions[fid];
    let (unit, constructor, function) = match f.node {
        FunctionRef::Function { unit, function } => (unit, None, Some(function)),
        FunctionRef::Constructor { unit, member } => (unit, Some(member), None),
        FunctionRef::None => return false,
    };
    let a = &program.unit(unit).ast;
    let metadata: &[ast::Annotation] = if let Some(member) = constructor {
        &a.members[member.0 as usize].metadata
    } else { a
        .members
        .iter()
        .find(|m| matches!(m.kind, ast::MemberKind::Method(id) if Some(id) == function))
        .map(|m| &*m.metadata)
        .or_else(|| a.decls.iter().find(|d| matches!(d.kind, ast::DeclKind::Function(id) if Some(id) == function)).map(|d| &*d.metadata))
        .unwrap_or(&[]) };
    metadata.iter().any(|an| pragma_with_string(interner, a, an, text))
}

/// The annotation is `@pragma("<text>", …)`.
pub fn pragma_with_string(interner: &Interner, ast: &dartforge_frontend::ast::Ast, an: &dartforge_frontend::ast::Annotation, text: &str) -> bool {
    if an.name.len() != 1 || interner.resolve(an.name[0].sym) != "pragma" {
        return false;
    }
    let Some(args) = &an.arguments else { return false };
    args.args.first().is_some_and(|a| match &ast.exprs[a.value.0 as usize].kind {
        dartforge_frontend::ast::ExprKind::String(lit) => lit.constant_value().is_some_and(|t| t.as_bytes() == text.as_bytes()),
        _ => false,
    })
}

/// The `vm:external-name`/`vm:recognized` pragmas of a function: the name of the
/// native, if there is one, and whether it is an intrinsic. The pragma lives on the `Member`/`Decl`
/// that declares the function (the same one `natives::inventory` reads).
pub fn native_pragma(
    program: &Program,
    interner: &Interner,
    fid: usize,
) -> (Option<String>, bool) {
    use dartforge_elements::model::FunctionRef;
    use dartforge_frontend::ast;
    let f = &program.functions[fid];
    let (unit, metadata): (_, &[ast::Annotation]) = match f.node {
        FunctionRef::Function { unit, function } => {
            let a = &program.unit(unit).ast;
            let m = a
                .members
                .iter()
                .find(|m| matches!(m.kind, ast::MemberKind::Method(id) if id == function))
                .map(|m| &*m.metadata)
                .or_else(|| {
                    a.decls
                        .iter()
                        .find(|d| matches!(d.kind, ast::DeclKind::Function(id) if id == function))
                        .map(|d| &*d.metadata)
                })
                .unwrap_or(&[]);
            (unit, m)
        }
        FunctionRef::Constructor { unit, member } => (unit, &*program.unit(unit).ast.members[member.0 as usize].metadata),
        FunctionRef::None => return (None, false),
    };
    let mut native = None;
    let mut recognized = false;
    for an in metadata {
        if an.name.len() != 1 || interner.resolve(an.name[0].sym) != "pragma" {
            continue;
        }
        let Some(args) = &an.arguments else { continue };
        let strings: Vec<Option<String>> = args
            .args
            .iter()
            .map(|a| match &program.unit(unit).ast.exprs[a.value.0 as usize].kind {
                ast::ExprKind::String(lit) => {
                    lit.constant_value().map(|t| String::from_utf8_lossy(t.as_bytes()).into_owned())
                }
                _ => None,
            })
            .collect();
        match strings.first().and_then(|t| t.as_deref()) {
            Some("vm:external-name") => native = strings.get(1).cloned().flatten(),
            Some("vm:recognized") => recognized = true,
            _ => {}
        }
    }
    (native, recognized)
}

/// The symbol belongs to a function of an SDK library (`df.dart$3a…`, the prefix
/// the stable path gives every `dart:` function and its closures and
/// entries).
pub fn sdk_symbol(sym_name: &str) -> bool {
    sym_name.starts_with("df.dart$3a")
}

/// Prunes the module starting from what belongs to the program: every non-SDK function
/// stays, and every SDK function it reaches (transitively) too. The
/// diagnostics of a source function (`source_errors`, by the symbol of the
/// top-level function that produced them) only go into `errors` if it stays.
pub fn prune(module: &mut Module) {
    let index: HashMap<&str, usize> =
        module.functions.iter().enumerate().map(|(i, f)| (f.symbol.as_str(), i)).collect();
    let mut live = vec![false; module.functions.len()];
    let mut stack: Vec<usize> = Vec::new();
    let mark = |s: &str, live: &mut Vec<bool>, stack: &mut Vec<usize>| {
        if let Some(&i) = index.get(s)
            && !live[i]
        {
            live[i] = true;
            stack.push(i);
        }
    };
    // Roots: the program functions; the RTI recipe getters
    // (`df.rti.*`) only stay if someone calls them.
    for f in &module.functions {
        if !sdk_symbol(&f.symbol) && !f.symbol.starts_with("df.rti.") {
            mark(&f.symbol, &mut live, &mut stack);
        }
    }
    for c in &module.classes {
        if let Some(s) = &c.to_string_symbol {
            mark(s, &mut live, &mut stack);
        }
        for (_, s) in &c.vtable {
            mark(s, &mut live, &mut stack);
        }
    }
    for s in &module.source_roots {
        mark(s, &mut live, &mut stack);
    }
    while let Some(i) = stack.pop() {
        for b in &module.functions[i].blocks {
            for (_, inst, _) in &b.instructions {
                match inst {
                    Instruction::CallStatic { symbol, .. } => mark(symbol, &mut live, &mut stack),
                    Instruction::AllocClosure { code_symbol, .. } | Instruction::TearOff { code_symbol } => {
                        mark(code_symbol, &mut live, &mut stack)
                    }
                    Instruction::AllocClosureTyped { code_symbol, typed, .. } => {
                        mark(code_symbol, &mut live, &mut stack);
                        mark(typed, &mut live, &mut stack);
                    }
                    Instruction::LoadGlobal { symbol: sym_name, .. } | Instruction::StoreGlobal { symbol: sym_name, .. } => {
                        mark(sym_name, &mut live, &mut stack)
                    }
                    _ => {}
                }
            }
        }
    }
    let live_set: HashSet<String> = module
        .functions
        .iter()
        .zip(&live)
        .filter(|(_, v)| **v)
        .map(|(f, _)| f.symbol.clone())
        .collect();
    let mut k = 0;
    module.functions.retain(|_| {
        let v = live[k];
        k += 1;
        v
    });
    module.dart_scalar_parameters.retain(|sym_name, _| live_set.contains(sym_name));
    module.dart_rti_parameters.retain(|sym_name, _| live_set.contains(sym_name));
    module.dart_vector_parameters.retain(|sym_name, _| live_set.contains(sym_name));
    module.arc_uniform_returns.retain(|sym_name| live_set.contains(sym_name));
    module.arc_shared_functions.retain(|sym_name| live_set.contains(sym_name));
    module.dart_ref_returns.retain(|sym_name| live_set.contains(sym_name));
    module.arc_sites.retain(|sym_name, _| live_set.contains(sym_name));
    for (owner, errors) in std::mem::take(&mut module.source_errors) {
        if live_set.contains(&owner) {
            module.errors.extend(errors);
        }
    }
}
