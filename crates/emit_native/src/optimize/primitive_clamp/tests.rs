//! Structural regressions complement the ordinary Dart differential fixture.
use super::*;

fn fixture() -> Module {
    let mut module = Module::new();
    module.primitive_clamp_certificate = true;
    module.arc_fixed_ancestry = true;
    module.functions.push(Function {
        symbol: "unit".into(),
        name: "unit".into(),
        params: vec![(ValueId(0), "receiver".into(), Type::Ref)],
        return_ty: Type::Ref,
        debug_info: None,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::Box {
                            op: int(0),
                            from: Type::I64,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(2),
                        Instruction::Box {
                            op: int(1),
                            from: Type::I64,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(3),
                        Instruction::CallSelector {
                            selector: "c:clamp".into(),
                            recv: Operand::Val(ValueId(0)),
                            args: vec![Operand::Val(ValueId(1)), Operand::Val(ValueId(2))],
                            names: Vec::new(),
                            tuple_types: int(0),
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(4),
                        Instruction::CallRuntime {
                            name: "dartforge_exception_pending".into(),
                            args: Vec::new(),
                            ret_ty: Type::I8,
                        },
                        Type::I8,
                    ),
                    (
                        ValueId(5),
                        Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(4)), int(0)),
                        Type::I1,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: Operand::Val(ValueId(5)),
                    then_block: BlockId(2),
                    else_block: BlockId(1),
                },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![(
                    ValueId(6),
                    Instruction::Phi {
                        incoming: vec![(BlockId(0), Operand::Val(ValueId(3)))],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(6)))),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: Vec::new(),
                terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
            },
        ],
    });
    module
}
fn extra(function: &mut Function, instruction: Instruction) {
    function.blocks[0]
        .instructions
        .insert(0, (ValueId(20), instruction, Type::Void));
}

#[test]
fn guards_preserve_original_bound_refs_fallback_pending_checks_and_successor_phis() {
    let mut module = fixture();
    let suffix = module.functions[0].blocks[0].instructions[3..].to_vec();
    assert_eq!(rewrite(&mut module), 1);
    assert_eq!(
        rewrite(&mut module),
        0,
        "fallback does not recursively expand"
    );
    let function = &module.functions[0];
    let definitions = Definitions::new(function).unwrap();
    let join = function
        .blocks
        .iter()
        .find(|b| {
            b.instructions
                .iter()
                .any(|(v, i, _)| *v == ValueId(3) && matches!(i, Instruction::Phi { .. }))
        })
        .unwrap();
    assert_eq!(
        format!("{:?}", &join.instructions[1..]),
        format!("{suffix:?}")
    );
    let (_, Instruction::Phi { incoming, .. }, _) = &join.instructions[0] else {
        panic!("original result must be a Ref join");
    };
    for value in [0, 1, 2] {
        assert!(
            incoming
                .iter()
                .any(|(_, operand)| *operand == Operand::Val(ValueId(value)))
        );
    }
    let successor = &function.blocks[1];
    let Instruction::Phi { incoming, .. } = &successor.instructions[0].1 else {
        panic!();
    };
    assert_eq!(incoming[0].0, join.id);
    for (block, b) in function.blocks.iter().enumerate() {
        for (_, instruction, _) in &b.instructions {
            if let Instruction::Phi { incoming, .. } = instruction {
                let actual: HashSet<_> = definitions.cfg.predecessors[block]
                    .iter()
                    .map(|position| function.blocks[*position].id)
                    .collect();
                assert_eq!(
                    incoming
                        .iter()
                        .map(|(predecessor, _)| *predecessor)
                        .collect::<HashSet<_>>(),
                    actual
                );
            }
        }
    }
    assert_eq!(
        function
            .blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .filter(|(_, i, _)| matches!(i, Instruction::CallSelector { .. }))
            .count(),
        1
    );
}

