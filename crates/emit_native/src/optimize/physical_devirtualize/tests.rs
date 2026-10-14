//! Exact physical getter target and uniform descriptor regression tests.

use super::*;

fn sample() -> Module {
    let mut module = Module::new();
    module.arc_fixed_ancestry = true;
    module.classes.push(ClassDef {
        id: 128,
        name: "Box".into(),
        field_count: 0,
        vtable: vec![],
        to_string_symbol: None,
    });
    module.method_tables.push((
        128,
        "table".into(),
        vec![("g:value".into(), "getter".into())],
    ));
    module.table_functions.insert(128, "table".into());
    module.functions.push(Function {
        symbol: "getter".into(),
        name: "getter".into(),
        debug_info: None,
        params: vec![
            (ValueId(0), "this".into(), Type::Ref),
            (ValueId(1), "args".into(), Type::Ptr),
            (ValueId(2), "desc".into(), Type::Ptr),
        ],
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(3),
                    Instruction::LoadIndexed {
                        base: Operand::Val(ValueId(2)),
                        index: Operand::Constant(Constant::Int(0)),
                    },
                    Type::I64,
                ),
                (
                    ValueId(4),
                    Instruction::LoadIndexed {
                        base: Operand::Val(ValueId(2)),
                        index: Operand::Constant(Constant::Int(1)),
                    },
                    Type::I64,
                ),
            ],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
        }],
    });
    module.functions.push(Function {
        symbol: "caller".into(),
        name: "caller".into(),
        debug_info: None,
        params: vec![],
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(0),
                    Instruction::AllocObject {
                        class_id: 128,
                        fields: vec![],
                    },
                    Type::Ref,
                ),
                (
                    ValueId(1),
                    Instruction::CallSelector {
                        selector: "g:value".into(),
                        recv: Operand::Val(ValueId(0)),
                        args: vec![],
                        names: vec![],
                        tuple_types: Operand::Constant(Constant::Int(0)),
                    },
                    Type::Ref,
                ),
            ],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
        }],
    });
    module
}

#[test]
fn direct_call_keeps_same_wrapper_and_exact_immutable_zero_argument_vectors() {
    let mut module = sample();
    let wrapper = format!("{:?}", module.functions[0]);
    assert_eq!(rewrite_getters(&mut module), 1);
    let instructions = &module.functions[1].blocks[0].instructions;
    assert!(
        matches!(&instructions[1], (ValueId(2), Instruction::ConstArray(v), Type::Ptr) if v == &[0])
    );
    assert!(
        matches!(&instructions[2], (ValueId(3), Instruction::ConstArray(v), Type::Ptr) if v == &[0, 0])
    );
    assert!(
        matches!(&instructions[3], (ValueId(1), Instruction::CallStatic { symbol, args, ret_ty: Type::Ref }, Type::Ref)
        if symbol == "getter" && args == &[Operand::Val(ValueId(0)), Operand::Val(ValueId(2)), Operand::Val(ValueId(3))])
    );
    assert_eq!(format!("{:?}", module.functions[0]), wrapper);
    assert_eq!(rewrite_getters(&mut module), 0);
}

#[test]
fn unknown_nullable_argument_and_abi_inputs_keep_dispatch() {
    for mutation in 0..13 {
        let mut module = sample();
        match mutation {
            0 => module.arc_fixed_ancestry = false,
            1 => module.arc_memory = true,
            2 => module.arc_prepared = true,
            3 => module.sdk_library = true,
            4 => module.functions[0].params[1].2 = Type::I64,
            5 => module.functions[0].return_ty = Type::I64,
            6 => {
                module.functions[1].blocks[0].instructions[0].1 = Instruction::Const(Constant::Null)
            }
            7 => {
                module.functions[1].blocks[0].instructions[0].1 = Instruction::CallRuntime {
                    name: "opaque".into(),
                    args: vec![],
                    ret_ty: Type::Ref,
                }
            }
            8 => {
                if let Instruction::CallSelector { args, .. } =
                    &mut module.functions[1].blocks[0].instructions[1].1
                {
                    args.push(Operand::Constant(Constant::Null));
                }
            }
            9 => {
                if let Instruction::CallSelector { names, .. } =
                    &mut module.functions[1].blocks[0].instructions[1].1
                {
                    names.push("named".into());
                }
            }
            10 => {
                if let Instruction::CallSelector { tuple_types, .. } =
                    &mut module.functions[1].blocks[0].instructions[1].1
                {
                    *tuple_types = Operand::Val(ValueId(0));
                }
            }
            11 => module.functions.push(module.functions[0].clone()),
            12 => module.functions[0].params[2].0 = ValueId(1),
            _ => unreachable!(),
        }
        let before = format!("{module:?}");
        assert_eq!(rewrite_getters(&mut module), 0, "mutation {mutation}");
        assert_eq!(format!("{module:?}"), before);
    }
}

