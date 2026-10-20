//! Conservative class-table activation for a completely covered fixed image.
//! Only artifact-bound, exposure-free empty SDK closures can cross the import boundary.
use super::{Reach, Summary};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

mod closures;
mod storage;

pub(super) fn narrow(module: &mut Module, sdk: &[&Summary]) -> Option<Reach> {
    module.closed_class_tables = None;
    if std::env::var("DARTFORGE_CLOSED_CLASS_REACHABILITY").as_deref() != Ok("1") {
        return None;
    }
    match audit(module, sdk) {
        Ok(reach) => Some(reach),
        Err(reason) => {
            if std::env::var("DARTFORGE_TRACE_CLASS_REACHABILITY").as_deref() == Ok("1") {
                eprintln!("  Closed class reachability: {reason}; retaining broad registration");
            }
            None
        }
    }
}

#[cfg(test)]
fn narrow_enabled(module: &mut Module, sdk: &[&Summary]) -> Option<Reach> {
    audit(module, sdk).ok()
}

/// Reports the first unproved boundary without publishing a partial class set.
fn audit(module: &mut Module, sdk: &[&Summary]) -> Result<Reach, String> {
    // A failed re-audit must never retain a certificate for an older body.
    module.closed_class_tables = None;
    if !module.arc_fixed_ancestry {
        return Err("class ancestry is not fixed".into());
    }
    if module.arc_memory {
        return Err("ARC memory mode is active".into());
    }
    if module.arc_prepared {
        return Err("ARC preparation is active".into());
    }
    if module.arc_aot_ports.is_some() {
        return Err("ARC AOT ports are active".into());
    }
    if module.sdk_library {
        return Err("module is an SDK library".into());
    }
    if !module.pruning_ambiguous_class_ids.is_empty() {
        return Err("class IDs are ambiguous".into());
    }
    if !super::receiver_provenance::registry::closed(&[module]) {
        return Err("class registry is not closed".into());
    }
    if !module.ffi_callbacks.is_empty() {
        return Err("FFI callbacks are not covered".into());
    }
    if !module.ffi_composites.is_empty() {
        return Err("FFI composites are not covered".into());
    }
    if !module.helpers.is_empty() {
        return Err("module helpers are not covered".into());
    }
    let functions: HashMap<_, _> = module
        .functions
        .iter()
        .map(|f| (f.symbol.as_str(), f))
        .collect();
    let classes: HashMap<_, _> = module.classes.iter().map(|c| (c.id, c)).collect();
    let tables: HashMap<_, _> = module
        .method_tables
        .iter()
        .map(|(cid, s, _)| (*cid, s))
        .collect();
    if functions.len() != module.functions.len() {
        return Err("function symbols are duplicated".into());
    }
    if classes.len() != module.classes.len() {
        return Err("class IDs are duplicated".into());
    }
    if tables.len() != module.method_tables.len() {
        return Err("method table class IDs are duplicated".into());
    }
    if module
        .method_tables
        .iter()
        .map(|(_, symbol, _)| symbol.as_str())
        .collect::<HashSet<_>>()
        .len()
        != tables.len()
    {
        return Err("method table symbols are duplicated".into());
    }
    // Metadata-only ancestors cannot be allocated by a covered instruction.
    // Runtime-created values are different: they need their tables even when
    // no explicit allocation appears in the program.
    let implicit_classes: HashSet<u32> = dartforge_runtime::layout::cid::SDK_CLASSES
        .iter()
        .map(|(cid, _, _)| *cid as u32)
        // The canonical Type object is allocated by the runtime RTI cache,
        // independently of program AllocObject instructions and positional CIDs.
        .chain(std::iter::once(crate::lower::rti::TYPE_CLASS as u32))
        .chain(
            module
                .runtime_cids
                .iter()
                .filter_map(|cid| u32::try_from(*cid).ok()),
        )
        .collect();
    for cid in &module.pruning_metadata_only_class_ids {
        if !classes.contains_key(cid)
            || tables.contains_key(cid)
            || module.table_functions.contains_key(cid)
            || implicit_classes.contains(cid)
        {
            return Err(format!(
                "nominal-only class {cid} has inconsistent declaration facts"
            ));
        }
    }
    for class in &module.classes {
        if !module.pruning_class_ids.contains(&class.id) {
            return Err(format!(
                "class {} ({}) has unproved ancestry",
                class.name, class.id
            ));
        }
        if !tables.contains_key(&class.id)
            && !module.pruning_metadata_only_class_ids.contains(&class.id)
        {
            return Err(format!(
                "class {} ({}) has no method table",
                class.name, class.id
            ));
        }
    }
    for (cid, symbol, _) in &module.method_tables {
        if !classes.contains_key(cid) || module.table_functions.get(cid) != Some(symbol) {
            return Err(format!(
                "method table {symbol} for class {cid} has no matching class/table function"
            ));
        }
    }
    if let Some(symbol) = functions
        .keys()
        .find(|symbol| symbol.starts_with("dartforge_"))
    {
        return Err(format!("function {symbol} overlaps the runtime namespace"));
    }
    // Function, table and data names share the emitted symbol namespace.
    let mut definitions: HashSet<String> = functions.keys().map(|s| (*s).to_string()).collect();
    for (_, table, pairs) in &module.method_tables {
        let mut selectors = HashSet::new();
        if pairs
            .iter()
            .any(|(selector, _)| !selectors.insert(super::selector_hash(selector)))
        {
            return Err("selector hashes collide".into());
        }
        if table.starts_with("dartforge_")
            || !definitions.insert(table.clone())
            || !definitions.insert(format!("{table}$d"))
        {
            return Err("table symbol definitions collide".into());
        }
    }
    for (_, _, global) in &module.globals {
        if global.starts_with("dartforge_")
            || !definitions.insert(global.clone())
            || !definitions.insert(format!("{global}$ok"))
        {
            return Err("global symbol definitions collide".into());
        }
    }
    if !definitions.insert("dartforge_entry".into()) {
        return Err("entry symbol definition collides".into());
    }
    let imports = if sdk.is_empty() {
        None
    } else {
        let inventory = module
            .imported_allocation_inventory
            .as_deref()
            .ok_or("SDK allocation inventory is unavailable")?;
        let cids = classes.keys().copied().collect();
        inventory.reconcile(sdk, &definitions, &cids, super::universal_parent::declared(module))
            .map_err(|detail| format!("SDK inventory does not match the selected image: {detail}"))?;
        Some(inventory)
    };
    for (cid, symbol) in &module.table_functions {
        if tables.get(cid).map(|table| table.as_str()) != Some(symbol.as_str())
            && !imports.is_some_and(|inventory| inventory.table_declaration(*cid, symbol))
        {
            return Err(format!(
                "table function {symbol} for class {cid} has no matching table"
            ));
        }
    }
    // The event entry is a deferred root: none of the native APIs admitted by
    // this proof can enqueue callbacks, and imported calls must be exposure-free.
    // Probe the other roots first; a nonempty or opaque event source must fail
    // complete coverage before this entry can be removed from the fixed image.
    let event_entry = module.call_dart.clone();
    if let Some(symbol) = &event_entry {
        // Native-controlled entrypoints may coexist with work not seeded by
        // the ordinary program roots; the empty-domain case does not cover them.
        if !module.source_roots.is_empty() || !module.ffi_trampolines.is_empty() {
            return Err(format!("event entry {symbol} has unproved native ingress"));
        }
        let valid = functions.get(symbol.as_str()).is_some_and(|body|
            body.return_ty == Type::Ref && body.params.len() == 1 && body.params[0].2 == Type::Ref);
        if !valid {
            return Err(format!("event entry {symbol} has an unproved protocol ABI"));
        }
    }
    // Managed references and raw argument-packet pointers from external entries
    // do not supply a closed class universe. Imported startup remains limited
    // to zero-argument void calls with a pure or certified nominal startup body.
    for symbol in module
        .entry_symbol
        .iter()
        .chain(&module.source_roots)
        .chain(module.record.iter())
        .chain(module.start_rti.iter())
        .chain(&module.sdk_records)
    {
        match functions.get(symbol.as_str()) {
            Some(body) if body.params.iter().all(|(_, _, ty)| scalar_entry_type(*ty)) => {}
            None if imports.is_some_and(|inventory| inventory.entry(symbol)) => {}
            _ => {
                let detail = imports.and_then(|inventory| inventory.entry_obligations(symbol))
                    .map(|detail| format!(": {detail}")).unwrap_or_default();
                return Err(format!(
                    "entry root {symbol} has an unproved body or input ABI{detail}"
                ));
            }
        }
    }
    // Primitive boxing, strings and exception paths create runtime classes
    // without AllocObject instructions. Protect their declared tables even if
    // the module's optional registration vector omits those CIDs.
    let runtime_classes: HashSet<u32> = implicit_classes
        .into_iter()
        .filter(|cid| tables.contains_key(cid))
        .collect();
    module.closed_class_tables = Some(runtime_classes.clone());
    let mut summary = super::hir_summary(module);
    module.closed_class_tables = None;
    if let Some(symbol) = &event_entry {
        let callback = summary.names.iter().position(|name| name == symbol)
            .ok_or("event entry is missing from the HIR summary")? as u32;
        let entry = summary.names.iter().position(|name| name == "dartforge_entry")
            .ok_or("program entry is missing from the HIR summary")? as u32;
        let definition = summary.definitions.iter_mut().find(|definition| definition.name == entry)
            .ok_or("program entry has no HIR definition")?;
        // Remove only the emitter's automatic loop edge. Actual calls, address
        // exposure, table entries and SDK references stay in the probe graph.
        definition.refs.retain(|node| *node != callback);
    }
    // The tentative lazy roots were used only to construct this probe summary.
    // Publish an active set only after every reachable definition is checked.
    let proof_sdk = imports.map(|inventory| inventory.class_proof_summaries());
    let mut all: Vec<&Summary> = proof_sdk.as_ref().map_or_else(|| sdk.to_vec(), |summaries| summaries.iter().collect());
    all.push(&summary);
    let reach = super::reach(&all, true);
    if let Some(symbol) = &event_entry
        && reach.names.iter().enumerate().any(|(node, name)| name == symbol && reach.live(node as u32))
    {
        return Err(format!("event entry {symbol} is exposed outside the loop"));
    }
    for (node, symbol) in reach.names.iter().enumerate() {
        let covered = definitions.contains(symbol)
            || covered_runtime(symbol) && !imports.is_some_and(|i| i.contains_definition(symbol))
            || imports.is_some_and(|i| i.known_node(symbol));
        if reach.live(node as u32) && !covered {
            return Err(format!(
                "reachable symbol {symbol} has no covered definition"
            ));
        }
    }
    let index: HashMap<_, _> = reach
        .names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i as u32))
        .collect();
    let live = |s: &str| index.get(s).is_some_and(|&i| reach.live(i));
    let valid_allocation = |cid: u32, count: usize| {
        classes.get(&cid).is_some_and(|c| c.field_count == count) && tables.contains_key(&cid)
    };
    let globals: HashMap<_, _> = module
        .globals
        .iter()
        .flat_map(|(id, ty, symbol)| {
            [
                (symbol.clone(), (*ty, (*ty == Type::Ref).then_some(*id))),
                (format!("{symbol}$ok"), (Type::I8, None)),
            ]
        })
        .collect();
    let closure_targets = closures::targets(module);
    // An implicit closure receiver is a layout witness only when that body
    // cannot also be entered as an ordinary object's protocol method.
    if module.method_tables.iter().flat_map(|(_, _, methods)| methods.iter())
        .any(|(_, target)| closure_targets.contains(target.as_str()))
        || module.classes.iter().any(|class|
            class.vtable.iter().any(|(_, target)| closure_targets.contains(target.as_str()))
            || class.to_string_symbol.as_ref().is_some_and(|target| closure_targets.contains(target.as_str())))
    {
        return Err("closure entry overlaps an object protocol target".into());
    }
    for function in &module.functions {
        if !live(&function.symbol) {
            continue;
        }
        let shape = super::receiver_provenance::Shape::new(function)
            .map_err(|_| format!("function {} has an unproved SSA/CFG shape", function.symbol))?;
        let closure_proof = closures::prove(function, &functions, &closure_targets, &shape)
            .ok_or_else(|| {
                format!(
                    "function {} has an unproved closure boundary",
                    function.symbol
                )
            })?;
        let storage = storage::Proof::new(function).ok_or_else(|| {
            format!("function {} has unproved field storage or representation", function.symbol)
        })?;
        let stack_slots: HashMap<_, _> = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|(value, instruction, result)| match instruction {
                Instruction::Alloca(ty) if *result == Type::Ptr && *ty != Type::Void => {
                    Some((*value, *ty))
                }
                _ => None,
            })
            .collect();
        for (value, instruction, result) in function.blocks.iter().flat_map(|b| &b.instructions) {
            let mut address_exposed = false;
            crate::optimize::operands::operands(instruction, &mut |operand| {
                address_exposed |= matches!(operand, Operand::Constant(Constant::Function(_)));
            });
            let valid = !address_exposed
                && match instruction {
                    Instruction::AllocObject { class_id, fields } => {
                        *result == Type::Ref && valid_allocation(*class_id, fields.len())
                    }
                    Instruction::AllocClosure { .. }
                    | Instruction::AllocClosureTyped { .. }
                    | Instruction::TearOff { .. } => closure_proof.handles.contains(value),
                    Instruction::TypedCall { .. } => closure_proof.calls.contains(value),
                    Instruction::CallClosure { closure, .. } => {
                        matches!(closure, Operand::Val(id) if closure_proof.handles.contains(id))
                    }
                    Instruction::CallRuntime { name, args, ret_ty }
                        if name == "dartforge_object_new" =>
                    {
                        runtime_call(name, args, *ret_ty, *result, &shape)
                            && matches!(args.as_slice(),[(Operand::Constant(Constant::Int(cid)),Type::I64),(Operand::Constant(Constant::Int(count)),Type::I64)]
                        if *ret_ty==Type::Ref && u32::try_from(*cid).ok().zip(usize::try_from(*count).ok()).is_some_and(|(cid,count)|valid_allocation(cid,count)))
                    }
                    Instruction::CallRuntime { name, args, ret_ty }
                    | Instruction::DeclaredFieldCall { name, args, ret_ty, .. } => {
                        let declared_field = matches!(instruction, Instruction::DeclaredFieldCall { .. });
                        (!declared_field || matches!(name.as_str(), "dartforge_object_get" | "dartforge_object_set"))
                            && runtime_call(name, args, *ret_ty, *result, &shape)
                            && match name.as_str() {
                                "dartforge_object_get" => closure_proof.reads.contains(value)
                                    || matches!(args.as_slice(), [(object, Type::Ref), (index, Type::I64)]
                                        if storage::index(function, index).is_some_and(|index| storage.read(object, index, *result))),
                                "dartforge_object_set" => matches!(args.as_slice(), [(object, Type::Ref), (index, Type::I64), _, _]
                                    if storage::index(function, index).is_some_and(|index| storage.write(object, index, *result))),
                                "dartforge_value_class" => *result == Type::I64 && matches!(args.as_slice(), [(_, Type::Ref)]),
                                "dartforge_mark_constant" => matches!(args.as_slice(), [(_, Type::Ref), (getter, Type::I64)]
                                    if storage::index(function, getter) == Some(0)),
                                _ => true,
                            }
                    }
                    Instruction::CallStatic {
                        symbol,
                        args,
                        ret_ty,
                    } => {
                        !closure_targets.contains(symbol.as_str())
                            && *ret_ty == *result
                            && match functions.get(symbol.as_str()) {
                                Some(callee) => {
                                    callee.return_ty == *ret_ty
                                        && callee.params.len() == args.len()
                                        && callee.params.iter().zip(args).all(
                                            |((_, _, ty), arg)| {
                                                shape.operand_type(arg) == Some(*ty)
                                            },
                                        )
                                }
                                None => {
                                    tables.values().any(|table| *table == symbol)
                                        && args.is_empty()
                                        && *ret_ty == Type::Ptr
                                        || imports.is_some_and(|inventory| {
                                            let types = args
                                                .iter()
                                                .map(|arg| shape.operand_type(arg))
                                                .collect::<Option<Vec<_>>>();
                                            types.is_some_and(|types| {
                                                inventory.call(symbol, &types, *ret_ty)
                                            })
                                        })
                                }
                            }
                    }
                    Instruction::CallSelector {
                        recv,
                        args,
                        tuple_types,
                        ..
                    } => {
                        *result == Type::Ref
                            && shape.operand_type(recv) == Some(Type::Ref)
                            && args
                                .iter()
                                .all(|arg| shape.operand_type(arg) == Some(Type::Ref))
                            && matches!(
                                shape.operand_type(tuple_types),
                                Some(Type::Ref | Type::I64)
                            )
                    }
                    Instruction::CallSelectorForward {
                        arc_owned,
                        recv,
                        args,
                        desc,
                        ..
                    } => {
                        !arc_owned
                            && *result == Type::Ref
                            && shape.operand_type(recv) == Some(Type::Ref)
                            && shape.operand_type(args) == Some(Type::Ptr)
                            && shape.operand_type(desc) == Some(Type::Ptr)
                    }
                    // A pointer identity cast preserves an existing address.
                    // Reinterpreting scalar bits or a managed value would bypass
                    // the external packet-input boundary above.
                    Instruction::Bitcast { op, to: Type::Ptr } => {
                        *result == Type::Ptr && shape.operand_type(op) == Some(Type::Ptr)
                    }
                    Instruction::Unbox { op, to } => {
                        *result == *to && scalar_entry_type(*to)
                            && shape.operand_type(op) == Some(Type::Ref)
                    }
                    Instruction::Alloca(ty) => *result == Type::Ptr && *ty != Type::Void,
                    Instruction::Load { ptr, ty } => {
                        *result == *ty
                            && *ty != Type::Ref
                            && matches!(ptr, Operand::Val(slot) if stack_slots.get(slot) == Some(ty))
                    }
                    Instruction::Store { ptr, val } => {
                        *result == Type::Void
                            && matches!(ptr, Operand::Val(slot)
                            if stack_slots.get(slot).is_some_and(|ty| shape.operand_type(val) == Some(*ty)))
                    }
                    Instruction::LoadGlobal { symbol, ty } => {
                        *result == *ty
                            && globals
                                .get(symbol)
                                .is_some_and(|(declared, _)| declared == ty)
                    }
                    Instruction::StoreGlobal {
                        symbol,
                        val,
                        ty,
                        root,
                    } => {
                        *result == Type::Void
                            && shape.operand_type(val) == Some(*ty)
                            && globals.get(symbol) == Some(&(*ty, *root))
                    }
                    // These paths require allocation/target coverage not yet exported
                    // by the SDK/native boundary. Conservatism is intentional, not a
                    // claim that every foreign function allocates a Dart object.
                    Instruction::Const(Constant::Function(_))
                    | Instruction::NativeCall { .. }
                    | Instruction::CompositeNativeCall { .. }
                    | Instruction::CallInterface { .. }
                    | Instruction::CallDynamic { .. }
                    | Instruction::CallClosureForward { .. }
                    | Instruction::AllocList { .. }
                    | Instruction::AllocRecord { .. }
                    | Instruction::AllocCell { .. }
                    | Instruction::AllocEnv { .. }
                    | Instruction::Bitcast { to: Type::Ref, .. }
                    | Instruction::NativeLoad { .. }
                    | Instruction::NativeWrite { .. }
                    | Instruction::FunctionTable(_) => false,
                    Instruction::ArcCopy { .. }
                    | Instruction::ArcDrop { .. }
                    | Instruction::ArcMove { .. }
                    | Instruction::ArcLoadStrong { .. }
                    | Instruction::ArcStoreStrong { .. } => false,
                    Instruction::LoadIndexed { .. } => false,
                    Instruction::GetField { object, index }
                    | Instruction::EnvGet { env: object, index } =>
                        closure_proof.reads.contains(value) || storage.read(object, *index, *result),
                    Instruction::CellGet { cell } => storage.read(cell, 0, *result),
                    Instruction::SetField { object, index, .. } => storage.write(object, *index, *result),
                    Instruction::CellSet { cell, .. } => storage.write(cell, 0, *result),
                    Instruction::GetListElement { .. } | Instruction::SetListElement { .. } => false,
                    Instruction::CheckNotNull(operand) => *result == Type::Ref && shape.operand_type(operand) == Some(Type::Ref),
                    Instruction::IsClass { object, .. } => *result == Type::I1 && shape.operand_type(object) == Some(Type::Ref),
                    Instruction::ConcatStrings { parts } => *result == Type::Ref
                        && parts.iter().all(|part| shape.operand_type(part) == Some(Type::Ref)),
                    Instruction::Box { .. } | Instruction::Phi { .. } | Instruction::Const(_)
                    | Instruction::ConstArray(..) => true,
                    Instruction::Add(..)
                    | Instruction::Sub(..)
                    | Instruction::Mul(..)
                    | Instruction::SDiv(..)
                    | Instruction::SRem(..)
                    | Instruction::Shl(..)
                    | Instruction::AShr(..)
                    | Instruction::LShr(..)
                    | Instruction::And(..)
                    | Instruction::Or(..)
                    | Instruction::Xor(..)
                    | Instruction::Neg(..)
                    | Instruction::Not(..)
                    | Instruction::FAdd(..)
                    | Instruction::FSub(..)
                    | Instruction::FMul(..)
                    | Instruction::FDiv(..)
                    | Instruction::FNeg(..)
                    | Instruction::ICmp(..)
                    | Instruction::FCmp(..)
                    | Instruction::LNot(..)
                    | Instruction::IntToDouble(..)
                    | Instruction::DoubleToInt(..)
                    | Instruction::ZExt { .. }
                    | Instruction::Trunc { .. }
                    | Instruction::Bitcast { .. }
                    | Instruction::Simd { .. } => !matches!(result, Type::Ref | Type::Ptr),
                };
            if !valid {
                return Err(format!(
                    "function {} value {value:?} has an unproved instruction: {instruction:?}",
                    function.symbol
                ));
            }
        }
        for block in &function.blocks {
            let mut address_exposed = false;
            crate::optimize::operands::terminator_operands(&block.terminator, &mut |operand| {
                address_exposed |= matches!(operand, Operand::Constant(Constant::Function(_)));
            });
            if address_exposed {
                return Err(format!(
                    "function {} terminator exposes a function address",
                    function.symbol
                ));
            }
        }
    }
    // Protocol references do not appear as HIR calls, but still need an actual
    // audited body. In particular, a runtime allocator's familiar name is not
    // evidence that it can be called with the toString protocol ABI.
    for class in &module.classes {
        if !tables.get(&class.id).is_some_and(|table| live(table)) {
            continue;
        }
        if class
            .vtable
            .iter()
            .any(|(_, target)| !functions.contains_key(target.as_str()))
        {
            return Err(format!(
                "class {} has a vtable target without a local body",
                class.name
            ));
        }
        if let Some(symbol) = &class.to_string_symbol {
            let body = functions.get(symbol.as_str()).ok_or_else(|| {
                format!("class {} has no local toString body: {symbol}", class.name)
            })?;
            if body.return_ty != Type::Ref
                || body.params.len() != 1
                || body.params[0].2 != Type::Ref
            {
                return Err(format!("class {} has an unproved toString ABI", class.name));
            }
        }
    }
    // A uniform table entry is called through the fixed Ref/argv/descriptor ABI.
    for (_, table, pairs) in &module.method_tables {
        let table_node = index.get(format!("{table}$d").as_str());
        for (selector, target) in pairs {
            if table_node
                .is_some_and(|&node| reach.live_table_pair(node, super::selector_hash(selector)))
            {
                let body = functions.get(target.as_str()).ok_or_else(|| {
                    format!("table {table} selector {selector} has no local body: {target}")
                })?;
                if body.return_ty != Type::Ref
                    || body.params.iter().map(|(_, _, ty)| *ty).collect::<Vec<_>>()
                        != [Type::Ref, Type::Ptr, Type::Ptr]
                {
                    return Err(format!(
                        "table {table} selector {selector} has an unproved uniform ABI"
                    ));
                }
            }
        }
    }
    let mut active = runtime_classes;
    for (cid, symbol, _) in &module.method_tables {
        if live(symbol) || live(&format!("{symbol}$d")) {
            active.insert(*cid);
        }
    }
    module.closed_class_tables = Some(active);
    // Complete coverage above establishes an empty callback domain. Publish
    // this transformation only now; failed probes preserve the original loop.
    // prune_hir removes the unreferenced bridge body using the returned graph.
    if event_entry.is_some() { module.call_dart = None; }
    Ok(reach)
}

