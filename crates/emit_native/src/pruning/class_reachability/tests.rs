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
            source.functions[0].blocks[0].terminator = Terminator::CondBranch {
                cond: Operand::Val(ValueId(2)),
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
