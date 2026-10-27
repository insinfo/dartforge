//! Representation and exceptional-control-flow tests for nullable integer merges.
use super::*;

fn value(n: u32) -> Operand {
    Operand::Val(ValueId(n))
}
fn sample() -> Function {
    Function {
        symbol: "count".into(),
        name: "count".into(),
        debug_info: None,
        params: vec![
            (ValueId(0), "nullable".into(), Type::Ref),
            (ValueId(1), "integer".into(), Type::I64),
        ],
        return_ty: Type::I64,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(2),
                    Instruction::ICmp(ICmpOp::Eq, value(0), Operand::Constant(Constant::Null)),
                    Type::I1,
                )],
                terminator: Terminator::CondBranch {
                    cond: value(2),
                    then_block: BlockId(2),
                    else_block: BlockId(1),
                },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![(
                    ValueId(3),
                    Instruction::Box {
                        op: value(1),
                        from: Type::I64,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Branch(BlockId(2)),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![
                    (
                        ValueId(4),
                        Instruction::Phi {
                            incoming: vec![
                                (BlockId(0), Operand::Constant(Constant::Null)),
                                (BlockId(1), value(3)),
                            ],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(5),
                        Instruction::ICmp(ICmpOp::Eq, value(4), Operand::Constant(Constant::Null)),
                        Type::I1,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: value(5),
                    then_block: BlockId(3),
                    else_block: BlockId(4),
                },
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Constant(Constant::Int(0)))),
            },
            BasicBlock {
                id: BlockId(4),
                instructions: vec![
                    (
                        ValueId(6),
                        Instruction::Unbox {
                            op: value(4),
                            to: Type::I64,
                        },
                        Type::I64,
                    ),
                    (
                        ValueId(7),
                        Instruction::CallRuntime {
                            name: "dartforge_exception_pending".into(),
                            args: vec![],
                            ret_ty: Type::I8,
                        },
                        Type::I8,
                    ),
                    (
                        ValueId(8),
                        Instruction::ICmp(
                            ICmpOp::Ne,
                            value(7),
                            Operand::Constant(Constant::Int(0)),
                        ),
                        Type::I1,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: value(8),
                    then_block: BlockId(5),
                    else_block: BlockId(6),
                },
            },
            BasicBlock {
                id: BlockId(5),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Constant(Constant::Int(-1)))),
            },
            BasicBlock {
                id: BlockId(6),
                instructions: vec![],
                terminator: Terminator::Return(Some(value(6))),
            },
        ],
    }
}

#[test]
fn preserves_pending_edges_and_full_width_integer_values() {
    for integer in [0, -1, i64::MIN, i64::MAX, 1i64 << 62] {
        let mut f = sample();
        if let Instruction::Box { op, .. } = &mut f.blocks[1].instructions[0].1 {
            *op = Operand::Constant(Constant::Int(integer));
        }
        let edges: Vec<_> = f
            .blocks
            .iter()
            .map(|b| format!("{:?}", b.terminator))
            .collect();
        let pending = format!("{:?}", f.blocks[4].instructions[1]);
        assert_eq!(rewrite_function(&mut f), 1);
        assert!(f.blocks[1].instructions.is_empty());
        assert!(
            matches!(&f.blocks[2].instructions[0].1, Instruction::Phi { incoming, ty: Type::I64 } if incoming[1].1 == Operand::Constant(Constant::Int(integer)))
        );
        assert!(matches!(
            f.blocks[2].instructions[1].1,
            Instruction::Phi { ty: Type::I1, .. }
        ));
        assert!(matches!(
            f.blocks[4].instructions[0].1,
            Instruction::Add(
                Operand::Val(ValueId(4)),
                Operand::Constant(Constant::Int(0))
            )
        ));
        assert_eq!(format!("{:?}", f.blocks[4].instructions[1]), pending);
        assert_eq!(
            f.blocks
                .iter()
                .map(|b| format!("{:?}", b.terminator))
                .collect::<Vec<_>>(),
            edges
        );
    }
}

