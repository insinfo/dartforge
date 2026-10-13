//! Cases of linear consumption, joins, backedges and exceptional results.
use super::*;

fn value(v: u32) -> Operand {
    Operand::Val(ValueId(v))
}
fn block(
    b: u32,
    instructions: Vec<(ValueId, Instruction, Type)>,
    terminator: Terminator,
) -> BasicBlock {
    BasicBlock {
        id: BlockId(b),
        instructions,
        terminator,
    }
}
fn function(blocks: Vec<BasicBlock>) -> Function {
    Function {
        symbol: "tokens".into(),
        name: "tokens".into(),
        debug_info: None,
        params: vec![
            (ValueId(0), "x".into(), Type::Ref),
            (ValueId(9), "cond".into(), Type::I1),
        ],
        return_ty: Type::Void,
        blocks,
    }
}
fn classes(owned: &[u32], trivial: &[u32]) -> HashMap<ValueId, Ownership> {
    let mut c = HashMap::from([
        (
            ValueId(0),
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0,
            },
        ),
        (ValueId(9), Ownership::Trivial),
    ]);
    c.extend(owned.iter().map(|&v| (ValueId(v), Ownership::Owned)));
    c.extend(trivial.iter().map(|&v| (ValueId(v), Ownership::Trivial)));
    c
}
fn copy_inst(v: u32, origin: u32) -> (ValueId, Instruction, Type) {
    (
        ValueId(v),
        Instruction::ArcCopy {
            value: value(origin),
        },
        Type::Ref,
    )
}
fn move_inst(v: u32, origin: u32) -> (ValueId, Instruction, Type) {
    (
        ValueId(v),
        Instruction::ArcMove {
            value: value(origin),
        },
        Type::Ref,
    )
}
fn drop(v: u32, origin: u32) -> (ValueId, Instruction, Type) {
    (
        ValueId(v),
        Instruction::ArcDrop {
            value: value(origin),
        },
        Type::Void,
    )
}
fn verify(f: &Function, c: &HashMap<ValueId, Ownership>) -> Result<(), String> {
    verify_tokens(f, c, &FunctionTables::default(), &TokenPlan::default())
}

#[test]
fn borrowed_phi_preserves_real_entry_owner() {
    let f = function(vec![
        block(0, vec![copy_inst(1, 0)], Terminator::Branch(BlockId(1))),
        block(
            1,
            vec![
                (
                    ValueId(2),
                    Instruction::Phi {
                        ty: Type::Ref,
                        incoming: vec![(BlockId(0), value(1))],
                    },
                    Type::Ref,
                ),
                copy_inst(3, 2),
                drop(4, 3),
                drop(5, 1),
            ],
            Terminator::Return(None),
        ),
    ]);
    let mut c = classes(&[1, 3], &[4, 5]);
    c.insert(
        ValueId(2),
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(1)),
            scope: 0,
        },
    );
    verify(&f, &c).unwrap();
    c.insert(
        ValueId(2),
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(0)),
            scope: 0,
        },
    );
    let error = verify(&f, &c).unwrap_err();
    assert!(error.contains("owner ou escopo incompatível"), "{error}");
}

#[test]
fn borrowed_phi_does_not_lose_scope_inherited_from_alias() {
    let mut f = function(vec![
        block(0, vec![], Terminator::Branch(BlockId(1))),
        block(
            1,
            vec![
                (
                    ValueId(2),
                    Instruction::Phi {
                        ty: Type::Ref,
                        incoming: vec![(BlockId(0), value(7))],
                    },
                    Type::Ref,
                ),
                copy_inst(3, 2),
                drop(4, 3),
                drop(5, 0),
            ],
            Terminator::Return(None),
        ),
    ]);
    f.params.extend([
        (ValueId(6), "emprestimo".into(), Type::Ref),
        (ValueId(7), "alias".into(), Type::Ref),
    ]);
    let mut c = classes(&[0, 3], &[4, 5]);
    c.insert(
        ValueId(6),
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(0)),
            scope: 7,
        },
    );
    c.insert(
        ValueId(7),
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(6)),
            scope: 0,
        },
    );
    c.insert(
        ValueId(2),
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(0)),
            scope: 0,
        },
    );
    assert!(
        verify(&f, &c)
            .unwrap_err()
            .contains("owner ou escopo incompatível")
    );
    // The result chain keeps the limit, even with a zero direct scope.
    c.insert(
        ValueId(2),
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(7)),
            scope: 0,
        },
    );
    verify(&f, &c).unwrap();
    // It may also represent the same limit directly, without losing the root.
    c.insert(
        ValueId(2),
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(0)),
            scope: 7,
        },
    );
    verify(&f, &c).unwrap();
}

