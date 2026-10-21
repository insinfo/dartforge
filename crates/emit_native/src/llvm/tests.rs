//! Tests of the contract in the emitted IR (docs/NATIVO-PLANO.md §6: E1, G1–G3).

use super::LlvmEmitter;
use crate::hir::*;

#[test]
fn closed_callback_returns_before_dart_or_argument_materialization() {
    for native in [
        CType::Void,
        CType::I8,
        CType::U8,
        CType::I16,
        CType::U16,
        CType::I32,
        CType::U32,
        CType::I64,
        CType::U64,
        CType::F32,
        CType::F64,
        CType::Bool,
        CType::Ptr,
        CType::Handle,
    ] {
        let mut module = Module::new();
        module.sdk_mode = true;
        module.ffi_callbacks.push(FfiCallback {
            key: "test_p".into(),
            body: "df.callback_test".into(),
            ret: NativeType::Prim(native),
            params: vec![NativeType::Prim(CType::Ptr)],
        });
        let ir = LlvmEmitter::new(&module).emit_all();
        let body = ir.split("@df.ffi.cbentry.0(").nth(1).unwrap();
        let closed = body.split("active_callback:").next().unwrap();
        assert!(closed.contains("icmp eq i64 %modo, 2"));
        assert!(closed.contains("closed_callback:\n"));
        assert!(!closed.contains("alloca"));
        assert!(!closed.contains("@df.callback_test"));
        assert!(!closed.contains("@dartforge_context"));
        assert!(!closed.contains("%a0 to i64"));
        if native == CType::Void {
            assert!(closed.contains("ret void"));
        } else {
            assert!(closed.contains("@dartforge_ffi_callback_late_return(ptr %ctx)"));
            assert!(closed.contains("%late_value"));
        }
    }
}

#[test]
fn foreign_sync_callback_uses_raw_bits_without_foreign_dart_state() {
    for tables in [false, true] {
        for native in [
            CType::Void,
            CType::I8,
            CType::U8,
            CType::I16,
            CType::U16,
            CType::I32,
            CType::U32,
            CType::I64,
            CType::U64,
            CType::F32,
            CType::F64,
            CType::Bool,
            CType::Ptr,
        ] {
            let mut module = Module::new();
            module.sdk_mode = true;
            module.table_exceptions = tables;
            module.ffi_callbacks.push(FfiCallback {
                key: "sync_test".into(),
                body: "df.callback_test".into(),
                ret: NativeType::Prim(native),
                params: vec![
                    NativeType::Prim(CType::I8),
                    NativeType::Prim(CType::U16),
                    NativeType::Prim(CType::F32),
                    NativeType::Prim(CType::F64),
                    NativeType::Prim(CType::Ptr),
                    NativeType::Prim(CType::Bool),
                ],
            });
            let ir = LlvmEmitter::new(&module).emit_all();
            let body = ir.split("@df.ffi.cbentry.0(").nth(1).unwrap();
            let packed = body
                .split("ouvinte:\n")
                .nth(1)
                .unwrap()
                .split("foreign_sync:\n")
                .next()
                .unwrap();
            assert!(packed.contains("sext i8 %a0 to i64"));
            assert!(packed.contains("zext i16 %a1 to i64"));
            assert!(packed.contains("fpext float %a2 to double"));
            assert!(packed.contains("bitcast double %of2 to i64"));
            assert!(packed.contains("bitcast double %a3 to i64"));
            assert!(packed.contains("ptrtoint ptr %a4 to i64"));
            assert!(packed.contains("zext i1 %a5 to i64"));
            assert!(packed.contains("icmp eq i64 %modo, 3"));
            let foreign = body
                .split("foreign_sync:\n")
                .nth(1)
                .unwrap()
                .split("listener_post:\n")
                .next()
                .unwrap();
            assert!(foreign.contains("@dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 6)"));
            for forbidden in [
                "@df.callback_test",
                "@dartforge_context",
                "@dartforge_ffi_callback_exit",
                "@dartforge_ffi_callback_post",
                "landingpad",
                "invoke ",
                "@dartforge_ffi_handle_object",
            ] {
                assert!(!foreign.contains(forbidden), "{native:?}: {foreign}");
            }
            match native {
                CType::Void => assert!(foreign.contains("ret void")),
                CType::F32 => {
                    assert!(foreign.contains("bitcast i64 %sync_bits to double"));
                    assert!(foreign.contains("fptrunc double %sync_double to float"));
                }
                CType::F64 => assert!(foreign.contains("bitcast i64 %sync_bits to double")),
                CType::Ptr => assert!(foreign.contains("inttoptr i64 %sync_bits to ptr")),
                CType::I64 | CType::U64 => assert!(foreign.contains("add i64 %sync_bits, 0")),
                _ => {
                    assert!(foreign.contains(&format!("trunc i64 %sync_bits to {}", native.llvm())))
                }
            }
            let listener = body
                .split("listener_post:\n")
                .nth(1)
                .unwrap()
                .split("local:\n")
                .next()
                .unwrap();
            assert!(listener.contains("@dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 6)"));
            let local = body.split("local:\n").nth(1).unwrap();
            assert!(local.contains("@df.callback_test"));
        }
    }
}

#[test]
fn foreign_sync_compound_arguments_share_listener_copy_addresses() {
    let layout = LayoutC {
        size: 16,
        alignment: 8,
        leaves: vec![(0, CType::F64), (8, CType::F64)],
    };
    let mut module = Module::new();
    module.sdk_mode = true;
    module.ffi_callbacks.push(FfiCallback {
        key: "sync_compound".into(),
        body: "df.callback_test".into(),
        ret: NativeType::Prim(CType::I32),
        params: vec![NativeType::Composite(layout)],
    });
    let ir = LlvmEmitter::new(&module).emit_all();
    let body = ir.split("@df.ffi.cbentry.0(").nth(1).unwrap();
    assert!(body.contains("%end0 = ptrtoint ptr "));
    assert!(body.contains("%o0 = add i64 %end0, 0"));
    assert!(body.contains("@dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 1)"));
    assert!(body.contains("@dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 1)"));
}

#[test]
fn closed_compound_callback_zeroes_direct_and_sret_abi_results() {
    for size in [8, 24] {
        let layout = LayoutC {
            size,
            alignment: 8,
            leaves: (0..size / 8).map(|i| (i * 8, CType::I64)).collect(),
        };
        let mut module = Module::new();
        module.sdk_mode = true;
        module.ffi_callbacks.push(FfiCallback {
            key: "test_compound".into(),
            body: "df.callback_test".into(),
            ret: NativeType::Composite(layout),
            params: vec![],
        });
        let ir = LlvmEmitter::new(&module).emit_all();
        let body = ir.split("@df.ffi.cbentry.0(").nth(1).unwrap();
        let closed = body.split("active_callback:").next().unwrap();
        if closed.contains("sret(") {
            assert!(closed.contains(&format!(
                "@llvm.memset.p0.i64(ptr %sret, i8 0, i64 {size}, i1 false)"
            )));
            assert!(closed.contains("ret void"));
        } else {
            assert!(closed.contains("zeroinitializer"));
        }
        assert!(!closed.contains("@dartforge_ffi_callback_exit"));
        assert!(!closed.contains("@df.callback_test"));
    }
}

#[test]
fn owned_factory_preserves_method_table_registration() {
    for owned in [false, true] {
        let name = if owned {
            "dartforge_arc_object_owned_v1"
        } else {
            "dartforge_object_new"
        };
        let mut m = Module::new();
        m.arc_memory = true;
        m.table_functions.insert(32001, "df.mt.C".into());
        m.functions.push(function(
            "criar",
            vec![],
            Type::Ref,
            vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(0),
                    Instruction::CallRuntime {
                        name: name.into(),
                        args: vec![
                            (Operand::Constant(Constant::Int(32001)), Type::I64),
                            (Operand::Constant(Constant::Int(40)), Type::I64),
                        ],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        ));
        let ir = LlvmEmitter::new(&m).emit_all();
        let body = body_of(&ir, "criar");
        let expected = if owned {
            "dartforge_arc_object_owned_t_v1"
        } else {
            "dartforge_object_new_t"
        };
        assert!(
            ir.contains(&format!("declare i64 @{expected}(i64, i64, ptr)")),
            "{ir}"
        );
        assert!(
            body.contains(&format!(
                "call i64 @{expected}(i64 32001, i64 40, ptr @df.mt.C)"
            )),
            "{body}"
        );
        assert!(ir.contains("declare ptr @df.mt.C()"), "{ir}");
    }
}

#[test]
fn typed_fields_keep_inline_llvm_type_and_extension() {
    let limit = dartforge_runtime::layout::INLINE_FIELDS;
    for index in [0, limit] {
        for (ty, llvm_ty, conversion) in [
            (Type::Ref, "i64", None),
            (Type::I64, "i64", None),
            (Type::F64, "double", Some("bitcast i64 %fb1 to double")),
            (Type::I1, "i1", Some("trunc i64 %fb1 to i1")),
            (Type::I8, "i8", Some("trunc i64 %fb1 to i8")),
        ] {
            let read = Instruction::GetField {
                object: Operand::Val(ValueId(0)),
                index,
            };
            assert_eq!(LlvmEmitter::result_type(&read, ty), ty);
            let ir = emit(function(
                "campo_tipado",
                vec![(ValueId(0), "obj".into(), Type::Ref)],
                ty,
                vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(1), read, ty)],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                }],
            ));
            let body = body_of(&ir, "campo_tipado");
            assert!(body.contains(&format!("ret {llvm_ty} %v1")), "{body}");
            if let Some(conversion) = conversion {
                assert!(body.contains(conversion), "{body}");
            }
            if index == limit {
                assert!(
                    body.contains(&format!("@dartforge_object_get(i64 %v0, i64 {limit})")),
                    "{body}"
                );
            } else {
                assert!(body.contains("load i64"), "{body}");
            }
        }
    }
}

