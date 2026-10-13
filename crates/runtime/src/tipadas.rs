//! Listas tipadas, visões e SIMD no espaço unificado (P4,
//! docs/NATIVO-ESPACO-UNIFICADO.md §2.5 e §3.3).
//!
//! A lista tipada interna (cids 22–35) é `BRUTO`: o comprimento em `b+16`, o
//! endereço dos dados em `b+24` (apontando para `b+32`, ou para a memória de fora
//! com `EXTERNAL`) e os bytes. A visão (36–65) é `INSTANCIA` de quatro campos: o
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
pub const fn type_of_cid(c: i32) -> Option<u8> {
    if cid::is_internal_typed(c) {
        Some((c - cid::PRIMEIRA_TIPADA) as u8)
    } else if c == cid::BYTE_DATA_VIEW || c == cid::UNMODIFIABLE_BYTE_DATA_VIEW {
        Some(tipo::BYTE_DATA)
    } else if cid::is_typed(c) {
        Some(((c - cid::PRIMEIRA_VISAO) % cid::ELEMENT_TYPES) as u8)
    } else {
        None
    }
}

/// O cid da visão do tipo `t` (inclusive o `ByteData`), modificável ou não.
pub const fn view_cid(t: u8, unmodifiable: bool) -> i32 {
    if t == tipo::BYTE_DATA {
        if unmodifiable { cid::UNMODIFIABLE_BYTE_DATA_VIEW } else { cid::BYTE_DATA_VIEW }
    } else {
        cid::visao(t, unmodifiable)
    }
}

/// O endereço do bloco de um handle.
const fn block(h: Ref) -> i64 {
    h - layout::DESLOCAMENTO_DO_HANDLE
}

/// Índices das palavras do corpo (a partir de `b+16`).
const WORD_LENGTH: usize = (layout::desl::COMPRIMENTO - layout::desl::CORPO) / 8;
const WORD_DATA: usize = (layout::desl::DADOS - layout::desl::CORPO) / 8;
const WORD_BASE: usize = (layout::desl::BASE_DA_VISAO - layout::desl::CORPO) / 8;
const WORD_OFFSET: usize = (layout::desl::DESLOCAMENTO_DA_VISAO - layout::desl::CORPO) / 8;
/// Campos de uma visão.
const VIEW_FIELDS: usize = 4;

const _: () = {
    assert!(WORD_LENGTH == 0 && WORD_DATA == 1 && WORD_BASE == 2 && WORD_OFFSET == 3);
    assert!(layout::desl::INTERNAL_BYTES == layout::desl::DADOS + 8);
};

/// A vista de uma lista tipada ou visão.
#[derive(Clone, Copy, Debug)]
pub struct TypedRef {
    pub cid: i32,
    /// O `TIPO_*` do elemento ([`tipo`]).
    pub tipo: u8,
    /// O comprimento em elementos.
    pub len: usize,
    /// O primeiro byte.
    pub data: *mut u8,
    /// O tamanho em bytes.
    pub bytes: usize,
    pub unmodifiable: bool,
    pub external_one: bool,
    /// A lista de base de uma visão.
    pub base: Option<Ref>,
    /// O deslocamento em bytes de uma visão na base.
    pub offset: usize,
}

impl TypedRef {
    /// Os bytes. Valem enquanto a lista vive e não há alocação sem raiz para ela
    /// (§2.14: o coletor não move).
    ///
    /// # Safety
    /// `self` saiu de [`Heap::tipada`] e a lista ainda vive.
    #[allow(unsafe_code)]
    pub unsafe fn piece<'a>(self) -> &'a [u8] {
        if self.bytes == 0 || self.data.is_null() {
            return &[];
        }
        // SAFETY: `data..data+bytes` é a memória da lista (a do bloco, ou a de
        // fora de `asTypedList`), viva pelo contrato do chamador.
        unsafe { std::slice::from_raw_parts(self.data, self.bytes) }
    }

