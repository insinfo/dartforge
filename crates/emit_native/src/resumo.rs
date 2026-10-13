//! Resumo estável de um texto: FNV-1a de 128 bits.
//!
//! Serve para comparar LLVM IR entre execuções (o teste de determinismo do
//! harness) e para montar chaves de cache em disco. Por isso não usa o
//! `DefaultHasher` da biblioteca padrão: o algoritmo dele não é garantido entre
//! versões do Rust, e uma chave gravada em disco tem de sobreviver à troca do
//! compilador. 128 bits porque a chave identifica o conteúdo sozinha, sem
//! comparar o texto de volta.

/// Base e primo do FNV-1a de 128 bits (IETF draft-eastlake-fnv).
const BASE: u128 = 0x6c62272e07bb014262b821756295c58d;
const PRIME: u128 = 0x0000000001000000000000000000013b;

/// FNV-1a de 128 bits em partes: `write` quantas vezes quiser, `end` no final.
/// O resultado é o mesmo de [`fnv1a_128`] sobre a concatenação das partes.
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

/// Resumo de um LLVM IR emitido: o hash e o tamanho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResumoIr {
    pub hash: u128,
    pub bytes: usize,
}

impl ResumoIr {
    pub fn de(text: &str) -> ResumoIr {
        ResumoIr { hash: fnv1a_128(text.as_bytes()), bytes: text.len() }
    }

    /// O hash em 32 dígitos hexadecimais.
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
