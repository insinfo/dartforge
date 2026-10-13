//! Common rules for constants and scalar operations covered by ARC.
//! They do not certify domain/overflow, field layout or uncovered operations.

use super::{TokenEffect, Ownership, TokenPlan};
use crate::hir::*;
use std::collections::HashMap;

pub(super) fn check(
    v: ValueId,
    inst: &Instruction,
    ty: &Type,
    types: &HashMap<ValueId, Type>,
) -> Result<bool, String> {
    // Scalar/null constants and permanent literals produce no
    // token. The variant of the constant determines its semantic type.
    let pure = match inst {
        // Constant data of the module: words or function addresses.
        Instruction::ConstArray(_) | Instruction::FunctionTable(_) => Some(Type::Ptr),
        Instruction::Const(Constant::Int(_)) => Some(Type::I64),
        Instruction::Const(Constant::Double(_)) => Some(Type::F64),
        Instruction::Const(Constant::Bool(_)) => Some(Type::I1),
        Instruction::Const(Constant::Null | Constant::String(_) | Constant::StringWtf8(_)) => {
            Some(Type::Ref)
        }
        Instruction::Box {
            from: Type::I1 | Type::I8,
            ..
        } => Some(Type::Ref),
        Instruction::ICmp(..) | Instruction::FCmp(..) | Instruction::LNot(_) => Some(Type::I1),
        // Bitwise logic is emitted in i64 with the operands extended
        // (`llvm/mod.rs`); the lowering declares it I1 when it uses it as a `bool`.
        Instruction::And(..) | Instruction::Or(..) | Instruction::Xor(..) if *ty == Type::I1 => Some(Type::I1),
        Instruction::Add(..)
        | Instruction::Sub(..)
        | Instruction::Mul(..)
        | Instruction::SDiv(..)
        | Instruction::SRem(..)
        | Instruction::Shl(..)
        | Instruction::AShr(..)
        | Instruction::LShr(..)
        | Instruction::And(..)
        | Instruction::Or(..)
        | Instruction::Xor(..)
        | Instruction::Neg(_)
        | Instruction::Not(_)
        | Instruction::DoubleToInt(_) => Some(Type::I64),
        Instruction::FAdd(..)
        | Instruction::FSub(..)
        | Instruction::FMul(..)
        | Instruction::FDiv(..)
        | Instruction::FNeg(_)
        | Instruction::IntToDouble(_) => Some(Type::F64),
        Instruction::Bitcast { to, .. } => Some(*to),
        // A 1- or 8-bit global, or a `double` one, holds no handle; I64 may
        // hold one and requires its own proof (`rti_caches`).
        Instruction::LoadGlobal { ty: t @ (Type::I1 | Type::I8 | Type::F64), .. } => Some(*t),
        // Storing a proven scalar (`check_origins`) in a non-Ref global.
        Instruction::StoreGlobal { ty: Type::I1 | Type::I8 | Type::I64 | Type::F64, root: None, .. } => Some(Type::Void),
        Instruction::ZExt { to, .. } | Instruction::Trunc { to, .. } => Some(*to),
        // Reading the lanes of a live SIMD box (final SDK classes,
        // only the runtime box implements them): the borrow of the box is used
        // in the instruction and the vector holds no handle.
        Instruction::Unbox { to, .. } if to.is_vector() => Some(*to),
        // SIMD arithmetic and conversions over unboxed vectors and
        // scalars. Reading and writing by address require a provenance proof: the
        // address derived from a list that `derived_set::verify` holds.
        Instruction::Simd { .. } => Some(*ty),
        Instruction::NativeLoad { .. } => Some(*ty),
        Instruction::NativeWrite { .. } => Some(Type::Void),
        _ => None,
    };
    if let Some(expected) = pure {
        let numeric = |op: &Operand| match op {
            Operand::Constant(Constant::Int(_) | Constant::Double(_)) => true,
            Operand::Val(v) => matches!(types.get(v), Some(Type::I64 | Type::F64)),
            _ => false,
        };
        let boolean = |op: &Operand| match op {
            Operand::Constant(Constant::Bool(_)) => true,
            Operand::Val(v) => matches!(types.get(v), Some(Type::I1 | Type::I8)),
            _ => false,
        };
        let integral = |op: &Operand| match op {
            Operand::Constant(Constant::Int(_) | Constant::Bool(_)) => true,
            Operand::Val(v) => matches!(types.get(v), Some(Type::I64 | Type::I1 | Type::I8)),
            _ => false,
        };
        let integer = |op: &Operand| match op {
            Operand::Constant(Constant::Int(_)) => true,
            Operand::Val(v) => types.get(v) == Some(&Type::I64),
            _ => false,
        };
        let invalid = match inst {
            Instruction::ZExt { op, from, to } | Instruction::Trunc { op, from, to } => {
                let origin = match op {
                    Operand::Val(v) => types.get(v).copied(),
                    Operand::Constant(Constant::Int(_)) => Some(Type::I64),
                    Operand::Constant(Constant::Bool(_)) => Some(Type::I1),
                    _ => None,
                };
                let valid_widths = if matches!(inst, Instruction::ZExt { .. }) {
                    matches!(
                        (from, to),
                        (Type::I1, Type::I8 | Type::I64) | (Type::I8, Type::I64)
                    )
                } else {
                    matches!(
                        (from, to),
                        (Type::I64, Type::I1 | Type::I8) | (Type::I8, Type::I1)
                    )
                };
                origin != Some(*from) || !valid_widths
            }
            Instruction::Bitcast { op, to } => {
                let origin = match op {
                    Operand::Val(v) => types.get(v).copied(),
                    Operand::Constant(Constant::Int(_)) => Some(Type::I64),
                    Operand::Constant(Constant::Double(_)) => Some(Type::F64),
                    _ => None,
                };
                !matches!(
                    (origin, to),
                    (Some(Type::I64), Type::F64) | (Some(Type::F64), Type::I64)
                )
            }
            Instruction::Box { op, from } => match op {
                Operand::Val(v) => types.get(v) != Some(from),
                Operand::Constant(Constant::Bool(_)) => *from != Type::I1,
                Operand::Constant(Constant::Int(n)) => *from != Type::I8 || !(0..=255).contains(n),
                _ => true,
            },
            Instruction::Add(a, b)
            | Instruction::Sub(a, b)
            | Instruction::Mul(a, b)
            | Instruction::SDiv(a, b)
            | Instruction::SRem(a, b)
            | Instruction::Shl(a, b)
            | Instruction::AShr(a, b)
            | Instruction::LShr(a, b) => !integer(a) || !integer(b),
            Instruction::And(a, b) | Instruction::Or(a, b) | Instruction::Xor(a, b) => {
                !integral(a) || !integral(b)
            }
            Instruction::Neg(op) | Instruction::Not(op) | Instruction::IntToDouble(op) => {
                !integer(op)
            }
            Instruction::FAdd(a, b)
            | Instruction::FSub(a, b)
            | Instruction::FMul(a, b)
            | Instruction::FDiv(a, b) => !numeric(a) || !numeric(b),
            Instruction::FNeg(op) | Instruction::DoubleToInt(op) => !numeric(op),
            Instruction::Unbox { op, .. } => !matches!(op, Operand::Val(v) if types.get(v) == Some(&Type::Ref)),
            Instruction::NativeLoad { address, index, ty: typ } => {
                !integer(address) || !integer(index) || *ty != typ.hir_type()
            }
            Instruction::NativeWrite { address, index, ty: typ, value } => {
                !integer(address) || !integer(index) || match value {
                    Operand::Val(v) => types.get(v) != Some(&typ.hir_type()),
                    Operand::Constant(Constant::Int(_)) => typ.hir_type() != Type::I64,
                    Operand::Constant(Constant::Double(_)) => typ.hir_type() != Type::F64,
                    _ => true,
                }
            }
            Instruction::Simd { args, .. } => args.iter().any(|op| match op {
                Operand::Val(v) => !types.get(v).is_some_and(|t| matches!(t, Type::I1 | Type::I8 | Type::I64 | Type::F64) || t.is_vector()),
                Operand::Constant(c) => !matches!(c, Constant::Int(_) | Constant::Double(_) | Constant::Bool(_)),
            }),
            Instruction::StoreGlobal { val, ty: stored, .. } => match val {
                Operand::Val(v) => types.get(v) != Some(stored),
                Operand::Constant(Constant::Int(_)) => !matches!(stored, Type::I64 | Type::I8),
                Operand::Constant(Constant::Bool(_)) => !matches!(stored, Type::I1 | Type::I8),
                Operand::Constant(Constant::Double(_)) => *stored != Type::F64,
                _ => true,
            },
            _ => false,
        };
        if invalid
            || matches!(inst, Instruction::FCmp(_, a, b) if !numeric(a) || !numeric(b))
            || matches!(inst, Instruction::LNot(op) if !boolean(op))
        {
            return Err(format!(
                "v{}: pure operation requires already evaluated scalars",
                v.0
            ));
        }
        if *ty != expected {
            return Err(format!("v{}: incompatible pure result type", v.0));
        }
        Ok(true)
    } else {
        Ok(false)
    }
}