#[test]
fn descriptor_and_argument_aliases_writes_forwarding_and_terminator_uses_refuse() {
    for mutation in 0..7 {
        let mut module = sample();
        let block = &mut module.functions[0].blocks[0];
        match mutation {
            0 => {
                block.instructions[0].1 = Instruction::LoadIndexed {
                    base: Operand::Val(ValueId(2)),
                    index: Operand::Constant(Constant::Int(2)),
                }
            }
            1 => block.instructions[0].2 = Type::Ref,
            2 => {
                block.instructions[0].1 = Instruction::LoadIndexed {
                    base: Operand::Val(ValueId(1)),
                    index: Operand::Constant(Constant::Int(0)),
                }
            }
            3 => {
                block.instructions[0].1 = Instruction::CallStatic {
                    symbol: "forward".into(),
                    args: vec![Operand::Val(ValueId(2))],
                    ret_ty: Type::I64,
                }
            }
            4 => {
                block.instructions[0].1 = Instruction::Store {
                    ptr: Operand::Val(ValueId(2)),
                    val: Operand::Constant(Constant::Int(0)),
                }
            }
            5 => block.terminator = Terminator::Return(Some(Operand::Val(ValueId(2)))),
            6 => block.instructions.push((
                ValueId(5),
                Instruction::Bitcast {
                    op: Operand::Val(ValueId(2)),
                    to: Type::I64,
                },
                Type::I64,
            )),
            _ => unreachable!(),
        }
        assert_eq!(rewrite_getters(&mut module), 0, "mutation {mutation}");
    }
}

#[test]
fn absent_ambiguous_conflicting_or_unregistered_tables_keep_nsm_and_override_dispatch() {
    for mutation in 0..8 {
        let mut module = sample();
        match mutation {
            0 => module.method_tables.clear(),
            1 => module.method_tables.push(module.method_tables[0].clone()),
            2 => module.classes.push(module.classes[0].clone()),
            3 => {
                module.pruning_ambiguous_class_ids.insert(128);
            }
            4 => {
                module.arc_external_ancestry.nominal_classes.insert(128);
            }
            5 => {
                module
                    .table_functions
                    .insert(128, "different_registration".into());
            }
            6 => {
                module.method_tables[0].2.clear();
            }
            7 => {
                module.method_tables[0]
                    .2
                    .push(("g:value".into(), "different_override".into()));
            }
            _ => unreachable!(),
        };
        assert_eq!(rewrite_getters(&mut module), 0, "mutation {mutation}");
    }
}

#[test]
fn throwing_wrapper_and_caller_pending_edges_are_preserved() {
    let mut module = sample();
    module.functions[0].blocks[0].instructions.push((
        ValueId(5),
        Instruction::CallRuntime {
            name: "may_throw".into(),
            args: vec![],
            ret_ty: Type::Void,
        },
        Type::Void,
    ));
    module.functions[1].blocks[0].instructions.push((
        ValueId(4),
        Instruction::CallRuntime {
            name: "dartforge_exception_pending".into(),
            args: vec![],
            ret_ty: Type::I8,
        },
        Type::I8,
    ));
    let wrapper = format!("{:?}", module.functions[0]);
    let pending = format!("{:?}", module.functions[1].blocks[0].instructions[2]);
    assert_eq!(rewrite_getters(&mut module), 1);
    assert_eq!(format!("{:?}", module.functions[0]), wrapper);
    assert_eq!(
        format!(
            "{:?}",
            module.functions[1].blocks[0].instructions.last().unwrap()
        ),
        pending
    );
}

#[test]
fn ordinary_dynamic_getters_have_real_local_wrapper_certificates() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let entry = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/devirtualization/physical_getters.dart");
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
        module.arc_fixed_ancestry = true;
        for function in &mut module.functions { super::super::mem2reg::promote(function); }
        let before: usize = module.functions.iter().flat_map(|f| &f.blocks).flat_map(|b| &b.instructions)
            .filter(|(_, i, _)| matches!(i, Instruction::CallSelector { selector, .. } if selector == "g:value" || selector == "g:failure")).count();
        assert_eq!(before, 4, "the ordinary fixture must exercise actual dynamic getters");
        assert_eq!(rewrite_getters(&mut module), 2, "direct physical allocations must certify both getter wrappers: {module:#?}");
        let remaining: usize = module.functions.iter().flat_map(|f| &f.blocks).flat_map(|b| &b.instructions)
            .filter(|(_, i, _)| matches!(i, Instruction::CallSelector { selector, .. } if selector == "g:value" || selector == "g:failure")).count();
        assert_eq!(remaining, 2, "factory error returns remain nullable and indirect");
        assert!(module.functions.iter().flat_map(|f| &f.blocks).flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_exception_pending")));
        let factory = module.functions.iter().find(|f| f.symbol.ends_with("..probeFactory")).unwrap();
        assert_eq!(factory.blocks.iter().flat_map(|b| &b.instructions).filter(|(_, i, _)| matches!(i, Instruction::CallSelector { selector, .. } if selector == "g:value" || selector == "g:failure")).count(), 2);
        let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
        assert!(ir.contains("Holder.value$g") && ir.contains("Holder.failure$g"));
    }).unwrap().join().unwrap();
}