    /// Os bytes graváveis (as regras de [`TypedRef::piece`]); o chamador não
    /// mantém outra fatia da mesma memória.
    ///
    /// # Safety
    /// Como em [`TypedRef::piece`], e sem outra referência viva aos mesmos bytes.
    #[allow(unsafe_code)]
    pub unsafe fn slice_mut<'a>(self) -> &'a mut [u8] {
        if self.bytes == 0 || self.data.is_null() {
            return &mut [];
        }
        // SAFETY: como em `piece`; a exclusividade é do chamador.
        unsafe { std::slice::from_raw_parts_mut(self.data, self.bytes) }
    }
}

impl Heap {
    /// Uma lista tipada interna do tipo `tipo` com `len` elementos zerados.
    pub fn new_typed(&mut self, tipo: u8, len: usize) -> Ref {
        debug_assert!(i32::from(tipo) < cid::ELEMENT_TYPES, "lista tipada interna de ByteData");
        // Um comprimento que não cabe no endereçamento satura: o teto do heap
        // encerra com a mensagem de memória, como a VM com `OutOfMemoryError`.
        let bytes = len.checked_mul(tamanho_do_elemento(tipo)).unwrap_or(usize::MAX / 2);
        let words = 2 + bytes.div_ceil(8);
        let h = self.allocate(cid::tipada(tipo), words, flags::BRUTO);
        let p = self.words_mut(h);
        p[WORD_LENGTH] = len as i64;
        p[WORD_DATA] = block(h) + layout::desl::INTERNAL_BYTES as i64;
        h
    }

    /// Uma lista tipada sobre `len` elementos da memória nativa em `address`
    /// (`asTypedList`): `BRUTO` + `EXTERNAL`, sem cópia.
    pub fn external_typed(&mut self, tipo: u8, address: *mut u8, len: usize) -> Ref {
        let h = self.allocate(cid::tipada(tipo), 2, flags::BRUTO | flags::EXTERNAL);
        let p = self.words_mut(h);
        p[WORD_LENGTH] = len as i64;
        p[WORD_DATA] = address as i64;
        h
    }

    /// Uma visão (`cid`) de `len` elementos sobre `base`, a partir de `offset`
    /// bytes. Uma visão passada como base é resolvida para a lista interna dela
    /// (os deslocamentos se somam), como no `typed_data_patch.dart`.
    pub fn new_view(&mut self, cid: i32, base: Ref, offset: usize, len: usize) -> Ref {
        let Some(b) = self.tipada(base) else {
            panic!("bug do compilador: visão sobre algo que não é lista tipada");
        };
        let (internal_one, offset) = match b.base {
            Some(i) => (i, b.offset + offset),
            None => (base, offset),
        };
        // O endereço é calculado antes de alocar: a base não se move.
        let data = (b.data as i64).wrapping_add((offset - b.offset) as i64);
        let frame = self.push_owning_frame(1);
        self.set_root(frame, 0, internal_one);
        let h = self.allocate_instance(cid, VIEW_FIELDS);
        // Objeto novo (jovem): as gravações não precisam de barreira; a da base
        // acende o bit de referência no mapa.
        self.set_field(h, WORD_LENGTH, len as i64, false);
        self.set_field(h, WORD_DATA, data, false);
        self.set_field(h, WORD_BASE, internal_one, true);
        self.set_field(h, WORD_OFFSET, offset as i64, false);
        // A visão já guarda a base antes de soltar a cópia temporária.
        self.pop_frame(frame);
        h
    }

    /// `h` é lista tipada ou visão (cid 22..=65)?
    pub fn is_typed(&self, h: Ref) -> bool {
        layout::is_object(h) && self.is_live_object(h) && cid::is_typed(self.header(h).class_id)
    }

