//! Bounded dominator sets for the structural SSA witness.
//!
//! The proof already rejects bodies larger than MAX_BLOCKS. Dense sets preserve
//! its fixed point while avoiding a tree allocation for each intersection.

use std::collections::BTreeSet;

pub(super) const MAX_BLOCKS: usize = 512;
const WORDS: usize = MAX_BLOCKS.div_ceil(u64::BITS as usize);
type Blocks = [u64; WORDS];

pub(super) struct Dominators {
    sets: Vec<Blocks>,
}

impl Dominators {
    /// Edges and reachability must already be validated by Shape. Block zero is
    /// the entry, is reachable, and has no incoming edges.
    pub(super) fn new(predecessors: &[Vec<usize>], reachable: &BTreeSet<usize>) -> Self {
        assert!(!predecessors.is_empty() && predecessors.len() <= MAX_BLOCKS);
        let mut all = [0; WORDS];
        for &block in reachable {
            all[block / 64] |= 1 << (block % 64);
        }
        let mut sets = vec![all; predecessors.len()];
        sets[0] = [0; WORDS];
        sets[0][0] = 1;
        loop {
            let mut changed = false;
            for &block in reachable.iter().filter(|&&block| block != 0) {
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
                return Self { sets };
            }
        }
    }

    pub(super) fn dominates(&self, definition: usize, block: usize) -> bool {
        self.sets[block][definition / 64] & (1 << (definition % 64)) != 0
    }
}

#[cfg(test)]
mod tests;
