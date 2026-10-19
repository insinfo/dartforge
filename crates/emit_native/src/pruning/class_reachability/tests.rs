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
        let text_adapter = format!("{text}$c");
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
                ("c:toString".into(), text_adapter.clone()),
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
        let mut adapter = function(&text_adapter, vec![]);
        adapter.return_ty = Type::Ref;
        adapter.params = vec![
            (ValueId(0), "self".into(), Type::Ref),
            (ValueId(1), "args".into(), Type::Ptr),
            (ValueId(2), "descriptor".into(), Type::Ptr),
        ];
        adapter.blocks[0].terminator = Terminator::Return(Some(Operand::Constant(Constant::Null)));
        module.functions.push(adapter);
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
    assert!(!ir.contains("define ptr @df.mt.Unused("));
    assert!(!ir.contains("@df.mt.Unused$d"));
    assert!(!ir.contains("define ptr @df.mt.Nested("));
    assert!(!ir.contains("@df.mt.Nested$d"));
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
                    code_symbol: "df.missing.closure".into(),
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

/// The closure body allocates a second class; its table must survive even when
/// the only path to that allocation is an indirect uniform closure invocation.
fn closure_fixture(tearoff: bool) -> Module {
    let mut module = fixture();
    let target = "df.test.create";
    let mut body = function(
        target,
        vec![(
            ValueId(3),
            Instruction::AllocObject {
                class_id: 302,
                fields: vec![],
            },
            Type::Ref,
        )],
    );
    body.params = vec![
        (ValueId(0), "closure".into(), Type::Ref),
        (ValueId(1), "args".into(), Type::Ptr),
        (ValueId(2), "descriptor".into(), Type::Ptr),
    ];
    body.return_ty = Type::Ref;
    body.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(3))));
    module.functions.push(body);
    module.functions[0].blocks[0].instructions.extend([
        (
            ValueId(1),
            if tearoff {
                Instruction::TearOff {
                    code_symbol: target.into(),
                }
            } else {
                Instruction::AllocClosure {
                    code_symbol: target.into(),
                    env: Operand::Constant(Constant::Null),
                }
            },
            Type::Ref,
        ),
        (
            ValueId(2),
            Instruction::CallClosure {
                closure: Operand::Val(ValueId(1)),
                args: vec![],
                names: vec![],
                ret_ty: Type::Ref,
                tuple_types: Operand::Constant(Constant::Null),
            },
            Type::Ref,
        ),
    ]);
    module
}

#[test]
fn bounded_uniform_closures_keep_transitive_allocations_and_drop_unrelated_tables() {
    for tearoff in [false, true] {
        let mut module = closure_fixture(tearoff);
        assert!(apply(&mut module), "tearoff={tearoff}");
        assert_eq!(
            module.closed_class_tables.as_ref().unwrap(),
            &HashSet::from([300, 302])
        );
        let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
        assert!(ir.contains("define ptr @df.mt.Nested("));
        assert!(!ir.contains("define ptr @df.mt.Unused("));
    }
}

fn typed_closure_fixture(runtime_reads: bool) -> Module {
    let mut module = closure_fixture(false);
    let typed = "df.test.create.typed";
    let mut body = function(
        typed,
        vec![(
            ValueId(1),
            Instruction::AllocObject {
                class_id: 302,
                fields: vec![],
            },
            Type::Ref,
        )],
    );
    body.params = vec![(ValueId(0), "environment".into(), Type::Ref)];
    body.return_ty = Type::Ref;
    body.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
    module.functions.push(body);
    let instructions = &mut module.functions[0].blocks[0].instructions;
    instructions[1].1 = Instruction::AllocClosureTyped {
        code_symbol: "df.test.create".into(),
        env: Operand::Constant(Constant::Null),
        typed: typed.into(),
        abi: 6,
        direct: false,
    };
    let read = |index, ty| {
        if runtime_reads {
            Instruction::CallRuntime {
                name: "dartforge_object_get".into(),
                args: vec![
                    (Operand::Val(ValueId(1)), Type::Ref),
                    (Operand::Constant(Constant::Int(index)), Type::I64),
                ],
                ret_ty: ty,
            }
        } else {
            Instruction::GetField {
                object: Operand::Val(ValueId(1)),
                index: index as usize,
            }
        }
    };
    instructions[2] = (ValueId(2), read(2, Type::I64), Type::I64);
    instructions.push((ValueId(3), read(1, Type::Ref), Type::Ref));
    instructions.push((
        ValueId(4),
        Instruction::TypedCall {
            target: Operand::Val(ValueId(2)),
            args: vec![(Operand::Val(ValueId(3)), Type::Ref)],
            ret: Type::Ref,
        },
        Type::Ref,
    ));
    module
}

#[test]
fn local_typed_closure_entries_have_bounded_allocation_reachability() {
    for runtime_reads in [false, true] {
        let mut module = typed_closure_fixture(runtime_reads);
        assert!(apply(&mut module), "runtime reads={runtime_reads}");
        assert_eq!(
            module.closed_class_tables.as_ref().unwrap(),
            &HashSet::from([300, 302])
        );
        let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
        assert!(ir.contains("define ptr @df.mt.Nested("));
        assert!(!ir.contains("define ptr @df.mt.Unused("));
    }
}

fn typed_scalar_argument(module: &mut Module, ty: Type, argument: Operand, abi: i64) {
    module
        .functions
        .last_mut()
        .unwrap()
        .params
        .push((ValueId(2), "argument".into(), ty));
    if let Instruction::AllocClosureTyped { abi: value, .. } =
        &mut module.functions[0].blocks[0].instructions[1].1
    {
        *value = abi;
    }
    if let Instruction::TypedCall { args, .. } =
        &mut module.functions[0].blocks[0].instructions[4].1
    {
        args.push((argument, ty));
    }
}

fn typed_scalar_result(module: &mut Module, ty: Type, value: Constant, abi: i64) {
    let body = module.functions.last_mut().unwrap();
    body.return_ty = ty;
    body.blocks[0].instructions[0] = (ValueId(1), Instruction::Const(value), ty);
    if let Instruction::AllocClosureTyped { abi: value, .. } =
        &mut module.functions[0].blocks[0].instructions[1].1
    {
        *value = abi;
    }
    let call = &mut module.functions[0].blocks[0].instructions[4];
    if let Instruction::TypedCall { ret, .. } = &mut call.1 {
        *ret = ty;
    }
    call.2 = ty;
}

#[test]
fn typed_scalar_representations_match_the_encoded_entry_abi() {
    for (ty, argument, abi) in [
        (Type::I64, Operand::Constant(Constant::Int(7)), 36),
        (Type::F64, Operand::Constant(Constant::Double(2.5)), 41),
        (Type::I1, Operand::Constant(Constant::Bool(true)), 46),
    ] {
        let mut module = typed_closure_fixture(true);
        typed_scalar_argument(&mut module, ty, argument, abi);
        assert!(apply(&mut module), "{ty:?}");
    }
    for (ty, value, abi) in [
        (Type::I64, Constant::Int(7), 7),
        (Type::F64, Constant::Double(2.5), 8),
        (Type::I1, Constant::Bool(true), 9),
    ] {
        let mut module = typed_closure_fixture(false);
        typed_scalar_result(&mut module, ty, value, abi);
        assert!(apply(&mut module), "result {ty:?}");
    }
}

