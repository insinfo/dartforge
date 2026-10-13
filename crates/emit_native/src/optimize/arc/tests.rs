use super::*;

#[test]
fn invoke_result_exists_only_after_success_including_in_phi() {
    let mut f = function(vec![
        block(
            0,
            vec![(
                ValueId(10),
                Instruction::CallStatic {
                    symbol: "produzir".into(),
                    args: vec![],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )],
            Terminator::CondBranch {
                cond: Operand::Constant(Constant::Bool(false)),
                then_block: BlockId(2),
                else_block: BlockId(1),
            },
        ),
        block(
            1,
            vec![],
            Terminator::Return(Some(Operand::Val(ValueId(10)))),
        ),
        block(2, vec![], Terminator::Return(None)),
    ]);
    let mut classes = parameter_classes();
    classes.insert(ValueId(10), Ownership::Owned);
    let tables = FunctionTables {
        invocations: HashMap::from([(ValueId(10), BlockId(2))]),
        landing_pads: HashSet::from([BlockId(2)]),
        ..Default::default()
    };
    assert!(classified_liveness_with_exceptions(&f, &classes, &tables).is_ok());
    f.blocks[2].terminator = Terminator::Return(Some(Operand::Val(ValueId(10))));
    assert!(
        classified_liveness_with_exceptions(&f, &classes, &tables)
            .unwrap_err()
            .contains("b2: resultado v10")
    );

    f.blocks[1].terminator = Terminator::Branch(BlockId(3));
    f.blocks[2].terminator = Terminator::Branch(BlockId(3));
    f.blocks.push(block(
        3,
        vec![(
            ValueId(11),
            Instruction::Phi {
                ty: Type::Ref,
                incoming: vec![
                    (BlockId(1), Operand::Val(ValueId(10))),
                    (BlockId(2), Operand::Constant(Constant::Null)),
                ],
            },
            Type::Ref,
        )],
        Terminator::Return(Some(Operand::Val(ValueId(11)))),
    ));
    classes.insert(ValueId(11), Ownership::Owned);
    assert!(classified_liveness_with_exceptions(&f, &classes, &tables).is_ok());
    if let Instruction::Phi { incoming, .. } = &mut f.blocks[3].instructions[0].1 {
        incoming[1].1 = Operand::Val(ValueId(10));
    }
    assert!(
        classified_liveness_with_exceptions(&f, &classes, &tables)
            .unwrap_err()
            .contains("b3: resultado v10")
    );

    // Phi no sucessor imediato usa o resultado na própria aresta que o publica.
    f.blocks.truncate(3);
    f.blocks[1].instructions = vec![(
        ValueId(11),
        Instruction::Phi {
            ty: Type::Ref,
            incoming: vec![(BlockId(0), Operand::Val(ValueId(10)))],
        },
        Type::Ref,
    )];
    f.blocks[1].terminator = Terminator::Return(Some(Operand::Val(ValueId(11))));
    f.blocks[2].terminator = Terminator::Return(None);
    assert!(classified_liveness_with_exceptions(&f, &classes, &tables).is_ok());
}

#[test]
fn invoke_does_not_sustain_alias_at_error_join() {
    let mut f = function(vec![
        block(
            0,
            vec![(
                ValueId(10),
                Instruction::CallStatic {
                    symbol: "produzir".into(),
                    args: vec![],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )],
            Terminator::CondBranch {
                cond: Operand::Constant(Constant::Bool(false)),
                then_block: BlockId(2),
                else_block: BlockId(1),
            },
        ),
        block(
            1,
            vec![(
                ValueId(11),
                Instruction::Bitcast {
                    op: Operand::Val(ValueId(10)),
                    to: Type::Ref,
                },
                Type::Ref,
            )],
            Terminator::Return(Some(Operand::Val(ValueId(11)))),
        ),
        block(2, vec![], Terminator::Return(None)),
    ]);
    let mut classes = parameter_classes();
    classes.insert(ValueId(10), Ownership::Owned);
    classes.insert(
        ValueId(11),
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(10)),
            scope: 0,
        },
    );
    let tables = FunctionTables {
        invocations: HashMap::from([(ValueId(10), BlockId(2))]),
        landing_pads: HashSet::from([BlockId(2)]),
        ..Default::default()
    };
    assert!(classified_liveness_with_exceptions(&f, &classes, &tables).is_ok());
    f.blocks[2].terminator = Terminator::Branch(BlockId(1));
    assert!(classified_liveness(&f, &classes).is_ok());
    assert_eq!(
        classified_liveness_with_exceptions(&f, &classes, &tables).unwrap_err(),
        "ARC003 em f: b1: v11 depende de resultado v10 indisponível no caminho excepcional; caminho [11, 10]"
    );
    f.blocks[0].terminator = Terminator::Branch(BlockId(1));
    assert!(
        classified_liveness_with_exceptions(&f, &classes, &tables)
            .unwrap_err()
            .contains("sem aresta de sucesso")
    );
}

