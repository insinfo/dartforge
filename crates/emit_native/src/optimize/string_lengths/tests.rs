//! Semantic checks with an independent bounded HIR interpreter and CFG controls.
use super::*;

fn value(id: u32) -> Operand {
    Operand::Val(ValueId(id))
}
fn int(n: i64) -> Operand {
    Operand::Constant(Constant::Int(n))
}
fn length(id: u32, string: u32) -> (ValueId, Instruction, Type) {
    (
        ValueId(id),
        Instruction::CallRuntime {
            name: "df.string_len".into(),
            args: vec![(value(string), Type::Ref)],
            ret_ty: Type::I64,
        },
        Type::I64,
    )
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
        symbol: "test_lengths".into(),
        name: "test_lengths".into(),
        params: vec![
            (ValueId(0), "string".into(), Type::Ref),
            (ValueId(1), "other".into(), Type::Ref),
            (ValueId(2), "flag".into(), Type::I1),
        ],
        return_ty: Type::I64,
        blocks,
        debug_info: None,
    }
}
fn loop_function() -> Function {
    function(vec![
        block(0, vec![], Terminator::Branch(BlockId(1))),
        block(
            1,
            vec![
                (
                    ValueId(3),
                    Instruction::Phi {
                        incoming: vec![(BlockId(0), int(0)), (BlockId(2), value(6))],
                        ty: Type::I64,
                    },
                    Type::I64,
                ),
                length(4, 0),
                (
                    ValueId(5),
                    Instruction::ICmp(ICmpOp::Slt, value(3), value(4)),
                    Type::I1,
                ),
            ],
            Terminator::CondBranch {
                cond: value(5),
                then_block: BlockId(2),
                else_block: BlockId(3),
            },
        ),
        block(
            2,
            vec![
                (ValueId(6), Instruction::Add(value(3), int(1)), Type::I64),
                length(7, 0),
                (
                    ValueId(8),
                    Instruction::CallRuntime {
                        name: "opaque".into(),
                        args: vec![(value(7), Type::I64)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
            ],
            Terminator::Branch(BlockId(1)),
        ),
        block(3, vec![], Terminator::Return(Some(value(3)))),
    ])
}

/// Evaluates simultaneous Phi inputs, observable calls and string-length reads.
/// Null is deliberately absent from lengths, so speculation onto a skipped
/// nullable path fails independently of the transformation's shape assertions.
fn execute(function: &Function, strings: [i64; 2], flag: bool) -> (i64, Vec<i64>, usize) {
    let lengths = HashMap::from([(11, strings[0]), (22, strings[1])]);
    let mut values = HashMap::from([
        (ValueId(0), if strings[0] < 0 { 0 } else { 11 }),
        (ValueId(1), 22),
        (ValueId(2), i64::from(flag)),
    ]);
    let read = |op: &Operand, values: &HashMap<ValueId, i64>| match op {
        Operand::Val(id) => values[id],
        Operand::Constant(Constant::Int(n)) => *n,
        _ => panic!("unsupported test operand {op:?}"),
    };
    let mut current = BlockId(0);
    let mut previous = BlockId(u32::MAX);
    let mut calls = Vec::new();
    let mut reads = 0;
    for _ in 0..1000 {
        let block = function.blocks.iter().find(|b| b.id == current).unwrap();
        let phis: Vec<_> = block
            .instructions
            .iter()
            .filter_map(|(id, instruction, _)| {
                if let Instruction::Phi { incoming, .. } = instruction {
                    Some((
                        *id,
                        read(
                            &incoming.iter().find(|(p, _)| *p == previous).unwrap().1,
                            &values,
                        ),
                    ))
                } else {
                    None
                }
            })
            .collect();
        values.extend(phis);
        for (id, instruction, _) in &block.instructions {
            let result = match instruction {
                Instruction::Phi { .. } => continue,
                Instruction::Const(op) => read(&Operand::Constant(op.clone()), &values),
                Instruction::Add(a, b) => read(a, &values) + read(b, &values),
                Instruction::ICmp(ICmpOp::Slt, a, b) => {
                    i64::from(read(a, &values) < read(b, &values))
                }
                Instruction::CallRuntime { name, args, .. } if name == "df.string_len" => {
                    reads += 1;
                    lengths[&read(&args[0].0, &values)]
                }
                Instruction::CallRuntime { name, args, .. } if name == "opaque" => {
                    calls.push(read(&args[0].0, &values));
                    0
                }
                _ => panic!("unsupported test instruction {instruction:?}"),
            };
            values.insert(*id, result);
        }
        previous = current;
        current = match &block.terminator {
            Terminator::Branch(to) => *to,
            Terminator::CondBranch {
                cond,
                then_block,
                else_block,
            } => {
                if read(cond, &values) != 0 {
                    *then_block
                } else {
                    *else_block
                }
            }
            Terminator::Return(Some(op)) => return (read(op, &values), calls, reads),
            other => panic!("unsupported test terminator {other:?}"),
        };
    }
    panic!("test interpreter exceeded step budget")
}

#[test]
fn loop_prefix_is_hoisted_and_dominating_reads_are_reused() {
    let before = loop_function();
    let mut after = before.clone();
    assert_eq!(rewrite_function(&mut after), 2);
    assert!(
        matches!(after.blocks[0].instructions.as_slice(), [(_, Instruction::CallRuntime { name, .. }, _)] if name == "df.string_len")
    );
    assert!(crate::pruning::receiver_provenance::has_structural_ssa_shape(&after));
    for len in 0..40 {
        let original = execute(&before, [len, 100], true);
        let optimized = execute(&after, [len, 100], true);
        assert_eq!((original.0, original.1), (optimized.0, optimized.1));
        assert_eq!(original.2, 2 * len as usize + 1);
        assert_eq!(optimized.2, 1);
    }
    assert_eq!(rewrite_function(&mut after), 0);
}

#[test]
fn loop_carried_string_identity_is_never_hoisted() {
    let mut before = loop_function();
    before.blocks[1].instructions.insert(
        1,
        (
            ValueId(9),
            Instruction::Phi {
                incoming: vec![(BlockId(0), value(0)), (BlockId(2), value(1))],
                ty: Type::Ref,
            },
            Type::Ref,
        ),
    );
    before.blocks[1].instructions[2] = length(4, 9);
    before.blocks[2].instructions[1] = length(7, 9);
    let mut after = before.clone();
    assert_eq!(rewrite_function(&mut after), 1);
    assert!(after.blocks[0].instructions.is_empty());
    for a in 0..8 {
        for b in 0..8 {
            let original = execute(&before, [a, b], true);
            let optimized = execute(&after, [a, b], true);
            assert_eq!((original.0, original.1), (optimized.0, optimized.1));
        }
    }
}

#[test]
fn conditional_entry_and_effectful_prefix_do_not_speculate() {
    let mut conditional = loop_function();
    conditional.blocks[0].terminator = Terminator::CondBranch {
        cond: value(2),
        then_block: BlockId(1),
        else_block: BlockId(4),
    };
    conditional
        .blocks
        .push(block(4, vec![], Terminator::Return(Some(int(42)))));
    let original = conditional.clone();
    assert_eq!(rewrite_function(&mut conditional), 1);
    assert!(conditional.blocks[0].instructions.is_empty());
    assert_eq!(execute(&conditional, [-1, 9], false), (42, vec![], 0));
    let old = execute(&original, [5, 9], true);
    let new = execute(&conditional, [5, 9], true);
    assert_eq!((old.0, old.1), (new.0, new.1));

    let mut effectful = loop_function();
    effectful.blocks[1].instructions.insert(
        1,
        (
            ValueId(10),
            Instruction::CallRuntime {
                name: "opaque".into(),
                args: vec![(int(123), Type::I64)],
                ret_ty: Type::Void,
            },
            Type::Void,
        ),
    );
    let original = effectful.clone();
    assert_eq!(rewrite_function(&mut effectful), 1);
    assert!(effectful.blocks[0].instructions.is_empty());
    let old = execute(&original, [5, 9], true);
    let new = execute(&effectful, [5, 9], true);
    assert_eq!((old.0, old.1), (new.0, new.1));
}

#[test]
fn siblings_and_join_cannot_reuse_non_dominating_reads() {
    let mut f = function(vec![
        block(
            0,
            vec![],
            Terminator::CondBranch {
                cond: value(2),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        block(1, vec![length(3, 0)], Terminator::Branch(BlockId(3))),
        block(2, vec![length(4, 0)], Terminator::Branch(BlockId(3))),
        block(3, vec![length(5, 0)], Terminator::Return(Some(value(5)))),
    ]);
    assert_eq!(rewrite_function(&mut f), 0);
    f.blocks[0].instructions.push(length(6, 0));
    let before = f.clone();
    assert_eq!(rewrite_function(&mut f), 3);
    assert!(crate::pruning::receiver_provenance::has_structural_ssa_shape(&f));
    for flag in [false, true] {
        let old = execute(&before, [17, 5], flag);
        let new = execute(&f, [17, 5], flag);
        assert_eq!((old.0, old.1), (new.0, new.1));
        assert_eq!(new.2, 1);
    }
}

#[test]
fn replacements_preserve_phi_edge_uses() {
    let mut f = function(vec![
        block(
            0,
            vec![length(3, 0)],
            Terminator::CondBranch {
                cond: value(2),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        block(1, vec![length(4, 0)], Terminator::Branch(BlockId(3))),
        block(2, vec![length(5, 0)], Terminator::Branch(BlockId(3))),
        block(
            3,
            vec![(
                ValueId(6),
                Instruction::Phi {
                    incoming: vec![(BlockId(1), value(4)), (BlockId(2), value(5))],
                    ty: Type::I64,
                },
                Type::I64,
            )],
            Terminator::Return(Some(value(6))),
        ),
    ]);
    let original = f.clone();
    assert_eq!(rewrite_function(&mut f), 2);
    assert!(crate::pruning::receiver_provenance::has_structural_ssa_shape(&f));
    for flag in [false, true] {
        assert_eq!(
            execute(&original, [31, 5], flag).0,
            execute(&f, [31, 5], flag).0
        );
    }
}

#[test]
fn malformed_unreachable_definitions_and_over_budget_bodies_are_unchanged() {
    let mut unreachable = loop_function();
    unreachable.blocks.push(block(
        4,
        vec![length(4, 0)],
        Terminator::Return(Some(int(0))),
    ));
    let mut budget = loop_function();
    for n in 4..513 {
        budget
            .blocks
            .push(block(n, vec![], Terminator::Return(Some(int(0)))));
    }
    for mut f in [unreachable, budget] {
        let original = format!("{f:?}");
        assert_eq!(rewrite_function(&mut f), 0);
        assert_eq!(format!("{f:?}"), original);
    }
}

#[test]
fn malformed_shapes_and_helper_abis_are_unchanged() {
    let original = loop_function();
    let mut cases = Vec::new();
    let mut f = original.clone();
    f.params[0].2 = Type::I64;
    cases.push(f);
    let mut f = original.clone();
    f.blocks[0].terminator = Terminator::Branch(BlockId(99));
    cases.push(f);
    let mut f = original.clone();
    f.blocks[2].instructions[1].0 = ValueId(4);
    cases.push(f);
    let mut f = original.clone();
    f.debug_info = Some(Box::default());
    cases.push(f);
    let mut f = original.clone();
    f.blocks[1].instructions[1].2 = Type::Ref;
    cases.push(f);
    let mut f = original.clone();
    for block in &mut f.blocks {
        for (_, instruction, _) in &mut block.instructions {
            if let Instruction::CallRuntime { name, ret_ty, .. } = instruction
                && name == "df.string_len"
            {
                *ret_ty = Type::Ref;
            }
        }
    }
    cases.push(f);
    for mut f in cases {
        let before = format!("{f:?}");
        assert_eq!(rewrite_function(&mut f), 0);
        assert_eq!(format!("{f:?}"), before);
    }
}

#[test]
fn constants_distinct_references_and_other_helpers_remain_separate() {
    let mut f = function(vec![block(
        0,
        vec![
            length(3, 0),
            length(4, 1),
            (
                ValueId(5),
                Instruction::CallRuntime {
                    name: "df.string_len".into(),
                    args: vec![(Operand::Constant(Constant::String("abc".into())), Type::Ref)],
                    ret_ty: Type::I64,
                },
                Type::I64,
            ),
        ],
        Terminator::Return(Some(value(4))),
    )]);
    assert_eq!(rewrite_function(&mut f), 0);
    for (_, instruction, _) in &mut f.blocks[0].instructions {
        if let Instruction::CallRuntime { name, .. } = instruction {
            *name = "df.string_hash".into();
        }
    }
    assert_eq!(rewrite_function(&mut f), 0);
    let mut module = Module::new();
    module.functions.push(loop_function());
    module.arc_prepared = true;
    assert_eq!(rewrite(&mut module), 0);
    module.arc_prepared = false;
    module.functions[0].symbol = "df.string_len".into();
    assert_eq!(rewrite(&mut module), 0);
}
