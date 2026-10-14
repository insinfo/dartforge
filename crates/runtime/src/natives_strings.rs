// Native runtime: `String` and `StringBuffer` natives of the SDK from source (P5b;
// unified space, P1).
//
// The natives of `runtime/lib/string.cc`, the string `Internal_*` ones
// (`internal_patch.dart`) and the overlay's `DartForge_string_*`/`DartForge_sb_*`
// (`native_sdk/core/string_patch.dart`, `string_buffer_patch.dart`) on top of the
// object space's strings (`text.rs`): reading through the block's borrowed view,
// creation in one allocation in canonical form. The receiver is the string's
// `Ref`; an `int` arrives as an `i64`. The table that lowering consults is
// `crates/emit_native/src/nativos.rs`; generated code does the hottest ones
// (`length`, `codeUnitAt`, `hashCode`, `allocate*`/`writeInto*`, `==`) inline
// (`lower/text.rs`) and only falls back here on the slow path.

/// `String_getLength`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_String_getLength(this: i64) -> i64 {
    with_string(this, |t| t.len() as i64)
}

/// `_StringBase.codeUnitAt(i)` (a VM intrinsic), with the index check: out of
/// range, `RangeError.range(i, 0, length - 1, "length")`. The slow path of
/// `@df.string_code_unit` (`lower/text.rs`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_string_codeUnitAt(this: i64, ix: i64) -> i64 {
    let r = with_string(this, |t| {
        usize::try_from(ix).ok().filter(|&i| i < t.len()).map(|i| i64::from(t.unit(i))).ok_or(t.len())
    });
    match r {
        Ok(u) => u,
        Err(n) => {
            throw_range_err(ix, 0, n as i64 - 1, "length");
            0
        }
    }
}

thread_local! {
    /// The one-character Latin-1 strings already returned by `String_charAt` in
    /// this isolate: the canonical literals (`Heap::string_literal`, permanent),
    /// 0 = not requested yet.
    static ONE_CHAR: std::cell::RefCell<[i64; 256]> = const { std::cell::RefCell::new([0; 256]) };
}

/// `String_charAt` (`s[i]`): the one-unit string; out of bounds,
/// `RangeError.range(i, 0, length - 1, "index")` (`StringValueAt`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_String_charAt(this: i64, ix: i64) -> i64 {
    let r = with_string(this, |t| usize::try_from(ix).ok().filter(|&i| i < t.len()).map(|i| t.unit(i)).ok_or(t.len()));
    let u = match r {
        Ok(u) => u,
        Err(n) => {
            throw_range_err(ix, 0, n as i64 - 1, "index");
            return 0;
        }
    };
    if u < 256 {
        // A Latin-1 unit comes back as the canonical one-character string, without
        // allocating: the VM returns the predefined symbol (`Symbols::FromCharCode`,
        // `String_charAt`), so `identical(s[0], 'a')` holds here too.
        let cache = ONE_CHAR.with(|c| c.borrow()[u as usize]);
        if cache != 0 {
            return cache;
        }
        let h = HEAP.with(|heap| heap.borrow_mut().string_literal(crate::text::TextRef::OneByte(&[u as u8])));
        ONE_CHAR.with(|c| c.borrow_mut()[u as usize] = h);
        return h;
    }
    HEAP.with(|heap| heap.borrow_mut().allocate_string(crate::text::TextRef::TwoByte(&[u])))
}

/// `String_concat` (`+`): one allocation, the units copied from both blocks.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_String_concat(this: i64, other_one: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().join_strings(&[this, other_one]))
}

/// `String_getHashCode`: the VM's `StringHasher`, computed once per string and
/// stored in the header (`Heap::string_hash`). The slow path of `@df.string_hash`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_String_getHashCode(this: i64) -> i64 {
    dartforge_string_hash(this)
}

/// `String_toUpperCase` and `String_toLowerCase` (`String::Transform`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_String_toUpperCase(this: i64) -> i64 {
    transform(this, true)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_String_toLowerCase(this: i64) -> i64 {
    transform(this, false)
}

/// `StringBase_substringUnchecked` and `OneByteString_substringUnchecked`:
/// `[start, end)` without checking (Dart already checked), in canonical form.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_StringBase_substringUnchecked(this: i64, start: i64, end: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().string_slice(this, start as usize, end as usize))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_OneByteString_substringUnchecked(this: i64, start: i64, end: i64) -> i64 {
    dartforge_native_StringBase_substringUnchecked(this, start, end)
}

/// `Internal_allocateOneByteString`: a `_OneByteString` of `n` zero units, to be
/// filled by `writeIntoOneByteString` (`dart:convert` and `_StringBase` build
/// strings this way). Generated code does it inline (`@df.string_allocate`); this is
/// the path of those who call the native.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Internal_allocateOneByteString(n: i64) -> i64 {
    dartforge_string_allocate(n, 0)
}

/// `Internal_allocateTwoByteString`: a `_TwoByteString` of `n` zero units. It stays
/// `TwoByte` even if it only receives Latin-1 units, as in the VM; equality is by
/// units, so this only costs space.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Internal_allocateTwoByteString(n: i64) -> i64 {
    dartforge_string_allocate(n, 1)
}

