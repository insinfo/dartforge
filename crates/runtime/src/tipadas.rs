//! Listas tipadas, visões e SIMD no espaço unificado (P4,
//! docs/NATIVO-ESPACO-UNIFICADO.md §2.5 e §3.3).
//!
//! A lista tipada interna (cids 22–35) é `BRUTO`: o comprimento em `b+16`, o
//! endereço dos dados em `b+24` (apontando para `b+32`, ou para a memória de fora
//! com `EXTERNO`) e os bytes. A visão (36–65) é `INSTANCIA` de quatro campos: o
//! comprimento e os dados nos mesmos deslocamentos (calculados na criação: a base
//! não se move), a base (`Ref`, a lista interna) e o deslocamento em bytes. Os
//! valores SIMD (19–21) são `BRUTO` de duas palavras, as pistas.
//!
//! Comprimento e dados no mesmo lugar nas três formas (o `PointerBase::data_` da
//! VM): o código gerado lê `len = [h+14]` e `p = [h+22]` sem saber qual delas é
//! (`llvm/tipados_ir.rs`). Nenhum dos dois muda enquanto a lista vive.

use crate::heap::Heap;
use crate::layout::{self, cid, flags, Ref};

/// Os tipos de elemento (`TIPO_*`, a ordem das classes de §2.4: o cid da lista
/// interna é `cid::tipada(tipo)`).
pub mod tipo {
    pub const INT8: u8 = 0;
    pub const UINT8: u8 = 1;
    pub const UINT8_CLAMPED: u8 = 2;
    pub const INT16: u8 = 3;
    pub const UINT16: u8 = 4;
    pub const INT32: u8 = 5;
    pub const UINT32: u8 = 6;
    pub const INT64: u8 = 7;
    pub const UINT64: u8 = 8;
    pub const FLOAT32: u8 = 9;
    pub const FLOAT64: u8 = 10;
    pub const FLOAT32X4: u8 = 11;
    pub const INT32X4: u8 = 12;
    pub const FLOAT64X2: u8 = 13;
    /// O `ByteData` (só em visão: `_ByteDataView`, cids 64 e 65); elementos de um
    /// byte.
    pub const BYTE_DATA: u8 = 14;
}

/// Bytes por elemento de um tipo.
pub const fn tamanho_do_elemento(t: u8) -> usize {
    match t {
        tipo::INT16 | tipo::UINT16 => 2,
        tipo::INT32 | tipo::UINT32 | tipo::FLOAT32 => 4,
        tipo::INT64 | tipo::UINT64 | tipo::FLOAT64 => 8,
        tipo::FLOAT32X4 | tipo::INT32X4 | tipo::FLOAT64X2 => 16,
        _ => 1,
    }
}

/// O tipo de elemento da lista tipada ou visão de cid `c` (`None` fora de 22..=65).
pub const fn tipo_do_cid(c: i32) -> Option<u8> {
    if cid::e_tipada_interna(c) {
        Some((c - cid::PRIMEIRA_TIPADA) as u8)
    } else if c == cid::BYTE_DATA_VIEW || c == cid::UNMODIFIABLE_BYTE_DATA_VIEW {
        Some(tipo::BYTE_DATA)
    } else if cid::e_tipada(c) {
        Some(((c - cid::PRIMEIRA_VISAO) % cid::TIPOS_DE_ELEMENTO) as u8)
    } else {
        None
    }
}

/// O cid da visão do tipo `t` (inclusive o `ByteData`), modificável ou não.
pub const fn cid_da_visao(t: u8, imutavel: bool) -> i32 {
    if t == tipo::BYTE_DATA {
        if imutavel { cid::UNMODIFIABLE_BYTE_DATA_VIEW } else { cid::BYTE_DATA_VIEW }
    } else {
        cid::visao(t, imutavel)
    }
}

/// O endereço do bloco de um handle.
const fn bloco(h: Ref) -> i64 {
    h - layout::DESLOCAMENTO_DO_HANDLE
}

/// Índices das palavras do corpo (a partir de `b+16`).
const PALAVRA_COMPRIMENTO: usize = (layout::desl::COMPRIMENTO - layout::desl::CORPO) / 8;
const PALAVRA_DADOS: usize = (layout::desl::DADOS - layout::desl::CORPO) / 8;
const PALAVRA_BASE: usize = (layout::desl::BASE_DA_VISAO - layout::desl::CORPO) / 8;
const PALAVRA_DESLOCAMENTO: usize = (layout::desl::DESLOCAMENTO_DA_VISAO - layout::desl::CORPO) / 8;
/// Campos de uma visão.
const CAMPOS_DA_VISAO: usize = 4;

const _: () = {
    assert!(PALAVRA_COMPRIMENTO == 0 && PALAVRA_DADOS == 1 && PALAVRA_BASE == 2 && PALAVRA_DESLOCAMENTO == 3);
    assert!(layout::desl::BYTES_INTERNOS == layout::desl::DADOS + 8);
};

