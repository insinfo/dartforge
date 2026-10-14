use super::*;

#[test]
fn ordinary_late_generic_notifier_dispatch_prepares_without_erasing_field_guards() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let entry = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/arc_ports/late_notifier_dispatch.dart");
        let sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
        let mut interner = dartforge_intern::Interner::new();
        let program = dartforge_elements::load::load(&entry, &sdk, None, &mut interner).unwrap();
        let mut table = dartforge_types::TypeTable::new();
        let core = dartforge_types::CoreTypes::init(&mut table, &program, &interner);
        let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
        let libraries: Vec<_> = program.libraries.iter().enumerate().filter(|(_, l)| !l.is_sdk)
            .map(|(i, _)| dartforge_elements::model::LibraryId(i as u32)).collect();
        let (bodies, _) = dartforge_types::infer_library_bodies_with_locals(
            &program, &interner, &mut table, &core, &mut outline, &libraries,
        );
        let mut context = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies)
            .with_source_sdk();
        context.arc_memory = true;
        let mut module = crate::lower::lower_program(&context);
        assert!(module.errors.is_empty(), "{:?}", module.errors);
        module.arc_memory = true;
        crate::optimize::optimize(&mut module);
        let original = module.functions.iter().find(|f| f.symbol.ends_with("Owner.update")).unwrap();
        assert!(original.blocks.iter().any(|b| matches!(b.terminator, Terminator::Switch { .. })), "fixture must exercise physical dispatch");
        let mut conservative = original.clone();
        assert!(super::super::prepare(&mut conservative, &module.arc_field_layouts,
            &mut DartFunctionPlan::default(), None, &HashSet::new()).is_err(), "fixture must reproduce the mixed-origin layout conflict");
        module.arc_external_callees = true;
        module.arc_external_layouts = crate::lower::sdk_arc_layouts(&context);
        let mut plans = module.functions.iter().map(|f|
            (f.symbol.clone(), DartFunctionPlan::default())).collect();
        crate::optimize::arc::prepare_arc_dart_module(&mut module, &mut plans).unwrap();
        let prepared = module.functions.iter().find(|f| f.symbol.ends_with("Owner.update")).unwrap();
        assert!(prepared.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i,
            Instruction::CallRuntime { name, .. } if name == "dartforge_arc_read_field_scalar_checked_v1")));
    }).unwrap().join().unwrap();
}

