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
/// Um Phi precisa de owner quando é devolvido (a convenção Owned do retorno),
/// quando junta empréstimos de raízes diferentes, ou quando alguma entrada é
/// Owned, diretamente ou por outro Phi que precise. As demais entradas dele — empréstimos, triviais e
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
    let pendentes: HashSet<BlockId> = tokens.pendencias.values().copied().collect();
    copiar(f, p, &classes, &pendentes)
}

/// [`copiar_emprestimos`] com as classes das definições já conhecidas (sem
/// as dos Phis): o caminho do `proteger`, depois de promover empréstimos.
///
/// # Erros
/// IDs SSA ou de bloco esgotados.
pub(super) fn copiar_com_classes(
    f: &mut Function,
    p: &PlanoFuncaoDart,
    classes: &HashMap<ValueId, Ownership>,
) -> Result<usize, String> {
    copiar(f, p, classes, &HashSet::new())
}

fn copiar(
    f: &mut Function,
    p: &PlanoFuncaoDart,
    classes: &HashMap<ValueId, Ownership>,
    pendentes_extras: &HashSet<BlockId>,
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
    let owned = |de: &ValueId| !phis.contains_key(de) && classes.get(de) == Some(&Ownership::Owned);
    // Também o Phi devolvido: a convenção Owned do retorno passa por ele.
    let devolvidos: HashSet<ValueId> = f.blocks.iter().filter_map(|b| match &b.terminator {
        Terminator::Return(Some(Operand::Val(v))) if phis.contains_key(v) => Some(*v),
        _ => None,
    }).collect();
    // Os contratos de empréstimo que chegam a cada Phi, com a entrada que
    // primeiro trouxe cada um: o produtor só deixa um Phi emprestado quando
    // todas as entradas diretas têm a mesma classe crua (owner e escopo); um
    // Phi interno entra com a classe que ele recebe (o empréstimo do
    // chamador preso à entrada de origem). Com dois contratos o Phi precisa
    // de owner — mesmo que os owners sejam aliases de uma raiz comum.
    let mut raizes: HashMap<ValueId, Vec<(Ownership, ValueId)>> = HashMap::new();
    loop {
        let mut mudou = false;
        for (v, (_, entradas)) in &phis {
            let mut atual = raizes.get(v).cloned().unwrap_or_default();
            let antes = atual.len();
            let mut juntar = |c: Ownership, de: ValueId| if !atual.iter().any(|(x, _)| *x == c) { atual.push((c, de)) };
            for (_, op) in entradas.iter() {
                let Operand::Val(de) = op else { continue };
                if phis.contains_key(de) {
                    // Com dois ou mais, o Phi interno precisa de owner e este
                    // também (entrada que precisa); basta propagar todos.
                    let internos = raizes.get(de).cloned().unwrap_or_default();
                    for (c, origem) in internos {
                        let c = match (c, raizes[de].len()) {
                            (Ownership::Borrowed { owner: OrigemOwner::Chamador, escopo }, 1) =>
                                Ownership::Borrowed { owner: OrigemOwner::Valor(origem), escopo },
                            (c, _) => c,
                        };
                        juntar(c, *de);
                    }
                } else if let Some(c @ Ownership::Borrowed { .. }) = classes.get(de) {
                    juntar(*c, *de);
                }
            }
            if atual.len() != antes || !raizes.contains_key(v) {
                raizes.insert(*v, atual);
                mudou = true;
            }
        }
        if !mudou {
            break;
        }
    }
    // O Phi devolvido com alguma entrada emprestada (direta ou por um Phi com
    // raiz de empréstimo) segue o desenho do keepalive (empréstimo até a
    // cópia do retorno); só o de entradas triviais (literais, inclusive por
    // Phis só deles) ou owners precisa das cópias aqui.
    let emprestada = |op: &Operand| matches!(op, Operand::Val(de)
        if matches!(classes.get(de), Some(Ownership::Borrowed { .. }))
            || raizes.get(de).is_some_and(|r| !r.is_empty()));
    // O owner de um empréstimo precisa dominar o Phi (ARC003): o laço que
    // carrega um alias de um valor definido depois do cabeçalho precisa de
    // owner próprio. Phis do mesmo bloco são simultâneos.
    let cfg = super::super::cfg::Cfg::novo(f);
    let indice: HashMap<BlockId, usize> = f.blocks.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
    let definicoes: HashMap<ValueId, (usize, bool)> = f.blocks.iter().enumerate()
        .flat_map(|(i, b)| b.instructions.iter().map(move |(v, inst, _)| (*v, (i, matches!(inst, Instruction::Phi { .. })))))
        .collect();
    let domina = |owner: ValueId, bloco: BlockId| {
        let Some(&(o, owner_phi)) = definicoes.get(&owner) else { return true };
        let mut b = indice[&bloco];
        if !cfg.alcancavel(b) {
            return true;
        }
        if o == b {
            return owner_phi;
        }
        loop {
            if b == o {
                return true;
            }
            if b == cfg.idom[b] {
                return false;
            }
            b = cfg.idom[b];
        }
    };
    let mut precisam: HashSet<ValueId> = phis
        .iter()
        .filter(|(v, (_, entradas))| (devolvidos.contains(v) && !entradas.iter().any(|(_, op)| emprestada(op)))
            || raizes.get(v).is_some_and(|r| r.len() > 1)
            || raizes.get(v).into_iter().flatten().any(|(c, _)| matches!(c,
                Ownership::Borrowed { owner: OrigemOwner::Valor(o), .. } if !domina(*o, phis[v].0)))
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
            // A entrada de um Phi é usada no fim do seu predecessor.
            if let Instruction::Phi { incoming, .. } = i {
                for (pred, o) in incoming {
                    if let Operand::Val(x) = o { usos.entry(*x).or_default().push(*pred); }
                }
            } else {
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
    let pendentes: HashSet<BlockId> = pendentes_extras.iter().chain(p.tokens.pendencias.values()).copied().collect();
    let destinos = |b: &BasicBlock| super::super::cfg::sucessores(&b.terminator);
    let blocos: HashMap<BlockId, &BasicBlock> = f.blocks.iter().map(|b| (b.id, b)).collect();
    // (predecessor, bloco do Phi) → [(Phi, entrada)].
    let mut arestas: BTreeMap<(u32, u32), Vec<(ValueId, Operand)>> = BTreeMap::new();
    let mut ordem: Vec<_> = precisam.iter().copied().collect();
    ordem.sort_unstable_by_key(|v| v.0);
    // Entradas owned já transferidas a um Phi, por aresta: um token só vai
    // para um Phi; os outros Phis da mesma aresta ganham cópia.
    let mut tomados: HashSet<(BlockId, ValueId)> = HashSet::new();
    for v in ordem {
        let (bloco, entradas) = phis[&v];
        for (pred, op) in entradas.iter() {
            let copiar = match op {
                // Um Phi do próprio bloco lido pela aresta é o valor da volta
                // anterior (cópia paralela): ganha cópia antes da junção.
                // Um Phi owned ainda lido depois da junção também não cede
                // o token.
                Operand::Val(de) if phis.contains_key(de) => !precisam.contains(de) || (phis[de].0 == bloco && *de != v)
                    || usado_depois(de, bloco),
                Operand::Val(de) => classes.get(de) != Some(&Ownership::Owned) || usado_depois(de, bloco),
                _ => false,
            };
            let copiar = copiar || matches!(op, Operand::Val(de) if !tomados.insert((*pred, *de)));
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

    #[test]
    fn entrada_owned_lida_por_outro_phi_adiante_ganha_copia() {
        // `x = c ? a : b; ...; y = c ? a : b`: a e b seguem vivos até o
        // segundo Phi, então o primeiro não pode tomar os tokens deles.
        let val = |n| Operand::Val(ValueId(n));
        let chamada = || Instruction::CallStatic { symbol: "fonte".into(), args: vec![], ret_ty: Type::Ref };
        let ramo = |id, destino| BasicBlock { id: BlockId(id), instructions: vec![], terminator: Terminator::Branch(BlockId(destino)) };
        let mut f = Function {
            symbol: "dois".into(), name: "dois".into(), depuracao: None,
            params: vec![(ValueId(0), "c".into(), Type::I1)], return_ty: Type::Ref,
            blocks: vec![
                BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(1), chamada(), Type::Ref), (ValueId(2), chamada(), Type::Ref)],
                    terminator: Terminator::CondBranch { cond: val(0), then_block: BlockId(1), else_block: BlockId(2) } },
                ramo(1, 3), ramo(2, 3),
                BasicBlock { id: BlockId(3), instructions: vec![
                    (ValueId(3), Instruction::Phi { incoming: vec![(BlockId(1), val(1)), (BlockId(2), val(2))], ty: Type::Ref }, Type::Ref)],
                    terminator: Terminator::CondBranch { cond: val(0), then_block: BlockId(4), else_block: BlockId(5) } },
                ramo(4, 6), ramo(5, 6),
                BasicBlock { id: BlockId(6), instructions: vec![
                    (ValueId(4), Instruction::Phi { incoming: vec![(BlockId(4), val(1)), (BlockId(5), val(2))], ty: Type::Ref }, Type::Ref)],
                    terminator: Terminator::Return(Some(val(4))) },
            ],
        };
        let fonte = Function {
            symbol: "fonte".into(), name: "fonte".into(), depuracao: None, params: vec![], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }],
        };
        let resumos = vec![
            super::super::chamadas::resumo_provisorio(&f, false),
            super::super::chamadas::resumo_provisorio(&fonte, false),
        ];
        let indice = super::super::chamadas::IndiceFuncoesDart::novo(&resumos).unwrap();
        let p = PlanoFuncaoDart::default();
        // Só o primeiro Phi copia: depois do segundo, a e b não são mais lidos.
        assert_eq!(copiar_emprestimos(&mut f, &indice, &p).unwrap(), 2);
        let Instruction::Phi { incoming, .. } = &f.blocks[3].instructions[0].1 else { unreachable!() };
        assert!(incoming.iter().all(|(_, op)| !matches!(op, Operand::Val(ValueId(1 | 2)))));
        let Instruction::Phi { incoming, .. } = &f.blocks[6].instructions[0].1 else { unreachable!() };
        assert_eq!(incoming[0].1, val(1));
    }

    #[test]
    fn mesma_entrada_owned_em_dois_phis_da_aresta_copia_uma_vez() {
        // `x = c ? a : b; y = c ? a : outra`: na aresta de b1 o token de a
        // vai para x; y recebe a cópia.
        let val = |n| Operand::Val(ValueId(n));
        let chamada = || Instruction::CallStatic { symbol: "fonte".into(), args: vec![], ret_ty: Type::Ref };
        let ramo = |id| BasicBlock { id: BlockId(id), instructions: vec![], terminator: Terminator::Branch(BlockId(3)) };
        let mut f = Function {
            symbol: "par".into(), name: "par".into(), depuracao: None,
            params: vec![(ValueId(0), "c".into(), Type::I1)], return_ty: Type::Ref,
            blocks: vec![
                BasicBlock { id: BlockId(0), instructions: vec![(ValueId(1), chamada(), Type::Ref)],
                    terminator: Terminator::CondBranch { cond: val(0), then_block: BlockId(1), else_block: BlockId(2) } },
                ramo(1),
                BasicBlock { id: BlockId(2), instructions: vec![
                    (ValueId(2), chamada(), Type::Ref), (ValueId(5), chamada(), Type::Ref)],
                    terminator: Terminator::Branch(BlockId(3)) },
                BasicBlock { id: BlockId(3), instructions: vec![
                    (ValueId(3), Instruction::Phi { incoming: vec![(BlockId(1), val(1)), (BlockId(2), val(2))], ty: Type::Ref }, Type::Ref),
                    (ValueId(4), Instruction::Phi { incoming: vec![(BlockId(1), val(1)), (BlockId(2), val(5))], ty: Type::Ref }, Type::Ref)],
                    terminator: Terminator::Return(Some(val(4))) },
            ],
        };
        let fonte = Function {
            symbol: "fonte".into(), name: "fonte".into(), depuracao: None, params: vec![], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }],
        };
        let resumos = vec![
            super::super::chamadas::resumo_provisorio(&f, false),
            super::super::chamadas::resumo_provisorio(&fonte, false),
        ];
        let indice = super::super::chamadas::IndiceFuncoesDart::novo(&resumos).unwrap();
        let p = PlanoFuncaoDart::default();
        assert_eq!(copiar_emprestimos(&mut f, &indice, &p).unwrap(), 1);
        let Instruction::Phi { incoming, .. } = &f.blocks[3].instructions[0].1 else { unreachable!() };
        assert_eq!(incoming[0].1, val(1));
        let Instruction::Phi { incoming, .. } = &f.blocks[3].instructions[1].1 else { unreachable!() };
        assert!(!matches!(incoming[0].1, Operand::Val(ValueId(1))));
        assert_eq!(incoming[1].1, val(5));
    }

    #[test]
    fn phi_devolvido_de_triviais_por_outro_phi_ganha_copias() {
        // `c ? true : (d ? false : null)` devolvido: o Phi de dentro só junta
        // triviais, não é empréstimo; o de fora precisa de owner.
        let val = |n| Operand::Val(ValueId(n));
        let caixa = |b| Instruction::Box { op: Operand::Constant(Constant::Bool(b)), from: Type::I1 };
        let mut f = Function {
            symbol: "bools".into(), name: "bools".into(), depuracao: None,
            params: vec![(ValueId(0), "c".into(), Type::I1), (ValueId(1), "d".into(), Type::I1)],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock { id: BlockId(0), instructions: vec![],
                    terminator: Terminator::CondBranch { cond: val(0), then_block: BlockId(1), else_block: BlockId(2) } },
                BasicBlock { id: BlockId(1), instructions: vec![(ValueId(2), caixa(true), Type::Ref)],
                    terminator: Terminator::Branch(BlockId(6)) },
                BasicBlock { id: BlockId(2), instructions: vec![],
                    terminator: Terminator::CondBranch { cond: val(1), then_block: BlockId(3), else_block: BlockId(4) } },
                BasicBlock { id: BlockId(3), instructions: vec![(ValueId(3), caixa(false), Type::Ref)],
                    terminator: Terminator::Branch(BlockId(5)) },
                BasicBlock { id: BlockId(4), instructions: vec![], terminator: Terminator::Branch(BlockId(5)) },
                BasicBlock { id: BlockId(5), instructions: vec![
                    (ValueId(4), Instruction::Phi { incoming: vec![(BlockId(3), val(3)), (BlockId(4), Operand::Constant(Constant::Null))], ty: Type::Ref }, Type::Ref)],
                    terminator: Terminator::Branch(BlockId(6)) },
                BasicBlock { id: BlockId(6), instructions: vec![
                    (ValueId(5), Instruction::Phi { incoming: vec![(BlockId(1), val(2)), (BlockId(5), val(4))], ty: Type::Ref }, Type::Ref)],
                    terminator: Terminator::Return(Some(val(5))) },
            ],
        };
        let resumos = vec![super::super::chamadas::resumo_provisorio(&f, false)];
        let indice = super::super::chamadas::IndiceFuncoesDart::novo(&resumos).unwrap();
        let mut p = PlanoFuncaoDart::default();
        assert_eq!(copiar_emprestimos(&mut f, &indice, &p).unwrap(), 2);
        super::super::chamadas::produzir_chamadas_e_instrucoes_dart(&f, &indice, &mut p.classes, &mut p.tokens).unwrap();
        assert_eq!(p.classes[&ValueId(5)], Ownership::Owned);
        assert_eq!(p.classes[&ValueId(4)], Ownership::Trivial);
    }

    #[test]
    fn phi_de_laco_com_owner_definido_no_corpo_ganha_copia() {
        // `var prox; while (c) { e = fonte(); prox = e.campo; }`: o owner do
        // empréstimo nasce depois do cabeçalho, que não pode tomá-lo emprestado.
        let val = |n| Operand::Val(ValueId(n));
        let mut f = Function {
            symbol: "laco".into(), name: "laco".into(), depuracao: None,
            params: vec![(ValueId(0), "c".into(), Type::I1)], return_ty: Type::Void,
            blocks: vec![
                BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::Branch(BlockId(1)) },
                BasicBlock { id: BlockId(1), instructions: vec![
                    (ValueId(2), Instruction::Phi { incoming: vec![(BlockId(0), Operand::Constant(Constant::Null)), (BlockId(3), val(5))], ty: Type::Ref }, Type::Ref)],
                    terminator: Terminator::CondBranch { cond: val(0), then_block: BlockId(2), else_block: BlockId(4) } },
                BasicBlock { id: BlockId(2), instructions: vec![
                    (ValueId(4), Instruction::CallStatic { symbol: "fonte".into(), args: vec![], ret_ty: Type::Ref }, Type::Ref),
                    (ValueId(5), Instruction::GetField { object: val(4), index: 0 }, Type::Ref)],
                    terminator: Terminator::Branch(BlockId(3)) },
                BasicBlock { id: BlockId(3), instructions: vec![], terminator: Terminator::Branch(BlockId(1)) },
                BasicBlock { id: BlockId(4), instructions: vec![], terminator: Terminator::Return(None) },
            ],
        };
        let fonte = Function {
            symbol: "fonte".into(), name: "fonte".into(), depuracao: None, params: vec![], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }],
        };
        let resumos = vec![
            super::super::chamadas::resumo_provisorio(&f, false),
            super::super::chamadas::resumo_provisorio(&fonte, false),
        ];
        let indice = super::super::chamadas::IndiceFuncoesDart::novo(&resumos).unwrap();
        let mut p = PlanoFuncaoDart::default();
        p.classes.insert(ValueId(5), Ownership::Borrowed { owner: OrigemOwner::Valor(ValueId(4)), escopo: 0 });
        assert_eq!(copiar_emprestimos(&mut f, &indice, &p).unwrap(), 1);
        let Instruction::Phi { incoming, .. } = &f.blocks[1].instructions[0].1 else { unreachable!() };
        let (aresta, copia) = incoming[1].clone();
        assert_ne!(aresta, BlockId(3));
        let bloco = f.blocks.iter().find(|b| b.id == aresta).unwrap();
        assert!(matches!(&bloco.instructions[..], [(v, Instruction::ArcCopy { value }, Type::Ref)]
            if Operand::Val(*v) == copia && *value == val(5)));
    }
}
