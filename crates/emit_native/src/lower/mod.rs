//! Lowering of the AST and type tables to the native HIR.
//!
//! The contract between this lowering, the emitter and the runtime is in
//! `docs/NATIVO-PLANO.md` §6 (R, E, N, G).

pub mod async_sm;
pub mod assignment;
/// The inline members of boxes, closures and records (P2 of the unified space).
pub mod boxes;
pub mod capture;
pub mod cascade;
pub mod calls;
pub mod closures;
pub mod statements;
pub mod primitive_const;
pub mod constants;
mod environment;
mod double_identity;
mod primitive_multiply;
mod primitive_clamp;
mod platform;
mod native_pump;
mod sync_native_callable;
mod runtime_policy;
pub mod dispatch;
pub mod typed_entry;
pub mod enums;
pub mod runtime_errors;
pub mod expressions;
pub mod extensions;
pub mod ffi;
pub mod externals;
pub mod fn_builder;
pub mod direct_functions;
pub mod inheritance;
/// The inline members of `_List`/`_GrowableList` (P3 of the unified space).
pub mod lists;
pub mod literals;
pub mod locals;
pub mod members;
mod layouts_arc;
pub mod operators;
pub mod patterns;
pub mod records;
pub mod rti;
pub mod sdk_source;
pub mod intrinsics;
pub mod simd;
pub mod strings;
pub mod typed_items;
pub mod extension_types;
pub mod verifier;

use crate::context::Context;
use crate::hir::*;
use dartforge_elements::model::{FunctionKind, FunctionRef, VariableId, VariableRef};
use dartforge_frontend::ast::{FunctionBody, MemberKind};

pub fn sanitize_symbol(name: &str) -> String {
    name.chars().map(|c| match c {
        '+' => "_plus_".to_string(),
        '-' => "_minus_".to_string(),
        '*' => "_mul_".to_string(),
        '/' => "_div_".to_string(),
        '%' => "_mod_".to_string(),
        '~' => "_tilde_".to_string(),
        '<' => "_lt_".to_string(),
        '>' => "_gt_".to_string(),
        '=' => "_eq_".to_string(),
        '!' => "_excl_".to_string(),
        '[' => "_lbr_".to_string(),
        ']' => "_rbr_".to_string(),
        '&' => "_amp_".to_string(),
        '|' => "_pipe_".to_string(),
        '^' => "_caret_".to_string(),
        c if c.is_ascii_alphanumeric() || c == '_' => c.to_string(),
        _ => format!("_x{:02x}_", c as u32),
    }).collect()
}

/// Does the function belong to the user program (not to the SDK)?
///
/// With the `vm` section really loaded, `Program::functions` has the whole
/// `dart:core`. The native backend does not compile the SDK from source
/// (yet — `docs/NATIVO-PLANO.md` §4, item 7): the Rust runtime implements
/// what the corpus uses. So only user functions become symbols, and a
/// call to an SDK function that has no implementation in the runtime is a
/// diagnostic (N1), not a `call` to a symbol that does not exist.
pub fn user_function(ctx: &Context, fid: usize) -> bool {
    let f = &ctx.program.functions[fid];
    ctx.compiled_library(f.library)
}

/// The function is a generative constructor (the synthetic one included): it returns
/// nothing and receives an already allocated `this` (N5).
pub fn generative_constructor(ctx: &Context, fid: usize) -> bool {
    let f = &ctx.program.functions[fid];
    match f.node {
        // That of an extension type returns the representation, like a factory
        // (`extension_types.rs`).
        FunctionRef::Constructor { .. } => !f.factory && !f.class.is_some_and(|c| ctx.is_extension_type(c)),
        FunctionRef::None => f.kind == FunctionKind::SyntheticConstructor,
        FunctionRef::Function { .. } => false,
    }
}

/// The owner of a member in the symbol: the class, the extension (`ext:<name>`) or
/// empty for the top level.
fn owner_in_symbol(ctx: &Context, class: Option<dartforge_elements::model::ClassId>, ext: Option<dartforge_elements::model::ExtensionId>) -> String {
    if let Some(c) = class {
        return ctx.symbol_name(ctx.program.classes[c.0 as usize].name).to_string();
    }
    if let Some(e) = ext {
        let x = ctx.program.extension(e);
        return match x.name {
            Some(n) => format!("ext:{}", ctx.symbol_name(n)),
            None => format!("ext:@{}", x.decl.decl.0),
        };
    }
    String::new()
}

/// Stable symbol of a program function (P2, docs/NATIVO-PLANO.md
/// §7.4): `df.<library>.<owner>.<member>`, derived from the **path** of the
/// declaration — never from internal indices of the front end —, with each part
/// escaped (`context::escape_str`). The member is the name; a setter ends with
/// `=`; a constructor is `new` or `new:<name>`. The `main` of the entry
/// library is `dart_main`. Inserting or reordering declarations does not change the
/// symbol of any other: it is the key of the per-library object cache and of
/// reload (hot reload R1).
pub fn symbol_of(ctx: &Context, fid: usize) -> String {
    let f = &ctx.program.functions[fid];
    let name_str = ctx.symbol_name(f.name);
    if f.class.is_none() && f.extension.is_none() && name_str == "main" && Some(f.library) == ctx.entry_lib {
        return "dart_main".to_string();
    }
    let member = match f.kind {
        FunctionKind::Constructor | FunctionKind::SyntheticConstructor => {
            if name_str.is_empty() { "new".to_string() } else { format!("new:{name_str}") }
        }
        FunctionKind::Setter if !name_str.ends_with('=') => format!("{name_str}="),
        _ => name_str.to_string(),
    };
    format!(
        "df.{}.{}.{}",
        crate::context::escape_str(&ctx.library_name(f.library)),
        crate::context::escape_str(&owner_in_symbol(ctx, f.class, f.extension)),
        crate::context::escape_str(&member)
    )
}

/// Top-level variable or static field: lives in a module global (N6).
pub fn is_global(ctx: &Context, vid: VariableId) -> bool {
    let v = &ctx.program.variables[vid.0 as usize];
    // Extension fields are always static (§13): globals like those of a
    // class.
    (v.class.is_none() || v.static_)
        && (v.extension.is_none() || v.static_)
        && matches!(
            v.node,
            VariableRef::TopLevel { .. } | VariableRef::Field { .. } | VariableRef::EnumConstant { .. }
        )
}

