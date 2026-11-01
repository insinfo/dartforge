//! One return block per function for each constant return (the large program,
//! docs/NATIVO-PRODUCAO-GRANDE.md §9).
//!
//! Each call that may leave a pending exception branches, when the flag is set,
//! to a block of its own that only returns the default value
//! (`FnBuilder::emit_call_with_check`): a function with 200 calls has 200
//! identical exits, each of them a closing of the roots frame and a `ret` in the
//! IR. Those blocks have no instruction and no successor, so no `phi` names them
//! and any one of them can stand for the others: the branches go to the first
//! one and the copies leave the function.

use super::operands::successors_mut;
use crate::hir::*;
use std::collections::HashMap;

/// Joins the empty blocks of `func` that return the same constant (or nothing);
/// the number of blocks removed. A function with debug information keeps them
/// (each exit has its source position).
pub fn share_exits(func: &mut Function) -> usize {
    if func.debug_info.is_some() {
        return 0;
    }
    let key = |b: &BasicBlock| -> Option<String> {
        if !b.instructions.is_empty() {
            return None;
        }
        match &b.terminator {
            Terminator::Return(None) => Some(String::new()),
            Terminator::Return(Some(Operand::Constant(c))) => Some(format!("{c:?}")),
            _ => None,
        }
    };
    let mut first: HashMap<String, BlockId> = HashMap::new();
    let mut redirect: HashMap<BlockId, BlockId> = HashMap::new();
    // The entry block is never redirected (nothing branches to it as an exit).
    for b in func.blocks.iter().skip(1) {
        if let Some(k) = key(b) {
            match first.get(&k) {
                Some(&kept) => {
                    redirect.insert(b.id, kept);
                }
                None => {
                    first.insert(k, b.id);
                }
            }
        }
    }
    if redirect.is_empty() {
        return 0;
    }
    for b in &mut func.blocks {
        successors_mut(&mut b.terminator, &mut |target| {
            if let Some(&kept) = redirect.get(target) {
                *target = kept;
            }
        });
    }
    let before = func.blocks.len();
    func.blocks.retain(|b| !redirect.contains_key(&b.id));
    before - func.blocks.len()
}

/// [`share_exits`] over every function of the module; the blocks removed.
pub fn share_exits_in(module: &mut Module) -> usize {
    module.functions.iter_mut().map(share_exits).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(id: u32, instructions: usize, terminator: Terminator) -> BasicBlock {
        BasicBlock {
            id: BlockId(id),
            instructions: (0..instructions)
                .map(|i| {
                    (
                        ValueId(100 + id * 10 + i as u32),
                        Instruction::Const(Constant::Int(i as i64)),
                        Type::I64,
                    )
                })
                .collect(),
            terminator,
        }
    }

    #[test]
    fn identical_exits_become_one() {
        let ret0 = || Terminator::Return(Some(Operand::Constant(Constant::Int(0))));
        let cond = |t, e| Terminator::CondBranch {
            cond: Operand::Constant(Constant::Bool(true)),
            then_block: BlockId(t),
            else_block: BlockId(e),
        };
        let mut f = Function {
            symbol: "f".into(),
            name: "f".into(),
            params: Vec::new(),
            return_ty: Type::I64,
            blocks: vec![
                block(0, 1, cond(1, 2)),
                block(1, 0, ret0()),
                block(2, 1, cond(3, 4)),
                block(3, 0, ret0()),
                block(
                    4,
                    0,
                    Terminator::Return(Some(Operand::Constant(Constant::Int(7)))),
                ),
            ],
            debug_info: None,
        };
        assert_eq!(share_exits(&mut f), 1);
        let ids: Vec<u32> = f.blocks.iter().map(|b| b.id.0).collect();
        assert_eq!(ids, vec![0, 1, 2, 4]);
        assert!(matches!(
            f.blocks[2].terminator,
            Terminator::CondBranch {
                then_block: BlockId(1),
                else_block: BlockId(4),
                ..
            }
        ));
    }
}
