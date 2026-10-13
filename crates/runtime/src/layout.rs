//! The layout contract of the unified space (docs/NATIVO-ESPACO-UNIFICADO.md §2 and §3.1).
//!
//! Every numeric constant that generated code uses to read or write a heap block
//! comes from here: the runtime uses it as `crate::layout` and the emitter as
//! `dartforge_runtime::layout`. The module depends on no other (it is also compiled
//! inside `RUNTIME_MAIN`, the text that the AOT passes to the standalone `rustc`).
//!
//! A block is a 16-byte header ([`crate::layout::Header`]) followed by the body, in
//! 8-byte words. The handle of a block is `block + 2`
//! ([`crate::layout::DESLOCAMENTO_DO_HANDLE`]); a `Smi` is odd and null is `0`
//! ([`crate::layout::smi`]). The collector walks the body by the format in `flags`
//! ([`crate::layout::flags::FORMA`]), without looking at the class.

/// A value in a reference position: `0` (null), a `Smi` (odd) or a block handle
/// (`block + 2`).
pub type Ref = i64;

/// The header of every block of the object space (16 bytes, fixed C layout: a
/// contract with the emitter).
///
/// * `estado`: [`estado::LIVRE`], [`estado::JOVEM`], [`estado::VELHO`],
///   [`estado::REMEMBERED`] or [`estado::PERMANENTE`];
/// * `flags`: the bits of [`flags`] (out-of-line body, body format, cards,
///   attachment, compact shape, external memory);
/// * `n`: for `INSTANCIA`, the number of fields; for `BRUTO`/`REFS`, the words of
///   the body (saturated at `u16::MAX` for a large object: the region knows the
///   real value);
/// * `class_id`: the cid;
/// * `map`: for `INSTANCIA`, the reference bits of the first 32 fields; for
///   strings, the hash (30 bits, 0 = not computed yet); for the others, 0;
/// * `metadata`: the RTI, `id + 1` (0 = none).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Header {
    pub estado: u8,
    pub flags: u8,
    pub n: u16,
    pub class_id: i32,
    pub map: u32,
    pub metadata: u32,
}

const _: () = {
    assert!(std::mem::size_of::<Header>() == TAMANHO_DO_CABECALHO);
    assert!(std::mem::align_of::<Header>() == 4);
    assert!(std::mem::offset_of!(Header, estado) == desl::ESTADO);
    assert!(std::mem::offset_of!(Header, flags) == desl::FLAGS);
    assert!(std::mem::offset_of!(Header, n) == desl::N);
    assert!(std::mem::offset_of!(Header, class_id) == desl::CLASSE);
    assert!(std::mem::offset_of!(Header, map) == desl::MAPA);
    assert!(std::mem::offset_of!(Header, metadata) == desl::METADATA);
};

/// Header bytes.
pub const TAMANHO_DO_CABECALHO: usize = 16;
/// The handle of a block is `block + DESLOCAMENTO_DO_HANDLE`.
pub const DESLOCAMENTO_DO_HANDLE: i64 = 2;
/// Size (and alignment) of a page of the object space.
pub const PAGINA: usize = 64 * 1024;
/// The beginning of each page holds the mark bitmap (one bit per word of the page).
pub const PAGE_HEAD: usize = PAGINA / 64;
/// Largest exact size class, in body words.
pub const LARGEST_CLASS: usize = 64;
/// The medium size classes, in body words: each one fills the page (`k` blocks of
/// `16 + 8w` bytes in the `PAGINA - PAGE_HEAD` usable bytes; see §2.6).
pub const CLASSES_MEDIAS: [usize; 24] = [
    70, 82, 98, 110, 124, 142, 166, 199, 222, 250, 286, 334, 401, 446, 502, 574, 670, 804, 894, 1006, 1150, 1342,
    1610, 2014,
];
/// Largest body (in words) outside a large region.
pub const LARGEST_MEDIUM: usize = 2014;
/// How many size classes: the exact `1..=64` ones and the medium ones; index 0 is not used.
pub const N_CLASSES: usize = 1 + LARGEST_CLASS + CLASSES_MEDIAS.len();
/// Largest body (in words) with inline allocation through the TLAB (`tlab[w]`, `w` in
/// `1..=TLAB_N`).
pub const TLAB_N: usize = 16;
/// Largest field index that the emitter reads inline (above it, the call
/// `dartforge_object_get`/`_set`).
pub const CAMPOS_EM_LINHA: usize = 4096;
/// Elements per card of a large list (the VM's `kSlotsPerCardLog2 = 5`).
pub const ELEMENTOS_POR_CARTAO: usize = 32;
/// Elements covered by one word of cards (64 cards).
pub const ELEMENTOS_POR_PALAVRA_DE_CARTAO: usize = 2048;
/// The first cid of the classes that are not in [`cid::DO_SDK`].
pub const PRIMEIRO_CID_LIVRE: i64 = 128;

