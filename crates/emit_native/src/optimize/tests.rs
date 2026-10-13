use super::*;

#[test]
fn tracing_pipeline_does_not_prepare_owned_boxing() {
    for by_tables in [false, true] {
        let mut m = Module::new();
        m.functions.push(Function {
            symbol: "caixa_tracing".into(), name: "caixa_tracing".into(),
            debug_info: None, params: vec![], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                (ValueId(0), Instruction::Box { op: Operand::Constant(Constant::Double(-0.0)), from: Type::F64 }, Type::Ref)
            ], terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }],
        });
        prepare_for_emission(&mut m, false, by_tables);
        assert!(!m.arc_memory);
        assert!(m.functions[0].blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i, Instruction::Box { from: Type::F64, .. })));
        for (_, i, _) in m.functions.iter().flat_map(|f| &f.blocks).flat_map(|b| &b.instructions) {
            assert!(!matches!(i, Instruction::ArcCopy { .. } | Instruction::ArcMove { .. } | Instruction::ArcDrop { .. }));
            assert!(!matches!(i, Instruction::CallRuntime { name, .. } if name.starts_with("dartforge_arc_")));
        }
    }
}

fn c(n: i64) -> Operand {
    Operand::Constant(Constant::Int(n))
}

#[test]
fn typed_fields_reinitialize_at_allocation_point_and_reject_mixed_bits() {
    for (ty, zero, value) in [
        (Type::F64, Constant::Double(0.0), Constant::Double(-0.0)),
        (Type::I1, Constant::Bool(false), Constant::Bool(true)),
        (Type::I8, Constant::Int(0), Constant::Int(255)),
    ] {
        let mut f = Function {
            symbol: "local_tipado".into(), name: "local_tipado".into(),
            debug_info: None, params: vec![], return_ty: ty,
            blocks: vec![
                BasicBlock { id: BlockId(0), instructions: vec![],
                    terminator: Terminator::Branch(BlockId(1)) },
                BasicBlock { id: BlockId(1), instructions: vec![
                    (ValueId(0), Instruction::CallRuntime {
                        name: "dartforge_object_new".into(),
                        args: vec![(c(128), Type::I64), (c(1), Type::I64)],
                        ret_ty: Type::Ref,
                    }, Type::Ref),
                    (ValueId(3), if ty == Type::I8 {
                        Instruction::Trunc { op: Operand::Constant(value), from: Type::I64, to: Type::I8 }
                    } else { Instruction::Const(value) }, ty),
                    (ValueId(1), Instruction::GetField { object: Operand::Val(ValueId(0)), index: 0 }, ty),
                    (ValueId(2), Instruction::SetField { object: Operand::Val(ValueId(0)), index: 0,
                        value: Operand::Val(ValueId(3)) }, Type::Void),
                ], terminator: Terminator::CondBranch {
                    cond: Operand::Constant(Constant::Bool(false)),
                    then_block: BlockId(1), else_block: BlockId(2),
                } },
                BasicBlock { id: BlockId(2), instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) },
            ],
        };
        let mut mixed = f.clone();
        mixed.blocks[1].instructions[3].1 = Instruction::CallRuntime {
            name: "dartforge_object_set".into(),
            args: vec![(Operand::Val(ValueId(0)), Type::Ref), (c(0), Type::I64),
                (c(0), Type::I64), (c(0), Type::I8)], ret_ty: Type::Void,
        };
        let before = format!("{mixed:?}");
        assert!(!escape::replace_objects(&mut mixed));
        assert_eq!(format!("{mixed:?}"), before);
        assert!(escape::replace_objects(&mut f));
        assert!(matches!(f.blocks[0].instructions.as_slice(),
            [(_, Instruction::Alloca(t), Type::Ptr)] if *t == ty));
        assert!(matches!(&f.blocks[1].instructions[0].1,
            Instruction::Store { val: Operand::Constant(z), .. } if *z == zero));
        assert!(matches!(&f.blocks[1].instructions[2].1,
            Instruction::Load { ty: t, .. } if *t == ty));
        assert!(!escape::replace_objects(&mut f));
    }
}

