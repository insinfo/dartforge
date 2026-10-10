//! Regras comuns de constantes e operações escalares cobertas pelo ARC.
//! Não certificam domínio/estouro, layout de campos ou operações não cobertas.

use super::{EfeitoTokens, Ownership, PlanoTokens};
use crate::hir::*;
use std::collections::HashMap;

pub(super) fn conferir(
    v: ValueId,
    inst: &Instruction,
    ty: &Type,
    tipos: &HashMap<ValueId, Type>,
) -> Result<bool, String> {
    // Constantes escalares/null e literais permanentes não produzem
    // token. A variante da constante determina seu tipo semântico.
    let puro = match inst {
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
        Instruction::ZExt { to, .. } | Instruction::Trunc { to, .. } => Some(*to),
        _ => None,
    };
    if let Some(esperado) = puro {
        let numerico = |op: &Operand| match op {
            Operand::Constant(Constant::Int(_) | Constant::Double(_)) => true,
            Operand::Val(v) => matches!(tipos.get(v), Some(Type::I64 | Type::F64)),
            _ => false,
        };
        let booleano = |op: &Operand| match op {
            Operand::Constant(Constant::Bool(_)) => true,
            Operand::Val(v) => matches!(tipos.get(v), Some(Type::I1 | Type::I8)),
            _ => false,
        };
        let inteiro = |op: &Operand| match op {
            Operand::Constant(Constant::Int(_)) => true,
            Operand::Val(v) => tipos.get(v) == Some(&Type::I64),
            _ => false,
        };
        let invalido = match inst {
            Instruction::ZExt { op, from, to } | Instruction::Trunc { op, from, to } => {
                let origem = match op {
                    Operand::Val(v) => tipos.get(v).copied(),
                    Operand::Constant(Constant::Int(_)) => Some(Type::I64),
                    Operand::Constant(Constant::Bool(_)) => Some(Type::I1),
                    _ => None,
                };
                let larguras_validas = if matches!(inst, Instruction::ZExt { .. }) {
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
                origem != Some(*from) || !larguras_validas
            }
            Instruction::Bitcast { op, to } => {
                let origem = match op {
                    Operand::Val(v) => tipos.get(v).copied(),
                    Operand::Constant(Constant::Int(_)) => Some(Type::I64),
                    Operand::Constant(Constant::Double(_)) => Some(Type::F64),
                    _ => None,
                };
                !matches!(
                    (origem, to),
                    (Some(Type::I64), Type::F64) | (Some(Type::F64), Type::I64)
                )
            }
            Instruction::Box { op, from } => match op {
                Operand::Val(v) => tipos.get(v) != Some(from),
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
            | Instruction::LShr(a, b)
            | Instruction::And(a, b)
            | Instruction::Or(a, b)
            | Instruction::Xor(a, b) => !inteiro(a) || !inteiro(b),
            Instruction::Neg(op) | Instruction::Not(op) | Instruction::IntToDouble(op) => {
                !inteiro(op)
            }
            Instruction::FAdd(a, b)
            | Instruction::FSub(a, b)
            | Instruction::FMul(a, b)
            | Instruction::FDiv(a, b) => !numerico(a) || !numerico(b),
            Instruction::FNeg(op) | Instruction::DoubleToInt(op) => !numerico(op),
            _ => false,
        };
        if invalido
            || matches!(inst, Instruction::FCmp(_, a, b) if !numerico(a) || !numerico(b))
            || matches!(inst, Instruction::LNot(op) if !booleano(op))
        {
            return Err(format!(
                "v{}: operação pura exige escalares já avaliados",
                v.0
            ));
        }
        if *ty != esperado {
            return Err(format!("v{}: tipo do resultado puro incompatível", v.0));
        }
        Ok(true)
    } else {
        Ok(false)
    }
}

pub(super) fn conferir_origens(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
) -> Result<(), String> {
    // Confere após resolver todos os produtores e Phis, independentemente
    // da ordem física dos blocos. Um i64 gerenciado não vira escalar por cast.
    for (v, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let operandos = match inst {
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
        for op in operandos.into_iter().flatten() {
            if let Operand::Val(origem) = op
                && classes.get(origem) != Some(&Ownership::Trivial)
            {
                return Err(format!("v{}: operação escalar exige origem Trivial", v.0));
            }
        }
    }
    Ok(())
}

pub(super) fn verificar(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    plano: &PlanoTokens,
) -> Result<(), String> {
    let tipos: HashMap<_, _> = f
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
        if conferir(*v, inst, ty, &tipos)? {
            let efeito = plano
                .instrucoes
                .get(v)
                .ok_or_else(|| format!("v{} sem contrato de operação pura", v.0))?;
            if classes.get(v) != Some(&Ownership::Trivial) || *efeito != EfeitoTokens::default() {
                return Err(format!("v{}: contrato incompatível com operação pura", v.0));
            }
        }
    }
    conferir_origens(f, classes)
}
