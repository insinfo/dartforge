use super::*;

fn graph() -> Function {
    Function {
        symbol: "trivial_graph".into(),
        name: "trivial_graph".into(),
        debug_info: None,
        params: vec![
            (ValueId(0), "source".into(), Type::Ref),
            (ValueId(9), "scalar".into(), Type::I64),
        ],
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(1),
                    Instruction::Phi {
                        incoming: vec![
                            (BlockId(0), Operand::Constant(Constant::Null)),
                            (BlockId(0), Operand::Val(ValueId(2))),
                        ],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(2),
                    Instruction::Phi {
                        incoming: vec![(BlockId(0), Operand::Val(ValueId(1)))],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
            ],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
        }],
    }
}

#[test]
fn anchored_graph_requires_every_dependency_to_be_trivial_and_founded() {
    for case in 0..12 {
        let mut f = graph();
        let mut classes = HashMap::new();
        let mut demanded = HashSet::new();
        match case {
            0 => {}
            1 => {
                classes.insert(ValueId(0), Ownership::Trivial);
                let Instruction::Phi { incoming, .. } = &mut f.blocks[0].instructions[0].1 else {
                    unreachable!()
                };
                incoming[0].1 = Operand::Val(ValueId(0));
            }
            2 => {
                let Instruction::Phi { incoming, .. } = &mut f.blocks[0].instructions[0].1 else {
                    unreachable!()
                };
                incoming[0].1 = Operand::Val(ValueId(2));
            }
            3 => {
                let Instruction::Phi { incoming, .. } = &mut f.blocks[0].instructions[1].1 else {
                    unreachable!()
                };
                incoming[0].1 = Operand::Val(ValueId(2));
            }
            4 => {
                let Instruction::Phi { incoming, .. } = &mut f.blocks[0].instructions[1].1 else {
                    unreachable!()
                };
                incoming[0].1 = Operand::Val(ValueId(0));
            }
            5 => {
                classes.insert(ValueId(0), Ownership::Owned);
                let Instruction::Phi { incoming, .. } = &mut f.blocks[0].instructions[1].1 else {
                    unreachable!()
                };
                incoming[0].1 = Operand::Val(ValueId(0));
            }
            6 => {
                classes.insert(
                    ValueId(0),
                    Ownership::Borrowed {
                        owner: super::super::super::OwnerOrigin::Caller,
                        scope: 0,
                    },
                );
                let Instruction::Phi { incoming, .. } = &mut f.blocks[0].instructions[1].1 else {
                    unreachable!()
                };
                incoming[0].1 = Operand::Val(ValueId(0));
            }
            7 => {
                demanded.insert(ValueId(2));
            }
            8 => {
                classes.insert(ValueId(0), Ownership::Trivial);
                demanded.insert(ValueId(0));
                let Instruction::Phi { incoming, .. } = &mut f.blocks[0].instructions[1].1 else {
                    unreachable!()
                };
                incoming[0].1 = Operand::Val(ValueId(0));
            }
            9 => {
                let Instruction::Phi { incoming, .. } = &mut f.blocks[0].instructions[1].1 else {
                    unreachable!()
                };
                incoming[0].1 = Operand::Val(ValueId(9));
            }
            10 => {
                let Instruction::Phi { ty, .. } = &mut f.blocks[0].instructions[1].1 else {
                    unreachable!()
                };
                *ty = Type::I64;
            }
            11 => f.blocks[0].instructions.push((
                ValueId(1),
                Instruction::Const(Constant::Null),
                Type::Ref,
            )),
            _ => unreachable!(),
        }
        let facts = prove(&f, &classes, &demanded);
        assert_eq!(facts.contains(&ValueId(1)), case < 2, "case {case}");
        assert_eq!(facts.contains(&ValueId(2)), case < 2, "case {case}");
    }
}