/// Stable path of a top-level variable or static field (P2).
fn variable_path(ctx: &Context, vid: VariableId) -> String {
    let v = &ctx.program.variables[vid.0 as usize];
    format!(
        "{}.{}.{}",
        crate::context::escape_str(&ctx.library_name(v.library)),
        crate::context::escape_str(&owner_in_symbol(ctx, v.class, v.extension)),
        crate::context::escape_str(ctx.symbol_name(v.name))
    )
}

/// Lazy getter of a global: `df.<path>`.
pub fn global_symbol(ctx: &Context, vid: VariableId) -> String {
    format!("df.{}", variable_path(ctx, vid))
}

/// The value of a global: `dfg.<path>` (the flag is `<value>$ok`).
pub fn global_value_symbol(ctx: &Context, vid: VariableId) -> String {
    format!("dfg.{}", variable_path(ctx, vid))
}

/// Dedicated getter of a `late` instance field with an initializer.
pub fn late_field_getter_symbol(ctx: &Context, vid: VariableId) -> String {
    format!("df.late.{}", variable_path(ctx, vid))
}

pub fn lower_program(ctx: &Context) -> Module {
    let mut module = Module::new();
    module.arc_memory = ctx.arc_memory;
    module.checked_double_identity = double_identity::certified(ctx);

    // 1. Registers known classes
    // Default Object class as id 0
    module.classes.push(ClassDef {
        id: 0,
        name: "Object".to_string(),
        field_count: 0,
        vtable: Vec::new(),
        to_string_symbol: None,
    });
    module.pruning_metadata_only_class_ids.insert(0);

    // The SDK always comes from source (docs/NATIVO-ESPACO-UNIFICADO.md §4.7): the
    // error classes are its own (P5c).
    module.sdk_mode = true;
    module.sdk_library = ctx.sdk_library;
    module.dwarf = ctx.dwarf;
    let mut module = lower_classes_and_functions(ctx, module);
    // Body chunks and adapters start uncertified. Publish the premise only
    // after joining every body from this complete selected source context.
    module.primitive_clamp_certificate = primitive_clamp::enabled_certificate(ctx);
    if module.primitive_clamp_certificate {
        module.primitive_clamp_targets = primitive_clamp::targets(ctx);
    }
    module
}

/// The `dart:async` fields that the symbolic stack trace walks to find who
/// awaits (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14; the
/// `USED_FIELD_LIST` of the VM's `AsyncAwareStackUnwinder`), when this module
/// has their classes: the position of each one in the object.
fn trace_fields(ctx: &Context, module: &mut Module) {
    if !ctx.stack_trace {
        return;
    }
    const FIELDS: &[(&str, &str)] = &[
        ("_AsyncAwaitCompleter", "_future"),
        ("_AsyncCompleter", "future"),
        ("_SyncCompleter", "future"),
        ("_Future", "_resultOrListeners"),
        ("_Future", "_state"),
        ("_FutureListener", "state"),
        ("_FutureListener", "callback"),
        ("_FutureListener", "result"),
        ("_FutureListener", "_nextListener"),
        ("_AsyncStarStreamController", "asyncStarBody"),
        ("_AsyncStarStreamController", "controller"),
        ("_BufferingStreamSubscription", "_onData"),
        ("_BufferingStreamSubscription", "_onDone"),
        ("_BufferingStreamSubscription", "_onError"),
        ("_BufferingStreamSubscription", "_state"),
        ("_StreamController", "_state"),
        ("_StreamController", "_varData"),
        ("_StreamControllerAddStreamState", "_varData"),
        ("_StreamIterator", "_hasValue"),
        ("_StreamIterator", "_stateData"),
        ("_AddStreamState", "addStreamFuture"),
    ];
    // The runtime finds the field by the exact class of the object: the declaring one and its
    // subclasses (the `_SyncStreamController` of a `_StreamController`,
    // the `_ControllerSubscription` of a `_BufferingStreamSubscription`),
    // like the VM's `Instance::GetField`.
    for (class, field) in FIELDS {
        let Some(declarer) = ctx.sdk_class("async", class) else { continue };
        for (k, _) in ctx.program.classes.iter().enumerate() {
            let c = dartforge_elements::model::ClassId(k as u32);
            let mut ancestor = Some(c);
            let mut descends = false;
            for _ in 0..64 {
                match ancestor {
                    Some(x) if x == declarer => {
                        descends = true;
                        break;
                    }
                    Some(x) => ancestor = ctx.program.class(x).supertype_class,
                    None => break,
                }
            }
            if !descends || !ctx.library_in_module(ctx.program.class(c).library) {
                continue;
            }
            let Some(id) = ctx.class_id_of(c) else { continue };
            let Some(i) = members::layout(ctx, c).iter().position(|&v| ctx.symbol_name(ctx.program.variables[v.0 as usize].name) == *field) else {
                continue;
            };
            module.trace_fields.push((id, format!("{class}.{field}"), i + enums::layout_base(ctx, c)));
        }
    }
}

/// The tear-off entries of the stream branches of the stack trace (§13.14) that this
/// module defined: `_StreamIterator._onData` (the `await for`) and
/// `_StreamController._add` (the `yield*`).
fn stack_trace_tearoffs(ctx: &Context, module: &mut Module) {
    if !ctx.stack_trace {
        return;
    }
    const TEAROFFS: &[(&str, &str, i64)] = &[("_StreamIterator", "_onData", 1), ("_StreamController", "_add", 2)];
    for (class, method, kind) in TEAROFFS {
        let Some(c) = ctx.sdk_class("async", class) else { continue };
        let Some(name_str) = ctx.interner.lookup(method) else { continue };
        let Some(&fid) = ctx.program.class(c).instance_members.get(&name_str) else { continue };
        let symbol = format!("{}$tearm", symbol_of(ctx, fid.0 as usize));
        if module.functions.iter().any(|f| f.symbol == symbol) {
            module.stack_trace_tearoffs.push((symbol, *kind));
        }
    }
}

/// The declared ARC layouts of the classes of the source SDK (those this module
/// does not emit), as the module of each SDK library registers them
/// (`layouts_arc::register`): the `arc_external_layouts` of the program
/// prepared against the prepared SDK.
pub fn sdk_arc_layouts(ctx: &Context) -> std::collections::HashMap<u32, Vec<Type>> {
    let mut m = Module::new();
    for (c_idx, class) in ctx.program.classes.iter().enumerate() {
        if ctx.library_in_module(class.library) || !ctx.program.library(class.library).is_sdk {
            continue;
        }
        let cid = dartforge_elements::model::ClassId(c_idx as u32);
        let Some(class_id) = ctx.class_id_of(cid) else { continue };
        layouts_arc::register(ctx, &mut m, class_id, &members::layout(ctx, cid), enums::layout_base(ctx, cid));
    }
    m.arc_field_layouts
}