#[test]
fn typed_code_addresses_and_signatures_cannot_escape_or_be_forged() {
    for variant in 0..16 {
        let mut module = typed_closure_fixture(false);
        match variant {
            0 => {
                if let Instruction::AllocClosureTyped { abi, .. } =
                    &mut module.functions[0].blocks[0].instructions[1].1
                {
                    *abi = 7;
                }
            }
            1 => module.functions.last_mut().unwrap().params[0].2 = Type::I64,
            2 => module.functions.last_mut().unwrap().return_ty = Type::I64,
            3 => {
                if let Instruction::AllocClosureTyped { typed, .. } =
                    &mut module.functions[0].blocks[0].instructions[1].1
                {
                    *typed = "df.missing.typed".into();
                }
            }
            4 => {
                if let Instruction::TypedCall { args, .. } =
                    &mut module.functions[0].blocks[0].instructions[4].1
                {
                    args[0] = (Operand::Constant(Constant::Int(0)), Type::I64);
                }
            }
            5 => {
                if let Instruction::TypedCall { ret, .. } =
                    &mut module.functions[0].blocks[0].instructions[4].1
                {
                    *ret = Type::I64;
                }
            }
            6 => module.functions[0].blocks[0].instructions[2].2 = Type::Ref,
            7 => {
                if let Instruction::GetField { index, .. } =
                    &mut module.functions[0].blocks[0].instructions[2].1
                {
                    *index = 0;
                }
            }
            8 => {
                if let Instruction::TypedCall { target, .. } =
                    &mut module.functions[0].blocks[0].instructions[4].1
                {
                    *target = Operand::Constant(Constant::Int(42));
                }
            }
            9 => module.functions[0].blocks[0].instructions.push((
                ValueId(5),
                Instruction::SetField {
                    object: Operand::Val(ValueId(1)),
                    index: 2,
                    value: Operand::Constant(Constant::Int(0)),
                },
                Type::Void,
            )),
            10 => {
                module.globals.push((0, Type::I64, "df.address".into()));
                module.functions[0].blocks[0].instructions.push((
                    ValueId(5),
                    Instruction::StoreGlobal {
                        symbol: "df.address".into(),
                        val: Operand::Val(ValueId(2)),
                        ty: Type::I64,
                        root: None,
                    },
                    Type::Void,
                ));
            }
            11 => {
                if let Instruction::TypedCall { args, .. } =
                    &mut module.functions[0].blocks[0].instructions[4].1
                {
                    args[0] = (Operand::Val(ValueId(1)), Type::Ref);
                }
            }
            12 => module.functions[0].blocks[0].instructions.push((
                ValueId(5),
                Instruction::GetField {
                    object: Operand::Val(ValueId(2)),
                    index: 1,
                },
                Type::Ref,
            )),
            13 => {
                module.functions.last_mut().unwrap().params.push((
                    ValueId(2),
                    "unsupported".into(),
                    Type::Ptr,
                ));
            }
            14 => {
                let call = &mut module.functions[0].blocks[0].instructions[4];
                if let Instruction::TypedCall { ret, .. } = &mut call.1 {
                    *ret = Type::I64;
                }
                call.2 = Type::I64;
            }
            _ => {
                for index in 0..13 {
                    module.functions.last_mut().unwrap().params.push((
                        ValueId(index + 2),
                        format!("arg{index}"),
                        Type::I64,
                    ));
                }
            }
        }
        assert!(!apply(&mut module), "typed variant {variant}");
        assert!(module.closed_class_tables.is_none());
    }
}

