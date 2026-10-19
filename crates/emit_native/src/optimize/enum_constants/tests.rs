use super::*;

fn constant(value: i64) -> Operand {
    Operand::Constant(Constant::Int(value))
}
fn call(
    value: u32,
    name: &str,
    args: Vec<(Operand, Type)>,
    ty: Type,
) -> (ValueId, Instruction, Type) {
    (
        ValueId(value),
        Instruction::CallRuntime {
            name: name.into(),
            args,
            ret_ty: ty,
        },
        ty,
    )
}
fn fixture() -> Module {
    let mut module = Module::new();
    module.arc_fixed_ancestry = true;
    module.classes.push(ClassDef {
        id: 300,
        name: "Simple".into(),
        field_count: 2,
        vtable: vec![],
        to_string_symbol: None,
    });
    for index in 0..2 {
        let getter = format!("getter{index}");
        let global = format!("cache{index}");
        let flag = format!("{global}$ok");
        let object = Operand::Val(ValueId(4));
        let function = Function {
            symbol: getter.clone(),
            name: getter.clone(),
            params: vec![],
            return_ty: Type::Ref,
            debug_info: None,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(0),
                            Instruction::LoadGlobal {
                                symbol: flag.clone(),
                                ty: Type::I8,
                            },
                            Type::I8,
                        ),
                        (
                            ValueId(1),
                            Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(0)), constant(0)),
                            Type::I1,
                        ),
                    ],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(1)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::LoadGlobal {
                            symbol: global.clone(),
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![
                        (
                            ValueId(3),
                            Instruction::StoreGlobal {
                                symbol: flag,
                                val: constant(1),
                                ty: Type::I8,
                                root: None,
                            },
                            Type::Void,
                        ),
                        call(
                            4,
                            "dartforge_object_new",
                            vec![(constant(300), Type::I64), (constant(2), Type::I64)],
                            Type::Ref,
                        ),
                        (
                            ValueId(5),
                            Instruction::StoreGlobal {
                                symbol: global.clone(),
                                val: object.clone(),
                                ty: Type::Ref,
                                root: Some(index),
                            },
                            Type::Void,
                        ),
                        call(
                            6,
                            "dartforge_mark_constant",
                            vec![
                                (object.clone(), Type::Ref),
                                (
                                    Operand::Constant(Constant::Function(getter.clone())),
                                    Type::I64,
                                ),
                            ],
                            Type::Void,
                        ),
                        call(
                            7,
                            "dartforge_object_set",
                            vec![
                                (object.clone(), Type::Ref),
                                (constant(0), Type::I64),
                                (constant(i64::from(index)), Type::I64),
                                (constant(0), Type::I8),
                            ],
                            Type::Void,
                        ),
                        (
                            ValueId(8),
                            Instruction::Const(Constant::String(format!("v{index}"))),
                            Type::Ref,
                        ),
                        call(
                            9,
                            "dartforge_object_set",
                            vec![
                                (object.clone(), Type::Ref),
                                (constant(1), Type::I64),
                                (Operand::Val(ValueId(8)), Type::I64),
                                (constant(1), Type::I8),
                            ],
                            Type::Void,
                        ),
                    ],
                    terminator: Terminator::Return(Some(object)),
                },
            ],
        };
        module.globals.push((index, Type::Ref, global.clone()));
        module.simple_enum_constants.insert(
            getter.clone(),
            SimpleEnumConstant {
                library_uri: "file:enum.dart".into(),
                unit_uri: "file:enum.dart".into(),
                owner: "Simple".into(),
                declaration: 0,
                index: index as usize,
                name: format!("v{index}"),
                class_id: 300,
                getter,
                global,
                root: index,
            },
        );
        module.functions.push(function);
    }
    module.functions.push(Function {
        symbol: "caller".into(),
        name: "caller".into(),
        params: vec![],
        return_ty: Type::Ref,
        debug_info: None,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::CallStatic {
                            symbol: "getter0".into(),
                            args: vec![],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    call(1, "dartforge_exception_pending", vec![], Type::I8),
                    (
                        ValueId(2),
                        Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(1)), constant(0)),
                        Type::I1,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: Operand::Val(ValueId(2)),
                    then_block: BlockId(2),
                    else_block: BlockId(1),
                },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
            },
        ],
    });
    module
}
#[test]
fn guarded_success_tokens_are_body_bound_and_fold_only_identity() {
    let module = fixture();
    let inventory = Inventory::new(&module);
    let caller = &module.functions[2];
    let token = inventory.token_for_call(caller, ValueId(0)).unwrap();
    assert!(
        inventory
            .token_for_call(&caller.clone(), ValueId(0))
            .is_none()
    );
    let mut clone = Function {
        symbol: "clone".into(),
        name: "clone".into(),
        params: vec![
            (ValueId(0), "from".into(), Type::Ref),
            (ValueId(1), "to".into(), Type::Ref),
        ],
        return_ty: Type::I1,
        debug_info: None,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                ValueId(2),
                Instruction::ICmp(
                    ICmpOp::Eq,
                    Operand::Val(ValueId(0)),
                    Operand::Val(ValueId(1)),
                ),
                Type::I1,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
        }],
    };
    let before = clone.params.clone();
    assert_eq!(
        inventory.fold_identity_comparisons(
            &mut clone,
            &HashMap::from([(ValueId(0), token.clone()), (ValueId(1), token)])
        ),
        1
    );
    assert!(matches!(
        clone.blocks[0].instructions[0].1,
        Instruction::Const(Constant::Bool(true))
    ));
    assert_eq!(before, clone.params);
}
#[test]
fn writer_marker_shape_aliases_and_unsuccessful_use_are_unknown() {
    for variant in 0..12 {
        let mut module = fixture();
        match variant {
            0 => module.functions.push(module.functions[0].clone()),
            1 => module.globals.push(module.globals[0].clone()),
            2 => {
                let write = module.functions[0].blocks[2].instructions[2].clone();
                module.functions[2].blocks[1].instructions.push(write);
            }
            3 => {
                module.functions[0].blocks[2].instructions[3].1 = Instruction::Const(Constant::Null)
            }
            4 => {
                module.functions[0].blocks[2].instructions[4].1 = Instruction::Const(Constant::Null)
            }
            5 => module
                .simple_enum_constants
                .get_mut("getter0")
                .unwrap()
                .getter
                .clear(),
            6 => {
                module.pruning_ambiguous_class_ids.insert(300);
            }
            7 => module.functions[2].blocks[0].instructions.truncate(1),
            8 => {
                module.functions[2].blocks[2].terminator =
                    Terminator::Return(Some(Operand::Val(ValueId(0))))
            }
            9 => module.functions[2].blocks[1].instructions.push((
                ValueId(90),
                Instruction::NativeCall {
                    target: Operand::Constant(Constant::Int(1)),
                    args: Vec::new(),
                    ret: CType::Void,
                },
                Type::Void,
            )),
            _ => module.functions[2].blocks[1].instructions.push((
                ValueId(91),
                Instruction::CompositeNativeCall {
                    target: Operand::Constant(Constant::Int(1)),
                    args: vec![(
                        Operand::Constant(Constant::Int(2)),
                        NativeType::Composite(LayoutC {
                            size: 8,
                            alignment: 8,
                            leaves: vec![(0, CType::I64)],
                        }),
                    )],
                    ret: NativeType::Prim(CType::Void),
                    dest: None,
                    variadic_flag: None,
                },
                Type::Void,
            )),
        };
        assert!(
            Inventory::new(&module)
                .token_for_call(&module.functions[2], ValueId(0))
                .is_none(),
            "variant {variant}"
        );
    }
}