#[cfg(test)]
pub(crate) fn class_registration_facts(ctx: &Context) -> Module {
    let mut module = Module::new();
    register_program_classes(ctx, &mut module);
    module
}

// Shared with the audit of the layout facts, without lowering bodies.
fn register_program_classes(ctx: &Context, module: &mut Module) {
    // Zero is the terminal universal parent used by the subtype registration
    // below, not an allocated nominal CID (fixed CIDs start at one). Missing
    // real parents remain open; this synthetic root adds no unknown ancestry.
    module.pruning_class_ids.insert(0);
    let mut counts = std::collections::HashMap::new();
    for cid in ctx.class_ids.iter().flatten() {
        *counts.entry(*cid).or_insert(0usize) += 1;
    }
    module.pruning_ambiguous_class_ids.extend(counts.into_iter().filter_map(|(cid, count)| (count > 1).then_some(cid)));
    for (c_idx, class) in ctx.program.classes.iter().enumerate() {
        // Registers the classes of the libraries emitted in this module. The declared
        // layout of the SDK does not certify the physical shape used by the runtime.
        if !ctx.library_in_module(class.library) {
            continue;
        }
        let name = ctx.symbol_name(class.name).to_string();
        // Stable id (P2): the order of the path of the class, not the load order.
        let Some(class_id) = ctx.class_id_of(dartforge_elements::model::ClassId(c_idx as u32)) else {
            continue;
        };
        let cid = dartforge_elements::model::ClassId(c_idx as u32);
        if class_id == 0 || class.modifiers.abstract_ || members::is_mixin(ctx, cid) {
            module.pruning_metadata_only_class_ids.insert(class_id);
        }
        let fields = members::layout(ctx, cid);
        let base = enums::layout_base(ctx, cid);
        layouts_arc::register(ctx, module, class_id, &fields, base);
        if class_id >= dartforge_runtime::layout::FIRST_FREE_CID as u32
            && !module.pruning_ambiguous_class_ids.contains(&class_id)
            && !ctx.program.class_with_homonym(cid)
        {
            let mut slots = vec![None; base];
            slots.extend(fields.iter().enumerate().map(|(index, &field)|
                members::declared_field_metadata(ctx, field, base + index)));
            if let std::collections::hash_map::Entry::Vacant(e) = module.declared_field_layouts.entry(class_id) {
                e.insert(slots);
            } else {
                module.declared_field_layouts.remove(&class_id);
                module.pruning_ambiguous_class_ids.insert(class_id);
            }
        }
        module.classes.push(ClassDef {
            id: class_id,
            name,
            field_count: fields.len() + base,
            vtable: Vec::new(),
            to_string_symbol: None,
        });

        // Registers subtyping edges
        if class.supertype_class.iter().chain(class.mixin_classes.iter())
            .chain(class.interface_classes.iter()).all(|&parent| ctx.class_id_of(parent).is_some())
        {
            module.pruning_class_ids.insert(class_id);
        }
        module.subtyping_edges.push((class_id, 0)); // Every nominal type inherits from Object
        if let Some(sup) = class.supertype_class.and_then(|s| ctx.class_id_of(s)) {
            module.subtyping_edges.push((class_id, sup));
        }
        // A program enum is a subtype of the SDK `Enum` (specification §13):
        // without the edge, `value is Enum` answered false. The superclass of the
        // outline does not carry `Enum`, so the edge is registered here.
        if !ctx.program.library(class.library).is_sdk
            && enums::is_enum(ctx, dartforge_elements::model::ClassId(c_idx as u32))
            && let Some(enum_sdk) = ctx.sdk_class("core", "Enum")
            && let Some(enum_id) = ctx.class_id_of(enum_sdk)
        {
            module.subtyping_edges.push((class_id, enum_id));
        }
        for mix in &class.mixin_classes {
            if let Some(m) = ctx.class_id_of(*mix) {
                module.subtyping_edges.push((class_id, m));
            }
        }
        for iface in &class.interface_classes {
            if let Some(i) = ctx.class_id_of(*iface) {
                module.subtyping_edges.push((class_id, i));
            }
        }
    }
}

fn lower_classes_and_functions(ctx: &Context, mut module: Module) -> Module {
    trace_fields(ctx, &mut module);
    register_program_classes(ctx, &mut module);

    // 2. User functions
    let functions: Vec<usize> = (0..ctx.program.functions.len())
        .filter(|&f| ctx.library_in_module(ctx.program.functions[f].library))
        .collect();
    lower_functions(ctx, &mut module, &functions);
    sdk_source::lower_adapters_and_tables(ctx, &mut module);
    let mut module = lower_globals_and_rest(ctx, module);
    sdk_source::record_shape_tables(ctx, &mut module);
    if !ctx.sdk_library {
        ffi::lower_ffi(ctx, &mut module);
    }
    stack_trace_tearoffs(ctx, &mut module);
    crate::optimize::arc::analysis::origins::register(&mut module, 0, ctx.arc_memory);
    module
}

/// Lowers a function of the module: one of the source SDK (P5c) is rejected on its own,
/// with the reason, when it does not lower (`sdk_source.rs`).
fn lower_one(ctx: &Context, module: &mut Module, f_idx: usize) {
    if ctx.program.library(ctx.program.functions[f_idx].library).is_sdk {
        sdk_source::lower_function_or_reject(ctx, module, f_idx);
    } else {
        lower_function(ctx, module, f_idx);
    }
}

/// How many threads lower the functions: the machine's, at most 4
/// (`DARTFORGE_THREADS_HIR`; 1 = sequential).
fn lowering_threads() -> usize {
    std::env::var("DARTFORGE_THREADS_HIR")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get().min(4)))
        .max(1)
}

