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
            .any(|c| !module.pruning_class_ids.contains(&c.id))
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
    {
        if functions
            .get(symbol.as_str())
            .is_some_and(|f| f.params.iter().any(|(_, _, t)| *t == Type::Ref))
        {
            return None;
        }
    }
    module.closed_class_tables = Some(HashSet::new());
    let summary = super::hir_summary(module);
    let reach = super::reach(&[&summary], true);
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
    let mut covered = true;
    for function in &module.functions {
        if !live(&function.symbol) {
            continue;
        }
        for (_, instruction, _) in function.blocks.iter().flat_map(|b| &b.instructions) {
            let valid = match instruction {
                Instruction::AllocObject { class_id, fields } => {
                    valid_allocation(*class_id, fields.len())
                }
                Instruction::CallRuntime { name, args, ret_ty }
                    if name == "dartforge_object_new" =>
                {
                    matches!(args.as_slice(),[(Operand::Constant(Constant::Int(cid)),Type::I64),(Operand::Constant(Constant::Int(count)),Type::I64)]
                        if *ret_ty==Type::Ref && u32::try_from(*cid).ok().zip(usize::try_from(*count).ok()).is_some_and(|(cid,count)|valid_allocation(cid,count)))
                }
                Instruction::CallRuntime { name, .. }
                | Instruction::DeclaredFieldCall { name, .. } => matches!(
                    name.as_str(),
                    "dartforge_object_get"
                        | "dartforge_object_set"
                        | "dartforge_mark_constant"
                        | "dartforge_exception_pending"
                        | "dartforge_exception_clear"
                ),
                Instruction::CallStatic { symbol, .. } => {
                    functions.contains_key(symbol.as_str())
                        || module.table_functions.values().any(|s| s == symbol)
                }
                // These paths require allocation/target coverage not yet exported
                // by the SDK/native boundary. Conservatism is intentional, not a
                // claim that every foreign function allocates a Dart object.
                Instruction::NativeCall { .. }
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
                | Instruction::Load { ty: Type::Ref, .. }
                | Instruction::Bitcast { to: Type::Ref, .. }
                | Instruction::NativeLoad {
                    ty: CType::Handle, ..
                }
                | Instruction::NativeWrite {
                    ty: CType::Handle, ..
                } => false,
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
                | Instruction::Alloca(..)
                | Instruction::Load { .. }
                | Instruction::Store { .. }
                | Instruction::GetField { .. }
                | Instruction::SetField { .. }
                | Instruction::GetListElement { .. }
                | Instruction::SetListElement { .. }
                | Instruction::CellGet { .. }
                | Instruction::CellSet { .. }
                | Instruction::EnvGet { .. }
                | Instruction::CheckNotNull(..)
                | Instruction::IsClass { .. }
                | Instruction::LoadGlobal { .. }
                | Instruction::StoreGlobal { .. }
                | Instruction::Simd { .. }
                | Instruction::Phi { .. }
                | Instruction::ConstArray(..)
                | Instruction::FunctionTable(..)
                | Instruction::CallSelector { .. }
                | Instruction::CallSelectorForward { .. }
                | Instruction::NativeLoad { .. }
                | Instruction::NativeWrite { .. }
                | Instruction::ConcatStrings { .. } => true,
            };
            if !valid {
                covered = false;
                break;
            }
        }
        if !covered {
            break;
        }
    }
    if !covered {
        module.closed_class_tables = None;
        return None;
    }
    let mut active: HashSet<u32> = module
        .runtime_cids
        .iter()
        .filter_map(|cid| u32::try_from(*cid).ok())
        .collect();
    for (cid, symbol, _) in &module.method_tables {
        if live(symbol) || live(&format!("{symbol}$d")) {
            active.insert(*cid);
        }
    }
    module.closed_class_tables = Some(active);
    Some(reach)
}

#[cfg(test)]
mod tests;
