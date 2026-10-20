//! The flow graph of a function: predecessors, reachable blocks and the
//! dominator tree (Cooper, Harvey and Kennedy, "A Simple, Fast
//! Dominance Algorithm"), with the dominance frontiers.

use super::operands::successors_mut;
use crate::hir::*;
use std::collections::HashMap;

pub struct Cfg {
    pub successors: Vec<Vec<usize>>,
    pub predecessors: Vec<Vec<usize>>,
    /// Reverse post-order from the entry (reachable blocks only).
    pub rpo: Vec<usize>,
    /// The immediate dominator of each reachable block (the entry points
    /// to itself); `usize::MAX` for unreachable ones.
    pub idom: Vec<usize>,
}

pub fn successors(t: &Terminator) -> Vec<BlockId> {
    let mut t = t.clone();
    let mut v = Vec::new();
    successors_mut(&mut t, &mut |b| v.push(*b));
    v
}

impl Cfg {
    pub fn new(func: &Function) -> Cfg {
        let pos: HashMap<BlockId, usize> = func
            .blocks
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id, i))
            .collect();
        let n = func.blocks.len();
        let mut succ = vec![Vec::new(); n];
        let mut pred = vec![Vec::new(); n];
        for (i, b) in func.blocks.iter().enumerate() {
            for s in successors(&b.terminator) {
                if let Some(&j) = pos.get(&s)
                    && !succ[i].contains(&j)
                {
                    succ[i].push(j);
                    pred[j].push(i);
                }
            }
        }
        // Iterative post-order from the entry (position 0).
        let mut seen = vec![false; n];
        let mut post_order = Vec::with_capacity(n);
        if n > 0 {
            let mut stack = vec![(0usize, 0usize)];
            seen[0] = true;
            while let Some((b, k)) = stack.last_mut() {
                if *k < succ[*b].len() {
                    let s = succ[*b][*k];
                    *k += 1;
                    if !seen[s] {
                        seen[s] = true;
                        stack.push((s, 0));
                    }
                } else {
                    post_order.push(*b);
                    stack.pop();
                }
            }
        }
        let rpo: Vec<usize> = post_order.iter().rev().copied().collect();
        let mut order = vec![usize::MAX; n];
        for (k, &b) in rpo.iter().enumerate() {
            order[b] = k;
        }
        let mut idom = vec![usize::MAX; n];
        if n > 0 {
            idom[0] = 0;
        }
        let mut changed = true;
        while changed {
            changed = false;
            for &b in rpo.iter().skip(1) {
                let mut new = usize::MAX;
                for &p in &pred[b] {
                    if idom[p] == usize::MAX {
                        continue;
                    }
                    new = if new == usize::MAX {
                        p
                    } else {
                        intersection(&idom, &order, p, new)
                    };
                }
                if new != usize::MAX && idom[b] != new {
                    idom[b] = new;
                    changed = true;
                }
            }
        }
        Cfg {
            successors: succ,
            predecessors: pred,
            rpo,
            idom,
        }
    }

    pub fn reachable(&self, b: usize) -> bool {
        self.idom[b] != usize::MAX
    }

    /// The dominance frontiers of each reachable block.
    pub fn boundaries(&self) -> Vec<Vec<usize>> {
        let n = self.idom.len();
        let mut df = vec![Vec::new(); n];
        for b in 0..n {
            if !self.reachable(b) {
                continue;
            }
            let preds: Vec<usize> = self.predecessors[b]
                .iter()
                .copied()
                .filter(|&p| self.reachable(p))
                .collect();
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

    /// The children of each block in the dominator tree.
    pub fn children(&self) -> Vec<Vec<usize>> {
        let mut f = vec![Vec::new(); self.idom.len()];
        for &b in &self.rpo {
            if b != 0 {
                f[self.idom[b]].push(b);
            }
        }
        f
    }
}

fn intersection(idom: &[usize], order: &[usize], mut a: usize, mut b: usize) -> usize {
    while a != b {
        while order[a] > order[b] {
            a = idom[a];
        }
        while order[b] > order[a] {
            b = idom[b];
        }
    }
    a
}
