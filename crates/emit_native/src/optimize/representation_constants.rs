//! Canonical literal representations at exact null and boolean comparisons.
//!
//! Legacy lowering compares managed null handles and one-bit booleans with
//! integer literals. Preserve the physical comparison while exposing its
//! semantic representation to later HIR folding and validation. Scalar values,
//! native addresses and nonzero managed words are never reinterpreted.

use crate::hir::*;
use std::collections::HashMap;

fn operand_type(operand: &Operand, types: &HashMap<ValueId, Type>) -> Option<Type> {
    match operand {
        Operand::Val(value) => types.get(value).copied(),
        Operand::Constant(Constant::Null) => Some(Type::Ref),
        Operand::Constant(Constant::Bool(_)) => Some(Type::I1),
        _ => None,
    }
}

fn types(function: &Function) -> Option<HashMap<ValueId, Type>> {
    let mut types = HashMap::new();
    for (value, _, ty) in &function.params {
        if types.insert(*value, *ty).is_some() {
            return None;
        }
    }
    for (value, _, ty) in function.blocks.iter().flat_map(|b| &b.instructions) {
        if types.insert(*value, *ty).is_some() {
            return None;
        }
    }
    Some(types)
}

fn canonicalize(literal: &mut Operand, counterpart: Option<Type>) -> bool {
    let replacement = match (&*literal, counterpart) {
        (Operand::Constant(Constant::Int(0)), Some(Type::Ref)) => Constant::Null,
        (Operand::Constant(Constant::Int(0)), Some(Type::I1)) => Constant::Bool(false),
        (Operand::Constant(Constant::Int(1)), Some(Type::I1)) => Constant::Bool(true),
        _ => return false,
    };
    *literal = Operand::Constant(replacement);
    true
}

/// Exposes exact managed-null and one-bit boolean literal definitions.
/// Other literal representations and instruction results remain untouched.
pub(super) fn propagate_literals(function: &mut Function) -> bool {
    if types(function).is_none() {
        return false;
    }
    let literals: HashMap<_, _> = function
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter_map(|(value, instruction, ty)| match (instruction, ty) {
            (Instruction::Const(Constant::Null), Type::Ref) => {
                Some((*value, Operand::Constant(Constant::Null)))
            }
            (Instruction::Const(Constant::Bool(boolean)), Type::I1) => {
                Some((*value, Operand::Constant(Constant::Bool(*boolean))))
            }
            _ => None,
        })
        .collect();
    if literals.is_empty() {
        return false;
    }
    for block in &mut function.blocks {
        block
            .instructions
            .retain(|(value, _, _)| !literals.contains_key(value));
    }
    super::operands::replace(function, &|value| literals.get(&value).cloned());
    true
}

/// Canonicalizes immediate literals only at exact Eq/Ne comparison boundaries.
/// Ambiguous SSA definitions and comparisons without an I1 result are skipped.
pub(super) fn normalize(function: &mut Function) -> bool {
    let Some(types) = types(function) else {
        return false;
    };
    let mut changed = false;
    for (_, instruction, ty) in function.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
        if *ty == Type::I1
            && let Instruction::ICmp(ICmpOp::Eq | ICmpOp::Ne, a, b) = instruction
        {
            let a_type = operand_type(a, &types);
            let b_type = operand_type(b, &types);
            changed |= canonicalize(a, b_type);
            changed |= canonicalize(b, a_type);
        }
    }
    changed
}

