use super::*;

fn pending_join() -> (Function, Function, DartFunctionPlan) {
    let val = |n| Operand::Val(ValueId(n));
    let f = Function {
        symbol: "pending_join".into(),
        name: "pending_join".into(),
        debug_info: None,
        params: vec![(ValueId(0), "other".into(), Type::Ref)],
        return_ty: Type::Ref,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::CallStatic {
                            symbol: "source".into(),
                            args: vec![],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
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
                        Instruction::ICmp(ICmpOp::Ne, val(2), Operand::Constant(Constant::Int(0))),
                        Type::I1,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: val(3),
                    then_block: BlockId(2),
                    else_block: BlockId(1),
                },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![(
                    ValueId(4),
                    Instruction::GetField {
                        object: val(1),
                        index: 0,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![(
                    ValueId(5),
                    Instruction::Phi {
                        incoming: vec![
                            (BlockId(1), val(4)),
                            (BlockId(2), Operand::Constant(Constant::Null)),
                        ],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(val(5))),
            },
        ],
    };
    let source = Function {
        symbol: "source".into(),
        name: "source".into(),
        debug_info: None,
        params: vec![],
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![],
            terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
        }],
    };
    let mut p = DartFunctionPlan::default();
    p.tokens.pendings.insert(ValueId(1), BlockId(2));
    p.classes.insert(
        ValueId(0),
        Ownership::Borrowed {
            owner: OwnerOrigin::Caller,
            scope: 0,
        },
    );
    p.classes.insert(
        ValueId(4),
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(1)),
            scope: 0,
        },
    );
    (f, source, p)
}

#[test]
fn pending_field_null_phi_copies_only_success_input_and_keeps_result_guard() {
    let (mut f, source, mut p) = pending_join();
    let summaries = vec![
        super::super::calls::provisional_summary(&f, true),
        super::super::calls::provisional_summary(&source, true),
    ];
    let index = super::super::calls::DartFunctionIndex::new(&summaries).unwrap();
    let mut before = p.clone();
    super::super::calls::produce_dart_calls_and_instructions(
        &f,
        &index,
        &mut before.classes,
        &mut before.tokens,
    )
    .unwrap();
    assert!(
        super::super::classification::liveness_with_exits(
            &f,
            &before.classes,
            &before.tables,
            &before.tokens.pendings
        )
        .unwrap_err()
        .contains("unavailable on the exceptional path")
    );
    assert_eq!(copy_borrows(&mut f, &index, &p).unwrap(), 1);
    assert!(f.blocks[2].instructions.is_empty());
    let edge = f.blocks.last().unwrap();
    assert!(matches!(
        edge.instructions.as_slice(),
        [(
            _,
            Instruction::ArcCopy {
                value: Operand::Val(ValueId(4))
            },
            Type::Ref
        )]
    ));
    assert!(matches!(f.blocks[1].terminator, Terminator::Branch(b) if b == edge.id));
    assert!(matches!(
        f.blocks[0].terminator,
        Terminator::CondBranch {
            then_block: BlockId(2),
            else_block: BlockId(1),
            ..
        }
    ));
    let Instruction::Phi { incoming, .. } = &f.blocks[3].instructions[0].1 else {
        unreachable!()
    };
    assert_eq!(incoming[1], (BlockId(2), Operand::Constant(Constant::Null)));
    super::super::calls::produce_dart_calls_and_instructions(
        &f,
        &index,
        &mut p.classes,
        &mut p.tokens,
    )
    .unwrap();
    assert_eq!(p.classes[&ValueId(5)], Ownership::Owned);
    super::super::classification::liveness_with_exits(
        &f,
        &p.classes,
        &p.tables,
        &p.tokens.pendings,
    )
    .unwrap();
    assert_eq!(copy_borrows(&mut f, &index, &p).unwrap(), 0);
}

#[test]
fn pending_field_other_borrow_phi_copies_both_valid_inputs() {
    let (mut f, source, mut p) = pending_join();
    let Instruction::Phi { incoming, .. } = &mut f.blocks[3].instructions[0].1 else {
        unreachable!()
    };
    incoming[1].1 = Operand::Val(ValueId(0));
    let summaries = vec![
        super::super::calls::provisional_summary(&f, true),
        super::super::calls::provisional_summary(&source, true),
    ];
    let index = super::super::calls::DartFunctionIndex::new(&summaries).unwrap();
    assert_eq!(copy_borrows(&mut f, &index, &p).unwrap(), 2);
    assert!(f.blocks[2].instructions.is_empty());
    assert!(
        f.blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .any(|(_, instruction, _)| matches!(
                instruction,
                Instruction::ArcCopy {
                    value: Operand::Val(ValueId(0))
                }
            ))
    );
    super::super::calls::produce_dart_calls_and_instructions(
        &f,
        &index,
        &mut p.classes,
        &mut p.tokens,
    )
    .unwrap();
    assert_eq!(p.classes[&ValueId(5)], Ownership::Owned);
    super::super::classification::liveness_with_exits(
        &f,
        &p.classes,
        &p.tables,
        &p.tokens.pendings,
    )
    .unwrap();
}

