//! O hasher dos mapas internos do runtime: o Fx (o do `rustc`), no lugar do
//! SipHash do `HashMap` padrão.
//!
//! As chaves daqui são ids, handles e tipos do próprio runtime (a tabela de
//! tipos, os caches de subtipo, os conjuntos de listas especiais) — nunca
//! dados que o programa Dart escolhe: os `Map` e `Set` do Dart são
//! implementados em Dart pelo SDK. Não há, portanto, colisão provocada a
//! evitar, e o SipHash custava cerca de um terço do tempo de um laço de
//! `Map.[]=` (medido com o callgrind). A ordem de iteração passa a ser
//! determinística; nada no runtime dependia dela.

use std::hash::{BuildHasherDefault, Hasher};

/// O `FxHasher`: rotaciona, mistura a palavra e multiplica.
#[derive(Default, Clone, Copy)]
pub struct Fx(u64);

const K: u64 = 0x517c_c1b7_2722_0a95;

impl Fx {
    #[inline]
    fn palavra(&mut self, w: u64) {
        self.0 = (self.0.rotate_left(5) ^ w).wrapping_mul(K);
    }
}

impl Hasher for Fx {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut pedacos = bytes.chunks_exact(8);
        for p in &mut pedacos {
            self.palavra(u64::from_le_bytes(p.try_into().unwrap_or_default()));
        }
        let resto = pedacos.remainder();
        if !resto.is_empty() {
            let mut b = [0u8; 8];
            b[..resto.len()].copy_from_slice(resto);
            self.palavra(u64::from_le_bytes(b));
        }
    }
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.palavra(u64::from(i));
    }
    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.palavra(u64::from(i));
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.palavra(u64::from(i));
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.palavra(i);
    }
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.palavra(i as u64);
    }
    #[inline]
    fn write_i64(&mut self, i: i64) {
        self.palavra(i as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

/// `HashMap` com o hasher Fx.
pub type HashMap<K, V> = std::collections::HashMap<K, V, BuildHasherDefault<Fx>>;
/// `HashSet` com o hasher Fx.
pub type HashSet<K> = std::collections::HashSet<K, BuildHasherDefault<Fx>>;
