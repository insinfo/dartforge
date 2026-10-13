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

/// The overlay's `_StringBase._iguais(other)` (`string_patch.dart`): the same UTF-16
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
// A one-word `RAW`+`ATTACHMENT` block: the pointer of a `Box<Vec<u16>>` with the
// written units, which the collector releases (`release_accumulator`) when the
// block dies (§2.8). Only `StringBuffer` sees it
// (`native_sdk/core/string_buffer_patch.dart`).

/// Releases the `Vec<u16>` of a dead accumulator (called by the collector).
///
/// # Safety
/// `p` came from `Box::into_raw` of a `Box<Vec<u16>>` in
/// [`dartforge_native_DartForge_sb_new`] and is released only once.
unsafe fn release_accumulator(p: *mut u8) {
    // SAFETY: contract above.
    drop(unsafe { Box::from_raw(p.cast::<Vec<u16>>()) });
}

/// A new accumulator with the units `units` (the empty `StringBuffer`, or the
/// copy of an accumulator that arrives in a message from another isolate, §2.12).
fn new_accumulator_with(units: Vec<u16>) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let h = heap.allocate(
            crate::layout::cid::STRING_ACCUMULATOR,
            1,
            crate::layout::flags::RAW | crate::layout::flags::ATTACHMENT,
        );
        let bytes = std::mem::size_of::<Vec<u16>>() + 2 * units.capacity();
        let p = Box::into_raw(Box::new(units)).cast::<u8>();
        // SAFETY: `release_accumulator` is the only release of `p`.
        unsafe { heap.attach(h, release_accumulator, p, bytes) };
        h
    })
}

/// The `Vec<u16>` of accumulator `h`.
///
/// # Panics
/// If `h` is not an accumulator (compiler bug).
fn accumulator_vec(heap: &Heap, h: i64) -> *mut Vec<u16> {
    assert!(
        crate::layout::is_object(h) && heap.class(h) == crate::layout::cid::STRING_ACCUMULATOR,
        "compiler bug: StringBuffer accumulator expected\nhandle {h}"
    );
    heap.attachment(h).cast::<Vec<u16>>()
}

/// Appends the units of `t` (which does not come from a block that may die here)
/// to the accumulator `accumulator`.
fn append_to_accumulator(accumulator: i64, t: crate::text::TextRef<'_>) {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let v = accumulator_vec(&heap, accumulator);
        // SAFETY: the attachment's `Vec` lives outside the heap; nothing else borrows it.
        let v = unsafe { &mut *v };
        match t {
            crate::text::TextRef::OneByte(b) => v.extend(b.iter().map(|&x| u16::from(x))),
            crate::text::TextRef::TwoByte(u) => v.extend_from_slice(u),
        }
    });
}

/// `StringBuffer._novo()`: an empty accumulator.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_sb_new() -> i64 {
    new_accumulator_with(Vec::new())
}

/// Appends the units of string `text` to the accumulator.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_sb_write(accumulator: i64, text: i64) {
    // The return is -1 only when `text` is not a string, which Dart does not allow.
    let n = dartforge_native_DartForge_sb_write_if_string(accumulator, text);
    assert!(n >= 0, "compiler bug: StringBuffer._escrever without a String");
}

/// The overlay's `StringBuffer._escreverSeTexto(accumulator, obj)`: if `obj` is a
/// string, appends its units to the accumulator and returns how many; otherwise
/// -1, changing nothing (Dart goes on through `toString`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_sb_write_if_string(accumulator: i64, obj: i64) -> i64 {
    if !crate::layout::is_object(obj) {
        return -1;
    }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        // The units read directly from the string's block (alive: it is the
        // argument); the accumulator lives outside the heap, so growing does not
        // move them.
        let Some(t) = heap.text(obj) else { return -1 };
        let v = accumulator_vec(&heap, accumulator);
        // SAFETY: the attachment's `Vec` lives outside the heap; nothing else borrows it.
        let v = unsafe { &mut *v };
        match t {
            crate::text::TextRef::OneByte(b) => v.extend(b.iter().map(|&x| u16::from(x))),
            crate::text::TextRef::TwoByte(u) => v.extend_from_slice(u),
        }
        t.len() as i64
    })
}

/// Appends the code point `code` to the accumulator (the overlay's `writeCharCode`,
/// which already checked `0 <= codigo <= 0x10FFFF`): one unit, or the surrogate
/// pair above 0xFFFF, like the VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_sb_write_char_code(accumulator: i64, code: i64) {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        // SAFETY: the attachment's `Vec` lives outside the heap; nothing else borrows it.
        let v = unsafe { &mut *accumulator_vec(&heap, accumulator) };
        crate::text::push_point(v, code as u32);
    });
}

/// The string with the accumulator's content (in canonical form); the accumulator
/// can keep growing.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_sb_to_string(accumulator: i64) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let v = accumulator_vec(&heap, accumulator);
        // SAFETY: the `Vec` lives outside the heap and is only released by the
        // accumulator's death, and the accumulator is rooted by the caller during
        // the allocation.
        let u: &[u16] = unsafe { &*v };
        heap.allocate_string(crate::text::TextRef::TwoByte(u))
    })
}

/// `_StringBase._deCodigos(list, start, end)` (DartForge, the `createFromCharCodes`
/// of `string_patch.dart`): the units `[start, end)` of a core list (`_List`,
/// `_GrowableList`, `_ImmutableList`), all `int`s in `0..=0xFF`, become the
/// `_OneByteString` in a single copy — the VM's `_scanCodeUnits` and per-unit
/// `_setAt`, which here were one runtime call each. Anything else (a unit above
/// 0xFF, negative or not an `int`, an invalid range, a typed list): null, and Dart
/// goes on through the VM's path, with its errors.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_string_from_char_codes(list: i64, start: i64, end: i64) -> i64 {
    let bytes = HEAP.with(|heap| {
        let heap = heap.borrow();
        if !crate::layout::is_object(list) || !heap.is_list(list) {
            return None;
        }
        let (i, f) = (usize::try_from(start).ok()?, usize::try_from(end).ok()?);
        if i > f || f > heap.list_len(list) {
            return None;
        }
        let e = heap.list_elements(list);
        let mut v = Vec::with_capacity(f - i);
        for k in i..f {
            let n = match e.get(k) {
                crate::heap::Value::Int(x) => x,
                crate::heap::Value::Ref(r) if crate::layout::smi::is_smi(r) => crate::layout::smi::value(r),
                _ => return None,
            };
            v.push(u8::try_from(n).ok()?);
        }
        Some(v)
    });
    bytes.map_or(0, |b| HEAP.with(|heap| heap.borrow_mut().allocate_string(crate::text::TextRef::OneByte(&b))))
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