const _: () = {
    assert!(N_CLASSES == 89);
    assert!(CLASSES_MEDIAS[CLASSES_MEDIAS.len() - 1] == LARGEST_MEDIUM);
    assert!(ELEMENTOS_POR_PALAVRA_DE_CARTAO == 64 * ELEMENTOS_POR_CARTAO);
    assert!(TLAB_N <= LARGEST_CLASS);
};

/// States of a block ([`Header::estado`]). 2 is reserved (the `MARKED` that today's
/// collector still uses during a collection in progress).
pub mod estado {
    /// Free block (on the free list or never used).
    pub const LIVRE: u8 = 0;
    /// Allocated since the last collection.
    pub const JOVEM: u8 = 1;
    /// Survived a collection.
    pub const VELHO: u8 = 3;
    /// An old block that received a reference to a young one after the last
    /// collection (the barrier put it on the remembered list).
    pub const REMEMBERED: u8 = 4;
    /// Static object, outside the heap (image section): never marked, swept,
    /// remembered or written.
    pub const PERMANENTE: u8 = 5;
}

/// Bits of [`Header::flags`].
pub mod flags {
    /// The fields (`INSTANCIA`) live in an out-of-line body.
    pub const FORA: u8 = 0x01;
    /// The mask of the body format.
    pub const FORMA: u8 = 0x06;
    /// Fields with the reference map (the collector follows the set bits).
    pub const INSTANCIA: u8 = 0x00;
    /// Bytes without references (the collector follows nothing).
    pub const BRUTO: u8 = 0x02;
    /// Word 0 raw; words 1.. are `Ref`s (the collector follows the non-null even ones).
    pub const REFS: u8 = 0x04;
    /// The large list has cards after the last element.
    pub const CARTOES: u8 = 0x08;
    /// The body holds a native pointer (`b+16`) released when the owner dies.
    pub const ATTACHMENT: u8 = 0x10;
    /// The mask of the compact shape of a `BRUTO` `_List`/`_ImmutableList`.
    pub const ELEMENTO: u8 = 0x60;
    /// `i64` elements.
    pub const ELEMENTO_INT: u8 = 0x20;
    /// Elements with the bits of an `f64`.
    pub const ELEMENTO_DOUBLE: u8 = 0x40;
    /// 0/1 elements.
    pub const ELEMENTO_BOOL: u8 = 0x60;
    /// A typed list over outside memory (`asTypedList`).
    pub const EXTERNAL: u8 = 0x80;
}