pub(super) fn check_origins(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
) -> Result<(), String> {
    // Checks after resolving every producer and Phi, regardless of the
    // physical block order. A managed i64 does not become a scalar by a cast.
    for (v, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        if let Instruction::Box { op: Operand::Val(origin), from } = inst
            && from.is_vector() && classes.get(origin) != Some(&Ownership::Trivial) {
            return Err(format!("v{}: SIMD box requires a Trivial origin in {}", v.0, f.symbol));
        }
        if let Instruction::Simd { args, .. } = inst
            && args.iter().any(|op| matches!(op, Operand::Val(o) if classes.get(o) != Some(&Ownership::Trivial)))
        {
            return Err(format!("v{}: SIMD operation requires Trivial origins in {}", v.0, f.symbol));
        }
        let natives: &[&Operand] = match inst {
            Instruction::NativeLoad { address, index, .. } => &[address, index],
            Instruction::NativeWrite { address, index, value, .. } => &[address, index, value],
            _ => &[],
        };
        if natives.iter().any(|op| matches!(op, Operand::Val(o) if classes.get(o) != Some(&Ownership::Trivial))) {
            return Err(format!("v{}: native access requires Trivial origins in {}", v.0, f.symbol));
        }
        let operands = match inst {
            Instruction::Bitcast {
                op,
                to: Type::I64 | Type::F64,
            }
            | Instruction::Neg(op)
            | Instruction::Not(op)
            | Instruction::FNeg(op)
            | Instruction::IntToDouble(op)
            | Instruction::DoubleToInt(op)
            | Instruction::LNot(op)
            | Instruction::Box {
                op,
                from: Type::I1 | Type::I8,
            }
            | Instruction::ZExt { op, .. }
            | Instruction::Trunc { op, .. } => [Some(op), None],
            Instruction::StoreGlobal { val, ty: t, .. } if *t != Type::Ref => [Some(val), None],
            Instruction::Add(a, b)
            | Instruction::Sub(a, b)
            | Instruction::Mul(a, b)
            | Instruction::SDiv(a, b)
            | Instruction::SRem(a, b)
            | Instruction::Shl(a, b)
            | Instruction::AShr(a, b)
            | Instruction::LShr(a, b)
            | Instruction::And(a, b)
            | Instruction::Or(a, b)
            | Instruction::Xor(a, b)
            | Instruction::FAdd(a, b)
            | Instruction::FSub(a, b)
            | Instruction::FMul(a, b)
            | Instruction::FDiv(a, b)
            | Instruction::FCmp(_, a, b) => [Some(a), Some(b)],
            _ => continue,
        };
        for op in operands.into_iter().flatten() {
            if let Operand::Val(origin) = op
                && classes.get(origin) != Some(&Ownership::Trivial)
            {
                return Err(format!("v{}: scalar operation requires a Trivial origin in {} (origin v{})", v.0, f.symbol, origin.0));
            }
        }
    }
    Ok(())
}

