//! Poda ramos de classe impossíveis usando todos os chamadores do grupo fechado.
//! Preserva IDs de consultas vivas e alcançabilidade das arestas excepcionais.

use super::*;

pub(super) fn preparar(
    m: &mut Module,
    planos: &mut HashMap<String, PlanoFuncaoDart>,
) -> Result<(), String> {
    let receivers = super::receivers::produzir(m);
    let mut mudou = false;
    for f in &mut m.functions {
        let Some(classes) = receivers.get(&f.symbol) else {
            continue;
        };
        let p = planos
            .get_mut(&f.symbol)
            .ok_or_else(|| format!("classe ARC: plano ausente de {}", f.symbol))?;
        let mut consultas = HashMap::new();
        for (v, i, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if let Instruction::CallRuntime {
                name,
                args,
                ret_ty: Type::I64,
            } = i
                && name == "dartforge_value_class"
                && *ty == Type::I64
                && let [(Operand::Val(obj), Type::Ref)] = args.as_slice()
                && let Some(cid) = classes.get(obj)
                && !p.tabelas.invocacoes.contains_key(v)
                && !p.tokens.pendencias.contains_key(v)
            {
                consultas.insert(*v, i64::from(*cid));
            }
        }
        if consultas.is_empty() {
            continue;
        }
        mudou = true;
        super::ssa::verificar(f)?;
        let arestas_antes: HashSet<_> = f
            .blocks
            .iter()
            .flat_map(|b| {
                super::super::cfg::sucessores(&b.terminator)
                    .into_iter()
                    .chain(
                        b.instructions
                            .iter()
                            .filter_map(|(v, _, _)| p.tabelas.invocacoes.get(v).copied()),
                    )
                    .map(move |destino| (b.id, destino))
            })
            .collect();
        if let Some(c) = &p.tabelas.cleanup_estrangeiro {
            c.conferir(f, &p.tabelas)?;
        }
        for b in &mut f.blocks {
            for (v, i, _) in &mut b.instructions {
                if let Some(cid) = consultas.get(v) {
                    *i = Instruction::Const(Constant::Int(*cid));
                }
            }
            if let Terminator::Switch {
                val: Operand::Val(v),
                default,
                cases,
            } = &b.terminator
                && let Some(cid) = consultas.get(v)
            {
                let destino = cases
                    .iter()
                    .find(|(c, _)| c == cid)
                    .map_or(*default, |(_, b)| *b);
                b.terminator = Terminator::Branch(destino);
            }
        }
        let blocos: HashMap<_, _> = f.blocks.iter().map(|b| (b.id, b)).collect();
        let mut vivos = HashSet::new();
        let mut fila = vec![f.blocks[0].id];
        let mut arestas = HashSet::new();
        while let Some(id) = fila.pop() {
            if !vivos.insert(id) {
                continue;
            }
            let b = blocos.get(&id).ok_or("classe ARC: destino fora do CFG")?;
            let destinos = super::super::cfg::sucessores(&b.terminator)
                .into_iter()
                .chain(
                    b.instructions
                        .iter()
                        .filter_map(|(v, _, _)| p.tabelas.invocacoes.get(v).copied()),
                );
            for destino in destinos {
                arestas.insert((id, destino));
                fila.push(destino);
            }
        }
        let mortos: HashSet<_> = f
            .blocks
            .iter()
            .filter(|b| !vivos.contains(&b.id))
            .map(|b| b.id)
            .collect();
        let ids_mortos: HashSet<_> = f
            .blocks
            .iter()
            .filter(|b| mortos.contains(&b.id))
            .flat_map(|b| &b.instructions)
            .map(|(v, _, _)| *v)
            .collect();
        f.blocks.retain(|b| !mortos.contains(&b.id));
        for (_, i, _) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
            if let Instruction::Phi { incoming, .. } = i {
                incoming.retain(|(b, _)| !mortos.contains(b));
            }
        }
        p.classes.retain(|v, _| !ids_mortos.contains(v));
        p.tokens.instrucoes.retain(|v, _| !ids_mortos.contains(v));
        p.tokens.pendencias.retain(|v, _| !ids_mortos.contains(v));
        p.tabelas.invocacoes.retain(|v, _| !ids_mortos.contains(v));
        p.tabelas.pousos.retain(|b| !mortos.contains(b));
        p.tabelas.saidas.retain(|b, _| !mortos.contains(b));
        p.owners_no_pouso.retain(|b, _| !mortos.contains(b));
        p.escopos.antes.retain(|v, _| !ids_mortos.contains(v));
        p.escopos.saidas.retain(|b, _| !mortos.contains(b));
        p.escopos
            .arestas
            .retain(|e, _| !arestas_antes.contains(e) || arestas.contains(e));
        // O certificado antigo pertence ao corpo anterior. A transação
        // completa recalcula owners/quadros e sela o cleanup após inserir ARC.
        p.tabelas.cleanup_estrangeiro = None;
        if let Some(sitios) = m.sitios_arc.get_mut(&f.symbol) {
            sitios.retain(|v, _| !ids_mortos.contains(v));
        }
        super::ssa::verificar(f)?;
    }
    if mudou {
        m.portas_arc_aot = None;
    }
    if m.excecoes_por_tabelas {
        m.tabelas = m
            .functions
            .iter()
            .map(|f| {
                planos
                    .get(&f.symbol)
                    .map(|p| p.tabelas.clone())
                    .ok_or_else(|| format!("classe ARC: plano ausente de {}", f.symbol))
            })
            .collect::<Result<_, _>>()?;
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    fn origem() -> (Module, HashMap<String, PlanoFuncaoDart>) {
        let mut m = Module::new();
        m.memoria_arc = true;
        m.layouts_campos_arc.insert(123, vec![Type::I64, Type::Ref]);
        m.classes.push(ClassDef {
            id: 123,
            name: "C".into(),
            field_count: 2,
            vtable: vec![],
            to_string_symbol: None,
        });
        m.functions.push(Function {
            symbol: "caller".into(),
            name: "caller".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::CallRuntime {
                            name: "dartforge_object_new".into(),
                            args: vec![
                                (Operand::Constant(Constant::Int(123)), Type::I64),
                                (Operand::Constant(Constant::Int(2)), Type::I64),
                            ],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(1),
                        Instruction::CallStatic {
                            symbol: "construtor".into(),
                            args: vec![Operand::Val(ValueId(0))],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ),
                ],
                terminator: Terminator::Return(None),
            }],
        });
        m.functions.push(Function {
            symbol: "construtor".into(),
            name: "construtor".into(),
            depuracao: None,
            params: vec![(ValueId(0), "this".into(), Type::Ref)],
            return_ty: Type::Void,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (ValueId(1), Instruction::Const(Constant::Int(7)), Type::I64),
                        (
                            ValueId(2),
                            Instruction::CallRuntime {
                                name: "dartforge_value_class".into(),
                                args: vec![(Operand::Val(ValueId(0)), Type::Ref)],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        ),
                    ],
                    terminator: Terminator::Switch {
                        val: Operand::Val(ValueId(2)),
                        default: BlockId(2),
                        cases: vec![(123, BlockId(1)), (124, BlockId(2))],
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(3),
                        Instruction::SetField {
                            object: Operand::Val(ValueId(0)),
                            index: 0,
                            value: Operand::Val(ValueId(1)),
                        },
                        Type::Void,
                    )],
                    terminator: Terminator::Return(None),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![(
                        ValueId(4),
                        Instruction::SetField {
                            object: Operand::Val(ValueId(0)),
                            index: 1,
                            value: Operand::Val(ValueId(1)),
                        },
                        Type::Void,
                    )],
                    terminator: Terminator::Return(None),
                },
            ],
        });
        let ps = m
            .functions
            .iter()
            .map(|f| (f.symbol.clone(), PlanoFuncaoDart::default()))
            .collect();
        (m, ps)
    }

    #[test]
    fn classe_de_todos_os_callers_tira_gravacao_impossivel_antes_das_guardas() {
        let (mut m, mut ps) = origem();
        ps.get_mut("construtor")
            .unwrap()
            .classes
            .insert(ValueId(4), Ownership::Trivial);
        preparar(&mut m, &mut ps).unwrap();
        assert!(!m.functions[1].blocks.iter().any(|b| b.id == BlockId(2)));
        assert!(!ps["construtor"].classes.contains_key(&ValueId(4)));
        preparar_arc_modulo_dart(&mut m, &mut ps).unwrap();
        for f in &m.functions {
            let p = &ps[&f.symbol];
            verificar_contrato_funcao_dart(f, &p.classes, &p.tokens, &p.tabelas, &p.escopos)
                .unwrap();
        }
    }

    #[test]
    fn parametro_aberto_ou_receiver_de_classe_divergente_nao_dobra_switch() {
        for caso in 0..4 {
            let (mut m, mut ps) = origem();
            if caso == 1 {
                m.ajudantes.push(("externa".into(), "construtor".into()));
            } else if caso == 2 {
                m.raizes_da_fonte.push("construtor".into());
            } else if caso == 3 {
                m.funcoes_de_tabela.insert(1, "construtor".into());
            } else {
                m.functions[0]
                    .params
                    .push((ValueId(7), "outra_origem".into(), Type::Ref));
                m.functions[0].blocks[0].instructions.push((
                    ValueId(8),
                    Instruction::CallStatic {
                        symbol: "construtor".into(),
                        args: vec![Operand::Val(ValueId(7))],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ));
            }
            let antes = format!("{m:?}");
            preparar(&mut m, &mut ps).unwrap();
            assert_eq!(format!("{m:?}"), antes);
        }
    }

    #[test]
    fn pouso_so_excepcional_permanece_alcancavel_e_escopo_da_consulta_fica() {
        let (mut m, mut ps) = origem();
        let p = ps.get_mut("construtor").unwrap();
        p.tabelas.invocacoes.insert(ValueId(9), BlockId(9));
        p.tabelas.pousos.insert(BlockId(9));
        p.tabelas.saidas.insert(BlockId(9), SaidaPorExcecao::Lanca);
        p.owners_no_pouso.insert(BlockId(9), vec![]);
        p.escopos
            .antes
            .insert(ValueId(2), vec![AlteracaoEscopo::Abrir(1)]);
        p.escopos
            .arestas
            .insert((BlockId(0), BlockId(2)), vec![AlteracaoEscopo::Fechar(1)]);
        m.functions[1].blocks[0].instructions.insert(
            0,
            (
                ValueId(9),
                Instruction::CallStatic {
                    symbol: "caller".into(),
                    args: vec![],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ),
        );
        m.functions[1].blocks.push(BasicBlock {
            id: BlockId(9),
            instructions: vec![],
            terminator: Terminator::Return(None),
        });
        preparar(&mut m, &mut ps).unwrap();
        assert!(m.functions[1].blocks.iter().any(|b| b.id == BlockId(9)));
        assert!(ps["construtor"].escopos.antes.contains_key(&ValueId(2)));
        assert!(
            !ps["construtor"]
                .escopos
                .arestas
                .contains_key(&(BlockId(0), BlockId(2)))
        );
    }
}
