//! Casos de consumo linear, junções, backedges e resultados excepcionais.
use super::*;

fn valor(v: u32) -> Operand {
    Operand::Val(ValueId(v))
}
fn bloco(
    b: u32,
    instructions: Vec<(ValueId, Instruction, Type)>,
    terminator: Terminator,
) -> BasicBlock {
    BasicBlock {
        id: BlockId(b),
        instructions,
        terminator,
    }
}
fn funcao(blocks: Vec<BasicBlock>) -> Function {
    Function {
        symbol: "tokens".into(),
        name: "tokens".into(),
        depuracao: None,
        params: vec![
            (ValueId(0), "x".into(), Type::Ref),
            (ValueId(9), "cond".into(), Type::I1),
        ],
        return_ty: Type::Void,
        blocks,
    }
}
fn classes(owned: &[u32], trivial: &[u32]) -> HashMap<ValueId, Ownership> {
    let mut c = HashMap::from([
        (
            ValueId(0),
            Ownership::Borrowed {
                owner: OrigemOwner::Chamador,
                escopo: 0,
            },
        ),
        (ValueId(9), Ownership::Trivial),
    ]);
    c.extend(owned.iter().map(|&v| (ValueId(v), Ownership::Owned)));
    c.extend(trivial.iter().map(|&v| (ValueId(v), Ownership::Trivial)));
    c
}
fn copia(v: u32, origem: u32) -> (ValueId, Instruction, Type) {
    (
        ValueId(v),
        Instruction::ArcCopy {
            value: valor(origem),
        },
        Type::Ref,
    )
}
fn movimento(v: u32, origem: u32) -> (ValueId, Instruction, Type) {
    (
        ValueId(v),
        Instruction::ArcMove {
            value: valor(origem),
        },
        Type::Ref,
    )
}
fn drop(v: u32, origem: u32) -> (ValueId, Instruction, Type) {
    (
        ValueId(v),
        Instruction::ArcDrop {
            value: valor(origem),
        },
        Type::Void,
    )
}
fn verificar(f: &Function, c: &HashMap<ValueId, Ownership>) -> Result<(), String> {
    verificar_tokens(f, c, &TabelasDaFuncao::default(), &PlanoTokens::default())
}

#[test]
fn extern_auditada_nao_aceita_plano_que_omite_consumo() {
    let chamada = Instruction::CallRuntime {
        name: "dartforge_arc_release".into(),
        args: vec![(valor(1), Type::Ref)],
        ret_ty: Type::Void,
    };
    let contrato = super::super::contrato_chamada_runtime(&chamada).unwrap();
    let f = funcao(vec![bloco(
        0,
        vec![copia(1, 0), (ValueId(2), chamada, Type::Void)],
        Terminator::Return(None),
    )]);
    let c = classes(&[1], &[2]);
    let mut plano = PlanoTokens::default();
    plano.instrucoes.insert(ValueId(2), contrato.efeito);
    assert!(verificar_tokens(&f, &c, &TabelasDaFuncao::default(), &plano).is_ok());
    plano.instrucoes.insert(ValueId(2), EfeitoTokens::default());
    let erro = verificar_tokens(&f, &c, &TabelasDaFuncao::default(), &plano).unwrap_err();
    assert!(erro.contains("ownership.tsv"));
}

