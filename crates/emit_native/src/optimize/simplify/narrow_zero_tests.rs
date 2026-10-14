use super::*;

fn function() -> Function {
    Function {
        symbol: "narrow_zero".into(),
        name: "narrow_zero".into(),
        params: vec![(ValueId(0), "object".into(), Type::Ref)],
        return_ty: Type::I8,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (ValueId(1), Instruction::Const(Constant::Int(0)), Type::I8),
                (
                    ValueId(2),
                    Instruction::ICmp(
                        ICmpOp::Ne,
                        Operand::Val(ValueId(1)),
                        Operand::Constant(Constant::Int(0)),
                    ),
                    Type::I1,
                ),
                (
                    ValueId(3),
                    Instruction::SetField {
                        object: Operand::Val(ValueId(0)),
                        index: 0,
                        value: Operand::Val(ValueId(1)),
                    },
                    Type::Void,
                ),
                (
                    ValueId(4),
                    Instruction::ZExt {
                        op: Operand::Val(ValueId(1)),
                        from: Type::I8,
                        to: Type::I64,
                    },
                    Type::I64,
                ),
                (
                    ValueId(5),
                    Instruction::CallRuntime {
                        name: "consume_narrow".into(),
                        args: vec![
                            (Operand::Val(ValueId(1)), Type::I8),
                            (Operand::Val(ValueId(2)), Type::I1),
                            (Operand::Val(ValueId(4)), Type::I64),
                        ],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
        }],
        debug_info: None,
    }
}

#[test]
fn narrow_zero_folds_comparison_without_widening_store_extension_call_or_return() {
    let mut f = function();
    assert!(fold_constants(&mut f));
    let instructions = &f.blocks[0].instructions;
    assert!(matches!(
        instructions[0],
        (ValueId(1), Instruction::Const(Constant::Int(0)), Type::I8)
    ));
    assert!(matches!(
        &instructions[1].1,
        Instruction::SetField {
            value: Operand::Val(ValueId(1)),
            ..
        }
    ));
    assert!(matches!(
        &instructions[2].1,
        Instruction::ZExt {
            op: Operand::Val(ValueId(1)),
            from: Type::I8,
            to: Type::I64
        }
    ));
    let Instruction::CallRuntime { args, .. } = &instructions[3].1 else {
        panic!("call remains")
    };
    assert_eq!(
        args,
        &vec![
            (Operand::Val(ValueId(1)), Type::I8),
            (Operand::Constant(Constant::Bool(false)), Type::I1),
            (Operand::Val(ValueId(4)), Type::I64)
        ]
    );
    assert!(matches!(
        f.blocks[0].terminator,
        Terminator::Return(Some(Operand::Val(ValueId(1))))
    ));
}

#[test]
fn nonzero_malformed_or_unavailable_narrow_definitions_do_not_fold_comparisons() {
    for case in 0..7 {
        let mut f = function();
        match case {
            0 => f.blocks[0].instructions[0].1 = Instruction::Const(Constant::Int(1)),
            1 => f.blocks[0].instructions[0].2 = Type::Ref,
            2 => f.blocks[0].instructions[0].1 = Instruction::Const(Constant::Bool(false)),
            3 => f.blocks[0].instructions[1].2 = Type::I8,
            4 => f.blocks[0].instructions.swap(0, 1),
            5 => f.params.push((ValueId(1), "duplicate".into(), Type::I8)),
            6 => {
                let definition = f.blocks[0].instructions.remove(0);
                f.blocks.push(BasicBlock {
                    id: BlockId(1),
                    instructions: vec![definition],
                    terminator: Terminator::Return(None),
                });
            }
            _ => unreachable!(),
        }
        fold_constants(&mut f);
        assert!(
            f.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .any(|(v, instruction, _)| *v == ValueId(2)
                    && matches!(instruction, Instruction::ICmp(..))),
            "case {case}"
        );
    }
}

#[test]
fn widened_false_and_folds_branch_without_erasing_eager_effects() {
    for case in 0..10 {
        let mut f = function();
        f.blocks[0].instructions = vec![
            (
                ValueId(1),
                Instruction::CallRuntime {
                    name: "effectful_predicate".into(),
                    args: vec![],
                    ret_ty: Type::I1,
                },
                Type::I1,
            ),
            (
                ValueId(2),
                Instruction::And(
                    Operand::Val(ValueId(1)),
                    Operand::Constant(Constant::Bool(false)),
                ),
                Type::I64,
            ),
            (
                ValueId(3),
                Instruction::ICmp(
                    ICmpOp::Ne,
                    Operand::Val(ValueId(2)),
                    Operand::Constant(Constant::Int(0)),
                ),
                Type::I1,
            ),
        ];
        f.blocks[0].terminator = Terminator::CondBranch {
            cond: Operand::Val(ValueId(3)),
            then_block: BlockId(1),
            else_block: BlockId(2),
        };
        for id in [1, 2] {
            f.blocks.push(BasicBlock {
                id: BlockId(id),
                instructions: vec![],
                terminator: Terminator::Return(None),
            });
        }
        match case {
            0 => {}
            1 => {
                if let Instruction::And(a, b) = &mut f.blocks[0].instructions[1].1 {
                    std::mem::swap(a, b);
                }
            }
            2 => f.blocks[0].instructions[0].2 = Type::I64,
            3 => f.blocks[0].instructions[1].2 = Type::I1,
            4 => {
                f.blocks[0].instructions[1].1 = Instruction::And(
                    Operand::Val(ValueId(1)),
                    Operand::Constant(Constant::Int(0)),
                )
            }
            5 => {
                f.blocks[0].instructions[1].1 = Instruction::Or(
                    Operand::Val(ValueId(1)),
                    Operand::Constant(Constant::Bool(false)),
                )
            }
            6 => f.blocks[0].instructions.swap(0, 1),
            7 => f.params.push((ValueId(1), "duplicate".into(), Type::I1)),
            8 => {
                if let Instruction::CallRuntime { ret_ty, .. } = &mut f.blocks[0].instructions[0].1
                {
                    *ret_ty = Type::I64;
                }
            }
            9 => {
                f.blocks[0].instructions[0].1 = Instruction::And(
                    Operand::Constant(Constant::Bool(true)),
                    Operand::Constant(Constant::Bool(true)),
                );
            }
            _ => unreachable!(),
        }
        let folded = fold_constants(&mut f);
        assert_eq!(folded, case < 2, "case {case}");
        remove_dead(&mut f);
        if case != 9 {
            assert!(f.blocks[0].instructions.iter().any(|(_, instruction, _)| matches!(instruction, Instruction::CallRuntime { name, .. } if name == "effectful_predicate")), "effect lost in case {case}");
        }
        if case < 2 {
            assert!(matches!(
                f.blocks[0].terminator,
                Terminator::Branch(BlockId(2))
            ));
        }
    }
}