// These representations contain scalar data, not managed inputs or raw packet
// address capabilities. New HIR representations need an explicit review here.
fn scalar_entry_type(ty: Type) -> bool {
    matches!(ty, Type::I1 | Type::I8 | Type::I64 | Type::F64
        | Type::V4F32 | Type::V4I32 | Type::V2F64)
}

/// Explicit creation/field/pending APIs admitted by this closed-image proof.
/// Allocation-neutral effects alone do not establish absence of native exposure.
/// None may enqueue deferred Dart work: successful coverage also certifies the
/// empty event-entry domain. Admitting a queue producer requires a new proof.
fn covered_runtime(symbol: &str) -> bool {
    matches!(
        symbol,
        "dartforge_object_new"
            | "dartforge_value_class"
            // Queries/cache updates use host allocations, not Dart objects or
            // class registry mutation (runtime/src/core.rs).
            | "dartforge_is_subclass"
            | "dartforge_object_get"
            | "dartforge_object_set"
            | "dartforge_mark_constant"
            | "dartforge_exception_pending"
            | "dartforge_exception_clear"
    )
}

/// Match the actual runtime declaration and every operand representation.
fn runtime_call(
    name: &str,
    args: &[(Operand, Type)],
    ret: Type,
    result: Type,
    shape: &super::receiver_provenance::Shape,
) -> bool {
    if !covered_runtime(name)
        || name == "dartforge_is_subclass" && args.iter().any(|(_, ty)| *ty != Type::I64)
        || ret != result
        || args
            .iter()
            .any(|(operand, ty)| shape.operand_type(operand) != Some(*ty))
    {
        return false;
    }
    let parameters = args
        .iter()
        .map(|(_, ty)| ty.llvm_ir())
        .collect::<Vec<_>>()
        .join(", ");
    let declaration = format!("declare {} @{name}({parameters})", ret.llvm_ir());
    crate::llvm::externs::EXTERNS.iter().any(|external| {
        external
            .decl
            .strip_prefix(&declaration)
            .is_some_and(|attributes| attributes.is_empty() || attributes.starts_with(' '))
    })
}

#[cfg(test)]
mod tests;
