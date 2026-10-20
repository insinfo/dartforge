use super::*;

fn old_liveness_with_exits(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tables: &FunctionTables,
    pendings: &HashMap<ValueId, BlockId>,
) -> Result<Liveness, String> {
    let v = classified_liveness(f, classes)?;
    let cfg = Cfg::new(f);
    let pos: HashMap<BlockId, usize> = f
        .blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect();
    let defs: HashMap<ValueId, usize> = f
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(i, b)| b.instructions.iter().map(move |(v, _, _)| (*v, i)))
        .collect();
    if pendings.keys().any(|v| tables.invocations.contains_key(v)) {
        return Err(format!(
            "ownership in {}: pending exit duplicated as invoke",
            f.symbol
        ));
    }
    let mut calls: Vec<_> = tables.invocations.iter().chain(pendings).collect();
    calls.sort_by_key(|(v, _)| v.0);
    for (&call, &landing_pad) in calls {
        let Some(&origin) = defs.get(&call) else {
            return Err(format!(
                "ownership in {}: missing invoke v{}",
                f.symbol, call.0
            ));
        };
        let b = &f.blocks[origin];
        if tables.invocations.contains_key(&call)
            && b.instructions
                .iter()
                .any(|(v, inst, _)| *v == call && matches!(inst, Instruction::CallRuntime { .. }))
        {
            return Err(format!(
                "ownership in {}: runtime v{} requires a pending check, not an LLVM invoke",
                f.symbol, call.0
            ));
        }
        let success = if pendings.contains_key(&call) {
            check_pending(b, call, landing_pad).and_then(|s| pos.get(&s).copied())
        } else {
            match &b.terminator {
                Terminator::CondBranch {
                    cond: Operand::Constant(Constant::Bool(false)),
                    then_block,
                    else_block,
                } if *then_block == landing_pad
                    && *else_block != landing_pad
                    && b.instructions.last().is_some_and(|(v, _, _)| *v == call)
                    && tables.landing_pads.contains(&landing_pad) =>
                {
                    pos.get(else_block).copied()
                }
                _ => None,
            }
        }
        .ok_or_else(|| {
            format!(
                "ownership in {}: invoke v{} without a valid success edge",
                f.symbol, call.0
            )
        })?;
        // Removing the edge distinguishes block dominance from result dominance.
        let without_success = old_reachable_without_edge(&cfg, (origin, success));
        let mut aliases: Vec<_> = classes.iter().filter_map(|(&alias,class)| {
            matches!(class, Ownership::Borrowed { owner: OwnerOrigin::Value(owner), .. } if *owner == call)
                .then_some(alias)
        }).collect();
        aliases.sort_by_key(|v| v.0);
        for alias in aliases {
            if let Some(&block) = defs.get(&alias)
                && cfg.reachable(block)
                && without_success.contains(&block)
            {
                return Err(format!(
                    "ARC003 in {}: b{}: v{} depends on result v{} unavailable on the exceptional path; path [{}, {}]",
                    f.symbol, f.blocks[block].id.0, alias.0, call.0, alias.0, call.0
                ));
            }
        }
        old_check_result_uses(f, &cfg, &pos, call, (origin, success), &without_success)?;
    }
    Ok(v)
}

fn old_check_result_uses(
    f: &Function,
    cfg: &Cfg,
    pos: &HashMap<BlockId, usize>,
    call: ValueId,
    success: (usize, usize),
    without_success: &HashSet<usize>,
) -> Result<(), String> {
    let uses = |o: &Operand| matches!(o, Operand::Val(v) if *v == call);
    for (i, b) in f.blocks.iter().enumerate() {
        if !cfg.reachable(i) {
            continue;
        }
        let mut invalid = false;
        for (_, inst, _) in &b.instructions {
            if let Instruction::Phi { incoming, .. } = inst {
                for (of, op) in incoming {
                    let Some(&p) = pos.get(of) else { continue };
                    // The success edge publishes the result; all the
                    // others must be dominated by that publication.
                    if uses(op)
                        && cfg.reachable(p)
                        && without_success.contains(&p)
                        && (p, i) != success
                    {
                        invalid = true;
                    }
                }
            } else if without_success.contains(&i) {
                crate::optimize::operands::operands(inst, &mut |o| invalid |= uses(o));
            }
        }
        if without_success.contains(&i) {
            crate::optimize::operands::terminator_operands(&b.terminator, &mut |o| {
                invalid |= uses(o)
            });
        }
        if invalid {
            return Err(format!(
                "ARC003 in {}: b{}: result v{} used without crossing its success edge",
                f.symbol, b.id.0, call.0
            ));
        }
    }
    Ok(())
}