fn function(
    symbol: &str,
    params: Vec<(ValueId, String, Type)>,
    return_ty: Type,
    blocks: Vec<BasicBlock>,
) -> Function {
    Function {
        symbol: symbol.to_string(),
        name: symbol.to_string(),
        debug_info: None,
        params,
        return_ty,
        blocks,
    }
}

fn emit(f: Function) -> String {
    let mut m = Module::new();
    m.functions.push(f);
    LlvmEmitter::new(&m).emit_all()
}

fn body_of<'a>(ir: &'a str, symbol: &str) -> &'a str {
    let ini = ir.find(&format!("@{symbol}(")).expect("emitted function");
    let end = ir[ini..].find("\n}\n").map_or(ir.len(), |f| ini + f);
    &ir[ini..end]
}

#[test]
fn arc_global_loads_real_bits_and_publishes_before_transferring_owner() {
    let slot = StrongSlot::Global {
        symbol: "dfg.recurso".into(),
    };
    let f = function(
        "arc_global",
        vec![(ValueId(0), "x".into(), Type::Ref)],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(1),
                    Instruction::ArcStoreStrong {
                        slot: slot.clone(),
                        value: Operand::Val(ValueId(0)),
                        mode: StrongStoreMode::Copy,
                    },
                    Type::Void,
                ),
                (
                    ValueId(2),
                    Instruction::ArcLoadStrong { slot: slot.clone() },
                    Type::Ref,
                ),
                (
                    ValueId(3),
                    Instruction::ArcStoreStrong {
                        slot,
                        value: Operand::Val(ValueId(2)),
                        mode: StrongStoreMode::Move,
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    );
    let mut m = Module::new();
    m.globals.push((0, Type::Ref, "dfg.recurso".into()));
    m.functions.push(f);
    assert!(crate::lower::verifier::verify(&m).is_empty());
    crate::optimize::optimize(&mut m);
    let ir = LlvmEmitter::new(&m).emit_all();
    let body = body_of(&ir, "arc_global");
    assert!(body.contains("%v2 = load i64, ptr %ga2"));
    assert!(body.contains("@dartforge_arc_retain(i64 %v2)"));
    assert!(
        body.find("@dartforge_arc_retain(i64 %v0)").unwrap()
            < body.find("store i64 %v0, ptr %ga1").unwrap()
    );
    assert!(
        body.find("store i64 %v2, ptr %ga3").unwrap()
            < body.find("@dartforge_arc_global_receive_v1").unwrap()
    );
    assert!(!body.contains("@dartforge_arc_release(i64 %v2)"));
    m.globals[0].1 = Type::I64;
    assert!(!crate::lower::verifier::verify(&m).is_empty());
    m.globals.clear();
    assert!(!crate::lower::verifier::verify(&m).is_empty());
    m.globals.extend([
        (0, Type::Ref, "dfg.recurso".into()),
        (1, Type::Ref, "dfg.recurso".into()),
    ]);
    assert!(!crate::lower::verifier::verify(&m).is_empty());
}

#[test]
fn emitted_arc_slots_preserve_descriptor_and_modes_after_optimizing() {
    let slot = StrongSlot::Frame {
        frame: Operand::Val(ValueId(0)),
        index: 7,
    };
    let f = function(
        "arc_slots",
        vec![
            (ValueId(0), "q".into(), Type::I64),
            (ValueId(1), "x".into(), Type::Ref),
        ],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(2),
                    Instruction::ArcStoreStrong {
                        slot: slot.clone(),
                        value: Operand::Val(ValueId(1)),
                        mode: StrongStoreMode::Copy,
                    },
                    Type::Void,
                ),
                (
                    ValueId(3),
                    Instruction::ArcLoadStrong { slot: slot.clone() },
                    Type::Ref,
                ),
                (
                    ValueId(4),
                    Instruction::ArcStoreStrong {
                        slot,
                        value: Operand::Val(ValueId(3)),
                        mode: StrongStoreMode::Move,
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    );
    let mut m = Module::new();
    m.functions.push(f);
    assert!(crate::lower::verifier::verify(&m).is_empty());
    crate::optimize::optimize(&mut m);
    let ir = LlvmEmitter::new(&m).emit_all();
    let body = body_of(&ir, "arc_slots");
    assert!(body.contains("@dartforge_arc_frame_copy_v1(i64 %v0, i64 7, i64 %v1)"));
    assert!(body.contains("%v3 = call i64 @dartforge_arc_frame_load_v1(i64 %v0, i64 7)"));
    assert!(body.contains("@dartforge_arc_frame_receive_v1(i64 %v0, i64 7, i64 %v3)"));
    assert!(!body.contains("@dartforge_arc_retain"));
    assert!(!body.contains("@dartforge_arc_release"));
    m.functions[0].params[0].2 = Type::Ptr;
    assert!(!crate::lower::verifier::verify(&m).is_empty());
}

#[test]
fn arc_operations_preserve_count_and_move_in_optimization() {
    let f = function(
        "arc_linear",
        vec![(ValueId(0), "x".into(), Type::Ref)],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(1),
                    Instruction::ArcCopy {
                        value: Operand::Val(ValueId(0)),
                    },
                    Type::Ref,
                ),
                (
                    ValueId(2),
                    Instruction::ArcMove {
                        value: Operand::Val(ValueId(1)),
                    },
                    Type::Ref,
                ),
                (
                    ValueId(3),
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(2)),
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    );
    let mut m = Module::new();
    m.functions.push(f);
    assert!(crate::lower::verifier::verify(&m).is_empty());
    crate::optimize::optimize(&mut m);
    assert!(crate::lower::verifier::verify(&m).is_empty());
    let ir = LlvmEmitter::new(&m).emit_all();
    let body = body_of(&ir, "arc_linear");
    assert_eq!(body.matches("call void @dartforge_arc_retain").count(), 1);
    assert_eq!(body.matches("call void @dartforge_arc_release").count(), 1);
    assert!(body.contains("%v2 = add i64 %v1, 0"));
    assert!(body.contains("@dartforge_arc_release(i64 %v2)"));
    assert!(!body.contains("@dartforge_box"));
}

#[test]
fn inlining_remaps_arc_operand_and_preserves_return_token() {
    let donate = function(
        "arc_doar",
        vec![(ValueId(0), "x".into(), Type::Ref)],
        Type::Ref,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                ValueId(1),
                Instruction::ArcCopy {
                    value: Operand::Val(ValueId(0)),
                },
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
        }],
    );
    let call = function(
        "arc_chamar",
        vec![(ValueId(10), "x".into(), Type::Ref)],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(11),
                    Instruction::CallStatic {
                        symbol: "arc_doar".into(),
                        args: vec![Operand::Val(ValueId(10))],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(12),
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(11)),
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    );
    let mut m = Module::new();
    m.functions = vec![donate, call];
    crate::optimize::optimize(&mut m);
    assert!(crate::lower::verifier::verify(&m).is_empty());
    let ir = LlvmEmitter::new(&m).emit_all();
    let body = body_of(&ir, "arc_chamar");
    assert!(!body.contains("call i64 @arc_doar"));
    assert!(body.contains("@dartforge_arc_retain(i64 %v10)"));
    assert_eq!(body.matches("call void @dartforge_arc_retain").count(), 1);
    assert_eq!(body.matches("call void @dartforge_arc_release").count(), 1);
}

#[test]
fn arc_operations_reject_unevaluated_literal_scalar_and_wrong_result() {
    for operation in 0..3 {
        let expected = if operation == 2 {
            Type::Void
        } else {
            Type::Ref
        };
        let wrong = if expected == Type::Void {
            Type::Ref
        } else {
            Type::Void
        };
        for (value, parameter, result) in [
            (Operand::Val(ValueId(0)), Type::I64, expected),
            (
                Operand::Constant(Constant::String("literal".into())),
                Type::Ref,
                expected,
            ),
            (Operand::Val(ValueId(0)), Type::Ref, wrong),
        ] {
            let inst = match operation {
                0 => Instruction::ArcCopy { value },
                1 => Instruction::ArcMove { value },
                _ => Instruction::ArcDrop { value },
            };
            assert_eq!(LlvmEmitter::result_type(&inst, result), expected);
            let f = function(
                "arc_invalida",
                vec![(ValueId(0), "x".into(), parameter)],
                Type::Void,
                vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(1), inst, result)],
                    terminator: Terminator::Return(None),
                }],
            );
            let mut m = Module::new();
            m.functions.push(f);
            let errors = crate::lower::verifier::verify(&m);
            assert!(
                errors.iter().any(|e| e.contains("ARC operation")),
                "{errors:?}"
            );
        }
    }
}

#[test]
fn arc_entry_rejects_incompatible_abi_before_registrations() {
    for sdk in [false, true] {
        for arc in [false, true] {
            let mut m = Module::new();
            m.sdk_mode = sdk;
            m.arc_memory = arc;
            let ir = LlvmEmitter::new(&m).emit_all();
            let body = body_of(
                &ir,
                if sdk {
                    "df.prepare_isolate"
                } else {
                    "dartforge_entry"
                },
            );
            if !arc {
                assert!(!body.contains("@dartforge_arc_verify_abi"));
                continue;
            }
            let activate = body.find("@dartforge_memory_arc_v1()").unwrap();
            let check = body.find("@dartforge_arc_verify_abi(i64 1)").unwrap();
            let compatible = body.find("df.arc.compatible:\n").unwrap();
            assert!(activate < check && check < compatible);
            assert!(body.contains("icmp eq i8 %df.arc.abi, 1"));
            assert!(body.contains("label %df.arc.compatible, label %df.arc.incompatible"));
            assert!(body.contains("df.arc.incompatible:\n  call void @llvm.trap()\n  unreachable"));
            assert!(ir.contains("declare void @llvm.trap()"));
        }
    }
}