#[test]
fn produtor_runtime_e_invoke_auditado_exigem_saida_de_erro() {
    let f = funcao(vec![
        bloco(
            0,
            vec![(
                ValueId(1),
                Instruction::CallRuntime {
                    name: "dartforge_gc_collect".into(),
                    args: vec![],
                    ret_ty: Type::Void,
                },
                Type::Void,
            )],
            Terminator::CondBranch {
                cond: Operand::Constant(Constant::Bool(false)),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        bloco(1, vec![], Terminator::Return(None)),
        bloco(2, vec![], Terminator::Return(None)),
    ]);
    let mut c = classes(&[], &[]);
    let mut p = PlanoTokens::default();
    super::super::produzir_contratos_runtime(&f, &mut c, &mut p).unwrap();
    let mut t = TabelasDaFuncao::default();
    t.invocacoes.insert(ValueId(1), BlockId(1));
    t.pousos.insert(BlockId(1));
    verificar_tokens(&f, &c, &t, &p).unwrap();
    assert!(verificar_tokens(&f, &c, &TabelasDaFuncao::default(), &p).is_err());
    p.instrucoes.get_mut(&ValueId(1)).unwrap().pode_falhar = false;
    assert!(
        verificar_tokens(&f, &c, &t, &p)
            .unwrap_err()
            .contains("ownership.tsv")
    );
}

#[test]
fn produtor_arc_classifica_operacoes_fixas_e_nao_deixa_estado_parcial() {
    let slot = SlotForte::Quadro {
        quadro: valor(9),
        indice: 0,
    };
    let mut f = funcao(vec![bloco(
        0,
        vec![
            copia(1, 0),
            (
                ValueId(2),
                Instruction::ArcStoreStrong {
                    slot: slot.clone(),
                    value: valor(1),
                    modo: ModoStoreForte::Move,
                },
                Type::Void,
            ),
            (ValueId(3), Instruction::ArcLoadStrong { slot }, Type::Ref),
            movimento(4, 3),
            drop(5, 4),
            (
                ValueId(6),
                Instruction::CallRuntime {
                    name: "dartforge_arc_quadro_fechar_v1".into(),
                    args: vec![(valor(9), Type::I64)],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ),
        ],
        Terminator::Return(None),
    )]);
    f.params[1].2 = Type::I64;
    let mut c = classes(&[], &[]);
    let inicial = c.clone();
    let mut p = PlanoTokens::default();
    f.blocks[0].instructions[4].2 = Type::I64;
    assert!(super::super::produzir_contratos_arc(&f, &mut c, &mut p).is_err());
    assert_eq!(c, inicial);
    assert!(p.instrucoes.is_empty());
    f.blocks[0].instructions[4].2 = Type::Void;
    super::super::produzir_contratos_arc(&f, &mut c, &mut p).unwrap();
    assert_eq!(c[&ValueId(3)], Ownership::Owned);
    assert_eq!(c[&ValueId(2)], Ownership::Trivial);
    assert_eq!(p.instrucoes.len(), 1);
    verificar_tokens(&f, &c, &TabelasDaFuncao::default(), &p).unwrap();
    p.instrucoes.insert(ValueId(1), EfeitoTokens::default());
    let antes = c.clone();
    assert!(super::super::produzir_contratos_arc(&f, &mut c, &mut p).is_err());
    assert_eq!(c, antes);
}

#[test]
fn copia_movimento_drop_e_consumo_duplo() {
    let mut f = funcao(vec![bloco(
        0,
        vec![copia(1, 0), movimento(2, 1), drop(3, 2)],
        Terminator::Return(None),
    )]);
    let mut c = classes(&[1, 2], &[3]);
    verificar(&f, &c).unwrap();
    f.blocks[0].instructions.push(drop(4, 2));
    c.insert(ValueId(4), Ownership::Trivial);
    assert!(verificar(&f, &c).unwrap_err().contains("indisponível v2"));
    f.blocks[0].instructions.pop();
    c.remove(&ValueId(4));
    f.blocks[0].instructions[2] = drop(3, 1);
    assert!(verificar(&f, &c).unwrap_err().contains("indisponível v1"));
    f.blocks[0].instructions[1] = movimento(2, 0);
    assert!(verificar(&f, &c).unwrap_err().contains("exige Owned"));
}

#[test]
fn saida_com_token_e_juncao_com_consumo_condicional_falham() {
    let f = funcao(vec![bloco(0, vec![copia(1, 0)], Terminator::Return(None))]);
    assert!(
        verificar(&f, &classes(&[1], &[]))
            .unwrap_err()
            .contains("não consumidos")
    );
    let f = funcao(vec![
        bloco(
            0,
            vec![copia(1, 0)],
            Terminator::CondBranch {
                cond: valor(9),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        bloco(1, vec![drop(3, 1)], Terminator::Branch(BlockId(3))),
        bloco(2, vec![], Terminator::Branch(BlockId(3))),
        bloco(3, vec![], Terminator::Return(None)),
    ]);
    let e = verificar(&f, &classes(&[1], &[3])).unwrap_err();
    assert!(
        e.contains("junção b3")
            && e.contains("caminho anterior [0, 1, 3]")
            && e.contains("caminho [0, 2]"),
        "{e}"
    );
}

#[test]
fn phi_nullable_transfere_um_token_em_cada_aresta() {
    let f = funcao(vec![
        bloco(
            0,
            vec![],
            Terminator::CondBranch {
                cond: valor(9),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        bloco(1, vec![copia(1, 0)], Terminator::Branch(BlockId(3))),
        bloco(2, vec![], Terminator::Branch(BlockId(3))),
        bloco(
            3,
            vec![
                (
                    ValueId(2),
                    Instruction::Phi {
                        incoming: vec![
                            (BlockId(1), valor(1)),
                            (BlockId(2), Operand::Constant(Constant::Null)),
                        ],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                drop(3, 2),
            ],
            Terminator::Return(None),
        ),
    ]);
    let mut c = classes(&[], &[]);
    let mut p = PlanoTokens::default();
    super::super::produzir_contratos_arc(&f, &mut c, &mut p).unwrap();
    verificar_tokens(&f, &c, &TabelasDaFuncao::default(), &p).unwrap();
}

#[test]
fn phi_em_loop_transfere_sem_acumular_e_troca_simultaneamente() {
    let f = funcao(vec![
        bloco(0, vec![copia(1, 0)], Terminator::Branch(BlockId(1))),
        bloco(
            1,
            vec![
                (
                    ValueId(2),
                    Instruction::Phi {
                        incoming: vec![(BlockId(0), valor(1)), (BlockId(1), valor(3))],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                movimento(3, 2),
            ],
            Terminator::CondBranch {
                cond: valor(9),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        bloco(2, vec![drop(4, 3)], Terminator::Return(None)),
    ]);
    let mut c = classes(&[], &[]);
    let mut p = PlanoTokens::default();
    super::super::produzir_contratos_arc(&f, &mut c, &mut p).unwrap();
    verificar_tokens(&f, &c, &TabelasDaFuncao::default(), &p).unwrap();
    let f = funcao(vec![
        bloco(
            0,
            vec![copia(1, 0), copia(2, 0)],
            Terminator::Branch(BlockId(1)),
        ),
        bloco(
            1,
            vec![
                (
                    ValueId(3),
                    Instruction::Phi {
                        incoming: vec![(BlockId(0), valor(1)), (BlockId(1), valor(4))],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(4),
                    Instruction::Phi {
                        incoming: vec![(BlockId(0), valor(2)), (BlockId(1), valor(3))],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
            ],
            Terminator::CondBranch {
                cond: valor(9),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        bloco(2, vec![drop(5, 3), drop(6, 4)], Terminator::Return(None)),
    ]);
    let mut c = classes(&[], &[]);
    let mut p = PlanoTokens::default();
    super::super::produzir_contratos_arc(&f, &mut c, &mut p).unwrap();
    verificar_tokens(&f, &c, &TabelasDaFuncao::default(), &p).unwrap();
}

#[test]
fn produtor_phi_recusa_emprestimo_e_ciclo_sem_origem_sem_alterar_mapas() {
    let phi = |incoming| {
        (
            ValueId(2),
            Instruction::Phi {
                incoming,
                ty: Type::Ref,
            },
            Type::Ref,
        )
    };
    let f = funcao(vec![
        bloco(
            0,
            vec![],
            Terminator::CondBranch {
                cond: valor(9),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        bloco(1, vec![copia(1, 0)], Terminator::Branch(BlockId(3))),
        bloco(2, vec![], Terminator::Branch(BlockId(3))),
        bloco(
            3,
            vec![
                phi(vec![(BlockId(1), valor(1)), (BlockId(2), valor(0))]),
                drop(3, 2),
            ],
            Terminator::Return(None),
        ),
    ]);
    let mut c = classes(&[], &[]);
    let inicial = c.clone();
    let mut p = PlanoTokens::default();
    assert!(
        super::super::produzir_contratos_arc(&f, &mut c, &mut p)
            .unwrap_err()
            .contains("entrada não é owned/null")
    );
    assert_eq!(c, inicial);
    assert!(p.instrucoes.is_empty());
    let f = funcao(vec![bloco(
        0,
        vec![phi(vec![(BlockId(0), valor(3))]), movimento(3, 2)],
        Terminator::Branch(BlockId(0)),
    )]);
    assert!(
        super::super::produzir_contratos_arc(&f, &mut c, &mut p)
            .unwrap_err()
            .contains("sem origem owned/null")
    );
    assert_eq!(c, inicial);
    assert!(p.instrucoes.is_empty());
}

#[test]
fn emprestimo_nao_sobrevive_ao_owner_consumido_nem_escapa_no_retorno() {
    let mut f = funcao(vec![bloco(
        0,
        vec![
            copia(1, 0),
            (
                ValueId(2),
                Instruction::EnvGet {
                    env: valor(1),
                    index: 0,
                },
                Type::Ref,
            ),
            drop(3, 1),
            copia(4, 2),
            drop(5, 4),
        ],
        Terminator::Return(None),
    )]);
    let mut c = classes(&[1, 4], &[3, 5]);
    c.insert(
        ValueId(2),
        Ownership::Borrowed {
            owner: OrigemOwner::Valor(ValueId(1)),
            escopo: 0,
        },
    );
    let mut p = PlanoTokens::default();
    p.instrucoes.insert(ValueId(2), EfeitoTokens::default());
    assert!(
        verificar_tokens(&f, &c, &TabelasDaFuncao::default(), &p)
            .unwrap_err()
            .contains("indisponível v1")
    );
    f.blocks[0].instructions.truncate(2);
    f.blocks[0].terminator = Terminator::Return(Some(valor(2)));
    f.return_ty = Type::Ref;
    c.remove(&ValueId(3));
    c.remove(&ValueId(4));
    c.remove(&ValueId(5));
    p.retorno = RetornoTokens::Borrowed;
    assert!(
        verificar_tokens(&f, &c, &TabelasDaFuncao::default(), &p)
            .unwrap_err()
            .contains("retorno não satisfaz")
    );
}

#[test]
fn invoke_consume_argumento_nas_duas_saidas_e_so_produz_no_sucesso() {
    let mut f = funcao(vec![
        bloco(
            0,
            vec![
                copia(1, 0),
                (
                    ValueId(2),
                    Instruction::CallStatic {
                        symbol: "consumir".into(),
                        args: vec![valor(1)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
            ],
            Terminator::CondBranch {
                cond: Operand::Constant(Constant::Bool(false)),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        bloco(
            1,
            vec![],
            Terminator::Return(Some(Operand::Constant(Constant::Null))),
        ),
        bloco(2, vec![], Terminator::Return(Some(valor(2)))),
    ]);
    f.return_ty = Type::Ref;
    let c = classes(&[1, 2], &[]);
    let mut t = TabelasDaFuncao::default();
    t.invocacoes.insert(ValueId(2), BlockId(1));
    t.pousos.insert(BlockId(1));
    let mut p = PlanoTokens {
        retorno: RetornoTokens::Owned,
        ..Default::default()
    };
    p.instrucoes.insert(
        ValueId(2),
        EfeitoTokens {
            sucesso: vec![ValueId(1)],
            erro: vec![ValueId(1)],
            pode_falhar: true,
            ..Default::default()
        },
    );
    verificar_tokens(&f, &c, &t, &p).unwrap();
    p.instrucoes.get_mut(&ValueId(2)).unwrap().erro.clear();
    assert!(
        verificar_tokens(&f, &c, &t, &p)
            .unwrap_err()
            .contains("não consumidos")
    );
    p.instrucoes.get_mut(&ValueId(2)).unwrap().pode_falhar = false;
    assert!(
        verificar_tokens(&f, &c, &t, &p)
            .unwrap_err()
            .contains("saídas excepcionais")
    );
}

#[test]
fn carga_forte_cria_token_e_store_move_consumo_unico() {
    let slot = SlotForte::Quadro {
        quadro: valor(9),
        indice: 0,
    };
    let mut f = funcao(vec![bloco(
        0,
        vec![
            (
                ValueId(1),
                Instruction::ArcStoreStrong {
                    slot: slot.clone(),
                    value: valor(0),
                    modo: ModoStoreForte::Copy,
                },
                Type::Void,
            ),
            (
                ValueId(2),
                Instruction::ArcLoadStrong { slot: slot.clone() },
                Type::Ref,
            ),
            (
                ValueId(3),
                Instruction::ArcStoreStrong {
                    slot,
                    value: valor(2),
                    modo: ModoStoreForte::Move,
                },
                Type::Void,
            ),
        ],
        Terminator::Return(None),
    )]);
    f.params[1].2 = Type::I64;
    let mut c = classes(&[2], &[1, 3]);
    verificar(&f, &c).unwrap();
    f.blocks[0].instructions.push(drop(4, 2));
    c.insert(ValueId(4), Ownership::Trivial);
    assert!(verificar(&f, &c).unwrap_err().contains("indisponível"));
    f.blocks[0].instructions.pop();
    c.remove(&ValueId(4));
    if let Instruction::ArcStoreStrong { modo, .. } = &mut f.blocks[0].instructions[2].1 {
        *modo = ModoStoreForte::Copy;
    }
    assert!(verificar(&f, &c).unwrap_err().contains("não consumidos"));
}

#[test]
fn contratos_ausentes_obsoletos_e_phi_com_consumo_duplicado_falham() {
    let f = funcao(vec![bloco(
        0,
        vec![(ValueId(1), Instruction::Const(Constant::Int(7)), Type::I64)],
        Terminator::Return(None),
    )]);
    let c = classes(&[], &[1]);
    assert!(verificar(&f, &c).unwrap_err().contains("sem contrato"));
    let mut p = PlanoTokens::default();
    p.instrucoes.insert(ValueId(1), EfeitoTokens::default());
    verificar_tokens(&f, &c, &TabelasDaFuncao::default(), &p).unwrap();
    p.instrucoes.insert(ValueId(99), EfeitoTokens::default());
    assert!(
        verificar_tokens(&f, &c, &TabelasDaFuncao::default(), &p)
            .unwrap_err()
            .contains("obsoleto")
    );
    let f = funcao(vec![
        bloco(0, vec![copia(1, 0)], Terminator::Branch(BlockId(1))),
        bloco(
            1,
            vec![
                (
                    ValueId(2),
                    Instruction::Phi {
                        incoming: vec![(BlockId(0), valor(1))],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(3),
                    Instruction::Phi {
                        incoming: vec![(BlockId(0), valor(1))],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                drop(4, 2),
                drop(5, 3),
            ],
            Terminator::Return(None),
        ),
    ]);
    assert!(
        verificar(&f, &classes(&[1, 2, 3], &[4, 5]))
            .unwrap_err()
            .contains("já consumido")
    );
}
