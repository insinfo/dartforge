//! The core lists in the unified space (P3, docs/NATIVO-ESPACO-UNIFICADO.md §3.3).
//!
//! `_List` (cid 8) and `_ImmutableList` (9) are `REFS` (the length in word 0, the
//! `Ref` elements after it; cards in large ones) or `BRUTO` with the compact form in
//! `flags` (`ELEMENTO_INT`/`_DOUBLE`/`_BOOL`); `_GrowableList` (10) is an
//! `INSTANCIA` with two fields: the length (raw) and the storage (`_List`, `Ref`),
//! like the VM's `GrowableObjectArray` (`raw_object.h:3561-3572`).
//!
//! Rules of this view:
//!
//! * **The class says** fixed (`_List`), unmodifiable (`_ImmutableList`) or growable
//!   (`_GrowableList`); the cid never changes after publication (§2.16). Fixing or
//!   making unmodifiable means copying into a new object ([`Heap::fixed_list_of`],
//!   [`Heap::unmodifiable_list_of`]).
//! * **The shape belongs to the storage** (the `_List` of a `_GrowableList`): only
//!   representation, never semantics. A compact shape that cannot hold the written
//!   value decompacts (§2.16): it builds a rooted general storage with the boxes and
//!   only then swaps, with no allocation between the change of the data and that of
//!   `flags`.
//! * **Every `Ref` write** into a published storage goes through
//!   [`Heap::write_ref`]/[`Heap::write_refs`] (barrier and card); a freshly
//!   allocated storage, still young and without other references, is filled
//!   through [`Heap::words_mut`].
//! * **Collection in the middle:** the allocating methods (`nova_*`,
//!   `list_get_ref`, `list_set`, `list_push`, `list_reserve`, `list_adjust_shape`,
//!   `list_copy`, `lista_*_de`) root what they hold between two allocations; the
//!   handles the caller holds after the call are the caller's.
//! * **Compaction accepts null.** [`Heap::list_adjust_shape`] is called after the
//!   reified `E` (`int`, `double` or `bool`, non-nullable) is written; a null within
//!   the length only exists before the SDK fills the list (`List<int>.filled`,
//!   `_GrowableList<int>(n)`) and becomes 0/0.0/false. For the same reason, writing
//!   null into a compact list **with a written type** (metadata ≠ 0: the shrinking
//!   `length =` clears the cut positions with null) writes zero instead of
//!   decompacting; without a written type, it decompacts.

use crate::heap::{Heap, Value};
use crate::layout::{self, cid, flags, Ref};

/// The shape of a list's elements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    /// `Ref` (`REFS`).
    General,
    /// Unboxed `i64`.
    Int,
    /// `f64` bits.
    Double,
    /// 0/1.
    Bool,
}

impl Element {
    /// The `flags` bits (`FORMA` and `ELEMENTO`) of a storage in this shape.
    pub const fn flags(self) -> u8 {
        match self {
            Element::General => flags::REFS,
            Element::Int => flags::BRUTO | flags::ELEMENTO_INT,
            Element::Double => flags::BRUTO | flags::ELEMENTO_DOUBLE,
            Element::Bool => flags::BRUTO | flags::ELEMENTO_BOOL,
        }
    }

    /// The shape of a storage from its `flags` bits.
    pub const fn from_flags(f: u8) -> Element {
        if f & flags::FORMA != flags::BRUTO {
            return Element::General;
        }
        match f & flags::ELEMENTO {
            flags::ELEMENTO_INT => Element::Int,
            flags::ELEMENTO_DOUBLE => Element::Double,
            flags::ELEMENTO_BOOL => Element::Bool,
            _ => Element::General,
        }
    }

    /// The ABI code of `dartforge_lista_nova` (0 general, 1 int, 2 double, 3 bool:
    /// the order of the variants).
    pub const fn code(self) -> i64 {
        match self {
            Element::General => 0,
            Element::Int => 1,
            Element::Double => 2,
            Element::Bool => 3,
        }
    }