#[test]
fn escaped_mutated_unresolved_or_mismatched_closures_keep_wide_registration() {
    for variant in 0..16 {
        let mut module = closure_fixture(variant % 2 == 0);
        let body = module.functions.last_mut().unwrap();
        match variant {
            0 => body.params[1].2 = Type::I64,
            1 => body.return_ty = Type::I64,
            // The closure's implicit receiver aliases its code-bearing object.
            2 => body.blocks[0].instructions.push((
                ValueId(4),
                Instruction::SetField {
                    object: Operand::Val(ValueId(0)),
                    index: 0,
                    value: Operand::Constant(Constant::Int(0)),
                },
                Type::Void,
            )),
            3 => {
                module.globals.push((7, Type::Ref, "dfg.closure".into()));
                module.functions[0].blocks[0].instructions.push((
                    ValueId(4),
                    Instruction::StoreGlobal {
                        symbol: "dfg.closure".into(),
                        val: Operand::Val(ValueId(1)),
                        ty: Type::Ref,
                        root: Some(7),
                    },
                    Type::Void,
                ));
            }
            4 => module.functions[0].blocks[0].instructions.push((
                ValueId(4),
                Instruction::SetField {
                    object: Operand::Val(ValueId(1)),
                    index: 0,
                    value: Operand::Constant(Constant::Int(0)),
                },
                Type::Void,
            )),
            5 => {
                if let Instruction::AllocClosure { code_symbol, .. } =
                    &mut module.functions[0].blocks[0].instructions[1].1
                {
                    *code_symbol = "sdk.unknown".into();
                }
            }
            6 => {
                if let Instruction::CallClosure { closure, .. } =
                    &mut module.functions[0].blocks[0].instructions[2].1
                {
                    *closure = Operand::Val(ValueId(0));
                }
            }
            7 => {
                if let Instruction::CallClosure { args, .. } =
                    &mut module.functions[0].blocks[0].instructions[2].1
                {
                    args.push(Operand::Val(ValueId(1)));
                }
            }
            8 => {
                if let Instruction::CallClosure { names, .. } =
                    &mut module.functions[0].blocks[0].instructions[2].1
                {
                    names.push("unexpected".into());
                }
            }
            9 => {
                if let Instruction::AllocClosure { env, .. } =
                    &mut module.functions[0].blocks[0].instructions[1].1
                {
                    *env = Operand::Constant(Constant::Int(1));
                }
            }
            10 => body.blocks[0].instructions.push((
                ValueId(4),
                Instruction::CallClosure {
                    closure: Operand::Val(ValueId(0)),
                    args: vec![],
                    names: vec![],
                    ret_ty: Type::Ref,
                    tuple_types: Operand::Constant(Constant::Null),
                },
                Type::Ref,
            )),
            11 => body.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(0)))),
            12 => body.blocks[0].instructions.push((
                ValueId(4),
                Instruction::GetField {
                    object: Operand::Val(ValueId(0)),
                    index: 0,
                },
                Type::Ref,
            )),
            13 => module.functions[0].blocks[0].instructions.extend([
                (ValueId(4), Instruction::Alloca(Type::I64), Type::Ptr),
                (
                    ValueId(5),
                    Instruction::CallStatic {
                        symbol: "df.test.create".into(),
                        args: vec![
                            Operand::Val(ValueId(1)),
                            Operand::Val(ValueId(4)),
                            Operand::Val(ValueId(4)),
                        ],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
            ]),
            14 => {
                if let Instruction::CallClosure { names, args, .. } =
                    &mut module.functions[0].blocks[0].instructions[2].1
                {
                    *names = vec!["duplicate".into(), "duplicate".into()];
                    *args = vec![Operand::Constant(Constant::Null); 2];
                }
            }
            _ => {
                module.functions[0].blocks[0].instructions[2].2 = Type::I64;
                if let Instruction::CallClosure { ret_ty, .. } =
                    &mut module.functions[0].blocks[0].instructions[2].1
                {
                    *ret_ty = Type::I64;
                }
            }
        }
        assert!(!apply(&mut module), "closure variant {variant}");
        assert!(module.closed_class_tables.is_none());
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

#[test]
fn unresolved_live_selector_target_cannot_close_the_image() {
    let mut module = fixture();
    module.method_tables[0].2[0].1 = "df.external.method".into();
    module.functions[0].blocks[0].instructions.push((
        ValueId(1),
        Instruction::CallSelector {
            selector: "c:used".into(),
            recv: Operand::Val(ValueId(0)),
            args: vec![],
            names: vec![],
            tuple_types: Operand::Constant(Constant::Null),
        },
        Type::Ref,
    ));
    assert!(!apply(&mut module));
    assert!(module.closed_class_tables.is_none());
}

#[test]
fn incompatible_static_call_does_not_prove_allocation_coverage() {
    let mut module = fixture();
    module.functions[0].blocks[0].instructions.push((
        ValueId(1),
        Instruction::CallStatic {
            symbol: "df.Used.method".into(),
            args: vec![],
            ret_ty: Type::Void,
        },
        Type::Void,
    ));
    assert!(!apply(&mut module));
    assert!(module.closed_class_tables.is_none());
}

#[test]
fn scalar_native_memory_access_does_not_close_managed_creation() {
    let mut module = fixture();
    module.functions[0].blocks[0].instructions.push((
        ValueId(1),
        Instruction::NativeWrite {
            address: Operand::Constant(Constant::Int(1)),
            index: Operand::Constant(Constant::Int(0)),
            ty: CType::I64,
            value: Operand::Constant(Constant::Int(301)),
        },
        Type::Void,
    ));
    assert!(!apply(&mut module));
    assert!(module.closed_class_tables.is_none());
}

#[test]
fn unknown_roots_function_addresses_and_symbol_collisions_keep_tables_wide() {
    for variant in 0..5 {
        let mut module = fixture();
        match variant {
            0 => module.entry_symbol = Some("df.missing.main".into()),
            1 => module.functions[0].blocks[0].instructions.push((
                ValueId(1),
                Instruction::FunctionTable(vec!["df.Used.method".into()]),
                Type::Ptr,
            )),
            2 => module.functions[0].blocks[0].instructions.push((
                ValueId(1),
                Instruction::Const(Constant::Function("df.Used.method".into())),
                Type::I64,
            )),
            3 => module.functions.push(function("df.mt.Used", vec![])),
            _ => module
                .helpers
                .push(("native binding".into(), "df.Used.method".into())),
        }
        assert!(!apply(&mut module), "variant {variant}");
        assert!(module.closed_class_tables.is_none());
    }
}

#[test]
fn malformed_bodies_and_uniform_entry_signatures_cannot_publish_a_certificate() {
    for variant in 0..4 {
        let mut module = fixture();
        match variant {
            0 => module.functions[0].blocks[0].instructions.push((
                ValueId(0),
                Instruction::Const(Constant::Null),
                Type::Ref,
            )),
            1 => module.functions[0].blocks[0].terminator = Terminator::Branch(BlockId(99)),
            2 => {
                let method = module
                    .functions
                    .iter_mut()
                    .find(|f| f.symbol == "df.Used.method")
                    .unwrap();
                method.params.pop();
                module.functions[0].blocks[0].instructions.push((
                    ValueId(1),
                    Instruction::CallSelector {
                        selector: "c:used".into(),
                        recv: Operand::Val(ValueId(0)),
                        args: vec![],
                        names: vec![],
                        tuple_types: Operand::Constant(Constant::Null),
                    },
                    Type::Ref,
                ));
            }
            _ => module.functions[0].return_ty = Type::Ref,
        }
        assert!(!apply(&mut module), "variant {variant}");
        assert!(module.closed_class_tables.is_none());
    }
}

#[test]
fn local_calls_retain_their_transitive_allocations_and_runtime_abi_is_checked() {
    let mut module = fixture();
    let mut factory = function(
        "df.local.factory",
        vec![(
            ValueId(1),
            Instruction::AllocObject {
                class_id: 302,
                fields: vec![],
            },
            Type::Ref,
        )],
    );
    factory
        .params
        .push((ValueId(0), "number".into(), Type::I64));
    module.functions.push(factory);
    module.functions[0].blocks[0].instructions.push((
        ValueId(1),
        Instruction::CallStatic {
            symbol: "df.local.factory".into(),
            args: vec![Operand::Constant(Constant::Int(7))],
            ret_ty: Type::Void,
        },
        Type::Void,
    ));
    assert!(apply(&mut module));
    assert_eq!(
        module.closed_class_tables.as_ref().unwrap(),
        &HashSet::from([300, 302])
    );
    for variant in 0..5 {
        let mut module = fixture();
        let mut args = vec![
            (Operand::Val(ValueId(0)), Type::Ref),
            (Operand::Constant(Constant::Int(0)), Type::I64),
        ];
        let mut ret_ty = Type::Void;
        let mut result = Type::Void;
        match variant {
            1 => {
                args.pop();
            }
            2 => args[0].1 = Type::I64,
            3 => {
                ret_ty = Type::F64;
                result = Type::F64;
            }
            4 => result = Type::I64,
            _ => {}
        }
        module.functions[0].blocks[0].instructions.push((
            ValueId(1),
            Instruction::CallRuntime {
                name: "dartforge_mark_constant".into(),
                args,
                ret_ty,
            },
            result,
        ));
        assert_eq!(
            apply(&mut module),
            variant == 0,
            "runtime variant {variant}"
        );
    }
}

#[test]
fn failed_reaudit_discards_the_tentative_and_previous_active_sets() {
    let mut module = fixture();
    assert!(narrow_enabled(&mut module, &[]).is_some());
    assert!(module.closed_class_tables.is_some());
    module.source_roots.push("df.missing.root".into());
    assert!(narrow_enabled(&mut module, &[]).is_none());
    assert!(module.closed_class_tables.is_none());
    module.closed_class_tables = Some(HashSet::from([300]));
    module.arc_fixed_ancestry = false;
    assert!(narrow_enabled(&mut module, &[]).is_none());
    assert!(module.closed_class_tables.is_none());
}

#[test]
fn protocol_references_cannot_smuggle_an_unchecked_runtime_or_missing_body() {
    for variant in 0..5 {
        let mut module = fixture();
        match variant {
            0 => module.entry_symbol = Some("dartforge_object_new".into()),
            1 => module.classes[0].to_string_symbol = Some("dartforge_object_new".into()),
            2 => module.classes[0]
                .vtable
                .push((9, "dartforge_object_get".into())),
            3 => {
                let duplicate = module.method_tables[0].2[0].clone();
                module.method_tables[0].2.push(duplicate);
            }
            _ => {
                module.method_tables.retain(|(cid, _, _)| *cid != 301);
                module.table_functions.remove(&301);
            }
        }
        assert!(!apply(&mut module), "variant {variant}");
        assert!(module.closed_class_tables.is_none());
    }
}

#[test]
fn stack_storage_is_bounded_and_foreign_or_retyped_slots_remain_unknown() {
    for variant in 0..5 {
        let mut module = fixture();
        let main = &mut module.functions[0];
        main.blocks[0]
            .instructions
            .push((ValueId(1), Instruction::Alloca(Type::I64), Type::Ptr));
        let pointer = if variant == 1 {
            main.params
                .push((ValueId(10), "foreign pointer".into(), Type::Ptr));
            Operand::Val(ValueId(10))
        } else {
            Operand::Val(ValueId(1))
        };
        main.blocks[0].instructions.push((
            ValueId(2),
            Instruction::Store {
                ptr: pointer,
                val: if variant == 2 {
                    Operand::Val(ValueId(0))
                } else {
                    Operand::Constant(Constant::Int(9))
                },
            },
            Type::Void,
        ));
        main.blocks[0].instructions.push((
            ValueId(3),
            Instruction::Load {
                ptr: Operand::Val(ValueId(1)),
                ty: if variant == 3 { Type::F64 } else { Type::I64 },
            },
            if variant == 3 { Type::F64 } else { Type::I64 },
        ));
        if variant == 4 {
            main.blocks[0].instructions[1].1 = Instruction::Alloca(Type::Void);
        }
        assert_eq!(apply(&mut module), variant == 0, "stack variant {variant}");
    }
}

#[test]
fn global_storage_requires_its_local_declaration_and_exact_representation() {
    for variant in 0..4 {
        let mut module = fixture();
        if variant != 1 {
            module.globals.push((7, Type::I64, "dfg.local".into()));
        }
        module.functions[0].blocks[0].instructions.push((
            ValueId(1),
            Instruction::StoreGlobal {
                symbol: "dfg.local".into(),
                val: Operand::Constant(Constant::Int(9)),
                ty: Type::I64,
                root: if variant == 2 { Some(7) } else { None },
            },
            Type::Void,
        ));
        module.functions[0].blocks[0].instructions.push((
            ValueId(2),
            Instruction::LoadGlobal {
                symbol: "dfg.local".into(),
                ty: if variant == 3 { Type::F64 } else { Type::I64 },
            },
            if variant == 3 { Type::F64 } else { Type::I64 },
        ));
        assert_eq!(apply(&mut module), variant == 0, "global variant {variant}");
    }
}

#[test]
#[ignore = "requires Clang and the native runtime"]
fn compiled_closed_selector_matches_eager_registration() {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(|| {
            let directory = tempfile::tempdir().unwrap();
            let mut source = fixture();
            nominal_ancestors(&mut source);
            source.runtime_cids = dartforge_runtime::layout::cid::SDK_CLASSES.iter()
                .map(|(cid, _, _)| i64::from(*cid)).collect();
            source.functions[0].blocks[0].instructions.extend([
                (
                    ValueId(1),
                    Instruction::CallSelector {
                        selector: "c:used".into(),
                        recv: Operand::Val(ValueId(0)),
                        args: vec![],
                        names: vec![],
                        tuple_types: Operand::Constant(Constant::Null),
                    },
                    Type::Ref,
                ),
                (
                    ValueId(2),
                    Instruction::ICmp(
                        ICmpOp::Eq,
                        Operand::Val(ValueId(1)),
                        Operand::Constant(Constant::Null),
                    ),
                    Type::I1,
                ),
            ]);
            // Positive transitive membership and an unrelated nominal target
            // must survive table pruning. The query may cache native bitmaps.
            for (base, target, expected) in [(3, 400, 1), (7, 500, 0)] {
                source.functions[0].blocks[0].instructions.extend([
                    (ValueId(base), Instruction::CallRuntime {
                        name: "dartforge_is_subclass".into(),
                        args: vec![(Operand::Constant(Constant::Int(300)), Type::I64),
                                   (Operand::Constant(Constant::Int(target)), Type::I64)],
                        ret_ty: Type::I8,
                    }, Type::I8),
                    (ValueId(base + 1), Instruction::ZExt {
                        op: Operand::Val(ValueId(base)), from: Type::I8, to: Type::I64,
                    }, Type::I64),
                    (ValueId(base + 2), Instruction::ICmp(ICmpOp::Eq,
                        Operand::Val(ValueId(base + 1)), Operand::Constant(Constant::Int(expected))), Type::I1),
                    (ValueId(base + 3), Instruction::And(
                        Operand::Val(ValueId(if base == 3 { 2 } else { 6 })),
                        Operand::Val(ValueId(base + 2))), Type::I1),
                ]);
            }
            source.functions[0].blocks[0].terminator = Terminator::CondBranch {
                cond: Operand::Val(ValueId(10)),
                then_block: BlockId(1),
                else_block: BlockId(2),
            };
            source.functions[0].blocks.extend([
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Throw(Operand::Constant(Constant::Null)),
                },
            ]);
            for closed in [false, true] {
                let mut module = source.clone();
                if closed {
                    assert!(apply(&mut module));
                }
                let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
                assert_eq!(ir.contains("define ptr @df.mt.Unused("), !closed);
                // This SDK-free ABI fixture links the runtime-owned main. The
                // SDK-style emitter wrapper normally replaces the SDK DLL's main.
                let wrapper = "define i32 @main() {\n  %r = call i32 @dartforge_init(ptr @dartforge_entry, ptr @dartforge_dispatch_toString)\n  ret i32 %r\n}\n";
                assert_eq!(ir.matches(wrapper).count(), 1);
                let ir = ir.replacen(wrapper, "", 1);
                let executable = directory
                    .path()
                    .join(crate::target::executable_name(if closed {
                        "closed"
                    } else {
                        "eager"
                    }));
                crate::driver::compile_and_link(
                    &ir,
                    &executable,
                    &crate::driver::NativeDriverOptions::default(),
                )
                .unwrap();
                for stressed in [false, true] {
                    let mut command = std::process::Command::new(&executable);
                    if stressed {
                        command.env("DARTFORGE_GC_STRESS", "1");
                    }
                    let output = command.output().unwrap();
                    assert!(
                        output.status.success(),
                        "closed={closed}, stress={stressed}: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    assert!(output.stdout.is_empty() && output.stderr.is_empty());
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
#[ignore = "requires Clang and the native runtime"]
fn compiled_typed_closures_preserve_dispatch_and_stress_gc() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let directory = tempfile::tempdir().unwrap();
        for runtime_reads in [false, true] {
            for scalar in 0..8 {
                let mut source = typed_closure_fixture(runtime_reads);
                match scalar {
                    1 => typed_scalar_argument(&mut source, Type::I64, Operand::Constant(Constant::Int(7)), 36),
                    2 => typed_scalar_argument(&mut source, Type::F64, Operand::Constant(Constant::Double(2.5)), 41),
                    3 => typed_scalar_argument(&mut source, Type::I1, Operand::Constant(Constant::Bool(true)), 46),
                    4 => typed_scalar_result(&mut source, Type::I64, Constant::Int(7), 7),
                    5 => typed_scalar_result(&mut source, Type::F64, Constant::Double(2.5), 8),
                    6 => typed_scalar_result(&mut source, Type::I1, Constant::Bool(true), 9),
                    7 => {
                        if let Instruction::AllocClosureTyped { env, direct, .. } =
                            &mut source.functions[0].blocks[0].instructions[1].1 {
                            *env = Operand::Val(ValueId(0));
                            *direct = true;
                        }
                        source.functions.last_mut().unwrap().blocks[0].terminator =
                            Terminator::Return(Some(Operand::Val(ValueId(0))));
                    }
                    _ => (),
                }
                source.functions[0].blocks[0].instructions.extend([
                    (ValueId(10), Instruction::CallRuntime { name: "dartforge_value_class".into(),
                        args: vec![(Operand::Val(ValueId(1)), Type::Ref)], ret_ty: Type::I64 }, Type::I64),
                    (ValueId(11), Instruction::CallRuntime { name: "dartforge_object_get".into(),
                        args: vec![(Operand::Val(ValueId(1)), Type::Ref), (Operand::Constant(Constant::Int(3)), Type::I64)],
                        ret_ty: Type::I64 }, Type::I64),
                ]);
                source.runtime_cids = dartforge_runtime::layout::cid::SDK_CLASSES.iter()
                    .map(|(cid, _, _)| i64::from(*cid)).collect();
                if scalar < 4 || scalar == 7 {
                source.functions[0].blocks[0].instructions.extend([
                    (ValueId(5), Instruction::CallSelector {
                        selector: "c:used".into(), recv: Operand::Val(ValueId(4)),
                        args: vec![], names: vec![], tuple_types: Operand::Constant(Constant::Null),
                    }, Type::Ref),
                    (ValueId(6), Instruction::ICmp(ICmpOp::Eq, Operand::Val(ValueId(5)),
                        Operand::Constant(Constant::Null)), Type::I1),
                ]);
                } else {
                    let comparison = match scalar {
                        4 => Instruction::ICmp(ICmpOp::Eq, Operand::Val(ValueId(4)), Operand::Constant(Constant::Int(7))),
                        5 => Instruction::FCmp(FCmpOp::Eq, Operand::Val(ValueId(4)), Operand::Constant(Constant::Double(2.5))),
                        _ => Instruction::ICmp(ICmpOp::Eq, Operand::Val(ValueId(4)), Operand::Constant(Constant::Bool(true))),
                    };
                    source.functions[0].blocks[0].instructions.push((ValueId(6), comparison, Type::I1));
                }
                source.functions[0].blocks[0].terminator = Terminator::CondBranch {
                    cond: Operand::Val(ValueId(6)), then_block: BlockId(1), else_block: BlockId(2),
                };
                source.functions[0].blocks.extend([
                    BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(None) },
                    BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Throw(Operand::Constant(Constant::Null)) },
                ]);
                for closed in [false, true] {
                    let mut module = source.clone();
                    if closed { assert!(apply(&mut module)); }
                    let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
                    assert!(ir.contains("define ptr @df.mt.Nested("));
                    assert_eq!(ir.contains("define ptr @df.mt.Unused("), !closed);
                    // This ABI fixture imports no SDK; use the runtime-owned main.
                    let wrapper = "define i32 @main() {\n  %r = call i32 @dartforge_init(ptr @dartforge_entry, ptr @dartforge_dispatch_toString)\n  ret i32 %r\n}\n";
                    assert_eq!(ir.matches(wrapper).count(), 1);
                    let executable = directory.path().join(crate::target::executable_name(
                        &format!("typed-{runtime_reads}-{scalar}-{closed}")));
                    crate::driver::compile_and_link(&ir.replacen(wrapper, "", 1), &executable,
                        &crate::driver::NativeDriverOptions::default()).unwrap();
                    for stress in [false, true] {
                        let mut command = std::process::Command::new(&executable);
                        if stress { command.env("DARTFORGE_GC_STRESS", "1"); }
                        let output = command.output().unwrap();
                        assert!(output.status.success(), "reads={runtime_reads}, scalar={scalar}, closed={closed}, stress={stress}: {}",
                            String::from_utf8_lossy(&output.stderr));
                        assert!(output.stdout.is_empty() && output.stderr.is_empty());
                    }
                }
            }
        }
    }).unwrap().join().unwrap();
}

#[test]
#[ignore = "requires Clang and the native runtime"]
fn compiled_uniform_closures_match_eager_registration_with_stress_gc() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let directory = tempfile::tempdir().unwrap();
        for (tearoff, captured) in [(false, false), (true, false), (false, true)] {
            let mut source = closure_fixture(tearoff);
            source.runtime_cids = dartforge_runtime::layout::cid::SDK_CLASSES.iter()
                .map(|(cid, _, _)| i64::from(*cid)).collect();
            if captured {
                if let Instruction::AllocClosure { env, .. } = &mut source.functions[0].blocks[0].instructions[1].1 {
                    *env = Operand::Val(ValueId(0));
                }
                source.functions.last_mut().unwrap().blocks[0].instructions[0].1 = Instruction::GetField {
                    object: Operand::Val(ValueId(0)), index: 1,
                };
            }
            source.functions[0].blocks[0].instructions.extend([
                (ValueId(3), Instruction::CallSelector {
                    selector: "c:used".into(), recv: Operand::Val(ValueId(2)),
                    args: vec![], names: vec![], tuple_types: Operand::Constant(Constant::Null),
                }, Type::Ref),
                (ValueId(4), Instruction::ICmp(ICmpOp::Eq, Operand::Val(ValueId(3)),
                    Operand::Constant(Constant::Null)), Type::I1),
            ]);
            source.functions[0].blocks[0].terminator = Terminator::CondBranch {
                cond: Operand::Val(ValueId(4)), then_block: BlockId(1), else_block: BlockId(2),
            };
            source.functions[0].blocks.extend([
                BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(None) },
                BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Throw(Operand::Constant(Constant::Null)) },
            ]);
            for closed in [false, true] {
                let mut module = source.clone();
                if closed { assert!(apply(&mut module)); }
                let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
                assert_eq!(ir.contains("define ptr @df.mt.Unused("), !closed);
                assert_eq!(ir.contains("define ptr @df.mt.Nested("), !closed || !captured);
                // Use the runtime-owned main for this SDK-free ABI fixture.
                let wrapper = "define i32 @main() {\n  %r = call i32 @dartforge_init(ptr @dartforge_entry, ptr @dartforge_dispatch_toString)\n  ret i32 %r\n}\n";
                assert_eq!(ir.matches(wrapper).count(), 1);
                let ir = ir.replacen(wrapper, "", 1);
                let executable = directory.path().join(crate::target::executable_name(
                    &format!("closure-{tearoff}-{captured}-{closed}")));
                crate::driver::compile_and_link(&ir, &executable,
                    &crate::driver::NativeDriverOptions::default()).unwrap();
                for stressed in [false, true] {
                    let mut command = std::process::Command::new(&executable);
                    if stressed { command.env("DARTFORGE_GC_STRESS", "1"); }
                    let output = command.output().unwrap();
                    assert!(output.status.success(),
                        "tearoff={tearoff}, captured={captured}, closed={closed}, stress={stressed}: {}",
                        String::from_utf8_lossy(&output.stderr));
                    assert!(output.stdout.is_empty() && output.stderr.is_empty());
                }
            }
        }
    }).unwrap().join().unwrap();
}

