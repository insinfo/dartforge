//! Bounded dominator sets for the structural SSA witness.
//!
//! The proof already rejects bodies larger than MAX_BLOCKS. Dense sets preserve
//! its fixed point while avoiding a tree allocation for each intersection.

pub(super) const MAX_BLOCKS: usize = 512;
const WORDS: usize = MAX_BLOCKS.div_ceil(u64::BITS as usize);
type Blocks = [u64; WORDS];

pub(super) struct Dominators {
    sets: Vec<Blocks>,
    #[cfg(test)]
    sweeps: usize,
}

/// Iterative depth-first finish order, reversed. Shape has already checked
/// successor indices and the block budget; a frame stores only one active edge
/// index, so duplicate switch edges cannot inflate a pending-node stack.
pub(super) fn reverse_postorder(successors: &[Vec<usize>]) -> Vec<usize> {
    assert!(!successors.is_empty() && successors.len() <= MAX_BLOCKS);
    let mut seen = [false; MAX_BLOCKS];
    let mut pending = vec![(0, 0)];
    let mut order = Vec::with_capacity(successors.len());
    seen[0] = true;
    while let Some((block, next)) = pending.last_mut() {
        if *next == successors[*block].len() {
            order.push(*block);
            pending.pop();
        } else {
            let successor = successors[*block][*next];
            *next += 1;
            if !seen[successor] {
                seen[successor] = true;
                pending.push((successor, 0));
            }
        }
    }
    order.reverse();
    order
}

impl Dominators {
    /// Edges must already be validated by Shape. Order lists every reachable
    /// block once, starting at entry zero, which has no incoming edges. Reverse
    /// postorder speeds convergence without changing the fixed-point equation.
    pub(super) fn new(predecessors: &[Vec<usize>], order: &[usize]) -> Self {
        assert!(!predecessors.is_empty() && predecessors.len() <= MAX_BLOCKS);
        let mut all = [0; WORDS];
        assert_eq!(order.first(), Some(&0));
        for &block in order {
            assert!(block < predecessors.len());
            let bit = 1 << (block % 64);
            assert_eq!(all[block / 64] & bit, 0, "duplicate traversal block");
            all[block / 64] |= bit;
        }
        let mut sets = vec![all; predecessors.len()];
        sets[0] = [0; WORDS];
        sets[0][0] = 1;
        #[cfg(test)]
        let mut sweeps = 0;
        loop {
            #[cfg(test)]
            {
                sweeps += 1;
            }
            let mut changed = false;
            for &block in &order[1..] {
                let mut next = all;
                for &parent in &predecessors[block] {
                    if all[parent / 64] & (1 << (parent % 64)) != 0 {
                        for (word, incoming) in next.iter_mut().zip(&sets[parent]) {
                            *word &= incoming;
                        }
                    }
                }
                next[block / 64] |= 1 << (block % 64);
                if next != sets[block] {
                    sets[block] = next;
                    changed = true;
                }
            }
            if !changed {
                return Self {
                    sets,
                    #[cfg(test)]
                    sweeps,
                };
            }
        }
    }

    pub(super) fn dominates(&self, definition: usize, block: usize) -> bool {
        self.sets[block][definition / 64] & (1 << (definition % 64)) != 0
    }
}

#[cfg(test)]
mod tests;
