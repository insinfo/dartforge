//! Expõe o sucesso e o unwind dos retornos Guarda antes de inserir tokens.
//! Este passe altera somente cópias temporárias na transação do conjunto Dart.

use super::*;

pub(super) fn separar_guardas(f: &mut Function, plano: &mut PlanoFuncaoDart) -> Result<(), String> {
    super::ssa::verificar(f)?;
    let guardas: Vec<_> = f
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| plano.tabelas.saidas.get(&b.id) == Some(&SaidaPorExcecao::Guarda))
        .map(|(indice, b)| (indice, b.id))
        .collect();
    if guardas.is_empty() {
        return Ok(());
    }
    let mut valor = f
        .params
        .iter()
        .map(|(v, _, _)| v.0)
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|(v, _, _)| v.0),
        )
        .chain(plano.classes.keys().map(|v| v.0))
        .chain(plano.tokens.instrucoes.keys().map(|v| v.0))
        .chain(plano.tokens.pendencias.keys().map(|v| v.0))
        .chain(plano.tabelas.invocacoes.keys().map(|v| v.0))
        .chain(plano.escopos.antes.keys().map(|v| v.0))
        .max()
        .map_or(0, |v| u64::from(v) + 1);
    // Metadados obsoletos não podem coincidir com blocos recém-inseridos.
    let mut bloco = f
        .blocks
        .iter()
        .map(|b| b.id.0)
        .chain(plano.tabelas.saidas.keys().map(|b| b.0))
        .chain(plano.tabelas.pousos.iter().map(|b| b.0))
        .chain(plano.tabelas.invocacoes.values().map(|b| b.0))
        .chain(plano.tokens.pendencias.values().map(|b| b.0))
        .chain(plano.escopos.saidas.keys().map(|b| b.0))
        .chain(plano.escopos.arestas.keys().flat_map(|(a, b)| [a.0, b.0]))
        .max()
        .map_or(0, |b| u64::from(b) + 1);
    let mut id_valor = || -> Result<ValueId, String> {
        let id = u32::try_from(valor).map_err(|_| "IDs SSA esgotados ao separar Guarda")?;
        valor += 1;
        Ok(ValueId(id))
    };
    let mut id_bloco = || -> Result<BlockId, String> {
        let id = u32::try_from(bloco).map_err(|_| "IDs de bloco esgotados ao separar Guarda")?;
        bloco += 1;
        Ok(BlockId(id))
    };
    // Só acrescentamos blocos: os índices originais permanecem válidos e
    // evitam uma busca linear por saída em funções com muitos retornos.
    for (indice, origem) in guardas {
        let b = &mut f.blocks[indice];
        let Terminator::Return(op) = &b.terminator else {
            return Err(format!("saída Guarda b{} não é Return", origem.0));
        };
        let retorno = op.clone();
        let pendente = id_valor()?;
        let cond = id_valor()?;
        let erro = id_bloco()?;
        let normal = id_bloco()?;
        b.instructions.extend([
            (
                pendente,
                Instruction::CallRuntime {
                    name: "dartforge_exception_pending".into(),
                    args: vec![],
                    ret_ty: Type::I8,
                },
                Type::I8,
            ),
            (
                cond,
                Instruction::ICmp(
                    ICmpOp::Ne,
                    Operand::Val(pendente),
                    Operand::Constant(Constant::Int(0)),
                ),
                Type::I1,
            ),
        ]);
        b.terminator = Terminator::CondBranch {
            cond: Operand::Val(cond),
            then_block: erro,
            else_block: normal,
        };
        f.blocks.extend([
            BasicBlock {
                id: erro,
                instructions: vec![],
                terminator: Terminator::Return(if f.return_ty == Type::Void {
                    None
                } else {
                    Some(super::super::operandos::constante_padrao(f.return_ty))
                }),
            },
            BasicBlock {
                id: normal,
                instructions: vec![],
                terminator: Terminator::Return(retorno),
            },
        ]);
        plano.tabelas.saidas.remove(&origem);
        plano.tabelas.saidas.insert(erro, SaidaPorExcecao::Lanca);
        // A retenção no retorno normal precisa ocorrer antes de fechar o
        // escopo do borrow. Cada saída fecha a mesma pilha léxica original.
        if let Some(eventos) = plano.escopos.saidas.remove(&origem) {
            plano.escopos.saidas.insert(erro, eventos.clone());
            plano.escopos.saidas.insert(normal, eventos);
        }
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::collections::HashMap;

    fn folha(owned: bool) -> (Vec<Function>, HashMap<String, PlanoFuncaoDart>) {
        let f = Function {
            symbol: "folha".into(),
            name: "folha".into(),
            depuracao: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: if owned {
                    vec![(
                        ValueId(1),
                        Instruction::ArcCopy {
                            value: Operand::Val(ValueId(0)),
                        },
                        Type::Ref,
                    )]
                } else {
                    vec![]
                },
                terminator: Terminator::Return(Some(Operand::Val(ValueId(if owned {
                    1
                } else {
                    0
                })))),
            }],
        };
        let mut p = PlanoFuncaoDart::default();
        p.tokens.retorno = RetornoTokens::Owned;
        p.tabelas.saidas.insert(BlockId(0), SaidaPorExcecao::Guarda);
        (vec![f], HashMap::from([("folha".into(), p)]))
    }

    #[test]
    fn guarda_transfere_owned_so_no_sucesso_e_retem_borrow_so_no_normal() {
        for owned in [false, true] {
            let (mut funcoes, mut planos) = folha(owned);
            let esperado = if owned { (0, 1) } else { (1, 0) };
            assert_eq!(
                inserir_arc_funcoes_dart(&mut funcoes, &mut planos).unwrap(),
                esperado
            );
            let f = &funcoes[0];
            let p = &planos["folha"];
            let (erro, normal) = match f.blocks[0].terminator {
                Terminator::CondBranch {
                    then_block,
                    else_block,
                    ..
                } => (then_block, else_block),
                _ => panic!("Guarda não separado"),
            };
            assert_eq!(p.tabelas.saidas[&erro], SaidaPorExcecao::Lanca);
            assert!(!p.tabelas.saidas.contains_key(&normal));
            let b_erro = f.blocks.iter().find(|b| b.id == erro).unwrap();
            assert!(
                !b_erro
                    .instructions
                    .iter()
                    .any(|(_, i, _)| matches!(i, Instruction::ArcCopy { .. }))
            );
            if owned {
                assert!(matches!(
                    b_erro.instructions[0].1,
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(1))
                    }
                ));
            } else {
                let b_normal = f.blocks.iter().find(|b| b.id == normal).unwrap();
                assert!(matches!(
                    b_normal.instructions[0].1,
                    Instruction::ArcCopy {
                        value: Operand::Val(ValueId(0))
                    }
                ));
            }
            let antes = format!("{funcoes:?}");
            assert_eq!(
                inserir_arc_funcoes_dart(&mut funcoes, &mut planos).unwrap(),
                (0, 0)
            );
            assert_eq!(format!("{funcoes:?}"), antes);
        }
    }

    #[test]
    fn guarda_transporta_fechamento_do_escopo_para_ambas_as_saidas() {
        let (mut funcoes, mut planos) = folha(false);
        let f = &mut funcoes[0];
        f.blocks[0].terminator = Terminator::Branch(BlockId(1));
        f.blocks.push(BasicBlock {
            id: BlockId(1),
            instructions: vec![(
                ValueId(1),
                Instruction::Phi {
                    ty: Type::Ref,
                    incoming: vec![(BlockId(0), Operand::Val(ValueId(0)))],
                },
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
        });
        let p = planos.get_mut("folha").unwrap();
        p.tabelas.saidas.clear();
        p.tabelas.saidas.insert(BlockId(1), SaidaPorExcecao::Guarda);
        p.classes.insert(
            ValueId(1),
            Ownership::Borrowed {
                owner: OrigemOwner::Valor(ValueId(0)),
                escopo: 7,
            },
        );
        p.escopos
            .antes
            .insert(ValueId(1), vec![AlteracaoEscopo::Abrir(7)]);
        p.escopos
            .saidas
            .insert(BlockId(1), vec![AlteracaoEscopo::Fechar(7)]);
        assert_eq!(
            inserir_arc_funcoes_dart(&mut funcoes, &mut planos).unwrap(),
            (1, 0)
        );
        let p = &planos["folha"];
        assert!(!p.escopos.saidas.contains_key(&BlockId(1)));
        if let Terminator::CondBranch {
            then_block,
            else_block,
            ..
        } = funcoes[0].blocks[1].terminator
        {
            for b in [then_block, else_block] {
                assert_eq!(p.escopos.saidas[&b], vec![AlteracaoEscopo::Fechar(7)]);
            }
        } else {
            panic!("Guarda não separado");
        }
    }

    #[test]
    fn multiplas_guardas_preservam_transferencia_e_rejeitam_falha_posterior() {
        for falha in 0..3 {
            let (mut funcoes, mut planos) = folha(true);
            let f = &mut funcoes[0];
            f.blocks[0].terminator = Terminator::CondBranch {
                cond: Operand::Constant(Constant::Bool(true)),
                then_block: BlockId(9),
                else_block: BlockId(4),
            };
            // IDs fora da ordem física não devem alterar o bloco escolhido.
            for id in [9, 4] {
                f.blocks.push(BasicBlock {
                    id: BlockId(id),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                });
            }
            let p = planos.get_mut("folha").unwrap();
            p.tabelas.saidas.clear();
            for id in [9, 4] {
                p.tabelas
                    .saidas
                    .insert(BlockId(id), SaidaPorExcecao::Guarda);
            }
            if falha == 1 {
                // A primeira saída já terá sido separada na cópia temporária.
                p.escopos
                    .saidas
                    .insert(BlockId(4), vec![AlteracaoEscopo::Fechar(7)]);
            } else if falha == 2 {
                // O primeiro Guarda usa os dois IDs restantes; o segundo
                // deve falhar sem publicar sequer a transformação inicial.
                funcoes[0].blocks[0].instructions.push((
                    ValueId(u32::MAX - 2),
                    Instruction::Const(Constant::Int(0)),
                    Type::I64,
                ));
            }
            let antes = format!("{funcoes:?}");
            let planos_antes = format!("{planos:?}");
            let resultado = inserir_arc_funcoes_dart(&mut funcoes, &mut planos);
            if falha != 0 {
                assert!(resultado.unwrap_err().contains(if falha == 1 {
                    "limite inválido"
                } else {
                    "IDs SSA esgotados"
                }));
                assert_eq!(format!("{funcoes:?}"), antes);
                assert_eq!(format!("{planos:?}"), planos_antes);
                continue;
            }
            assert_eq!(resultado.unwrap(), (0, 2));
            assert_eq!(funcoes[0].blocks.len(), 7);
            let p = &planos["folha"];
            assert_eq!(p.tabelas.saidas.len(), 2);
            for original in &funcoes[0].blocks[1..3] {
                let Terminator::CondBranch {
                    then_block,
                    else_block,
                    ..
                } = original.terminator
                else {
                    panic!("Guarda não separado");
                };
                assert_eq!(p.tabelas.saidas[&then_block], SaidaPorExcecao::Lanca);
                let erro = funcoes[0]
                    .blocks
                    .iter()
                    .find(|b| b.id == then_block)
                    .unwrap();
                let normal = funcoes[0]
                    .blocks
                    .iter()
                    .find(|b| b.id == else_block)
                    .unwrap();
                assert!(matches!(
                    erro.instructions[0].1,
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(1))
                    }
                ));
                assert!(normal.instructions.is_empty());
                assert!(matches!(
                    normal.terminator,
                    Terminator::Return(Some(Operand::Val(ValueId(1))))
                ));
            }
            assert_eq!(
                inserir_arc_funcoes_dart(&mut funcoes, &mut planos).unwrap(),
                (0, 0)
            );
        }
    }

    #[test]
    fn guarda_nao_publica_cfg_tabelas_ou_escopos_apos_falha() {
        for esgotado in [false, true] {
            let (mut funcoes, mut planos) = folha(false);
            if esgotado {
                funcoes[0].blocks[0].instructions.push((
                    ValueId(u32::MAX),
                    Instruction::Const(Constant::Int(0)),
                    Type::I64,
                ));
            } else {
                planos
                    .get_mut("folha")
                    .unwrap()
                    .escopos
                    .saidas
                    .insert(BlockId(0), vec![AlteracaoEscopo::Fechar(7)]);
            }
            let antes = format!("{funcoes:?}");
            let planos_antes = format!("{planos:?}");
            let erro = inserir_arc_funcoes_dart(&mut funcoes, &mut planos).unwrap_err();
            assert!(
                erro.contains(if esgotado {
                    "IDs SSA esgotados"
                } else {
                    "limite inválido"
                }),
                "{erro}"
            );
            assert_eq!(format!("{funcoes:?}"), antes);
            assert_eq!(format!("{planos:?}"), planos_antes);
        }
    }
}
