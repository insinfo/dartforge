//! Promoção dos locais (`Alloca` lido e gravado só por `Load`/`Store`) a
//! valores SSA, com `phi` nas fronteiras de dominância das gravações
//! (Cytron et al.). O LLVM faria o mesmo depois, mas os passes da HIR
//! (substituição escalar, inlining) precisam ver os valores; e um local
//! `Ref` promovido deixa de ter slot fixo no quadro de raízes (a
//! vivacidade decide).

use super::cfg::Cfg;
use super::operandos::*;
use crate::hir::*;
use std::collections::{HashMap, HashSet};

/// Os tipos são compatíveis para trocar a leitura pelo valor gravado? O
/// emissor converte entre escalares pela representação; um `Ref` (raiz
/// do coletor) só troca com `Ref`.
pub fn compativel(local: Type, valor: Type) -> bool {
    local == valor || (local != Type::Ref && valor != Type::Ref && !local.e_vetor() && !valor.e_vetor())
}

/// Promove os locais promovíveis de `func`. Devolve se mudou algo.
pub fn promover(func: &mut Function) -> bool {
    let tipos = tipos_da_funcao(func);
    // Os candidatos e os blocos que gravam cada um.
    let mut locais: HashMap<ValueId, Type> = HashMap::new();
    for b in &func.blocks {
        for (v, inst, _) in &b.instructions {
            if let Instruction::Alloca(t) = inst
                && !t.e_vetor()
                && !matches!(t, Type::Void | Type::Ptr)
            {
                locais.insert(*v, *t);
            }
        }
    }
    if locais.is_empty() {
        return false;
    }
    let mut recusados: HashSet<ValueId> = HashSet::new();
    for b in &func.blocks {
        for (_, inst, _) in &b.instructions {
            match inst {
                Instruction::Load { ptr: Operand::Val(_), .. } => {}
                Instruction::Store { ptr: Operand::Val(p), val } => {
                    if let Some(&t) = locais.get(p) {
                        let ok = match val {
                            Operand::Val(v) if v == p => false,
                            _ => tipo_do_operando(val, &tipos).is_some_and(|tv| compativel(t, tv)),
                        };
                        if !ok {
                            recusados.insert(*p);
                        }
                    }
                    if let Operand::Val(v) = val
                        && locais.contains_key(v)
                    {
                        recusados.insert(*v);
                    }
                }
                outra => operandos(outra, &mut |o| {
                    if let Operand::Val(v) = o
                        && locais.contains_key(v)
                    {
                        recusados.insert(*v);
                    }
                }),
            }
        }
        operandos_do_terminador(&b.terminator, &mut |o| {
            if let Operand::Val(v) = o
                && locais.contains_key(v)
            {
                recusados.insert(*v);
            }
        });
    }
    locais.retain(|v, _| !recusados.contains(v));
    if locais.is_empty() {
        return false;
    }

    let cfg = Cfg::novo(func);
    let n = func.blocks.len();
    let df = cfg.fronteiras();
    let (mut prox_valor, _) = maiores_ids(func);

    // Os `phi` de cada local, por bloco.
    let mut ordem_locais: Vec<ValueId> = locais.keys().copied().collect();
    ordem_locais.sort_by_key(|v| v.0);
    let mut phis: Vec<Vec<(ValueId, ValueId)>> = vec![Vec::new(); n]; // (local, phi)
    for &local in &ordem_locais {
        let mut defs: Vec<usize> = Vec::new();
        for (i, b) in func.blocks.iter().enumerate() {
            if cfg.alcancavel(i)
                && b.instructions.iter().any(|(_, inst, _)| matches!(inst, Instruction::Store { ptr: Operand::Val(p), .. } if *p == local))
            {
                defs.push(i);
            }
        }
        let mut tem_phi = vec![false; n];
        let mut fila = defs.clone();
        while let Some(b) = fila.pop() {
            for &y in &df[b] {
                if !tem_phi[y] {
                    tem_phi[y] = true;
                    prox_valor += 1;
                    phis[y].push((local, ValueId(prox_valor)));
                    fila.push(y);
                }
            }
        }
    }

    // Renomeação pela árvore de dominância.
    let filhos = cfg.filhos();
    let mut troca: HashMap<ValueId, Operand> = HashMap::new();
    let mut entradas: Vec<Vec<(ValueId, BlockId, Operand)>> = vec![Vec::new(); n]; // (phi, origem, valor)
    let mut pilhas: HashMap<ValueId, Vec<Operand>> = ordem_locais.iter().map(|&l| (l, Vec::new())).collect();
    let ids: Vec<BlockId> = func.blocks.iter().map(|b| b.id).collect();
    // Pilha explícita: (bloco, fase, quantos empilhados por local).
    enum Passo {
        Entrar(usize),
        Sair(Vec<ValueId>),
    }
    let mut trabalho = vec![Passo::Entrar(0)];
    while let Some(p) = trabalho.pop() {
        match p {
            Passo::Sair(empilhados) => {
                for l in empilhados {
                    pilhas.get_mut(&l).expect("local").pop();
                }
            }
            Passo::Entrar(b) => {
                let mut empilhados = Vec::new();
                for &(l, phi) in &phis[b] {
                    pilhas.get_mut(&l).expect("local").push(Operand::Val(phi));
                    empilhados.push(l);
                }
                for (v, inst, _) in &func.blocks[b].instructions {
                    match inst {
                        Instruction::Load { ptr: Operand::Val(p), .. } if locais.contains_key(p) => {
                            let atual = pilhas[p].last().cloned().unwrap_or_else(|| constante_padrao(locais[p]));
                            troca.insert(*v, atual);
                        }
                        Instruction::Store { ptr: Operand::Val(p), val } if locais.contains_key(p) => {
                            pilhas.get_mut(p).expect("local").push(val.clone());
                            empilhados.push(*p);
                        }
                        _ => {}
                    }
                }
                for &s in &cfg.sucessores[b] {
                    for &(l, phi) in &phis[s] {
                        let atual = pilhas[&l].last().cloned().unwrap_or_else(|| constante_padrao(locais[&l]));
                        entradas[s].push((phi, ids[b], atual));
                    }
                }
                trabalho.push(Passo::Sair(empilhados));
                for &f in filhos[b].iter().rev() {
                    trabalho.push(Passo::Entrar(f));
                }
            }
        }
    }

    // Leituras em blocos inalcançáveis: a constante neutra.
    for (i, b) in func.blocks.iter().enumerate() {
        if cfg.alcancavel(i) {
            continue;
        }
        for (v, inst, _) in &b.instructions {
            if let Instruction::Load { ptr: Operand::Val(p), .. } = inst
                && let Some(&t) = locais.get(p)
            {
                troca.insert(*v, constante_padrao(t));
            }
        }
    }

    // Monta os `phi` e tira locais, leituras e gravações.
    for (i, b) in func.blocks.iter_mut().enumerate() {
        b.instructions.retain(|(v, inst, _)| match inst {
            Instruction::Alloca(_) => !locais.contains_key(v),
            Instruction::Load { ptr: Operand::Val(p), .. } | Instruction::Store { ptr: Operand::Val(p), .. } => {
                !locais.contains_key(p)
            }
            _ => true,
        });
        let novos: Vec<(ValueId, Instruction, Type)> = phis[i]
            .iter()
            .map(|&(l, phi)| {
                let incoming = entradas[i].iter().filter(|(p, _, _)| *p == phi).map(|(_, o, v)| (*o, v.clone())).collect();
                (phi, Instruction::Phi { incoming, ty: locais[&l] }, locais[&l])
            })
            .collect();
        b.instructions.splice(0..0, novos);
    }
    let resolver = |mut o: Operand| {
        let mut passos = 0;
        while let Operand::Val(v) = &o {
            match troca.get(v) {
                Some(n) if passos < 1_000_000 => {
                    o = n.clone();
                    passos += 1;
                }
                _ => break,
            }
        }
        o
    };
    let resolvidos: HashMap<ValueId, Operand> = troca.keys().map(|&k| (k, resolver(Operand::Val(k)))).collect();
    substituir(func, &|v| resolvidos.get(&v).cloned());
    podar_phis(func, &phis.iter().flatten().map(|&(_, p)| p).collect());
    true
}

/// Tira os `phi` inseridos que ninguém usa (fora eles mesmos).
fn podar_phis(func: &mut Function, inseridos: &HashSet<ValueId>) {
    loop {
        let mut usados: HashSet<ValueId> = HashSet::new();
        for b in &func.blocks {
            for (v, inst, _) in &b.instructions {
                operandos(inst, &mut |o| {
                    if let Operand::Val(u) = o
                        && u != v
                    {
                        usados.insert(*u);
                    }
                });
            }
            operandos_do_terminador(&b.terminator, &mut |o| {
                if let Operand::Val(u) = o {
                    usados.insert(*u);
                }
            });
        }
        let mut tirou = false;
        for b in &mut func.blocks {
            let antes = b.instructions.len();
            b.instructions.retain(|(v, _, _)| !inseridos.contains(v) || usados.contains(v));
            tirou |= b.instructions.len() != antes;
        }
        if !tirou {
            break;
        }
    }
}
