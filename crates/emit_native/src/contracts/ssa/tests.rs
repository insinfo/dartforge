use super::*;
use crate::hir::{BasicBlock, FCmpOp};
fn val(id: u32) -> Operand {
    Operand::Val(ValueId(id))
}
fn int(value: i64) -> Operand {
    Operand::Constant(Constant::Int(value))
}
fn block(id: u32, instructions: Vec<(ValueId, I, Type)>, terminator: Terminator) -> BasicBlock {
    BasicBlock {
        id: BlockId(id),
        instructions,
        terminator,
    }
}
fn function(blocks: Vec<BasicBlock>) -> Function {
    Function {
        symbol: "checked".into(),
        name: "checked".into(),
        params: vec![(ValueId(0), "reference".into(), Type::Ref)],
        return_ty: Type::I64,
        blocks,
        debug_info: None,
    }
}
fn check(function: &Function) -> Result<ValidatedSubset, ValidationFailure> {
    validate(
        &Module {
            functions: vec![function.clone()],
            ..Module::default()
        },
        function,
    )
}
fn bad(function: Function, fragment: &str) {
    let error = check(&function).unwrap_err();
    assert_eq!(error.kind, FailureKind::Malformed, "{error:?}");
    assert!(error.reason.contains(fragment), "{error:?}");
}

