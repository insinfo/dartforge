use super::*;

fn classes_de_parametros() -> HashMap<ValueId, Ownership> {
    HashMap::from([
        (
            ValueId(0),
            Ownership::Borrowed {
                owner: OrigemOwner::Chamador,
                escopo: 0,
            },
        ),
        (
            ValueId(1),
            Ownership::Borrowed {
                owner: OrigemOwner::Chamador,
                escopo: 0,
            },
        ),
        (ValueId(9), Ownership::Trivial),
    ])
}

#[test]
fn classificacao_preserva_alias_gerenciado_representado_por_i64() {
    let mut f = funcao(vec![bloco(
        0,
        vec![
            (
                ValueId(2),
                Instruction::Bitcast {
                    op: Operand::Val(ValueId(0)),
                    to: Type::I64,
                },
                Type::I64,
            ),
            (
                ValueId(3),
                Instruction::CallRuntime {
                    name: "dartforge_print".into(),
                    args: vec![(Operand::Val(ValueId(2)), Type::I64)],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ),
        ],
        Terminator::Return(None),
    )]);
    f.return_ty = Type::Void;
    let mut classes = classes_de_parametros();
    classes.insert(
        ValueId(2),
        Ownership::Borrowed {
            owner: OrigemOwner::Valor(ValueId(0)),
            escopo: 1,
        },
    );
    classes.insert(ValueId(3), Ownership::Trivial);
    let v = vivacidade_classificada(&f, &classes).unwrap();
    assert_eq!(v.antes[&ValueId(3)], refs(&[0, 2]));
    assert!(v.depois[&ValueId(3)].is_empty());
}

#[test]
fn inventario_tipado_recusa_cobertura_incompleta_e_id_obsoleto() {
    let f = funcao(vec![bloco(0, vec![], Terminator::Return(None))]);
    let mut classes = classes_de_parametros();
    classes.remove(&ValueId(1));
    assert!(
        vivacidade_classificada(&f, &classes)
            .unwrap_err()
            .contains("v1 sem classificação")
    );
    classes = classes_de_parametros();
    classes.insert(ValueId(8), Ownership::Owned);
    assert!(
        vivacidade_classificada(&f, &classes)
            .unwrap_err()
            .contains("v8 não existe")
    );
}

#[test]
fn resultado_local_nao_pode_alegar_owner_do_chamador() {
    let f = funcao(vec![bloco(
        0,
        vec![(
            ValueId(2),
            Instruction::Bitcast {
                op: Operand::Val(ValueId(0)),
                to: Type::Ref,
            },
            Type::Ref,
        )],
        Terminator::Return(None),
    )]);
    let mut classes = classes_de_parametros();
    classes.insert(
        ValueId(2),
        Ownership::Borrowed {
            owner: OrigemOwner::Chamador,
            escopo: 0,
        },
    );
    assert!(
        vivacidade_classificada(&f, &classes)
            .unwrap_err()
            .contains("v2 local não é parâmetro")
    );
    classes.insert(
        ValueId(2),
        Ownership::Borrowed {
            owner: OrigemOwner::Valor(ValueId(1)),
            escopo: 0,
        },
    );
    classes.insert(ValueId(1), Ownership::Trivial);
    assert!(
        vivacidade_classificada(&f, &classes)
            .unwrap_err()
            .contains("ARC003")
    );
}

#[test]
fn emprestimo_transitivo_segura_owner_ate_o_ultimo_uso() {
    let mut f = funcao(vec![bloco(
        0,
        vec![(
            ValueId(3),
            Instruction::CallRuntime {
                name: "dartforge_print".into(),
                args: vec![(Operand::Val(ValueId(2)), Type::Ref)],
                ret_ty: Type::Void,
            },
            Type::Void,
        )],
        Terminator::Return(None),
    )]);
    f.params.push((ValueId(2), "alias2".into(), Type::Ref));
    f.return_ty = Type::Void;
    let v = vivacidade_com_emprestimos(
        &f,
        &refs(&[0, 1, 2]),
        &HashMap::from([(ValueId(2), ValueId(1)), (ValueId(1), ValueId(0))]),
    )
    .unwrap();
    assert_eq!(v.antes[&ValueId(3)], refs(&[0, 1, 2]));
    assert!(v.depois[&ValueId(3)].is_empty());
    assert_eq!(
        vivacidade(&f, &refs(&[0, 1, 2])).antes[&ValueId(3)],
        refs(&[2])
    );
}

#[test]
fn owners_de_emprestimos_em_phi_ficam_na_aresta_correta() {
    let mut f = funcao(vec![
        bloco(
            0,
            vec![],
            Terminator::CondBranch {
                cond: Operand::Val(ValueId(9)),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        bloco(1, vec![], Terminator::Branch(BlockId(3))),
        bloco(2, vec![], Terminator::Branch(BlockId(3))),
        bloco(
            3,
            vec![(
                ValueId(4),
                Instruction::Phi {
                    incoming: vec![
                        (BlockId(1), Operand::Val(ValueId(2))),
                        (BlockId(2), Operand::Val(ValueId(3))),
                    ],
                    ty: Type::Ref,
                },
                Type::Ref,
            )],
            Terminator::Return(Some(Operand::Val(ValueId(4)))),
        ),
    ]);
    f.params.extend([
        (ValueId(2), "alias_x".into(), Type::Ref),
        (ValueId(3), "alias_y".into(), Type::Ref),
    ]);
    let v = vivacidade_com_emprestimos(
        &f,
        &refs(&[0, 1, 2, 3, 4]),
        &HashMap::from([(ValueId(2), ValueId(0)), (ValueId(3), ValueId(1))]),
    )
    .unwrap();
    assert_eq!(v.arestas[&(BlockId(1), BlockId(3))], refs(&[0, 2]));
    assert_eq!(v.arestas[&(BlockId(2), BlockId(3))], refs(&[1, 3]));
}

#[test]
fn emprestimo_recusa_owner_ausente_e_ciclo_com_caminho_estavel() {
    let f = funcao(vec![bloco(0, vec![], Terminator::Return(None))]);
    let faltante = vivacidade_com_emprestimos(
        &f,
        &refs(&[0, 1, 2]),
        &HashMap::from([(ValueId(1), ValueId(2))]),
    )
    .unwrap_err();
    assert_eq!(faltante.funcao, "f");
    assert_eq!(faltante.caminho, vec![ValueId(1), ValueId(2)]);
    let fora_do_inventario =
        vivacidade_com_emprestimos(&f, &refs(&[1]), &HashMap::from([(ValueId(1), ValueId(0))]))
            .unwrap_err();
    assert!(fora_do_inventario.to_string().contains("ARC003"));
    let ciclo = [(ValueId(1), ValueId(0)), (ValueId(0), ValueId(1))];
    let a = vivacidade_com_emprestimos(&f, &refs(&[0, 1]), &HashMap::from(ciclo)).unwrap_err();
    let b = vivacidade_com_emprestimos(&f, &refs(&[0, 1]), &HashMap::from([ciclo[1], ciclo[0]]))
        .unwrap_err();
    assert_eq!(a, b);
    assert_eq!(a.caminho, vec![ValueId(0), ValueId(1), ValueId(0)]);
}

#[test]
fn resultado_da_chamada_so_e_vivo_na_aresta_normal() {
    let mut m = Module {
        functions: vec![funcao(vec![bloco(
            0,
            vec![(
                ValueId(10),
                Instruction::CallStatic {
                    symbol: "pode_lancar".into(),
                    args: Vec::new(),
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )],
            Terminator::Return(Some(Operand::Val(ValueId(10)))),
        )])],
        ..Default::default()
    };
    let _plano = super::super::tabelas::preparar(&mut m);
    let Terminator::CondBranch {
        then_block: pouso,
        else_block: continuacao,
        ..
    } = m.functions[0].blocks[0].terminator
    else {
        panic!("CFG não preparado")
    };
    let inventario: HashSet<ValueId> = m.functions[0]
        .blocks
        .iter()
        .flat_map(|b| {
            b.instructions
                .iter()
                .filter(|(_, _, t)| *t == Type::Ref)
                .map(|(v, _, _)| *v)
        })
        .collect();
    let v = vivacidade(&m.functions[0], &inventario);
    assert_eq!(v.arestas[&(BlockId(0), continuacao)], refs(&[10]));
    assert!(v.arestas[&(BlockId(0), pouso)].is_empty());
    assert!(
        v.arestas[&(pouso, continuacao)].is_empty(),
        "o valor padrão do erro não é o resultado inexistente"
    );
    assert!(v.antes[&ValueId(10)].is_empty());
}

fn bloco(
    id: u32,
    instructions: Vec<(ValueId, Instruction, Type)>,
    terminator: Terminator,
) -> BasicBlock {
    BasicBlock {
        id: BlockId(id),
        instructions,
        terminator,
    }
}

fn funcao(blocks: Vec<BasicBlock>) -> Function {
    Function {
        symbol: "f".into(),
        name: "f".into(),
        depuracao: None,
        params: vec![
            (ValueId(0), "x".into(), Type::Ref),
            (ValueId(1), "y".into(), Type::Ref),
            (ValueId(9), "cond".into(), Type::I1),
        ],
        return_ty: Type::Ref,
        blocks,
    }
}

fn refs(ids: &[u32]) -> HashSet<ValueId> {
    ids.iter().copied().map(ValueId).collect()
}

#[test]
fn phi_so_exige_a_entrada_do_predecessor_correto() {
    let f = funcao(vec![
        bloco(
            0,
            vec![],
            Terminator::CondBranch {
                cond: Operand::Val(ValueId(9)),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        bloco(1, vec![], Terminator::Branch(BlockId(3))),
        bloco(2, vec![], Terminator::Branch(BlockId(3))),
        bloco(
            3,
            vec![(
                ValueId(2),
                Instruction::Phi {
                    incoming: vec![
                        (BlockId(1), Operand::Val(ValueId(0))),
                        (BlockId(2), Operand::Val(ValueId(1))),
                    ],
                    ty: Type::Ref,
                },
                Type::Ref,
            )],
            Terminator::Return(Some(Operand::Val(ValueId(2)))),
        ),
    ]);
    let v = vivacidade(&f, &refs(&[0, 1, 2]));
    assert_eq!(v.arestas[&(BlockId(1), BlockId(3))], refs(&[0]));
    assert_eq!(v.arestas[&(BlockId(2), BlockId(3))], refs(&[1]));
    assert_eq!(v.entrada[&BlockId(0)], refs(&[0, 1]));
    assert!(v.entrada[&BlockId(3)].is_empty());
    assert_eq!(v.depois[&ValueId(2)], refs(&[2]));
}

#[test]
fn backedge_converge_sem_carregar_o_resultado_da_proxima_iteracao() {
    let f = funcao(vec![
        bloco(0, vec![], Terminator::Branch(BlockId(1))),
        bloco(
            1,
            vec![
                (
                    ValueId(2),
                    Instruction::Phi {
                        incoming: vec![
                            (BlockId(0), Operand::Val(ValueId(0))),
                            (BlockId(2), Operand::Val(ValueId(3))),
                        ],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(3),
                    Instruction::CallStatic {
                        symbol: "produzir".into(),
                        args: vec![Operand::Val(ValueId(2))],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
            ],
            Terminator::CondBranch {
                cond: Operand::Val(ValueId(9)),
                then_block: BlockId(2),
                else_block: BlockId(3),
            },
        ),
        bloco(2, vec![], Terminator::Branch(BlockId(1))),
        bloco(
            3,
            vec![],
            Terminator::Return(Some(Operand::Val(ValueId(2)))),
        ),
    ]);
    let v = vivacidade(&f, &refs(&[0, 2, 3]));
    assert_eq!(v.arestas[&(BlockId(0), BlockId(1))], refs(&[0]));
    assert_eq!(v.arestas[&(BlockId(2), BlockId(1))], refs(&[3]));
    assert_eq!(v.arestas[&(BlockId(1), BlockId(3))], refs(&[2]));
    assert_eq!(v.antes[&ValueId(3)], refs(&[2]));
    assert_eq!(v.depois[&ValueId(3)], refs(&[2, 3]));
    assert!(v.entrada[&BlockId(1)].is_empty());
}

#[test]
fn inalcançavel_e_escalar_nao_criam_obrigacao_de_vida() {
    let f = funcao(vec![
        bloco(
            0,
            vec![],
            Terminator::Return(Some(Operand::Val(ValueId(0)))),
        ),
        bloco(
            1,
            vec![],
            Terminator::Return(Some(Operand::Val(ValueId(1)))),
        ),
    ]);
    let v = vivacidade(&f, &refs(&[0, 1]));
    assert_eq!(v.entrada[&BlockId(0)], refs(&[0]));
    assert!(!v.entrada.contains_key(&BlockId(1)));
    let sem_ref = vivacidade(&f, &HashSet::new());
    assert!(sem_ref.entrada[&BlockId(0)].is_empty());
}
