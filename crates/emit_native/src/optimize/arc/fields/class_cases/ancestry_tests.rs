use super::*;

fn metadata() -> Module {
    let mut m = Module::new();
    m.arc_fixed_ancestry = true;
    for cid in [100, 123, 124, 125, 126, 127] {
        m.classes.push(ClassDef {
            id: cid,
            name: format!("class{cid}"),
            field_count: if matches!(cid, 124 | 125) { 2 } else { 1 },
            vtable: vec![],
            to_string_symbol: None,
        });
        m.pruning_class_ids.insert(cid);
        m.subtyping_edges.push((cid, 0));
    }
    m.pruning_class_ids.insert(0);
    m.subtyping_edges
        .extend([(0, 0), (123, 100), (124, 100), (125, 124), (127, 126)]);
    m
}

#[test]
fn ordinary_nullable_sibling_nodes_prepare_and_ancestry_mutations_invalidate_ports() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let entry = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/arc_ports/nullable_huffman_nodes.dart");
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
        let core_library = program.libraries.iter().position(|library| library.uri == "dart:core").unwrap();
        let core_context = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies)
            .with_source_sdk().only_the_library(dartforge_elements::model::LibraryId(core_library as u32));
        let core_facts = crate::lower::class_registration_facts(&core_context);
        let core_summary = crate::pruning::hir_summary(&core_facts);
        let mut m = crate::lower::lower_program(&context);
        assert!(m.errors.is_empty(), "{:?}", m.errors);
        m.arc_memory = true;
        m.arc_fixed_ancestry = true;
        m.arc_external_ancestry = ArcExternalAncestry::from_summaries(&[&core_summary]);
        m.arc_external_callees = true;
        m.arc_external_layouts = crate::lower::sdk_arc_layouts(&context);
        crate::optimize::optimize(&mut m);
        let original = m.functions.iter().find(|f| f.symbol.ends_with(".readNode")).unwrap().clone();
        assert!(original.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i,
            Instruction::CallRuntime { name, .. } if name == "dartforge_is_subclass")));
        assert!(super::super::prepare(&mut original.clone(), &m.arc_field_layouts, &mut DartFunctionPlan::default(), None, &HashSet::new()).is_err());
        let mut plans = m.functions.iter().map(|f| (f.symbol.clone(), DartFunctionPlan::default())).collect();
        crate::optimize::arc::prepare_arc_dart_module(&mut m, &mut plans).unwrap();
        let prepared = m.functions.iter().find(|f| f.symbol.ends_with(".readNode")).unwrap();
        assert!(prepared.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i,
            Instruction::CallRuntime { name, .. } if name == "dartforge_arc_read_field_scalar_checked_v1")));
        crate::optimize::arc::certify_arc_aot_ports(&mut m, &plans).unwrap();
        let certificate = m.arc_aot_ports.as_ref().unwrap();
        assert!(certificate.check(&m).is_ok());
        let parent = m.classes.iter().find(|c| c.name == "Parent").unwrap().id;
        let leaf = m.classes.iter().find(|c| c.name == "Leaf").unwrap().id;
        for case in 0..5 {
            let mut changed = m.clone();
            match case {
                0 => changed.arc_fixed_ancestry = false,
                1 => changed.subtyping_edges.push((parent, leaf)),
                2 => { changed.pruning_class_ids.remove(&parent); },
                3 => { changed.pruning_ambiguous_class_ids.insert(parent); },
                4 => changed.arc_external_ancestry = ArcExternalAncestry::default(),
                _ => unreachable!(),
            }
            assert!(certificate.check(&changed).is_err(), "case {case}");
        }
    }).unwrap().join().unwrap();
}

fn subtype_function() -> Function {
    let mut f = super::tests::function();
    f.blocks[3].instructions.extend([
        (
            ValueId(6),
            Instruction::CallRuntime {
                name: "dartforge_is_subclass".into(),
                args: vec![
                    (Operand::Val(ValueId(4)), Type::I64),
                    (Operand::Constant(Constant::Int(124)), Type::I64),
                ],
                ret_ty: Type::I8,
            },
            Type::I8,
        ),
        (
            ValueId(7),
            Instruction::ICmp(
                ICmpOp::Ne,
                Operand::Val(ValueId(6)),
                Operand::Constant(Constant::Int(0)),
            ),
            Type::I1,
        ),
    ]);
    f.blocks[3].terminator = Terminator::CondBranch {
        cond: Operand::Val(ValueId(7)),
        then_block: BlockId(4),
        else_block: BlockId(5),
    };
    f
}