fn value(n: u32) -> Operand {
    Operand::Val(ValueId(n))
}
pub(super) fn function() -> Function {
    let block = |n, instructions, terminator| BasicBlock {
        id: BlockId(n),
        instructions,
        terminator,
    };
    Function {
        symbol: "case_access".into(),
        name: "case_access".into(),
        debug_info: None,
        params: vec![
            (ValueId(0), "opaque".into(), Type::Ref),
            (ValueId(9), "condition".into(), Type::I1),
        ],
        return_ty: Type::I64,
        blocks: vec![
            block(
                0,
                vec![(
                    ValueId(1),
                    Instruction::AllocObject {
                        class_id: 123,
                        fields: vec![Operand::Constant(Constant::Null)],
                    },
                    Type::Ref,
                )],
                Terminator::CondBranch {
                    cond: value(9),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
            ),
            block(1, vec![], Terminator::Branch(BlockId(3))),
            block(2, vec![], Terminator::Branch(BlockId(3))),
            block(
                3,
                vec![
                    (
                        ValueId(3),
                        Instruction::Phi {
                            incoming: vec![(BlockId(1), value(1)), (BlockId(2), value(0))],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(4),
                        Instruction::CallRuntime {
                            name: "dartforge_value_class".into(),
                            args: vec![(value(3), Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    ),
                ],
                Terminator::Switch {
                    val: value(4),
                    default: BlockId(5),
                    cases: vec![(124, BlockId(4))],
                },
            ),
            block(
                4,
                vec![(
                    ValueId(5),
                    Instruction::GetField {
                        object: value(3),
                        index: 1,
                    },
                    Type::I64,
                )],
                Terminator::Return(Some(value(5))),
            ),
            block(
                5,
                vec![],
                Terminator::Return(Some(Operand::Constant(Constant::Int(0)))),
            ),
        ],
    }
}

#[test]
fn exact_case_filters_only_allocations_and_preserves_unknown_and_formal_hints() {
    let f = function();
    let cases = Cases::new(&f, true);
    let classes = HashMap::from([(ValueId(1), 123), (ValueId(0), 123)]);
    let mut origin = Origin {
        sites: HashSet::from([ValueId(1), ValueId(0)]),
        unknown: true,
    };
    cases.refine(&mut origin, &value(3), BlockId(4), &classes);
    assert_eq!(origin.sites, HashSet::from([ValueId(0)]));
    assert!(origin.unknown);
    let mut matching = Origin {
        sites: HashSet::from([ValueId(1)]),
        unknown: false,
    };
    cases.refine(
        &mut matching,
        &value(3),
        BlockId(4),
        &HashMap::from([(ValueId(1), 124)]),
    );
    assert_eq!(matching.sites, HashSet::from([ValueId(1)]));
    assert!(!matching.unknown);
}

#[test]
fn unproved_class_queries_and_case_edges_never_filter_origins() {
    for case in 0..12 {
        let mut f = function();
        match case {
            0 => {
                if let Instruction::CallRuntime { name, .. } = &mut f.blocks[3].instructions[1].1 {
                    *name = "opaque_class".into();
                }
            }
            1 => {
                if let Instruction::CallRuntime { args, .. } = &mut f.blocks[3].instructions[1].1 {
                    args[0].0 = value(0);
                }
            }
            2 => {
                if let Instruction::CallRuntime { args, .. } = &mut f.blocks[3].instructions[1].1 {
                    args[0].1 = Type::I64;
                }
            }
            3 => {
                if let Instruction::CallRuntime { ret_ty, .. } = &mut f.blocks[3].instructions[1].1
                {
                    *ret_ty = Type::Ref;
                }
            }
            4 => f.blocks[3].instructions[1].2 = Type::Ref,
            5 => {
                if let Terminator::Switch { default, .. } = &mut f.blocks[3].terminator {
                    *default = BlockId(4);
                }
            }
            6 => {
                if let Terminator::Switch { cases, .. } = &mut f.blocks[3].terminator {
                    cases.push((125, BlockId(4)));
                }
            }
            7 => f.blocks.push(BasicBlock {
                id: BlockId(6),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(4)),
            }),
            8 => {
                if let Terminator::Switch { cases, .. } = &mut f.blocks[3].terminator {
                    cases[0].0 = -1;
                }
            }
            9 => f.blocks[3].instructions[0].2 = Type::I64,
            10 => {
                f.blocks[3].instructions[1].1 = Instruction::CallStatic {
                    symbol: "dartforge_value_class".into(),
                    args: vec![value(3)],
                    ret_ty: Type::I64,
                }
            }
            11 => f.blocks[3].instructions[1].0 = ValueId(1),
            _ => unreachable!(),
        }
        let mut origin = Origin {
            sites: HashSet::from([ValueId(1)]),
            unknown: true,
        };
        Cases::new(&f, true).refine(
            &mut origin,
            &value(3),
            BlockId(4),
            &HashMap::from([(ValueId(1), 123)]),
        );
        assert_eq!(origin.sites, HashSet::from([ValueId(1)]), "case {case}");
    }
    let f = function();
    let mut origin = Origin {
        sites: HashSet::from([ValueId(1)]),
        unknown: true,
    };
    Cases::new(&f, false).refine(
        &mut origin,
        &value(3),
        BlockId(4),
        &HashMap::from([(ValueId(1), 123)]),
    );
    assert_eq!(origin.sites, HashSet::from([ValueId(1)]));
}

#[test]
fn refinement_does_not_cross_a_join_or_change_other_receiver_aliases() {
    let mut f = function();
    f.blocks[4].terminator = Terminator::Branch(BlockId(6));
    f.blocks[5].terminator = Terminator::Branch(BlockId(6));
    f.blocks.push(BasicBlock {
        id: BlockId(6),
        instructions: vec![],
        terminator: Terminator::Return(Some(Operand::Constant(Constant::Int(0)))),
    });
    for (receiver, block) in [(value(3), BlockId(6)), (value(0), BlockId(4))] {
        let mut origin = Origin {
            sites: HashSet::from([ValueId(1)]),
            unknown: true,
        };
        Cases::new(&f, true).refine(
            &mut origin,
            &receiver,
            block,
            &HashMap::from([(ValueId(1), 123)]),
        );
        assert_eq!(origin.sites, HashSet::from([ValueId(1)]));
    }
}

#[test]
fn malformed_cfg_and_non_dominating_query_or_receiver_never_refine() {
    for case in 0..6 {
        let mut f = function();
        match case {
            0 => f.blocks.push(f.blocks[4].clone()),
            1 => {
                if let Terminator::Switch { cases, .. } = &mut f.blocks[3].terminator {
                    cases[0].1 = BlockId(999);
                }
            }
            2 => f.blocks[5].terminator = Terminator::Branch(BlockId(0)),
            3 => {
                let query = f.blocks[3].instructions.pop().unwrap();
                f.blocks[3].instructions.clear();
                f.blocks[0].instructions.push((
                    ValueId(3),
                    Instruction::ArcCopy { value: value(0) },
                    Type::Ref,
                ));
                f.blocks[1].instructions.push(query);
                let cfg = Cfg::new(&f);
                assert!(!dominates(&cfg.idom, 1, 3));
                assert!(dominates(&cfg.idom, 0, 1));
            }
            4 => f.blocks[3].instructions.swap(0, 1),
            5 => {
                f.blocks[3].instructions.remove(0);
                f.blocks[1].instructions.push((
                    ValueId(3),
                    Instruction::ArcCopy { value: value(0) },
                    Type::Ref,
                ));
                let cfg = Cfg::new(&f);
                assert!(dominates(&cfg.idom, 3, 3));
                assert!(!dominates(&cfg.idom, 1, 3));
            }
            _ => unreachable!(),
        }
        let mut origin = Origin {
            sites: HashSet::from([ValueId(1)]),
            unknown: true,
        };
        Cases::new(&f, true).refine(
            &mut origin,
            &value(3),
            BlockId(4),
            &HashMap::from([(ValueId(1), 123)]),
        );
        assert_eq!(origin.sites, HashSet::from([ValueId(1)]), "case {case}");
        assert!(origin.unknown);
    }
}

#[test]
fn field_preparation_keeps_checked_layouts_and_rejects_remaining_contradictions() {
    let layouts = HashMap::from([(123, vec![Type::Ref]), (124, vec![Type::Ref, Type::I64])]);
    let mut f = function();
    assert!(
        super::super::prepare(
            &mut f.clone(),
            &layouts,
            &mut DartFunctionPlan::default(),
            None,
            &HashSet::new()
        )
        .is_err()
    );
    super::super::prepare_with_class_cases(
        &mut f,
        &layouts,
        &mut DartFunctionPlan::default(),
        None,
        &HashSet::new(),
        true,
    )
    .unwrap();
    assert!(f.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i,
        Instruction::CallRuntime { name, .. } if name == "dartforge_arc_read_field_scalar_checked_v1")));
    assert!(
        f.blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .any(|(_, i, _)| matches!(i,
        Instruction::ConstArray(fields) if fields == &[124, 2]))
    );
    let mut nominal = function();
    assert!(
        super::super::prepare_with_class_cases(
            &mut nominal,
            &layouts,
            &mut DartFunctionPlan::default(),
            Some(&HashMap::from([(ValueId(0), 123)])),
            &HashSet::new(),
            true
        )
        .is_err()
    );
    let mut matching_bad_layout = function();
    if let Instruction::AllocObject { class_id, .. } =
        &mut matching_bad_layout.blocks[0].instructions[0].1
    {
        *class_id = 124;
    }
    let wrong = HashMap::from([(124, vec![Type::Ref])]);
    assert!(
        super::super::prepare_with_class_cases(
            &mut matching_bad_layout,
            &wrong,
            &mut DartFunctionPlan::default(),
            None,
            &HashSet::new(),
            true
        )
        .is_err()
    );
}
