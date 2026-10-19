// Native runtime: string helpers of the fragments, the JIT literal, interpolation
// and the `String` externs of the "by name" lowering.
//
// Strings are `_OneByteString`/`_TwoByteString` blocks of the object space
// (`text.rs`, docs/NATIVO-ESPACO-UNIFICADO.md §2.5): read through the view
// (`Heap::text`, a `TextRef` borrowed from the block, without copying) and
// created in one allocation in canonical form. Every index, length and search
// here is in code units, as in `dart:core`; nothing goes through a Rust `String`
// (which would lose lone surrogates).
//
// The externs of the "by name" mechanism (`dartforge_string_*` except the
// interpolation and literal ones) belong to the path without the SDK from source
// and go away with it (P5, §3.7); until then they keep working on the new
// representation.

/// Allocates a string with the units of `t` (canonical form). The caller roots
/// the result.
fn allocate_string(t: Text) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().allocate_string(t.view()))
}

/// Allocates a string from text the runtime formatted (valid UTF-8).
fn allocate_str(s: &str) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().allocate_str(s))
}

/// Borrows the view of string `handle`, without copying. `f` must not allocate on
/// the heap (the borrow is open); whoever allocates the result does so afterwards.
/// A value that is not a string is a compiler bug (N4).
fn with_string<R>(handle: i64, f: impl FnOnce(crate::text::TextRef<'_>) -> R) -> R {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.text(handle) {
            Some(t) => f(t),
            None => panic!("compiler bug: string expected\nhandle {handle}"),
        }
    })
}

/// The construction copy of a string's units (for the algorithms that allocate in
/// the middle). Not a string: `None`.
fn string_copy(handle: i64) -> Option<Text> {
    HEAP.with(|heap| heap.borrow().text(handle).map(crate::text::TextRef::to_string_fn))
}

/// The copy of a string; a value that is not a string is a compiler bug (N4).
fn copied_string(handle: i64) -> Text {
    with_string(handle, |t| t.to_string_fn())
}

/// Throws `RangeError.range(value, min, max, name)`, like `dart:core`'s
/// `RangeError.checkValidRange`/`checkValueInInterval`.
fn throw_range_err(value: i64, min: i64, max: i64, name: &str) {
    let n = allocate_str(name);
    let err = with_roots(&[n], || dartforge_range_error_range(value, min, max, n, 0));
    dartforge_exception_throw(err, 3);
}

/// `dart:core`'s `RangeError.checkValidRange(start, end, length)`
/// (`errors.dart`): returns the effective end, or throws and returns `None`.
/// `end < 0` is the by-name lowering's "absent".
fn valid_range(start: i64, end: i64, len: usize) -> Option<(usize, usize)> {
    let len = len as i64;
    if start < 0 || start > len {
        throw_range_err(start, 0, len, "start");
        return None;
    }
    let end = if end < 0 { len } else { end };
    if start > end || end > len {
        throw_range_err(end, start, len, "end");
        return None;
    }
    Some((start as usize, end as usize))
}

/// The `int` of a list element (`Value::Int` of the compact form, `Smi` or
/// `_Mint`); `None` if it is not an `int`.
fn value_int(heap: &Heap, v: crate::heap::Value) -> Option<i64> {
    match v {
        crate::heap::Value::Int(x) => Some(x),
        crate::heap::Value::Ref(r) if crate::layout::smi::is_smi(r) => Some(crate::layout::smi::value(r)),
        crate::heap::Value::Ref(r) if crate::layout::is_object(r) => heap.int_of(r),
        _ => None,
    }
}

/// Copies WTF-8 (valid UTF-8 is WTF-8) from an LLVM constant to the canonical
/// literal (`Heap::string_literal`): the JIT literal, whose modules have no static
/// objects (a generation's memory is freed, §2.11).
/// SAFETY: ptr must point to len readable bytes; the emitter guarantees that region.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_string_new(ptr: *const u8, len: i64) -> i64 {
    let len = usize::try_from(len).expect("invalid length");
    let bytes = if len == 0 {
        &[]
    } else {
        // SAFETY: the only foreign pointer read; the LLVM constant's contract.
        unsafe { std::slice::from_raw_parts(ptr, len) }
    };
    // The literal already seen by this thread (isolate): by the constant's
    // address, checked against the stored bytes — another constant may occupy the
    // same address after a JIT reload. Without this, every evaluation converted
    // the bytes to UTF-16 and searched the heap's literal map.
    let key = (ptr as usize, bytes.len());
    let found_item = LITERALS_BY_ADDRESS.with(|m| m.borrow().get(&key).and_then(|(b, h)| (**b == *bytes).then_some(*h)));
    if let Some(h) = found_item {
        return h;
    }
    let t = Text::from_wtf8(bytes);
    let h = HEAP.with(|heap| heap.borrow_mut().string_literal(t.view()));
    LITERALS_BY_ADDRESS.with(|m| m.borrow_mut().insert(key, (bytes.into(), h)));
    h
}