/// The fixed cids (§2.4): the same in the runtime, the emitter, the JIT and every generation.
pub mod cid {
    pub const NULL: i32 = 1;
    pub const SMI: i32 = 2;
    pub const MINT: i32 = 3;
    pub const DOUBLE: i32 = 4;
    pub const BOOL: i32 = 5;
    pub const ONE_BYTE_STRING: i32 = 6;
    pub const TWO_BYTE_STRING: i32 = 7;
    pub const LIST: i32 = 8;
    pub const IMMUTABLE_LIST: i32 = 9;
    pub const GROWABLE_LIST: i32 = 10;
    pub const CLOSURE: i32 = 11;
    pub const RECORD: i32 = 12;
    pub const CONTEXTO: i32 = 13;
    pub const CELULA: i32 = 14;
    pub const STRING_ACCUMULATOR: i32 = 15;
    pub const REGEXP_PROGRAM: i32 = 16;
    pub const SEND_PORT: i32 = 17;
    pub const CAPABILITY: i32 = 18;
    pub const FLOAT32X4: i32 = 19;
    pub const INT32X4: i32 = 20;
    pub const FLOAT64X2: i32 = 21;
    /// `_Int8List` + t, for t = `TIPO_*` 0–13 of `typed_data.rs`.
    pub const PRIMEIRA_TIPADA: i32 = 22;
    /// `_Int8ArrayView` + t.
    pub const PRIMEIRA_VISAO: i32 = 36;
    /// `_UnmodifiableInt8ArrayView` + t.
    pub const FIRST_UNMODIFIABLE_VIEW: i32 = 50;
    pub const BYTE_DATA_VIEW: i32 = 64;
    pub const UNMODIFIABLE_BYTE_DATA_VIEW: i32 = 65;
    /// How many typed list element types (`TYPE_INT8` … `TYPE_FLOAT64X2`).
    pub const ELEMENT_TYPES: i32 = 14;

    /// The library of the runtime's internal classes (with no Dart declaration): the
    /// cid stays reserved and the emitter's numbering does not find the class in the SDK.
    pub const INTERNAL: &str = "";

    /// `(cid, library, class)` of every class with a fixed cid, in cid order: the
    /// source of the emitter's numbering (`context.rs`) and of the ABI check
    /// (`dartforge_register_cids`). The library is the URI (`dart:core`), the key of
    /// the stable numbering; the runtime's internal ones have [`INTERNAL`].
    pub const DO_SDK: &[(i32, &str, &str)] = &[
        (NULL, "dart:core", "Null"),
        (SMI, "dart:core", "_Smi"),
        (MINT, "dart:core", "_Mint"),
        (DOUBLE, "dart:core", "_Double"),
        (BOOL, "dart:core", "bool"),
        (ONE_BYTE_STRING, "dart:core", "_OneByteString"),
        (TWO_BYTE_STRING, "dart:core", "_TwoByteString"),
        (LIST, "dart:core", "_List"),
        (IMMUTABLE_LIST, "dart:core", "_ImmutableList"),
        (GROWABLE_LIST, "dart:core", "_GrowableList"),
        (CLOSURE, "dart:core", "_Closure"),
        (RECORD, "dart:core", "_Record"),
        (CONTEXTO, INTERNAL, "_Contexto"),
        (CELULA, INTERNAL, "_Celula"),
        (STRING_ACCUMULATOR, INTERNAL, "_AcumuladorDeTexto"),
        (REGEXP_PROGRAM, INTERNAL, "_ProgramaDeRegExp"),
        (SEND_PORT, "dart:isolate", "_SendPort"),
        (CAPABILITY, "dart:isolate", "_Capability"),
        (FLOAT32X4, "dart:typed_data", "_Float32x4"),
        (INT32X4, "dart:typed_data", "_Int32x4"),
        (FLOAT64X2, "dart:typed_data", "_Float64x2"),
        (22, "dart:typed_data", "_Int8List"),
        (23, "dart:typed_data", "_Uint8List"),
        (24, "dart:typed_data", "_Uint8ClampedList"),
        (25, "dart:typed_data", "_Int16List"),
        (26, "dart:typed_data", "_Uint16List"),
        (27, "dart:typed_data", "_Int32List"),
        (28, "dart:typed_data", "_Uint32List"),
        (29, "dart:typed_data", "_Int64List"),
        (30, "dart:typed_data", "_Uint64List"),
        (31, "dart:typed_data", "_Float32List"),
        (32, "dart:typed_data", "_Float64List"),
        (33, "dart:typed_data", "_Float32x4List"),
        (34, "dart:typed_data", "_Int32x4List"),
        (35, "dart:typed_data", "_Float64x2List"),
        (36, "dart:typed_data", "_Int8ArrayView"),
        (37, "dart:typed_data", "_Uint8ArrayView"),
        (38, "dart:typed_data", "_Uint8ClampedArrayView"),
        (39, "dart:typed_data", "_Int16ArrayView"),
        (40, "dart:typed_data", "_Uint16ArrayView"),
        (41, "dart:typed_data", "_Int32ArrayView"),
        (42, "dart:typed_data", "_Uint32ArrayView"),
        (43, "dart:typed_data", "_Int64ArrayView"),
        (44, "dart:typed_data", "_Uint64ArrayView"),
        (45, "dart:typed_data", "_Float32ArrayView"),
        (46, "dart:typed_data", "_Float64ArrayView"),
        (47, "dart:typed_data", "_Float32x4ArrayView"),
        (48, "dart:typed_data", "_Int32x4ArrayView"),
        (49, "dart:typed_data", "_Float64x2ArrayView"),
        (50, "dart:typed_data", "_UnmodifiableInt8ArrayView"),
        (51, "dart:typed_data", "_UnmodifiableUint8ArrayView"),
        (52, "dart:typed_data", "_UnmodifiableUint8ClampedArrayView"),
        (53, "dart:typed_data", "_UnmodifiableInt16ArrayView"),
        (54, "dart:typed_data", "_UnmodifiableUint16ArrayView"),
        (55, "dart:typed_data", "_UnmodifiableInt32ArrayView"),
        (56, "dart:typed_data", "_UnmodifiableUint32ArrayView"),
        (57, "dart:typed_data", "_UnmodifiableInt64ArrayView"),
        (58, "dart:typed_data", "_UnmodifiableUint64ArrayView"),
        (59, "dart:typed_data", "_UnmodifiableFloat32ArrayView"),
        (60, "dart:typed_data", "_UnmodifiableFloat64ArrayView"),
        (61, "dart:typed_data", "_UnmodifiableFloat32x4ArrayView"),
        (62, "dart:typed_data", "_UnmodifiableInt32x4ArrayView"),
        (63, "dart:typed_data", "_UnmodifiableFloat64x2ArrayView"),
        (BYTE_DATA_VIEW, "dart:typed_data", "_ByteDataView"),
        (UNMODIFIABLE_BYTE_DATA_VIEW, "dart:typed_data", "_UnmodifiableByteDataView"),
    ];

