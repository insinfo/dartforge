//! Expõe o sucesso e o unwind dos retornos Guarda antes de inserir tokens.
//! Este passe altera somente cópias temporárias na transação do conjunto Dart.

use super::*;

// Publica a exceção antes de liberar owners locais. O Throw Ref passa pela
// ABI auditada e por duas saídas com cleanup; Lanca só desenrola após os drops.
// O protocolo pending também funciona quando o emissor não usa tabelas.
pub(super) fn preparar_lancamentos_ref(
    f: &mut Function,
    plano: &mut PlanoFuncaoDart,
) -> Result<(), String> {
    super::ssa::verificar(f)?;
    let tipos: HashMap<_, _> = f.params.iter().map(|(v, _, t)| (*v, *t))
        .chain(f.blocks.iter().flat_map(|b| &b.instructions).map(|(v, _, t)| (*v, *t)))
        .collect();
    let lancamentos: Vec<_> = f.blocks.iter().enumerate().filter_map(|(i, b)| {
        if let Terminator::Throw(o) = &b.terminator { Some((i, b.id, o.clone())) } else { None }
    }).collect();
    if lancamentos.is_empty() { return Ok(()); }
    let mut valor = tipos.keys().map(|v| v.0)
        .chain(plano.classes.keys().map(|v| v.0))
        .chain(plano.tokens.instrucoes.keys().map(|v| v.0))
        .chain(plano.tokens.pendencias.keys().map(|v| v.0))
        .chain(plano.tabelas.invocacoes.keys().map(|v| v.0))
        .chain(plano.escopos.antes.keys().map(|v| v.0))
        .max().map_or(0, |v| u64::from(v) + 1);
    let mut bloco = f.blocks.iter().map(|b| b.id.0)
        .chain(plano.tabelas.saidas.keys().map(|b| b.0))
        .chain(plano.tabelas.pousos.iter().map(|b| b.0))
        .chain(plano.tabelas.invocacoes.values().map(|b| b.0))
        .chain(plano.tokens.pendencias.values().map(|b| b.0))
        .chain(plano.escopos.saidas.keys().map(|b| b.0))
        .chain(plano.escopos.arestas.keys().flat_map(|(a, b)| [a.0, b.0]))
        .max().map_or(0, |b| u64::from(b) + 1);
    let mut id_valor = || -> Result<ValueId, String> {
        let id = u32::try_from(valor).map_err(|_| "IDs SSA esgotados ao preparar Throw")?;
        valor += 1; Ok(ValueId(id))
    };
    let mut id_bloco = || -> Result<BlockId, String> {
        let id = u32::try_from(bloco).map_err(|_| "IDs de bloco esgotados ao preparar Throw")?;
        bloco += 1; Ok(BlockId(id))
    };
    for (i, origem, payload) in lancamentos {
        if !matches!(&payload, Operand::Constant(Constant::Null))
            && !matches!(&payload, Operand::Val(v) if tipos.get(v) == Some(&Type::Ref)) {
            return Err(format!("{} b{}: Throw exige payload Ref preparado", f.symbol, origem.0));
        }
        if plano.tabelas.saidas.contains_key(&origem) {
            return Err(format!("{} b{}: Throw conflita com saída fornecida", f.symbol, origem.0));
        }
        let chamada = id_valor()?;
        let pending = id_valor()?;
        let cond = id_valor()?;
        let erro = id_bloco()?;
        let normal = id_bloco()?;
        f.blocks[i].instructions.extend([
            (chamada, Instruction::CallRuntime { name: "dartforge_arc_lancar_ref_v1".into(),
                args: vec![(payload, Type::Ref)], ret_ty: Type::Void }, Type::Void),
            (pending, Instruction::CallRuntime { name: "dartforge_exception_pending".into(),
                args: vec![], ret_ty: Type::I8 }, Type::I8),
            (cond, Instruction::ICmp(ICmpOp::Ne, Operand::Val(pending),
                Operand::Constant(Constant::Int(0))), Type::I1),
        ]);
        f.blocks[i].terminator = Terminator::CondBranch {
            cond: Operand::Val(cond), then_block: erro, else_block: normal,
        };
        let placeholder = if f.return_ty == Type::Void { None } else {
            Some(super::super::operandos::constante_padrao(f.return_ty))
        };
        for id in [erro, normal] {
            f.blocks.push(BasicBlock { id, instructions: vec![],
                terminator: Terminator::Return(placeholder.clone()) });
        }
        plano.tabelas.saidas.insert(erro, SaidaPorExcecao::Lanca);
        if let Some(eventos) = plano.escopos.saidas.remove(&origem) {
            plano.escopos.saidas.insert(erro, eventos.clone());
            plano.escopos.saidas.insert(normal, eventos);
        }
    }
    Ok(())
}