#[test]
fn wtf8_literal_preserves_lone_surrogate_in_ir() {
    let f = function(
        "literal_wtf8",
        vec![],
        Type::Ref,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                ValueId(0),
                Instruction::Const(Constant::StringWtf8(vec![0xED, 0xA0, 0xBD])),
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
        }],
    );
    let ir = emit(f);
    assert!(ir.contains("[3 x i8] c\"\\ED\\A0\\BD\""), "{ir}");
    assert!(
        ir.contains("@dartforge_string_new(ptr @.str.0, i64 3)"),
        "{ir}"
    );
    assert!(!ir.contains("\\EF\\BF\\BD"), "surrogate was replaced: {ir}");
}

/// §2.11: in AOT (`static_objects`), the literal is a static object of the
/// module — a `_TwoByteString` (cid 7) with the lone surrogate as a code unit, the
/// header `PERMANENT | RAW | w | cid` and the VM hash —, with no call nor
/// cache at the use site.
#[test]
fn static_literal_in_aot() {
    use dartforge_runtime::layout;
    let f = function(
        "literal_estatico",
        vec![],
        Type::Ref,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                ValueId(0),
                Instruction::Const(Constant::StringWtf8(vec![0xED, 0xA0, 0xBD])),
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
        }],
    );
    let mut m = Module::new();
    m.functions.push(f);
    let ir = LlvmEmitter::new(&m).with_static_objects(true).emit_all();
    let body = body_of(&ir, "literal_estatico");
    assert!(!body.contains("@dartforge_string_new("), "{body}");
    // The name of the global is `@"df.s.<blake3>"` (quoted: it has dots).
    assert!(ir.contains("df.s."), "{ir}");
    assert!(ir.contains("linkonce_odr"), "{ir}");
    let header = layout::header_word(
        layout::state::PERMANENT,
        layout::flags::RAW,
        layout::string_words(1, true),
        layout::cid::TWO_BYTE_STRING,
    );
    assert!(
        ir.contains(&format!("i64 {}", header as i64)),
        "header {header:#x}: {ir}"
    );
    let hash = layout::string_hash([0xD83Du16]);
    assert!(ir.contains(&format!("i64 {hash}")), "hash {hash}: {ir}");
}

/// G1/G3: a function with a live `Ref` at a collection point opens the frame, roots
/// the parameter (an argument of the allocating call) and closes the frame before
/// EVERY `ret` — including the one of the exception exit. The result of the call is only
/// used in the `ret`, with no collection in between: it gets no root (`roots.rs`).
#[test]
fn roots_frame_on_every_ret() {
    let v0 = ValueId(0);
    let v1 = ValueId(1);
    let f = function(
        "f",
        vec![(v0, "s".to_string(), Type::Ref)],
        Type::Ref,
        vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    v1,
                    Instruction::CallRuntime {
                        name: "dartforge_string_concat".to_string(),
                        args: vec![(Operand::Val(v0), Type::Ref), (Operand::Val(v0), Type::Ref)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::CondBranch {
                    cond: Operand::Constant(Constant::Bool(true)),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(v1))),
            },
            // The exception exit returns the default value through the same `ret`.
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
        ],
    );
    let ir = emit(f);
    let body = body_of(&ir, "f");
    assert!(
        body.contains("%gcq = alloca { ptr, i64, [1 x i64] }")
            && body.contains("%ctx = call ptr @dartforge_context()")
            && body.contains("store ptr %gcq, ptr %ctxtopo"),
        "{body}"
    );
    assert!(body.contains("store i64 %v0, ptr %gcs0"), "{body}");
    assert!(!body.contains("store i64 %v1, ptr"), "{body}");
    // The exit of the stack check (C22) returns before the frame is
    // linked: nothing to close there.
    let (prologue, rest) = body
        .split_once("pilha.ok:")
        .expect("stack check in the prologue");
    assert!(
        prologue.contains("pilha.estouro:")
            && prologue.contains("call void @dartforge_stack_overflow()"),
        "{body}"
    );
    assert!(
        !prologue.contains("store ptr %gcq, ptr %ctxtopo"),
        "the frame cannot be chained on overflow: {body}"
    );
    assert_eq!(prologue.matches("\n  ret ").count(), 1, "{body}");
    let rets = rest.matches("\n  ret ").count();
    let pops = rest.matches("store ptr %gcvolta").count();
    assert_eq!(rets, 2, "{body}");
    assert_eq!(pops, rets, "every ret closes the frame: {body}");
}

/// A call that allocates.
fn concat(a: ValueId, b: ValueId) -> Instruction {
    Instruction::CallRuntime {
        name: "dartforge_string_concat".to_string(),
        args: vec![(Operand::Val(a), Type::Ref), (Operand::Val(b), Type::Ref)],
        ret_ty: Type::Ref,
    }
}

/// Liveness: `v1` crosses the second call (it is used after it) and
/// gets a root; `v0` dies at the first and `v1` is born from it, so the two
/// share the slot; `v2` only goes to the `ret`.
#[test]
fn roots_by_liveness_and_shared_slot() {
    let (v0, v1, v2, v3) = (ValueId(0), ValueId(1), ValueId(2), ValueId(3));
    let f = function(
        "g",
        vec![(v0, "s".to_string(), Type::Ref)],
        Type::Ref,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (v1, concat(v0, v0), Type::Ref),
                (v2, concat(v1, v1), Type::Ref),
                (v3, concat(v2, v1), Type::Ref),
            ],
            terminator: Terminator::Return(Some(Operand::Val(v3))),
        }],
    );
    let ir = emit(f);
    let body = body_of(&ir, "g");
    // v0 and v1 in slot 0 (they do not interfere: v0 dies at the definition of v1); v2
    // is an argument of the third call together with v1, so it interferes and goes
    // to slot 1; v3 does not go through a collection.
    assert!(body.contains("[2 x i64]"), "{body}");
    assert!(body.contains("store i64 %v0, ptr %gcs0"), "{body}");
    assert!(body.contains("store i64 %v1, ptr %gcs0"), "{body}");
    assert!(body.contains("store i64 %v2, ptr %gcs1"), "{body}");
    assert!(!body.contains("store i64 %v3, ptr"), "{body}");
}

/// G: a function without a `Ref` value opens no frame.
#[test]
fn without_ref_without_frame() {
    let v0 = ValueId(0);
    let f = function(
        "g",
        vec![(v0, "n".to_string(), Type::I64)],
        Type::I64,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![],
            terminator: Terminator::Return(Some(Operand::Val(v0))),
        }],
    );
    let ir = emit(f);
    assert!(!body_of(&ir, "g").contains("gc_push"));
}