/// `p = Point(a, b); return p.x + p.y` in the shape the lowering produces: the
/// object vanishes, and the sum of the arguments remains.
#[test]
fn temporary_object_vanishes() {
    let v = ValueId;
    let new = Function {
        symbol: "Ponto.new".into(),
        name: "Ponto".into(),
        debug_info: None,
        params: vec![(v(0), "this".into(), Type::Ref), (v(1), "x".into(), Type::F64), (v(2), "y".into(), Type::F64)],
        return_ty: Type::Void,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (v(3), Instruction::Bitcast { op: Operand::Val(v(1)), to: Type::I64 }, Type::I64),
                (
                    v(4),
                    Instruction::CallRuntime {
                        name: "dartforge_object_set".into(),
                        args: vec![(Operand::Val(v(0)), Type::Ref), (c(0), Type::I64), (Operand::Val(v(3)), Type::I64), (c(0), Type::I8)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
                (v(5), Instruction::Bitcast { op: Operand::Val(v(2)), to: Type::I64 }, Type::I64),
                (
                    v(6),
                    Instruction::CallRuntime {
                        name: "dartforge_object_set".into(),
                        args: vec![(Operand::Val(v(0)), Type::Ref), (c(1), Type::I64), (Operand::Val(v(5)), Type::I64), (c(0), Type::I8)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    };
    let get = |r: u32, i: i64| {
        (
            v(r),
            Instruction::CallRuntime {
                name: "dartforge_object_get".into(),
                args: vec![(Operand::Val(v(12)), Type::Ref), (c(i), Type::I64)],
                ret_ty: Type::I64,
            },
            Type::I64,
        )
    };
    let sum = Function {
        symbol: "soma".into(),
        name: "soma".into(),
        debug_info: None,
        params: vec![(v(0), "a".into(), Type::F64), (v(1), "b".into(), Type::F64)],
        return_ty: Type::F64,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (v(2), Instruction::Alloca(Type::Ref), Type::Ptr),
                    (
                        v(3),
                        Instruction::CallRuntime {
                            name: "dartforge_object_new".into(),
                            args: vec![(c(921), Type::I64), (c(2), Type::I64)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        v(4),
                        Instruction::CallStatic { symbol: "Ponto.new".into(), args: vec![Operand::Val(v(3)), Operand::Val(v(0)), Operand::Val(v(1))], ret_ty: Type::Void },
                        Type::Void,
                    ),
                    (v(5), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                    (v(6), Instruction::ICmp(ICmpOp::Ne, Operand::Val(v(5)), c(0)), Type::I1),
                ],
                terminator: Terminator::CondBranch { cond: Operand::Val(v(6)), then_block: BlockId(2), else_block: BlockId(1) },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![
                    (v(11), Instruction::Store { ptr: Operand::Val(v(2)), val: Operand::Val(v(3)) }, Type::Void),
                    (v(12), Instruction::Load { ptr: Operand::Val(v(2)), ty: Type::Ref }, Type::Ref),
                    get(13, 0),
                    (v(14), Instruction::Bitcast { op: Operand::Val(v(13)), to: Type::F64 }, Type::F64),
                    get(15, 1),
                    (v(16), Instruction::Bitcast { op: Operand::Val(v(15)), to: Type::F64 }, Type::F64),
                    (v(17), Instruction::FAdd(Operand::Val(v(14)), Operand::Val(v(16))), Type::F64),
                ],
                terminator: Terminator::Return(Some(Operand::Val(v(17)))),
            },
            BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Double(0.0)))) },
        ],
    };
    let mut m = Module { functions: vec![new, sum], ..Default::default() };
    optimize(&mut m);
    let f = &m.functions[1];
    let text = format!("{:?}", f.blocks);
    assert!(!text.contains("object_new") && !text.contains("object_get") && !text.contains("Ponto.new"), "{text}");
    assert!(!text.contains("exception_pending") && !text.contains("Alloca"), "{text}");
    assert_eq!(f.blocks.len(), 1, "{text}");
    assert!(text.contains("FAdd"), "{text}");
}

/// An object that escapes (is returned) stays allocated.
#[test]
fn escaping_object_stays() {
    let v = ValueId;
    let f = Function {
        symbol: "f".into(),
        name: "f".into(),
        debug_info: None,
        params: vec![],
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                v(0),
                Instruction::CallRuntime { name: "dartforge_object_new".into(), args: vec![(c(921), Type::I64), (c(1), Type::I64)], ret_ty: Type::Ref },
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(v(0)))),
        }],
    };
    let mut m = Module { functions: vec![f], ..Default::default() };
    optimize(&mut m);
    assert!(format!("{:?}", m.functions[0].blocks).contains("object_new"));
}

