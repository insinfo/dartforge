//! Pré-condições estruturais para publicar os planos ARC de uma função.
//! Tipagem completa e proveniência continuam pertencendo a outros passes.

use super::super::{
    cfg::{Cfg, sucessores},
    operandos::{operandos, operandos_do_terminador},
};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

pub(super) fn verificar(f: &Function) -> Result<(), String> {
    let erro = |m: String| format!("SSA ARC em {}: {m}", f.symbol);
    if f.blocks.is_empty() {
        return Err(erro("função sem entrada".into()));
    }
    let mut blocos = HashMap::new();
    let mut defs = HashMap::new();
    for (v, _, _) in &f.params {
        if defs.insert(*v, (None, 0, false)).is_some() {
            return Err(erro(format!("v{} repetido", v.0)));
        }
    }
    for (bi, b) in f.blocks.iter().enumerate() {
        if blocos.insert(b.id, bi).is_some() {
            return Err(erro(format!("b{} repetido", b.id.0)));
        }
        let mut ordinaria = false;
        for (i, (v, inst, _)) in b.instructions.iter().enumerate() {
            let phi = matches!(inst, Instruction::Phi { .. });
            if phi && ordinaria {
                return Err(erro(format!("Phi v{} após instrução ordinária", v.0)));
            }
            ordinaria |= !phi;
            if defs.insert(*v, (Some(bi), i, phi)).is_some() {
                return Err(erro(format!("v{} repetido", v.0)));
            }
        }
    }
    for b in &f.blocks {
        for s in sucessores(&b.terminator) {
            if !blocos.contains_key(&s) {
                return Err(erro(format!("b{} aponta para b{} ausente", b.id.0, s.0)));
            }
        }
    }
    let cfg = Cfg::novo(f);
    let tipos: HashMap<_, _> = f
        .params
        .iter()
        .map(|(v, _, ty)| (*v, *ty))
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|(v, _, ty)| (*v, *ty)),
        )
        .collect();
    let referencia = |op: &Operand| {
        matches!(op, Operand::Constant(Constant::Null))
            || matches!(op, Operand::Val(v) if tipos.get(v) == Some(&Type::Ref))
    };
    let conferir = |op: &Operand, bi: usize, pos: usize| -> Result<(), String> {
        let Operand::Val(v) = op else { return Ok(()) };
        let Some(&(origem, i, phi)) = defs.get(v) else {
            return Err(erro(format!("v{} ausente", v.0)));
        };
        let Some(origem) = origem else { return Ok(()) };
        // Nos blocos mortos, exige existência; não inventa dominadores.
        if !cfg.alcancavel(bi) {
            return Ok(());
        }
        let domina = if origem == bi {
            phi || i < pos
        } else {
            let mut atual = bi;
            loop {
                if atual == origem {
                    break true;
                }
                if cfg.idom[atual] == atual {
                    break false;
                }
                atual = cfg.idom[atual];
            }
        };
        if domina {
            Ok(())
        } else {
            Err(erro(format!(
                "v{} não domina uso em b{}",
                v.0, f.blocks[bi].id.0
            )))
        }
    };
    for (bi, b) in f.blocks.iter().enumerate() {
        for (pos, (v, inst, _)) in b.instructions.iter().enumerate() {
            let mut ausente = None;
            operandos(inst, &mut |op| {
                if let Operand::Val(arg) = op
                    && !defs.contains_key(arg)
                    && ausente.is_none()
                {
                    ausente = Some(*arg);
                }
            });
            if let Some(arg) = ausente {
                return Err(erro(format!("v{} ausente", arg.0)));
            }
            match inst {
                Instruction::ArcCopy { value }
                | Instruction::ArcMove { value }
                | Instruction::ArcDrop { value }
                | Instruction::ArcStoreStrong { value, .. }
                    if !referencia(value) =>
                {
                    return Err(erro(format!(
                        "v{}: operação ARC exige referência SSA ou null",
                        v.0
                    )));
                }
                _ => {}
            }
            if let Instruction::ArcLoadStrong { slot } | Instruction::ArcStoreStrong { slot, .. } =
                inst
            {
                if let SlotForte::Quadro { quadro, .. } = slot
                    && !matches!(quadro, Operand::Val(q) if tipos.get(q) == Some(&Type::I64))
                {
                    return Err(erro(format!("v{}: quadro ARC exige SSA I64", v.0)));
                }
            }
            if let Instruction::Phi { incoming, .. } = inst {
                let mut entradas = HashSet::new();
                for (p, op) in incoming {
                    let Some(&pi) = blocos.get(p) else {
                        return Err(erro(format!("predecessor b{} ausente", p.0)));
                    };
                    if !cfg.predecessores[bi].contains(&pi) || !entradas.insert(pi) {
                        return Err(erro(format!(
                            "entrada Phi inválida de b{} para b{}",
                            p.0, b.id.0
                        )));
                    }
                    conferir(op, pi, f.blocks[pi].instructions.len())?;
                }
                if entradas.len() != cfg.predecessores[bi].len() {
                    return Err(erro(format!("Phi em b{} não cobre predecessores", b.id.0)));
                }
            } else {
                let mut resultado = Ok(());
                operandos(inst, &mut |op| {
                    if resultado.is_ok() {
                        resultado = conferir(op, bi, pos);
                    }
                });
                resultado?;
            }
        }
        let mut resultado = Ok(());
        operandos_do_terminador(&b.terminator, &mut |op| {
            if resultado.is_ok() {
                resultado = conferir(op, bi, b.instructions.len());
            }
        });
        resultado?;
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    fn funcao() -> Function {
        Function {
            symbol: "ssa".into(),
            name: "ssa".into(),
            depuracao: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(1),
                    Instruction::ArcCopy {
                        value: Operand::Val(ValueId(0)),
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
            }],
        }
    }

    #[test]
    fn operacoes_arc_nao_criam_referencias_por_anotacao() {
        let mut f = funcao();
        f.params[0].2 = Type::I64;
        assert!(
            verificar(&f)
                .unwrap_err()
                .contains("referência SSA ou null")
        );
        f.blocks[0].instructions[0].1 = Instruction::ArcCopy {
            value: Operand::Constant(Constant::Int(0)),
        };
        assert!(verificar(&f).is_err());
        f.blocks[0].instructions[0].1 = Instruction::ArcCopy {
            value: Operand::Constant(Constant::Null),
        };
        verificar(&f).unwrap();
        f.blocks[0].instructions[0].1 = Instruction::ArcLoadStrong {
            slot: SlotForte::Quadro {
                quadro: Operand::Val(ValueId(0)),
                indice: 0,
            },
        };
        verificar(&f).unwrap();
        f.params[0].2 = Type::Ref;
        assert!(
            verificar(&f)
                .unwrap_err()
                .contains("quadro ARC exige SSA I64")
        );
    }

    #[test]
    fn recusa_destinos_ids_e_usos_invalidos() {
        let mut f = funcao();
        verificar(&f).unwrap();
        f.blocks[0].terminator = Terminator::Branch(BlockId(99));
        assert!(verificar(&f).unwrap_err().contains("b99 ausente"));
        let mut classes = HashMap::from([(ValueId(0), super::super::Ownership::Owned)]);
        let antes = classes.clone();
        let mut plano = super::super::PlanoTokens::default();
        assert!(
            super::super::produzir_e_verificar_tokens(
                &f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &super::super::PlanoEscopos::default()
            )
            .is_err()
        );
        assert_eq!(classes, antes);
        assert!(plano.instrucoes.is_empty());
        f = funcao();
        f.blocks[0].instructions[0].0 = ValueId(0);
        assert!(verificar(&f).unwrap_err().contains("v0 repetido"));
        f = funcao();
        f.blocks[0].instructions[0].1 = Instruction::ArcCopy {
            value: Operand::Val(ValueId(1)),
        };
        assert!(verificar(&f).unwrap_err().contains("não domina"));
        f.blocks[0].instructions[0].1 = Instruction::ArcCopy {
            value: Operand::Val(ValueId(99)),
        };
        assert!(verificar(&f).unwrap_err().contains("v99 ausente"));
    }

    #[test]
    fn phi_de_laco_usa_valor_na_aresta_e_exige_cobertura() {
        let mut f = funcao();
        f.blocks[0].terminator = Terminator::Branch(BlockId(1));
        f.blocks.push(BasicBlock {
            id: BlockId(1),
            instructions: vec![
                (
                    ValueId(2),
                    Instruction::Phi {
                        ty: Type::Ref,
                        incoming: vec![
                            (BlockId(0), Operand::Val(ValueId(1))),
                            (BlockId(1), Operand::Val(ValueId(3))),
                        ],
                    },
                    Type::Ref,
                ),
                (
                    ValueId(3),
                    Instruction::ArcMove {
                        value: Operand::Val(ValueId(2)),
                    },
                    Type::Ref,
                ),
            ],
            terminator: Terminator::Branch(BlockId(1)),
        });
        verificar(&f).unwrap();
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming[0].1 = Operand::Val(ValueId(3));
        }
        assert!(verificar(&f).unwrap_err().contains("não domina"));
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming[0].1 = Operand::Val(ValueId(1));
        }
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming.pop();
        }
        assert!(verificar(&f).unwrap_err().contains("não cobre"));
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming.push((BlockId(0), Operand::Val(ValueId(1))));
        }
        assert!(verificar(&f).unwrap_err().contains("entrada Phi inválida"));
    }
}
