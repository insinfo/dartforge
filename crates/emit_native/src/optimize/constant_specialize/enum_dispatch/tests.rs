use super::*;

fn fixture() -> (Function, HashMap<ValueId, EnumToken>) {
    let token = EnumToken {
        library_uri: "file:enum.dart".into(),
        unit_uri: "file:enum.dart".into(),
        owner: "Mode".into(),
        declaration: 0,
        index: 1,
        getter: "mode1".into(),
    };
    (
        Function {
            symbol: "choose".into(),
            name: "choose".into(),
            params: vec![(ValueId(0), "mode".into(), Type::Ref)],
            return_ty: Type::I64,
            debug_info: None,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::GetField {
                            object: Operand::Val(ValueId(0)),
                            index: 0,
                        },
                        Type::I64,
                    )],
                    terminator: Terminator::Switch {
                        val: Operand::Val(ValueId(1)),
                        default: BlockId(1),
                        cases: vec![(1, BlockId(2))],
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Constant(Constant::Int(7)))),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Constant(Constant::Int(9)))),
                },
            ],
        },
        HashMap::from([(ValueId(0), token)]),
    )
}

#[test]
fn observed_index_is_kept_and_miss_forwards_real_abi_without_clearing_exceptions() {
    let (mut function, facts) = fixture();
    assert_eq!(guard_switches(&mut function, &facts, "original"), 1);
    assert!(matches!(
        function.blocks[0].instructions[0].1,
        Instruction::GetField { index: 0, .. }
    ));
    assert!(matches!(
        function.blocks[0].instructions[1].1,
        Instruction::ICmp(
            ICmpOp::Eq,
            Operand::Val(ValueId(1)),
            Operand::Constant(Constant::Int(1))
        )
    ));
    assert!(matches!(
        function.blocks[0].terminator,
        Terminator::CondBranch {
            then_block: BlockId(2),
            else_block: BlockId(3),
            ..
        }
    ));
    assert!(
        matches!(&function.blocks[3].instructions[0].1, Instruction::CallStatic { symbol, args, ret_ty: Type::I64 }
        if symbol == "original" && args == &[Operand::Val(ValueId(0))])
    );
    assert_eq!(function.blocks[3].instructions.len(), 1);
    assert!(matches!(
        function.blocks[3].terminator,
        Terminator::Return(Some(Operand::Val(_)))
    ));
}

#[test]
fn effects_allocations_unknown_receivers_joins_cycles_and_exhausted_ids_refuse_replay() {
    for variant in 0..8 {
        let (mut function, facts) = fixture();
        match variant {
            0 => function.blocks[0].instructions.insert(
                0,
                (
                    ValueId(10),
                    Instruction::CallRuntime {
                        name: "observable".into(),
                        args: vec![],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
            ),
            1 => function.blocks[0].instructions.insert(
                0,
                (
                    ValueId(10),
                    Instruction::Const(Constant::String("allocation".into())),
                    Type::Ref,
                ),
            ),
            2 => {
                if let Instruction::GetField { index, .. } =
                    &mut function.blocks[0].instructions[0].1
                {
                    *index = 1;
                }
            }
            3 => {
                function
                    .params
                    .push((ValueId(20), "unknown".into(), Type::Ref));
                if let Instruction::GetField { object, .. } =
                    &mut function.blocks[0].instructions[0].1
                {
                    *object = Operand::Val(ValueId(20));
                }
            }
            4 => function.blocks[2].terminator = Terminator::Branch(BlockId(0)),
            5 => {
                if let Terminator::Switch { cases, .. } = &mut function.blocks[0].terminator {
                    cases.push((1, BlockId(1)));
                }
            }
            6 => {
                function.blocks[2].id = BlockId(u32::MAX);
                if let Terminator::Switch { cases, .. } = &mut function.blocks[0].terminator {
                    cases[0].1 = BlockId(u32::MAX);
                }
            }
            _ => function.blocks[2].instructions.push((
                ValueId(u32::MAX),
                Instruction::Const(Constant::Int(1)),
                Type::I64,
            )),
        }
        let before = format!("{function:?}");
        assert_eq!(
            guard_switches(&mut function, &facts, "original"),
            0,
            "variant {variant}"
        );
        assert_eq!(format!("{function:?}"), before, "variant {variant}");
    }
}

#[test]
fn predecessor_effects_and_merges_are_barriers_but_later_effects_are_kept() {
    for variant in 0..3 {
        let (mut function, facts) = fixture();
        let mut dispatch = function.blocks.remove(0);
        dispatch.id = BlockId(3);
        function.blocks.insert(
            0,
            BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(3)),
            },
        );
        function.blocks.push(dispatch);
        match variant {
            0 => function.blocks[0].instructions.push((
                ValueId(10),
                Instruction::CallRuntime {
                    name: "effect".into(),
                    args: vec![],
                    ret_ty: Type::Void,
                },
                Type::Void,
            )),
            1 => {
                function
                    .params
                    .push((ValueId(11), "condition".into(), Type::I1));
                function.blocks[0].terminator = Terminator::CondBranch {
                    cond: Operand::Val(ValueId(11)),
                    then_block: BlockId(3),
                    else_block: BlockId(4),
                };
                function.blocks.push(BasicBlock {
                    id: BlockId(4),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(3)),
                });
            }
            _ => function.blocks[2].instructions.push((
                ValueId(12),
                Instruction::CallRuntime {
                    name: "later_effect".into(),
                    args: vec![],
                    ret_ty: Type::Void,
                },
                Type::Void,
            )),
        }
        assert_eq!(
            guard_switches(&mut function, &facts, "original"),
            usize::from(variant == 2)
        );
        if variant == 2 {
            assert!(
                matches!(&function.blocks[2].instructions[0].1, Instruction::CallRuntime { name, .. } if name == "later_effect")
            );
        }
    }
}

