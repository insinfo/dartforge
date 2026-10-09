//! Geração de pousos locais para propagar falhas de chamadas Dart diretas.
//! Executa nas cópias da transação, preservando sítios já preparados.

use super::*;

pub(super) fn preparar(
    f: &mut Function,
    plano: &mut PlanoFuncaoDart,
    nao_lancam: &HashSet<String>,
) -> Result<(), String> {
    let ausente = |v: &ValueId, inst: &Instruction| {
        matches!(inst,
        Instruction::CallStatic { symbol, .. }
        if !nao_lancam.contains(symbol) && !plano.tabelas.invocacoes.contains_key(v))
    };
    if !f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|(v, i, _)| ausente(v, i))
    {
        return Ok(());
    }
    // Uma conferência existente pode selecionar catch/finally. Não a
    // substituímos por propagação sem os sítios fornecidos pelo lowering.
    if f.blocks.iter().any(|b| {
        b.instructions.windows(2).any(|par| {
            ausente(&par[0].0, &par[0].1)
                && matches!(&par[1].1, Instruction::CallRuntime { name, .. }
                if name == "dartforge_exception_pending")
        })
    }) {
        return Err(format!(
            "{}: conferência de pendência Dart exige sítio preparado",
            f.symbol
        ));
    }
    super::ssa::verificar(f)?;
    let pilhas = super::escopos::pilhas_nas_chamadas(f, &plano.escopos)?;
    let mut proximo = f
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
    let mut reservar = || -> Result<BlockId, String> {
        let id =
            u32::try_from(proximo).map_err(|_| "IDs de bloco esgotados ao preparar invoke Dart")?;
        proximo += 1;
        Ok(BlockId(id))
    };
    let mut finais = HashMap::new();
    let originais = f.blocks.len();
    for indice in 0..originais {
        let origem = f.blocks[indice].id;
        let instrucoes = std::mem::take(&mut f.blocks[indice].instructions);
        let terminador = f.blocks[indice].terminator.clone();
        let mut atual = indice;
        for (v, inst, ty) in instrucoes {
            let dividir = matches!(&inst, Instruction::CallStatic { symbol, .. }
                if !nao_lancam.contains(symbol) && !plano.tabelas.invocacoes.contains_key(&v));
            f.blocks[atual].instructions.push((v, inst, ty));
            if !dividir {
                continue;
            }
            let erro = reservar()?;
            let normal = reservar()?;
            f.blocks[atual].terminator = Terminator::CondBranch {
                cond: Operand::Constant(Constant::Bool(false)),
                then_block: erro,
                else_block: normal,
            };
            plano.tabelas.invocacoes.insert(v, erro);
            plano.tabelas.pousos.insert(erro);
            plano.tabelas.saidas.insert(erro, SaidaPorExcecao::Lanca);
            // O pouso libera os tokens antes de sair dos escopos ativos.
            // Chamadas inalcançáveis não têm pilha executável a fechar.
            let fechar: Vec<_> = pilhas
                .get(&v)
                .into_iter()
                .flatten()
                .rev()
                .filter(|e| **e != 0)
                .map(|e| AlteracaoEscopo::Fechar(*e))
                .collect();
            if !fechar.is_empty() {
                plano.escopos.saidas.insert(erro, fechar);
            }
            f.blocks.push(BasicBlock {
                id: erro,
                instructions: vec![],
                terminator: Terminator::Return(if f.return_ty == Type::Void {
                    None
                } else {
                    Some(super::super::operandos::constante_padrao(f.return_ty))
                }),
            });
            atual = f.blocks.len();
            f.blocks.push(BasicBlock {
                id: normal,
                instructions: vec![],
                terminator: terminador.clone(),
            });
        }
        f.blocks[atual].terminator = terminador;
        if atual != indice {
            finais.insert(origem, f.blocks[atual].id);
        }
    }
    // Só o último segmento conserva o terminador original. Atualizamos
    // Phis e eventos uma vez por função, sem buscas por chamada/sucessor.
    for b in &mut f.blocks {
        for (_, inst, _) in &mut b.instructions {
            if let Instruction::Phi { incoming, .. } = inst {
                for (de, _) in incoming {
                    if let Some(novo) = finais.get(de) {
                        *de = *novo;
                    }
                }
            }
        }
    }
    plano.escopos.arestas = std::mem::take(&mut plano.escopos.arestas)
        .into_iter()
        .map(|((de, para), eventos)| ((*finais.get(&de).unwrap_or(&de), para), eventos))
        .collect();
    for (origem, final_) in finais {
        if let Some(eventos) = plano.escopos.saidas.remove(&origem) {
            plano.escopos.saidas.insert(final_, eventos);
        }
        if let Some(saida) = plano.tabelas.saidas.remove(&origem) {
            plano.tabelas.saidas.insert(final_, saida);
        }
    }
    super::ssa::verificar(f)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn conjunto() -> (Vec<Function>, HashMap<String, PlanoFuncaoDart>) {
        let chamada = |v| {
            (
                ValueId(v),
                Instruction::CallStatic {
                    symbol: "folha".into(),
                    args: vec![Operand::Val(ValueId(v - 1))],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        };
        let caller = Function {
            symbol: "caller".into(),
            name: "caller".into(),
            depuracao: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::ArcCopy {
                            value: Operand::Val(ValueId(0)),
                        },
                        Type::Ref,
                    ),
                    chamada(2),
                    chamada(3),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
            }],
        };
        let folha = Function {
            symbol: "folha".into(),
            name: "folha".into(),
            depuracao: None,
            params: caller.params.clone(),
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        };
        let mut p = PlanoFuncaoDart::default();
        p.tokens.retorno = RetornoTokens::Owned;
        let mut q = p.clone();
        q.tabelas.confere_pilha = true;
        (
            vec![caller, folha],
            HashMap::from([("caller".into(), p), ("folha".into(), q)]),
        )
    }

    #[test]
    fn chamadas_consecutivas_tem_cleanup_exato_e_fecham_escopos() {
        let (mut funcoes, mut planos) = conjunto();
        let p = planos.get_mut("caller").unwrap();
        p.escopos
            .antes
            .insert(ValueId(2), vec![AlteracaoEscopo::Abrir(7)]);
        p.escopos
            .saidas
            .insert(BlockId(0), vec![AlteracaoEscopo::Fechar(7)]);
        assert_eq!(
            preparar_arc_funcoes_dart(&mut funcoes, &mut planos).unwrap(),
            (1, 5)
        );
        let p = &planos["caller"];
        for (v, esperado) in [(2, vec![1]), (3, vec![1, 2])] {
            let erro = p.tabelas.invocacoes[&ValueId(v)];
            assert!(p.tabelas.pousos.contains(&erro));
            assert_eq!(p.tabelas.saidas[&erro], SaidaPorExcecao::Lanca);
            assert_eq!(p.escopos.saidas[&erro], vec![AlteracaoEscopo::Fechar(7)]);
            let b = funcoes[0].blocks.iter().find(|b| b.id == erro).unwrap();
            let drops: Vec<_> = b
                .instructions
                .iter()
                .filter_map(|(_, i, _)| match i {
                    Instruction::ArcDrop {
                        value: Operand::Val(v),
                    } => Some(v.0),
                    _ => None,
                })
                .collect();
            assert_eq!(drops, esperado);
        }
        assert!(!p.escopos.saidas.contains_key(&BlockId(0)));
        let antes = format!("{funcoes:?}");
        assert_eq!(
            preparar_arc_funcoes_dart(&mut funcoes, &mut planos).unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{funcoes:?}"), antes);
    }

    #[test]
    fn phi_e_limite_na_aresta_seguem_a_continuacao_normal() {
        let (mut funcoes, mut planos) = conjunto();
        let f = &mut funcoes[0];
        f.blocks[0].instructions.pop();
        f.blocks[0].terminator = Terminator::Branch(BlockId(9));
        f.blocks.push(BasicBlock {
            id: BlockId(9),
            instructions: vec![(
                ValueId(3),
                Instruction::Phi {
                    ty: Type::Ref,
                    incoming: vec![(BlockId(0), Operand::Val(ValueId(2)))],
                },
                Type::Ref,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
        });
        let p = planos.get_mut("caller").unwrap();
        p.escopos
            .antes
            .insert(ValueId(2), vec![AlteracaoEscopo::Abrir(7)]);
        p.escopos
            .arestas
            .insert((BlockId(0), BlockId(9)), vec![AlteracaoEscopo::Fechar(7)]);
        assert_eq!(
            preparar_arc_funcoes_dart(&mut funcoes, &mut planos).unwrap(),
            (1, 2)
        );
        let Instruction::Phi { incoming, .. } = &funcoes[0].blocks[1].instructions[0].1 else {
            panic!("Phi removido");
        };
        let de = incoming[0].0;
        assert_ne!(de, BlockId(0));
        assert_eq!(
            planos["caller"].escopos.arestas[&(de, BlockId(9))],
            vec![AlteracaoEscopo::Fechar(7)]
        );
        assert!(
            !planos["caller"]
                .escopos
                .arestas
                .contains_key(&(BlockId(0), BlockId(9)))
        );
    }

    #[test]
    fn tratador_existente_e_preservado_ao_preparar_chamada_posterior() {
        let (mut funcoes, mut planos) = conjunto();
        let f = &mut funcoes[0];
        let segunda = f.blocks[0].instructions.pop().unwrap();
        f.blocks[0].terminator = Terminator::CondBranch {
            cond: Operand::Constant(Constant::Bool(false)),
            then_block: BlockId(7),
            else_block: BlockId(4),
        };
        f.blocks.push(BasicBlock {
            id: BlockId(4),
            instructions: vec![segunda],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
        });
        f.blocks.push(BasicBlock {
            id: BlockId(7),
            instructions: vec![(
                ValueId(4),
                Instruction::CallRuntime {
                    name: "dartforge_exception_clear".into(),
                    args: vec![],
                    ret_ty: Type::Void,
                },
                Type::Void,
            )],
            terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
        });
        let p = planos.get_mut("caller").unwrap();
        p.tabelas.invocacoes.insert(ValueId(2), BlockId(7));
        p.tabelas.pousos.insert(BlockId(7));
        assert_eq!(
            preparar_arc_funcoes_dart(&mut funcoes, &mut planos).unwrap(),
            (1, 5)
        );
        let p = &planos["caller"];
        assert_eq!(p.tabelas.invocacoes[&ValueId(2)], BlockId(7));
        assert!(!p.tabelas.saidas.contains_key(&BlockId(7)));
        assert_eq!(
            p.tabelas.saidas[&p.tabelas.invocacoes[&ValueId(3)]],
            SaidaPorExcecao::Lanca
        );
    }

    #[test]
    fn falha_de_contrato_ou_ids_preserva_toda_a_transacao() {
        for esgotado in [false, true] {
            let (mut funcoes, mut planos) = conjunto();
            if esgotado {
                funcoes[0].blocks.push(BasicBlock {
                    id: BlockId(u32::MAX - 2),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
                });
            } else if let Instruction::CallStatic { symbol, .. } =
                &mut funcoes[0].blocks[0].instructions[2].1
            {
                *symbol = "ausente".into();
            }
            let antes = format!("{funcoes:?}");
            let planos_antes = format!("{planos:?}");
            let erro = preparar_arc_funcoes_dart(&mut funcoes, &mut planos).unwrap_err();
            assert!(
                erro.contains(if esgotado {
                    "IDs de bloco esgotados"
                } else {
                    "ausente"
                }),
                "{erro}"
            );
            assert_eq!(format!("{funcoes:?}"), antes);
            assert_eq!(format!("{planos:?}"), planos_antes);
        }
    }

    #[test]
    fn conferencia_existente_nao_e_substituida_por_propagacao() {
        let (mut funcoes, mut planos) = conjunto();
        funcoes[0].blocks[0].instructions.pop();
        funcoes[0].blocks[0].instructions.push((
            ValueId(3),
            Instruction::CallRuntime {
                name: "dartforge_exception_pending".into(),
                args: vec![],
                ret_ty: Type::I8,
            },
            Type::I8,
        ));
        funcoes[0].blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(2))));
        let antes = format!("{funcoes:?}");
        let planos_antes = format!("{planos:?}");
        assert!(
            preparar_arc_funcoes_dart(&mut funcoes, &mut planos)
                .unwrap_err()
                .contains("conferência de pendência Dart exige sítio preparado")
        );
        assert_eq!(format!("{funcoes:?}"), antes);
        assert_eq!(format!("{planos:?}"), planos_antes);
    }
}