#[test]
fn validates_primitive_control_flow_and_reference_identity() {
    let f = function(vec![
        block(
            0,
            vec![(
                ValueId(1),
                I::ICmp(ICmpOp::Eq, val(0), Operand::Constant(Constant::Null)),
                Type::I1,
            )],
            Terminator::CondBranch {
                cond: val(1),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        block(1, vec![], Terminator::Return(Some(int(1)))),
        block(2, vec![], Terminator::Return(Some(int(0)))),
    ]);
    assert_eq!(check(&f).unwrap().blocks, 3);
}
#[test]
fn undefined_duplicate_and_use_before_definition_are_malformed() {
    bad(
        function(vec![block(0, vec![], Terminator::Return(Some(val(99))))]),
        "undefined",
    );
    bad(
        function(vec![block(
            0,
            vec![(ValueId(0), I::Const(Constant::Int(1)), Type::I64)],
            Terminator::Return(Some(int(0))),
        )]),
        "duplicate SSA",
    );
    bad(
        function(vec![block(
            0,
            vec![
                (ValueId(1), I::Add(val(2), int(1)), Type::I64),
                (ValueId(2), I::Const(Constant::Int(2)), Type::I64),
            ],
            Terminator::Return(Some(val(1))),
        )]),
        "before its definition",
    );
}
#[test]
fn sibling_definitions_do_not_dominate_join_uses() {
    bad(
        function(vec![
            block(
                0,
                vec![],
                Terminator::CondBranch {
                    cond: Operand::Constant(Constant::Bool(true)),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
            ),
            block(
                1,
                vec![(ValueId(1), I::Const(Constant::Int(3)), Type::I64)],
                Terminator::Branch(BlockId(3)),
            ),
            block(2, vec![], Terminator::Branch(BlockId(3))),
            block(3, vec![], Terminator::Return(Some(val(1)))),
        ]),
        "does not dominate",
    );
}
fn loop_function() -> Function {
    function(vec![
        block(0, vec![], Terminator::Branch(BlockId(1))),
        block(
            1,
            vec![
                (
                    ValueId(1),
                    I::Phi {
                        incoming: vec![(BlockId(0), int(0)), (BlockId(1), val(2))],
                        ty: Type::I64,
                    },
                    Type::I64,
                ),
                (ValueId(2), I::Add(val(1), int(1)), Type::I64),
                (ValueId(3), I::ICmp(ICmpOp::Slt, val(2), int(3)), Type::I1),
            ],
            Terminator::CondBranch {
                cond: val(3),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        block(2, vec![], Terminator::Return(Some(val(1)))),
    ])
}
#[test]
fn loop_phi_backedge_definition_is_available_at_predecessor_exit() {
    assert!(check(&loop_function()).is_ok());
}
#[test]
fn phi_requires_exact_predecessors_types_prefix_and_edge_dominance() {
    let mut f = loop_function();
    if let I::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
        incoming.pop();
    }
    bad(f, "predecessor edges");
    let mut f = loop_function();
    if let I::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
        incoming[0].1 = val(2);
    }
    bad(f, "does not dominate");
    let mut f = loop_function();
    if let I::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
        incoming[0].1 = Operand::Constant(Constant::Bool(false));
    }
    bad(f, "operand must");
    let mut f = loop_function();
    f.blocks[1]
        .instructions
        .insert(0, (ValueId(4), I::Const(Constant::Int(5)), Type::I64));
    bad(f, "block prefix");
}
#[test]
fn duplicate_edges_need_phi_multiplicity_and_identical_operands() {
    let f = function(vec![
        block(
            0,
            vec![],
            Terminator::CondBranch {
                cond: Operand::Constant(Constant::Bool(true)),
                then_block: BlockId(1),
                else_block: BlockId(1),
            },
        ),
        block(
            1,
            vec![(
                ValueId(1),
                I::Phi {
                    incoming: vec![(BlockId(0), int(1)), (BlockId(0), int(1))],
                    ty: Type::I64,
                },
                Type::I64,
            )],
            Terminator::Return(Some(val(1))),
        ),
    ]);
    assert!(check(&f).is_ok());
    let mut different = f.clone();
    if let I::Phi { incoming, .. } = &mut different.blocks[1].instructions[0].1 {
        incoming[1].1 = int(2);
    }
    bad(different, "same operand");
    let mut missing = f;
    if let I::Phi { incoming, .. } = &mut missing.blocks[1].instructions[0].1 {
        incoming.pop();
    }
    bad(missing, "multiplicity");
}
#[test]
fn rejects_missing_blocks_entry_predecessors_duplicate_cases_and_wrong_branches() {
    bad(
        function(vec![block(0, vec![], Terminator::Branch(BlockId(9)))]),
        "unavailable",
    );
    bad(
        function(vec![block(0, vec![], Terminator::Branch(BlockId(0)))]),
        "entry block",
    );
    bad(
        function(vec![
            block(
                0,
                vec![],
                Terminator::CondBranch {
                    cond: int(1),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
            ),
            block(1, vec![], Terminator::Return(Some(int(1)))),
            block(2, vec![], Terminator::Return(Some(int(2)))),
        ]),
        "operand must have type I1",
    );
    bad(
        function(vec![
            block(
                0,
                vec![],
                Terminator::Switch {
                    val: int(1),
                    default: BlockId(1),
                    cases: vec![(1, BlockId(1)), (1, BlockId(1))],
                },
            ),
            block(1, vec![], Terminator::Return(Some(int(1)))),
        ]),
        "duplicate switch",
    );
}
#[test]
fn exact_primitive_comparison_and_conversion_types_are_required() {
    bad(
        function(vec![block(
            0,
            vec![(ValueId(1), I::FAdd(int(1), int(2)), Type::F64)],
            Terminator::Return(Some(int(0))),
        )]),
        "operand must have type F64",
    );
    bad(
        function(vec![block(
            0,
            vec![(ValueId(1), I::ICmp(ICmpOp::Eq, int(1), int(2)), Type::I64)],
            Terminator::Return(Some(int(0))),
        )]),
        "incompatible",
    );
    bad(
        function(vec![block(
            0,
            vec![(
                ValueId(1),
                I::ZExt {
                    op: int(1),
                    from: Type::I64,
                    to: Type::I8,
                },
                Type::I8,
            )],
            Terminator::Return(Some(int(0))),
        )]),
        "incompatible",
    );
    bad(
        function(vec![block(
            0,
            vec![(
                ValueId(1),
                I::Trunc {
                    op: int(1),
                    from: Type::I8,
                    to: Type::I1,
                },
                Type::I1,
            )],
            Terminator::Return(Some(int(0))),
        )]),
        "operand must have type I8",
    );
    bad(
        function(vec![block(
            0,
            vec![(
                ValueId(1),
                I::Bitcast {
                    op: int(1),
                    to: Type::I8,
                },
                Type::I8,
            )],
            Terminator::Return(Some(int(0))),
        )]),
        "incompatible",
    );
    bad(
        function(vec![block(
            0,
            vec![(
                ValueId(1),
                I::FCmp(FCmpOp::Eq, Operand::Constant(Constant::Double(1.0)), int(1)),
                Type::I1,
            )],
            Terminator::Return(Some(int(0))),
        )]),
        "F64",
    );
}
#[test]
fn correct_primitive_width_changes_are_valid() {
    let f = function(vec![block(
        0,
        vec![
            (
                ValueId(1),
                I::Trunc {
                    op: int(9),
                    from: Type::I64,
                    to: Type::I8,
                },
                Type::I8,
            ),
            (
                ValueId(2),
                I::ZExt {
                    op: val(1),
                    from: Type::I8,
                    to: Type::I64,
                },
                Type::I64,
            ),
            (
                ValueId(3),
                I::Bitcast {
                    op: val(2),
                    to: Type::F64,
                },
                Type::F64,
            ),
            (
                ValueId(4),
                I::Bitcast {
                    op: val(3),
                    to: Type::I64,
                },
                Type::I64,
            ),
        ],
        Terminator::Return(Some(val(4))),
    )]);
    assert!(check(&f).is_ok());
}
#[test]
fn void_values_cannot_be_used_and_returns_have_exact_abi_types() {
    bad(
        function(vec![block(0, vec![], Terminator::Return(None))]),
        "non-void",
    );
    bad(
        function(vec![block(
            0,
            vec![],
            Terminator::Return(Some(Operand::Constant(Constant::Bool(true)))),
        )]),
        "I64",
    );
    let mut callee = function(vec![block(0, vec![], Terminator::Return(None))]);
    callee.symbol = "callee".into();
    callee.return_ty = Type::Void;
    let caller = function(vec![block(
        0,
        vec![(
            ValueId(1),
            I::CallStatic {
                symbol: "callee".into(),
                args: vec![val(0)],
                ret_ty: Type::Void,
            },
            Type::Void,
        )],
        Terminator::Return(Some(val(1))),
    )]);
    let m = Module {
        functions: vec![callee, caller.clone()],
        ..Module::default()
    };
    assert!(validate(&m, &caller).unwrap_err().reason.contains("void"));
}
#[test]
fn unreachable_components_and_unknown_effects_are_not_malformed_ssa_claims() {
    let f = function(vec![
        block(0, vec![], Terminator::Return(Some(int(0)))),
        block(1, vec![], Terminator::Return(Some(int(1)))),
    ]);
    assert_eq!(check(&f).unwrap_err().kind, FailureKind::Unsupported);
    let f = function(vec![block(
        0,
        vec![(
            ValueId(1),
            I::GetField {
                object: val(0),
                index: 0,
            },
            Type::I64,
        )],
        Terminator::Return(Some(val(1))),
    )]);
    assert_eq!(check(&f).unwrap_err().kind, FailureKind::Unsupported);
}

#[test]
fn actual_lowered_bodies_validate_at_the_prepared_emission_stage() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let directory = tempfile::tempdir().unwrap();
        let entry = directory.path().join("main.dart");
        std::fs::write(&entry, "bool missing(Object? value) => value == null; double shifted(Object? unused, double value) => value + 1.0; void main() { print(missing(null)); print(shifted(null, 2.0)); }").unwrap();
        let sdk = dartforge_elements::sdk::SdkLayout::load(crate::sdk_tests(), "dartdevc").unwrap();
        let mut interner = dartforge_intern::Interner::new();
        let program = dartforge_elements::load::load(&entry, &sdk, None, &mut interner).unwrap();
        let mut table = dartforge_types::TypeTable::new();
        let core = dartforge_types::CoreTypes::init(&mut table, &program, &interner);
        let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
        let libraries: Vec<_> = program.libraries.iter().enumerate().filter(|(_, l)| !l.is_sdk).map(|(i, _)| dartforge_elements::model::LibraryId(i as u32)).collect();
        let (bodies, _) = dartforge_types::infer_library_bodies_with_locals(&program, &interner, &mut table, &core, &mut outline, &libraries);
        let context = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies);
        let symbol = |name| {
            let fid = program.functions.iter().position(|f| interner.resolve(f.name) == name && !program.library(f.library).is_sdk).unwrap();
            crate::lower::symbol_of(&context, fid)
        };
        let shifted = symbol("shifted");
        let missing = symbol("missing");
        let mut module = crate::lower::lower_program(&context);
        let raw = module.functions.iter().find(|f| f.symbol == shifted).unwrap();
        // Source lowering uses stack slots before the real mem2reg pipeline.
        assert_eq!(validate(&module, raw).unwrap_err().kind, FailureKind::Unsupported);
        crate::optimize::prepare_for_emission(&mut module, false, false);
        let function = module.functions.iter().find(|f| f.symbol == shifted).unwrap();
        assert!(validate(&module, function).is_ok(), "{:?}: {function:?}", validate(&module, function));
        let proof = super::super::no_escape::verify_parameter(&module, &shifted, 0);
        let super::super::no_escape::CheckResult::Proved(proof) = proof else { panic!("{proof:?}: {function:?}"); };
        assert!(!proof.outstanding_requirements.contains(&super::super::no_escape::ProofRequirement::ValidTypedSsa));
        assert!(proof.emission_audit_required);
        // Representation normalization and folding remove the dead equality
        // dispatch while preserving the actual reference identity predicate.
        let nullable = module.functions.iter().find(|f| f.symbol == missing).unwrap();
        assert!(validate(&module, nullable).is_ok(), "{:?}: {nullable:?}", validate(&module, nullable));
        assert!(nullable.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i, I::ICmp(crate::hir::ICmpOp::Eq, Operand::Val(_), Operand::Constant(Constant::Null)))));
        let proof = super::super::no_escape::verify_parameter(&module, &missing, 0);
        let super::super::no_escape::CheckResult::Proved(proof) = proof else { panic!("{proof:?}: {nullable:?}"); };
        assert!(!proof.outstanding_requirements.contains(&super::super::no_escape::ProofRequirement::ValidTypedSsa));
        assert!(proof.emission_audit_required);
    }).unwrap().join().unwrap();
}