#[test]
fn complete_receiver_union_requires_one_wrapper_for_every_actual_class() {
    for different_target in [false, true] {
        let mut module = sample();
        module.classes.push(ClassDef {
            id: 129,
            name: "Derived".into(),
            field_count: 0,
            vtable: vec![],
            to_string_symbol: None,
        });
        module.method_tables.push((
            129,
            "derived_table".into(),
            vec![(
                "g:value".into(),
                if different_target {
                    "override"
                } else {
                    "getter"
                }
                .into(),
            )],
        ));
        module.table_functions.insert(129, "derived_table".into());
        let mut override_body = module.functions[0].clone();
        override_body.symbol = "override".into();
        module.functions.push(override_body);
        let caller = &mut module.functions[1];
        caller
            .params
            .push((ValueId(9), "condition".into(), Type::I1));
        caller.blocks = vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::CondBranch {
                    cond: Operand::Val(ValueId(9)),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![(
                    ValueId(0),
                    Instruction::AllocObject {
                        class_id: 128,
                        fields: vec![],
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![(
                    ValueId(2),
                    Instruction::AllocObject {
                        class_id: 129,
                        fields: vec![],
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![
                    (
                        ValueId(3),
                        Instruction::Phi {
                            incoming: vec![
                                (BlockId(1), Operand::Val(ValueId(0))),
                                (BlockId(2), Operand::Val(ValueId(2))),
                            ],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(4),
                        Instruction::CallSelector {
                            selector: "g:value".into(),
                            recv: Operand::Val(ValueId(3)),
                            args: vec![],
                            names: vec![],
                            tuple_types: Operand::Constant(Constant::Int(0)),
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(4)))),
            },
        ];
        assert_eq!(rewrite_getters(&mut module), usize::from(!different_target));
    }
}

#[test]
fn nominal_receiver_hint_and_shadowed_runtime_registration_do_not_certify() {
    let mut module = sample();
    module.functions[1]
        .params
        .push((ValueId(0), "this".into(), Type::Ref));
    module.functions[1].blocks[0].instructions.remove(0);
    module
        .pruning_this_receivers
        .insert("caller".into(), (ValueId(0), 128));
    assert_eq!(rewrite_getters(&mut module), 0);
    for shadow in [
        "dartforge_register_methods",
        "dartforge_selector",
        "dartforge_object_new",
    ] {
        let mut module = sample();
        let mut body = module.functions[0].clone();
        body.symbol = shadow.into();
        module.functions.push(body);
        assert_eq!(rewrite_getters(&mut module), 0, "shadow {shadow}");
    }
}

#[test]
fn production_optimizer_hook_rewrites_before_inlining() {
    let mut module = sample();
    super::super::optimize(&mut module);
    assert!(
        !module.functions[1]
            .blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .any(|(_, instruction, _)| matches!(instruction, Instruction::CallSelector { .. }))
    );
    assert!(module.functions[1].blocks.iter().flat_map(|b| &b.instructions).any(|(_, instruction, _)| matches!(instruction, Instruction::CallStatic { symbol, .. } if symbol == "getter")), "descriptor reads intentionally keep the same uniform wrapper out of inlining");
}

#[test]
fn ordinary_nullable_factory_unknown_and_override_boundaries_remain_indirect() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let entry = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/devirtualization/physical_getter_boundaries.dart");
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
        module.arc_fixed_ancestry = true;
        for function in &mut module.functions { super::super::mem2reg::promote(function); }
        let original = module.functions.clone();
        assert_eq!(rewrite_getters(&mut module), 2, "only direct.value and direct.failure qualify");
        for suffix in ["..readUnknown", "..probeMixed"] {
            let before = original.iter().find(|f| f.symbol.ends_with(suffix)).unwrap();
            let after = module.functions.iter().find(|f| f.symbol.ends_with(suffix)).unwrap();
            assert_eq!(format!("{before:?}"), format!("{after:?}"), "unknown parameter or actual overriding receiver union must remain untouched");
        }
        let main = module.functions.iter().find(|f| f.symbol == "dart_main").unwrap();
        assert!(main.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i, Instruction::CallSelector { args, names, .. } if !args.is_empty() && names == &["named"])));
        assert!(main.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i, Instruction::CallStatic { symbol, .. } if symbol.ends_with("..namedValue"))));
    }).unwrap().join().unwrap();
}