/// Lowers the functions `functions` into `module`. A large program goes in
/// contiguous chunks, one per thread, each one into its own module, joined
/// in order: the result is that of the sequential order (the same IR, the same
/// keys in the object cache). Each function only writes into the module its own
/// functions, globals, errors and rejections, the `toString` of its class and the
/// entry (`main`); the context only has query memos.
fn lower_functions(ctx: &Context, module: &mut Module, functions: &[usize]) {
    let n = lowering_threads();
    if n == 1 || functions.len() < 4096 {
        for &f in functions {
            lower_one(ctx, module, f);
        }
        return;
    }
    // Small chunks distributed on demand (the expensive functions
    // cluster), each into its own module; the join follows the order of the chunks.
    let sdk_mode = module.sdk_mode;
    let sdk_library = module.sdk_library;
    let classes = &module.classes;
    let chunks: Vec<&[usize]> = functions.chunks(512).collect();
    let done_set: Vec<std::sync::Mutex<Option<Module>>> = chunks.iter().map(|_| std::sync::Mutex::new(None)).collect();
    let next_item = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        let tasks: Vec<_> = (0..n)
            .map(|_| {
                let (chunks, done_set, next_item) = (&chunks, &done_set, &next_item);
                std::thread::Builder::new()
                    // The same stack as the compilation thread (`lib.rs`).
                    .stack_size(64 << 20)
                    .spawn_scoped(s, move || loop {
                        let i = next_item.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(chunk) = chunks.get(i) else { break };
                        let mut m = Module::new();
                        m.sdk_mode = sdk_mode;
                        m.sdk_library = sdk_library;
                        m.classes = classes.clone();
                        for &f in chunk.iter() {
                            lower_one(ctx, &mut m, f);
                        }
                        *done_set[i].lock().unwrap_or_else(|e| e.into_inner()) = Some(m);
                    })
                    .expect("lowering thread")
            })
            .collect();
        for t in tasks {
            if let Err(p) = t.join() {
                std::panic::resume_unwind(p);
            }
        }
    });
    let parts = done_set.into_iter().map(|m| m.into_inner().unwrap_or_else(|e| e.into_inner()).expect("lowered chunk"));
    for mut m in parts {
        module.functions.extend(std::mem::take(&mut m.functions));
        join_arc_facts(module, &mut m);
        module.globals.extend(m.globals);
        module.errors.extend(m.errors);
        module.source_errors.extend(m.source_errors);
        module.rejected.extend(m.rejected);
        module.helpers.extend(m.helpers);
        if m.entry_symbol.is_some() {
            module.entry_symbol = m.entry_symbol;
            module.entry_params = m.entry_params;
        }
        for (c, d) in module.classes.iter_mut().zip(m.classes.iter()) {
            if c.to_string_symbol.is_none() && d.to_string_symbol.is_some() {
                c.to_string_symbol.clone_from(&d.to_string_symbol);
            }
        }
    }
}

/// P6: the symbols of the source functions that have a body — an `external` whose
/// patch the loader did not bind (`patched_by` empty, NATIVO-PEDIDOS) has the
/// same symbol as the patch member, and the patch is what counts. Computed once
/// per context.
fn with_source_body<'c>(ctx: &'c Context) -> &'c std::collections::HashSet<String> {
    ctx.with_source_body.get_or_init(|| {
        ctx.program
            .functions
            .iter()
            .enumerate()
            .filter(|(i, f)| ctx.from_source.contains(&f.library) && members::has_body(ctx, *i))
            .map(|(i, _)| symbol_of(ctx, i))
            .collect()
    })
}

/// Joins into `dest` the ARC facts that the lowering of `origin` produced (by
/// symbol and by SSA ID of the same function): scalar/RTI/vector parameters,
/// Ref and uniform returns, RTI caches, shared and direct local functions,
/// and allocation sites. A separately lowered chunk that loses any of
/// them changes the ARC convention of the function silently — the Ref return without
/// `dart_ref_returns` was no longer Owned.
pub(crate) fn join_arc_facts(dest: &mut Module, origin: &mut Module) {
    dest.async_frame_layouts.append(&mut origin.async_frame_layouts);
    dest.primitive_clamp_certificate &= origin.primitive_clamp_certificate
        && dest.primitive_clamp_targets == origin.primitive_clamp_targets;
    if !dest.primitive_clamp_certificate { dest.primitive_clamp_targets.clear(); }
    dest.pruning_this_receivers.extend(std::mem::take(&mut origin.pruning_this_receivers));
    dest.pruning_class_ids.extend(std::mem::take(&mut origin.pruning_class_ids));
    dest.pruning_metadata_only_class_ids.extend(std::mem::take(&mut origin.pruning_metadata_only_class_ids));
    dest.pruning_ambiguous_class_ids.extend(std::mem::take(&mut origin.pruning_ambiguous_class_ids));
    dest.inline_policies.extend(std::mem::take(&mut origin.inline_policies));
    for (getter, premise) in std::mem::take(&mut origin.simple_enum_constants) {
        match dest.simple_enum_constants.entry(getter) {
            std::collections::hash_map::Entry::Vacant(entry) => { entry.insert(premise); }
            // Empty getter is a permanent ambiguity tombstone. A third chunk
            // cannot revive a declaration whose body ownership already collided.
            std::collections::hash_map::Entry::Occupied(mut entry) => entry.get_mut().getter.clear(),
        }
    }
    dest.dart_scalar_parameters.extend(std::mem::take(&mut origin.dart_scalar_parameters));
    dest.dart_rti_parameters.extend(std::mem::take(&mut origin.dart_rti_parameters));
    dest.dart_vector_parameters.extend(std::mem::take(&mut origin.dart_vector_parameters));
    dest.arc_uniform_returns.extend(std::mem::take(&mut origin.arc_uniform_returns));
    dest.arc_rti_caches.extend(std::mem::take(&mut origin.arc_rti_caches));
    dest.arc_shared_functions.extend(std::mem::take(&mut origin.arc_shared_functions));
    dest.arc_direct_local_functions.extend(std::mem::take(&mut origin.arc_direct_local_functions));
    dest.dart_ref_returns.extend(std::mem::take(&mut origin.dart_ref_returns));
    dest.arc_sites.extend(std::mem::take(&mut origin.arc_sites));
    dest.dart_scalar_globals.extend(std::mem::take(&mut origin.dart_scalar_globals));
}

