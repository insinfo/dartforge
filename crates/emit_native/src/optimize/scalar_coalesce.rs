//! Keep guarded nullable integer intermediates scalar without changing exception edges.

use super::{
    cfg::Cfg,
    operands::{operands, terminator_operands},
};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

/// Removes closed integer box/null merges in tracing code, including SDK bodies.
pub(super) fn rewrite(module: &mut Module) -> usize {
    if module.arc_memory
        || module.arc_prepared
        || module.arc_aot_ports.is_some()
        || std::env::var("DARTFORGE_NO_SCALAR_COALESCE").as_deref() == Ok("1")
        || module.functions.iter().any(|f| {
            matches!(
                f.symbol.as_str(),
                "dartforge_unbox_int" | "df.box_int" | "df.unbox_int" | "df.allocate"
            )
        })
    {
        return 0;
    }
    module.functions.iter_mut().map(rewrite_function).sum()
}

/// Preserves predecessor labels and all calls, pending checks and terminators.
fn rewrite_function(f: &mut Function) -> usize {
    if f.blocks.is_empty()
        || f.blocks.len() > 512
        || f.blocks.iter().map(|b| b.instructions.len()).sum::<usize>() > 8192
    {
        return 0;
    }
    let mut blocks = HashMap::new();
    let mut types = HashMap::new();
    let mut definitions = HashMap::new();
    let mut uses: HashMap<ValueId, Vec<(usize, usize)>> = HashMap::new();
    for (v, _, t) in &f.params {
        if types.insert(*v, Some(*t)).is_some() {
            return 0;
        }
    }
    for (bi, b) in f.blocks.iter().enumerate() {
        if blocks.insert(b.id, bi).is_some() {
            return 0;
        }
        for (ii, (v, i, t)) in b.instructions.iter().enumerate() {
            if types
                .insert(
                    *v,
                    (crate::llvm::LlvmEmitter::result_type(i, *t) == *t).then_some(*t),
                )
                .is_some()
            {
                return 0;
            }
            definitions.insert(*v, (bi, ii));
            operands(i, &mut |op| {
                if let Operand::Val(v) = op {
                    uses.entry(*v).or_default().push((bi, ii));
                }
            });
        }
        terminator_operands(&b.terminator, &mut |op| {
            if let Operand::Val(v) = op {
                uses.entry(*v).or_default().push((bi, usize::MAX));
            }
        });
    }
    if f.blocks.iter().any(|b| {
        super::cfg::successors(&b.terminator)
            .iter()
            .any(|id| !blocks.contains_key(id))
    }) {
        return 0;
    }
    let cfg = Cfg::new(f);
    if !cfg.predecessors[0].is_empty() {
        return 0;
    }
    // Acyclic functions make edge availability and exclusion of reentry explicit.
    let mut indegree: Vec<_> = cfg.predecessors.iter().map(Vec::len).collect();
    let mut ready: Vec<_> = indegree
        .iter()
        .enumerate()
        .filter_map(|(i, n)| (*n == 0).then_some(i))
        .collect();
    let mut seen = 0;
    while let Some(b) = ready.pop() {
        seen += 1;
        for &s in &cfg.successors[b] {
            indegree[s] -= 1;
            if indegree[s] == 0 {
                ready.push(s);
            }
        }
    }
    if seen != f.blocks.len() {
        return 0;
    }
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
    let available = |op: &Operand, bi: usize, ii: usize| match op {
        Operand::Constant(Constant::Int(_)) => true,
        Operand::Val(v) if types.get(v) == Some(&Some(Type::I64)) => definitions
            .get(v)
            .is_none_or(|&(db, di)| dominates(db, bi) && (db != bi || di < ii)),
        _ => false,
    };
    let Some(mut fresh) = types.keys().map(|v| v.0).max().unwrap_or(0).checked_add(1) else {
        return 0;
    };
    let mut edits = Vec::new();
    for (bi, b) in f.blocks.iter().enumerate() {
        if cfg.idom[bi] == usize::MAX {
            continue;
        }
        for (ii, (phi, inst, tuple)) in b.instructions.iter().enumerate() {
            let Instruction::Phi {
                incoming,
                ty: Type::Ref,
            } = inst
            else {
                continue;
            };
            if *tuple != Type::Ref || incoming.len() != 2 || cfg.predecessors[bi].len() != 2 {
                continue;
            }
            let labels: HashSet<_> = incoming.iter().map(|(id, _)| *id).collect();
            if labels.len() != 2
                || cfg.predecessors[bi]
                    .iter()
                    .any(|p| !labels.contains(&f.blocks[*p].id))
            {
                continue;
            }
            let mut scalar_inputs = Vec::new();
            let mut null_inputs = Vec::new();
            let mut boxed_location = None;
            let mut valid = true;
            for (label, op) in incoming {
                match op {
                    Operand::Constant(Constant::Null) => {
                        scalar_inputs.push((*label, Operand::Constant(Constant::Int(0))));
                        null_inputs.push((*label, Operand::Constant(Constant::Bool(true))));
                    }
                    Operand::Val(v) => {
                        let Some(&(db, di)) = definitions.get(v) else {
                            valid = false;
                            break;
                        };
                        let (_, producer, ty) = &f.blocks[db].instructions[di];
                        let Instruction::Box {
                            op: scalar,
                            from: Type::I64,
                        } = producer
                        else {
                            valid = false;
                            break;
                        };
                        if *ty != Type::Ref
                            || types.get(v) != Some(&Some(Type::Ref))
                            || uses.get(v).map(Vec::as_slice) != Some(&[(bi, ii)][..])
                            || f.blocks[db].id != *label
                            || !available(scalar, db, di)
                            || boxed_location.is_some()
                        {
                            valid = false;
                            break;
                        }
                        boxed_location = Some((db, di));
                        scalar_inputs.push((*label, scalar.clone()));
                        null_inputs.push((*label, Operand::Constant(Constant::Bool(false))));
                    }
                    _ => {
                        valid = false;
                        break;
                    }
                }
            }
            let Some(box_location) = boxed_location else {
                continue;
            };
            if !valid {
                continue;
            }
            let Some(phi_uses) = uses.get(phi) else {
                continue;
            };
            if phi_uses.len() != 2 {
                continue;
            }
            let mut query = None;
            let mut unbox = None;
            for &(ub, ui) in phi_uses {
                if ui == usize::MAX {
                    valid = false;
                    break;
                }
                let (v, user, ty) = &f.blocks[ub].instructions[ui];
                match user {
                    Instruction::ICmp(
                        op @ (ICmpOp::Eq | ICmpOp::Ne),
                        Operand::Val(p),
                        Operand::Constant(Constant::Null),
                    ) if p == phi && *ty == Type::I1 && ub == bi && ui > ii => {
                        query = Some((*v, ui, *op))
                    }
                    Instruction::Unbox {
                        op: Operand::Val(p),
                        to: Type::I64,
                    } if p == phi && *ty == Type::I64 && ui == 0 => unbox = Some((ub, ui)),
                    _ => valid = false,
                }
            }
            let (Some((condition, qi, comparison)), Some((ub, ui))) = (query, unbox) else {
                continue;
            };
            let Terminator::CondBranch {
                cond: Operand::Val(c),
                then_block,
                else_block,
            } = b.terminator
            else {
                continue;
            };
            let nonnull = if comparison == ICmpOp::Eq {
                else_block
            } else {
                then_block
            };
            if !valid
                || c != condition
                || then_block == else_block
                || f.blocks[ub].id != nonnull
                || cfg.predecessors[ub] != [bi]
                || !dominates(bi, ub)
                || types.get(&condition) != Some(&Some(Type::I1))
                || types.get(&f.blocks[ub].instructions[ui].0) != Some(&Some(Type::I64))
            {
                continue;
            }
            let Some(next) = fresh.checked_add(1) else {
                return 0;
            };
            edits.push((
                bi,
                ii,
                qi,
                ub,
                ui,
                box_location,
                ValueId(fresh),
                comparison,
                scalar_inputs,
                null_inputs,
            ));
            fresh = next;
        }
    }
    let count = edits.len();
    let mut removals = HashSet::new();
    let mut additions = Vec::new();
    for (bi, ii, qi, ub, ui, boxed, null_id, comparison, scalar_inputs, null_inputs) in edits {
        let phi = f.blocks[bi].instructions[ii].0;
        f.blocks[bi].instructions[ii].1 = Instruction::Phi {
            incoming: scalar_inputs,
            ty: Type::I64,
        };
        f.blocks[bi].instructions[ii].2 = Type::I64;
        f.blocks[bi].instructions[qi].1 = Instruction::ICmp(
            ICmpOp::Eq,
            Operand::Val(null_id),
            Operand::Constant(Constant::Bool(comparison == ICmpOp::Eq)),
        );
        f.blocks[ub].instructions[ui].1 =
            Instruction::Add(Operand::Val(phi), Operand::Constant(Constant::Int(0)));
        additions.push((bi, null_id, null_inputs));
        removals.insert(boxed);
    }
    for (bi, b) in f.blocks.iter_mut().enumerate() {
        let mut ii = 0;
        b.instructions.retain(|_| {
            let keep = !removals.contains(&(bi, ii));
            ii += 1;
            keep
        });
    }
    for (bi, null_id, null_inputs) in additions {
        let prefix = f.blocks[bi]
            .instructions
            .iter()
            .take_while(|(_, i, _)| matches!(i, Instruction::Phi { .. }))
            .count();
        f.blocks[bi].instructions.insert(
            prefix,
            (
                null_id,
                Instruction::Phi {
                    incoming: null_inputs,
                    ty: Type::I1,
                },
                Type::I1,
            ),
        );
    }
    count
}

#[cfg(test)]
mod tests;
