use super::*;

fn fixture() -> Function {
    Function {
        symbol: "checked".into(),
        name: "checked".into(),
        params: vec![(ValueId(0), "value".into(), Type::Ref)],
        return_ty: Type::Ref,
        debug_info: None,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::Unbox {
                            op: Operand::Val(ValueId(0)),
                            to: Type::F64,
                        },
                        Type::F64,
                    ),
                    (
                        ValueId(2),
                        Instruction::CallRuntime {
                            name: "dartforge_exception_pending".into(),
                            args: vec![],
                            ret_ty: Type::I8,
                        },
                        Type::I8,
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
                ],
                terminator: Terminator::CondBranch {
                    cond: Operand::Val(ValueId(3)),
                    then_block: BlockId(2),
                    else_block: BlockId(1),
                },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![(
                    ValueId(4),
                    Instruction::Box {
                        op: Operand::Val(ValueId(1)),
                        from: Type::F64,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(4)))),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
            },
        ],
    }
}

#[test]
fn keeps_validation_error_edge_and_returns_original_on_success() {
    let mut f = fixture();
    assert_eq!(rewrite_function(&mut f), 1);
    assert!(matches!(
        f.blocks[0].instructions[0].1,
        Instruction::Unbox { to: Type::F64, .. }
    ));
    assert_eq!(f.blocks[1].instructions.len(), 0);
    assert!(matches!(
        f.blocks[1].terminator,
        Terminator::Return(Some(Operand::Val(ValueId(0))))
    ));
    assert!(matches!(
        f.blocks[2].terminator,
        Terminator::Return(Some(Operand::Constant(Constant::Null)))
    ));
}

#[test]
fn refuses_mutation_calls_ambiguous_entry_wrong_scalar_and_invalid_types() {
    for variant in 0..5 {
        let mut f = fixture();
        match variant {
            0 => f.blocks[1].instructions.insert(
                0,
                (
                    ValueId(5),
                    Instruction::CallRuntime {
                        name: "mutate".into(),
                        args: vec![],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                ),
            ),
            1 => f.blocks[2].terminator = Terminator::Branch(BlockId(1)),
            2 => f.blocks[0].instructions.insert(
                1,
                (
                    ValueId(5),
                    Instruction::CallRuntime {
                        name: "mutate".into(),
                        args: vec![],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                ),
            ),
            3 => {
                f.blocks[1].instructions[0].1 = Instruction::Box {
                    op: Operand::Val(ValueId(2)),
                    from: Type::F64,
                }
            }
            _ => f.params[0].2 = Type::I64,
        }
        assert_eq!(rewrite_function(&mut f), 0, "variant {variant}");
    }
}

#[test]
fn rejects_missing_targets_duplicate_blocks_and_non_dominating_refs() {
    for variant in 0..3 {
        let mut f = fixture();
        match variant {
            0 => f.blocks[2].terminator = Terminator::Branch(BlockId(99)),
            1 => f.blocks[2].id = BlockId(1),
            _ => {
                f.blocks[2].instructions.push((
                    ValueId(5),
                    Instruction::Const(Constant::Null),
                    Type::Ref,
                ));
                f.blocks[0].instructions[0].1 = Instruction::Unbox {
                    op: Operand::Val(ValueId(5)),
                    to: Type::F64,
                };
            }
        }
        assert_eq!(rewrite_function(&mut f), 0, "variant {variant}");
    }
}

#[test]
fn refuses_unknown_sdk_and_arc_or_reloadable_images() {
    for variant in 0..5 {
        let mut module = Module::new();
        module.checked_double_identity = true;
        module.arc_fixed_ancestry = true;
        module.functions.push(fixture());
        match variant {
            0 => module.checked_double_identity = false,
            1 => module.arc_memory = true,
            2 => module.arc_prepared = true,
            3 => module.arc_fixed_ancestry = false,
            _ => module.functions[0].symbol = "dartforge_unbox_double".into(),
        }
        assert_eq!(rewrite(&mut module), 0, "variant {variant}");
    }
}

#[test]
fn unrelated_widened_boolean_or_is_not_a_ref_representation_proof() {
    let mut f = fixture();
    f.blocks[0].instructions.insert(
        0,
        (
            ValueId(5),
            Instruction::Or(
                Operand::Constant(Constant::Bool(false)),
                Operand::Constant(Constant::Bool(true)),
            ),
            Type::I1,
        ),
    );
    assert_eq!(rewrite_function(&mut f), 1);
    assert!(matches!(f.blocks[0].instructions[0].1, Instruction::Or(..)));
    let mut malformed = fixture();
    malformed.blocks[0].instructions.insert(
        0,
        (
            ValueId(5),
            Instruction::Const(Constant::Double(1.0)),
            Type::Ref,
        ),
    );
    malformed.blocks[0].instructions[1].1 = Instruction::Unbox {
        op: Operand::Val(ValueId(5)),
        to: Type::F64,
    };
    assert_eq!(
        rewrite_function(&mut malformed),
        0,
        "the original Ref must have a physically compatible definition"
    );
}
