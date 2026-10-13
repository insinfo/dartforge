// Native runtime: the fast path of the default `_Map` and `_Set`
// (`native_sdk/collection/compact_hash.dart`) for `int` and `String` keys.
//
// The table is the SDK's, without any change of shape: the `_index`
// (`Uint32List`, linear probes with the hash pattern in the high bits) and the
// `_data` (`_List` with key and value side by side in the map, only the key in the
// set), with the insertion order and the deletion markers of `compact_hash.dart`.
// What changes is who walks it: in the VM, the compiler specializes `_hashCode` and
// `_equals` of `_OperatorEqualsAndHashCode` for `int` and `String` and inlines
// everything; here each step would be a call through the selector, with the
// covariance check of the list's `[]=`. These functions do the probe, the
// comparison and the write at once, with the same arithmetic as the Dart code
// (`_hashPattern`, `_firstProbe`, `_nextProbe`), so the table they leave is bit for
// bit the one the Dart code would leave — and the Dart code keeps working on it
// (removal, iteration, `_rehash`).
//
// Only a key whose `==` and `hashCode` the runtime knows without calling Dart goes
// through: `int` (`Smi` or `_Mint`: the `hashCode` of the VM's `HashIntegerOp`) and
// `String` (the VM's `StringHasher`, stored in the string's header; equality by
// code units). Since `int` and `String` have no subclasses, `key == entry` is known
// for every entry, with one exception: `int == double` compares numbers
// (`1 == 1.0`); that encounter returns "don't know". "Don't know" (another key, a
// table that needs to grow, an unexpected shape) changes nothing, and Dart redoes
// the operation through the SDK path.
//
// Unified space (docs/NATIVO-ESPACO-UNIFICADO.md §4.5, item 4): the `_data` is a
// general `_List` (`REFS`: each entry a `Ref` — `Smi`, `_Mint`, string…) and the
// `_index` an internal `_Uint32List` (cid 28). A compact data list, one that is
// unmodifiable on write or of another class returns "don't know".
//
// Without the `Heap` (docs/NATIVO-PLANO.md §13.2, item 1): the blocks are read
// through the offsets of the `layout` contract, the way generated code reads them
// (`@df.list_*`, `@df.string_*`), without `HEAP.with`, `borrow` or the
// `live_block` checks; each object's header tells the class and the shape before
// any read of the body. A write into `_data` applies generated code's element
// barrier (`@df.element_barrier`). Nothing here allocates, throws or calls Dart.

use crate::layout::{Header, HANDLE_OFFSET};

/// The header of object `h` (`is_object(h)`; also valid for the statics).
///
/// # Safety
/// `h` is the handle of a live object that generated code passed.
#[inline(always)]
unsafe fn raw_header<'a>(h: i64) -> &'a Header {
    // SAFETY: the function's contract (the block starts at `h - 2`).
    unsafe { &*((h - HANDLE_OFFSET) as *const Header) }
}

/// The address of offset `d` (from `layout::offsets`, relative to the block) of `h`.
#[inline(always)]
fn in_block(h: i64, d: usize) -> *mut u8 {
    (h - HANDLE_OFFSET + d as i64) as *mut u8
}

/// The 8-byte word at offset `d` of `h`'s block.
///
/// # Safety
/// `d` is within the block of a live object.
#[inline(always)]
unsafe fn raw_word(h: i64, d: usize) -> i64 {
    // SAFETY: the function's contract; bodies are aligned to 8.
    unsafe { *(in_block(h, d) as *const i64) }
}

/// The `_index` seen through its bytes: the `u32` positions.
#[derive(Clone, Copy)]
struct Index {
    p: *mut u32,
    size: usize,
}

