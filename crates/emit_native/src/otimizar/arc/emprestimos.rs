//! Barreiras conservadoras para aliases derivados de owners locais.
//! Não produz keepalive, prova aliases de slots ou certifica versões/pins.

use super::super::{
    cfg::Cfg,
    operandos::{operandos, operandos_do_terminador},
};
use super::{OrigemOwner, Ownership};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

pub(super) fn verificar(f: &Function, classes: &HashMap<ValueId, Ownership>) -> Result<(), String> {
    let derivados: HashSet<_> = classes
        .iter()
        .filter_map(|(v, c)| {
            matches!(
                c,
                Ownership::Borrowed {
                    owner: OrigemOwner::Valor(_),
                    ..
                }
            )
            .then_some(*v)
        })
        .collect();
    if derivados.is_empty() {
        return Ok(());
    }
    let tipos: HashMap<_, _> = f
        .params
        .iter()
        .map(|(v, _, t)| (*v, *t))
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| b.instructions.iter().map(|(v, _, t)| (*v, *t))),
        )
        .collect();
    let cfg = Cfg::novo(f);
    let pos: HashMap<_, _> = f
        .blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect();
    let mut barreiras = HashSet::new();
    let mut reentradas = HashSet::new();
    for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let unbox_bool = super::contratos::chamada_unbox_bool(inst);
        let chamada = unbox_bool.as_ref().unwrap_or(inst);
        let invalida = match chamada {
            Instruction::CallRuntime { name, .. } => {
                match dartforge_runtime::ownership::contrato(name) {
                    Ok(c) => {
                        if c.chama_dart {
                            reentradas.insert(*v);
                        }
                        c.invalida_borrows
                    }
                    Err(_) => {
                        reentradas.insert(*v);
                        true // Contrato fornecido não prova ausência de invalidação.
                    }
                }
            }
            Instruction::Phi { .. }
            | Instruction::ArcCopy { .. }
            | Instruction::ArcMove { .. }
            | Instruction::ArcLoadStrong { .. }
            | Instruction::Alloca(_)
            | Instruction::Load { .. }
            | Instruction::GetField { .. }
            | Instruction::EnvGet { .. } => false,
            Instruction::ArcDrop { .. } | Instruction::ArcStoreStrong { .. } => true,
            Instruction::CallStatic { .. }
            | Instruction::CallClosure { .. }
            | Instruction::CallClosureRepasse { .. } => {
                reentradas.insert(*v);
                true
            }
            Instruction::Const(Constant::String(_) | Constant::StringWtf8(_)) => true,
            _ => {
                let invalida = !super::puros::conferir(*v, inst, ty, &tipos)?;
                if invalida {
                    reentradas.insert(*v);
                }
                invalida
            }
        };
        if invalida {
            barreiras.insert(*v);
        }
    }
    let transferir = |bi: usize, mut estado: HashSet<ValueId>| {
        for (v, _, _) in &f.blocks[bi].instructions {
            if barreiras.contains(v) {
                estado.extend(&derivados);
            }
            // A definição nasce depois do efeito da chamada. Phi só é válido
            // se cada operando for válido em sua própria aresta, conferida abaixo.
            estado.remove(v);
        }
        estado
    };
    let mut entradas = vec![HashSet::new(); f.blocks.len()];
    let mut saidas = entradas.clone();
    loop {
        let mut mudou = false;
        for &bi in &cfg.rpo {
            let mut estado = HashSet::new();
            for &p in &cfg.predecessores[bi] {
                if cfg.idom[p] != usize::MAX {
                    estado.extend(&saidas[p]);
                }
            }
            let saida = transferir(bi, estado.clone());
            mudou |= entradas[bi] != estado || saidas[bi] != saida;
            entradas[bi] = estado;
            saidas[bi] = saida;
        }
        if !mudou {
            break;
        }
    }
    let conferir = |op: &Operand, estado: &HashSet<ValueId>, ponto: String| {
        if let Operand::Val(v) = op {
            let mut atual = *v;
            let mut vistos = HashSet::new();
            while vistos.insert(atual) {
                if estado.contains(&atual) {
                    return Err(format!(
                        "empréstimo invalidado em {}: {ponto}: v{} depende de v{}; copie para Owned antes da barreira",
                        f.symbol, v.0, atual.0
                    ));
                }
                match classes.get(&atual) {
                    Some(Ownership::Borrowed {
                        owner: OrigemOwner::Valor(p),
                        ..
                    }) => atual = *p,
                    _ => break,
                }
            }
        }
        Ok(())
    };
    for &bi in &cfg.rpo {
        let b = &f.blocks[bi];
        let mut estado = entradas[bi].clone();
        for (v, inst, _) in &b.instructions {
            if let Instruction::Phi { incoming, .. } = inst {
                for (p, op) in incoming {
                    let pi = pos[p];
                    if cfg.idom[pi] != usize::MAX {
                        conferir(
                            op,
                            &saidas[pi],
                            format!("b{} -> b{} Phi v{}", p.0, b.id.0, v.0),
                        )?;
                    }
                }
            } else {
                // Uma chamada que pode reentrar Dart não pode depender de
                // uma aresta mutável nem durante sua própria execução.
                if reentradas.contains(v) {
                    estado.extend(&derivados);
                }
                let mut erro = None;
                operandos(inst, &mut |op| {
                    if erro.is_none() {
                        erro = conferir(op, &estado, format!("b{} v{}", b.id.0, v.0)).err();
                    }
                });
                if let Some(erro) = erro {
                    return Err(erro);
                }
            }
            if barreiras.contains(v) {
                estado.extend(&derivados);
            }
            estado.remove(v);
        }
        let mut erro = None;
        operandos_do_terminador(&b.terminator, &mut |op| {
            if erro.is_none() {
                erro = conferir(op, &estado, format!("b{} saída", b.id.0)).err();
            }
        });
        if let Some(erro) = erro {
            return Err(erro);
        }
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    fn leitura() -> (ValueId, Instruction, Type) {
        (
            ValueId(2),
            Instruction::CallRuntime {
                name: "dartforge_arc_ler_campo_ref_v1".into(),
                args: vec![
                    (Operand::Val(ValueId(0)), Type::Ref),
                    (Operand::Constant(Constant::Int(0)), Type::I64),
                ],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )
    }

    fn troca() -> (ValueId, Instruction, Type) {
        (
            ValueId(5),
            Instruction::CallRuntime {
                name: "dartforge_arc_gravar_campo_ref_v1".into(),
                args: vec![
                    (Operand::Val(ValueId(0)), Type::Ref),
                    (Operand::Constant(Constant::Int(0)), Type::I64),
                    (Operand::Constant(Constant::Null), Type::Ref),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        )
    }

    fn funcao(blocks: Vec<BasicBlock>) -> (Function, HashMap<ValueId, Ownership>) {
        (
            Function {
                symbol: "fluxo_borrow".into(),
                name: "fluxo_borrow".into(),
                depuracao: None,
                params: vec![
                    (ValueId(0), "objeto".into(), Type::Ref),
                    (ValueId(1), "cond".into(), Type::I1),
                ],
                return_ty: Type::Void,
                blocks,
            },
            HashMap::from([
                (
                    ValueId(0),
                    Ownership::Borrowed {
                        owner: OrigemOwner::Chamador,
                        escopo: 0,
                    },
                ),
                (
                    ValueId(2),
                    Ownership::Borrowed {
                        owner: OrigemOwner::Valor(ValueId(0)),
                        escopo: 0,
                    },
                ),
                (
                    ValueId(4),
                    Ownership::Borrowed {
                        owner: OrigemOwner::Valor(ValueId(0)),
                        escopo: 0,
                    },
                ),
            ]),
        )
    }

    #[test]
    fn phi_confere_cada_aresta_e_laco_nao_revalida_leitura_da_entrada() {
        for invalida in [false, true] {
            let (f, c) = funcao(vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![leitura()],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(1)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: if invalida { vec![troca()] } else { vec![] },
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(3),
                    instructions: vec![(
                        ValueId(4),
                        Instruction::Phi {
                            incoming: vec![
                                (BlockId(1), Operand::Val(ValueId(2))),
                                (BlockId(2), Operand::Val(ValueId(2))),
                            ],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(None),
                },
            ]);
            let resultado = verificar(&f, &c);
            if invalida {
                assert!(resultado.unwrap_err().contains("b1 -> b3 Phi v4"));
            } else {
                resultado.unwrap();
            }
        }
        for reler in [false, true] {
            let mut corpo = vec![
                (
                    ValueId(3),
                    Instruction::ArcCopy {
                        value: Operand::Val(ValueId(2)),
                    },
                    Type::Ref,
                ),
                troca(),
            ];
            if reler {
                corpo.insert(0, leitura());
            }
            let (f, c) = funcao(vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: if reler { vec![] } else { vec![leitura()] },
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: corpo,
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(1)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                },
            ]);
            let resultado = verificar(&f, &c);
            if reler {
                resultado.unwrap();
            } else {
                assert!(resultado.unwrap_err().contains("empréstimo invalidado"));
            }
        }
    }

    #[test]
    fn chamada_opaca_exige_owner_independente_do_argumento_derivado() {
        let (f, c) = funcao(vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                leitura(),
                (
                    ValueId(3),
                    Instruction::CallStatic {
                        symbol: "callback".into(),
                        args: vec![Operand::Val(ValueId(2))],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }]);
        assert!(verificar(&f, &c).unwrap_err().contains("b0 v3"));
    }
}