    /// The cid of the internal typed list of type `tipo` (`TIPO_*`).
    pub const fn tipada(tipo: u8) -> i32 {
        PRIMEIRA_TIPADA + tipo as i32
    }

    /// The cid of the view of type `tipo`, modifiable or not.
    pub const fn visao(tipo: u8, unmodifiable: bool) -> i32 {
        (if unmodifiable { FIRST_UNMODIFIABLE_VIEW } else { PRIMEIRA_VISAO }) + tipo as i32
    }

    /// `_OneByteString` or `_TwoByteString`.
    pub const fn is_string(c: i32) -> bool {
        (c.wrapping_sub(ONE_BYTE_STRING) as u32) < 2
    }

    /// `_List` or `_ImmutableList`.
    pub const fn is_fixed_list(c: i32) -> bool {
        (c.wrapping_sub(LIST) as u32) < 2
    }

    /// Core list: `_List`, `_ImmutableList` or `_GrowableList`.
    pub const fn e_lista(c: i32) -> bool {
        (c.wrapping_sub(LIST) as u32) < 3
    }

    /// Internal typed list (`_Int8List` … `_Float64x2List`).
    pub const fn is_internal_typed(c: i32) -> bool {
        (c.wrapping_sub(PRIMEIRA_TIPADA) as u32) < ELEMENT_TYPES as u32
    }

    /// Any typed list or view (22..=65).
    pub const fn is_typed(c: i32) -> bool {
        (c.wrapping_sub(PRIMEIRA_TIPADA) as u32) < 44
    }

    /// Unmodifiable view (`_UnmodifiableXArrayView`, `_UnmodifiableByteDataView`).
    pub const fn is_unmodifiable_view(c: i32) -> bool {
        (c.wrapping_sub(FIRST_UNMODIFIABLE_VIEW) as u32) < ELEMENT_TYPES as u32 || c == UNMODIFIABLE_BYTE_DATA_VIEW
    }

    /// SIMD value (`_Float32x4`, `_Int32x4`, `_Float64x2`).
    pub const fn is_simd(c: i32) -> bool {
        (c.wrapping_sub(FLOAT32X4) as u32) < 3
    }
}