thread_local! {
    /// `dartforge_string_new`: (address, length) of the constant → the bytes and
    /// the literal's handle (permanent in this thread's heap).
    static LITERALS_BY_ADDRESS: RefCell<crate::hash::HashMap<(usize, usize), (Box<[u8]>, i64)>> =
        RefCell::new(crate::hash::HashMap::default());
}

/// The digit pairs from 00 to 99, to write two digits per division.
const DECIMAL_PAIRS: &[u8; 200] = b"\
0001020304050607080910111213141516171819\
2021222324252627282930313233343536373839\
4041424344454647484950515253545556575859\
6061626364656667686970717273747576777879\
8081828384858687888990919293949596979899";

/// The powers of ten that fit in a `u64`.
const POWERS_OF_TEN: [u64; 20] = {
    let mut p = [1u64; 20];
    let mut i = 1;
    while i < 20 {
        p[i] = p[i - 1] * 10;
        i += 1;
    }
    p
};

/// How many characters `v.toString()` has (the sign included): what
/// [`write_decimal`] writes, without writing it.
pub(crate) fn decimal_len(v: i64) -> usize {
    let n = v.unsigned_abs();
    let mut digits = 1;
    while digits < POWERS_OF_TEN.len() && n >= POWERS_OF_TEN[digits] {
        digits += 1;
    }
    digits + usize::from(v < 0)
}

/// The decimal digits of `v` (with the sign) at the end of `buf`: the VM's
/// `int.toString()`, without the intermediate string. Returns the first byte's index.
pub(crate) fn write_decimal(buf: &mut [u8; 20], v: i64) -> usize {
    let mut i = buf.len();
    let mut n = v.unsigned_abs();
    while n >= 100 {
        let pair = (n % 100) as usize * 2;
        n /= 100;
        i -= 2;
        buf[i..i + 2].copy_from_slice(&DECIMAL_PAIRS[pair..pair + 2]);
    }
    if n >= 10 {
        let pair = n as usize * 2;
        i -= 2;
        buf[i..i + 2].copy_from_slice(&DECIMAL_PAIRS[pair..pair + 2]);
    } else {
        i -= 1;
        buf[i] = b'0' + n as u8;
    }
    if v < 0 {
        i -= 1;
        buf[i] = b'-';
    }
    i
}

/// The interpolation `'a$b c'` with unboxed `int` parts (`JuntarTextos`,
/// `llvm/textos_ir.rs`): `n` (kind, bits) pairs — kind 0, the `Ref` of a string;
/// 1, an `int`, written in decimal directly into the result. One measurement
/// (length and form), one allocation and the copies of the blocks' units, with no
/// intermediate copy.
///
/// # Safety
/// `part_list` points to `2·n` readable words (the emitter's temporary); the
/// strings are rooted by the emitter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_string_join_typed(part_list: *const i64, n: i64) -> i64 {
    // SAFETY: the contract above.
    unsafe { join_typed(part_list, n, false) }
}

/// The interpolation of `dartforge_string_join_typed` under ARC: hands over a
/// code owner of the result.
///
/// The same (kind, bits) pairs. The strings stay rooted in an owning frame during
/// the allocation, which may collect; the result copies the units and does not
/// retain the parts. The caller transfers the owner or calls release exactly once.
/// Neither reenters Dart nor leaves a pending exception.
///
/// # Safety
/// `part_list` points to `2·n` readable words; each part of kind 0 is a live
/// string.
///
/// # Panics
/// Negative `n`, a kind other than 0/1 or a kind-0 part that is not a string;
/// aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let seven = dartforge_to_string_i64(7);
/// let part_list = [0, seven, 1, -42];
/// let text = unsafe { dartforge_arc_string_join_owned_v1(part_list.as_ptr(), 2) };
/// assert_eq!(dartforge_arc_observe_heap_v1(text), 3);
/// assert_eq!(dartforge_string_equals(text, dartforge_to_string_i64(-742)), 0);
/// dartforge_arc_release(text);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observe_heap_v1(text), 0);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_string_join_owned_v1(part_list: *const i64, n: i64) -> i64 {
    // SAFETY: the contract above.
    unsafe { join_typed(part_list, n, true) }
}

