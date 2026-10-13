//! Stable digest of a text: 128-bit FNV-1a.
//!
//! Used to compare LLVM IR across runs (the harness determinism test) and to
//! build on-disk cache keys. That is why it does not use the standard library's
//! `DefaultHasher`: its algorithm is not guaranteed across Rust versions, and a
//! key written to disk has to survive a compiler change. 128 bits because the
//! key identifies the content on its own, without comparing the text back.

/// Offset basis and prime of 128-bit FNV-1a (IETF draft-eastlake-fnv).
const BASE: u128 = 0x6c62272e07bb014262b821756295c58d;
const PRIME: u128 = 0x0000000001000000000000000000013b;

/// 128-bit FNV-1a in parts: `write` as many times as needed, `end` at the end.
/// The result is the same as [`fnv1a_128`] over the concatenation of the parts.
#[derive(Debug, Clone, Copy)]
pub struct Fnv128(u128);

impl Default for Fnv128 {
    fn default() -> Self {
        Fnv128(BASE)
    }
}

impl Fnv128 {
    pub fn write(&mut self, data: &[u8]) -> &mut Self {
        for b in data {
            self.0 ^= u128::from(*b);
            self.0 = self.0.wrapping_mul(PRIME);
        }
        self
    }

    pub fn end(&self) -> u128 {
        self.0
    }
}

pub fn fnv1a_128(data: &[u8]) -> u128 {
    Fnv128::default().write(data).end()
}

/// Digest of an emitted LLVM IR: the hash and the size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResumoIr {
    pub hash: u128,
    pub bytes: usize,
}

impl ResumoIr {
    pub fn de(text: &str) -> ResumoIr {
        ResumoIr { hash: fnv1a_128(text.as_bytes()), bytes: text.len() }
    }

    /// The hash as 32 hexadecimal digits.
    pub fn hex(&self) -> String {
        format!("{:032x}", self.hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_vectors() {
        assert_eq!(format!("{:032x}", fnv1a_128(b"")), "6c62272e07bb014262b821756295c58d");
        assert_eq!(format!("{:032x}", fnv1a_128(b"a")), "d228cb696f1a8caf78912b704e4a8964");
        assert_eq!(format!("{:032x}", fnv1a_128(b"foobar")), "343e1662793c64bf6f0d3597ba446f18");
    }

    #[test]
    fn in_parts_is_same_as_whole() {
        assert_eq!(Fnv128::default().write(b"foo").write(b"").write(b"bar").end(), fnv1a_128(b"foobar"));
    }

    #[test]
    fn resumo() {
        let r = ResumoIr::de("a");
        assert_eq!(r.hex(), "d228cb696f1a8caf78912b704e4a8964");
        assert_eq!(r.bytes, 1);
    }
}