#[test]
fn implicit_boxing_keeps_runtime_class_tables_without_declared_registration_ids() {
    let mut module = fixture();
    let cid = dartforge_runtime::layout::cid::DOUBLE as u32;
    assert!(module.runtime_cids.is_empty());
    module.classes[1].id = cid;
    module.pruning_class_ids.remove(&301);
    module.pruning_class_ids.insert(cid);
    let table = module.table_functions.remove(&301).unwrap();
    module.table_functions.insert(cid, table);
    module.method_tables[1].0 = cid;
    module.functions[0].blocks[0].instructions.extend([
        (
            ValueId(1),
            Instruction::Box {
                op: Operand::Constant(Constant::Double(1.5)),
                from: Type::F64,
            },
            Type::Ref,
        ),
        (
            ValueId(2),
            Instruction::CallSelector {
                selector: "c:used".into(),
                recv: Operand::Val(ValueId(1)),
                args: vec![],
                names: vec![],
                tuple_types: Operand::Constant(Constant::Null),
            },
            Type::Ref,
        ),
    ]);
    assert!(apply(&mut module));
    assert_eq!(
        module.closed_class_tables.as_ref().unwrap(),
        &HashSet::from([300, cid])
    );
    assert!(
        module
            .functions
            .iter()
            .any(|f| f.symbol == "df.Unused.method")
    );
    assert!(
        !module
            .functions
            .iter()
            .any(|f| f.symbol == "df.Nested.method")
    );
    let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
    assert!(ir.contains("define ptr @df.mt.Unused("));
    assert!(!ir.contains("define ptr @df.mt.Nested("));
}