/// A local written on both sides of an `if` becomes a `phi`.
#[test]
fn local_becomes_phi() {
    let v = ValueId;
    let f = Function {
        symbol: "g".into(),
        name: "g".into(),
        debug_info: None,
        params: vec![(v(0), "c".into(), Type::I1)],
        return_ty: Type::I64,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![(v(1), Instruction::Alloca(Type::I64), Type::Ptr)],
                terminator: Terminator::CondBranch { cond: Operand::Val(v(0)), then_block: BlockId(1), else_block: BlockId(2) },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![(v(2), Instruction::Store { ptr: Operand::Val(v(1)), val: c(1) }, Type::Void)],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![(v(3), Instruction::Store { ptr: Operand::Val(v(1)), val: c(2) }, Type::Void)],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![(v(4), Instruction::Load { ptr: Operand::Val(v(1)), ty: Type::I64 }, Type::I64)],
                terminator: Terminator::Return(Some(Operand::Val(v(4)))),
            },
        ],
    };
    let mut m = Module { functions: vec![f], ..Default::default() };
    optimize(&mut m);
    let text = format!("{:?}", m.functions[0].blocks);
    assert!(text.contains("Phi") && !text.contains("Alloca") && !text.contains("Load"), "{text}");
}

#[test]
fn allocation_class_removes_impossible_access_from_other_mixin() {
    for case in 0..4 {
        let (inst, ty) = match case {
            0 => (Instruction::CallRuntime { name: "dartforge_object_new".into(), args: vec![(c(547), Type::I64), (c(5), Type::I64)], ret_ty: Type::Ref }, Type::Ref),
            1 => (Instruction::AllocObject { class_id: 547, fields: vec![] }, Type::Ref),
            2 => (Instruction::CallRuntime { name: "opaca".into(), args: vec![], ret_ty: Type::Ref }, Type::Ref),
            _ => (Instruction::Const(Constant::Int(547)), Type::I64),
        };
        let mut f = Function { symbol: "mixin".into(), name: "mixin".into(), debug_info: None, params: vec![], return_ty: Type::I64,
            blocks: vec![
                BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(0), inst, ty),
                    (ValueId(1), Instruction::CallRuntime { name: "dartforge_value_class".into(), args: vec![(Operand::Val(ValueId(0)), Type::Ref)], ret_ty: Type::I64 }, Type::I64),
                ], terminator: Terminator::Switch { val: Operand::Val(ValueId(1)), default: BlockId(1), cases: vec![(547, BlockId(1)), (554, BlockId(2))] } },
                BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(c(1))) },
                BasicBlock { id: BlockId(2), instructions: vec![(ValueId(2), Instruction::GetField { object: Operand::Val(ValueId(0)), index: 6 }, Type::I64)], terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))) },
            ] };
        simplify::fold_constants(&mut f);
        simplify::remove_unreachable(&mut f);
        if case < 2 { assert_eq!(f.blocks.len(), 2); assert!(matches!(f.blocks[0].terminator, Terminator::Branch(BlockId(1)))); }
        else { assert_eq!(f.blocks.len(), 3); assert!(matches!(f.blocks[0].terminator, Terminator::Switch { .. })); }
    }
}