/// `Internal_writeIntoOneByteString`: writes the unit `code & 0xFF` at the index
/// (the string was just allocated; Dart checked the index and the range).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Internal_writeIntoOneByteString(string: i64, ix: i64, code: i64) {
    HEAP.with(|heap| heap.borrow_mut().one_units_mut(string)[ix as usize] = code as u8);
}

/// `Internal_writeIntoTwoByteString`: writes the unit `code & 0xFFFF`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Internal_writeIntoTwoByteString(string: i64, ix: i64, code: i64) {
    HEAP.with(|heap| heap.borrow_mut().two_units_mut(string)[ix as usize] = code as u16);
}

// The loops of `string_patch.dart` that the VM does with the `codeUnitAt`
// intrinsic: chunk comparison (`startsWith`, `endsWith`), search (`indexOf`,
// `lastIndexOf`, `contains`) and splitting by one character (`split`). The result
// is the same loop, done over the blocks' views.

/// `_StringBase._substringMatches(start, other)`: an empty `other` matches; out of
/// bounds, it does not.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_string_region_equals(this: i64, start: i64, other_one: i64) -> u8 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let t = heap.text(this).expect("compiler bug: string expected");
        let o = heap.text(other_one).expect("compiler bug: string expected");
        if o.is_empty() {
            return 1;
        }
        let Ok(i) = usize::try_from(start) else { return 0 };
        u8::from(t.matches_at(o, i))
    })
}

/// `indexOf(String other, start)` after the check of `start` (done in Dart): the
/// first index `>= start` where `other` matches, or -1.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_string_index_of(this: i64, other_one: i64, start: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let t = heap.text(this).expect("compiler bug: string expected");
        let o = heap.text(other_one).expect("compiler bug: string expected");
        if o.len() > t.len() {
            return -1;
        }
        let start = usize::try_from(start).unwrap_or(0);
        t.search_for(o, start).map_or(-1, |i| i as i64)
    })
}

/// `lastIndexOf(String other, start)` after the check of `start`: the last index
/// `<= start` where `other` matches, or -1.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_string_last_index_of(this: i64, other_one: i64, start: i64) -> i64 {
    if start < 0 {
        return -1;
    }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let t = heap.text(this).expect("compiler bug: string expected");
        let o = heap.text(other_one).expect("compiler bug: string expected");
        t.find_last(o, start as usize).map_or(-1, |i| i as i64)
    })
}

/// `_OneByteString._splitWithCharCode(charCode)`: appends to `list` (Dart's new
/// `<String>[]`) the pieces between the occurrences of unit `code`, in order — the
/// same loop, with each piece allocated here.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_string_split_by_code(this: i64, list: i64, code: i64) {
    // The boundaries first (without allocating); then each piece copied from the
    // block. `this` and `list` are rooted by the caller; each piece, as soon as it
    // is allocated, goes into the list (the collection of a following allocation
    // sees it).
    let ranges: Vec<(usize, usize)> = with_string(this, |t| {
        let mut v = Vec::new();
        let mut start = 0;
        for (i, u) in t.units().enumerate() {
            if i64::from(u) == code {
                v.push((start, i));
                start = i + 1;
            }
        }
        v.push((start, t.len()));
        v
    });
    for (a, b) in ranges {
        HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            let h = heap.string_slice(this, a, b);
            heap.list_push(list, crate::heap::Value::Ref(h));
        });
    }
}

/// The overlay's `_StringBase._equalStrings(other)` (`string_patch.dart`): the same UTF-16
/// units (the loop of the VM's `==`, done here).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_string_equals(this: i64, other_one: i64) -> u8 {
    HEAP.with(|heap| u8::from(heap.borrow().strings_equal(this, other_one)))
}

/// The whole `_StringBase.==` (the VM's `String_equality` intrinsic): `identical`,
/// the `other is String` test and the units. What is not an object (`null`, `Smi`)
/// differs without consulting the heap. The slow path of `@df.string_equals_object`
/// (`lower/text.rs`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_string_equals_object(this: i64, other_one: i64) -> u8 {
    if this == other_one {
        return 1;
    }
    if !crate::layout::is_object(other_one) {
        return 0;
    }
    HEAP.with(|heap| u8::from(heap.borrow().strings_equal(this, other_one)))
}

/// The overlay's `printToConsole(line)` (`DartForge_print`): the line and the
/// line terminator, like the VM's `print` (a lone surrogate becomes U+FFFD).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_print(line: i64) {
    use std::io::Write;
    let mut bytes = if line == 0 {
        b"null".to_vec()
    } else {
        with_string(line, |t| t.to_vm_utf8())
    };
    bytes.push(b'\n');
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(&bytes);
}

/// The (non-null) `Ref` strings of `list[start..end)`, in order.
fn concat_range_parts(list: i64, start: i64, end: i64) -> Vec<i64> {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let n = heap.list_len(list) as i64;
        (start.max(0)..end.min(n))
            .filter_map(|i| match heap.list_get(list, i as usize) {
                crate::heap::Value::Ref(r) if r != 0 => Some(r),
                _ => None,
            })
            .collect()
    })
}

/// `_StringBase._concatRangeNative(strings, start, end)`: the concatenation of
/// `strings[start..end]` (`String_concatRange`), in one allocation.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_String_concatRange(list: i64, start: i64, end: i64) -> i64 {
    // The strings are alive through the list (rooted by the caller).
    let part_list = concat_range_parts(list, start, end);
    HEAP.with(|heap| heap.borrow_mut().join_strings(&part_list))
}

