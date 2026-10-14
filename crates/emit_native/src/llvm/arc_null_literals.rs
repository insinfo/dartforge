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
    assert!(!ir.contains("call void @dartforge_arc_retain(i64 0)"));
    assert!(!ir.contains("call void @dartforge_arc_release(i64 0)"));
    assert_eq!(format!("{:?}", module.functions), before);
    assert_eq!(format!("{classes:?}/{plan:?}"), verified);
}

#[test]
fn nullable_and_known_null_ssa_still_emit_owner_calls() {
    let module = module(Operand::Val(ValueId(0)), Some(Type::Ref));
    let ir = LlvmEmitter::new(&module).emit_all();
    assert!(ir.contains("call void @dartforge_arc_retain(i64 %v0)"));
    assert!(ir.contains("call void @dartforge_arc_release(i64 %v0)"));
    let mut module = module;
    module.functions[0].params.clear();
    module.functions[0].blocks[0].instructions.insert(
        0,
        (ValueId(0), Instruction::Const(Constant::Null), Type::Ref),
    );
    let ir = LlvmEmitter::new(&module).emit_all();
    assert!(ir.contains("call void @dartforge_arc_retain(i64 %v0)"));
    assert!(ir.contains("call void @dartforge_arc_release(i64 %v0)"));
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
