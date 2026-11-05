//! Declaration traversal order independently of stable arena positions.

/// Creation positions of elements whose arena slots have been permuted.
/// Identity ranges are implicit; stored ranges are bounded by the current
/// arenas and are truncated with the program, rather than with reload count.
/// Arenas are classes, extensions, typedefs, functions and variables, in that order.
///
/// ```
/// let mut order = dartforge_elements::ElementOrder::default();
/// order.permute(0, 4, &[1, 0]);
/// assert_eq!(order.position(0, 4), 5);
/// assert_eq!(order.position(0, 5), 4);
/// ```
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ElementOrder([Range; 5]);

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Range {
    start: usize,
    positions: Vec<u32>,
}

impl Range {
    fn trim(&mut self) {
        let first = self
            .positions
            .iter()
            .enumerate()
            .take_while(|(i, position)| **position as usize == self.start + i)
            .count();
        self.positions.drain(..first);
        self.start += first;
        while self
            .positions
            .last()
            .is_some_and(|p| *p as usize == self.start + self.positions.len() - 1)
        {
            self.positions.pop();
        }
        if self.positions.is_empty() {
            *self = Self::default();
        }
    }
}

impl ElementOrder {
    /// The creation position of an element with the given stable ID.
    ///
    /// # Panics
    ///
    /// When `arena` is not in `0..5`.
    ///
    /// ```
    /// assert_eq!(dartforge_elements::ElementOrder::default().position(3, 12), 12);
    /// ```
    pub fn position(&self, arena: usize, id: u32) -> u32 {
        let range = &self.0[arena];
        (id as usize)
            .checked_sub(range.start)
            .and_then(|i| range.positions.get(i))
            .copied()
            .unwrap_or(id)
    }

    /// Records a permutation mapping each source offset to its destination
    /// offset in the arena range beginning at `start`. Existing order is
    /// composed with the permutation; the element arenas are changed by the caller.
    ///
    /// # Panics
    ///
    /// When `arena` is not in `0..5`, the range exceeds `u32::MAX`, or
    /// `destinations` is not a permutation of `0..destinations.len()`.
    ///
    /// ```
    /// let mut order = dartforge_elements::ElementOrder::default();
    /// order.permute(4, 7, &[2, 0, 1]);
    /// assert_eq!((order.position(4, 7), order.position(4, 8)), (8, 9));
    /// ```
    pub fn permute(&mut self, arena: usize, start: usize, destinations: &[usize]) {
        let end = start
            .checked_add(destinations.len())
            .filter(|end| *end <= u32::MAX as usize)
            .expect("element order range fits u32");
        let mut used = vec![false; destinations.len()];
        assert!(
            destinations.iter().all(|&target| {
                used.get_mut(target)
                    .is_some_and(|used| !std::mem::replace(used, true))
            }),
            "one destination per element"
        );
        let previous: Vec<u32> = (start..end)
            .map(|id| self.position(arena, id as u32))
            .collect();
        let range = &mut self.0[arena];
        if range.positions.is_empty() {
            range.start = start;
        } else if start < range.start {
            let mut prefix: Vec<u32> = (start..range.start).map(|id| id as u32).collect();
            prefix.append(&mut range.positions);
            range.positions = prefix;
            range.start = start;
        }
        range
            .positions
            .extend((range.start + range.positions.len()..end).map(|id| id as u32));
        for (source, &target) in destinations.iter().enumerate() {
            range.positions[start + target - range.start] = previous[source];
        }
        range.trim();
    }

    /// A lower bound on the creation positions of IDs at or after `from`.
    /// A creation-ordered list ending below it cannot contain retired IDs.
    pub(crate) fn first_retired_position(&self, arena: usize, from: usize) -> usize {
        let range = &self.0[arena];
        let start = from.saturating_sub(range.start).min(range.positions.len());
        range.positions[start..]
            .iter()
            .copied()
            .min()
            .map_or(from, |position| from.min(position as usize))
    }

    pub(crate) fn truncate(&mut self, covered: [usize; 5]) {
        for (range, end) in self.0.iter_mut().zip(covered) {
            range.positions.truncate(end.saturating_sub(range.start));
            range.trim();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composition_and_truncation_keep_only_live_order() {
        let mut order = ElementOrder::default();
        order.permute(0, 7, &[2, 0, 1]);
        order.permute(0, 8, &[1, 0]);
        assert_eq!(
            (7..10).map(|id| order.position(0, id)).collect::<Vec<_>>(),
            [8, 7, 9]
        );
        assert_eq!(order.0[0].positions.len(), 2);
        order.truncate([7, 0, 0, 0, 0]);
        assert_eq!(order, ElementOrder::default());
        for _ in 0..128 {
            order.permute(3, 100, &[1, 0]);
            order.permute(3, 100, &[1, 0]);
            assert_eq!(order, ElementOrder::default());
        }
    }
}