/// G2: a `store` into a `Ref` local updates the slot of the `alloca`.
#[test]
fn ref_local_has_own_slot() {
    let (p, v0, v1) = (ValueId(0), ValueId(1), ValueId(2));
    let f = function(
        "h",
        vec![],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (p, Instruction::Alloca(Type::Ref), Type::Ptr),
                (
                    v0,
                    Instruction::Const(Constant::String("x".to_string())),
                    Type::Ref,
                ),
                (
                    v1,
                    Instruction::Store {
                        ptr: Operand::Val(p),
                        val: Operand::Val(v0),
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    );
    let ir = emit(f);
    let body = body_of(&ir, "h");
    assert!(body.contains("store i64 %v1, ptr %v0"), "{body}");
    assert!(body.contains("store i64 %v1, ptr %gcs0"), "{body}");
}

/// E1: writing a `Ref` into a field carries `is_ref = 1`; a scalar, 0 — inline,
/// in the `(bits, is_ref)` pair of the field vector.
#[test]
fn ref_field_carries_is_ref() {
    let (v0, v1) = (ValueId(0), ValueId(1));
    let f = function(
        "k",
        vec![
            (v0, "o".to_string(), Type::Ref),
            (v1, "s".to_string(), Type::Ref),
        ],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(2),
                    Instruction::SetField {
                        object: Operand::Val(v0),
                        index: 0,
                        value: Operand::Val(v1),
                    },
                    Type::Void,
                ),
                (
                    ValueId(3),
                    Instruction::SetField {
                        object: Operand::Val(v0),
                        index: 1,
                        value: Operand::Constant(Constant::Int(7)),
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    );
    let ir = emit(f);
    let body = body_of(&ir, "k");
    // The address of the fields comes from the object itself (`h + 14`), without a call.
    assert!(!body.contains("@dartforge_object_fields("), "{body}");
    // The header at `h - 2`; the fields, 8-byte words after it; the
    // `is_ref` is the bit of the field in the header map.
    assert!(body.contains("%fxa2 = add i64 %v0, -2"), "{body}");
    assert!(
        body.contains("%fp2 = getelementptr inbounds i8, ptr %fcb2, i64 16"),
        "{body}"
    );
    assert!(
        body.contains("ptr %fp2, i64 0\n  store i64 %v1, ptr %fg2"),
        "{body}"
    );
    assert!(body.contains("%fms2 = or i32 %fm2, 1"), "{body}");
    assert!(body.contains("store i64 7, ptr %fg3"), "{body}");
    assert!(
        body.contains("%fmc3 = and i32 %fm3, -3\n  store i32 %fmc3"),
        "{body}"
    );
    assert!(!body.contains("@dartforge_object_set("), "{body}");
}

/// E3: the verifier rejects an integer constant in a `Ref` position and a tag
/// inconsistent with the representation (the flat `(bits, tag)` ABI of
/// throwing).
#[test]
fn verifier_rejects_integer_as_ref_and_wrong_tag() {
    let v0 = ValueId(0);
    let f = function(
        "m",
        vec![(v0, "l".to_string(), Type::Ref)],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(1),
                    Instruction::CallRuntime {
                        name: "dartforge_print_handle".to_string(),
                        args: vec![(Operand::Constant(Constant::Int(0)), Type::Ref)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
                (
                    ValueId(2),
                    Instruction::CallRuntime {
                        name: "dartforge_exception_throw".to_string(),
                        args: vec![
                            (Operand::Constant(Constant::Int(5)), Type::I64),
                            (Operand::Constant(Constant::Int(3)), Type::I8),
                        ],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    );
    let mut m = Module::new();
    m.functions.push(f);
    let errors = crate::lower::verifier::verify(&m);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("integer constant in a Ref position")),
        "{errors:?}"
    );
    assert!(
        errors.iter().any(|e| e.contains("tag 3 for a I64 value")),
        "{errors:?}"
    );
}

/// Risk 12 (docs/NATIVO-ESPACO-UNIFICADO.md §6): the capture read in the
/// representation it was written with; the positional record only with `Ref`; and
/// the instructions that left with the unified space (`AllocMap`, `AllocSet`).
#[test]
fn verifier_checks_captures_records_and_removed_instructions() {
    let (x, env, clo) = (ValueId(0), ValueId(1), ValueId(2));
    // `f(x)` creates the closure `g` with the environment `[x: I64, I64 cell]` and a
    // record with an `I64`.
    let cel = ValueId(3);
    let creator = function(
        "f",
        vec![(x, "x".to_string(), Type::I64)],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    cel,
                    Instruction::AllocCell {
                        value: Operand::Val(x),
                    },
                    Type::Ref,
                ),
                (
                    env,
                    Instruction::AllocEnv {
                        values: vec![Operand::Val(x), Operand::Val(cel)],
                    },
                    Type::Ref,
                ),
                (
                    clo,
                    Instruction::AllocClosure {
                        code_symbol: "g".to_string(),
                        env: Operand::Val(env),
                    },
                    Type::Ref,
                ),
                (
                    ValueId(4),
                    Instruction::AllocRecord {
                        elements: vec![(Operand::Val(x), 1)],
                    },
                    Type::Ref,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    );
    // `g` reads capture 0 as `Ref` (written as `I64`) and the cell as `F64`.
    let (e, c0, c1, c2) = (ValueId(0), ValueId(1), ValueId(2), ValueId(3));
    let body = function(
        "g",
        vec![(e, "env".to_string(), Type::Ref)],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    c0,
                    Instruction::EnvGet {
                        env: Operand::Val(e),
                        index: 0,
                    },
                    Type::Ref,
                ),
                (
                    c1,
                    Instruction::EnvGet {
                        env: Operand::Val(e),
                        index: 1,
                    },
                    Type::Ref,
                ),
                (
                    c2,
                    Instruction::CellGet {
                        cell: Operand::Val(c1),
                    },
                    Type::F64,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    );
    let mut m = Module::new();
    m.functions.push(creator);
    m.functions.push(body);
    let errors = crate::lower::verifier::verify(&m);
    let has = |s: &str| errors.iter().any(|e| e.contains(s));
    assert!(
        has("environment capture 0: read as Ref, written as I64"),
        "{errors:?}"
    );
    assert!(has("cell: read as F64, written as I64"), "{errors:?}");
    assert!(has("positional record: field I64"), "{errors:?}");
    // Capture 1 (the cell, `Ref`) read as `Ref`: no error.
    assert!(!has("captura 1"), "{errors:?}");
}

// Tests the body emitter on a Windows host too, without pretending Unix AOT support.
#[test]
fn cleanup_resumes_original_pair_and_rejects_incoherent_metadata() {
    let f = function(
        "retoma",
        vec![(ValueId(0), "x".into(), Type::Ref)],
        Type::Void,
        vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::ArcCopy {
                            value: Operand::Val(ValueId(0)),
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(2),
                        Instruction::CallStatic {
                            symbol: "falha".into(),
                            args: vec![],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: Operand::Constant(Constant::Bool(false)),
                    then_block: BlockId(2),
                    else_block: BlockId(1),
                },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![(
                    ValueId(4),
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(1)),
                    },
                    Type::Void,
                )],
                terminator: Terminator::Return(None),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![(
                    ValueId(3),
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(1)),
                    },
                    Type::Void,
                )],
                terminator: Terminator::Return(None),
            },
        ],
    );
    let t = FunctionTables {
        invocations: [(ValueId(2), BlockId(2))].into(),
        landing_pads: [BlockId(2)].into(),
        exits: [(BlockId(2), ExceptionExit::Resume)].into(),
        ..Default::default()
    };
    crate::optimize::arc::check_resumes(&f, &t).unwrap();
    let m = Module::new();
    let mut e = LlvmEmitter::new(&m);
    e.tab = Some(&t);
    e.emit_function(&f);
    let ir = e.out;
    assert!(ir.contains("personality ptr @dartforge_personality_cleanup_itanium"));
    #[cfg(feature = "embedded-llvm")]
    check_cleanup_objects(&ir);
    let landing_pad = &ir[ir.find("b2:").unwrap()..];
    assert!(landing_pad.contains("%lpad2 = landingpad { ptr, i32 } cleanup"));
    assert!(landing_pad.contains("resume { ptr, i32 } %lpad2"));
    assert!(!landing_pad.contains("@df.throw"));
    assert!(
        landing_pad.find("@dartforge_arc_release").unwrap() < landing_pad.find("resume").unwrap()
    );
    let mut bad = t.clone();
    bad.landing_pads.clear();
    assert!(
        crate::optimize::arc::check_resumes(&f, &bad)
            .unwrap_err()
            .contains("landing pad")
    );
    bad = t.clone();
    bad.invocations.insert(ValueId(1000), BlockId(2));
    assert!(
        crate::optimize::arc::check_resumes(&f, &bad)
            .unwrap_err()
            .contains("exactly one invoke")
    );
    let mut body = f.clone();
    body.blocks[0].terminator = Terminator::Branch(BlockId(2));
    assert!(
        crate::optimize::arc::check_resumes(&body, &t)
            .unwrap_err()
            .contains("without an invoke")
    );
    body = f.clone();
    body.blocks[2].instructions.push((
        ValueId(5),
        Instruction::CallRuntime {
            name: "dartforge_exception_clear".into(),
            args: vec![],
            ret_ty: Type::Void,
        },
        Type::Void,
    ));
    assert!(
        crate::optimize::arc::check_resumes(&body, &t)
            .unwrap_err()
            .contains("only admits ARC drops")
    );
    let mut e = LlvmEmitter::new(&m);
    e.tab = Some(&t);
    e.maps = true;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| e.emit_function(&f))).is_err()
    );
}

#[cfg(windows)]
#[test]
#[should_panic(expected = "Windows needs SEH funclets")]
fn resume_does_not_promise_seh_support_in_public_emitter() {
    let mut m = Module::new();
    m.tables.push(FunctionTables {
        exits: [(BlockId(1), ExceptionExit::Resume)].into(),
        ..Default::default()
    });
    LlvmEmitter::new(&m).emit_all();
}

#[cfg(feature = "embedded-llvm")]
fn check_cleanup_objects(ir: &str) {
    // The emitted body is verified and generates Itanium code, without using the
    // Windows header nor promising cross-compilation of Dart modules.
    let triple = if cfg!(target_arch = "aarch64") {
        "aarch64-unknown-linux-gnu"
    } else {
        "x86_64-unknown-linux-gnu"
    };
    let complete = format!(
        r#"target triple = "{triple}"
        declare i32 @dartforge_personality_cleanup_itanium(...)
        declare i32 @llvm.eh.typeid.for(ptr)
        declare ptr @dartforge_context()
        declare void @dartforge_stack_overflow()
        declare i1 @llvm.expect.i1(i1, i1)
        declare void @df.throw()
        declare void @dartforge_arc_retain(i64)
        declare void @dartforge_arc_release(i64)
        declare i64 @dartforge_effects_level() nounwind
        declare void @dartforge_effects_restore(i64) nounwind
        declare void @falha()
        {ir}"#
    );
    for optimize in [false, true] {
        let object = dartforge_llvm::generate(
            "cleanup-retoma",
            &complete,
            &dartforge_llvm::Options {
                optimize,
                format: dartforge_llvm::Format::Object,
                cpu: None,
            },
        )
        .unwrap_or_else(|error| panic!("{error}\n{complete}"));
        assert!(!object.is_empty());
    }
}