fn old_reachable_without_edge(cfg: &Cfg, removed: (usize, usize)) -> HashSet<usize> {
    let mut seen_set = HashSet::new();
    let mut queue = if cfg.successors.is_empty() {
        vec![]
    } else {
        vec![0]
    };
    while let Some(b) = queue.pop() {
        if seen_set.insert(b) {
            queue.extend(
                cfg.successors[b]
                    .iter()
                    .copied()
                    .filter(|&s| (b, s) != removed),
            );
        }
    }
    seen_set
}

fn fixture(
    invoke: bool,
) -> (
    Function,
    HashMap<ValueId, Ownership>,
    FunctionTables,
    HashMap<ValueId, BlockId>,
) {
    let mut instructions = vec![(
        ValueId(1),
        Instruction::CallStatic {
            symbol: "source".into(),
            args: vec![],
            ret_ty: Type::Ref,
        },
        Type::Ref,
    )];
    let cond = if invoke {
        Operand::Constant(Constant::Bool(false))
    } else {
        instructions.extend([
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
        ]);
        Operand::Val(ValueId(3))
    };
    let f = Function {
        symbol: "availability".into(),
        name: "availability".into(),
        debug_info: None,
        params: vec![(ValueId(0), "caller".into(), Type::Ref)],
        return_ty: Type::Ref,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions,
                terminator: Terminator::CondBranch {
                    cond,
                    then_block: BlockId(2),
                    else_block: BlockId(1),
                },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
            },
        ],
    };
    let mut classes = HashMap::from([
        (
            ValueId(0),
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0,
            },
        ),
        (ValueId(1), Ownership::Owned),
    ]);
    if !invoke {
        classes.extend([
            (ValueId(2), Ownership::Trivial),
            (ValueId(3), Ownership::Trivial),
        ]);
    }
    let mut tables = FunctionTables::default();
    let mut pending = HashMap::new();
    if invoke {
        tables.invocations.insert(ValueId(1), BlockId(2));
        tables.landing_pads.insert(BlockId(2));
    } else {
        pending.insert(ValueId(1), BlockId(2));
    }
    (f, classes, tables, pending)
}

fn assert_same(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tables: &FunctionTables,
    pending: &HashMap<ValueId, BlockId>,
) {
    match (
        old_liveness_with_exits(f, classes, tables, pending),
        liveness_with_exits(f, classes, tables, pending),
    ) {
        (Ok(expected), Ok(actual)) => {
            assert_eq!(expected.entry, actual.entry);
            assert_eq!(expected.exit, actual.exit);
            assert_eq!(expected.before, actual.before);
            assert_eq!(expected.after, actual.after);
            assert_eq!(expected.edges, actual.edges);
        }
        (Err(expected), Err(actual)) => assert_eq!(expected, actual),
        (expected, actual) => panic!("oracle mismatch: {expected:?} / {actual:?}"),
    }
}