#[test]
fn pending_result_on_exceptional_input_or_direct_use_remains_rejected() {
    for direct in [false, true] {
        let (mut f, source, mut p) = pending_join();
        if direct {
            f.blocks[2].instructions.push((
                ValueId(6),
                Instruction::GetField {
                    object: Operand::Val(ValueId(1)),
                    index: 0,
                },
                Type::Ref,
            ));
            p.classes.insert(
                ValueId(6),
                Ownership::Borrowed {
                    owner: OwnerOrigin::Value(ValueId(1)),
                    scope: 0,
                },
            );
        } else {
            let Instruction::Phi { incoming, .. } = &mut f.blocks[3].instructions[0].1 else {
                unreachable!()
            };
            incoming[1].1 = Operand::Val(ValueId(4));
        }
        let summaries = vec![
            super::super::calls::provisional_summary(&f, true),
            super::super::calls::provisional_summary(&source, true),
        ];
        let index = super::super::calls::DartFunctionIndex::new(&summaries).unwrap();
        copy_borrows(&mut f, &index, &p).unwrap();
        assert!(
            f.blocks[2]
                .instructions
                .iter()
                .all(|(_, instruction, _)| !matches!(instruction, Instruction::ArcCopy { .. }))
        );
        let produced = super::super::calls::produce_dart_calls_and_instructions(
            &f,
            &index,
            &mut p.classes,
            &mut p.tokens,
        );
        if direct {
            produced.unwrap();
        } else if produced.is_err() {
            continue;
        }
        let error = super::super::classification::liveness_with_exits(
            &f,
            &p.classes,
            &p.tables,
            &p.tokens.pendings,
        )
        .unwrap_err();
        assert!(error.contains("ARC003"), "{error}");
    }
}

#[test]
fn ordinary_inlined_field_return_and_catch_prepare_with_conditional_owned_join() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let entry = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/arc_ports/pending_field_phi.dart");
        let sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
        let mut interner = dartforge_intern::Interner::new();
        let program = dartforge_elements::load::load(&entry, &sdk, None, &mut interner).unwrap();
        let mut table = dartforge_types::TypeTable::new();
        let core = dartforge_types::CoreTypes::init(&mut table, &program, &interner);
        let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
        let libraries: Vec<_> = program.libraries.iter().enumerate().filter(|(_, library)| !library.is_sdk).map(|(i, _)| dartforge_elements::model::LibraryId(i as u32)).collect();
        let (bodies, _) = dartforge_types::infer_library_bodies_with_locals(&program, &interner, &mut table, &core, &mut outline, &libraries);
        let mut context = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies).with_source_sdk();
        context.arc_memory = true;
        let mut m = crate::lower::lower_program(&context);
        assert!(m.errors.is_empty(), "{:?}", m.errors);
        m.arc_memory = true;
        m.arc_external_callees = true;
        m.arc_external_layouts = crate::lower::sdk_arc_layouts(&context);
        crate::optimize::optimize(&mut m);
        let mut plans = m.functions.iter().map(|f| (f.symbol.clone(), DartFunctionPlan::default())).collect();
        crate::optimize::arc::prepare_arc_dart_module(&mut m, &mut plans).unwrap();
        let f = m.functions.iter().find(|f| f.symbol.ends_with("..attempt")).unwrap();
        assert!(f.blocks.iter().flat_map(|b| &b.instructions).any(|(_, instruction, _)| matches!(instruction, Instruction::ArcCopy { .. })));
        assert!(f.blocks.iter().flat_map(|b| &b.instructions).any(|(_, instruction, _)| matches!(instruction, Instruction::ArcDrop { .. })));
        assert!(f.blocks.iter().flat_map(|b| &b.instructions).any(|(_, instruction, _)| matches!(instruction, Instruction::CallRuntime { name, .. } if name == "dartforge_exception_pending")));
        crate::optimize::arc::certify_arc_aot_ports(&mut m, &plans).unwrap();
    }).unwrap().join().unwrap();
}

#[test]
fn successful_only_join_stays_borrowed_and_malformed_pending_metadata_never_promotes() {
    for case in 0..4 {
        let (mut f, source, mut p) = pending_join();
        if case == 0 {
            f.blocks[2].terminator = Terminator::Return(Some(Operand::Constant(Constant::Null)));
            let Instruction::Phi { incoming, .. } = &mut f.blocks[3].instructions[0].1 else {
                unreachable!()
            };
            incoming.retain(|(pred, _)| *pred == BlockId(1));
        } else if case == 1 {
            if let Instruction::CallRuntime { ret_ty, .. } = &mut f.blocks[0].instructions[1].1 {
                *ret_ty = Type::I64;
            }
        } else if case == 2 {
            f.blocks[0].instructions[1].2 = Type::I64;
        } else {
            p.tokens.pendings.insert(ValueId(1), BlockId(99));
        }
        let before = format!("{f:?}");
        let summaries = vec![
            super::super::calls::provisional_summary(&f, true),
            super::super::calls::provisional_summary(&source, true),
        ];
        let index = super::super::calls::DartFunctionIndex::new(&summaries).unwrap();
        assert_eq!(copy_borrows(&mut f, &index, &p).unwrap(), 0, "case {case}");
        assert_eq!(format!("{f:?}"), before, "case {case}");
    }
}