#[test]
fn catch_in_cleanup_profile_checks_selector_and_remaps_landing_pad_phi() {
    let call = |v| {
        (
            ValueId(v),
            Instruction::CallStatic {
                symbol: "falha".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        )
    };
    let drop = |v| {
        (
            ValueId(v),
            Instruction::ArcDrop {
                value: Operand::Val(ValueId(1)),
            },
            Type::Void,
        )
    };
    let branch = |p, n| Terminator::CondBranch {
        cond: Operand::Constant(Constant::Bool(false)),
        then_block: BlockId(p),
        else_block: BlockId(n),
    };
    let f = function(
        "misto",
        vec![(ValueId(0), "x".into(), Type::Ref)],
        Type::Void,
        vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::ArcCopy {
                            value: Operand::Val(ValueId(0)),
                        },
                        Type::Ref,
                    ),
                    call(2),
                ],
                terminator: branch(4, 1),
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![call(5)],
                terminator: branch(2, 3),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![drop(3)],
                terminator: Terminator::Return(None),
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![drop(4)],
                terminator: Terminator::Return(None),
            },
            BasicBlock {
                id: BlockId(4),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(5)),
            },
            BasicBlock {
                id: BlockId(5),
                instructions: vec![
                    (
                        ValueId(6),
                        Instruction::Phi {
                            incoming: vec![(BlockId(4), Operand::Constant(Constant::Int(1)))],
                            ty: Type::I64,
                        },
                        Type::I64,
                    ),
                    drop(7),
                ],
                terminator: Terminator::Return(None),
            },
        ],
    );
    let t = FunctionTables {
        invocations: [(ValueId(2), BlockId(4)), (ValueId(5), BlockId(2))].into(),
        landing_pads: [BlockId(2), BlockId(4)].into(),
        exits: [(BlockId(2), ExceptionExit::Resume)].into(),
        ..Default::default()
    };
    let failure = function(
        "falha",
        vec![],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![],
            terminator: Terminator::Return(None),
        }],
    );
    let mut functions = vec![f, failure];
    let plan = crate::optimize::arc::DartFunctionPlan {
        tables: t,
        ..Default::default()
    };
    let mut leaf = crate::optimize::arc::DartFunctionPlan::default();
    leaf.tables.checks_stack = true;
    let mut plans =
        std::collections::HashMap::from([("misto".into(), plan), ("falha".into(), leaf)]);
    assert_eq!(
        crate::optimize::arc::insert_arc_dart_functions(&mut functions, &mut plans).unwrap(),
        (0, 0)
    );
    let f = &functions[0];
    let t = &plans["misto"].tables;
    let certificate = t.foreign_cleanup.as_ref().unwrap();
    assert_eq!(certificate.owners(BlockId(4)), &[ValueId(1)]);
    let mut stale = f.clone();
    stale.name.push_str(" alterada");
    assert!(certificate.check(&stale, t).is_err());
    let mut stale_tables = t.clone();
    stale_tables.invocations.remove(&ValueId(2));
    assert!(certificate.check(f, &stale_tables).is_err());
    let m = Module::new();
    let mut e = LlvmEmitter::new(&m);
    e.tab = Some(t);
    e.emit_function(f);
    let ir = e.out;
    if super::externs::check_effects() {
        assert!(ir.contains("%efnivel = call i64 @dartforge_effects_level()"));
        let restoration = ir
            .find("call void @dartforge_effects_restore(i64 %efnivel)")
            .unwrap();
        assert!(restoration < ir.find("lpad4.estrangeira:").unwrap());
    }
    assert!(ir.contains("%lpseletor4 = extractvalue { ptr, i32 } %lpad4, 1"));
    assert!(ir.contains("%lptipo4 = call i32 @llvm.eh.typeid.for(ptr null)"));
    assert!(ir.contains("lpad4.estrangeira:\n  call void @dartforge_arc_release(i64 %v1)"));
    let foreign = ir
        .split("lpad4.estrangeira:\n")
        .nth(1)
        .unwrap()
        .split("lpad4.dart:")
        .next()
        .unwrap();
    assert!(foreign.contains("resume { ptr, i32 } %lpad4"));
    assert_eq!(foreign.matches("@dartforge_arc_release").count(), 1);
    if ir.contains("%gcant =") {
        assert!(foreign.contains("store ptr %gcant, ptr %ctxtopo"));
    }
    assert!(ir.contains("%lpad4.dart ]"), "{ir}");
    assert!(!ir.contains("%lpseletor2"));
    #[cfg(feature = "embedded-llvm")]
    check_cleanup_objects(&ir);
}

#[test]
fn foreign_cleanup_closes_local_frames_in_lifo() {
    let val = |id| Operand::Val(ValueId(id));
    let open = |id| {
        (
            ValueId(id),
            Instruction::CallRuntime {
                name: "dartforge_arc_frame_open_v1".into(),
                args: vec![(Operand::Constant(Constant::Int(1)), Type::I64)],
                ret_ty: Type::I64,
            },
            Type::I64,
        )
    };
    let close = |id, q| {
        (
            ValueId(id),
            Instruction::CallRuntime {
                name: "dartforge_arc_frame_close_v1".into(),
                args: vec![(val(q), Type::I64)],
                ret_ty: Type::Void,
            },
            Type::Void,
        )
    };
    let f = function(
        "quadros_estrangeiros",
        vec![],
        Type::Void,
        vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    open(0),
                    open(1),
                    (
                        ValueId(2),
                        Instruction::CallStatic {
                            symbol: "falha".into(),
                            args: vec![],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: Operand::Constant(Constant::Bool(false)),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![close(3, 1), close(4, 0)],
                terminator: Terminator::Return(None),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![(
                    ValueId(7),
                    Instruction::CallStatic {
                        symbol: "falha".into(),
                        args: vec![],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                )],
                terminator: Terminator::CondBranch {
                    cond: Operand::Constant(Constant::Bool(false)),
                    then_block: BlockId(3),
                    else_block: BlockId(4),
                },
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![close(5, 1), close(6, 0)],
                terminator: Terminator::Return(None),
            },
            BasicBlock {
                id: BlockId(4),
                instructions: vec![close(8, 1), close(9, 0)],
                terminator: Terminator::Return(None),
            },
        ],
    );
    let failure = function(
        "falha",
        vec![],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![],
            terminator: Terminator::Return(None),
        }],
    );
    let mut plan = crate::optimize::arc::DartFunctionPlan::default();
    plan.tables.invocations.insert(ValueId(2), BlockId(1));
    plan.tables.landing_pads.insert(BlockId(1));
    plan.tables.exits.insert(BlockId(1), ExceptionExit::Guard);
    plan.tables.invocations.insert(ValueId(7), BlockId(3));
    plan.tables.landing_pads.insert(BlockId(3));
    plan.tables.exits.insert(BlockId(3), ExceptionExit::Resume);
    let mut leaf = crate::optimize::arc::DartFunctionPlan::default();
    leaf.tables.checks_stack = true;
    let mut plans = std::collections::HashMap::from([
        ("quadros_estrangeiros".into(), plan),
        ("falha".into(), leaf),
    ]);
    let mut functions = vec![f, failure];
    crate::optimize::arc::insert_arc_dart_functions(&mut functions, &mut plans).unwrap();
    let t = &plans["quadros_estrangeiros"].tables;
    assert_eq!(
        t.foreign_cleanup.as_ref().unwrap().frames(BlockId(1)),
        &[ValueId(0), ValueId(1)]
    );
    let m = Module::new();
    let mut e = LlvmEmitter::new(&m);
    e.tab = Some(t);
    e.emit_function(&functions[0]);
    let ir = e.out;
    let excerpt = ir
        .split("lpad1.estrangeira:")
        .nth(1)
        .unwrap()
        .split("resume ")
        .next()
        .unwrap();
    let internal = excerpt
        .find("call void @dartforge_arc_frame_close_v1(i64 %v1)")
        .unwrap();
    let external = excerpt
        .find("call void @dartforge_arc_frame_close_v1(i64 %v0)")
        .unwrap();
    assert!(internal < external);
    assert_eq!(excerpt.matches("@dartforge_arc_frame_close_v1").count(), 2);
    let resume = ir
        .split("b3:\n")
        .nth(1)
        .unwrap()
        .split("resume ")
        .next()
        .unwrap();
    assert!(!resume.contains("catch ptr null"));
    assert!(
        resume
            .find("@dartforge_arc_frame_close_v1(i64 %v1)")
            .unwrap()
            < resume
                .find("@dartforge_arc_frame_close_v1(i64 %v0)")
                .unwrap()
    );
    let mut invalidates = functions[0].clone();
    invalidates
        .blocks
        .iter_mut()
        .find(|b| b.id == BlockId(3))
        .unwrap()
        .instructions
        .swap(0, 1);
    assert!(
        crate::optimize::arc::check_resumes(&invalidates, t)
            .unwrap_err()
            .contains("out of LIFO order")
    );
    let mut without_closing = functions[0].clone();
    without_closing
        .blocks
        .iter_mut()
        .find(|b| b.id == BlockId(3))
        .unwrap()
        .instructions
        .pop();
    assert!(
        crate::optimize::arc::check_resumes(&without_closing, t)
            .unwrap_err()
            .contains("frames still open")
    );
    let mut collection = functions[0].clone();
    collection
        .blocks
        .iter_mut()
        .find(|b| b.id == BlockId(3))
        .unwrap()
        .instructions
        .push((
            ValueId(10),
            Instruction::CallRuntime {
                name: "dartforge_arc_collect".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
    assert!(
        crate::optimize::arc::check_resumes(&collection, t)
            .unwrap_err()
            .contains("only admits ARC drops")
    );
    assert!(ir.contains("resume { ptr, i32 } %lpad3"));

    #[cfg(feature = "embedded-llvm")]
    {
        let complete = format!(
            "declare i64 @dartforge_arc_frame_open_v1(i64) nounwind\ndeclare void @dartforge_arc_frame_close_v1(i64) nounwind\n{ir}"
        );
        check_cleanup_objects(&complete);
    }
}

#[test]
fn declaration_inline_policies_emit_preferences_and_prohibitions() {
    for dwarf in [false, true] {
        let mut m = Module::new();
        m.dwarf = dwarf;
        for (name, policy) in [
            ("preferred", InlinePolicy::Prefer),
            ("forbidden", InlinePolicy::Never),
        ] {
            m.inline_policies.insert(name.into(), policy);
            m.functions.push(function(
                name,
                vec![],
                Type::I64,
                vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Constant(Constant::Int(1)))),
                }],
            ));
        }
        let ir = LlvmEmitter::new(&m).with_optimize_size(true).emit_all();
        let header = |name: &str| {
            ir.lines()
                .find(|l| l.starts_with("define ") && l.contains(&format!("@{name}(")))
                .unwrap()
                .to_owned()
        };
        assert!(header("forbidden").contains(" optsize noinline"));
        assert_eq!(header("preferred").contains(" inlinehint"), !dwarf);
        assert!(!header("preferred").contains("alwaysinline"));
    }
}