/// Offsets from the block (the handle is block + 2: the word at `b + k` is at
/// `h + k - 2`).
pub mod desl {
    pub const ESTADO: usize = 0;
    pub const FLAGS: usize = 1;
    pub const N: usize = 2;
    pub const CLASSE: usize = 4;
    pub const MAPA: usize = 8;
    /// The hash of strings lives in the place of the map.
    pub const HASH_DO_TEXTO: usize = 8;
    pub const METADATA: usize = 12;
    /// The first word of the body.
    pub const CORPO: usize = 16;
    /// Strings, `_List`, `_ImmutableList`, `_GrowableList`, typed lists and views.
    pub const COMPRIMENTO: usize = 16;
    /// The code units of a string.
    pub const UNIDADES: usize = 24;
    /// The first element of a `_List`/`_ImmutableList`.
    pub const ELEMENTOS: usize = 24;
    /// The value of `_Mint`, `_Double`, `bool`, and the SIMD lanes.
    pub const VALOR: usize = 16;
    /// The address of the bytes of a typed list (internal, external or view).
    pub const DADOS: usize = 24;
    /// The bytes of an internal typed list.
    pub const INTERNAL_BYTES: usize = 32;
    pub const BASE_DA_VISAO: usize = 32;
    pub const DESLOCAMENTO_DA_VISAO: usize = 40;
    /// The storage (`_List`) of a `_GrowableList`.
    pub const EXPANSIVEL_DADOS: usize = 24;
    pub const CLOSURE_CODIGO: usize = 16;
    pub const CLOSURE_CONTEXTO: usize = 24;
    pub const CLOSURE_TIPADO: usize = 32;
    pub const CLOSURE_ABI: usize = 40;
    pub const RECORD_FORMA: usize = 16;
    pub const RECORD_CAMPOS: usize = 24;
    /// The native pointer of a block with `ATTACHMENT`.
    pub const ATTACHMENT: usize = 16;
}

/// Offsets of the thread's `Context` (§3.4).
pub mod contexto {
    pub const PENDING: usize = 0;
    pub const TOPO: usize = 8;
    pub const AREAS: usize = 16;
    pub const N_AREAS: usize = 24;
    pub const INTERRUPT: usize = 32;
    pub const EMPTIES: usize = 40;
    pub const REGISTERED: usize = 48;
    pub const N_REGISTERED: usize = 56;
    /// The cursor of the TLAB of `w` words at `TLAB + 16·w`, the end at
    /// `TLAB + 16·w + 8`, `w` in `1..=TLAB_N`.
    pub const TLAB: usize = 64;
    pub const SUBTIPOS: usize = 336;
    pub const N_SUBTIPOS: usize = 344;
    pub const LARGURA_SUBTIPOS: usize = 352;
    /// The handles of the static `true` and `false` boxes of the thread's runtime
    /// (generated code reads them from here, not from the data symbols: between the
    /// executable and the SDK DLL only the active runtime counts, §4.10 item 9).
    pub const TRUE: usize = 360;
    pub const FALSE: usize = 368;
    /// The thread's native stack limit (`Context::stack_limit`).
    pub const LIMITE_DA_PILHA: usize = 376;

    /// The cursor of the TLAB of `w` words.
    pub const fn tlab_cursor(w: usize) -> usize {
        TLAB + 16 * w
    }

    /// The end of the TLAB of `w` words.
    pub const fn tlab_fim(w: usize) -> usize {
        TLAB + 16 * w + 8
    }

    const _: () = assert!(tlab_fim(super::TLAB_N) + 8 == SUBTIPOS);
}

