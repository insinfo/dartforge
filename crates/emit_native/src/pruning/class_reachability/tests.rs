use super::*;

fn function(symbol: &str, instructions: Vec<(ValueId, Instruction, Type)>) -> Function {
    Function {
        symbol: symbol.into(),
        name: symbol.into(),
        params: vec![],
        return_ty: Type::Void,
        debug_info: None,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions,
            terminator: Terminator::Return(None),
        }],
    }
}
fn fixture() -> Module {
    let mut module = Module::new();
    module.arc_fixed_ancestry = true;
    module.sdk_mode = true;
    module.entry_symbol = Some("df.test.main".into());
    module.functions.push(function(
        "df.test.main",
        vec![(
            ValueId(0),
            Instruction::AllocObject {
                class_id: 300,
                fields: vec![],
            },
            Type::Ref,
        )],
    ));
    for (cid, name) in [(300, "Used"), (301, "Unused"), (302, "Nested")] {
        let method = format!("df.{name}.method");
        let text = format!("df.{name}.toString");
        module.classes.push(ClassDef {
            id: cid,
            name: name.into(),
            field_count: 0,
            vtable: vec![(1, method.clone())],
            to_string_symbol: Some(text.clone()),
        });
        module.pruning_class_ids.insert(cid);
        let table = format!("df.mt.{name}");
        module.table_functions.insert(cid, table.clone());
        module.method_tables.push((
            cid,
            table,
            vec![
                ("c:used".into(), method.clone()),
                ("c:toString".into(), text.clone()),
            ],
        ));
        let mut body = function(&method, vec![]);
        body.return_ty = Type::Ref;
        body.params = vec![
            (ValueId(0), "self".into(), Type::Ref),
            (ValueId(1), "args".into(), Type::Ptr),
            (ValueId(2), "descriptor".into(), Type::Ptr),
        ];
        body.blocks[0].terminator = Terminator::Return(Some(Operand::Constant(Constant::Null)));
        module.functions.push(body);
        let mut body = function(&text, vec![]);
        body.return_ty = Type::Ref;
        body.params = vec![(ValueId(0), "self".into(), Type::Ref)];
        body.blocks[0].terminator = Terminator::Return(Some(Operand::Constant(Constant::Null)));
        module.functions.push(body);
    }
    module
}
fn apply(module: &mut Module) -> bool {
    let Some(reach) = narrow_enabled(module, &[]) else {
        return false;
    };
    let index: HashMap<_, _> = reach
        .names
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i as u32))
        .collect();
    module.functions.retain(|f| {
        index
            .get(f.symbol.as_str())
            .is_some_and(|&node| reach.live(node))
    });
    true
}
#[test]
fn unallocated_tables_protocol_methods_and_emitted_dispatch_disappear() {
    let mut module = fixture();
    assert!(apply(&mut module));
    assert_eq!(
        module.closed_class_tables.as_ref().unwrap(),
        &HashSet::from([300])
    );
    assert!(
        module
            .functions
            .iter()
            .any(|f| f.symbol == "df.Used.toString")
    );
    assert!(
        !module
            .functions
            .iter()
            .any(|f| f.symbol == "df.Unused.toString")
    );
    let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
    assert!(ir.contains("call void @dartforge_register_table(i64 300"));
    assert!(!ir.contains("call void @dartforge_register_table(i64 301"));
    assert!(!ir.contains("call void @dartforge_register_table(i64 302"));
    assert!(!ir.contains("call i64 @df.Unused.toString("));
    module.sdk_mode = false;
    let legacy_ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
    assert!(legacy_ir.contains("call i64 @df.Used.toString("));
    assert!(!legacy_ir.contains("call i64 @df.Unused.toString("));
    assert!(
        module.classes.iter().any(|c| c.id == 301),
        "nominal metadata remains available"
    );
}
#[test]
fn inherited_implementation_transitively_activates_its_allocated_class() {
    let mut module = fixture();
    module.subtyping_edges.push((300, 301));
    module.classes[0].vtable.push((2, "df.inherited".into()));
    module.functions.push(function(
        "df.inherited",
        vec![(
            ValueId(0),
            Instruction::AllocObject {
                class_id: 302,
                fields: vec![],
            },
            Type::Ref,
        )],
    ));
    assert!(apply(&mut module));
    assert_eq!(
        module.closed_class_tables.as_ref().unwrap(),
        &HashSet::from([300, 302])
    );
    assert!(module.functions.iter().any(|f| f.symbol == "df.inherited"));
    assert!(
        module
            .functions
            .iter()
            .any(|f| f.symbol == "df.Nested.toString")
    );
}
#[test]
fn unknown_creation_exposure_and_entry_inputs_keep_original_wide_behavior() {
    for variant in 0..8 {
        let mut module = fixture();
        match variant {
            0 => module.functions[0].blocks[0].instructions.push((
                ValueId(1),
                Instruction::NativeCall {
                    target: Operand::Constant(Constant::Int(1)),
                    args: vec![],
                    ret: CType::Void,
                },
                Type::Void,
            )),
            1 => module.functions[0].blocks[0].instructions.push((
                ValueId(1),
                Instruction::CallRuntime {
                    name: "unproved_factory".into(),
                    args: vec![],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )),
            2 => module.functions[0].blocks[0].instructions.push((
                ValueId(1),
                Instruction::TearOff {
                    code_symbol: "df.Unused.method".into(),
                },
                Type::Ref,
            )),
            3 => module.sdk_records.push("sdk.foreign".into()),
            4 => module.arc_fixed_ancestry = false,
            5 => module
                .pruning_ambiguous_class_ids
                .insert(300)
                .then_some(())
                .unwrap(),
            6 => module.functions[0]
                .params
                .push((ValueId(99), "foreign".into(), Type::Ref)),
            _ => module.functions[0].blocks[0].instructions.push((
                ValueId(1),
                Instruction::CallRuntime {
                    name: "dartforge_object_new".into(),
                    args: vec![
                        (Operand::Val(ValueId(0)), Type::I64),
                        (Operand::Constant(Constant::Int(0)), Type::I64),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )),
        }
        assert!(!apply(&mut module), "unknown variant{variant}");
        assert!(module.closed_class_tables.is_none());
        let summary = super::super::hir_summary(&module);
        let reach = super::super::reach(&[&summary], true);
        let node = reach
            .names
            .iter()
            .position(|s| s == "df.Unused.toString")
            .unwrap() as u32;
        assert!(reach.live(node), "original unconditional roots remain");
    }
}
#[test]
fn inactive_foreign_body_is_not_mistaken_for_an_active_exposure() {
    let mut module = fixture();
    module.functions.push(function(
        "df.dead.native",
        vec![(
            ValueId(0),
            Instruction::NativeCall {
                target: Operand::Constant(Constant::Int(1)),
                args: vec![],
                ret: CType::Void,
            },
            Type::Void,
        )],
    ));
    assert!(apply(&mut module));
    assert!(
        !module
            .functions
            .iter()
            .any(|f| f.symbol == "df.dead.native")
    );
    let mut module = fixture();
    module.functions[0].blocks[0].instructions.push((
        ValueId(1),
        Instruction::CallStatic {
            symbol: "df.dead.native".into(),
            args: vec![],
            ret_ty: Type::Void,
        },
        Type::Void,
    ));
    assert!(
        !apply(&mut module),
        "an unresolved call cannot be certified neutral"
    );
}