/// The body of both joins; `owned` roots the parts and hands over the owner.
unsafe fn join_typed(part_list: *const i64, n: i64, owned: bool) -> i64 {
    let n = usize::try_from(n).expect("invalid number of parts");
    // SAFETY: guaranteed by the caller.
    let part_list: &[i64] = if n == 0 { &[] } else { unsafe { std::slice::from_raw_parts(part_list, 2 * n) } };
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let frame = owned.then(|| {
            assert!(part_list.chunks_exact(2).all(|p| matches!(p[0], 0 | 1)), "invalid part kind");
            let text: Vec<i64> = part_list.chunks_exact(2).filter(|p| p[0] == 0).map(|p| p[1]).collect();
            let frame = heap.push_owning_frame(text.len());
            for (k, &t) in text.iter().enumerate() { heap.set_root(frame, k, t); }
            frame
        });
        let (mut total, mut one) = (0usize, true);
        for p in part_list.chunks_exact(2) {
            if p[0] == 1 {
                total += decimal_len(p[1]);
            } else {
                let t = heap.text(p[1]).expect("compiler bug: string expected in the interpolation");
                total += t.len();
                one &= t.fits_in_one_byte();
            }
        }
        let two = !one;
        let (r, units) = heap.new_string_units(total, two);
        let mut buf = [0u8; 20];
        let mut pos = 0;
        for p in part_list.chunks_exact(2) {
            let t = if p[0] == 1 {
                let i = write_decimal(&mut buf, p[1]);
                crate::text::TextRef::OneByte(&buf[i..])
            } else {
                // Rooted by the emitter (or by `frame`): read again after the
                // allocation, which may have collected.
                heap.text(p[1]).expect("compiler bug: string expected in the interpolation")
            };
            // SAFETY: `pos + t.len() ≤ total` units of room from `units`, in the
            // form chosen for all the parts; the result is another block than any
            // part, and `buf` is on the stack.
            unsafe { crate::text::write_units(units.add(pos * crate::text::unit_size(two)), two, t) };
            pos += t.len();
        }
        if let Some(frame) = frame {
            heap.retain_code_owner(r);
            heap.pop_frame(frame);
        }
        r
    })
}

/// Concatenates non-null strings; the arguments must be rooted by the emitter.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_concat(a: i64, b: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().join_strings(&[a, b]))
}

/// A growable list of integers (no allocation in the middle: the compact form).
fn int_list(value_list: Vec<i64>) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let n = value_list.len();
        let l = heap.new_growable(n, n, crate::lists::Element::Int);
        for (i, v) in value_list.into_iter().enumerate() {
            heap.list_set(l, i, crate::heap::Value::Int(v));
        }
        l
    })
}

/// The search pattern: a string or a `RegExp` (the compiled program, cid 16).
enum Pattern {
    Text(Text),
    RegExp(i64),
}

fn pattern_of(handle: i64) -> Option<Pattern> {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        if let Some(t) = heap.text(handle) {
            return Some(Pattern::Text(t.to_string_fn()));
        }
        (crate::layout::is_object(handle) && heap.class(handle) == crate::layout::cid::REGEXP_PROGRAM)
            .then_some(Pattern::RegExp(handle))
    })
}

/// Next occurrence of the pattern from `since`: (start, length).
fn next_item(target: &Text, pattern: &Pattern, since: usize) -> Option<(usize, usize)> {
    match pattern {
        Pattern::Text(p) => target.search_for(p, since).map(|i| (i, p.len())),
        Pattern::RegExp(p) => {
            let caps = regexp_match_at(*p, &target.to_vec(), since, false)?;
            Some((caps[0] as usize, (caps[1] - caps[0]) as usize))
        }
    }
}

/// `indexOf(pattern, [start])` in units; -1 if there is none.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_index_of(handle: i64, pat_handle: i64, start: i64) -> i64 {
    let t = copied_string(handle);
    let Some(p) = pattern_of(pat_handle) else { return -1 };
    if start < 0 || start as usize > t.len() {
        throw_range_err(start, 0, t.len() as i64, "start");
        return 0;
    }
    next_item(&t, &p, start as usize).map_or(-1, |(i, _)| i as i64)
}

/// A part of a list of pieces: the `bool` before the text (the match mark of
/// `splitMapJoin`) and the text.
type Chunk = (Option<bool>, Text);