#[test]
fn ordinary_simple_enum_lowering_certifies_current_getter_and_rejects_enhanced_shape() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        use dartforge_types::table::{CoreTypes, TypeTable};
        let directory=tempfile::tempdir().unwrap();let entry=directory.path().join("main.dart");
        std::fs::write(&entry,"enum Simple { a, b }\nenum Enhanced { a(1); const Enhanced(this.x); final int x; }\nvoid main() { print(Simple.a == Simple.b); print(Enhanced.a.x); }\n").unwrap();
        let sdk=crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();let mut interner=dartforge_intern::Interner::new();
        let (program,diagnostics)=dartforge_elements::load::load_lenient(&entry,&sdk,None,&mut interner);assert!(diagnostics.is_empty());
        let mut table=TypeTable::new();let core=CoreTypes::init(&mut table,&program,&interner);
        let (mut outline,_)=dartforge_types::resolve_outline(&program,&interner,&mut table,&core);
        let libraries:Vec<_>=program.libraries.iter().enumerate().filter(|(_,library)|!library.is_sdk).map(|(i,_)|dartforge_elements::model::LibraryId(i as u32)).collect();
        let (bodies,_)=dartforge_types::infer_library_bodies(&program,&interner,&mut table,&core,&mut outline,&libraries);
        let context=crate::context::Context::new(&program,&interner,&table,&core,&outline,&bodies).with_source_sdk();
        let mut module=crate::lower::lower_program(&context);assert!(module.errors.is_empty(),"{:?}",module.errors);module.arc_fixed_ancestry=true;
        assert_eq!(module.simple_enum_constants.len(),2);
        assert!(module.simple_enum_constants.values().all(|premise|premise.owner=="Simple"));
        // Lowering emits native adapters for the complete loaded SDK signature
        // table. Only the existing recipe/reachability fixpoint proves them dead.
        assert!(Inventory::new(&module).getters.is_empty());
        assert!(!module.ffi_trampolines.is_empty());
        crate::pruning::prune_hir(&mut module,&[]);
        let inventory=Inventory::new(&module);assert_eq!(inventory.getters.len(),2);
        let main=module.functions.iter().find(|f|f.symbol=="dart_main").unwrap();
        let calls:Vec<_>=main.blocks.iter().flat_map(|b|&b.instructions).filter_map(|(value,instruction,_)| match instruction {
            Instruction::CallStatic { symbol,.. } if module.simple_enum_constants.contains_key(symbol)=>Some(*value),_=>None,
        }).collect();
        assert_eq!(calls.len(),2);assert!(calls.iter().all(|value|inventory.token_for_call(main,*value).is_some()));
        let mut clone = main.clone();
        assert_eq!(inventory.fold_identity_comparisons(&mut clone, &HashMap::new()), 1,
            "real warm/cold enum phis must reach the existing identity comparison");
        // Three independently lowered chunks with the same declaration ownership
        // permanently poison the premise; a third join cannot revive it.
        let mut combined=Module::new();
        for _ in 0..3 { let mut chunk=Module::new();chunk.simple_enum_constants=module.simple_enum_constants.clone();crate::lower::join_arc_facts(&mut combined,&mut chunk); }
        assert!(combined.simple_enum_constants.values().all(|premise|premise.getter.is_empty()));
    }).unwrap().join().unwrap();
}

