//! A matching name is insufficient: only closed cache bodies obtain facts.

use super::*;
use crate::hir::{BasicBlock, StrongStoreMode};

fn fixture() -> Module {
    let cache = "cache".to_string();
    let flag = "cache$ok".to_string();
    let function = Function {
        symbol: "literal_factory".into(),
        name: "const".into(),
        params: vec![],
        return_ty: Type::Ref,
        debug_info: None,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        I::LoadGlobal {
                            symbol: flag.clone(),
                            ty: Type::I8,
                        },
                        Type::I8,
                    ),
                    (
                        ValueId(1),
                        I::ICmp(
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
                    I::LoadGlobal {
                        symbol: cache.clone(),
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
                        I::CallRuntime {
                            name: "dartforge_object_new".into(),
                            args: vec![
                                (Operand::Constant(Constant::Int(315)), Type::I64),
                                (Operand::Constant(Constant::Int(0)), Type::I64),
                            ],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(4),
                        I::CallRuntime {
                            name: "dartforge_canonical_constant".into(),
                            args: vec![(Operand::Val(ValueId(3)), Type::Ref)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(5),
                        I::StoreGlobal {
                            symbol: cache.clone(),
                            val: Operand::Val(ValueId(4)),
                            ty: Type::Ref,
                            root: Some(123),
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(6),
                        I::CallRuntime {
                            name: "dartforge_mark_constant".into(),
                            args: vec![
                                (Operand::Val(ValueId(4)), Type::Ref),
                                (
                                    Operand::Constant(Constant::Function("literal_factory".into())),
                                    Type::I64,
                                ),
                            ],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(7),
                        I::StoreGlobal {
                            symbol: flag.clone(),
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
    };
    let caller = Function {
        symbol: "caller".into(),
        name: "caller".into(),
        params: vec![],
        return_ty: Type::Ref,
        debug_info: None,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![(
                ValueId(0),
                I::CallStatic {
                    symbol: function.symbol.clone(),
                    args: vec![],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
        }],
    };
    Module {
        functions: vec![function, caller],
        globals: vec![(123, Type::Ref, cache)],
        arc_fixed_ancestry: true,
        ..Module::default()
    }
}

fn known(module: &Module) -> Option<Classes> {
    let proof = analyze(&[module]);
    proof.returns.get("caller").cloned()
}

#[test]
fn cache_allocation_propagates_to_callers_without_using_getter_name() {
    let module = fixture();
    assert_eq!(known(&module), Some(Classes::from([315])));
    // Borrowed body identity still binds the caller facts to its exact code.
    let proof = analyze(&[&module]);
    assert_eq!(
        proof.values_for(&module.functions[1]).unwrap()[&ValueId(0)],
        Classes::from([315])
    );
    assert!(proof.values_for(&module.functions[1].clone()).is_none());
}

#[test]
fn cache_rejects_changed_returns_publication_and_opaque_effects() {
    let mut variants = Vec::new();
    let mut null = fixture();
    null.functions[0].blocks[1].terminator =
        Terminator::Return(Some(Operand::Constant(Constant::Null)));
    variants.push(null);
    let mut wrong_store = fixture();
    if let I::StoreGlobal { val, .. } = &mut wrong_store.functions[0].blocks[2].instructions[2].1 {
        *val = Operand::Constant(Constant::Null);
    }
    variants.push(wrong_store);
    let mut early_ready = fixture();
    early_ready.functions[0].blocks[2].instructions.swap(2, 4);
    variants.push(early_ready);
    let mut callback = fixture();
    callback.functions[0].blocks[2].instructions.insert(
        1,
        (
            ValueId(8),
            I::CallStatic {
                symbol: "callback".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        ),
    );
    variants.push(callback);
    let mut reversed_condition = fixture();
    if let I::ICmp(comparison, _, _) =
        &mut reversed_condition.functions[0].blocks[0].instructions[1].1
    {
        *comparison = ICmpOp::Eq;
    }
    variants.push(reversed_condition);
    let mut error_path = fixture();
    error_path.functions[0].blocks.push(BasicBlock {
        id: BlockId(3),
        instructions: vec![],
        terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
    });
    variants.push(error_path);
    let mut empty_args = fixture();
    if let I::CallRuntime { args, .. } = &mut empty_args.functions[0].blocks[2].instructions[1].1 {
        args.clear();
    }
    variants.push(empty_args);
    let mut null_arg = fixture();
    if let I::CallRuntime { args, .. } = &mut null_arg.functions[0].blocks[2].instructions[1].1 {
        args[0].0 = Operand::Constant(Constant::Null);
    }
    variants.push(null_arg);
    let mut no_declaration = fixture();
    no_declaration.globals.clear();
    variants.push(no_declaration);
    let mut false_flag = fixture();
    if let I::StoreGlobal { val, .. } = &mut false_flag.functions[0].blocks[2].instructions[4].1 {
        *val = Operand::Constant(Constant::Int(0));
    }
    variants.push(false_flag);
    for module in variants {
        assert_eq!(known(&module), None);
    }
}

#[test]
fn cache_rejects_external_writers_arc_stores_duplicates_and_runtime_homonyms() {
    for symbol in ["cache", "cache$ok"] {
        let mut module = fixture();
        module.functions[1].blocks[0].instructions.insert(
            0,
            (
                ValueId(1),
                I::StoreGlobal {
                    symbol: symbol.into(),
                    val: Operand::Constant(Constant::Null),
                    ty: Type::Ref,
                    root: Some(123),
                },
                Type::Void,
            ),
        );
        assert_eq!(known(&module), None);
        let mut module = fixture();
        module.functions[1].blocks[0].instructions.insert(
            0,
            (
                ValueId(1),
                I::ArcStoreStrong {
                    slot: StrongSlot::Global {
                        symbol: symbol.into(),
                    },
                    value: Operand::Constant(Constant::Null),
                    mode: StrongStoreMode::Copy,
                },
                Type::Void,
            ),
        );
        assert_eq!(known(&module), None);
    }
    let mut duplicate = fixture();
    duplicate.functions.push(duplicate.functions[0].clone());
    assert_eq!(known(&duplicate), None);
    let mut duplicate_global = fixture();
    duplicate_global
        .globals
        .push(duplicate_global.globals[0].clone());
    assert_eq!(known(&duplicate_global), None);
    for name in [
        "dartforge_object_new",
        "dartforge_canonical_constant",
        "dartforge_mark_constant",
        "dartforge_globals_area",
        "dartforge_globals_area_id",
    ] {
        let mut module = fixture();
        let mut homonym = module.functions[1].clone();
        homonym.symbol = name.into();
        module.functions.push(homonym);
        assert_eq!(known(&module), None);
    }
    for symbol in ["cache", "cache$ok"] {
        let mut escaped = fixture();
        escaped.functions[1].blocks[0].instructions.insert(
            0,
            (
                ValueId(1),
                I::Const(Constant::Function(symbol.into())),
                Type::I64,
            ),
        );
        assert_eq!(known(&escaped), None);
    }
    let mut root_alias = fixture();
    root_alias
        .globals
        .push((123, Type::Ref, "different.cache".into()));
    assert_eq!(known(&root_alias), None);
    for symbol in ["dartforge_globals_area", "dartforge_globals_area_id"] {
        let mut area = fixture();
        area.functions[1].blocks[0].instructions.insert(
            0,
            (
                ValueId(1),
                I::CallRuntime {
                    name: symbol.into(),
                    args: vec![],
                    ret_ty: Type::Ptr,
                },
                Type::Ptr,
            ),
        );
        assert_eq!(known(&area), None);
        let mut escaped = fixture();
        escaped.functions[1].blocks[0].instructions.insert(
            0,
            (
                ValueId(1),
                I::Const(Constant::Function(symbol.into())),
                Type::I64,
            ),
        );
        assert_eq!(known(&escaped), None);
    }
    let mut lookup = fixture();
    lookup.functions[1].blocks[0].instructions.insert(
        0,
        (
            ValueId(1),
            I::Const(Constant::StringWtf8(b"dartforge_globals_area".to_vec())),
            Type::Ref,
        ),
    );
    assert_eq!(known(&lookup), None);
    let mut reloadable = fixture();
    reloadable.arc_fixed_ancestry = false;
    assert_eq!(known(&reloadable), None);
    for cid in [1, 6, 7, 8, 10, -1, i64::from(i32::MAX) + 1] {
        let mut module = fixture();
        if let I::CallRuntime { args, .. } = &mut module.functions[0].blocks[2].instructions[0].1 {
            args[0].0 = Operand::Constant(Constant::Int(cid));
        }
        assert_eq!(known(&module), None);
    }
}

#[test]
fn certified_receiver_limits_both_typed_and_untyped_selector_domains() {
    fn method(symbol: &str) -> Function {
        Function {
            symbol: symbol.into(),
            name: symbol.into(),
            params: vec![],
            return_ty: Type::Ref,
            debug_info: None,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
            }],
        }
    }
    for unknown in [false, true] {
        let mut module = fixture();
        module.entry_symbol = Some("caller".into());
        let caller = &mut module.functions[1];
        let receiver = if unknown {
            caller
                .params
                .push((ValueId(2), "receiver".into(), Type::Ref));
            Operand::Val(ValueId(2))
        } else {
            Operand::Val(ValueId(0))
        };
        caller.blocks[0].instructions.push((
            ValueId(1),
            I::CallSelector {
                selector: "tc:convert".into(),
                recv: receiver,
                args: vec![],
                names: vec![],
                tuple_types: Operand::Constant(Constant::Int(0)),
            },
            Type::Ref,
        ));
        caller.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
        for name in [
            "Encoder.convert$c",
            "Encoder.convert$tc",
            "Pixel.convert$c",
            "Pixel.convert$tc",
        ] {
            module.functions.push(method(name));
        }
        module.method_tables = vec![
            (
                315,
                "df.mt.Encoder".into(),
                vec![
                    ("c:convert".into(), "Encoder.convert$c".into()),
                    ("tc:convert".into(), "Encoder.convert$tc".into()),
                ],
            ),
            (
                316,
                "df.mt.Pixel".into(),
                vec![
                    ("c:convert".into(), "Pixel.convert$c".into()),
                    ("tc:convert".into(), "Pixel.convert$tc".into()),
                ],
            ),
        ];
        crate::pruning::prune_hir(&mut module, &[]);
        assert!(
            module
                .functions
                .iter()
                .any(|f| f.symbol == "Encoder.convert$c")
        );
        assert!(
            module
                .functions
                .iter()
                .any(|f| f.symbol == "Encoder.convert$tc")
        );
        for name in ["Pixel.convert$c", "Pixel.convert$tc"] {
            assert_eq!(module.functions.iter().any(|f| f.symbol == name), unknown);
        }
    }
}
