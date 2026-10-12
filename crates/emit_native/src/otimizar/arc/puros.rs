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
        Instruction::ConstArray(_) => Some(Type::Ptr),
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
        // A lógica bit a bit é emitida em i64 com os operandos estendidos
        // (`llvm/mod.rs`); o lowering a declara I1 quando a usa como `bool`.
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
        // Uma global de 1 ou 8 bits, ou `double`, não guarda handle; I64 pode
        // guardar e exige prova própria (`caches_rti`).
        Instruction::LoadGlobal { ty: t @ (Type::I1 | Type::I8 | Type::F64), .. } => Some(*t),
        // Gravar um escalar comprovado (`conferir_origens`) numa global não-Ref.
        Instruction::StoreGlobal { ty: Type::I1 | Type::I8 | Type::I64 | Type::F64, raiz: None, .. } => Some(Type::Void),
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
        let integral = |op: &Operand| match op {
            Operand::Constant(Constant::Int(_) | Constant::Bool(_)) => true,
            Operand::Val(v) => matches!(tipos.get(v), Some(Type::I64 | Type::I1 | Type::I8)),
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
            | Instruction::LShr(a, b) => !inteiro(a) || !inteiro(b),
            Instruction::And(a, b) | Instruction::Or(a, b) | Instruction::Xor(a, b) => {
                !integral(a) || !integral(b)
            }
            Instruction::Neg(op) | Instruction::Not(op) | Instruction::IntToDouble(op) => {
                !inteiro(op)
            }
            Instruction::FAdd(a, b)
            | Instruction::FSub(a, b)
            | Instruction::FMul(a, b)
            | Instruction::FDiv(a, b) => !numerico(a) || !numerico(b),
            Instruction::FNeg(op) | Instruction::DoubleToInt(op) => !numerico(op),
            Instruction::StoreGlobal { val, ty: gravado, .. } => match val {
                Operand::Val(v) => tipos.get(v) != Some(gravado),
                Operand::Constant(Constant::Int(_)) => !matches!(gravado, Type::I64 | Type::I8),
                Operand::Constant(Constant::Bool(_)) => !matches!(gravado, Type::I1 | Type::I8),
                Operand::Constant(Constant::Double(_)) => *gravado != Type::F64,
                _ => true,
            },
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
        if let Instruction::Box { op: Operand::Val(origem), from } = inst
            && from.e_vetor() && classes.get(origem) != Some(&Ownership::Trivial) {
            return Err(format!("v{}: caixa SIMD exige origem Trivial em {}", v.0, f.symbol));
        }
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
        for op in operandos.into_iter().flatten() {
            if let Operand::Val(origem) = op
                && classes.get(origem) != Some(&Ownership::Trivial)
            {
                return Err(format!("v{}: operação escalar exige origem Trivial em {} (origem v{})", v.0, f.symbol, origem.0));
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

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn logica_bit_a_bit_aceita_operandos_integrais_em_i1_ou_i64() {
        let tipos = HashMap::from([(ValueId(1), Type::I1), (ValueId(2), Type::I1), (ValueId(3), Type::I64), (ValueId(4), Type::Ref), (ValueId(5), Type::F64)]);
        let v = |n| Operand::Val(ValueId(n));
        let ou = |a, b| Instruction::Or(a, b);
        assert!(conferir(ValueId(9), &ou(v(1), v(2)), &Type::I1, &tipos).unwrap());
        assert!(conferir(ValueId(9), &Instruction::And(v(1), v(2)), &Type::I64, &tipos).unwrap());
        assert!(conferir(ValueId(9), &Instruction::And(v(1), Operand::Constant(Constant::Bool(true))), &Type::I1, &tipos).unwrap());
        assert!(conferir(ValueId(9), &ou(v(3), Operand::Constant(Constant::Int(1))), &Type::I64, &tipos).unwrap());
        assert!(conferir(ValueId(9), &ou(v(1), v(3)), &Type::I64, &tipos).unwrap());
        // Referência, double ou resultado de outra representação.
        assert!(conferir(ValueId(9), &Instruction::Xor(v(4), v(3)), &Type::I64, &tipos).is_err());
        assert!(conferir(ValueId(9), &Instruction::And(v(5), v(3)), &Type::I64, &tipos).is_err());
        assert!(conferir(ValueId(9), &ou(v(1), v(2)), &Type::F64, &tipos).is_err());
        // Os deslocamentos seguem exigindo inteiros.
        assert!(conferir(ValueId(9), &Instruction::Shl(v(1), v(3)), &Type::I64, &tipos).is_err());
        // Globais estreitas ou `double` são escalares; I64 não é por largura.
        let global = |ty| Instruction::LoadGlobal { simbolo: "g".into(), ty };
        assert!(conferir(ValueId(9), &global(Type::I8), &Type::I8, &tipos).unwrap());
        assert!(conferir(ValueId(9), &global(Type::F64), &Type::F64, &tipos).unwrap());
        assert!(!conferir(ValueId(9), &global(Type::I64), &Type::I64, &tipos).unwrap());
        assert!(conferir(ValueId(9), &global(Type::I8), &Type::I64, &tipos).is_err());
        let gravar = |ty, val| Instruction::StoreGlobal { simbolo: "g".into(), val, ty, raiz: None };
        assert!(conferir(ValueId(9), &gravar(Type::I8, Operand::Constant(Constant::Int(1))), &Type::Void, &tipos).unwrap());
        assert!(conferir(ValueId(9), &gravar(Type::I64, v(3)), &Type::Void, &tipos).unwrap());
        assert!(conferir(ValueId(9), &gravar(Type::I64, v(4)), &Type::Void, &tipos).is_err());
        assert!(!conferir(ValueId(9), &gravar(Type::Ref, v(4)), &Type::Void, &tipos).unwrap());
    }
}