/// A `Ref` with a tagged `Smi` (R10, docs/NATIVO-PLANO.md §6.2 and §7.1).
///
/// A `Ref` in generated code is an `i64` with the tag in the low bit, like the VM's
/// `Smi` (`runtime/vm/object.h`; there `kSmiTag = 0` and a heap pointer has bit 1 —
/// here the bit is inverted so that `0` remains null):
///
/// * `0` — null;
/// * **even** and positive — a heap handle (`block + 2`, [`is_object`]);
/// * **odd** — a small `int`, `(v << 1) | 1`, for `v` in `[-2^62, 2^62)`: the
///   `Smi`. It does not allocate, is not a root, and the collector never follows it.
///
/// An `int` outside that range in a `Ref` position goes to the heap as a `_Mint`.
/// The form is canonical: a value that fits in a `Smi` is never boxed.
pub mod smi {
    /// Smallest and largest `int` that fit in a `Smi` (63 signed bits).
    pub const MIN: i64 = -(1 << 62);
    pub const MAX: i64 = (1 << 62) - 1;

    /// The `Ref` of an `int` that fits in a `Smi`.
    #[inline]
    pub fn de(v: i64) -> Option<i64> {
        (MIN..=MAX).contains(&v).then_some((v << 1) | 1)
    }

    /// Is the `Ref` a `Smi`?
    #[inline]
    pub fn is_smi(r: i64) -> bool {
        r & 1 == 1
    }

    /// The value of a `Smi` (arithmetic shift).
    #[inline]
    pub fn value(r: i64) -> i64 {
        r >> 1
    }

    /// Does the `Ref` point into the heap (it is neither null nor a `Smi`)?
    #[inline]
    pub fn e_handle(r: i64) -> bool {
        r != 0 && r & 1 == 0
    }
}

/// Is the `Ref` the handle of a block (`block + 2`, blocks aligned to 8)? A negative
/// value is never an object.
#[inline]
pub fn is_object(h: Ref) -> bool {
    h & (7 | i64::MIN) == 2
}

/// Extension words of the reference map for `n` fields (the first 32 bits are in the
/// header).
#[inline]
pub const fn palavras_do_mapa(n: usize) -> usize {
    if n > 32 { (n - 32).div_ceil(64) } else { 0 }
}

/// Fields the block keeps: at least one (the place for the free-list link and for
/// the address of the out-of-line body).
#[inline]
pub const fn capacidade(n: usize) -> usize {
    if n == 0 { 1 } else { n }
}

/// Body words of an `INSTANCIA` with `n` fields.
#[inline]
pub const fn palavras_de_instancia(n: usize) -> usize {
    capacidade(n) + palavras_do_mapa(n)
}

/// Body words of a string of `len` units: the length and the units (1 or 2 bytes
/// each), padded with zeros up to the word.
#[inline]
pub const fn palavras_de_texto(len: usize, dois: bool) -> usize {
    1 + (len * if dois { 2 } else { 1 }).div_ceil(8)
}

/// Does a list of `len` elements have cards (is it a large object)?
#[inline]
pub const fn has_cards(len: usize) -> bool {
    1 + len > LARGEST_MEDIUM
}

/// Card words after the last element of a large list.
#[inline]
pub const fn card_words(len: usize) -> usize {
    if has_cards(len) { len.div_ceil(ELEMENTOS_POR_PALAVRA_DE_CARTAO) } else { 0 }
}

/// Body words of a `_List`/`_ImmutableList` of `len` elements: the length, the
/// elements and, if large, the cards.
#[inline]
pub const fn list_words(len: usize) -> usize {
    1 + len + card_words(len)
}

/// Body words of an internal typed list of `len` elements of `tamanho_do_elemento`
/// bytes: the length, the data address and the bytes.
#[inline]
pub const fn typed_words(len: usize, tamanho_do_elemento: usize) -> usize {
    2 + (len * tamanho_do_elemento).div_ceil(8)
}

/// The size class of a body of `words` words: `1..=64` exact (0 words uses class 1),
/// `65..=88` medium; `None` above [`LARGEST_MEDIUM`] (large region).
#[inline]
pub const fn size_class(words: usize) -> Option<usize> {
    if words <= LARGEST_CLASS {
        return Some(if words == 0 { 1 } else { words });
    }
    let mut i = 0;
    while i < CLASSES_MEDIAS.len() {
        if words <= CLASSES_MEDIAS[i] {
            return Some(LARGEST_CLASS + 1 + i);
        }
        i += 1;
    }
    None
}

/// The body words of a block of class `class` (`1..N_CLASSES`).
#[inline]
pub const fn class_words(class: usize) -> usize {
    if class <= LARGEST_CLASS { class } else { CLASSES_MEDIAS[class - LARGEST_CLASS - 1] }
}