/// Model the real lazy access: guarded warm load or checked cold getter, then phi.
fn cached_caller() -> Module {
    let mut module = fixture();
    let caller = &mut module.functions[2];
    let mut cold = caller.blocks[0].clone();
    cold.id = BlockId(4);
    let mut success = caller.blocks[1].clone();
    success.terminator = Terminator::Branch(BlockId(5));
    let error = caller.blocks[2].clone();
    let entry = BasicBlock {
        id: BlockId(0),
        instructions: vec![
            (
                ValueId(20),
                Instruction::LoadGlobal {
                    symbol: "cache0$ok".into(),
                    ty: Type::I8,
                },
                Type::I8,
            ),
            (
                ValueId(21),
                Instruction::ICmp(ICmpOp::Eq, Operand::Val(ValueId(20)), constant(1)),
                Type::I1,
            ),
        ],
        terminator: Terminator::CondBranch {
            cond: Operand::Val(ValueId(21)),
            then_block: BlockId(3),
            else_block: BlockId(4),
        },
    };
    let warm = BasicBlock {
        id: BlockId(3),
        instructions: vec![(
            ValueId(22),
            Instruction::LoadGlobal {
                symbol: "cache0".into(),
                ty: Type::Ref,
            },
            Type::Ref,
        )],
        terminator: Terminator::Branch(BlockId(5)),
    };
    let join = BasicBlock {
        id: BlockId(5),
        instructions: vec![
            (
                ValueId(23),
                Instruction::Phi {
                    incoming: vec![
                        (BlockId(3), Operand::Val(ValueId(22))),
                        (BlockId(1), Operand::Val(ValueId(0))),
                    ],
                    ty: Type::Ref,
                },
                Type::Ref,
            ),
            (
                ValueId(24),
                Instruction::ICmp(
                    ICmpOp::Eq,
                    Operand::Val(ValueId(23)),
                    Operand::Val(ValueId(23)),
                ),
                Type::I1,
            ),
        ],
        terminator: Terminator::Return(Some(Operand::Val(ValueId(24)))),
    };
    caller.return_ty = Type::I1;
    caller.blocks = vec![entry, cold, success, error, warm, join];
    module
}

#[test]
fn lazy_cache_paths_merge_only_after_edge_success_and_keep_real_evaluation() {
    let module = cached_caller();
    let inventory = Inventory::new(&module);
    let caller = &module.functions[2];
    let cold = inventory.token_for_call(caller, ValueId(0)).unwrap();
    assert_eq!(
        inventory.token_for_call(caller, ValueId(22)),
        Some(cold.clone())
    );
    assert_eq!(inventory.token_for_call(caller, ValueId(23)), Some(cold));
    let mut clone = caller.clone();
    let before = format!("{:?}", &clone.blocks[..5]);
    assert_eq!(
        inventory.fold_identity_comparisons(&mut clone, &HashMap::new()),
        1
    );
    assert_eq!(
        format!("{:?}", &clone.blocks[..5]),
        before,
        "getter, warm read and pending check must stay intact"
    );
    assert!(matches!(
        clone.blocks[5].instructions[1].1,
        Instruction::Const(Constant::Bool(true))
    ));
    // Reordering the physical block array cannot change edge semantics.
    let mut shuffled = caller.clone();
    shuffled.blocks[1..].reverse();
    assert_eq!(
        inventory.fold_identity_comparisons(&mut shuffled, &HashMap::new()),
        1
    );
}

