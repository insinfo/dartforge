//! O grafo de fluxo de uma função: predecessores, blocos alcançáveis e a
//! árvore de dominância (Cooper, Harvey e Kennedy, "A Simple, Fast
//! Dominance Algorithm"), com as fronteiras de dominância.

use super::operandos::sucessores_mut;
use crate::hir::*;
use std::collections::HashMap;

pub struct Cfg {
    pub sucessores: Vec<Vec<usize>>,
    pub predecessores: Vec<Vec<usize>>,
    /// Ordem reversa pós-ordem a partir da entrada (só os alcançáveis).
    pub rpo: Vec<usize>,
    /// O dominador imediato de cada bloco alcançável (a entrada aponta
    /// para si); `usize::MAX` nos inalcançáveis.
    pub idom: Vec<usize>,
}

pub fn sucessores(t: &Terminator) -> Vec<BlockId> {
    let mut t = t.clone();
    let mut v = Vec::new();
    sucessores_mut(&mut t, &mut |b| v.push(*b));
    v
}

impl Cfg {
    pub fn novo(func: &Function) -> Cfg {
        let pos: HashMap<BlockId, usize> = func.blocks.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
        let n = func.blocks.len();
        let mut suc = vec![Vec::new(); n];
        let mut pred = vec![Vec::new(); n];
        for (i, b) in func.blocks.iter().enumerate() {
            for s in sucessores(&b.terminator) {
                if let Some(&j) = pos.get(&s)
                    && !suc[i].contains(&j)
                {
                    suc[i].push(j);
                    pred[j].push(i);
                }
            }
        }
        // Pós-ordem iterativa a partir da entrada (posição 0).
        let mut visto = vec![false; n];
        let mut pos_ordem = Vec::with_capacity(n);
        if n > 0 {
            let mut pilha = vec![(0usize, 0usize)];
            visto[0] = true;
            while let Some((b, k)) = pilha.last_mut() {
                if *k < suc[*b].len() {
                    let s = suc[*b][*k];
                    *k += 1;
                    if !visto[s] {
                        visto[s] = true;
                        pilha.push((s, 0));
                    }
                } else {
                    pos_ordem.push(*b);
                    pilha.pop();
                }
            }
        }
        let rpo: Vec<usize> = pos_ordem.iter().rev().copied().collect();
        let mut ordem = vec![usize::MAX; n];
        for (k, &b) in rpo.iter().enumerate() {
            ordem[b] = k;
        }
        let mut idom = vec![usize::MAX; n];
        if n > 0 {
            idom[0] = 0;
        }
        let mut mudou = true;
        while mudou {
            mudou = false;
            for &b in rpo.iter().skip(1) {
                let mut novo = usize::MAX;
                for &p in &pred[b] {
                    if idom[p] == usize::MAX {
                        continue;
                    }
                    novo = if novo == usize::MAX { p } else { intersecao(&idom, &ordem, p, novo) };
                }
                if novo != usize::MAX && idom[b] != novo {
                    idom[b] = novo;
                    mudou = true;
                }
            }
        }
        Cfg { sucessores: suc, predecessores: pred, rpo, idom }
    }

    pub fn alcancavel(&self, b: usize) -> bool {
        self.idom[b] != usize::MAX
    }

    /// As fronteiras de dominância de cada bloco alcançável.
    pub fn fronteiras(&self) -> Vec<Vec<usize>> {
        let n = self.idom.len();
        let mut df = vec![Vec::new(); n];
        for b in 0..n {
            if !self.alcancavel(b) {
                continue;
            }
            let preds: Vec<usize> = self.predecessores[b].iter().copied().filter(|&p| self.alcancavel(p)).collect();
            if preds.len() < 2 {
                continue;
            }
            for p in preds {
                let mut r = p;
                while r != self.idom[b] {
                    if !df[r].contains(&b) {
                        df[r].push(b);
                    }
                    if r == self.idom[r] {
                        break;
                    }
                    r = self.idom[r];
                }
            }
        }
        df
    }

    /// Os filhos de cada bloco na árvore de dominância.
    pub fn filhos(&self) -> Vec<Vec<usize>> {
        let mut f = vec![Vec::new(); self.idom.len()];
        for &b in &self.rpo {
            if b != 0 {
                f[self.idom[b]].push(b);
            }
        }
        f
    }
}

fn intersecao(idom: &[usize], ordem: &[usize], mut a: usize, mut b: usize) -> usize {
    while a != b {
        while ordem[a] > ordem[b] {
            a = idom[a];
        }
        while ordem[b] > ordem[a] {
            b = idom[b];
        }
    }
    a
}
