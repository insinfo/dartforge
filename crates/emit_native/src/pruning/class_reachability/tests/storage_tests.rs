//! Storage representation and bounds regressions for closed class registration.
use super::*;

fn local(fields: Vec<Operand>) -> Module {
    let mut module = fixture();
    module.classes[0].field_count = fields.len();
    module.functions[0].params = vec![(ValueId(20), "bits".into(), Type::I64)];
    module.functions[0].blocks[0].instructions[0].1 = Instruction::AllocObject {
        class_id: 300,
        fields,
    };
    module
}

fn runtime_read(index: i64, declared: bool) -> Instruction {
    let name = "dartforge_object_get".into();
    let args = vec![
        (Operand::Val(ValueId(0)), Type::Ref),
        (Operand::Constant(Constant::Int(index)), Type::I64),
    ];
    if declared {
        Instruction::DeclaredFieldCall {
            declaration: crate::hir::FieldDeclaration {
                library_uri: "package:test/storage.dart".into(),
                owner: "Used".into(),
                name: "value".into(),
                is_final: false,
                is_late: false,
                layout_index: 0,
                storage_type: Type::Ref,
            },
            name,
            args,
            ret_ty: Type::Ref,
        }
    } else {
        Instruction::CallRuntime {
            name,
            args,
            ret_ty: Type::Ref,
        }
    }
}

#[test]
fn every_field_read_path_requires_a_managed_storage_witness() {
    let object = Operand::Val(ValueId(0));
    for read in [
        Instruction::GetField {
            object: object.clone(),
            index: 0,
        },
        Instruction::EnvGet {
            env: object.clone(),
            index: 0,
        },
        Instruction::CellGet { cell: object },
        runtime_read(0, false),
        runtime_read(0, true),
    ] {
        for scalar in [false, true] {
            let field = if scalar {
                Operand::Val(ValueId(20))
            } else {
                Operand::Constant(Constant::Null)
            };
            let mut module = local(vec![field]);
            module.functions[0].blocks[0]
                .instructions
                .push((ValueId(1), read.clone(), Type::Ref));
            assert_eq!(apply(&mut module), !scalar, "{read:?}, scalar={scalar}");
            assert_eq!(module.closed_class_tables.is_some(), !scalar);
        }
    }
}

#[test]
fn heap_accesses_require_local_receivers_and_in_bounds_indices() {
    for index in [0, 1, usize::MAX] {
        for store in [false, true] {
            let mut module = local(vec![Operand::Constant(Constant::Null)]);
            let object = Operand::Val(ValueId(0));
            let (instruction, ty) = if store {
                (
                    Instruction::SetField {
                        object,
                        index,
                        value: Operand::Constant(Constant::Null),
                    },
                    Type::Void,
                )
            } else {
                (Instruction::GetField { object, index }, Type::Ref)
            };
            module.functions[0].blocks[0]
                .instructions
                .push((ValueId(1), instruction, ty));
            assert_eq!(
                apply(&mut module),
                index == 0,
                "index={index}, store={store}"
            );
        }
    }
    for index in [-1, 1, i64::MAX] {
        let mut module = local(vec![Operand::Constant(Constant::Null)]);
        module.functions[0].blocks[0].instructions.push((
            ValueId(1),
            runtime_read(index, false),
            Type::Ref,
        ));
        assert!(!apply(&mut module), "runtime index={index}");
    }
    let mut module = local(vec![]);
    let method = &mut module.functions[1];
    method.blocks[0].instructions.push((
        ValueId(3),
        Instruction::GetField {
            object: Operand::Val(ValueId(0)),
            index: 0,
        },
        Type::Ref,
    ));
    assert!(
        !apply(&mut module),
        "protocol self has no local storage witness"
    );
}

#[test]
fn later_scalar_stores_through_aliases_invalidate_reference_reads() {
    for alias in [false, true] {
        for scalar in [false, true] {
            let mut module = local(vec![Operand::Constant(Constant::Null)]);
            let instructions = &mut module.functions[0].blocks[0].instructions;
            instructions.push((
                ValueId(1),
                Instruction::GetField {
                    object: Operand::Val(ValueId(0)),
                    index: 0,
                },
                Type::Ref,
            ));
            instructions.push((
                ValueId(2),
                Instruction::CheckNotNull(Operand::Val(ValueId(0))),
                Type::Ref,
            ));
            instructions.push((
                ValueId(3),
                Instruction::SetField {
                    object: Operand::Val(ValueId(if alias { 2 } else { 0 })),
                    index: 0,
                    value: if scalar {
                        Operand::Val(ValueId(20))
                    } else {
                        Operand::Constant(Constant::Null)
                    },
                },
                Type::Void,
            ));
            assert_eq!(
                apply(&mut module),
                !scalar,
                "alias={alias}, scalar={scalar}"
            );
        }
    }
}

