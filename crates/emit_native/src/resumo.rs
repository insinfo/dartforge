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
const PRIMO: u128 = 0x0000000001000000000000000000013b;

/// FNV-1a de 128 bits em partes: `escrever` quantas vezes quiser, `fim` no final.
/// O resultado é o mesmo de [`fnv1a_128`] sobre a concatenação das partes.
#[derive(Debug, Clone, Copy)]
pub struct Fnv128(u128);

impl Default for Fnv128 {
    fn default() -> Self {
        Fnv128(BASE)
    }
}

impl Fnv128 {
    pub fn escrever(&mut self, dados: &[u8]) -> &mut Self {
        for b in dados {
            self.0 ^= u128::from(*b);
            self.0 = self.0.wrapping_mul(PRIMO);
        }
        self
    }

    pub fn fim(&self) -> u128 {
        self.0
    }
}

pub fn fnv1a_128(dados: &[u8]) -> u128 {
    Fnv128::default().escrever(dados).fim()
}

/// Resumo de um LLVM IR emitido: o hash e o tamanho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResumoIr {
    pub hash: u128,
    pub bytes: usize,
}

impl ResumoIr {
    pub fn de(texto: &str) -> ResumoIr {
        ResumoIr { hash: fnv1a_128(texto.as_bytes()), bytes: texto.len() }
    }

    /// O hash em 32 dígitos hexadecimais.
    pub fn hex(&self) -> String {
        format!("{:032x}", self.hash)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn vetores_publicados() {
        assert_eq!(format!("{:032x}", fnv1a_128(b"")), "6c62272e07bb014262b821756295c58d");
        assert_eq!(format!("{:032x}", fnv1a_128(b"a")), "d228cb696f1a8caf78912b704e4a8964");
        assert_eq!(format!("{:032x}", fnv1a_128(b"foobar")), "343e1662793c64bf6f0d3597ba446f18");
    }

    #[test]
    fn em_partes_e_o_mesmo_que_inteiro() {
        assert_eq!(Fnv128::default().escrever(b"foo").escrever(b"").escrever(b"bar").fim(), fnv1a_128(b"foobar"));
    }

    #[test]
    fn resumo() {
        let r = ResumoIr::de("a");
        assert_eq!(r.hex(), "d228cb696f1a8caf78912b704e4a8964");
        assert_eq!(r.bytes, 1);
    }
}