#[test]
fn constant_switch_removes_only_eliminated_edge_phi_entry() {
    for (value, expected) in [(547, BlockId(1)), (999, BlockId(1)), (554, BlockId(2))] {
        let mut f = Function { symbol: "switch_phi".into(), name: "switch_phi".into(), debug_info: None,
            params: vec![], return_ty: Type::I64, blocks: vec![
                BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::Switch { val: c(value), default: BlockId(1), cases: vec![(547, BlockId(1)), (554, BlockId(2)), (555, BlockId(2))] } },
                BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Branch(BlockId(3)) },
                BasicBlock { id: BlockId(2), instructions: vec![(ValueId(0), Instruction::Phi { incoming: vec![(BlockId(0), c(7)), (BlockId(3), c(9))], ty: Type::I64 }, Type::I64)], terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) },
                BasicBlock { id: BlockId(3), instructions: vec![], terminator: Terminator::Branch(BlockId(2)) },
            ] };
        simplify::fold_constants(&mut f);
        assert!(matches!(f.blocks[0].terminator, Terminator::Branch(b) if b == expected));
        let Instruction::Phi { incoming, .. } = &f.blocks[2].instructions[0].1 else { panic!("Phi removido sem conferir suas arestas"); };
        assert!(incoming.contains(&(BlockId(3), c(9))));
        assert_eq!(incoming.contains(&(BlockId(0), c(7))), expected == BlockId(2));
    }
}

/// The local that a small function received by address (the capture of a
/// direct local function) is left with only `Load`/`Store` after inlining and is
/// promoted.
#[test]
fn by_address_local_vanishes_after_inlining() {
    let v = ValueId;
    let inc = Function {
        symbol: "inc".into(),
        name: "inc".into(),
        debug_info: None,
        params: vec![(v(0), "p".into(), Type::Ptr)],
        return_ty: Type::Void,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (v(1), Instruction::Load { ptr: Operand::Val(v(0)), ty: Type::I64 }, Type::I64),
                (v(2), Instruction::Add(Operand::Val(v(1)), c(1)), Type::I64),
                (v(3), Instruction::Store { ptr: Operand::Val(v(0)), val: Operand::Val(v(2)) }, Type::Void),
            ],
            terminator: Terminator::Return(None),
        }],
    };
    let g = Function {
        symbol: "g".into(),
        name: "g".into(),
        debug_info: None,
        params: vec![],
        return_ty: Type::I64,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (v(0), Instruction::Alloca(Type::I64), Type::Ptr),
                (v(1), Instruction::Store { ptr: Operand::Val(v(0)), val: c(5) }, Type::Void),
                (v(2), Instruction::CallStatic { symbol: "inc".into(), args: vec![Operand::Val(v(0))], ret_ty: Type::Void }, Type::Void),
                (v(3), Instruction::Load { ptr: Operand::Val(v(0)), ty: Type::I64 }, Type::I64),
            ],
            terminator: Terminator::Return(Some(Operand::Val(v(3)))),
        }],
    };
    let mut m = Module { functions: vec![g, inc], ..Default::default() };
    optimize(&mut m);
    let text = format!("{:?}", m.functions[0].blocks);
    assert!(!text.contains("Alloca") && !text.contains("Load") && !text.contains("CallStatic"), "{text}");
}

/// `null == 0` (the null handle) is constant: the dead branch goes away.
#[test]
fn constant_null_comparison_folds() {
    let v = ValueId;
    let f = Function {
        symbol: "h".into(),
        name: "h".into(),
        debug_info: None,
        params: vec![],
        return_ty: Type::I64,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![(v(0), Instruction::ICmp(ICmpOp::Eq, Operand::Constant(Constant::Null), c(0)), Type::I1)],
                terminator: Terminator::CondBranch { cond: Operand::Val(v(0)), then_block: BlockId(1), else_block: BlockId(2) },
            },
            BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(c(1))) },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![(v(1), Instruction::GetField { object: Operand::Constant(Constant::Null), index: 0 }, Type::I64)],
                terminator: Terminator::Return(Some(Operand::Val(v(1)))),
            },
        ],
    };
    let mut m = Module { functions: vec![f], ..Default::default() };
    optimize(&mut m);
    let text = format!("{:?}", m.functions[0].blocks);
    assert!(!text.contains("GetField") && m.functions[0].blocks.len() == 1, "{text}");
}
