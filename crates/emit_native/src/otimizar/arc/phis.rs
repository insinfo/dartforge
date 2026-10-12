//! Cópias nas arestas dos Phis Ref que juntam owners e empréstimos.
//! Um Phi que recebe algum owner transfere um token em toda aresta: cada
//! entrada emprestada ou trivial ganha um `ArcCopy` num bloco novo
//! na própria aresta. Arestas de pouso, de pendência ou com eventos de escopo
//! ficam como estão, e o produtor de contratos continua a recusá-las.

use super::*;
use std::collections::BTreeMap;

/// Insere as cópias das entradas não-owned dos Phis Ref que precisam de
/// owner e devolve quantas inseriu.
///
/// Classifica as definições sem resolver Phis (`chamadas::classificar_sem_phis`);
/// se isso falhar, não altera nada e deixa o erro para a produção completa.
/// Um Phi precisa de owner quando é devolvido (a convenção Owned do retorno)
/// ou alguma entrada é Owned, diretamente ou por outro Phi que precise. As demais entradas dele — empréstimos, triviais e
/// Phis emprestados — são copiadas na aresta; null não.
///
/// # Erros
/// IDs SSA ou de bloco esgotados.
pub(super) fn copiar_emprestimos(
    f: &mut Function,
    indice: &super::chamadas::IndiceFuncoesDart<'_>,
    p: &PlanoFuncaoDart,
) -> Result<usize, String> {
    let phis: HashMap<ValueId, (BlockId, &Vec<(BlockId, Operand)>)> = f
        .blocks
        .iter()
        .flat_map(|b| b.instructions.iter().map(move |i| (b.id, i)))
        .filter_map(|(b, (v, i, ty))| match i {
            Instruction::Phi { incoming, ty: Type::Ref } if *ty == Type::Ref => Some((*v, (b, incoming))),
            _ => None,
        })
        .collect();
    if phis.is_empty() {
        return Ok(0);
    }
    let mut classes = p.classes.clone();
    let mut tokens = p.tokens.clone();
    if super::chamadas::classificar_sem_phis(f, indice, &mut classes, &mut tokens).is_err() {
        return Ok(0);
    }
    let owned = |de: &ValueId| !phis.contains_key(de) && classes.get(de) == Some(&Ownership::Owned);
    // Também o Phi devolvido: a convenção Owned do retorno passa por ele.
    let devolvidos: HashSet<ValueId> = f.blocks.iter().filter_map(|b| match &b.terminator {
        Terminator::Return(Some(Operand::Val(v))) if phis.contains_key(v) => Some(*v),
        _ => None,
    }).collect();
    // O Phi devolvido com alguma entrada emprestada segue o desenho do
    // keepalive (empréstimo até a cópia do retorno); só o de entradas
    // triviais (literais) ou owners precisa das cópias aqui.
    let emprestada = |op: &Operand| matches!(op, Operand::Val(de)
        if matches!(classes.get(de), Some(Ownership::Borrowed { .. })) || phis.contains_key(de));
    let mut precisam: HashSet<ValueId> = phis
        .iter()
        .filter(|(v, (_, entradas))| (devolvidos.contains(v) && !entradas.iter().any(|(_, op)| emprestada(op)))
            || entradas.iter().any(|(_, op)| matches!(op, Operand::Val(de) if owned(de))))
        .map(|(v, _)| *v)
        .collect();
    loop {
        let novos: Vec<_> = phis
            .iter()
            .filter(|(v, (_, entradas))| {
                !precisam.contains(v)
                    && entradas.iter().any(|(_, op)| matches!(op, Operand::Val(de) if precisam.contains(de)))
            })
            .map(|(v, _)| *v)
            .collect();
        if novos.is_empty() {
            break;
        }
        precisam.extend(novos);
    }
    // Uma entrada owned usada depois da junção (no bloco do Phi ou adiante)
    // não pode ter o token transferido ao Phi: ela também ganha a cópia.
    let mut usos: HashMap<ValueId, Vec<BlockId>> = HashMap::new();
    for b in &f.blocks {
        for (_, i, _) in &b.instructions {
            if !matches!(i, Instruction::Phi { .. }) {
                super::super::operandos::operandos(i, &mut |o| if let Operand::Val(x) = o { usos.entry(*x).or_default().push(b.id); });
            }
        }
        super::super::operandos::operandos_do_terminador(&b.terminator, &mut |o| if let Operand::Val(x) = o { usos.entry(*x).or_default().push(b.id); });
    }
    let sucessores: HashMap<BlockId, Vec<BlockId>> = f.blocks.iter().map(|b| (b.id, super::super::cfg::sucessores(&b.terminator))).collect();
    let definicao: HashMap<ValueId, BlockId> = f.blocks.iter()
        .flat_map(|b| b.instructions.iter().map(move |(v, _, _)| (*v, b.id))).collect();
    // `de` vivo na entrada do bloco do Phi: um uso alcançável dali sem
    // passar pela (re)definição dele — num laço, o que vem depois da
    // definição já é o valor da volta seguinte.
    let usado_depois = |de: &ValueId, bloco: BlockId| {
        let Some(bs) = usos.get(de) else { return false };
        let def = definicao.get(de).copied();
        let mut vistos = HashSet::from([bloco]);
        let mut fila = vec![bloco];
        while let Some(b) = fila.pop() {
            if Some(b) == def {
                continue;
            }
            if bs.contains(&b) {
                return true;
            }
            for s in sucessores.get(&b).into_iter().flatten() {
                if vistos.insert(*s) {
                    fila.push(*s);
                }
            }
        }
        false
    };
    let pendentes: HashSet<BlockId> = tokens.pendencias.values().chain(p.tokens.pendencias.values()).copied().collect();
    let destinos = |b: &BasicBlock| super::super::cfg::sucessores(&b.terminator);
    let blocos: HashMap<BlockId, &BasicBlock> = f.blocks.iter().map(|b| (b.id, b)).collect();
    // (predecessor, bloco do Phi) → [(Phi, entrada)].
    let mut arestas: BTreeMap<(u32, u32), Vec<(ValueId, Operand)>> = BTreeMap::new();
    let mut ordem: Vec<_> = precisam.iter().copied().collect();
    ordem.sort_unstable_by_key(|v| v.0);
    for v in ordem {
        let (bloco, entradas) = phis[&v];
        for (pred, op) in entradas.iter() {
            let copiar = match op {
                Operand::Val(de) if phis.contains_key(de) => !precisam.contains(de),
                Operand::Val(de) => classes.get(de) != Some(&Ownership::Owned) || usado_depois(de, bloco),
                _ => false,
            };
            let admissivel = !p.tabelas.pousos.contains(&bloco)
                && !pendentes.contains(&bloco)
                && !p.escopos.arestas.contains_key(&(*pred, bloco))
                && blocos.get(pred).is_some_and(|b| {
                    matches!(b.terminator, Terminator::Branch(_) | Terminator::CondBranch { .. })
                        && destinos(b).contains(&bloco)
                });
            if copiar && admissivel {
                arestas.entry((pred.0, bloco.0)).or_default().push((v, op.clone()));
            }
        }
    }
    if arestas.is_empty() {
        return Ok(0);
    }
    let mut valor = super::arestas::proximo_valor(f, p);
    let mut proximo_bloco = f
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
    let mut id_valor = || -> Result<ValueId, String> {
        let id = u32::try_from(valor).map_err(|_| "IDs SSA esgotados ao copiar entradas de Phi")?;
        valor += 1;
        Ok(ValueId(id))
    };
    let mut copias = 0;
    for ((pred, bloco), entradas) in arestas {
        let (pred, bloco) = (BlockId(pred), BlockId(bloco));
        let aresta = BlockId(u32::try_from(proximo_bloco).map_err(|_| "IDs de bloco esgotados ao copiar entradas de Phi")?);
        proximo_bloco += 1;
        let mut instrucoes = Vec::new();
        let mut trocas = HashMap::new();
        for (phi, op) in entradas {
            let v = id_valor()?;
            instrucoes.push((v, Instruction::ArcCopy { value: op }, Type::Ref));
            trocas.insert(phi, v);
            copias += 1;
        }
        let b = f.blocks.iter_mut().find(|b| b.id == pred).expect("predecessor conferido");
        match &mut b.terminator {
            Terminator::Branch(destino) => *destino = aresta,
            Terminator::CondBranch { then_block, else_block, .. } => {
                for destino in [then_block, else_block] {
                    if *destino == bloco {
                        *destino = aresta;
                    }
                }
            }
            _ => unreachable!("terminador conferido"),
        }
        // Todos os Phis do bloco passam a chegar pela aresta nova.
        let alvo = f.blocks.iter_mut().find(|b| b.id == bloco).expect("bloco do Phi");
        for (v, i, _) in &mut alvo.instructions {
            if let Instruction::Phi { incoming, .. } = i {
                for (de, op) in incoming.iter_mut() {
                    if *de == pred {
                        *de = aresta;
                        if let Some(copia) = trocas.get(v) {
                            *op = Operand::Val(*copia);
                        }
                    }
                }
            }
        }
        f.blocks.push(BasicBlock { id: aresta, instructions: instrucoes, terminator: Terminator::Branch(bloco) });
    }
    Ok(copias)
}

