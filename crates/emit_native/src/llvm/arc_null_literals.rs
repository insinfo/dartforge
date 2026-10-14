use super::*;

fn module(value: Operand, parameter_type: Option<Type>) -> Module {
    let mut module = Module::new();
    module.arc_memory = true;
    module.functions.push(Function {
        symbol: "null_operations".into(),
        name: "null_operations".into(),
        debug_info: None,
        params: parameter_type
            .map(|ty| vec![(ValueId(0), "value".into(), ty)])
            .unwrap_or_default(),
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(1),
                    Instruction::ArcCopy {
                        value: value.clone(),
                    },
                    Type::Ref,
                ),
                (ValueId(2), Instruction::ArcDrop { value }, Type::Void),
            ],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
        }],
    });
    module
}

#[test]
fn literal_null_elides_physical_calls_but_preserves_copy_ssa_and_hir() {
    let module = module(Operand::Constant(Constant::Null), None);
    let mut classes = std::collections::HashMap::new();
    let mut plan = crate::optimize::arc::TokenPlan {
        ret: crate::optimize::arc::TokenReturn::Owned,
        ..Default::default()
    };
    crate::optimize::arc::produce_and_verify_tokens(
        &module.functions[0],
        &mut classes,
        &mut plan,
        &FunctionTables::default(),
        &crate::optimize::arc::ScopePlan::default(),
    )
    .unwrap();
    assert_eq!(classes[&ValueId(1)], crate::optimize::arc::Ownership::Owned);
    let verified = format!("{classes:?}/{plan:?}");
    let before = format!("{:?}", module.functions);
    let ir = LlvmEmitter::new(&module).emit_all();
    assert!(ir.contains("%v1 = add i64 0, 0"));
    assert!(ir.contains("ret i64 %v1"));
    assert_no_physical_calls(&ir, "0");
    assert_eq!(format!("{:?}", module.functions), before);
    assert_eq!(format!("{classes:?}/{plan:?}"), verified);
}

#[test]
fn nullable_reference_keeps_calls_but_exact_null_ssa_elides_them() {
    let module = module(Operand::Val(ValueId(0)), Some(Type::Ref));
    let ir = LlvmEmitter::new(&module).emit_all();
    assert!(ir.contains("call void @df.arc_retain_ref(i64 %v0)"));
    assert!(ir.contains("call void @df.arc_release_ref(i64 %v0)"));
    let mut module = module;
    module.functions[0].params.clear();
    module.functions[0].blocks[0].instructions.insert(
        0,
        (ValueId(0), Instruction::Const(Constant::Null), Type::Ref),
    );
    let ir = LlvmEmitter::new(&module).emit_all();
    assert_no_physical_calls(&ir, "%v0");
}

fn produced(instruction: Instruction, ty: Type) -> Module {
    let mut module = module(Operand::Val(ValueId(0)), None);
    module.functions[0].blocks[0]
        .instructions
        .insert(0, (ValueId(0), instruction, ty));
    module
}

fn emits_calls(module: &Module) -> bool {
    let ir = LlvmEmitter::new(module).emit_all();
    ir.contains("call void @df.arc_retain_ref(i64 %v0)")
        && ir.contains("call void @df.arc_release_ref(i64 %v0)")
}

fn assert_no_physical_calls(ir: &str, operand: &str) {
    for target in [
        "dartforge_arc_retain",
        "dartforge_arc_release",
        super::arc_tag_guards::RETAIN,
        super::arc_tag_guards::RELEASE,
    ] {
        assert!(!ir.contains(&format!("call void @{target}(i64 {operand})")));
    }
}

#[test]
fn boxed_smi_boundaries_elide_calls_without_changing_verified_tokens() {
    for n in [layout::smi::MIN, -1, 0, 1, layout::smi::MAX] {
        let module = produced(
            Instruction::Box {
                op: Operand::Constant(Constant::Int(n)),
                from: Type::I64,
            },
            Type::Ref,
        );
        let mut classes = std::collections::HashMap::new();
        classes.insert(ValueId(0), crate::optimize::arc::Ownership::Owned);
        let mut plan = crate::optimize::arc::TokenPlan {
            ret: crate::optimize::arc::TokenReturn::Owned,
            ..Default::default()
        };
        plan.instructions
            .insert(ValueId(0), crate::optimize::arc::TokenEffect::default());
        crate::optimize::arc::produce_and_verify_tokens(
            &module.functions[0],
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &crate::optimize::arc::ScopePlan::default(),
        )
        .unwrap();
        let snapshot = format!("{:?}/{classes:?}/{plan:?}", module.functions);
        let ir = LlvmEmitter::new(&module).emit_all();
        assert!(ir.contains("%v0 = call i64 @df.box_int"));
        assert!(ir.contains("%v1 = add i64 %v0, 0"));
        assert_no_physical_calls(&ir, "%v0");
        assert_eq!(
            format!("{:?}/{classes:?}/{plan:?}", module.functions),
            snapshot
        );
    }
    for n in [
        layout::smi::MIN - 1,
        layout::smi::MAX + 1,
        i64::MIN,
        i64::MAX,
    ] {
        assert!(emits_calls(&produced(
            Instruction::Box {
                op: Operand::Constant(Constant::Int(n)),
                from: Type::I64
            },
            Type::Ref
        )));
    }
}

