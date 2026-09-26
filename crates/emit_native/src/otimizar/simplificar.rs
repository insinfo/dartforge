//! Limpezas do CFG depois dos outros passes: a conferência de exceção
//! depois de uma chamada que não lança, comparações e desvios constantes,
//! blocos inalcançáveis, blocos encadeados e valores puros sem uso.

use super::cfg::{Cfg, sucessores};
use super::efeitos::{confere_tudo, e_conferencia};
use super::operandos::*;
use crate::hir::*;
use std::collections::{HashMap, HashSet};

/// Troca por 0 a conferência de exceção logo depois de uma chamada a uma
/// função que não lança.
pub fn conferencias_mortas(func: &mut Function, nao_lancam: &HashSet<String>) -> bool {
    if !confere_tudo(func, nao_lancam) {
        return false;
    }
    let mut mortas: HashSet<ValueId> = HashSet::new();
    for b in &func.blocks {
        for par in b.instructions.windows(2) {
            if let Instruction::CallStatic { symbol, .. } = &par[0].1
                && nao_lancam.contains(symbol)
                && e_conferencia(&par[1].1)
            {
                mortas.insert(par[1].0);
            }
        }
    }
    if mortas.is_empty() {
        return false;
    }
    for b in &mut func.blocks {
        b.instructions.retain(|(v, _, _)| !mortas.contains(v));
    }
    substituir(func, &|v| mortas.contains(&v).then_some(Operand::Constant(Constant::Int(0))));
    true
}

fn inteiro(o: &Operand) -> Option<i64> {
    match o {
        Operand::Constant(Constant::Int(n)) => Some(*n),
        Operand::Constant(Constant::Bool(b)) => Some(i64::from(*b)),
        _ => None,
    }
}

/// Dobra comparações inteiras e extensões de constantes, e desvios por
/// condição constante.
pub fn dobrar_constantes(func: &mut Function) -> bool {
    let mut mudou = false;
    loop {
        let mut valores: HashMap<ValueId, Operand> = HashMap::new();
        for b in &func.blocks {
            for (v, inst, _) in &b.instructions {
                let r = match inst {
                    Instruction::ICmp(op, a, c) => match (inteiro(a), inteiro(c)) {
                        (Some(x), Some(y)) => Some(match op {
                            ICmpOp::Eq => x == y,
                            ICmpOp::Ne => x != y,
                            ICmpOp::Slt => x < y,
                            ICmpOp::Sle => x <= y,
                            ICmpOp::Sgt => x > y,
                            ICmpOp::Sge => x >= y,
                            ICmpOp::Ult => (x as u64) < (y as u64),
                        })
                        .map(|b| Operand::Constant(Constant::Bool(b))),
                        _ => None,
                    },
                    Instruction::ZExt { op, from: Type::I1 | Type::I8, .. } => {
                        inteiro(op).map(|n| Operand::Constant(Constant::Int(n & 0xFF)))
                    }
                    _ => None,
                };
                if let Some(r) = r {
                    valores.insert(*v, r);
                }
            }
        }
        let mut mudou_aqui = false;
        if !valores.is_empty() {
            for b in &mut func.blocks {
                b.instructions.retain(|(v, _, _)| !valores.contains_key(v));
            }
            substituir(func, &|v| valores.get(&v).cloned());
            mudou_aqui = true;
        }
        // Desvios constantes.
        let ids: Vec<BlockId> = func.blocks.iter().map(|b| b.id).collect();
        let mut arestas_tiradas: Vec<(BlockId, BlockId)> = Vec::new();
        for (i, b) in func.blocks.iter_mut().enumerate() {
            if let Terminator::CondBranch { cond, then_block, else_block } = &b.terminator
                && let Some(c) = inteiro(cond)
            {
                let (vai, fica) = if c != 0 { (*then_block, *else_block) } else { (*else_block, *then_block) };
                if vai != fica {
                    arestas_tiradas.push((ids[i], fica));
                }
                b.terminator = Terminator::Branch(vai);
                mudou_aqui = true;
            }
        }
        for (origem, destino) in arestas_tiradas {
            if let Some(b) = func.blocks.iter_mut().find(|b| b.id == destino) {
                tirar_entradas(b, origem);
            }
        }
        if !mudou_aqui {
            return mudou;
        }
        mudou = true;
    }
}

fn tirar_entradas(b: &mut BasicBlock, origem: BlockId) {
    for (_, inst, _) in &mut b.instructions {
        if let Instruction::Phi { incoming, .. } = inst {
            incoming.retain(|(o, _)| *o != origem);
        }
    }
}

/// Tira os blocos inalcançáveis (e as entradas de `phi` vindas deles).
pub fn tirar_inalcancaveis(func: &mut Function) -> bool {
    let cfg = Cfg::novo(func);
    let mortos: HashSet<BlockId> =
        func.blocks.iter().enumerate().filter(|(i, _)| !cfg.alcancavel(*i)).map(|(_, b)| b.id).collect();
    if mortos.is_empty() {
        return false;
    }
    func.blocks.retain(|b| !mortos.contains(&b.id));
    for b in &mut func.blocks {
        for (_, inst, _) in &mut b.instructions {
            if let Instruction::Phi { incoming, .. } = inst {
                incoming.retain(|(o, _)| !mortos.contains(o));
            }
        }
    }
    true
}

