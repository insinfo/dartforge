//! Reuse canonical double boxes only after their original checked unbox succeeds.
use super::{
    cfg::Cfg,
    operands::{operands, operands_mut, terminator_operands, terminator_operands_mut},
};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

pub(super) fn rewrite(module: &mut Module) -> usize {
    if std::env::var("DARTFORGE_NO_CHECKED_DOUBLE_ROUNDTRIP").as_deref() == Ok("1")
        || !module.checked_double_identity
        || !module.arc_fixed_ancestry
        || module.arc_memory
        || module.arc_prepared
        || module.arc_aot_ports.is_some()
        || module.sdk_library
        || module.functions.iter().any(|f| {
            matches!(
                f.symbol.as_str(),
                "dartforge_unbox_double"
                    | "dartforge_box_double"
                    | "dartforge_native_Identical_comparison"
                    | "dartforge_native_DartForge_double_hashCode"
            )
        })
    {
        return 0;
    }
    module.functions.iter_mut().map(rewrite_function).sum()
}

fn rewrite_function(f: &mut Function) -> usize {
    if f.blocks.first().is_none_or(|b| b.id != BlockId(0)) {
        return 0;
    }
    let mut ids = HashSet::new();
    let mut types = HashMap::new();
    let mut locations = HashMap::new();
    for (v, _, t) in &f.params {
        if types.insert(*v, Some(*t)).is_some() {
            return 0;
        }
    }
    for (bi, b) in f.blocks.iter().enumerate() {
        if !ids.insert(b.id) {
            return 0;
        }
        for (ii, (v, i, t)) in b.instructions.iter().enumerate() {
            locations.insert(*v, (bi, ii));
            let proven = (crate::llvm::LlvmEmitter::result_type(i, *t) == *t).then_some(*t);
            if types.insert(*v, proven).is_some() {
                return 0;
            }
        }
    }
    if f.blocks.iter().any(|b| {
        super::cfg::successors(&b.terminator)
            .iter()
            .any(|id| !ids.contains(id))
    }) {
        return 0;
    }
    let cfg = Cfg::new(f);
    let dominates = |a: usize, mut b: usize| {
        if cfg.idom[b] == usize::MAX {
            return false;
        }
        while a != b {
            let next = cfg.idom[b];
            if next == b {
                return false;
            }
            b = next;
        }
        true
    };
    let mut replacements = HashMap::new();
    for (bi, b) in f.blocks.iter().enumerate() {
        if cfg.idom[bi] == usize::MAX || cfg.predecessors[bi].len() != 1 {
            continue;
        }
        let Some((
            boxed,
            Instruction::Box {
                op: Operand::Val(scalar),
                from: Type::F64,
            },
            Type::Ref,
        )) = b.instructions.first()
        else {
            continue;
        };
        if !b.instructions[1..].iter().all(|(_, i, t)| matches!(i, Instruction::CallRuntime { name, args, ret_ty: Type::Void } if name == "dartforge_exception_clear" && args.is_empty()) && *t == Type::Void)
            || !matches!(b.terminator, Terminator::Return(Some(Operand::Val(v))) if v == *boxed)
        {
            continue;
        }
        let mut other_use = false;
        for (ui, user) in f.blocks.iter().enumerate() {
            for (_, inst, _) in &user.instructions {
                operands(inst, &mut |op| {
                    other_use |= matches!(op, Operand::Val(v) if v == boxed);
                });
            }
            if ui != bi {
                terminator_operands(&user.terminator, &mut |op| {
                    other_use |= matches!(op, Operand::Val(v) if v == boxed);
                });
            }
        }
        if other_use {
            continue;
        }
        let pi = cfg.predecessors[bi][0];
        let predecessor = &f.blocks[pi];
        let n = predecessor.instructions.len();
        if n < 3 {
            continue;
        }
        let tail = &predecessor.instructions[n - 3..];
        let (
            unboxed,
            Instruction::Unbox {
                op: Operand::Val(original),
                to: Type::F64,
            },
            Type::F64,
        ) = &tail[0]
        else {
            continue;
        };
        if scalar != unboxed || types.get(original) != Some(&Some(Type::Ref)) {
            continue;
        }
        if let Some(&(definition_block, definition_position)) = locations.get(original) {
            if !dominates(definition_block, pi)
                || (definition_block == pi && definition_position >= n - 3)
            {
                continue;
            }
        } else if !f
            .params
            .iter()
            .any(|(v, _, t)| v == original && *t == Type::Ref)
        {
            continue;
        }
        let (
            pending,
            Instruction::CallRuntime {
                name,
                args,
                ret_ty: Type::I8,
            },
            Type::I8,
        ) = &tail[1]
        else {
            continue;
        };
        if name != "dartforge_exception_pending" || !args.is_empty() {
            continue;
        }
        let (
            condition,
            Instruction::ICmp(ICmpOp::Ne, Operand::Val(read), Operand::Constant(Constant::Int(0))),
            Type::I1,
        ) = &tail[2]
        else {
            continue;
        };
        if pending != read {
            continue;
        }
        let Terminator::CondBranch {
            cond: Operand::Val(test),
            then_block,
            else_block,
        } = predecessor.terminator
        else {
            continue;
        };
        if test != *condition || else_block != b.id || then_block == b.id {
            continue;
        }
        // The checked-success edge is the sole entrance. There is no intervening
        // call, allocation, mutation, join, or backedge between the load and reuse.
        replacements.insert(*boxed, Operand::Val(*original));
    }
    let count = replacements.len();
    if count == 0 {
        return 0;
    }
    let replace = &mut |op: &mut Operand| {
        if let Operand::Val(v) = op
            && let Some(value) = replacements.get(v)
        {
            *op = value.clone();
        }
    };
    for b in &mut f.blocks {
        for (_, i, _) in &mut b.instructions {
            operands_mut(i, replace);
        }
        terminator_operands_mut(&mut b.terminator, replace);
        b.instructions
            .retain(|(v, _, _)| !replacements.contains_key(v));
    }
    count
}

#[cfg(test)]
mod tests;