#[test]
fn owner_in_other_branch_does_not_sustain_alias() {
    let f = function(vec![
        block(
            0,
            vec![],
            Terminator::CondBranch {
                cond: Operand::Val(ValueId(9)),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        block(
            1,
            vec![(
                ValueId(3),
                Instruction::CallStatic {
                    symbol: "produzir".into(),
                    args: vec![],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )],
            Terminator::Return(None),
        ),
        block(
            2,
            vec![(
                ValueId(2),
                Instruction::Bitcast {
                    op: Operand::Val(ValueId(0)),
                    to: Type::Ref,
                },
                Type::Ref,
            )],
            Terminator::Return(Some(Operand::Val(ValueId(2)))),
        ),
    ]);
    let mut classes = parameter_classes();
    classes.insert(ValueId(3), Ownership::Owned);
    classes.insert(
        ValueId(2),
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(3)),
            scope: 1,
        },
    );
    let e = classified_liveness(&f, &classes).unwrap_err();
    assert!(
        e.contains("ARC003") && e.contains("b2") && e.contains("[2, 3]"),
        "{e}"
    );
}

#[test]
fn owner_defined_after_alias_and_parameter_local_owner_are_rejected() {
    let f = function(vec![block(
        0,
        vec![
            (
                ValueId(2),
                Instruction::Bitcast {
                    op: Operand::Val(ValueId(0)),
                    to: Type::Ref,
                },
                Type::Ref,
            ),
            (
                ValueId(3),
                Instruction::CallStatic {
                    symbol: "produzir".into(),
                    args: vec![],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ),
        ],
        Terminator::Return(Some(Operand::Val(ValueId(2)))),
    )]);
    let mut classes = parameter_classes();
    classes.insert(ValueId(3), Ownership::Owned);
    classes.insert(
        ValueId(2),
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(3)),
            scope: 1,
        },
    );
    assert!(
        classified_liveness(&f, &classes)
            .unwrap_err()
            .contains("não domina")
    );
    classes.insert(ValueId(2), Ownership::Trivial);
    classes.insert(
        ValueId(0),
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(3)),
            scope: 0,
        },
    );
    assert!(
        classified_liveness(&f, &classes)
            .unwrap_err()
            .contains("parâmetro")
    );
}

#[test]
fn owner_phi_and_alias_phi_are_defined_simultaneously() {
    let f = function(vec![
        block(0, vec![], Terminator::Branch(BlockId(1))),
        block(
            1,
            vec![
                (
                    ValueId(2),
                    Instruction::Phi {
                        incoming: vec![(BlockId(0), Operand::Val(ValueId(0)))],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(3),
                    Instruction::Phi {
                        incoming: vec![(BlockId(0), Operand::Val(ValueId(0)))],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
            ],
            Terminator::Return(Some(Operand::Val(ValueId(2)))),
        ),
    ]);
    let mut classes = parameter_classes();
    classes.insert(ValueId(3), Ownership::Owned);
    classes.insert(
        ValueId(2),
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(3)),
            scope: 1,
        },
    );
    let v = classified_liveness(&f, &classes).unwrap();
    assert_eq!(v.edges[&(BlockId(0), BlockId(1))], refs(&[0]));
    assert!(v.entry[&BlockId(1)].is_empty());
}

fn parameter_classes() -> HashMap<ValueId, Ownership> {
    HashMap::from([
        (
            ValueId(0),
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0,
            },
        ),
        (
            ValueId(1),
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0,
            },
        ),
        (ValueId(9), Ownership::Trivial),
    ])
}