fn checked_scalar_sdk(
    ir: &str,
) -> (
    tempfile::TempDir,
    Summary,
    std::sync::Arc<super::super::allocation_inventory::imports::Inventory>,
) {
    checked_sdk(ir, &[])
}

fn checked_sdk(
    ir: &str,
    tables: &[super::super::ModuleTable],
) -> (
    tempfile::TempDir,
    Summary,
    std::sync::Arc<super::super::allocation_inventory::imports::Inventory>,
) {
    let directory = tempfile::tempdir().unwrap();
    let runtime = directory.path().join("runtime.a");
    std::fs::write(&runtime, b"test binding, not executable runtime code").unwrap();
    std::fs::write(
        directory.path().join("test.o"),
        b"test binding, not executable object code",
    )
    .unwrap();
    let summary = super::super::summarize(ir, "dart:test", tables);
    std::fs::write(directory.path().join("test.poda"), summary.to_text()).unwrap();
    let artifacts = vec!["test.o".to_string(), "test.poda".to_string()];
    let bundle = super::super::allocation_inventory::closure::audit(&[("dart:test", ir)]);
    super::super::allocation_inventory::publication::publish(
        directory.path(),
        "test-key",
        true,
        &artifacts,
        &runtime,
        &bundle,
    )
    .unwrap();
    let inventory = super::super::allocation_inventory::imports::Inventory::load(
        directory.path(),
        "test-key",
        true,
        &artifacts,
        &runtime,
    )
    .unwrap();
    (directory, summary, std::sync::Arc::new(inventory))
}