#[test]
fn borrowed_phi_checks_lexical_limit_in_full_wrapper() {
    use super::super::{ScopeChange, ScopePlan, produce_and_verify_tokens};
    let f = function(vec![
        block(
            0,
            vec![
                (
                    ValueId(6),
                    Instruction::EnvGet { env: value(0), index: 0 },
                    Type::Ref,
                ),
                (
                    ValueId(7),
                    Instruction::EnvGet { env: value(6), index: 0 },
                    Type::Ref,
                ),
            ],
            Terminator::Branch(BlockId(1)),
        ),
        block(
            1,
            vec![
                (
                    ValueId(2),
                    Instruction::Phi {
                        ty: Type::Ref,
                        incoming: vec![(BlockId(0), value(7))],
                    },
                    Type::Ref,
                ),
                copy_inst(3, 2),
                drop(4, 3),
                drop(5, 0),
            ],
            Terminator::Return(None),
        ),
    ]);
    let mut c = classes(&[0], &[]);
    c.insert(
        ValueId(6),
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(0)),
            scope: 7,
        },
    );
    c.insert(
        ValueId(7),
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(6)),
            scope: 0,
        },
    );
    c.insert(
        ValueId(2),
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(7)),
            scope: 0,
        },
    );
    // The provenance of getters is a premise; the read does not run Dart.
    let mut p = TokenPlan::default();
    p.instructions.insert(ValueId(6), TokenEffect::default());
    p.instructions.insert(ValueId(7), TokenEffect::default());
    let mut e = ScopePlan {
        before: HashMap::from([(ValueId(6), vec![ScopeChange::Open(7)])]),
        exits: HashMap::from([(BlockId(1), vec![ScopeChange::Close(7)])]),
        ..Default::default()
    };
    produce_and_verify_tokens(&f, &mut c, &mut p, &FunctionTables::default(), &e).unwrap();
    let before = c.clone();
    let effects = p.instructions.clone();
    e.exits.clear();
    e.before.insert(ValueId(3), vec![ScopeChange::Close(7)]);
    assert!(
        produce_and_verify_tokens(&f, &mut c, &mut p, &FunctionTables::default(), &e)
            .unwrap_err()
            .contains("fora do escopo 7")
    );
    assert_eq!(c, before);
    assert_eq!(p.instructions, effects);
}

#[test]
fn audited_extern_does_not_accept_plan_omitting_consumption() {
    let call = Instruction::CallRuntime {
        name: "dartforge_arc_release".into(),
        args: vec![(value(1), Type::Ref)],
        ret_ty: Type::Void,
    };
    let contract = super::super::runtime_call_contract(&call).unwrap();
    let f = function(vec![block(
        0,
        vec![copy_inst(1, 0), (ValueId(2), call, Type::Void)],
        Terminator::Return(None),
    )]);
    let c = classes(&[1], &[2]);
    let mut plan = TokenPlan::default();
    plan.instructions.insert(ValueId(2), contract.effect);
    assert!(verify_tokens(&f, &c, &FunctionTables::default(), &plan).is_ok());
    plan.instructions.insert(ValueId(2), TokenEffect::default());
    let error = verify_tokens(&f, &c, &FunctionTables::default(), &plan).unwrap_err();
    assert!(error.contains("ownership.tsv"));
}