/// Bytes of a block with a body of `words` words.
#[inline]
pub const fn bytes_do_bloco(words: usize) -> usize {
    TAMANHO_DO_CABECALHO + 8 * words
}

/// Word 0 of the header, as the inline allocation writes it:
/// `estado | flags << 8 | n << 16 | cid << 32`, with `n` saturated at `u16::MAX`
/// (for `INSTANCIA`, `n` is the number of fields; for the others, the body words).
#[inline]
pub const fn palavra_do_cabecalho(estado: u8, flags: u8, n: usize, cid: i32) -> u64 {
    let n = if n > u16::MAX as usize { u16::MAX as u64 } else { n as u64 };
    estado as u64 | (flags as u64) << 8 | n << 16 | (cid as u32 as u64) << 32
}

/// The `hashCode` of a string: the VM's `StringHasher` (`runtime/vm/hash.h`,
/// `CombineHashes` per unit and `FinalizeHash` in 30 bits), with 0 replaced by 1. It
/// is what the runtime writes into `map` and what the emitter writes into static
/// literals.
pub fn hash_de_texto(unidades: impl IntoIterator<Item = u16>) -> u32 {
    let mut h: u32 = 0;
    for u in unidades {
        h = h.wrapping_add(u32::from(u));
        h = h.wrapping_add(h << 10);
        h ^= h >> 6;
    }
    h = h.wrapping_add(h << 3);
    h ^= h >> 11;
    h = h.wrapping_add(h << 15);
    h &= (1u32 << 30) - 1;
    if h == 0 { 1 } else { h }
}

#[cfg(test)]
mod testes_layout {
    use super::*;

    #[test]
    fn layout_size_class_is_monotonic_and_covers() {
        let mut previous = 0;
        for w in 0..=LARGEST_MEDIUM {
            let c = size_class(w).expect("body class up to the largest medium one");
            assert!(c >= 1 && c < N_CLASSES, "class {c} of {w} words out of range");
            assert!(c >= previous, "size_class is not monotonic at {w}");
            assert!(class_words(c) >= w, "class {c} does not fit {w} words");
            // The smallest class that fits: the previous one does not fit.
            if c > 1 {
                assert!(class_words(c - 1) < w.max(1), "class {c} is not the smallest for {w}");
            }
            previous = c;
        }
        assert_eq!(size_class(LARGEST_MEDIUM + 1), None);
        assert_eq!(size_class(0), Some(1));
        assert_eq!(size_class(64), Some(64));
        assert_eq!(size_class(65), Some(65));
        assert_eq!(class_words(N_CLASSES - 1), LARGEST_MEDIUM);
    }

    #[test]
    fn layout_medium_classes_fill_the_page() {
        let useful = PAGINA - PAGE_HEAD;
        let blocks = [112, 96, 80, 72, 64, 56, 48, 40, 36, 32, 28, 24, 20, 18, 16, 14, 12, 10, 9, 8, 7, 6, 5, 4];
        for (i, &w) in CLASSES_MEDIAS.iter().enumerate() {
            let k = useful / bytes_do_bloco(w);
            assert_eq!(k, blocks[i], "medium class of {w} words");
            // The largest that fits the same number of blocks: one more word loses one.
            assert!(useful / bytes_do_bloco(w + 1) < k, "the class of {w} words does not fill the page");
        }
        // Maximum internal waste of 25% (between 1610 and 2014).
        let mut previous = LARGEST_CLASS;
        for &w in &CLASSES_MEDIAS {
            assert!((w - previous - 1) * 4 <= w, "internal waste above 25% before {w}");
            previous = w;
        }
    }