#[test]
fn maps_native_declarations_follow_certified_gc_effects() {
    let mut module = Module::new();
    module.sdk_mode = true;
    for maps in [false, true] {
        let mut emitter = LlvmEmitter::new(&module).with_roots_by_maps(maps);
        emitter.annotate_external(
            "dartforge_native_DartForge_string_equals_object",
            Type::I8,
            &[Type::Ref, Type::Ref],
        );
        emitter.annotate_external(
            "dartforge_native_DartForge_mirrors_name",
            Type::Ref,
            &[Type::Ref],
        );
        emitter.annotate_external(
            "dartforge_native_unclassified_test",
            Type::Ref,
            &[Type::Ref],
        );
        let ir = emitter.emit_all();
        for (name, leaf) in [
            ("dartforge_native_DartForge_string_equals_object", true),
            ("dartforge_native_DartForge_mirrors_name", false),
            ("dartforge_native_unclassified_test", false),
        ] {
            let declaration = ir
                .lines()
                .find(|line| line.starts_with("declare ") && line.contains(&format!("@{name}(")))
                .unwrap();
            assert_eq!(
                declaration.contains("\"gc-leaf-function\""),
                maps && leaf,
                "{declaration}"
            );
        }
    }
}

#[test]
fn maps_rti_evaluation_keeps_the_source_operation_effects() {
    let mut module = Module::new();
    module.sdk_mode = true;
    module.functions.push(function(
        "rti_maps_effects",
        vec![(ValueId(0), "receiver".into(), Type::Ref)],
        Type::I64,
        vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(1),
                    Instruction::CallRuntime {
                        name: "dartforge_rti_eval".into(),
                        args: vec![
                            (Operand::Constant(Constant::Int(1)), Type::I64),
                            (Operand::Val(ValueId(0)), Type::Ref),
                            (Operand::Constant(Constant::Int(1024)), Type::I64),
                            (Operand::Constant(Constant::Int(0)), Type::I64),
                        ],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                )],
                terminator: Terminator::Branch(BlockId(1)),
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![(
                    ValueId(2),
                    Instruction::Phi {
                        incoming: vec![(BlockId(0), Operand::Val(ValueId(1)))],
                        ty: Type::I64,
                    },
                    Type::I64,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
            },
        ],
    ));
    let ir = LlvmEmitter::new(&module)
        .with_roots_by_maps(true)
        .emit_all();
    let body = body_of(&ir, "rti_maps_effects");
    assert!(body.contains("call i64 @dartforge_rti_eval("), "{body}");
    assert!(!body.contains("@dartforge_rti_eval_cache("), "{body}");
    assert!(
        !body.contains("%ra1.fim"),
        "Phi predecessor must name an emitted block: {body}"
    );
}

/// The stack check compares the stack pointer of the entry
/// (`llvm.read_register`) with `Context::stack_limit`: no marker `alloca` in
/// the frame.
#[test]
fn stack_check_reads_the_stack_pointer_register() {
    let v0 = ValueId(0);
    let f = function(
        "pilha",
        vec![(v0, "s".to_string(), Type::Ref)],
        Type::Ref,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(ValueId(1), concat(v0, v0), Type::Ref)],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
        }],
    );
    let ir = emit(f);
    let body = body_of(&ir, "pilha");
    let register = if cfg!(target_arch = "x86_64") {
        "rsp"
    } else {
        "sp"
    };
    assert!(
        body.contains(&format!(
            "%pilhasp = call i64 @llvm.read_register.i64(metadata !{{!\"{register}\"}})"
        )),
        "{body}"
    );
    assert!(
        body.contains("%pilhal = load i64, ptr %pilhalp")
            && body.contains("%pilhab = icmp ult i64 %pilhasp, %pilhal"),
        "{body}"
    );
    assert!(
        !body.contains("%pilhaq") && !body.contains("alloca i8, align 1"),
        "{body}"
    );
    assert!(
        ir.contains("declare i64 @llvm.read_register.i64(metadata)"),
        "{ir}"
    );
    assert!(
        ir.lines().any(|line| line.starts_with("define ")
            && line.contains("@pilha(")
            && line.contains("\"disable-tail-calls\"=\"true\"")),
        "{ir}"
    );
}

/// A module with class 32001 (`df.mt.C`; two fields: a `Ref` and an `int`,
/// declared when `declared`) and a function that allocates it inline and writes
/// field 0 with a `Ref`, field 1 with an `int` and then field 1 with a `Ref`.
fn allocation_module(declared: bool) -> Module {
    let mut m = Module::new();
    m.table_functions.insert(32001, "df.mt.C".into());
    if declared {
        let field = |name: &str, index: usize, storage_type: Type| {
            Some(FieldDeclaration {
                library_uri: "package:x/x.dart".into(),
                owner: "C".into(),
                name: name.into(),
                is_final: false,
                is_late: false,
                layout_index: index,
                storage_type,
            })
        };
        m.declared_field_layouts.insert(
            32001,
            vec![field("r", 0, Type::Ref), field("n", 1, Type::I64)],
        );
    }
    let (p, v0) = (ValueId(0), ValueId(1));
    m.functions.push(function(
        "alocar",
        vec![(p, "s".into(), Type::Ref)],
        Type::Ref,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    v0,
                    Instruction::CallRuntime {
                        name: "dartforge_object_new".into(),
                        args: vec![
                            (Operand::Constant(Constant::Int(32001)), Type::I64),
                            (Operand::Constant(Constant::Int(2)), Type::I64),
                        ],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(2),
                    Instruction::SetField {
                        object: Operand::Val(v0),
                        index: 0,
                        value: Operand::Val(p),
                    },
                    Type::Void,
                ),
                (
                    ValueId(3),
                    Instruction::SetField {
                        object: Operand::Val(v0),
                        index: 1,
                        value: Operand::Constant(Constant::Int(7)),
                    },
                    Type::Void,
                ),
                (
                    ValueId(4),
                    Instruction::SetField {
                        object: Operand::Val(v0),
                        index: 1,
                        value: Operand::Val(p),
                    },
                    Type::Void,
                ),
                (
                    ValueId(5),
                    Instruction::SetField {
                        object: Operand::Val(v0),
                        index: 0,
                        value: Operand::Constant(Constant::Null),
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(Some(Operand::Val(v0))),
        }],
    ));
    m
}

/// Production registers every assembled table at startup
/// (`with_tables_registered_at_startup`): the inline allocation keeps only the
/// TLAB test and the slow path still cites the table (the pruning edge). In
/// both profiles the dead `store i64 0` of the first field is gone.
#[test]
fn inline_allocation_without_registration_test_when_tables_registered_at_startup() {
    let m = allocation_module(false);
    let lazy = LlvmEmitter::new(&m).emit_all();
    let body = body_of(&lazy, "alocar");
    assert!(
        body.contains("ao1.reg:") && body.contains("%trv1 = load i8"),
        "{body}"
    );
    assert!(!body.contains("store i64 0, ptr %tpp1"), "{body}");
    let eager = LlvmEmitter::new(&m)
        .with_tables_registered_at_startup(true)
        .emit_all();
    let body = body_of(&eager, "alocar");
    assert!(
        !body.contains("ao1.reg:") && !body.contains("%trv1") && !body.contains("%tn1 = load i64"),
        "{body}"
    );
    assert!(
        body.contains("br i1 %tx1, label %ao1.lento, label %ao1.rapido"),
        "{body}"
    );
    assert!(
        body.contains("%tl1 = call i64 @dartforge_object_new_t(i64 32001, i64 2, ptr @df.mt.C)"),
        "{body}"
    );
    assert!(!body.contains("store i64 0, ptr %tpp1"), "{body}");
}

/// Level A of the static reference map, in production: the allocation of a
/// class with a declared layout writes the map of its fields (fast path: a
/// store in the header; slow path: `dartforge_object_new_mt`), the stores into
/// the fresh object whose `is_ref` agrees with the map do not touch it, and a
/// store that disagrees still corrects the bit. Without the layout, or outside
/// production, the map stays by store.
#[test]
fn static_reference_map_at_allocation_and_fresh_stores_without_rmw() {
    let m = allocation_module(true);
    let ir = LlvmEmitter::new(&m)
        .with_production(true)
        .with_tables_registered_at_startup(true)
        .emit_all();
    let body = body_of(&ir, "alocar");
    assert!(
        body.contains(
            "%tmp1 = getelementptr inbounds i8, ptr %tb1, i64 8\n  store i32 1, ptr %tmp1, align 4"
        ),
        "{body}"
    );
    assert!(
        body.contains("call i64 @dartforge_object_new_mt(i64 32001, i64 2, i64 1, ptr @df.mt.C)"),
        "{body}"
    );
    assert!(
        body.contains("store i64 %v0, ptr %fg2") && body.contains("store i64 7, ptr %fg3"),
        "{body}"
    );
    assert!(
        !body.contains("%fm2 = load i32") && !body.contains("%fm3 = load i32"),
        "{body}"
    );
    assert!(
        body.contains("%fms4 = or i32 %fm4, 2"),
        "a store that disagrees with the map corrects the bit: {body}"
    );
    // Null into a fresh field: the zero of the handout, nothing to store; the
    // bit agrees with the map, nothing to update.
    assert!(!body.contains("%fg5") && !body.contains("%fm5"), "{body}");
    assert!(
        ir.contains("declare i64 @dartforge_object_new_mt(i64, i64, i64, ptr)"),
        "{ir}"
    );
    // Without the declared layout: map 0 and the read-modify-write by store.
    let unknown = LlvmEmitter::new(&allocation_module(false))
        .with_production(true)
        .emit_all();
    let body = body_of(&unknown, "alocar");
    assert!(
        !body.contains("%tmp1")
            && body.contains("%fms2 = or i32 %fm2, 1")
            && body.contains("%fmc3 = and i32 %fm3, -3"),
        "{body}"
    );
    // The null store into the fresh object is still not emitted, its bit is.
    assert!(
        !body.contains("%fg5") && body.contains("%fms5 = or i32 %fm5, 1"),
        "{body}"
    );
    assert!(
        body.contains("call i64 @dartforge_object_new_t(i64 32001, i64 2, ptr @df.mt.C)"),
        "{body}"
    );
    // Development: the same, even with the layout.
    let development = LlvmEmitter::new(&m).emit_all();
    let body = body_of(&development, "alocar");
    assert!(
        !body.contains("%tmp1") && body.contains("%fms2 = or i32 %fm2, 1"),
        "{body}"
    );
    assert!(!body.contains("dartforge_object_new_m"), "{body}");
}