/// Lowers a function (top-level, method, constructor) into the module.
pub fn lower_function(ctx: &Context, module: &mut Module, f_idx: usize) {
    let start = module.functions.len();
    let symbol = symbol_of(ctx, f_idx);
    lower_one_function(ctx, module, f_idx);
    // The patch supplies the body, while the public SDK declaration can supply
    // the request. A prohibition wins if both declarations carry a request.
    let public = ctx.program.public(dartforge_elements::model::FunctionElementId(f_idx as u32)).0 as usize;
    let has = |name| [f_idx, public].into_iter().any(|f|
        crate::source::has_pragma(ctx.program, ctx.interner, f, name));
    // `vm:never-inline` on a `vm:recognized asm-intrinsic` member (the
    // operators of `_Double` and `_IntegerImplementation`) exists because the VM
    // replaces the call by the intrinsic before its inliner runs; nothing
    // replaces the body here, and honoring it only kept `_Double.*`, `<`, `==`…
    // out of the HIR inliner and `noinline` in LLVM.
    let asm_intrinsic = [f_idx, public].into_iter().any(|f|
        crate::natives::recognized_kind(ctx.program, ctx.interner, &ctx.program.functions[f]).as_deref() == Some("asm-intrinsic"));
    let policy = if has("vm:never-inline") && !asm_intrinsic { Some(crate::hir::InlinePolicy::Never) }
        else if has("vm:prefer-inline") { Some(crate::hir::InlinePolicy::Prefer) }
        else { None };
    if let Some(policy) = policy {
        for f in &module.functions[start..] {
            if f.symbol == symbol || f.symbol.strip_prefix(&format!("{symbol}$async$q"))
                .is_some_and(|suffix| suffix.len() == 8 && suffix.bytes().all(|b| b.is_ascii_hexdigit())) {
                module.inline_policies.insert(f.symbol.clone(), policy);
            }
        }
    }
    crate::optimize::arc::analysis::origins::register(module, start, ctx.arc_memory);
    // The convention belongs to the Dart body identified by the front end, not to the
    // width of returns of thunks/FFI or of extra functions created in the body.
    if ctx.arc_memory
        && module.functions[start..].iter().any(|f| f.symbol == symbol && f.return_ty == Type::Ref)
    {
        module.dart_ref_returns.insert(symbol);
    }
}

