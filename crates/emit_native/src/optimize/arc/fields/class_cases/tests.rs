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
        let owner_reads: Vec<_> = original.blocks.iter().flat_map(|b| &b.instructions)
            .filter_map(|(value, instruction, ty)| match instruction {
                Instruction::CallRuntime { name, args, ret_ty: Type::Ref }
                    if name == "dartforge_arc_read_field_late_ref_v1" && *ty == Type::Ref => Some((*value, args.clone())),
                _ => None,
            }).collect();
        assert!(!owner_reads.is_empty(), "the late notifier already uses the ARC late-field protocol");
        // Follow actual reads in either body; do not depend on whether notify
        // was inlined into the dispatcher by an earlier optimization.
        let scalar_reads: Vec<_> = module.functions.iter().filter(|f|
            f.symbol.ends_with("Owner.update") || f.symbol.ends_with("Derived.notify"))
            .flat_map(|f| f.blocks.iter().flat_map(|b| &b.instructions).filter_map(|(value, instruction, ty)| match instruction {
                Instruction::GetField { object, index } if *ty == Type::I64 => Some((f.symbol.clone(), *value, object.clone(), *index)),
                _ => None,
            })).collect();
        assert!(!scalar_reads.is_empty(), "fixture must exercise the actual holds field");
        let derived = program.classes.iter().enumerate().find(|(_, class)|
            !program.library(class.library).is_sdk && context.symbol_name(class.name) == "Derived").unwrap();
        let class_id = context.class_id_of(dartforge_elements::model::ClassId(derived.0 as u32)).unwrap();
        let layout = &module.arc_field_layouts[&class_id];
        assert!(scalar_reads.iter().all(|(_, _, _, index)| layout.get(*index) == Some(&Type::I64)));
        let field_count = layout.len();
        // The real body must still fail closed when its scalar representation
        // has no admissible class in the image. Do not turn stale expectations
        // about inlining into permission to reinterpret a Ref slot as an int.
        let mut incompatible = module.arc_field_layouts.clone();
        for fields in incompatible.values_mut() {
            for (_, _, _, index) in &scalar_reads {
                if fields.get(*index) == Some(&Type::I64) { fields[*index] = Type::Ref; }
            }
        }
        let mut incompatible_body = module.functions.iter().find(|f| f.symbol == scalar_reads[0].0).unwrap().clone();
        let error = super::super::prepare_with_class_cases(&mut incompatible_body, &incompatible,
            &mut DartFunctionPlan::default(), None, &HashSet::new(), true).unwrap_err();
        assert!(error.contains("no matching layout declared"), "{error}");
        module.arc_external_callees = true;
        module.arc_external_layouts = crate::lower::sdk_arc_layouts(&context);
        let mut plans = module.functions.iter().map(|f|
            (f.symbol.clone(), DartFunctionPlan::default())).collect();
        crate::optimize::arc::prepare_arc_dart_module(&mut module, &mut plans).unwrap();
        let prepared = module.functions.iter().find(|f| f.symbol.ends_with("Owner.update")).unwrap();
        for (value, original_args) in owner_reads {
            assert!(prepared.blocks.iter().flat_map(|b| &b.instructions).any(|(result, i, ty)|
                *result == value && *ty == Type::Ref && matches!(i,
                    Instruction::CallRuntime { name, args, ret_ty: Type::Ref }
                        if name == "dartforge_arc_read_field_late_ref_v1" && args == &original_args)),
                "the original late Ref read must retain its receiver, field, result and guard");
        }
        for (symbol, value, object, index) in scalar_reads {
            let prepared = module.functions.iter().find(|f| f.symbol == symbol).unwrap();
            let (instruction, ty) = prepared.blocks.iter().flat_map(|b| &b.instructions)
                .find(|(result, _, _)| *result == value).map(|(_, i, ty)| (i, ty)).unwrap();
            let Instruction::CallRuntime { name, args, ret_ty } = instruction else { panic!("scalar field lost its checked accessor"); };
            assert_eq!(name, "dartforge_arc_read_field_scalar_checked_v1");
            assert_eq!((*ty, *ret_ty), (Type::I64, Type::I64));
            assert_eq!(args[0], (object, Type::Ref));
            assert_eq!(args[1], (Operand::Constant(Constant::Int(index as i64)), Type::I64));
            let (Operand::Val(vector), Type::Ptr) = &args[2] else { panic!("guard must carry the image layout vector"); };
            let fields = prepared.blocks.iter().flat_map(|b| &b.instructions).find_map(|(v, i, _)|
                match i { Instruction::ConstArray(fields) if v == vector => Some(fields), _ => None }).unwrap();
            assert!(fields.chunks_exact(2).any(|pair| pair == [i64::from(class_id), field_count as i64]),
                "the checked layout must admit the declared Derived representation");
            assert_eq!(args[3], (Operand::Constant(Constant::Int((fields.len() / 2) as i64)), Type::I64));
            assert_eq!(args[4], (Operand::Constant(Constant::Int(1)), Type::I64), "holds is an i64 scalar, never a Ref");
        }
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