impl Index {
    /// The `_index`, if `h` is an internal `_Uint32List` (cid 28, without `EXTERNAL`).
    #[inline(always)]
    fn of(h: i64) -> Option<Index> {
        use crate::layout::{cid, offsets, is_object, flags};
        if !is_object(h) {
            return None;
        }
        // SAFETY: live object (generated code passed the `_index` field).
        let c = unsafe { raw_header(h) };
        if c.class_id != cid::typed(crate::typed_lists::ty::UINT32) || c.flags & flags::EXTERNAL != 0 {
            return None;
        }
        // SAFETY: every typed list has the length at `b+16` and the data pointer
        // at `b+24` (§2.5 of the specification).
        let (size, p) = unsafe { (raw_word(h, offsets::LENGTH) as usize, raw_word(h, offsets::DATA) as *mut u32) };
        Some(Index { p, size })
    }

    #[inline(always)]
    fn do_read(self, i: usize) -> u32 {
        debug_assert!(i < self.size);
        // SAFETY: `i < size` (the probe masks by the size).
        unsafe { *self.p.add(i) }
    }

    #[inline(always)]
    fn write_value(self, i: usize, pair: u32) {
        debug_assert!(i < self.size);
        // SAFETY: as in `do_read`; bytes without references, no barrier.
        unsafe { *self.p.add(i) = pair };
    }
}

/// The elements of a general `_List`/`_ImmutableList` (`REFS`).
#[derive(Clone, Copy)]
struct Data {
    h: i64,
    p: *mut i64,
    len: usize,
}

impl Data {
    /// The entries of `h`, if it is a general `_List` (writing) or a general
    /// `_List`/`_ImmutableList` (reading).
    #[inline(always)]
    fn of(h: i64, write_value: bool) -> Option<Data> {
        use crate::layout::{cid, offsets, is_object, state, flags};
        if !is_object(h) {
            return None;
        }
        // SAFETY: live object.
        let c = unsafe { raw_header(h) };
        let serve = if write_value { c.class_id == cid::LIST && c.state != state::PERMANENT } else { cid::is_fixed_list(c.class_id) };
        if !serve || c.flags & (flags::FORMAT | flags::ELEMENT) != flags::REFS {
            return None;
        }
        // SAFETY: `REFS` `_List`: the length in word 0 of the body, the elements
        // from word 1 on.
        let len = unsafe { raw_word(h, offsets::LENGTH) } as usize;
        Some(Data { h, p: in_block(h, offsets::ELEMENTS).cast(), len })
    }

    /// The data of a general `_GrowableList` (the JSON listener's stack): the
    /// storage cut at the length.
    #[inline(always)]
    fn from_growable(h: i64) -> Option<Data> {
        use crate::layout::{cid, offsets, is_object};
        if !is_object(h) {
            return None;
        }
        // SAFETY: live object.
        if unsafe { raw_header(h) }.class_id != cid::GROWABLE_LIST {
            return None;
        }
        // SAFETY: `_GrowableList`: length in field 0, storage in field 1.
        let (len, a) = unsafe { (raw_word(h, offsets::LENGTH) as usize, raw_word(h, offsets::GROWABLE_DATA)) };
        let mut d = Data::of(a, false)?;
        if len > d.len {
            return None;
        }
        d.len = len;
        Some(d)
    }

    #[inline(always)]
    fn do_read(self, i: usize) -> i64 {
        debug_assert!(i < self.len);
        // SAFETY: `i < len`.
        unsafe { *self.p.add(i) }
    }

    /// Writes the `Ref` `v` into entry `i`, with generated code's element barrier:
    /// an old (or remembered) object that receives a young child gets the entry's
    /// card dirtied, if it has cards, and the old object is remembered.
    #[inline(always)]
    fn write_value(self, i: usize, v: i64) {
        use crate::layout::{state, flags};
        debug_assert!(i < self.len);
        // SAFETY: `i < len`; a writable `_List` (checked in `of`).
        let old = unsafe { std::mem::replace(&mut *self.p.add(i), v) };
        if arc_enabled() {
            arc_raw_write(self.h, old, v);
        }
        // SAFETY: live object.
        let c = unsafe { raw_header(self.h) };
        let e = c.state;
        if (e == state::OLD || e == state::REMEMBERED) && young_child(v) {
            if c.flags & flags::CARDS != 0 {
                // SAFETY: a `REFS` block with `CARDS` of `len` elements, `i < len`.
                unsafe { crate::space::dirty_card((self.h - HANDLE_OFFSET) as *mut Header, self.len, i) };
            }
            if e == state::OLD {
                dartforge_remember(self.h);
            }
        }
    }
}

