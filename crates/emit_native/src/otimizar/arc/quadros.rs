//! Vida dos quadros proprietários locais abertos pela ABI ARC auditada.
//! Exige IDs SSA diretos e confere limites constantes quando a capacidade é
//! conhecida; não certifica quadros importados, aliases ou limites dinâmicos.

use super::super::cfg::Cfg;
use crate::hir::*;
use std::collections::{HashMap, VecDeque};

pub(super) fn verificar(f: &Function) -> Result<(), String> {
    analisar(f).map(|_| ())
}

fn analisar(f: &Function) -> Result<HashMap<BlockId, Vec<ValueId>>, String> {
    let cfg = Cfg::novo(f);
    let capacidades: HashMap<_, _> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter_map(|(v, inst, _)| match inst {
            Instruction::CallRuntime { name, args, .. }
                if name == "dartforge_arc_quadro_abrir_v1" =>
            {
                match args.first() {
                    Some((Operand::Constant(Constant::Int(n)), _)) => Some((*v, *n)),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect();
    if f.blocks.is_empty() {
        return Err("quadros ARC: função sem entrada".into());
    }
    let mut entradas: Vec<Option<Vec<ValueId>>> = vec![None; f.blocks.len()];
    entradas[0] = Some(Vec::new());
    let mut fila = VecDeque::from([0]);
    while let Some(bi) = fila.pop_front() {
        let b = &f.blocks[bi];
        let erro = |m: String| format!("quadros ARC em {} b{}: {m}", f.symbol, b.id.0);
        let mut ativos = entradas[bi].clone().unwrap();
        for (v, inst, _) in &b.instructions {
            let usar = |op: &Operand, ativos: &[ValueId]| -> Result<ValueId, String> {
                let Operand::Val(q) = op else {
                    return Err(erro("quadro exige ID SSA local".into()));
                };
                if !ativos.contains(q) {
                    return Err(erro(format!("quadro v{} não está aberto", q.0)));
                }
                Ok(*q)
            };
            let indice = |q: ValueId, i: i64| -> Result<(), String> {
                if i < 0 || capacidades.get(&q).is_some_and(|n| i >= *n) {
                    return Err(erro(format!("índice {i} fora do quadro v{}", q.0)));
                }
                Ok(())
            };
            match inst {
                Instruction::ArcLoadStrong {
                    slot: SlotForte::Quadro { quadro, indice: i },
                }
                | Instruction::ArcStoreStrong {
                    slot: SlotForte::Quadro { quadro, indice: i },
                    ..
                } => {
                    let q = usar(quadro, &ativos)?;
                    indice(q, i64::from(*i))?;
                }
                Instruction::CallRuntime { name, args, .. }
                    if name.starts_with("dartforge_arc_quadro_") =>
                {
                    if name == "dartforge_arc_quadro_abrir_v1" {
                        if capacidades.get(v).is_some_and(|n| *n < 0) {
                            return Err(erro("capacidade negativa de quadro".into()));
                        }
                        if ativos.contains(v) {
                            return Err(erro(format!("quadro v{} aberto duas vezes", v.0)));
                        }
                        ativos.push(*v);
                    } else {
                        let primeiro = args
                            .first()
                            .ok_or_else(|| erro("quadro sem argumento".into()))?;
                        let q = usar(&primeiro.0, &ativos)?;
                        if name != "dartforge_arc_quadro_fechar_v1" {
                            if let Some((Operand::Constant(Constant::Int(i)), _)) = args.get(1) {
                                indice(q, *i)?;
                            }
                        }
                        if name == "dartforge_arc_quadro_fechar_v1" {
                            if ativos.last() != Some(&q) {
                                return Err(erro(format!("fechamento fora de LIFO de v{}", q.0)));
                            }
                            ativos.pop();
                        } else if name == "dartforge_arc_quadro_mover_v1" {
                            let destino = args
                                .get(2)
                                .ok_or_else(|| erro("move sem quadro destino".into()))?;
                            let q_destino = usar(&destino.0, &ativos)?;
                            if let Some((Operand::Constant(Constant::Int(i)), _)) = args.get(3) {
                                indice(q_destino, *i)?;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if cfg.sucessores[bi].is_empty() && !ativos.is_empty() {
            return Err(erro(format!("quadros ainda abertos na saída: {ativos:?}")));
        }
        for &s in &cfg.sucessores[bi] {
            match &entradas[s] {
                Some(anterior) if *anterior != ativos => {
                    return Err(erro(format!(
                        "pilhas de quadros diferentes na junção b{}",
                        f.blocks[s].id.0
                    )));
                }
                Some(_) => {}
                None => {
                    entradas[s] = Some(ativos.clone());
                    fila.push_back(s);
                }
            }
        }
    }
    Ok(f.blocks
        .iter()
        .zip(entradas)
        .filter_map(|(b, q)| q.map(|q| (b.id, q)))
        .collect())
}

pub(super) fn na_entrada_dos_pousos(
    f: &Function,
    t: &TabelasDaFuncao,
) -> Result<HashMap<BlockId, Vec<ValueId>>, String> {
    let entradas = analisar(f)?;
    let mut resultado = HashMap::new();
    let mut pousos: Vec<_> = t.pousos.iter().copied().collect();
    pousos.sort_by_key(|b| b.0);
    for b in pousos {
        let quadros = entradas
            .get(&b)
            .ok_or_else(|| format!("quadros ARC: pouso b{} sem entrada", b.0))?;
        resultado.insert(b, quadros.clone());
    }
    Ok(resultado)
}

#[cfg(test)]
mod testes {
    use super::*;
    fn chamada(
        v: u32,
        nome: &str,
        args: Vec<(Operand, Type)>,
        ty: Type,
    ) -> (ValueId, Instruction, Type) {
        (
            ValueId(v),
            Instruction::CallRuntime {
                name: nome.into(),
                args,
                ret_ty: ty,
            },
            ty,
        )
    }
    fn exemplo() -> Function {
        Function {
            symbol: "quadros".into(),
            name: "quadros".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    chamada(
                        0,
                        "dartforge_arc_quadro_abrir_v1",
                        vec![(Operand::Constant(Constant::Int(1)), Type::I64)],
                        Type::I64,
                    ),
                    chamada(
                        1,
                        "dartforge_arc_quadro_fechar_v1",
                        vec![(Operand::Val(ValueId(0)), Type::I64)],
                        Type::Void,
                    ),
                ],
                terminator: Terminator::Return(None),
            }],
        }
    }
    #[test]
    fn fechamento_e_uso_exigem_quadro_local_ativo() {
        let mut f = exemplo();
        verificar(&f).unwrap();
        f.blocks[0].instructions.push((
            ValueId(2),
            Instruction::ArcLoadStrong {
                slot: SlotForte::Quadro {
                    quadro: Operand::Val(ValueId(0)),
                    indice: 0,
                },
            },
            Type::Ref,
        ));
        assert!(verificar(&f).unwrap_err().contains("não está aberto"));
        f.blocks[0].instructions.pop();
        f.blocks[0].instructions.pop();
        assert!(verificar(&f).unwrap_err().contains("ainda abertos"));
        f = exemplo();
        let fechar = f.blocks[0].instructions[1].clone();
        f.blocks[0].instructions.push(fechar);
        assert!(verificar(&f).unwrap_err().contains("não está aberto"));
    }
    #[test]
    fn pilha_divergente_no_cfg_nao_publica_planos() {
        let mut f = exemplo();
        let fechamento = f.blocks[0].instructions.pop().unwrap();
        f.blocks[0].terminator = Terminator::CondBranch {
            cond: Operand::Constant(Constant::Bool(true)),
            then_block: BlockId(1),
            else_block: BlockId(2),
        };
        f.blocks.extend([
            BasicBlock {
                id: BlockId(1),
                instructions: vec![fechamento],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
        ]);
        let mut classes = std::collections::HashMap::new();
        let mut plano = super::super::PlanoTokens::default();
        let erro = super::super::produzir_e_verificar_tokens(
            &f,
            &mut classes,
            &mut plano,
            &TabelasDaFuncao::default(),
            &super::super::PlanoEscopos::default(),
        )
        .unwrap_err();
        assert!(erro.contains("pilhas de quadros diferentes"), "{erro}");
        assert!(classes.is_empty() && plano.instrucoes.is_empty());
    }

    #[test]
    fn slots_constantes_exigem_indice_dentro_da_capacidade() {
        let mut f = exemplo();
        f.blocks[0].instructions.insert(
            1,
            (
                ValueId(2),
                Instruction::ArcLoadStrong {
                    slot: SlotForte::Quadro {
                        quadro: Operand::Val(ValueId(0)),
                        indice: 0,
                    },
                },
                Type::Ref,
            ),
        );
        verificar(&f).unwrap();
        f.blocks[0].instructions[1].1 = Instruction::ArcLoadStrong {
            slot: SlotForte::Quadro {
                quadro: Operand::Val(ValueId(0)),
                indice: 1,
            },
        };
        assert!(verificar(&f).unwrap_err().contains("índice 1 fora"));
        f.blocks[0].instructions[1] = chamada(
            2,
            "dartforge_arc_quadro_carregar_v1",
            vec![
                (Operand::Val(ValueId(0)), Type::I64),
                (Operand::Constant(Constant::Int(-1)), Type::I64),
            ],
            Type::Ref,
        );
        assert!(verificar(&f).unwrap_err().contains("índice -1 fora"));
        f = exemplo();
        f.blocks[0].instructions[0] = chamada(
            0,
            "dartforge_arc_quadro_abrir_v1",
            vec![(Operand::Constant(Constant::Int(-1)), Type::I64)],
            Type::I64,
        );
        assert!(verificar(&f).unwrap_err().contains("capacidade negativa"));
    }

    #[test]
    fn fechamento_obedece_lifo() {
        let mut f = exemplo();
        f.blocks[0].instructions.insert(
            1,
            chamada(
                2,
                "dartforge_arc_quadro_abrir_v1",
                vec![(Operand::Constant(Constant::Int(1)), Type::I64)],
                Type::I64,
            ),
        );
        assert!(verificar(&f).unwrap_err().contains("fora de LIFO"));
    }
}