#[test]
fn indexed_result_availability_matches_original_for_uses_aliases_and_shapes() {
    for invoke in [false, true] {
        for case in 0..16 {
            let (mut f, mut classes, mut tables, mut pending) = fixture(invoke);
            match case {
                0 => {}
                1 => f.blocks[2].terminator = Terminator::Return(Some(Operand::Val(ValueId(1)))),
                2 => f.blocks[1].terminator = Terminator::Return(Some(Operand::Val(ValueId(1)))),
                3 | 4 => {
                    f.blocks[if case == 3 { 2 } else { 1 }].instructions.push((
                        ValueId(4),
                        Instruction::Bitcast {
                            op: Operand::Val(ValueId(1)),
                            to: Type::Ref,
                        },
                        Type::Ref,
                    ));
                    classes.insert(
                        ValueId(4),
                        Ownership::Borrowed {
                            owner: OwnerOrigin::Value(ValueId(1)),
                            scope: 0,
                        },
                    );
                }
                5 => {
                    f.blocks[1].instructions.push((
                        ValueId(4),
                        Instruction::Bitcast {
                            op: Operand::Val(ValueId(1)),
                            to: Type::Ref,
                        },
                        Type::Ref,
                    ));
                    f.blocks[2].instructions.push((
                        ValueId(5),
                        Instruction::Bitcast {
                            op: Operand::Val(ValueId(4)),
                            to: Type::Ref,
                        },
                        Type::Ref,
                    ));
                    classes.insert(
                        ValueId(4),
                        Ownership::Borrowed {
                            owner: OwnerOrigin::Value(ValueId(1)),
                            scope: 0,
                        },
                    );
                    classes.insert(
                        ValueId(5),
                        Ownership::Borrowed {
                            owner: OwnerOrigin::Value(ValueId(4)),
                            scope: 0,
                        },
                    );
                }
                6 | 7 => {
                    f.blocks[1].terminator = Terminator::Branch(BlockId(3));
                    f.blocks[2].terminator = Terminator::Branch(BlockId(3));
                    f.blocks.push(BasicBlock {
                        id: BlockId(3),
                        instructions: vec![(
                            ValueId(4),
                            Instruction::Phi {
                                incoming: vec![
                                    (BlockId(1), Operand::Val(ValueId(1))),
                                    (
                                        BlockId(2),
                                        if case == 6 {
                                            Operand::Constant(Constant::Null)
                                        } else {
                                            Operand::Val(ValueId(1))
                                        },
                                    ),
                                ],
                                ty: Type::Ref,
                            },
                            Type::Ref,
                        )],
                        terminator: Terminator::Return(Some(Operand::Val(ValueId(4)))),
                    });
                    classes.insert(ValueId(4), Ownership::Owned);
                }
                8 => {
                    // A direct Phi use on the publication edge itself is valid.
                    f.blocks[1].instructions.push((
                        ValueId(4),
                        Instruction::Phi {
                            incoming: vec![(BlockId(0), Operand::Val(ValueId(1)))],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    ));
                    classes.insert(ValueId(4), Ownership::Owned);
                }
                9 => {
                    f.blocks.push(BasicBlock {
                        id: BlockId(30),
                        instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                    });
                }
                10 => {
                    // Sparse IDs and vector order differ from numerical order.
                    f.blocks[1].id = BlockId(700);
                    f.blocks[2].id = BlockId(20);
                    if let Terminator::CondBranch {
                        then_block,
                        else_block,
                        ..
                    } = &mut f.blocks[0].terminator
                    {
                        *then_block = BlockId(20);
                        *else_block = BlockId(700);
                    }
                    if invoke {
                        tables.invocations.insert(ValueId(1), BlockId(20));
                        tables.landing_pads = HashSet::from([BlockId(20)]);
                    } else {
                        pending.insert(ValueId(1), BlockId(20));
                    }
                    f.blocks.swap(1, 2);
                }
                11 => {
                    if let Terminator::CondBranch { else_block, .. } = &mut f.blocks[0].terminator {
                        *else_block = BlockId(2);
                    }
                }
                12 => {
                    if invoke {
                        tables.landing_pads.clear();
                    } else if let Instruction::CallRuntime { ret_ty, .. } =
                        &mut f.blocks[0].instructions[1].1
                    {
                        *ret_ty = Type::I64;
                    }
                }
                13 => {
                    pending.insert(ValueId(99), BlockId(2));
                }
                14 => {
                    tables.invocations.insert(ValueId(1), BlockId(2));
                    pending.insert(ValueId(1), BlockId(2));
                }
                15 => {
                    f.blocks[0].instructions[0].1 = Instruction::CallRuntime {
                        name: "fallible".into(),
                        args: vec![],
                        ret_ty: Type::Ref,
                    };
                }
                _ => unreachable!(),
            }
            assert_same(&f, &classes, &tables, &pending);
        }
    }
}

#[test]
fn dense_removed_edge_reachability_matches_hash_oracle_on_small_graphs() {
    for n in 0..=3 {
        for bits in 0..(1u32 << (n * n)) {
            let successors = (0..n)
                .map(|from| {
                    (0..n)
                        .filter(|&to| bits & (1 << (from * n + to)) != 0)
                        .collect()
                })
                .collect();
            let cfg = Cfg {
                successors,
                predecessors: vec![],
                rpo: vec![],
                idom: vec![],
            };
            for from in 0..=n {
                for to in 0..=n {
                    assert_eq!(
                        reachable_without_edge(&cfg, (from, to)),
                        old_reachable_without_edge(&cfg, (from, to))
                    );
                }
            }
        }
    }
}

#[test]
fn unused_results_still_validate_shapes_and_exceptional_token_consumption() {
    let (f, classes, tables, pending) = fixture(false);
    let uses = ResultUses::new(
        &f,
        &classes,
        &HashMap::from([(BlockId(0), 0), (BlockId(1), 1), (BlockId(2), 2)]),
        &[(&ValueId(1), &BlockId(2))],
    );
    assert!(uses.aliases.is_empty() && uses.direct.is_empty());
    let mut broken = f.clone();
    broken.blocks[0].instructions.pop();
    assert!(liveness_with_exits(&broken, &classes, &tables, &pending).is_err());
    let mut plan = super::super::TokenPlan {
        pendings: pending,
        ..Default::default()
    };
    plan.instructions.insert(
        ValueId(1),
        super::super::TokenEffect {
            always: vec![],
            success: vec![],
            error: vec![ValueId(1)],
            may_fail: true,
        },
    );
    assert!(super::super::verify_tokens(&f, &classes, &tables, &plan).is_err());
}

#[test]
fn indexed_checks_keep_call_id_and_alias_before_direct_use_error_order() {
    let (mut f, mut classes, tables, mut pending) = fixture(false);
    // Lower call ID occurs second in execution and must still be checked first.
    f.blocks[0].instructions[0].0 = ValueId(30);
    classes.remove(&ValueId(1));
    classes.insert(ValueId(30), Ownership::Owned);
    pending.remove(&ValueId(1));
    pending.insert(ValueId(30), BlockId(2));
    f.blocks[2].terminator = Terminator::Return(Some(Operand::Val(ValueId(30))));
    f.blocks[1].instructions = vec![
        (
            ValueId(5),
            Instruction::CallStatic {
                symbol: "source".into(),
                args: vec![],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ),
        (
            ValueId(12),
            Instruction::CallRuntime {
                name: "dartforge_exception_pending".into(),
                args: vec![],
                ret_ty: Type::I8,
            },
            Type::I8,
        ),
        (
            ValueId(13),
            Instruction::ICmp(
                ICmpOp::Ne,
                Operand::Val(ValueId(12)),
                Operand::Constant(Constant::Int(0)),
            ),
            Type::I1,
        ),
    ];
    f.blocks[1].terminator = Terminator::CondBranch {
        cond: Operand::Val(ValueId(13)),
        then_block: BlockId(4),
        else_block: BlockId(3),
    };
    f.blocks.push(BasicBlock {
        id: BlockId(3),
        instructions: vec![],
        terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
    });
    f.blocks.push(BasicBlock {
        id: BlockId(4),
        instructions: vec![],
        terminator: Terminator::Return(Some(Operand::Val(ValueId(5)))),
    });
    classes.extend([
        (ValueId(5), Ownership::Owned),
        (ValueId(12), Ownership::Trivial),
        (ValueId(13), Ownership::Trivial),
    ]);
    pending.insert(ValueId(5), BlockId(4));
    assert_same(&f, &classes, &tables, &pending);
    assert!(
        liveness_with_exits(&f, &classes, &tables, &pending)
            .unwrap_err()
            .contains("result v5 used")
    );
    // Alias IDs are checked in ascending order, before any direct result use.
    for alias in [9, 7] {
        f.blocks[4].instructions.push((
            ValueId(alias),
            Instruction::Bitcast {
                op: Operand::Val(ValueId(5)),
                to: Type::Ref,
            },
            Type::Ref,
        ));
        classes.insert(
            ValueId(alias),
            Ownership::Borrowed {
                owner: OwnerOrigin::Value(ValueId(5)),
                scope: 0,
            },
        );
    }
    assert_same(&f, &classes, &tables, &pending);
    assert!(
        liveness_with_exits(&f, &classes, &tables, &pending)
            .unwrap_err()
            .contains("v7 depends on result v5")
    );
}

#[test]
fn no_exit_metadata_does_not_skip_complete_classification() {
    let (f, mut classes, tables, _) = fixture(false);
    let empty = HashMap::new();
    assert_same(&f, &classes, &tables, &empty);
    classes.remove(&ValueId(1));
    assert_same(&f, &classes, &tables, &empty);
    assert!(
        liveness_with_exits(&f, &classes, &tables, &empty)
            .unwrap_err()
            .contains("without classification")
    );
}