/// Does the written `Ref` require the barrier (`@df.young_child`)? Only a young object.
#[inline(always)]
fn young_child(v: i64) -> bool {
    // SAFETY: live object.
    crate::layout::is_object(v) && unsafe { raw_header(v) }.state == crate::layout::state::YOUNG
}

/// A key that the runtime compares and hashes by itself.
#[derive(Clone, Copy)]
enum HashKey {
    Int(i64),
    /// The string's handle.
    Text(i64),
}

/// What the key's `==` says about an entry.
enum Comparison {
    Equal,
    Different,
    Unknown,
}

/// The result of the probe.
enum Probe {
    /// Entry `d` of `_data` (the key's).
    Found(usize),
    /// The `_index` position where the key would go and the hash pattern.
    Missing { point: usize, pattern: u32 },
}

/// The `int` of a `Smi` or `_Mint`.
#[inline(always)]
fn integer(k: i64) -> Option<i64> {
    use crate::layout::{cid, offsets, is_object, smi};
    if smi::is_smi(k) {
        return Some(smi::value(k));
    }
    // SAFETY: live object; the `_Mint` box has the value at `b+16`.
    if is_object(k) && unsafe { raw_header(k) }.class_id == cid::MINT {
        return Some(unsafe { raw_word(k, offsets::VALUE) });
    }
    None
}

/// The code units of a string: one-byte (`_OneByteString`) or two-byte.
#[derive(Clone, Copy)]
enum Units {
    OneByte(*const u8, usize),
    TwoByte(*const u16, usize),
}

impl Units {
    /// The units of `h`, if it is a string.
    #[inline(always)]
    fn of(h: i64) -> Option<Units> {
        use crate::layout::{cid, offsets, is_object};
        if !is_object(h) {
            return None;
        }
        // SAFETY: live object.
        let c = unsafe { raw_header(h) }.class_id;
        if !cid::is_string(c) {
            return None;
        }
        // SAFETY: string: the length at `b+16`, the units at `b+24`.
        let len = unsafe { raw_word(h, offsets::LENGTH) } as usize;
        let p = in_block(h, offsets::UNITS);
        Some(if c == cid::ONE_BYTE_STRING { Units::OneByte(p, len) } else { Units::TwoByte(p.cast(), len) })
    }

    fn len(self) -> usize {
        match self {
            Units::OneByte(_, n) | Units::TwoByte(_, n) => n,
        }
    }

    /// Unit `i` (`i < len`).
    #[inline(always)]
    fn at(self, i: usize) -> u16 {
        // SAFETY: `i < len` (the caller checks).
        unsafe {
            match self {
                Units::OneByte(p, _) => u16::from(*p.add(i)),
                Units::TwoByte(p, _) => *p.add(i),
            }
        }
    }

    /// The same units?
    fn equal(self, other: Units) -> bool {
        if self.len() != other.len() {
            return false;
        }
        // SAFETY: both slices have `len` units of their type.
        unsafe {
            match (self, other) {
                (Units::OneByte(a, n), Units::OneByte(b, _)) => std::slice::from_raw_parts(a, n) == std::slice::from_raw_parts(b, n),
                (Units::TwoByte(a, n), Units::TwoByte(b, _)) => std::slice::from_raw_parts(a, n) == std::slice::from_raw_parts(b, n),
                _ => (0..self.len()).all(|i| self.at(i) == other.at(i)),
            }
        }
    }
}