pub(super) fn verify(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    plan: &TokenPlan,
) -> Result<(), String> {
    let types: HashMap<_, _> = f
        .params
        .iter()
        .map(|(v, _, t)| (*v, *t))
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|(v, _, t)| (*v, *t)),
        )
        .collect();
    for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
        if check(*v, inst, ty, &types)? {
            let effect = plan
                .instructions
                .get(v)
                .ok_or_else(|| format!("v{} without a pure operation contract", v.0))?;
            if classes.get(v) != Some(&Ownership::Trivial) || *effect != TokenEffect::default() {
                return Err(format!("v{}: contract incompatible with pure operation", v.0));
            }
        }
    }
    super::derived_set::verify(f)?;
    check_origins(f, classes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitwise_logic_accepts_integral_operands_in_i1_or_i64() {
        let types = HashMap::from([(ValueId(1), Type::I1), (ValueId(2), Type::I1), (ValueId(3), Type::I64), (ValueId(4), Type::Ref), (ValueId(5), Type::F64)]);
        let v = |n| Operand::Val(ValueId(n));
        let or = |a, b| Instruction::Or(a, b);
        assert!(check(ValueId(9), &or(v(1), v(2)), &Type::I1, &types).unwrap());
        assert!(check(ValueId(9), &Instruction::And(v(1), v(2)), &Type::I64, &types).unwrap());
        assert!(check(ValueId(9), &Instruction::And(v(1), Operand::Constant(Constant::Bool(true))), &Type::I1, &types).unwrap());
        assert!(check(ValueId(9), &or(v(3), Operand::Constant(Constant::Int(1))), &Type::I64, &types).unwrap());
        assert!(check(ValueId(9), &or(v(1), v(3)), &Type::I64, &types).unwrap());
        // A reference, a double or a result of another representation.
        assert!(check(ValueId(9), &Instruction::Xor(v(4), v(3)), &Type::I64, &types).is_err());
        assert!(check(ValueId(9), &Instruction::And(v(5), v(3)), &Type::I64, &types).is_err());
        assert!(check(ValueId(9), &or(v(1), v(2)), &Type::F64, &types).is_err());
        // Shifts still require integers.
        assert!(check(ValueId(9), &Instruction::Shl(v(1), v(3)), &Type::I64, &types).is_err());
        // Narrow or `double` globals are scalars; I64 is not, by width alone.
        let global = |ty| Instruction::LoadGlobal { symbol: "g".into(), ty };
        assert!(check(ValueId(9), &global(Type::I8), &Type::I8, &types).unwrap());
        assert!(check(ValueId(9), &global(Type::F64), &Type::F64, &types).unwrap());
        assert!(!check(ValueId(9), &global(Type::I64), &Type::I64, &types).unwrap());
        assert!(check(ValueId(9), &global(Type::I8), &Type::I64, &types).is_err());
        let store = |ty, val| Instruction::StoreGlobal { symbol: "g".into(), val, ty, root: None };
        assert!(check(ValueId(9), &store(Type::I8, Operand::Constant(Constant::Int(1))), &Type::Void, &types).unwrap());
        assert!(check(ValueId(9), &store(Type::I64, v(3)), &Type::Void, &types).unwrap());
        assert!(check(ValueId(9), &store(Type::I64, v(4)), &Type::Void, &types).is_err());
        assert!(!check(ValueId(9), &store(Type::Ref, v(4)), &Type::Void, &types).unwrap());
    }
}
