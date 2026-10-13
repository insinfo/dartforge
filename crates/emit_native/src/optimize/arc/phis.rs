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
/// Classifica as definições sem resolver Phis (`calls::classify_without_phis`);
/// se isso falhar, não altera nada e deixa o erro para a produção completa.
/// Um Phi precisa de owner quando é devolvido (a convenção Owned do retorno),
/// quando junta empréstimos de raízes diferentes, ou quando alguma entrada é
/// Owned, diretamente ou por outro Phi que precise. As demais entradas dele — empréstimos, triviais e
/// Phis emprestados — são copiadas na aresta; null não.
///
/// # Erros
/// IDs SSA ou de bloco esgotados.
pub(super) fn copy_borrows(
    f: &mut Function,
    index: &super::calls::DartFunctionIndex<'_>,
    p: &DartFunctionPlan,
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
    if super::calls::classify_without_phis(f, index, &mut classes, &mut tokens).is_err() {
        return Ok(0);
    }
    let pending_set: HashSet<BlockId> = tokens.pendings.values().copied().collect();
    copy(f, p, &classes, &pending_set)
}

/// [`copy_borrows`] com as classes das definições já conhecidas (sem
/// as dos Phis): o caminho do `protect`, depois de promover empréstimos.
///
/// # Erros
/// IDs SSA ou de bloco esgotados.
pub(super) fn copy_with_classes(
    f: &mut Function,
    p: &DartFunctionPlan,
    classes: &HashMap<ValueId, Ownership>,
) -> Result<usize, String> {
    copy(f, p, classes, &HashSet::new())
}

