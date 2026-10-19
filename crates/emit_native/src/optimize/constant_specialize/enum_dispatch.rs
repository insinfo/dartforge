//! Guard observed enum indices instead of assuming that identity proves heap contents.
use crate::hir::*;
use crate::optimize::{cfg::Cfg, enum_constants::EnumToken};
use std::collections::{HashMap, HashSet};

const MAX_GUARDS: usize = 32;
const MAX_PREFIX_BLOCKS: usize = 32;
const MAX_PREFIX_INSTRUCTIONS: usize = 256;

/// Specialize switch edges behind runtime index guards. A miss calls the
/// original ABI and forwards its result. Only a unique, effect-free entry prefix may be replayed;
/// every field read remains in its original location and feeds the guard.
pub(super) fn guard_switches(
    function: &mut Function,
    formals: &HashMap<ValueId, EnumToken>,
    original: &str,
) -> usize {
    if formals.is_empty() || function.blocks.is_empty() {
        return 0;
    }
    let Some(types) = super::types(function) else {
        return 0;
    };
    let cfg = Cfg::new(function);
    if !cfg.predecessors[0].is_empty() {
        return 0;
    }
    let mut plans = Vec::new();
    for (position, block) in function.blocks.iter().enumerate() {
        let Terminator::Switch {
            val: Operand::Val(value),
            default,
            cases,
        } = &block.terminator
        else {
            continue;
        };
        if types.get(value) != Some(&Type::I64)
            || !cfg.reachable(position)
            || cases
                .iter()
                .map(|(key, _)| key)
                .collect::<HashSet<_>>()
                .len()
                != cases.len()
        {
            continue;
        }
        let Some(expected) = block
            .instructions
            .iter()
            .find_map(|(result, instruction, ty)| match instruction {
                Instruction::GetField {
                    object: Operand::Val(parameter),
                    index: 0,
                } if result == value
                    && *ty == Type::I64
                    && types.get(parameter) == Some(&Type::Ref) =>
                {
                    formals
                        .get(parameter)
                        .and_then(|token| i64::try_from(token.index).ok())
                }
                _ => None,
            })
        else {
            continue;
        };
        if !replayable_prefix(function, &cfg, position, formals, &types) {
            continue;
        }
        let destination = cases
            .iter()
            .find(|(key, _)| *key == expected)
            .map_or(*default, |(_, target)| *target);
        plans.push((position, *value, expected, destination));
        if plans.len() > MAX_GUARDS {
            return 0;
        }
    }
    if plans.is_empty() {
        return 0;
    }
    let Some(block_id) = function
        .blocks
        .iter()
        .map(|b| b.id.0)
        .max()
        .unwrap()
        .checked_add(1)
    else {
        return 0;
    };
    let maximum = function
        .params
        .iter()
        .map(|(v, _, _)| v.0)
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|b| b.instructions.iter().map(|(v, _, _)| v.0)),
        )
        .max()
        .unwrap_or(0);
    let Some(first_value) = maximum.checked_add(1) else {
        return 0;
    };
    let Ok(extra) = u32::try_from(plans.len()) else {
        return 0;
    };
    let Some(return_value) = first_value.checked_add(extra) else {
        return 0;
    };
    let fallback = BlockId(block_id);
    for (offset, (position, value, expected, destination)) in plans.iter().enumerate() {
        let condition = ValueId(first_value + offset as u32);
        let block = &mut function.blocks[*position];
        block.instructions.push((
            condition,
            Instruction::ICmp(
                ICmpOp::Eq,
                Operand::Val(*value),
                Operand::Constant(Constant::Int(*expected)),
            ),
            Type::I1,
        ));
        block.terminator = Terminator::CondBranch {
            cond: Operand::Val(condition),
            then_block: *destination,
            else_block: fallback,
        };
    }
    // Removing a switch edge also removes that predecessor's phi entry, even
    // when the destination remains reachable through another branch.
    let destinations: HashMap<_, _> = plans
        .iter()
        .map(|(position, _, _, destination)| (function.blocks[*position].id, *destination))
        .collect();
    for block in &mut function.blocks {
        for (_, instruction, _) in &mut block.instructions {
            if let Instruction::Phi { incoming, .. } = instruction {
                incoming.retain(|(predecessor, _)| {
                    destinations
                        .get(predecessor)
                        .is_none_or(|destination| *destination == block.id)
                });
            }
        }
    }
    function.blocks.push(BasicBlock {
        id: fallback,
        instructions: vec![(
            ValueId(return_value),
            Instruction::CallStatic {
                symbol: original.into(),
                args: function
                    .params
                    .iter()
                    .map(|(value, _, _)| Operand::Val(*value))
                    .collect(),
                ret_ty: function.return_ty,
            },
            function.return_ty,
        )],
        // Propagate the original function's pending exception unchanged. No
        // exception clear/check or ABI conversion is inserted on this tail path.
        terminator: Terminator::Return(
            (function.return_ty != Type::Void).then_some(Operand::Val(ValueId(return_value))),
        ),
    });
    plans.len()
}

fn replayable_prefix(
    function: &Function,
    cfg: &Cfg,
    mut block: usize,
    formals: &HashMap<ValueId, EnumToken>,
    types: &HashMap<ValueId, Type>,
) -> bool {
    let mut path = Vec::new();
    let mut seen = HashSet::new();
    loop {
        if path.len() >= MAX_PREFIX_BLOCKS || !seen.insert(block) {
            return false;
        }
        path.push(block);
        if block == 0 {
            break;
        }
        let [parent] = cfg.predecessors[block].as_slice() else {
            return false;
        };
        block = *parent;
    }
    let mut available: HashSet<_> = function.params.iter().map(|(v, _, _)| *v).collect();
    let mut work = 0;
    for &block in path.iter().rev() {
        for (value, instruction, ty) in &function.blocks[block].instructions {
            work += 1;
            if work > MAX_PREFIX_INSTRUCTIONS {
                return false;
            }
            let allowed = match instruction {
                Instruction::Const(Constant::Int(_)) => matches!(ty, Type::I64 | Type::I8),
                Instruction::Const(Constant::Bool(_)) => *ty == Type::I1,
                Instruction::ICmp(ICmpOp::Eq | ICmpOp::Ne, _, _) => *ty == Type::I1,
                Instruction::GetField {
                    object: Operand::Val(parameter),
                    index: 0,
                } => {
                    *ty == Type::I64
                        && types.get(parameter) == Some(&Type::Ref)
                        && formals.contains_key(parameter)
                }
                _ => false,
            };
            if !allowed {
                return false;
            }
            let mut valid = true;
            crate::optimize::operands::operands(instruction, &mut |operand| {
                if let Operand::Val(source) = operand {
                    valid &= available.contains(source);
                }
            });
            if !valid {
                return false;
            }
            available.insert(*value);
        }
        let mut valid = true;
        crate::optimize::operands::terminator_operands(
            &function.blocks[block].terminator,
            &mut |operand| {
                if let Operand::Val(value) = operand {
                    valid &= available.contains(value);
                }
            },
        );
        if !valid {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests;