fn lower_one_function(ctx: &Context, module: &mut Module, f_idx: usize) {
    let func_elem = &ctx.program.functions[f_idx];
    if func_elem.external {
        // `external` (constructor included): the body is that of the patch or the
        // native (`sdk_source::call_external`); nothing to lower here — an
        // empty body with the same symbol would take the place of the patch.
        return;
    }
    {
        let name = ctx.symbol_name(func_elem.name);
        let symbol = symbol_of(ctx, f_idx);

        match func_elem.node {
            FunctionRef::Function { unit, function } => {
                let ast = &ctx.program.unit(unit).ast;
                let ast_func = ast.function(function);
                if matches!(ast_func.body, FunctionBody::Empty | FunctionBody::Native(_)) {
                    // P6: an `external` of a source library with a native — the
                    // body is the call to the runtime (`externals.rs`).
                    if func_elem.external
                        && func_elem.patched_by.is_none()
                        && ctx.from_source.contains(&func_elem.library)
                        && !with_source_body(ctx).contains(&symbol)
                    {
                        let ret_ty = ctx
                            .outline
                            .functions
                            .get(f_idx)
                            .map_or(Type::Void, |d| ctx.to_hir_type(d.return_type));
                        let mut builder = fn_builder::FnBuilder::new(ctx, unit, symbol, name.to_string(), ret_ty);
                        let is_instance = func_elem.class.is_some() && !func_elem.static_;
                        builder.declare_parameters(f_idx, is_instance);
                        if !builder.lower_external(f_idx, ast_func.span) {
                            builder.unsupported(&format!("external `{name}` without a native"), ast_func.span);
                            builder.terminate(Terminator::Return(None));
                        }
                        builder.finish(module);
                    }
                    // Abstract or external: there is no body to compile.
                    return;
                }

                let is_main = symbol == "dart_main";
                if is_main {
                    module.entry_symbol = Some(symbol.clone());
                    module.entry_params = ast_func.parameters.as_deref().map_or(0, <[_]>::len);
                }

                if let Some(c_id) = func_elem.class
                    && name == "toString"
                    && !func_elem.static_
                    && let Some(id) = ctx.class_id_of(c_id)
                    && let Some(c) = module.classes.iter_mut().find(|c| c.id == id)
                {
                    c.to_string_symbol = Some(symbol.clone());
                }

                let ret_ty = if is_main {
                    Type::Void
                } else if ctx.outline.functions.get(f_idx).is_some() {
                    ctx.hir_return(f_idx)
                } else {
                    Type::Void
                };

                let mut builder = fn_builder::FnBuilder::new(ctx, unit, symbol, name.to_string(), ret_ty);
                // Only this versioned package declaration has a native body;
                // other libraries and unrecognized versions retain their Dart fallback.
                if ast_func.type_params.is_empty()
                    && (builder.lower_native_pump_contract(f_idx)
                        || builder.lower_sync_callback_support(f_idx)
                        || builder.lower_runtime_policy(f_idx))
                {
                    builder.finish(module);
                    return;
                }
                // The symbolic stack trace (§13.14): the token of the function, the hidden
                // supports of `async_patch` and `_FutureListener.handleValue`,
                // whose listener starts the chain of awaiters.
                if ctx.stack_trace {
                    builder.stack_trace.token = ast_func.name.and_then(|n| ctx.line_and_column(unit, n.span.start));
                    if crate::source::has_pragma(ctx.program, ctx.interner, f_idx, "dartforge:hidden-frame") {
                        builder.stack_trace.marks |= crate::hir::stack_trace_marks::HIDDEN;
                    }
                    if name == "handleValue"
                        && ctx.program.library(func_elem.library).uri == "dart:async"
                        && func_elem.class.is_some_and(|c| ctx.symbol_name(ctx.program.class(c).name) == "_FutureListener")
                    {
                        builder.stack_trace.marks |= crate::hir::stack_trace_marks::LISTEN;
                    }
                }
                // The capture analysis walks the whole body; the pruned
                // body (C7) does not use it.
                if !ctx.pruned_function(f_idx) {
                    builder.prepare_captures(
                        ast,
                        capture::Root {
                            parameters: ast_func.parameters.as_deref().unwrap_or(&[]),
                            body: Some(&ast_func.body),
                            initializers: &[],
                        },
                    );
                }
                let is_instance_member = func_elem.class.is_some() && !func_elem.static_;
                if let Some(e) = func_elem.extension.filter(|_| !func_elem.static_) {
                    // Extension instance member (P4): the receiver is the
                    // first parameter, in the representation of the `on` type (with
                    // a scalar parameter fact when it is scalar).
                    let r = builder.extension_receiver_repr(e);
                    let this = builder.add_param_dart("this".to_string(), r);
                    builder.this_param = Some(Operand::Val(this));
                    builder.finalizable_this = ctx.classify_this(f_idx);
                    builder.declare_parameters(f_idx, false);
                    let on = ctx.outline.extensions[e.0 as usize].on;
                    builder.this_extension = Some((e, on));
                    if let dartforge_types::table::Type::Interface { class, .. } = ctx.table.get(on)
                        && ctx.compiled_library(ctx.program.classes[class.0 as usize].library)
                    {
                        builder.enclosing_class = Some(*class);
                    }
                } else if let Some(c) = extension_types::instance_owner(ctx, f_idx) {
                    // Extension type instance member: the receiver is the
                    // first parameter, in the representation; the type
                    // parameters of the type come in the tuple (`extension_types.rs`).
                    let r = builder.static_this_repr(f_idx).unwrap_or(Type::Ref);
                    let this = builder.add_param_dart("this".to_string(), r);
                    builder.this_param = Some(Operand::Val(this));
                    builder.finalizable_this = ctx.classify_this(f_idx);
                    builder.declare_parameters(f_idx, false);
                    builder.this_ext_type = ctx.te.this.get(&c).map(|&t| (c, t));
                } else {
                    builder.declare_parameters(f_idx, is_instance_member);
                }
                if is_instance_member && extension_types::instance_owner(ctx, f_idx).is_none() {
                    builder.enclosing_class = func_elem.class;
                }
                builder.member_class = func_elem.class;
                if builder.stack_trace.marks & crate::hir::stack_trace_marks::LISTEN != 0
                    && let Some(t) = builder.this_param.clone()
                {
                    builder.stack_trace.stack = Some((1, t));
                }
                // RTI: a generic function receives the tuple of the type
                // arguments in the last parameter (`M<i>` of the recipes).
                if builder.generic_function(f_idx) {
                    let t = builder.add_param_rti("$types".to_string());
                    builder.type_tuple = Some(Operand::Val(t));
                    builder.function_type_params =
                        builder.type_params_of(f_idx).iter().map(|&p| ctx.table.param(p).name).collect();
                }

                // Outside the closed world of the program (C7, `native_world.rs`):
                // the symbol stays (tables and entries cite it), the body throws.
                if ctx.pruned_function(f_idx) {
                    builder.pruned_body();
                    builder.finish(module);
                    return;
                }
                match ast_func.modifier {
                    // P6: the `async` body becomes a state machine
                    // (`async_sm.rs`); the `sync*`/`async*` generators too.
                    dartforge_frontend::ast::AsyncModifier::None => match &ast_func.body {
                        FunctionBody::Block(stmt_id) => {
                            builder.lower_stmt(ast, *stmt_id);
                        }
                        FunctionBody::Expression(expr_id) => {
                            let ret_op = builder.lower_expr(ast, *expr_id);
                            builder.terminate(Terminator::Return(Some(ret_op)));
                        }
                        _ => {}
                    },
                    modifier => {
                        builder.lower_async_body(
                            ast,
                            ast_func.parameters.as_deref().unwrap_or(&[]),
                            &ast_func.body,
                            ast_func.span,
                            ctx.outline.functions.get(f_idx).map(|d| async_sm::AsyncReturn::Typed(d.return_type)),
                            async_sm::body_type(modifier),
                        );
                    }
                }
                builder.finish(module);
            }
            FunctionRef::Constructor { unit, member } => {
                let ast = &ctx.program.unit(unit).ast;
                let MemberKind::Constructor(ctor) = &ast.member(member).kind else {
                    return;
                };
                let Some(cid) = func_elem.class else { return };
                if !func_elem.factory && ctx.is_extension_type(cid) {
                    // Generative constructor of an extension type: returns the
                    // representation (`extension_types.rs`).
                    let ret = ctx.te.this.get(&cid).map_or(Type::Ref, |&t| ctx.to_hir_type(t));
                    let mut builder = fn_builder::FnBuilder::new(ctx, unit, symbol, name.to_string(), ret);
                    builder.prepare_captures(
                        ast,
                        capture::Root {
                            parameters: &ctor.parameters,
                            body: Some(&ctor.body),
                            initializers: &ctor.initializers,
                        },
                    );
                    builder.declare_parameters(f_idx, false);
                    builder.lower_constructor_te(ast, cid, ctor, ast.member(member).span);
                    builder.finish(module);
                    return;
                }
                if func_elem.factory {
                    // The factory of an extension type returns the representation.
                    let ret = if ctx.is_extension_type(cid) {
                        ctx.te.this.get(&cid).map_or(Type::Ref, |&t| ctx.to_hir_type(t))
                    } else {
                        Type::Ref
                    };
                    let mut builder = fn_builder::FnBuilder::new(ctx, unit, symbol, name.to_string(), ret);
                    builder.prepare_captures(
                        ast,
                        capture::Root {
                            parameters: &ctor.parameters,
                            body: Some(&ctor.body),
                            initializers: &ctor.initializers,
                        },
                    );
                    builder.declare_parameters(f_idx, false);
                    // RTI: the factory of a generic class receives the type arguments
                    // of the class in the tuple (there is no `this`).
                    builder.enclosing_class = Some(cid);
                    if builder.generic_class(cid) {
                        let t = builder.add_param_rti("$types".to_string());
                        builder.type_tuple = Some(Operand::Val(t));
                        builder.class_by_tuple = true;
                    }
                    if let Some(r) = &ctor.redirect {
                        // `factory C(…) = D.name;` (P4): the parameters pass
                        // as they are (positionals in order, named ones by
                        // name) to the target constructor.
                        let span = ast.member(member).span;
                        let target = builder.type_constructor(ast.ty(r.ty), r.constructor.map(|n| n.sym));
                        match target {
                            Some(t) => {
                                let mut evaluated = Vec::new();
                                for (i, p) in ctx.outline.functions[f_idx].parameters.iter().enumerate() {
                                    let Some(sym) = p.name else { continue };
                                    let v = builder.read_local_by_name(sym).unwrap_or(Operand::Constant(Constant::Null));
                                    let name_str = (ctor.parameters.get(i).map(|q| q.kind)
                                        == Some(dartforge_frontend::ast::ParameterKind::Named))
                                    .then_some(sym);
                                    evaluated.push((name_str, v));
                                }
                                // `factory C<T>(…) = C<T>._` must hand the
                                // reified parameters to the target constructor.
                                // Without that, `MapEntry<K,V>` is born as
                                // `MapEntry<dynamic,dynamic>` even when the
                                // factory received `L<K,V>` through the ABI.
                                let target_class = ctx.program.functions[t.0 as usize].class;
                                if target_class == Some(cid) {
                                    builder.creation_type = Some(ctx.outline.functions[f_idx].return_type);
                                }
                                let (target_rti, target_tuple) = if target_class
                                    .is_some_and(|target| builder.generic_class(target))
                                {
                                    let annotation = ast.ty(r.ty);
                                    let rti = builder.annotation_recipe(annotation)
                                        .map(|recipe| builder.recipe_rti(&recipe));
                                    let tuple = builder.annotation_tuple(annotation);
                                    (rti, tuple)
                                } else {
                                    (None, None)
                                };
                                let r = builder.instantiate_evaluated_with_rti(
                                    t, &evaluated, span, target_rti, target_tuple,
                                );
                                builder.terminate(Terminator::Return(Some(r)));
                            }
                            // `factory E.x(R v) = E;` for the primary one of an
                            // extension type: the argument itself.
                            None if ctx.is_extension_type(cid)
                                && extension_types::primary_name(ctx, cid)
                                    == r.constructor.map(|n| n.sym).or_else(|| ctx.interner.lookup(""))
                                && let Some(v) = ctx.outline.functions[f_idx]
                                    .parameters
                                    .first()
                                    .and_then(|p| p.name)
                                    .and_then(|n| builder.read_local_by_name(n)) =>
                            {
                                let ret = builder.func.return_ty;
                                let v = builder.coerce(v, ret);
                                builder.terminate(Terminator::Return(Some(v)));
                            }
                            None => {
                                builder.unsupported("redirecting factory", span);
                                builder.terminate(Terminator::Return(Some(Operand::Constant(Constant::Null))));
                            }
                        }
                    } else {
                        match &ctor.body {
                            FunctionBody::Block(stmt_id) => builder.lower_stmt(ast, *stmt_id),
                            FunctionBody::Expression(expr_id) => {
                                let r = builder.lower_expr(ast, *expr_id);
                                builder.terminate(Terminator::Return(Some(r)));
                            }
                            _ => {}
                        }
                    }
                    builder.finish(module);
                } else {
                    let mut builder = fn_builder::FnBuilder::new(ctx, unit, symbol, name.to_string(), Type::Void);
                    builder.prepare_captures(
                        ast,
                        capture::Root {
                            parameters: &ctor.parameters,
                            body: Some(&ctor.body),
                            initializers: &ctor.initializers,
                        },
                    );
                    builder.declare_parameters(f_idx, true);
                    builder.enclosing_class = Some(cid);
                    builder.lower_constructor(ast, cid, ctor, ast.member(member).span);
                    builder.finish(module);
                }
            }
            FunctionRef::None => {
                // Synthetic default constructor (`class A { int x = 1; }`):
                // field initializers and the implicit `super()`.
                if func_elem.kind != FunctionKind::SyntheticConstructor {
                    return;
                }
                let Some(cid) = func_elem.class else { return };
                let Some(decl) = ctx.program.classes[cid.0 as usize].decl else { return };
                let mut builder = fn_builder::FnBuilder::new(ctx, decl.unit, symbol, name.to_string(), Type::Void);
                builder.declare_parameters(f_idx, true);
                builder.enclosing_class = Some(cid);
                let ast = &ctx.program.unit(decl.unit).ast;
                builder.lower_synthetic_constructor(ast, cid);
                builder.finish(module);
            }
        }
    }
}

