//! Typed lists, views and SIMD in the unified space (P4,
//! docs/NATIVO-ESPACO-UNIFICADO.md §2.5 and §3.3).
//!
//! The internal typed list (cids 22–35) is `RAW`: the length at `b+16`, the
//! data address at `b+24` (pointing to `b+32`, or to outside memory
//! with `EXTERNAL`) and the bytes. The view (36–65) is an `INSTANCE` of four fields: the
//! length and the data at the same offsets (computed at creation: the base
//! does not move), the base (`Ref`, the internal list) and the offset in bytes. The
//! SIMD values (19–21) are two-word `RAW`s, the lanes.
//!
//! Length and data at the same place in all three forms (the VM's `PointerBase::data_`):
//! the generated code reads `len = [h+14]` and `p = [h+22]` without knowing which one it is
//! (`llvm/tipados_ir.rs`). Neither changes while the list lives.

use crate::heap::Heap;
use crate::layout::{self, cid, flags, Ref};

/// The element types (`TIPO_*`, the order of the classes of §2.4: the cid of the internal
/// list is `cid::typed(ty)`).
pub mod ty {
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
    /// `ByteData` (only as a view: `_ByteDataView`, cids 64 and 65); one-byte
    /// elements.
    pub const BYTE_DATA: u8 = 14;
}

/// Bytes per element of a type.
pub const fn element_size(t: u8) -> usize {
    match t {
        ty::INT16 | ty::UINT16 => 2,
        ty::INT32 | ty::UINT32 | ty::FLOAT32 => 4,
        ty::INT64 | ty::UINT64 | ty::FLOAT64 => 8,
        ty::FLOAT32X4 | ty::INT32X4 | ty::FLOAT64X2 => 16,
        _ => 1,
    }
}

/// The element type of the typed list or view with cid `c` (`None` outside 22..=65).
pub const fn type_of_cid(c: i32) -> Option<u8> {
    if cid::is_internal_typed(c) {
        Some((c - cid::FIRST_TYPED) as u8)
    } else if c == cid::BYTE_DATA_VIEW || c == cid::UNMODIFIABLE_BYTE_DATA_VIEW {
        Some(ty::BYTE_DATA)
    } else if cid::is_typed(c) {
        Some(((c - cid::FIRST_VIEW) % cid::ELEMENT_TYPES) as u8)
    } else {
        None
    }
}

/// The cid of the view of type `t` (including `ByteData`), modifiable or not.
pub const fn view_cid(t: u8, unmodifiable: bool) -> i32 {
    if t == ty::BYTE_DATA {
        if unmodifiable { cid::UNMODIFIABLE_BYTE_DATA_VIEW } else { cid::BYTE_DATA_VIEW }
    } else {
        cid::view(t, unmodifiable)
    }
}

/// The block address of a handle.
const fn block(h: Ref) -> i64 {
    h - layout::HANDLE_OFFSET
}

/// Indices of the body words (from `b+16`).
const WORD_LENGTH: usize = (layout::offsets::LENGTH - layout::offsets::BODY) / 8;
const WORD_DATA: usize = (layout::offsets::DATA - layout::offsets::BODY) / 8;
const WORD_BASE: usize = (layout::offsets::VIEW_BASE - layout::offsets::BODY) / 8;
const WORD_OFFSET: usize = (layout::offsets::VIEW_OFFSET - layout::offsets::BODY) / 8;
/// Fields of a view.
const VIEW_FIELDS: usize = 4;

const _: () = {
    assert!(WORD_LENGTH == 0 && WORD_DATA == 1 && WORD_BASE == 2 && WORD_OFFSET == 3);
    assert!(layout::offsets::INTERNAL_BYTES == layout::offsets::DATA + 8);
};

/// The view of a typed list or view.
#[derive(Clone, Copy, Debug)]
pub struct TypedRef {
    pub cid: i32,
    /// The `TIPO_*` of the element ([`ty`]).
    pub ty: u8,
    /// The length in elements.
    pub len: usize,
    /// The first byte.
    pub data: *mut u8,
    /// The size in bytes.
    pub bytes: usize,
    pub unmodifiable: bool,
    pub external_one: bool,
    /// The base list of a view.
    pub base: Option<Ref>,
    /// The offset in bytes of a view into the base.
    pub offset: usize,
}

impl TypedRef {
    /// The bytes. Valid while the list lives and there is no rootless allocation for it
    /// (§2.14: the collector does not move).
    ///
    /// # Safety
    /// `self` came from [`Heap::typed`] and the list is still alive.
    #[allow(unsafe_code)]
    pub unsafe fn piece<'a>(self) -> &'a [u8] {
        if self.bytes == 0 || self.data.is_null() {
            return &[];
        }
        // SAFETY: `data..data+bytes` is the list's memory (the block's, or the
        // outside one of `asTypedList`), alive by the caller's contract.
        unsafe { std::slice::from_raw_parts(self.data, self.bytes) }
    }

    /// The writable bytes (the rules of [`TypedRef::piece`]); the caller does not
    /// keep another slice of the same memory.
    ///
    /// # Safety
    /// As in [`TypedRef::piece`], and without another live reference to the same bytes.
    #[allow(unsafe_code)]
    pub unsafe fn slice_mut<'a>(self) -> &'a mut [u8] {
        if self.bytes == 0 || self.data.is_null() {
            return &mut [];
        }
        // SAFETY: as in `piece`; exclusivity is up to the caller.
        unsafe { std::slice::from_raw_parts_mut(self.data, self.bytes) }
    }
}