/// A vista de uma lista tipada ou visão.
#[derive(Clone, Copy, Debug)]
pub struct TipadaRef {
    pub cid: i32,
    /// O `TIPO_*` do elemento ([`tipo`]).
    pub tipo: u8,
    /// O comprimento em elementos.
    pub len: usize,
    /// O primeiro byte.
    pub dados: *mut u8,
    /// O tamanho em bytes.
    pub bytes: usize,
    pub imutavel: bool,
    pub externa: bool,
    /// A lista de base de uma visão.
    pub base: Option<Ref>,
    /// O deslocamento em bytes de uma visão na base.
    pub deslocamento: usize,
}

impl TipadaRef {
    /// Os bytes. Valem enquanto a lista vive e não há alocação sem raiz para ela
    /// (§2.14: o coletor não move).
    ///
    /// # Safety
    /// `self` saiu de [`Heap::tipada`] e a lista ainda vive.
    #[allow(unsafe_code)]
    pub unsafe fn fatia<'a>(self) -> &'a [u8] {
        if self.bytes == 0 || self.dados.is_null() {
            return &[];
        }
        // SAFETY: `dados..dados+bytes` é a memória da lista (a do bloco, ou a de
        // fora de `asTypedList`), viva pelo contrato do chamador.
        unsafe { std::slice::from_raw_parts(self.dados, self.bytes) }
    }

    /// Os bytes graváveis (as regras de [`TipadaRef::fatia`]); o chamador não
    /// mantém outra fatia da mesma memória.
    ///
    /// # Safety
    /// Como em [`TipadaRef::fatia`], e sem outra referência viva aos mesmos bytes.
    #[allow(unsafe_code)]
    pub unsafe fn fatia_mut<'a>(self) -> &'a mut [u8] {
        if self.bytes == 0 || self.dados.is_null() {
            return &mut [];
        }
        // SAFETY: como em `fatia`; a exclusividade é do chamador.
        unsafe { std::slice::from_raw_parts_mut(self.dados, self.bytes) }
    }
}

impl Heap {
    /// Uma lista tipada interna do tipo `tipo` com `len` elementos zerados.
    pub fn nova_tipada(&mut self, tipo: u8, len: usize) -> Ref {
        debug_assert!(i32::from(tipo) < cid::TIPOS_DE_ELEMENTO, "lista tipada interna de ByteData");
        // Um comprimento que não cabe no endereçamento satura: o teto do heap
        // encerra com a mensagem de memória, como a VM com `OutOfMemoryError`.
        let bytes = len.checked_mul(tamanho_do_elemento(tipo)).unwrap_or(usize::MAX / 2);
        let palavras = 2 + bytes.div_ceil(8);
        let h = self.alocar(cid::tipada(tipo), palavras, flags::BRUTO);
        let p = self.palavras_mut(h);
        p[PALAVRA_COMPRIMENTO] = len as i64;
        p[PALAVRA_DADOS] = bloco(h) + layout::desl::BYTES_INTERNOS as i64;
        h
    }

    /// Uma lista tipada sobre `len` elementos da memória nativa em `endereco`
    /// (`asTypedList`): `BRUTO` + `EXTERNO`, sem cópia.
    pub fn tipada_externa(&mut self, tipo: u8, endereco: *mut u8, len: usize) -> Ref {
        let h = self.alocar(cid::tipada(tipo), 2, flags::BRUTO | flags::EXTERNO);
        let p = self.palavras_mut(h);
        p[PALAVRA_COMPRIMENTO] = len as i64;
        p[PALAVRA_DADOS] = endereco as i64;
        h
    }

    /// Uma visão (`cid`) de `len` elementos sobre `base`, a partir de `deslocamento`
    /// bytes. Uma visão passada como base é resolvida para a lista interna dela
    /// (os deslocamentos se somam), como no `typed_data_patch.dart`.
    pub fn nova_visao(&mut self, cid: i32, base: Ref, deslocamento: usize, len: usize) -> Ref {
        let Some(b) = self.tipada(base) else {
            panic!("bug do compilador: visão sobre algo que não é lista tipada");
        };
        let (interna, deslocamento) = match b.base {
            Some(i) => (i, b.deslocamento + deslocamento),
            None => (base, deslocamento),
        };
        // O endereço é calculado antes de alocar: a base não se move.
        let dados = (b.dados as i64).wrapping_add((deslocamento - b.deslocamento) as i64);
        let quadro = self.push_frame_with_slots(1);
        self.set_root(quadro, 0, interna);
        let h = self.alocar_instancia(cid, CAMPOS_DA_VISAO);
        self.pop_frame(quadro);
        // Objeto novo (jovem): as gravações não precisam de barreira; a da base
        // acende o bit de referência no mapa.
        self.definir_campo(h, PALAVRA_COMPRIMENTO, len as i64, false);
        self.definir_campo(h, PALAVRA_DADOS, dados, false);
        self.definir_campo(h, PALAVRA_BASE, interna, true);
        self.definir_campo(h, PALAVRA_DESLOCAMENTO, deslocamento as i64, false);
        h
    }

