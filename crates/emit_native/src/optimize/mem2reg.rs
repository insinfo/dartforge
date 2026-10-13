//! Promotion of locals (an `Alloca` read and written only by `Load`/`Store`) to
//! SSA values, with a `phi` at the dominance frontiers of the writes
//! (Cytron et al.). LLVM would do the same later, but the HIR passes
//! (scalar replacement, inlining) need to see the values; and a promoted `Ref`
//! local no longer has a fixed slot in the roots frame (liveness
//! decides).

use super::cfg::Cfg;
use super::operands::*;
use crate::hir::*;
use std::collections::{HashMap, HashSet};

/// Are the types compatible to replace the read with the written value? The
/// emitter converts between scalars by representation; a `Ref` (a collector
/// root) is only exchanged with a `Ref`.
pub fn compatible(local: Type, value: Type) -> bool {
    local == value || (local != Type::Ref && value != Type::Ref && !local.is_vector() && !value.is_vector())
}

/// Promotes the promotable locals of `func`. Returns whether anything changed.
pub fn promote(func: &mut Function) -> bool {
    let types = function_types(func);
    // The candidates and the blocks that write each one.
    let mut locals: HashMap<ValueId, Type> = HashMap::new();
    for b in &func.blocks {
        for (v, inst, _) in &b.instructions {
            if let Instruction::Alloca(t) = inst
                && !t.is_vector()
                && !matches!(t, Type::Void | Type::Ptr)
            {
                locals.insert(*v, *t);
            }
        }
    }
    if locals.is_empty() {
        return false;
    }
    let mut rejected: HashSet<ValueId> = HashSet::new();
    for b in &func.blocks {
        for (_, inst, _) in &b.instructions {
            match inst {
                Instruction::Load { ptr: Operand::Val(_), .. } => {}
                Instruction::Store { ptr: Operand::Val(p), val } => {
                    if let Some(&t) = locals.get(p) {
                        let ok = match val {
                            Operand::Val(v) if v == p => false,
                            _ => operand_type(val, &types).is_some_and(|tv| compatible(t, tv)),
                        };
                        if !ok {
                            rejected.insert(*p);
                        }
                    }
                    if let Operand::Val(v) = val
                        && locals.contains_key(v)
                    {
                        rejected.insert(*v);
                    }
                }
                another => operands(another, &mut |o| {
                    if let Operand::Val(v) = o
                        && locals.contains_key(v)
                    {
                        rejected.insert(*v);
                    }
                }),
            }
        }
        terminator_operands(&b.terminator, &mut |o| {
            if let Operand::Val(v) = o
                && locals.contains_key(v)
            {
                rejected.insert(*v);
            }
        });
    }
    locals.retain(|v, _| !rejected.contains(v));
    if locals.is_empty() {
        return false;
    }

    let cfg = Cfg::new(func);
    let n = func.blocks.len();
    let df = cfg.boundaries();
    let (mut next_value, _) = largest_ids(func);

    // The `phi`s of each local, per block.
    let mut local_order: Vec<ValueId> = locals.keys().copied().collect();
    local_order.sort_by_key(|v| v.0);
    let mut phis: Vec<Vec<(ValueId, ValueId)>> = vec![Vec::new(); n]; // (local, phi)
    for &local in &local_order {
        let mut defs: Vec<usize> = Vec::new();
        for (i, b) in func.blocks.iter().enumerate() {
            if cfg.reachable(i)
                && b.instructions.iter().any(|(_, inst, _)| matches!(inst, Instruction::Store { ptr: Operand::Val(p), .. } if *p == local))
            {
                defs.push(i);
            }
        }
        let mut has_phi = vec![false; n];
        let mut queue = defs.clone();
        while let Some(b) = queue.pop() {
            for &y in &df[b] {
                if !has_phi[y] {
                    has_phi[y] = true;
                    next_value += 1;
                    phis[y].push((local, ValueId(next_value)));
                    queue.push(y);
                }
            }
        }
    }

    // Renaming over the dominator tree.
    let children = cfg.children();
    let mut swap: HashMap<ValueId, Operand> = HashMap::new();
    let mut entries: Vec<Vec<(ValueId, BlockId, Operand)>> = vec![Vec::new(); n]; // (phi, origem, valor)
    let mut stacks: HashMap<ValueId, Vec<Operand>> = local_order.iter().map(|&l| (l, Vec::new())).collect();
    let ids: Vec<BlockId> = func.blocks.iter().map(|b| b.id).collect();
    // Explicit stack: (block, phase, how many were pushed per local).
    enum Step {
        Enter(usize),
        Exit(Vec<ValueId>),
    }
    let mut work = vec![Step::Enter(0)];
    while let Some(p) = work.pop() {
        match p {
            Step::Exit(stacked) => {
                for l in stacked {
                    stacks.get_mut(&l).expect("local").pop();
                }
            }
            Step::Enter(b) => {
                let mut stacked = Vec::new();
                for &(l, phi) in &phis[b] {
                    stacks.get_mut(&l).expect("local").push(Operand::Val(phi));
                    stacked.push(l);
                }
                for (v, inst, _) in &func.blocks[b].instructions {
                    match inst {
                        Instruction::Load { ptr: Operand::Val(p), .. } if locals.contains_key(p) => {
                            let current = stacks[p].last().cloned().unwrap_or_else(|| default_constant(locals[p]));
                            swap.insert(*v, current);
                        }
                        Instruction::Store { ptr: Operand::Val(p), val } if locals.contains_key(p) => {
                            stacks.get_mut(p).expect("local").push(val.clone());
                            stacked.push(*p);
                        }
                        _ => {}
                    }
                }
                for &s in &cfg.successors[b] {
                    for &(l, phi) in &phis[s] {
                        let current = stacks[&l].last().cloned().unwrap_or_else(|| default_constant(locals[&l]));
                        entries[s].push((phi, ids[b], current));
                    }
                }
                work.push(Step::Exit(stacked));
                for &f in children[b].iter().rev() {
                    work.push(Step::Enter(f));
                }
            }
        }
    }

    // Reads in unreachable blocks: the neutral constant.
    for (i, b) in func.blocks.iter().enumerate() {
        if cfg.reachable(i) {
            continue;
        }
        for (v, inst, _) in &b.instructions {
            if let Instruction::Load { ptr: Operand::Val(p), .. } = inst
                && let Some(&t) = locals.get(p)
            {
                swap.insert(*v, default_constant(t));
            }
        }
    }

    // Builds the `phi`s and removes locals, reads and writes.
    for (i, b) in func.blocks.iter_mut().enumerate() {
        b.instructions.retain(|(v, inst, _)| match inst {
            Instruction::Alloca(_) => !locals.contains_key(v),
            Instruction::Load { ptr: Operand::Val(p), .. } | Instruction::Store { ptr: Operand::Val(p), .. } => {
                !locals.contains_key(p)
            }
            _ => true,
        });
        let new_set: Vec<(ValueId, Instruction, Type)> = phis[i]
            .iter()
            .map(|&(l, phi)| {
                let incoming = entries[i].iter().filter(|(p, _, _)| *p == phi).map(|(_, o, v)| (*o, v.clone())).collect();
                (phi, Instruction::Phi { incoming, ty: locals[&l] }, locals[&l])
            })
            .collect();
        b.instructions.splice(0..0, new_set);
    }
    let solve = |mut o: Operand| {
        let mut steps = 0;
        while let Operand::Val(v) = &o {
            match swap.get(v) {
                Some(n) if steps < 1_000_000 => {
                    o = n.clone();
                    steps += 1;
                }
                _ => break,
            }
        }
        o
    };
    let resolved_set: HashMap<ValueId, Operand> = swap.keys().map(|&k| (k, solve(Operand::Val(k)))).collect();
    replace(func, &|v| resolved_set.get(&v).cloned());
    prune_phis(func, &phis.iter().flatten().map(|&(_, p)| p).collect());
    true
}

/// Removes the inserted `phi`s that nobody uses (other than themselves).
fn prune_phis(func: &mut Function, inserted: &HashSet<ValueId>) {
    loop {
        let mut used_set: HashSet<ValueId> = HashSet::new();
        for b in &func.blocks {
            for (v, inst, _) in &b.instructions {
                operands(inst, &mut |o| {
                    if let Operand::Val(u) = o
                        && u != v
                    {
                        used_set.insert(*u);
                    }
                });
            }
            terminator_operands(&b.terminator, &mut |o| {
                if let Operand::Val(u) = o {
                    used_set.insert(*u);
                }
            });
        }
        let mut removed_done = false;
        for b in &mut func.blocks {
            let before = b.instructions.len();
            b.instructions.retain(|(v, _, _)| !inserted.contains(v) || used_set.contains(v));
            removed_done |= b.instructions.len() != before;
        }
        if !removed_done {
            break;
        }
    }
}
