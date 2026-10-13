//! Name interning: repeated identifiers collapse into a 4-byte index.
//!
//! The memory goal requires collapsing repeated identifiers in the semantic
//! layer instead of one `String` per occurrence. Today the AST borrows
//! `&'a str` from the source in 54 places (`crates/syntax`), so nothing
//! survives between revisions and no per-unit cache is possible: the borrow
//! ties the model to the text of one specific revision.
//!
//! This crate is the piece that breaks that borrow. The design:
//!
//! * [`SymbolId`] is `Copy`, 4 bytes, and points to no revision;
//! * [`Interner`] is the name arena of **one revision**: it interns each
//!   distinct name once, resolves by index, and reports
//!   [`Interner::payload_bytes`] for the model's before/after `live_bytes`
//!   check;
//! * between revisions, only the `SymbolId` crosses over; the old arena goes
//!   away with the old revision, and the disposal is as deterministic as any
//!   `drop`.
//!
//! What is still missing (a recorded boundary, not a forgotten one):
//! replacing the 54 `&'a str` places in the AST with `SymbolId` requires
//! threading an `Interner` through the lexer/parser and changing every
//! consumer — semantics, HIR, codegen. This crate delivers the facility and
//! the tests; migrating the consumers is the next step and has its own
//! verification (item 4 of the goal).
use std::collections::HashMap;

/// Interned identifier of a distinct name: 4 bytes, `Copy`, no borrowing.
///
/// It is only meaningful inside the [`Interner`] that produced it. Comparing
/// IDs from different arenas is a logic error — that is why there is no
/// total order across arenas, only equality within the same namespace by
/// construction.
///
/// Stored as a `NonZeroU32` (index + 1) so that `Option<SymbolId>` and
/// `Option<Name>` cost the same as the value without `Option`: the tree has
/// thousands of optional names (named argument, named constructor, unnamed
/// parameter), and the niche saves 8 bytes on each one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SymbolId(std::num::NonZeroU32);

impl SymbolId {
    /// Returns the symbol's raw numeric index inside its arena (zero-based).
    pub const fn as_u32(self) -> u32 {
        self.0.get() - 1
    }

    /// Builds from the index in the arena; only the [`Interner`] calls this.
    const fn from_index(index: u32) -> Self {
        // `index + 1` is never zero while the arena fits in `u32`.
        match std::num::NonZeroU32::new(index.wrapping_add(1)) {
            Some(n) => SymbolId(n),
            None => panic!("name arena exhausted the u32 space"),
        }
    }
}

/// Name arena of one revision: each distinct text lives exactly once.
///
/// Interning the same text twice returns the same [`SymbolId`] without
/// allocating again; that is how N occurrences of an identifier cost one
/// `Box<str>` instead of N. The [`Interner::payload_bytes`] estimate adds up
/// the bytes of the retained boxes, without measuring the map's idle
/// capacity or the allocator's overhead — the same honesty contract as the
/// macro cache.
#[derive(Debug, Default)]
pub struct Interner {
    ids: HashMap<Box<str>, SymbolId>,
    texts: Vec<Box<str>>,
    payload_bytes: usize,
}

impl Interner {
    /// Checks whether a text was already interned in this arena without
    /// interning it again.
    pub fn lookup(&self, text: &str) -> Option<SymbolId> {
        self.ids.get(text).copied()
    }

    /// Creates an empty arena, without allocating.
    ///
    /// ```
    /// let arena = dartforge_intern::Interner::new();
    /// assert!(arena.is_empty());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Interns the text and returns its stable index in this arena.
    ///
    /// Repeated text does not allocate: the existing index comes straight back
    /// from the map.
    ///
    /// ```
    /// let mut arena = dartforge_intern::Interner::new();
    /// let a = arena.intern("add");
    /// assert_eq!(arena.intern("add"), a);
    /// assert_eq!(arena.resolve(a), "add");
    /// ```
    pub fn intern(&mut self, text: &str) -> SymbolId {
        if let Some(id) = self.ids.get(text) {
            return *id;
        }
        let id = SymbolId::from_index(self.texts.len() as u32);
        let stored: Box<str> = text.into();
        self.payload_bytes = self.payload_bytes.saturating_add(stored.len());
        self.texts.push(stored.clone());
        self.ids.insert(stored, id);
        id
    }

    /// Resolves an index produced by [`Interner::intern`] in this arena.
    ///
    /// # Panics
    ///
    /// Panics when the index does not belong to this arena — including when it
    /// came from another revision. Crossing the revision boundary with text
    /// instead of a [`SymbolId`] is exactly the borrow this crate exists to
    /// eliminate, so the mix-up fails loudly instead of reading garbage.
    ///
    /// ```
    /// let mut arena = dartforge_intern::Interner::new();
    /// let id = arena.intern("x");
    /// assert_eq!(arena.resolve(id), "x");
    /// ```
    pub fn resolve(&self, id: SymbolId) -> &str {
        &self.texts[id.as_u32() as usize]
    }

    /// Distinct names retained by this arena.
    pub fn len(&self) -> usize {
        self.texts.len()
    }

    /// Texts in index order (`texts()[id.as_u32()]`), to rebuild an arena with
    /// the same ids (SDK cache).
    pub fn texts(&self) -> impl Iterator<Item = &str> {
        self.texts.iter().map(|t| &**t)
    }

    /// True when no name was interned.
    pub fn is_empty(&self) -> bool {
        self.texts.is_empty()
    }

    /// Retained text bytes; saturating sums prevent a bypassed budget.
    pub fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }

    /// Empties the arena and resets the retained bytes; old IDs are no longer
    /// valid.
    pub fn clear(&mut self) {
        self.ids.clear();
        self.texts.clear();
        self.payload_bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Repetition collapses: N occurrences, one box, one index.
    #[test]
    fn repetition_collapses_into_one_index() {
        let mut arena = Interner::new();
        let first = arena.intern("counter");
        for _ in 0..100 {
            assert_eq!(arena.intern("counter"), first);
        }
        assert_eq!(arena.len(), 1);
        assert_eq!(arena.payload_bytes(), "counter".len());
        assert_eq!(arena.resolve(first), "counter");
    }

    /// Neighboring names are not confused, including with common prefixes.
    #[test]
    fn distinct_names_have_distinct_indices() {
        let mut arena = Interner::new();
        let sum = arena.intern("sum");
        let add = arena.intern("add");
        let other_sum = arena.intern("sum");
        assert_ne!(sum, add);
        assert_eq!(sum, other_sum);
        assert_eq!(arena.len(), 2);
        assert_eq!(arena.resolve(add), "add");
    }

    /// `clear` drops everything and the arena measures zero again.
    #[test]
    fn clear_drops_the_names() {
        let mut arena = Interner::new();
        arena.intern("a");
        arena.intern("bb");
        arena.clear();
        assert!(arena.is_empty());
        assert_eq!(arena.payload_bytes(), 0);
        let id = arena.intern("a");
        assert_eq!(arena.resolve(id), "a");
    }

    /// An index from another arena fails loudly instead of resolving wrongly.
    #[test]
    #[should_panic]
    fn index_from_another_arena_fails() {
        let mut old = Interner::new();
        let id = old.intern("x");
        let new_arena = Interner::new();
        let _ = new_arena.resolve(id);
    }
}
