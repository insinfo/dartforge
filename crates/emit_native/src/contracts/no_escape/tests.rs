use super::*;
use crate::hir::{BasicBlock, BlockId, Constant};

fn parameter(id: u32) -> Operand {
    Operand::Val(ValueId(id))
}
fn block(instructions: Vec<(ValueId, I, Type)>, terminator: Terminator) -> BasicBlock {
    BasicBlock {
        id: BlockId(0),
        instructions,
        terminator,
    }
}
fn function(
    symbol: &str,
    return_ty: Type,
    instructions: Vec<(ValueId, I, Type)>,
    terminator: Terminator,
) -> Function {
    Function {
        symbol: symbol.into(),
        name: symbol.into(),
        params: vec![(ValueId(0), "borrowed".into(), Type::Ref)],
        return_ty,
        blocks: vec![block(instructions, terminator)],
        debug_info: None,
    }
}
fn module(functions: Vec<Function>) -> Module {
    Module {
        functions,
        ..Module::default()
    }
}
fn reason(module: &Module, symbol: &str) -> Unproved {
    match verify_parameter(module, symbol, 0) {
        CheckResult::Unproved(reason) => reason,
        proof => panic!("unexpected proof: {proof:?}"),
    }
}

#[test]
fn managed_reference_identity_predicates_do_not_publish_reference_words() {
    let function = function(
        "missing",
        Type::I1,
        vec![(
            ValueId(1),
            I::ICmp(
                crate::hir::ICmpOp::Eq,
                parameter(0),
                Operand::Constant(Constant::Null),
            ),
            Type::I1,
        )],
        Terminator::Return(Some(parameter(1))),
    );
    let result = verify_parameter(&module(vec![function]), "missing", 0);
    let CheckResult::Proved(proof) = result else {
        panic!("{result:?}");
    };
    assert_eq!(proof.parameter_index, 0);
    assert!(proof.emission_audit_required);
    assert_eq!(proof.dependencies.len(), 1);
    assert_eq!(proof.dependencies[0].hir_blake3.len(), 64);
}

#[test]
fn return_of_any_transitive_reference_is_unproved() {
    let function = function(
        "escape",
        Type::Ref,
        vec![
            (
                ValueId(1),
                I::GetField {
                    object: parameter(0),
                    index: 0,
                },
                Type::Ref,
            ),
            (
                ValueId(2),
                I::GetField {
                    object: parameter(1),
                    index: 1,
                },
                Type::Ref,
            ),
        ],
        Terminator::Return(Some(parameter(2))),
    );
    assert!(reason(&module(vec![function]), "escape")
        .reason
        .contains("layout witness"));
}

#[test]
fn all_other_reference_arguments_and_globals_preserve_possible_preexisting_aliases() {
    let mut other = function(
        "other",
        Type::Ref,
        vec![],
        Terminator::Return(Some(parameter(1))),
    );
    other
        .params
        .push((ValueId(1), "possibly_same_object".into(), Type::Ref));
    assert!(reason(&module(vec![other]), "other")
        .reason
        .contains("preexisting alias"));
    let global = function(
        "global",
        Type::Ref,
        vec![(
            ValueId(1),
            I::LoadGlobal {
                symbol: "existing".into(),
                ty: Type::Ref,
            },
            Type::Ref,
        )],
        Terminator::Return(Some(parameter(1))),
    );
    assert!(reason(&module(vec![global]), "global")
        .reason
        .contains("layout witness"));
}

#[test]
fn publication_then_restoration_is_not_accepted() {
    let function = function(
        "temporary",
        Type::Void,
        vec![
            (
                ValueId(1),
                I::StoreGlobal {
                    symbol: "observer".into(),
                    val: parameter(0),
                    ty: Type::Ref,
                    root: Some(1),
                },
                Type::Void,
            ),
            (
                ValueId(2),
                I::StoreGlobal {
                    symbol: "observer".into(),
                    val: Operand::Constant(Constant::Null),
                    ty: Type::Ref,
                    root: Some(1),
                },
                Type::Void,
            ),
        ],
        Terminator::Return(None),
    );
    assert!(reason(&module(vec![function]), "temporary")
        .reason
        .contains("unavailable"));
}

#[test]
fn exceptional_publication_closures_and_external_object_stores_are_unproved() {
    let thrown = function("throw", Type::Void, vec![], Terminator::Throw(parameter(0)));
    assert!(reason(&module(vec![thrown]), "throw")
        .reason
        .contains("exceptional"));
    let closure = function(
        "closure",
        Type::Void,
        vec![(
            ValueId(1),
            I::AllocClosure {
                code_symbol: "later".into(),
                env: parameter(0),
            },
            Type::Ref,
        )],
        Terminator::Return(None),
    );
    assert!(reason(&module(vec![closure]), "closure")
        .reason
        .contains("AllocClosure"));
    let field = function(
        "store",
        Type::Void,
        vec![(
            ValueId(1),
            I::SetField {
                object: parameter(0),
                index: 0,
                value: parameter(0),
            },
            Type::Void,
        )],
        Terminator::Return(None),
    );
    assert!(reason(&module(vec![field]), "store")
        .reason
        .contains("SetField"));
}