#[test]
fn cache_guards_phi_edges_and_exceptional_uses_cannot_forge_a_token() {
    for variant in 0..12 {
        let mut module = cached_caller();
        let caller = &mut module.functions[2];
        match variant {
            0 => {
                if let Instruction::LoadGlobal { symbol, .. } =
                    &mut caller.blocks[0].instructions[0].1
                {
                    *symbol = "cache1$ok".into();
                }
            }
            1 => {
                if let Instruction::ICmp(_, _, right) = &mut caller.blocks[0].instructions[1].1 {
                    *right = constant(0);
                }
            }
            2 => {
                if let Terminator::CondBranch {
                    then_block,
                    else_block,
                    ..
                } = &mut caller.blocks[0].terminator
                {
                    std::mem::swap(then_block, else_block);
                }
            }
            3 => {
                if let Instruction::CallStatic { symbol, .. } =
                    &mut caller.blocks[1].instructions[0].1
                {
                    *symbol = "getter1".into();
                }
            }
            4 => {
                if let Instruction::Phi { incoming, .. } = &mut caller.blocks[5].instructions[0].1 {
                    incoming[1].1 = Operand::Constant(Constant::Null);
                }
            }
            5 => caller.blocks[3].instructions.push(call(
                30,
                "observe",
                vec![(Operand::Val(ValueId(0)), Type::Ref)],
                Type::Void,
            )),
            6 => {
                if let Instruction::Phi { incoming, .. } = &mut caller.blocks[5].instructions[0].1 {
                    incoming[1].0 = incoming[0].0;
                }
            }
            7 => {
                if let Instruction::Phi { incoming, .. } = &mut caller.blocks[5].instructions[0].1 {
                    incoming[1].1 = Operand::Val(ValueId(23));
                }
            }
            8 => caller.blocks[4].instructions[0].0 = ValueId(23),
            9 => caller.blocks[4].terminator = Terminator::Branch(BlockId(1)),
            10 => {
                if let Terminator::CondBranch {
                    then_block,
                    else_block,
                    ..
                } = &mut caller.blocks[1].terminator
                {
                    *then_block = *else_block;
                }
            }
            _ => caller.blocks[4].instructions.insert(0, call(32, "observe", vec![(Operand::Val(ValueId(22)), Type::Ref)], Type::Void)),
        }
        let inventory = Inventory::new(&module);
        assert!(
            inventory
                .token_for_call(&module.functions[2], ValueId(23))
                .is_none(),
            "variant {variant}"
        );
        let mut clone = module.functions[2].clone();
        assert_eq!(
            inventory.fold_identity_comparisons(&mut clone, &HashMap::new()),
            0,
            "variant {variant}"
        );
    }
    let module = cached_caller();
    let inventory = Inventory::new(&module);
    let mut clone = module.functions[2].clone();
    clone.blocks[4].instructions.push((
        ValueId(31),
        Instruction::StoreGlobal {
            symbol: "cache0".into(),
            val: Operand::Constant(Constant::Null),
            ty: Type::Ref,
            root: Some(0),
        },
        Type::Void,
    ));
    assert_eq!(
        inventory.fold_identity_comparisons(&mut clone, &HashMap::new()),
        0
    );
}

#[test]
fn pending_success_edge_can_feed_a_phi_without_an_intermediate_block() {
    let mut module = fixture();
    let caller = &mut module.functions[2];
    caller.blocks[1].instructions.push((
        ValueId(10),
        Instruction::Phi {
            incoming: vec![(BlockId(0), Operand::Val(ValueId(0)))],
            ty: Type::Ref,
        },
        Type::Ref,
    ));
    caller.blocks[1].terminator = Terminator::Return(Some(Operand::Val(ValueId(10))));
    let inventory = Inventory::new(&module);
    let caller = &module.functions[2];
    assert!(inventory.token_for_call(caller, ValueId(0)).is_some());
    assert_eq!(
        inventory.token_for_call(caller, ValueId(0)),
        inventory.token_for_call(caller, ValueId(10))
    );
}

#[test]
fn guarded_cache_load_can_share_a_block_with_other_work() {
    let mut module = cached_caller();
    let warm = &mut module.functions[2].blocks[4];
    warm.instructions.insert(
        0,
        (ValueId(30), Instruction::Const(Constant::Int(7)), Type::I64),
    );
    warm.instructions
        .push((ValueId(31), Instruction::Const(Constant::Int(8)), Type::I64));
    let inventory = Inventory::new(&module);
    assert!(
        inventory
            .token_for_call(&module.functions[2], ValueId(23))
            .is_some()
    );
}