#[test]
fn classification_preserves_managed_alias_represented_by_i64() {
    let mut f = function(vec![block(
        0,
        vec![
            (
                ValueId(2),
                Instruction::Bitcast {
                    op: Operand::Val(ValueId(0)),
                    to: Type::I64,
                },
                Type::I64,
            ),
            (
                ValueId(3),
                Instruction::CallRuntime {
                    name: "dartforge_print".into(),
                    args: vec![(Operand::Val(ValueId(2)), Type::I64)],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ),
        ],
        Terminator::Return(None),
    )]);
    f.return_ty = Type::Void;
    let mut classes = parameter_classes();
    classes.insert(
        ValueId(2),
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(0)),
            scope: 1,
        },
    );
    classes.insert(ValueId(3), Ownership::Trivial);
    let v = classified_liveness(&f, &classes).unwrap();
    assert_eq!(v.before[&ValueId(3)], refs(&[0, 2]));
    assert!(v.after[&ValueId(3)].is_empty());
}

#[test]
fn typed_inventory_rejects_incomplete_coverage_and_stale_id() {
    let f = function(vec![block(0, vec![], Terminator::Return(None))]);
    let mut classes = parameter_classes();
    classes.remove(&ValueId(1));
    assert!(
        classified_liveness(&f, &classes)
            .unwrap_err()
            .contains("v1 sem classificação")
    );
    classes = parameter_classes();
    classes.insert(ValueId(8), Ownership::Owned);
    assert!(
        classified_liveness(&f, &classes)
            .unwrap_err()
            .contains("v8 não existe")
    );
}

#[test]
fn local_result_cannot_claim_caller_owner() {
    let f = function(vec![block(
        0,
        vec![(
            ValueId(2),
            Instruction::Bitcast {
                op: Operand::Val(ValueId(0)),
                to: Type::Ref,
            },
            Type::Ref,
        )],
        Terminator::Return(None),
    )]);
    let mut classes = parameter_classes();
    classes.insert(
        ValueId(2),
        Ownership::Borrowed {
            owner: OwnerOrigin::Caller,
            scope: 0,
        },
    );
    assert!(
        classified_liveness(&f, &classes)
            .unwrap_err()
            .contains("v2 local não é parâmetro")
    );
    classes.insert(
        ValueId(2),
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(1)),
            scope: 0,
        },
    );
    classes.insert(ValueId(1), Ownership::Trivial);
    assert!(
        classified_liveness(&f, &classes)
            .unwrap_err()
            .contains("ARC003")
    );
}

#[test]
fn transitive_borrow_holds_owner_until_last_use() {
    let mut f = function(vec![block(
        0,
        vec![(
            ValueId(3),
            Instruction::CallRuntime {
                name: "dartforge_print".into(),
                args: vec![(Operand::Val(ValueId(2)), Type::Ref)],
                ret_ty: Type::Void,
            },
            Type::Void,
        )],
        Terminator::Return(None),
    )]);
    f.params.push((ValueId(2), "alias2".into(), Type::Ref));
    f.return_ty = Type::Void;
    let v = liveness_with_borrows(
        &f,
        &refs(&[0, 1, 2]),
        &HashMap::from([(ValueId(2), ValueId(1)), (ValueId(1), ValueId(0))]),
    )
    .unwrap();
    assert_eq!(v.before[&ValueId(3)], refs(&[0, 1, 2]));
    assert!(v.after[&ValueId(3)].is_empty());
    assert_eq!(
        liveness(&f, &refs(&[0, 1, 2])).before[&ValueId(3)],
        refs(&[2])
    );
}