/// A growable list with the pieces, in order, allocating each one with the list
/// already rooted (G6: a collection in the middle sees the previous pieces through
/// the list).
fn chunk_list(items: Vec<Chunk>) -> i64 {
    let list = HEAP.with(|h| h.borrow_mut().new_growable(0, 0, crate::lists::Element::General));
    with_roots(&[list], || {
        for (before, text) in items {
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                if let Some(b) = before {
                    h.list_push(list, crate::heap::Value::Bool(b));
                }
                let x = h.allocate_string(text.view());
                h.list_push(list, crate::heap::Value::Ref(x));
            });
        }
    });
    list
}

/// `pattern.allMatches(target)`: (start, length) of each match, without overlap;
/// after an empty match the search restarts one unit ahead
/// (`_StringAllMatchesIterator.moveNext`, "Empty match, don't start at same
/// location again"). An empty string pattern matches at `0..=length`.
fn occurrences(target: &Text, pattern: &Pattern) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    let mut i = 0;
    while i <= target.len() {
        let Some((a, n)) = next_item(target, pattern, i) else { break };
        v.push((a, n));
        i = if n == 0 { a + 1 } else { a + n };
    }
    v
}

fn empty_pattern(p: &Pattern) -> bool {
    matches!(p, Pattern::Text(t) if t.is_empty())
}

/// `_StringBase`'s `padLeft`/`padRight`: `delta = width - length`; with nothing to
/// do it returns the receiver itself; the padding is repeated `delta` times whole
/// (it is not cut).
fn fill(handle: i64, width: i64, pad_handle: i64, left: bool) -> i64 {
    let t = copied_string(handle);
    let pad = string_copy(pad_handle).unwrap_or_else(|| Text::from_rust_str(" "));
    let delta = width - t.len() as i64;
    if delta <= 0 || pad.is_empty() {
        return handle;
    }
    let mut out = StringMut::new();
    if !left {
        out.push_string(&t);
    }
    for _ in 0..delta {
        out.push_string(&pad);
    }
    if left {
        out.push_string(&t);
    }
    allocate_string(out.end())
}

/// The VM's `toUpperCase`/`toLowerCase` (`String::Transform`, `object.cc`):
/// **simple** mapping, one code point to one code point (`CaseMapping`), so
/// `'ß'.toUpperCase()` is `'ß'` (JS gives `SS`). Rust's full mapping is only used
/// when it gives a single character. Nothing to change returns the receiver
/// itself.
fn transform(handle: i64, uppercase: bool) -> i64 {
    let new = with_string(handle, |t| {
        // Pure ASCII: the single-pass path, without the Unicode table.
        if let crate::text::TextRef::OneByte(b) = t
            && b.is_ascii()
        {
            let changes_flag = if uppercase { b.iter().any(u8::is_ascii_lowercase) } else { b.iter().any(u8::is_ascii_uppercase) };
            if !changes_flag {
                return None;
            }
            let v: Vec<u8> = b.iter().map(|c| if uppercase { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() }).collect();
            return Some(Text::OneByte(v));
        }
        let mut u = Vec::with_capacity(t.len());
        let mut changed = false;
        for p in t.points() {
            let new = char::from_u32(p).map_or(p, |c| {
                let mut it: Box<dyn Iterator<Item = char>> =
                    if uppercase { Box::new(c.to_uppercase()) } else { Box::new(c.to_lowercase()) };
                match (it.next(), it.next()) {
                    (Some(x), None) => x as u32,
                    _ => p,
                }
            });
            changed |= new != p;
            crate::text::push_point(&mut u, new);
        }
        changed.then(|| Text::from_units(u))
    });
    match new {
        Some(t) => allocate_string(t),
        None => handle,
    }
}

/// Repeats a string `times` times (`'a' * 3`); zero or negative gives `''`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_repeat(handle: i64, times: i64) -> i64 {
    let times = usize::try_from(times).unwrap_or(0);
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let (n, one) = heap.text(handle).map_or((0, true), |t| (t.len(), t.fits_in_one_byte()));
        let total = n.checked_mul(times).expect("string too large");
        let (r, units) = heap.new_string_units(total, !one);
        if n > 0 {
            // Rooted by the emitter: read again after the allocation.
            let t = heap.text(handle).expect("compiler bug: string expected");
            for k in 0..times {
                // SAFETY: `k·n + n ≤ total` units of room from `units`, in the
                // source's form; `r` is another block than `handle`.
                unsafe { crate::text::write_units(units.add(k * n * crate::text::unit_size(!one)), !one, t) };
            }
        }
        r
    })
}

/// `_StringBase._isTwoByteWhitespace` (`string_patch.dart`), per unit.
fn is_dart_whitespace(u: u16) -> bool {
    if u <= 32 {
        return u == 32 || (9..=13).contains(&u);
    }
    if u < 0x85 {
        return false;
    }
    if u == 0x85 || u == 0xA0 {
        return true;
    }
    if u <= 0x200A {
        u == 0x1680 || 0x2000 <= u
    } else {
        matches!(u, 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000 | 0xFEFF)
    }
}