#[cfg(test)]
mod testes {
    use super::*;

    /// `c ? this.campo : chamada()`: o campo emprestado e o owner da chamada.
    fn misto() -> (Function, Function) {
        let val = |n| Operand::Val(ValueId(n));
        let f = Function {
            symbol: "misto".into(), name: "misto".into(), depuracao: None,
            params: vec![(ValueId(0), "this".into(), Type::Ref), (ValueId(1), "c".into(), Type::I1)],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock { id: BlockId(0), instructions: vec![],
                    terminator: Terminator::CondBranch { cond: val(1), then_block: BlockId(1), else_block: BlockId(2) } },
                BasicBlock { id: BlockId(1), instructions: vec![
                    (ValueId(3), Instruction::GetField { object: val(0), index: 0 }, Type::Ref)],
                    terminator: Terminator::Branch(BlockId(3)) },
                BasicBlock { id: BlockId(2), instructions: vec![
                    (ValueId(4), Instruction::CallStatic { symbol: "fonte".into(), args: vec![], ret_ty: Type::Ref }, Type::Ref)],
                    terminator: Terminator::Branch(BlockId(3)) },
                BasicBlock { id: BlockId(3), instructions: vec![
                    (ValueId(5), Instruction::Phi { incoming: vec![(BlockId(1), val(3)), (BlockId(2), val(4))], ty: Type::Ref }, Type::Ref)],
                    terminator: Terminator::Return(Some(val(5))) },
            ],
        };
        let fonte = Function {
            symbol: "fonte".into(), name: "fonte".into(), depuracao: None, params: vec![], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }],
        };
        (f, fonte)
    }

    #[test]
    fn phi_misto_copia_o_emprestimo_na_aresta_e_o_produtor_aceita() {
        {
            let (mut f, fonte) = misto();
            let resumos = vec![
                super::super::chamadas::resumo_provisorio(&f, false),
                super::super::chamadas::resumo_provisorio(&fonte, false),
            ];
            let indice = super::super::chamadas::IndiceFuncoesDart::novo(&resumos).unwrap();
            let mut p = PlanoFuncaoDart::default();
            p.classes.insert(ValueId(3), Ownership::Borrowed { owner: OrigemOwner::Valor(ValueId(0)), escopo: 0 });
            let mut sem_copia = p.clone();
            let erro = super::super::chamadas::produzir_chamadas_e_instrucoes_dart(&f, &indice, &mut sem_copia.classes, &mut sem_copia.tokens)
                .unwrap_err();
            assert!(erro.contains("copie o empréstimo na aresta"), "{erro}");
            assert_eq!(copiar_emprestimos(&mut f, &indice, &p).unwrap(), 1);
            let aresta = f.blocks.last().unwrap();
            assert!(matches!(aresta.instructions.last(), Some((_, Instruction::ArcCopy { .. }, Type::Ref))));
            assert!(matches!(aresta.terminator, Terminator::Branch(BlockId(3))));
            assert!(matches!(f.blocks[1].terminator, Terminator::Branch(b) if b == aresta.id));
            let Instruction::Phi { incoming, .. } = &f.blocks[3].instructions[0].1 else { unreachable!() };
            assert_eq!(incoming[0], (aresta.id, Operand::Val(aresta.instructions.last().unwrap().0)));
            assert_eq!(incoming[1], (BlockId(2), Operand::Val(ValueId(4))));
            super::super::chamadas::produzir_chamadas_e_instrucoes_dart(&f, &indice, &mut p.classes, &mut p.tokens).unwrap();
            assert_eq!(p.classes[&ValueId(5)], Ownership::Owned);
            // Idempotente: nada mais a copiar.
            assert_eq!(copiar_emprestimos(&mut f, &indice, &p).unwrap(), 0);
        }
    }

    #[test]
    fn phi_so_emprestado_ou_aresta_de_pouso_fica_intacto() {
        let (f0, fonte) = misto();
        let resumos = vec![
            super::super::chamadas::resumo_provisorio(&f0, false),
            super::super::chamadas::resumo_provisorio(&fonte, false),
        ];
        let indice = super::super::chamadas::IndiceFuncoesDart::novo(&resumos).unwrap();
        let mut p = PlanoFuncaoDart::default();
        p.classes.insert(ValueId(3), Ownership::Borrowed { owner: OrigemOwner::Valor(ValueId(0)), escopo: 0 });
        // O bloco do Phi é um pouso: a aresta não é dividida.
        let mut f = f0.clone();
        p.tabelas.pousos.insert(BlockId(3));
        assert_eq!(copiar_emprestimos(&mut f, &indice, &p).unwrap(), 0);
        assert_eq!(format!("{f:?}"), format!("{f0:?}"));
        p.tabelas.pousos.clear();
        // Sem owner entre as entradas e sem ser devolvido, o Phi continua
        // emprestado.
        let mut f = f0.clone();
        let Instruction::Phi { incoming, .. } = &mut f.blocks[3].instructions[0].1 else { unreachable!() };
        incoming[1].1 = Operand::Constant(Constant::Null);
        f.blocks[3].terminator = Terminator::Return(Some(Operand::Constant(Constant::Null)));
        let antes = format!("{f:?}");
        assert_eq!(copiar_emprestimos(&mut f, &indice, &p).unwrap(), 0);
        assert_eq!(format!("{f:?}"), antes);
        // Devolvido com entrada emprestada, segue o keepalive: nada aqui.
        f.blocks[3].terminator = Terminator::Return(Some(Operand::Val(ValueId(5))));
        assert_eq!(copiar_emprestimos(&mut f, &indice, &p).unwrap(), 0);
    }
}