/// The elements `[start, end)` of a list of integers (codes): a core list or a
/// typed list (the `Uint16List` of `String.fromCharCodes`), the way the VM natives
/// read `TypedData`. An element that is not an `int` becomes -1.
fn string_list_codes(list: i64, start: i64, end: i64) -> Vec<i64> {
    use crate::typed_lists::ty;
    HEAP.with(|heap| {
        let heap = heap.borrow();
        if let Some(t) = heap.typed(list) {
            // SAFETY: the list is alive (the caller has it) and nothing allocates here.
            let b = unsafe { t.piece() };
            let s = crate::typed_lists::element_size(t.ty);
            let (i, f) = (start.max(0) as usize, (end.max(0) as usize).min(t.len));
            return (i..f.max(i))
                .map(|k| {
                    let o = k * s;
                    match t.ty {
                        ty::INT8 => i64::from(b[o] as i8),
                        ty::INT16 => i64::from(i16::from_ne_bytes([b[o], b[o + 1]])),
                        ty::UINT16 => i64::from(u16::from_ne_bytes([b[o], b[o + 1]])),
                        ty::INT32 => i64::from(i32::from_ne_bytes(b[o..o + 4].try_into().unwrap())),
                        ty::UINT32 => i64::from(u32::from_ne_bytes(b[o..o + 4].try_into().unwrap())),
                        ty::INT64 | ty::UINT64 => i64::from_ne_bytes(b[o..o + 8].try_into().unwrap()),
                        _ => i64::from(b[o]),
                    }
                })
                .collect();
        }
        let n = heap.list_len(list) as i64;
        (start.max(0)..end.min(n)).map(|i| value_int(&heap, heap.list_get(list, i as usize)).unwrap_or(-1)).collect()
    })
}

/// `_OneByteString._allocateFromOneByteList(list, start, end)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_OneByteString_allocateFromOneByteList(list: i64, start: i64, end: i64) -> i64 {
    let u: Vec<u8> = string_list_codes(list, start, end).into_iter().map(|c| (c & 0xFF) as u8).collect();
    HEAP.with(|heap| heap.borrow_mut().allocate_string(crate::text::TextRef::OneByte(&u)))
}

/// `_TwoByteString._allocateFromTwoByteList(list, start, end)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_TwoByteString_allocateFromTwoByteList(list: i64, start: i64, end: i64) -> i64 {
    let u: Vec<u16> = string_list_codes(list, start, end).into_iter().map(|c| (c & 0xFFFF) as u16).collect();
    HEAP.with(|heap| heap.borrow_mut().allocate_string(crate::text::TextRef::TwoByte(&u)))
}

/// `_StringBase._createFromCodePoints(list, start, end)`: code points (those above
/// U+FFFF become the surrogate pair).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_StringBase_createFromCodePoints(list: i64, start: i64, end: i64) -> i64 {
    let mut u = Vec::new();
    for c in string_list_codes(list, start, end) {
        // Outside `0..=0x10FFFF`: `ArgumentError` without a message, like the VM's
        // `StringBase_createFromCodePoints`.
        if !(0..=0x10FFFF).contains(&c) {
            let e = dartforge_argument_error_new(0, 0);
            dartforge_exception_throw(e, 3);
            return 0;
        }
        crate::text::push_point(&mut u, c as u32);
    }
    HEAP.with(|heap| heap.borrow_mut().allocate_string(crate::text::TextRef::TwoByte(&u)))
}

/// `_StringBase._joinReplaceAllResult(base, matches, length, oneByte)`: the slices
/// of `base` (a negative `Smi` `-(start << 11 | size)`, or the pair start, end) and
/// the replacements, in order (`string_patch.dart`). One measurement, one
/// allocation and the copies of the blocks.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_StringBase_joinReplaceAllResult(base: i64, part_list: i64, _size: i64, _one_byte: u8) -> i64 {
    // (source, start, end) of each piece; the sources are alive through the list
    // and through `base` (rooted by the caller).
    let chunks: Vec<(i64, usize, usize)> = HEAP.with(|heap| {
        let heap = heap.borrow();
        let n = heap.list_len(part_list);
        let mut v = Vec::new();
        let mut i = 0;
        while i < n {
            let x = heap.list_get(part_list, i);
            match x {
                crate::heap::Value::Ref(r) if crate::layout::is_object(r) && heap.is_string(r) => {
                    let k = heap.text(r).map_or(0, crate::text::TextRef::len);
                    v.push((r, 0, k));
                }
                _ => {
                    let a = value_int(&heap, x).unwrap_or(0);
                    let (begin, end) = if a < 0 {
                        let bits = -a;
                        let begin = bits >> 11;
                        (begin, begin + (bits & ((1 << 11) - 1)))
                    } else {
                        i += 1;
                        (a, value_int(&heap, heap.list_get(part_list, i)).unwrap_or(a))
                    };
                    v.push((base, begin as usize, end as usize));
                }
            }
            i += 1;
        }
        v
    });
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let (mut total, mut one) = (0, true);
        for &(f, a, b) in &chunks {
            let t = heap.text(f).expect("compiler bug: string expected").piece(a, b);
            total += t.len();
            one &= t.fits_in_one_byte();
        }
        let r = heap.new_string(total, !one);
        let mut pos = 0;
        for &(f, a, b) in &chunks {
            heap.copy_string(r, pos, f, a, b);
            pos += b - a;
        }
        r
    })
}