#[test]
fn verified_imported_scalar_closures_can_close_the_program_graph() {
    let ir = "define i64 @df.dart$3atest..leaf(i64 %x) {\n %sum = add i64 %x, 1\n ret i64 %sum\n}\ndefine void @df.dart$3atest..startup() {\n ret void\n}\n";
    let (_directory, summary, inventory) = checked_scalar_sdk(ir);
    let mut module = fixture();
    module.sdk_records.push("df.dart$3atest..startup".into());
    module.functions[0].blocks[0].instructions.push((
        ValueId(1),
        Instruction::CallStatic {
            symbol: "df.dart$3atest..leaf".into(),
            args: vec![Operand::Constant(Constant::Int(7))],
            ret_ty: Type::I64,
        },
        Type::I64,
    ));
    assert!(
        narrow_enabled(&mut module, &[&summary]).is_none(),
        "declarations alone are not a certificate"
    );
    module.imported_allocation_inventory = Some(inventory);
    assert!(narrow_enabled(&mut module, &[&summary]).is_some());
    assert_eq!(
        module.closed_class_tables.as_ref().unwrap(),
        &HashSet::from([300])
    );
    let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
    assert!(!ir.contains("define ptr @df.mt.Unused("));
}

#[test]
fn changed_or_unproved_imports_do_not_hide_missing_allocation_coverage() {
    for variant in 0..5 {
        let body = if variant == 3 {
            "%v = call i64 @opaque_native(i64 %x)\n ret i64 %v"
        } else {
            "ret i64 %x"
        };
        let ir = format!("define i64 @df.dart$3atest..leaf(i64 %x) {{\n {body}\n}}\n");
        let (_directory, mut summary, inventory) = checked_scalar_sdk(&ir);
        let mut module = fixture();
        module.imported_allocation_inventory = Some(inventory);
        let argument = if variant == 1 {
            Operand::Constant(Constant::Double(1.0))
        } else {
            Operand::Constant(Constant::Int(7))
        };
        module.functions[0].blocks[0].instructions.push((
            ValueId(1),
            Instruction::CallStatic {
                symbol: "df.dart$3atest..leaf".into(),
                args: vec![argument],
                ret_ty: Type::I64,
            },
            Type::I64,
        ));
        match variant {
            0 => summary.definitions[0].refs.push(0),
            2 => module
                .functions
                .push(function("df.dart$3atest..leaf", vec![])),
            4 => module.sdk_records.push("df.unproved.startup".into()),
            _ => {}
        }
        assert!(
            narrow_enabled(&mut module, &[&summary]).is_none(),
            "variant {variant}"
        );
        assert!(module.closed_class_tables.is_none());
    }
}

#[test]
fn rejection_identifies_the_boundary_and_discards_previous_class_coverage() {
    let mut module = fixture();
    assert!(audit(&mut module, &[]).is_ok());
    assert!(module.closed_class_tables.is_some());
    module.source_roots.push("df.external.startup".into());
    assert_eq!(
        audit(&mut module, &[]).err().as_deref(),
        Some("entry root df.external.startup has an unproved body or input ABI")
    );
    assert!(module.closed_class_tables.is_none());

    module.source_roots.clear();
    module.functions[0].blocks[0].instructions.push((
        ValueId(1),
        Instruction::AllocList { elements: vec![] },
        Type::Ref,
    ));
    let reason = audit(&mut module, &[])
        .err()
        .expect("list allocation is not covered");
    assert!(reason.contains("df.test.main value ValueId(1)"), "{reason}");
    assert!(reason.contains("AllocList"), "{reason}");
    assert!(module.closed_class_tables.is_none());
}