#[test]
fn borrow_owners_in_phi_stay_on_correct_edge() {
    let mut f = function(vec![
        block(
            0,
            vec![],
            Terminator::CondBranch {
                cond: Operand::Val(ValueId(9)),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        block(1, vec![], Terminator::Branch(BlockId(3))),
        block(2, vec![], Terminator::Branch(BlockId(3))),
        block(
            3,
            vec![(
                ValueId(4),
                Instruction::Phi {
                    incoming: vec![
                        (BlockId(1), Operand::Val(ValueId(2))),
                        (BlockId(2), Operand::Val(ValueId(3))),
                    ],
                    ty: Type::Ref,
                },
                Type::Ref,
            )],
            Terminator::Return(Some(Operand::Val(ValueId(4)))),
        ),
    ]);
    f.params.extend([
        (ValueId(2), "alias_x".into(), Type::Ref),
        (ValueId(3), "alias_y".into(), Type::Ref),
    ]);
    let v = liveness_with_borrows(
        &f,
        &refs(&[0, 1, 2, 3, 4]),
        &HashMap::from([(ValueId(2), ValueId(0)), (ValueId(3), ValueId(1))]),
    )
    .unwrap();
    assert_eq!(v.edges[&(BlockId(1), BlockId(3))], refs(&[0, 2]));
    assert_eq!(v.edges[&(BlockId(2), BlockId(3))], refs(&[1, 3]));
}

#[test]
fn borrow_rejects_missing_owner_and_cycle_with_stable_path() {
    let f = function(vec![block(0, vec![], Terminator::Return(None))]);
    let missing_one = liveness_with_borrows(
        &f,
        &refs(&[0, 1, 2]),
        &HashMap::from([(ValueId(1), ValueId(2))]),
    )
    .unwrap_err();
    assert_eq!(missing_one.function, "f");
    assert_eq!(missing_one.path, vec![ValueId(1), ValueId(2)]);
    let outside_inventory =
        liveness_with_borrows(&f, &refs(&[1]), &HashMap::from([(ValueId(1), ValueId(0))]))
            .unwrap_err();
    assert!(outside_inventory.to_string().contains("ARC003"));
    let cycle = [(ValueId(1), ValueId(0)), (ValueId(0), ValueId(1))];
    let a = liveness_with_borrows(&f, &refs(&[0, 1]), &HashMap::from(cycle)).unwrap_err();
    let b = liveness_with_borrows(&f, &refs(&[0, 1]), &HashMap::from([cycle[1], cycle[0]]))
        .unwrap_err();
    assert_eq!(a, b);
    assert_eq!(a.path, vec![ValueId(0), ValueId(1), ValueId(0)]);
}

#[test]
fn call_result_is_live_only_on_normal_edge() {
    let mut m = Module {
        functions: vec![function(vec![block(
            0,
            vec![(
                ValueId(10),
                Instruction::CallStatic {
                    symbol: "pode_lancar".into(),
                    args: Vec::new(),
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )],
            Terminator::Return(Some(Operand::Val(ValueId(10)))),
        )])],
        ..Default::default()
    };
    let _plan = super::super::tables::prepare(&mut m);
    let Terminator::CondBranch {
        then_block: landing_pad,
        else_block: continuation,
        ..
    } = m.functions[0].blocks[0].terminator
    else {
        panic!("CFG não preparado")
    };
    let inventory: HashSet<ValueId> = m.functions[0]
        .blocks
        .iter()
        .flat_map(|b| {
            b.instructions
                .iter()
                .filter(|(_, _, t)| *t == Type::Ref)
                .map(|(v, _, _)| *v)
        })
        .collect();
    let v = liveness(&m.functions[0], &inventory);
    assert_eq!(v.edges[&(BlockId(0), continuation)], refs(&[10]));
    assert!(v.edges[&(BlockId(0), landing_pad)].is_empty());
    assert!(
        v.edges[&(landing_pad, continuation)].is_empty(),
        "o valor padrão do erro não é o resultado inexistente"
    );
    assert!(v.before[&ValueId(10)].is_empty());
}

fn block(
    id: u32,
    instructions: Vec<(ValueId, Instruction, Type)>,
    terminator: Terminator,
) -> BasicBlock {
    BasicBlock {
        id: BlockId(id),
        instructions,
        terminator,
    }
}

fn function(blocks: Vec<BasicBlock>) -> Function {
    Function {
        symbol: "f".into(),
        name: "f".into(),
        debug_info: None,
        params: vec![
            (ValueId(0), "x".into(), Type::Ref),
            (ValueId(1), "y".into(), Type::Ref),
            (ValueId(9), "cond".into(), Type::I1),
        ],
        return_ty: Type::Ref,
        blocks,
    }
}

fn refs(ids: &[u32]) -> HashSet<ValueId> {
    ids.iter().copied().map(ValueId).collect()
}

#[test]
fn phi_requires_only_correct_predecessor_entry() {
    let f = function(vec![
        block(
            0,
            vec![],
            Terminator::CondBranch {
                cond: Operand::Val(ValueId(9)),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        block(1, vec![], Terminator::Branch(BlockId(3))),
        block(2, vec![], Terminator::Branch(BlockId(3))),
        block(
            3,
            vec![(
                ValueId(2),
                Instruction::Phi {
                    incoming: vec![
                        (BlockId(1), Operand::Val(ValueId(0))),
                        (BlockId(2), Operand::Val(ValueId(1))),
                    ],
                    ty: Type::Ref,
                },
                Type::Ref,
            )],
            Terminator::Return(Some(Operand::Val(ValueId(2)))),
        ),
    ]);
    let v = liveness(&f, &refs(&[0, 1, 2]));
    assert_eq!(v.edges[&(BlockId(1), BlockId(3))], refs(&[0]));
    assert_eq!(v.edges[&(BlockId(2), BlockId(3))], refs(&[1]));
    assert_eq!(v.entry[&BlockId(0)], refs(&[0, 1]));
    assert!(v.entry[&BlockId(3)].is_empty());
    assert_eq!(v.after[&ValueId(2)], refs(&[2]));
}

#[test]
fn backedge_converges_without_loading_next_iteration_result() {
    let f = function(vec![
        block(0, vec![], Terminator::Branch(BlockId(1))),
        block(
            1,
            vec![
                (
                    ValueId(2),
                    Instruction::Phi {
                        incoming: vec![
                            (BlockId(0), Operand::Val(ValueId(0))),
                            (BlockId(2), Operand::Val(ValueId(3))),
                        ],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(3),
                    Instruction::CallStatic {
                        symbol: "produzir".into(),
                        args: vec![Operand::Val(ValueId(2))],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
            ],
            Terminator::CondBranch {
                cond: Operand::Val(ValueId(9)),
                then_block: BlockId(2),
                else_block: BlockId(3),
            },
        ),
        block(2, vec![], Terminator::Branch(BlockId(1))),
        block(
            3,
            vec![],
            Terminator::Return(Some(Operand::Val(ValueId(2)))),
        ),
    ]);
    let v = liveness(&f, &refs(&[0, 2, 3]));
    assert_eq!(v.edges[&(BlockId(0), BlockId(1))], refs(&[0]));
    assert_eq!(v.edges[&(BlockId(2), BlockId(1))], refs(&[3]));
    assert_eq!(v.edges[&(BlockId(1), BlockId(3))], refs(&[2]));
    assert_eq!(v.before[&ValueId(3)], refs(&[2]));
    assert_eq!(v.after[&ValueId(3)], refs(&[2, 3]));
    assert!(v.entry[&BlockId(1)].is_empty());
}

#[test]
fn unreachable_and_scalar_create_no_liveness_obligation() {
    let f = function(vec![
        block(
            0,
            vec![],
            Terminator::Return(Some(Operand::Val(ValueId(0)))),
        ),
        block(
            1,
            vec![],
            Terminator::Return(Some(Operand::Val(ValueId(1)))),
        ),
    ]);
    let v = liveness(&f, &refs(&[0, 1]));
    assert_eq!(v.entry[&BlockId(0)], refs(&[0]));
    assert!(!v.entry.contains_key(&BlockId(1)));
    let without_ref = liveness(&f, &HashSet::new());
    assert!(without_ref.entry[&BlockId(0)].is_empty());
}