/// Junta `a -> b` quando `b` só tem `a` como predecessor e `a` só vai a
/// `b` (sem `phi` em `b`, ou com os `phi` de uma entrada resolvidos).
pub fn juntar_blocos(func: &mut Function) -> bool {
    let mut mudou = false;
    loop {
        let mut preds: HashMap<BlockId, usize> = HashMap::new();
        for b in &func.blocks {
            for s in sucessores(&b.terminator) {
                *preds.entry(s).or_default() += 1;
            }
        }
        let alvo = func.blocks.iter().enumerate().find_map(|(i, b)| match b.terminator {
            Terminator::Branch(s) if s != b.id && s.0 != 0 && preds.get(&s) == Some(&1) => Some((i, s)),
            _ => None,
        });
        let Some((i, s)) = alvo else { return mudou };
        let j = func.blocks.iter().position(|b| b.id == s).expect("sucessor existe");
        let mut filho = func.blocks.remove(j);
        let i = if j < i { i - 1 } else { i };
        let pai = func.blocks[i].id;
        // Os `phi` de uma só entrada viram o valor dela.
        let mut troca: HashMap<ValueId, Operand> = HashMap::new();
        filho.instructions.retain(|(v, inst, _)| match inst {
            Instruction::Phi { incoming, .. } => {
                let valor = incoming.first().map(|(_, o)| o.clone()).unwrap_or(Operand::Constant(Constant::Null));
                troca.insert(*v, valor);
                false
            }
            _ => true,
        });
        func.blocks[i].instructions.extend(filho.instructions);
        func.blocks[i].terminator = filho.terminator;
        // Os sucessores do filho agora vêm do pai.
        for b in &mut func.blocks {
            for (_, inst, _) in &mut b.instructions {
                if let Instruction::Phi { incoming, .. } = inst {
                    for (o, _) in incoming.iter_mut() {
                        if *o == s {
                            *o = pai;
                        }
                    }
                }
            }
        }
        if !troca.is_empty() {
            substituir(func, &|v| troca.get(&v).cloned());
        }
        mudou = true;
    }
}

/// A instrução sem efeito além do resultado (tirável se ninguém o usa)?
fn pura(inst: &Instruction) -> bool {
    match inst {
        Instruction::Const(_)
        | Instruction::Add(..)
        | Instruction::Sub(..)
        | Instruction::Mul(..)
        | Instruction::Shl(..)
        | Instruction::AShr(..)
        | Instruction::LShr(..)
        | Instruction::And(..)
        | Instruction::Or(..)
        | Instruction::Xor(..)
        | Instruction::Neg(_)
        | Instruction::Not(_)
        | Instruction::FAdd(..)
        | Instruction::FSub(..)
        | Instruction::FMul(..)
        | Instruction::FDiv(..)
        | Instruction::FNeg(_)
        | Instruction::ICmp(..)
        | Instruction::FCmp(..)
        | Instruction::LNot(_)
        | Instruction::IntToDouble(_)
        | Instruction::ZExt { .. }
        | Instruction::Trunc { .. }
        | Instruction::Bitcast { .. }
        | Instruction::Box { .. }
        | Instruction::GetField { .. }
        | Instruction::Load { .. }
        | Instruction::Phi { .. }
        | Instruction::IsClass { .. }
        | Instruction::EnvGet { .. }
        | Instruction::CellGet { .. } => true,
        Instruction::Unbox { to, .. } => to.e_vetor(),
        Instruction::Simd { op, .. } => *op != OpSimd::Grava,
        Instruction::CallRuntime { name, .. } => matches!(
            name.as_str(),
            "dartforge_object_new" | "dartforge_object_campos" | "dartforge_exception_pending" | "dartforge_typed_len" | "dartforge_typed_ptr"
        ),
        _ => false,
    }
}

/// Tira os valores puros que ninguém usa.
pub fn tirar_mortos(func: &mut Function) -> bool {
    let mut mudou = false;
    loop {
        let mut usados: HashSet<ValueId> = HashSet::new();
        for b in &func.blocks {
            for (v, inst, _) in &b.instructions {
                operandos(inst, &mut |o| {
                    if let Operand::Val(u) = o
                        && u != v
                    {
                        usados.insert(*u);
                    }
                });
            }
            operandos_do_terminador(&b.terminator, &mut |o| {
                if let Operand::Val(u) = o {
                    usados.insert(*u);
                }
            });
        }
        let mut tirou = false;
        for b in &mut func.blocks {
            let antes = b.instructions.len();
            b.instructions.retain(|(v, inst, _)| usados.contains(v) || !pura(inst));
            tirou |= b.instructions.len() != antes;
        }
        if !tirou {
            return mudou;
        }
        mudou = true;
    }
}