#[test]
fn unsupported_images_exposure_collisions_types_and_invalid_ssa_are_unchanged() {
    for variant in 0..19 {
        let mut module = fixture();
        match variant {
            0 => module.primitive_clamp_certificate = false,
            1 => module.arc_fixed_ancestry = false,
            2 => module.arc_memory = true,
            3 => module.arc_prepared = true,
            4 => module.table_exceptions = true,
            5 => module.sdk_library = true,
            6 => {
                module
                    .pruning_ambiguous_class_ids
                    .insert(dartforge_runtime::layout::cid::DOUBLE as u32);
            }
            7 => module.classes.push(ClassDef {
                id: dartforge_runtime::layout::cid::DOUBLE as u32,
                name: "Spoof".into(),
                field_count: 1,
                vtable: Vec::new(),
                to_string_symbol: None,
            }),
            8 => extra(
                &mut module.functions[0],
                Instruction::NativeCall {
                    target: int(1),
                    args: Vec::new(),
                    ret: CType::Void,
                },
            ),
            9 => extra(
                &mut module.functions[0],
                Instruction::CompositeNativeCall {
                    target: int(1),
                    args: Vec::new(),
                    ret: NativeType::Prim(CType::Void),
                    dest: None,
                    variadic_flag: None,
                },
            ),
            10 => extra(
                &mut module.functions[0],
                Instruction::CallRuntime {
                    name: "dartforge_globals_area".into(),
                    args: Vec::new(),
                    ret_ty: Type::Ptr,
                },
            ),
            11 => extra(
                &mut module.functions[0],
                Instruction::CallStatic {
                    symbol: "dartforge_register_table".into(),
                    args: Vec::new(),
                    ret_ty: Type::Void,
                },
            ),
            12 => module.functions.push(module.functions[0].clone()),
            13 => module.functions[0].blocks[0].instructions[0].0 = ValueId(0),
            14 => module.functions[0].blocks[0].instructions[0].2 = Type::I64,
            15 => {
                module.functions[0].blocks[0].instructions[0].1 = Instruction::Box {
                    op: Operand::Constant(Constant::Double(0.0)),
                    from: Type::F64,
                }
            }
            16 => module.functions[0].blocks[1].id = BlockId(0),
            17 => {
                let bound = module.functions[0].blocks[0].instructions.remove(0);
                module.functions[0].blocks[1].instructions.push(bound);
            }
            _ => extra(
                &mut module.functions[0],
                Instruction::CallRuntime {
                    name: "dartforge_native_Ffi_dl_lookup".into(),
                    args: Vec::new(),
                    ret_ty: Type::Ref,
                },
            ),
        }
        let before = format!("{:?}", module.functions);
        assert_eq!(rewrite(&mut module), 0, "variant {variant}");
        assert_eq!(
            format!("{:?}", module.functions),
            before,
            "rejected variant {variant} must stay unchanged"
        );
    }
}

#[test]
fn only_resolved_static_targets_and_exact_non_generic_typed_selectors_are_candidates() {
    for selector in ["c:clamp", "tc:clamp"] {
        let mut module = fixture();
        if let Instruction::CallSelector {
            selector: actual, ..
        } = &mut module.functions[0].blocks[0].instructions[2].1
        {
            *actual = selector.into();
        }
        assert_eq!(rewrite(&mut module), 1);
    }
    for valid in [false, true] {
        let mut module = fixture();
        module
            .primitive_clamp_targets
            .insert("actual.selected.body".into());
        module.functions[0].blocks[0].instructions[2].1 = Instruction::CallStatic {
            symbol: if valid {
                "actual.selected.body"
            } else {
                "unrelated.clamp"
            }
            .into(),
            args: vec![
                Operand::Val(ValueId(0)),
                Operand::Val(ValueId(1)),
                Operand::Val(ValueId(2)),
            ],
            ret_ty: Type::Ref,
        };
        assert_eq!(rewrite(&mut module), usize::from(valid));
    }
    let mut module = fixture();
    if let Instruction::CallSelector { tuple_types, .. } =
        &mut module.functions[0].blocks[0].instructions[2].1
    {
        *tuple_types = int(9);
    }
    assert_eq!(
        rewrite(&mut module),
        0,
        "a generic call cannot inherit the nongeneric SDK ABI premise"
    );
}

#[test]
fn joins_never_create_or_revive_a_source_certificate() {
    let mut dest = fixture();
    let mut origin = fixture();
    dest.primitive_clamp_targets.insert("a".into());
    origin.primitive_clamp_targets.insert("b".into());
    crate::lower::join_arc_facts(&mut dest, &mut origin);
    assert!(!dest.primitive_clamp_certificate);
    let mut matching = fixture();
    crate::lower::join_arc_facts(&mut dest, &mut matching);
    assert!(!dest.primitive_clamp_certificate);
    let mut empty = Module::new();
    crate::lower::join_arc_facts(&mut empty, &mut matching);
    assert!(!empty.primitive_clamp_certificate);
}