fn copy(
    f: &mut Function,
    p: &DartFunctionPlan,
    classes: &HashMap<ValueId, Ownership>,
    extra_pending: &HashSet<BlockId>,
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
    let returned_set: HashSet<ValueId> = f.blocks.iter().filter_map(|b| match &b.terminator {
        Terminator::Return(Some(Operand::Val(v))) if phis.contains_key(v) => Some(*v),
        _ => None,
    }).collect();
    // Os contratos de empréstimo que chegam a cada Phi, com a entrada que
    // primeiro trouxe cada um: o produtor só deixa um Phi emprestado quando
    // todas as entradas diretas têm a mesma classe crua (owner e escopo); um
    // Phi interno entra com a classe que ele recebe (o empréstimo do
    // chamador preso à entrada de origem). Com dois contratos o Phi precisa
    // de owner — mesmo que os owners sejam aliases de uma raiz comum.
    let mut roots: HashMap<ValueId, Vec<(Ownership, ValueId)>> = HashMap::new();
    loop {
        let mut changed = false;
        for (v, (_, entries)) in &phis {
            let mut current = roots.get(v).cloned().unwrap_or_default();
            let before = current.len();
            let mut join = |c: Ownership, de: ValueId| if !current.iter().any(|(x, _)| *x == c) { current.push((c, de)) };
            for (_, op) in entries.iter() {
                let Operand::Val(de) = op else { continue };
                if phis.contains_key(de) {
                    // Com dois ou mais, o Phi interno precisa de owner e este
                    // também (entrada que precisa); basta propagar todos.
                    let internals = roots.get(de).cloned().unwrap_or_default();
                    for (c, origin) in internals {
                        let c = match (c, roots[de].len()) {
                            (Ownership::Borrowed { owner: OwnerOrigin::Caller, scope }, 1) =>
                                Ownership::Borrowed { owner: OwnerOrigin::Value(origin), scope },
                            (c, _) => c,
                        };
                        join(c, *de);
                    }
                } else if let Some(c @ Ownership::Borrowed { .. }) = classes.get(de) {
                    join(*c, *de);
                }
            }
            if current.len() != before || !roots.contains_key(v) {
                roots.insert(*v, current);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // O Phi devolvido com alguma entrada emprestada (direta ou por um Phi com
    // raiz de empréstimo) segue o desenho do keepalive (empréstimo até a
    // cópia do retorno); só o de entradas triviais (literais, inclusive por
    // Phis só deles) ou owners precisa das cópias aqui.
    let borrowed = |op: &Operand| matches!(op, Operand::Val(de)
        if matches!(classes.get(de), Some(Ownership::Borrowed { .. }))
            || roots.get(de).is_some_and(|r| !r.is_empty()));
    // O owner de um empréstimo precisa dominar o Phi (ARC003): o laço que
    // carrega um alias de um valor definido depois do cabeçalho precisa de
    // owner próprio. Phis do mesmo bloco são simultâneos.
    let cfg = super::super::cfg::Cfg::new(f);
    let index: HashMap<BlockId, usize> = f.blocks.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
    let definitions: HashMap<ValueId, (usize, bool)> = f.blocks.iter().enumerate()
        .flat_map(|(i, b)| b.instructions.iter().map(move |(v, inst, _)| (*v, (i, matches!(inst, Instruction::Phi { .. })))))
        .collect();
    let dominates = |owner: ValueId, block: BlockId| {
        let Some(&(o, owner_phi)) = definitions.get(&owner) else { return true };
        let mut b = index[&block];
        if !cfg.reachable(b) {
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
    let mut need: HashSet<ValueId> = phis
        .iter()
        .filter(|(v, (_, entries))| (returned_set.contains(v) && !entries.iter().any(|(_, op)| borrowed(op)))
            || roots.get(v).is_some_and(|r| r.len() > 1)
            || roots.get(v).into_iter().flatten().any(|(c, _)| matches!(c,
                Ownership::Borrowed { owner: OwnerOrigin::Value(o), .. } if !dominates(*o, phis[v].0)))
            || entries.iter().any(|(_, op)| matches!(op, Operand::Val(de) if owned(de))))
        .map(|(v, _)| *v)
        .collect();
    loop {
        let new_set: Vec<_> = phis
            .iter()
            .filter(|(v, (_, entries))| {
                !need.contains(v)
                    && entries.iter().any(|(_, op)| matches!(op, Operand::Val(de) if need.contains(de)))
            })
            .map(|(v, _)| *v)
            .collect();
        if new_set.is_empty() {
            break;
        }
        need.extend(new_set);
    }
    // Uma entrada owned usada depois da junção (no bloco do Phi ou adiante)
    // não pode ter o token transferido ao Phi: ela também ganha a cópia.
    let mut uses: HashMap<ValueId, Vec<BlockId>> = HashMap::new();
    for b in &f.blocks {
        for (_, i, _) in &b.instructions {
            // A entrada de um Phi é usada no fim do seu predecessor.
            if let Instruction::Phi { incoming, .. } = i {
                for (pred, o) in incoming {
                    if let Operand::Val(x) = o { uses.entry(*x).or_default().push(*pred); }
                }
            } else {
                super::super::operands::operands(i, &mut |o| if let Operand::Val(x) = o { uses.entry(*x).or_default().push(b.id); });
            }
        }
        super::super::operands::terminator_operands(&b.terminator, &mut |o| if let Operand::Val(x) = o { uses.entry(*x).or_default().push(b.id); });
    }
    let successors: HashMap<BlockId, Vec<BlockId>> = f.blocks.iter().map(|b| (b.id, super::super::cfg::successors(&b.terminator))).collect();
    let definition: HashMap<ValueId, BlockId> = f.blocks.iter()
        .flat_map(|b| b.instructions.iter().map(move |(v, _, _)| (*v, b.id))).collect();
    // `de` vivo na entrada do bloco do Phi: um uso alcançável dali sem
    // passar pela (re)definição dele — num laço, o que vem depois da
    // definição já é o valor da volta seguinte.
    let used_after = |de: &ValueId, block: BlockId| {
        let Some(bs) = uses.get(de) else { return false };
        let def = definition.get(de).copied();
        let mut seen_set = HashSet::from([block]);
        let mut queue = vec![block];
        while let Some(b) = queue.pop() {
            if Some(b) == def {
                continue;
            }
            if bs.contains(&b) {
                return true;
            }
            for s in successors.get(&b).into_iter().flatten() {
                if seen_set.insert(*s) {
                    queue.push(*s);
                }
            }
        }
        false
    };
    let pending_set: HashSet<BlockId> = extra_pending.iter().chain(p.tokens.pendings.values()).copied().collect();
    let dests = |b: &BasicBlock| super::super::cfg::successors(&b.terminator);
    let blocks: HashMap<BlockId, &BasicBlock> = f.blocks.iter().map(|b| (b.id, b)).collect();
    // (predecessor, bloco do Phi) → [(Phi, entrada)].
    let mut edges: BTreeMap<(u32, u32), Vec<(ValueId, Operand)>> = BTreeMap::new();
    let mut order: Vec<_> = need.iter().copied().collect();
    order.sort_unstable_by_key(|v| v.0);
    // Entradas owned já transferidas a um Phi, por aresta: um token só vai
    // para um Phi; os outros Phis da mesma aresta ganham cópia.
    let mut taken: HashSet<(BlockId, ValueId)> = HashSet::new();
    for v in order {
        let (block, entries) = phis[&v];
        for (pred, op) in entries.iter() {
            let copy = match op {
                // Um Phi do próprio bloco lido pela aresta é o valor da volta
                // anterior (cópia paralela): ganha cópia antes da junção.
                // Um Phi owned ainda lido depois da junção também não cede
                // o token.
                Operand::Val(de) if phis.contains_key(de) => !need.contains(de) || (phis[de].0 == block && *de != v)
                    || used_after(de, block),
                Operand::Val(de) => classes.get(de) != Some(&Ownership::Owned) || used_after(de, block),
                _ => false,
            };
            let copy = copy || matches!(op, Operand::Val(de) if !taken.insert((*pred, *de)));
            let admissible = !p.tables.landing_pads.contains(&block)
                && !pending_set.contains(&block)
                && !p.scopes.edges.contains_key(&(*pred, block))
                && blocks.get(pred).is_some_and(|b| {
                    matches!(b.terminator, Terminator::Branch(_) | Terminator::CondBranch { .. })
                        && dests(b).contains(&block)
                });
            if copy && admissible {
                edges.entry((pred.0, block.0)).or_default().push((v, op.clone()));
            }
        }
    }
    if edges.is_empty() {
        return Ok(0);
    }
    let mut value = super::edges::next_val(f, p);
    let mut next_block = f
        .blocks
        .iter()
        .map(|b| b.id.0)
        .chain(p.tables.exits.keys().map(|b| b.0))
        .chain(p.tables.landing_pads.iter().map(|b| b.0))
        .chain(p.tables.invocations.values().map(|b| b.0))
        .chain(p.tokens.pendings.values().map(|b| b.0))
        .chain(p.scopes.exits.keys().map(|b| b.0))
        .chain(p.scopes.edges.keys().flat_map(|(a, b)| [a.0, b.0]))
        .max()
        .map_or(0, |b| u64::from(b) + 1);
    let mut value_id = || -> Result<ValueId, String> {
        let id = u32::try_from(value).map_err(|_| "IDs SSA esgotados ao copiar entradas de Phi")?;
        value += 1;
        Ok(ValueId(id))
    };
    let mut copies = 0;
    for ((pred, block), entries) in edges {
        let (pred, block) = (BlockId(pred), BlockId(block));
        let edge = BlockId(u32::try_from(next_block).map_err(|_| "IDs de bloco esgotados ao copiar entradas de Phi")?);
        next_block += 1;
        let mut instructions = Vec::new();
        let mut swaps = HashMap::new();
        for (phi, op) in entries {
            let v = value_id()?;
            instructions.push((v, Instruction::ArcCopy { value: op }, Type::Ref));
            swaps.insert(phi, v);
            copies += 1;
        }
        let b = f.blocks.iter_mut().find(|b| b.id == pred).expect("predecessor conferido");
        match &mut b.terminator {
            Terminator::Branch(dest) => *dest = edge,
            Terminator::CondBranch { then_block, else_block, .. } => {
                for dest in [then_block, else_block] {
                    if *dest == block {
                        *dest = edge;
                    }
                }
            }
            _ => unreachable!("terminador conferido"),
        }
        // Todos os Phis do bloco passam a chegar pela aresta nova.
        let alvo = f.blocks.iter_mut().find(|b| b.id == block).expect("bloco do Phi");
        for (v, i, _) in &mut alvo.instructions {
            if let Instruction::Phi { incoming, .. } = i {
                for (de, op) in incoming.iter_mut() {
                    if *de == pred {
                        *de = edge;
                        if let Some(copy_inst) = swaps.get(v) {
                            *op = Operand::Val(*copy_inst);
                        }
                    }
                }
            }
        }
        f.blocks.push(BasicBlock { id: edge, instructions: instructions, terminator: Terminator::Branch(block) });
    }
    Ok(copies)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `c ? this.field : call()`: o campo emprestado e o owner da chamada.
    fn mixed() -> (Function, Function) {
        let val = |n| Operand::Val(ValueId(n));
        let f = Function {
            symbol: "misto".into(), name: "misto".into(), debug_info: None,
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
        let source = Function {
            symbol: "fonte".into(), name: "fonte".into(), debug_info: None, params: vec![], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }],
        };
        (f, source)
    }

    #[test]
    fn mixed_phi_copies_borrow_on_edge_and_producer_accepts() {
        {
            let (mut f, source) = mixed();
            let summaries = vec![
                super::super::calls::provisional_summary(&f, false),
                super::super::calls::provisional_summary(&source, false),
            ];
            let index = super::super::calls::DartFunctionIndex::new(&summaries).unwrap();
            let mut p = DartFunctionPlan::default();
            p.classes.insert(ValueId(3), Ownership::Borrowed { owner: OwnerOrigin::Value(ValueId(0)), scope: 0 });
            let mut without_copy = p.clone();
            let error = super::super::calls::produce_dart_calls_and_instructions(&f, &index, &mut without_copy.classes, &mut without_copy.tokens)
                .unwrap_err();
            assert!(error.contains("copie o empréstimo na aresta"), "{error}");
            assert_eq!(copy_borrows(&mut f, &index, &p).unwrap(), 1);
            let edge = f.blocks.last().unwrap();
            assert!(matches!(edge.instructions.last(), Some((_, Instruction::ArcCopy { .. }, Type::Ref))));
            assert!(matches!(edge.terminator, Terminator::Branch(BlockId(3))));
            assert!(matches!(f.blocks[1].terminator, Terminator::Branch(b) if b == edge.id));
            let Instruction::Phi { incoming, .. } = &f.blocks[3].instructions[0].1 else { unreachable!() };
            assert_eq!(incoming[0], (edge.id, Operand::Val(edge.instructions.last().unwrap().0)));
            assert_eq!(incoming[1], (BlockId(2), Operand::Val(ValueId(4))));
            super::super::calls::produce_dart_calls_and_instructions(&f, &index, &mut p.classes, &mut p.tokens).unwrap();
            assert_eq!(p.classes[&ValueId(5)], Ownership::Owned);
            // Idempotente: nada mais a copiar.
            assert_eq!(copy_borrows(&mut f, &index, &p).unwrap(), 0);
        }
    }

    #[test]
    fn borrow_only_phi_or_landing_pad_edge_stays_intact() {
        let (f0, source) = mixed();
        let summaries = vec![
            super::super::calls::provisional_summary(&f0, false),
            super::super::calls::provisional_summary(&source, false),
        ];
        let index = super::super::calls::DartFunctionIndex::new(&summaries).unwrap();
        let mut p = DartFunctionPlan::default();
        p.classes.insert(ValueId(3), Ownership::Borrowed { owner: OwnerOrigin::Value(ValueId(0)), scope: 0 });
        // O bloco do Phi é um pouso: a aresta não é dividida.
        let mut f = f0.clone();
        p.tables.landing_pads.insert(BlockId(3));
        assert_eq!(copy_borrows(&mut f, &index, &p).unwrap(), 0);
        assert_eq!(format!("{f:?}"), format!("{f0:?}"));
        p.tables.landing_pads.clear();
        // Sem owner entre as entradas e sem ser devolvido, o Phi continua
        // emprestado.
        let mut f = f0.clone();
        let Instruction::Phi { incoming, .. } = &mut f.blocks[3].instructions[0].1 else { unreachable!() };
        incoming[1].1 = Operand::Constant(Constant::Null);
        f.blocks[3].terminator = Terminator::Return(Some(Operand::Constant(Constant::Null)));
        let before = format!("{f:?}");
        assert_eq!(copy_borrows(&mut f, &index, &p).unwrap(), 0);
        assert_eq!(format!("{f:?}"), before);
        // Devolvido com entrada emprestada, segue o keepalive: nada aqui.
        f.blocks[3].terminator = Terminator::Return(Some(Operand::Val(ValueId(5))));
        assert_eq!(copy_borrows(&mut f, &index, &p).unwrap(), 0);
    }

    #[test]
    fn owned_entry_read_by_later_phi_gets_copy() {
        // `x = c ? a : b; ...; y = c ? a : b`: a e b seguem vivos até o
        // segundo Phi, então o primeiro não pode tomar os tokens deles.
        let val = |n| Operand::Val(ValueId(n));
        let call = || Instruction::CallStatic { symbol: "fonte".into(), args: vec![], ret_ty: Type::Ref };
        let branch = |id, dest| BasicBlock { id: BlockId(id), instructions: vec![], terminator: Terminator::Branch(BlockId(dest)) };
        let mut f = Function {
            symbol: "dois".into(), name: "dois".into(), debug_info: None,
            params: vec![(ValueId(0), "c".into(), Type::I1)], return_ty: Type::Ref,
            blocks: vec![
                BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(1), call(), Type::Ref), (ValueId(2), call(), Type::Ref)],
                    terminator: Terminator::CondBranch { cond: val(0), then_block: BlockId(1), else_block: BlockId(2) } },
                branch(1, 3), branch(2, 3),
                BasicBlock { id: BlockId(3), instructions: vec![
                    (ValueId(3), Instruction::Phi { incoming: vec![(BlockId(1), val(1)), (BlockId(2), val(2))], ty: Type::Ref }, Type::Ref)],
                    terminator: Terminator::CondBranch { cond: val(0), then_block: BlockId(4), else_block: BlockId(5) } },
                branch(4, 6), branch(5, 6),
                BasicBlock { id: BlockId(6), instructions: vec![
                    (ValueId(4), Instruction::Phi { incoming: vec![(BlockId(4), val(1)), (BlockId(5), val(2))], ty: Type::Ref }, Type::Ref)],
                    terminator: Terminator::Return(Some(val(4))) },
            ],
        };
        let source = Function {
            symbol: "fonte".into(), name: "fonte".into(), debug_info: None, params: vec![], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }],
        };
        let summaries = vec![
            super::super::calls::provisional_summary(&f, false),
            super::super::calls::provisional_summary(&source, false),
        ];
        let index = super::super::calls::DartFunctionIndex::new(&summaries).unwrap();
        let p = DartFunctionPlan::default();
        // Só o primeiro Phi copia: depois do segundo, a e b não são mais lidos.
        assert_eq!(copy_borrows(&mut f, &index, &p).unwrap(), 2);
        let Instruction::Phi { incoming, .. } = &f.blocks[3].instructions[0].1 else { unreachable!() };
        assert!(incoming.iter().all(|(_, op)| !matches!(op, Operand::Val(ValueId(1 | 2)))));
        let Instruction::Phi { incoming, .. } = &f.blocks[6].instructions[0].1 else { unreachable!() };
        assert_eq!(incoming[0].1, val(1));
    }

    #[test]
    fn same_owned_entry_in_two_edge_phis_copies_once() {
        // `x = c ? a : b; y = c ? a : another`: na aresta de b1 o token de a
        // vai para x; y recebe a cópia.
        let val = |n| Operand::Val(ValueId(n));
        let call = || Instruction::CallStatic { symbol: "fonte".into(), args: vec![], ret_ty: Type::Ref };
        let branch = |id| BasicBlock { id: BlockId(id), instructions: vec![], terminator: Terminator::Branch(BlockId(3)) };
        let mut f = Function {
            symbol: "par".into(), name: "par".into(), debug_info: None,
            params: vec![(ValueId(0), "c".into(), Type::I1)], return_ty: Type::Ref,
            blocks: vec![
                BasicBlock { id: BlockId(0), instructions: vec![(ValueId(1), call(), Type::Ref)],
                    terminator: Terminator::CondBranch { cond: val(0), then_block: BlockId(1), else_block: BlockId(2) } },
                branch(1),
                BasicBlock { id: BlockId(2), instructions: vec![
                    (ValueId(2), call(), Type::Ref), (ValueId(5), call(), Type::Ref)],
                    terminator: Terminator::Branch(BlockId(3)) },
                BasicBlock { id: BlockId(3), instructions: vec![
                    (ValueId(3), Instruction::Phi { incoming: vec![(BlockId(1), val(1)), (BlockId(2), val(2))], ty: Type::Ref }, Type::Ref),
                    (ValueId(4), Instruction::Phi { incoming: vec![(BlockId(1), val(1)), (BlockId(2), val(5))], ty: Type::Ref }, Type::Ref)],
                    terminator: Terminator::Return(Some(val(4))) },
            ],
        };
        let source = Function {
            symbol: "fonte".into(), name: "fonte".into(), debug_info: None, params: vec![], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }],
        };
        let summaries = vec![
            super::super::calls::provisional_summary(&f, false),
            super::super::calls::provisional_summary(&source, false),
        ];
        let index = super::super::calls::DartFunctionIndex::new(&summaries).unwrap();
        let p = DartFunctionPlan::default();
        assert_eq!(copy_borrows(&mut f, &index, &p).unwrap(), 1);
        let Instruction::Phi { incoming, .. } = &f.blocks[3].instructions[0].1 else { unreachable!() };
        assert_eq!(incoming[0].1, val(1));
        let Instruction::Phi { incoming, .. } = &f.blocks[3].instructions[1].1 else { unreachable!() };
        assert!(!matches!(incoming[0].1, Operand::Val(ValueId(1))));
        assert_eq!(incoming[1].1, val(5));
    }

    #[test]
    fn phi_returned_from_trivials_through_other_phi_gets_copies() {
        // `c ? true : (d ? false : null)` devolvido: o Phi de dentro só junta
        // triviais, não é empréstimo; o de fora precisa de owner.
        let val = |n| Operand::Val(ValueId(n));
        let boxed = |b| Instruction::Box { op: Operand::Constant(Constant::Bool(b)), from: Type::I1 };
        let mut f = Function {
            symbol: "bools".into(), name: "bools".into(), debug_info: None,
            params: vec![(ValueId(0), "c".into(), Type::I1), (ValueId(1), "d".into(), Type::I1)],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock { id: BlockId(0), instructions: vec![],
                    terminator: Terminator::CondBranch { cond: val(0), then_block: BlockId(1), else_block: BlockId(2) } },
                BasicBlock { id: BlockId(1), instructions: vec![(ValueId(2), boxed(true), Type::Ref)],
                    terminator: Terminator::Branch(BlockId(6)) },
                BasicBlock { id: BlockId(2), instructions: vec![],
                    terminator: Terminator::CondBranch { cond: val(1), then_block: BlockId(3), else_block: BlockId(4) } },
                BasicBlock { id: BlockId(3), instructions: vec![(ValueId(3), boxed(false), Type::Ref)],
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
        let summaries = vec![super::super::calls::provisional_summary(&f, false)];
        let index = super::super::calls::DartFunctionIndex::new(&summaries).unwrap();
        let mut p = DartFunctionPlan::default();
        assert_eq!(copy_borrows(&mut f, &index, &p).unwrap(), 2);
        super::super::calls::produce_dart_calls_and_instructions(&f, &index, &mut p.classes, &mut p.tokens).unwrap();
        assert_eq!(p.classes[&ValueId(5)], Ownership::Owned);
        assert_eq!(p.classes[&ValueId(4)], Ownership::Trivial);
    }

    #[test]
    fn loop_phi_with_owner_defined_in_body_gets_copy() {
        // `var next; while (c) { e = source(); next = e.field; }`: o owner do
        // empréstimo nasce depois do cabeçalho, que não pode tomá-lo emprestado.
        let val = |n| Operand::Val(ValueId(n));
        let mut f = Function {
            symbol: "laco".into(), name: "laco".into(), debug_info: None,
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
        let source = Function {
            symbol: "fonte".into(), name: "fonte".into(), debug_info: None, params: vec![], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }],
        };
        let summaries = vec![
            super::super::calls::provisional_summary(&f, false),
            super::super::calls::provisional_summary(&source, false),
        ];
        let index = super::super::calls::DartFunctionIndex::new(&summaries).unwrap();
        let mut p = DartFunctionPlan::default();
        p.classes.insert(ValueId(5), Ownership::Borrowed { owner: OwnerOrigin::Value(ValueId(4)), scope: 0 });
        assert_eq!(copy_borrows(&mut f, &index, &p).unwrap(), 1);
        let Instruction::Phi { incoming, .. } = &f.blocks[1].instructions[0].1 else { unreachable!() };
        let (edge, copy_inst) = incoming[1].clone();
        assert_ne!(edge, BlockId(3));
        let block = f.blocks.iter().find(|b| b.id == edge).unwrap();
        assert!(matches!(&block.instructions[..], [(v, Instruction::ArcCopy { value }, Type::Ref)]
            if Operand::Val(*v) == copy_inst && *value == val(5)));
    }
}
