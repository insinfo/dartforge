//! Direct getter calls certified by physical allocations and complete local wrappers.

use super::operands::{operands, terminator_operands};
use crate::hir::*;
use crate::pruning::receiver_provenance;
use std::collections::{HashMap, HashSet};

/// Rewrites only fixed-image tracing calls, preserving the same uniform wrapper.
/// Descriptor readers must use the canonical zero-argument counts exclusively;
/// argument readers, writes, escapes, unknown bodies and ARC boundaries stay indirect.
pub(crate) fn rewrite_getters(module: &mut Module) -> usize {
    if std::env::var("DARTFORGE_NO_PHYSICAL_GETTER_DEVIRTUALIZATION").as_deref() == Ok("1")
        || !module.arc_fixed_ancestry
        || module.arc_memory
        || module.arc_prepared
        || module.arc_aot_ports.is_some()
        || module.sdk_library
        || !receiver_provenance::enabled()
        || module.functions.iter().any(|function| {
            matches!(
                function.symbol.as_str(),
                "dartforge_selector" | "dartforge_register_methods" | "dartforge_object_new"
            )
        })
    {
        return 0;
    }
    let changes = {
        let proof = receiver_provenance::analyze(&[module]);
        let mut functions = HashMap::new();
        let mut duplicate_functions = HashSet::new();
        for function in &module.functions {
            if functions
                .insert(function.symbol.as_str(), function)
                .is_some()
            {
                duplicate_functions.insert(function.symbol.as_str());
            }
        }
        let wrappers: HashSet<&str> = functions
            .iter()
            .filter_map(|(&symbol, function)| {
                (!duplicate_functions.contains(symbol)
                    && proof.values_for(function).is_some()
                    && wrapper_is_compatible(function))
                .then_some(symbol)
            })
            .collect();
        let mut tables = HashMap::new();
        let mut duplicate_tables = HashSet::new();
        for (cid, symbol, pairs) in &module.method_tables {
            if tables.insert(*cid, (symbol, pairs)).is_some() {
                duplicate_tables.insert(*cid);
            }
        }
        let mut class_counts = HashMap::new();
        for class in &module.classes {
            *class_counts.entry(class.id).or_insert(0usize) += 1;
        }
        let mut changes = HashMap::new();
        for (fi, function) in module.functions.iter().enumerate() {
            let Some(values) = proof.values_for(function) else {
                continue;
            };
            for (bi, block) in function.blocks.iter().enumerate() {
                for (ii, (_, instruction, ty)) in block.instructions.iter().enumerate() {
                    let Instruction::CallSelector {
                        selector,
                        recv: Operand::Val(recv),
                        args,
                        names,
                        tuple_types,
                    } = instruction
                    else {
                        continue;
                    };
                    if *ty != Type::Ref
                        || !selector.starts_with("g:")
                        || !args.is_empty()
                        || !names.is_empty()
                        || *tuple_types != Operand::Constant(Constant::Int(0))
                    {
                        continue;
                    }
                    let Some(classes) = values.get(recv).filter(|classes| !classes.is_empty())
                    else {
                        continue;
                    };
                    let hash = crate::lower::closures::hash_name(selector);
                    let mut target = None;
                    let mut complete = true;
                    for cid in classes {
                        if *cid < dartforge_runtime::layout::FIRST_FREE_CID as u32
                            || class_counts.get(cid) != Some(&1)
                            || duplicate_tables.contains(cid)
                            || module.pruning_ambiguous_class_ids.contains(cid)
                            || module.arc_external_ancestry.nominal_classes.contains(cid)
                            || module.arc_external_ancestry.ambiguous_classes.contains(cid)
                        {
                            complete = false;
                            break;
                        }
                        let Some((table, pairs)) = tables.get(cid) else {
                            complete = false;
                            break;
                        };
                        if module.table_functions.get(cid) != Some(*table) {
                            complete = false;
                            break;
                        }
                        let matching: Vec<_> = pairs
                            .iter()
                            .filter(|(name, _)| crate::lower::closures::hash_name(name) == hash)
                            .collect();
                        let [(name, symbol)] = matching.as_slice() else {
                            complete = false;
                            break;
                        };
                        if name != selector
                            || !wrappers.contains(symbol.as_str())
                            || target.is_some_and(|previous: &str| previous != symbol.as_str())
                        {
                            complete = false;
                            break;
                        }
                        target = Some(symbol.as_str());
                    }
                    if complete && let Some(target) = target {
                        changes.insert((fi, bi, ii), (target.to_string(), *recv));
                    }
                }
            }
        }
        changes
    };
    if changes.is_empty() {
        return 0;
    }
    let mut count = 0;
    for (fi, function) in module.functions.iter_mut().enumerate() {
        let needed = changes.keys().filter(|(index, _, _)| *index == fi).count();
        if needed == 0 {
            continue;
        }
        let maximum = function
            .params
            .iter()
            .map(|(id, _, _)| id.0)
            .chain(
                function
                    .blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .map(|(id, _, _)| id.0),
            )
            .max()
            .unwrap_or(0);
        let Some(mut next) = maximum.checked_add(1) else {
            continue;
        };
        if next
            .checked_add(needed.saturating_mul(2).try_into().unwrap_or(u32::MAX))
            .is_none()
        {
            continue;
        }
        for (bi, block) in function.blocks.iter_mut().enumerate() {
            let mut rewritten = Vec::with_capacity(block.instructions.len());
            for (ii, (value, instruction, ty)) in std::mem::take(&mut block.instructions)
                .into_iter()
                .enumerate()
            {
                if let Some((target, recv)) = changes.get(&(fi, bi, ii)) {
                    let args = ValueId(next);
                    let desc = ValueId(next + 1);
                    next += 2;
                    rewritten.push((args, Instruction::ConstArray(vec![0]), Type::Ptr));
                    rewritten.push((desc, Instruction::ConstArray(vec![0, 0]), Type::Ptr));
                    rewritten.push((
                        value,
                        Instruction::CallStatic {
                            symbol: target.clone(),
                            args: vec![Operand::Val(*recv), Operand::Val(args), Operand::Val(desc)],
                            ret_ty: Type::Ref,
                        },
                        ty,
                    ));
                    count += 1;
                } else {
                    rewritten.push((value, instruction, ty));
                }
            }
            block.instructions = rewritten;
        }
    }
    count
}

/// Audits all blocks, including unreachable ones, without permitting vector aliases.
fn wrapper_is_compatible(function: &Function) -> bool {
    let [
        (_, _, Type::Ref),
        (args, _, Type::Ptr),
        (desc, _, Type::Ptr),
    ] = function.params.as_slice()
    else {
        return false;
    };
    if function.return_ty != Type::Ref {
        return false;
    }
    for block in &function.blocks {
        for (_, instruction, ty) in &block.instructions {
            if let Instruction::LoadIndexed {
                base: Operand::Val(base),
                index: Operand::Constant(Constant::Int(index)),
            } = instruction
                && base == desc
                && matches!(index, 0 | 1)
                && *ty == Type::I64
            {
                continue;
            }
            let mut uses_vector = false;
            operands(
                instruction,
                &mut |op| {
                    uses_vector |=
                        matches!(op, Operand::Val(value) if value == args || value == desc)
                },
            );
            if uses_vector {
                return false;
            }
        }
        let mut uses_vector = false;
        terminator_operands(&block.terminator, &mut |op| {
            uses_vector |= matches!(op, Operand::Val(value) if value == args || value == desc)
        });
        if uses_vector {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests;
