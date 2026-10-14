//! Closed physical field initialization and refusal tests.
use super::*;
use crate::hir::{BasicBlock, ClassDef};

fn val(v: u32) -> Operand {
    Operand::Val(ValueId(v))
}
fn int(v: i64) -> Operand {
    Operand::Constant(Constant::Int(v))
}
fn decl() -> FieldDeclaration {
    FieldDeclaration {
        library_uri: "package:sample/model.dart".into(),
        owner: "Parent".into(),
        name: "child".into(),
        is_final: true,
        is_late: false,
        layout_index: 0,
        storage_type: Type::Ref,
    }
}
fn block(id: u32, instructions: Vec<(ValueId, I, Type)>, terminator: Terminator) -> BasicBlock {
    BasicBlock {
        id: BlockId(id),
        instructions,
        terminator,
    }
}
fn function(
    symbol: &str,
    params: Vec<(ValueId, String, Type)>,
    blocks: Vec<BasicBlock>,
) -> Function {
    Function {
        symbol: symbol.into(),
        name: symbol.into(),
        params,
        blocks,
        return_ty: Type::Ref,
        debug_info: None,
    }
}
fn sample() -> Module {
    let mut m = Module::new();
    m.arc_fixed_ancestry = true;
    m.pruning_class_ids.insert(128);
    m.classes.push(ClassDef {
        id: 128,
        name: "Parent".into(),
        field_count: 1,
        vtable: vec![],
        to_string_symbol: None,
    });
    m.declared_field_layouts.insert(128, vec![Some(decl())]);
    m.globals.push((14, Type::Ref, "cache".into()));
    m.functions.push(function(
        "child",
        vec![],
        vec![block(
            0,
            vec![(
                ValueId(0),
                I::AllocObject {
                    class_id: 129,
                    fields: vec![],
                },
                Type::Ref,
            )],
            Terminator::Return(Some(val(0))),
        )],
    ));
    m.functions.push(function(
        "cache.get",
        vec![],
        vec![
            block(
                0,
                vec![
                    (
                        ValueId(0),
                        I::LoadGlobal {
                            symbol: "cache$ok".into(),
                            ty: Type::I8,
                        },
                        Type::I8,
                    ),
                    (ValueId(1), I::ICmp(ICmpOp::Ne, val(0), int(0)), Type::I1),
                ],
                Terminator::CondBranch {
                    cond: val(1),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
            ),
            block(
                1,
                vec![(
                    ValueId(2),
                    I::LoadGlobal {
                        symbol: "cache".into(),
                        ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                Terminator::Return(Some(val(2))),
            ),
            block(
                2,
                vec![
                    (
                        ValueId(3),
                        I::CallRuntime {
                            name: "dartforge_object_new".into(),
                            args: vec![(int(128), Type::I64), (int(1), Type::I64)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(4),
                        I::CallStatic {
                            symbol: "child".into(),
                            args: vec![],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(5),
                        I::CallRuntime {
                            name: "dartforge_exception_pending".into(),
                            args: vec![],
                            ret_ty: Type::I8,
                        },
                        Type::I8,
                    ),
                    (ValueId(6), I::ICmp(ICmpOp::Ne, val(5), int(0)), Type::I1),
                ],
                Terminator::CondBranch {
                    cond: val(6),
                    then_block: BlockId(4),
                    else_block: BlockId(3),
                },
            ),
            block(
                3,
                vec![
                    (
                        ValueId(7),
                        I::DeclaredFieldCall {
                            declaration: decl(),
                            name: "dartforge_object_set".into(),
                            args: vec![
                                (val(3), Type::Ref),
                                (int(0), Type::I64),
                                (val(4), Type::I64),
                                (int(1), Type::I8),
                            ],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(8),
                        I::CallRuntime {
                            name: "dartforge_canonical_constant".into(),
                            args: vec![(val(3), Type::Ref)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(9),
                        I::StoreGlobal {
                            symbol: "cache".into(),
                            val: val(8),
                            ty: Type::Ref,
                            root: Some(14),
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(10),
                        I::CallRuntime {
                            name: "dartforge_mark_constant".into(),
                            args: vec![
                                (val(8), Type::Ref),
                                (
                                    Operand::Constant(Constant::Function("cache.get".into())),
                                    Type::I64,
                                ),
                            ],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(11),
                        I::StoreGlobal {
                            symbol: "cache$ok".into(),
                            val: int(1),
                            ty: Type::I8,
                            root: None,
                        },
                        Type::Void,
                    ),
                ],
                Terminator::Return(Some(val(8))),
            ),
            block(
                4,
                vec![],
                Terminator::Return(Some(Operand::Constant(Constant::Null))),
            ),
        ],
    ));
    m.functions.push(function(
        "read",
        vec![(ValueId(0), "receiver".into(), Type::Ref)],
        vec![block(
            0,
            vec![(
                ValueId(1),
                I::DeclaredFieldCall {
                    declaration: decl(),
                    name: "dartforge_object_get".into(),
                    args: vec![(val(0), Type::Ref), (int(0), Type::I64)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )],
            Terminator::Return(Some(val(1))),
        )],
    ));
    m
}
fn query(m: &Module, receivers: Classes) -> Option<Classes> {
    let p = super::super::analyze(&[m]);
    let i = Inventory::new(&[m], &p, true);
    i.classes_for_read(&m.functions[2], ValueId(1), &receivers)
}

#[test]
fn requires_physical_receiver_and_guarded_complete_constant_initialization() {
    let m = sample();
    assert_eq!(query(&m, Classes::from([128])), Some(Classes::from([129])));
    assert_eq!(query(&m, Classes::new()), None);
    assert_eq!(query(&m, Classes::from([128, 1])), None);
    assert_eq!(query(&m, Classes::from([130])), None);
    let p = super::super::analyze(&[&m]);
    let i = Inventory::new(&[&m], &p, true);
    let clone = m.functions[2].clone();
    assert_eq!(
        i.classes_for_read(&clone, ValueId(1), &Classes::from([128])),
        None
    );
}
#[test]
fn refuses_missing_guard_failure_publication_and_partial_or_raw_initialization() {
    for case in 0..12 {
        let mut m = sample();
        match case {
            0 => m.functions[1].blocks[2].terminator = Terminator::Branch(BlockId(3)),
            1 => m.functions[1].blocks[4].terminator = Terminator::Branch(BlockId(3)),
            2 => {
                m.functions[1].blocks[3].instructions.remove(0);
            }
            3 => {
                if let I::DeclaredFieldCall { declaration, .. } =
                    &mut m.functions[1].blocks[3].instructions[0].1
                {
                    declaration.is_late = true;
                }
            }
            4 => {
                if let I::DeclaredFieldCall { declaration, .. } =
                    &mut m.functions[1].blocks[3].instructions[0].1
                {
                    declaration.owner = "Other".into();
                }
            }
            5 => m.functions[1].blocks[3].instructions.swap(0, 1),
            6 => m.declared_field_layouts.clear(),
            7 => {
                m.functions[0].blocks[0].instructions[0].1 = I::LoadGlobal {
                    symbol: "opaque".into(),
                    ty: Type::Ref,
                }
            }
            8 => {
                m.functions[1].blocks[3].instructions[0].1 = I::CallRuntime {
                    name: "dartforge_object_set".into(),
                    args: vec![
                        (val(3), Type::Ref),
                        (int(0), Type::I64),
                        (val(4), Type::I64),
                        (int(1), Type::I8),
                    ],
                    ret_ty: Type::Void,
                }
            }
            9 => m.globals.push((14, Type::Ref, "alias".into())),
            10 => m.functions[1].blocks[3].terminator = Terminator::Branch(BlockId(2)),
            11 => m.arc_fixed_ancestry = false,
            _ => unreachable!(),
        };
        assert_eq!(query(&m, Classes::from([128])), None, "case {case}");
    }
}
#[test]
fn refuses_other_births_unknown_alias_writers_and_conflicting_layouts() {
    for case in 0..6 {
        let mut m = sample();
        match case {
            0 => m.functions.push(function(
                "other",
                vec![],
                vec![block(
                    0,
                    vec![(
                        ValueId(0),
                        I::AllocObject {
                            class_id: 128,
                            fields: vec![Operand::Constant(Constant::Null)],
                        },
                        Type::Ref,
                    )],
                    Terminator::Return(Some(val(0))),
                )],
            )),
            1 => m.functions[2].blocks[0].instructions.push((
                ValueId(2),
                I::CallRuntime {
                    name: "dartforge_object_set".into(),
                    args: vec![
                        (val(0), Type::Ref),
                        (int(0), Type::I64),
                        (int(0), Type::I64),
                        (int(1), Type::I8),
                    ],
                    ret_ty: Type::Void,
                },
                Type::Void,
            )),
            2 => {
                m.declared_field_layouts.get_mut(&128).unwrap()[0]
                    .as_mut()
                    .unwrap()
                    .is_final = false
            }
            3 => {
                m.pruning_ambiguous_class_ids.insert(128);
            }
            4 => {
                m.pruning_class_ids.clear();
            }
            5 => {
                m.arc_external_ancestry.ambiguous_classes.insert(128);
            }
            _ => unreachable!(),
        };
        assert_eq!(query(&m, Classes::from([128])), None, "case {case}");
    }
}

#[test]
fn incomplete_source_corpus_never_certifies_even_with_layout_and_all_local_writers() {
    let m = sample();
    let p = super::super::analyze(&[&m]);
    let inventory = Inventory::new(&[&m], &p, false);
    assert_eq!(
        inventory.classes_for_read(&m.functions[2], ValueId(1), &Classes::from([128])),
        None
    );
}