/// `AllocObject` in production: the map of the written fields in one word
/// (`dartforge_object_new_m`), no read-modify-write per field.
#[test]
fn alloc_object_writes_the_map_once_in_production() {
    let p = ValueId(0);
    let mut m = Module::new();
    m.functions.push(function(
        "quadro",
        vec![(p, "s".into(), Type::Ref)],
        Type::Ref,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                ValueId(1),
                Instruction::AllocObject {
                    class_id: 32001,
                    fields: vec![
                        Operand::Val(p),
                        Operand::Constant(Constant::Int(7)),
                        Operand::Val(p),
                    ],
                },
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
        }],
    ));
    let production = LlvmEmitter::new(&m).with_production(true).emit_all();
    let body = body_of(&production, "quadro");
    assert!(
        body.contains("call i64 @dartforge_object_new_m(i64 32001, i64 3, i64 5)"),
        "{body}"
    );
    assert!(!body.contains("load i32"), "{body}");
    let development = LlvmEmitter::new(&m).emit_all();
    let body = body_of(&development, "quadro");
    assert!(
        body.contains("call i64 @dartforge_object_new(i64 32001, i64 3)")
            && body.contains("%fms1.0 = or i32 %fm1.0, 1"),
        "{body}"
    );
}

/// Without outside bodies (AOT, the production SDK) the address of a field keeps
/// the null/`Smi` select and drops the test of the `OUT_OF_LINE` flag.
#[test]
fn field_address_without_outside_body() {
    let p = ValueId(0);
    let mut m = Module::new();
    m.functions.push(function(
        "ler",
        vec![(p, "o".into(), Type::Ref)],
        Type::Ref,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                ValueId(1),
                Instruction::GetField {
                    object: Operand::Val(p),
                    index: 0,
                },
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
        }],
    ));
    let with_outside = LlvmEmitter::new(&m).emit_all();
    let body = body_of(&with_outside, "ler");
    assert!(
        body.contains("%fxh1 = select i1 %fxo1, ptr %fxq1, ptr %fxv1")
            && body.contains("%fcb1 = select i1 %fxfo1, ptr %fxc1, ptr %fxh1"),
        "{body}"
    );
    let without = LlvmEmitter::new(&m)
        .with_body_without_outside(true)
        .emit_all();
    let body = body_of(&without, "ler");
    assert!(
        body.contains("%fcb1 = select i1 %fxo1, ptr %fxq1, ptr %fxv1"),
        "{body}"
    );
    assert!(
        !body.contains("%fxf1") && !body.contains("%fxc1") && !body.contains("%fxh1"),
        "{body}"
    );
    assert!(
        body.contains("%fp1 = getelementptr inbounds i8, ptr %fcb1, i64 16"),
        "{body}"
    );
}

/// The call of a function value reads the entry of a `_Closure` inline (class
/// `CLOSURE`, field 0) and only goes to `dartforge_closure_entry` for any
/// other value; a null entry still falls to `@df_clo_invalido`.
#[test]
fn closure_call_reads_the_entry_inline() {
    let p = ValueId(0);
    let f = function(
        "chamar",
        vec![(p, "f".into(), Type::Ref)],
        Type::Ref,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                ValueId(1),
                Instruction::CallClosure {
                    closure: Operand::Val(p),
                    args: vec![],
                    names: vec![],
                    ret_ty: Type::Ref,
                    tuple_types: Operand::Constant(Constant::Int(0)),
                },
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
        }],
    );
    let ir = emit(f);
    let body = body_of(&ir, "chamar");
    let closure = dartforge_runtime::layout::cid::CLOSURE;
    assert!(
        body.contains("%cek1 = and i64 %v0, -9223372036854775801")
            && body.contains("br i1 %ceo1, label %ce1.obj, label %ce1.lento"),
        "{body}"
    );
    assert!(
        body.contains("%ceca1 = add i64 %v0, 2")
            && body.contains(&format!("%ceq1 = icmp eq i32 %cec1, {closure}")),
        "{body}"
    );
    assert!(body.contains("%cefa1 = add i64 %v0, 14\n  %cefp1 = inttoptr i64 %cefa1 to ptr\n  %cer1 = load i64, ptr %cefp1, align 8"), "{body}");
    assert!(
        body.contains("ce1.lento:\n  %cel1 = call i64 @dartforge_closure_entry(i64 %v0)"),
        "{body}"
    );
    assert!(
        body.contains("ce1.fim:\n  %cc1 = phi i64 [ %cer1, %ce1.rapido ], [ %cel1, %ce1.lento ]"),
        "{body}"
    );
    assert!(
        body.contains("@df_clo_invalido")
            && body.contains("call i64 %cf1(i64 %v0, ptr %cargs1, ptr @"),
        "{body}"
    );
}

#[path = "tests/static_image_registration.rs"]
mod static_image_registration;

/// A recursive function without any allocation keeps its live references out of
/// a roots frame: its calls to itself are not collection points
/// (`non_collecting_functions`). A caller that allocates keeps its frame, and
/// without the AOT guarantee (no outside bodies: a JIT reload could replace the
/// callee) nothing changes.
#[test]
fn calls_to_non_collecting_functions_are_not_collection_points() {
    let (n, inner, equal) = (ValueId(0), ValueId(1), ValueId(2));
    let walk = function(
        "walk",
        vec![(n, "n".to_string(), Type::Ref)],
        Type::Ref,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    inner,
                    Instruction::CallStatic {
                        symbol: "walk".to_string(),
                        args: vec![Operand::Val(n)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    equal,
                    Instruction::ICmp(
                        ICmpOp::Eq,
                        Operand::Val(n),
                        Operand::Constant(Constant::Null),
                    ),
                    Type::I1,
                ),
            ],
            // `n` is live after the recursive call.
            terminator: Terminator::Return(Some(Operand::Val(n))),
        }],
    );
    let (s, joined, back) = (ValueId(0), ValueId(1), ValueId(2));
    let joins = function(
        "joins",
        vec![(s, "s".to_string(), Type::Ref)],
        Type::Ref,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    joined,
                    Instruction::CallRuntime {
                        name: "dartforge_string_concat".to_string(),
                        args: vec![(Operand::Val(s), Type::Ref), (Operand::Val(s), Type::Ref)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    back,
                    Instruction::CallStatic {
                        symbol: "walk".to_string(),
                        args: vec![Operand::Val(joined)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
            ],
            terminator: Terminator::Return(Some(Operand::Val(joined))),
        }],
    );
    let mut m = Module::new();
    m.functions.push(walk);
    m.functions.push(joins);
    let aot = LlvmEmitter::new(&m)
        .with_body_without_outside(true)
        .emit_all();
    assert!(
        !body_of(&aot, "walk").contains("%gcq = alloca"),
        "{}",
        body_of(&aot, "walk")
    );
    assert!(body_of(&aot, "joins").contains("%gcq = alloca"));
    let reloadable = LlvmEmitter::new(&m).emit_all();
    assert!(body_of(&reloadable, "walk").contains("%gcq = alloca"));
}

/// In the lean area of production a store into a `Ref` global is a plain
/// `store`: the descriptor lists the slot and the collector reads it. With the
/// full area (development, JIT) the store still registers the root.
#[test]
fn lean_area_lists_ref_slots_instead_of_registering_stores() {
    let v0 = ValueId(0);
    let f = function(
        "keep",
        vec![(v0, "x".to_string(), Type::Ref)],
        Type::Void,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                ValueId(1),
                Instruction::StoreGlobal {
                    symbol: "dfg.kept".into(),
                    val: Operand::Val(v0),
                    ty: Type::Ref,
                    root: Some(0),
                },
                Type::Void,
            )],
            terminator: Terminator::Return(None),
        }],
    );
    let mut m = Module::new();
    m.globals.push((0, Type::Ref, "dfg.kept".into()));
    m.functions.push(f);
    let lean = LlvmEmitter::new(&m).with_lean_area(true).emit_all();
    assert!(!body_of(&lean, "keep").contains("dartforge_gc_global_root"));
    let descriptor = lean
        .lines()
        .find(|l| l.starts_with("@df.area = "))
        .expect("area descriptor");
    let slot = descriptor
        .rsplit("i64 ")
        .next()
        .and_then(|s| s.trim_end_matches(']').parse::<i64>().ok());
    assert!(descriptor.contains("[4 x i64]"), "{descriptor}");
    assert!(slot.is_some_and(|s| s >= 0), "{descriptor}");
    let full = LlvmEmitter::new(&m).emit_all();
    assert!(body_of(&full, "keep").contains("dartforge_gc_global_root"));
}