#[test]
fn explicitly_dropped_null_phi_keeps_an_owned_logical_token() {
    let mut f = Function {
        symbol: "owned_null".into(),
        name: "owned_null".into(),
        debug_info: None,
        params: vec![],
        return_ty: Type::Void,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(1)),
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::Phi {
                            incoming: vec![(BlockId(0), Operand::Constant(Constant::Null))],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(2),
                        Instruction::ArcDrop {
                            value: Operand::Val(ValueId(1)),
                        },
                        Type::Void,
                    ),
                ],
                terminator: Terminator::Return(None),
            },
        ],
    };
    let mut classes = HashMap::new();
    let mut plan = TokenPlan::default();
    super::super::produce_and_verify_tokens(
        &f,
        &mut classes,
        &mut plan,
        &super::super::super::FunctionTables::default(),
        &ScopePlan::default(),
    )
    .unwrap();
    assert_eq!(classes[&ValueId(1)], Ownership::Owned);
    assert!(matches!(
        f.blocks[1].instructions[1].1,
        Instruction::ArcDrop {
            value: Operand::Val(ValueId(1))
        }
    ));
    // A directly returned Phi also keeps the Owned return convention.
    f.blocks[1].instructions.pop();
    f.return_ty = Type::Ref;
    f.blocks[1].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
    classes.clear();
    plan = TokenPlan {
        ret: super::super::super::TokenReturn::Owned,
        ..Default::default()
    };
    super::super::produce_and_verify_tokens(
        &f,
        &mut classes,
        &mut plan,
        &super::super::super::FunctionTables::default(),
        &ScopePlan::default(),
    )
    .unwrap();
    assert_eq!(classes[&ValueId(1)], Ownership::Owned);
}

#[test]
fn ordinary_nullable_coalesce_with_throwing_inline_fallback_certifies() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let entry = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/arc_ports/null_phi_coalesce.dart");
        let sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
        let mut interner = dartforge_intern::Interner::new();
        let program = dartforge_elements::load::load(&entry, &sdk, None, &mut interner).unwrap();
        let mut table = dartforge_types::TypeTable::new();
        let core = dartforge_types::CoreTypes::init(&mut table, &program, &interner);
        let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
        let libraries: Vec<_> = program.libraries.iter().enumerate().filter(|(_, library)| !library.is_sdk).map(|(i, _)| dartforge_elements::model::LibraryId(i as u32)).collect();
        let (bodies, _) = dartforge_types::infer_library_bodies_with_locals(&program, &interner, &mut table, &core, &mut outline, &libraries);
        let mut context = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies).with_source_sdk(); context.arc_memory = true;
        let mut m = crate::lower::lower_program(&context); assert!(m.errors.is_empty(), "{:?}", m.errors);
        m.arc_memory = true; m.arc_external_callees = true; m.arc_external_layouts = crate::lower::sdk_arc_layouts(&context);
        crate::optimize::optimize(&mut m);
        let before = m.functions.iter().find(|f| f.symbol.ends_with("Label.text")).unwrap();
        assert!(before.blocks.iter().flat_map(|b| &b.instructions).any(|(_, instruction, _)| matches!(instruction, Instruction::Phi { incoming, ty: Type::Ref } if incoming.len() > 1 && incoming.iter().all(|(_, operand)| *operand == Operand::Constant(Constant::Null)))));
        let mut plans = m.functions.iter().map(|f| (f.symbol.clone(), super::super::super::DartFunctionPlan::default())).collect();
        crate::optimize::arc::prepare_arc_dart_module(&mut m, &mut plans).unwrap();
        let after = m.functions.iter().find(|f| f.symbol.ends_with("Label.text")).unwrap();
        assert!(after.blocks.iter().flat_map(|b| &b.instructions).any(|(_, instruction, _)| matches!(instruction, Instruction::CallRuntime { name, .. } if name == "dartforge_exception_pending")));
        assert!(after.blocks.iter().flat_map(|b| &b.instructions).any(|(_, instruction, _)| matches!(instruction, Instruction::CallRuntime { name, .. } if name == "dartforge_arc_throw_ref_v1")));
        crate::optimize::arc::certify_arc_aot_ports(&mut m, &plans).unwrap();
    }).unwrap().join().unwrap();
}
