//! Cleanup of dead tokens on the normal edges of the ARC transaction.
//! Preserves Phi transfers and borrow dependencies. Landing pads and
//! exceptional edges require the cleanup protocol already prepared by the lowering.
//! Semantic lifetime extensions must be materialized as uses in the HIR;
//! this pass does not deduce the absence of a Finalizable, a capture or a suspension.

use super::*;

pub(super) fn next_val(f: &Function, p: &DartFunctionPlan) -> u64 {
    f.params
        .iter()
        .map(|(v, _, _)| v.0)
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| b.instructions.iter().map(|(v, _, _)| v.0)),
        )
        .chain(p.classes.keys().map(|v| v.0))
        .chain(p.tokens.instructions.keys().map(|v| v.0))
        .chain(p.tokens.pendings.keys().map(|v| v.0))
        .chain(p.tables.invocations.keys().map(|v| v.0))
        .chain(p.scopes.before.keys().map(|v| v.0))
        .max()
        .map_or(0, |v| u64::from(v) + 1)
}

// Operates only on private bodies/maps of the closed set.
pub(super) fn prepare(
    f: &mut Function,
    p: &mut DartFunctionPlan,
) -> Result<(usize, usize), String> {
    let mut next_item = next_val(f, p);
    let mut id = || {
        let v = u32::try_from(next_item).map_err(|_| "SSA IDs exhausted while preparing ARC edges")?;
        next_item += 1;
        Ok(ValueId(v))
    };
    // The linear analysis must see the Owned transfer of the return before
    // deciding what is dead on the previous edges.
    let mut copies = super::contracts::copy_returns(f, &p.classes, &mut id)?;
    super::produce_arc_contracts(f, &mut p.classes, &mut p.tokens)?;
    let mut drops = 0;
    loop {
        let plans = super::tokens::cleanup_edges(f, &p.classes, &p.tables, &p.tokens)?;
        if plans.is_empty() {
            break;
        }
        let mut plans: Vec<_> = plans.into_iter().collect();
        plans.sort_unstable_by_key(|((a, b), _)| (a.0, b.0));
        let mut value = next_val(f, p);
        let mut block = f
            .blocks
            .iter()
            .map(|b| b.id.0)
            .chain(p.tables.exits.keys().map(|b| b.0))
            .chain(p.tables.landing_pads.iter().map(|b| b.0))
            .chain(p.tables.invocations.values().map(|b| b.0))
            .chain(p.tokens.pendings.values().map(|b| b.0))
            .chain(p.scopes.exits.keys().map(|b| b.0))
            .chain(p.scopes.edges.keys().flat_map(|(a, b)| [a.0, b.0]))
            .max()
            .map_or(0, |b| u64::from(b) + 1);
        for ((origin, dest), dead_set) in plans {
            let new = BlockId(
                u32::try_from(block)
                    .map_err(|_| "block IDs exhausted while preparing ARC edges")?,
            );
            block += 1;
            let mut instructions = Vec::new();
            for dead in dead_set {
                let v = ValueId(
                    u32::try_from(value)
                        .map_err(|_| "SSA IDs exhausted while preparing ARC edges")?,
                );
                value += 1;
                instructions.push((
                    v,
                    Instruction::ArcDrop {
                        value: Operand::Val(dead),
                    },
                    Type::Void,
                ));
                drops += 1;
            }
            let b = f.blocks.iter_mut().find(|b| b.id == origin).unwrap();
            match &mut b.terminator {
                Terminator::Branch(target) => {
                    if *target == dest {
                        *target = new;
                    }
                }
                Terminator::CondBranch {
                    then_block,
                    else_block,
                    ..
                } => {
                    for target in [then_block, else_block] {
                        if *target == dest {
                            *target = new;
                        }
                    }
                }
                Terminator::Switch { default, cases, .. } => {
                    if *default == dest {
                        *default = new;
                    }
                    for (_, target) in cases {
                        if *target == dest {
                            *target = new;
                        }
                    }
                }
                _ => return Err("cleanup edge without a corresponding branch".into()),
            }
            for b in &mut f.blocks {
                if b.id != dest {
                    continue;
                }
                for (_, inst, _) in &mut b.instructions {
                    if let Instruction::Phi { incoming, .. } = inst {
                        for (of, _) in incoming {
                            if *of == origin {
                                *of = new;
                            }
                        }
                    }
                }
            }
            if let Some(limits) = p.scopes.edges.remove(&(origin, dest)) {
                p.scopes.edges.insert((new, dest), limits);
            }
            f.blocks.push(BasicBlock {
                id: new,
                instructions,
                terminator: Terminator::Branch(dest),
            });
        }
        super::produce_arc_contracts(f, &mut p.classes, &mut p.tokens)?;
        // Release is a conservative barrier; new keepalives must
        // enter the next inventory before more drops are planned.
        copies += super::borrows::protect(f, p)?;
    }
    Ok((copies, drops))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn critical_edge_closes_dead_owner_and_carries_limit_without_changing_tracing() {
        for case in 0..4 {
            let arc = case != 0;
            let mut m = Module::new();
            m.arc_memory = arc;
            m.functions.push(Function {
                symbol: "critica".into(),
                name: "critica".into(),
                debug_info: None,
                params: vec![
                    (ValueId(0), "x".into(), Type::Ref),
                    (ValueId(1), "cond".into(), Type::I1),
                ],
                return_ty: Type::Void,
                blocks: vec![
                    BasicBlock {
                        id: BlockId(0),
                        instructions: vec![],
                        terminator: Terminator::CondBranch {
                            cond: Operand::Val(ValueId(1)),
                            then_block: BlockId(1),
                            else_block: BlockId(2),
                        },
                    },
                    BasicBlock {
                        id: BlockId(1),
                        instructions: vec![(
                            ValueId(2),
                            Instruction::ArcCopy {
                                value: Operand::Val(ValueId(0)),
                            },
                            Type::Ref,
                        )],
                        terminator: Terminator::CondBranch {
                            cond: Operand::Val(ValueId(1)),
                            then_block: BlockId(3),
                            else_block: BlockId(4),
                        },
                    },
                    BasicBlock {
                        id: BlockId(2),
                        instructions: vec![],
                        terminator: Terminator::Branch(BlockId(3)),
                    },
                    BasicBlock {
                        id: BlockId(3),
                        instructions: vec![],
                        terminator: Terminator::Return(None),
                    },
                    BasicBlock {
                        id: BlockId(4),
                        instructions: vec![(
                            ValueId(3),
                            Instruction::ArcDrop {
                                value: Operand::Val(ValueId(2)),
                            },
                            Type::Void,
                        )],
                        terminator: Terminator::Branch(BlockId(3)),
                    },
                ],
            });
            if case == 2 {
                m.functions[0].blocks[2].instructions.push((
                    ValueId(u32::MAX),
                    Instruction::Const(Constant::Int(0)),
                    Type::I64,
                ));
            } else if case == 3 {
                m.functions[0].blocks.push(BasicBlock {
                    id: BlockId(u32::MAX),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                });
            }
            let mut p = DartFunctionPlan::default();
            p.scopes
                .before
                .insert(ValueId(2), vec![ScopeChange::Open(7)]);
            for b in [3, 4] {
                p.scopes
                    .edges
                    .insert((BlockId(1), BlockId(b)), vec![ScopeChange::Close(7)]);
            }
            let mut plans = HashMap::from([("critica".into(), p)]);
            let before = format!("{m:?}/{plans:?}");
            if case >= 2 {
                assert!(
                    prepare_arc_dart_module(&mut m, &mut plans)
                        .unwrap_err()
                        .contains("IDs")
                );
                assert_eq!(format!("{m:?}/{plans:?}"), before);
                continue;
            }
            assert_eq!(
                prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                (0, usize::from(arc))
            );
            if !arc {
                assert_eq!(format!("{m:?}/{plans:?}"), before);
                continue;
            }
            let new = m.functions[0].blocks.last().unwrap();
            assert_eq!(new.id, BlockId(5));
            assert!(matches!(
                new.instructions[0].1,
                Instruction::ArcDrop {
                    value: Operand::Val(ValueId(2))
                }
            ));
            assert_eq!(
                plans["critica"].scopes.edges[&(BlockId(5), BlockId(3))],
                vec![ScopeChange::Close(7)]
            );
            assert!(
                !plans["critica"]
                    .scopes
                    .edges
                    .contains_key(&(BlockId(1), BlockId(3)))
            );
            assert!(matches!(
                m.functions[0].blocks[1].terminator,
                Terminator::CondBranch {
                    then_block: BlockId(5),
                    else_block: BlockId(4),
                    ..
                }
            ));
            let prepared = format!("{m:?}/{plans:?}");
            assert_eq!(
                prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                (0, 0)
            );
            assert_eq!(format!("{m:?}/{plans:?}"), prepared);
            crate::llvm::LlvmEmitter::new(&m).emit_all();
        }
    }

    #[test]
    fn borrowed_phi_keeps_owner_until_return_copy() {
        let mut m = Module::new();
        m.arc_memory = true;
        let read = |v| {
            (
                ValueId(v),
                Instruction::CallRuntime {
                    name: "dartforge_arc_read_field_ref_v1".into(),
                    args: vec![
                        (Operand::Val(ValueId(2)), Type::Ref),
                        (Operand::Constant(Constant::Int(0)), Type::I64),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        };
        m.functions.push(Function {
            symbol: "phi_borrow_vivo".into(),
            name: "phi_borrow_vivo".into(),
            debug_info: None,
            params: vec![
                (ValueId(0), "objeto".into(), Type::Ref),
                (ValueId(1), "cond".into(), Type::I1),
            ],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::ArcCopy {
                            value: Operand::Val(ValueId(0)),
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(1)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![read(3)],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![read(4)],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(3),
                    instructions: vec![(
                        ValueId(5),
                        Instruction::Phi {
                            incoming: vec![
                                (BlockId(1), Operand::Val(ValueId(3))),
                                (BlockId(2), Operand::Val(ValueId(4))),
                            ],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(5)))),
                },
            ],
        });
        let mut p = DartFunctionPlan::default();
        p.tokens.ret = TokenReturn::Owned;
        let mut plans = HashMap::from([("phi_borrow_vivo".into(), p)]);
        assert_eq!(
            prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
            (1, 1)
        );
        assert_eq!(m.functions[0].blocks.len(), 4);
        let ops = &m.functions[0].blocks[3].instructions;
        assert!(matches!(
            ops[1].1,
            Instruction::ArcCopy {
                value: Operand::Val(ValueId(5))
            }
        ));
        assert!(matches!(
            ops[2].1,
            Instruction::ArcDrop {
                value: Operand::Val(ValueId(2))
            }
        ));
        assert!(matches!(
            plans["phi_borrow_vivo"].classes[&ValueId(5)],
            Ownership::Borrowed {
                owner: OwnerOrigin::Value(ValueId(2)),
                ..
            }
        ));
    }

    #[test]
    fn loop_allocation_releases_token_on_backedge_and_exit() {
        let mut m = Module::new();
        m.arc_memory = true;
        m.functions.push(Function {
            symbol: "laco_owned".into(),
            name: "laco_owned".into(),
            debug_info: None,
            params: vec![(ValueId(0), "repetir".into(), Type::I1)],
            return_ty: Type::Void,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::CallRuntime {
                            name: "dartforge_arc_box_int_owned_v1".into(),
                            args: vec![(Operand::Constant(Constant::Int(i64::MAX)), Type::I64)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(0)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                },
            ],
        });
        let mut plans = HashMap::from([("laco_owned".into(), DartFunctionPlan::default())]);
        assert_eq!(
            prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
            (0, 2)
        );
        for b in &m.functions[0].blocks[3..] {
            assert!(matches!(
                b.instructions[0].1,
                Instruction::ArcDrop {
                    value: Operand::Val(ValueId(1))
                }
            ));
        }
        let before = format!("{m:?}/{plans:?}");
        assert_eq!(
            prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{m:?}/{plans:?}"), before);
        crate::llvm::LlvmEmitter::new(&m).emit_all();
    }
}