#[test]
fn verified_unused_sdk_table_declarations_do_not_require_local_definitions() {
    let tables = vec![(900, "df.mt.imported".into(), vec![])];
    let (_directory, summary, inventory) = checked_sdk("", &tables);
    let mut module = fixture();
    module.table_functions.insert(900, "df.mt.imported".into());
    module.imported_allocation_inventory = Some(inventory);
    assert!(audit(&mut module, &[&summary]).is_ok());
    assert_eq!(
        module.closed_class_tables.as_ref().unwrap(),
        &HashSet::from([300])
    );
}

#[test]
fn imported_table_declarations_do_not_authorize_calls_allocations_or_collisions() {
    for variant in 0..8 {
        let tables = vec![(900, "df.mt.imported".into(), vec![])];
        let (_directory, mut summary, inventory) = checked_sdk("", &tables);
        let mut module = fixture();
        module.table_functions.insert(900, "df.mt.imported".into());
        module.imported_allocation_inventory = Some(inventory);
        match variant {
            0 => {
                module.table_functions.insert(901, "df.mt.imported".into());
            }
            1 => {
                module
                    .table_functions
                    .insert(900, "df.mt.unverified".into());
            }
            2 => {
                module.imported_allocation_inventory = None;
            }
            3 => {
                summary.tables[0].cid = 901;
            }
            4 => module.functions[0].blocks[0].instructions.push((
                ValueId(1),
                Instruction::CallStatic {
                    symbol: "df.mt.imported".into(),
                    args: vec![],
                    ret_ty: Type::Ptr,
                },
                Type::Ptr,
            )),
            5 => module.functions[0].blocks[0].instructions.push((
                ValueId(1),
                Instruction::AllocObject {
                    class_id: 900,
                    fields: vec![],
                },
                Type::Ref,
            )),
            6 => module.functions.push(function("df.mt.imported", vec![])),
            _ => {}
        }
        let selected = if variant == 7 { vec![] } else { vec![&summary] };
        assert!(audit(&mut module, &selected).is_err(), "variant {variant}");
        assert!(module.closed_class_tables.is_none(), "variant {variant}");
    }
}

fn nominal_ancestors(module: &mut Module) {
    for (cid, name) in [
        (0, "Object"),
        (400, "AbstractBase"),
        (401, "Intermediate"),
        (500, "Unrelated"),
    ] {
        module.classes.push(ClassDef {
            id: cid,
            name: name.into(),
            field_count: 0,
            vtable: vec![],
            to_string_symbol: None,
        });
        module.pruning_class_ids.insert(cid);
        module.pruning_metadata_only_class_ids.insert(cid);
    }
    module
        .subtyping_edges
        .extend([(300, 401), (401, 400), (400, 0)]);
}

#[test]
fn tableless_nominal_ancestors_keep_subtype_metadata_without_becoming_allocatable() {
    let mut module = fixture();
    nominal_ancestors(&mut module);
    assert!(apply(&mut module));
    assert_eq!(
        module.closed_class_tables.as_ref().unwrap(),
        &HashSet::from([300])
    );
    assert!(module.classes.iter().any(|class| class.id == 400));
    let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
    assert!(ir.contains("call void @dartforge_register_subclass(i64 300, i64 401)"));
    assert!(ir.contains("call void @dartforge_register_subclass(i64 400, i64 0)"));
}

#[test]
fn tableless_metadata_does_not_cover_allocations_runtime_classes_or_open_ancestry() {
    for variant in 0..13 {
        let mut module = fixture();
        nominal_ancestors(&mut module);
        match variant {
            0 | 1 => module.functions[0].blocks[0].instructions.push((
                ValueId(1),
                Instruction::AllocObject {
                    class_id: if variant == 0 { 0 } else { 400 },
                    fields: vec![],
                },
                Type::Ref,
            )),
            2 => module.functions[0].blocks[0].instructions.push((
                ValueId(1),
                Instruction::CallRuntime {
                    name: "dartforge_object_new".into(),
                    args: vec![
                        (Operand::Constant(Constant::Int(400)), Type::I64),
                        (Operand::Constant(Constant::Int(0)), Type::I64),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )),
            3 => module.runtime_cids.push(400),
            4 => {
                module
                    .classes
                    .iter_mut()
                    .find(|class| class.id == 400)
                    .unwrap()
                    .id = dartforge_runtime::layout::cid::ONE_BYTE_STRING as u32;
                let cid = dartforge_runtime::layout::cid::ONE_BYTE_STRING as u32;
                module.pruning_class_ids.insert(cid);
                module.pruning_metadata_only_class_ids.remove(&400);
                module.pruning_metadata_only_class_ids.insert(cid);
            }
            5 => {
                module.pruning_class_ids.remove(&400);
            }
            6 => {
                module
                    .table_functions
                    .insert(400, "df.mt.AbstractBase".into());
            }
            7 => {
                module.pruning_metadata_only_class_ids.insert(999);
            }
            8 => {
                module.pruning_metadata_only_class_ids.insert(300);
            }
            9 => {
                module.pruning_metadata_only_class_ids.remove(&400);
            }
            _ => {
                module.functions[0].blocks[0].instructions.push((
                    ValueId(1),
                    Instruction::CallRuntime {
                        name: "dartforge_is_subclass".into(),
                        args: if variant == 10 {
                            vec![]
                        } else {
                            vec![
                                (
                                    if variant == 11 {
                                        Operand::Val(ValueId(0))
                                    } else {
                                        Operand::Constant(Constant::Int(300))
                                    },
                                    if variant == 11 { Type::Ref } else { Type::I64 },
                                ),
                                (Operand::Constant(Constant::Int(400)), Type::I64),
                            ]
                        },
                        ret_ty: if variant == 12 { Type::I64 } else { Type::I8 },
                    },
                    if variant == 12 { Type::I64 } else { Type::I8 },
                ));
            }
        }
        assert!(audit(&mut module, &[]).is_err(), "variant {variant}");
        assert!(module.closed_class_tables.is_none(), "variant {variant}");
    }
}

#[test]
#[ignore = "requires the source SDK"]
fn lowering_marks_only_nominal_ancestors_and_preserves_the_premises_when_joining() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let directory = tempfile::tempdir().unwrap();
        let entry = directory.path().join("main.dart");
        std::fs::write(&entry, "abstract class Base {}\nmixin Feature {}\nclass Concrete extends Base with Feature {}\nvoid main() { Concrete(); }\n").unwrap();
        let sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
        let mut interner = dartforge_intern::Interner::new();
        let program = dartforge_elements::load::load(&entry, &sdk, None, &mut interner).unwrap();
        let mut table = dartforge_types::TypeTable::new();
        let core = dartforge_types::CoreTypes::init(&mut table, &program, &interner);
        let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
        let libraries: Vec<_> = program.libraries.iter().enumerate().filter(|(_, library)| !library.is_sdk)
            .map(|(index, _)| dartforge_elements::model::LibraryId(index as u32)).collect();
        let (bodies, _) = dartforge_types::infer_library_bodies_with_locals(
            &program, &interner, &mut table, &core, &mut outline, &libraries);

        let context = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies)
            .with_source_sdk();
        let lowered = crate::lower::lower_program(&context);
        assert!(lowered.errors.is_empty(), "{:?}", lowered.errors);
        assert!(lowered.classes.iter().any(|class| class.id == 0));
        assert!(lowered.pruning_metadata_only_class_ids.contains(&0));
        let type_cid = crate::lower::rti::TYPE_CLASS as u32;
        assert!(lowered.subtyping_edges.iter().any(|(child, parent)| *child == type_cid && *parent != type_cid));
        assert!(lowered.pruning_class_ids.contains(&type_cid));
        assert_eq!(lowered.table_functions.get(&type_cid).map(String::as_str), Some("df.mt.$type"));
        let mut facts = crate::lower::class_registration_facts(&context);
        for (name, metadata_only) in [("Base", true), ("Feature", true), ("Concrete", false)] {
            let class = facts.classes.iter().find(|class| class.name == name).unwrap();
            assert_eq!(facts.pruning_metadata_only_class_ids.contains(&class.id), metadata_only, "{name}");
        }
        let expected = facts.pruning_metadata_only_class_ids.clone();
        let mut combined = Module::new();
        crate::lower::join_arc_facts(&mut combined, &mut facts);
        assert_eq!(combined.pruning_metadata_only_class_ids, expected);
        assert!(facts.pruning_metadata_only_class_ids.is_empty());
    }).unwrap().join().unwrap();
}