    /// The shape of an ABI code; `None` for any other value.
    pub const fn from_code(c: i64) -> Option<Element> {
        match c {
            0 => Some(Element::General),
            1 => Some(Element::Int),
            2 => Some(Element::Double),
            3 => Some(Element::Bool),
            _ => None,
        }
    }

    /// The bits of `v` in this compact shape, if it holds it unboxed.
    fn bits_of(self, v: Value) -> Option<i64> {
        match (self, v) {
            (Element::Int, Value::Int(x)) => Some(x),
            (Element::Double, Value::Double(d)) => Some(d.to_bits() as i64),
            (Element::Bool, Value::Bool(b)) => Some(i64::from(b)),
            _ => None,
        }
    }

    /// The value of the bits `w` of an element in this shape.
    fn value_of(self, w: i64) -> Value {
        match self {
            Element::General => Value::Ref(w),
            Element::Int => Value::Int(w),
            Element::Double => Value::Double(f64::from_bits(w as u64)),
            Element::Bool => Value::Bool(w != 0),
        }
    }
}

/// The elements of a list, cut at the length, in the storage's shape.
#[derive(Clone, Copy)]
pub enum RefElements<'a> {
    General(&'a [i64]),
    Int(&'a [i64]),
    Double(&'a [f64]),
    Bool(&'a [i64]),
}

impl RefElements<'_> {
    /// How many elements.
    pub fn len(&self) -> usize {
        match self {
            RefElements::General(p) | RefElements::Int(p) | RefElements::Bool(p) => p.len(),
            RefElements::Double(p) => p.len(),
        }
    }

    /// No elements?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Element `i`, unboxed.
    pub fn get(&self, i: usize) -> Value {
        match self {
            RefElements::General(p) => Value::Ref(p[i]),
            RefElements::Int(p) => Value::Int(p[i]),
            RefElements::Double(p) => Value::Double(p[i]),
            RefElements::Bool(p) => Value::Bool(p[i] != 0),
        }
    }
}

impl Heap {
    /// A `_List` or `_ImmutableList` (`cid`) of `len` zeroed elements (null, 0, 0.0
    /// or false, depending on the shape).
    ///
    /// The body always has the card words of a large list (`layout::list_words`),
    /// in the compact shape too: decompacting in place (§2.16) needs them.
    pub fn new_list(&mut self, cid: i32, len: usize, e: Element) -> Ref {
        assert!(cid::is_fixed_list(cid), "runtime bug: new_list with cid {cid}");
        let mut f = e.flags();
        if layout::has_cards(len) {
            f |= flags::CARTOES;
        }
        let h = self.allocate(cid, layout::list_words(len), f);
        self.words_mut(h)[0] = len as i64;
        h
    }

    /// A `_GrowableList` of `len` zeroed elements and a storage of `capacidade`
    /// (at least `len`).
    pub fn new_growable(&mut self, len: usize, capacidade: usize, e: Element) -> Ref {
        let data = self.new_list(cid::LIST, capacidade.max(len), e);
        let h = self.list_with_roots(&[data], |heap| heap.allocate_instance(cid::GROWABLE_LIST, 2));
        self.set_field(h, 0, len as i64, false);
        self.set_field(h, 1, data, true);
        h
    }

    /// Is `h` a core list (cid 8, 9 or 10)? No panic for null, a `Smi` or a dead
    /// object.
    pub fn e_lista(&self, h: Ref) -> bool {
        layout::is_object(h) && self.is_live_object(h) && cid::e_lista(self.header(h).class_id)
    }

    /// The length.
    pub fn list_len(&self, h: Ref) -> usize {
        match self.list_cid(h) {
            cid::GROWABLE_LIST => self.list_growable_field(h, 0) as usize,
            _ => self.words(h)[0] as usize,
        }
    }

    /// The capacity (the storage's length).
    pub fn list_capacity(&self, h: Ref) -> usize {
        let a = self.list_data(h);
        self.words(a)[0] as usize
    }

    /// The shape of the elements (of the storage, in a `_GrowableList`).
    pub fn list_shape(&self, h: Ref) -> Element {
        let a = self.list_data(h);
        Element::from_flags(self.header(a).flags)
    }

    /// The elements, cut at the length.
    #[allow(unsafe_code)]
    pub fn list_elements(&self, h: Ref) -> RefElements<'_> {
        let n = self.list_len(h);
        let a = self.list_data(h);
        let shape = Element::from_flags(self.header(a).flags);
        let p = &self.words(a)[1..1 + n];
        match shape {
            Element::General => RefElements::General(p),
            Element::Int => RefElements::Int(p),
            Element::Bool => RefElements::Bool(p),
            // SAFETY: `i64` and `f64` have the same size and alignment, and every
            // bit pattern is an `f64`; the slice is the same memory, only read.
            Element::Double => RefElements::Double(unsafe { std::slice::from_raw_parts(p.as_ptr().cast::<f64>(), n) }),
        }
    }

    /// Element `i`, unboxed. Panics if `i` is outside the length.
    pub fn list_get(&self, h: Ref, i: usize) -> Value {
        let n = self.list_len(h);
        assert!(i < n, "runtime bug: element {i} of a list of {n}");
        let a = self.list_data(h);
        let shape = Element::from_flags(self.header(a).flags);
        shape.value_of(self.words(a)[1 + i])
    }

    /// Element `i` in a `Ref` position (boxes the compact one; may collect).
    pub fn list_get_ref(&mut self, h: Ref, i: usize) -> Ref {
        match self.list_get(h, i) {
            Value::Ref(r) => r,
            v => self.to_ref(v),
        }
    }

    /// Writes element `i` (decompacts if the shape does not hold `v`; with a
    /// barrier). Panics if `i` is outside the length. May collect (box,
    /// decompaction).
    pub fn list_set(&mut self, h: Ref, i: usize, v: Value) {
        let n = self.list_len(h);
        assert!(i < n, "runtime bug: write into element {i} of a list of {n}");
        self.list_write_to_storage(h, i, v);
    }

    /// Appends to the end of a `_GrowableList` (grows if needed: the next capacity
    /// is `(capacidade * 2) | 3`, the VM's `_nextCapacity`,
    /// `growable_array.dart:385`). May collect.
    pub fn list_push(&mut self, h: Ref, v: Value) {
        assert_eq!(self.list_cid(h), cid::GROWABLE_LIST, "runtime bug: list_push on a fixed list");
        let len = self.list_len(h);
        let cap = self.list_capacity(h);
        if len == cap {
            let r = match v {
                Value::Ref(r) => r,
                _ => 0,
            };
            self.list_with_roots(&[h, r], |heap| heap.list_reserve(h, (cap * 2) | 3));
        }
        self.set_field(h, 0, (len + 1) as i64, false);
        self.list_write_to_storage(h, len, v);
    }

    /// Changes the length of a `_GrowableList` (without a barrier; the storage must
    /// hold `len`).
    pub fn list_set_len(&mut self, h: Ref, len: usize) {
        assert_eq!(self.list_cid(h), cid::GROWABLE_LIST, "runtime bug: length of a fixed list");
        debug_assert!(len <= self.list_capacity(h), "length {len} beyond the capacity");
        self.set_field(h, 0, len as i64, false);
    }

    /// The storage of a `_GrowableList`; the list itself, if it is fixed or
    /// unmodifiable (generated code's `@df.lista_armazenamento`).
    pub fn list_data(&self, h: Ref) -> Ref {
        match self.list_cid(h) {
            cid::GROWABLE_LIST => self.list_growable_field(h, 1),
            _ => h,
        }
    }

    /// Swaps the storage of a `_GrowableList` (with a barrier). `data` is a `_List`.
    pub fn list_set_data(&mut self, h: Ref, data: Ref) {
        assert_eq!(self.list_cid(h), cid::GROWABLE_LIST, "runtime bug: storage of a fixed list");
        debug_assert!(cid::is_fixed_list(self.class(data)), "the storage is a _List");
        self.set_field(h, 1, data, true);
    }

    /// Ensures storage for `capacidade` elements in a `_GrowableList`: a new storage,
    /// of the same shape, with the elements up to the length. Nothing if the capacity
    /// is already enough. May collect.
    pub fn list_reserve(&mut self, h: Ref, capacidade: usize) {
        assert_eq!(self.list_cid(h), cid::GROWABLE_LIST, "runtime bug: reserve on a fixed list");
        if capacidade <= self.list_capacity(h) {
            return;
        }
        let shape = self.list_shape(h);
        let new = self.list_with_roots(&[h], |heap| heap.new_list(cid::LIST, capacidade, shape));
        let n = self.list_len(h);
        let old_gen = self.list_data(h);
        // The new one is young and nobody else sees it: raw copy, no barrier.
        let copy: Vec<i64> = self.words(old_gen)[1..1 + n].to_vec();
        self.words_mut(new)[1..1 + n].copy_from_slice(&copy);
        self.list_set_data(h, new);
    }

    /// Changes the storage's shape, converting the elements in place (§2.15,
    /// §2.16). For the compact shape, only if every element up to the length fits in
    /// it (null counts as 0/0.0/false: see the module header); for the general one,
    /// it decompacts (allocates the boxes). A `_GrowableList` with an empty storage
    /// gets an empty storage of its own in the shape (the empty one is shared, the
    /// SDK's `_emptyList`). May collect.
    pub fn list_adjust_shape(&mut self, h: Ref, e: Element) {
        let current = self.list_shape(h);
        if current == e {
            return;
        }
        let a = self.list_data(h);
        let cap = self.words(a)[0] as usize;
        if self.list_cid(h) == cid::GROWABLE_LIST && cap == 0 {
            let new = self.list_with_roots(&[h], |heap| heap.new_list(cid::LIST, 0, e));
            self.list_set_data(h, new);
            return;
        }
        if current != Element::General {
            self.list_decompact(h);
            if e == Element::General {
                return;
            }
        }
        self.list_compact(h, e);
    }

    /// Copies `n` elements from `de[start..]` to `to[dest..]` (with a barrier; `to`
    /// decompacts if an element does not fit its shape). The ranges are within the
    /// lengths; `de` and `to` may be the same list (the copy is `memmove`'s). May
    /// collect.
    pub fn list_copy(&mut self, de: Ref, start: usize, to: Ref, dest: usize, n: usize) {
        if n == 0 {
            return;
        }
        let (nd, np) = (self.list_len(de), self.list_len(to));
        assert!(start + n <= nd && dest + n <= np, "runtime bug: copy outside the lists");
        let (fd, fp) = (self.list_shape(de), self.list_shape(to));
        let (ad, ap) = (self.list_data(de), self.list_data(to));
        if fd == fp {
            let source: Vec<i64> = self.words(ad)[1 + start..1 + start + n].to_vec();
            if fp == Element::General {
                self.write_refs(ap, 1 + dest, &source);
            } else {
                self.words_mut(ap)[1 + dest..1 + dest + n].copy_from_slice(&source);
            }
            return;
        }
        // Different shapes: element by element, with whatever boxes and
        // decompaction of the destination are needed. The values come out first
        // (the source may be the destination itself, which decompaction swaps).
        let value_list: Vec<Value> = (0..n).map(|k| self.list_get(de, start + k)).collect();
        self.list_with_roots(&[de, to], |heap| {
            for (k, v) in value_list.into_iter().enumerate() {
                heap.list_write_to_storage(to, dest + k, v);
            }
        });
    }

    /// A new `_List` with the elements of `h` (in the same shape). May collect.
    pub fn fixed_list_of(&mut self, h: Ref) -> Ref {
        self.list_copy_of(h, cid::LIST)
    }

    /// A new `_ImmutableList` with the elements of `h` (in the same shape). May
    /// collect.
    pub fn unmodifiable_list_of(&mut self, h: Ref) -> Ref {
        self.list_copy_of(h, cid::IMMUTABLE_LIST)
    }

    /// A general `_List`/`_ImmutableList` (`cid`) with the `Ref`s of `value_list`
    /// (the lists the runtime builds: arguments, results, literals). The `Ref`s must
    /// stay alive until the allocation (the caller roots them, if needed).
    pub fn ref_list(&mut self, cid: i32, value_list: &[Ref]) -> Ref {
        let h = self.new_list(cid, value_list.len(), Element::General);
        // Freshly allocated, young: no barrier.
        self.words_mut(h)[1..1 + value_list.len()].copy_from_slice(value_list);
        h
    }

    /// Writes `v` into every element of `h` (`List.filled`): a normal write into
    /// the first (the box, if the general shape requires it, or the decompaction, if
    /// the compact one does not hold `v`), and the same words into the others — the
    /// same object in every position, as in the VM. May collect.
    pub fn list_fill(&mut self, h: Ref, v: Value) {
        let n = self.list_len(h);
        if n == 0 {
            return;
        }
        self.list_set(h, 0, v);
        let a = self.list_data(h);
        let w = self.words(a)[1];
        if self.list_shape(h) == Element::General {
            self.write_refs(a, 1, &vec![w; n]);
        } else {
            self.words_mut(a)[1..1 + n].fill(w);
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// The cid of a core list; panics (N4) if `h` is not one.
    fn list_cid(&self, h: Ref) -> i32 {
        let c = self.class(h);
        assert!(cid::e_lista(c), "runtime bug: core list expected (cid {c}, handle {h})");
        c
    }

    /// Field `i` (0 length, 1 storage) of a `_GrowableList`.
    fn list_growable_field(&self, h: Ref, i: usize) -> i64 {
        self.object(h).expect("_GrowableList is an INSTANCIA").field(i).0
    }

    /// `f` with owning copies of `hs` in a temporary runtime frame. Null and `Smi`
    /// do not count RC. The result must be published with no collection between the
    /// closing of this frame and the caller's write of the reference.
    pub(crate) fn list_with_roots<R>(&mut self, hs: &[Ref], f: impl FnOnce(&mut Heap) -> R) -> R {
        let frame = self.push_owning_frame(hs.len());
        for (i, &x) in hs.iter().enumerate() {
            self.set_root(frame, i, x);
        }
        let r = f(self);
        self.pop_frame(frame);
        r
    }

    /// Writes `v` into element `i` of `h`'s storage (the length was already checked
    /// by the caller).
    fn list_write_to_storage(&mut self, h: Ref, i: usize, v: Value) {
        let a = self.list_data(h);
        let shape = Element::from_flags(self.header(a).flags);
        if shape == Element::General {
            let r = match v {
                Value::Ref(r) => r,
                // The box may collect: `h` stays rooted (the storage is read again
                // afterwards).
                other_one => self.list_with_roots(&[h], |heap| heap.to_ref(other_one)),
            };
            let a = self.list_data(h);
            self.write_ref(a, 1 + i, r);
            return;
        }
        // Compact shape: the unboxed value, if it fits.
        let v = match v {
            Value::Ref(r) => self.value(r),
            other_one => other_one,
        };
        if let Some(bits) = shape.bits_of(v) {
            self.words_mut(a)[1 + i] = bits;
            return;
        }
        if v == Value::Ref(0) && self.metadata(h) != 0 {
            // Clearing a cut position of a list of non-nullable scalar `E` (the
            // shrinking `length =`): zero, without decompacting.
            self.words_mut(a)[1 + i] = 0;
            return;
        }
        let r = match v {
            Value::Ref(r) => r,
            other_one => self.list_with_roots(&[h], |heap| heap.to_ref(other_one)),
        };
        self.list_with_roots(&[h, r], |heap| heap.list_decompact(h));
        let a = self.list_data(h);
        self.write_ref(a, 1 + i, r);
    }

    /// Moves the storage of `h` to the general shape, with the boxes of the
    /// elements up to the length (§2.16). In a `_GrowableList`, a new general
    /// storage (the old one may be shared); in a `_List`/`_ImmutableList`, a
    /// temporary general list with the boxes, copied into `h`'s body after swapping
    /// `flags` — with no allocation between the two things. May collect.
    fn list_decompact(&mut self, h: Ref) {
        let shape = self.list_shape(h);
        if shape == Element::General {
            return;
        }
        let n = self.list_len(h);
        let growable = self.list_cid(h) == cid::GROWABLE_LIST;
        let size = if growable { self.list_capacity(h) } else { n };
        let general = self.list_with_roots(&[h], |heap| heap.new_list(cid::LIST, size, Element::General));
        self.list_with_roots(&[h, general], |heap| {
            for i in 0..n {
                let bits = heap.words(heap.list_data(h))[1 + i];
                let r = heap.to_ref(shape.value_of(bits));
                // `general` may have aged in a collection caused by the next box.
                heap.write_ref(general, 1 + i, r);
            }
        });
        if growable {
            self.list_set_data(h, general);
            return;
        }
        // From here to the end, no allocation: the body of `h` changes format.
        let refs: Vec<i64> = self.words(general)[1..1 + n].to_vec();
        self.set_flags(h, flags::FORMA | flags::ELEMENTO, Element::General.flags());
        self.write_refs(h, 1, &refs);
    }

    /// Moves the general storage of `h` to the compact shape `e`, in place, if every
    /// element up to the length fits in it (null counts as zero). Does not allocate.
    fn list_compact(&mut self, h: Ref, e: Element) {
        debug_assert!(e != Element::General && self.list_shape(h) == Element::General);
        let n = self.list_len(h);
        let a = self.list_data(h);
        let cap = self.words(a)[0] as usize;
        let mut bits = Vec::with_capacity(cap);
        for &r in &self.words(a)[1..1 + cap] {
            if r == 0 {
                bits.push(0);
                continue;
            }
            match e.bits_of(self.value(r)) {
                Some(b) => bits.push(b),
                // An element within the length that does not fit: it stays general.
                // Beyond it (the rest of the capacity), the value is not observable
                // and becomes zero.
                None if bits.len() < n => return,
                None => bits.push(0),
            }
        }
        self.set_flags(a, flags::FORMA | flags::ELEMENTO, e.flags());
        self.words_mut(a)[1..1 + cap].copy_from_slice(&bits);
    }

    /// The copy of `h` into a new fixed list (`cid`), in the same shape. May collect.
    fn list_copy_of(&mut self, h: Ref, cid: i32) -> Ref {
        let n = self.list_len(h);
        let shape = self.list_shape(h);
        let new = self.list_with_roots(&[h], |heap| heap.new_list(cid, n, shape));
        let a = self.list_data(h);
        let source: Vec<i64> = self.words(a)[1..1 + n].to_vec();
        // Freshly allocated, young: no barrier.
        self.words_mut(new)[1..1 + n].copy_from_slice(&source);
        new
    }
}

#[cfg(test)]
mod tests_lists {
    use super::*;

    #[test]
    fn lists_flags_and_codes_round_trip() {
        for e in [Element::General, Element::Int, Element::Double, Element::Bool] {
            assert_eq!(Element::from_flags(e.flags()), e);
            assert_eq!(Element::from_flags(e.flags() | flags::CARTOES), e);
            assert_eq!(Element::from_code(e.code()), Some(e));
        }
        assert_eq!(Element::from_code(4), None);
        // `@df.list_shape` compares `flags & 0x66` with these values.
        assert_eq!(Element::Int.flags(), 0x22);
        assert_eq!(Element::Double.flags(), 0x42);
        assert_eq!(Element::Bool.flags(), 0x62);
        assert_eq!(Element::General.flags(), 0x04);
    }

    #[test]
    fn lists_bits_of_each_shape() {
        assert_eq!(Element::Int.bits_of(Value::Int(-3)), Some(-3));
        assert_eq!(Element::Double.bits_of(Value::Double(1.5)), Some(1.5f64.to_bits() as i64));
        assert_eq!(Element::Bool.bits_of(Value::Bool(true)), Some(1));
        assert_eq!(Element::Int.bits_of(Value::Double(1.0)), None);
        assert_eq!(Element::Int.bits_of(Value::Ref(0)), None);
        assert_eq!(Element::Double.value_of(2.5f64.to_bits() as i64), Value::Double(2.5));
        assert_eq!(Element::Bool.value_of(0), Value::Bool(false));
    }
}
