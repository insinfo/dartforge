//! The hasher of the runtime's internal maps: Fx (the one from `rustc`), instead of
//! the SipHash of the standard `HashMap`.
//!
//! The keys here are the runtime's own ids, handles and types (the type table,
//! the subtype caches, the sets of special lists) — never data chosen by the
//! Dart program: Dart's `Map` and `Set` are implemented in Dart by the SDK. So
//! there are no provoked collisions to avoid, and SipHash cost about a third of
//! the time of a `Map.[]=` loop (measured with callgrind). Iteration order
//! becomes deterministic; nothing in the runtime depended on it.

use std::hash::{BuildHasherDefault, Hasher};

/// The `FxHasher`: rotate, mix in the word and multiply.
#[derive(Default, Clone, Copy)]
pub struct Fx(u64);

const K: u64 = 0x517c_c1b7_2722_0a95;

impl Fx {
    #[inline]
    fn word(&mut self, w: u64) {
        self.0 = (self.0.rotate_left(5) ^ w).wrapping_mul(K);
    }
}

impl Hasher for Fx {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let (chunks, rest) = bytes.as_chunks::<8>();
        for p in chunks {
            self.word(u64::from_le_bytes(*p));
        }
        if !rest.is_empty() {
            let mut b = [0u8; 8];
            b[..rest.len()].copy_from_slice(rest);
            self.word(u64::from_le_bytes(b));
        }
    }
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.word(u64::from(i));
    }
    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.word(u64::from(i));
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.word(u64::from(i));
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.word(i);
    }
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.word(i as u64);
    }
    #[inline]
    fn write_i64(&mut self, i: i64) {
        self.word(i as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

/// `HashMap` with the Fx hasher.
pub type HashMap<K, V> = std::collections::HashMap<K, V, BuildHasherDefault<Fx>>;
/// `HashSet` with the Fx hasher.
pub type HashSet<K> = std::collections::HashSet<K, BuildHasherDefault<Fx>>;
