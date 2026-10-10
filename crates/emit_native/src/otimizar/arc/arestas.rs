//! Cleanup de tokens mortos nas arestas normais da transação ARC.
//! Preserva transferências Phi e dependências de borrows. Pousos e arestas
//! excepcionais exigem o protocolo de cleanup já preparado pelo lowering.
//! Extensões de vida semânticas devem estar materializadas como usos na HIR;
//! este passe não deduz ausência de Finalizable, captura ou suspensão.

use super::*;

pub(super) fn proximo_valor(f: &Function, p: &PlanoFuncaoDart) -> u64 {
    f.params
        .iter()
        .map(|(v, _, _)| v.0)
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| b.instructions.iter().map(|(v, _, _)| v.0)),
        )
        .chain(p.classes.keys().map(|v| v.0))
        .chain(p.tokens.instrucoes.keys().map(|v| v.0))
        .chain(p.tokens.pendencias.keys().map(|v| v.0))
        .chain(p.tabelas.invocacoes.keys().map(|v| v.0))
        .chain(p.escopos.antes.keys().map(|v| v.0))
        .max()
        .map_or(0, |v| u64::from(v) + 1)
}

// Opera apenas sobre corpos/mapas privados do conjunto fechado.
pub(super) fn preparar(
    f: &mut Function,
    p: &mut PlanoFuncaoDart,
) -> Result<(usize, usize), String> {
    let mut proximo = proximo_valor(f, p);
    let mut id = || {
        let v = u32::try_from(proximo).map_err(|_| "IDs SSA esgotados ao preparar arestas ARC")?;
        proximo += 1;
        Ok(ValueId(v))
    };
    // A análise linear precisa ver a transferência Owned do retorno antes
    // de decidir o que fica morto nas arestas anteriores.
    let mut copias = super::contratos::copiar_retornos(f, &p.classes, &mut id)?;
    super::produzir_contratos_arc(f, &mut p.classes, &mut p.tokens)?;
    let mut drops = 0;
    loop {
        let planos = super::tokens::arestas_para_cleanup(f, &p.classes, &p.tabelas, &p.tokens)?;
        if planos.is_empty() {
            break;
        }
        let mut planos: Vec<_> = planos.into_iter().collect();
        planos.sort_unstable_by_key(|((a, b), _)| (a.0, b.0));
        let mut valor = proximo_valor(f, p);
        let mut bloco = f
            .blocks
            .iter()
            .map(|b| b.id.0)
            .chain(p.tabelas.saidas.keys().map(|b| b.0))
            .chain(p.tabelas.pousos.iter().map(|b| b.0))
            .chain(p.tabelas.invocacoes.values().map(|b| b.0))
            .chain(p.tokens.pendencias.values().map(|b| b.0))
            .chain(p.escopos.saidas.keys().map(|b| b.0))
            .chain(p.escopos.arestas.keys().flat_map(|(a, b)| [a.0, b.0]))
            .max()
            .map_or(0, |b| u64::from(b) + 1);
        for ((origem, destino), mortos) in planos {
            let novo = BlockId(
                u32::try_from(bloco)
                    .map_err(|_| "IDs de bloco esgotados ao preparar arestas ARC")?,
            );
            bloco += 1;
            let mut instructions = Vec::new();
            for morto in mortos {
                let v = ValueId(
                    u32::try_from(valor)
                        .map_err(|_| "IDs SSA esgotados ao preparar arestas ARC")?,
                );
                valor += 1;
                instructions.push((
                    v,
                    Instruction::ArcDrop {
                        value: Operand::Val(morto),
                    },
                    Type::Void,
                ));
                drops += 1;
            }
            let b = f.blocks.iter_mut().find(|b| b.id == origem).unwrap();
            match &mut b.terminator {
                Terminator::Branch(alvo) => {
                    if *alvo == destino {
                        *alvo = novo;
                    }
                }
                Terminator::CondBranch {
                    then_block,
                    else_block,
                    ..
                } => {
                    for alvo in [then_block, else_block] {
                        if *alvo == destino {
                            *alvo = novo;
                        }
                    }
                }
                Terminator::Switch { default, cases, .. } => {
                    if *default == destino {
                        *default = novo;
                    }
                    for (_, alvo) in cases {
                        if *alvo == destino {
                            *alvo = novo;
                        }
                    }
                }
                _ => return Err("aresta de cleanup sem desvio correspondente".into()),
            }
            for b in &mut f.blocks {
                if b.id != destino {
                    continue;
                }
                for (_, inst, _) in &mut b.instructions {
                    if let Instruction::Phi { incoming, .. } = inst {
                        for (de, _) in incoming {
                            if *de == origem {
                                *de = novo;
                            }
                        }
                    }
                }
            }
            if let Some(limites) = p.escopos.arestas.remove(&(origem, destino)) {
                p.escopos.arestas.insert((novo, destino), limites);
            }
            f.blocks.push(BasicBlock {
                id: novo,
                instructions,
                terminator: Terminator::Branch(destino),
            });
        }
        super::produzir_contratos_arc(f, &mut p.classes, &mut p.tokens)?;
        // Release é uma barreira conservadora; novos keepalives precisam
        // entrar no próximo inventário antes de planejar mais drops.
        copias += super::emprestimos::proteger(f, p)?;
    }
    Ok((copias, drops))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn aresta_critica_fecha_owner_morto_e_transporta_limite_sem_alterar_tracing() {
        for caso in 0..4 {
            let arc = caso != 0;
            let mut m = Module::new();
            m.memoria_arc = arc;
            m.functions.push(Function {
                symbol: "critica".into(),
                name: "critica".into(),
                depuracao: None,
                params: vec![
                    (ValueId(0), "x".into(), Type::Ref),
                    (ValueId(1), "cond".into(), Type::I1),
                ],
                return_ty: Type::Void,
                blocks: vec![
                    BasicBlock {
                        id: BlockId(0),
                        instructions: vec![],
                        terminator: Terminator::CondBranch {
                            cond: Operand::Val(ValueId(1)),
                            then_block: BlockId(1),
                            else_block: BlockId(2),
                        },
                    },
                    BasicBlock {
                        id: BlockId(1),
                        instructions: vec![(
                            ValueId(2),
                            Instruction::ArcCopy {
                                value: Operand::Val(ValueId(0)),
                            },
                            Type::Ref,
                        )],
                        terminator: Terminator::CondBranch {
                            cond: Operand::Val(ValueId(1)),
                            then_block: BlockId(3),
                            else_block: BlockId(4),
                        },
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
                    BasicBlock {
                        id: BlockId(4),
                        instructions: vec![(
                            ValueId(3),
                            Instruction::ArcDrop {
                                value: Operand::Val(ValueId(2)),
                            },
                            Type::Void,
                        )],
                        terminator: Terminator::Branch(BlockId(3)),
                    },
                ],
            });
            if caso == 2 {
                m.functions[0].blocks[2].instructions.push((
                    ValueId(u32::MAX),
                    Instruction::Const(Constant::Int(0)),
                    Type::I64,
                ));
            } else if caso == 3 {
                m.functions[0].blocks.push(BasicBlock {
                    id: BlockId(u32::MAX),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                });
            }
            let mut p = PlanoFuncaoDart::default();
            p.escopos
                .antes
                .insert(ValueId(2), vec![AlteracaoEscopo::Abrir(7)]);
            for b in [3, 4] {
                p.escopos
                    .arestas
                    .insert((BlockId(1), BlockId(b)), vec![AlteracaoEscopo::Fechar(7)]);
            }
            let mut planos = HashMap::from([("critica".into(), p)]);
            let antes = format!("{m:?}/{planos:?}");
            if caso >= 2 {
                assert!(
                    preparar_arc_modulo_dart(&mut m, &mut planos)
                        .unwrap_err()
                        .contains("IDs")
                );
                assert_eq!(format!("{m:?}/{planos:?}"), antes);
                continue;
            }
            assert_eq!(
                preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                (0, usize::from(arc))
            );
            if !arc {
                assert_eq!(format!("{m:?}/{planos:?}"), antes);
                continue;
            }
            let novo = m.functions[0].blocks.last().unwrap();
            assert_eq!(novo.id, BlockId(5));
            assert!(matches!(
                novo.instructions[0].1,
                Instruction::ArcDrop {
                    value: Operand::Val(ValueId(2))
                }
            ));
            assert_eq!(
                planos["critica"].escopos.arestas[&(BlockId(5), BlockId(3))],
                vec![AlteracaoEscopo::Fechar(7)]
            );
            assert!(
                !planos["critica"]
                    .escopos
                    .arestas
                    .contains_key(&(BlockId(1), BlockId(3)))
            );
            assert!(matches!(
                m.functions[0].blocks[1].terminator,
                Terminator::CondBranch {
                    then_block: BlockId(5),
                    else_block: BlockId(4),
                    ..
                }
            ));
            let preparado = format!("{m:?}/{planos:?}");
            assert_eq!(
                preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                (0, 0)
            );
            assert_eq!(format!("{m:?}/{planos:?}"), preparado);
            crate::llvm::LlvmEmitter::new(&m).emit_all();
        }
    }

    #[test]
    fn phi_emprestado_mantem_owner_ate_copiar_o_retorno() {
        let mut m = Module::new();
        m.memoria_arc = true;
        let leitura = |v| {
            (
                ValueId(v),
                Instruction::CallRuntime {
                    name: "dartforge_arc_ler_campo_ref_v1".into(),
                    args: vec![
                        (Operand::Val(ValueId(2)), Type::Ref),
                        (Operand::Constant(Constant::Int(0)), Type::I64),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        };
        m.functions.push(Function {
            symbol: "phi_borrow_vivo".into(),
            name: "phi_borrow_vivo".into(),
            depuracao: None,
            params: vec![
                (ValueId(0), "objeto".into(), Type::Ref),
                (ValueId(1), "cond".into(), Type::I1),
            ],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::ArcCopy {
                            value: Operand::Val(ValueId(0)),
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(1)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![leitura(3)],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![leitura(4)],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(3),
                    instructions: vec![(
                        ValueId(5),
                        Instruction::Phi {
                            incoming: vec![
                                (BlockId(1), Operand::Val(ValueId(3))),
                                (BlockId(2), Operand::Val(ValueId(4))),
                            ],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(5)))),
                },
            ],
        });
        let mut p = PlanoFuncaoDart::default();
        p.tokens.retorno = RetornoTokens::Owned;
        let mut planos = HashMap::from([("phi_borrow_vivo".into(), p)]);
        assert_eq!(
            preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
            (1, 1)
        );
        assert_eq!(m.functions[0].blocks.len(), 4);
        let ops = &m.functions[0].blocks[3].instructions;
        assert!(matches!(
            ops[1].1,
            Instruction::ArcCopy {
                value: Operand::Val(ValueId(5))
            }
        ));
        assert!(matches!(
            ops[2].1,
            Instruction::ArcDrop {
                value: Operand::Val(ValueId(2))
            }
        ));
        assert!(matches!(
            planos["phi_borrow_vivo"].classes[&ValueId(5)],
            Ownership::Borrowed {
                owner: OrigemOwner::Valor(ValueId(2)),
                ..
            }
        ));
    }

    #[test]
    fn alocacao_em_laco_libera_token_na_volta_e_na_saida() {
        let mut m = Module::new();
        m.memoria_arc = true;
        m.functions.push(Function {
            symbol: "laco_owned".into(),
            name: "laco_owned".into(),
            depuracao: None,
            params: vec![(ValueId(0), "repetir".into(), Type::I1)],
            return_ty: Type::Void,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::CallRuntime {
                            name: "dartforge_arc_box_int_owned_v1".into(),
                            args: vec![(Operand::Constant(Constant::Int(i64::MAX)), Type::I64)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(0)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                },
            ],
        });
        let mut planos = HashMap::from([("laco_owned".into(), PlanoFuncaoDart::default())]);
        assert_eq!(
            preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
            (0, 2)
        );
        for b in &m.functions[0].blocks[3..] {
            assert!(matches!(
                b.instructions[0].1,
                Instruction::ArcDrop {
                    value: Operand::Val(ValueId(1))
                }
            ));
        }
        let antes = format!("{m:?}/{planos:?}");
        assert_eq!(
            preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{m:?}/{planos:?}"), antes);
        crate::llvm::LlvmEmitter::new(&m).emit_all();
    }
}