// ─── The `StringBuffer` accumulator (`_AcumuladorDeTexto`, cid 15) ──────────
//
// A one-word RAW+ATTACHMENT block owns adaptive native storage. Latin-1
// writes stay one byte per UTF-16 unit; the first wider unit promotes once.
// The collector releases the attachment when its accumulator header dies.

/// Native storage preserving exact UTF-16 units, including lone surrogates.
enum TextAccumulator {
    OneByte(Vec<u8>),
    TwoByte(Vec<u16>),
}

impl TextAccumulator {
    fn from_units(units: Vec<u16>) -> Self {
        if units.iter().all(|&unit| unit <= 255) {
            Self::OneByte(units.into_iter().map(|unit| unit as u8).collect())
        } else {
            Self::TwoByte(units)
        }
    }

    fn text(&self) -> crate::text::TextRef<'_> {
        match self {
            Self::OneByte(bytes) => crate::text::TextRef::OneByte(bytes),
            Self::TwoByte(units) => crate::text::TextRef::TwoByte(units),
        }
    }

    fn accounted_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + match self {
            Self::OneByte(bytes) => bytes.capacity(),
            Self::TwoByte(units) => 2 * units.capacity(),
        }
    }

    fn append(&mut self, text: crate::text::TextRef<'_>) {
        if let Self::OneByte(bytes) = self {
            match text {
                crate::text::TextRef::OneByte(source) => {
                    bytes.extend_from_slice(source);
                    return;
                }
                crate::text::TextRef::TwoByte(source) if text.fits_in_one_byte() => {
                    bytes.extend(source.iter().map(|&unit| unit as u8));
                    return;
                }
                crate::text::TextRef::TwoByte(source) => {
                    let mut units = Vec::with_capacity(bytes.capacity().max(bytes.len() + source.len()));
                    units.extend(bytes.iter().map(|&byte| u16::from(byte)));
                    units.extend_from_slice(source);
                    *self = Self::TwoByte(units);
                    return;
                }
            }
        }
        if let Self::TwoByte(units) = self {
            match text {
                crate::text::TextRef::OneByte(bytes) => units.extend(bytes.iter().map(|&byte| u16::from(byte))),
                crate::text::TextRef::TwoByte(source) => units.extend_from_slice(source),
            }
        }
    }

    fn append_point(&mut self, code: u32) {
        match self {
            Self::OneByte(bytes) if code <= 255 => bytes.push(code as u8),
            Self::TwoByte(units) => crate::text::push_point(units, code),
            Self::OneByte(bytes) => {
                let extra = usize::from(code > 0xffff) + 1;
                let mut units = Vec::with_capacity(bytes.capacity().max(bytes.len() + extra));
                units.extend(bytes.iter().map(|&byte| u16::from(byte)));
                crate::text::push_point(&mut units, code);
                *self = Self::TwoByte(units);
            }
        }
    }
}

/// Releases native accumulator storage when its managed header dies.
///
/// # Safety
/// `p` is a Box<TextAccumulator> created by new_accumulator_with, released once.
unsafe fn release_accumulator(p: *mut u8) {
    // SAFETY: contract above.
    drop(unsafe { Box::from_raw(p.cast::<TextAccumulator>()) });
}

/// Empty storage or a copy received from another isolate.
fn new_accumulator_with(units: Vec<u16>) -> i64 {
    let storage = TextAccumulator::from_units(units);
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let h = heap.allocate(
            crate::layout::cid::STRING_ACCUMULATOR,
            1,
            crate::layout::flags::RAW | crate::layout::flags::ATTACHMENT,
        );
        let bytes = storage.accounted_bytes();
        let p = Box::into_raw(Box::new(storage)).cast::<u8>();
        // SAFETY: release_accumulator is the only release of p; attach never collects.
        unsafe { heap.attach(h, release_accumulator, p, bytes) };
        h
    })
}

/// Native storage of a live accumulator; only its owner isolate may mutate it.
///
/// # Panics
/// If h is not an accumulator (compiler bug).
fn accumulator_storage(heap: &Heap, h: i64) -> *mut TextAccumulator {
    assert!(
        crate::layout::is_object(h) && heap.class(h) == crate::layout::cid::STRING_ACCUMULATOR,
        "compiler bug: StringBuffer accumulator expected\nhandle {h}"
    );
    heap.attachment(h).cast::<TextAccumulator>()
}

/// Appends a borrowed native text view without a managed allocation or collection.
fn append_to_accumulator(accumulator: i64, text: crate::text::TextRef<'_>) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        // SAFETY: this owner's native attachment has no other mutable borrow;
        // growing it does not move the independent source text view.
        let storage = unsafe { &mut *accumulator_storage(&heap, accumulator) };
        let before = storage.accounted_bytes();
        storage.append(text);
        let after = storage.accounted_bytes();
        if before != after { heap.adjust_attachment(accumulator, after); }
    });
}

/// `StringBuffer._create()`: an empty accumulator.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_sb_new() -> i64 {
    new_accumulator_with(Vec::new())
}

