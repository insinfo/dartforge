//! Conservative class-table activation for a completely covered fixed image.
//! Imported allocation summaries and native exposures remain open in this first version.
use super::{Reach, Summary};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

pub(super) fn narrow(module: &mut Module, sdk: &[&Summary]) -> Option<Reach> {
    module.closed_class_tables = None;
    if std::env::var("DARTFORGE_CLOSED_CLASS_REACHABILITY").as_deref() != Ok("1") {
        return None;
    }
    narrow_enabled(module, sdk)
}

fn narrow_enabled(module: &mut Module, sdk: &[&Summary]) -> Option<Reach> {
    // A failed re-audit must never retain a certificate for an older body.
    module.closed_class_tables = None;
    if !module.arc_fixed_ancestry
        || module.arc_memory
        || module.arc_prepared
        || module.arc_aot_ports.is_some()
        || module.sdk_library
        || !sdk.is_empty()
        || !module.sdk_records.is_empty()
        || !module.pruning_ambiguous_class_ids.is_empty()
        || !super::receiver_provenance::registry::closed(&[module])
        || !module.ffi_callbacks.is_empty()
        || !module.ffi_composites.is_empty()
        || !module.helpers.is_empty()
    {
        return None;
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
    if functions.len() != module.functions.len()
        || classes.len() != module.classes.len()
        || tables.len() != module.method_tables.len()
        || module
            .method_tables
            .iter()
            .map(|(_, s, _)| s.as_str())
            .collect::<HashSet<_>>()
            .len()
            != tables.len()
        || module
            .table_functions
            .iter()
            .any(|(cid, s)| tables.get(cid).map(|t| t.as_str()) != Some(s.as_str()))
        || module
            .classes
            .iter()
            .any(|c| !module.pruning_class_ids.contains(&c.id) || !tables.contains_key(&c.id))
        || module.method_tables.iter().any(|(cid, s, _)| {
            !classes.contains_key(cid) || module.table_functions.get(cid) != Some(s)
        })
        || functions.keys().any(|s| s.starts_with("dartforge_"))
    {
        return None;
    }
    // Entry-point parameters supplied by an external caller have no complete
    // allocation witness. The first version does not infer their class universe.
    for symbol in module
        .entry_symbol
        .iter()
        .chain(&module.source_roots)
        .chain(module.call_dart.iter())
        .chain(module.record.iter())
        .chain(module.start_rti.iter())
    {
        if functions
            .get(symbol.as_str())
            .is_none_or(|f| f.params.iter().any(|(_, _, t)| *t == Type::Ref))
        {
            return None;
        }
    }
    // Function, table and data names share the emitted symbol namespace.
    let mut definitions: HashSet<String> = functions.keys().map(|s| (*s).to_string()).collect();
    for (_, table, pairs) in &module.method_tables {
        let mut selectors = HashSet::new();
        if pairs
            .iter()
            .any(|(selector, _)| !selectors.insert(super::selector_hash(selector)))
        {
            return None;
        }
        if table.starts_with("dartforge_")
            || !definitions.insert(table.clone())
            || !definitions.insert(format!("{table}$d"))
        {
            return None;
        }
    }
    for (_, _, global) in &module.globals {
        if global.starts_with("dartforge_")
            || !definitions.insert(global.clone())
            || !definitions.insert(format!("{global}$ok"))
        {
            return None;
        }
    }
    if !definitions.insert("dartforge_entry".into()) {
        return None;
    }
    // Primitive boxing, strings and exception paths create runtime classes
    // without AllocObject instructions. Protect their declared tables even if
    // the module's optional registration vector omits those CIDs.
    let runtime_classes: HashSet<u32> = dartforge_runtime::layout::cid::SDK_CLASSES
        .iter()
        .map(|(cid, _, _)| *cid as u32)
        .chain(
            module
                .runtime_cids
                .iter()
                .filter_map(|cid| u32::try_from(*cid).ok()),
        )
        .filter(|cid| tables.contains_key(cid))
        .collect();
    module.closed_class_tables = Some(runtime_classes.clone());
    let summary = super::hir_summary(module);
    // The tentative lazy roots are for constructing the graph only. Publish
    // an active set only after every reachable definition has been checked.
    module.closed_class_tables = None;
    let reach = super::reach(&[&summary], true);
    for (node, symbol) in reach.names.iter().enumerate() {
        if reach.live(node as u32) && !definitions.contains(symbol) && !covered_runtime(symbol) {
            return None;
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
    let mut covered = true;
    for function in &module.functions {
        if !live(&function.symbol) {
            continue;
        }
        let shape = super::receiver_provenance::Shape::new(function).ok()?;
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
        for (_, instruction, result) in function.blocks.iter().flat_map(|b| &b.instructions) {
            let mut address_exposed = false;
            crate::optimize::operands::operands(instruction, &mut |operand| {
                address_exposed |= matches!(operand, Operand::Constant(Constant::Function(_)));
            });
            let valid = !address_exposed
                && match instruction {
                    Instruction::AllocObject { class_id, fields } => {
                        *result == Type::Ref && valid_allocation(*class_id, fields.len())
                    }
                    Instruction::CallRuntime { name, args, ret_ty }
                        if name == "dartforge_object_new" =>
                    {
                        runtime_call(name, args, *ret_ty, *result, &shape)
                            && matches!(args.as_slice(),[(Operand::Constant(Constant::Int(cid)),Type::I64),(Operand::Constant(Constant::Int(count)),Type::I64)]
                        if *ret_ty==Type::Ref && u32::try_from(*cid).ok().zip(usize::try_from(*count).ok()).is_some_and(|(cid,count)|valid_allocation(cid,count)))
                    }
                    Instruction::CallRuntime { name, args, ret_ty }
                    | Instruction::DeclaredFieldCall {
                        name, args, ret_ty, ..
                    } => runtime_call(name, args, *ret_ty, *result, &shape),
                    Instruction::CallStatic {
                        symbol,
                        args,
                        ret_ty,
                    } => {
                        *ret_ty == *result
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
                    | Instruction::CallClosure { .. }
                    | Instruction::CallClosureForward { .. }
                    | Instruction::TypedCall { .. }
                    | Instruction::AllocClosure { .. }
                    | Instruction::AllocClosureTyped { .. }
                    | Instruction::TearOff { .. }
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
                    Instruction::Const(_)
                    | Instruction::Add(..)
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
                    | Instruction::Box { .. }
                    | Instruction::Unbox { .. }
                    | Instruction::GetField { .. }
                    | Instruction::SetField { .. }
                    | Instruction::GetListElement { .. }
                    | Instruction::SetListElement { .. }
                    | Instruction::CellGet { .. }
                    | Instruction::CellSet { .. }
                    | Instruction::EnvGet { .. }
                    | Instruction::CheckNotNull(..)
                    | Instruction::IsClass { .. }
                    | Instruction::Simd { .. }
                    | Instruction::Phi { .. }
                    | Instruction::ConstArray(..)
                    | Instruction::ConcatStrings { .. } => true,
                };
            if !valid {
                covered = false;
                break;
            }
        }
        for block in &function.blocks {
            crate::optimize::operands::terminator_operands(&block.terminator, &mut |operand| {
                covered &= !matches!(operand, Operand::Constant(Constant::Function(_)));
            });
        }
        if !covered {
            break;
        }
    }
    // Protocol references do not appear as HIR calls, but still need an actual
    // audited body. In particular, a runtime allocator's familiar name is not
    // evidence that it can be called with the toString protocol ABI.
    for class in &module.classes {
        if !live(tables[&class.id]) {
            continue;
        }
        if class
            .vtable
            .iter()
            .any(|(_, target)| !functions.contains_key(target.as_str()))
        {
            return None;
        }
        if let Some(symbol) = &class.to_string_symbol {
            let body = functions.get(symbol.as_str())?;
            if body.return_ty != Type::Ref
                || body.params.len() != 1
                || body.params[0].2 != Type::Ref
            {
                return None;
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
                let body = functions.get(target.as_str())?;
                if body.return_ty != Type::Ref
                    || body.params.iter().map(|(_, _, ty)| *ty).collect::<Vec<_>>()
                        != [Type::Ref, Type::Ptr, Type::Ptr]
                {
                    return None;
                }
            }
        }
    }
    if !covered {
        module.closed_class_tables = None;
        return None;
    }
    let mut active = runtime_classes;
    for (cid, symbol, _) in &module.method_tables {
        if live(symbol) || live(&format!("{symbol}$d")) {
            active.insert(*cid);
        }
    }
    module.closed_class_tables = Some(active);
    Some(reach)
}

/// Explicit creation/field/pending APIs admitted by this closed-image proof.
/// Allocation-neutral effects alone do not establish absence of native exposure.
fn covered_runtime(symbol: &str) -> bool {
    matches!(
        symbol,
        "dartforge_object_new"
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
    crate::llvm::externs::EXTERNS
        .iter()
        .any(|external| external.decl == declaration)
}

#[cfg(test)]
mod tests;