fn refine(f: &Function, metadata: &Module, canonical: bool, block: BlockId, cid: u32) -> Origin {
    let ancestry = PhysicalAncestry::from_module(metadata);
    let mut origin = Origin {
        sites: HashSet::from([ValueId(1)]),
        unknown: true,
    };
    Cases::with_ancestry(f, canonical, Some(&ancestry)).refine(
        &mut origin,
        &Operand::Val(ValueId(3)),
        block,
        &HashMap::from([(ValueId(1), cid)]),
    );
    origin
}

#[test]
fn complete_registered_ancestry_preserves_subclasses_interfaces_and_terminal_object() {
    let ancestry = PhysicalAncestry::from_module(&metadata());
    assert!(ancestry.excludes(123, 124));
    assert!(!ancestry.excludes(125, 124));
    assert!(!ancestry.excludes(127, 126));
    assert!(!ancestry.excludes(123, 0));
    assert!(!ancestry.excludes(123, 123));
    for case in 0..10 {
        let mut m = metadata();
        match case {
            0 => m.arc_fixed_ancestry = false,
            1 => {
                m.pruning_class_ids.remove(&123);
            }
            2 => {
                m.pruning_class_ids.remove(&100);
            }
            3 => {
                m.pruning_ambiguous_class_ids.insert(123);
            }
            4 => {
                m.pruning_ambiguous_class_ids.insert(124);
            }
            5 => m.subtyping_edges.push((123, 999)),
            6 => m.subtyping_edges.push((100, 123)),
            7 => m.subtyping_edges.push((0, 124)),
            8 => m.classes.push(m.classes[1].clone()),
            9 => m.classes.retain(|c| c.id != 123),
            _ => unreachable!(),
        }
        assert!(
            !PhysicalAncestry::from_module(&m).excludes(123, 124),
            "case {case}"
        );
    }
    assert!(!ancestry.excludes(999, 124));
    assert!(!ancestry.excludes(123, 999));
}

#[test]
fn positive_subtype_case_filters_only_incompatible_allocations_and_keeps_checked_access() {
    let f = subtype_function();
    let m = metadata();
    let filtered = refine(&f, &m, true, BlockId(4), 123);
    assert!(filtered.sites.is_empty());
    assert!(filtered.unknown);
    assert_eq!(
        refine(&f, &m, true, BlockId(4), 125).sites,
        HashSet::from([ValueId(1)])
    );
    assert_eq!(
        refine(&f, &m, false, BlockId(4), 123).sites,
        HashSet::from([ValueId(1)])
    );
    let layouts = HashMap::from([(123, vec![Type::Ref]), (124, vec![Type::Ref, Type::I64])]);
    let ancestry = PhysicalAncestry::from_module(&m);
    let mut prepared = f;
    super::super::prepare_with_ancestry(
        &mut prepared,
        &layouts,
        &mut DartFunctionPlan::default(),
        None,
        &HashSet::new(),
        true,
        Some(&ancestry),
    )
    .unwrap();
    assert!(prepared.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i,
        Instruction::CallRuntime { name, .. } if name == "dartforge_arc_read_field_scalar_checked_v1")));
}

#[test]
fn false_shared_join_and_reentered_subtype_edges_do_not_refine() {
    let m = metadata();
    let f = subtype_function();
    assert_eq!(
        refine(&f, &m, true, BlockId(5), 123).sites,
        HashSet::from([ValueId(1)])
    );
    for case in 0..4 {
        let mut f = subtype_function();
        match case {
            0 => {
                f.blocks[3].terminator = Terminator::CondBranch {
                    cond: Operand::Val(ValueId(7)),
                    then_block: BlockId(4),
                    else_block: BlockId(4),
                }
            }
            1 => {
                f.blocks[4].terminator = Terminator::Branch(BlockId(6));
                f.blocks[5].terminator = Terminator::Branch(BlockId(6));
                f.blocks.push(BasicBlock {
                    id: BlockId(6),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                });
            }
            2 => f.blocks[4].terminator = Terminator::Branch(BlockId(4)),
            3 => f.blocks[5].terminator = Terminator::Branch(BlockId(4)),
            _ => unreachable!(),
        }
        let block = if case == 1 { BlockId(6) } else { BlockId(4) };
        assert_eq!(
            refine(&f, &m, true, block, 123).sites,
            HashSet::from([ValueId(1)]),
            "case {case}"
        );
    }
}