#[test]
fn native_guard_hit_miss_and_pending_exception_match_original_execution() {
    let (mut original, facts) = fixture();
    original.symbol = "original".into();
    original.blocks[1].instructions.push((
        ValueId(10),
        Instruction::CallRuntime {
            name: "dartforge_exception_throw".into(),
            args: vec![
                (Operand::Constant(Constant::Int(77)), Type::I64),
                (Operand::Constant(Constant::Int(1)), Type::I8),
            ],
            ret_ty: Type::Void,
        },
        Type::Void,
    ));
    original.return_ty = Type::Ref;
    for position in [1, 2] {
        let Terminator::Return(Some(value)) = original.blocks[position].terminator.clone() else {
            panic!("return expected");
        };
        let boxed = ValueId(20 + position as u32);
        original.blocks[position].instructions.insert(
            0,
            (
                boxed,
                Instruction::Box {
                    op: value,
                    from: Type::I64,
                },
                Type::Ref,
            ),
        );
        original.blocks[position].terminator = Terminator::Return(Some(Operand::Val(boxed)));
    }
    let mut clone = original.clone();
    assert_eq!(guard_switches(&mut clone, &facts, "original"), 1);
    clone.symbol = "candidate".into();
    let mut module = Module::new();
    module.entry_symbol = Some("dart_main".into());
    module.classes.push(ClassDef {
        id: 300,
        name: "Mode".into(),
        field_count: 2,
        vtable: vec![],
        to_string_symbol: None,
    });
    let mut instructions = vec![(
        ValueId(0),
        Instruction::AllocObject {
            class_id: 300,
            fields: vec![
                Operand::Constant(Constant::Int(1)),
                Operand::Constant(Constant::Int(0)),
            ],
        },
        Type::Ref,
    )];
    let mut next = 1;
    for observed in [1, 0] {
        let mut push = |instruction, ty| {
            let value = ValueId(next);
            next += 1;
            instructions.push((value, instruction, ty));
            value
        };
        push(
            Instruction::SetField {
                object: Operand::Val(ValueId(0)),
                index: 0,
                value: Operand::Constant(Constant::Int(observed)),
            },
            Type::Void,
        );
        let result = push(
            Instruction::CallStatic {
                symbol: "candidate".into(),
                args: vec![Operand::Val(ValueId(0))],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        let result = push(
            Instruction::Unbox {
                op: Operand::Val(result),
                to: Type::I64,
            },
            Type::I64,
        );
        push(
            Instruction::CallRuntime {
                name: "dartforge_print_i64".into(),
                args: vec![(Operand::Val(result), Type::I64)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        let pending = push(
            Instruction::CallRuntime {
                name: "dartforge_exception_pending".into(),
                args: vec![],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        let pending = push(
            Instruction::ZExt {
                op: Operand::Val(pending),
                from: Type::I8,
                to: Type::I64,
            },
            Type::I64,
        );
        push(
            Instruction::CallRuntime {
                name: "dartforge_print_i64".into(),
                args: vec![(Operand::Val(pending), Type::I64)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        push(
            Instruction::CallRuntime {
                name: "dartforge_exception_clear".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
    }
    module.functions = vec![
        original,
        clone,
        Function {
            symbol: "dart_main".into(),
            name: "main".into(),
            params: vec![],
            return_ty: Type::Void,
            debug_info: None,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions,
                terminator: Terminator::Return(None),
            }],
        },
    ];
    let mut ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
    ir.push_str("\ndeclare void @dartforge_print_i64(i64)\n");
    let directory = tempfile::tempdir().unwrap();
    let binary = directory
        .path()
        .join(if cfg!(windows) { "guard.exe" } else { "guard" });
    crate::driver::compile_and_link(&ir, &binary, &crate::driver::NativeDriverOptions::default())
        .unwrap();
    let output = std::process::Command::new(binary).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .replace("\r\n", "\n"),
        "9\n0\n7\n1\n"
    );
}

#[test]
fn removed_switch_edges_are_removed_from_live_successor_phis() {
    let (mut function, facts) = fixture();
    function.blocks[1].instructions.push((
        ValueId(10),
        Instruction::Phi {
            incoming: vec![
                (BlockId(0), Operand::Constant(Constant::Int(3))),
                (BlockId(2), Operand::Constant(Constant::Int(4))),
            ],
            ty: Type::I64,
        },
        Type::I64,
    ));
    function.blocks[2].terminator = Terminator::Branch(BlockId(1));
    assert_eq!(guard_switches(&mut function, &facts, "original"), 1);
    let Instruction::Phi { incoming, .. } = &function.blocks[1].instructions[0].1 else {
        panic!("phi expected");
    };
    assert_eq!(
        incoming,
        &vec![(BlockId(2), Operand::Constant(Constant::Int(4)))]
    );
}

#[test]
fn analysis_budgets_refuse_large_prefixes_and_guard_sets_without_mutation() {
    for variant in 0..3 {
        let (mut function, facts) = fixture();
        match variant {
            0 => {
                for n in 0..MAX_PREFIX_INSTRUCTIONS {
                    function.blocks[0].instructions.insert(
                        0,
                        (
                            ValueId(1000 + n as u32),
                            Instruction::Const(Constant::Int(0)),
                            Type::I64,
                        ),
                    );
                }
            }
            1 => {
                let mut dispatch = function.blocks.remove(0);
                dispatch.id = BlockId(100);
                for n in 0..MAX_PREFIX_BLOCKS {
                    function.blocks.insert(
                        n,
                        BasicBlock {
                            id: BlockId(if n == 0 { 0 } else { 3 + n as u32 }),
                            instructions: vec![],
                            terminator: Terminator::Branch(BlockId(
                                if n + 1 == MAX_PREFIX_BLOCKS {
                                    100
                                } else {
                                    4 + n as u32
                                },
                            )),
                        },
                    );
                }
                function.blocks.push(dispatch);
            }
            _ => {
                let mut cases = Vec::new();
                for n in 0..MAX_GUARDS {
                    let id = BlockId(4 + n as u32);
                    cases.push((n as i64 + 1, id));
                    let value = ValueId(100 + n as u32);
                    function.blocks.push(BasicBlock {
                        id,
                        instructions: vec![(
                            value,
                            Instruction::GetField {
                                object: Operand::Val(ValueId(0)),
                                index: 0,
                            },
                            Type::I64,
                        )],
                        terminator: Terminator::Switch {
                            val: Operand::Val(value),
                            default: BlockId(1),
                            cases: vec![(1, BlockId(2))],
                        },
                    });
                }
                if let Terminator::Switch { cases: targets, .. } =
                    &mut function.blocks[0].terminator
                {
                    *targets = cases;
                }
            }
        }
        let before = format!("{function:?}");
        assert_eq!(
            guard_switches(&mut function, &facts, "original"),
            0,
            "variant {variant}"
        );
        assert_eq!(format!("{function:?}"), before);
    }
}