impl Heap {
    /// An internal typed list of type `ty` with `len` zeroed elements.
    pub fn new_typed(&mut self, ty: u8, len: usize) -> Ref {
        debug_assert!(i32::from(ty) < cid::ELEMENT_TYPES, "internal typed list of ByteData");
        // A length that does not fit the address space saturates: the heap ceiling
        // ends with the memory message, like the VM with `OutOfMemoryError`.
        let bytes = len.checked_mul(element_size(ty)).unwrap_or(usize::MAX / 2);
        let words = 2 + bytes.div_ceil(8);
        let h = self.allocate(cid::typed(ty), words, flags::RAW);
        let p = self.fresh_body(h);
        // SAFETY: the body of the block just allocated has `words ≥ 2` words: the
        // length and the address of the data.
        #[allow(unsafe_code)]
        unsafe {
            p.add(WORD_LENGTH).write(len as i64);
            p.add(WORD_DATA).write(block(h) + layout::offsets::INTERNAL_BYTES as i64);
        }
        h
    }

    /// A typed list over `len` elements of the native memory at `address`
    /// (`asTypedList`): `RAW` + `EXTERNAL`, without copying.
    pub fn external_typed(&mut self, ty: u8, address: *mut u8, len: usize) -> Ref {
        let h = self.allocate(cid::typed(ty), 2, flags::RAW | flags::EXTERNAL);
        let p = self.words_mut(h);
        p[WORD_LENGTH] = len as i64;
        p[WORD_DATA] = address as i64;
        h
    }

    /// A view (`cid`) of `len` elements over `base`, starting at `offset`
    /// bytes. A view passed as the base is resolved to its internal list
    /// (the offsets add up), as in `typed_data_patch.dart`.
    pub fn new_view(&mut self, cid: i32, base: Ref, offset: usize, len: usize) -> Ref {
        let Some(b) = self.typed(base) else {
            panic!("compiler bug: view over something that is not a typed list");
        };
        let (internal_one, offset) = match b.base {
            Some(i) => (i, b.offset + offset),
            None => (base, offset),
        };
        // The address is computed before allocating: the base does not move.
        let data = (b.data as i64).wrapping_add((offset - b.offset) as i64);
        let frame = self.push_owning_frame(1);
        self.set_root(frame, 0, internal_one);
        let h = self.allocate_instance(cid, VIEW_FIELDS);
        // New (young) object: the writes need no barrier; the base's one
        // sets the reference bit in the map.
        self.set_field(h, WORD_LENGTH, len as i64, false);
        self.set_field(h, WORD_DATA, data, false);
        self.set_field(h, WORD_BASE, internal_one, true);
        self.set_field(h, WORD_OFFSET, offset as i64, false);
        // The view already keeps the base before releasing the temporary copy.
        self.pop_frame(frame);
        h
    }

    /// Is `h` a typed list or view (cid 22..=65)?
    pub fn is_typed(&self, h: Ref) -> bool {
        layout::is_object(h) && self.is_live_object(h) && cid::is_typed(self.header(h).class_id)
    }

    /// The view of the typed list or view `h`; `None` for what is not.
    pub fn typed(&self, h: Ref) -> Option<TypedRef> {
        if !layout::is_object(h) || !self.is_live_object(h) {
            return None;
        }
        let cab = self.header(h);
        let c = cab.class_id;
        let t = type_of_cid(c)?;
        let p = self.words(h);
        let len = p[WORD_LENGTH] as usize;
        let data = p[WORD_DATA] as usize as *mut u8;
        let bytes = len * element_size(t);
        Some(if cid::is_internal_typed(c) {
            TypedRef {
                cid: c,
                ty: t,
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
                ty: t,
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

    /// The bytes of the typed list or view `h`.
    #[allow(unsafe_code)]
    pub fn typed_bytes(&self, h: Ref) -> Option<&[u8]> {
        let t = self.typed(h)?;
        // SAFETY: the list lives while the heap borrow lasts (no collection
        // with `&self`).
        Some(unsafe { t.piece() })
    }

    /// The writable bytes; `None` if `h` is not a typed list or is unmodifiable.
    #[allow(unsafe_code)]
    pub fn typed_bytes_mut(&mut self, h: Ref) -> Option<&mut [u8]> {
        let t = self.typed(h).filter(|t| !t.unmodifiable)?;
        // SAFETY: as in `typed_bytes`; the exclusive heap borrow
        // prevents another slice through the API.
        Some(unsafe { t.slice_mut() })
    }

    /// A SIMD value (`cid` 19–21) with the lanes.
    pub fn new_simd(&mut self, cid: i32, hints_list: [u8; 16]) -> Ref {
        debug_assert!(layout::cid::is_simd(cid));
        let h = self.allocate(cid, 2, flags::RAW);
        self.bytes_mut(h)[..16].copy_from_slice(&hints_list);
        h
    }

    /// The lanes of the SIMD value `h`.
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
            assert_eq!(type_of_cid(cid::typed(t)), Some(t));
            assert_eq!(type_of_cid(cid::view(t, false)), Some(t));
            assert_eq!(type_of_cid(cid::view(t, true)), Some(t));
            assert_eq!(view_cid(t, true), cid::view(t, true));
        }
        assert_eq!(type_of_cid(cid::BYTE_DATA_VIEW), Some(ty::BYTE_DATA));
        assert_eq!(type_of_cid(cid::UNMODIFIABLE_BYTE_DATA_VIEW), Some(ty::BYTE_DATA));
        assert_eq!(view_cid(ty::BYTE_DATA, false), cid::BYTE_DATA_VIEW);
        assert_eq!(type_of_cid(cid::FLOAT32X4), None);
        assert_eq!(type_of_cid(66), None);
        assert_eq!(element_size(ty::FLOAT64X2), 16);
        assert_eq!(element_size(ty::BYTE_DATA), 1);
    }
}