#[test]
fn refuses_shared_values_loops_wrong_types_and_unavailable_definitions() {
    for variant in 0..16 {
        let mut f = sample();
        match variant {
            0 => f.blocks[3].terminator = Terminator::Return(Some(value(4))),
            1 => f.blocks[3].terminator = Terminator::Return(Some(value(3))),
            2 => f.blocks[6].terminator = Terminator::Branch(BlockId(2)),
            3 => f.blocks[3].terminator = Terminator::Branch(BlockId(4)),
            4 => f.blocks[1].instructions[0].2 = Type::I64,
            5 => {
                if let Instruction::Box { from, .. } = &mut f.blocks[1].instructions[0].1 {
                    *from = Type::F64;
                }
            }
            6 => {
                if let Instruction::Phi { incoming, .. } = &mut f.blocks[2].instructions[0].1 {
                    incoming[0].0 = BlockId(1);
                }
            }
            7 => f.blocks[4].instructions[0].2 = Type::Ref,
            8 => f.blocks[2].terminator = Terminator::Branch(BlockId(4)),
            9 => f.blocks[0].terminator = Terminator::Branch(BlockId(99)),
            10 => {
                f.params.pop();
                f.blocks[3].instructions.push((
                    ValueId(1),
                    Instruction::Const(Constant::Int(9)),
                    Type::I64,
                ));
            }
            11 => f.blocks[6].id = BlockId(5),
            12 => f.params[1].2 = Type::Ref,
            13 => {
                if let Instruction::Phi { incoming, .. } = &mut f.blocks[2].instructions[0].1 {
                    incoming[0].1 = value(0);
                }
            }
            14 => {
                f.blocks[1].instructions[0].1 = Instruction::CallRuntime {
                    name: "opaque_integer_box".into(),
                    args: vec![(value(1), Type::I64)],
                    ret_ty: Type::Ref,
                }
            }
            15 => f.blocks[2].instructions[0].2 = Type::I64,
            _ => unreachable!(),
        }
        let original = format!("{f:?}");
        assert_eq!(rewrite_function(&mut f), 0, "variant {variant}");
        assert_eq!(format!("{f:?}"), original);
    }
}

#[test]
fn local_proof_allows_sdk_and_arc_memory_and_refuses_prepared_and_runtime_homonyms() {
    let mut module = Module::new();
    module.sdk_library = true;
    module.functions.push(sample());
    assert_eq!(rewrite(&mut module), 1);
    // Before ARC preparation the merged box is an ordinary fresh object that
    // never escapes: removing it leaves fewer owners for the preparation.
    let mut module = Module::new();
    module.arc_memory = true;
    module.functions.push(sample());
    assert_eq!(rewrite(&mut module), 1);
    for variant in 0..3 {
        let mut module = Module::new();
        module.functions.push(sample());
        match variant {
            0 => module.arc_prepared = true,
            1 => module.functions[0].symbol = "dartforge_unbox_int".into(),
            2 => module.functions[0].symbol = "df.allocate".into(),
            _ => unreachable!(),
        }
        assert_eq!(rewrite(&mut module), 0);
    }
}

#[test]
fn ordinary_recursive_nullable_count_removes_both_box_unbox_pairs() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let entry = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/devirtualization/scalar_nullable_count.dart");
        let sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
        let mut interner = dartforge_intern::Interner::new();
        let program = dartforge_elements::load::load(&entry, &sdk, None, &mut interner).unwrap();
        let mut table = dartforge_types::TypeTable::new();
        let core = dartforge_types::CoreTypes::init(&mut table, &program, &interner);
        let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
        let libraries: Vec<_> = program.libraries.iter().enumerate().filter(|(_, library)| !library.is_sdk).map(|(i, _)| dartforge_elements::model::LibraryId(i as u32)).collect();
        let (bodies, _) = dartforge_types::infer_library_bodies_with_locals(&program, &interner, &mut table, &core, &mut outline, &libraries);
        let context = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies).with_source_sdk();
        let mut module = crate::lower::lower_program(&context);
        assert!(module.errors.is_empty(), "{:?}", module.errors);
        // Production optimize invokes this pass; the ordinary method remains recursive.
        super::super::optimize(&mut module);
        let f = module.functions.iter().find(|f| f.name == "Node.count" || f.symbol.contains("Node.count")).expect("ordinary count method");
        assert_eq!(f.blocks.iter().flat_map(|b| &b.instructions).filter(|(_, i, _)| matches!(i, Instruction::Box { .. } | Instruction::Unbox { .. })).count(), 0, "{f:#?}");
        assert_eq!(f.blocks.iter().flat_map(|b| &b.instructions).filter(|(_, i, _)| matches!(i, Instruction::CallStatic { symbol, .. } if symbol.contains("Node.count"))).count(), 2);
    }).unwrap().join().unwrap();
}
