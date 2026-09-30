//! Os operandos de uma instrução e de um terminador, para ler e para
//! trocar (inclusive as entradas dos `phi`).

use crate::hir::*;

macro_rules! visitar {
    ($nome:ident, $nome_t:ident, $($m:tt)*) => {
        /// Visita cada operando da instrução, na ordem de avaliação.
        pub fn $nome(inst: & $($m)* Instruction, f: &mut dyn FnMut(& $($m)* Operand)) {
            match inst {
                Instruction::Const(_)
                | Instruction::Alloca(_)
                | Instruction::LoadGlobal { .. }
                | Instruction::TearOff { .. }
                | Instruction::ConstArray(_) => {}
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
                | Instruction::ICmp(_, a, b)
                | Instruction::FCmp(_, a, b) => {
                    f(a);
                    f(b);
                }
                Instruction::Neg(a)
                | Instruction::Not(a)
                | Instruction::FNeg(a)
                | Instruction::LNot(a)
                | Instruction::IntToDouble(a)
                | Instruction::DoubleToInt(a)
                | Instruction::CheckNotNull(a) => f(a),
                Instruction::ZExt { op: a, .. }
                | Instruction::Trunc { op: a, .. }
                | Instruction::Bitcast { op: a, .. }
                | Instruction::Box { op: a, .. }
                | Instruction::Unbox { op: a, .. } => f(a),
                Instruction::AllocObject { fields, .. } => fields.$nome_t().for_each(|a| f(a)),
                Instruction::AllocList { elements } | Instruction::AllocRecord { elements } => {
                    elements.$nome_t().for_each(|(a, _)| f(a))
                }
                Instruction::AllocCell { value } => f(value),
                Instruction::AllocEnv { values } => values.$nome_t().for_each(|a| f(a)),
                Instruction::JuntarTextos { partes } => partes.$nome_t().for_each(|a| f(a)),
                Instruction::AllocClosure { env, .. } | Instruction::AllocClosureTipada { env, .. } => f(env),
                Instruction::ChamadaTipada { alvo, args, .. } => {
                    f(alvo);
                    args.$nome_t().for_each(|(a, _)| f(a));
                }
                Instruction::Load { ptr, .. } => f(ptr),
                Instruction::Store { ptr, val } => {
                    f(ptr);
                    f(val);
                }
                Instruction::GetField { object, .. } => f(object),
                Instruction::SetField { object, value, .. } => {
                    f(object);
                    f(value);
                }
                Instruction::GetListElement { list, index } => {
                    f(list);
                    f(index);
                }
                Instruction::SetListElement { list, index, value } => {
                    f(list);
                    f(index);
                    f(value);
                }
                Instruction::CellGet { cell } => f(cell),
                Instruction::CellSet { cell, value } => {
                    f(cell);
                    f(value);
                }
                Instruction::EnvGet { env, .. } => f(env),
                Instruction::CallStatic { args, .. } => args.$nome_t().for_each(|a| f(a)),
                Instruction::CallInterface { receiver, args, .. } | Instruction::CallDynamic { receiver, args, .. } => {
                    f(receiver);
                    args.$nome_t().for_each(|a| f(a));
                }
                Instruction::CallClosure { closure, args, tupla_tipos, .. } => {
                    f(closure);
                    args.$nome_t().for_each(|a| f(a));
                    f(tupla_tipos);
                }
                Instruction::CallRuntime { args, .. } => args.$nome_t().for_each(|(a, _)| f(a)),
                Instruction::IsClass { object, .. } => f(object),
                Instruction::StoreGlobal { val, .. } => f(val),
                Instruction::Phi { incoming, .. } => incoming.$nome_t().for_each(|(_, a)| f(a)),
                Instruction::LoadIndexed { base, index } => {
                    f(base);
                    f(index);
                }
                Instruction::CallSeletor { recv, args, tupla_tipos, .. } => {
                    f(recv);
                    args.$nome_t().for_each(|a| f(a));
                    f(tupla_tipos);
                }
                Instruction::CallClosureRepasse { closure, args, desc } => {
                    f(closure);
                    f(args);
                    f(desc);
                }
                Instruction::CallSeletorRepasse { recv, args, desc, .. } => {
                    f(recv);
                    f(args);
                    f(desc);
                }
                Instruction::ChamadaNativa { alvo, args, .. } => {
                    f(alvo);
                    args.$nome_t().for_each(|(a, _)| f(a));
                }
                Instruction::CargaNativa { endereco, indice, .. } => {
                    f(endereco);
                    f(indice);
                }
                Instruction::GravacaoNativa { endereco, indice, valor, .. } => {
                    f(endereco);
                    f(indice);
                    f(valor);
                }
                Instruction::ChamadaNativaComposta { alvo, args, destino, .. } => {
                    f(alvo);
                    args.$nome_t().for_each(|(a, _)| f(a));
                    destino.$nome_t().for_each(|a| f(a));
                }
                Instruction::Simd { args, .. } => args.$nome_t().for_each(|a| f(a)),
            }
        }
    };
}

