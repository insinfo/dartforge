//! Reuse the logical length of the same immutable Dart string reference.
//!
//! This recognizes only the private, already-lowered `df.string_len(Ref) -> I64`
//! contract. It does not infer string identity from pointers, mutate ownership
//! plans, or attach LLVM memory invariance metadata. Constants are excluded:
//! materializing a string constant can allocate in the JIT.

use super::{
    cfg::Cfg,
    operands::{operands_mut, terminator_operands_mut},
};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

/// Runs before ownership preparation; debug bodies retain their original reads.
pub(super) fn rewrite(module: &mut Module) -> usize {
    if module.arc_prepared
        || std::env::var("DARTFORGE_NO_STRING_LENGTH_REUSE").as_deref() == Ok("1")
        || module.functions.iter().any(|f| f.symbol == "df.string_len")
    {
        return 0;
    }
    module.functions.iter_mut().map(rewrite_function).sum()
}

/// Recognizes the exact helper ABI, including its instruction result annotation.
fn receiver(instruction: &Instruction, ty: Type) -> Option<ValueId> {
    match instruction {
        Instruction::CallRuntime { name, args, ret_ty }
            if name == "df.string_len" && *ret_ty == Type::I64 && ty == Type::I64 =>
        {
            match args.as_slice() {
                [(Operand::Val(value), Type::Ref)] => Some(*value),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Tests reachable dominance without accepting an unreachable sentinel as a node.
fn dominates(cfg: &Cfg, ancestor: usize, mut block: usize) -> bool {
    if !cfg.reachable(block) {
        return false;
    }
    while block != ancestor {
        let parent = cfg.idom[block];
        if parent == block {
            return false;
        }
        block = parent;
    }
    true
}

fn rewrite_function(function: &mut Function) -> usize {
    if function.debug_info.is_some()
        || function.blocks.first().is_none_or(|b| b.id != BlockId(0))
        || !function
            .blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .any(|(_, instruction, ty)| receiver(instruction, *ty).is_some())
        || !crate::pruning::receiver_provenance::has_structural_ssa_shape(function)
    {
        return 0;
    }
    // Check all definitions, including unreachable ones. Structural SSA checks
    // alone do not certify instruction result representations.
    let mut types = HashMap::new();
    let mut definitions = HashMap::new();
    for (value, _, ty) in &function.params {
        if types.insert(*value, *ty).is_some() {
            return 0;
        }
        definitions.insert(*value, None);
    }
    for (block, body) in function.blocks.iter().enumerate() {
        for (value, instruction, ty) in &body.instructions {
            if types.insert(*value, *ty).is_some()
                || crate::llvm::LlvmEmitter::result_type(instruction, *ty) != *ty
            {
                return 0;
            }
            definitions.insert(*value, Some(block));
        }
    }
    let cfg = Cfg::new(function);
    let mut changed = hoist_prefixes(function, &cfg, &types, &definitions);
    let children = cfg.children();
    let mut available = HashMap::new();
    let mut replacements = HashMap::new();
    // Each dominator scope rolls back only the facts it introduced. No map is
    // cloned per block, and sibling branches cannot borrow each other's reads.
    let mut undo = Vec::new();
    let mut pending = vec![(0, false, 0)];
    while let Some((block, exiting, checkpoint)) = pending.pop() {
        if exiting {
            for value in undo.drain(checkpoint..) {
                available.remove(&value);
            }
            continue;
        }
        let checkpoint = undo.len();
        for (result, instruction, ty) in &function.blocks[block].instructions {
            let Some(value) = receiver(instruction, *ty) else {
                continue;
            };
            if types.get(&value) != Some(&Type::Ref) {
                continue;
            }
            if let Some(&prior) = available.get(&value) {
                replacements.insert(*result, prior);
            } else {
                available.insert(value, *result);
                undo.push(value);
            }
        }
        pending.push((block, true, checkpoint));
        pending.extend(children[block].iter().rev().map(|&child| (child, false, 0)));
    }
    changed += replacements.len();
    if !replacements.is_empty() {
        let mut replace = |operand: &mut Operand| {
            if let Operand::Val(value) = operand
                && let Some(prior) = replacements.get(value)
            {
                *value = *prior;
            }
        };
        for block in &mut function.blocks {
            block
                .instructions
                .retain(|(value, _, _)| !replacements.contains_key(value));
            for (_, instruction, _) in &mut block.instructions {
                operands_mut(instruction, &mut replace);
            }
            terminator_operands_mut(&mut block.terminator, &mut replace);
        }
    }
    changed
}

/// Hoists only a leading read prefix past an unconditional preheader branch.
/// That prefix already executes on every entry into the header, even for a
/// zero-trip loop. No checks, allocation, calls, or conditional entry are crossed.
fn hoist_prefixes(
    function: &mut Function,
    cfg: &Cfg,
    types: &HashMap<ValueId, Type>,
    definitions: &HashMap<ValueId, Option<usize>>,
) -> usize {
    let mut loops: HashMap<usize, HashSet<usize>> = HashMap::new();
    for &latch in &cfg.rpo {
        for &header in &cfg.successors[latch] {
            if !dominates(cfg, header, latch) {
                continue;
            }
            let members = loops
                .entry(header)
                .or_insert_with(|| HashSet::from([header]));
            let mut pending = vec![latch];
            while let Some(block) = pending.pop() {
                if members.insert(block) {
                    pending.extend(
                        cfg.predecessors[block]
                            .iter()
                            .copied()
                            .filter(|&p| cfg.reachable(p)),
                    );
                }
            }
        }
    }
    let mut changed = 0;
    for &header in &cfg.rpo {
        let Some(members) = loops.get(&header) else {
            continue;
        };
        let outside: Vec<_> = cfg.predecessors[header]
            .iter()
            .copied()
            .filter(|p| !members.contains(p))
            .collect();
        let [preheader] = outside.as_slice() else {
            continue;
        };
        if !matches!(function.blocks[*preheader].terminator,
            Terminator::Branch(id) if id == function.blocks[header].id)
        {
            continue;
        }
        let instructions = &function.blocks[header].instructions;
        let start = instructions
            .iter()
            .take_while(|(_, i, _)| matches!(i, Instruction::Phi { .. }))
            .count();
        let count = instructions[start..]
            .iter()
            .take_while(|(_, instruction, ty)| {
                let Some(value) = receiver(instruction, *ty) else {
                    return false;
                };
                types.get(&value) == Some(&Type::Ref)
                    && match definitions.get(&value) {
                        Some(None) => true,
                        Some(Some(block)) => {
                            !members.contains(block) && dominates(cfg, *block, *preheader)
                        }
                        None => false,
                    }
            })
            .count();
        if count != 0 {
            let moved: Vec<_> = function.blocks[header]
                .instructions
                .drain(start..start + count)
                .collect();
            function.blocks[*preheader].instructions.extend(moved);
            changed += count;
        }
    }
    changed
}

#[cfg(test)]
mod tests;