/// Dá a cada guarda de pendência a própria cópia de um bloco de saída
/// compartilhado (sem Phi, terminando em retorno ou `Unreachable`, sem ser
/// pouso, alvo de invoke ou extremo de aresta de escopo; a classe da saída,
/// os eventos de escopo dela e a saída pending da chamada vão junto): os
/// owners vivos diferem de caminho para caminho, e
/// o cleanup de cada saída precisa ver os seus. As definições da cópia ganham
/// IDs novos; nada fora do bloco as usa. Devolve quantas cópias criou.
pub(super) fn separar_saidas_compartilhadas(f: &mut Function, plano: &mut PlanoFuncaoDart) -> Result<usize, String> {
    let guarda = |b: &BasicBlock| -> Option<BlockId> {
        let Terminator::CondBranch { cond: Operand::Val(c), then_block, .. } = &b.terminator else { return None };
        let n = b.instructions.len();
        let [(p, Instruction::CallRuntime { name, .. }, _), (ic, Instruction::ICmp(ICmpOp::Ne, Operand::Val(pv), Operand::Constant(Constant::Int(0))), _)] =
            b.instructions.get(n.checked_sub(2)?..)? else { return None };
        (name == "dartforge_exception_pending" && pv == p && ic == c).then_some(*then_block)
    };
    let mut predecessores: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
    for b in &f.blocks {
        for s in super::super::cfg::sucessores(&b.terminator) {
            predecessores.entry(s).or_default().push(b.id);
        }
    }
    let reservados = |id: &BlockId| plano.tabelas.pousos.contains(id)
        || plano.tabelas.invocacoes.values().any(|b| b == id)
        || plano.escopos.arestas.keys().any(|(a, b)| a == id || b == id);
    // Também o trampolim vazio até um tratador (`Branch`): a aresta normal da
    // cópia até ele recebe o cleanup do seu caminho.
    let trampolim = |b: &BasicBlock| b.instructions.is_empty()
        && matches!(b.terminator, Terminator::Branch(d) if !reservados(&d));
    let alvos: Vec<BlockId> = f.blocks.iter().filter(|b| {
        (matches!(b.terminator, Terminator::Return(_) | Terminator::Unreachable) || trampolim(b))
            && !b.instructions.iter().any(|(_, i, _)| matches!(i, Instruction::Phi { .. }))
            && predecessores.get(&b.id).is_some_and(|ps| ps.len() > 1)
            && !reservados(&b.id)
    }).map(|b| b.id).collect();
    // Um tratador (bloco com instruções, alvo de guardas e de outros
    // caminhos): cada guarda ganha um trampolim vazio até ele, separado abaixo.
    let guardas_de = |alvo: BlockId, f: &Function| -> Vec<BlockId> {
        predecessores.get(&alvo).into_iter().flatten().copied()
            .filter(|p| f.blocks.iter().find(|b| b.id == *p).and_then(guarda) == Some(alvo))
            .collect()
    };
    let tratadores: Vec<BlockId> = f.blocks.iter().filter(|b| {
        !alvos.contains(&b.id)
            && !reservados(&b.id)
            && predecessores.get(&b.id).is_some_and(|ps| ps.len() > 1)
    }).map(|b| b.id).filter(|t| !guardas_de(*t, f).is_empty()).collect();
    if alvos.is_empty() && tratadores.is_empty() {
        return Ok(0);
    }
    let mut valor = super::arestas::proximo_valor(f, plano);
    let mut bloco = f.blocks.iter().map(|b| b.id.0)
        .chain(plano.tabelas.saidas.keys().map(|b| b.0))
        .chain(plano.tabelas.pousos.iter().map(|b| b.0))
        .chain(plano.tabelas.invocacoes.values().map(|b| b.0))
        .chain(plano.tokens.pendencias.values().map(|b| b.0))
        .chain(plano.escopos.saidas.keys().map(|b| b.0))
        .chain(plano.escopos.arestas.keys().flat_map(|(a, b)| [a.0, b.0]))
        .max().map_or(0, |b| u64::from(b) + 1);
    let mut copias = 0;
    for alvo in alvos {
        let ps = predecessores[&alvo].clone();
        // A primeira guarda fica com o original; as outras ganham cópias.
        let mut guardas: Vec<BlockId> = ps.iter().copied().filter(|p| {
            f.blocks.iter().find(|b| b.id == *p).and_then(guarda) == Some(alvo)
        }).collect();
        if guardas.len() == ps.len() {
            guardas.remove(0);
        }
        let original = f.blocks.iter().find(|b| b.id == alvo).expect("alvo existe").clone();
        for p in guardas {
            let id = BlockId(u32::try_from(bloco).map_err(|_| "IDs de bloco esgotados ao separar saídas")?);
            bloco += 1;
            let mut novos = HashMap::new();
            let mut instrucoes = Vec::with_capacity(original.instructions.len());
            for (v, inst, ty) in &original.instructions {
                let n = ValueId(u32::try_from(valor).map_err(|_| "IDs SSA esgotados ao separar saídas")?);
                valor += 1;
                novos.insert(*v, n);
                let mut inst = inst.clone();
                super::super::operandos::operandos_mut(&mut inst, &mut |o| {
                    if let Operand::Val(x) = o && let Some(y) = novos.get(x) { *x = *y; }
                });
                instrucoes.push((n, inst, *ty));
            }
            let mut terminador = original.terminator.clone();
            if let Terminator::Return(Some(Operand::Val(x))) = &mut terminador && let Some(y) = novos.get(x) {
                *x = *y;
            }
            // Os Phis do destino do trampolim recebem a mesma entrada pela cópia.
            if let Terminator::Branch(destino) = terminador {
                for (_, inst, _) in f.blocks.iter_mut().filter(|b| b.id == destino).flat_map(|b| &mut b.instructions) {
                    if let Instruction::Phi { incoming, .. } = inst
                        && let Some(op) = incoming.iter().find(|(de, _)| *de == alvo).map(|(_, op)| op.clone())
                    {
                        incoming.push((id, op));
                    }
                }
            }
            f.blocks.push(BasicBlock { id, instructions: instrucoes, terminator: terminador });
            if let Some(classe) = plano.tabelas.saidas.get(&alvo).cloned() {
                plano.tabelas.saidas.insert(id, classe);
            }
            if let Some(eventos) = plano.escopos.saidas.get(&alvo).cloned() {
                plano.escopos.saidas.insert(id, eventos);
            }
            let b = f.blocks.iter_mut().find(|b| b.id == p).expect("guarda existe");
            if let Terminator::CondBranch { then_block, .. } = &mut b.terminator {
                *then_block = id;
            }
            // A saída pending registrada da chamada da guarda segue a cópia.
            if let Some((chamada, _, _)) = b.instructions.len().checked_sub(3).map(|k| &b.instructions[k])
                && plano.tokens.pendencias.get(chamada) == Some(&alvo)
            {
                plano.tokens.pendencias.insert(*chamada, id);
            }
            copias += 1;
        }
    }
    for tratador in tratadores {
        for p in guardas_de(tratador, f) {
            let id = BlockId(u32::try_from(bloco).map_err(|_| "IDs de bloco esgotados ao separar saídas")?);
            bloco += 1;
            for (_, inst, _) in f.blocks.iter_mut().filter(|b| b.id == tratador).flat_map(|b| &mut b.instructions) {
                if let Instruction::Phi { incoming, .. } = inst {
                    for (de, _) in incoming.iter_mut().filter(|(de, _)| *de == p) {
                        *de = id;
                    }
                }
            }
            f.blocks.push(BasicBlock { id, instructions: Vec::new(), terminator: Terminator::Branch(tratador) });
            let b = f.blocks.iter_mut().find(|b| b.id == p).expect("guarda existe");
            if let Terminator::CondBranch { then_block, .. } = &mut b.terminator {
                *then_block = id;
            }
            if let Some((chamada, _, _)) = b.instructions.len().checked_sub(3).map(|k| &b.instructions[k])
                && plano.tokens.pendencias.get(chamada) == Some(&tratador)
            {
                plano.tokens.pendencias.insert(*chamada, id);
            }
            copias += 1;
        }
    }
    Ok(copias)
}

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

    #[test]
    fn throw_ref_publica_payload_e_libera_locais_sem_transferir_retorno() {
        for caso in 0..4 {
            let mut m = Module::new(); m.memoria_arc = true;
            m.functions.push(Function { symbol: "lancar".into(), name: "lancar".into(),
                depuracao: None, params: vec![(ValueId(0), "payload".into(),
                    if caso == 3 { Type::I64 } else { Type::Ref })], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![(ValueId(1),
                    Instruction::ArcCopy { value: Operand::Val(ValueId(0)) }, Type::Ref)],
                    terminator: Terminator::Throw(match caso {
                        1 => Operand::Val(ValueId(1)),
                        2 => Operand::Constant(Constant::Null),
                        _ => Operand::Val(ValueId(0)),
                    }) }] });
            m.retornos_ref_dart.insert("lancar".into());
            if caso == 3 {
                m.parametros_escalares_dart.insert("lancar".into(), HashSet::from([ValueId(0)]));
                m.functions[0].blocks[0].instructions[0].1 = Instruction::ArcCopy {
                    value: Operand::Constant(Constant::Null),
                };
            }
            let mut planos = HashMap::from([("lancar".into(), PlanoFuncaoDart::default())]);
            let antes = format!("{m:?}/{planos:?}");
            let resultado = preparar_arc_modulo_dart(&mut m, &mut planos);
            if caso == 3 {
                let erro = resultado.unwrap_err();
                assert!(erro.contains("Throw exige payload Ref"), "{erro}");
                assert_eq!(format!("{m:?}/{planos:?}"), antes);
                continue;
            }
            resultado.unwrap();
            let f = &m.functions[0];
            assert!(f.blocks.iter().all(|b| !matches!(b.terminator, Terminator::Throw(_))));
            let publicacoes = f.blocks.iter().flat_map(|b| &b.instructions)
                .filter(|(_, i, _)| matches!(i, Instruction::CallRuntime { name, .. }
                    if name == "dartforge_arc_lancar_ref_v1")).count();
            assert_eq!(publicacoes, 1);
            // Um local morto pode ser liberado antes do Throw. O payload
            // Owned, porém, só pode ser liberado depois de publicar a exceção.
            assert!(f.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)|
                matches!(i, Instruction::ArcDrop { value: Operand::Val(ValueId(1)) })));
            if caso == 1 {
                let b = &f.blocks[0];
                let publicacao = b.instructions.iter().position(|(_, i, _)| matches!(i,
                    Instruction::CallRuntime { name, .. } if name == "dartforge_arc_lancar_ref_v1")).unwrap();
                assert!(!b.instructions[..publicacao].iter().any(|(_, i, _)| matches!(i,
                    Instruction::ArcDrop { value: Operand::Val(ValueId(1)) })));
            }
            for b in f.blocks.iter().filter(|b| matches!(b.terminator, Terminator::Return(_))) {
                assert!(matches!(b.terminator, Terminator::Return(Some(Operand::Constant(Constant::Null)))));
            }
            let preparado = format!("{m:?}/{planos:?}");
            assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
            assert_eq!(format!("{m:?}/{planos:?}"), preparado);
        }
    }

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