/// Appends the units of string `text` to the accumulator.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_sb_write(accumulator: i64, text: i64) {
    // The return is -1 only when `text` is not a string, which Dart does not allow.
    let n = dartforge_native_DartForge_sb_write_if_string(accumulator, text);
    assert!(n >= 0, "compiler bug: StringBuffer._write without a String");
}

/// The overlay's `StringBuffer._writeIfString(accumulator, obj)`: if `obj` is a
/// string, appends its units to the accumulator and returns how many; otherwise
/// -1, changing nothing (Dart goes on through `toString`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_sb_write_if_string(accumulator: i64, obj: i64) -> i64 {
    if !crate::layout::is_object(obj) {
        return -1;
    }
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let pointer = accumulator_storage(&heap, accumulator);
        let (length, before, after) = {
            let Some(text) = heap.text(obj) else { return -1 };
            // SAFETY: the source is an independent managed string; native Vec
            // growth never moves it or collects. Its borrow ends before accounting.
            let storage = unsafe { &mut *pointer };
            let before = storage.accounted_bytes();
            storage.append(text);
            (text.len() as i64, before, storage.accounted_bytes())
        };
        if before != after { heap.adjust_attachment(accumulator, after); }
        length
    })
}

/// Appends the code point `code` to the accumulator (the overlay's `writeCharCode`,
/// which already checked `0 <= codigo <= 0x10FFFF`): one unit, or the surrogate
/// pair above 0xFFFF, like the VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_sb_write_char_code(accumulator: i64, code: i64) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        // SAFETY: native attachment mutation belongs to this owner and never collects.
        let storage = unsafe { &mut *accumulator_storage(&heap, accumulator) };
        let before = storage.accounted_bytes();
        storage.append_point(code as u32);
        let after = storage.accounted_bytes();
        if before != after { heap.adjust_attachment(accumulator, after); }
    });
}

/// The string with the accumulator's content (in canonical form); the accumulator
/// can keep growing.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_sb_to_string(accumulator: i64) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let v = accumulator_storage(&heap, accumulator);
        // SAFETY: the `Vec` lives outside the heap and is only released by the
        // accumulator's death, and the accumulator is rooted by the caller during
        // the allocation.
        let text = unsafe { (&*v).text() };
        heap.allocate_string(text)
    })
}

/// `_StringBase._fromCharCodes(list, start, end)` (DartForge, the `createFromCharCodes`
/// of `string_patch.dart`): the units `[start, end)` of a core list (`_List`,
/// `_GrowableList`, `_ImmutableList`), all `int`s in `0..=0xFF`, become the
/// `_OneByteString` in a single copy — the VM's `_scanCodeUnits` and per-unit
/// `_setAt`, which here were one runtime call each. Anything else (a unit above
/// 0xFF, negative or not an `int`, an invalid range, a typed list): null, and Dart
/// goes on through the VM's path, with its errors.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_string_from_char_codes(list: i64, start: i64, end: i64) -> i64 {
    HEAP.with(|heap| latin1_core_list(&mut heap.borrow_mut(), list, start, end).unwrap_or(0))
}

fn latin1_unit(value: crate::heap::Value) -> Option<u8> {
    let n = match value {
        crate::heap::Value::Int(n) => n,
        crate::heap::Value::Ref(r) if crate::layout::smi::is_smi(r) => crate::layout::smi::value(r),
        _ => return None,
    };
    u8::try_from(n).ok()
}

fn latin1_core_list(heap: &mut Heap, list: i64, start: i64, end: i64) -> Option<i64> {
    if !crate::layout::is_object(list) || !heap.is_list(list) {
        return None;
    }
    let (start, end) = (usize::try_from(start).ok()?, usize::try_from(end).ok()?);
    if start > end || end > heap.list_len(list) {
        return None;
    }
    // Refuse without allocation or mutation, exactly as the SDK fallback expects.
    let elements = heap.list_elements(list);
    for index in start..end {
        latin1_unit(elements.get(index))?;
    }
    Some(heap.list_with_roots(&[list], |heap| {
        let string = heap.new_string(end - start, false);
        // Allocation may collect, so reacquire the source only after it completes.
        let destination = heap.one_units_mut(string).as_mut_ptr();
        let elements = heap.list_elements(list);
        for index in start..end {
            let unit = latin1_unit(elements.get(index))
                .expect("validated Latin-1 list changed during allocation");
            // SAFETY: the fresh string has exactly end-start bytes. The rooted
            // source is a different live allocation; neither this read nor write
            // allocates, collects or calls Dart. The heap does not move objects.
            #[allow(unsafe_code)]
            unsafe {
                destination.add(index - start).write(unit);
            }
        }
        string
    }))
}

#[cfg(test)]
mod latin1_core_list_tests {
    use super::*;
    use crate::{
        heap::Value,
        layout::{cid, smi},
        lists::Element,
    };

