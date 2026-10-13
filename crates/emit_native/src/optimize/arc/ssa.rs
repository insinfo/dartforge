//! Structural preconditions for publishing the ARC plans of a function.
//! Complete typing and provenance still belong to other passes.

use super::super::{
    cfg::{Cfg, successors},
    operands::{operands, terminator_operands},
};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

pub(super) fn verify(f: &Function) -> Result<(), String> {
    let error = |m: String| format!("SSA ARC em {}: {m}", f.symbol);
    if f.blocks.is_empty() {
        return Err(error("função sem entrada".into()));
    }
    let mut blocks = HashMap::new();
    let mut defs = HashMap::new();
    for (v, _, _) in &f.params {
        if defs.insert(*v, (None, 0, false)).is_some() {
            return Err(error(format!("v{} repetido", v.0)));
        }
    }
    for (bi, b) in f.blocks.iter().enumerate() {
        if blocks.insert(b.id, bi).is_some() {
            return Err(error(format!("b{} repetido", b.id.0)));
        }
        let mut ordinary = false;
        for (i, (v, inst, _)) in b.instructions.iter().enumerate() {
            let phi = matches!(inst, Instruction::Phi { .. });
            if phi && ordinary {
                return Err(error(format!("Phi v{} após instrução ordinária", v.0)));
            }
            ordinary |= !phi;
            if defs.insert(*v, (Some(bi), i, phi)).is_some() {
                return Err(error(format!("v{} repetido", v.0)));
            }
        }
    }
    for b in &f.blocks {
        for s in successors(&b.terminator) {
            if !blocks.contains_key(&s) {
                return Err(error(format!("b{} aponta para b{} ausente", b.id.0, s.0)));
            }
        }
    }
    let cfg = Cfg::new(f);
    let types: HashMap<_, _> = f
        .params
        .iter()
        .map(|(v, _, ty)| (*v, *ty))
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|(v, _, ty)| (*v, *ty)),
        )
        .collect();
    let reference = |op: &Operand| {
        matches!(op, Operand::Constant(Constant::Null))
            || matches!(op, Operand::Val(v) if types.get(v) == Some(&Type::Ref))
    };
    let check = |op: &Operand, bi: usize, pos: usize| -> Result<(), String> {
        let Operand::Val(v) = op else { return Ok(()) };
        let Some(&(origin, i, phi)) = defs.get(v) else {
            return Err(error(format!("v{} ausente", v.0)));
        };
        let Some(origin) = origin else { return Ok(()) };
        // In dead blocks, require existence; do not invent dominators.
        if !cfg.reachable(bi) {
            return Ok(());
        }
        let dominates = if origin == bi {
            phi || i < pos
        } else {
            let mut current = bi;
            loop {
                if current == origin {
                    break true;
                }
                if cfg.idom[current] == current {
                    break false;
                }
                current = cfg.idom[current];
            }
        };
        if dominates {
            Ok(())
        } else {
            Err(error(format!(
                "v{} não domina uso em b{}",
                v.0, f.blocks[bi].id.0
            )))
        }
    };
    for (bi, b) in f.blocks.iter().enumerate() {
        for (pos, (v, inst, _)) in b.instructions.iter().enumerate() {
            let mut absent = None;
            operands(inst, &mut |op| {
                if let Operand::Val(arg) = op
                    && !defs.contains_key(arg)
                    && absent.is_none()
                {
                    absent = Some(*arg);
                }
            });
            if let Some(arg) = absent {
                return Err(error(format!("v{} ausente", arg.0)));
            }
            match inst {
                Instruction::ArcCopy { value }
                | Instruction::ArcMove { value }
                | Instruction::ArcDrop { value }
                | Instruction::ArcStoreStrong { value, .. }
                    if !reference(value) =>
                {
                    return Err(error(format!(
                        "v{}: operação ARC exige referência SSA ou null",
                        v.0
                    )));
                }
                _ => {}
            }
            if let Instruction::ArcLoadStrong { slot } | Instruction::ArcStoreStrong { slot, .. } =
                inst
            {
                if let StrongSlot::Quadro { frame, .. } = slot
                    && !matches!(frame, Operand::Val(q) if types.get(q) == Some(&Type::I64))
                {
                    return Err(error(format!("v{}: quadro ARC exige SSA I64", v.0)));
                }
            }
            if let Instruction::Phi { incoming, ty, .. } = inst {
                let mut entries = HashSet::new();
                for (p, op) in incoming {
                    if *ty == Type::Ref && !reference(op) {
                        return Err(error(format!("Phi v{} exige entrada SSA Ref ou null", v.0)));
                    }
                    let Some(&pi) = blocks.get(p) else {
                        return Err(error(format!("predecessor b{} ausente", p.0)));
                    };
                    if !cfg.predecessors[bi].contains(&pi) || !entries.insert(pi) {
                        return Err(error(format!(
                            "entrada Phi inválida de b{} para b{}",
                            p.0, b.id.0
                        )));
                    }
                    check(op, pi, f.blocks[pi].instructions.len())?;
                }
                if entries.len() != cfg.predecessors[bi].len() {
                    return Err(error(format!("Phi em b{} não cobre predecessores", b.id.0)));
                }
            } else {
                let mut result = Ok(());
                operands(inst, &mut |op| {
                    if result.is_ok() {
                        result = check(op, bi, pos);
                    }
                });
                result?;
            }
        }
        let mut result = Ok(());
        terminator_operands(&b.terminator, &mut |op| {
            if result.is_ok() {
                result = check(op, bi, b.instructions.len());
            }
        });
        result?;
        if f.return_ty == Type::Ref
            && let Terminator::Return(Some(op)) = &b.terminator
            && !reference(op)
            && !matches!(
                op,
                Operand::Constant(Constant::String(_) | Constant::StringWtf8(_))
            )
        {
            return Err(error(format!(
                "retorno Ref em b{} exige SSA Ref, null ou literal permanente",
                b.id.0
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ref_return_does_not_accept_scalar_with_owned_contract() {
        let f = Function {
            symbol: "retorno_ref".into(),
            name: "retorno_ref".into(),
            debug_info: None,
            params: vec![(ValueId(0), "bits".into(), Type::I64)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        };
        let mut classes = HashMap::from([(ValueId(0), super::super::Ownership::Owned)]);
        let before = classes.clone();
        let mut plan = super::super::TokenPlan {
            ret: super::super::TokenReturn::Owned,
            ..Default::default()
        };
        assert!(
            super::super::produce_and_verify_tokens(
                &f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &super::super::ScopePlan::default()
            )
            .unwrap_err()
            .contains("retorno Ref")
        );
        assert_eq!(classes, before);
        assert!(plan.instructions.is_empty());
        let mut valid = f.clone();
        valid.params[0].2 = Type::Ref;
        super::super::produce_and_verify_tokens(
            &valid,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &super::super::ScopePlan::default(),
        )
        .unwrap();
        for constant in [
            Constant::Null,
            Constant::String("literal".into()),
            Constant::StringWtf8(vec![120]),
        ] {
            valid.blocks[0].terminator = Terminator::Return(Some(Operand::Constant(constant)));
            verify(&valid).unwrap();
        }
    }

    fn function() -> Function {
        Function {
            symbol: "ssa".into(),
            name: "ssa".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(1),
                    Instruction::ArcCopy {
                        value: Operand::Val(ValueId(0)),
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
            }],
        }
    }

    #[test]
    fn arc_operations_do_not_create_references_by_annotation() {
        let mut f = function();
        f.params[0].2 = Type::I64;
        assert!(
            verify(&f)
                .unwrap_err()
                .contains("referência SSA ou null")
        );
        f.blocks[0].instructions[0].1 = Instruction::ArcCopy {
            value: Operand::Constant(Constant::Int(0)),
        };
        assert!(verify(&f).is_err());
        f.blocks[0].instructions[0].1 = Instruction::ArcCopy {
            value: Operand::Constant(Constant::Null),
        };
        verify(&f).unwrap();
        f.blocks[0].instructions[0].1 = Instruction::ArcLoadStrong {
            slot: StrongSlot::Quadro {
                frame: Operand::Val(ValueId(0)),
                index: 0,
            },
        };
        verify(&f).unwrap();
        f.params[0].2 = Type::Ref;
        assert!(
            verify(&f)
                .unwrap_err()
                .contains("quadro ARC exige SSA I64")
        );
    }

    #[test]
    fn rejects_invalid_destinations_ids_and_uses() {
        let mut f = function();
        verify(&f).unwrap();
        f.blocks[0].terminator = Terminator::Branch(BlockId(99));
        assert!(verify(&f).unwrap_err().contains("b99 ausente"));
        let mut classes = HashMap::from([(ValueId(0), super::super::Ownership::Owned)]);
        let before = classes.clone();
        let mut plan = super::super::TokenPlan::default();
        assert!(
            super::super::produce_and_verify_tokens(
                &f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &super::super::ScopePlan::default()
            )
            .is_err()
        );
        assert_eq!(classes, before);
        assert!(plan.instructions.is_empty());
        f = function();
        f.blocks[0].instructions[0].0 = ValueId(0);
        assert!(verify(&f).unwrap_err().contains("v0 repetido"));
        f = function();
        f.blocks[0].instructions[0].1 = Instruction::ArcCopy {
            value: Operand::Val(ValueId(1)),
        };
        assert!(verify(&f).unwrap_err().contains("não domina"));
        f.blocks[0].instructions[0].1 = Instruction::ArcCopy {
            value: Operand::Val(ValueId(99)),
        };
        assert!(verify(&f).unwrap_err().contains("v99 ausente"));
    }

    #[test]
    fn loop_phi_uses_edge_value_and_requires_coverage() {
        let mut f = function();
        f.blocks[0].terminator = Terminator::Branch(BlockId(1));
        f.blocks.push(BasicBlock {
            id: BlockId(1),
            instructions: vec![
                (
                    ValueId(2),
                    Instruction::Phi {
                        ty: Type::Ref,
                        incoming: vec![
                            (BlockId(0), Operand::Val(ValueId(1))),
                            (BlockId(1), Operand::Val(ValueId(3))),
                        ],
                    },
                    Type::Ref,
                ),
                (
                    ValueId(3),
                    Instruction::ArcMove {
                        value: Operand::Val(ValueId(2)),
                    },
                    Type::Ref,
                ),
            ],
            terminator: Terminator::Branch(BlockId(1)),
        });
        verify(&f).unwrap();
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming[0].1 = Operand::Val(ValueId(3));
        }
        assert!(verify(&f).unwrap_err().contains("não domina"));
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming[0].1 = Operand::Val(ValueId(1));
        }
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming.pop();
        }
        assert!(verify(&f).unwrap_err().contains("não cobre"));
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming.push((BlockId(0), Operand::Val(ValueId(1))));
        }
        assert!(verify(&f).unwrap_err().contains("entrada Phi inválida"));
    }
}