/// The `hashCode` of string `h` (that of `Heap::string_hash`): the header's, or
/// computed and written into it on the first query (except for a static string,
/// which already carries it and lives in read-only memory).
#[inline(always)]
fn raw_string_hash(h: i64, u: Units) -> u32 {
    use crate::layout::{offsets, state};
    let p = in_block(h, offsets::STRING_HASH).cast::<u32>();
    // SAFETY: `map` of a live string's header; the heap belongs to this thread.
    let current = unsafe { *p };
    if current != 0 {
        return current;
    }
    let x = match u {
        // SAFETY: the string's units.
        Units::OneByte(q, n) => crate::layout::string_hash(unsafe { std::slice::from_raw_parts(q, n) }.iter().map(|&b| u16::from(b))),
        Units::TwoByte(q, n) => crate::layout::string_hash(unsafe { std::slice::from_raw_parts(q, n) }.iter().copied()),
    };
    // SAFETY: as above; the state is byte 0 of the block.
    if unsafe { raw_header(h) }.state != state::PERMANENT {
        unsafe { *p = x };
    }
    x
}

/// The key and its `hashCode`, if the runtime knows them: `int` (`Smi` or `_Mint`,
/// the VM's `hashCode`, the same as `_Smi.hashCode` in generated code) or a string
/// (the header's hash).
#[inline(always)]
fn hash_key(k: i64) -> Option<(HashKey, i64)> {
    if let Some(v) = integer(k) {
        return Some((HashKey::Int(v), dartforge_native_DartForge_int_hashCode(v)));
    }
    let u = Units::of(k)?;
    Some((HashKey::Text(k), i64::from(raw_string_hash(k, u))))
}

/// `key == entry` (the `==` of `int` or of `String`), with the `Ref` entry.
#[inline(always)]
fn compare_key(key: HashKey, input: i64) -> Comparison {
    use crate::layout::{cid, is_object, smi};
    match key {
        HashKey::Int(v) => {
            if smi::is_smi(input) {
                return equal_if(smi::value(input) == v);
            }
            if !is_object(input) {
                return Comparison::Different;
            }
            // SAFETY: live object (an entry of `_data`).
            match unsafe { raw_header(input) }.class_id {
                cid::MINT => equal_if(integer(input) == Some(v)),
                // `1 == 1.0`: `num`'s `==`, which Dart decides.
                cid::DOUBLE => Comparison::Unknown,
                _ => Comparison::Different,
            }
        }
        HashKey::Text(h) => {
            if input == h {
                return Comparison::Equal;
            }
            let (Some(a), Some(b)) = (Units::of(h), Units::of(input)) else {
                return Comparison::Different;
            };
            // The already computed hashes differ: different units.
            // SAFETY: `map` of two live strings, only read.
            let (ha, hb) = unsafe {
                (
                    *in_block(h, crate::layout::offsets::STRING_HASH).cast::<u32>(),
                    *in_block(input, crate::layout::offsets::STRING_HASH).cast::<u32>(),
                )
            };
            if ha != 0 && hb != 0 && ha != hb {
                return Comparison::Different;
            }
            equal_if(a.equal(b))
        }
    }
}

#[inline(always)]
fn equal_if(b: bool) -> Comparison {
    if b { Comparison::Equal } else { Comparison::Different }
}

/// The probe of `_findValueOrInsertPoint` (map, `step` 2) and of `_add` (set,
/// `step` 1), with `_HashBase`'s arithmetic. `None`: equality is not known without
/// Dart, or the table has an unexpected shape.
#[inline(always)]
fn probe(ix: Index, data: Data, mask: i64, key: HashKey, hash: i64, step: usize) -> Option<Probe> {
    let size = ix.size;
    if size == 0 || !size.is_power_of_two() {
        return None;
    }
    let half = (size >> 1) as i64;
    let masked = hash & mask;
    // `_hashPattern`: fits in 32 bits by the choice of `_hashMask`.
    let pattern = if masked == 0 { half } else { masked.wrapping_mul(half) };
    let pattern = u32::try_from(pattern).ok()?;
    let size_mask = size - 1;
    let maximum = size >> 1;
    // `_firstProbe`.
    let i0 = (hash as usize) & size_mask;
    let mut i = ((i0 << 1) + i0) & size_mask;
    let mut first_removed: Option<usize> = None;
    loop {
        let pair = ix.do_read(i);
        if pair == 0 {
            break;
        }
        if pair == 1 {
            first_removed.get_or_insert(i);
        } else {
            let input = (pattern ^ pair) as usize;
            if input < maximum {
                let d = input * step;
                if d >= data.len {
                    return None;
                }
                match compare_key(key, data.do_read(d)) {
                    Comparison::Equal => return Some(Probe::Found(d)),
                    Comparison::Different => {}
                    Comparison::Unknown => return None,
                }
            }
        }
        i = (i + 1) & size_mask;
    }
    Some(Probe::Missing { point: first_removed.unwrap_or(i), pattern })
}