#[test]
fn erased_setter_reference_flags_must_match_the_actual_value() {
    for scalar in [false, true] {
        for flag in [0, 1] {
            let mut module = local(vec![Operand::Constant(Constant::Null)]);
            module.functions[0].blocks[0].instructions.extend([
                (
                    ValueId(1),
                    Instruction::Const(Constant::Int(flag)),
                    Type::I8,
                ),
                (
                    ValueId(2),
                    Instruction::CallRuntime {
                        name: "dartforge_object_set".into(),
                        args: vec![
                            (Operand::Val(ValueId(0)), Type::Ref),
                            (Operand::Constant(Constant::Int(0)), Type::I64),
                            (
                                if scalar {
                                    Operand::Val(ValueId(20))
                                } else {
                                    Operand::Constant(Constant::Null)
                                },
                                if scalar { Type::I64 } else { Type::Ref },
                            ),
                            (Operand::Val(ValueId(1)), Type::I8),
                        ],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
            ]);
            assert_eq!(
                apply(&mut module),
                flag == i64::from(!scalar),
                "scalar={scalar}, flag={flag}"
            );
        }
    }
}

#[test]
fn scalar_results_cannot_acquire_reference_or_pointer_capabilities() {
    for ty in [Type::Ref, Type::Ptr] {
        for instruction in [
            Instruction::Const(Constant::Int(123)),
            Instruction::ZExt {
                op: Operand::Val(ValueId(20)),
                from: Type::I64,
                to: ty,
            },
            Instruction::CheckNotNull(Operand::Val(ValueId(20))),
        ] {
            let mut module = local(vec![]);
            module.functions[0].blocks[0]
                .instructions
                .push((ValueId(1), instruction.clone(), ty));
            assert!(!apply(&mut module), "{instruction:?} as {ty:?}");
            assert!(module.closed_class_tables.is_none());
        }
    }
}

#[test]
fn scalar_words_do_not_authorize_native_constant_getters() {
    let mut module = local(vec![]);
    module.functions[0].blocks[0].instructions.push((
        ValueId(1),
        Instruction::CallRuntime {
            name: "dartforge_mark_constant".into(),
            args: vec![
                (Operand::Val(ValueId(0)), Type::Ref),
                (Operand::Val(ValueId(20)), Type::I64),
            ],
            ret_ty: Type::Void,
        },
        Type::Void,
    ));
    assert!(!apply(&mut module));
    assert!(module.closed_class_tables.is_none());
}

#[test]
fn closure_storage_witnesses_cannot_be_reused_as_object_protocol_entries() {
    for protocol in [false, true] {
        let mut module = closure_fixture(false);
        if protocol {
            module.method_tables[0]
                .2
                .push(("c:forged".into(), "df.test.create".into()));
        } else {
            module.functions[0].blocks[0].instructions.push((
                ValueId(4),
                Instruction::Alloca(Type::I64),
                Type::Ptr,
            ));
            module.functions[0].blocks[0].instructions.push((
                ValueId(3),
                Instruction::CallStatic {
                    symbol: "df.test.create".into(),
                    args: vec![
                        Operand::Val(ValueId(0)),
                        Operand::Val(ValueId(4)),
                        Operand::Val(ValueId(4)),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ));
        }
        assert!(!apply(&mut module), "protocol={protocol}");
        assert!(module.closed_class_tables.is_none());
    }
}

#[test]
fn compiled_storage_witnesses_preserve_scalar_and_managed_reads_under_stress_gc() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let directory = tempfile::tempdir().unwrap();
        for managed in [false, true] {
            let mut source = fixture();
            source.classes[0].field_count = 1;
            source.runtime_cids = dartforge_runtime::layout::cid::SDK_CLASSES.iter()
                .map(|(cid, _, _)| i64::from(*cid)).collect();
            let mut instructions = vec![(ValueId(0), Instruction::AllocObject {
                class_id: 302, fields: vec![],
            }, Type::Ref), (ValueId(1), Instruction::AllocObject {
                class_id: 300,
                fields: vec![if managed { Operand::Val(ValueId(0)) } else { Operand::Constant(Constant::Int(37)) }],
            }, Type::Ref), (ValueId(2), Instruction::GetField {
                object: Operand::Val(ValueId(1)), index: 0,
            }, if managed { Type::Ref } else { Type::I64 })];
            let observed = if managed {
                instructions.push((ValueId(3), Instruction::CallRuntime {
                    name: "dartforge_value_class".into(),
                    args: vec![(Operand::Val(ValueId(2)), Type::Ref)], ret_ty: Type::I64,
                }, Type::I64));
                ValueId(3)
            } else { ValueId(2) };
            instructions.push((ValueId(4), Instruction::ICmp(ICmpOp::Eq,
                Operand::Val(observed), Operand::Constant(Constant::Int(if managed { 302 } else { 37 }))), Type::I1));
            source.functions[0].blocks = vec![
                BasicBlock { id: BlockId(0), instructions, terminator: Terminator::CondBranch {
                    cond: Operand::Val(ValueId(4)), then_block: BlockId(1), else_block: BlockId(2),
                }},
                BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(None) },
                BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Throw(Operand::Val(ValueId(1))) },
            ];
            for closed in [false, true] {
                let mut module = source.clone();
                if closed { assert!(apply(&mut module)); }
                let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
                let wrapper = "define i32 @main() {\n  %r = call i32 @dartforge_init(ptr @dartforge_entry, ptr @dartforge_dispatch_toString)\n  ret i32 %r\n}\n";
                assert_eq!(ir.matches(wrapper).count(), 1);
                let executable = directory.path().join(crate::target::executable_name(&format!("storage-{managed}-{closed}")));
                crate::driver::compile_and_link(&ir.replacen(wrapper, "", 1), &executable,
                    &crate::driver::NativeDriverOptions::default()).unwrap();
                for stressed in [false, true] {
                    let mut command = std::process::Command::new(&executable);
                    if stressed { command.env("DARTFORGE_GC_STRESS", "1"); }
                    let output = command.output().unwrap();
                    assert!(output.status.success(), "managed={managed}, closed={closed}, stress={stressed}: {}",
                        String::from_utf8_lossy(&output.stderr));
                    assert!(output.stdout.is_empty() && output.stderr.is_empty());
                }
            }
        }
    }).unwrap().join().unwrap();
}