#[test]
fn canonical_widened_boolean_and_implies_subtype_but_integer_or_and_false_tests_do_not() {
    for case in 0..7 {
        let mut f = subtype_function();
        f.blocks[3].instructions.extend([
            (
                ValueId(8),
                Instruction::And(
                    Operand::Val(ValueId(7)),
                    Operand::Constant(Constant::Bool(true)),
                ),
                Type::I64,
            ),
            (
                ValueId(10),
                Instruction::ICmp(
                    ICmpOp::Ne,
                    Operand::Val(ValueId(8)),
                    Operand::Constant(Constant::Int(0)),
                ),
                Type::I1,
            ),
        ]);
        f.blocks[3].terminator = Terminator::CondBranch {
            cond: Operand::Val(ValueId(10)),
            then_block: BlockId(4),
            else_block: BlockId(5),
        };
        match case {
            0 => {}
            1 => {
                f.blocks[3].instructions[4].1 = Instruction::Or(
                    Operand::Val(ValueId(7)),
                    Operand::Constant(Constant::Bool(true)),
                )
            }
            2 => {
                f.blocks[3].instructions[4].1 = Instruction::And(
                    Operand::Val(ValueId(7)),
                    Operand::Constant(Constant::Int(3)),
                )
            }
            3 => {
                f.blocks[3].instructions[4].2 = Type::I1;
                f.blocks[3].terminator = Terminator::CondBranch {
                    cond: Operand::Val(ValueId(8)),
                    then_block: BlockId(4),
                    else_block: BlockId(5),
                };
            }
            4 => {
                f.blocks[3].instructions[3].1 = Instruction::ICmp(
                    ICmpOp::Eq,
                    Operand::Val(ValueId(6)),
                    Operand::Constant(Constant::Int(0)),
                )
            }
            5 => f.blocks[3].instructions.swap(2, 3),
            6 => f.blocks[3].instructions.swap(4, 5),
            _ => unreachable!(),
        }
        let result = refine(&f, &metadata(), true, BlockId(4), 123);
        assert_eq!(result.sites.is_empty(), case == 0, "case {case}");
    }
}

#[test]
fn malformed_subtype_queries_and_nonconstant_targets_never_prove_exclusions() {
    for case in 0..8 {
        let mut f = subtype_function();
        match case {
            0 => {
                if let Instruction::CallRuntime { args, .. } = &mut f.blocks[3].instructions[2].1 {
                    args[0].1 = Type::I8;
                }
            }
            1 => {
                if let Instruction::CallRuntime { args, .. } = &mut f.blocks[3].instructions[2].1 {
                    args[1].1 = Type::I8;
                }
            }
            2 => {
                if let Instruction::CallRuntime { ret_ty, .. } = &mut f.blocks[3].instructions[2].1
                {
                    *ret_ty = Type::I64;
                }
            }
            3 => f.blocks[3].instructions[2].2 = Type::I64,
            4 => {
                if let Instruction::CallRuntime { args, .. } = &mut f.blocks[3].instructions[2].1 {
                    args[1].0 = Operand::Constant(Constant::Int(-1));
                }
            }
            5 => {
                if let Instruction::CallRuntime { args, .. } = &mut f.blocks[3].instructions[2].1 {
                    args[1].0 = Operand::Val(ValueId(4));
                }
            }
            6 => {
                if let Instruction::CallRuntime { name, .. } = &mut f.blocks[3].instructions[2].1 {
                    *name = "opaque_subclass".into();
                }
            }
            7 => f.blocks[3].instructions.swap(1, 2),
            _ => unreachable!(),
        }
        assert_eq!(
            refine(&f, &metadata(), true, BlockId(4), 123).sites,
            HashSet::from([ValueId(1)]),
            "case {case}"
        );
    }
}

#[test]
fn external_registered_ancestry_is_not_an_allocation_or_nominal_hint_proof() {
    let mut summary = crate::pruning::Summary::default();
    summary.nominal_classes = vec![0, 200];
    summary.parent_edges = vec![(200, 0)];
    let mut m = metadata();
    m.subtyping_edges.push((100, 200));
    assert!(!PhysicalAncestry::from_module(&m).excludes(123, 124));
    m.arc_external_ancestry = ArcExternalAncestry::from_summaries(&[&summary]);
    assert!(PhysicalAncestry::from_module(&m).excludes(123, 124));
    for case in 0..9 {
        let mut changed = m.clone();
        match case {
            0 => {
                changed.arc_external_ancestry.nominal_classes.remove(&200);
            }
            1 => {
                changed.arc_external_ancestry.ambiguous_classes.insert(200);
            }
            2 => changed.arc_external_ancestry.parent_edges.push((200, 999)),
            3 => changed.arc_external_ancestry.parent_edges.push((200, 100)),
            4 => changed.arc_external_ancestry.parent_edges.push((0, 124)),
            5 => {
                changed.arc_external_ancestry.nominal_classes.insert(123);
            }
            6 => changed.arc_external_ancestry.parent_edges.push((123, 200)),
            7 => {
                changed.arc_external_ancestry =
                    ArcExternalAncestry::from_summaries(&[&summary, &summary])
            }
            8 => {
                let mut invalid = summary.clone();
                invalid.parent_edges.push((-1, 0));
                changed.arc_external_ancestry = ArcExternalAncestry::from_summaries(&[&invalid]);
            }
            _ => unreachable!(),
        }
        assert!(
            !PhysicalAncestry::from_module(&changed).excludes(123, 124),
            "case {case}"
        );
    }
    let f = subtype_function();
    let ancestry = PhysicalAncestry::from_module(&m);
    let mut formal = Origin {
        sites: HashSet::from([ValueId(0)]),
        unknown: true,
    };
    Cases::with_ancestry(&f, true, Some(&ancestry)).refine(
        &mut formal,
        &Operand::Val(ValueId(3)),
        BlockId(4),
        &HashMap::from([(ValueId(0), 123)]),
    );
    assert_eq!(formal.sites, HashSet::from([ValueId(0)]));
    assert!(formal.unknown);
    let mut missing_allocation = Origin {
        sites: HashSet::from([ValueId(1)]),
        unknown: true,
    };
    Cases::with_ancestry(&f, true, Some(&ancestry)).refine(
        &mut missing_allocation,
        &Operand::Val(ValueId(3)),
        BlockId(4),
        &HashMap::new(),
    );
    assert_eq!(missing_allocation.sites, HashSet::from([ValueId(1)]));
}