    #[test]
    fn native_copies_latin1_slices_across_collection_and_releases_source_edges() {
        for arc in [false, true] {
            for class in [cid::LIST, cid::IMMUTABLE_LIST, cid::GROWABLE_LIST] {
                for shape in [Element::Int, Element::General] {
                    std::thread::spawn(move || {
                        HEAP.with(|heap| {
                            let mut heap = heap.borrow_mut();
                            *heap = Heap::new(true);
                            if arc {
                                heap.enable_arc();
                            }
                        });
                        let expected: Vec<_> = (0..1024).map(|index| (index & 255) as u8).collect();
                        let (list, storage, roots, before) = HEAP.with(|heap| {
                            let mut heap = heap.borrow_mut();
                            let roots = heap.push_owning_frame(2);
                            let list = if class == cid::GROWABLE_LIST {
                                heap.new_growable(expected.len(), expected.len() + 3, shape)
                            } else {
                                heap.new_list(class, expected.len(), shape)
                            };
                            heap.set_root(roots, 0, list);
                            for (index, &unit) in expected.iter().enumerate() {
                                let value = if shape == Element::Int {
                                    Value::Int(i64::from(unit))
                                } else {
                                    Value::Ref(smi::of(i64::from(unit)).unwrap())
                                };
                                heap.list_set(list, index, value);
                            }
                            (list, heap.list_data(list), roots, heap.stats())
                        });
                        let string =
                            dartforge_native_DartForge_string_from_char_codes(list, 13, 1000);
                        assert_ne!(string, 0);
                        HEAP.with(|heap| {
                            let mut heap = heap.borrow_mut();
                            assert_eq!(heap.stats().allocations, before.allocations + 1);
                            assert!(heap.stats().collections > before.collections);
                            heap.set_root(roots, 1, string);
                            assert_eq!(heap.class(string), cid::ONE_BYTE_STRING);
                            assert_eq!(
                                heap.text(string).unwrap().units().collect::<Vec<_>>(),
                                expected[13..1000]
                                    .iter()
                                    .map(|&unit| u16::from(unit))
                                    .collect::<Vec<_>>()
                            );
                            assert_eq!(heap.list_len(list), expected.len());
                            assert_eq!(heap.list_shape(list), shape);
                            heap.set_root(roots, 0, 0);
                            heap.collect();
                            assert!(!heap.is_live_object(list));
                            assert!(!heap.is_live_object(storage));
                            assert_eq!(heap.text(string).unwrap().len(), 987);
                            heap.pop_frame(roots);
                            heap.collect();
                            assert!(!heap.is_live_object(string));
                        });
                    })
                    .join()
                    .unwrap();
                }
            }
        }
    }

    #[test]
    fn native_refuses_wide_negative_null_mint_and_typed_inputs_without_allocating() {
        for arc in [false, true] {
            std::thread::spawn(move || {
                HEAP.with(|heap| {
                    let mut heap = heap.borrow_mut();
                    *heap = Heap::new(true);
                    if arc {
                        heap.enable_arc();
                    }
                });
                for value in [256, -1] {
                    let (list, roots) = HEAP.with(|heap| {
                        let mut heap = heap.borrow_mut();
                        let roots = heap.push_owning_frame(1);
                        let list = heap.new_list(cid::LIST, 1, Element::Int);
                        heap.set_root(roots, 0, list);
                        heap.list_set(list, 0, Value::Int(value));
                        (list, roots)
                    });
                    let before = HEAP.with(|heap| heap.borrow().stats().allocations);
                    for (start, end) in [(0, 1), (-1, 1), (1, 0), (0, 2)] {
                        assert_eq!(
                            dartforge_native_DartForge_string_from_char_codes(list, start, end),
                            0
                        );
                        assert_eq!(HEAP.with(|heap| heap.borrow().stats().allocations), before);
                    }
                    HEAP.with(|heap| heap.borrow_mut().pop_frame(roots));
                }
                let (list, mint, typed, roots) = HEAP.with(|heap| {
                    let mut heap = heap.borrow_mut();
                    let roots = heap.push_owning_frame(3);
                    let list = heap.new_list(cid::LIST, 2, Element::General);
                    heap.set_root(roots, 0, list);
                    let mint = heap.allocate(cid::MINT, 1, crate::layout::flags::RAW);
                    heap.words_mut(mint)[0] = 65;
                    heap.set_root(roots, 1, mint);
                    heap.list_set(list, 1, Value::Ref(mint));
                    let typed = heap.new_typed(crate::typed_lists::ty::UINT8, 1);
                    heap.set_root(roots, 2, typed);
                    (list, mint, typed, roots)
                });
                let before = HEAP.with(|heap| heap.borrow().stats().allocations);
                for (source, start, end) in [
                    (list, 0, 1),
                    (list, 1, 2),
                    (typed, 0, 1),
                    (mint, 0, 1),
                    (0, 0, 0),
                ] {
                    assert_eq!(
                        dartforge_native_DartForge_string_from_char_codes(source, start, end),
                        0
                    );
                    assert_eq!(HEAP.with(|heap| heap.borrow().stats().allocations), before);
                }
                HEAP.with(|heap| heap.borrow_mut().pop_frame(roots));
            })
            .join()
            .unwrap();
        }
    }

    #[test]
    fn native_empty_slice_keeps_existing_acceptance() {
        std::thread::spawn(|| {
            HEAP.with(|heap| *heap.borrow_mut() = Heap::new(true));
            for shape in [
                Element::General,
                Element::Int,
                Element::Double,
                Element::Bool,
            ] {
                let list = HEAP.with(|heap| heap.borrow_mut().new_list(cid::LIST, 0, shape));
                let string = dartforge_native_DartForge_string_from_char_codes(list, 0, 0);
                assert_ne!(string, 0);
                assert_eq!(
                    HEAP.with(|heap| heap.borrow().text(string).unwrap().len()),
                    0
                );
            }
        })
        .join()
        .unwrap();
    }
}