/// Folds pure one-bit boolean identities after constant propagation.
/// Producers of operands remain in place: eager side effects are not skipped.
pub(super) fn fold_booleans(function: &mut Function) -> bool {
    let Some(types) = types(function) else {
        return false;
    };
    let mut replacements = HashMap::new();
    for (value, instruction, ty) in function.blocks.iter().flat_map(|b| &b.instructions) {
        if *ty != Type::I1 {
            continue;
        }
        let (a, b) = match instruction {
            Instruction::And(a, b) | Instruction::Or(a, b) | Instruction::Xor(a, b) => (a, b),
            _ => continue,
        };
        if operand_type(a, &types) != Some(Type::I1) || operand_type(b, &types) != Some(Type::I1) {
            continue;
        }
        let boolean = |o: &Operand| match o {
            Operand::Constant(Constant::Bool(b)) => Some(*b),
            _ => None,
        };
        let replacement = match (instruction, boolean(a), boolean(b)) {
            (Instruction::And(..), Some(false), _) | (Instruction::And(..), _, Some(false)) => {
                Some(Operand::Constant(Constant::Bool(false)))
            }
            (Instruction::Or(..), Some(true), _) | (Instruction::Or(..), _, Some(true)) => {
                Some(Operand::Constant(Constant::Bool(true)))
            }
            (Instruction::And(..), Some(true), _)
            | (Instruction::Or(..), Some(false), _)
            | (Instruction::Xor(..), Some(false), _) => Some(b.clone()),
            (Instruction::And(..), _, Some(true))
            | (Instruction::Or(..), _, Some(false))
            | (Instruction::Xor(..), _, Some(false)) => Some(a.clone()),
            (Instruction::Xor(..), Some(a), Some(b)) => {
                Some(Operand::Constant(Constant::Bool(a ^ b)))
            }
            _ => None,
        };
        if let Some(replacement) = replacement {
            replacements.insert(*value, replacement);
        }
    }
    if replacements.is_empty() {
        return false;
    }
    // Resolve copies before removing definitions, including identities chained
    // across blocks. The valid SSA input precondition rules out value cycles.
    for key in replacements.keys().copied().collect::<Vec<_>>() {
        let mut operand = replacements[&key].clone();
        for _ in 0..replacements.len() {
            let Operand::Val(value) = &operand else {
                break;
            };
            let Some(next) = replacements.get(value) else {
                break;
            };
            operand = next.clone();
        }
        replacements.insert(key, operand);
    }
    for block in &mut function.blocks {
        block
            .instructions
            .retain(|(value, _, _)| !replacements.contains_key(value));
    }
    super::operands::replace(function, &|value| replacements.get(&value).cloned());
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compare(ty: Type, op: ICmpOp, literal: Constant, reversed: bool) -> Function {
        let value = Operand::Val(ValueId(0));
        let literal = Operand::Constant(literal);
        let (a, b) = if reversed {
            (literal, value)
        } else {
            (value, literal)
        };
        Function {
            symbol: "comparison".into(),
            name: "comparison".into(),
            debug_info: None,
            params: vec![(ValueId(0), "value".into(), ty)],
            return_ty: Type::I1,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(ValueId(1), Instruction::ICmp(op, a, b), Type::I1)],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
            }],
        }
    }

    #[test]
    fn exact_null_and_boolean_literals_normalize_both_operand_orders() {
        for reversed in [false, true] {
            for op in [ICmpOp::Eq, ICmpOp::Ne] {
                for (ty, int, expected) in [
                    (Type::Ref, 0, Constant::Null),
                    (Type::I1, 0, Constant::Bool(false)),
                    (Type::I1, 1, Constant::Bool(true)),
                ] {
                    let mut f = compare(ty, op, Constant::Int(int), reversed);
                    assert!(normalize(&mut f));
                    let Instruction::ICmp(_, a, b) = &f.blocks[0].instructions[0].1 else {
                        panic!("comparison");
                    };
                    let literal = if reversed { a } else { b };
                    assert_eq!(*literal, Operand::Constant(expected));
                    assert!(!normalize(&mut f));
                }
            }
        }
    }

    #[test]
    fn numeric_address_positive_reference_and_ordered_cases_are_unchanged() {
        for reversed in [false, true] {
            for (ty, op, literal) in [
                (Type::I64, ICmpOp::Eq, Constant::Int(0)),
                (Type::Ptr, ICmpOp::Eq, Constant::Int(0)),
                (Type::Ref, ICmpOp::Eq, Constant::Int(1)),
                (Type::Ref, ICmpOp::Eq, Constant::Int(-1)),
                (Type::I1, ICmpOp::Ne, Constant::Int(2)),
                (Type::Ref, ICmpOp::Slt, Constant::Int(0)),
                (Type::F64, ICmpOp::Eq, Constant::Double(-0.0)),
                (Type::F64, ICmpOp::Eq, Constant::Double(f64::NAN)),
            ] {
                let mut f = compare(ty, op, literal, reversed);
                let before = format!("{f:?}");
                assert!(!normalize(&mut f));
                assert_eq!(format!("{f:?}"), before);
            }
        }
    }

    #[test]
    fn duplicate_unknown_and_nonboolean_result_comparisons_are_skipped() {
        let original = compare(Type::Ref, ICmpOp::Eq, Constant::Int(0), false);
        let mut duplicate = original.clone();
        duplicate
            .params
            .push((ValueId(0), "ambiguous".into(), Type::I64));
        assert!(!normalize(&mut duplicate));
        let mut unknown = original.clone();
        unknown.params.clear();
        assert!(!normalize(&mut unknown));
        let mut wrong_result = original;
        wrong_result.blocks[0].instructions[0].2 = Type::I64;
        assert!(!normalize(&mut wrong_result));
    }
    #[test]
    fn boolean_identities_preserve_eager_effectful_operand_producers() {
        for (instruction, expected) in [
            (
                Instruction::Or(
                    Operand::Val(ValueId(1)),
                    Operand::Constant(Constant::Bool(true)),
                ),
                true,
            ),
            (
                Instruction::And(
                    Operand::Constant(Constant::Bool(false)),
                    Operand::Val(ValueId(1)),
                ),
                false,
            ),
        ] {
            let mut f = compare(Type::I1, ICmpOp::Eq, Constant::Int(0), false);
            f.blocks[0].instructions = vec![
                (
                    ValueId(1),
                    Instruction::CallStatic {
                        symbol: "observableBoolean".into(),
                        args: vec![],
                        ret_ty: Type::I1,
                    },
                    Type::I1,
                ),
                (ValueId(2), instruction, Type::I1),
            ];
            f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(2))));
            assert!(fold_booleans(&mut f));
            super::super::simplify::remove_dead(&mut f);
            assert!(
                matches!(&f.blocks[0].instructions[..], [(_, Instruction::CallStatic {symbol, ..}, Type::I1)] if symbol == "observableBoolean")
            );
            assert!(
                matches!(&f.blocks[0].terminator, Terminator::Return(Some(Operand::Constant(Constant::Bool(value)))) if *value == expected)
            );
        }
    }

    #[test]
    fn identities_resolve_chains_without_retyping_numeric_bit_operations() {
        let mut f = compare(Type::I1, ICmpOp::Eq, Constant::Int(0), false);
        f.blocks[0].instructions = vec![
            (
                ValueId(1),
                Instruction::And(
                    Operand::Val(ValueId(0)),
                    Operand::Constant(Constant::Bool(true)),
                ),
                Type::I1,
            ),
            (
                ValueId(2),
                Instruction::Or(
                    Operand::Constant(Constant::Bool(false)),
                    Operand::Val(ValueId(1)),
                ),
                Type::I1,
            ),
            (
                ValueId(3),
                Instruction::Xor(
                    Operand::Val(ValueId(2)),
                    Operand::Constant(Constant::Bool(false)),
                ),
                Type::I1,
            ),
        ];
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(3))));
        assert!(fold_booleans(&mut f));
        assert!(f.blocks[0].instructions.is_empty());
        assert!(matches!(
            &f.blocks[0].terminator,
            Terminator::Return(Some(Operand::Val(ValueId(0))))
        ));
        assert!(!fold_booleans(&mut f));
        let mut numeric = compare(Type::I64, ICmpOp::Eq, Constant::Int(0), false);
        numeric.blocks[0].instructions[0].1 = Instruction::Or(
            Operand::Val(ValueId(0)),
            Operand::Constant(Constant::Bool(true)),
        );
        assert!(!fold_booleans(&mut numeric));
    }

    #[test]
    fn real_nullable_equality_discards_dispatch_at_prepared_hir_stage() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let directory = tempfile::tempdir().unwrap();
            let entry = directory.path().join("main.dart");
            std::fs::write(&entry, "bool missing(Object? value) => value == null; bool present(Object? value) => value != null; void main() { print(missing(null)); print(present(Object())); }").unwrap();
            let sdk = dartforge_elements::sdk::SdkLayout::load(crate::sdk_tests(), "dartdevc").unwrap();
            let mut interner = dartforge_intern::Interner::new();
            let program = dartforge_elements::load::load(&entry, &sdk, None, &mut interner).unwrap();
            let mut table = dartforge_types::TypeTable::new();
            let core = dartforge_types::CoreTypes::init(&mut table, &program, &interner);
            let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
            let libraries: Vec<_> = program.libraries.iter().enumerate().filter(|(_, l)| !l.is_sdk).map(|(i, _)| dartforge_elements::model::LibraryId(i as u32)).collect();
            let (bodies, _) = dartforge_types::infer_library_bodies_with_locals(&program, &interner, &mut table, &core, &mut outline, &libraries);
            let context = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies);
            let symbols: Vec<_> = ["missing", "present"].map(|name| {
                let fid = program.functions.iter().position(|f| interner.resolve(f.name) == name && !program.library(f.library).is_sdk).unwrap();
                crate::lower::symbol_of(&context, fid)
            }).into_iter().collect();
            let raw = crate::lower::lower_program(&context);
            for symbol in &symbols {
                let f = raw.functions.iter().find(|f| &f.symbol == symbol).unwrap();
                assert!(f.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i, Instruction::CallSelector { .. })), "fixture must contain real nullable equality dispatch: {f:?}");
            }
            for (arc, tables) in [(false, false), (false, true), (true, true)] {
                let mut module = raw.clone();
                super::super::prepare_for_emission(&mut module, arc, tables);
                for symbol in &symbols {
                    let f = module.functions.iter().find(|f| &f.symbol == symbol).unwrap();
                    assert!(!f.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i, Instruction::CallSelector { .. })), "dead equality dispatch survived: {f:?}");
                    assert!(f.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i, Instruction::ICmp(ICmpOp::Eq | ICmpOp::Ne, Operand::Val(_), Operand::Constant(Constant::Null)))), "real parameter identity predicate must remain: {f:?}");
                }
            }
        }).unwrap().join().unwrap();
    }
    #[test]
    fn only_exact_typed_null_and_boolean_definitions_are_propagated() {
        let mut f = compare(Type::I1, ICmpOp::Eq, Constant::Int(0), false);
        f.blocks[0].instructions = vec![
            (ValueId(1), Instruction::Const(Constant::Null), Type::Ref),
            (
                ValueId(2),
                Instruction::Const(Constant::Bool(true)),
                Type::I1,
            ),
            (ValueId(3), Instruction::Const(Constant::Int(0)), Type::I64),
            (
                ValueId(4),
                Instruction::Const(Constant::Double(-0.0)),
                Type::F64,
            ),
            (ValueId(5), Instruction::Const(Constant::Int(1)), Type::Ref),
        ];
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(2))));
        assert!(propagate_literals(&mut f));
        assert_eq!(
            f.blocks[0]
                .instructions
                .iter()
                .map(|(v, _, _)| v.0)
                .collect::<Vec<_>>(),
            vec![3, 4, 5]
        );
        assert!(matches!(
            f.blocks[0].terminator,
            Terminator::Return(Some(Operand::Constant(Constant::Bool(true))))
        ));
        assert!(!propagate_literals(&mut f));
    }

    #[test]
    fn constant_boolean_bit_operations_follow_complete_truth_tables() {
        for a in [false, true] {
            for b in [false, true] {
                let left = Operand::Constant(Constant::Bool(a));
                let right = Operand::Constant(Constant::Bool(b));
                for (instruction, expected) in [
                    (Instruction::And(left.clone(), right.clone()), a & b),
                    (Instruction::Or(left.clone(), right.clone()), a | b),
                    (Instruction::Xor(left.clone(), right.clone()), a ^ b),
                ] {
                    let mut f = compare(Type::I1, ICmpOp::Eq, Constant::Int(0), false);
                    f.blocks[0].instructions = vec![(ValueId(1), instruction, Type::I1)];
                    assert!(fold_booleans(&mut f));
                    assert!(
                        matches!(&f.blocks[0].terminator, Terminator::Return(Some(Operand::Constant(Constant::Bool(value)))) if *value == expected)
                    );
                }
            }
        }
    }
}