    /// A vista da lista tipada ou visão `h`; `None` para o que não é.
    pub fn tipada(&self, h: Ref) -> Option<TypedRef> {
        if !layout::is_object(h) || !self.is_live_object(h) {
            return None;
        }
        let cab = self.header(h);
        let c = cab.class_id;
        let t = type_of_cid(c)?;
        let p = self.words(h);
        let len = p[WORD_LENGTH] as usize;
        let data = p[WORD_DATA] as usize as *mut u8;
        let bytes = len * tamanho_do_elemento(t);
        Some(if cid::is_internal_typed(c) {
            TypedRef {
                cid: c,
                tipo: t,
                len,
                data,
                bytes,
                unmodifiable: false,
                external_one: cab.flags & flags::EXTERNAL != 0,
                base: None,
                offset: 0,
            }
        } else {
            TypedRef {
                cid: c,
                tipo: t,
                len,
                data,
                bytes,
                unmodifiable: cid::is_unmodifiable_view(c),
                external_one: false,
                base: Some(p[WORD_BASE]),
                offset: p[WORD_OFFSET] as usize,
            }
        })
    }

    /// Os bytes da lista tipada ou visão `h`.
    #[allow(unsafe_code)]
    pub fn typed_bytes(&self, h: Ref) -> Option<&[u8]> {
        let t = self.tipada(h)?;
        // SAFETY: a lista vive enquanto dura o empréstimo do heap (nenhuma coleta
        // com `&self`).
        Some(unsafe { t.piece() })
    }

    /// Os bytes graváveis; `None` se `h` não é lista tipada ou é não modificável.
    #[allow(unsafe_code)]
    pub fn typed_bytes_mut(&mut self, h: Ref) -> Option<&mut [u8]> {
        let t = self.tipada(h).filter(|t| !t.unmodifiable)?;
        // SAFETY: como em `typed_bytes`; o empréstimo exclusivo do heap
        // impede outra fatia pela API.
        Some(unsafe { t.slice_mut() })
    }

    /// Um valor SIMD (`cid` 19–21) com as pistas.
    pub fn new_simd(&mut self, cid: i32, hints_list: [u8; 16]) -> Ref {
        debug_assert!(layout::cid::is_simd(cid));
        let h = self.allocate(cid, 2, flags::BRUTO);
        self.bytes_mut(h)[..16].copy_from_slice(&hints_list);
        h
    }

    /// As pistas do valor SIMD `h`.
    pub fn simd(&self, h: Ref) -> Option<[u8; 16]> {
        if !layout::is_object(h) || !self.is_live_object(h) || !cid::is_simd(self.header(h).class_id) {
            return None;
        }
        let mut b = [0u8; 16];
        b.copy_from_slice(&self.bytes(h)[..16]);
        Some(b)
    }
}

#[cfg(test)]
mod tests_typed {
    use super::*;

    #[test]
    fn typed_type_of_cid_covers_ranges() {
        for t in 0..14u8 {
            assert_eq!(type_of_cid(cid::tipada(t)), Some(t));
            assert_eq!(type_of_cid(cid::visao(t, false)), Some(t));
            assert_eq!(type_of_cid(cid::visao(t, true)), Some(t));
            assert_eq!(view_cid(t, true), cid::visao(t, true));
        }
        assert_eq!(type_of_cid(cid::BYTE_DATA_VIEW), Some(tipo::BYTE_DATA));
        assert_eq!(type_of_cid(cid::UNMODIFIABLE_BYTE_DATA_VIEW), Some(tipo::BYTE_DATA));
        assert_eq!(view_cid(tipo::BYTE_DATA, false), cid::BYTE_DATA_VIEW);
        assert_eq!(type_of_cid(cid::FLOAT32X4), None);
        assert_eq!(type_of_cid(66), None);
        assert_eq!(tamanho_do_elemento(tipo::FLOAT64X2), 16);
        assert_eq!(tamanho_do_elemento(tipo::BYTE_DATA), 1);
    }
}