// ─── Unified space (docs/NATIVO-ESPACO-UNIFICADO.md §3.6) ────────────────────

/// A new string of `len` zeroed units (`two` ≠ 0: `_TwoByteString`): the slow
/// path of `@df.string_allocate` (above 16 words, or an exhausted TLAB). Does not
/// throw.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_allocate(len: i64, two: i64) -> i64 {
    let len = usize::try_from(len).unwrap_or(0);
    HEAP.with(|heap| heap.borrow_mut().new_string(len, two != 0))
}

/// [`dartforge_string_allocate`] under ARC: the new string (zeroed units, not yet
/// published) with a code owner, handed over before another collection.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let s = dartforge_arc_string_allocate_owned_v1(3, 0);
/// assert_eq!(dartforge_arc_observe_heap_v1(s), 3);
/// dartforge_arc_release(s);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_string_allocate_owned_v1(len: i64, two: i64) -> i64 {
    let len = usize::try_from(len).unwrap_or(0);
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let s = heap.new_string(len, two != 0);
        heap.retain_code_owner(s);
        s
    })
}

/// The hash of string `s`, computed and written into the header (`map`): the slow
/// path of `@df.string_hash`. Neither allocates nor throws.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_hash(s: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.string_hash(s) {
            Some(x) => i64::from(x),
            None => panic!("compiler bug: string hashCode on a value that is not a string\nhandle {s}"),
        }
    })
}

/// Do the two strings have the same units? The slow path of `@df.string_equals` (both
/// are strings; identity, length and hashes were already checked inline). Neither
/// allocates nor throws.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_equals(a: i64, b: i64) -> i8 {
    HEAP.with(|heap| i8::from(heap.borrow().strings_equal(a, b)))
}


#[cfg(test)]
mod adaptive_accumulator_tests {
    use super::*;

    #[test]
    fn latin1_storage_promotes_once_and_preserves_utf16_code_units() {
        let mut storage = TextAccumulator::from_units(Vec::new());
        storage.append(crate::text::TextRef::OneByte(b"json"));
        storage.append(crate::text::TextRef::TwoByte(&[0, 0x80, 0xff]));
        assert!(matches!(storage, TextAccumulator::OneByte(_)));
        assert_eq!(storage.text().units().collect::<Vec<_>>(), [106, 115, 111, 110, 0, 128, 255]);
        storage.append_point(0x100);
        assert!(matches!(storage, TextAccumulator::TwoByte(_)));
        storage.append_point(0xd800);
        storage.append_point(0x1f600);
        storage.append_point(0xdc00);
        storage.append_point(0xff);
        assert!(matches!(storage, TextAccumulator::TwoByte(_)));
        assert_eq!(storage.text().units().collect::<Vec<_>>(),
            [106, 115, 111, 110, 0, 128, 255, 0x100, 0xd800, 0xd83d, 0xde00, 0xdc00, 255]);
        let reconstructed = TextAccumulator::from_units(vec![0xff, 0, 0x80]);
        assert!(matches!(reconstructed, TextAccumulator::OneByte(_)));
    }

    #[test]
    #[allow(unsafe_code)]
    fn native_growth_and_promotion_update_capacity_accounting_without_collection() {
        std::thread::spawn(|| {
            HEAP.with(|heap| *heap.borrow_mut() = Heap::new(false));
            let accumulator = dartforge_native_DartForge_sb_new();
            let initial = HEAP.with(|heap| heap.borrow().stats().estimated_bytes);
            let initial_capacity = HEAP.with(|heap| unsafe {
                (&*accumulator_storage(&heap.borrow(), accumulator)).accounted_bytes()
            });
            let before_collections = HEAP.with(|heap| heap.borrow().stats().collections);
            append_to_accumulator(accumulator, crate::text::TextRef::OneByte(&[b'a'; 1024]));
            let one_bytes = HEAP.with(|heap| unsafe {
                (&*accumulator_storage(&heap.borrow(), accumulator)).accounted_bytes()
            });
            assert_eq!(HEAP.with(|heap| heap.borrow().stats().estimated_bytes), initial + one_bytes - initial_capacity);
            let before_append = HEAP.with(|heap| heap.borrow().stats().estimated_bytes);
            dartforge_native_DartForge_sb_write_char_code(accumulator, 0x100);
            let two_bytes = HEAP.with(|heap| unsafe {
                (&*accumulator_storage(&heap.borrow(), accumulator)).accounted_bytes()
            });
            assert_eq!(HEAP.with(|heap| heap.borrow().stats().estimated_bytes), before_append + two_bytes - one_bytes);
            assert!(two_bytes > one_bytes);
            assert_eq!(HEAP.with(|heap| heap.borrow().stats().collections), before_collections);
            // No managed reference protects this header: collection must release
            // the complete attachment, including capacities added after creation.
            HEAP.with(|heap| {
                let mut heap = heap.borrow_mut();
                heap.collect();
                assert_eq!(heap.stats().estimated_bytes, 0);
            });
        }).join().unwrap();
    }