#[test]
fn constant_subtype_queries_fold_only_complete_canonical_false_facts() {
    for case in 0..16 {
        let mut m = metadata();
        let mut f = subtype_function();
        let Instruction::CallRuntime { args, .. } = &mut f.blocks[3].instructions[2].1 else {
            unreachable!()
        };
        args[0].0 = Operand::Constant(Constant::Int(123));
        match case {
            0 => {}
            1 => m.arc_fixed_ancestry = false,
            2 => {
                m.pruning_class_ids.remove(&100);
            }
            3 => {
                if let Instruction::CallRuntime { args, .. } = &mut f.blocks[3].instructions[2].1 {
                    args[0].0 = Operand::Constant(Constant::Int(125));
                }
            }
            4 => {
                if let Instruction::CallRuntime { args, .. } = &mut f.blocks[3].instructions[2].1 {
                    args[1].0 = Operand::Constant(Constant::Int(0));
                }
            }
            5 => {
                if let Instruction::CallRuntime { args, .. } = &mut f.blocks[3].instructions[2].1 {
                    args[0].1 = Type::I8;
                }
            }
            6 => {
                if let Instruction::CallRuntime { args, .. } = &mut f.blocks[3].instructions[2].1 {
                    args[1].1 = Type::I8;
                }
            }
            7 => {
                if let Instruction::CallRuntime { ret_ty, .. } = &mut f.blocks[3].instructions[2].1
                {
                    *ret_ty = Type::I64;
                }
            }
            8 => f.blocks[3].instructions[2].2 = Type::I64,
            9 => {
                if let Instruction::CallRuntime { args, .. } = &mut f.blocks[3].instructions[2].1 {
                    args[0].0 = Operand::Val(ValueId(4));
                }
            }
            10 => {
                if let Instruction::CallRuntime { name, .. } = &mut f.blocks[3].instructions[2].1 {
                    *name = "opaque_subclass".into();
                }
            }
            11 => {
                if let Instruction::CallRuntime { args, .. } = &mut f.blocks[3].instructions[2].1 {
                    args[0].0 = Operand::Constant(Constant::Int(-1));
                }
            }
            12 => {
                if let Instruction::CallRuntime { args, .. } = &mut f.blocks[3].instructions[2].1 {
                    args[0].0 = Operand::Constant(Constant::Int(i64::from(i32::MAX) + 1));
                }
            }
            13 => {
                m.pruning_ambiguous_class_ids.insert(123);
            }
            14 => {
                let mut shadow = f.clone();
                shadow.symbol = "dartforge_is_subclass".into();
                m.functions.push(shadow);
            }
            15 => {
                if let Instruction::CallRuntime { args, .. } = &mut f.blocks[3].instructions[2].1 {
                    args.push((Operand::Constant(Constant::Int(0)), Type::I64));
                }
            }
            _ => unreachable!(),
        }
        let before = f.blocks[3].instructions.clone();
        m.functions.push(f);
        assert_eq!(super::super::fold_fixed_subtype_tests(&mut m), case == 0);
        let after = &m.functions.last().unwrap().blocks[3].instructions;
        if case == 0 {
            assert!(matches!(after[2].1, Instruction::Const(Constant::Int(0))));
            assert_eq!(after[2].2, Type::I8);
            assert_eq!(format!("{:?}", &after[..2]), format!("{:?}", &before[..2]));
            assert_eq!(format!("{:?}", &after[3..]), format!("{:?}", &before[3..]));
        } else {
            assert_eq!(format!("{after:?}"), format!("{before:?}"), "case {case}");
        }
    }
}