    #[test]
    fn layout_string_hash_equals_vm() {
        for s in ["", "a", "abc", "hello world", "ç€😀", "0123456789abcdef0123456789"] {
            let t = crate::heap::Texto::de_str(s);
            let u: Vec<u16> = s.encode_utf16().collect();
            assert_eq!(i64::from(hash_de_texto(u.iter().copied())), t.hash_vm(), "hash of {s:?}");
        }
        // Lone surrogate and a unit above 0xFF.
        let u = vec![0xD800, 0x61, 0x1234, 0xFF];
        let t = crate::heap::Texto::from_units(u.clone());
        assert_eq!(i64::from(hash_de_texto(u)), t.hash_vm());
        assert_ne!(hash_de_texto([]), 0);
        assert!(hash_de_texto("xyz".encode_utf16()) < 1 << 30);
    }

    #[test]
    fn layout_header_word_and_handles() {
        let p = palavra_do_cabecalho(estado::JOVEM, flags::BRUTO, 3, cid::ONE_BYTE_STRING);
        assert_eq!(p, 1 | 0x02 << 8 | 3 << 16 | 6 << 32);
        assert_eq!(palavra_do_cabecalho(estado::PERMANENTE, 0, 100_000, 200) >> 16 & 0xFFFF, 0xFFFF);
        assert_eq!(palavra_do_cabecalho(0, 0, 0, -1) >> 32, 0xFFFF_FFFF);
        assert!(is_object(0x1000 + DESLOCAMENTO_DO_HANDLE));
        assert!(!is_object(0));
        assert!(!is_object(0x1000));
        assert!(!is_object(0x1004));
        assert!(!is_object(0x1006));
        assert!(!is_object(smi::de(7).unwrap()));
        assert!(!is_object(i64::MIN | 2));
        assert_eq!(smi::value(smi::de(-5).unwrap()), -5);
        assert_eq!(smi::de(smi::MAX + 1), None);
    }

    #[test]
    fn layout_body_sizes() {
        assert_eq!(palavras_de_texto(0, false), 1);
        assert_eq!(palavras_de_texto(8, false), 2);
        assert_eq!(palavras_de_texto(9, false), 3);
        assert_eq!(palavras_de_texto(4, true), 2);
        assert_eq!(palavras_de_texto(5, true), 3);
        assert_eq!(list_words(0), 1);
        assert_eq!(list_words(LARGEST_MEDIUM - 1), LARGEST_MEDIUM);
        assert!(!has_cards(LARGEST_MEDIUM - 1));
        assert!(has_cards(LARGEST_MEDIUM));
        assert_eq!(list_words(LARGEST_MEDIUM), 1 + LARGEST_MEDIUM + 1);
        assert_eq!(list_words(4096), 1 + 4096 + 2);
        assert_eq!(typed_words(3, 1), 3);
        assert_eq!(typed_words(2, 8), 4);
        assert_eq!(palavras_de_instancia(0), 1);
        assert_eq!(palavras_de_instancia(33), 34);
    }

    #[test]
    fn layout_sdk_cids_in_order_and_predicates() {
        for (i, &(c, _, _)) in cid::DO_SDK.iter().enumerate() {
            assert_eq!(c, i as i32 + 1, "DO_SDK out of order at position {i}");
            assert!(i64::from(c) < PRIMEIRO_CID_LIVRE);
        }
        assert_eq!(cid::DO_SDK.len(), cid::UNMODIFIABLE_BYTE_DATA_VIEW as usize);
        assert_eq!(cid::tipada(1), 23);
        assert_eq!(cid::DO_SDK[cid::tipada(13) as usize - 1].2, "_Float64x2List");
        assert_eq!(cid::DO_SDK[cid::visao(1, false) as usize - 1].2, "_Uint8ArrayView");
        assert_eq!(cid::DO_SDK[cid::visao(13, true) as usize - 1].2, "_UnmodifiableFloat64x2ArrayView");
        for c in 0..200 {
            assert_eq!(cid::is_string(c), c == 6 || c == 7);
            assert_eq!(cid::is_fixed_list(c), c == 8 || c == 9);
            assert_eq!(cid::e_lista(c), (8..=10).contains(&c));
            assert_eq!(cid::is_internal_typed(c), (22..=35).contains(&c));
            assert_eq!(cid::is_typed(c), (22..=65).contains(&c));
            assert_eq!(cid::is_unmodifiable_view(c), (50..=63).contains(&c) || c == 65);
            assert_eq!(cid::is_simd(c), (19..=21).contains(&c));
        }
    }
}