#[test]
fn boolean_static_boxes_and_dynamic_scalars_keep_runtime_inventory_calls() {
    for b in [false, true] {
        assert!(emits_calls(&produced(
            Instruction::Box {
                op: Operand::Constant(Constant::Bool(b)),
                from: Type::I1
            },
            Type::Ref
        )));
    }
    for n in [0, 1, 255] {
        assert!(emits_calls(&produced(
            Instruction::Box {
                op: Operand::Constant(Constant::Int(n)),
                from: Type::I8
            },
            Type::Ref
        )));
    }
    let mut dynamic = produced(
        Instruction::Box {
            op: Operand::Val(ValueId(3)),
            from: Type::I64,
        },
        Type::Ref,
    );
    dynamic.functions[0]
        .params
        .push((ValueId(3), "integer".into(), Type::I64));
    assert!(emits_calls(&dynamic));
    assert!(emits_calls(&produced(
        Instruction::Box {
            op: Operand::Constant(Constant::Double(0.0)),
            from: Type::F64
        },
        Type::Ref
    )));
    assert!(emits_calls(&produced(
        Instruction::Const(Constant::String("permanent string".into())),
        Type::Ref
    )));
}

#[test]
fn physical_elision_preserves_intervening_collection_call_and_box_producer() {
    let mut module = produced(
        Instruction::Box {
            op: Operand::Constant(Constant::Int(17)),
            from: Type::I64,
        },
        Type::Ref,
    );
    module.functions[0].blocks[0].instructions.insert(
        1,
        (
            ValueId(3),
            Instruction::CallRuntime {
                name: "dartforge_arc_collect".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        ),
    );
    let ir = LlvmEmitter::new(&module).emit_all();
    assert!(ir.contains("%v0 = call i64 @df.box_int(i64 17)"));
    assert!(ir.contains("call void @dartforge_arc_collect()"));
    assert!(ir.contains("%v1 = add i64 %v0, 0"));
    assert_no_physical_calls(&ir, "%v0");
}

#[test]
fn earlier_i64_constant_and_copy_move_chain_are_physical_facts_only() {
    let mut module = produced(
        Instruction::Box {
            op: Operand::Val(ValueId(3)),
            from: Type::I64,
        },
        Type::Ref,
    );
    module.functions[0].blocks[0].instructions.insert(
        0,
        (ValueId(3), Instruction::Const(Constant::Int(17)), Type::I64),
    );
    module.functions[0].blocks[0].instructions.extend([
        (
            ValueId(4),
            Instruction::ArcMove {
                value: Operand::Val(ValueId(1)),
            },
            Type::Ref,
        ),
        (
            ValueId(5),
            Instruction::ArcCopy {
                value: Operand::Val(ValueId(4)),
            },
            Type::Ref,
        ),
        (
            ValueId(6),
            Instruction::ArcDrop {
                value: Operand::Val(ValueId(5)),
            },
            Type::Void,
        ),
    ]);
    let ir = LlvmEmitter::new(&module).emit_all();
    for value in [0, 4, 5] {
        assert_no_physical_calls(&ir, &format!("%v{value}"));
    }
    assert!(ir.contains("%v4 = add i64 %v1, 0"));
    assert!(ir.contains("%v5 = add i64 %v4, 0"));
}

#[test]
fn canonical_owned_box_requires_exact_abi_and_no_hir_homonym() {
    let instruction = Instruction::CallRuntime {
        name: "dartforge_arc_box_int_owned_v1".into(),
        args: vec![(Operand::Constant(Constant::Int(31)), Type::I64)],
        ret_ty: Type::Ref,
    };
    let mut module = produced(instruction.clone(), Type::Ref);
    assert_no_physical_calls(&LlvmEmitter::new(&module).emit_all(), "%v0");
    let mut homonym = module.functions[0].clone();
    homonym.symbol = "dartforge_arc_box_int_owned_v1".into();
    homonym.params = vec![(ValueId(10), "x".into(), Type::I64)];
    homonym.blocks = vec![BasicBlock {
        id: BlockId(0),
        instructions: vec![],
        terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
    }];
    module.functions.push(homonym);
    assert!(emits_calls(&module));
    for args in [
        vec![],
        vec![(Operand::Constant(Constant::Int(31)), Type::I8)],
        vec![(Operand::Constant(Constant::Int(31)), Type::I64); 2],
        vec![(Operand::Constant(Constant::Bool(true)), Type::I64)],
    ] {
        let mut malformed = produced(instruction.clone(), Type::Ref);
        if let Instruction::CallRuntime { args: current, .. } =
            &mut malformed.functions[0].blocks[0].instructions[0].1
        {
            *current = args;
        }
        let proof = super::arc_immediates::ImmediateRefs::new(&malformed.functions[0], true);
        assert!(!proof.contains(&Operand::Val(ValueId(0)), BlockId(0), 1));
    }
}

#[test]
fn malformed_types_order_duplicate_ids_and_cross_block_facts_are_refused() {
    let box_integer = Instruction::Box {
        op: Operand::Constant(Constant::Int(9)),
        from: Type::I64,
    };
    let base = produced(box_integer.clone(), Type::Ref).functions.remove(0);
    let knows = |function: &Function| {
        super::arc_immediates::ImmediateRefs::new(function, true).contains(
            &Operand::Val(ValueId(0)),
            BlockId(0),
            10,
        )
    };
    let mut wrong_type = base.clone();
    wrong_type.blocks[0].instructions[0].2 = Type::I64;
    assert!(!knows(&wrong_type));
    let mut duplicate = base.clone();
    duplicate.blocks[0]
        .instructions
        .push((ValueId(0), box_integer.clone(), Type::Ref));
    assert!(!knows(&duplicate));
    let mut parameter = base.clone();
    parameter
        .params
        .push((ValueId(0), "shadow".into(), Type::Ref));
    assert!(!knows(&parameter));
    let mut duplicate_block = base.clone();
    duplicate_block.blocks.push(BasicBlock {
        id: BlockId(0),
        instructions: vec![],
        terminator: Terminator::Return(None),
    });
    assert!(!knows(&duplicate_block));
    let proof = super::arc_immediates::ImmediateRefs::new(&base, true);
    assert!(!proof.contains(&Operand::Val(ValueId(0)), BlockId(0), 0));
    assert!(!proof.contains(&Operand::Val(ValueId(0)), BlockId(1), 10));
    for ty in [Type::I1, Type::I8, Type::Ref] {
        let mut wrong_scalar = base.clone();
        wrong_scalar.blocks[0].instructions[0].1 = Instruction::Box {
            op: Operand::Val(ValueId(3)),
            from: Type::I64,
        };
        wrong_scalar.blocks[0]
            .instructions
            .insert(0, (ValueId(3), Instruction::Const(Constant::Int(9)), ty));
        assert!(!knows(&wrong_scalar));
    }
    let mut future_scalar = base.clone();
    future_scalar.blocks[0].instructions[0].1 = Instruction::Box {
        op: Operand::Val(ValueId(3)),
        from: Type::I64,
    };
    future_scalar.blocks[0].instructions.push((
        ValueId(3),
        Instruction::Const(Constant::Int(9)),
        Type::I64,
    ));
    assert!(!knows(&future_scalar));
    let mut wrong_return = base.clone();
    wrong_return.blocks[0].instructions[0].1 = Instruction::CallRuntime {
        name: "dartforge_arc_box_int_owned_v1".into(),
        args: vec![(Operand::Constant(Constant::Int(9)), Type::I64)],
        ret_ty: Type::I64,
    };
    assert!(!knows(&wrong_return));
}

#[test]
fn invalid_scalar_operands_are_not_silently_reinterpreted_as_null() {
    for value in [
        Operand::Constant(Constant::Int(0)),
        Operand::Constant(Constant::Bool(false)),
    ] {
        let module = module(value, None);
        assert!(std::panic::catch_unwind(|| LlvmEmitter::new(&module).emit_all()).is_err());
    }
    let module = module(Operand::Val(ValueId(0)), Some(Type::I64));
    assert!(std::panic::catch_unwind(|| LlvmEmitter::new(&module).emit_all()).is_err());
}