    #[test]
    fn rooted_stress_snapshots_are_independent_of_further_writes_and_clear() {
        std::thread::spawn(|| {
            HEAP.with(|heap| *heap.borrow_mut() = Heap::new(true));
            let frame = HEAP.with(|heap| heap.borrow_mut().push_frame_with_slots(3));
            let accumulator = dartforge_native_DartForge_sb_new();
            HEAP.with(|heap| heap.borrow_mut().set_root(frame, 0, accumulator));
            let text = HEAP.with(|heap| heap.borrow_mut().allocate_string(crate::text::TextRef::OneByte(b"ascii")));
            HEAP.with(|heap| heap.borrow_mut().set_root(frame, 2, text));
            assert_eq!(dartforge_native_DartForge_sb_write_if_string(accumulator, text), 5);
            let snapshot = dartforge_native_DartForge_sb_to_string(accumulator);
            HEAP.with(|heap| heap.borrow_mut().set_root(frame, 1, snapshot));
            dartforge_native_DartForge_sb_write_char_code(accumulator, 0xd800);
            dartforge_native_DartForge_sb_write_char_code(accumulator, 0x1f600);
            assert_eq!(with_string(snapshot, |text| text.units().collect::<Vec<_>>()), [97, 115, 99, 105, 105]);
            assert_eq!(HEAP.with(|heap| accumulator_units(&heap.borrow(), accumulator)), [97, 115, 99, 105, 105, 0xd800, 0xd83d, 0xde00]);
            // clear() swaps in a fresh accumulator; the previous snapshot stays rooted.
            let empty = dartforge_native_DartForge_sb_new();
            HEAP.with(|heap| heap.borrow_mut().set_root(frame, 0, empty));
            HEAP.with(|heap| heap.borrow_mut().collect());
            assert_eq!(with_string(snapshot, |text| text.units().collect::<Vec<_>>()), [97, 115, 99, 105, 105]);
            assert_eq!(dartforge_native_DartForge_sb_write_if_string(empty, 1), -1);
            HEAP.with(|heap| {
                let mut heap = heap.borrow_mut();
                heap.pop_frame(frame);
                heap.collect();
                assert_eq!(heap.stats().estimated_bytes, 0);
            });
        }).join().unwrap();
    }

    #[test]
    fn capacity_growth_exhausts_deferral_without_collecting_borrowed_inputs() {
        std::thread::spawn(|| {
            HEAP.with(|heap| *heap.borrow_mut() = Heap::new(false));
            let frame = HEAP.with(|heap| heap.borrow_mut().push_frame_with_slots(2));
            let accumulator = dartforge_native_DartForge_sb_new();
            HEAP.with(|heap| heap.borrow_mut().set_root(frame, 0, accumulator));
            let source = HEAP.with(|heap| heap.borrow_mut().allocate_string(
                crate::text::TextRef::OneByte(&[b'x'; 1024])));
            HEAP.with(|heap| heap.borrow_mut().set_root(frame, 1, source));
            let before = HEAP.with(|heap| {
                let mut heap = heap.borrow_mut();
                assert_eq!(heap.gc_policy_defer(std::time::Duration::from_secs(10), 64), 1);
                heap.stats().collections
            });
            // The only growth here is the native attachment. This exceeds the
            // allowance but must not collect its borrowed managed string/header.
            assert_eq!(dartforge_native_DartForge_sb_write_if_string(accumulator, source), 1024);
            HEAP.with(|heap| {
                let mut heap = heap.borrow_mut();
                assert_eq!(heap.stats().collections, before);
                assert_eq!(heap.gc_policy_defer(std::time::Duration::from_secs(10), 64), 0);
                assert_eq!(accumulator_units(&heap, accumulator), vec![u16::from(b'x'); 1024]);
                assert_eq!(heap.text(source).unwrap().len(), 1024);
                // A normal managed allocation is the next safe collection point.
                heap.allocate_object(333, 1);
                assert_eq!(heap.stats().collections, before + 1);
                assert_eq!(accumulator_units(&heap, accumulator), vec![u16::from(b'x'); 1024]);
                assert_eq!(heap.text(source).unwrap().len(), 1024);
                assert_eq!(heap.gc_policy_defer(std::time::Duration::from_secs(10), 64), 1);
                heap.gc_policy_cancel();
                heap.pop_frame(frame);
                heap.collect();
                assert_eq!(heap.stats().estimated_bytes, 0);
            });
        }).join().unwrap();
    }

    #[test]
    fn isolate_copy_preserves_units_and_has_independent_native_storage() {
        std::thread::spawn(|| {
            HEAP.with(|heap| *heap.borrow_mut() = Heap::new(false));
            let source = new_accumulator_with(vec![0xff, 0xd800, 0xdc00, 0x100]);
            let portable = HEAP.with(|heap| accumulator_units(&heap.borrow(), source));
            let copy = allocate_attachment(&GNode::Accumulator(portable.clone())).unwrap();
            dartforge_native_DartForge_sb_write_char_code(source, 65);
            dartforge_native_DartForge_sb_write_char_code(copy, 66);
            assert_eq!(HEAP.with(|heap| accumulator_units(&heap.borrow(), source)), [255, 0xd800, 0xdc00, 256, 65]);
            assert_eq!(HEAP.with(|heap| accumulator_units(&heap.borrow(), copy)), [255, 0xd800, 0xdc00, 256, 66]);
            assert_eq!(portable, [255, 0xd800, 0xdc00, 256]);
            HEAP.with(|heap| heap.borrow_mut().collect());
        }).join().unwrap();
    }
}