#[cfg(test)]
mod testes_saidas_compartilhadas {
    use super::*;

    #[test]
    fn cada_guarda_ganha_a_propria_saida_e_o_resto_fica() {
        let val = |n| Operand::Val(ValueId(n));
        let guarda = |p: u32, c: u32, erro: u32, segue: u32, chamada: Instruction| BasicBlock {
            id: BlockId(p),
            instructions: vec![
                (ValueId(c), chamada, Type::Ref),
                (ValueId(c + 1), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                (ValueId(c + 2), Instruction::ICmp(ICmpOp::Ne, val(c + 1), Operand::Constant(Constant::Int(0))), Type::I1),
            ],
            terminator: Terminator::CondBranch { cond: val(c + 2), then_block: BlockId(erro), else_block: BlockId(segue) },
        };
        let chamada = || Instruction::CallStatic { symbol: "g".into(), args: vec![], ret_ty: Type::Ref };
        let mut f = Function { symbol: "f".into(), name: "f".into(), depuracao: None, params: vec![], return_ty: Type::Ref,
            blocks: vec![
                guarda(0, 1, 9, 1, chamada()),
                guarda(1, 4, 9, 2, chamada()),
                BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(val(4))) },
                BasicBlock { id: BlockId(9), instructions: vec![
                    (ValueId(20), Instruction::StoreGlobal { simbolo: "ok".into(), val: Operand::Constant(Constant::Int(0)), ty: Type::I8, raiz: None }, Type::Void),
                ], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
            ] };
        let mut plano = PlanoFuncaoDart::default();
        plano.tabelas.saidas.insert(BlockId(9), SaidaPorExcecao::Lanca);
        assert_eq!(separar_saidas_compartilhadas(&mut f, &mut plano).unwrap(), 1);
        let Terminator::CondBranch { then_block: a, .. } = f.blocks[0].terminator else { unreachable!() };
        let Terminator::CondBranch { then_block: b, .. } = f.blocks[1].terminator else { unreachable!() };
        assert_eq!(a, BlockId(9));
        assert_ne!(b, BlockId(9));
        assert_eq!(plano.tabelas.saidas.get(&b), Some(&SaidaPorExcecao::Lanca));
        let copia = f.blocks.iter().find(|x| x.id == b).unwrap();
        assert_ne!(copia.instructions[0].0, ValueId(20));
        assert!(matches!(&copia.instructions[0].1, Instruction::StoreGlobal { simbolo, .. } if simbolo == "ok"));
        super::super::ssa::verificar(&f).unwrap();
        // Uma saída de pouso fica como está.
        let mut g = f.clone();
        if let Terminator::CondBranch { then_block, .. } = &mut g.blocks[1].terminator { *then_block = BlockId(9); }
        g.blocks.retain(|x| x.id != b);
        let mut plano = PlanoFuncaoDart::default();
        plano.tabelas.pousos.insert(BlockId(9));
        assert_eq!(separar_saidas_compartilhadas(&mut g, &mut plano).unwrap(), 0);
    }