#[test]
fn opaque_calls_fail_even_when_the_requested_parameter_is_not_an_argument() {
    let function = function(
        "opaque",
        Type::Void,
        vec![(
            ValueId(1),
            I::CallStatic {
                symbol: "foreign_observer".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        )],
        Terminator::Return(None),
    );
    let failure = reason(&module(vec![function]), "opaque");
    assert!(failure.reason.contains("preexisting aliases"));
    assert!(failure.path.join(" -> ").contains("block 0 value 1"));
}

#[test]
fn static_dependencies_are_proved_by_their_bodies_and_hashes_change_after_mutation() {
    let helper = function(
        "helper",
        Type::I1,
        vec![(
            ValueId(1),
            I::ICmp(
                crate::hir::ICmpOp::Eq,
                parameter(0),
                Operand::Constant(Constant::Null),
            ),
            Type::I1,
        )],
        Terminator::Return(Some(parameter(1))),
    );
    let caller = function(
        "caller",
        Type::I1,
        vec![(
            ValueId(1),
            I::CallStatic {
                symbol: "helper".into(),
                args: vec![parameter(0)],
                ret_ty: Type::I1,
            },
            Type::I1,
        )],
        Terminator::Return(Some(parameter(1))),
    );
    let mut graph = module(vec![caller, helper]);
    let CheckResult::Proved(proof) = verify_parameter(&graph, "caller", 0) else {
        panic!();
    };
    assert_eq!(
        proof
            .dependencies
            .iter()
            .map(|d| d.symbol.as_str())
            .collect::<Vec<_>>(),
        ["caller", "helper"]
    );
    graph.functions[1].blocks[0].instructions.push((
        ValueId(2),
        I::StoreGlobal {
            symbol: "published".into(),
            val: parameter(0),
            ty: Type::Ref,
            root: Some(3),
        },
        Type::Void,
    ));
    let failure = reason(&graph, "caller");
    assert!(failure.path.join(" -> ").contains("helper"));
}

#[test]
fn recursive_annotations_cannot_prove_each_other() {
    let a = function(
        "a",
        Type::Void,
        vec![(
            ValueId(1),
            I::CallStatic {
                symbol: "b".into(),
                args: vec![parameter(0)],
                ret_ty: Type::Void,
            },
            Type::Void,
        )],
        Terminator::Return(None),
    );
    let b = function(
        "b",
        Type::Void,
        vec![(
            ValueId(1),
            I::CallStatic {
                symbol: "a".into(),
                args: vec![parameter(0)],
                ret_ty: Type::Void,
            },
            Type::Void,
        )],
        Terminator::Return(None),
    );
    assert!(reason(&module(vec![a, b]), "a")
        .reason
        .contains("fixed-point"));
}

#[test]
fn stack_cells_need_initialization_and_alias_witnesses_before_reference_reads() {
    let function = function(
        "local",
        Type::Ref,
        vec![
            (ValueId(1), I::Alloca(Type::Ref), Type::Ptr),
            (
                ValueId(2),
                I::Store {
                    ptr: parameter(1),
                    val: parameter(0),
                },
                Type::Void,
            ),
            (
                ValueId(3),
                I::Load {
                    ptr: parameter(1),
                    ty: Type::Ref,
                },
                Type::Ref,
            ),
        ],
        Terminator::Return(Some(parameter(3))),
    );
    assert!(reason(&module(vec![function]), "local")
        .reason
        .contains("definite-initialization"));
}

#[test]
fn integer_field_reads_cannot_hide_native_handles() {
    let function = function(
        "address",
        Type::I64,
        vec![(
            ValueId(1),
            I::GetField {
                object: parameter(0),
                index: 0,
            },
            Type::I64,
        )],
        Terminator::Return(Some(parameter(1))),
    );
    assert!(reason(&module(vec![function]), "address")
        .reason
        .contains("native-handle bits"));
}

#[test]
fn llvm_boundary_coercions_need_an_independent_effect_audit() {
    let function = function(
        "boxing",
        Type::Ref,
        vec![],
        Terminator::Return(Some(Operand::Constant(Constant::Int(1)))),
    );
    assert!(reason(&module(vec![function]), "boxing")
        .reason
        .contains("conversion"));
}

#[test]
fn cfg_branches_and_loop_reference_phi_cannot_drop_derived_reference_state() {
    let mut function = function("phi", Type::Ref, vec![], Terminator::Branch(BlockId(1)));
    function.blocks.push(BasicBlock {
        id: BlockId(1),
        instructions: vec![(
            ValueId(1),
            I::Phi {
                incoming: vec![(BlockId(0), parameter(0)), (BlockId(1), parameter(1))],
                ty: Type::Ref,
            },
            Type::Ref,
        )],
        terminator: Terminator::CondBranch {
            cond: Operand::Constant(Constant::Bool(true)),
            then_block: BlockId(1),
            else_block: BlockId(2),
        },
    });
    function.blocks.push(BasicBlock {
        id: BlockId(2),
        instructions: vec![],
        terminator: Terminator::Return(Some(parameter(1))),
    });
    assert!(reason(&module(vec![function]), "phi")
        .reason
        .contains("return may publish"));
}

#[test]
fn registered_float_field_type_cannot_reconstruct_a_managed_handle_as_integer() {
    let function = function(
        "punned",
        Type::I64,
        vec![
            (
                ValueId(1),
                I::GetField {
                    object: parameter(0),
                    index: 0,
                },
                Type::F64,
            ),
            (
                ValueId(2),
                I::Bitcast {
                    op: parameter(1),
                    to: Type::I64,
                },
                Type::I64,
            ),
        ],
        Terminator::Return(Some(parameter(2))),
    );
    assert!(reason(&module(vec![function]), "punned")
        .reason
        .contains("layout witness"));
}

#[test]
fn global_registered_scalar_type_cannot_hide_reference_slot_representation() {
    let function = function(
        "global_punned",
        Type::I64,
        vec![(
            ValueId(1),
            I::LoadGlobal {
                symbol: "ref_slot".into(),
                ty: Type::I64,
            },
            Type::I64,
        )],
        Terminator::Return(Some(parameter(1))),
    );
    let mut graph = module(vec![function]);
    graph.globals.push((1, Type::Ref, "ref_slot".into()));
    assert!(reason(&graph, "global_punned")
        .reason
        .contains("layout witness"));
}

#[test]
fn uninitialized_stack_scalar_cannot_publish_stale_reference_words() {
    let function = function(
        "uninitialized",
        Type::I64,
        vec![
            (ValueId(1), I::Alloca(Type::I64), Type::Ptr),
            (
                ValueId(2),
                I::Load {
                    ptr: parameter(1),
                    ty: Type::I64,
                },
                Type::I64,
            ),
        ],
        Terminator::Return(Some(parameter(2))),
    );
    assert!(reason(&module(vec![function]), "uninitialized")
        .reason
        .contains("definite-initialization"));
}

#[test]
fn dependency_hash_excludes_unordered_debug_maps_and_preserves_scalar_nan_bits() {
    let mut function = function(
        "fingerprint",
        Type::F64,
        vec![],
        Terminator::Return(Some(Operand::Constant(Constant::Double(f64::from_bits(
            0x7ff8000000000001,
        ))))),
    );
    let first = body_fingerprint_input(&function);
    let mut debug = crate::hir::FunctionDebugInfo::default();
    debug.positions.insert(ValueId(2), (3, 4));
    debug.positions.insert(ValueId(1), (1, 2));
    function.debug_info = Some(Box::new(debug));
    assert_eq!(first, body_fingerprint_input(&function));
    function.blocks[0].terminator = Terminator::Return(Some(Operand::Constant(Constant::Double(
        f64::from_bits(0x7ff8000000000002),
    ))));
    assert_ne!(first, body_fingerprint_input(&function));
}

#[test]
fn valid_hir_evidence_discharges_only_the_checked_typed_ssa_obligation() {
    let function = function(
        "ssa_checked",
        Type::I1,
        vec![(
            ValueId(1),
            I::ICmp(
                crate::hir::ICmpOp::Eq,
                parameter(0),
                Operand::Constant(Constant::Null),
            ),
            Type::I1,
        )],
        Terminator::Return(Some(parameter(1))),
    );
    let CheckResult::Proved(proof) = verify_parameter(&module(vec![function]), "ssa_checked", 0)
    else {
        panic!();
    };
    assert!(!proof
        .outstanding_requirements
        .contains(&ProofRequirement::ValidTypedSsa));
    assert!(proof
        .outstanding_requirements
        .contains(&ProofRequirement::NativeScalarAliasProvenance));
    assert!(proof
        .outstanding_requirements
        .contains(&ProofRequirement::GeneratedAbiHelpersAndCleanup));
}

#[test]
fn malformed_scalar_operation_never_produces_hir_proof() {
    let function = function(
        "bad_types",
        Type::F64,
        vec![(
            ValueId(1),
            I::Add(
                Operand::Constant(Constant::Double(1.0)),
                Operand::Constant(Constant::Double(2.0)),
            ),
            Type::F64,
        )],
        Terminator::Return(Some(parameter(1))),
    );
    let failure = reason(&module(vec![function]), "bad_types");
    assert!(failure.path.join(" -> ").contains("Malformed"));
}