#[test]
fn duplicate_float_phi_edges_preserve_signed_zero_and_nan_payload_identity() {
    let make = |left: f64, right: f64| {
        let mut f = function(vec![
            block(
                0,
                vec![],
                Terminator::CondBranch {
                    cond: Operand::Constant(Constant::Bool(true)),
                    then_block: BlockId(1),
                    else_block: BlockId(1),
                },
            ),
            block(
                1,
                vec![(
                    ValueId(1),
                    I::Phi {
                        incoming: vec![
                            (BlockId(0), Operand::Constant(Constant::Double(left))),
                            (BlockId(0), Operand::Constant(Constant::Double(right))),
                        ],
                        ty: Type::F64,
                    },
                    Type::F64,
                )],
                Terminator::Return(Some(val(1))),
            ),
        ]);
        f.return_ty = Type::F64;
        f
    };
    bad(make(0.0, -0.0), "same operand");
    let first = f64::from_bits(0x7ff8000000000001);
    let second = f64::from_bits(0x7ff8000000000002);
    assert!(check(&make(first, first)).is_ok());
    bad(make(first, second), "same operand");
}

#[test]
fn duplicate_blocks_parameters_and_void_parameters_are_malformed() {
    let mut f = function(vec![block(0, vec![], Terminator::Return(Some(int(0))))]);
    f.params.push((ValueId(0), "duplicate".into(), Type::Ref));
    bad(f, "parameter");
    let mut f = function(vec![block(0, vec![], Terminator::Return(Some(int(0))))]);
    f.params[0].2 = Type::Void;
    bad(f, "parameter");
    bad(
        function(vec![
            block(0, vec![], Terminator::Return(Some(int(0)))),
            block(0, vec![], Terminator::Return(Some(int(1)))),
        ]),
        "duplicate block",
    );
}