/// `map[key] = value` on the already resolved table: the new `_usedData` (the same,
/// if the key was already there), or -1 when Dart has to do the operation (a key of
/// another type, `int` against `double`, a full table: `_rehash` belongs to Dart).
#[inline(always)]
fn map_write(ix: Index, data: Data, mask: i64, used_count: i64, key: i64, value: i64) -> i64 {
    let Some((k, hash)) = hash_key(key) else { return -1 };
    match probe(ix, data, mask, k, hash, 2) {
        Some(Probe::Found(d)) => {
            if d + 1 >= data.len {
                return -1;
            }
            data.write_value(d + 1, value);
            used_count
        }
        Some(Probe::Missing { point, pattern }) => {
            if used_count < 0 || used_count as usize + 1 >= data.len {
                return -1;
            }
            let u = used_count as usize;
            ix.write_value(point, pattern | (u >> 1) as u32);
            data.write_value(u, key);
            data.write_value(u + 1, value);
            used_count + 2
        }
        None => -1,
    }
}

/// `_Set.add(key)` on the already resolved table: the new `_usedData` (the same, if
/// the key was already there), or -1.
#[inline(always)]
fn set_add(ix: Index, data: Data, mask: i64, used_count: i64, key: i64) -> i64 {
    let Some((k, hash)) = hash_key(key) else { return -1 };
    match probe(ix, data, mask, k, hash, 1) {
        // The set keeps the key that was already there.
        Some(Probe::Found(_)) => used_count,
        Some(Probe::Missing { point, pattern }) => {
            if used_count < 0 || used_count as usize >= data.len {
                return -1;
            }
            let u = used_count as usize;
            ix.write_value(point, pattern | u as u32);
            data.write_value(u, key);
            used_count + 1
        }
        None => -1,
    }
}

/// `_Map[key] = value` for an `int`/`String` key: returns the new `_usedData` (the
/// same, if the key was already there), or -1 when Dart has to do the operation (a
/// key of another type, `int` against `double`, a full table: `_rehash` belongs to
/// Dart).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_hash_map_store(ix: i64, data: i64, mask: i64, used_count: i64, key: i64, value: i64) -> i64 {
    match (Index::of(ix), Data::of(data, true)) {
        (Some(i), Some(d)) => map_write(i, d, mask, used_count, key, value),
        _ => -1,
    }
}

/// `_Map._getValueOrData(key)` for an `int`/`String` key: the value, or `data` (the
/// `_data`) when the key is missing — the SDK's protocol —, or `ix` (never a map
/// value) when Dart has to search.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_hash_map_lookup(ix: i64, data: i64, mask: i64, key: i64) -> i64 {
    let (Some(ind), Some(el), Some((k, hash))) = (Index::of(ix), Data::of(data, false), hash_key(key)) else {
        return ix;
    };
    match probe(ind, el, mask, k, hash, 2) {
        Some(Probe::Found(d)) if d + 1 < el.len => el.do_read(d + 1),
        Some(Probe::Missing { .. }) => data,
        _ => ix,
    }
}

/// `_Set.add(key)` for an `int`/`String` key: the new `_usedData` (the same, if the
/// key was already there), or -1 when Dart has to do the operation.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_hash_set_add(ix: i64, data: i64, mask: i64, used_count: i64, key: i64) -> i64 {
    match (Index::of(ix), Data::of(data, true)) {
        (Some(i), Some(d)) => set_add(i, d, mask, used_count, key),
        _ => -1,
    }
}