    /// `h` é lista tipada ou visão (cid 22..=65)?
    pub fn e_tipada(&self, h: Ref) -> bool {
        layout::e_objeto(h) && self.e_objeto_vivo(h) && cid::e_tipada(self.cabecalho(h).class_id)
    }

    /// A vista da lista tipada ou visão `h`; `None` para o que não é.
    pub fn tipada(&self, h: Ref) -> Option<TipadaRef> {
        if !layout::e_objeto(h) || !self.e_objeto_vivo(h) {
            return None;
        }
        let cab = self.cabecalho(h);
        let c = cab.class_id;
        let t = tipo_do_cid(c)?;
        let p = self.palavras(h);
        let len = p[PALAVRA_COMPRIMENTO] as usize;
        let dados = p[PALAVRA_DADOS] as usize as *mut u8;
        let bytes = len * tamanho_do_elemento(t);
        Some(if cid::e_tipada_interna(c) {
            TipadaRef {
                cid: c,
                tipo: t,
                len,
                dados,
                bytes,
                imutavel: false,
                externa: cab.flags & flags::EXTERNO != 0,
                base: None,
                deslocamento: 0,
            }
        } else {
            TipadaRef {
                cid: c,
                tipo: t,
                len,
                dados,
                bytes,
                imutavel: cid::e_visao_imutavel(c),
                externa: false,
                base: Some(p[PALAVRA_BASE]),
                deslocamento: p[PALAVRA_DESLOCAMENTO] as usize,
            }
        })
    }

    /// Os bytes da lista tipada ou visão `h`.
    #[allow(unsafe_code)]
    pub fn bytes_da_tipada(&self, h: Ref) -> Option<&[u8]> {
        let t = self.tipada(h)?;
        // SAFETY: a lista vive enquanto dura o empréstimo do heap (nenhuma coleta
        // com `&self`).
        Some(unsafe { t.fatia() })
    }

    /// Os bytes graváveis; `None` se `h` não é lista tipada ou é não modificável.
    #[allow(unsafe_code)]
    pub fn bytes_da_tipada_mut(&mut self, h: Ref) -> Option<&mut [u8]> {
        let t = self.tipada(h).filter(|t| !t.imutavel)?;
        // SAFETY: como em `bytes_da_tipada`; o empréstimo exclusivo do heap
        // impede outra fatia pela API.
        Some(unsafe { t.fatia_mut() })
    }

    /// Um valor SIMD (`cid` 19–21) com as pistas.
    pub fn novo_simd(&mut self, cid: i32, pistas: [u8; 16]) -> Ref {
        debug_assert!(layout::cid::e_simd(cid));
        let h = self.alocar(cid, 2, flags::BRUTO);
        self.bytes_mut(h)[..16].copy_from_slice(&pistas);
        h
    }

    /// As pistas do valor SIMD `h`.
    pub fn simd(&self, h: Ref) -> Option<[u8; 16]> {
        if !layout::e_objeto(h) || !self.e_objeto_vivo(h) || !cid::e_simd(self.cabecalho(h).class_id) {
            return None;
        }
        let mut b = [0u8; 16];
        b.copy_from_slice(&self.bytes(h)[..16]);
        Some(b)
    }
}

#[cfg(test)]
mod testes_tipadas {
    use super::*;

    #[test]
    fn tipadas_tipo_do_cid_cobre_as_faixas() {
        for t in 0..14u8 {
            assert_eq!(tipo_do_cid(cid::tipada(t)), Some(t));
            assert_eq!(tipo_do_cid(cid::visao(t, false)), Some(t));
            assert_eq!(tipo_do_cid(cid::visao(t, true)), Some(t));
            assert_eq!(cid_da_visao(t, true), cid::visao(t, true));
        }
        assert_eq!(tipo_do_cid(cid::BYTE_DATA_VIEW), Some(tipo::BYTE_DATA));
        assert_eq!(tipo_do_cid(cid::UNMODIFIABLE_BYTE_DATA_VIEW), Some(tipo::BYTE_DATA));
        assert_eq!(cid_da_visao(tipo::BYTE_DATA, false), cid::BYTE_DATA_VIEW);
        assert_eq!(tipo_do_cid(cid::FLOAT32X4), None);
        assert_eq!(tipo_do_cid(66), None);
        assert_eq!(tamanho_do_elemento(tipo::FLOAT64X2), 16);
        assert_eq!(tamanho_do_elemento(tipo::BYTE_DATA), 1);
    }
}