fn lower_globals_and_rest(ctx: &Context, mut module: Module) -> Module {
    // The initializer cannot be expanded at each read point: a
    // self-reference must call the getter at run time and observe reentrancy.
    for (v_idx, v) in ctx.program.variables.iter().enumerate() {
        let vid = VariableId(v_idx as u32);
        let VariableRef::Field { unit, member, index } = v.node else { continue };
        if !ctx.library_in_module(v.library) || !v.late || v.static_ || v.class.is_none() {
            continue;
        }
        let MemberKind::Field(list) = &ctx.program.unit(unit).ast.member(member).kind else { continue };
        if list.variables.get(index).and_then(|x| x.initializer).is_none() {
            continue;
        }
        let build = |m: &mut Module| {
            let mut b = fn_builder::FnBuilder::new(
                ctx,
                unit,
                late_field_getter_symbol(ctx, vid),
                ctx.symbol_name(v.name).to_string(),
                Type::Ref,
            );
            let obj = Operand::Val(b.add_param("this".to_string(), Type::Ref));
            b.this_param = Some(obj.clone());
            b.finalizable_this = v.class.and_then(|c| ctx.classify_finalizable_class(c));
            b.enclosing_class = v.class;
            // A field outside the closed world (C7): nobody reads it.
            if ctx.pruned_field(vid) {
                b.pruned_body();
                b.finish(m);
                return;
            }
            b.lower_late_field_getter(obj, vid, ctx.program.unit(unit).ast.member(member).span);
            b.finish(m);
        };
        // A getter that does not lower becomes a rejection, like the other members
        // (`sdk_source::lower_function_or_reject`).
        sdk_source::lower_late_getter_or_reject(ctx, &mut module, vid, unit, build);
    }

    // 3. User top-level variables and static fields: one lazy
    // getter per global (N6).
    for (v_idx, v) in ctx.program.variables.iter().enumerate() {
        let vid = VariableId(v_idx as u32);
        if !ctx.library_in_module(v.library) || !is_global(ctx, vid) {
            continue;
        }
        let unit = match v.node {
            VariableRef::TopLevel { unit, .. }
            | VariableRef::Field { unit, .. }
            | VariableRef::EnumConstant { unit, .. } => unit,
            _ => continue,
        };
        if matches!(v.node, VariableRef::EnumConstant { .. }) {
            // Enum value (P3): a canonical object created on the first read.
            module.globals.push((vid.0, Type::Ref, global_value_symbol(ctx, vid)));
            let mut builder = fn_builder::FnBuilder::new(
                ctx,
                unit,
                global_symbol(ctx, vid),
                ctx.symbol_name(v.name).to_string(),
                Type::Ref,
            );
            builder.lower_enum_value(vid);
            builder.finish(&mut module);
            if let Some(premise) = enums::simple_enum_constant(ctx, vid) {
                module.simple_enum_constants.insert(premise.getter.clone(), premise);
            }
            continue;
        }
        let ty = members::variable_type(ctx, vid);
        let repr = ctx.to_hir_type(ty);
        let repr = if repr == Type::Void { Type::Ref } else { repr };
        module.globals.push((vid.0, repr, global_value_symbol(ctx, vid)));
        if ctx.arc_memory && matches!(repr, Type::I64 | Type::F64 | Type::I1 | Type::I8) {
            module.dart_scalar_globals.insert(global_value_symbol(ctx, vid));
        }
        // The getter (or its rejection) and the setter that another module calls to
        // write (P5c). Outside the closed world (C7), the getter throws.
        if ctx.pruned_global(vid) {
            sdk_source::lower_pruned_global(ctx, &mut module, vid, unit, repr);
            continue;
        }
        sdk_source::lower_global_or_reject(ctx, &mut module, vid, unit, repr);
    }

    // The tear-off entries are generated by whoever uses them; two functions that
    // take the same tear-off generate the same entry. The first one stays.
    let mut seen_set = std::collections::HashSet::new();
    module.functions.retain(|f| seen_set.insert(f.symbol.clone()));
    let mut seen_set = std::collections::HashSet::new();
    module.globals.retain(|g| seen_set.insert(g.2.clone()));

    // P6: the event loop (for users of `dart:async`) needs the callback the
    // runtime uses to invoke a closure (`events.rs`).
    if ctx.uses_dart_async
        && let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied())
    {
        let symbol = "dartforge_call_dart0".to_string();
        let mut b = fn_builder::FnBuilder::new(ctx, u, symbol.clone(), "chamar".to_string(), Type::Ref);
        let clo = Operand::Val(b.add_param("closure".to_string(), Type::Ref));
        let r = b.call_function_value(clo, &[]);
        b.terminate(Terminator::Return(Some(r)));
        b.finish(&mut module);
        // Under ARC, the runtime calls it as a Dart port (`dart_r1`, the event
        // loop): a callback with an Owned return, which the native scope releases.
        if ctx.arc_memory {
            module.dart_ref_returns.insert(symbol.clone());
            module.helpers.push((symbol.clone(), symbol.clone()));
        }
        module.call_dart = Some(symbol);
    }

    // P6: the source libraries come in whole; what the program
    // reaches stays (`source.rs`).
    if !ctx.from_source.is_empty() {
        crate::source::prune(&mut module);
    }
    // Frame metadata follows local allocations, independently of the event-loop
    // callback. Source pruning may have removed every allocating body.
    if module.functions.iter().any(|function| {
        function.blocks.iter().flat_map(|block| &block.instructions).any(|(_, instruction, _)| {
            matches!(instruction, Instruction::AllocObject { class_id, .. } if *class_id == async_sm::ASYNC_FRAME_ID)
        })
    }) {
        module.classes.push(ClassDef {
            id: async_sm::ASYNC_FRAME_ID,
            name: "_AsyncFrame".to_string(),
            field_count: 0,
            vtable: Vec::new(),
            to_string_symbol: None,
        });
    }
    // RTI: the type universe of the remaining recipes (`rti.rs`).
    rti::register_universe(ctx, &mut module);

    // E3: the verifier runs over the finished module; a problem here is a compiler
    // bug, and the module is not emitted.
    if module.errors.is_empty() {
        let problems = verifier::verify(&module);
        module.errors.extend(problems);
    }

    // Record shapes with a named field (P3, `records.rs`): one "class"
    // per shape, with a generated `toString()`, and the structural equality.
    for k in 0..ctx.record_shapes.len() {
        let id = crate::context::SHAPE_BASE_ID + k as u32;
        let (npos, names) = &ctx.record_shapes[k];
        let symbol = format!("df.$record.{k}.toString");
        module.classes.push(ClassDef {
            id,
            name: "Record".to_string(),
            field_count: npos + names.len(),
            vtable: Vec::new(),
            to_string_symbol: Some(symbol.clone()),
        });
        module.subtyping_edges.push((id, 0));
        // With the source SDK, `is Record`/`is Object` ask about the
        // `dart:core` classes (`(x: 1) is Record` gave false).
        for name_str in ["Record", "Object"] {
            if let Some(c) = ctx.sdk_class("core", name_str).and_then(|c| ctx.class_id_of(c)) {
                module.subtyping_edges.push((id, c));
            }
        }
        let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied()) else {
            continue;
        };
        let mut builder = fn_builder::FnBuilder::new(ctx, u, symbol, "toString".to_string(), Type::Ref);
        builder.lower_shape_to_string(k);
        builder.finish(&mut module);
        let hash_symbol = format!("df.$record.{k}.hashCode");
        let mut builder = fn_builder::FnBuilder::new(ctx, u, hash_symbol, "hashCode".to_string(), Type::I64);
        builder.lower_shape_hash(k);
        builder.finish(&mut module);
    }
    if !ctx.record_shapes.is_empty()
        && let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied())
    {
        let mut builder = fn_builder::FnBuilder::new(
            ctx,
            u,
            records::EQUAL_SYMBOL.to_string(),
            "==".to_string(),
            Type::I1,
        );
        builder.lower_record_equality();
        builder.finish(&mut module);
    }

    // Default `toString()` of the program enums (`Enum.value`), when the
    // enum does not declare its own.
    for (c_idx, class) in ctx.program.classes.iter().enumerate() {
        let cid = dartforge_elements::model::ClassId(c_idx as u32);
        if !enums::is_enum(ctx, cid) || !ctx.library_in_module(class.library) {
            continue;
        }
        let Some(id) = ctx.class_id_of(cid) else { continue };
        if module.classes.iter().any(|c| c.id == id && c.to_string_symbol.is_some()) {
            continue;
        }
        let Some(decl) = class.decl else { continue };
        let symbol = format!(
            "df.{}.{}.toString",
            crate::context::escape_str(&ctx.library_name(class.library)),
            crate::context::escape_str(ctx.symbol_name(class.name))
        );
        let mut builder = fn_builder::FnBuilder::new(ctx, decl.unit, symbol.clone(), "toString".to_string(), Type::Ref);
        builder.lower_enum_to_string(cid);
        builder.finish(&mut module);
        if let Some(c) = module.classes.iter_mut().find(|c| c.id == id) {
            c.to_string_symbol = Some(symbol);
        }
    }

    // Propagates to_string_symbol to subclasses that did not override it.
    for c_idx in 0..ctx.program.classes.len() {
        let Some(class_id) = ctx.class_id_of(dartforge_elements::model::ClassId(c_idx as u32)) else {
            continue;
        };
        let mut curr_cid = ctx.program.classes[c_idx].supertype_class;
        while let Some(sup) = curr_cid {
            let Some(sup_class_id) = ctx.class_id_of(sup) else {
                break;
            };
            let sup_ts = module.classes.iter().find(|c| c.id == sup_class_id).and_then(|c| c.to_string_symbol.clone());
            if let Some(ts) = sup_ts {
                if let Some(c_def) = module.classes.iter_mut().find(|c| c.id == class_id)
                    && c_def.to_string_symbol.is_none() {
                        c_def.to_string_symbol = Some(ts);
                    }
                break;
            }
            curr_cid = ctx.program.classes[sup.0 as usize].supertype_class;
        }
    }

    module
}