fn type_table_fixture() -> Module {
    let mut module = fixture();
    let cid = crate::lower::rti::TYPE_CLASS as u32;
    let class = module
        .classes
        .iter_mut()
        .find(|class| class.id == 301)
        .unwrap();
    class.id = cid;
    class.field_count = 1;
    module.pruning_class_ids.remove(&301);
    module.pruning_class_ids.insert(cid);
    module
        .method_tables
        .iter_mut()
        .find(|(class, _, _)| *class == 301)
        .unwrap()
        .0 = cid;
    let table = module.table_functions.remove(&301).unwrap();
    module.table_functions.insert(cid, table);
    module
}

#[test]
fn canonical_type_table_stays_live_without_explicit_allocations_or_runtime_vector() {
    let mut module = type_table_fixture();
    assert!(module.runtime_cids.is_empty());
    assert!(apply(&mut module));
    assert_eq!(
        module.closed_class_tables.as_ref().unwrap(),
        &HashSet::from([300, crate::lower::rti::TYPE_CLASS as u32])
    );
    assert!(
        module
            .functions
            .iter()
            .any(|function| function.symbol == "df.Unused.toString")
    );
}

#[test]
fn canonical_type_identity_cannot_be_relabelled_as_nominal_or_authorize_native_factories() {
    for nominal_only in [false, true] {
        let mut module = type_table_fixture();
        if nominal_only {
            let cid = crate::lower::rti::TYPE_CLASS as u32;
            module.pruning_metadata_only_class_ids.insert(cid);
            module.method_tables.retain(|(class, _, _)| *class != cid);
            module.table_functions.remove(&cid);
        } else {
            module.functions[0].blocks[0].instructions.push((
                ValueId(1),
                Instruction::CallRuntime {
                    name: "dartforge_rti_object_type".into(),
                    args: vec![(Operand::Constant(Constant::Int(0)), Type::I64)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ));
        }
        assert!(audit(&mut module, &[]).is_err());
        assert!(module.closed_class_tables.is_none());
    }
}

#[test]
#[ignore = "requires Clang and the native runtime"]
fn compiled_runtime_type_factory_matches_the_emitter_cid_and_scalar_payload() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let mut module = fixture();
        module.runtime_cids = dartforge_runtime::layout::cid::SDK_CLASSES.iter()
            .map(|(cid, _, _)| i64::from(*cid)).collect();
        module.functions[0].blocks[0].instructions = vec![
            (ValueId(0), Instruction::CallRuntime { name: "dartforge_rti_object_type".into(),
                args: vec![(Operand::Constant(Constant::Int(0)), Type::I64)], ret_ty: Type::Ref }, Type::Ref),
            (ValueId(1), Instruction::AllocObject { class_id: 300, fields: vec![] }, Type::Ref),
            (ValueId(2), Instruction::CallRuntime { name: "dartforge_rti_object_type".into(),
                args: vec![(Operand::Constant(Constant::Int(0)), Type::I64)], ret_ty: Type::Ref }, Type::Ref),
            (ValueId(3), Instruction::ICmp(ICmpOp::Eq, Operand::Val(ValueId(0)), Operand::Val(ValueId(2))), Type::I1),
            (ValueId(4), Instruction::CallRuntime { name: "dartforge_value_class".into(),
                args: vec![(Operand::Val(ValueId(0)), Type::Ref)], ret_ty: Type::I64 }, Type::I64),
            (ValueId(5), Instruction::ICmp(ICmpOp::Eq, Operand::Val(ValueId(4)),
                Operand::Constant(Constant::Int(crate::lower::rti::TYPE_CLASS))), Type::I1),
            (ValueId(6), Instruction::And(Operand::Val(ValueId(3)), Operand::Val(ValueId(5))), Type::I1),
            (ValueId(7), Instruction::CallRuntime { name: "dartforge_object_get".into(),
                args: vec![(Operand::Val(ValueId(0)), Type::Ref), (Operand::Constant(Constant::Int(0)), Type::I64)],
                ret_ty: Type::I64 }, Type::I64),
            (ValueId(8), Instruction::ICmp(ICmpOp::Eq, Operand::Val(ValueId(7)),
                Operand::Constant(Constant::Int(0))), Type::I1),
            (ValueId(9), Instruction::And(Operand::Val(ValueId(6)), Operand::Val(ValueId(8))), Type::I1),
        ];
        module.functions[0].blocks[0].terminator = Terminator::CondBranch {
            cond: Operand::Val(ValueId(9)), then_block: BlockId(1), else_block: BlockId(2),
        };
        module.functions[0].blocks.extend([
            BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(None) },
            BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Throw(Operand::Constant(Constant::Null)) },
        ]);
        // This checks the runtime ABI; it does not admit the factory in the proof.
        let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
        let wrapper = "define i32 @main() {\n  %r = call i32 @dartforge_init(ptr @dartforge_entry, ptr @dartforge_dispatch_toString)\n  ret i32 %r\n}\n";
        assert_eq!(ir.matches(wrapper).count(), 1);
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join(crate::target::executable_name("runtime-type-abi"));
        crate::driver::compile_and_link(&ir.replacen(wrapper, "", 1), &executable,
            &crate::driver::NativeDriverOptions::default()).unwrap();
        for stress in [false, true] {
            let mut command = std::process::Command::new(&executable);
            if stress { command.env("DARTFORGE_GC_STRESS", "1"); }
            let output = command.output().unwrap();
            assert!(output.status.success(), "stress={stress}: {}", String::from_utf8_lossy(&output.stderr));
            assert!(output.stdout.is_empty() && output.stderr.is_empty());
        }
    }).unwrap().join().unwrap();
}