/// The range without Dart whitespace at the requested ends.
fn trimmed_range(t: crate::text::TextRef<'_>, left: bool, right: bool) -> (usize, usize) {
    let n = t.len();
    let mut a = 0;
    if left {
        while a < n && is_dart_whitespace(t.unit(a)) {
            a += 1;
        }
    }
    let mut b = n;
    if right {
        while b > a && is_dart_whitespace(t.unit(b - 1)) {
            b -= 1;
        }
    }
    (a, b)
}

/// `trim`/`trimLeft`/`trimRight`: nothing to remove returns the receiver itself.
fn trim_free(handle: i64, left: bool, right: bool) -> i64 {
    let (n, (a, b)) = with_string(handle, |t| (t.len(), trimmed_range(t, left, right)));
    if a == 0 && b == n {
        return handle;
    }
    HEAP.with(|heap| heap.borrow_mut().string_slice(handle, a, b))
}

/// The trimmed text of a string, for the `parse` functions (no Dart whitespace at the ends).
fn string_to_parse(handle: i64) -> String {
    with_string(handle, |t| {
        let (a, b) = trimmed_range(t, true, true);
        t.piece(a, b).to_dart_string()
    })
}

#[cfg(test)]
mod tests_owned_join {
    use super::*;

    #[test]
    fn owned_join_roots_parts_copies_units_and_releases_on_release() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            // The parts lose the test's root before the call (no collection
            // between the two): during the allocation, only the factory protects them.
            let frame = HEAP.with(|h| h.borrow_mut().push_owning_frame(2));
            let one = HEAP.with(|h| h.borrow_mut().allocate_str("part without owner"));
            HEAP.with(|h| h.borrow_mut().set_root(frame, 0, one));
            let two = allocate_string(Text::from_units(vec![0xD800, 0x41]));
            HEAP.with(|h| h.borrow_mut().pop_frame(frame));
            let part_list = [0, one, 1, i64::MIN, 0, two, 1, 0];
            let r = unsafe { dartforge_arc_string_join_owned_v1(part_list.as_ptr(), 4) };
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(r), 3);
            let expected: Vec<u16> = "part without owner-9223372036854775808".encode_utf16()
                .chain([0xD800, 0x41, u16::from(b'0')]).collect();
            HEAP.with(|h| {
                let heap = h.borrow();
                let t = heap.text(r).unwrap();
                assert_eq!((0..t.len()).map(|i| t.unit(i)).collect::<Vec<_>>(), expected);
            });
            dartforge_arc_release(r);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(r), 0);
            let empty = unsafe { dartforge_arc_string_join_owned_v1(std::ptr::null(), 0) };
            assert_eq!(dartforge_arc_observe_heap_v1(empty), 3);
            dartforge_arc_release(empty);
            HEAP.with(|h| { h.replace(previous); });
        }
    }
}

#[cfg(test)]
mod tests_decimal_text {
    use super::*;

    /// `int.toString` (and the interpolation's digits) through the pairs table:
    /// the text of every magnitude, `i64::MIN` included, and its length.
    #[test]
    fn to_string_i64_writes_the_decimal_text_of_every_magnitude() {
        let mut values = vec![0, 1, -1, 9, 10, 99, 100, 101, i64::MAX, i64::MIN, i64::MIN + 1, i64::MAX - 1];
        let mut p = 1i64;
        for _ in 0..18 {
            p *= 10;
            values.extend([p - 1, p, p + 1, -(p - 1), -p, -(p + 1)]);
        }
        for v in values {
            let expected = v.to_string();
            assert_eq!(decimal_len(v), expected.len(), "{v}");
            let mut buf = [0u8; 20];
            let i = write_decimal(&mut buf, v);
            assert_eq!(std::str::from_utf8(&buf[i..]).unwrap(), expected, "{v}");
            let h = dartforge_to_string_i64(v);
            HEAP.with(|heap| assert_eq!(heap.borrow().text(h).unwrap(), expected.as_str(), "{v}"));
            let part_list = [1, v, 0, h, 1, v.wrapping_neg()];
            // SAFETY: three readable pairs; `h` is a live string.
            let joined = unsafe { dartforge_string_join_typed(part_list.as_ptr(), 3) };
            let expected = format!("{expected}{expected}{}", v.wrapping_neg());
            HEAP.with(|heap| assert_eq!(heap.borrow().text(joined).unwrap(), expected.as_str(), "{v}"));
        }
    }
}