/// `_Set._getKeyOrData(key)` for an `int`/`String` key: the key that is in the set,
/// or `data` when it is missing, or `ix` when Dart has to search.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_hash_set_lookup(ix: i64, data: i64, mask: i64, key: i64) -> i64 {
    let (Some(ind), Some(el), Some((k, hash))) = (Index::of(ix), Data::of(data, false), hash_key(key)) else {
        return ix;
    };
    match probe(ind, el, mask, k, hash, 1) {
        Some(Probe::Found(d)) => el.do_read(d),
        Some(Probe::Missing { .. }) => data,
        None => ix,
    }
}

/// The result of a batch: the next position in `pair_list` (where the batch
/// stopped) in the high 32 bits and the new `_usedData` in the low ones.
#[inline(always)]
fn batch(cur_position: usize, used_count: i64) -> i64 {
    ((cur_position as i64) << 32) | (used_count & 0xFFFF_FFFF)
}

/// The entries of `pair_list`: a general `_List`/`_ImmutableList` (the old `_data`
/// of `_rehash`) or a general `_GrowableList` (the JSON listener's stack).
#[inline(always)]
fn entries(pair_list: i64) -> Option<Data> {
    Data::of(pair_list, false).or_else(|| Data::from_growable(pair_list))
}

/// The `_Map` batch (docs/NATIVO-PLANO.md §13.2, items 2 and 3): `map[k] = v` for
/// each pair `(pair_list[i], pair_list[i + 1])`, `i` from `of` to `until` in steps
/// of 2, in order, as `_set` would do. With `skip_removed`, a pair whose key is the
/// list itself (`_HashBase._isDeleted` of the old `_data`) is skipped. Stops at the
/// first pair the runtime cannot write (a key of another type, `int` against
/// `double`, a full table) and returns [`batch`]: Dart writes that pair and calls
/// again from the next one.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_hash_map_fill(ix: i64, data: i64, mask: i64, used_count: i64, pair_list: i64, of: i64, until: i64, skip_removed: u8) -> i64 {
    let (Some(ind), Some(d), Some(p)) = (Index::of(ix), Data::of(data, true), entries(pair_list)) else {
        return batch(of.max(0) as usize, used_count);
    };
    let (mut i, until) = (of.max(0) as usize, (until.max(0) as usize).min(p.len));
    let mut u = used_count;
    while i + 1 < until {
        let k = p.do_read(i);
        if !(skip_removed != 0 && k == pair_list) {
            let r = map_write(ind, d, mask, u, k, p.do_read(i + 1));
            if r < 0 {
                break;
            }
            u = r;
        }
        i += 2;
    }
    batch(i, u)
}

/// The `_Set` batch (the `_init` of `_rehash`): `add(olds_set[i])` for `i` from `of`
/// to `until`, skipping the removed ones (the key that is the list itself). Stops at
/// the first element the runtime cannot insert and returns [`batch`].
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_hash_set_fill(ix: i64, data: i64, mask: i64, used_count: i64, olds_set: i64, of: i64, until: i64) -> i64 {
    let (Some(ind), Some(d), Some(p)) = (Index::of(ix), Data::of(data, true), entries(olds_set)) else {
        return batch(of.max(0) as usize, used_count);
    };
    let (mut i, until) = (of.max(0) as usize, (until.max(0) as usize).min(p.len));
    let mut u = used_count;
    while i < until {
        let k = p.do_read(i);
        if k != olds_set {
            let r = set_add(ind, d, mask, u, k);
            if r < 0 {
                break;
            }
            u = r;
        }
        i += 1;
    }
    batch(i, u)
}

/// `_dfNewData(n)` (`compact_hash.dart`): the new `_data` of a table, a general
/// `_List` of `n` nulls without a reified type (the SDK's `new List.filled(n, null)`,
/// without evaluating `List<Object?>`: the list never leaves the table). May
/// collect.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_hash_new_data(n: i64) -> i64 {
    let n = usize::try_from(n).unwrap_or(0);
    HEAP.with(|heap| heap.borrow_mut().new_list(crate::layout::cid::LIST, n, crate::lists::Element::General))
}