visitar!(operandos, iter,);
visitar!(operandos_mut, iter_mut, mut);

/// Visita os operandos do terminador.
pub fn operandos_do_terminador_mut(t: &mut Terminator, f: &mut dyn FnMut(&mut Operand)) {
    match t {
        Terminator::Return(Some(o)) | Terminator::Throw(o) => f(o),
        Terminator::CondBranch { cond, .. } => f(cond),
        Terminator::Switch { val, .. } => f(val),
        Terminator::Return(None) | Terminator::Branch(_) | Terminator::Unreachable => {}
    }
}

/// Visita os operandos do terminador (leitura).
pub fn operandos_do_terminador(t: &Terminator, f: &mut dyn FnMut(&Operand)) {
    match t {
        Terminator::Return(Some(o)) | Terminator::Throw(o) => f(o),
        Terminator::CondBranch { cond, .. } => f(cond),
        Terminator::Switch { val, .. } => f(val),
        Terminator::Return(None) | Terminator::Branch(_) | Terminator::Unreachable => {}
    }
}

/// Os sucessores do terminador, trocáveis.
pub fn sucessores_mut(t: &mut Terminator, f: &mut dyn FnMut(&mut BlockId)) {
    match t {
        Terminator::Branch(b) => f(b),
        Terminator::CondBranch { then_block, else_block, .. } => {
            f(then_block);
            f(else_block);
        }
        Terminator::Switch { default, cases, .. } => {
            f(default);
            cases.iter_mut().for_each(|(_, b)| f(b));
        }
        Terminator::Return(_) | Terminator::Throw(_) | Terminator::Unreachable => {}
    }
}

/// Troca, na função inteira, cada uso de um valor pelo operando que
/// `troca` der.
pub fn substituir(func: &mut Function, troca: &dyn Fn(ValueId) -> Option<Operand>) {
    let mut t = |o: &mut Operand| {
        if let Operand::Val(v) = o
            && let Some(n) = troca(*v)
        {
            *o = n;
        }
    };
    for b in &mut func.blocks {
        for (_, inst, _) in &mut b.instructions {
            operandos_mut(inst, &mut t);
        }
        operandos_do_terminador_mut(&mut b.terminator, &mut t);
    }
}

/// O maior id de valor e de bloco da função (para numerar os novos).
pub fn maiores_ids(func: &Function) -> (u32, u32) {
    let mut v = func.params.iter().map(|(p, _, _)| p.0).max().unwrap_or(0);
    let mut b = 0;
    for bl in &func.blocks {
        b = b.max(bl.id.0);
        for (id, _, _) in &bl.instructions {
            v = v.max(id.0);
        }
    }
    (v, b)
}

/// O tipo de um operando: o registrado do valor, ou o natural da constante.
pub fn tipo_do_operando(o: &Operand, tipos: &std::collections::HashMap<ValueId, Type>) -> Option<Type> {
    match o {
        Operand::Val(v) => tipos.get(v).copied(),
        Operand::Constant(c) => Some(match c {
            Constant::Int(_) => Type::I64,
            Constant::Double(_) => Type::F64,
            Constant::Bool(_) => Type::I1,
            Constant::String(_) | Constant::StringWtf8(_) | Constant::Null => Type::Ref,
            Constant::Funcao(_) => Type::I64,
        }),
    }
}

/// Os tipos de todos os valores da função (parâmetros e instruções).
pub fn tipos_da_funcao(func: &Function) -> std::collections::HashMap<ValueId, Type> {
    let mut t: std::collections::HashMap<ValueId, Type> = func.params.iter().map(|(v, _, ty)| (*v, *ty)).collect();
    for b in &func.blocks {
        for (v, _, ty) in &b.instructions {
            t.insert(*v, *ty);
        }
    }
    t
}

/// A constante neutra de um tipo (o valor de um local lido antes de
/// gravado, que o Dart só permite para `null`).
pub fn constante_padrao(ty: Type) -> Operand {
    Operand::Constant(match ty {
        Type::I64 | Type::I8 => Constant::Int(0),
        Type::F64 => Constant::Double(0.0),
        Type::I1 => Constant::Bool(false),
        _ => Constant::Null,
    })
}