    #[test]
    fn tratador_compartilhado_ganha_um_trampolim_por_guarda() {
        let val = |n| Operand::Val(ValueId(n));
        let guarda = |p: u32, c: u32, segue: u32| BasicBlock {
            id: BlockId(p),
            instructions: vec![
                (ValueId(c), Instruction::CallStatic { symbol: "g".into(), args: vec![], ret_ty: Type::Ref }, Type::Ref),
                (ValueId(c + 1), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                (ValueId(c + 2), Instruction::ICmp(ICmpOp::Ne, val(c + 1), Operand::Constant(Constant::Int(0))), Type::I1),
            ],
            terminator: Terminator::CondBranch { cond: val(c + 2), then_block: BlockId(9), else_block: BlockId(segue) },
        };
        let mut f = Function { symbol: "f".into(), name: "f".into(), depuracao: None, params: vec![], return_ty: Type::Ref,
            blocks: vec![
                guarda(0, 1, 1),
                guarda(1, 4, 2),
                BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(val(4))) },
                // O tratador: um Phi pelas guardas e o `clear` da pendência.
                BasicBlock { id: BlockId(9), instructions: vec![
                    (ValueId(20), Instruction::Phi { incoming: vec![
                        (BlockId(0), Operand::Constant(Constant::Int(1))),
                        (BlockId(1), Operand::Constant(Constant::Int(2)))], ty: Type::I64 }, Type::I64),
                    (ValueId(21), Instruction::CallRuntime { name: "dartforge_exception_clear".into(), args: vec![], ret_ty: Type::Void }, Type::Void),
                ], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
            ] };
        let mut plano = PlanoFuncaoDart::default();
        plano.tokens.pendencias.insert(ValueId(1), BlockId(9));
        plano.tokens.pendencias.insert(ValueId(4), BlockId(9));
        assert_eq!(separar_saidas_compartilhadas(&mut f, &mut plano).unwrap(), 2);
        for (g, chamada, constante) in [(0, 1, 1), (1, 4, 2)] {
            let Terminator::CondBranch { then_block: t, .. } = f.blocks[g].terminator else { unreachable!() };
            let trampolim = f.blocks.iter().find(|x| x.id == t).unwrap();
            assert!(trampolim.instructions.is_empty());
            assert!(matches!(trampolim.terminator, Terminator::Branch(BlockId(9))));
            assert_eq!(plano.tokens.pendencias[&ValueId(chamada)], t);
            let Instruction::Phi { incoming, .. } = &f.blocks[3].instructions[0].1 else { unreachable!() };
            assert!(incoming.contains(&(t, Operand::Constant(Constant::Int(constante)))));
        }
        super::super::ssa::verificar(&f).unwrap();
        // Idempotente: o tratador agora só tem trampolins como predecessores.
        assert_eq!(separar_saidas_compartilhadas(&mut f, &mut plano).unwrap(), 0);
    }
}