/// The lazy type getter of `lower::rti`, exactly as `recipe_rti` builds it.
fn rti_getter(symbol: &str, global: &str, recipe: &str) -> Function {
    let ok = format!("{global}$ok");
    function(
        symbol,
        vec![],
        Type::I64,
        vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::LoadGlobal {
                            symbol: ok.clone(),
                            ty: Type::I8,
                        },
                        Type::I8,
                    ),
                    (
                        ValueId(1),
                        Instruction::ICmp(
                            ICmpOp::Ne,
                            Operand::Val(ValueId(0)),
                            Operand::Constant(Constant::Int(0)),
                        ),
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
                        symbol: global.into(),
                        ty: Type::I64,
                    },
                    Type::I64,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![
                    (
                        ValueId(3),
                        Instruction::Const(Constant::String(recipe.into())),
                        Type::Ref,
                    ),
                    (
                        ValueId(4),
                        Instruction::CallRuntime {
                            name: "dartforge_rti_recipe".into(),
                            args: vec![(Operand::Val(ValueId(3)), Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    ),
                    (
                        ValueId(5),
                        Instruction::StoreGlobal {
                            symbol: global.into(),
                            val: Operand::Val(ValueId(4)),
                            ty: Type::I64,
                            root: None,
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(6),
                        Instruction::StoreGlobal {
                            symbol: ok,
                            val: Operand::Constant(Constant::Int(1)),
                            ty: Type::I8,
                            root: None,
                        },
                        Type::Void,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(4)))),
            },
        ],
    )
}

#[test]
fn rti_getters_share_one_fill_helper_with_static_strings() {
    let mut m = Module::new();
    for (k, recipe) in ["List<int>", "Map<String,int>"].iter().enumerate() {
        let global = format!("dfr.{k}.lib");
        m.globals.push((0, Type::I64, global.clone()));
        m.functions
            .push(rti_getter(&format!("df.rti.{k}.lib"), &global, recipe));
    }
    let ir = LlvmEmitter::new(&m).with_static_objects(true).emit_all();
    assert_eq!(
        ir.matches("define internal i64 @df.rti_fill(").count(),
        1,
        "{ir}"
    );
    for k in 0..2 {
        let body = body_of(&ir, &format!("df.rti.{k}.lib"));
        assert!(body.contains("tail call i64 @df.rti_fill(i64 "), "{body}");
        assert!(!body.contains("df.get_area"), "{body}");
    }
    assert!(ir.contains("@dartforge_rti_recipe("), "{ir}");
    // The value slot and its flag are those of the area (`$ok` follows the value).
    let first = body_of(&ir, "df.rti.0.lib");
    assert!(first.contains("@df.rti_fill(i64 0, i64 1, "), "{first}");
    // Without static strings (JIT), the getter keeps its own body.
    let jit = LlvmEmitter::new(&m).emit_all();
    assert!(!jit.contains("@df.rti_fill("), "{jit}");
    assert!(body_of(&jit, "df.rti.0.lib").contains("dartforge_rti_recipe"));
}

#[test]
fn rti_getters_with_another_shape_keep_their_body() {
    let mut m = Module::new();
    m.globals.push((0, Type::I64, "dfr.x.lib".into()));
    let mut f = rti_getter("df.rti.x.lib", "dfr.x.lib", "int");
    // Another flag value: not the shape of `recipe_rti`.
    if let Instruction::StoreGlobal { val, .. } = &mut f.blocks[2].instructions[3].1 {
        *val = Operand::Constant(Constant::Int(2));
    }
    m.functions.push(f);
    let ir = LlvmEmitter::new(&m).with_static_objects(true).emit_all();
    assert!(!ir.contains("@df.rti_fill("), "{ir}");
}

/// `build(d)`: the leaf (`d == 0`) keeps nothing alive across a collection
/// point; the other branch keeps its first call's result across the second.
/// With `in_loop`, that branch is a loop body.
fn frame_placement_fixture(in_loop: bool) -> Function {
    let (d, zero, leaf, first, second, joined, again) = (
        ValueId(0),
        ValueId(1),
        ValueId(2),
        ValueId(3),
        ValueId(4),
        ValueId(5),
        ValueId(6),
    );
    let call = |v: ValueId| {
        (
            v,
            Instruction::CallStatic {
                symbol: "build".to_string(),
                args: vec![Operand::Val(d)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )
    };
    let mut body = vec![
        call(first),
        call(second),
        (
            joined,
            Instruction::CallRuntime {
                name: "dartforge_string_concat".to_string(),
                args: vec![
                    (Operand::Val(first), Type::Ref),
                    (Operand::Val(second), Type::Ref),
                ],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ),
    ];
    let body_end = if in_loop {
        body.push((
            again,
            Instruction::ICmp(
                ICmpOp::Eq,
                Operand::Val(d),
                Operand::Constant(Constant::Int(1)),
            ),
            Type::I1,
        ));
        Terminator::CondBranch {
            cond: Operand::Val(again),
            then_block: BlockId(2),
            else_block: BlockId(3),
        }
    } else {
        Terminator::Return(Some(Operand::Val(joined)))
    };
    function(
        "build",
        vec![(d, "d".to_string(), Type::I64)],
        Type::Ref,
        vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    zero,
                    Instruction::ICmp(
                        ICmpOp::Eq,
                        Operand::Val(d),
                        Operand::Constant(Constant::Int(0)),
                    ),
                    Type::I1,
                )],
                terminator: Terminator::CondBranch {
                    cond: Operand::Val(zero),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![call(leaf)],
                terminator: Terminator::Return(Some(Operand::Val(leaf))),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: body,
                terminator: body_end,
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
            },
        ],
    )
}

#[test]
fn roots_frame_opens_where_a_slot_is_first_needed() {
    let mut m = Module::new();
    m.functions.push(frame_placement_fixture(false));
    let ir = LlvmEmitter::new(&m)
        .with_body_without_outside(true)
        .emit_all();
    let body = body_of(&ir, "build");
    let init = body.find("store { ptr, i64").expect("the frame opens");
    let b2 = body.find("\nb2:").expect("block b2");
    assert!(init > b2, "{body}");
    assert_eq!(body.matches("store { ptr, i64").count(), 1, "{body}");
    // Every return restores the top read at the entry.
    assert!(!body.contains("%gcvolta"), "{body}");
    assert_eq!(
        body.matches("store ptr %gcant, ptr %ctxtopo").count(),
        3,
        "{body}"
    );
}

#[test]
fn roots_frame_in_a_loop_opens_at_the_entry() {
    let mut m = Module::new();
    m.functions.push(frame_placement_fixture(true));
    let ir = LlvmEmitter::new(&m)
        .with_body_without_outside(true)
        .emit_all();
    let body = body_of(&ir, "build");
    let init = body.find("store { ptr, i64").expect("the frame opens");
    let b1 = body.find("\nb1:").expect("block b1");
    assert!(init < b1, "{body}");
}

/// `ty(m, h, t)`: one `dartforge_rti_eval` with the model `m`, `this` `h`, the class
/// `class` and the tuple `t`.
fn rti_eval_fixture(class: Operand) -> Function {
    let (m, h, t, r) = (ValueId(0), ValueId(1), ValueId(2), ValueId(3));
    function(
        "ty",
        vec![
            (m, "m".to_string(), Type::I64),
            (h, "h".to_string(), Type::Ref),
            (t, "t".to_string(), Type::I64),
        ],
        Type::I64,
        vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                r,
                Instruction::CallRuntime {
                    name: "dartforge_rti_eval".to_string(),
                    args: vec![
                        (Operand::Val(m), Type::I64),
                        (Operand::Val(h), Type::Ref),
                        (class, Type::I64),
                        (Operand::Val(t), Type::I64),
                    ],
                    ret_ty: Type::I64,
                },
                Type::I64,
            )],
            terminator: Terminator::Return(Some(Operand::Val(r))),
        }],
    )
}

#[test]
fn rti_eval_with_a_constant_class_goes_through_the_site_cache() {
    let mut m = Module::new();
    m.functions
        .push(rti_eval_fixture(Operand::Constant(Constant::Int(398))));
    let ir = LlvmEmitter::new(&m).emit_all();
    let body = body_of(&ir, "ty");
    assert!(
        body.contains("call i64 @df.rti_eval_site(ptr %rq"),
        "{body}"
    );
    assert!(!body.contains("@dartforge_rti_eval("), "{body}");
    assert_eq!(
        ir.matches("define internal i64 @df.rti_eval_site(").count(),
        1
    );
    // The helper keys by `this`, the tuple and the model, and evaluates on a miss.
    let helper = body_of(&ir, "df.rti_eval_site");
    assert!(helper.contains("call i64 @dartforge_rti_eval(i64 %m, i64 %h, i64 %k, i64 %t)"));
    assert_eq!(helper.matches("store i64").count(), 4, "{helper}");
}

#[test]
fn rti_eval_with_a_varying_class_is_called_directly() {
    let mut m = Module::new();
    let mut f = rti_eval_fixture(Operand::Constant(Constant::Int(0)));
    f.params.push((ValueId(9), "k".to_string(), Type::I64));
    if let Instruction::CallRuntime { args, .. } = &mut f.blocks[0].instructions[0].1 {
        args[2].0 = Operand::Val(ValueId(9));
    }
    m.functions.push(f);
    let ir = LlvmEmitter::new(&m).emit_all();
    let body = body_of(&ir, "ty");
    assert!(body.contains("@dartforge_rti_eval("), "{body}");
    assert!(!body.contains("@df.rti_eval_site("), "{body}");
}