#[test]
fn runtime_pending_checks_cleanup_without_llvm_landing_pad() {
    let mut f = function(vec![
        block(
            0,
            vec![
                copy_inst(1, 0),
                (
                    ValueId(2),
                    Instruction::CallRuntime {
                        name: "dartforge_print_handle".into(),
                        args: vec![(value(1), Type::Ref)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
                (
                    ValueId(3),
                    Instruction::CallRuntime {
                        name: "dartforge_exception_pending".into(),
                        args: vec![],
                        ret_ty: Type::I8,
                    },
                    Type::I8,
                ),
                (
                    ValueId(4),
                    Instruction::ICmp(ICmpOp::Ne, value(3), Operand::Constant(Constant::Int(0))),
                    Type::I1,
                ),
            ],
            Terminator::CondBranch {
                cond: value(4),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        block(1, vec![drop(5, 1)], Terminator::Return(None)),
        block(2, vec![drop(6, 1)], Terminator::Return(None)),
    ]);
    let mut c = classes(&[], &[]);
    let mut p = TokenPlan::default();
    let t = FunctionTables::default();
    p.pendings.insert(ValueId(2), BlockId(2));
    let before = c.clone();
    assert!(super::super::produce_arc_contracts(&f, &mut c, &mut p).is_err());
    assert_eq!(c, before);
    assert!(p.instructions.is_empty());
    assert_eq!(p.pendings[&ValueId(2)], BlockId(2));
    p.pendings.clear();
    super::super::produce_and_verify_tokens(
        &f,
        &mut c,
        &mut p,
        &t,
        &super::super::ScopePlan::default(),
    )
    .unwrap();
    assert!(t.invocations.is_empty() && t.landing_pads.is_empty());
    assert_eq!(p.pendings[&ValueId(2)], BlockId(1));
    f.blocks[1].instructions.clear();
    c.remove(&ValueId(5));
    assert!(
        verify_tokens(&f, &c, &t, &p)
            .unwrap_err()
            .contains("não consumidos")
    );
    f.blocks[1].instructions.push(drop(5, 1));
    c.insert(ValueId(5), Ownership::Trivial);
    f.blocks[0].terminator = Terminator::CondBranch {
        cond: value(4),
        then_block: BlockId(2),
        else_block: BlockId(1),
    };
    assert!(verify_tokens(&f, &c, &t, &p).is_err());
}

#[test]
fn runtime_pending_cannot_be_published_as_llvm_invoke() {
    let f = function(vec![
        block(
            0,
            vec![(
                ValueId(1),
                Instruction::CallRuntime {
                    name: "dartforge_gc_collect".into(),
                    args: vec![],
                    ret_ty: Type::Void,
                },
                Type::Void,
            )],
            Terminator::CondBranch {
                cond: Operand::Constant(Constant::Bool(false)),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        block(1, vec![], Terminator::Return(None)),
        block(2, vec![], Terminator::Return(None)),
    ]);
    let mut c = classes(&[], &[]);
    let mut p = TokenPlan::default();
    super::super::produce_runtime_contracts(&f, &mut c, &mut p).unwrap();
    let mut t = FunctionTables::default();
    t.invocations.insert(ValueId(1), BlockId(1));
    t.landing_pads.insert(BlockId(1));
    assert!(
        verify_tokens(&f, &c, &t, &p)
            .unwrap_err()
            .contains("não invoke LLVM")
    );
}

#[test]
fn audited_pending_runtime_producer_requires_error_exit() {
    let f = function(vec![
        block(
            0,
            vec![
                (
                    ValueId(1),
                    Instruction::CallRuntime {
                        name: "dartforge_gc_collect".into(),
                        args: vec![],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
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
                    Instruction::ICmp(ICmpOp::Ne, value(2), Operand::Constant(Constant::Int(0))),
                    Type::I1,
                ),
            ],
            Terminator::CondBranch {
                cond: value(3),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        block(1, vec![], Terminator::Return(None)),
        block(2, vec![], Terminator::Return(None)),
    ]);
    let mut c = classes(&[], &[]);
    let mut p = TokenPlan::default();
    super::super::produce_arc_contracts(&f, &mut c, &mut p).unwrap();
    let t = FunctionTables::default();
    verify_tokens(&f, &c, &t, &p).unwrap();
    p.pendings.clear();
    assert!(verify_tokens(&f, &c, &FunctionTables::default(), &p).is_err());
    p.pendings.insert(ValueId(1), BlockId(1));
    p.instructions.get_mut(&ValueId(1)).unwrap().may_fail = false;
    assert!(
        verify_tokens(&f, &c, &t, &p)
            .unwrap_err()
            .contains("ownership.tsv")
    );
}

#[test]
fn arc_producer_classifies_fixed_operations_and_leaves_no_partial_state() {
    let slot = StrongSlot::Quadro {
        frame: value(9),
        index: 0,
    };
    let mut f = function(vec![block(
        0,
        vec![
            copy_inst(1, 0),
            (
                ValueId(2),
                Instruction::ArcStoreStrong {
                    slot: slot.clone(),
                    value: value(1),
                    mode: StrongStoreMode::Move,
                },
                Type::Void,
            ),
            (ValueId(3), Instruction::ArcLoadStrong { slot }, Type::Ref),
            move_inst(4, 3),
            drop(5, 4),
            (
                ValueId(6),
                Instruction::CallRuntime {
                    name: "dartforge_arc_quadro_fechar_v1".into(),
                    args: vec![(value(9), Type::I64)],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ),
        ],
        Terminator::Return(None),
    )]);
    f.params[1].2 = Type::I64;
    let mut c = classes(&[], &[]);
    let initial = c.clone();
    let mut p = TokenPlan::default();
    f.blocks[0].instructions[4].2 = Type::I64;
    assert!(super::super::produce_arc_contracts(&f, &mut c, &mut p).is_err());
    assert_eq!(c, initial);
    assert!(p.instructions.is_empty());
    f.blocks[0].instructions[4].2 = Type::Void;
    super::super::produce_arc_contracts(&f, &mut c, &mut p).unwrap();
    assert_eq!(c[&ValueId(3)], Ownership::Owned);
    assert_eq!(c[&ValueId(2)], Ownership::Trivial);
    assert_eq!(p.instructions.len(), 1);
    verify_tokens(&f, &c, &FunctionTables::default(), &p).unwrap();
    p.instructions.insert(ValueId(1), TokenEffect::default());
    let before = c.clone();
    assert!(super::super::produce_arc_contracts(&f, &mut c, &mut p).is_err());
    assert_eq!(c, before);
}

#[test]
fn copy_move_drop_and_double_consumption() {
    let mut f = function(vec![block(
        0,
        vec![copy_inst(1, 0), move_inst(2, 1), drop(3, 2)],
        Terminator::Return(None),
    )]);
    let mut c = classes(&[1, 2], &[3]);
    verify(&f, &c).unwrap();
    f.blocks[0].instructions.push(drop(4, 2));
    c.insert(ValueId(4), Ownership::Trivial);
    assert!(verify(&f, &c).unwrap_err().contains("indisponível v2"));
    f.blocks[0].instructions.pop();
    c.remove(&ValueId(4));
    f.blocks[0].instructions[2] = drop(3, 1);
    assert!(verify(&f, &c).unwrap_err().contains("indisponível v1"));
    f.blocks[0].instructions[1] = move_inst(2, 0);
    assert!(verify(&f, &c).unwrap_err().contains("exige Owned"));
}

#[test]
fn exit_with_token_and_join_with_conditional_consumption_fail() {
    let f = function(vec![block(0, vec![copy_inst(1, 0)], Terminator::Return(None))]);
    assert!(
        verify(&f, &classes(&[1], &[]))
            .unwrap_err()
            .contains("não consumidos")
    );
    let f = function(vec![
        block(
            0,
            vec![copy_inst(1, 0)],
            Terminator::CondBranch {
                cond: value(9),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        block(1, vec![drop(3, 1)], Terminator::Branch(BlockId(3))),
        block(2, vec![], Terminator::Branch(BlockId(3))),
        block(3, vec![], Terminator::Return(None)),
    ]);
    let e = verify(&f, &classes(&[1], &[3])).unwrap_err();
    assert!(
        e.contains("junção b3")
            && e.contains("caminho anterior [0, 1, 3]")
            && e.contains("caminho [0, 2]"),
        "{e}"
    );
}

#[test]
fn nullable_phi_transfers_one_token_per_edge() {
    let f = function(vec![
        block(
            0,
            vec![],
            Terminator::CondBranch {
                cond: value(9),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        block(1, vec![copy_inst(1, 0)], Terminator::Branch(BlockId(3))),
        block(2, vec![], Terminator::Branch(BlockId(3))),
        block(
            3,
            vec![
                (
                    ValueId(2),
                    Instruction::Phi {
                        incoming: vec![
                            (BlockId(1), value(1)),
                            (BlockId(2), Operand::Constant(Constant::Null)),
                        ],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                drop(3, 2),
            ],
            Terminator::Return(None),
        ),
    ]);
    let mut c = classes(&[], &[]);
    let mut p = TokenPlan::default();
    super::super::produce_arc_contracts(&f, &mut c, &mut p).unwrap();
    verify_tokens(&f, &c, &FunctionTables::default(), &p).unwrap();
}

#[test]
fn loop_phi_transfers_without_accumulating_and_swaps_simultaneously() {
    let f = function(vec![
        block(0, vec![copy_inst(1, 0)], Terminator::Branch(BlockId(1))),
        block(
            1,
            vec![
                (
                    ValueId(2),
                    Instruction::Phi {
                        incoming: vec![(BlockId(0), value(1)), (BlockId(1), value(3))],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                move_inst(3, 2),
            ],
            Terminator::CondBranch {
                cond: value(9),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        block(2, vec![drop(4, 3)], Terminator::Return(None)),
    ]);
    let mut c = classes(&[], &[]);
    let mut p = TokenPlan::default();
    super::super::produce_arc_contracts(&f, &mut c, &mut p).unwrap();
    verify_tokens(&f, &c, &FunctionTables::default(), &p).unwrap();
    let f = function(vec![
        block(
            0,
            vec![copy_inst(1, 0), copy_inst(2, 0)],
            Terminator::Branch(BlockId(1)),
        ),
        block(
            1,
            vec![
                (
                    ValueId(3),
                    Instruction::Phi {
                        incoming: vec![(BlockId(0), value(1)), (BlockId(1), value(4))],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(4),
                    Instruction::Phi {
                        incoming: vec![(BlockId(0), value(2)), (BlockId(1), value(3))],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
            ],
            Terminator::CondBranch {
                cond: value(9),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        block(2, vec![drop(5, 3), drop(6, 4)], Terminator::Return(None)),
    ]);
    let mut c = classes(&[], &[]);
    let mut p = TokenPlan::default();
    super::super::produce_arc_contracts(&f, &mut c, &mut p).unwrap();
    verify_tokens(&f, &c, &FunctionTables::default(), &p).unwrap();
}

#[test]
fn phi_producer_rejects_borrow_and_originless_cycle_without_changing_maps() {
    let phi = |incoming| {
        (
            ValueId(2),
            Instruction::Phi {
                incoming,
                ty: Type::Ref,
            },
            Type::Ref,
        )
    };
    let f = function(vec![
        block(
            0,
            vec![],
            Terminator::CondBranch {
                cond: value(9),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        block(1, vec![copy_inst(1, 0)], Terminator::Branch(BlockId(3))),
        block(2, vec![], Terminator::Branch(BlockId(3))),
        block(
            3,
            vec![
                phi(vec![(BlockId(1), value(1)), (BlockId(2), value(0))]),
                drop(3, 2),
            ],
            Terminator::Return(None),
        ),
    ]);
    let mut c = classes(&[], &[]);
    let initial = c.clone();
    let mut p = TokenPlan::default();
    assert!(
        super::super::produce_arc_contracts(&f, &mut c, &mut p)
            .unwrap_err()
            .contains("entrada não é owned/null")
    );
    assert_eq!(c, initial);
    assert!(p.instructions.is_empty());
    let f = function(vec![block(
        0,
        vec![phi(vec![(BlockId(0), value(3))]), move_inst(3, 2)],
        Terminator::Branch(BlockId(0)),
    )]);
    assert!(
        super::super::produce_arc_contracts(&f, &mut c, &mut p)
            .unwrap_err()
            .contains("sem origem owned/null")
    );
    assert_eq!(c, initial);
    assert!(p.instructions.is_empty());
}

#[test]
fn verified_production_does_not_publish_function_metadata_with_leaked_token() {
    let mut f = function(vec![block(0, vec![copy_inst(1, 0)], Terminator::Return(None))]);
    let mut c = classes(&[], &[]);
    let initial = c.clone();
    let mut p = TokenPlan::default();
    let t = FunctionTables::default();
    let s = super::super::ScopePlan::default();
    assert!(
        super::super::produce_and_verify_tokens(&f, &mut c, &mut p, &t, &s)
            .unwrap_err()
            .contains("não consumidos")
    );
    assert_eq!(c, initial);
    assert!(p.instructions.is_empty());
    f.blocks[0].instructions.push(drop(2, 1));
    super::super::produce_and_verify_tokens(&f, &mut c, &mut p, &t, &s).unwrap();
    assert_eq!(c[&ValueId(1)], Ownership::Owned);
    assert_eq!(c[&ValueId(2)], Ownership::Trivial);
    // The parameter classification comes from the semantic producer; a
    // nonexistent scope cannot be hidden by the generation of the primitives.
    let mut c = initial.clone();
    c.insert(
        ValueId(0),
        Ownership::Borrowed {
            owner: OwnerOrigin::Caller,
            scope: 7,
        },
    );
    let before = c.clone();
    assert!(super::super::produce_and_verify_tokens(&f, &mut c, &mut p, &t, &s).is_err());
    assert_eq!(c, before);
}

#[test]
fn audited_borrowed_result_depends_on_argument_owner() {
    let mut f = function(vec![block(
        0,
        vec![
            copy_inst(1, 0),
            (
                ValueId(2),
                Instruction::CallRuntime {
                    name: "dartforge_nativo_DartForge_record_fieldAt".into(),
                    args: vec![
                        (value(1), Type::Ref),
                        (Operand::Constant(Constant::Int(0)), Type::I64),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ),
        ],
        Terminator::Branch(BlockId(2)),
    ), block(
        2,
        vec![copy_inst(4, 2), drop(5, 4), drop(6, 1)],
        Terminator::Return(None),
    )]);
    let mut c = classes(&[], &[]);
    let mut p = TokenPlan::default();
    let t = FunctionTables::default();
    super::super::produce_and_verify_tokens(
        &f,
        &mut c,
        &mut p,
        &t,
        &super::super::ScopePlan::default(),
    )
    .unwrap();
    assert_eq!(
        c[&ValueId(2)],
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(1)),
            scope: 0
        }
    );
    assert!(!p.pendings.contains_key(&ValueId(2)));
    // Releasing the owner before the last use of the borrow is rejected.
    f.blocks[1].instructions.rotate_right(1);
    let error = verify_tokens(&f, &c, &t, &p).unwrap_err();
    assert!(error.contains("indisponível v1"), "{error}");
}

#[test]
fn borrow_does_not_outlive_consumed_owner_or_escape_in_return() {
    let mut f = function(vec![block(
        0,
        vec![
            copy_inst(1, 0),
            (
                ValueId(2),
                Instruction::EnvGet {
                    env: value(1),
                    index: 0,
                },
                Type::Ref,
            ),
            drop(3, 1),
            copy_inst(4, 2),
            drop(5, 4),
        ],
        Terminator::Return(None),
    )]);
    let mut c = classes(&[1, 4], &[3, 5]);
    c.insert(
        ValueId(2),
        Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(1)),
            scope: 0,
        },
    );
    let mut p = TokenPlan::default();
    p.instructions.insert(ValueId(2), TokenEffect::default());
    assert!(
        verify_tokens(&f, &c, &FunctionTables::default(), &p)
            .unwrap_err()
            .contains("indisponível v1")
    );
    f.blocks[0].instructions.truncate(2);
    f.blocks[0].terminator = Terminator::Return(Some(value(2)));
    f.return_ty = Type::Ref;
    c.remove(&ValueId(3));
    c.remove(&ValueId(4));
    c.remove(&ValueId(5));
    p.ret = TokenReturn::Borrowed;
    assert!(
        verify_tokens(&f, &c, &FunctionTables::default(), &p)
            .unwrap_err()
            .contains("retorno não satisfaz")
    );
}

#[test]
fn invoke_consumes_argument_on_both_exits_and_produces_only_on_success() {
    let mut f = function(vec![
        block(
            0,
            vec![
                copy_inst(1, 0),
                (
                    ValueId(2),
                    Instruction::CallStatic {
                        symbol: "consumir".into(),
                        args: vec![value(1)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
            ],
            Terminator::CondBranch {
                cond: Operand::Constant(Constant::Bool(false)),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        block(
            1,
            vec![],
            Terminator::Return(Some(Operand::Constant(Constant::Null))),
        ),
        block(2, vec![], Terminator::Return(Some(value(2)))),
    ]);
    f.return_ty = Type::Ref;
    let c = classes(&[1, 2], &[]);
    let mut t = FunctionTables::default();
    t.invocations.insert(ValueId(2), BlockId(1));
    t.landing_pads.insert(BlockId(1));
    let mut p = TokenPlan {
        ret: TokenReturn::Owned,
        ..Default::default()
    };
    p.instructions.insert(
        ValueId(2),
        TokenEffect {
            success: vec![ValueId(1)],
            error: vec![ValueId(1)],
            may_fail: true,
            ..Default::default()
        },
    );
    verify_tokens(&f, &c, &t, &p).unwrap();
    assert_eq!(
        tokens_at_landing_pad_entry(&f, &c, &t, &p).unwrap()[&BlockId(1)],
        vec![]
    );
    // An independent owner must remain on error; the call result does not exist.
    f.blocks[0].instructions.insert(1, copy_inst(6, 0));
    f.blocks[1].instructions.push(drop(7, 6));
    f.blocks[2].instructions.push(drop(8, 6));
    let mut c = c;
    c.insert(ValueId(6), Ownership::Owned);
    c.insert(ValueId(7), Ownership::Trivial);
    c.insert(ValueId(8), Ownership::Trivial);
    assert_eq!(
        tokens_at_landing_pad_entry(&f, &c, &t, &p).unwrap()[&BlockId(1)],
        vec![ValueId(6)]
    );
    p.instructions.get_mut(&ValueId(2)).unwrap().error.clear();
    assert!(
        verify_tokens(&f, &c, &t, &p)
            .unwrap_err()
            .contains("não consumidos")
    );
    // Invoke of a callee that does not fail: the landing pad is dead and the
    // exceptional exit is accepted; what is missing is still the consumption.
    p.instructions.get_mut(&ValueId(2)).unwrap().may_fail = false;
    let error = verify_tokens(&f, &c, &t, &p).unwrap_err();
    assert!(!error.contains("saídas excepcionais") && error.contains("não consumidos"), "{error}");
}

#[test]
fn strong_load_creates_token_and_store_moves_single_consumption() {
    let slot = StrongSlot::Quadro {
        frame: value(9),
        index: 0,
    };
    let mut f = function(vec![block(
        0,
        vec![
            (
                ValueId(1),
                Instruction::ArcStoreStrong {
                    slot: slot.clone(),
                    value: value(0),
                    mode: StrongStoreMode::Copy,
                },
                Type::Void,
            ),
            (
                ValueId(2),
                Instruction::ArcLoadStrong { slot: slot.clone() },
                Type::Ref,
            ),
            (
                ValueId(3),
                Instruction::ArcStoreStrong {
                    slot,
                    value: value(2),
                    mode: StrongStoreMode::Move,
                },
                Type::Void,
            ),
        ],
        Terminator::Return(None),
    )]);
    f.params[1].2 = Type::I64;
    let mut c = classes(&[2], &[1, 3]);
    verify(&f, &c).unwrap();
    f.blocks[0].instructions.push(drop(4, 2));
    c.insert(ValueId(4), Ownership::Trivial);
    assert!(verify(&f, &c).unwrap_err().contains("indisponível"));
    f.blocks[0].instructions.pop();
    c.remove(&ValueId(4));
    if let Instruction::ArcStoreStrong { mode, .. } = &mut f.blocks[0].instructions[2].1 {
        *mode = StrongStoreMode::Copy;
    }
    assert!(verify(&f, &c).unwrap_err().contains("não consumidos"));
}

#[test]
fn missing_stale_contracts_and_phi_with_double_consumption_fail() {
    let f = function(vec![block(
        0,
        vec![(ValueId(1), Instruction::Const(Constant::Int(7)), Type::I64)],
        Terminator::Return(None),
    )]);
    let c = classes(&[], &[1]);
    assert!(verify(&f, &c).unwrap_err().contains("sem contrato"));
    let mut p = TokenPlan::default();
    p.instructions.insert(ValueId(1), TokenEffect::default());
    verify_tokens(&f, &c, &FunctionTables::default(), &p).unwrap();
    p.instructions.insert(ValueId(99), TokenEffect::default());
    assert!(
        verify_tokens(&f, &c, &FunctionTables::default(), &p)
            .unwrap_err()
            .contains("obsoleto")
    );
    let f = function(vec![
        block(0, vec![copy_inst(1, 0)], Terminator::Branch(BlockId(1))),
        block(
            1,
            vec![
                (
                    ValueId(2),
                    Instruction::Phi {
                        incoming: vec![(BlockId(0), value(1))],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(3),
                    Instruction::Phi {
                        incoming: vec![(BlockId(0), value(1))],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                drop(4, 2),
                drop(5, 3),
            ],
            Terminator::Return(None),
        ),
    ]);
    assert!(
        verify(&f, &classes(&[1, 2, 3], &[4, 5]))
            .unwrap_err()
            .contains("já consumido")
    );
}
